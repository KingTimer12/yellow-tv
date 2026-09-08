# YellowTV

Player de listas IPTV em Tauri 2 + Solid. Canais ao vivo, filmes e séries com
sinopse e elenco, tudo rodando localmente — nenhum servidor, nenhuma hospedagem.

## Rodar

```sh
bun install
bun run tauri dev
```

## As listas

O app lê três coisas de um diretório de dados:

```
<dados>/lista_pro.json        canais de TV
<dados>/vod/filmes.json       filmes
<dados>/vod/series.json       séries (sem episódios)
<dados>/vod/series/<id>.json  episódios de uma série
```

Para gerar:

```sh
bun run update-channels   # canais
bun run update-vod        # filmes e séries
```

Por padrão os scripts escrevem em `./data`; `YELLOWTV_DATA_DIR` muda o destino.

O Rust procura os dados nesta ordem: `YELLOWTV_DATA_DIR`, a pasta de dados do app
(`~/Library/Application Support/YellowTV/data` no macOS), `./data` e `../data`.
O caminho escolhido aparece no console ao iniciar.

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
