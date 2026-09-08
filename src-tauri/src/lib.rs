mod catalog;
mod meta;
mod proxy;

use std::path::PathBuf;

use catalog::{Catalog, CatalogPage, Channel, DataStatus, ItemWithRelated, Kind, SeriesEpisodes};
use meta::{Meta, Tmdb};
use tauri::{Manager, State};

struct AppState {
    catalog: Catalog,
    tmdb: Tmdb,
    proxy_port: u16,
}

/// Onde as listas geradas por `scripts/` são procuradas, em ordem: a variável de
/// ambiente, a pasta de dados do app e `data/` na raiz do projeto (o `tauri dev`
/// roda com o diretório atual em `src-tauri`, daí o `../data`).
fn resolve_data_dir(app_data: PathBuf) -> PathBuf {
    if let Some(configured) = std::env::var_os("YELLOWTV_DATA_DIR") {
        return PathBuf::from(configured);
    }

    let candidates = [
        app_data.join("data"),
        PathBuf::from("data"),
        PathBuf::from("../data"),
    ];

    for candidate in &candidates {
        if candidate.join("lista_pro.json").is_file() {
            return candidate.clone();
        }
    }

    app_data.join("data")
}

#[tauri::command]
fn data_status(state: State<'_, AppState>) -> DataStatus {
    state.catalog.status()
}

#[tauri::command]
fn channels(state: State<'_, AppState>) -> Result<Vec<Channel>, String> {
    state.catalog.channels().map(|channels| (*channels).clone())
}

#[tauri::command]
fn catalog_page(
    state: State<'_, AppState>,
    kind: String,
    query: String,
    group: Option<String>,
    page: usize,
) -> Result<CatalogPage, String> {
    state.catalog.page(
        Kind::parse(&kind)?,
        &query,
        group.as_deref().filter(|value| !value.is_empty()),
        page,
    )
}

#[tauri::command]
fn catalog_item(
    state: State<'_, AppState>,
    kind: String,
    id: String,
) -> Result<ItemWithRelated, String> {
    state.catalog.item(Kind::parse(&kind)?, &id)
}

#[tauri::command]
fn series_episodes(state: State<'_, AppState>, id: String) -> Result<SeriesEpisodes, String> {
    state.catalog.episodes(&id)
}

#[tauri::command]
async fn title_meta(
    state: State<'_, AppState>,
    kind: String,
    title: String,
    year: Option<i64>,
) -> Result<Meta, String> {
    Ok(state.tmdb.meta(&kind, &title, year).await)
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
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let data_dir = resolve_data_dir(app_data.clone());
            let proxy_port = proxy::start()?;

            println!("YellowTV: dados em {}", data_dir.display());
            println!("YellowTV: proxy de stream em 127.0.0.1:{proxy_port}");

            app.manage(AppState {
                catalog: Catalog::new(data_dir),
                tmdb: Tmdb::new(app_data.join("tmdb-cache")),
                proxy_port,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            data_status,
            channels,
            catalog_page,
            catalog_item,
            series_episodes,
            title_meta,
            stream_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
