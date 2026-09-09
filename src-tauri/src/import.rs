//! Ingestão de listas M3U no banco.
//!
//! Uma transação por import: ou a lista entra inteira, ou nada muda. Os `INSERT`
//! usam `ON CONFLICT` porque a mesma obra aparece em listas diferentes — é assim
//! que o dedup sai de graça.

use std::collections::HashSet;
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

const SOURCE_COLUMNS: &str = "id, url, label, kind, enabled, added_at, last_sync_at, item_count";

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
pub fn upsert_source(conn: &Connection, url: &str, label: &str, kind: &str) -> Result<Source, String> {
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
        .query_row("SELECT id FROM sources WHERE url = ?1", [url.trim()], |row| row.get(0))
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

    let mut movie_count = 0usize;
    let mut series_ids: HashSet<String> = HashSet::new();
    let mut channels = 0usize;
    let mut failure: Option<String> = None;

    let outcome = {
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
                    EntryKind::Movie => movie_count += 1,
                    EntryKind::Series => {
                        series_ids.insert(item_id);
                    }
                    EntryKind::Channel => channels += 1,
                },
                Err(error) => failure = Some(stringify(error)),
            }
        });

        drop(insert_item);
        drop(insert_episode);
        drop(insert_stream);

        outcome
    };

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

    // Streams de episódio contam pela série dona, não por episódio: o que
    // importa aqui é quantos títulos distintos a lista trouxe.
    let distinct: i64 = transaction
        .query_row(
            "SELECT count(DISTINCT COALESCE(e.series_id, s.owner_id))
             FROM streams s
             LEFT JOIN episodes e ON s.owner_kind = 'episode' AND e.id = s.owner_id
             WHERE s.source_id = ?1",
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
    Ok(ImportReport {
        source,
        parsed: outcome.parsed,
        discarded: outcome.discarded,
        movies: movie_count,
        series: series_ids.len(),
        channels,
    })
}

fn stringify(error: impl std::fmt::Display) -> String {
    format!("banco recusou a operação: {error}")
}
