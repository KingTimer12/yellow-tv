use yellow_tv_lib::{db, library};

fn seed(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "INSERT INTO items(id, kind, title, title_norm, created_at)
           VALUES('m1','movie','Duna','duna',0),('s1','series','Fargo','fargo',0);
         INSERT INTO episodes(id, series_id, season, episode, title) VALUES
           ('e102','s1',1,2,'Ep 2'),
           ('e101','s1',1,1,'Ep 1'),
           ('e201','s1',2,1,'Ep 1');",
    )
    .unwrap();
}

#[test]
fn posicao_abaixo_do_minimo_nao_cria_linha() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    let saved = library::set_progress(&conn, "m1", "item", 12.0, Some(6000.0)).unwrap();
    assert!(saved.is_none(), "clique acidental não entra no histórico");
    assert!(library::progress_for(&conn, "m1", "item").unwrap().is_none());
}

#[test]
fn posicao_acima_do_minimo_cria_e_depois_atualiza() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::set_progress(&conn, "m1", "item", 45.0, Some(6000.0)).unwrap();
    library::set_progress(&conn, "m1", "item", 90.0, Some(6000.0)).unwrap();
    let stored = library::progress_for(&conn, "m1", "item").unwrap().unwrap();
    assert_eq!(stored.position_secs, 90.0);
    assert!(!stored.completed);
    let rows: i64 = conn
        .query_row("SELECT count(*) FROM progress", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 1, "upsert, não insert duplicado");
}

#[test]
fn linha_existente_aceita_recuo_abaixo_do_minimo() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::set_progress(&conn, "m1", "item", 400.0, Some(6000.0)).unwrap();
    library::set_progress(&conn, "m1", "item", 5.0, Some(6000.0)).unwrap();
    let stored = library::progress_for(&conn, "m1", "item").unwrap().unwrap();
    assert_eq!(stored.position_secs, 5.0, "quem já tem histórico pode voltar ao começo");
}

#[test]
fn noventa_e_dois_por_cento_marca_como_concluido() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::set_progress(&conn, "m1", "item", 5519.0, Some(6000.0)).unwrap();
    assert!(!library::progress_for(&conn, "m1", "item").unwrap().unwrap().completed);
    library::set_progress(&conn, "m1", "item", 5521.0, Some(6000.0)).unwrap();
    assert!(library::progress_for(&conn, "m1", "item").unwrap().unwrap().completed);
}

#[test]
fn sem_duracao_nunca_conclui_sozinho() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::set_progress(&conn, "m1", "item", 99999.0, None).unwrap();
    assert!(!library::progress_for(&conn, "m1", "item").unwrap().unwrap().completed);
}

#[test]
fn marcar_a_mao_funciona_nos_dois_sentidos() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::mark_watched(&conn, "m1", "item", true).unwrap();
    assert!(library::progress_for(&conn, "m1", "item").unwrap().unwrap().completed);
    library::mark_watched(&conn, "m1", "item", false).unwrap();
    let stored = library::progress_for(&conn, "m1", "item").unwrap().unwrap();
    assert!(!stored.completed);
    assert_eq!(stored.position_secs, 0.0, "desmarcar zera a posição");
}

#[test]
fn continuar_assistindo_ordena_por_mais_recente_e_ignora_concluidos() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::set_progress(&conn, "m1", "item", 100.0, Some(6000.0)).unwrap();
    library::set_progress(&conn, "e101", "episode", 200.0, Some(2400.0)).unwrap();
    library::mark_watched(&conn, "e201", "episode", true).unwrap();

    let entries = library::continue_watching(&conn, 10).unwrap();
    assert_eq!(entries.len(), 2, "o concluído não aparece");
    assert!(entries.iter().all(|entry| entry.title == "Duna" || entry.title == "Fargo"));

    let episode = entries.iter().find(|entry| entry.owner_kind == "episode").unwrap();
    assert_eq!(episode.title, "Fargo", "episódio se apresenta pelo nome da série");
    assert_eq!(episode.item_id, "s1");
    assert_eq!(episode.season, Some(1));
    assert_eq!(episode.episode, Some(1));
    assert!((episode.percent - 200.0 / 2400.0).abs() < 1e-6);
}

#[test]
fn continuar_assistindo_respeita_o_limite() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    library::set_progress(&conn, "m1", "item", 100.0, Some(6000.0)).unwrap();
    library::set_progress(&conn, "e101", "episode", 100.0, Some(2400.0)).unwrap();
    assert_eq!(library::continue_watching(&conn, 1).unwrap().len(), 1);
}

#[test]
fn proximo_episodio_ignora_a_ordem_de_insercao() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    let next = library::next_episode(&conn, "s1").unwrap().unwrap();
    assert_eq!((next.season, next.episode), (1, 1));

    library::mark_watched(&conn, "e101", "episode", true).unwrap();
    let next = library::next_episode(&conn, "s1").unwrap().unwrap();
    assert_eq!((next.season, next.episode), (1, 2));

    library::mark_watched(&conn, "e102", "episode", true).unwrap();
    let next = library::next_episode(&conn, "s1").unwrap().unwrap();
    assert_eq!((next.season, next.episode), (2, 1));
}

#[test]
fn serie_toda_concluida_devolve_none() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    for id in ["e101", "e102", "e201"] {
        library::mark_watched(&conn, id, "episode", true).unwrap();
    }
    assert!(library::next_episode(&conn, "s1").unwrap().is_none());
}

#[test]
fn favorito_alterna_e_lista() {
    let conn = db::open_memory().unwrap();
    seed(&conn);
    assert!(library::toggle_favorite(&conn, "m1").unwrap());
    assert_eq!(library::favorites(&conn).unwrap(), vec!["m1".to_owned()]);
    assert!(!library::toggle_favorite(&conn, "m1").unwrap());
    assert!(library::favorites(&conn).unwrap().is_empty());
}
