use yellow_tv_lib::db;

#[test]
fn migra_para_versao_atual() {
    let conn = db::open_memory().expect("banco em memória");
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, db::SCHEMA_VERSION);
}

#[test]
fn cria_todas_as_tabelas() {
    let conn = db::open_memory().unwrap();
    for table in [
        "sources", "items", "episodes", "streams", "progress", "favorites", "meta", "settings",
        "items_fts",
    ] {
        let found: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = ?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(found, 1, "tabela ausente: {table}");
    }
}

#[test]
fn fts5_esta_disponivel_e_sincronizada() {
    let conn = db::open_memory().unwrap();
    conn.execute(
        "INSERT INTO items(id, kind, title, title_norm, created_at) VALUES('a','movie','Caçador','cacador',0)",
        [],
    )
    .unwrap();
    let hits: i64 = conn
        .query_row(
            "SELECT count(*) FROM items_fts WHERE items_fts MATCH 'cacador'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(hits, 1);
}

#[test]
fn migrar_duas_vezes_e_inofensivo() {
    let dir = std::env::temp_dir().join(format!("yellowtv-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("twice.db");
    let _ = std::fs::remove_file(&file);
    db::open(&file).unwrap();
    db::open(&file).expect("segunda abertura não deve falhar");
    std::fs::remove_file(&file).unwrap();
}

#[test]
fn remover_source_apaga_streams_mas_preserva_progresso() {
    let conn = db::open_memory().unwrap();
    conn.execute_batch(
        "INSERT INTO sources(id, url, label, kind, added_at) VALUES(1,'http://a','A','url',0);
         INSERT INTO items(id, kind, title, title_norm, created_at) VALUES('i1','movie','X','x',0);
         INSERT INTO streams(owner_id, owner_kind, source_id, url) VALUES('i1','item',1,'http://s');
         INSERT INTO progress(owner_id, owner_kind, position_secs, updated_at) VALUES('i1','item',120.0,0);",
    )
    .unwrap();
    conn.execute("DELETE FROM sources WHERE id = 1", []).unwrap();

    let streams: i64 = conn
        .query_row("SELECT count(*) FROM streams", [], |row| row.get(0))
        .unwrap();
    let progress: i64 = conn
        .query_row("SELECT count(*) FROM progress", [], |row| row.get(0))
        .unwrap();
    assert_eq!(streams, 0, "streams devem cair com a source");
    assert_eq!(progress, 1, "progresso sobrevive à remoção da lista");
}

// Um banco já existente na v1 tem que ganhar a coluna nova sem perder dados —
// este é o primeiro upgrade real que o app faz na máquina de alguém.
#[test]
fn banco_v1_ganha_a_coluna_variant_sem_perder_linhas() {
    let dir = std::env::temp_dir().join(format!("yellowtv-migracao-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("v1.sqlite");
    let _ = std::fs::remove_file(&path);

    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE streams(
               id INTEGER PRIMARY KEY,
               owner_id TEXT NOT NULL,
               owner_kind TEXT NOT NULL,
               source_id INTEGER NOT NULL,
               url TEXT NOT NULL,
               quality TEXT,
               channel_number INTEGER,
               UNIQUE(owner_id, url)
             );
             INSERT INTO streams(owner_id, owner_kind, source_id, url)
               VALUES('item-1', 'item', 1, 'http://host/a.mp4');
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }

    let conn = db::open(&path).expect("migração v1 para v2");
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);

    let variant: Option<String> = conn
        .query_row("SELECT variant FROM streams WHERE owner_id = 'item-1'", [], |row| row.get(0))
        .expect("a linha antiga continua lá, agora com a coluna nova");
    assert_eq!(variant, None);

    let _ = std::fs::remove_file(&path);
}
