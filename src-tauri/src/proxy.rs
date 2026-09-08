//! Proxy local de streams.
//!
//! A WebView roda em `tauri://localhost` e as listas IPTV servem em HTTP puro, que
//! o navegador embutido bloqueia como conteúdo inseguro. Este proxy escuta em
//! 127.0.0.1, repassa o stream com `reqwest` e devolve por HTTP local — o que
//! resolve de uma vez o conteúdo misto e a política de origem.
//!
//! `Range` é repassado nos dois sentidos, senão não há como arrastar a barra de um
//! filme .mp4.

use std::net::{Ipv4Addr, SocketAddr, TcpListener};

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde::Deserialize;

/// Alguns servidores Xtream recusam clientes que não se anunciam como player.
const PLAYER_AGENT: &str = "VLC/3.0.20 LibVLC/3.0.20";

/// Cabeçalhos que fazem sentido repassar da origem para a WebView.
const FORWARDED: [HeaderName; 5] = [
    header::CONTENT_TYPE,
    header::CONTENT_LENGTH,
    header::CONTENT_RANGE,
    header::ACCEPT_RANGES,
    header::CACHE_CONTROL,
];

#[derive(Deserialize)]
struct StreamQuery {
    url: String,
}

async fn stream(
    State(client): State<reqwest::Client>,
    Query(query): Query<StreamQuery>,
    headers: HeaderMap,
) -> Response {
    if !(query.url.starts_with("http://") || query.url.starts_with("https://")) {
        return (StatusCode::BAD_REQUEST, "somente http e https").into_response();
    }

    let mut request = client.get(&query.url).header(header::USER_AGENT, PLAYER_AGENT);
    if let Some(range) = headers.get(header::RANGE) {
        request = request.header(header::RANGE, range);
    }

    let upstream = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("origem não respondeu: {error}"),
            )
                .into_response()
        }
    };

    let status = StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = Response::builder().status(status);

    for name in FORWARDED {
        if let Some(value) = upstream.headers().get(&name) {
            response = response.header(name, value);
        }
    }
    // A WebView pede de outra origem (tauri://localhost), então o CORS é obrigatório.
    response = response.header(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );

    response
        .body(Body::from_stream(upstream.bytes_stream()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Reserva a porta de forma síncrona (o front precisa dela na inicialização) e
/// devolve o número junto da tarefa que atende as conexões.
pub fn start() -> std::io::Result<u16> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;

    let client = reqwest::Client::builder()
        .user_agent(PLAYER_AGENT)
        .build()
        .expect("cliente HTTP");

    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("proxy de stream não subiu: {error}");
                return;
            }
        };
        let router = Router::new()
            .route("/stream", get(stream))
            .with_state(client);
        if let Err(error) = axum::serve(listener, router).await {
            eprintln!("proxy de stream encerrou: {error}");
        }
    });

    Ok(port)
}
