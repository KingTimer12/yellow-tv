//! Título normalizado e identificadores estáveis.
//!
//! O id de um item nasce de `(kind, title_norm, year)` e nunca da URL: é isso que
//! faz o mesmo filme de duas listas colapsar num item só e o progresso sobreviver
//! a um re-sync.

use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

/// Ruído que as listas acrescentam ao nome e que não distingue uma obra de outra.
const NOISE: &[&str] = &[
    "4k", "8k", "uhd", "fhd", "hd", "sd", "h265", "h264", "hevc", "x265", "x264", "l", "alt",
    "alternativo", "alternativa", "dublado", "dub", "legendado", "leg", "nacional", "multi",
    "1080p", "720p", "480p", "2160p",
];

/// Minúsculas, sem acento, sem parênteses/colchetes, sem tokens de qualidade,
/// espaços colapsados. Alimenta `items.title_norm` e, por consequência, o hash.
pub fn normalize(title: &str) -> String {
    let mut plain = String::with_capacity(title.len());
    let mut depth = 0usize;
    for character in title.chars() {
        match character {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => plain.push(character),
            _ => {}
        }
    }

    let folded: String = plain
        .to_lowercase()
        .nfd()
        .filter(|character| !matches!(*character, '\u{0300}'..='\u{036f}'))
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect();

    folded
        .split_whitespace()
        .filter(|token| !NOISE.contains(token))
        // Um "2" sozinho depois de "alt" é lixo; "Duna 2" perde o número apenas
        // quando ele é o resto de um token de qualidade — por isso o filtro só
        // remove dígitos isolados no fim da cadeia.
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .skip_while(|token| token.len() <= 2 && token.chars().all(|c| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned()
}

fn digest(input: &str) -> String {
    hex::encode(&Sha256::digest(input.as_bytes())[..16])
}

pub fn item_id(kind: &str, title_norm: &str, year: Option<i64>) -> String {
    digest(&format!("{kind}\u{1f}{title_norm}\u{1f}{}", year.unwrap_or(0)))
}

pub fn episode_id(series_id: &str, season: i64, episode: i64) -> String {
    digest(&format!("{series_id}\u{1f}{season}\u{1f}{episode}"))
}
