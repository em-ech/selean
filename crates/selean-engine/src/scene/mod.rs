//! Scene graph with dirty flag tracking.
//!
//! The scene graph represents the design node tree with efficient change tracking
//! via dirty flag bitmasks. Only nodes with non-zero dirty flags are processed
//! during rendering.

mod dirty;
mod node;
mod store;
pub mod transform;

pub use dirty::DirtyFlags;
pub use node::{BlendMode, BoundingBox, Color, SceneNode, SceneNodeKind};
pub use store::SceneGraph;
pub use transform::{Transform2D, TransformColumns};
