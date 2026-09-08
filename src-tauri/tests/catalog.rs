//! Verifica a camada de dados contra as listas reais em ../data.

use std::path::PathBuf;

#[path = "../src/catalog.rs"]
mod catalog;

use catalog::{Catalog, Kind};

fn catalog() -> Catalog {
    Catalog::new(PathBuf::from("../data"))
}

#[test]
fn le_canais_ordenados() {
    let channels = catalog().channels().expect("canais");
    assert_eq!(channels.len(), 3677);
    assert_eq!(channels[0].channel_number, 1);
    assert!(channels.windows(2).all(|pair| pair[0].channel_number <= pair[1].channel_number));
}

#[test]
fn pagina_filmes_e_conta_grupos() {
    let page = catalog().page(Kind::Filmes, "", None, 0).expect("página");
    assert_eq!(page.total, 17_718);
    assert_eq!(page.items.len(), 60);
    assert!(page.groups.iter().any(|group| group.name == "Filmes | Drama"));
}

#[test]
fn busca_ignora_acento_e_caixa() {
    let page = catalog()
        .page(Kind::Filmes, "e o vento levou", None, 0)
        .expect("busca");
    assert!(
        page.items.iter().any(|item| item.title.contains("Vento Levou")),
        "esperava achar o título buscado, veio {:?}",
        page.items.iter().map(|i| &i.title).collect::<Vec<_>>()
    );
}

#[test]
fn segunda_pagina_traz_itens_diferentes() {
    let catalog = catalog();
    let first = catalog.page(Kind::Series, "", None, 0).expect("página 0");
    let second = catalog.page(Kind::Series, "", None, 1).expect("página 1");
    assert_ne!(first.items[0].id, second.items[0].id);
}

#[test]
fn filtra_por_grupo() {
    let page = catalog()
        .page(Kind::Series, "", Some("Series | Netflix"), 0)
        .expect("grupo");
    assert!(page.total > 1000);
    assert!(page.items.iter().all(|item| item.group == "Series | Netflix"));
}

#[test]
fn item_traz_relacionados_do_mesmo_grupo() {
    let found = catalog()
        .item(Kind::Series, "des-encanto-series-netflix")
        .expect("série");
    assert_eq!(found.item.title, "(Des)encanto");
    assert_eq!(found.item.episode_count, Some(50));
    assert!(!found.related.is_empty());
    assert!(found.related.iter().all(|item| item.group == found.item.group));
}

#[test]
fn episodios_vem_ordenados() {
    let detail = catalog()
        .episodes("des-encanto-series-netflix")
        .expect("episódios");
    assert_eq!(detail.episodes.len(), 50);
    assert!(detail
        .episodes
        .windows(2)
        .all(|pair| (pair[0].season, pair[0].episode) <= (pair[1].season, pair[1].episode)));
}

#[test]
fn recusa_id_que_escapa_do_diretorio() {
    let error = catalog().episodes("../../../etc/passwd").unwrap_err();
    assert!(error.contains("inválido"), "veio: {error}");
}

#[test]
fn erro_aponta_o_arquivo_que_falta() {
    let error = Catalog::new(PathBuf::from("/nao/existe"))
        .channels()
        .unwrap_err();
    assert!(error.contains("lista_pro.json"), "veio: {error}");
}
