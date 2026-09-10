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

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
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
    let conn = state.conn()?;
    import::list_sources(&conn)
}

/// Arquivo temporário que se apaga sozinho quando sai de escopo.
struct TempFile {
    path: std::path::PathBuf,
}

impl TempFile {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        let name = format!("yellowtv-import-{}-{unique}.m3u", std::process::id());
        Self {
            path: std::env::temp_dir().join(name),
        }
    }

    fn open(&self) -> Result<Box<dyn std::io::BufRead>, String> {
        let file = std::fs::File::open(&self.path)
            .map_err(|error| format!("não foi possível reabrir o arquivo baixado: {error}"))?;
        Ok(Box::new(BufReader::new(file)))
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn open_local(path: &str) -> Result<Box<dyn std::io::BufRead>, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("não foi possível abrir {path}: {error}"))?;
    Ok(Box::new(BufReader::new(file)))
}

/// Baixa a lista para um arquivo temporário, em streaming.
///
/// O download é assíncrono por obrigação, não por gosto: `reqwest::blocking`
/// monta um runtime tokio só dele e o dropa ao sair da função, o que entra em
/// pânico quando a chamada já está dentro de um runtime — que é o caso de
/// qualquer comando `async` do Tauri. Gravar em disco em vez de na memória
/// mantém o parse em streaming e o consumo constante mesmo em listas grandes.
async fn download_to_temp(url: &str) -> Result<TempFile, String> {
    // `without_url` em todo erro: a URL de uma lista Xtream carrega
    // `username`/`password` e não pode aparecer numa mensagem de erro.
    let response = reqwest::get(url)
        .await
        .map_err(|error| format!("não foi possível baixar a lista: {}", error.without_url()))?;
    if !response.status().is_success() {
        return Err(format!("a lista respondeu {}", response.status()));
    }

    let temp = TempFile::new();
    let mut file = tokio::fs::File::create(&temp.path)
        .await
        .map_err(|error| format!("não foi possível criar o arquivo temporário: {error}"))?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|error| format!("download interrompido: {}", error.without_url()))?;
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("falha ao gravar a lista baixada: {error}"))?;
    }
    file.flush()
        .await
        .map_err(|error| format!("falha ao gravar a lista baixada: {error}"))?;
    Ok(temp)
}

/// Abre a origem da lista. Fica fora dos comandos porque o `TempFile` precisa
/// continuar vivo enquanto a ingestão lê o arquivo.
async fn open_source(source: &Source) -> (Result<Box<dyn std::io::BufRead>, String>, Option<TempFile>) {
    if source.kind == "file" {
        return (open_local(&source.url), None);
    }
    match download_to_temp(&source.url).await {
        Ok(temp) => {
            let reader = temp.open();
            (reader, Some(temp))
        }
        Err(error) => (Err(error), None),
    }
}

/// Os dois helpers abaixo existem para que nenhum `MutexGuard` do `Connection`
/// (que não é `Send`) apareça no corpo de uma função `async`.
fn begin_import(
    state: &AppState,
    url: &str,
    label: &str,
    kind: &str,
) -> Result<(Source, bool), String> {
    let conn = state.conn()?;
    import::begin_add(&conn, url, label, kind)
}

fn finish_import(
    app: &AppHandle,
    state: &AppState,
    source: &Source,
    was_new: bool,
    reader: Result<Box<dyn std::io::BufRead>, String>,
) -> Result<ImportReport, String> {
    let mut conn = state.conn()?;
    let source_id = source.id;
    let handle = app.clone();
    import::finish_add(&mut conn, source, was_new, reader, &mut move |parsed| {
        let _ = handle.emit("import:progress", ImportProgress { source_id, parsed });
    })
}

#[tauri::command]
async fn add_source(
    app: AppHandle,
    state: State<'_, AppState>,
    url_or_path: String,
    kind: String,
    label: Option<String>,
) -> Result<ImportReport, String> {
    let label = label
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default_label(&url_or_path));
    let (source, was_new) = begin_import(&state, &url_or_path, &label, &kind)?;
    // O download roda sem o lock; só depois o banco é travado para a ingestão.
    let (reader, _temp) = open_source(&source).await;
    finish_import(&app, &state, &source, was_new, reader)
}

fn load_source(state: &AppState, id: i64) -> Result<Source, String> {
    let conn = state.conn()?;
    import::get_source(&conn, id)
}

#[tauri::command]
async fn sync_source(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<ImportReport, String> {
    let source = load_source(&state, id)?;
    let (reader, _temp) = open_source(&source).await;
    finish_import(&app, &state, &source, false, reader)
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
    let conn = state.conn()?;
    catalog::page(
        &conn,
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
    let conn = state.conn()?;
    catalog::item(&conn, Kind::parse(&kind)?, &id)
}

#[tauri::command]
fn series_episodes(state: State<'_, AppState>, id: String) -> Result<SeriesEpisodes, String> {
    let conn = state.conn()?;
    catalog::episodes(&conn, &id)
}

#[tauri::command]
fn board(state: State<'_, AppState>) -> Result<Vec<Row>, String> {
    let conn = state.conn()?;
    catalog::board(&conn)
}

#[tauri::command]
fn continue_watching(state: State<'_, AppState>, limit: i64) -> Result<Vec<ContinueEntry>, String> {
    let conn = state.conn()?;
    library::continue_watching(&conn, limit)
}

#[tauri::command]
fn set_progress(
    state: State<'_, AppState>,
    owner_id: String,
    owner_kind: String,
    position: f64,
    duration: Option<f64>,
) -> Result<Option<Progress>, String> {
    let conn = state.conn()?;
    library::set_progress(&conn, &owner_id, &owner_kind, position, duration)
}

#[tauri::command]
fn mark_watched(
    state: State<'_, AppState>,
    owner_id: String,
    owner_kind: String,
    completed: bool,
) -> Result<(), String> {
    let conn = state.conn()?;
    library::mark_watched(&conn, &owner_id, &owner_kind, completed)
}

#[tauri::command]
fn next_episode(state: State<'_, AppState>, series_id: String) -> Result<Option<EpisodeRef>, String> {
    let conn = state.conn()?;
    library::next_episode(&conn, &series_id)
}

#[tauri::command]
fn toggle_favorite(state: State<'_, AppState>, owner_id: String) -> Result<bool, String> {
    let conn = state.conn()?;
    library::toggle_favorite(&conn, &owner_id)
}

#[tauri::command]
fn favorites(state: State<'_, AppState>) -> Result<Vec<CatalogItem>, String> {
    let conn = state.conn()?;
    let ids = library::favorites(&conn)?;
    catalog::by_ids(&conn, &ids)
}

#[tauri::command]
fn get_setting(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    let conn = state.conn()?;
    settings::get(&conn, &key)
}

#[tauri::command]
fn set_setting(state: State<'_, AppState>, key: String, value: String) -> Result<(), String> {
    let conn = state.conn()?;
    settings::set(&conn, &key, &value)
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
        (meta::cached(&conn, &cache_id), settings::tmdb_key(&conn))
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
