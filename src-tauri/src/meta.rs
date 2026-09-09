//! Sinopse, nota e elenco vindos do TMDB.
//!
//! O IMDb não tem API pública; o TMDB é a fonte aberta equivalente — e é onde os
//! pôsteres das listas já estão hospedados. Cada resposta é gravada em disco porque
//! a busca é por título e a chave gratuita tem limite de taxa.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db;
use crate::ids;

const IMAGE_BASE: &str = "https://image.tmdb.org/t/p";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CastMember {
    pub name: String,
    pub character: String,
    pub photo: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Meta {
    pub overview: String,
    pub tagline: String,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    pub rating: Option<f64>,
    pub votes: Option<i64>,
    pub year: Option<i64>,
    pub runtime: Option<i64>,
    pub genres: Vec<String>,
    pub cast: Vec<CastMember>,
    #[serde(rename = "imdbUrl")]
    pub imdb_url: Option<String>,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Meta {
    pub fn empty(reason: Option<String>) -> Self {
        Self {
            source: "none".into(),
            reason,
            ..Default::default()
        }
    }
}

fn image(path: Option<&str>, size: &str) -> Option<String> {
    path.filter(|value| !value.is_empty())
        .map(|value| format!("{IMAGE_BASE}/{size}{value}"))
}

/// Remove o ruído que as listas acrescentam: "[L]", "[XXX]", "(2019)", marcas de qualidade.
fn clean_title(title: &str) -> String {
    let mut cleaned = String::with_capacity(title.len());
    let mut depth = 0usize;
    for character in title.chars() {
        match character {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => cleaned.push(character),
            _ => {}
        }
    }
    cleaned
        .split_whitespace()
        .filter(|word| {
            !matches!(
                word.to_lowercase().as_str(),
                "4k" | "hd" | "fhd" | "sd" | "dublado" | "legendado" | "nacional"
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

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

    async fn get(&self, path: &str, key: &str, params: &[(&str, &str)]) -> Result<Value, String> {
        let mut request = self
            .client
            .get(format!("https://api.themoviedb.org/3{path}"))
            .query(&[("language", "pt-BR")])
            .query(params);

        // Um token v4 é um JWT e vai no cabeçalho; uma chave v3 vai na query.
        if key.starts_with("ey") {
            request = request.bearer_auth(key);
        } else {
            request = request.query(&[("api_key", key)]);
        }

        let response = request
            .send()
            .await
            .map_err(|error| format!("falha ao falar com o TMDB: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("TMDB respondeu {} em {path}", response.status()));
        }
        response
            .json()
            .await
            .map_err(|error| format!("resposta do TMDB ilegível: {error}"))
    }

    async fn lookup(&self, key: &str, title: &str, kind: &str, year: Option<i64>) -> Result<Meta, String> {
        let query = clean_title(title);
        let year_text = year.map(|value| value.to_string());
        let mut params: Vec<(&str, &str)> = vec![("query", &query), ("include_adult", "true")];
        if let Some(year) = year_text.as_deref() {
            params.push((
                if kind == "movie" {
                    "year"
                } else {
                    "first_air_date_year"
                },
                year,
            ));
        }

        let search = self.get(&format!("/search/{kind}"), key, &params).await?;
        let Some(hit) = search["results"].get(0) else {
            return Ok(Meta::empty(None));
        };
        let id = hit["id"].as_i64().unwrap_or_default();

        let details = self
            .get(
                &format!("/{kind}/{id}"),
                key,
                &[("append_to_response", "credits,external_ids")],
            )
            .await?;

        let date = details["release_date"]
            .as_str()
            .or_else(|| details["first_air_date"].as_str())
            .unwrap_or_default();

        let imdb_id = details["imdb_id"]
            .as_str()
            .or_else(|| details["external_ids"]["imdb_id"].as_str())
            .filter(|value| !value.is_empty());

        Ok(Meta {
            overview: details["overview"].as_str().unwrap_or_default().to_owned(),
            tagline: details["tagline"].as_str().unwrap_or_default().to_owned(),
            poster: image(details["poster_path"].as_str(), "w500"),
            backdrop: image(details["backdrop_path"].as_str(), "w1280"),
            rating: details["vote_average"].as_f64(),
            votes: details["vote_count"].as_i64(),
            year: date.get(0..4).and_then(|value| value.parse().ok()),
            runtime: details["runtime"]
                .as_i64()
                .or_else(|| details["episode_run_time"][0].as_i64()),
            genres: details["genres"]
                .as_array()
                .map(|genres| {
                    genres
                        .iter()
                        .filter_map(|genre| genre["name"].as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            cast: details["credits"]["cast"]
                .as_array()
                .map(|cast| {
                    cast.iter()
                        .take(12)
                        .map(|person| CastMember {
                            name: person["name"].as_str().unwrap_or_default().to_owned(),
                            character: person["character"].as_str().unwrap_or_default().to_owned(),
                            photo: image(person["profile_path"].as_str(), "w185"),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            imdb_url: imdb_id.map(|id| format!("https://www.imdb.com/title/{id}/")),
            source: "tmdb".into(),
            reason: None,
        })
    }

    /// Busca no TMDB. O cache e a chave ficam com o chamador porque `Connection`
    /// não cruza `await`.
    pub async fn fetch(&self, key: Option<String>, kind: &str, title: &str, year: Option<i64>) -> Meta {
        let kind = if kind == "series" { "tv" } else { "movie" };
        let Some(key) = key else {
            return Meta::empty(Some("chave do TMDB não configurada".into()));
        };
        match self.lookup(&key, title, kind, year).await {
            Ok(meta) => meta,
            Err(reason) => Meta::empty(Some(reason)),
        }
    }
}

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
