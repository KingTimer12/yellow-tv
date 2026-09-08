//! Leitura do catálogo local: canais de TV, filmes e séries.
//!
//! Os arquivos são os mesmos que os scripts em `scripts/` geram. Cada um é lido e
//! desserializado uma única vez por execução — `filmes.json` tem ~5 MB e o parse
//! não deve repetir a cada busca.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

pub const PAGE_SIZE: usize = 60;

/// Um canal de TV, exatamente como está em `lista_pro.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub logo: String,
    #[serde(default)]
    pub category: String,
    #[serde(rename = "channelNumber")]
    pub channel_number: i64,
}

/// Filmes e séries compartilham a estrutura: os campos exclusivos são opcionais,
/// então a mesma struct serve para ler os dois arquivos e devolvê-los ao front.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub logo: String,
    #[serde(default)]
    pub group: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seasons: Vec<i64>,
    #[serde(
        default,
        rename = "episodeCount",
        skip_serializing_if = "Option::is_none"
    )]
    pub episode_count: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Episode {
    pub season: i64,
    pub episode: i64,
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeriesEpisodes {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub logo: String,
    #[serde(default)]
    pub group: String,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Serialize)]
pub struct GroupCount {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct CatalogPage {
    pub total: usize,
    pub page: usize,
    #[serde(rename = "pageSize")]
    pub page_size: usize,
    pub items: Vec<CatalogItem>,
    pub groups: Vec<GroupCount>,
}

#[derive(Debug, Serialize)]
pub struct ItemWithRelated {
    pub item: CatalogItem,
    pub related: Vec<CatalogItem>,
}

#[derive(Debug, Serialize)]
pub struct DataStatus {
    pub dir: String,
    pub channels: bool,
    pub filmes: bool,
    pub series: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Filmes,
    Series,
}

impl Kind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "filmes" => Ok(Kind::Filmes),
            "series" => Ok(Kind::Series),
            other => Err(format!("tipo de catálogo inválido: {other}")),
        }
    }

    fn file_name(self) -> &'static str {
        match self {
            Kind::Filmes => "filmes.json",
            Kind::Series => "series.json",
        }
    }
}

/// Minúsculas e sem acento, para que "cacador" encontre "Caçador".
pub fn fold(value: &str) -> String {
    value
        .to_lowercase()
        .nfd()
        .filter(|character| !matches!(*character, '\u{0300}'..='\u{036f}'))
        .collect()
}

pub struct Catalog {
    root: PathBuf,
    channels: Mutex<Option<Arc<Vec<Channel>>>>,
    items: Mutex<HashMap<Kind, Arc<Vec<CatalogItem>>>>,
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("não foi possível ler {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{} tem formato inesperado: {error}", path.display()))
}

impl Catalog {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            channels: Mutex::new(None),
            items: Mutex::new(HashMap::new()),
        }
    }

    pub fn status(&self) -> DataStatus {
        DataStatus {
            dir: self.root.display().to_string(),
            channels: self.root.join("lista_pro.json").is_file(),
            filmes: self.root.join("vod").join("filmes.json").is_file(),
            series: self.root.join("vod").join("series.json").is_file(),
        }
    }

    pub fn channels(&self) -> Result<Arc<Vec<Channel>>, String> {
        let mut cached = self.channels.lock().unwrap();
        if let Some(channels) = cached.as_ref() {
            return Ok(Arc::clone(channels));
        }
        let mut channels: Vec<Channel> = read_json(&self.root.join("lista_pro.json"))?;
        channels.sort_by_key(|channel| channel.channel_number);
        let shared = Arc::new(channels);
        *cached = Some(Arc::clone(&shared));
        Ok(shared)
    }

    fn items(&self, kind: Kind) -> Result<Arc<Vec<CatalogItem>>, String> {
        let mut cached = self.items.lock().unwrap();
        if let Some(items) = cached.get(&kind) {
            return Ok(Arc::clone(items));
        }
        let items: Vec<CatalogItem> = read_json(&self.root.join("vod").join(kind.file_name()))?;
        let shared = Arc::new(items);
        cached.insert(kind, Arc::clone(&shared));
        Ok(shared)
    }

    pub fn page(
        &self,
        kind: Kind,
        query: &str,
        group: Option<&str>,
        page: usize,
    ) -> Result<CatalogPage, String> {
        let items = self.items(kind)?;
        let terms: Vec<String> = fold(query)
            .split_whitespace()
            .map(str::to_owned)
            .collect();

        let filtered: Vec<&CatalogItem> = items
            .iter()
            .filter(|item| {
                if let Some(group) = group {
                    if item.group != group {
                        return false;
                    }
                }
                if terms.is_empty() {
                    return true;
                }
                let haystack = fold(&item.title);
                terms.iter().all(|term| haystack.contains(term))
            })
            .collect();

        let mut tally: HashMap<&str, usize> = HashMap::new();
        for item in items.iter() {
            *tally.entry(item.group.as_str()).or_insert(0) += 1;
        }
        let mut groups: Vec<GroupCount> = tally
            .into_iter()
            .map(|(name, count)| GroupCount {
                name: name.to_owned(),
                count,
            })
            .collect();
        groups.sort_by_key(|group| std::cmp::Reverse(group.count));

        Ok(CatalogPage {
            total: filtered.len(),
            page,
            page_size: PAGE_SIZE,
            items: filtered
                .into_iter()
                .skip(page * PAGE_SIZE)
                .take(PAGE_SIZE)
                .cloned()
                .collect(),
            groups,
        })
    }

    pub fn item(&self, kind: Kind, id: &str) -> Result<ItemWithRelated, String> {
        let items = self.items(kind)?;
        let item = items
            .iter()
            .find(|candidate| candidate.id == id)
            .ok_or_else(|| format!("título não encontrado: {id}"))?;

        let related = items
            .iter()
            .filter(|other| other.group == item.group && other.id != item.id)
            .take(18)
            .cloned()
            .collect();

        Ok(ItemWithRelated {
            item: item.clone(),
            related,
        })
    }

    /// Os episódios ficam num arquivo por série, então só o pedido é lido do disco.
    pub fn episodes(&self, id: &str) -> Result<SeriesEpisodes, String> {
        if id.contains('/') || id.contains('\\') || id.contains("..") {
            return Err(format!("identificador inválido: {id}"));
        }
        read_json(&self.root.join("vod").join("series").join(format!("{id}.json")))
    }
}
