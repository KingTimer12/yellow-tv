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
