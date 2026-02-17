//! Scene graph node types.
//!
//! Each node in the scene graph represents a visual element on the canvas.
//! Nodes carry their visual properties, bounding box, and dirty flags for
//! incremental rendering.

use selean_common::types::NodeId;

use super::DirtyFlags;

/// The visual type of a scene node, determining how it is rendered.
#[derive(Debug, Clone, PartialEq)]
pub enum SceneNodeKind {
    /// A rectangular frame, potentially with fill, stroke, and corner radius.
    Frame {
        /// Corner radius in logical pixels. 0.0 means sharp corners.
        corner_radius: [f32; 4],
    },
    /// A text element with content and basic typography info.
    Text {
        /// The text content to render.
        content: String,
        /// Font size in logical pixels.
        font_size: f32,
    },
    /// A raster image, referenced by asset ID or path.
    Image {
        /// Identifier or path for the image asset.
        asset_ref: String,
    },
    /// A vector shape defined by a path.
    Vector {
        /// SVG-like path data. Full path parsing is deferred to a later milestone.
        path_data: String,
    },
    /// A logical grouping of child nodes with no visual representation of its own.
    Group,
}

/// An axis-aligned bounding box in logical (pre-transform) coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    /// Left edge x coordinate.
    pub x: f32,
    /// Top edge y coordinate.
    pub y: f32,
    /// Width in logical pixels.
    pub width: f32,
    /// Height in logical pixels.
    pub height: f32,
}

impl BoundingBox {
    /// Creates a new bounding box.
    ///
    /// Width and height are clamped to zero if negative.
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width: width.max(0.0),
            height: height.max(0.0),
        }
    }

    /// Returns the right edge (x + width).
    #[must_use]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Returns the bottom edge (y + height).
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Returns `true` if this bounding box intersects with `other`.
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Returns `true` if this bounding box fully contains `other`.
    #[must_use]
    pub fn contains_box(&self, other: &Self) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.right() >= other.right()
            && self.bottom() >= other.bottom()
    }

    /// Returns `true` if the point (px, py) is inside this bounding box.
    #[must_use]
    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }

    /// Returns `true` if the bounding box has zero area.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.width == 0.0 || self.height == 0.0
    }
}

/// RGBA color with linear (non-premultiplied) components in [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red component.
    pub r: f32,
    /// Green component.
    pub g: f32,
    /// Blue component.
    pub b: f32,
    /// Alpha component.
    pub a: f32,
}

impl Color {
    /// Opaque white.
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    /// Opaque black.
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    /// Fully transparent.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Creates a new color, clamping all components to [0.0, 1.0].
    #[must_use]
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
            a: a.clamp(0.0, 1.0),
        }
    }
}

/// A node in the scene graph.
///
/// Holds its identity, visual type, spatial bounds, styling, hierarchy info,
/// and dirty flags for incremental rendering.
#[derive(Debug, Clone)]
pub struct SceneNode {
    /// Unique identifier for this node.
    pub id: NodeId,
    /// Human-readable name (e.g., "Header Frame", "Submit Button").
    pub name: String,
    /// The visual type and type-specific properties.
    pub kind: SceneNodeKind,
    /// Axis-aligned bounding box in logical coordinates.
    pub bounds: BoundingBox,
    /// Fill color. `None` means no fill.
    pub fill: Option<Color>,
    /// Stroke color. `None` means no stroke.
    pub stroke: Option<Color>,
    /// Stroke width in logical pixels.
    pub stroke_width: f32,
    /// Opacity in [0.0, 1.0]. Applied during compositing.
    pub opacity: f32,
    /// Whether this node is visible. Invisible nodes are skipped entirely.
    pub visible: bool,
    /// IDs of child nodes, in render order (back to front).
    pub children: Vec<NodeId>,
    /// ID of the parent node, if any. Root nodes have `None`.
    pub parent: Option<NodeId>,
    /// Dirty flags indicating which properties have changed.
    pub dirty: DirtyFlags,
}

impl SceneNode {
    /// Creates a new scene node with the given ID, name, kind, and bounds.
    ///
    /// The node starts with all dirty flags set (since it's newly created and
    /// needs to be rendered for the first time).
    #[must_use]
    pub fn new(id: NodeId, name: String, kind: SceneNodeKind, bounds: BoundingBox) -> Self {
        Self {
            id,
            name,
            kind,
            bounds,
            fill: None,
            stroke: None,
            stroke_width: 0.0,
            opacity: 1.0,
            visible: true,
            children: Vec::new(),
            parent: None,
            dirty: DirtyFlags::ALL,
        }
    }

    /// Marks the geometry as dirty (position or size changed).
    pub fn mark_geometry_dirty(&mut self) {
        self.dirty |= DirtyFlags::GEOMETRY;
    }

    /// Marks the style as dirty (fill, stroke, opacity changed).
    pub fn mark_style_dirty(&mut self) {
        self.dirty |= DirtyFlags::STYLE;
    }

    /// Marks the children list as dirty (child added, removed, or reordered).
    pub fn mark_children_dirty(&mut self) {
        self.dirty |= DirtyFlags::CHILDREN;
    }

    /// Clears all dirty flags after the node has been rendered.
    pub fn clear_dirty(&mut self) {
        self.dirty = DirtyFlags::NONE;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // Exact float comparisons are intentional in tests with known values.

    use super::*;

    fn test_node() -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            "Test Frame".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(10.0, 20.0, 100.0, 50.0),
        )
    }

    // --- BoundingBox tests ---

    #[test]
    fn bounding_box_edges() {
        let bb = BoundingBox::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(bb.right(), 110.0);
        assert_eq!(bb.bottom(), 70.0);
    }

    #[test]
    fn bounding_box_negative_dimensions_clamped() {
        let bb = BoundingBox::new(0.0, 0.0, -10.0, -5.0);
        assert_eq!(bb.width, 0.0);
        assert_eq!(bb.height, 0.0);
    }

    #[test]
    fn bounding_box_intersects() {
        let a = BoundingBox::new(0.0, 0.0, 100.0, 100.0);
        let b = BoundingBox::new(50.0, 50.0, 100.0, 100.0);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));
    }

    #[test]
    fn bounding_box_no_intersect() {
        let a = BoundingBox::new(0.0, 0.0, 50.0, 50.0);
        let b = BoundingBox::new(100.0, 100.0, 50.0, 50.0);
        assert!(!a.intersects(&b));
        assert!(!b.intersects(&a));
    }

    #[test]
    fn bounding_box_edge_touching_no_intersect() {
        // Touching at edge (right edge of a == left edge of b) should NOT intersect
        // because we use strict inequalities.
        let a = BoundingBox::new(0.0, 0.0, 50.0, 50.0);
        let b = BoundingBox::new(50.0, 0.0, 50.0, 50.0);
        assert!(!a.intersects(&b));
    }

    #[test]
    fn bounding_box_contains_box() {
        let outer = BoundingBox::new(0.0, 0.0, 100.0, 100.0);
        let inner = BoundingBox::new(10.0, 10.0, 30.0, 30.0);
        assert!(outer.contains_box(&inner));
        assert!(!inner.contains_box(&outer));
    }

    #[test]
    fn bounding_box_contains_self() {
        let bb = BoundingBox::new(10.0, 20.0, 30.0, 40.0);
        assert!(bb.contains_box(&bb));
    }

    #[test]
    fn bounding_box_contains_point() {
        let bb = BoundingBox::new(0.0, 0.0, 100.0, 100.0);
        assert!(bb.contains_point(50.0, 50.0));
        assert!(bb.contains_point(0.0, 0.0)); // corners are inclusive
        assert!(bb.contains_point(100.0, 100.0));
        assert!(!bb.contains_point(101.0, 50.0));
        assert!(!bb.contains_point(-1.0, 50.0));
    }

    #[test]
    fn bounding_box_empty() {
        assert!(BoundingBox::new(0.0, 0.0, 0.0, 100.0).is_empty());
        assert!(BoundingBox::new(0.0, 0.0, 100.0, 0.0).is_empty());
        assert!(!BoundingBox::new(0.0, 0.0, 1.0, 1.0).is_empty());
    }

    // --- Color tests ---

    #[test]
    fn color_clamping() {
        let c = Color::new(1.5, -0.5, 0.5, 2.0);
        assert_eq!(c.r, 1.0);
        assert_eq!(c.g, 0.0);
        assert_eq!(c.b, 0.5);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn color_constants() {
        assert_eq!(Color::WHITE.r, 1.0);
        assert_eq!(Color::BLACK.r, 0.0);
        assert_eq!(Color::TRANSPARENT.a, 0.0);
    }

    // --- SceneNode tests ---

    #[test]
    fn new_node_is_fully_dirty() {
        let node = test_node();
        assert!(node.dirty.contains(DirtyFlags::ALL));
    }

    #[test]
    fn clear_dirty_resets_flags() {
        let mut node = test_node();
        node.clear_dirty();
        assert!(node.dirty.is_clean());
    }

    #[test]
    fn mark_geometry_dirty_sets_flag() {
        let mut node = test_node();
        node.clear_dirty();
        node.mark_geometry_dirty();
        assert!(node.dirty.contains(DirtyFlags::GEOMETRY));
        assert!(!node.dirty.contains(DirtyFlags::STYLE));
    }

    #[test]
    fn mark_style_dirty_sets_flag() {
        let mut node = test_node();
        node.clear_dirty();
        node.mark_style_dirty();
        assert!(node.dirty.contains(DirtyFlags::STYLE));
        assert!(!node.dirty.contains(DirtyFlags::GEOMETRY));
    }

    #[test]
    fn mark_children_dirty_sets_flag() {
        let mut node = test_node();
        node.clear_dirty();
        node.mark_children_dirty();
        assert!(node.dirty.contains(DirtyFlags::CHILDREN));
    }

    #[test]
    fn multiple_dirty_marks_combine() {
        let mut node = test_node();
        node.clear_dirty();
        node.mark_geometry_dirty();
        node.mark_style_dirty();
        assert!(node.dirty.contains(DirtyFlags::GEOMETRY));
        assert!(node.dirty.contains(DirtyFlags::STYLE));
    }

    #[test]
    fn default_node_properties() {
        let node = test_node();
        assert!(node.fill.is_none());
        assert!(node.stroke.is_none());
        assert_eq!(node.stroke_width, 0.0);
        assert_eq!(node.opacity, 1.0);
        assert!(node.visible);
        assert!(node.children.is_empty());
        assert!(node.parent.is_none());
    }

    #[test]
    fn node_with_children() {
        let mut parent = test_node();
        let child_id = NodeId::new();
        parent.children.push(child_id);
        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.children[0], child_id);
    }
}
