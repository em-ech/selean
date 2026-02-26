//! Re-exports from `selean-engine::command::descriptor`.
//!
//! `CommandDescriptor` and `create_frame_node` now live in `selean-engine`.
//! This module re-exports them for backward compatibility within this crate.

pub use selean_engine::command::{CommandDescriptor, create_frame_node};
