//! Server binary for the Selean design platform.
//!
//! Reads configuration from environment variables and starts an HTTP server
//! with the Selean API routes, CORS for dev, and static file serving.
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::time::Duration;

use selean_server::{
    AppState, AuthConfig, CollabState, create_router_with_options, load_snapshots_into,
    start_snapshot_task,
};
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

    let mut state = match AppState::from_env() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to initialize: {e}");
            eprintln!("Set the ANTHROPIC_API_KEY environment variable and try again.");
            std::process::exit(1);
        }
    };

    // Connect to database if DATABASE_URL is set.
    if let Ok(db_config) = selean_db::DbConfig::from_env() {
        match selean_db::create_pool(&db_config).await {
            Ok(pool) => {
                tracing::info!("database connected and migrations applied");
                state = state.with_db(pool);
            }
            Err(e) => {
                tracing::error!("database connection failed: {e}");
                tracing::warn!(
                    "continuing without database; document persistence via snapshots only"
                );
            }
        }
    } else {
        tracing::info!("DATABASE_URL not set; running without database");
    }

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

    let auth_config = AuthConfig::from_env();
    if auth_config.is_enabled() {
        tracing::info!("Legacy auth enabled (SELEAN_AUTH_SECRET is set)");
    }
    if state.jwt.is_some() {
        tracing::info!("JWT auth enabled (JWT_SECRET is set)");
    }
    if !auth_config.is_enabled() && state.jwt.is_none() {
        tracing::warn!("Auth disabled (set JWT_SECRET or SELEAN_AUTH_SECRET to enable)");
    }

    let snapshot_dir: PathBuf = std::env::var("SELEAN_DATA_DIR")
        .unwrap_or_else(|_| "data/rooms".to_string())
        .into();

    let snapshot_interval_secs: u64 = std::env::var("SELEAN_SNAPSHOT_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    let collab = CollabState::new();

    // Load room snapshots from previous sessions.
    match load_snapshots_into(&snapshot_dir, &collab) {
        Ok(0) => {}
        Ok(n) => tracing::info!(
            "loaded {n} room snapshot(s) from {}",
            snapshot_dir.display()
        ),
        Err(e) => tracing::error!("failed to load room snapshots: {e}"),
    }

    // Start periodic snapshot task.
    let _snapshot_handle = start_snapshot_task(
        Duration::from_secs(snapshot_interval_secs),
        snapshot_dir.clone(),
        collab.clone(),
    );
    tracing::info!(
        "room snapshots: dir={}, interval={snapshot_interval_secs}s",
        snapshot_dir.display()
    );

    let app = create_router_with_options(state, collab, auth_config)
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
