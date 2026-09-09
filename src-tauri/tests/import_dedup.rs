use std::io::Cursor;
use yellow_tv_lib::{db, import};

const LISTA_A: &str = "#EXTM3U\n\
#EXTINF:-1 group-title=\"Filmes\",Duna (2021) 4K\nhttp://a/duna-4k\n\
#EXTINF:-1 group-title=\"Series\",Fargo S01E01\nhttp://a/fargo-101\n\
#EXTINF:-1 group-title=\"Series\",Fargo S01E02\nhttp://a/fargo-102\n\
#EXTINF:-1 tvg-chno=\"12\",Globo SP HD\nhttp://a/globo\n";

const LISTA_B: &str = "#EXTM3U\n\
#EXTINF:-1 group-title=\"Filmes\",DUNA (2021) FHD\nhttp://b/duna-fhd\n";

fn ingest(conn: &mut rusqlite::Connection, url: &str, text: &str) -> import::ImportReport {
    let source = import::upsert_source(conn, url, "Teste", "url").unwrap();
    let mut seen = 0usize;
    import::ingest(conn, source.id, Cursor::new(text.to_owned()), &mut |count| seen = count)
        .unwrap()
}

#[test]
fn conta_o_que_entrou_por_tipo() {
    let mut conn = db::open_memory().unwrap();
    let report = ingest(&mut conn, "http://a", LISTA_A);
    assert_eq!(report.parsed, 4);
    assert_eq!(report.discarded, 0);
    assert_eq!(report.movies, 1);
    assert_eq!(report.series, 1, "duas linhas, uma série");
    assert_eq!(report.channels, 1);
}

#[test]
fn episodios_viram_linhas_em_episodes() {
    let mut conn = db::open_memory().unwrap();
    ingest(&mut conn, "http://a", LISTA_A);
    let episodes: i64 = conn
        .query_row("SELECT count(*) FROM episodes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(episodes, 2);
    let owner_kinds: i64 = conn
        .query_row(
            "SELECT count(*) FROM streams WHERE owner_kind = 'episode'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(owner_kinds, 2, "stream de episódio aponta para episodes.id");
}

#[test]
fn duas_listas_com_o_mesmo_filme_geram_um_item_e_dois_streams() {
    let mut conn = db::open_memory().unwrap();
    ingest(&mut conn, "http://a", LISTA_A);
    ingest(&mut conn, "http://b", LISTA_B);

    let movies: i64 = conn
        .query_row("SELECT count(*) FROM items WHERE kind = 'movie'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(movies, 1, "mesmo filme em duas listas é um item");

    let streams: i64 = conn
        .query_row(
            "SELECT count(*) FROM streams WHERE owner_kind = 'item'
             AND owner_id = (SELECT id FROM items WHERE kind = 'movie')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(streams, 2, "duas fontes de stream para o mesmo item");
}

#[test]
fn reimportar_a_mesma_lista_nao_duplica() {
    let mut conn = db::open_memory().unwrap();
    ingest(&mut conn, "http://a", LISTA_A);
    ingest(&mut conn, "http://a", LISTA_A);
    let streams: i64 = conn
        .query_row("SELECT count(*) FROM streams", [], |row| row.get(0))
        .unwrap();
    assert_eq!(streams, 4);
}

#[test]
fn lista_vazia_falha_e_faz_rollback() {
    let mut conn = db::open_memory().unwrap();
    let source = import::upsert_source(&mut conn, "http://vazia", "Vazia", "url").unwrap();
    let result = import::ingest(
        &mut conn,
        source.id,
        Cursor::new("#EXTM3U\n".to_owned()),
        &mut |_| {},
    );
    assert!(result.is_err(), "lista sem entradas reconhecidas deve falhar");
    let items: i64 = conn
        .query_row("SELECT count(*) FROM items", [], |row| row.get(0))
        .unwrap();
    assert_eq!(items, 0);
}

#[test]
fn linhas_ruins_sao_contadas_sem_derrubar_o_import() {
    let mut conn = db::open_memory().unwrap();
    let report = ingest(
        &mut conn,
        "http://mista",
        "#EXTINF:-1 tvg-id=\"x\"\nhttp://h/x\n#EXTINF:-1,Duna (2021)\nhttp://h/d\n",
    );
    assert_eq!(report.parsed, 1);
    assert_eq!(report.discarded, 1);
}

#[test]
fn source_registra_contagem_e_data_de_sync() {
    let mut conn = db::open_memory().unwrap();
    let report = ingest(&mut conn, "http://a", LISTA_A);
    let stored = import::get_source(&conn, report.source.id).unwrap();
    assert_eq!(stored.item_count, 3, "3 títulos distintos");
    assert!(stored.last_sync_at.is_some());
}

#[test]
fn remover_source_apaga_itens_orfaos_e_preserva_progresso() {
    let mut conn = db::open_memory().unwrap();
    let report = ingest(&mut conn, "http://a", LISTA_A);
    let movie_id: String = conn
        .query_row("SELECT id FROM items WHERE kind = 'movie'", [], |row| row.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO progress(owner_id, owner_kind, position_secs, updated_at)
         VALUES(?1,'item',600.0,0)",
        [&movie_id],
    )
    .unwrap();

    import::remove_source(&mut conn, report.source.id).unwrap();

    let items: i64 = conn
        .query_row("SELECT count(*) FROM items", [], |row| row.get(0))
        .unwrap();
    let progress: i64 = conn
        .query_row("SELECT count(*) FROM progress", [], |row| row.get(0))
        .unwrap();
    assert_eq!(items, 0, "sem stream, o item é órfão e sai");
    assert_eq!(progress, 1, "o histórico fica para quando a lista voltar");
}

#[test]
fn progresso_reencontra_o_item_quando_a_lista_volta() {
    let mut conn = db::open_memory().unwrap();
    let report = ingest(&mut conn, "http://a", LISTA_A);
    let movie_id: String = conn
        .query_row("SELECT id FROM items WHERE kind = 'movie'", [], |row| row.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO progress(owner_id, owner_kind, position_secs, updated_at)
         VALUES(?1,'item',600.0,0)",
        [&movie_id],
    )
    .unwrap();
    import::remove_source(&mut conn, report.source.id).unwrap();
    ingest(&mut conn, "http://a", LISTA_A);

    let matched: i64 = conn
        .query_row(
            "SELECT count(*) FROM progress p JOIN items i ON i.id = p.owner_id",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(matched, 1, "o hash não depende da URL, então o progresso casa de novo");
}

#[test]
fn list_sources_devolve_o_que_foi_adicionado() {
    let mut conn = db::open_memory().unwrap();
    import::upsert_source(&mut conn, "http://a", "A", "url").unwrap();
    import::upsert_source(&mut conn, "/tmp/b.m3u", "B", "file").unwrap();
    let sources = import::list_sources(&conn).unwrap();
    assert_eq!(sources.len(), 2);
    assert!(sources.iter().any(|source| source.kind == "file"));
}
