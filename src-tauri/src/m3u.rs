//! Parser de listas M3U/M3U8 em streaming.
//!
//! A lista chega com 100 mil linhas ou mais: o parser nunca guarda o texto
//! inteiro, entrega uma `Entry` por vez ao chamador e conta o que descartou. Uma
//! linha ruim não derruba o import.

use std::io::BufRead;

use unicode_normalization::UnicodeNormalization;

use crate::ids;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Movie,
    Series,
    Channel,
}

impl EntryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntryKind::Movie => "movie",
            EntryKind::Series => "series",
            EntryKind::Channel => "channel",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub kind: EntryKind,
    pub title: String,
    pub title_norm: String,
    pub year: Option<i64>,
    pub season: Option<i64>,
    pub episode: Option<i64>,
    pub episode_title: Option<String>,
    pub logo: Option<String>,
    pub group_name: Option<String>,
    pub quality: Option<String>,
    pub channel_number: Option<i64>,
    /// "Dublado" ou "Legendado", quando a lista marca. `ids::normalize` joga
    /// essas marcas fora ao montar o título — de propósito, para que as duas
    /// versões caiam no mesmo item — então o rótulo tem que ser guardado aqui
    /// ou não há como distinguir os dois streams depois.
    pub variant: Option<String>,
    pub url: String,
}

impl Entry {
    /// Id do título ao qual esta linha pertence — para série, o id da série.
    pub fn item_id(&self) -> String {
        ids::item_id(self.kind.as_str(), &self.title_norm, self.year)
    }

    /// Id do episódio, quando a linha é de série com temporada e episódio.
    pub fn episode_id(&self) -> Option<String> {
        match (self.season, self.episode) {
            (Some(season), Some(episode)) => {
                Some(ids::episode_id(&self.item_id(), season, episode))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub parsed: usize,
    pub discarded: usize,
}

/// Marcas de versão legendada. "l" entra porque "[L]" é a convenção dominante
/// nas listas em português.
const LEGENDADO_TOKENS: &[&str] = &[
    "l", "leg", "legendado", "legendada", "legendados", "legendadas", "sub", "subbed",
];
/// Marcas de versão dublada. Sem "n" solto: letra única gera falso positivo
/// demais em título.
const DUBLADO_TOKENS: &[&str] = &["dub", "dublado", "dublada", "dubladas", "dublados"];

const QUALITY_TOKENS: &[&str] = &[
    "4K", "8K", "UHD", "FHD", "HD", "SD", "H265", "H264", "HEVC", "1080P", "720P", "2160P",
];

/// `tvg-logo="x" group-title="y",Nome` → atributos e nome.
fn split_extinf(line: &str) -> Option<(Vec<(String, String)>, String)> {
    let body = line.strip_prefix("#EXTINF:")?;
    // A vírgula que separa atributos do nome é a última fora de aspas.
    let mut in_quotes = false;
    let mut split_at = None;
    for (index, character) in body.char_indices() {
        match character {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => split_at = Some(index),
            _ => {}
        }
    }
    let split_at = split_at?;
    let (head, name) = (&body[..split_at], body[split_at + 1..].trim());
    if name.is_empty() {
        return None;
    }

    let mut attrs = Vec::new();
    let bytes: Vec<char> = head.chars().collect();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        // key=
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor] != '=' {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            break;
        }
        let key: String = bytes[start..cursor].iter().collect();
        cursor += 1; // pula o '='
        let value: String = if bytes.get(cursor) == Some(&'"') {
            cursor += 1;
            let value_start = cursor;
            while cursor < bytes.len() && bytes[cursor] != '"' {
                cursor += 1;
            }
            let value = bytes[value_start..cursor].iter().collect();
            cursor += 1; // pula o '"' de fechamento
            value
        } else {
            let value_start = cursor;
            while cursor < bytes.len() && !bytes[cursor].is_whitespace() {
                cursor += 1;
            }
            bytes[value_start..cursor].iter().collect()
        };
        let key = key
            .trim_start_matches(|c: char| c == '-' || c.is_numeric())
            .trim()
            .to_lowercase();
        if !key.is_empty() {
            attrs.push((key, value));
        }
        while cursor < bytes.len() && bytes[cursor].is_whitespace() {
            cursor += 1;
        }
    }

    Some((attrs, name.to_owned()))
}

fn attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.is_empty())
}

/// Minúsculas, sem acento, tudo que não é alfanumérico vira espaço. Colchetes
/// e parênteses viram separador, então "[L]" sai como o token "l".
fn fold(value: &str) -> String {
    value
        .to_lowercase()
        .nfd()
        .filter(|character| !matches!(*character, '\u{0300}'..='\u{036f}'))
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect()
}

/// Versão do áudio, quando a lista marca.
///
/// O nome manda. O grupo só é consultado para o sinal de legendado ("Series |
/// Legendadas"): no sentido oposto ele mente, porque "Filmes | Nacionais" quer
/// dizer produção brasileira, não áudio dublado.
fn detect_variant(name: &str, group: Option<&str>) -> Option<String> {
    for token in fold(name).split_whitespace() {
        if LEGENDADO_TOKENS.contains(&token) {
            return Some("Legendado".to_owned());
        }
        if DUBLADO_TOKENS.contains(&token) {
            return Some("Dublado".to_owned());
        }
    }
    let group = group?;
    fold(group)
        .split_whitespace()
        .any(|token| LEGENDADO_TOKENS.contains(&token))
        .then(|| "Legendado".to_owned())
}

/// Marca de episódio no nome. Devolve `(temporada, episódio, título da série)`.
fn series_marks(name: &str) -> Option<(i64, i64, String)> {
    let chars: Vec<char> = name.chars().collect();
    let lower: Vec<char> = name.to_lowercase().chars().collect();

    // Padrão SxxEyy, com ou sem separador: "S01E01", "S01 E01", "S01-E01".
    // O separador é obrigatório de aceitar: listas inteiras usam só a forma com
    // espaço, e sem isso todo episódio vira um filme solto.
    for index in 0..chars.len() {
        if lower[index] != 's' {
            continue;
        }
        // O 's' precisa abrir um token, senão "Os 8 Escolhidos" viraria episódio.
        if index > 0 && chars[index - 1].is_alphanumeric() {
            continue;
        }
        let mut cursor = index + 1;
        let season_start = cursor;
        while cursor < chars.len() && chars[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor == season_start {
            continue;
        }
        // `continue`, não `?`: um número absurdo numa linha não pode abortar a
        // busca e fazer a linha inteira ser classificada errado.
        let Ok(season) = chars[season_start..cursor]
            .iter()
            .collect::<String>()
            .parse::<i64>()
        else {
            continue;
        };
        let mut separator = cursor;
        while separator < chars.len() && matches!(chars[separator], ' ' | '-' | '.' | '_') {
            separator += 1;
        }
        if separator >= chars.len() || lower[separator] != 'e' {
            continue;
        }
        let mut cursor = separator + 1;
        let episode_start = cursor;
        while cursor < chars.len() && chars[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor == episode_start {
            continue;
        }
        let Ok(episode) = chars[episode_start..cursor]
            .iter()
            .collect::<String>()
            .parse::<i64>()
        else {
            continue;
        };
        let head: String = chars[..index].iter().collect();
        return Some((season, episode, head));
    }

    // Padrão NxM
    for index in 0..chars.len() {
        if lower[index] != 'x' || index == 0 {
            continue;
        }
        let mut before = index;
        while before > 0 && chars[before - 1].is_ascii_digit() {
            before -= 1;
        }
        if before == index {
            continue;
        }
        let mut after = index + 1;
        let episode_start = after;
        while after < chars.len() && chars[after].is_ascii_digit() {
            after += 1;
        }
        if after == episode_start {
            continue;
        }
        let Ok(season) = chars[before..index].iter().collect::<String>().parse::<i64>() else {
            continue;
        };
        let Ok(episode) = chars[episode_start..after].iter().collect::<String>().parse::<i64>()
        else {
            continue;
        };
        let head: String = chars[..before].iter().collect();
        return Some((season, episode, head));
    }

    // "Temporada N Episodio M", com ou sem acento. Usa uma dobra própria (sem
    // `ids::normalize`) porque aquela função descarta dígitos soltos no fim —
    // exatamente o número de episódio que este padrão precisa preservar.
    let folded: String = name
        .to_lowercase()
        .nfd()
        .filter(|character| !matches!(*character, '\u{0300}'..='\u{036f}'))
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect();
    let tokens: Vec<&str> = folded.split_whitespace().collect();
    // Tokens originais (mesma contagem, preservando maiúsculas) para o título.
    let raw_tokens: Vec<&str> = name.split_whitespace().collect();
    let mut season = None;
    let mut episode = None;
    let mut cut = tokens.len();
    for (index, token) in tokens.iter().enumerate() {
        let next = tokens.get(index + 1).and_then(|value| value.parse::<i64>().ok());
        if *token == "temporada" && next.is_some() {
            season = next;
            cut = cut.min(index);
        }
        if (*token == "episodio" || *token == "ep") && next.is_some() {
            episode = next;
            cut = cut.min(index);
        }
    }
    match (season, episode) {
        (Some(season), Some(episode)) if tokens.len() == raw_tokens.len() => {
            let head = raw_tokens[..cut].join(" ");
            Some((season, episode, head))
        }
        _ => None,
    }
}

/// Ano em `(2019)` ou como último token do nome.
fn extract_year(name: &str) -> (Option<i64>, String) {
    let trimmed = name.trim();
    if let Some(open) = trimmed.rfind('(') {
        let inside: String = trimmed[open + 1..].trim_end_matches(')').trim().to_owned();
        if let Ok(year) = inside.parse::<i64>() {
            if (1900..=2100).contains(&year) {
                return (Some(year), trimmed[..open].trim().to_owned());
            }
        }
    }
    let mut tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if let Some(last) = tokens.last() {
        if let Ok(year) = last.parse::<i64>() {
            if (1900..=2100).contains(&year) {
                tokens.pop();
                return (Some(year), tokens.join(" "));
            }
        }
    }
    (None, trimmed.to_owned())
}

/// Tokens de qualidade encontrados, e o nome sem eles.
fn strip_quality(name: &str) -> (Option<String>, String) {
    let mut found: Vec<String> = Vec::new();
    let kept: Vec<&str> = name
        .split_whitespace()
        .filter(|token| {
            let bare = token.trim_matches(|c: char| !c.is_alphanumeric()).to_uppercase();
            if QUALITY_TOKENS.contains(&bare.as_str()) {
                found.push(bare);
                false
            } else {
                true
            }
        })
        .collect();
    let quality = if found.is_empty() {
        None
    } else {
        found.dedup();
        Some(found.join(" "))
    };
    (quality, kept.join(" "))
}

fn looks_like_channel(attrs: &[(String, String)], group: Option<&str>) -> bool {
    if attr(attrs, "tvg-id").is_some() || attr(attrs, "tvg-chno").is_some() {
        return true;
    }
    let Some(group) = group else { return false };
    let folded = ids::normalize(group);
    // Token inteiro, não substring: "Series | DirecTV" e "Series | PlutoTV"
    // contêm "tv" no meio de uma palavra e viravam canal.
    if folded
        .split_whitespace()
        .any(|token| matches!(token, "canais" | "canal" | "tv" | "live" | "abertos"))
    {
        return true;
    }
    folded.contains("ao vivo")
}

fn build(attrs: Vec<(String, String)>, name: String, url: String) -> Entry {
    let logo = attr(&attrs, "tvg-logo").map(str::to_owned);
    let group_name = attr(&attrs, "group-title").map(str::to_owned);
    let channel_number = attr(&attrs, "tvg-chno").and_then(|value| value.parse().ok());

    // Tokens de qualidade podem aparecer antes ou depois da marca de episódio,
    // então são removidos do nome inteiro antes de procurar a marca.
    let (quality, clean) = strip_quality(&name);
    let variant = detect_variant(&name, group_name.as_deref());

    // Regra 1: marca de episódio manda, mesmo em grupo de canal.
    if let Some((season, episode, head)) = series_marks(&clean) {
        let (_, without_year) = extract_year(&head);
        let title = without_year.trim().to_owned();
        return Entry {
            kind: EntryKind::Series,
            title_norm: ids::normalize(&title),
            title,
            year: None,
            season: Some(season),
            episode: Some(episode),
            episode_title: None,
            logo,
            group_name,
            quality,
            channel_number: None,
            variant,
            url,
        };
    }

    // Regra 2: atributos de EPG ou grupo de canal.
    if looks_like_channel(&attrs, group_name.as_deref()) {
        let title = clean.trim().to_owned();
        return Entry {
            kind: EntryKind::Channel,
            title_norm: ids::normalize(&title),
            title,
            year: None,
            season: None,
            episode: None,
            episode_title: None,
            logo,
            group_name,
            quality,
            channel_number,
            variant,
            url,
        };
    }

    // Regra 3: filme.
    let (year, title) = extract_year(&clean);
    let title = title.trim().to_owned();
    Entry {
        kind: EntryKind::Movie,
        title_norm: ids::normalize(&title),
        title,
        year,
        season: None,
        episode: None,
        episode_title: None,
        logo,
        group_name,
        quality,
        channel_number: None,
        variant,
        url,
    }
}

/// Estado do `#EXTINF` mais recente, ainda esperando a URL da linha seguinte.
enum Pending {
    /// Nenhum `#EXTINF` em aberto.
    None,
    /// `#EXTINF` bem formado, esperando URL.
    Ready(Vec<(String, String)>, String),
    /// `#EXTINF` malformado (sem nome) — já contado como descarte; a próxima
    /// linha de conteúdo pertence a ele e é só ignorada, sem descarte extra.
    Broken,
}

pub fn parse_each<R: BufRead>(reader: R, mut sink: impl FnMut(Entry)) -> Outcome {
    let mut outcome = Outcome::default();
    let mut pending = Pending::None;

    for line in reader.lines() {
        let Ok(line) = line else {
            outcome.discarded += 1;
            continue;
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("#EXTINF:") {
            if matches!(pending, Pending::Ready(..)) {
                // O #EXTINF anterior ficou sem URL.
                outcome.discarded += 1;
            }
            pending = match split_extinf(line) {
                Some((attrs, name)) => Pending::Ready(attrs, name),
                None => {
                    outcome.discarded += 1;
                    Pending::Broken
                }
            };
            continue;
        }

        if line.starts_with('#') {
            continue; // #EXTM3U, #EXTVLCOPT, #EXTGRP e afins
        }

        match std::mem::replace(&mut pending, Pending::None) {
            Pending::Ready(attrs, name) => {
                sink(build(attrs, name, line.to_owned()));
                outcome.parsed += 1;
            }
            Pending::Broken => {} // já descartado quando o #EXTINF quebrou
            Pending::None => outcome.discarded += 1, // URL órfã
        }
    }

    if matches!(pending, Pending::Ready(..)) {
        outcome.discarded += 1;
    }
    outcome
}

/// Conveniência para testes e listas pequenas.
pub fn parse_all<R: BufRead>(reader: R) -> (Vec<Entry>, Outcome) {
    let mut entries = Vec::new();
    let outcome = parse_each(reader, |entry| entries.push(entry));
    (entries, outcome)
}
