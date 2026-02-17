//! Scene graph with dirty flag tracking.
//!
//! The scene graph represents the design node tree with efficient change tracking
//! via dirty flag bitmasks. Only nodes with non-zero dirty flags are processed
//! during rendering.

mod dirty;
mod node;

pub use dirty::DirtyFlags;
pub use node::{SceneNode, SceneNodeKind};
