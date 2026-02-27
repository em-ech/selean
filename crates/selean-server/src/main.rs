//! Server binary for the Selean design platform.
//!
//! Reads configuration from environment variables and starts an HTTP server
//! with the Selean API routes, CORS for dev, and static file serving.
#![allow(clippy::expect_used)]

use selean_server::{AppState, create_router};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let state = match AppState::from_env() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to initialize: {e}");
            eprintln!("Set the ANTHROPIC_API_KEY environment variable and try again.");
            std::process::exit(1);
        }
    };

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);

    let static_dir =
        std::env::var("STATIC_DIR").unwrap_or_else(|_| "web/selean-app/dist".to_string());

    let cors = CorsLayer::new()
        .allow_origin(["http://localhost:3000".parse().expect("valid origin")])
        .allow_methods(Any)
        .allow_headers(Any);

    let app = create_router(state)
        .layer(cors)
        .fallback_service(ServeDir::new(&static_dir));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Selean server listening on http://{addr}");
    tracing::info!("Static files: {static_dir}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app).await.expect("server error");
}
