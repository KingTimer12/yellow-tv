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
/// Canais são consumidos como lista única (a barra lateral do player precisa de
/// todos), e cada página repetia o `GROUP BY group_name` inteiro. Uma página só
/// cobre qualquer lista real e o agregado roda uma vez.
pub const CHANNEL_PAGE_SIZE: i64 = 10_000;
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
    /// Segundos salvos: é daqui que o front tira o `startAt` do episódio.
    pub position_secs: f64,
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

/// Tamanho de página por tipo: ver `CHANNEL_PAGE_SIZE`.
pub fn page_size(kind: Kind) -> i64 {
    match kind {
        Kind::Channel => CHANNEL_PAGE_SIZE,
        _ => PAGE_SIZE,
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
    let page_size = page_size(kind);
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
            params_from_iter(binds.iter().map(|value| &**value)),
            |row| row.get(0),
        )
        .map_err(stringify)?;

    let mut statement = conn
        .prepare(&format!(
            "SELECT {ITEM_COLUMNS} {ITEM_JOIN} WHERE {where_clause}
             ORDER BY i.title LIMIT {page_size} OFFSET {}",
            page * page_size
        ))
        .map_err(stringify)?;
    let items = statement
        .query_map(
            params_from_iter(binds.iter().map(|value| &**value)),
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
        page_size,
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
            let rows = statement
                .query_map(rusqlite::params![kind.as_str(), group, id], read_item)
                .map_err(stringify)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(stringify)?;
            rows
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
                position,
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
    for (id, season, episode, episode_title, position_secs, percent, completed) in partial {
        episodes.push(EpisodeRow {
            streams: streams_for(conn, &id, "episode")?,
            id,
            season,
            episode,
            title: episode_title,
            position_secs,
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
