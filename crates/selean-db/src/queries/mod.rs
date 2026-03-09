//! Database query modules.
//!
//! Each module provides CRUD operations for its corresponding model,
//! using `sqlx` compile-time checked queries.

pub mod assets;
pub mod documents;
pub mod sessions;
pub mod users;
pub mod workspaces;
