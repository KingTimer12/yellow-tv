use yellow_tv_lib::{db, meta, settings};

#[test]
fn setting_grava_le_e_sobrescreve() {
    let conn = db::open_memory().unwrap();
    assert!(settings::get(&conn, settings::TMDB_KEY).unwrap().is_none());
    settings::set(&conn, settings::TMDB_KEY, "abc").unwrap();
    assert_eq!(settings::get(&conn, settings::TMDB_KEY).unwrap().as_deref(), Some("abc"));
    settings::set(&conn, settings::TMDB_KEY, "def").unwrap();
    assert_eq!(settings::get(&conn, settings::TMDB_KEY).unwrap().as_deref(), Some("def"));
}

#[test]
fn valor_em_branco_conta_como_ausente() {
    let conn = db::open_memory().unwrap();
    settings::set(&conn, settings::TMDB_KEY, "   ").unwrap();
    assert!(settings::tmdb_key(&conn).is_none());
}

#[test]
fn cache_de_meta_vai_e_volta_da_tabela() {
    let conn = db::open_memory().unwrap();
    let id = meta::cache_id("movie", "Duna", Some(2021));
    assert!(meta::cached(&conn, &id).is_none());

    let mut payload = meta::Meta::default();
    payload.overview = "Areia".into();
    payload.source = "tmdb".into();
    meta::store(&conn, &id, &payload).unwrap();

    let restored = meta::cached(&conn, &id).expect("cache preenchido");
    assert_eq!(restored.overview, "Areia");
}

#[test]
fn cache_id_e_estavel_e_sensivel_ao_kind() {
    assert_eq!(
        meta::cache_id("movie", "Duna", Some(2021)),
        meta::cache_id("movie", "duna 4K", Some(2021)),
        "o cache segue o título normalizado, não o texto cru"
    );
    assert_ne!(
        meta::cache_id("movie", "Fargo", None),
        meta::cache_id("tv", "Fargo", None)
    );
}
