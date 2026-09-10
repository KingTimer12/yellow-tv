use std::io::Cursor;
use yellow_tv_lib::{catalog, db, import, library};

const LISTA: &str = "#EXTM3U\n\
#EXTINF:-1 group-title=\"Filmes | Ficção\",Duna (2021)\nhttp://a/duna\n\
#EXTINF:-1 group-title=\"Filmes | Ficção\",Blade Runner (1982)\nhttp://a/br\n\
#EXTINF:-1 group-title=\"Filmes | Ação\",Caçador de Androides (1982)\nhttp://a/ca\n\
#EXTINF:-1 group-title=\"Series | Drama\",Fargo S01E01\nhttp://a/f101\n\
#EXTINF:-1 group-title=\"Series | Drama\",Fargo S01E02\nhttp://a/f102\n\
#EXTINF:-1 tvg-chno=\"12\",Globo SP\nhttp://a/globo\n";

fn banco() -> rusqlite::Connection {
    let mut conn = db::open_memory().unwrap();
    let source = import::upsert_source(&conn, "http://a", "A", "url").unwrap();
    import::ingest(&mut conn, source.id, Cursor::new(LISTA.to_owned()), &mut |_| {}).unwrap();
    conn
}

#[test]
fn pagina_lista_apenas_o_kind_pedido() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Movie, "", None, 0, false).unwrap();
    assert_eq!(page.total, 3);
    assert!(page.items.iter().all(|item| item.kind == "movie"));
    assert_eq!(page.page_size, catalog::PAGE_SIZE);
}

#[test]
fn busca_encontra_sem_acento_e_por_prefixo() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Movie, "cacador", None, 0, false).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].title, "Caçador de Androides");

    let prefixo = catalog::page(&conn, catalog::Kind::Movie, "bla", None, 0, false).unwrap();
    assert_eq!(prefixo.total, 1, "prefixo casa: 'bla' encontra Blade Runner");
}

#[test]
fn busca_com_dois_termos_exige_os_dois() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Movie, "blade runner", None, 0, false).unwrap();
    assert_eq!(page.total, 1);
    let nenhum = catalog::page(&conn, catalog::Kind::Movie, "blade duna", None, 0, false).unwrap();
    assert_eq!(nenhum.total, 0);
}

#[test]
fn busca_com_caractere_estranho_nao_explode() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Movie, "\"duna* OR", None, 0, false).unwrap();
    assert!(page.total <= 3, "consulta sanitizada, sem erro de sintaxe FTS");
}

#[test]
fn filtro_de_grupo_e_contagem_de_grupos() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Movie, "", Some("Filmes | Ficção"), 0, false).unwrap();
    assert_eq!(page.total, 2);
    let ficcao = page
        .groups
        .iter()
        .find(|group| group.name == "Filmes | Ficção")
        .unwrap();
    assert_eq!(ficcao.count, 2);
    assert!(
        page.groups.iter().all(|group| group.name.starts_with("Filmes")),
        "grupos de série não vazam para a aba de filmes"
    );
}

#[test]
fn paginacao_respeita_a_pagina_pedida() {
    let conn = banco();
    let segunda = catalog::page(&conn, catalog::Kind::Movie, "", None, 1, false).unwrap();
    assert_eq!(segunda.total, 3);
    assert!(segunda.items.is_empty(), "só há uma página de 60");
}

#[test]
fn filtro_de_nao_assistidos_esconde_concluidos() {
    let conn = banco();
    let id = conn
        .query_row("SELECT id FROM items WHERE title = 'Duna'", [], |row| row.get::<_, String>(0))
        .unwrap();
    library::mark_watched(&conn, &id, "item", true).unwrap();

    let todos = catalog::page(&conn, catalog::Kind::Movie, "", None, 0, false).unwrap();
    let restantes = catalog::page(&conn, catalog::Kind::Movie, "", None, 0, true).unwrap();
    assert_eq!(todos.total, 3);
    assert_eq!(restantes.total, 2);
}

#[test]
fn item_traz_progresso_streams_e_relacionados() {
    let conn = banco();
    let id = conn
        .query_row("SELECT id FROM items WHERE title = 'Duna'", [], |row| row.get::<_, String>(0))
        .unwrap();
    library::set_progress(&conn, &id, "item", 600.0, Some(6000.0)).unwrap();

    let detail = catalog::item(&conn, catalog::Kind::Movie, &id).unwrap();
    assert_eq!(detail.item.title, "Duna");
    assert_eq!(detail.streams.len(), 1);
    assert_eq!(detail.streams[0].source_label, "A");
    assert_eq!(detail.progress.unwrap().position_secs, 600.0);
    assert!(
        detail.related.iter().all(|other| other.id != id),
        "relacionados não incluem o próprio título"
    );
    assert!((detail.item.percent - 0.1).abs() < 1e-6);
}

#[test]
fn item_inexistente_devolve_erro_legivel() {
    let conn = banco();
    let error = catalog::item(&conn, catalog::Kind::Movie, "nao-existe").unwrap_err();
    assert!(error.contains("não encontrado"));
}

#[test]
fn serie_lista_episodios_ordenados_com_progresso() {
    let conn = banco();
    let series_id = conn
        .query_row("SELECT id FROM items WHERE kind = 'series'", [], |row| row.get::<_, String>(0))
        .unwrap();
    let detail = catalog::episodes(&conn, &series_id).unwrap();
    assert_eq!(detail.title, "Fargo");
    assert_eq!(detail.episodes.len(), 2);
    assert_eq!(
        (detail.episodes[0].season, detail.episodes[0].episode),
        (1, 1)
    );
    assert_eq!(detail.episodes[0].streams.len(), 1);
    assert!(!detail.episodes[0].completed);
}

#[test]
fn serie_reporta_temporadas_e_total_de_episodios() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Series, "", None, 0, false).unwrap();
    let fargo = &page.items[0];
    assert_eq!(fargo.seasons, 1);
    assert_eq!(fargo.episode_count, 2);
}

#[test]
fn canais_expoem_o_numero_do_canal() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Channel, "", None, 0, false).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].channel_number, Some(12));
}

#[test]
fn board_tem_continuar_recentes_e_generos() {
    let conn = banco();
    let id = conn
        .query_row("SELECT id FROM items WHERE title = 'Duna'", [], |row| row.get::<_, String>(0))
        .unwrap();
    library::set_progress(&conn, &id, "item", 600.0, Some(6000.0)).unwrap();

    let rows = catalog::board(&conn).unwrap();
    assert_eq!(rows[0].key, "continue");
    assert_eq!(rows[0].items.len(), 1);
    assert_eq!(rows[1].key, "recent");
    assert!(
        rows.iter().any(|row| row.key.starts_with("group:")),
        "linhas por gênero aparecem depois"
    );
    assert!(
        rows.iter().all(|row| !row.items.is_empty()),
        "linha vazia não vai para a tela"
    );
}

#[test]
fn board_sem_progresso_nao_traz_linha_de_continuar() {
    let conn = banco();
    let rows = catalog::board(&conn).unwrap();
    assert!(rows.iter().all(|row| row.key != "continue"));
}

#[test]
fn by_ids_preserva_a_ordem_pedida() {
    let conn = banco();
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM items WHERE kind = 'movie' ORDER BY title")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<String>>>()
        .unwrap();
    let reversed: Vec<String> = ids.iter().rev().cloned().collect();
    let items = catalog::by_ids(&conn, &reversed).unwrap();
    let got: Vec<String> = items.into_iter().map(|item| item.id).collect();
    assert_eq!(got, reversed);
}

// --- Correções da revisão final ---

/// Retomada de episódio: sem `position_secs` na linha, `Serie.tsx` não tem o
/// que passar como `startAt` e o episódio sempre recomeçava do zero.
#[test]
fn episodio_expoe_a_posicao_salva_para_retomar() {
    let conn = banco();
    let series_id = conn
        .query_row("SELECT id FROM items WHERE kind = 'series'", [], |row| row.get::<_, String>(0))
        .unwrap();
    let primeiro = catalog::episodes(&conn, &series_id).unwrap().episodes[0]
        .id
        .clone();
    library::set_progress(&conn, &primeiro, "episode", 420.0, Some(1400.0)).unwrap();

    let detail = catalog::episodes(&conn, &series_id).unwrap();
    assert_eq!(detail.episodes[0].position_secs, 420.0);
    assert!((detail.episodes[0].percent - 0.3).abs() < 1e-6);
    assert_eq!(
        detail.episodes[1].position_secs, 0.0,
        "sem progresso, a posição é zero"
    );
}

/// I11: canais vêm em uma página só, senão a lista do player dispara dezenas de
/// `catalog_page` e cada uma refaz o `GROUP BY group_name` inteiro.
#[test]
fn canais_cabem_em_uma_unica_pagina() {
    let conn = banco();
    let page = catalog::page(&conn, catalog::Kind::Channel, "", None, 0, false).unwrap();
    assert_eq!(page.page_size, catalog::CHANNEL_PAGE_SIZE);
    assert!(page.page_size > catalog::PAGE_SIZE);
    assert_eq!(
        (page.total as f64 / page.page_size as f64).ceil() as i64,
        1,
        "uma página basta"
    );
}
