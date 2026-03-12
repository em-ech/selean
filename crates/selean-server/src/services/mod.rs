//! Service layer for business logic.
//!
//! Extracts business rules from route handlers into testable, reusable
//! service functions. Handlers become thin: parse request, call service,
//! format response.

pub mod billing;
pub mod documents;
pub mod errors;
pub mod workspaces;
