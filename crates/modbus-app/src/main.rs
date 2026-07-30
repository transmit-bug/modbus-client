//! Binary entry point: wires the driver/engine/server together, embeds the
//! compiled frontend, and serves static assets + the WebSocket from one
//! localhost HTTP server.

use axum::{
    extract::OriginalUri,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use modbus_engine::{ConnectionManager, SharedConnectionManager};
use modbus_server::app as server_app;
use rust_embed::{Embed, EmbeddedFile};
use std::net::SocketAddr;
use std::sync::Arc;

/// The compiled frontend, embedded at build time.
#[derive(Embed)]
#[folder = "../../frontend/dist"]
struct FrontendAsset;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "modbus=info,info".into()),
        )
        .init();

    let manager: SharedConnectionManager = Arc::new(ConnectionManager::new());
    let addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    let app: Router = server_app(manager).fallback(serve_asset);

    tracing::info!("Modbus client listening on http://{addr}");
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

/// Serve an embedded frontend asset, falling back to `index.html` for any
/// unknown path (SPA routing).
async fn serve_asset(OriginalUri(uri): OriginalUri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if let Some(file) = FrontendAsset::get(path) {
        return asset_response(path, file);
    }
    if let Some(file) = FrontendAsset::get("index.html") {
        return asset_response("index.html", file);
    }
    (StatusCode::NOT_FOUND, "frontend not built; run `npm run build` in frontend/").into_response()
}

fn asset_response(path: &str, file: EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, mime.as_ref())],
        file.data.into_owned(),
    )
        .into_response()
}
