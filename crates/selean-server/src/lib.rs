//! HTTP server for the Selean design platform.
//!
//! Provides a REST API for:
//! - `POST /api/chat`: Claude API proxy with tool definitions and SSE streaming
//! - `POST /api/tools`: Execute tool calls against a scene
//! - `GET /api/tools`: List available LLM tools
//!
//! Static file serving for the React frontend is handled by tower-http.

pub mod chat;
pub mod routes;
pub mod state;

pub use routes::create_router;
pub use state::AppState;
