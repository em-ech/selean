//! R-tree spatial index for viewport virtualization and hit testing.
//!
//! Stores axis-aligned bounding boxes for all scene nodes and enables
//! efficient spatial queries:
//! - **Viewport culling:** Which nodes are visible on screen?
//! - **Hit testing:** Which node is under the cursor?
//! - **Region queries:** Which nodes intersect an arbitrary rectangle?

mod index;

pub use index::{SpatialEntry, SpatialIndex};
