# YellowTV

Player de listas IPTV em Tauri 2 + Solid. Canais ao vivo, filmes e séries com
sinopse e elenco, tudo rodando localmente — nenhum servidor, nenhuma hospedagem.

## Rodar

```sh
bun install
bun run tauri dev
```

## Dados

O catálogo é local. Na primeira execução o app abre `/setup`: cole a URL da sua
lista M3U (ou escolha um arquivo `.m3u`), e o Rust parseia, normaliza e grava
tudo em `app_data/yellowtv.db`. Filmes, séries, canais, progresso e favoritos
vivem nesse banco. A chave do TMDB é opcional e pode ser colada no onboarding ou
exportada como `TMDB_API_KEY`.

Não existe mais nenhum JSON pré-gerado no repositório.

## Sinopse e elenco

Vêm do TMDB — o IMDb não tem API pública, e é lá que os pôsteres das listas já
estão hospedados. Sem chave o app funciona, só sem sinopse:

```sh
TMDB_API_KEY=sua_chave bun run tauri dev
```

Aceita chave v3 ou token v4. As respostas ficam em cache na pasta de dados do app.

## Como o vídeo toca

As listas servem em HTTP puro e a WebView bloqueia conteúdo inseguro, então o Rust
sobe um proxy em `127.0.0.1` e repassa o stream — inclusive o cabeçalho `Range`,
para arrastar a barra de um filme. O motor é escolhido pela URL: `.m3u8` vai para
o hls.js, `.mp4` toca nativo, e o resto (MPEG-TS dos canais) vai para o mpegts.js.

## Testes

```sh
cd src-tauri && cargo test
```
