//! Scene graph node types.
//!
//! Each node in the scene graph represents a visual element on the canvas.
//! Nodes carry their visual properties, bounding box, and dirty flags for
//! incremental rendering.

use serde::{Deserialize, Serialize};

use selean_common::types::NodeId;

use super::DirtyFlags;
use super::clip::ClipMode;
use super::transform::Transform2D;

/// Font style for text elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum FontStyle {
    /// Normal (upright) text.
    #[default]
    Normal,
    /// Italic text.
    Italic,
}

impl std::fmt::Display for FontStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Normal => "Normal",
            Self::Italic => "Italic",
        })
    }
}

impl std::str::FromStr for FontStyle {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Normal" => Ok(Self::Normal),
            "Italic" => Ok(Self::Italic),
            _ => Err(format!("unknown font style: {s}")),
        }
    }
}

/// Text alignment for text elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TextAlign {
    /// Left-aligned text.
    #[default]
    Left,
    /// Center-aligned text.
    Center,
    /// Right-aligned text.
    Right,
    /// Justified text.
    Justify,
}

impl std::fmt::Display for TextAlign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Left => "Left",
            Self::Center => "Center",
            Self::Right => "Right",
            Self::Justify => "Justify",
        })
    }
}

impl std::str::FromStr for TextAlign {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Left" => Ok(Self::Left),
            "Center" => Ok(Self::Center),
            "Right" => Ok(Self::Right),
            "Justify" => Ok(Self::Justify),
            _ => Err(format!("unknown text align: {s}")),
        }
    }
}

/// The visual type of a scene node, determining how it is rendered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SceneNodeKind {
    /// A rectangular frame, potentially with fill, stroke, and corner radius.
    Frame {
        /// Corner radius in logical pixels. 0.0 means sharp corners.
        corner_radius: [f32; 4],
    },
    /// A text element with content and typography info.
    Text {
        /// The text content to render.
        content: String,
        /// Font size in logical pixels.
        font_size: f32,
        /// Font family name.
        #[serde(default = "default_font_family")]
        font_family: String,
        /// Font weight (100-900).
        #[serde(default = "default_font_weight")]
        font_weight: u16,
        /// Font style (Normal, Italic).
        #[serde(default)]
        font_style: FontStyle,
        /// Text alignment.
        #[serde(default)]
        text_align: TextAlign,
        /// Line height multiplier.
        #[serde(default = "default_line_height")]
        line_height: f32,
        /// Text-specific color, overrides node fill when set.
        #[serde(default)]
        text_color: Option<Color>,
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

impl SceneNodeKind {
    /// Returns the kind as a static string tag.
    #[must_use]
    pub fn kind_tag(&self) -> &'static str {
        match self {
            Self::Frame { .. } => "Frame",
            Self::Text { .. } => "Text",
            Self::Image { .. } => "Image",
            Self::Vector { .. } => "Vector",
            Self::Group => "Group",
        }
    }
}

/// An axis-aligned bounding box in logical (pre-transform) coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

    /// Returns the smallest AABB enclosing both bounding boxes.
    ///
    /// If either box is empty, returns the other. If both are empty, returns
    /// an empty box at the origin.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let min_x = self.x.min(other.x);
        let min_y = self.y.min(other.y);
        let max_x = self.right().max(other.right());
        let max_y = self.bottom().max(other.bottom());
        Self::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }
}

/// RGBA color with linear (non-premultiplied) components in [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

/// Compositing blend mode for a scene node.
///
/// Determines how the node's pixels are composited with the destination (background).
/// Only `Normal` and `Add` can be rendered with a single wgpu blend state.
/// Other modes require multi-pass rendering with intermediate render targets
/// and are defined here for API completeness; they fall back to `Normal` with a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BlendMode {
    /// Standard alpha blending (source-over).
    #[default]
    Normal,
    /// Additive blending: source color is added to destination.
    Add,
    // --- Future modes (require multi-pass rendering) ---
    /// Multiply: darkens by multiplying source and destination.
    Multiply,
    /// Screen: lightens by inverting, multiplying, and inverting again.
    Screen,
    /// Overlay: combines Multiply and Screen based on destination luminance.
    Overlay,
    /// Darken: keeps the darker of source and destination per channel.
    Darken,
    /// Lighten: keeps the lighter of source and destination per channel.
    Lighten,
    /// Color Dodge: brightens destination to reflect source.
    ColorDodge,
    /// Color Burn: darkens destination to reflect source.
    ColorBurn,
    /// Hard Light: combines Multiply and Screen based on source luminance.
    HardLight,
    /// Soft Light: similar to Hard Light but softer.
    SoftLight,
    /// Difference: absolute difference between source and destination.
    Difference,
    /// Exclusion: similar to Difference but lower contrast.
    Exclusion,
}

impl BlendMode {
    /// Returns `true` if this blend mode can be rendered natively with a wgpu blend state
    /// (no multi-pass rendering required).
    #[must_use]
    pub fn is_native(&self) -> bool {
        matches!(self, Self::Normal | Self::Add)
    }

    /// Returns the 0-based shader index for non-native blend modes.
    ///
    /// Used by the blend composite shader to dispatch to the correct formula.
    /// Native modes (Normal, Add) return `u32::MAX` as a sentinel (never used).
    #[must_use]
    pub fn shader_index(&self) -> u32 {
        match self {
            Self::Multiply => 0,
            Self::Screen => 1,
            Self::Overlay => 2,
            Self::Darken => 3,
            Self::Lighten => 4,
            Self::ColorDodge => 5,
            Self::ColorBurn => 6,
            Self::HardLight => 7,
            Self::SoftLight => 8,
            Self::Difference => 9,
            Self::Exclusion => 10,
            Self::Normal | Self::Add => u32::MAX,
        }
    }
}

impl std::fmt::Display for BlendMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Normal => "Normal",
            Self::Add => "Add",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
            Self::Darken => "Darken",
            Self::Lighten => "Lighten",
            Self::ColorDodge => "ColorDodge",
            Self::ColorBurn => "ColorBurn",
            Self::HardLight => "HardLight",
            Self::SoftLight => "SoftLight",
            Self::Difference => "Difference",
            Self::Exclusion => "Exclusion",
        })
    }
}

impl std::str::FromStr for BlendMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Normal" => Ok(Self::Normal),
            "Add" => Ok(Self::Add),
            "Multiply" => Ok(Self::Multiply),
            "Screen" => Ok(Self::Screen),
            "Overlay" => Ok(Self::Overlay),
            "Darken" => Ok(Self::Darken),
            "Lighten" => Ok(Self::Lighten),
            "ColorDodge" => Ok(Self::ColorDodge),
            "ColorBurn" => Ok(Self::ColorBurn),
            "HardLight" => Ok(Self::HardLight),
            "SoftLight" => Ok(Self::SoftLight),
            "Difference" => Ok(Self::Difference),
            "Exclusion" => Ok(Self::Exclusion),
            _ => Err(format!("unknown blend mode: {s}")),
        }
    }
}

fn dirty_all() -> DirtyFlags {
    DirtyFlags::ALL
}

fn identity_transform() -> Transform2D {
    Transform2D::identity()
}

fn default_font_family() -> String {
    "Inter".to_string()
}

fn default_font_weight() -> u16 {
    400
}

fn default_line_height() -> f32 {
    1.2
}

/// A node in the scene graph.
///
/// Holds its identity, visual type, spatial bounds, styling, hierarchy info,
/// and dirty flags for incremental rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Local transform (user-set). Applied relative to the node's position.
    pub local_transform: Transform2D,
    /// Cached world transform (`parent.world_transform * local_transform`).
    /// Recomputed lazily during the prepare phase when `TRANSFORM` is dirty.
    #[serde(skip, default = "identity_transform")]
    pub world_transform: Transform2D,
    /// Compositing blend mode.
    pub blend_mode: BlendMode,
    /// Clipping mode. When not `None`, children are clipped to this node's bounds.
    pub clip_mode: ClipMode,
    /// Scroll offset applied to children. Children are translated by
    /// `(-scroll_offset[0], -scroll_offset[1])` in the node's coordinate space.
    /// The node itself renders at its normal position.
    pub scroll_offset: [f32; 2],
    /// IDs of child nodes, in render order (back to front).
    pub children: Vec<NodeId>,
    /// ID of the parent node, if any. Root nodes have `None`.
    pub parent: Option<NodeId>,
    /// Dirty flags indicating which properties have changed.
    #[serde(skip, default = "dirty_all")]
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
            local_transform: Transform2D::identity(),
            world_transform: Transform2D::identity(),
            blend_mode: BlendMode::Normal,
            clip_mode: ClipMode::None,
            scroll_offset: [0.0, 0.0],
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
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

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

    // --- BoundingBox::union tests ---

    #[test]
    fn bounding_box_union_overlapping() {
        let a = BoundingBox::new(0.0, 0.0, 100.0, 100.0);
        let b = BoundingBox::new(50.0, 50.0, 100.0, 100.0);
        let u = a.union(&b);
        assert_eq!(u.x, 0.0);
        assert_eq!(u.y, 0.0);
        assert_eq!(u.width, 150.0);
        assert_eq!(u.height, 150.0);
    }

    #[test]
    fn bounding_box_union_disjoint() {
        let a = BoundingBox::new(0.0, 0.0, 50.0, 50.0);
        let b = BoundingBox::new(200.0, 200.0, 30.0, 30.0);
        let u = a.union(&b);
        assert_eq!(u.x, 0.0);
        assert_eq!(u.y, 0.0);
        assert_eq!(u.width, 230.0);
        assert_eq!(u.height, 230.0);
    }

    #[test]
    fn bounding_box_union_contained() {
        let outer = BoundingBox::new(0.0, 0.0, 200.0, 200.0);
        let inner = BoundingBox::new(50.0, 50.0, 30.0, 30.0);
        let u = outer.union(&inner);
        assert_eq!(u.x, 0.0);
        assert_eq!(u.y, 0.0);
        assert_eq!(u.width, 200.0);
        assert_eq!(u.height, 200.0);
    }

    #[test]
    fn bounding_box_union_empty_left() {
        let empty = BoundingBox::new(0.0, 0.0, 0.0, 0.0);
        let b = BoundingBox::new(10.0, 20.0, 50.0, 60.0);
        let u = empty.union(&b);
        assert_eq!(u, b);
    }

    #[test]
    fn bounding_box_union_empty_right() {
        let a = BoundingBox::new(10.0, 20.0, 50.0, 60.0);
        let empty = BoundingBox::new(0.0, 0.0, 0.0, 0.0);
        let u = a.union(&empty);
        assert_eq!(u, a);
    }

    #[test]
    fn bounding_box_union_both_empty() {
        let a = BoundingBox::new(0.0, 0.0, 0.0, 0.0);
        let b = BoundingBox::new(0.0, 0.0, 0.0, 0.0);
        let u = a.union(&b);
        assert!(u.is_empty());
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
        assert!(node.local_transform.is_identity());
        assert!(node.world_transform.is_identity());
        assert_eq!(node.blend_mode, BlendMode::Normal);
        assert_eq!(node.clip_mode, ClipMode::None);
        assert_eq!(node.scroll_offset, [0.0, 0.0]);
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

    // --- SceneNodeKind::kind_tag tests ---

    #[test]
    fn kind_tag_frame() {
        let kind = SceneNodeKind::Frame {
            corner_radius: [0.0; 4],
        };
        assert_eq!(kind.kind_tag(), "Frame");
    }

    #[test]
    fn kind_tag_text() {
        let kind = SceneNodeKind::Text {
            content: String::new(),
            font_size: 16.0,
            font_family: "Inter".to_string(),
            font_weight: 400,
            font_style: FontStyle::Normal,
            text_align: TextAlign::Left,
            line_height: 1.2,
            text_color: None,
        };
        assert_eq!(kind.kind_tag(), "Text");
    }

    // --- FontStyle tests ---

    #[test]
    fn font_style_display_roundtrip() {
        for &style in &[FontStyle::Normal, FontStyle::Italic] {
            let s = style.to_string();
            let back: FontStyle = s.parse().unwrap();
            assert_eq!(back, style);
        }
    }

    #[test]
    fn font_style_from_str_invalid() {
        let result: Result<FontStyle, _> = "Bold".parse();
        assert!(result.is_err());
        assert!(result.err().unwrap().contains("unknown font style"));
    }

    #[test]
    fn font_style_default_is_normal() {
        assert_eq!(FontStyle::default(), FontStyle::Normal);
    }

    // --- TextAlign tests ---

    #[test]
    fn text_align_display_roundtrip() {
        for &align in &[
            TextAlign::Left,
            TextAlign::Center,
            TextAlign::Right,
            TextAlign::Justify,
        ] {
            let s = align.to_string();
            let back: TextAlign = s.parse().unwrap();
            assert_eq!(back, align);
        }
    }

    #[test]
    fn text_align_from_str_invalid() {
        let result: Result<TextAlign, _> = "Start".parse();
        assert!(result.is_err());
        assert!(result.err().unwrap().contains("unknown text align"));
    }

    #[test]
    fn text_align_default_is_left() {
        assert_eq!(TextAlign::default(), TextAlign::Left);
    }

    #[test]
    fn kind_tag_image() {
        let kind = SceneNodeKind::Image {
            asset_ref: String::new(),
        };
        assert_eq!(kind.kind_tag(), "Image");
    }

    #[test]
    fn kind_tag_vector() {
        let kind = SceneNodeKind::Vector {
            path_data: String::new(),
        };
        assert_eq!(kind.kind_tag(), "Vector");
    }

    #[test]
    fn kind_tag_group() {
        assert_eq!(SceneNodeKind::Group.kind_tag(), "Group");
    }

    // --- BlendMode::FromStr tests ---

    #[test]
    fn blend_mode_from_str_all_variants() {
        let modes = [
            "Normal",
            "Add",
            "Multiply",
            "Screen",
            "Overlay",
            "Darken",
            "Lighten",
            "ColorDodge",
            "ColorBurn",
            "HardLight",
            "SoftLight",
            "Difference",
            "Exclusion",
        ];
        for mode_str in &modes {
            let parsed: BlendMode = mode_str.parse().unwrap();
            assert_eq!(parsed.to_string(), *mode_str);
        }
    }

    #[test]
    fn blend_mode_from_str_invalid() {
        let result: Result<BlendMode, _> = "NotAMode".parse();
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.contains("unknown blend mode"));
    }

    #[test]
    fn blend_mode_display_roundtrip() {
        let mode = BlendMode::ColorDodge;
        let s = mode.to_string();
        let back: BlendMode = s.parse().unwrap();
        assert_eq!(back, mode);
    }
}
