//! Authentication and authorization module.
//!
//! Supports two auth modes:
//! 1. **Legacy shared secret**: `SELEAN_AUTH_SECRET` env var (backward compatible)
//! 2. **JWT-based auth**: `JWT_SECRET` env var with user accounts via database
//!
//! When both are configured, JWT takes precedence. When neither is set,
//! auth is disabled (open access for local dev).

pub mod config;
pub mod jwt;
pub mod middleware;
pub mod password;
pub mod routes;

pub use config::AuthConfig;
pub use jwt::JwtConfig;
pub use middleware::auth_middleware;
