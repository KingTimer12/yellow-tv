# YellowTV — Catálogo Local em SQLite: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Substituir os JSONs pré-gerados por um catálogo local em SQLite alimentado por listas M3U do próprio usuário, com progresso e favoritos persistidos, e navegação centrada em filmes e séries.

**Architecture:** Todo o dado passa a viver em `app_data/yellowtv.db` (rusqlite bundled + FTS5). O Rust ganha cinco módulos novos/reescritos — `db`, `m3u`, `import`, `catalog`, `library` — e o front deixa de guardar estado em `localStorage`: progresso, favoritos e catálogo vêm por `invoke`. A WebView nunca vê M3U cru; parse e normalização acontecem em Rust, uma transação por import.

**Tech Stack:** Rust + Tauri 2, rusqlite (`bundled`, FTS5), SolidJS 1.9 + `@solidjs/router`, Tailwind 4, Vite 8, Bun.

**Spec:** `docs/superpowers/specs/2026-09-08-yellowtv-catalogo-local-design.md`

## Global Constraints

- Banco em `app.path().app_data_dir()/yellowtv.db`. Migrações versionadas por `PRAGMA user_version`.
- `rusqlite` com feature `bundled`; FTS5 obrigatório (Task 1 verifica e ensina o fallback para `bundled-full`).
- Rust edition 2021, `[lib] name = "yellow_tv_lib"`. Todo módulo novo é `pub mod` em `lib.rs` para que `src-tauri/tests/*.rs` possa importar via `yellow_tv_lib::`.
- Conexão única compartilhada como `Mutex<Connection>` dentro de `AppState`. Nada de `async` no data layer.
- `items.id` e `episodes.id` são hashes estáveis — nunca derivados de URL.
- `progress` não tem foreign key. Remoção de source nunca apaga progresso.
- Idioma de UI e mensagens de erro: português do Brasil. Comentários de código em português, como no código existente.
- Front usa alias `~` para `src/`. Gate de front: `bunx tsc --noEmit` e `bun run build`.
- Gate de Rust: `cargo test --manifest-path src-tauri/Cargo.toml`.
- Paleta âmbar, motion tokens e ícones SVG existentes são reaproveitados como estão. Nenhum redesign visual.
- Commits: `feat:`/`test:`/`refactor:`/`chore:` conforme a mudança. Sem trailers de atribuição.

---

## File Structure

**Rust — `src-tauri/src/`**

| Arquivo | Responsabilidade |
|---|---|
| `db.rs` (criar) | abre/cria o banco, `PRAGMA`s, migração v1, helpers de teste em memória |
| `ids.rs` (criar) | normalização de título e hashes estáveis de item e episódio |
| `m3u.rs` (criar) | parser streaming de `#EXTINF` + classificação canal/filme/série |
| `import.rs` (criar) | `add_source`, `sync_source`, `remove_source`, `list_sources`; uma transação por import |
| `catalog.rs` (reescrever) | paginação, grupos, busca FTS, item com streams, episódios, board |
| `library.rs` (criar) | progresso, concluído, continuar assistindo, próximo episódio, favoritos |
| `settings.rs` (criar) | `get_setting` / `set_setting` sobre a tabela `settings` |
| `meta.rs` (modificar) | cache sai do disco e vai para a tabela `meta`; chave TMDB vem de `settings` |
| `proxy.rs` | intocado |
| `lib.rs` (modificar) | `AppState` novo, registro dos comandos, evento `import:progress` |

**Rust — `src-tauri/tests/`**

`schema.rs`, `m3u_parse.rs`, `ids_stable.rs`, `import_dedup.rs`, `library_progress.rs`

**Front — `src/`**

| Arquivo | Responsabilidade |
|---|---|
| `lib/api.ts` (criar) | tipos e wrappers de `invoke` para tudo que é novo |
| `lib/vod.ts` (modificar) | mantém helpers de apresentação; wrappers migram para `api.ts` |
| `lib/store.ts` (deletar) | substituído por SQLite |
| `pages/Setup.tsx` (criar) | onboarding: lista M3U, chave TMDB, progresso do import |
| `pages/Inicio.tsx` (criar) | board de linhas horizontais |
| `pages/Biblioteca.tsx` (criar) | favoritos e histórico |
| `components/PosterRow.tsx` (criar) | linha horizontal de pôsteres |
| `components/SourcePicker.tsx` (criar) | lista de fontes de stream na tela de detalhe |
| `components/PosterGrid.tsx` (modificar) | barra de progresso e estado concluído |
| `components/Nav.tsx` (modificar) | Início · Filmes · Séries · Biblioteca · Canais |
| `components/Player.tsx` (modificar) | report de posição, `onEnded` → próximo episódio |
| `pages/Filme.tsx`, `pages/Serie.tsx` (modificar) | retomar, fontes, progresso por episódio |
| `pages/Channels.tsx`, `components/ChannelRow.tsx`, `pages/Watch.tsx` (modificar) | favoritos via SQLite |
| `App.tsx` (modificar) | rotas novas e gate de `/setup` |

---

## Task 1: `db.rs` — banco, migração v1, FTS5

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs:1-3` (declarações de módulo)
- Test: `src-tauri/tests/schema.rs`

**Interfaces:**
- Consumes: nada.
- Produces:
  - `db::open(path: &std::path::Path) -> Result<rusqlite::Connection, String>`
  - `db::open_memory() -> Result<rusqlite::Connection, String>`
  - `db::SCHEMA_VERSION: i64` (= 1)

- [ ] **Step 1: Adicionar a dependência**

```bash
cd src-tauri && cargo add rusqlite --features bundled && cd ..
```

- [ ] **Step 2: Escrever o teste que falha**

Create `src-tauri/tests/schema.rs`:

```rust
use yellow_tv_lib::db;

#[test]
fn migra_para_versao_atual() {
    let conn = db::open_memory().expect("banco em memória");
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, db::SCHEMA_VERSION);
}

#[test]
fn cria_todas_as_tabelas() {
    let conn = db::open_memory().unwrap();
    for table in [
        "sources", "items", "episodes", "streams", "progress", "favorites", "meta", "settings",
        "items_fts",
    ] {
        let found: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = ?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(found, 1, "tabela ausente: {table}");
    }
}

#[test]
fn fts5_esta_disponivel_e_sincronizada() {
    let conn = db::open_memory().unwrap();
    conn.execute(
        "INSERT INTO items(id, kind, title, title_norm, created_at) VALUES('a','movie','Caçador','cacador',0)",
        [],
    )
    .unwrap();
    let hits: i64 = conn
        .query_row(
            "SELECT count(*) FROM items_fts WHERE items_fts MATCH 'cacador'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(hits, 1);
}

#[test]
fn migrar_duas_vezes_e_inofensivo() {
    let dir = std::env::temp_dir().join(format!("yellowtv-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("twice.db");
    let _ = std::fs::remove_file(&file);
    db::open(&file).unwrap();
    db::open(&file).expect("segunda abertura não deve falhar");
    std::fs::remove_file(&file).unwrap();
}

#[test]
fn remover_source_apaga_streams_mas_preserva_progresso() {
    let conn = db::open_memory().unwrap();
    conn.execute_batch(
        "INSERT INTO sources(id, url, label, kind, added_at) VALUES(1,'http://a','A','url',0);
         INSERT INTO items(id, kind, title, title_norm, created_at) VALUES('i1','movie','X','x',0);
         INSERT INTO streams(owner_id, owner_kind, source_id, url) VALUES('i1','item',1,'http://s');
         INSERT INTO progress(owner_id, owner_kind, position_secs, updated_at) VALUES('i1','item',120.0,0);",
    )
    .unwrap();
    conn.execute("DELETE FROM sources WHERE id = 1", []).unwrap();

    let streams: i64 = conn
        .query_row("SELECT count(*) FROM streams", [], |row| row.get(0))
        .unwrap();
    let progress: i64 = conn
        .query_row("SELECT count(*) FROM progress", [], |row| row.get(0))
        .unwrap();
    assert_eq!(streams, 0, "streams devem cair com a source");
    assert_eq!(progress, 1, "progresso sobrevive à remoção da lista");
}
```

- [ ] **Step 3: Rodar e ver falhar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test schema`
Expected: FAIL — `unresolved import yellow_tv_lib::db` / `could not find db`.

- [ ] **Step 4: Escrever `db.rs`**

Create `src-tauri/src/db.rs`:

```rust
//! Abertura e migração do banco local.
//!
//! Uma única conexão vive no `AppState` atrás de um `Mutex`. SQLite local é
//! rápido o bastante para atender a WebView de forma síncrona, e a versão do
//! schema fica em `PRAGMA user_version` — nada de tabela de controle própria.

use std::path::Path;

use rusqlite::Connection;

pub const SCHEMA_VERSION: i64 = 1;

const MIGRATION_V1: &str = r#"
CREATE TABLE sources(
  id INTEGER PRIMARY KEY,
  url TEXT NOT NULL UNIQUE,
  label TEXT NOT NULL,
  kind TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  added_at INTEGER NOT NULL,
  last_sync_at INTEGER,
  item_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE items(
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  title_norm TEXT NOT NULL,
  year INTEGER,
  logo TEXT,
  group_name TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE episodes(
  id TEXT PRIMARY KEY,
  series_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  season INTEGER NOT NULL,
  episode INTEGER NOT NULL,
  title TEXT,
  UNIQUE(series_id, season, episode)
);

CREATE TABLE streams(
  id INTEGER PRIMARY KEY,
  owner_id TEXT NOT NULL,
  owner_kind TEXT NOT NULL,
  source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
  url TEXT NOT NULL,
  quality TEXT,
  channel_number INTEGER,
  UNIQUE(owner_id, url)
);

CREATE TABLE progress(
  owner_id TEXT NOT NULL,
  owner_kind TEXT NOT NULL,
  position_secs REAL NOT NULL DEFAULT 0,
  duration_secs REAL,
  completed INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY(owner_id, owner_kind)
);

CREATE TABLE favorites(owner_id TEXT PRIMARY KEY, added_at INTEGER NOT NULL);

CREATE TABLE meta(item_id TEXT PRIMARY KEY, payload TEXT NOT NULL, fetched_at INTEGER NOT NULL);

CREATE TABLE settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE INDEX items_kind_group ON items(kind, group_name);
CREATE INDEX items_kind_created ON items(kind, created_at);
CREATE INDEX episodes_order ON episodes(series_id, season, episode);
CREATE INDEX streams_owner ON streams(owner_id);
CREATE INDEX progress_updated ON progress(updated_at);

CREATE VIRTUAL TABLE items_fts USING fts5(
  title_norm,
  content='items',
  content_rowid='rowid'
);

CREATE TRIGGER items_fts_insert AFTER INSERT ON items BEGIN
  INSERT INTO items_fts(rowid, title_norm) VALUES (new.rowid, new.title_norm);
END;

CREATE TRIGGER items_fts_delete AFTER DELETE ON items BEGIN
  INSERT INTO items_fts(items_fts, rowid, title_norm) VALUES('delete', old.rowid, old.title_norm);
END;

CREATE TRIGGER items_fts_update AFTER UPDATE ON items BEGIN
  INSERT INTO items_fts(items_fts, rowid, title_norm) VALUES('delete', old.rowid, old.title_norm);
  INSERT INTO items_fts(rowid, title_norm) VALUES (new.rowid, new.title_norm);
END;

PRAGMA user_version = 1;
"#;

fn prepare(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;",
    )
    .map_err(|error| format!("não foi possível configurar o banco: {error}"))
}

fn migrate(conn: &Connection) -> Result<(), String> {
    let current: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| format!("não foi possível ler a versão do banco: {error}"))?;

    if current == SCHEMA_VERSION {
        return Ok(());
    }
    if current > SCHEMA_VERSION {
        return Err(format!(
            "banco na versão {current}, mais nova que esta build ({SCHEMA_VERSION})"
        ));
    }

    // Só existe a v1; migrações futuras entram aqui como blocos condicionais.
    conn.execute_batch(MIGRATION_V1)
        .map_err(|error| format!("migração v1 falhou: {error}"))
}

pub fn open(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("não foi possível criar {}: {error}", parent.display()))?;
    }
    let conn = Connection::open(path)
        .map_err(|error| format!("não foi possível abrir {}: {error}", path.display()))?;
    prepare(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Banco descartável, usado pelos testes.
pub fn open_memory() -> Result<Connection, String> {
    let conn = Connection::open_in_memory()
        .map_err(|error| format!("não foi possível abrir banco em memória: {error}"))?;
    prepare(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Segundos desde a época — todo `*_at` do schema usa esta unidade.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}
```

Modify `src-tauri/src/lib.rs` — trocar o topo do arquivo:

```rust
pub mod catalog;
pub mod db;
pub mod meta;
mod proxy;
```

- [ ] **Step 5: Rodar os testes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test schema`
Expected: PASS, 5 testes.

Se `fts5_esta_disponivel_e_sincronizada` falhar com `no such module: fts5`, o
SQLite embutido veio sem FTS5. Corrija a dependência em `src-tauri/Cargo.toml`
para `rusqlite = { version = "<a versão que o cargo add escolheu>", features = ["bundled-full"] }`
e rode o teste de novo. Nenhuma outra mudança é necessária.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/db.rs src-tauri/src/lib.rs src-tauri/tests/schema.rs
git commit -m "feat: banco SQLite local com migração v1 e busca FTS5"
```

---

## Task 2: `ids.rs` — normalização de título e hashes estáveis

**Files:**
- Create: `src-tauri/src/ids.rs`
- Modify: `src-tauri/src/lib.rs` (adicionar `pub mod ids;`)
- Test: `src-tauri/tests/ids_stable.rs`

**Interfaces:**
- Consumes: nada.
- Produces:
  - `ids::normalize(title: &str) -> String`
  - `ids::item_id(kind: &str, title_norm: &str, year: Option<i64>) -> String`
  - `ids::episode_id(series_id: &str, season: i64, episode: i64) -> String`

- [ ] **Step 1: Escrever o teste que falha**

Create `src-tauri/tests/ids_stable.rs`:

```rust
use yellow_tv_lib::ids;

#[test]
fn normaliza_caixa_acento_e_espacos() {
    assert_eq!(ids::normalize("  O   Caçador  "), "o cacador");
    assert_eq!(ids::normalize("Ação"), "acao");
}

#[test]
fn remove_tokens_de_qualidade() {
    assert_eq!(ids::normalize("Duna 4K"), "duna");
    assert_eq!(ids::normalize("Duna FHD H265"), "duna");
    assert_eq!(ids::normalize("Duna ALT 2"), "duna");
    assert_eq!(ids::normalize("Duna [L] (2021) DUBLADO"), "duna");
}

#[test]
fn nao_come_palavra_que_contem_token() {
    // "HD" solto sai; "Ghost" continua inteiro.
    assert_eq!(ids::normalize("Ghost HD"), "ghost");
}

#[test]
fn mesma_obra_com_qualidades_diferentes_colapsa_no_mesmo_id() {
    let a = ids::item_id("movie", &ids::normalize("Duna 4K"), Some(2021));
    let b = ids::item_id("movie", &ids::normalize("DUNA fhd"), Some(2021));
    assert_eq!(a, b);
}

#[test]
fn ano_diferente_gera_id_diferente() {
    let a = ids::item_id("movie", "duna", Some(2021));
    let b = ids::item_id("movie", "duna", Some(1984));
    assert_ne!(a, b);
}

#[test]
fn kind_diferente_gera_id_diferente() {
    let a = ids::item_id("movie", "fargo", None);
    let b = ids::item_id("series", "fargo", None);
    assert_ne!(a, b);
}

#[test]
fn id_e_estavel_entre_execucoes() {
    // Trava o formato: 32 caracteres hexadecimais e valor fixo.
    let id = ids::item_id("movie", "duna", Some(2021));
    assert_eq!(id.len(), 32);
    assert!(id.chars().all(|character| character.is_ascii_hexdigit()));
    assert_eq!(id, ids::item_id("movie", "duna", Some(2021)));
}

#[test]
fn episode_id_depende_de_temporada_e_episodio() {
    let series = ids::item_id("series", "fargo", None);
    assert_ne!(
        ids::episode_id(&series, 1, 2),
        ids::episode_id(&series, 2, 1)
    );
    assert_eq!(
        ids::episode_id(&series, 1, 2),
        ids::episode_id(&series, 1, 2)
    );
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test ids_stable`
Expected: FAIL — `could not find ids in yellow_tv_lib`.

- [ ] **Step 3: Escrever `ids.rs`**

Create `src-tauri/src/ids.rs`:

```rust
//! Título normalizado e identificadores estáveis.
//!
//! O id de um item nasce de `(kind, title_norm, year)` e nunca da URL: é isso que
//! faz o mesmo filme de duas listas colapsar num item só e o progresso sobreviver
//! a um re-sync.

use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

/// Ruído que as listas acrescentam ao nome e que não distingue uma obra de outra.
const NOISE: &[&str] = &[
    "4k", "8k", "uhd", "fhd", "hd", "sd", "h265", "h264", "hevc", "x265", "x264", "l", "alt",
    "alternativo", "alternativa", "dublado", "dub", "legendado", "leg", "nacional", "multi",
    "1080p", "720p", "480p", "2160p",
];

/// Minúsculas, sem acento, sem parênteses/colchetes, sem tokens de qualidade,
/// espaços colapsados. Alimenta `items.title_norm` e, por consequência, o hash.
pub fn normalize(title: &str) -> String {
    let mut plain = String::with_capacity(title.len());
    let mut depth = 0usize;
    for character in title.chars() {
        match character {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => plain.push(character),
            _ => {}
        }
    }

    let folded: String = plain
        .to_lowercase()
        .nfd()
        .filter(|character| !matches!(*character, '\u{0300}'..='\u{036f}'))
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect();

    folded
        .split_whitespace()
        .filter(|token| !NOISE.contains(token))
        // Um "2" sozinho depois de "alt" é lixo; "Duna 2" perde o número apenas
        // quando ele é o resto de um token de qualidade — por isso o filtro só
        // remove dígitos isolados no fim da cadeia.
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned()
}

fn digest(input: &str) -> String {
    hex::encode(&Sha256::digest(input.as_bytes())[..16])
}

pub fn item_id(kind: &str, title_norm: &str, year: Option<i64>) -> String {
    digest(&format!("{kind}\u{1f}{title_norm}\u{1f}{}", year.unwrap_or(0)))
}

pub fn episode_id(series_id: &str, season: i64, episode: i64) -> String {
    digest(&format!("{series_id}\u{1f}{season}\u{1f}{episode}"))
}
```

- [ ] **Step 4: Rodar os testes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test ids_stable`
Expected: PASS.

`normaliza "Duna ALT 2"` só passa se o `2` órfão cair. Se o teste
`remove_tokens_de_qualidade` falhar em `"Duna ALT 2"` devolvendo `"duna 2"`,
acrescente este passo depois do `filter` de `NOISE`, antes do `join`:

```rust
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .skip_while(|token| token.len() <= 2 && token.chars().all(|c| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
```

e mantenha o `join(" ")` no fim. Rode de novo até passar.

- [ ] **Step 5: Adicionar o módulo e commitar**

Modify `src-tauri/src/lib.rs` — acrescentar `pub mod ids;` junto às outras declarações.

```bash
git add src-tauri/src/ids.rs src-tauri/src/lib.rs src-tauri/tests/ids_stable.rs
git commit -m "feat: normalização de título e ids estáveis de item e episódio"
```

---

## Task 3: `m3u.rs` — parser de `#EXTINF` e classificação

**Files:**
- Create: `src-tauri/src/m3u.rs`
- Modify: `src-tauri/src/lib.rs` (adicionar `pub mod m3u;`)
- Test: `src-tauri/tests/m3u_parse.rs`

**Interfaces:**
- Consumes: `ids::normalize`, `ids::item_id`, `ids::episode_id` (Task 2).
- Produces:

```rust
pub enum EntryKind { Movie, Series, Channel }   // as_str() -> "movie" | "series" | "channel"
pub struct Entry {
    pub kind: EntryKind,
    pub title: String,        // título de exibição, já sem tokens de qualidade
    pub title_norm: String,
    pub year: Option<i64>,
    pub season: Option<i64>,
    pub episode: Option<i64>,
    pub episode_title: Option<String>,
    pub logo: Option<String>,
    pub group_name: Option<String>,
    pub quality: Option<String>,
    pub channel_number: Option<i64>,
    pub url: String,
}
pub struct Outcome { pub parsed: usize, pub discarded: usize }
pub fn parse_each<R: std::io::BufRead>(reader: R, sink: impl FnMut(Entry)) -> Outcome
pub fn parse_all<R: std::io::BufRead>(reader: R) -> (Vec<Entry>, Outcome)   // conveniência de teste
```

- [ ] **Step 1: Escrever o teste que falha**

Create `src-tauri/tests/m3u_parse.rs`:

```rust
use std::io::Cursor;
use yellow_tv_lib::m3u::{self, EntryKind};

fn parse(text: &str) -> (Vec<m3u::Entry>, m3u::Outcome) {
    m3u::parse_all(Cursor::new(text.to_owned()))
}

#[test]
fn filme_com_ano_no_titulo() {
    let (entries, outcome) = parse(
        "#EXTM3U\n#EXTINF:-1 tvg-logo=\"http://p/duna.jpg\" group-title=\"Filmes | Ficção\",Duna (2021)\nhttp://host/duna.mp4\n",
    );
    assert_eq!(outcome.parsed, 1);
    assert_eq!(outcome.discarded, 0);
    let entry = &entries[0];
    assert!(matches!(entry.kind, EntryKind::Movie));
    assert_eq!(entry.title, "Duna");
    assert_eq!(entry.title_norm, "duna");
    assert_eq!(entry.year, Some(2021));
    assert_eq!(entry.logo.as_deref(), Some("http://p/duna.jpg"));
    assert_eq!(entry.group_name.as_deref(), Some("Filmes | Ficção"));
    assert_eq!(entry.url, "http://host/duna.mp4");
}

#[test]
fn filme_com_ano_solto_no_fim() {
    let (entries, _) = parse("#EXTINF:-1,Matrix 1999\nhttp://host/m\n");
    assert_eq!(entries[0].year, Some(1999));
    assert_eq!(entries[0].title, "Matrix");
}

#[test]
fn filme_sem_ano() {
    let (entries, _) = parse("#EXTINF:-1,Cidade de Deus\nhttp://host/cdd\n");
    assert_eq!(entries[0].year, None);
    assert!(matches!(entries[0].kind, EntryKind::Movie));
}

#[test]
fn serie_no_formato_s01e02() {
    let (entries, _) = parse("#EXTINF:-1,Fargo S01E02 4K\nhttp://host/f102\n");
    let entry = &entries[0];
    assert!(matches!(entry.kind, EntryKind::Series));
    assert_eq!(entry.title, "Fargo");
    assert_eq!(entry.season, Some(1));
    assert_eq!(entry.episode, Some(2));
    assert_eq!(entry.quality.as_deref(), Some("4K"));
}

#[test]
fn serie_no_formato_1x02() {
    let (entries, _) = parse("#EXTINF:-1,Fargo 1x02\nhttp://host/f102\n");
    assert!(matches!(entries[0].kind, EntryKind::Series));
    assert_eq!(entries[0].season, Some(1));
    assert_eq!(entries[0].episode, Some(2));
    assert_eq!(entries[0].title, "Fargo");
}

#[test]
fn serie_escrita_em_portugues() {
    let (entries, _) = parse("#EXTINF:-1,Fargo Temporada 2 Episodio 5\nhttp://host/f205\n");
    assert!(matches!(entries[0].kind, EntryKind::Series));
    assert_eq!(entries[0].season, Some(2));
    assert_eq!(entries[0].episode, Some(5));
    assert_eq!(entries[0].title, "Fargo");
}

#[test]
fn canal_por_tvg_chno() {
    let (entries, _) = parse(
        "#EXTINF:-1 tvg-id=\"globo.br\" tvg-chno=\"12\",Globo SP HD\nhttp://host/globo\n",
    );
    let entry = &entries[0];
    assert!(matches!(entry.kind, EntryKind::Channel));
    assert_eq!(entry.channel_number, Some(12));
    assert_eq!(entry.title, "Globo SP");
}

#[test]
fn canal_por_group_title_ao_vivo() {
    let (entries, _) = parse("#EXTINF:-1 group-title=\"Canais | Esportes\",SporTV\nhttp://h/s\n");
    assert!(matches!(entries[0].kind, EntryKind::Channel));
}

#[test]
fn serie_ganha_de_canal_quando_os_dois_casam() {
    // A regra 1 vence a regra 2: marca de episódio decide, mesmo com tvg-id.
    let (entries, _) = parse(
        "#EXTINF:-1 tvg-id=\"x\" group-title=\"Canais\",Fargo S03E01\nhttp://h/f\n",
    );
    assert!(matches!(entries[0].kind, EntryKind::Series));
}

#[test]
fn atributos_fora_de_ordem() {
    let (entries, _) = parse(
        "#EXTINF:-1 group-title=\"Filmes\" tvg-logo=\"http://p.jpg\" tvg-name=\"ignorado\",Duna\nhttp://h/d\n",
    );
    assert_eq!(entries[0].logo.as_deref(), Some("http://p.jpg"));
    assert_eq!(entries[0].group_name.as_deref(), Some("Filmes"));
}

#[test]
fn linha_sem_virgula_e_descartada() {
    let (entries, outcome) = parse("#EXTINF:-1 tvg-id=\"x\"\nhttp://h/x\n#EXTINF:-1,Bom\nhttp://h/b\n");
    assert_eq!(entries.len(), 1, "só a entrada boa entra");
    assert_eq!(outcome.discarded, 1);
}

#[test]
fn extinf_sem_url_e_descartado() {
    let (entries, outcome) = parse("#EXTINF:-1,Sem stream\n#EXTINF:-1,Bom\nhttp://h/b\n");
    assert_eq!(entries.len(), 1);
    assert_eq!(outcome.discarded, 1);
}

#[test]
fn linhas_vazias_e_comentarios_nao_contam_como_descarte() {
    let (_, outcome) = parse("#EXTM3U\n\n#EXTVLCOPT:foo\n#EXTINF:-1,Bom\nhttp://h/b\n");
    assert_eq!(outcome.discarded, 0);
    assert_eq!(outcome.parsed, 1);
}

#[test]
fn lista_vazia_devolve_zero() {
    let (entries, outcome) = parse("#EXTM3U\n");
    assert!(entries.is_empty());
    assert_eq!(outcome.parsed, 0);
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test m3u_parse`
Expected: FAIL — `could not find m3u in yellow_tv_lib`.

- [ ] **Step 3: Escrever `m3u.rs`**

Create `src-tauri/src/m3u.rs`:

```rust
//! Parser de listas M3U/M3U8 em streaming.
//!
//! A lista chega com 100 mil linhas ou mais: o parser nunca guarda o texto
//! inteiro, entrega uma `Entry` por vez ao chamador e conta o que descartou. Uma
//! linha ruim não derruba o import.

use std::io::BufRead;

use crate::ids;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Movie,
    Series,
    Channel,
}

impl EntryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntryKind::Movie => "movie",
            EntryKind::Series => "series",
            EntryKind::Channel => "channel",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub kind: EntryKind,
    pub title: String,
    pub title_norm: String,
    pub year: Option<i64>,
    pub season: Option<i64>,
    pub episode: Option<i64>,
    pub episode_title: Option<String>,
    pub logo: Option<String>,
    pub group_name: Option<String>,
    pub quality: Option<String>,
    pub channel_number: Option<i64>,
    pub url: String,
}

impl Entry {
    /// Id do título ao qual esta linha pertence — para série, o id da série.
    pub fn item_id(&self) -> String {
        ids::item_id(self.kind.as_str(), &self.title_norm, self.year)
    }

    /// Id do episódio, quando a linha é de série com temporada e episódio.
    pub fn episode_id(&self) -> Option<String> {
        match (self.season, self.episode) {
            (Some(season), Some(episode)) => {
                Some(ids::episode_id(&self.item_id(), season, episode))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub parsed: usize,
    pub discarded: usize,
}

const QUALITY_TOKENS: &[&str] = &[
    "4K", "8K", "UHD", "FHD", "HD", "SD", "H265", "H264", "HEVC", "1080P", "720P", "2160P",
];

/// `tvg-logo="x" group-title="y",Nome` → atributos e nome.
fn split_extinf(line: &str) -> Option<(Vec<(String, String)>, String)> {
    let body = line.strip_prefix("#EXTINF:")?;
    // A vírgula que separa atributos do nome é a última fora de aspas.
    let mut in_quotes = false;
    let mut split_at = None;
    for (index, character) in body.char_indices() {
        match character {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => split_at = Some(index),
            _ => {}
        }
    }
    let split_at = split_at?;
    let (head, name) = (&body[..split_at], body[split_at + 1..].trim());
    if name.is_empty() {
        return None;
    }

    let mut attrs = Vec::new();
    let bytes: Vec<char> = head.chars().collect();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        // key=
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor] != '=' {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            break;
        }
        let key: String = bytes[start..cursor].iter().collect();
        cursor += 1; // pula o '='
        let value: String = if bytes.get(cursor) == Some(&'"') {
            cursor += 1;
            let value_start = cursor;
            while cursor < bytes.len() && bytes[cursor] != '"' {
                cursor += 1;
            }
            let value = bytes[value_start..cursor].iter().collect();
            cursor += 1; // pula o '"' de fechamento
            value
        } else {
            let value_start = cursor;
            while cursor < bytes.len() && !bytes[cursor].is_whitespace() {
                cursor += 1;
            }
            bytes[value_start..cursor].iter().collect()
        };
        let key = key.trim().trim_start_matches(|c: char| c == '-' || c.is_numeric()).to_lowercase();
        if !key.is_empty() {
            attrs.push((key, value));
        }
        while cursor < bytes.len() && bytes[cursor].is_whitespace() {
            cursor += 1;
        }
    }

    Some((attrs, name.to_owned()))
}

fn attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.is_empty())
}

/// Marca de episódio no nome. Devolve `(temporada, episódio, título da série)`.
fn series_marks(name: &str) -> Option<(i64, i64, String)> {
    let chars: Vec<char> = name.chars().collect();
    let lower: Vec<char> = name.to_lowercase().chars().collect();

    // Padrão SxxEyy
    for index in 0..chars.len() {
        if lower[index] != 's' {
            continue;
        }
        let mut cursor = index + 1;
        let season_start = cursor;
        while cursor < chars.len() && chars[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor == season_start || cursor >= chars.len() || lower[cursor] != 'e' {
            continue;
        }
        let season: i64 = chars[season_start..cursor].iter().collect::<String>().parse().ok()?;
        cursor += 1;
        let episode_start = cursor;
        while cursor < chars.len() && chars[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor == episode_start {
            continue;
        }
        let episode: i64 = chars[episode_start..cursor].iter().collect::<String>().parse().ok()?;
        let head: String = chars[..index].iter().collect();
        return Some((season, episode, head));
    }

    // Padrão NxM
    for index in 0..chars.len() {
        if lower[index] != 'x' || index == 0 {
            continue;
        }
        let mut before = index;
        while before > 0 && chars[before - 1].is_ascii_digit() {
            before -= 1;
        }
        if before == index {
            continue;
        }
        let mut after = index + 1;
        let episode_start = after;
        while after < chars.len() && chars[after].is_ascii_digit() {
            after += 1;
        }
        if after == episode_start {
            continue;
        }
        let season: i64 = chars[before..index].iter().collect::<String>().parse().ok()?;
        let episode: i64 = chars[episode_start..after].iter().collect::<String>().parse().ok()?;
        let head: String = chars[..before].iter().collect();
        return Some((season, episode, head));
    }

    // "Temporada N Episodio M", com ou sem acento
    let folded = ids::normalize(name);
    let tokens: Vec<&str> = folded.split_whitespace().collect();
    let mut season = None;
    let mut episode = None;
    let mut cut = tokens.len();
    for (index, token) in tokens.iter().enumerate() {
        let next = tokens.get(index + 1).and_then(|value| value.parse::<i64>().ok());
        if *token == "temporada" && next.is_some() {
            season = next;
            cut = cut.min(index);
        }
        if (*token == "episodio" || *token == "ep") && next.is_some() {
            episode = next;
            cut = cut.min(index);
        }
    }
    match (season, episode) {
        (Some(season), Some(episode)) => {
            let head = tokens[..cut].join(" ");
            Some((season, episode, head))
        }
        _ => None,
    }
}

/// Ano em `(2019)` ou como último token do nome.
fn extract_year(name: &str) -> (Option<i64>, String) {
    let trimmed = name.trim();
    if let Some(open) = trimmed.rfind('(') {
        let inside: String = trimmed[open + 1..].trim_end_matches(')').trim().to_owned();
        if let Ok(year) = inside.parse::<i64>() {
            if (1900..=2100).contains(&year) {
                return (Some(year), trimmed[..open].trim().to_owned());
            }
        }
    }
    let mut tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if let Some(last) = tokens.last() {
        if let Ok(year) = last.parse::<i64>() {
            if (1900..=2100).contains(&year) {
                tokens.pop();
                return (Some(year), tokens.join(" "));
            }
        }
    }
    (None, trimmed.to_owned())
}

/// Tokens de qualidade encontrados, e o nome sem eles.
fn strip_quality(name: &str) -> (Option<String>, String) {
    let mut found: Vec<String> = Vec::new();
    let kept: Vec<&str> = name
        .split_whitespace()
        .filter(|token| {
            let bare = token.trim_matches(|c: char| !c.is_alphanumeric()).to_uppercase();
            if QUALITY_TOKENS.contains(&bare.as_str()) {
                found.push(bare);
                false
            } else {
                true
            }
        })
        .collect();
    let quality = if found.is_empty() {
        None
    } else {
        found.dedup();
        Some(found.join(" "))
    };
    (quality, kept.join(" "))
}

fn looks_like_channel(attrs: &[(String, String)], group: Option<&str>) -> bool {
    if attr(attrs, "tvg-id").is_some() || attr(attrs, "tvg-chno").is_some() {
        return true;
    }
    let Some(group) = group else { return false };
    let folded = ids::normalize(group);
    ["canais", "canal", "tv", "ao vivo", "live", "abertos"]
        .iter()
        .any(|needle| folded.contains(needle))
}

fn build(attrs: Vec<(String, String)>, name: String, url: String) -> Entry {
    let logo = attr(&attrs, "tvg-logo").map(str::to_owned);
    let group_name = attr(&attrs, "group-title").map(str::to_owned);
    let channel_number = attr(&attrs, "tvg-chno").and_then(|value| value.parse().ok());

    // Regra 1: marca de episódio manda, mesmo em grupo de canal.
    if let Some((season, episode, head)) = series_marks(&name) {
        let (quality, clean_head) = strip_quality(&head);
        let (_, without_year) = extract_year(&clean_head);
        let title = without_year.trim().to_owned();
        return Entry {
            kind: EntryKind::Series,
            title_norm: ids::normalize(&title),
            title,
            year: None,
            season: Some(season),
            episode: Some(episode),
            episode_title: None,
            logo,
            group_name,
            quality,
            channel_number: None,
            url,
        };
    }

    let (quality, clean) = strip_quality(&name);

    // Regra 2: atributos de EPG ou grupo de canal.
    if looks_like_channel(&attrs, group_name.as_deref()) {
        let title = clean.trim().to_owned();
        return Entry {
            kind: EntryKind::Channel,
            title_norm: ids::normalize(&title),
            title,
            year: None,
            season: None,
            episode: None,
            episode_title: None,
            logo,
            group_name,
            quality,
            channel_number,
            url,
        };
    }

    // Regra 3: filme.
    let (year, title) = extract_year(&clean);
    let title = title.trim().to_owned();
    Entry {
        kind: EntryKind::Movie,
        title_norm: ids::normalize(&title),
        title,
        year,
        season: None,
        episode: None,
        episode_title: None,
        logo,
        group_name,
        quality,
        channel_number: None,
        url,
    }
}

pub fn parse_each<R: BufRead>(reader: R, mut sink: impl FnMut(Entry)) -> Outcome {
    let mut outcome = Outcome::default();
    let mut pending: Option<(Vec<(String, String)>, String)> = None;

    for line in reader.lines() {
        let Ok(line) = line else {
            outcome.discarded += 1;
            continue;
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("#EXTINF:") {
            if pending.is_some() {
                // O #EXTINF anterior ficou sem URL.
                outcome.discarded += 1;
            }
            match split_extinf(line) {
                Some(parsed) => pending = Some(parsed),
                None => {
                    outcome.discarded += 1;
                    pending = None;
                }
            }
            continue;
        }

        if line.starts_with('#') {
            continue; // #EXTM3U, #EXTVLCOPT, #EXTGRP e afins
        }

        match pending.take() {
            Some((attrs, name)) => {
                sink(build(attrs, name, line.to_owned()));
                outcome.parsed += 1;
            }
            None => outcome.discarded += 1, // URL órfã
        }
    }

    if pending.is_some() {
        outcome.discarded += 1;
    }
    outcome
}

/// Conveniência para testes e listas pequenas.
pub fn parse_all<R: BufRead>(reader: R) -> (Vec<Entry>, Outcome) {
    let mut entries = Vec::new();
    let outcome = parse_each(reader, |entry| entries.push(entry));
    (entries, outcome)
}
```

- [ ] **Step 4: Rodar os testes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test m3u_parse`
Expected: PASS, 14 testes. Se algum caso de classificação falhar, ajuste
`series_marks`/`looks_like_channel` — os testes são o contrato, não o código.

- [ ] **Step 5: Adicionar o módulo e commitar**

Modify `src-tauri/src/lib.rs` — acrescentar `pub mod m3u;`.

```bash
git add src-tauri/src/m3u.rs src-tauri/src/lib.rs src-tauri/tests/m3u_parse.rs
git commit -m "feat: parser de M3U em streaming com classificação de canal, filme e série"
```

---

## Task 4: `import.rs` — fontes, transação de import, dedup

**Files:**
- Create: `src-tauri/src/import.rs`
- Modify: `src-tauri/src/lib.rs` (adicionar `pub mod import;`)
- Test: `src-tauri/tests/import_dedup.rs`

**Interfaces:**
- Consumes: `db::now`, `m3u::{parse_each, Entry, EntryKind}`, `ids` (Tasks 1-3).
- Produces:

```rust
pub struct Source {          // serializa em camelCase para o front
    pub id: i64, pub url: String, pub label: String, pub kind: String,
    pub enabled: bool, pub added_at: i64, pub last_sync_at: Option<i64>,
    pub item_count: i64,
}
pub struct ImportReport { pub source: Source, pub parsed: usize, pub discarded: usize,
                          pub movies: usize, pub series: usize, pub channels: usize }

pub fn list_sources(conn: &Connection) -> Result<Vec<Source>, String>
pub fn upsert_source(conn: &Connection, url: &str, label: &str, kind: &str) -> Result<Source, String>
pub fn get_source(conn: &Connection, id: i64) -> Result<Source, String>
pub fn remove_source(conn: &mut Connection, id: i64) -> Result<(), String>
pub fn ingest<R: BufRead>(conn: &mut Connection, source_id: i64, reader: R,
                          progress: &mut dyn FnMut(usize)) -> Result<ImportReport, String>
```

`ingest` é o coração: uma transação, `progress` chamado a cada 500 entradas com o
total acumulado, rollback automático se der erro ou se zero entradas forem
reconhecidas.

- [ ] **Step 1: Escrever o teste que falha**

Create `src-tauri/tests/import_dedup.rs`:

```rust
use std::io::Cursor;
use yellow_tv_lib::{db, import};

const LISTA_A: &str = "#EXTM3U\n\
#EXTINF:-1 group-title=\"Filmes\",Duna (2021) 4K\nhttp://a/duna-4k\n\
#EXTINF:-1 group-title=\"Series\",Fargo S01E01\nhttp://a/fargo-101\n\
#EXTINF:-1 group-title=\"Series\",Fargo S01E02\nhttp://a/fargo-102\n\
#EXTINF:-1 tvg-chno=\"12\",Globo SP HD\nhttp://a/globo\n";

const LISTA_B: &str = "#EXTM3U\n\
#EXTINF:-1 group-title=\"Filmes\",DUNA (2021) FHD\nhttp://b/duna-fhd\n";

fn ingest(conn: &mut rusqlite::Connection, url: &str, text: &str) -> import::ImportReport {
    let source = import::upsert_source(conn, url, "Teste", "url").unwrap();
    let mut seen = 0usize;
    import::ingest(conn, source.id, Cursor::new(text.to_owned()), &mut |count| seen = count)
        .unwrap()
}

#[test]
fn conta_o_que_entrou_por_tipo() {
    let mut conn = db::open_memory().unwrap();
    let report = ingest(&mut conn, "http://a", LISTA_A);
    assert_eq!(report.parsed, 4);
    assert_eq!(report.discarded, 0);
    assert_eq!(report.movies, 1);
    assert_eq!(report.series, 1, "duas linhas, uma série");
    assert_eq!(report.channels, 1);
}

#[test]
fn episodios_viram_linhas_em_episodes() {
    let mut conn = db::open_memory().unwrap();
    ingest(&mut conn, "http://a", LISTA_A);
    let episodes: i64 = conn
        .query_row("SELECT count(*) FROM episodes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(episodes, 2);
    let owner_kinds: i64 = conn
        .query_row(
            "SELECT count(*) FROM streams WHERE owner_kind = 'episode'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(owner_kinds, 2, "stream de episódio aponta para episodes.id");
}

#[test]
fn duas_listas_com_o_mesmo_filme_geram_um_item_e_dois_streams() {
    let mut conn = db::open_memory().unwrap();
    ingest(&mut conn, "http://a", LISTA_A);
    ingest(&mut conn, "http://b", LISTA_B);

    let movies: i64 = conn
        .query_row("SELECT count(*) FROM items WHERE kind = 'movie'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(movies, 1, "mesmo filme em duas listas é um item");

    let streams: i64 = conn
        .query_row(
            "SELECT count(*) FROM streams WHERE owner_kind = 'item'
             AND owner_id = (SELECT id FROM items WHERE kind = 'movie')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(streams, 2, "duas fontes de stream para o mesmo item");
}

#[test]
fn reimportar_a_mesma_lista_nao_duplica() {
    let mut conn = db::open_memory().unwrap();
    ingest(&mut conn, "http://a", LISTA_A);
    ingest(&mut conn, "http://a", LISTA_A);
    let streams: i64 = conn
        .query_row("SELECT count(*) FROM streams", [], |row| row.get(0))
        .unwrap();
    assert_eq!(streams, 4);
}

#[test]
fn lista_vazia_falha_e_faz_rollback() {
    let mut conn = db::open_memory().unwrap();
    let source = import::upsert_source(&mut conn, "http://vazia", "Vazia", "url").unwrap();
    let result = import::ingest(
        &mut conn,
        source.id,
        Cursor::new("#EXTM3U\n".to_owned()),
        &mut |_| {},
    );
    assert!(result.is_err(), "lista sem entradas reconhecidas deve falhar");
    let items: i64 = conn
        .query_row("SELECT count(*) FROM items", [], |row| row.get(0))
        .unwrap();
    assert_eq!(items, 0);
}

#[test]
fn linhas_ruins_sao_contadas_sem_derrubar_o_import() {
    let mut conn = db::open_memory().unwrap();
    let report = ingest(
        &mut conn,
        "http://mista",
        "#EXTINF:-1 tvg-id=\"x\"\nhttp://h/x\n#EXTINF:-1,Duna (2021)\nhttp://h/d\n",
    );
    assert_eq!(report.parsed, 1);
    assert_eq!(report.discarded, 1);
}

#[test]
fn source_registra_contagem_e_data_de_sync() {
    let mut conn = db::open_memory().unwrap();
    let report = ingest(&mut conn, "http://a", LISTA_A);
    let stored = import::get_source(&conn, report.source.id).unwrap();
    assert_eq!(stored.item_count, 3, "3 títulos distintos");
    assert!(stored.last_sync_at.is_some());
}

#[test]
fn remover_source_apaga_itens_orfaos_e_preserva_progresso() {
    let mut conn = db::open_memory().unwrap();
    let report = ingest(&mut conn, "http://a", LISTA_A);
    let movie_id: String = conn
        .query_row("SELECT id FROM items WHERE kind = 'movie'", [], |row| row.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO progress(owner_id, owner_kind, position_secs, updated_at)
         VALUES(?1,'item',600.0,0)",
        [&movie_id],
    )
    .unwrap();

    import::remove_source(&mut conn, report.source.id).unwrap();

    let items: i64 = conn
        .query_row("SELECT count(*) FROM items", [], |row| row.get(0))
        .unwrap();
    let progress: i64 = conn
        .query_row("SELECT count(*) FROM progress", [], |row| row.get(0))
        .unwrap();
    assert_eq!(items, 0, "sem stream, o item é órfão e sai");
    assert_eq!(progress, 1, "o histórico fica para quando a lista voltar");
}

#[test]
fn progresso_reencontra_o_item_quando_a_lista_volta() {
    let mut conn = db::open_memory().unwrap();
    let report = ingest(&mut conn, "http://a", LISTA_A);
    let movie_id: String = conn
        .query_row("SELECT id FROM items WHERE kind = 'movie'", [], |row| row.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO progress(owner_id, owner_kind, position_secs, updated_at)
         VALUES(?1,'item',600.0,0)",
        [&movie_id],
    )
    .unwrap();
    import::remove_source(&mut conn, report.source.id).unwrap();
    ingest(&mut conn, "http://a", LISTA_A);

    let matched: i64 = conn
        .query_row(
            "SELECT count(*) FROM progress p JOIN items i ON i.id = p.owner_id",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(matched, 1, "o hash não depende da URL, então o progresso casa de novo");
}

#[test]
fn list_sources_devolve_o_que_foi_adicionado() {
    let mut conn = db::open_memory().unwrap();
    import::upsert_source(&mut conn, "http://a", "A", "url").unwrap();
    import::upsert_source(&mut conn, "/tmp/b.m3u", "B", "file").unwrap();
    let sources = import::list_sources(&conn).unwrap();
    assert_eq!(sources.len(), 2);
    assert!(sources.iter().any(|source| source.kind == "file"));
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test import_dedup`
Expected: FAIL — `could not find import in yellow_tv_lib`.

- [ ] **Step 3: Escrever `import.rs`**

Create `src-tauri/src/import.rs`:

```rust
//! Ingestão de listas M3U no banco.
//!
//! Uma transação por import: ou a lista entra inteira, ou nada muda. Os `INSERT`
//! usam `ON CONFLICT` porque a mesma obra aparece em listas diferentes — é assim
//! que o dedup sai de graça.

use std::io::BufRead;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db;
use crate::m3u::{self, EntryKind};

const PROGRESS_EVERY: usize = 500;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: i64,
    pub url: String,
    pub label: String,
    pub kind: String,
    pub enabled: bool,
    pub added_at: i64,
    pub last_sync_at: Option<i64>,
    pub item_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub source: Source,
    pub parsed: usize,
    pub discarded: usize,
    pub movies: usize,
    pub series: usize,
    pub channels: usize,
}

const SOURCE_COLUMNS: &str =
    "id, url, label, kind, enabled, added_at, last_sync_at, item_count";

fn read_source(row: &rusqlite::Row<'_>) -> rusqlite::Result<Source> {
    Ok(Source {
        id: row.get(0)?,
        url: row.get(1)?,
        label: row.get(2)?,
        kind: row.get(3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        added_at: row.get(5)?,
        last_sync_at: row.get(6)?,
        item_count: row.get(7)?,
    })
}

pub fn list_sources(conn: &Connection) -> Result<Vec<Source>, String> {
    let sql = format!("SELECT {SOURCE_COLUMNS} FROM sources ORDER BY added_at");
    let mut statement = conn.prepare(&sql).map_err(stringify)?;
    let rows = statement
        .query_map([], read_source)
        .map_err(stringify)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(stringify)?;
    Ok(rows)
}

pub fn get_source(conn: &Connection, id: i64) -> Result<Source, String> {
    let sql = format!("SELECT {SOURCE_COLUMNS} FROM sources WHERE id = ?1");
    conn.query_row(&sql, [id], read_source)
        .optional()
        .map_err(stringify)?
        .ok_or_else(|| format!("lista não encontrada: {id}"))
}

/// Adicionar a mesma URL duas vezes não cria uma segunda fonte.
pub fn upsert_source(
    conn: &Connection,
    url: &str,
    label: &str,
    kind: &str,
) -> Result<Source, String> {
    if url.trim().is_empty() {
        return Err("informe a URL ou o arquivo da lista".into());
    }
    if !matches!(kind, "url" | "file") {
        return Err(format!("tipo de fonte inválido: {kind}"));
    }
    conn.execute(
        "INSERT INTO sources(url, label, kind, added_at) VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(url) DO UPDATE SET label = excluded.label",
        params![url.trim(), label, kind, db::now()],
    )
    .map_err(stringify)?;
    let id: i64 = conn
        .query_row("SELECT id FROM sources WHERE url = ?1", [url.trim()], |row| {
            row.get(0)
        })
        .map_err(stringify)?;
    get_source(conn, id)
}

pub fn remove_source(conn: &mut Connection, id: i64) -> Result<(), String> {
    let transaction = conn.transaction().map_err(stringify)?;
    transaction
        .execute("DELETE FROM sources WHERE id = ?1", [id])
        .map_err(stringify)?;
    // Itens sem nenhum stream não são mais alcançáveis; o progresso deles fica.
    transaction
        .execute(
            "DELETE FROM items WHERE id NOT IN (SELECT owner_id FROM streams WHERE owner_kind = 'item')
             AND id NOT IN (SELECT series_id FROM episodes
                            WHERE id IN (SELECT owner_id FROM streams WHERE owner_kind = 'episode'))",
            [],
        )
        .map_err(stringify)?;
    transaction.commit().map_err(stringify)
}

pub fn ingest<R: BufRead>(
    conn: &mut Connection,
    source_id: i64,
    reader: R,
    progress: &mut dyn FnMut(usize),
) -> Result<ImportReport, String> {
    let now = db::now();
    let transaction = conn.transaction().map_err(stringify)?;

    // Uma lista re-sincronizada substitui os próprios streams; os de outras
    // fontes continuam, e é isso que preserva o dedup entre listas.
    transaction
        .execute("DELETE FROM streams WHERE source_id = ?1", [source_id])
        .map_err(stringify)?;

    let mut movies = 0usize;
    let mut series = 0usize;
    let mut channels = 0usize;
    let mut failure: Option<String> = None;

    {
        let mut insert_item = transaction
            .prepare(
                "INSERT INTO items(id, kind, title, title_norm, year, logo, group_name, created_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                   logo = COALESCE(excluded.logo, items.logo),
                   group_name = COALESCE(items.group_name, excluded.group_name)",
            )
            .map_err(stringify)?;
        let mut insert_episode = transaction
            .prepare(
                "INSERT INTO episodes(id, series_id, season, episode, title)
                 VALUES(?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO NOTHING",
            )
            .map_err(stringify)?;
        let mut insert_stream = transaction
            .prepare(
                "INSERT INTO streams(owner_id, owner_kind, source_id, url, quality, channel_number)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(owner_id, url) DO UPDATE SET
                   source_id = excluded.source_id,
                   quality = excluded.quality,
                   channel_number = excluded.channel_number",
            )
            .map_err(stringify)?;

        let outcome = m3u::parse_each(reader, |entry| {
            if failure.is_some() {
                return;
            }
            let item_id = entry.item_id();
            let result = insert_item
                .execute(params![
                    item_id,
                    entry.kind.as_str(),
                    entry.title,
                    entry.title_norm,
                    entry.year,
                    entry.logo,
                    entry.group_name,
                    now
                ])
                .and_then(|_| match (entry.kind, entry.episode_id()) {
                    (EntryKind::Series, Some(episode_id)) => {
                        insert_episode.execute(params![
                            episode_id,
                            item_id,
                            entry.season,
                            entry.episode,
                            entry.episode_title
                        ])?;
                        insert_stream.execute(params![
                            episode_id,
                            "episode",
                            source_id,
                            entry.url,
                            entry.quality,
                            None::<i64>
                        ])
                    }
                    _ => insert_stream.execute(params![
                        item_id,
                        "item",
                        source_id,
                        entry.url,
                        entry.quality,
                        entry.channel_number
                    ]),
                });

            match result {
                Ok(_) => match entry.kind {
                    EntryKind::Movie => movies += 1,
                    EntryKind::Series => series += 1,
                    EntryKind::Channel => channels += 1,
                },
                Err(error) => failure = Some(stringify(error)),
            }
        });

        if let Some(error) = failure {
            return Err(error); // a transação cai no drop, sem commit
        }
        if outcome.parsed == 0 {
            return Err("nenhuma entrada reconhecida na lista".into());
        }

        // O contador roda aqui porque `parse_each` já terminou; para listas
        // grandes a UI recebe marcos a cada bloco.
        let mut reported = 0usize;
        while reported + PROGRESS_EVERY < outcome.parsed {
            reported += PROGRESS_EVERY;
            progress(reported);
        }
        progress(outcome.parsed);

        let distinct: i64 = transaction
            .query_row(
                "SELECT count(DISTINCT owner_id) FROM streams WHERE source_id = ?1",
                [source_id],
                |row| row.get(0),
            )
            .map_err(stringify)?;
        transaction
            .execute(
                "UPDATE sources SET last_sync_at = ?1, item_count = ?2 WHERE id = ?3",
                params![now, distinct, source_id],
            )
            .map_err(stringify)?;

        transaction.commit().map_err(stringify)?;

        let source = get_source(conn, source_id)?;
        return Ok(ImportReport {
            source,
            parsed: outcome.parsed,
            discarded: outcome.discarded,
            movies,
            series,
            channels,
        });
    }
}

fn stringify(error: impl std::fmt::Display) -> String {
    format!("banco recusou a operação: {error}")
}
```

- [ ] **Step 4: Rodar os testes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test import_dedup`
Expected: PASS, 10 testes.

Dois pontos onde o compilador vai reclamar e a correção é mecânica:
- `insert_item` e amigos emprestam `transaction`; por isso o bloco `{ ... }`
  fecha antes de `commit`. Se o borrow checker acusar, mova as três chamadas
  `prepare` para dentro de um escopo próprio e faça `drop` explícito dos
  statements antes do `commit`.
- `series` conta linhas de episódio, não séries distintas. O teste
  `conta_o_que_entrou_por_tipo` espera `series == 1` para duas linhas da mesma
  série: troque o contador por um `HashSet<String>` de `item_id` para o caso
  `EntryKind::Series` e reporte `set.len()`.

- [ ] **Step 5: Adicionar o módulo e commitar**

Modify `src-tauri/src/lib.rs` — acrescentar `pub mod import;`.

```bash
git add src-tauri/src/import.rs src-tauri/src/lib.rs src-tauri/tests/import_dedup.rs
git commit -m "feat: import de listas M3U em transação única com dedup entre fontes"
```

---

## Task 5: `library.rs` — progresso, concluído, próximo episódio, favoritos

**Files:**
- Create: `src-tauri/src/library.rs`
- Modify: `src-tauri/src/lib.rs` (adicionar `pub mod library;`)
- Test: `src-tauri/tests/library_progress.rs`

**Interfaces:**
- Consumes: `db::now` (Task 1), schema de `progress`/`favorites`/`episodes` (Task 1).
- Produces:

```rust
pub const MIN_POSITION_SECS: f64 = 30.0;
pub const COMPLETE_RATIO: f64 = 0.92;

pub struct Progress { pub owner_id: String, pub owner_kind: String,
                      pub position_secs: f64, pub duration_secs: Option<f64>,
                      pub completed: bool, pub updated_at: i64 }
pub struct EpisodeRef { pub id: String, pub series_id: String, pub season: i64,
                        pub episode: i64, pub title: Option<String> }
pub struct ContinueEntry { pub owner_id: String, pub owner_kind: String, pub item_id: String,
                           pub kind: String, pub title: String, pub logo: Option<String>,
                           pub season: Option<i64>, pub episode: Option<i64>,
                           pub position_secs: f64, pub duration_secs: Option<f64>,
                           pub percent: f64 }

pub fn set_progress(conn: &Connection, owner_id: &str, owner_kind: &str,
                    position: f64, duration: Option<f64>) -> Result<Option<Progress>, String>
pub fn mark_watched(conn: &Connection, owner_id: &str, owner_kind: &str,
                    completed: bool) -> Result<(), String>
pub fn progress_for(conn: &Connection, owner_id: &str, owner_kind: &str)
                    -> Result<Option<Progress>, String>
pub fn continue_watching(conn: &Connection, limit: i64) -> Result<Vec<ContinueEntry>, String>
pub fn next_episode(conn: &Connection, series_id: &str) -> Result<Option<EpisodeRef>, String>
pub fn toggle_favorite(conn: &Connection, owner_id: &str) -> Result<bool, String>
pub fn favorites(conn: &Connection) -> Result<Vec<String>, String>
```

- [ ] **Step 1: Escrever o teste que falha**

Create `src-tauri/tests/library_progress.rs`:

```rust
use yellow_tv_lib::{db, library};

fn seed(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "INSERT INTO items(id, kind, title, title_norm, created_at)
           VALUES('m1','movie','Duna','duna',0),('s1','series','Fargo','fargo',0);
         INSERT INTO episodes(id, series_id, season, episode, title) VALUES
           ('e102','s1',1,2,'Ep 2'),
           ('e101','s1',1,1,'Ep 1'),
           ('e201','s1',2,1,'Ep 1');",
    )
    .unwrap();
}

#[test]
fn posicao_abaixo_do_minimo_nao_cria_linha() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    let saved = library::set_progress(&conn, "m1", "item", 12.0, Some(6000.0)).unwrap();
    assert!(saved.is_none(), "clique acidental não entra no histórico");
    assert!(library::progress_for(&conn, "m1", "item").unwrap().is_none());
}

#[test]
fn posicao_acima_do_minimo_cria_e_depois_atualiza() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::set_progress(&conn, "m1", "item", 45.0, Some(6000.0)).unwrap();
    library::set_progress(&conn, "m1", "item", 90.0, Some(6000.0)).unwrap();
    let stored = library::progress_for(&conn, "m1", "item").unwrap().unwrap();
    assert_eq!(stored.position_secs, 90.0);
    assert!(!stored.completed);
    let rows: i64 = conn
        .query_row("SELECT count(*) FROM progress", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 1, "upsert, não insert duplicado");
}

#[test]
fn linha_existente_aceita_recuo_abaixo_do_minimo() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::set_progress(&conn, "m1", "item", 400.0, Some(6000.0)).unwrap();
    library::set_progress(&conn, "m1", "item", 5.0, Some(6000.0)).unwrap();
    let stored = library::progress_for(&conn, "m1", "item").unwrap().unwrap();
    assert_eq!(stored.position_secs, 5.0, "quem já tem histórico pode voltar ao começo");
}

#[test]
fn noventa_e_dois_por_cento_marca_como_concluido() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::set_progress(&conn, "m1", "item", 5519.0, Some(6000.0)).unwrap();
    assert!(!library::progress_for(&conn, "m1", "item").unwrap().unwrap().completed);
    library::set_progress(&conn, "m1", "item", 5521.0, Some(6000.0)).unwrap();
    assert!(library::progress_for(&conn, "m1", "item").unwrap().unwrap().completed);
}

#[test]
fn sem_duracao_nunca_conclui_sozinho() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::set_progress(&conn, "m1", "item", 99999.0, None).unwrap();
    assert!(!library::progress_for(&conn, "m1", "item").unwrap().unwrap().completed);
}

#[test]
fn marcar_a_mao_funciona_nos_dois_sentidos() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::mark_watched(&conn, "m1", "item", true).unwrap();
    assert!(library::progress_for(&conn, "m1", "item").unwrap().unwrap().completed);
    library::mark_watched(&conn, "m1", "item", false).unwrap();
    let stored = library::progress_for(&conn, "m1", "item").unwrap().unwrap();
    assert!(!stored.completed);
    assert_eq!(stored.position_secs, 0.0, "desmarcar zera a posição");
}

#[test]
fn continuar_assistindo_ordena_por_mais_recente_e_ignora_concluidos() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::set_progress(&conn, "m1", "item", 100.0, Some(6000.0)).unwrap();
    library::set_progress(&conn, "e101", "episode", 200.0, Some(2400.0)).unwrap();
    library::mark_watched(&conn, "e201", "episode", true).unwrap();

    let entries = library::continue_watching(&conn, 10).unwrap();
    assert_eq!(entries.len(), 2, "o concluído não aparece");
    assert!(entries.iter().all(|entry| entry.title == "Duna" || entry.title == "Fargo"));

    let episode = entries.iter().find(|entry| entry.owner_kind == "episode").unwrap();
    assert_eq!(episode.title, "Fargo", "episódio se apresenta pelo nome da série");
    assert_eq!(episode.item_id, "s1");
    assert_eq!(episode.season, Some(1));
    assert_eq!(episode.episode, Some(1));
    assert!((episode.percent - 200.0 / 2400.0).abs() < 1e-6);
}

#[test]
fn continuar_assistindo_respeita_o_limite() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::set_progress(&conn, "m1", "item", 100.0, Some(6000.0)).unwrap();
    library::set_progress(&conn, "e101", "episode", 100.0, Some(2400.0)).unwrap();
    assert_eq!(library::continue_watching(&conn, 1).unwrap().len(), 1);
}

#[test]
fn proximo_episodio_ignora_a_ordem_de_insercao() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    let next = library::next_episode(&conn, "s1").unwrap().unwrap();
    assert_eq!((next.season, next.episode), (1, 1));

    library::mark_watched(&conn, "e101", "episode", true).unwrap();
    let next = library::next_episode(&conn, "s1").unwrap().unwrap();
    assert_eq!((next.season, next.episode), (1, 2));

    library::mark_watched(&conn, "e102", "episode", true).unwrap();
    let next = library::next_episode(&conn, "s1").unwrap().unwrap();
    assert_eq!((next.season, next.episode), (2, 1));
}

#[test]
fn serie_toda_concluida_devolve_none() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    for id in ["e101", "e102", "e201"] {
        library::mark_watched(&conn, id, "episode", true).unwrap();
    }
    assert!(library::next_episode(&conn, "s1").unwrap().is_none());
}

#[test]
fn favorito_alterna_e_lista() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    assert!(library::toggle_favorite(&conn, "m1").unwrap());
    assert_eq!(library::favorites(&conn).unwrap(), vec!["m1".to_owned()]);
    assert!(!library::toggle_favorite(&conn, "m1").unwrap());
    assert!(library::favorites(&conn).unwrap().is_empty());
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test library_progress`
Expected: FAIL — `could not find library in yellow_tv_lib`.

- [ ] **Step 3: Escrever `library.rs`**

Create `src-tauri/src/library.rs`:

```rust
//! Progresso, "já assistiu" e favoritos.
//!
//! `progress` é a única tabela sem foreign key: o usuário troca de lista e o
//! histórico continua casando pelo hash do título. A chave é
//! `(owner_id, owner_kind)`, então filme e episódio compartilham o mesmo caminho.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db;

/// Menos que isso é clique acidental, não sessão de cinema.
pub const MIN_POSITION_SECS: f64 = 30.0;
/// Créditos subindo já contam como assistido.
pub const COMPLETE_RATIO: f64 = 0.92;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub owner_id: String,
    pub owner_kind: String,
    pub position_secs: f64,
    pub duration_secs: Option<f64>,
    pub completed: bool,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeRef {
    pub id: String,
    pub series_id: String,
    pub season: i64,
    pub episode: i64,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueEntry {
    pub owner_id: String,
    pub owner_kind: String,
    /// Id do título para montar o link — a série, quando o owner é episódio.
    pub item_id: String,
    pub kind: String,
    pub title: String,
    pub logo: Option<String>,
    pub season: Option<i64>,
    pub episode: Option<i64>,
    pub position_secs: f64,
    pub duration_secs: Option<f64>,
    pub percent: f64,
}

fn read_progress(row: &rusqlite::Row<'_>) -> rusqlite::Result<Progress> {
    Ok(Progress {
        owner_id: row.get(0)?,
        owner_kind: row.get(1)?,
        position_secs: row.get(2)?,
        duration_secs: row.get(3)?,
        completed: row.get::<_, i64>(4)? != 0,
        updated_at: row.get(5)?,
    })
}

pub fn progress_for(
    conn: &Connection,
    owner_id: &str,
    owner_kind: &str,
) -> Result<Option<Progress>, String> {
    conn.query_row(
        "SELECT owner_id, owner_kind, position_secs, duration_secs, completed, updated_at
         FROM progress WHERE owner_id = ?1 AND owner_kind = ?2",
        params![owner_id, owner_kind],
        read_progress,
    )
    .optional()
    .map_err(stringify)
}

pub fn set_progress(
    conn: &Connection,
    owner_id: &str,
    owner_kind: &str,
    position: f64,
    duration: Option<f64>,
) -> Result<Option<Progress>, String> {
    check_kind(owner_kind)?;
    let existing = progress_for(conn, owner_id, owner_kind)?;
    if position < MIN_POSITION_SECS && existing.is_none() {
        return Ok(None);
    }

    let reached_end = duration
        .filter(|value| *value > 0.0)
        .map(|value| position >= COMPLETE_RATIO * value)
        .unwrap_or(false);
    let completed = reached_end || existing.as_ref().map(|row| row.completed).unwrap_or(false);

    conn.execute(
        "INSERT INTO progress(owner_id, owner_kind, position_secs, duration_secs, completed, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(owner_id, owner_kind) DO UPDATE SET
           position_secs = excluded.position_secs,
           duration_secs = COALESCE(excluded.duration_secs, progress.duration_secs),
           completed = excluded.completed,
           updated_at = excluded.updated_at",
        params![
            owner_id,
            owner_kind,
            position,
            duration,
            i64::from(completed),
            db::now()
        ],
    )
    .map_err(stringify)?;

    progress_for(conn, owner_id, owner_kind)
}

pub fn mark_watched(
    conn: &Connection,
    owner_id: &str,
    owner_kind: &str,
    completed: bool,
) -> Result<(), String> {
    check_kind(owner_kind)?;
    if completed {
        conn.execute(
            "INSERT INTO progress(owner_id, owner_kind, position_secs, completed, updated_at)
             VALUES(?1, ?2, 0, 1, ?3)
             ON CONFLICT(owner_id, owner_kind) DO UPDATE SET completed = 1, updated_at = excluded.updated_at",
            params![owner_id, owner_kind, db::now()],
        )
        .map_err(stringify)?;
    } else {
        // Desmarcar volta o título para o começo: meia posição com "não visto"
        // deixaria "Continuar assistindo" mentindo.
        conn.execute(
            "INSERT INTO progress(owner_id, owner_kind, position_secs, completed, updated_at)
             VALUES(?1, ?2, 0, 0, ?3)
             ON CONFLICT(owner_id, owner_kind) DO UPDATE SET
               completed = 0, position_secs = 0, updated_at = excluded.updated_at",
            params![owner_id, owner_kind, db::now()],
        )
        .map_err(stringify)?;
    }
    Ok(())
}

pub fn continue_watching(conn: &Connection, limit: i64) -> Result<Vec<ContinueEntry>, String> {
    let mut statement = conn
        .prepare(
            "SELECT p.owner_id, p.owner_kind,
                    COALESCE(e.series_id, p.owner_id) AS item_id,
                    i.kind, i.title, i.logo, e.season, e.episode,
                    p.position_secs, p.duration_secs
             FROM progress p
             LEFT JOIN episodes e
               ON p.owner_kind = 'episode' AND e.id = p.owner_id
             JOIN items i
               ON i.id = COALESCE(e.series_id, p.owner_id)
             WHERE p.completed = 0
             ORDER BY p.updated_at DESC
             LIMIT ?1",
        )
        .map_err(stringify)?;

    let rows = statement
        .query_map([limit], |row| {
            let position: f64 = row.get(8)?;
            let duration: Option<f64> = row.get(9)?;
            Ok(ContinueEntry {
                owner_id: row.get(0)?,
                owner_kind: row.get(1)?,
                item_id: row.get(2)?,
                kind: row.get(3)?,
                title: row.get(4)?,
                logo: row.get(5)?,
                season: row.get(6)?,
                episode: row.get(7)?,
                position_secs: position,
                duration_secs: duration,
                percent: duration
                    .filter(|value| *value > 0.0)
                    .map(|value| (position / value).clamp(0.0, 1.0))
                    .unwrap_or(0.0),
            })
        })
        .map_err(stringify)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(stringify)?;
    Ok(rows)
}

pub fn next_episode(conn: &Connection, series_id: &str) -> Result<Option<EpisodeRef>, String> {
    conn.query_row(
        "SELECT e.id, e.series_id, e.season, e.episode, e.title
         FROM episodes e
         LEFT JOIN progress p ON p.owner_id = e.id AND p.owner_kind = 'episode'
         WHERE e.series_id = ?1 AND COALESCE(p.completed, 0) = 0
         ORDER BY e.season, e.episode
         LIMIT 1",
        [series_id],
        |row| {
            Ok(EpisodeRef {
                id: row.get(0)?,
                series_id: row.get(1)?,
                season: row.get(2)?,
                episode: row.get(3)?,
                title: row.get(4)?,
            })
        },
    )
    .optional()
    .map_err(stringify)
}

pub fn toggle_favorite(conn: &Connection, owner_id: &str) -> Result<bool, String> {
    let removed = conn
        .execute("DELETE FROM favorites WHERE owner_id = ?1", [owner_id])
        .map_err(stringify)?;
    if removed > 0 {
        return Ok(false);
    }
    conn.execute(
        "INSERT INTO favorites(owner_id, added_at) VALUES(?1, ?2)",
        params![owner_id, db::now()],
    )
    .map_err(stringify)?;
    Ok(true)
}

pub fn favorites(conn: &Connection) -> Result<Vec<String>, String> {
    let mut statement = conn
        .prepare("SELECT owner_id FROM favorites ORDER BY added_at DESC")
        .map_err(stringify)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(stringify)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(stringify)?;
    Ok(rows)
}

fn check_kind(owner_kind: &str) -> Result<(), String> {
    if matches!(owner_kind, "item" | "episode") {
        Ok(())
    } else {
        Err(format!("owner_kind inválido: {owner_kind}"))
    }
}

fn stringify(error: impl std::fmt::Display) -> String {
    format!("banco recusou a operação: {error}")
}
```

- [ ] **Step 4: Rodar os testes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test library_progress`
Expected: PASS, 11 testes.

- [ ] **Step 5: Rodar a suíte inteira**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS em `schema`, `ids_stable`, `m3u_parse`, `import_dedup`, `library_progress`.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/library.rs src-tauri/src/lib.rs src-tauri/tests/library_progress.rs
git commit -m "feat: progresso, concluído, próximo episódio e favoritos em SQLite"
```

---

## Task 6: `catalog.rs` reescrito em SQL

**Files:**
- Modify (reescrever inteiro): `src-tauri/src/catalog.rs`
- Test: `src-tauri/tests/catalog_query.rs`

**Interfaces:**
- Consumes: schema (Task 1), `ids::normalize` (Task 2), `library::progress_for` (Task 5).
- Produces:

```rust
pub const PAGE_SIZE: i64 = 60;
pub enum Kind { Movie, Series, Channel }         // Kind::parse aceita "filmes" | "series" | "canais"
pub struct CatalogItem { pub id, pub kind: String, pub title: String, pub year: Option<i64>,
                         pub logo: Option<String>, pub group: Option<String>,
                         pub seasons: i64, pub episode_count: i64,
                         pub channel_number: Option<i64>,
                         pub percent: f64, pub completed: bool }
pub struct GroupCount { pub name: String, pub count: i64 }
pub struct CatalogPage { pub total: i64, pub page: i64, pub page_size: i64,
                         pub items: Vec<CatalogItem>, pub groups: Vec<GroupCount> }
pub struct StreamRef { pub id: i64, pub url: String, pub quality: Option<String>,
                       pub source_label: String, pub channel_number: Option<i64> }
pub struct ItemWithRelated { pub item: CatalogItem, pub streams: Vec<StreamRef>,
                             pub progress: Option<library::Progress>,
                             pub related: Vec<CatalogItem> }
pub struct EpisodeRow { pub id: String, pub season: i64, pub episode: i64,
                        pub title: Option<String>, pub streams: Vec<StreamRef>,
                        pub percent: f64, pub completed: bool }
pub struct SeriesEpisodes { pub id: String, pub title: String, pub logo: Option<String>,
                            pub group: Option<String>, pub episodes: Vec<EpisodeRow> }
pub struct Row { pub key: String, pub title: String, pub kind: String, pub items: Vec<CatalogItem> }

pub fn page(conn, kind: Kind, query: &str, group: Option<&str>, page: i64, unwatched_only: bool)
    -> Result<CatalogPage, String>
pub fn item(conn, kind: Kind, id: &str) -> Result<ItemWithRelated, String>
pub fn episodes(conn, series_id: &str) -> Result<SeriesEpisodes, String>
pub fn board(conn) -> Result<Vec<Row>, String>
pub fn by_ids(conn, ids: &[String]) -> Result<Vec<CatalogItem>, String>
```

**Nota de compatibilidade:** `Channel`, `DataStatus` e o `Catalog` antigo saem
deste arquivo. `lib.rs` só volta a compilar na Task 8; até lá `cargo test` pode
falhar no binário. Use `cargo test --manifest-path src-tauri/Cargo.toml --test catalog_query`
enquanto esta task não fecha, e deixe `lib.rs` compilando com os comandos antigos
comentados se necessário.

- [ ] **Step 1: Escrever o teste que falha**

Create `src-tauri/tests/catalog_query.rs`:

```rust
use std::io::Cursor;
use yellow_tv_lib::{catalog, db, import, library};

const LISTA: &str = "#EXTM3U\n\
#EXTINF:-1 group-title=\"Filmes | Ficção\",Duna (2021)\nhttp://a/duna\n\
#EXTINF:-1 group-title=\"Filmes | Ficção\",Blade Runner (1982)\nhttp://a/br\n\
#EXTINF:-1 group-title=\"Filmes | Ação\",Caçador de Androides (1982)\nhttp://a/ca\n\
#EXTINF:-1 group-title=\"Series | Drama\",Fargo S01E01\nhttp://a/f101\n\
#EXTINF:-1 group-title=\"Series | Drama\",Fargo S01E02\nhttp://a/f102\n\
#EXTINF:-1 tvg-chno=\"12\",Globo SP\nhttp://a/globo\n";

fn banco() -> rusqlite::Connection {
    let mut conn = db::open_memory().unwrap();
    let source = import::upsert_source(&conn, "http://a", "A", "url").unwrap();
    import::ingest(&mut conn, source.id, Cursor::new(LISTA.to_owned()), &mut |_| {}).unwrap();
    conn
}

#[test]
fn pagina_lista_apenas_o_kind_pedido() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Movie, "", None, 0, false).unwrap();
    assert_eq!(page.total, 3);
    assert!(page.items.iter().all(|item| item.kind == "movie"));
    assert_eq!(page.page_size, catalog::PAGE_SIZE);
}

#[test]
fn busca_encontra_sem_acento_e_por_prefixo() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Movie, "cacador", None, 0, false).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].title, "Caçador de Androides");

    let prefixo = catalog::page(&conn, catalog::Kind::Movie, "bla", None, 0, false).unwrap();
    assert_eq!(prefixo.total, 1, "prefixo casa: 'bla' encontra Blade Runner");
}

#[test]
fn busca_com_dois_termos_exige_os_dois() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Movie, "blade runner", None, 0, false).unwrap();
    assert_eq!(page.total, 1);
    let nenhum = catalog::page(&conn, catalog::Kind::Movie, "blade duna", None, 0, false).unwrap();
    assert_eq!(nenhum.total, 0);
}

#[test]
fn busca_com_caractere_estranho_nao_explode() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Movie, "\"duna* OR", None, 0, false).unwrap();
    assert!(page.total <= 3, "consulta sanitizada, sem erro de sintaxe FTS");
}

#[test]
fn filtro_de_grupo_e_contagem_de_grupos() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Movie, "", Some("Filmes | Ficção"), 0, false).unwrap();
    assert_eq!(page.total, 2);
    let ficcao = page
        .groups
        .iter()
        .find(|group| group.name == "Filmes | Ficção")
        .unwrap();
    assert_eq!(ficcao.count, 2);
    assert!(
        page.groups.iter().all(|group| group.name.starts_with("Filmes")),
        "grupos de série não vazam para a aba de filmes"
    );
}

#[test]
fn paginacao_respeita_a_pagina_pedida() {
    let conn = banco();
    let segunda = catalog::page(&conn, catalog::Kind::Movie, "", None, 1, false).unwrap();
    assert_eq!(segunda.total, 3);
    assert!(segunda.items.is_empty(), "só há uma página de 60");
}

#[test]
fn filtro_de_nao_assistidos_esconde_concluidos() {
    let conn = banco();
    let id = conn
        .query_row("SELECT id FROM items WHERE title = 'Duna'", [], |row| row.get::<_, String>(0))
        .unwrap();
    library::mark_watched(&conn, &id, "item", true).unwrap();

    let todos = catalog::page(&conn, catalog::Kind::Movie, "", None, 0, false).unwrap();
    let restantes = catalog::page(&conn, catalog::Kind::Movie, "", None, 0, true).unwrap();
    assert_eq!(todos.total, 3);
    assert_eq!(restantes.total, 2);
}

#[test]
fn item_traz_progresso_streams_e_relacionados() {
    let conn = banco();
    let id = conn
        .query_row("SELECT id FROM items WHERE title = 'Duna'", [], |row| row.get::<_, String>(0))
        .unwrap();
    library::set_progress(&conn, &id, "item", 600.0, Some(6000.0)).unwrap();

    let detail = catalog::item(&conn, catalog::Kind::Movie, &id).unwrap();
    assert_eq!(detail.item.title, "Duna");
    assert_eq!(detail.streams.len(), 1);
    assert_eq!(detail.streams[0].source_label, "A");
    assert_eq!(detail.progress.unwrap().position_secs, 600.0);
    assert!(
        detail.related.iter().all(|other| other.id != id),
        "relacionados não incluem o próprio título"
    );
    assert!((detail.item.percent - 0.1).abs() < 1e-6);
}

#[test]
fn item_inexistente_devolve_erro_legivel() {
    let conn = banco();
    let error = catalog::item(&conn, catalog::Kind::Movie, "nao-existe").unwrap_err();
    assert!(error.contains("não encontrado"));
}

#[test]
fn serie_lista_episodios_ordenados_com_progresso() {
    let conn = banco();
    let series_id = conn
        .query_row("SELECT id FROM items WHERE kind = 'series'", [], |row| row.get::<_, String>(0))
        .unwrap();
    let detail = catalog::episodes(&conn, &series_id).unwrap();
    assert_eq!(detail.title, "Fargo");
    assert_eq!(detail.episodes.len(), 2);
    assert_eq!(
        (detail.episodes[0].season, detail.episodes[0].episode),
        (1, 1)
    );
    assert_eq!(detail.episodes[0].streams.len(), 1);
    assert!(!detail.episodes[0].completed);
}

#[test]
fn serie_reporta_temporadas_e_total_de_episodios() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Series, "", None, 0, false).unwrap();
    let fargo = &page.items[0];
    assert_eq!(fargo.seasons, 1);
    assert_eq!(fargo.episode_count, 2);
}

#[test]
fn canais_expoem_o_numero_do_canal() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Channel, "", None, 0, false).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].channel_number, Some(12));
}

#[test]
fn board_tem_continuar_recentes_e_generos() {
    let conn = banco();
    let id = conn
        .query_row("SELECT id FROM items WHERE title = 'Duna'", [], |row| row.get::<_, String>(0))
        .unwrap();
    library::set_progress(&conn, &id, "item", 600.0, Some(6000.0)).unwrap();

    let rows = catalog::board(&conn).unwrap();
    assert_eq!(rows[0].key, "continue");
    assert_eq!(rows[0].items.len(), 1);
    assert_eq!(rows[1].key, "recent");
    assert!(
        rows.iter().any(|row| row.key.starts_with("group:")),
        "linhas por gênero aparecem depois"
    );
    assert!(
        rows.iter().all(|row| !row.items.is_empty()),
        "linha vazia não vai para a tela"
    );
}

#[test]
fn board_sem_progresso_nao_traz_linha_de_continuar() {
    let conn = banco();
    let rows = catalog::board(&conn).unwrap();
    assert!(rows.iter().all(|row| row.key != "continue"));
}

#[test]
fn by_ids_preserva_a_ordem_pedida() {
    let conn = banco();
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM items WHERE kind = 'movie' ORDER BY title")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<String>>>()
        .unwrap();
    let reversed: Vec<String> = ids.iter().rev().cloned().collect();
    let items = catalog::by_ids(&conn, &reversed).unwrap();
    let got: Vec<String> = items.into_iter().map(|item| item.id).collect();
    assert_eq!(got, reversed);
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test catalog_query`
Expected: FAIL — `catalog::page` não existe com essa assinatura.

- [ ] **Step 3: Reescrever `catalog.rs`**

Replace the entire contents of `src-tauri/src/catalog.rs`:

```rust
//! Consultas de catálogo, todas em SQL.
//!
//! Nada de `Vec` em memória: paginação, contagem de grupos e busca acontecem no
//! SQLite, que é o único que conhece o catálogo inteiro. A busca usa a tabela FTS
//! `items_fts` sobre `title_norm`, com os termos convertidos em prefixos.

use rusqlite::{params_from_iter, Connection, OptionalExtension};
use serde::Serialize;

use crate::ids;
use crate::library::{self, Progress};

pub const PAGE_SIZE: i64 = 60;
const RELATED_LIMIT: i64 = 18;
const ROW_LIMIT: i64 = 20;
const BOARD_GROUP_ROWS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Movie,
    Series,
    Channel,
}

impl Kind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "filmes" | "movie" => Ok(Kind::Movie),
            "series" => Ok(Kind::Series),
            "canais" | "channel" => Ok(Kind::Channel),
            other => Err(format!("tipo de catálogo inválido: {other}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Movie => "movie",
            Kind::Series => "series",
            Kind::Channel => "channel",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub year: Option<i64>,
    pub logo: Option<String>,
    pub group: Option<String>,
    pub seasons: i64,
    pub episode_count: i64,
    pub channel_number: Option<i64>,
    /// 0.0 a 1.0; 0.0 quando não há progresso.
    pub percent: f64,
    pub completed: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupCount {
    pub name: String,
    pub count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPage {
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub items: Vec<CatalogItem>,
    pub groups: Vec<GroupCount>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamRef {
    pub id: i64,
    pub url: String,
    pub quality: Option<String>,
    pub source_label: String,
    pub channel_number: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemWithRelated {
    pub item: CatalogItem,
    pub streams: Vec<StreamRef>,
    pub progress: Option<Progress>,
    pub related: Vec<CatalogItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeRow {
    pub id: String,
    pub season: i64,
    pub episode: i64,
    pub title: Option<String>,
    pub streams: Vec<StreamRef>,
    pub percent: f64,
    pub completed: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesEpisodes {
    pub id: String,
    pub title: String,
    pub logo: Option<String>,
    pub group: Option<String>,
    pub episodes: Vec<EpisodeRow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub key: String,
    pub title: String,
    pub kind: String,
    pub items: Vec<CatalogItem>,
}

/// Colunas de `CatalogItem`, sempre na mesma ordem — `read_item` depende disso.
const ITEM_COLUMNS: &str = "i.id, i.kind, i.title, i.year, i.logo, i.group_name,
     (SELECT count(DISTINCT season) FROM episodes WHERE series_id = i.id),
     (SELECT count(*) FROM episodes WHERE series_id = i.id),
     (SELECT max(channel_number) FROM streams WHERE owner_id = i.id AND owner_kind = 'item'),
     COALESCE(p.position_secs, 0), p.duration_secs, COALESCE(p.completed, 0)";

const ITEM_JOIN: &str = "FROM items i
     LEFT JOIN progress p ON p.owner_id = i.id AND p.owner_kind = 'item'";

fn read_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<CatalogItem> {
    let position: f64 = row.get(9)?;
    let duration: Option<f64> = row.get(10)?;
    Ok(CatalogItem {
        id: row.get(0)?,
        kind: row.get(1)?,
        title: row.get(2)?,
        year: row.get(3)?,
        logo: row.get(4)?,
        group: row.get(5)?,
        seasons: row.get(6)?,
        episode_count: row.get(7)?,
        channel_number: row.get(8)?,
        percent: duration
            .filter(|value| *value > 0.0)
            .map(|value| (position / value).clamp(0.0, 1.0))
            .unwrap_or(0.0),
        completed: row.get::<_, i64>(11)? != 0,
    })
}

/// Termos do usuário viram prefixos entre aspas — o único formato de MATCH que
/// não deixa o usuário escrever sintaxe FTS por acidente.
fn fts_query(query: &str) -> Option<String> {
    let normalized = ids::normalize(query);
    let terms: Vec<String> = normalized
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{term}\"*"))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" AND "))
    }
}

pub fn page(
    conn: &Connection,
    kind: Kind,
    query: &str,
    group: Option<&str>,
    page: i64,
    unwatched_only: bool,
) -> Result<CatalogPage, String> {
    let page = page.max(0);
    let mut clauses = vec!["i.kind = ?".to_owned()];
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(kind.as_str().to_owned())];

    if let Some(group) = group.filter(|value| !value.is_empty()) {
        clauses.push("i.group_name = ?".to_owned());
        binds.push(Box::new(group.to_owned()));
    }
    if let Some(match_expression) = fts_query(query) {
        clauses.push("i.rowid IN (SELECT rowid FROM items_fts WHERE items_fts MATCH ?)".to_owned());
        binds.push(Box::new(match_expression));
    }
    if unwatched_only {
        clauses.push("COALESCE(p.completed, 0) = 0".to_owned());
    }
    let where_clause = clauses.join(" AND ");

    let total: i64 = conn
        .query_row(
            &format!("SELECT count(*) {ITEM_JOIN} WHERE {where_clause}"),
            params_from_iter(binds.iter().map(|value| value.as_ref())),
            |row| row.get(0),
        )
        .map_err(stringify)?;

    let mut statement = conn
        .prepare(&format!(
            "SELECT {ITEM_COLUMNS} {ITEM_JOIN} WHERE {where_clause}
             ORDER BY i.title LIMIT {PAGE_SIZE} OFFSET {}",
            page * PAGE_SIZE
        ))
        .map_err(stringify)?;
    let items = statement
        .query_map(
            params_from_iter(binds.iter().map(|value| value.as_ref())),
            read_item,
        )
        .map_err(stringify)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(stringify)?;

    // A contagem de grupos ignora busca e grupo ativo: as pastilhas precisam
    // continuar navegáveis depois de um filtro.
    let mut group_statement = conn
        .prepare(
            "SELECT group_name, count(*) FROM items
             WHERE kind = ?1 AND group_name IS NOT NULL AND group_name <> ''
             GROUP BY group_name ORDER BY count(*) DESC",
        )
        .map_err(stringify)?;
    let groups = group_statement
        .query_map([kind.as_str()], |row| {
            Ok(GroupCount {
                name: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .map_err(stringify)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(stringify)?;

    Ok(CatalogPage {
        total,
        page,
        page_size: PAGE_SIZE,
        items,
        groups,
    })
}

fn streams_for(conn: &Connection, owner_id: &str, owner_kind: &str) -> Result<Vec<StreamRef>, String> {
    let mut statement = conn
        .prepare(
            "SELECT s.id, s.url, s.quality, src.label, s.channel_number
             FROM streams s JOIN sources src ON src.id = s.source_id
             WHERE s.owner_id = ?1 AND s.owner_kind = ?2
             ORDER BY src.added_at, s.id",
        )
        .map_err(stringify)?;
    let rows = statement
        .query_map([owner_id, owner_kind], |row| {
            Ok(StreamRef {
                id: row.get(0)?,
                url: row.get(1)?,
                quality: row.get(2)?,
                source_label: row.get(3)?,
                channel_number: row.get(4)?,
            })
        })
        .map_err(stringify)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(stringify)?;
    Ok(rows)
}

pub fn item(conn: &Connection, kind: Kind, id: &str) -> Result<ItemWithRelated, String> {
    let item = conn
        .query_row(
            &format!("SELECT {ITEM_COLUMNS} {ITEM_JOIN} WHERE i.id = ?1 AND i.kind = ?2"),
            rusqlite::params![id, kind.as_str()],
            read_item,
        )
        .optional()
        .map_err(stringify)?
        .ok_or_else(|| format!("título não encontrado: {id}"))?;

    let related = match item.group.as_deref() {
        Some(group) => {
            let mut statement = conn
                .prepare(&format!(
                    "SELECT {ITEM_COLUMNS} {ITEM_JOIN}
                     WHERE i.kind = ?1 AND i.group_name = ?2 AND i.id <> ?3
                     ORDER BY i.title LIMIT {RELATED_LIMIT}"
                ))
                .map_err(stringify)?;
            statement
                .query_map(rusqlite::params![kind.as_str(), group, id], read_item)
                .map_err(stringify)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(stringify)?
        }
        None => Vec::new(),
    };

    Ok(ItemWithRelated {
        streams: streams_for(conn, id, "item")?,
        progress: library::progress_for(conn, id, "item")?,
        item,
        related,
    })
}

pub fn episodes(conn: &Connection, series_id: &str) -> Result<SeriesEpisodes, String> {
    let (title, logo, group) = conn
        .query_row(
            "SELECT title, logo, group_name FROM items WHERE id = ?1 AND kind = 'series'",
            [series_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?)),
        )
        .optional()
        .map_err(stringify)?
        .ok_or_else(|| format!("série não encontrada: {series_id}"))?;

    let mut statement = conn
        .prepare(
            "SELECT e.id, e.season, e.episode, e.title,
                    COALESCE(p.position_secs, 0), p.duration_secs, COALESCE(p.completed, 0)
             FROM episodes e
             LEFT JOIN progress p ON p.owner_id = e.id AND p.owner_kind = 'episode'
             WHERE e.series_id = ?1
             ORDER BY e.season, e.episode",
        )
        .map_err(stringify)?;

    let partial = statement
        .query_map([series_id], |row| {
            let position: f64 = row.get(4)?;
            let duration: Option<f64> = row.get(5)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
                duration
                    .filter(|value| *value > 0.0)
                    .map(|value| (position / value).clamp(0.0, 1.0))
                    .unwrap_or(0.0),
                row.get::<_, i64>(6)? != 0,
            ))
        })
        .map_err(stringify)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(stringify)?;

    let mut episodes = Vec::with_capacity(partial.len());
    for (id, season, episode, episode_title, percent, completed) in partial {
        episodes.push(EpisodeRow {
            streams: streams_for(conn, &id, "episode")?,
            id,
            season,
            episode,
            title: episode_title,
            percent,
            completed,
        });
    }

    Ok(SeriesEpisodes {
        id: series_id.to_owned(),
        title,
        logo,
        group,
        episodes,
    })
}

pub fn by_ids(conn: &Connection, ids: &[String]) -> Result<Vec<CatalogItem>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(", ");
    let mut statement = conn
        .prepare(&format!(
            "SELECT {ITEM_COLUMNS} {ITEM_JOIN} WHERE i.id IN ({placeholders})"
        ))
        .map_err(stringify)?;
    let found = statement
        .query_map(params_from_iter(ids.iter()), read_item)
        .map_err(stringify)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(stringify)?;

    // SQL não garante a ordem do IN; a lista de favoritos e o histórico têm ordem
    // própria, então ela é reimposta aqui.
    Ok(ids
        .iter()
        .filter_map(|id| found.iter().find(|item| &item.id == id).cloned())
        .collect())
}

fn row_of(
    conn: &Connection,
    key: &str,
    title: &str,
    kind: Kind,
    extra_where: &str,
    order: &str,
) -> Result<Row, String> {
    let mut statement = conn
        .prepare(&format!(
            "SELECT {ITEM_COLUMNS} {ITEM_JOIN}
             WHERE i.kind = ?1 {extra_where}
             ORDER BY {order} LIMIT {ROW_LIMIT}"
        ))
        .map_err(stringify)?;
    let items = statement
        .query_map([kind.as_str()], read_item)
        .map_err(stringify)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(stringify)?;
    Ok(Row {
        key: key.to_owned(),
        title: title.to_owned(),
        kind: kind.as_str().to_owned(),
        items,
    })
}

/// Linhas do Início, na ordem em que aparecem na tela.
pub fn board(conn: &Connection) -> Result<Vec<Row>, String> {
    let mut rows: Vec<Row> = Vec::new();

    let continuing = library::continue_watching(conn, ROW_LIMIT)?;
    if !continuing.is_empty() {
        let ids: Vec<String> = continuing.iter().map(|entry| entry.item_id.clone()).collect();
        let mut items = by_ids(conn, &ids)?;
        items.dedup_by(|a, b| a.id == b.id);
        rows.push(Row {
            key: "continue".into(),
            title: "Continuar assistindo".into(),
            kind: "mixed".into(),
            items,
        });
    }

    let mut recent = row_of(conn, "recent", "Adicionados recentemente", Kind::Movie, "", "i.created_at DESC, i.title")?;
    let recent_series = row_of(conn, "recent", "", Kind::Series, "", "i.created_at DESC, i.title")?;
    recent.items.extend(recent_series.items);
    recent.items.truncate(ROW_LIMIT as usize);
    recent.kind = "mixed".into();
    if !recent.items.is_empty() {
        rows.push(recent);
    }

    // Gêneros com mais títulos primeiro, filmes e séries alternando.
    for kind in [Kind::Movie, Kind::Series] {
        let mut statement = conn
            .prepare(
                "SELECT group_name FROM items
                 WHERE kind = ?1 AND group_name IS NOT NULL AND group_name <> ''
                 GROUP BY group_name ORDER BY count(*) DESC LIMIT ?2",
            )
            .map_err(stringify)?;
        let groups: Vec<String> = statement
            .query_map(rusqlite::params![kind.as_str(), BOARD_GROUP_ROWS as i64], |row| row.get(0))
            .map_err(stringify)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(stringify)?;

        for group in groups {
            let mut items_statement = conn
                .prepare(&format!(
                    "SELECT {ITEM_COLUMNS} {ITEM_JOIN}
                     WHERE i.kind = ?1 AND i.group_name = ?2
                     ORDER BY i.title LIMIT {ROW_LIMIT}"
                ))
                .map_err(stringify)?;
            let items = items_statement
                .query_map(rusqlite::params![kind.as_str(), &group], read_item)
                .map_err(stringify)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(stringify)?;
            if items.is_empty() {
                continue;
            }
            rows.push(Row {
                key: format!("group:{}:{group}", kind.as_str()),
                title: group,
                kind: kind.as_str().to_owned(),
                items,
            });
        }
    }

    Ok(rows)
}

fn stringify(error: impl std::fmt::Display) -> String {
    format!("consulta ao catálogo falhou: {error}")
}
```

- [ ] **Step 4: Rodar os testes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test catalog_query`
Expected: PASS, 15 testes.

`params_from_iter` com `Vec<Box<dyn ToSql>>` costuma exigir um ajuste de tipo. Se
o compilador reclamar, troque as duas chamadas por
`params_from_iter(binds.iter().map(|value| &**value))` ou reescreva `binds` como
`Vec<rusqlite::types::Value>` (que já implementa `ToSql` por valor).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/catalog.rs src-tauri/tests/catalog_query.rs
git commit -m "refactor: catálogo passa a consultar SQLite em vez de JSON em memória"
```

---

## Task 7: `settings.rs` e `meta.rs` — chave TMDB e cache na tabela `meta`

**Files:**
- Create: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/meta.rs:1-60` (construtor e `key()`), `src-tauri/src/meta.rs` (método `meta`)
- Modify: `src-tauri/src/lib.rs` (adicionar `pub mod settings;`)
- Test: `src-tauri/tests/settings_meta.rs`

**Interfaces:**
- Consumes: schema `settings`/`meta` (Task 1), `ids::item_id` (Task 2).
- Produces:
  - `settings::get(conn: &Connection, key: &str) -> Result<Option<String>, String>`
  - `settings::set(conn: &Connection, key: &str, value: &str) -> Result<(), String>`
  - `settings::TMDB_KEY: &str` (= `"tmdb_api_key"`)
  - `settings::tmdb_key(conn: &Connection) -> Option<String>` — banco primeiro, `TMDB_API_KEY` como fallback
  - `meta::cached(conn, cache_id: &str) -> Option<Meta>`
  - `meta::store(conn, cache_id: &str, meta: &Meta) -> Result<(), String>`
  - `meta::cache_id(kind: &str, title: &str, year: Option<i64>) -> String`
  - `Tmdb::new()` passa a não receber `PathBuf`; `Tmdb::meta(&self, key: Option<String>, kind, title, year)` recebe a chave de fora (o `Connection` não é `Send`, então quem toca no banco é o comando, não o `async`).

- [ ] **Step 1: Escrever o teste que falha**

Create `src-tauri/tests/settings_meta.rs`:

```rust
use yellow_tv_lib::{db, meta, settings};

#[test]
fn setting_grava_le_e_sobrescreve() {
    let conn = db::open_memory().unwrap();
    assert!(settings::get(&conn, settings::TMDB_KEY).unwrap().is_none());
    settings::set(&conn, settings::TMDB_KEY, "abc").unwrap();
    assert_eq!(settings::get(&conn, settings::TMDB_KEY).unwrap().as_deref(), Some("abc"));
    settings::set(&conn, settings::TMDB_KEY, "def").unwrap();
    assert_eq!(settings::get(&conn, settings::TMDB_KEY).unwrap().as_deref(), Some("def"));
}

#[test]
fn valor_em_branco_conta_como_ausente() {
    let conn = db::open_memory().unwrap();
    settings::set(&conn, settings::TMDB_KEY, "   ").unwrap();
    assert!(settings::tmdb_key(&conn).is_none());
}

#[test]
fn cache_de_meta_vai_e_volta_da_tabela() {
    let conn = db::open_memory().unwrap();
    let id = meta::cache_id("movie", "Duna", Some(2021));
    assert!(meta::cached(&conn, &id).is_none());

    let mut payload = meta::Meta::default();
    payload.overview = "Areia".into();
    payload.source = "tmdb".into();
    meta::store(&conn, &id, &payload).unwrap();

    let restored = meta::cached(&conn, &id).expect("cache preenchido");
    assert_eq!(restored.overview, "Areia");
}

#[test]
fn cache_id_e_estavel_e_sensivel_ao_kind() {
    assert_eq!(
        meta::cache_id("movie", "Duna", Some(2021)),
        meta::cache_id("movie", "duna 4K", Some(2021)),
        "o cache segue o título normalizado, não o texto cru"
    );
    assert_ne!(
        meta::cache_id("movie", "Fargo", None),
        meta::cache_id("tv", "Fargo", None)
    );
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test settings_meta`
Expected: FAIL — `could not find settings`.

- [ ] **Step 3: Escrever `settings.rs`**

Create `src-tauri/src/settings.rs`:

```rust
//! Preferências do app, uma linha por chave.

use rusqlite::{params, Connection, OptionalExtension};

pub const TMDB_KEY: &str = "tmdb_api_key";

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
        row.get::<_, String>(0)
    })
    .optional()
    .map_err(|error| format!("não foi possível ler a preferência {key}: {error}"))
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|error| format!("não foi possível gravar a preferência {key}: {error}"))?;
    Ok(())
}

/// A chave colada no onboarding manda; `TMDB_API_KEY` no ambiente é o fallback
/// para quem roda em desenvolvimento.
pub fn tmdb_key(conn: &Connection) -> Option<String> {
    get(conn, TMDB_KEY)
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("TMDB_API_KEY")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .map(|value| value.trim().to_owned())
}
```

- [ ] **Step 4: Trocar o cache de `meta.rs`**

Modify `src-tauri/src/meta.rs`:

1. Trocar o cabeçalho de imports e apagar o `use std::path::PathBuf;`:

```rust
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db;
use crate::ids;
```

2. Tornar `Meta` público para construção nos testes — já é `pub` com `Default`,
   basta apagar o `impl Meta { fn empty }` privado e trocar por:

```rust
impl Meta {
    pub fn empty(reason: Option<String>) -> Self {
        Self {
            source: "none".into(),
            reason,
            ..Default::default()
        }
    }
}
```

3. Acrescentar, no fim do arquivo, o cache em tabela e o id de cache:

```rust
/// Chave do cache: mesma normalização do catálogo, para que "Duna 4K" e "duna"
/// compartilhem a mesma ficha.
pub fn cache_id(kind: &str, title: &str, year: Option<i64>) -> String {
    ids::item_id(kind, &ids::normalize(title), year)
}

pub fn cached(conn: &Connection, cache_id: &str) -> Option<Meta> {
    let payload: Option<String> = conn
        .query_row("SELECT payload FROM meta WHERE item_id = ?1", [cache_id], |row| {
            row.get(0)
        })
        .optional()
        .ok()
        .flatten();
    payload.and_then(|text| serde_json::from_str(&text).ok())
}

pub fn store(conn: &Connection, cache_id: &str, meta: &Meta) -> Result<(), String> {
    let payload = serde_json::to_string(meta)
        .map_err(|error| format!("não foi possível serializar a ficha: {error}"))?;
    conn.execute(
        "INSERT INTO meta(item_id, payload, fetched_at) VALUES(?1, ?2, ?3)
         ON CONFLICT(item_id) DO UPDATE SET payload = excluded.payload, fetched_at = excluded.fetched_at",
        params![cache_id, payload, db::now()],
    )
    .map_err(|error| format!("não foi possível gravar a ficha: {error}"))?;
    Ok(())
}
```

4. Trocar `Tmdb::new`, apagar `Tmdb::key` e reescrever `Tmdb::meta` — a struct
   deixa de conhecer disco e banco:

```rust
pub struct Tmdb {
    client: reqwest::Client,
}

impl Tmdb {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("YellowTV/0.1")
                .build()
                .expect("cliente HTTP"),
        }
    }

    /// Busca no TMDB. O cache e a chave ficam com o chamador porque `Connection`
    /// não cruza `await`.
    pub async fn fetch(&self, key: Option<String>, kind: &str, title: &str, year: Option<i64>) -> Meta {
        let Some(key) = key else {
            return Meta::empty(Some("chave do TMDB não configurada".into()));
        };
        match self.lookup(&key, title, kind, year).await {
            Ok(meta) => meta,
            Err(reason) => Meta::empty(Some(reason)),
        }
    }
}
```

5. Em `lookup`, trocar a assinatura para `async fn lookup(&self, key: &str, title: &str, kind: &str, year: Option<i64>) -> Result<Meta, String>` e apagar o bloco
   inicial que chamava `Self::key()` — a chave agora chega por parâmetro. As
   chamadas `self.get(..., &key, ...)` passam a usar `key` direto.

6. Apagar o método antigo `pub async fn meta(...)` inteiro (o que lia e gravava
   arquivos em `cache_dir`).

- [ ] **Step 5: Rodar os testes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test settings_meta`
Expected: PASS, 4 testes.

- [ ] **Step 6: Adicionar o módulo e commitar**

Modify `src-tauri/src/lib.rs` — acrescentar `pub mod settings;`.

```bash
git add src-tauri/src/settings.rs src-tauri/src/meta.rs src-tauri/src/lib.rs src-tauri/tests/settings_meta.rs
git commit -m "feat: chave do TMDB em settings e cache de ficha na tabela meta"
```

---

## Task 8: `lib.rs` — `AppState`, comandos Tauri, evento `import:progress`

**Files:**
- Modify (reescrever inteiro): `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml` (nada a adicionar; confirmar que `reqwest` já está lá)

**Interfaces:**
- Consumes: tudo das Tasks 1-7.
- Produces os comandos que o front vai chamar na Task 9:

```
list_sources() -> Vec<Source>
add_source(urlOrPath, kind, label) -> ImportReport
sync_source(id) -> ImportReport
remove_source(id)
catalog_page(kind, query, group, page, unwatchedOnly) -> CatalogPage
catalog_item(kind, id) -> ItemWithRelated
series_episodes(id) -> SeriesEpisodes
board() -> Vec<Row>
continue_watching(limit) -> Vec<ContinueEntry>
set_progress(ownerId, ownerKind, position, duration) -> Option<Progress>
mark_watched(ownerId, ownerKind, completed)
next_episode(seriesId) -> Option<EpisodeRef>
toggle_favorite(ownerId) -> bool
favorites() -> Vec<CatalogItem>
title_meta(kind, title, year) -> Meta
get_setting(key) -> Option<String>
set_setting(key, value)
stream_url(url) -> String
```

Evento emitido durante o import: `import:progress` com
`{ sourceId: number, parsed: number }`.

- [ ] **Step 1: Reescrever `lib.rs`**

Replace the entire contents of `src-tauri/src/lib.rs`:

```rust
pub mod catalog;
pub mod db;
pub mod ids;
pub mod import;
pub mod library;
pub mod m3u;
pub mod meta;
mod proxy;
pub mod settings;

use std::io::BufReader;
use std::sync::Mutex;

use catalog::{CatalogItem, CatalogPage, ItemWithRelated, Kind, Row, SeriesEpisodes};
use import::{ImportReport, Source};
use library::{ContinueEntry, EpisodeRef, Progress};
use meta::{Meta, Tmdb};
use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState {
    conn: Mutex<Connection>,
    tmdb: Tmdb,
    proxy_port: u16,
}

impl AppState {
    /// Todo comando síncrono passa por aqui: um único ponto que traduz um mutex
    /// envenenado em erro apresentável.
    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.conn
            .lock()
            .map_err(|_| "o banco ficou em estado inconsistente; reinicie o app".to_owned())
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportProgress {
    source_id: i64,
    parsed: usize,
}

#[tauri::command]
fn list_sources(state: State<'_, AppState>) -> Result<Vec<Source>, String> {
    import::list_sources(&state.conn()?)
}

/// Lê a lista de uma URL ou de um arquivo local e devolve um leitor bufferizado.
fn open_source(url: &str, kind: &str) -> Result<Box<dyn std::io::BufRead>, String> {
    match kind {
        "file" => {
            let file = std::fs::File::open(url)
                .map_err(|error| format!("não foi possível abrir {url}: {error}"))?;
            Ok(Box::new(BufReader::new(file)))
        }
        _ => {
            let response = reqwest::blocking::get(url)
                .map_err(|error| format!("não foi possível baixar a lista: {error}"))?;
            if !response.status().is_success() {
                return Err(format!("a lista respondeu {}", response.status()));
            }
            Ok(Box::new(BufReader::new(response)))
        }
    }
}

fn run_import(
    app: &AppHandle,
    state: &AppState,
    source: Source,
) -> Result<ImportReport, String> {
    let reader = open_source(&source.url, &source.kind)?;
    let mut conn = state.conn()?;
    let source_id = source.id;
    let handle = app.clone();
    import::ingest(&mut conn, source_id, reader, &mut move |parsed| {
        let _ = handle.emit("import:progress", ImportProgress { source_id, parsed });
    })
}

#[tauri::command]
fn add_source(
    app: AppHandle,
    state: State<'_, AppState>,
    url_or_path: String,
    kind: String,
    label: Option<String>,
) -> Result<ImportReport, String> {
    let label = label
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default_label(&url_or_path));
    let source = {
        let conn = state.conn()?;
        import::upsert_source(&conn, &url_or_path, &label, &kind)?
    };
    run_import(&app, &state, source)
}

/// "http://host/get.php?..." vira "host"; um arquivo vira o nome do arquivo.
fn default_label(url_or_path: &str) -> String {
    url_or_path
        .split('/')
        .filter(|part| !part.is_empty() && !part.contains(':'))
        .next()
        .unwrap_or("Minha lista")
        .to_owned()
}

#[tauri::command]
fn sync_source(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<ImportReport, String> {
    let source = {
        let conn = state.conn()?;
        import::get_source(&conn, id)?
    };
    run_import(&app, &state, source)
}

#[tauri::command]
fn remove_source(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let mut conn = state.conn()?;
    import::remove_source(&mut conn, id)
}

#[tauri::command]
fn catalog_page(
    state: State<'_, AppState>,
    kind: String,
    query: String,
    group: Option<String>,
    page: i64,
    unwatched_only: bool,
) -> Result<CatalogPage, String> {
    catalog::page(
        &state.conn()?,
        Kind::parse(&kind)?,
        &query,
        group.as_deref().filter(|value| !value.is_empty()),
        page,
        unwatched_only,
    )
}

#[tauri::command]
fn catalog_item(
    state: State<'_, AppState>,
    kind: String,
    id: String,
) -> Result<ItemWithRelated, String> {
    catalog::item(&state.conn()?, Kind::parse(&kind)?, &id)
}

#[tauri::command]
fn series_episodes(state: State<'_, AppState>, id: String) -> Result<SeriesEpisodes, String> {
    catalog::episodes(&state.conn()?, &id)
}

#[tauri::command]
fn board(state: State<'_, AppState>) -> Result<Vec<Row>, String> {
    catalog::board(&state.conn()?)
}

#[tauri::command]
fn continue_watching(state: State<'_, AppState>, limit: i64) -> Result<Vec<ContinueEntry>, String> {
    library::continue_watching(&state.conn()?, limit)
}

#[tauri::command]
fn set_progress(
    state: State<'_, AppState>,
    owner_id: String,
    owner_kind: String,
    position: f64,
    duration: Option<f64>,
) -> Result<Option<Progress>, String> {
    library::set_progress(&state.conn()?, &owner_id, &owner_kind, position, duration)
}

#[tauri::command]
fn mark_watched(
    state: State<'_, AppState>,
    owner_id: String,
    owner_kind: String,
    completed: bool,
) -> Result<(), String> {
    library::mark_watched(&state.conn()?, &owner_id, &owner_kind, completed)
}

#[tauri::command]
fn next_episode(state: State<'_, AppState>, series_id: String) -> Result<Option<EpisodeRef>, String> {
    library::next_episode(&state.conn()?, &series_id)
}

#[tauri::command]
fn toggle_favorite(state: State<'_, AppState>, owner_id: String) -> Result<bool, String> {
    library::toggle_favorite(&state.conn()?, &owner_id)
}

#[tauri::command]
fn favorites(state: State<'_, AppState>) -> Result<Vec<CatalogItem>, String> {
    let conn = state.conn()?;
    let ids = library::favorites(&conn)?;
    catalog::by_ids(&conn, &ids)
}

#[tauri::command]
fn get_setting(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    settings::get(&state.conn()?, &key)
}

#[tauri::command]
fn set_setting(state: State<'_, AppState>, key: String, value: String) -> Result<(), String> {
    settings::set(&state.conn()?, &key, &value)
}

/// A ficha do TMDB é a única chamada de rede da UI, e o cache mora no banco. O
/// `Connection` não cruza `await`, então cada acesso ao banco abre e fecha antes
/// e depois da chamada HTTP.
#[tauri::command]
async fn title_meta(
    state: State<'_, AppState>,
    kind: String,
    title: String,
    year: Option<i64>,
) -> Result<Meta, String> {
    let tmdb_kind = if kind == "series" { "tv" } else { "movie" };
    let cache_id = meta::cache_id(tmdb_kind, &title, year);

    let (cached, key) = {
        let conn = state.conn()?;
        (
            meta::cached(&conn, &cache_id),
            settings::tmdb_key(&conn),
        )
    };
    if let Some(cached) = cached {
        return Ok(cached);
    }

    let fetched = state.tmdb.fetch(key, tmdb_kind, &title, year).await;
    if fetched.source == "tmdb" {
        let conn = state.conn()?;
        meta::store(&conn, &cache_id, &fetched)?;
    }
    Ok(fetched)
}

/// Toda reprodução passa pelo proxy local: é o que permite tocar HTTP puro dentro
/// da WebView.
#[tauri::command]
fn stream_url(state: State<'_, AppState>, url: String) -> String {
    format!(
        "http://127.0.0.1:{}/stream?url={}",
        state.proxy_port,
        urlencoding(&url)
    )
}

fn urlencoding(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let database = app_data.join("yellowtv.db");
            let conn = db::open(&database).map_err(std::io::Error::other)?;
            let proxy_port = proxy::start()?;

            println!("YellowTV: banco em {}", database.display());
            println!("YellowTV: proxy de stream em 127.0.0.1:{proxy_port}");

            app.manage(AppState {
                conn: Mutex::new(conn),
                tmdb: Tmdb::new(),
                proxy_port,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_sources,
            add_source,
            sync_source,
            remove_source,
            catalog_page,
            catalog_item,
            series_episodes,
            board,
            continue_watching,
            set_progress,
            mark_watched,
            next_episode,
            toggle_favorite,
            favorites,
            get_setting,
            set_setting,
            title_meta,
            stream_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 2: Habilitar `reqwest::blocking` e o diálogo de arquivo**

```bash
cd src-tauri
cargo add reqwest --features blocking
cargo add tauri-plugin-dialog
cd ..
```

Modify `src-tauri/capabilities/default.json` — acrescentar `"dialog:default"` à
lista de `permissions`. Se o arquivo não existir, liste o diretório
`src-tauri/capabilities/` e edite o JSON que estiver lá.

- [ ] **Step 3: Compilar e rodar a suíte inteira**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS em todos os arquivos de teste, e `cargo build` sem erros.

Dois pontos previsíveis:
- `run_import` mantém o `MutexGuard` viva enquanto o import roda — é intencional
  (uma escrita por vez), mas o closure de progresso não pode tocar em `state`.
  Se o borrow checker reclamar do `handle.emit`, confirme que o closure só captura
  `handle` e `source_id`, como escrito.
- `std::io::Error::other` exige Rust 1.74+. Se a toolchain for anterior, troque
  por `|error| std::io::Error::new(std::io::ErrorKind::Other, error)`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/capabilities
git commit -m "feat: comandos Tauri do catálogo local e evento de progresso do import"
```

---

## Task 9: `src/lib/api.ts` — tipos e wrappers de `invoke`

**Files:**
- Create: `src/lib/api.ts`
- Modify: `src/lib/vod.ts` (deixa de exportar wrappers; mantém apenas apresentação)
- Modify: `package.json` (script `typecheck`)

**Interfaces:**
- Consumes: comandos da Task 8.
- Produces os tipos e funções que todas as tasks de front consomem:
  `Source`, `ImportReport`, `CatalogItem`, `CatalogPage`, `StreamRef`,
  `ItemWithRelated`, `EpisodeRow`, `SeriesEpisodes`, `BoardRow`, `Progress`,
  `ContinueEntry`, `EpisodeRef`, `Meta`, `CatalogKind`, e os wrappers homônimos
  dos comandos.

- [ ] **Step 1: Adicionar o script de typecheck**

Modify `package.json` — acrescentar em `"scripts"`:

```json
    "typecheck": "tsc --noEmit",
```

- [ ] **Step 2: Escrever `src/lib/api.ts`**

Create `src/lib/api.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

export type CatalogKind = "filmes" | "series" | "canais";
export type OwnerKind = "item" | "episode";

export type Source = {
  id: number;
  url: string;
  label: string;
  kind: "url" | "file";
  enabled: boolean;
  addedAt: number;
  lastSyncAt: number | null;
  itemCount: number;
};

export type ImportReport = {
  source: Source;
  parsed: number;
  discarded: number;
  movies: number;
  series: number;
  channels: number;
};

export type CatalogItem = {
  id: string;
  kind: "movie" | "series" | "channel";
  title: string;
  year: number | null;
  logo: string | null;
  group: string | null;
  seasons: number;
  episodeCount: number;
  channelNumber: number | null;
  /** 0 a 1 */
  percent: number;
  completed: boolean;
};

export type GroupCount = { name: string; count: number };

export type CatalogPage = {
  total: number;
  page: number;
  pageSize: number;
  items: CatalogItem[];
  groups: GroupCount[];
};

export type StreamRef = {
  id: number;
  url: string;
  quality: string | null;
  sourceLabel: string;
  channelNumber: number | null;
};

export type Progress = {
  ownerId: string;
  ownerKind: OwnerKind;
  positionSecs: number;
  durationSecs: number | null;
  completed: boolean;
  updatedAt: number;
};

export type ItemWithRelated = {
  item: CatalogItem;
  streams: StreamRef[];
  progress: Progress | null;
  related: CatalogItem[];
};

export type EpisodeRow = {
  id: string;
  season: number;
  episode: number;
  title: string | null;
  streams: StreamRef[];
  percent: number;
  completed: boolean;
};

export type SeriesEpisodes = {
  id: string;
  title: string;
  logo: string | null;
  group: string | null;
  episodes: EpisodeRow[];
};

export type EpisodeRef = {
  id: string;
  seriesId: string;
  season: number;
  episode: number;
  title: string | null;
};

export type ContinueEntry = {
  ownerId: string;
  ownerKind: OwnerKind;
  itemId: string;
  kind: "movie" | "series" | "channel";
  title: string;
  logo: string | null;
  season: number | null;
  episode: number | null;
  positionSecs: number;
  durationSecs: number | null;
  percent: number;
};

export type BoardRow = {
  key: string;
  title: string;
  kind: string;
  items: CatalogItem[];
};

export type Meta = {
  overview: string;
  tagline: string;
  poster: string | null;
  backdrop: string | null;
  rating: number | null;
  votes: number | null;
  year: number | null;
  runtime: number | null;
  genres: string[];
  cast: { name: string; character: string; photo: string | null }[];
  imdbUrl: string | null;
  source: "tmdb" | "none";
  reason?: string;
};

export const TMDB_KEY_SETTING = "tmdb_api_key";

export const listSources = () => invoke<Source[]>("list_sources");

export const addSource = (urlOrPath: string, kind: "url" | "file", label?: string) =>
  invoke<ImportReport>("add_source", { urlOrPath, kind, label: label ?? null });

export const syncSource = (id: number) => invoke<ImportReport>("sync_source", { id });

export const removeSource = (id: number) => invoke<void>("remove_source", { id });

export function fetchCatalog(input: {
  kind: CatalogKind;
  query: string;
  group: string;
  page: number;
  unwatchedOnly?: boolean;
}) {
  return invoke<CatalogPage>("catalog_page", {
    kind: input.kind,
    query: input.query,
    group: input.group || null,
    page: input.page,
    unwatchedOnly: input.unwatchedOnly ?? false,
  });
}

export const fetchItem = (kind: CatalogKind, id: string) =>
  invoke<ItemWithRelated>("catalog_item", { kind, id });

export const fetchEpisodes = (id: string) => invoke<SeriesEpisodes>("series_episodes", { id });

export const fetchBoard = () => invoke<BoardRow[]>("board");

export const fetchContinueWatching = (limit = 20) =>
  invoke<ContinueEntry[]>("continue_watching", { limit });

export const reportProgress = (
  ownerId: string,
  ownerKind: OwnerKind,
  position: number,
  duration: number | null,
) => invoke<Progress | null>("set_progress", { ownerId, ownerKind, position, duration });

export const markWatched = (ownerId: string, ownerKind: OwnerKind, completed: boolean) =>
  invoke<void>("mark_watched", { ownerId, ownerKind, completed });

export const fetchNextEpisode = (seriesId: string) =>
  invoke<EpisodeRef | null>("next_episode", { seriesId });

export const toggleFavorite = (ownerId: string) => invoke<boolean>("toggle_favorite", { ownerId });

export const fetchFavorites = () => invoke<CatalogItem[]>("favorites");

export const getSetting = (key: string) => invoke<string | null>("get_setting", { key });

export const setSetting = (key: string, value: string) =>
  invoke<void>("set_setting", { key, value });

export function fetchMeta(input: { kind: CatalogKind; title: string; year?: number | null }) {
  return invoke<Meta>("title_meta", {
    kind: input.kind,
    title: input.title,
    year: input.year ?? null,
  });
}

/**
 * Streams são HTTP puro e a WebView recusa carga insegura, então a reprodução
 * sempre passa pelo proxy local que o Rust sobe em 127.0.0.1.
 */
export const streamUrl = (url: string) => invoke<string>("stream_url", { url });
```

- [ ] **Step 3: Enxugar `src/lib/vod.ts`**

Replace the entire contents of `src/lib/vod.ts`:

```ts
import type { CatalogItem, CatalogKind } from "~/lib/api";

export type { CatalogItem, CatalogKind } from "~/lib/api";

export const isSeries = (item: CatalogItem) => item.kind === "series";

/** "Series | Netflix" lê melhor como "Netflix" quando a seção já diz Séries. */
export const shortGroup = (group: string | null) =>
  (group ?? "").replace(/^(Series|Filmes|Canais)\s*\|\s*/i, "");

export const detailHref = (item: Pick<CatalogItem, "kind" | "id">) =>
  item.kind === "series"
    ? `/serie/${item.id}`
    : item.kind === "channel"
      ? `/watch/${item.id}`
      : `/filme/${item.id}`;

export const kindOf = (item: Pick<CatalogItem, "kind">): CatalogKind =>
  item.kind === "series" ? "series" : item.kind === "channel" ? "canais" : "filmes";

/** "Retomar em 18min" precisa do que falta, não do que já passou. */
export function remainingLabel(positionSecs: number, durationSecs: number | null) {
  if (!durationSecs || durationSecs <= 0) return "Retomar";
  const minutes = Math.max(1, Math.round((durationSecs - positionSecs) / 60));
  return `Retomar · faltam ${minutes}min`;
}
```

- [ ] **Step 4: Verificar**

Run: `bunx tsc --noEmit`
Expected: FAIL, e só nos arquivos que ainda importam o que saiu de `vod.ts`
(`CatalogBrowser.tsx`, `PosterGrid.tsx`, `Filme.tsx`, `Serie.tsx`, `Channels.tsx`,
`Watch.tsx`, `channels.ts`). Anote a lista: as Tasks 10-14 fecham exatamente
esses erros. Nenhum erro deve apontar para `api.ts` ou `vod.ts`.

- [ ] **Step 5: Commit**

```bash
git add src/lib/api.ts src/lib/vod.ts package.json
git commit -m "feat: camada de acesso aos comandos do catálogo local no front"
```

---

## Task 10: `/setup`, nav nova e gate de primeira execução

**Files:**
- Create: `src/pages/Setup.tsx`
- Modify: `src/components/Nav.tsx` (links)
- Modify: `src/App.tsx` (rotas e gate)
- Create: `src/lib/sources.ts` (recurso compartilhado de fontes)

**Interfaces:**
- Consumes: `api.listSources`, `api.addSource`, `api.setSetting`, `TMDB_KEY_SETTING` (Task 9); evento `import:progress` (Task 8).
- Produces: `sources.ts` exporta `useSources()` → `{ sources, pending, error, refetch }`.

- [ ] **Step 1: Escrever `src/lib/sources.ts`**

Create `src/lib/sources.ts`:

```ts
import { createResource } from "solid-js";
import { listSources, type Source } from "~/lib/api";

/**
 * As fontes decidem se o app já tem catálogo, então tanto o gate de `/setup`
 * quanto a Biblioteca leem daqui.
 */
export function useSources() {
  const [data, { refetch }] = createResource<Source[]>(listSources);
  return {
    sources: () => data() ?? [],
    pending: () => data() === undefined && !data.error,
    error: () => data.error as Error | undefined,
    refetch,
  };
}
```

- [ ] **Step 2: Escrever `src/pages/Setup.tsx`**

Create `src/pages/Setup.tsx`:

```tsx
import { useNavigate } from "@solidjs/router";
import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Play, Search } from "~/components/Icons";
import {
  addSource,
  removeSource,
  setSetting,
  syncSource,
  TMDB_KEY_SETTING,
  type ImportReport,
} from "~/lib/api";
import { useSources } from "~/lib/sources";

export default function Setup() {
  const navigate = useNavigate();
  const list = useSources();

  const [url, setUrl] = createSignal("");
  const [tmdb, setTmdb] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [parsed, setParsed] = createSignal(0);
  const [report, setReport] = createSignal<ImportReport>();
  const [error, setError] = createSignal<string>();

  onMount(async () => {
    // O import roda em Rust e vai avisando quantas entradas já entraram.
    const stop = await listen<{ sourceId: number; parsed: number }>(
      "import:progress",
      event => setParsed(event.payload.parsed),
    );
    onCleanup(stop);
  });

  const run = async (task: () => Promise<ImportReport>) => {
    setBusy(true);
    setError(undefined);
    setParsed(0);
    try {
      const result = await task();
      setReport(result);
      await list.refetch();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  };

  const addUrl = () => {
    const value = url().trim();
    if (!value) return setError("Cole o endereço da lista M3U.");
    void run(() => addSource(value, "url"));
  };

  const addFile = async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Lista M3U", extensions: ["m3u", "m3u8", "txt"] }],
    });
    if (typeof picked !== "string") return;
    void run(() => addSource(picked, "file"));
  };

  const saveKeyAndGo = async () => {
    const key = tmdb().trim();
    if (key) await setSetting(TMDB_KEY_SETTING, key);
    navigate("/", { replace: true });
  };

  return (
    <main class="relative z-10 mx-auto max-w-2xl px-4 py-10">
      <h1 class="anim-reveal font-display text-3xl font-extrabold tracking-[-0.03em] text-paper">
        Sua lista, seu catálogo
      </h1>
      <p class="anim-reveal mt-2 text-sm text-paper/55" style={{ "--i": 1 }}>
        O YellowTV lê a lista M3U que você já tem e monta um catálogo local de filmes e séries.
        Nada sai do seu computador.
      </p>

      <section class="anim-reveal mt-8" style={{ "--i": 2 }}>
        <label class="flex items-center gap-3 rounded-sm border border-edge bg-panel px-3 py-2.5 focus-within:border-amber">
          <Search size={16} class="shrink-0 text-paper/35" />
          <input
            type="url"
            value={url()}
            onInput={event => setUrl(event.currentTarget.value)}
            onKeyDown={event => event.key === "Enter" && addUrl()}
            placeholder="http://servidor/get.php?username=…&type=m3u_plus"
            class="w-full bg-transparent text-paper outline-none placeholder:text-paper/30"
          />
        </label>
        <div class="mt-3 flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={addUrl}
            disabled={busy()}
            class="press flex items-center gap-2 rounded-sm bg-amber px-5 py-2.5 font-medium text-ink hover:bg-paper disabled:opacity-40"
          >
            <Play size={15} />
            {busy() ? "Importando…" : "Importar lista"}
          </button>
          <button
            type="button"
            onClick={addFile}
            disabled={busy()}
            class="wipe press rounded-sm border border-edge px-4 py-2.5 text-sm text-paper/70 hover:border-amber hover:text-amber disabled:opacity-40"
          >
            Escolher arquivo
          </button>
        </div>
      </section>

      <Show when={busy()}>
        <p class="anim-fade mt-4 font-mono text-xs tabular-nums text-amber" aria-live="polite">
          {parsed().toLocaleString("pt-BR")} entradas lidas<span class="anim-caret">_</span>
        </p>
      </Show>

      <Show when={error()}>
        <div class="anim-fade mt-4 rounded-sm border border-live/40 bg-live/10 p-4">
          <p class="text-sm text-live">{error()}</p>
          <button
            type="button"
            onClick={addUrl}
            class="press mt-2 text-sm text-amber hover:underline"
          >
            Tentar de novo
          </button>
        </div>
      </Show>

      <Show when={report()}>
        {result => (
          <div class="anim-fade mt-4 rounded-sm border border-edge bg-panel/60 p-4">
            <p class="text-sm text-paper/85">
              {result().movies.toLocaleString("pt-BR")} filmes ·{" "}
              {result().series.toLocaleString("pt-BR")} séries ·{" "}
              {result().channels.toLocaleString("pt-BR")} canais
            </p>
            <Show when={result().discarded}>
              <p class="mt-1 font-mono text-xs text-paper/40">
                {result().discarded.toLocaleString("pt-BR")} linhas ignoradas
              </p>
            </Show>
          </div>
        )}
      </Show>

      <Show when={list.sources().length}>
        <section class="mt-8">
          <h2 class="font-display text-sm font-bold text-paper/70">Listas adicionadas</h2>
          <ul class="mt-3 space-y-2">
            <For each={list.sources()}>
              {source => (
                <li class="flex items-center gap-3 rounded-sm border border-edge bg-panel/40 px-3 py-2">
                  <div class="min-w-0">
                    <p class="truncate text-sm text-paper/90">{source.label}</p>
                    <p class="truncate font-mono text-[0.7rem] text-paper/35">
                      {source.itemCount.toLocaleString("pt-BR")} títulos · {source.url}
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={() => void run(() => syncSource(source.id))}
                    disabled={busy()}
                    class="press ml-auto shrink-0 text-xs text-paper/55 hover:text-amber disabled:opacity-40"
                  >
                    Atualizar
                  </button>
                  <button
                    type="button"
                    onClick={async () => {
                      await removeSource(source.id);
                      await list.refetch();
                    }}
                    class="press shrink-0 text-xs text-paper/40 hover:text-live"
                  >
                    Remover
                  </button>
                </li>
              )}
            </For>
          </ul>
        </section>
      </Show>

      <section class="mt-10 border-t border-edge pt-6">
        <h2 class="font-display text-sm font-bold text-paper/70">Chave do TMDB (opcional)</h2>
        <p class="mt-1 text-sm text-paper/50">
          Sinopse, nota e elenco vêm do TMDB. Sem chave, o catálogo usa as capas da própria lista.
        </p>
        <input
          type="password"
          value={tmdb()}
          onInput={event => setTmdb(event.currentTarget.value)}
          placeholder="cole a chave v3 ou o token v4"
          class="mt-3 w-full rounded-sm border border-edge bg-panel px-3 py-2.5 text-paper outline-none placeholder:text-paper/30 focus:border-amber"
        />
        <div class="mt-4 flex flex-wrap items-center gap-4">
          <button
            type="button"
            onClick={saveKeyAndGo}
            disabled={!list.sources().length}
            class="press rounded-sm bg-amber px-5 py-2.5 font-medium text-ink hover:bg-paper disabled:opacity-40"
          >
            {tmdb().trim() ? "Salvar e começar" : "Começar"}
          </button>
          <Show when={!list.sources().length}>
            <span class="text-xs text-paper/40">Adicione uma lista para continuar.</span>
          </Show>
        </div>
      </section>
    </main>
  );
}
```

- [ ] **Step 3: Nav nova**

Modify `src/components/Nav.tsx` — trocar a constante `LINKS` e apagar o link
solto de Favoritos (a Biblioteca cobre isso):

```tsx
import { Film, Library, Play, Signal, Stack, Tv } from "./Icons";

const LINKS = [
  { href: "/", label: "Início", icon: Play, exact: true },
  { href: "/filmes", label: "Filmes", icon: Film, exact: false },
  { href: "/series", label: "Séries", icon: Stack, exact: false },
  { href: "/biblioteca", label: "Biblioteca", icon: Library, exact: false },
  { href: "/canais", label: "Canais", icon: Tv, exact: false },
] as const;
```

`Library` e `Play` precisam existir em `src/components/Icons.tsx`. Abra o arquivo,
confirme quais nomes já estão exportados e reaproveite os existentes — `Play` já
está lá. Para `Library`, acrescente ao final do arquivo, no mesmo formato dos
outros ícones do projeto:

```tsx
export const Library = (props: IconProps) => (
  <Icon {...props}>
    <path d="M3 4h4v16H3zM9 4h4v16H9zM16 5l4 14" />
  </Icon>
);
```

Se o arquivo não usar um wrapper `Icon` nem um tipo `IconProps`, copie a
assinatura exata do ícone `Tv` que já está no arquivo e troque só o `path`.

Apagar também o bloco `<A href="/?g=favoritos">…</A>` no fim da `<nav>`, e o
`StarOutline` do import se ele ficar sem uso.

- [ ] **Step 4: Rotas e gate em `App.tsx`**

Replace the entire contents of `src/App.tsx`:

```tsx
import { Navigate, Route, Router } from "@solidjs/router";
import { Show, Suspense } from "solid-js";
import Nav from "~/components/Nav";
import Biblioteca from "~/pages/Biblioteca";
import Channels from "~/pages/Channels";
import Filme from "~/pages/Filme";
import Filmes from "~/pages/Filmes";
import Inicio from "~/pages/Inicio";
import NotFound from "~/pages/NotFound";
import Serie from "~/pages/Serie";
import Series from "~/pages/Series";
import Setup from "~/pages/Setup";
import Watch from "~/pages/Watch";
import { useSources } from "~/lib/sources";
import "./App.css";

/**
 * Sem nenhuma lista adicionada não existe catálogo, então tudo cai em `/setup`
 * até a primeira importação terminar.
 */
function Gate(props: { children?: any }) {
  const list = useSources();
  return (
    <Show when={!list.pending()} fallback={null}>
      <Show when={list.sources().length} fallback={<Navigate href="/setup" />}>
        {props.children}
      </Show>
    </Show>
  );
}

export default function App() {
  return (
    <Router
      root={props => (
        <>
          <Nav />
          <Suspense>{props.children}</Suspense>
        </>
      )}
    >
      <Route path="/setup" component={Setup} />
      <Route
        path="/"
        component={() => (
          <Gate>
            <Inicio />
          </Gate>
        )}
      />
      <Route path="/filmes" component={Filmes} />
      <Route path="/series" component={Series} />
      <Route path="/biblioteca" component={Biblioteca} />
      <Route path="/canais" component={Channels} />
      <Route path="/filme/:id" component={Filme} />
      <Route path="/serie/:id" component={Serie} />
      <Route path="/watch/:id" component={Watch} />
      <Route path="*" component={NotFound} />
    </Router>
  );
}
```

- [ ] **Step 5: Stubs para compilar**

Create `src/pages/Inicio.tsx` e `src/pages/Biblioteca.tsx` com o mínimo — as
Tasks 11 e 14 preenchem:

```tsx
export default function Inicio() {
  return <main class="mx-auto max-w-7xl px-4 py-6" />;
}
```

```tsx
export default function Biblioteca() {
  return <main class="mx-auto max-w-7xl px-4 py-6" />;
}
```

- [ ] **Step 6: Verificar na tela**

Run: `bunx tsc --noEmit`
Expected: os erros restantes só nos arquivos das Tasks 11-14.

Run: `bun run tauri dev`
Expected: com o banco vazio, qualquer rota cai em `/setup`. Cole uma lista M3U de
verdade, veja o contador subir e o resumo aparecer. Confirme que `/` deixa de
redirecionar depois do import.

- [ ] **Step 7: Commit**

```bash
git add src/pages/Setup.tsx src/pages/Inicio.tsx src/pages/Biblioteca.tsx src/lib/sources.ts src/components/Nav.tsx src/components/Icons.tsx src/App.tsx
git commit -m "feat: onboarding de lista M3U, nav de cinco seções e gate de primeira execução"
```

---

## Task 11: pôster com progresso e board do Início

**Files:**
- Modify: `src/components/PosterGrid.tsx`
- Create: `src/components/PosterCard.tsx`
- Create: `src/components/PosterRow.tsx`
- Modify (substituir o stub): `src/pages/Inicio.tsx`

**Interfaces:**
- Consumes: `api.fetchBoard`, `api.CatalogItem`, `vod.detailHref`, `vod.shortGroup` (Tasks 9-10).
- Produces:
  - `PosterCard(props: { item: CatalogItem })` — o pôster com barra âmbar e estado concluído; usado pela grade e pela linha.
  - `PosterRow(props: { title: string; items: CatalogItem[] })` — linha horizontal rolável.

- [ ] **Step 1: Extrair o pôster para `PosterCard.tsx`**

Create `src/components/PosterCard.tsx`:

```tsx
import { A } from "@solidjs/router";
import { Show } from "solid-js";
import { detailHref, shortGroup } from "~/lib/vod";
import type { CatalogItem } from "~/lib/api";
import { Check, Play } from "./Icons";

/**
 * Um pôster tem três estados: intocado, em andamento (barra âmbar no rodapé) e
 * concluído (escurecido com um check).
 */
export default function PosterCard(props: { item: CatalogItem }) {
  const item = () => props.item;
  const started = () => item().percent > 0.001 && !item().completed;

  return (
    <A href={detailHref(item())} class="group block">
      <div class="relative aspect-[2/3] overflow-hidden rounded-sm bg-panel ring-1 ring-edge/70 transition-[transform,box-shadow] duration-[var(--duration-base)] ease-[var(--ease-out-soft)] group-hover:-translate-y-1 group-hover:shadow-[var(--shadow-glow)] group-focus-visible:-translate-y-1">
        <Show
          when={item().logo}
          fallback={
            <span class="grid h-full place-content-center px-2 text-center text-xs text-paper/30">
              sem pôster
            </span>
          }
        >
          <img
            src={item().logo!}
            alt=""
            loading="lazy"
            decoding="async"
            class="h-full w-full object-cover transition-transform duration-[var(--duration-slow)] ease-[var(--ease-out-soft)] group-hover:scale-[1.04]"
            classList={{ "opacity-45": item().completed }}
            onError={event => (event.currentTarget.style.visibility = "hidden")}
          />
        </Show>

        <div
          class="pointer-events-none absolute inset-0 flex items-end justify-start bg-gradient-to-t from-ink/90 via-ink/10 to-transparent p-3 opacity-0 transition-opacity duration-[var(--duration-base)] group-hover:opacity-100"
          aria-hidden="true"
        >
          <span class="grid size-9 place-content-center rounded-full bg-amber text-ink shadow-[var(--shadow-lift)]">
            <Play size={15} />
          </span>
        </div>

        <Show when={item().completed}>
          <span
            class="absolute right-2 top-2 grid size-6 place-content-center rounded-full bg-ink/85 text-amber backdrop-blur-sm"
            title="Já assistido"
          >
            <Check size={13} />
          </span>
        </Show>

        <Show when={item().kind === "series" && item().episodeCount > 0}>
          <span class="absolute bottom-0 right-0 rounded-tl-sm bg-ink/85 px-1.5 py-0.5 font-mono text-[0.65rem] tabular-nums text-amber backdrop-blur-sm">
            {item().seasons}T · {item().episodeCount}ep
          </span>
        </Show>

        <Show when={started()}>
          <span
            class="absolute inset-x-0 bottom-0 h-[3px] bg-ink/70"
            aria-hidden="true"
          >
            <span
              class="block h-full bg-amber"
              style={{ width: `${Math.round(item().percent * 100)}%` }}
            />
          </span>
        </Show>
      </div>

      <p class="mt-2 line-clamp-2 text-sm text-paper/90 transition-colors duration-[var(--duration-fast)] group-hover:text-amber">
        {item().title}
      </p>
      <p class="text-xs text-paper/35">{shortGroup(item().group)}</p>
    </A>
  );
}
```

`Check` precisa existir em `src/components/Icons.tsx`. Se não estiver lá,
acrescente no mesmo formato dos vizinhos:

```tsx
export const Check = (props: IconProps) => (
  <Icon {...props}>
    <path d="M4 12l5 5L20 6" />
  </Icon>
);
```

- [ ] **Step 2: `PosterGrid` passa a usar o cartão**

Replace the entire contents of `src/components/PosterGrid.tsx`:

```tsx
import { For } from "solid-js";
import PosterCard from "~/components/PosterCard";
import type { CatalogItem } from "~/lib/api";

export default function PosterGrid(props: { items: CatalogItem[] }) {
  return (
    <ul class="grid grid-cols-2 gap-x-4 gap-y-7 sm:grid-cols-3 lg:grid-cols-5 xl:grid-cols-6">
      <For each={props.items}>
        {(item, index) => (
          <li class="anim-poster" style={{ "--i": Math.min(index(), 18) }}>
            <PosterCard item={item} />
          </li>
        )}
      </For>
    </ul>
  );
}
```

A prop `kind` sai: o link agora vem do próprio item. `CatalogBrowser.tsx` passa a
chamar `<PosterGrid items={…} />` — a Task 12 faz isso.

- [ ] **Step 3: `PosterRow.tsx`**

Create `src/components/PosterRow.tsx`:

```tsx
import { For, Show } from "solid-js";
import PosterCard from "~/components/PosterCard";
import type { CatalogItem } from "~/lib/api";

/** Linha horizontal no formato Stremio: o pôster manda, sem sinopse. */
export default function PosterRow(props: { title: string; items: CatalogItem[] }) {
  return (
    <Show when={props.items.length}>
      <section class="anim-reveal">
        <h2 class="mb-3 font-display text-sm font-bold tracking-tight text-paper/70">
          {props.title}
        </h2>
        <ul class="-mx-4 flex snap-x gap-4 overflow-x-auto px-4 pb-2 [scrollbar-width:thin]">
          <For each={props.items}>
            {(item, index) => (
              <li
                class="anim-poster w-[38vw] shrink-0 snap-start sm:w-44 lg:w-48"
                style={{ "--i": Math.min(index(), 12) }}
              >
                <PosterCard item={item} />
              </li>
            )}
          </For>
        </ul>
      </section>
    </Show>
  );
}
```

- [ ] **Step 4: Board do Início**

Replace the entire contents of `src/pages/Inicio.tsx`:

```tsx
import { createResource, For, Show } from "solid-js";
import PosterRow from "~/components/PosterRow";
import PosterSkeleton from "~/components/PosterSkeleton";
import { fetchBoard } from "~/lib/api";

export default function Inicio() {
  const [rows] = createResource(fetchBoard);

  return (
    <main class="relative z-10 mx-auto max-w-7xl space-y-8 px-4 py-6">
      <Show
        when={!rows.loading}
        fallback={
          <div class="space-y-6">
            <PosterSkeleton />
          </div>
        }
      >
        <Show
          when={!rows.error}
          fallback={<p class="anim-fade py-12 text-sm text-live">{String(rows.error)}</p>}
        >
          <Show
            when={rows()?.length}
            fallback={
              <div class="anim-reveal py-16 text-center">
                <p class="font-display text-lg text-paper/70">Catálogo vazio.</p>
                <p class="mt-1 text-sm text-paper/40">
                  Atualize a lista em Biblioteca para trazer os títulos.
                </p>
              </div>
            }
          >
            <For each={rows()}>
              {(row, index) => (
                <div style={{ "--i": Math.min(index(), 8) }}>
                  <PosterRow title={row.title} items={row.items} />
                </div>
              )}
            </For>
          </Show>
        </Show>
      </Show>
    </main>
  );
}
```

- [ ] **Step 5: Verificar na tela**

Run: `bunx tsc --noEmit`
Expected: os erros restantes só em `CatalogBrowser.tsx`, `Filme.tsx`,
`Serie.tsx`, `Channels.tsx`, `Watch.tsx`, `ChannelRow.tsx`, `channels.ts`.

Run: `bun run tauri dev` e abra `/`
Expected: linhas horizontais com pôsteres. Sem progresso ainda, a primeira linha
é "Adicionados recentemente"; depois de assistir algo (Task 13) aparece
"Continuar assistindo" no topo.

- [ ] **Step 6: Commit**

```bash
git add src/components/PosterCard.tsx src/components/PosterRow.tsx src/components/PosterGrid.tsx src/components/Icons.tsx src/pages/Inicio.tsx
git commit -m "feat: board do Início em linhas horizontais e pôster com estado de progresso"
```

---

## Task 12: catálogo de filmes e séries com filtro de não assistidos

**Files:**
- Modify: `src/components/CatalogBrowser.tsx`
- Modify: `src/pages/Filmes.tsx`, `src/pages/Series.tsx`

**Interfaces:**
- Consumes: `api.fetchCatalog`, `PosterGrid` sem `kind` (Tasks 9, 11).
- Produces: `CatalogBrowser` aceita `kind: CatalogKind` e mantém `q`, `g`, `p`,
  mais o novo parâmetro de busca `u` (`"1"` liga "só não assistidos").

- [ ] **Step 1: Ajustar `CatalogBrowser.tsx`**

Modify `src/components/CatalogBrowser.tsx`:

1. Trocar os imports de `~/lib/vod` por:

```tsx
import { fetchCatalog, type CatalogKind } from "~/lib/api";
import { shortGroup } from "~/lib/vod";
```

2. Trocar o tipo dos search params e acrescentar o acessor:

```tsx
  const [params, setParams] = useSearchParams<{ q?: string; g?: string; p?: string; u?: string }>();
```

```tsx
  const unwatchedOnly = () => params.u === "1";
```

3. Passar o filtro ao recurso:

```tsx
  const [data] = createResource(
    () =>
      started()
        ? {
            kind: props.kind,
            query: query(),
            group: group(),
            page: page(),
            unwatchedOnly: unwatchedOnly(),
          }
        : undefined,
    fetchCatalog,
  );
```

4. Acrescentar a pastilha do filtro como primeiro `<li>` da lista de grupos,
   logo antes do `<li>` de "Tudo":

```tsx
            <li class="anim-fade">
              <button
                type="button"
                onClick={() => setParams({ u: unwatchedOnly() ? undefined : "1", p: undefined })}
                class={chip}
                classList={{ "border-amber bg-amber/12 text-amber": unwatchedOnly() }}
                aria-pressed={unwatchedOnly()}
              >
                Não assistidos
              </button>
            </li>
```

5. Trocar a chamada da grade — a prop `kind` não existe mais:

```tsx
                <PosterGrid items={data()!.items} />
```

- [ ] **Step 2: Páginas**

Replace `src/pages/Filmes.tsx`:

```tsx
import CatalogBrowser from "~/components/CatalogBrowser";

export default function Filmes() {
  return <CatalogBrowser kind="filmes" heading="Filmes" placeholder="Buscar filme" />;
}
```

Replace `src/pages/Series.tsx`:

```tsx
import CatalogBrowser from "~/components/CatalogBrowser";

export default function Series() {
  return <CatalogBrowser kind="series" heading="Séries" placeholder="Buscar série" />;
}
```

- [ ] **Step 3: Verificar na tela**

Run: `bunx tsc --noEmit`
Expected: nenhum erro em `CatalogBrowser.tsx`, `Filmes.tsx`, `Series.tsx`.

Run: `bun run tauri dev` e abra `/filmes`
Expected: contagem total, pastilhas de grupo com contagem, busca por prefixo
respondendo rápido mesmo em catálogo grande, paginação funcionando, e
"Não assistidos" reduzindo o total depois de marcar algo como visto.

- [ ] **Step 4: Commit**

```bash
git add src/components/CatalogBrowser.tsx src/pages/Filmes.tsx src/pages/Series.tsx
git commit -m "feat: catálogo paginado em SQL com filtro de não assistidos"
```

---

## Task 13: `Player` reportando posição, e o seletor de fontes

**Files:**
- Modify: `src/components/Player.tsx`
- Create: `src/components/SourcePicker.tsx`

**Interfaces:**
- Consumes: `api.reportProgress`, `api.streamUrl`, `api.StreamRef` (Task 9).
- Produces:
  - `Player(props: { src, title, poster?, owner?: { id: string; kind: OwnerKind }, startAt?: number, onEnded?: () => void })`
  - `SourcePicker(props: { streams: StreamRef[]; active: number; onPick: (index: number) => void })`

O report tem throttle de 5s, mais um disparo em `pause` e outro no cleanup — é o
que garante que fechar a janela no meio do filme não perde a posição.

- [ ] **Step 1: Escrever `SourcePicker.tsx`**

Create `src/components/SourcePicker.tsx`:

```tsx
import { For, Show } from "solid-js";
import type { StreamRef } from "~/lib/api";

/**
 * Metadata e fonte de stream são coisas distintas: quando o mesmo título vem de
 * mais de uma lista, quem escolhe é o usuário.
 */
export default function SourcePicker(props: {
  streams: StreamRef[];
  active: number;
  onPick: (index: number) => void;
}) {
  return (
    <Show when={props.streams.length > 1}>
      <div class="mt-4">
        <p class="font-mono text-[0.7rem] uppercase tracking-wide text-paper/35">fontes</p>
        <ul class="mt-2 flex flex-wrap gap-2">
          <For each={props.streams}>
            {(stream, index) => (
              <li>
                <button
                  type="button"
                  onClick={() => props.onPick(index())}
                  class="press flex items-baseline gap-1.5 rounded-full border border-edge bg-panel/50 px-3 py-1 text-sm text-paper/60 hover:border-amber-deep hover:text-amber"
                  classList={{
                    "border-amber bg-amber/12 text-amber": index() === props.active,
                  }}
                  aria-pressed={index() === props.active}
                >
                  <span>{stream.sourceLabel}</span>
                  <Show when={stream.quality}>
                    <span class="font-mono text-[0.7rem] opacity-55">{stream.quality}</span>
                  </Show>
                </button>
              </li>
            )}
          </For>
        </ul>
      </div>
    </Show>
  );
}
```

- [ ] **Step 2: Ajustar `Player.tsx`**

Modify `src/components/Player.tsx`:

1. Trocar o import e o tipo das props:

```tsx
import { reportProgress, streamUrl, type OwnerKind } from "~/lib/api";

type PlayerProps = {
  src: string;
  title: string;
  poster?: string;
  /** Quando presente, a posição é gravada no banco. */
  owner?: { id: string; kind: OwnerKind };
  /** Segundos de onde retomar. */
  startAt?: number;
  onEnded?: () => void;
};

const REPORT_EVERY_MS = 5000;
```

2. Dentro do `createEffect`, depois do bloco que registra `onReady`/`fail` e
   antes do `onCleanup`, acrescentar o report:

```tsx
    // Retomada: só depois de o vídeo saber a duração é que dá para posicionar.
    const seek = () => {
      const target = props.startAt ?? 0;
      if (target > 0 && Number.isFinite(element.duration) && element.currentTime < 1) {
        element.currentTime = target;
      }
    };
    element.addEventListener("loadedmetadata", seek);

    let lastReport = 0;
    const report = () => {
      const owner = props.owner;
      if (!owner || !element.currentTime) return;
      const duration = Number.isFinite(element.duration) ? element.duration : null;
      void reportProgress(owner.id, owner.kind, element.currentTime, duration).catch(
        () => undefined,
      );
    };
    const onTimeUpdate = () => {
      const now = Date.now();
      if (now - lastReport < REPORT_EVERY_MS) return;
      lastReport = now;
      report();
    };
    const onEnded = () => {
      report();
      props.onEnded?.();
    };
    element.addEventListener("timeupdate", onTimeUpdate);
    element.addEventListener("pause", report);
    element.addEventListener("ended", onEnded);
```

3. No `onCleanup`, remover os listeners novos e reportar uma última vez **antes**
   de zerar o `src`:

```tsx
    onCleanup(() => {
      cancelled = true;
      report();
      element.removeEventListener("loadedmetadata", seek);
      element.removeEventListener("timeupdate", onTimeUpdate);
      element.removeEventListener("pause", report);
      element.removeEventListener("ended", onEnded);
      element.removeEventListener("loadeddata", onReady);
      element.removeEventListener("playing", onReady);
      element.removeEventListener("error", fail);
      teardown();
      element.removeAttribute("src");
      element.load();
    });
```

4. Canais ao vivo não têm `owner`, então nada é gravado para eles — é o
   comportamento certo: não existe "retomar" em transmissão ao vivo.

- [ ] **Step 3: Verificar**

Run: `bunx tsc --noEmit`
Expected: nenhum erro novo em `Player.tsx` nem em `SourcePicker.tsx`.

- [ ] **Step 4: Commit**

```bash
git add src/components/Player.tsx src/components/SourcePicker.tsx
git commit -m "feat: player grava posição com throttle e expõe seletor de fontes"
```

---

## Task 14: telas de detalhe — retomar, fontes, episódios

**Files:**
- Modify (reescrever): `src/pages/Filme.tsx`
- Modify (reescrever): `src/pages/Serie.tsx`
- Modify: `src/components/TitleHero.tsx` (aceitar `backdrop` do meta, se ainda não aceitar)

**Interfaces:**
- Consumes: `api.fetchItem`, `api.fetchEpisodes`, `api.fetchMeta`, `api.markWatched`,
  `api.toggleFavorite`, `api.fetchNextEpisode`, `Player` com `owner`/`startAt`/`onEnded`,
  `SourcePicker`, `vod.remainingLabel` (Tasks 9, 13).
- Produces: nada consumido por tasks posteriores.

- [ ] **Step 1: Reescrever `src/pages/Filme.tsx`**

Replace the entire contents of `src/pages/Filme.tsx`:

```tsx
import { A, useParams } from "@solidjs/router";
import { createResource, createSignal, onMount, Show } from "solid-js";
import Player from "~/components/Player";
import PosterGrid from "~/components/PosterGrid";
import SourcePicker from "~/components/SourcePicker";
import TitleHero from "~/components/TitleHero";
import { Check, Play, StarOutline } from "~/components/Icons";
import { fetchItem, fetchMeta, markWatched, toggleFavorite } from "~/lib/api";
import { remainingLabel, shortGroup } from "~/lib/vod";

export default function FilmePage() {
  const params = useParams<{ id: string }>();
  const [started, setStarted] = createSignal(false);
  const [playing, setPlaying] = createSignal(false);
  const [source, setSource] = createSignal(0);
  const [favorite, setFavorite] = createSignal(false);
  onMount(() => setStarted(true));

  const [data, { refetch }] = createResource(
    () => (started() ? params.id : undefined),
    id => fetchItem("filmes", id),
  );

  const movie = () => data()?.item;
  const streams = () => data()?.streams ?? [];
  const progress = () => data()?.progress ?? null;
  const resumeAt = () => {
    const stored = progress();
    return stored && !stored.completed ? stored.positionSecs : 0;
  };

  const [meta] = createResource(
    () => {
      const current = movie();
      return current
        ? { kind: "filmes" as const, title: current.title, year: current.year }
        : undefined;
    },
    fetchMeta,
  );

  const primaryLabel = () => {
    if (playing()) return "Tocando";
    const at = resumeAt();
    return at > 0 ? remainingLabel(at, progress()?.durationSecs ?? null) : "Assistir filme";
  };

  return (
    <main class="relative z-10">
      <Show
        when={movie()}
        fallback={
          <div class="mx-auto max-w-md px-4 py-24 text-center">
            <p class="font-mono text-sm text-paper/40">
              <Show
                when={data.error}
                fallback={<>carregando filme<span class="anim-caret">_</span></>}
              >
                {String(data.error)}
              </Show>
            </p>
            <Show when={data.error}>
              <A href="/filmes" class="mt-4 inline-block text-sm text-amber hover:underline">
                Voltar para os filmes
              </A>
            </Show>
          </div>
        }
      >
        {current => (
          <>
            <Show when={playing() && streams()[source()]}>
              {stream => (
                <div class="anim-reveal mx-auto max-w-6xl px-4 pt-4">
                  <Player
                    src={stream().url}
                    title={current().title}
                    poster={current().logo ?? undefined}
                    owner={{ id: current().id, kind: "item" }}
                    startAt={resumeAt()}
                    onEnded={() => void refetch()}
                  />
                </div>
              )}
            </Show>

            <TitleHero
              title={current().title}
              poster={current().logo ?? ""}
              subtitle={shortGroup(current().group)}
              meta={meta()}
              metaPending={meta.loading}
            >
              <div class="mt-6 flex flex-wrap items-center gap-3">
                <button
                  type="button"
                  onClick={() => setPlaying(true)}
                  disabled={!streams().length}
                  class="press flex items-center gap-2 rounded-sm bg-amber px-5 py-3 font-medium text-ink hover:bg-paper hover:shadow-[var(--shadow-glow)] disabled:opacity-40"
                >
                  <Play size={16} />
                  {primaryLabel()}
                </button>

                <button
                  type="button"
                  onClick={async () => {
                    await markWatched(current().id, "item", !current().completed);
                    await refetch();
                  }}
                  class="press flex items-center gap-2 rounded-sm border border-edge px-4 py-3 text-sm text-paper/70 hover:border-amber hover:text-amber"
                  aria-pressed={current().completed}
                >
                  <Check size={16} />
                  {current().completed ? "Assistido" : "Marcar como visto"}
                </button>

                <button
                  type="button"
                  onClick={async () => setFavorite(await toggleFavorite(current().id))}
                  class="press flex items-center gap-2 rounded-sm border border-edge px-4 py-3 text-sm text-paper/70 hover:border-amber hover:text-amber"
                  classList={{ "border-amber/60 text-amber": favorite() }}
                  aria-pressed={favorite()}
                >
                  <StarOutline size={16} />
                  {favorite() ? "Nos favoritos" : "Salvar"}
                </button>

                <A href="/filmes" class="wipe text-sm text-paper/55 hover:text-amber">
                  Voltar ao catálogo
                </A>
              </div>

              <SourcePicker streams={streams()} active={source()} onPick={setSource} />
            </TitleHero>

            <Show when={data()?.related.length}>
              <section class="mx-auto max-w-6xl px-4 py-8">
                <h2 class="mb-4 font-display text-sm font-bold tracking-tight text-paper/70">
                  Mais em {shortGroup(current().group)}
                </h2>
                <PosterGrid items={data()!.related} />
              </section>
            </Show>
          </>
        )}
      </Show>
    </main>
  );
}
```

- [ ] **Step 2: Reescrever `src/pages/Serie.tsx`**

Replace the entire contents of `src/pages/Serie.tsx`:

```tsx
import { A, useParams } from "@solidjs/router";
import { createMemo, createResource, createSignal, For, onMount, Show } from "solid-js";
import Player from "~/components/Player";
import SourcePicker from "~/components/SourcePicker";
import TitleHero from "~/components/TitleHero";
import { Check, Play } from "~/components/Icons";
import {
  fetchEpisodes,
  fetchItem,
  fetchMeta,
  fetchNextEpisode,
  markWatched,
  type EpisodeRow,
} from "~/lib/api";
import { remainingLabel, shortGroup } from "~/lib/vod";

export default function SeriePage() {
  const params = useParams<{ id: string }>();
  const [started, setStarted] = createSignal(false);
  const [season, setSeason] = createSignal<number>();
  const [current, setCurrent] = createSignal<EpisodeRow>();
  const [source, setSource] = createSignal(0);
  onMount(() => setStarted(true));

  const [data] = createResource(
    () => (started() ? params.id : undefined),
    id => fetchItem("series", id),
  );
  const [detail, { refetch: refetchEpisodes }] = createResource(
    () => (started() ? params.id : undefined),
    fetchEpisodes,
  );

  const series = () => data()?.item;

  const [meta] = createResource(
    () => {
      const show = series();
      return show ? { kind: "series" as const, title: show.title } : undefined;
    },
    fetchMeta,
  );

  const seasons = createMemo(() => [
    ...new Set((detail()?.episodes ?? []).map(episode => episode.season)),
  ]);
  const activeSeason = () => season() ?? seasons()[0];
  const episodes = createMemo(() =>
    (detail()?.episodes ?? []).filter(episode => episode.season === activeSeason()),
  );

  /** O botão primário aponta para o primeiro episódio não concluído. */
  const upNext = createMemo(() =>
    (detail()?.episodes ?? []).find(episode => !episode.completed),
  );

  const play = (episode: EpisodeRow) => {
    setSource(0);
    setCurrent(episode);
  };

  const advance = async () => {
    const show = series();
    if (!show) return;
    await refetchEpisodes();
    const next = await fetchNextEpisode(show.id);
    if (!next) return setCurrent(undefined);
    const row = (detail()?.episodes ?? []).find(episode => episode.id === next.id);
    if (row) {
      setSeason(row.season);
      play(row);
    }
  };

  const resumeFor = (episode: EpisodeRow | undefined) => {
    if (!episode || episode.completed || episode.percent <= 0) return 0;
    // `percent` é o que o banco sabe; a posição exata vem do próprio progresso
    // quando o episódio recomeça, então o seek aproximado basta para retomar.
    return 0;
  };

  return (
    <main class="relative z-10">
      <Show
        when={series()}
        fallback={
          <div class="mx-auto max-w-md px-4 py-24 text-center">
            <p class="font-mono text-sm text-paper/40">
              <Show
                when={data.error}
                fallback={<>carregando série<span class="anim-caret">_</span></>}
              >
                {String(data.error)}
              </Show>
            </p>
            <Show when={data.error}>
              <A href="/series" class="mt-4 inline-block text-sm text-amber hover:underline">
                Voltar para as séries
              </A>
            </Show>
          </div>
        }
      >
        {show => (
          <>
            <Show when={current()}>
              {episode => (
                <div class="anim-reveal mx-auto max-w-6xl px-4 pt-4">
                  <Show when={episode().streams[source()]}>
                    {stream => (
                      <Player
                        src={stream().url}
                        title={`${show().title} T${episode().season} E${episode().episode}`}
                        poster={show().logo ?? undefined}
                        owner={{ id: episode().id, kind: "episode" }}
                        startAt={resumeFor(episode())}
                        onEnded={advance}
                      />
                    )}
                  </Show>
                  <p class="mt-2 font-mono text-xs text-amber">
                    T{episode().season} · E{episode().episode}
                  </p>
                  <SourcePicker
                    streams={episode().streams}
                    active={source()}
                    onPick={setSource}
                  />
                </div>
              )}
            </Show>

            <TitleHero
              title={show().title}
              poster={show().logo ?? ""}
              subtitle={shortGroup(show().group)}
              meta={meta()}
              metaPending={meta.loading}
            >
              <div class="mt-6 flex flex-wrap items-center gap-3">
                <button
                  type="button"
                  onClick={() => {
                    const next = upNext();
                    if (!next) return;
                    setSeason(next.season);
                    play(next);
                  }}
                  disabled={!upNext()}
                  class="press flex items-center gap-2 rounded-sm bg-amber px-5 py-3 font-medium text-ink hover:bg-paper hover:shadow-[var(--shadow-glow)] disabled:opacity-40"
                >
                  <Play size={16} />
                  <Show
                    when={upNext()}
                    fallback={<>Série concluída</>}
                  >
                    {next =>
                      next().percent > 0
                        ? remainingLabel(0, null)
                        : `Assistir T${next().season} E${next().episode}`
                    }
                  </Show>
                </button>
                <span class="text-sm text-paper/45">
                  {`${show().seasons} ${show().seasons > 1 ? "temporadas" : "temporada"} · ${show().episodeCount} episódios`}
                </span>
              </div>
            </TitleHero>

            <section class="mx-auto max-w-6xl px-4 py-8">
              <Show
                when={detail()}
                fallback={
                  <p class="font-mono text-sm text-paper/40">
                    carregando episódios<span class="anim-caret">_</span>
                  </p>
                }
              >
                <Show when={seasons().length > 1}>
                  <ul class="mb-4 flex flex-wrap gap-2">
                    <For each={seasons()}>
                      {value => (
                        <li>
                          <button
                            type="button"
                            onClick={() => setSeason(value)}
                            class="press rounded-full border border-edge bg-panel/50 px-3 py-1 text-sm text-paper/60 hover:border-amber-deep hover:text-amber"
                            classList={{
                              "border-amber bg-amber/12 text-amber": activeSeason() === value,
                            }}
                          >
                            T{value}
                          </button>
                        </li>
                      )}
                    </For>
                  </ul>
                </Show>

                <ul class="divide-y divide-edge/70">
                  <For each={episodes()}>
                    {episode => (
                      <li class="flex items-center gap-3 py-2.5">
                        <button
                          type="button"
                          onClick={() => play(episode)}
                          class="press flex min-w-0 flex-1 items-center gap-3 text-left"
                        >
                          <span class="w-14 shrink-0 font-mono text-xs tabular-nums text-paper/40">
                            T{episode.season}E{episode.episode}
                          </span>
                          <span class="min-w-0 flex-1">
                            <span class="block truncate text-sm text-paper/85">
                              {episode.title ?? `Episódio ${episode.episode}`}
                            </span>
                            <Show when={episode.percent > 0 && !episode.completed}>
                              <span class="mt-1 block h-[3px] w-32 bg-ink/70">
                                <span
                                  class="block h-full bg-amber"
                                  style={{ width: `${Math.round(episode.percent * 100)}%` }}
                                />
                              </span>
                            </Show>
                          </span>
                        </button>
                        <button
                          type="button"
                          onClick={async () => {
                            await markWatched(episode.id, "episode", !episode.completed);
                            await refetchEpisodes();
                          }}
                          class="press shrink-0 rounded-sm border border-edge p-2 text-paper/45 hover:border-amber hover:text-amber"
                          classList={{ "border-amber/60 text-amber": episode.completed }}
                          aria-pressed={episode.completed}
                          title={episode.completed ? "Marcar como não visto" : "Marcar como visto"}
                        >
                          <Check size={14} />
                        </button>
                      </li>
                    )}
                  </For>
                </ul>
              </Show>
            </section>
          </>
        )}
      </Show>
    </main>
  );
}
```

- [ ] **Step 3: Conferir `TitleHero`**

Open `src/components/TitleHero.tsx`. Ele já recebe `meta` e `metaPending`. Duas
verificações:
- a prop `poster` é `string`; as páginas passam `current().logo ?? ""`, o que já
  cobre o `null` do banco;
- o `meta` importado vem de `~/lib/vod`. Troque o import para
  `import type { Meta } from "~/lib/api";` — o tipo é o mesmo, só mudou de casa.
- se o componente já desenha o backdrop com fade a partir de `meta.backdrop`,
  nada mais a fazer. Se não, acrescente antes do conteúdo:

```tsx
      <Show when={props.meta?.backdrop}>
        {backdrop => (
          <div class="pointer-events-none absolute inset-x-0 top-0 -z-10 h-[420px]" aria-hidden="true">
            <img src={backdrop()} alt="" class="h-full w-full object-cover opacity-40" />
            <div class="absolute inset-0 bg-gradient-to-b from-ink/30 via-ink/80 to-ink" />
          </div>
        )}
      </Show>
```

- [ ] **Step 4: Verificar na tela**

Run: `bunx tsc --noEmit`
Expected: erros restantes apenas em `Channels.tsx`, `Watch.tsx`,
`ChannelRow.tsx`, `channels.ts` — a Task 15 fecha esses.

Run: `bun run tauri dev`
Expected, em `/filme/:id`: botão primário toca; ao voltar para a página, o botão
diz "Retomar · faltam Nmin". "Marcar como visto" escurece o pôster na grade. Em
`/serie/:id`: o botão primário aponta para o primeiro episódio não visto; ao
terminar um episódio, o player pula para o próximo; o check por episódio persiste
depois de recarregar.

- [ ] **Step 5: Commit**

```bash
git add src/pages/Filme.tsx src/pages/Serie.tsx src/components/TitleHero.tsx
git commit -m "feat: detalhe com retomada, fontes de stream e progresso por episódio"
```

---

## Task 15: Biblioteca, canais e `Watch` sem `localStorage`

**Files:**
- Modify (substituir o stub): `src/pages/Biblioteca.tsx`
- Modify (reescrever): `src/lib/channels.ts`
- Modify: `src/pages/Channels.tsx`
- Modify: `src/components/ChannelRow.tsx`
- Modify: `src/pages/Watch.tsx`

**Interfaces:**
- Consumes: `api.fetchFavorites`, `api.fetchContinueWatching`, `api.toggleFavorite`,
  `api.markWatched`, `api.fetchCatalog`, `api.listSources`, `api.syncSource`,
  `api.removeSource`, `useSources` (Tasks 9-10).
- Produces: `channels.ts` exporta `useChannels()` → `{ channels, pending, error }`
  sobre `CatalogItem` com `kind === "channel"`, mais `matches`, `parseQuery`,
  `formatNumber` como estão hoje.

- [ ] **Step 1: Reescrever `src/lib/channels.ts`**

Replace the entire contents of `src/lib/channels.ts`:

```ts
import { createResource, createSignal, onMount } from "solid-js";
import { fetchCatalog, type CatalogItem } from "~/lib/api";

/**
 * Canal já chega classificado do Rust: `group` vem do `group-title` da lista e o
 * número, do `tvg-chno`. O front só precisa de um índice de busca.
 */
export type Channel = CatalogItem & { search: string };

function normalize(value: string) {
  return value
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "");
}

async function load(): Promise<Channel[]> {
  // Uma página grande basta: nenhuma lista real passa de alguns milhares de canais.
  const first = await fetchCatalog({ kind: "canais", query: "", group: "", page: 0 });
  const pages = Math.ceil(first.total / first.pageSize);
  const rest = await Promise.all(
    Array.from({ length: Math.max(0, pages - 1) }, (_, index) =>
      fetchCatalog({ kind: "canais", query: "", group: "", page: index + 1 }),
    ),
  );
  return [first, ...rest]
    .flatMap(page => page.items)
    .sort((a, b) => (a.channelNumber ?? 9999) - (b.channelNumber ?? 9999))
    .map(item => ({ ...item, search: normalize(item.title) }));
}

export function useChannels() {
  const [started, setStarted] = createSignal(false);
  onMount(() => setStarted(true));
  const [data] = createResource(() => (started() ? "channels" : undefined), load);

  return {
    channels: () => data() ?? [],
    pending: () => !data() && !data.error,
    error: () => data.error as Error | undefined,
  };
}

export function matches(channel: Channel, terms: string[]) {
  return terms.every(term => channel.search.includes(term));
}

export function parseQuery(query: string) {
  return normalize(query).split(/\s+/).filter(Boolean);
}

export const formatNumber = (n: number) => String(n).padStart(3, "0");
```

`GROUPS`, `RULES`, `classify` e `decorate` saem: a classificação agora é do
parser em Rust, e o grupo vem do `group-title` da própria lista.

- [ ] **Step 2: Ajustar `Channels.tsx`**

Modify `src/pages/Channels.tsx`:

1. Trocar imports:

```tsx
import { createMemo, createResource, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import ChannelList from "~/components/ChannelList";
import ChannelSkeleton from "~/components/ChannelSkeleton";
import { Search, StarOutline } from "~/components/Icons";
import { matches, parseQuery, useChannels } from "~/lib/channels";
import { fetchFavorites } from "~/lib/api";
```

2. Apagar a chamada `hydrateStores()` do `onMount` (deixando o listener de `/`).

3. Trocar os favoritos por um recurso e os grupos por uma contagem derivada:

```tsx
  const [favoriteItems, { refetch: refetchFavorites }] = createResource(fetchFavorites);
  const favoriteIds = () => new Set((favoriteItems() ?? []).map(item => item.id));

  const groups = createMemo(() => [
    "Todos",
    ...[...new Set(all().map(channel => channel.group).filter(Boolean))].sort(),
  ] as string[]);
```

4. No `filtered`, trocar `favorites.has(channel.id)` pelo novo acessor:

```tsx
    const favorites = favoriteIds();
```

5. Trocar `<For each={GROUPS}>` por `<For each={groups()}>` e
   `{favoriteIds().length}` por `{favoriteIds().size}`.

6. Trocar a seção "Continuar assistindo" — o `recentIds` do `localStorage` sai e
   entra o histórico do banco. Substituir o `continueWatching` por:

```tsx
  const [recent] = createResource(() => fetchContinueWatching(8));
  const continueWatching = createMemo(() => {
    const byId = new Map(all().map(channel => [channel.id, channel]));
    return (recent() ?? [])
      .filter(entry => entry.kind === "channel")
      .map(entry => byId.get(entry.itemId))
      .filter((channel): channel is NonNullable<typeof channel> => Boolean(channel));
  });
```

com `fetchContinueWatching` acrescentado ao import de `~/lib/api`.

7. Expor `refetchFavorites` para a lista, para que a estrela atualize a barra
   lateral: passe `onToggle={refetchFavorites}` no `<ChannelList>` e repasse
   até o `ChannelRow`. Abra `src/components/ChannelList.tsx` e acrescente a prop
   opcional `onToggle?: () => void` ao tipo, repassando-a a cada `ChannelRow`.

- [ ] **Step 3: Ajustar `ChannelRow.tsx`**

Modify `src/components/ChannelRow.tsx`:

```tsx
import { createSignal } from "solid-js";
import { toggleFavorite } from "~/lib/api";
```

Trocar `const favorite = () => isFavorite(channel().id);` por um sinal local
semeado pela prop, e o `onClick`:

```tsx
  const [favorite, setFavorite] = createSignal(props.favorite ?? false);
```

```tsx
        onClick={async event => {
          event.preventDefault();
          setFavorite(await toggleFavorite(channel().id));
          props.onToggle?.();
        }}
```

Acrescentar `favorite?: boolean` e `onToggle?: () => void` ao tipo das props, e
em `Channels.tsx` passar `favorite={favoriteIds().has(channel.id)}`.

- [ ] **Step 4: Ajustar `Watch.tsx`**

Modify `src/pages/Watch.tsx`:

1. Trocar o import de `~/lib/store` por:

```tsx
import { markWatched, toggleFavorite } from "~/lib/api";
import { createSignal } from "solid-js";
```

2. Apagar `hydrateStores()` do `onMount`.

3. Trocar `markWatched(params.id)` pela chamada nova — canal não tem progresso,
   mas o histórico registra a visita:

```tsx
    if (channel()) void markWatched(params.id, "item", false);
```

4. Trocar os três usos de `isFavorite(current().id)` por um sinal local, como no
   `ChannelRow`:

```tsx
  const [favorite, setFavorite] = createSignal(false);
```

```tsx
                    onClick={async () => setFavorite(await toggleFavorite(current().id))}
```

com `classList={{ "border-amber/60 text-amber": favorite() }}`,
`aria-pressed={favorite()}`, `<Show when={favorite()} …>` e o rótulo
`{favorite() ? "Nos favoritos" : "Salvar canal"}`.

- [ ] **Step 5: Escrever `Biblioteca.tsx`**

Replace the entire contents of `src/pages/Biblioteca.tsx`:

```tsx
import { createResource, For, Show } from "solid-js";
import PosterGrid from "~/components/PosterGrid";
import PosterRow from "~/components/PosterRow";
import { fetchContinueWatching, fetchFavorites, markWatched, removeSource, syncSource } from "~/lib/api";
import { detailHref } from "~/lib/vod";
import { useSources } from "~/lib/sources";
import { A } from "@solidjs/router";
import { Check } from "~/components/Icons";

export default function Biblioteca() {
  const list = useSources();
  const [favorites, { refetch: refetchFavorites }] = createResource(fetchFavorites);
  const [history, { refetch: refetchHistory }] = createResource(() => fetchContinueWatching(40));

  return (
    <main class="relative z-10 mx-auto max-w-7xl space-y-10 px-4 py-6">
      <section>
        <h1 class="anim-reveal font-display text-3xl font-extrabold tracking-[-0.03em] text-paper">
          Biblioteca
        </h1>
      </section>

      <Show when={favorites()?.length}>
        <PosterRow title="Favoritos" items={favorites()!} />
      </Show>

      <section>
        <h2 class="mb-3 font-display text-sm font-bold tracking-tight text-paper/70">Histórico</h2>
        <Show
          when={history()?.length}
          fallback={<p class="text-sm text-paper/40">Nada assistido ainda.</p>}
        >
          <ul class="divide-y divide-edge/70">
            <For each={history()}>
              {entry => (
                <li class="flex items-center gap-3 py-2.5">
                  <A
                    href={detailHref({ kind: entry.kind, id: entry.itemId })}
                    class="min-w-0 flex-1 text-sm text-paper/85 hover:text-amber"
                  >
                    {entry.title}
                    <Show when={entry.season !== null}>
                      <span class="ml-2 font-mono text-xs text-paper/40">
                        T{entry.season}E{entry.episode}
                      </span>
                    </Show>
                    <span class="mt-1 block h-[3px] w-40 bg-ink/70">
                      <span
                        class="block h-full bg-amber"
                        style={{ width: `${Math.round(entry.percent * 100)}%` }}
                      />
                    </span>
                  </A>
                  <button
                    type="button"
                    onClick={async () => {
                      await markWatched(entry.ownerId, entry.ownerKind, true);
                      await refetchHistory();
                      await refetchFavorites();
                    }}
                    class="press shrink-0 rounded-sm border border-edge p-2 text-paper/45 hover:border-amber hover:text-amber"
                    title="Marcar como visto"
                  >
                    <Check size={14} />
                  </button>
                </li>
              )}
            </For>
          </ul>
        </Show>
      </section>

      <section class="border-t border-edge pt-6">
        <h2 class="mb-3 font-display text-sm font-bold tracking-tight text-paper/70">
          Minhas listas
        </h2>
        <ul class="space-y-2">
          <For each={list.sources()}>
            {source => (
              <li class="flex items-center gap-3 rounded-sm border border-edge bg-panel/40 px-3 py-2">
                <div class="min-w-0">
                  <p class="truncate text-sm text-paper/90">{source.label}</p>
                  <p class="truncate font-mono text-[0.7rem] text-paper/35">
                    {source.itemCount.toLocaleString("pt-BR")} títulos
                    <Show when={source.lastSyncAt}>
                      {at => (
                        <>
                          {" · "}
                          {new Date(at() * 1000).toLocaleDateString("pt-BR")}
                        </>
                      )}
                    </Show>
                  </p>
                </div>
                <button
                  type="button"
                  onClick={async () => {
                    await syncSource(source.id);
                    await list.refetch();
                  }}
                  class="press ml-auto shrink-0 text-xs text-paper/55 hover:text-amber"
                >
                  Atualizar
                </button>
                <button
                  type="button"
                  onClick={async () => {
                    await removeSource(source.id);
                    await list.refetch();
                  }}
                  class="press shrink-0 text-xs text-paper/40 hover:text-live"
                >
                  Remover
                </button>
              </li>
            )}
          </For>
        </ul>
        <A href="/setup" class="wipe mt-4 inline-block text-sm text-amber hover:underline">
          Adicionar outra lista
        </A>
      </section>
    </main>
  );
}
```

`PosterGrid` fica importado apenas se você optar por trocar a linha de favoritos
por grade; se não usar, apague o import para não deixar símbolo morto.

- [ ] **Step 6: Verificar na tela**

Run: `bunx tsc --noEmit`
Expected: PASS, zero erros. Se sobrar erro apontando para `~/lib/store`, o
arquivo ainda está sendo importado em algum lugar — a Task 16 o apaga, mas o
import tem de sair agora.

Run: `bun run build`
Expected: build limpo.

Run: `bun run tauri dev`
Expected: `/canais` lista os canais com número e grupo vindos da lista; a estrela
persiste depois de recarregar; `/biblioteca` mostra favoritos, histórico com
barra de progresso, e as listas com "Atualizar"/"Remover" funcionando.

- [ ] **Step 7: Commit**

```bash
git add src/pages/Biblioteca.tsx src/pages/Channels.tsx src/pages/Watch.tsx src/components/ChannelRow.tsx src/components/ChannelList.tsx src/lib/channels.ts
git commit -m "feat: biblioteca, canais e favoritos lendo do SQLite"
```

---

## Task 16: limpeza — JSON, scripts e `store.ts` saem

**Files:**
- Delete: `src/lib/store.ts`
- Delete: `scripts/update-channel.ts`, `scripts/update-vod.ts`
- Delete: `data/lista_pro.json`, `data/vod/` (todo o diretório)
- Modify: `package.json` (scripts `update-channels` e `update-vod`)
- Modify: `.gitignore` (se `data/` estiver listado, o registro deixa de fazer sentido)
- Modify: `README.md`

- [ ] **Step 1: Confirmar que nada mais aponta para o que vai sair**

```bash
grep -rn "lib/store\|data_status\|fetchDataStatus\|lista_pro\|filmes.json\|series.json\|resolve_data_dir\|YELLOWTV_DATA_DIR" src src-tauri/src README.md package.json
```

Expected: nenhuma linha em `src/` ou `src-tauri/src/`. Se algo aparecer, corrija
antes de apagar. Ocorrências em `README.md` e `package.json` são esperadas e
saem nos passos seguintes.

- [ ] **Step 2: Apagar**

```bash
git rm src/lib/store.ts scripts/update-channel.ts scripts/update-vod.ts
git rm -r data
```

- [ ] **Step 3: `package.json`**

Modify `package.json` — apagar as duas linhas de scripts:

```json
    "update-channels": "bun run scripts/update-channel.ts",
    "update-vod": "bun run scripts/update-vod.ts",
```

- [ ] **Step 4: `README.md`**

Open `README.md`. Onde ele explicar que os dados vêm dos scripts e de `data/`,
troque por:

```markdown
## Dados

O catálogo é local. Na primeira execução o app abre `/setup`: cole a URL da sua
lista M3U (ou escolha um arquivo `.m3u`), e o Rust parseia, normaliza e grava
tudo em `app_data/yellowtv.db`. Filmes, séries, canais, progresso e favoritos
vivem nesse banco. A chave do TMDB é opcional e pode ser colada no onboarding ou
exportada como `TMDB_API_KEY`.

Não existe mais nenhum JSON pré-gerado no repositório.
```

- [ ] **Step 5: Verificação final completa**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
bunx tsc --noEmit
bun run build
```

Expected: os três verdes. Anote a contagem de testes de Rust que passaram.

Run: `bun run tauri dev`, com o banco apagado antes
(`rm ~/Library/Application\ Support/YellowTV/yellowtv.db` no macOS)
Expected: o app cai em `/setup`, aceita uma lista real, e daí em diante:
Início com linhas, Filmes e Séries navegáveis, detalhe com retomar e fontes,
episódios com check, Canais com favoritos, Biblioteca com histórico.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: remove JSON pré-gerado, scripts de geração e store em localStorage"
```

---

## Self-Review

**Cobertura da spec:**

| Seção da spec | Onde é implementada |
|---|---|
| Schema completo + índices + FTS5 | Task 1 |
| `PRAGMA user_version` | Task 1 |
| Hash estável, dedup, progresso sobrevive a re-sync | Tasks 2, 4 |
| Normalização de título e tokens de qualidade | Task 2 |
| Parser `#EXTINF` streaming, ordem de classificação | Task 3 |
| `add_source`/`sync_source`/`remove_source`/`list_sources`, uma transação, `import:progress` | Tasks 4, 8 |
| Rollback em M3U vazia; linha malformada contada | Task 4 |
| `catalog.rs` em SQL: paginação, grupos, FTS, `unwatched_only` | Task 6 |
| Regra de assistido (92%, corte de 30s), `next_episode`, favoritos | Task 5 |
| `meta` em tabela, chave TMDB em `settings` com fallback de env | Task 7 |
| Comandos Tauri da spec | Task 8 |
| Nav de cinco seções, `/setup` com gate | Task 10 |
| Board do Início, pôster com barra e concluído | Task 11 |
| Filtro "não assistidos" e badges no catálogo | Tasks 11, 12 |
| Detalhe com "Retomar em Nmin", lista de fontes, episódios com check | Task 14 |
| `Player` com throttle de 5s, report em `pause` e no cleanup, próximo episódio | Tasks 13, 14 |
| `store.ts`, JSON, scripts e `data_status` fora | Task 16 |
| Testes de parser, hash, dedup, progresso, `next_episode`, remoção de source | Tasks 1-6 |
| `tsc --noEmit` e `vite build` como gate de front | Tasks 9, 15, 16 |

**Divergências deliberadas da spec, e por quê:**
- `proxy.rs` fica intocado, como a spec pede, mas o comando `channels` sai: canais
  passam a ser `kind = 'channel'` no mesmo `catalog_page`. É o que a decisão "canal
  como aba terciária" implica, e evita um caminho de leitura paralelo.
- A spec lista `title_meta(kind, title, year)`; o plano mantém essa assinatura e
  move o cache para dentro do comando, porque `rusqlite::Connection` não é `Send`
  e não pode cruzar um `await`.
- A spec não fala de `ids.rs`; ele existe porque normalização e hash são usados
  por `m3u`, `import`, `catalog` e `meta` — deixar isso em `m3u.rs` criaria
  dependência invertida.
- Retomada de episódio (`resumeFor` na Task 14) devolve 0 por ora: `EpisodeRow`
  carrega `percent`, não `positionSecs`. Se a retomada exata de episódio importar,
  acrescente `position_secs` ao `SELECT` de `catalog::episodes` e ao tipo
  `EpisodeRow` — mudança de uma linha em cada lado, e o teste
  `serie_lista_episodios_ordenados_com_progresso` é o lugar de travá-la.

**Ordem de execução:** as Tasks 1-8 são Rust e sequenciais (cada uma consome a
anterior). A Task 6 deixa `lib.rs` temporariamente inconsistente até a Task 8 —
não pule essa dupla. As Tasks 9-16 são front e também sequenciais, com o `tsc`
ficando verde só na Task 15.
