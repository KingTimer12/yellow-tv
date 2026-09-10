use yellow_tv_lib::subtitles::{decode, srt_to_vtt};

#[test]
fn comeca_com_o_cabecalho_webvtt() {
    assert!(srt_to_vtt("").starts_with("WEBVTT\n\n"));
}

#[test]
fn converte_a_virgula_do_milissegundo() {
    let vtt = srt_to_vtt("1\n00:00:01,500 --> 00:00:04,000\nOlá\n");
    assert!(vtt.contains("00:00:01.500 --> 00:00:04.000"), "{vtt}");
    assert!(vtt.contains("Olá"));
}

// O índice numérico do SRT vira "cue identifier" inútil no WebVTT.
#[test]
fn descarta_o_indice_numerico() {
    let vtt = srt_to_vtt("42\n00:00:01,000 --> 00:00:02,000\nfala\n");
    assert!(!vtt.contains("42"), "{vtt}");
}

// Sem escape, uma fala com "<" engole o resto da legenda.
#[test]
fn escapa_marcacao_no_texto() {
    let vtt = srt_to_vtt("1\n00:00:01,000 --> 00:00:02,000\n<risos> & cia\n");
    assert!(vtt.contains("&lt;risos&gt; &amp; cia"), "{vtt}");
}

#[test]
fn descarta_posicionamento_depois_do_fim() {
    let vtt = srt_to_vtt("1\n00:00:01,000 --> 00:00:02,000 X1:100 Y1:200\nfala\n");
    assert!(vtt.contains("00:00:01.000 --> 00:00:02.000\n"), "{vtt}");
    assert!(!vtt.contains("X1"), "{vtt}");
}

#[test]
fn linha_torta_nao_derruba_o_resto() {
    let vtt = srt_to_vtt("1\nlixo --> aqui\nfala\n\n2\n00:00:05,000 --> 00:00:06,000\nfala 2\n");
    assert!(vtt.contains("00:00:05.000 --> 00:00:06.000"), "{vtt}");
    assert!(vtt.contains("fala 2"));
}

#[test]
fn decodifica_utf8_e_latin1() {
    assert_eq!(decode("Ação".as_bytes()), "Ação");
    // "Ação" em Latin-1: o 0xE7 é o ç e o 0xE3 é o ã.
    assert_eq!(decode(&[b'A', 0xE7, 0xE3, b'o']), "Ação");
}

#[test]
fn remove_o_bom() {
    assert_eq!(decode(&[0xEF, 0xBB, 0xBF, b'o', b'i']), "oi");
}
