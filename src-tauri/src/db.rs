//! Abertura e migração do banco local.
//!
//! Uma única conexão vive no `AppState` atrás de um `Mutex`. SQLite local é
//! rápido o bastante para atender a WebView de forma síncrona, e a versão do
//! schema fica em `PRAGMA user_version` — nada de tabela de controle própria.

use std::path::Path;

use rusqlite::Connection;

pub const SCHEMA_VERSION: i64 = 2;

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

/// v2: rótulo de variante do stream ("Dublado", "Legendado"). Vem do nome da
/// linha na lista, que `ids::normalize` descarta ao montar o título — sem
/// guardar aqui, duas versões do mesmo episódio ficam indistinguíveis.
const MIGRATION_V2: &str = r#"
ALTER TABLE streams ADD COLUMN variant TEXT;
PRAGMA user_version = 2;
"#;

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

    if current < 1 {
        conn.execute_batch(MIGRATION_V1)
            .map_err(|error| format!("migração v1 falhou: {error}"))?;
    }
    if current < 2 {
        conn.execute_batch(MIGRATION_V2)
            .map_err(|error| format!("migração v2 falhou: {error}"))?;
    }
    Ok(())
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
