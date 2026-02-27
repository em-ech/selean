//! Page model for multi-page documents.
//!
//! Each page contains its own scene graph with independent node hierarchies,
//! spatial index, and transform state.

use selean_common::types::PageId;

use crate::scene::SceneGraph;

/// A single page in a document.
///
/// Each page has its own scene graph, dimensions, and display name.
/// Pages are analogous to slides in presentation software or pages in
/// design tools like Figma.
#[derive(Clone)]
pub struct Page {
    /// Unique identifier for this page.
    pub id: PageId,
    /// Human-readable page name.
    pub name: String,
    /// Page width in logical pixels.
    pub width: f32,
    /// Page height in logical pixels.
    pub height: f32,
    /// The scene graph for this page.
    pub scene: SceneGraph,
}

impl Page {
    /// Creates a new empty page with the given dimensions.
    #[must_use]
    pub fn new(name: impl Into<String>, width: f32, height: f32) -> Self {
        Self {
            id: PageId::new(),
            name: name.into(),
            width,
            height,
            scene: SceneGraph::new(),
        }
    }

    /// Creates a new page with a pre-built scene graph.
    #[must_use]
    pub fn with_scene(
        id: PageId,
        name: impl Into<String>,
        width: f32,
        height: f32,
        scene: SceneGraph,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            width,
            height,
            scene,
        }
    }
}
