//! HTTP server for the Selean design platform.
//!
//! Provides a REST API for:
//! - `POST /api/chat`: Claude API proxy with tool definitions and SSE streaming
//! - `POST /api/tools`: Execute tool calls against a scene
//! - `GET /api/tools`: List available LLM tools
//!
//! Static file serving for the React frontend is handled by tower-http.

pub mod auth;
pub mod chat;
pub mod collab;
pub mod indesign_bridge;
pub mod routes;
pub mod state;
pub mod stream;

pub use auth::AuthConfig;
pub use collab::snapshot::{load_snapshots_into, start_snapshot_task};
pub use collab::ws_handler::CollabState;
pub use routes::{create_router, create_router_with_options};
pub use state::AppState;
