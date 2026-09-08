# YellowTV — catálogo local em SQLite, foco em filmes e séries

Data: 2026-09-08
Status: aprovado (abordagem A)

## Problema

App hoje lê JSON pré-gerado por `scripts/update-channel.ts` e
`scripts/update-vod.ts`, distribuído junto com projeto. Três consequências:

1. App carrega dado de terceiros que não é dele.
2. Catálogo inteiro vive em memória (`filmes.json` ~5 MB), toda busca varre `Vec`.
3. Canais ao vivo ocupam rota raiz, mas filmes e séries são o produto.

Progresso e favoritos vivem em `localStorage`: "já assistiu" é por-WebView, sem
posição de retomada, some se storage limpo.

## Objetivo

Usuário adiciona própria lista M3U. App parseia, normaliza, guarda tudo em
SQLite local, incluindo progresso por filme e por episódio. Navegação vira
filmes e séries no formato Stremio, canais como aba terciária.

## Decisões tomadas

| Decisão | Escolha |
|---|---|
| Data layer | `rusqlite` com feature `bundled` + FTS5, parser M3U em Rust |
| Canais | aba terciária no fim da nav |
| Progresso | posição + duração + flag de concluído + próximo episódio |
| Listas M3U | várias simultâneas, catálogo unificado com dedup |
| Metadata TMDB | sob demanda, cache permanente na tabela `meta` |
| Chave TMDB | colada no onboarding, com "pular por agora"; env var como fallback |

Descartado: `sqlx` (macro-build pesado, async não ganha nada em SQLite local) e
`tauri-plugin-sql` com lógica no front (jogaria parse de 100k linhas na WebView,
apagaria fronteira existente).

## Schema

Banco em `app_data/yellowtv.db`. Migrações versionadas via `PRAGMA user_version`.

```sql
sources(
  id INTEGER PRIMARY KEY, url TEXT NOT NULL UNIQUE, label TEXT NOT NULL,
  kind TEXT NOT NULL,              -- 'url' | 'file'
  enabled INTEGER NOT NULL DEFAULT 1,
  added_at INTEGER NOT NULL, last_sync_at INTEGER, item_count INTEGER NOT NULL DEFAULT 0
)

items(
  id TEXT PRIMARY KEY,             -- hash estável de (kind, title_norm, year)
  kind TEXT NOT NULL,              -- 'movie' | 'series' | 'channel'
  title TEXT NOT NULL, title_norm TEXT NOT NULL,
  year INTEGER, logo TEXT, group_name TEXT, created_at INTEGER NOT NULL
)

episodes(
  id TEXT PRIMARY KEY,             -- hash de (series_id, season, episode)
  series_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  season INTEGER NOT NULL, episode INTEGER NOT NULL, title TEXT,
  UNIQUE(series_id, season, episode)
)

streams(
  id INTEGER PRIMARY KEY,
  owner_id TEXT NOT NULL,          -- items.id ou episodes.id
  owner_kind TEXT NOT NULL,        -- 'item' | 'episode'
  source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
  url TEXT NOT NULL, quality TEXT, channel_number INTEGER,
  UNIQUE(owner_id, url)
)

progress(
  owner_id TEXT NOT NULL, owner_kind TEXT NOT NULL,
  position_secs REAL NOT NULL DEFAULT 0, duration_secs REAL,
  completed INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL,
  PRIMARY KEY(owner_id, owner_kind)
)

favorites(owner_id TEXT PRIMARY KEY, added_at INTEGER NOT NULL)

meta(item_id TEXT PRIMARY KEY, payload TEXT NOT NULL, fetched_at INTEGER NOT NULL)

settings(key TEXT PRIMARY KEY, value TEXT NOT NULL)

items_fts  -- FTS5 external content sobre items(title_norm), sincronizada por triggers
```

Índices: `items(kind, group_name)`, `items(kind, created_at)`,
`episodes(series_id, season, episode)`, `streams(owner_id)`,
`progress(updated_at)`.

### Por que `items` é separado de `streams`

`items.id` vem do título normalizado e do ano, não da URL. Mesmo filme de duas
listas = **um** item, dois streams. Três ganhos de graça:

- dedup entre listas;
- progresso sobrevive a re-sync e troca de lista, chave não depende da URL;
- lista de fontes na tela de detalhe quando título tem mais de um stream —
  modelo Stremio, metadata e fonte de stream são coisas distintas.

`progress` sem foreign key para `items`/`episodes`: usuário remove lista e
readiciona, histórico continua casando pelo hash. Limpeza de órfãos explícita,
não em cascata.

### Regra de "assistido"

`completed = 1` quando usuário marca manualmente, ou quando
`position_secs >= 0.92 * duration_secs` no momento do report. Progresso abaixo
de 30 segundos não cria linha — evita poluir "Continuar assistindo" com cliques
acidentais.

`next_episode(series_id)`: primeiro episódio ordenado por `(season, episode)`
sem `completed = 1`; todos concluídos devolve `None`.

## Rust

| Módulo | Responsabilidade |
|---|---|
| `db.rs` | abre conexão, aplica migrações, expõe `Mutex<Connection>` no `AppState` |
| `m3u.rs` | parser de `#EXTINF` em streaming; classifica canal/filme/série; extrai ano, temporada, episódio, tokens de qualidade |
| `import.rs` | `add_source`, `sync_source`, `remove_source`, `list_sources`; uma transação por import; emite `import:progress` |
| `catalog.rs` | reescrito: paginação, grupos, filtros, busca FTS em SQL |
| `library.rs` | `set_progress`, `mark_watched`, `continue_watching`, `next_episode`, favoritos |
| `meta.rs` | cache migra de arquivos para tabela `meta`; chave TMDB vem de `settings`, `TMDB_API_KEY` como fallback |
| `proxy.rs` | intocado |

Sai do código: `resolve_data_dir`, toda leitura de JSON, comando `data_status`,
`scripts/update-channel.ts`, `scripts/update-vod.ts` e `data/*.json`.

### Classificação no parser

Ordem de decisão, primeira que casa vence:

1. `SxxExx`, `xXy`, `temporada N episodio M` no título → série (temporada e
   episódio extraídos; título da série é o que sobra).
2. `group-title` casando canal ao vivo, ou presença de `tvg-id`/`tvg-chno` →
   canal.
3. Resto → filme; ano extraído de `(2019)` ou ` 2019` no fim do título.

Normalização de título: minúsculas, NFD sem diacríticos, tokens de qualidade
removidos (`4K`, `FHD`, `H265`, `ALT 2`…), espaços colapsados. Alimenta
`title_norm` e, por consequência, hash do id.

### Comandos Tauri

```
list_sources() -> Vec<Source>
add_source(url_or_path, kind) -> Source          // dispara import
sync_source(id) -> Source
remove_source(id)
catalog_page(kind, query, group, page, unwatched_only) -> CatalogPage
catalog_item(kind, id) -> ItemWithRelated        // inclui streams e progresso
series_episodes(id) -> SeriesEpisodes            // inclui progresso por episódio
board() -> Vec<Row>                              // linhas do Início
continue_watching(limit) -> Vec<ProgressEntry>
set_progress(owner_id, owner_kind, position, duration)
mark_watched(owner_id, owner_kind, completed)
next_episode(series_id) -> Option<Episode>
toggle_favorite(owner_id) -> bool
title_meta(kind, title, year) -> Meta
get_setting(key) / set_setting(key, value)
stream_url(url) -> String
```

## Frontend

Nav: **Início · Filmes · Séries · Biblioteca · Canais**.

- **`/setup`** — onboarding. Colar URL M3U ou escolher arquivo, chave TMDB
  opcional com "pular por agora", barra de progresso do import alimentada pelo
  evento `import:progress`. Enquanto `list_sources()` vazio, qualquer rota
  redireciona para cá.
- **`/` Início** — board formato Stremio: linhas horizontais de pôster.
  *Continuar assistindo* primeiro (com barra de progresso), depois *Adicionados
  recentemente*, depois linhas por gênero de filmes e séries. Sem sinopse na
  home; pôster manda.
- **`/filmes`, `/series`** — `CatalogBrowser` atual, mais filtro "não
  assistidos" e badges de progresso nos pôsteres.
- **`/biblioteca`** — favoritos e histórico, com marcar/desmarcar visto.
- **`/canais`** — lista atual, sem mudança estrutural.
- **Detalhe** — backdrop grande com fade para o fundo, ficha, botão primário que
  vira *Retomar em 18min* quando há progresso, lista de fontes quando título tem
  mais de um stream, episódios com check de concluído e barra parcial.
- **Pôster** ganha dois estados novos: barra âmbar de progresso no rodapé, e
  escurecido com check quando concluído.
- `src/lib/store.ts` (localStorage) removido; progresso e favoritos vêm do
  SQLite via `invoke`.
- `Player` reporta posição com throttle de 5s, mais report em `pause` e no
  cleanup; ao terminar chama `next_episode` e navega.

Design visual (paleta âmbar, motion tokens, ícones SVG, reveals) já no lugar,
reaproveitado como está.

## Erros

- **M3U inacessível ou vazia**: import falha inteiro, transação faz rollback,
  `/setup` mostra causa (HTTP, timeout, zero entradas reconhecidas) com botão de
  tentar novamente.
- **Linha malformada**: descartada, contada, total de descartes aparece no resumo
  do import. Uma linha ruim não derruba import.
- **TMDB sem chave ou fora do ar**: `Meta::empty` com motivo, como hoje. Catálogo
  continua usando capas do próprio M3U.
- **Stream morto**: `Player` já trata; nada muda.
- **Banco corrompido**: open falha, app avisa e oferece recriar. Sem migração
  automática de banco ilegível.

## Testes

Rust, em `src-tauri/tests`:

- parser: fixtures de `#EXTINF` reais cobrindo filme com ano, filme sem ano,
  série `S01E02`, série `1x02`, canal com `tvg-chno`, linha sem vírgula, linha
  sem URL, atributos fora de ordem;
- estabilidade do hash: mesmo título com qualidade diferente colapsa no mesmo
  `items.id`;
- dedup: duas sources com mesmo filme geram um item e dois streams;
- progresso: upsert, limiar de 92%, corte de 30 segundos;
- `next_episode`: temporadas inseridas fora de ordem, série toda concluída;
- remoção de source: streams somem, `progress` sobrevive.

Front: `tsc --noEmit` e `vite build`.

## Ordem de execução

1. `db.rs`, migrações, testes de schema
2. `m3u.rs` e testes de parsing
3. `import.rs`, comandos, evento de progresso
4. `catalog.rs` reescrito em SQL
5. `library.rs` (progresso e favoritos)
6. `/setup` e nav nova
7. Board do Início e estados de progresso no pôster
8. `Player` reportando posição e próximo episódio
9. Limpeza: JSON, scripts, `store.ts`, comando `data_status`

## Fora de escopo

Sincronização entre dispositivos, contas, Xtream API como fonte alternativa,
legendas externas, transcodificação e download offline.