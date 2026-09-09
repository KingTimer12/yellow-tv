use std::io::Cursor;
use yellow_tv_lib::m3u::{self, EntryKind};

fn parse(text: &str) -> (Vec<m3u::Entry>, m3u::Outcome) {
    m3u::parse_all(Cursor::new(text.to_owned()))
}

#[test]
fn filme_com_ano_no_titulo() {
    let (entries, outcome) = parse(
        "#EXTM3U\n#EXTINF:-1 tvg-logo=\"http://p/duna.jpg\" group-title=\"Filmes | Ficção\",Duna (2021)\nhttp://host/duna.mp4\n",
    );
    assert_eq!(outcome.parsed, 1);
    assert_eq!(outcome.discarded, 0);
    let entry = &entries[0];
    assert!(matches!(entry.kind, EntryKind::Movie));
    assert_eq!(entry.title, "Duna");
    assert_eq!(entry.title_norm, "duna");
    assert_eq!(entry.year, Some(2021));
    assert_eq!(entry.logo.as_deref(), Some("http://p/duna.jpg"));
    assert_eq!(entry.group_name.as_deref(), Some("Filmes | Ficção"));
    assert_eq!(entry.url, "http://host/duna.mp4");
}

#[test]
fn filme_com_ano_solto_no_fim() {
    let (entries, _) = parse("#EXTINF:-1,Matrix 1999\nhttp://host/m\n");
    assert_eq!(entries[0].year, Some(1999));
    assert_eq!(entries[0].title, "Matrix");
}

#[test]
fn filme_sem_ano() {
    let (entries, _) = parse("#EXTINF:-1,Cidade de Deus\nhttp://host/cdd\n");
    assert_eq!(entries[0].year, None);
    assert!(matches!(entries[0].kind, EntryKind::Movie));
}

#[test]
fn serie_no_formato_s01e02() {
    let (entries, _) = parse("#EXTINF:-1,Fargo S01E02 4K\nhttp://host/f102\n");
    let entry = &entries[0];
    assert!(matches!(entry.kind, EntryKind::Series));
    assert_eq!(entry.title, "Fargo");
    assert_eq!(entry.season, Some(1));
    assert_eq!(entry.episode, Some(2));
    assert_eq!(entry.quality.as_deref(), Some("4K"));
}

#[test]
fn serie_no_formato_1x02() {
    let (entries, _) = parse("#EXTINF:-1,Fargo 1x02\nhttp://host/f102\n");
    assert!(matches!(entries[0].kind, EntryKind::Series));
    assert_eq!(entries[0].season, Some(1));
    assert_eq!(entries[0].episode, Some(2));
    assert_eq!(entries[0].title, "Fargo");
}

#[test]
fn serie_escrita_em_portugues() {
    let (entries, _) = parse("#EXTINF:-1,Fargo Temporada 2 Episodio 5\nhttp://host/f205\n");
    assert!(matches!(entries[0].kind, EntryKind::Series));
    assert_eq!(entries[0].season, Some(2));
    assert_eq!(entries[0].episode, Some(5));
    assert_eq!(entries[0].title, "Fargo");
}

#[test]
fn canal_por_tvg_chno() {
    let (entries, _) = parse(
        "#EXTINF:-1 tvg-id=\"globo.br\" tvg-chno=\"12\",Globo SP HD\nhttp://host/globo\n",
    );
    let entry = &entries[0];
    assert!(matches!(entry.kind, EntryKind::Channel));
    assert_eq!(entry.channel_number, Some(12));
    assert_eq!(entry.title, "Globo SP");
}

#[test]
fn canal_por_group_title_ao_vivo() {
    let (entries, _) = parse("#EXTINF:-1 group-title=\"Canais | Esportes\",SporTV\nhttp://h/s\n");
    assert!(matches!(entries[0].kind, EntryKind::Channel));
}

#[test]
fn serie_ganha_de_canal_quando_os_dois_casam() {
    // A regra 1 vence a regra 2: marca de episódio decide, mesmo com tvg-id.
    let (entries, _) = parse(
        "#EXTINF:-1 tvg-id=\"x\" group-title=\"Canais\",Fargo S03E01\nhttp://h/f\n",
    );
    assert!(matches!(entries[0].kind, EntryKind::Series));
}

#[test]
fn atributos_fora_de_ordem() {
    let (entries, _) = parse(
        "#EXTINF:-1 group-title=\"Filmes\" tvg-logo=\"http://p.jpg\" tvg-name=\"ignorado\",Duna\nhttp://h/d\n",
    );
    assert_eq!(entries[0].logo.as_deref(), Some("http://p.jpg"));
    assert_eq!(entries[0].group_name.as_deref(), Some("Filmes"));
}

#[test]
fn linha_sem_virgula_e_descartada() {
    let (entries, outcome) = parse("#EXTINF:-1 tvg-id=\"x\"\nhttp://h/x\n#EXTINF:-1,Bom\nhttp://h/b\n");
    assert_eq!(entries.len(), 1, "só a entrada boa entra");
    assert_eq!(outcome.discarded, 1);
}

#[test]
fn extinf_sem_url_e_descartado() {
    let (entries, outcome) = parse("#EXTINF:-1,Sem stream\n#EXTINF:-1,Bom\nhttp://h/b\n");
    assert_eq!(entries.len(), 1);
    assert_eq!(outcome.discarded, 1);
}

#[test]
fn linhas_vazias_e_comentarios_nao_contam_como_descarte() {
    let (_, outcome) = parse("#EXTM3U\n\n#EXTVLCOPT:foo\n#EXTINF:-1,Bom\nhttp://h/b\n");
    assert_eq!(outcome.discarded, 0);
    assert_eq!(outcome.parsed, 1);
}

#[test]
fn lista_vazia_devolve_zero() {
    let (entries, outcome) = parse("#EXTM3U\n");
    assert!(entries.is_empty());
    assert_eq!(outcome.parsed, 0);
}
