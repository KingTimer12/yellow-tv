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
