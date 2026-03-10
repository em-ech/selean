//! Database model types.
//!
//! Each model maps directly to a database table row. Models use `sqlx::FromRow`
//! for automatic deserialization from query results.

pub mod asset;
pub mod billing;
pub mod document;
pub mod session;
pub mod user;
pub mod workspace;
