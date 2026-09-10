//! Conversão de legenda SRT para WebVTT.
//!
//! O `<track>` do HTML só aceita WebVTT, e legenda baixada por aí é quase
//! sempre `.srt`. A diferença entre os dois formatos é pequena — cabeçalho,
//! vírgula do milissegundo e escape de `&`, `<`, `>` — mas é o suficiente para
//! o navegador recusar o arquivo inteiro em silêncio.

/// Converte SRT em WebVTT. Nunca falha: uma legenda torta rende uma legenda
/// incompleta, o que é melhor do que nenhuma no meio de um filme.
pub fn srt_to_vtt(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 16);
    out.push_str("WEBVTT\n\n");

    for line in input.lines() {
        let line = line.trim_end_matches('\r');
        // O índice numérico do SRT não existe em WebVTT; deixar passar não
        // quebra nada, mas vira um "cue identifier" inútil.
        if is_index(line) {
            continue;
        }
        if let Some(converted) = convert_timing(line) {
            out.push_str(&converted);
        } else {
            out.push_str(&escape(line));
        }
        out.push('\n');
    }
    out
}

fn is_index(line: &str) -> bool {
    !line.is_empty() && line.chars().all(|character| character.is_ascii_digit())
}

/// "00:00:01,000 --> 00:00:04,000" → "00:00:01.000 --> 00:00:04.000".
/// Também aceita o carimbo curto "00:01,000", que o WebVTT permite.
fn convert_timing(line: &str) -> Option<String> {
    let (start, rest) = line.split_once("-->")?;
    let start = normalize_stamp(start.trim())?;
    // Depois do fim pode vir posicionamento ("X1:.. Y1:.."), que o WebVTT
    // ignora com sintaxe própria — descartar é mais seguro que traduzir errado.
    let end = rest.trim().split_whitespace().next()?;
    let end = normalize_stamp(end)?;
    Some(format!("{start} --> {end}"))
}

fn normalize_stamp(stamp: &str) -> Option<String> {
    let (clock, millis) = stamp.rsplit_once(',').or_else(|| stamp.rsplit_once('.'))?;
    if millis.len() != 3 || !millis.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let parts: Vec<&str> = clock.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    if !parts.iter().all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    Some(format!("{clock}.{millis}"))
}

/// WebVTT trata `&`, `<` e `>` como marcação; sem escapar, uma fala com "<"
/// engole o resto da legenda.
fn escape(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for character in line.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}

/// Decodifica o arquivo de legenda em texto.
///
/// Legenda em português baixada por aí vem em UTF-8 ou em Latin-1, e as duas
/// convivem. Ler tudo como UTF-8 transforma um arquivo Latin-1 inteiro em erro;
/// ler tudo como Latin-1 estraga os acentos de um UTF-8. Tenta UTF-8 primeiro,
/// porque um texto Latin-1 com acento quase nunca é UTF-8 válido.
pub fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        // ISO-8859-1 é o mapeamento direto byte → code point.
        Err(_) => bytes.iter().map(|byte| *byte as char).collect(),
    }
}

/// Lê um arquivo de legenda do disco e devolve WebVTT pronto para um `<track>`.
pub fn load(path: &std::path::Path) -> Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("não foi possível ler a legenda: {error}"))?;
    let text = decode(&bytes);
    let already_vtt = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("vtt"));
    Ok(if already_vtt { text } else { srt_to_vtt(&text) })
}
