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

// --- Correções da revisão final ---

/// C2: uma fonte criada agora cujo primeiro import falha não pode sobreviver,
/// senão `list_sources` fica não-vazio para sempre e o portão de `/setup` some.
#[test]
fn fonte_nova_some_quando_o_primeiro_import_falha() {
    let mut conn = db::open_memory().unwrap();
    let (source, was_new) = import::begin_add(&conn, "http://ruim", "Ruim", "url").unwrap();
    assert!(was_new);

    let vazia: Result<Cursor<String>, String> = Ok(Cursor::new("#EXTM3U\n".to_owned()));
    let erro = import::finish_add(&mut conn, &source, was_new, vazia, &mut |_| {});
    assert!(erro.is_err());
    assert!(
        import::list_sources(&conn).unwrap().is_empty(),
        "a fonte órfã precisa ser removida"
    );
}

/// C2: falha de download também desfaz a fonte recém-criada.
#[test]
fn fonte_nova_some_quando_o_download_falha() {
    let mut conn = db::open_memory().unwrap();
    let (source, was_new) = import::begin_add(&conn, "http://offline", "Off", "url").unwrap();
    let reader: Result<Cursor<String>, String> = Err("não foi possível baixar a lista".into());
    assert!(import::finish_add(&mut conn, &source, was_new, reader, &mut |_| {}).is_err());
    assert!(import::list_sources(&conn).unwrap().is_empty());
}

/// C2: re-sincronizar uma fonte que já existia nunca pode apagá-la, mesmo que o
/// import falhe.
#[test]
fn fonte_existente_sobrevive_a_um_import_que_falha() {
    let mut conn = db::open_memory().unwrap();
    ingest(&mut conn, "http://a", LISTA_A);

    let (source, was_new) = import::begin_add(&conn, "http://a", "Teste", "url").unwrap();
    assert!(!was_new, "a fonte já existia");
    let vazia: Result<Cursor<String>, String> = Ok(Cursor::new("#EXTM3U\n".to_owned()));
    assert!(import::finish_add(&mut conn, &source, was_new, vazia, &mut |_| {}).is_err());
    assert_eq!(import::list_sources(&conn).unwrap().len(), 1);
}

/// I10: um re-sync que perdeu títulos não pode deixar itens sem stream para
/// trás — eles apareceriam com Play desabilitado e inflando a contagem do grupo.
#[test]
fn resync_remove_itens_que_sumiram_da_lista() {
    let mut conn = db::open_memory().unwrap();
    ingest(&mut conn, "http://a", LISTA_A);
    let antes: i64 = conn
        .query_row("SELECT count(*) FROM items", [], |row| row.get(0))
        .unwrap();
    assert_eq!(antes, 3, "filme, série e canal");

    // A mesma fonte volta só com o canal.
    let menor = "#EXTM3U\n#EXTINF:-1 tvg-chno=\"12\",Globo SP HD\nhttp://a/globo\n";
    ingest(&mut conn, "http://a", menor);

    let restantes: Vec<String> = conn
        .prepare("SELECT id FROM items ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(restantes.len(), 1, "só o canal continua alcançável");
}

/// I10: o item que sumiu de uma lista mas continua em outra permanece.
#[test]
fn resync_preserva_item_que_outra_lista_ainda_serve() {
    let mut conn = db::open_memory().unwrap();
    ingest(&mut conn, "http://a", LISTA_A);
    ingest(&mut conn, "http://b", LISTA_B);

    let so_canal = "#EXTM3U\n#EXTINF:-1 tvg-chno=\"12\",Globo SP HD\nhttp://a/globo\n";
    ingest(&mut conn, "http://a", so_canal);

    let duna: i64 = conn
        .query_row(
            "SELECT count(*) FROM items WHERE kind = 'movie'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(duna, 1, "a lista B ainda serve Duna");
}

/// I5: o progresso precisa chegar durante o parse, não como rajada no fim.
#[test]
fn progresso_e_emitido_durante_o_parse() {
    let mut conn = db::open_memory().unwrap();
    let source = import::upsert_source(&conn, "http://grande", "Grande", "url").unwrap();

    let mut lista = String::from("#EXTM3U\n");
    for index in 0..1200 {
        lista.push_str(&format!(
            "#EXTINF:-1 tvg-chno=\"{index}\",Canal {index}\nhttp://grande/{index}\n"
        ));
    }

    // O sink registra quantas entradas já estavam no banco quando cada marco
    // chegou: com a emissão em rajada no fim, todos veriam a lista inteira.
    let mut marcos: Vec<usize> = Vec::new();
    let report = {
        let marcos = &mut marcos;
        import::ingest(
            &mut conn,
            source.id,
            Cursor::new(lista),
            &mut |parsed| marcos.push(parsed),
        )
        .unwrap()
    };

    assert_eq!(report.parsed, 1200);
    assert!(
        marcos.len() >= 3,
        "esperado um marco a cada bloco, veio {marcos:?}"
    );
    assert_eq!(marcos[0], 500, "o primeiro marco é parcial, não o total");
    assert_eq!(marcos.last().copied(), Some(1200));
}

// O relatório precisa falar numa unidade só. Antes, `series` contava séries
// distintas enquanto `movies` contava linhas do arquivo, então os números não
// somavam e diziam coisas diferentes com a mesma cara.
#[test]
fn relatorio_conta_titulos_distintos_e_soma_o_total_da_fonte() {
    let mut conn = db::open_memory().unwrap();
    let source = import::upsert_source(&conn, "http://lista", "Lista", "url").unwrap();

    // 6 linhas: 4 episódios de uma série só, e 2 do mesmo filme (duas versões).
    let lista = concat!(
        "#EXTINF:-1 group-title=\"Series | Netflix\",Origem S01 E01\nhttp://host/1\n",
        "#EXTINF:-1 group-title=\"Series | Netflix\",Origem S01 E02\nhttp://host/2\n",
        "#EXTINF:-1 group-title=\"Series | Legendadas\",Origem [L] S01 E01\nhttp://host/3\n",
        "#EXTINF:-1 group-title=\"Series | Legendadas\",Origem [L] S01 E02\nhttp://host/4\n",
        "#EXTINF:-1 group-title=\"Filmes | Acao\",Duna (2021)\nhttp://host/5\n",
        "#EXTINF:-1 group-title=\"Filmes | Legendados\",Duna [L] (2021)\nhttp://host/6\n",
    );

    let report = import::ingest(
        &mut conn,
        source.id,
        std::io::Cursor::new(lista.to_owned()),
        &mut |_| {},
    )
    .unwrap();

    assert_eq!(report.parsed, 6, "seis linhas lidas");
    assert_eq!(report.series, 1, "uma série, não quatro episódios");
    assert_eq!(report.movies, 1, "um filme, não duas versões");
    assert_eq!(report.channels, 0);
    // O total da fonte é a soma das três, na mesma unidade.
    assert_eq!(
        report.source.item_count,
        (report.movies + report.series + report.channels) as i64
    );
}
