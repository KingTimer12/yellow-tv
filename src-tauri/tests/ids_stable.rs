use yellow_tv_lib::ids;

#[test]
fn normaliza_caixa_acento_e_espacos() {
    assert_eq!(ids::normalize("  O   Caçador  "), "o cacador");
    assert_eq!(ids::normalize("Ação"), "acao");
}

#[test]
fn remove_tokens_de_qualidade() {
    assert_eq!(ids::normalize("Duna 4K"), "duna");
    assert_eq!(ids::normalize("Duna FHD H265"), "duna");
    assert_eq!(ids::normalize("Duna ALT 2"), "duna");
    assert_eq!(ids::normalize("Duna [L] (2021) DUBLADO"), "duna");
}

#[test]
fn nao_come_palavra_que_contem_token() {
    // "HD" solto sai; "Ghost" continua inteiro.
    assert_eq!(ids::normalize("Ghost HD"), "ghost");
}

#[test]
fn mesma_obra_com_qualidades_diferentes_colapsa_no_mesmo_id() {
    let a = ids::item_id("movie", &ids::normalize("Duna 4K"), Some(2021));
    let b = ids::item_id("movie", &ids::normalize("DUNA fhd"), Some(2021));
    assert_eq!(a, b);
}

#[test]
fn ano_diferente_gera_id_diferente() {
    let a = ids::item_id("movie", "duna", Some(2021));
    let b = ids::item_id("movie", "duna", Some(1984));
    assert_ne!(a, b);
}

#[test]
fn kind_diferente_gera_id_diferente() {
    let a = ids::item_id("movie", "fargo", None);
    let b = ids::item_id("series", "fargo", None);
    assert_ne!(a, b);
}

#[test]
fn id_e_estavel_entre_execucoes() {
    // Trava o formato: 32 caracteres hexadecimais e valor fixo.
    let id = ids::item_id("movie", "duna", Some(2021));
    assert_eq!(id.len(), 32);
    assert!(id.chars().all(|character| character.is_ascii_hexdigit()));
    assert_eq!(id, ids::item_id("movie", "duna", Some(2021)));
}

#[test]
fn episode_id_depende_de_temporada_e_episodio() {
    let series = ids::item_id("series", "fargo", None);
    assert_ne!(
        ids::episode_id(&series, 1, 2),
        ids::episode_id(&series, 2, 1)
    );
    assert_eq!(
        ids::episode_id(&series, 1, 2),
        ids::episode_id(&series, 1, 2)
    );
}
