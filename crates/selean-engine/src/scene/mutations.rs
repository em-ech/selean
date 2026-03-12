//! Property mutation methods for `SceneGraph`.
//!
//! These methods auto-sync the spatial index and propagate dirty flags up the
//! ancestor chain. Extracted from `store.rs` for code organization.

use selean_common::types::NodeId;

use super::clip::ClipMode;
use super::dirty::DirtyFlags;
use super::node::{
    BlendMode, BoundingBox, Color, Effect, FontStyle, Gradient, SceneNodeKind, TextAlign,
};
use super::store::SceneGraph;
use super::transform::Transform2D;

impl SceneGraph {
    // --- Property mutations (auto-sync spatial index + dirty propagation) ---

    /// Updates a node's bounding box.
    ///
    /// Syncs the spatial index (using the cached world transform for AABB) and
    /// marks the node with `DIRTY_GEOMETRY`. Returns `false` if the node doesn't exist.
    pub fn set_bounds(&mut self, id: NodeId, bounds: BoundingBox) -> bool {
        let found = self.mutate_node(id, DirtyFlags::GEOMETRY, |node| {
            node.bounds = bounds;
            true
        });
        if found {
            // Use the cached world transform to compute the spatial AABB.
            // If the transform is also dirty this frame, recompute_world_transforms()
            // will correct the spatial AABB during the prepare phase.
            if let Some(node) = self.nodes().get(&id) {
                let world_aabb = node.world_transform.transform_aabb(&bounds);
                self.sync_spatial(id, &world_aabb);
            }
        }
        found
    }

    /// Updates a node's fill color.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_fill(&mut self, id: NodeId, fill: Option<Color>) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.fill = fill;
            true
        })
    }

    /// Updates a node's gradient fill.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_fill_gradient(&mut self, id: NodeId, fill_gradient: Option<Gradient>) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.fill_gradient = fill_gradient;
            true
        })
    }

    /// Updates a node's visual effects list.
    ///
    /// Marks the node with `DIRTY_EFFECTS`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_effects(&mut self, id: NodeId, effects: Vec<Effect>) -> bool {
        self.mutate_node(id, DirtyFlags::EFFECTS, |node| {
            node.effects = effects;
            true
        })
    }

    /// Updates a node's stroke color.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_stroke(&mut self, id: NodeId, stroke: Option<Color>) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.stroke = stroke;
            true
        })
    }

    /// Updates a node's stroke width.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_stroke_width(&mut self, id: NodeId, width: f32) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.stroke_width = width;
            true
        })
    }

    /// Updates a node's opacity.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_opacity(&mut self, id: NodeId, opacity: f32) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.opacity = opacity.clamp(0.0, 1.0);
            true
        })
    }

    /// Updates a node's visibility.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist.
    pub fn set_visible(&mut self, id: NodeId, visible: bool) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.visible = visible;
            true
        })
    }

    /// Updates a node's name.
    ///
    /// Does not set any dirty flags (name is not a visual property).
    /// Returns `false` if the node doesn't exist.
    pub fn set_name(&mut self, id: NodeId, name: String) -> bool {
        if let Some(node) = self.get_mut(id) {
            node.name = name;
            true
        } else {
            false
        }
    }

    /// Updates the text content of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_text_content(&mut self, id: NodeId, content: String) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                content: ref mut c, ..
            } = node.kind
            {
                *c = content;
                true
            } else {
                false
            }
        })
    }

    /// Updates the font size of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_font_size(&mut self, id: NodeId, font_size: f32) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                font_size: ref mut fs,
                ..
            } = node.kind
            {
                *fs = font_size;
                true
            } else {
                false
            }
        })
    }

    /// Updates the corner radius of a Frame node.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist or is not a Frame node.
    pub fn set_corner_radius(&mut self, id: NodeId, corner_radius: [f32; 4]) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            if let SceneNodeKind::Frame {
                corner_radius: ref mut cr,
            } = node.kind
            {
                *cr = corner_radius;
                true
            } else {
                false
            }
        })
    }

    /// Updates the font family of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_font_family(&mut self, id: NodeId, font_family: String) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                font_family: ref mut ff,
                ..
            } = node.kind
            {
                *ff = font_family;
                true
            } else {
                false
            }
        })
    }

    /// Updates the font weight of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_font_weight(&mut self, id: NodeId, font_weight: u16) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                font_weight: ref mut fw,
                ..
            } = node.kind
            {
                *fw = font_weight;
                true
            } else {
                false
            }
        })
    }

    /// Updates the font style of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_font_style(&mut self, id: NodeId, font_style: FontStyle) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                font_style: ref mut fs,
                ..
            } = node.kind
            {
                *fs = font_style;
                true
            } else {
                false
            }
        })
    }

    /// Updates the text alignment of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_text_align(&mut self, id: NodeId, text_align: TextAlign) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                text_align: ref mut ta,
                ..
            } = node.kind
            {
                *ta = text_align;
                true
            } else {
                false
            }
        })
    }

    /// Updates the line height of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_line_height(&mut self, id: NodeId, line_height: f32) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                line_height: ref mut lh,
                ..
            } = node.kind
            {
                *lh = line_height;
                true
            } else {
                false
            }
        })
    }

    /// Updates the text color of a Text node.
    ///
    /// Marks the node with `DIRTY_TEXT`.
    /// Returns `false` if the node doesn't exist or is not a Text node.
    pub fn set_text_color(&mut self, id: NodeId, text_color: Option<Color>) -> bool {
        self.mutate_node(id, DirtyFlags::TEXT, |node| {
            if let SceneNodeKind::Text {
                text_color: ref mut tc,
                ..
            } = node.kind
            {
                *tc = text_color;
                true
            } else {
                false
            }
        })
    }

    /// Updates the path data of a Vector node.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist or is not a Vector node.
    pub fn set_path_data(&mut self, id: NodeId, new_path_data: String) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            if let SceneNodeKind::Vector {
                path_data: ref mut pd,
            } = node.kind
            {
                *pd = new_path_data;
                true
            } else {
                false
            }
        })
    }

    /// Updates the asset reference of an Image node.
    ///
    /// Marks the node with `DIRTY_STYLE`.
    /// Returns `false` if the node doesn't exist or is not an Image node.
    pub fn set_asset_ref(&mut self, id: NodeId, new_asset_ref: String) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            if let SceneNodeKind::Image {
                asset_ref: ref mut ar,
            } = node.kind
            {
                *ar = new_asset_ref;
                true
            } else {
                false
            }
        })
    }

    // --- Transform mutations ---

    /// Sets the local transform for a node.
    ///
    /// Marks the node and all descendants with `TRANSFORM` dirty.
    /// The world transform is recomputed lazily during the prepare phase.
    /// Returns `false` if the node doesn't exist.
    pub fn set_transform(&mut self, id: NodeId, transform: Transform2D) -> bool {
        if let Some(node) = self.get_mut(id) {
            node.local_transform = transform;
            node.dirty |= DirtyFlags::TRANSFORM;
        } else {
            return false;
        }

        self.propagate_transform_dirty_down(id);
        self.propagate_dirty_up_from_child(id);
        true
    }

    /// Sets the local transform to a pure rotation around the node's center.
    ///
    /// This **replaces** the entire local transform (it does not compose with
    /// any existing scale or skew). Use [`set_transform`] for composed transforms.
    ///
    /// Returns `false` if the node doesn't exist.
    pub fn set_rotation(&mut self, id: NodeId, angle_rad: f32) -> bool {
        let Some(node) = self.get(id) else {
            return false;
        };
        let cx = node.bounds.x + node.bounds.width * 0.5;
        let cy = node.bounds.y + node.bounds.height * 0.5;
        let transform = Transform2D::from_rotation_around(angle_rad, cx, cy);
        self.set_transform(id, transform)
    }

    /// Sets the local transform to a pure scale around the node's center.
    ///
    /// This **replaces** the entire local transform (it does not compose with
    /// any existing rotation or skew). Use [`set_transform`] for composed transforms.
    ///
    /// Returns `false` if the node doesn't exist.
    pub fn set_scale(&mut self, id: NodeId, sx: f32, sy: f32) -> bool {
        let Some(node) = self.get(id) else {
            return false;
        };
        let cx = node.bounds.x + node.bounds.width * 0.5;
        let cy = node.bounds.y + node.bounds.height * 0.5;
        let transform = Transform2D::from_scale_around(sx, sy, cx, cy);
        self.set_transform(id, transform)
    }

    /// Sets the blend mode for a node.
    ///
    /// Marks the node with `STYLE` dirty.
    /// Returns `false` if the node doesn't exist.
    pub fn set_blend_mode(&mut self, id: NodeId, mode: BlendMode) -> bool {
        self.mutate_node(id, DirtyFlags::STYLE, |node| {
            node.blend_mode = mode;
            true
        })
    }

    /// Sets the clip mode for a node.
    ///
    /// When set to a mode other than `None`, all descendants are clipped to
    /// this node's bounds. Marks the node with `CLIP` dirty and propagates
    /// `CHILDREN` up to ancestors.
    /// Returns `false` if the node doesn't exist.
    pub fn set_clip_mode(&mut self, id: NodeId, mode: ClipMode) -> bool {
        self.mutate_node(id, DirtyFlags::CLIP, |node| {
            node.clip_mode = mode;
            true
        })
    }

    // --- Scroll offset mutations ---

    /// Sets the scroll offset for a node.
    ///
    /// Children of this node will be translated by `(-x, -y)` in the node's
    /// coordinate space. The node itself renders at its normal position.
    /// Marks the node and all descendants with `TRANSFORM` dirty (same as
    /// `set_transform`) and propagates `CHILDREN` up.
    /// Returns `false` if the node does not exist.
    pub fn set_scroll_offset(&mut self, id: NodeId, x: f32, y: f32) -> bool {
        if let Some(node) = self.get_mut(id) {
            node.scroll_offset = [x, y];
            node.dirty |= DirtyFlags::TRANSFORM;
        } else {
            return false;
        }

        self.propagate_transform_dirty_down(id);
        self.propagate_dirty_up_from_child(id);
        true
    }

    /// Sets the scroll offset, clamped to `[0..max_x, 0..max_y]`.
    ///
    /// Computes `max_scroll` and clamps the requested offset. Negative values
    /// are clamped to zero. Returns `false` if the node does not exist.
    pub fn set_scroll_offset_clamped(&mut self, id: NodeId, x: f32, y: f32) -> bool {
        let Some(max) = self.max_scroll(id) else {
            return false;
        };
        let clamped_x = x.clamp(0.0, max[0]);
        let clamped_y = y.clamp(0.0, max[1]);
        self.set_scroll_offset(id, clamped_x, clamped_y)
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::expect_used)]
mod tests {
    use selean_common::types::NodeId;

    use crate::scene::clip::ClipMode;
    use crate::scene::dirty::DirtyFlags;
    use crate::scene::node::{
        BlendMode, BoundingBox, Color, Effect, FontStyle, SceneNode, SceneNodeKind, TextAlign,
    };
    use crate::scene::store::SceneGraph;
    use crate::scene::transform::Transform2D;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_frame(name: &str) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        )
    }

    fn make_text(name: &str, content: &str, font_size: f32) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Text {
                content: content.to_string(),
                font_size,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(0.0, 0.0, 200.0, 50.0),
        )
    }

    fn make_vector(name: &str, path_data: &str) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Vector {
                path_data: path_data.to_string(),
            },
            BoundingBox::new(0.0, 0.0, 50.0, 50.0),
        )
    }

    fn make_image(name: &str, asset_ref: &str) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Image {
                asset_ref: asset_ref.to_string(),
            },
            BoundingBox::new(0.0, 0.0, 80.0, 80.0),
        )
    }

    fn bogus_id() -> NodeId {
        NodeId::new()
    }

    // -----------------------------------------------------------------------
    // 1. Happy-path set_* on valid nodes (returns true)
    // -----------------------------------------------------------------------

    #[test]
    fn set_bounds_valid_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        let new_bounds = BoundingBox::new(10.0, 20.0, 300.0, 400.0);
        assert!(sg.set_bounds(id, new_bounds));
        assert_eq!(sg.get(id).expect("node").bounds, new_bounds);
    }

    #[test]
    fn set_fill_valid_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        let color = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        assert!(sg.set_fill(id, color));
        assert_eq!(sg.get(id).expect("node").fill, color);
    }

    #[test]
    fn set_stroke_and_stroke_width_valid() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        assert!(sg.set_stroke(id, Some(Color::new(0.0, 1.0, 0.0, 1.0))));
        assert!(sg.set_stroke_width(id, 3.5));
        assert_eq!(sg.get(id).expect("node").stroke_width, 3.5);
    }

    #[test]
    fn set_opacity_valid_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        assert!(sg.set_opacity(id, 0.5));
        assert_eq!(sg.get(id).expect("node").opacity, 0.5);
    }

    #[test]
    fn set_visible_valid_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        assert!(sg.set_visible(id, false));
        assert!(!sg.get(id).expect("node").visible);
    }

    #[test]
    fn set_name_valid_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("old"));
        assert!(sg.set_name(id, "new".to_string()));
        assert_eq!(sg.get(id).expect("node").name, "new");
    }

    #[test]
    fn set_text_content_valid_text_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_text("t", "hello", 16.0));
        assert!(sg.set_text_content(id, "world".to_string()));
        if let SceneNodeKind::Text { content, .. } = &sg.get(id).expect("node").kind {
            assert_eq!(content, "world");
        } else {
            panic!("expected Text node");
        }
    }

    #[test]
    fn set_font_size_valid_text_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_text("t", "hi", 12.0));
        assert!(sg.set_font_size(id, 24.0));
        if let SceneNodeKind::Text { font_size, .. } = &sg.get(id).expect("node").kind {
            assert_eq!(*font_size, 24.0);
        } else {
            panic!("expected Text node");
        }
    }

    #[test]
    fn set_font_family_valid_text_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_text("t", "hi", 12.0));
        assert!(sg.set_font_family(id, "Roboto".to_string()));
        if let SceneNodeKind::Text { font_family, .. } = &sg.get(id).expect("node").kind {
            assert_eq!(font_family, "Roboto");
        } else {
            panic!("expected Text node");
        }
    }

    #[test]
    fn set_font_weight_valid_text_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_text("t", "hi", 12.0));
        assert!(sg.set_font_weight(id, 700));
        if let SceneNodeKind::Text { font_weight, .. } = &sg.get(id).expect("node").kind {
            assert_eq!(*font_weight, 700);
        } else {
            panic!("expected Text node");
        }
    }

    #[test]
    fn set_font_style_valid_text_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_text("t", "hi", 12.0));
        assert!(sg.set_font_style(id, FontStyle::Italic));
        if let SceneNodeKind::Text { font_style, .. } = &sg.get(id).expect("node").kind {
            assert_eq!(*font_style, FontStyle::Italic);
        } else {
            panic!("expected Text node");
        }
    }

    #[test]
    fn set_text_align_valid_text_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_text("t", "hi", 12.0));
        assert!(sg.set_text_align(id, TextAlign::Center));
        if let SceneNodeKind::Text { text_align, .. } = &sg.get(id).expect("node").kind {
            assert_eq!(*text_align, TextAlign::Center);
        } else {
            panic!("expected Text node");
        }
    }

    #[test]
    fn set_line_height_valid_text_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_text("t", "hi", 12.0));
        assert!(sg.set_line_height(id, 1.8));
        if let SceneNodeKind::Text { line_height, .. } = &sg.get(id).expect("node").kind {
            assert_eq!(*line_height, 1.8);
        } else {
            panic!("expected Text node");
        }
    }

    #[test]
    fn set_text_color_valid_text_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_text("t", "hi", 12.0));
        let color = Some(Color::new(0.0, 0.0, 1.0, 1.0));
        assert!(sg.set_text_color(id, color));
        if let SceneNodeKind::Text { text_color, .. } = &sg.get(id).expect("node").kind {
            assert_eq!(*text_color, color);
        } else {
            panic!("expected Text node");
        }
    }

    #[test]
    fn set_corner_radius_valid_frame() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        assert!(sg.set_corner_radius(id, [8.0, 8.0, 0.0, 0.0]));
        if let SceneNodeKind::Frame { corner_radius } = &sg.get(id).expect("node").kind {
            assert_eq!(*corner_radius, [8.0, 8.0, 0.0, 0.0]);
        } else {
            panic!("expected Frame node");
        }
    }

    #[test]
    fn set_path_data_valid_vector() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_vector("v", "M0 0"));
        assert!(sg.set_path_data(id, "M10 10 L20 20".to_string()));
        if let SceneNodeKind::Vector { path_data } = &sg.get(id).expect("node").kind {
            assert_eq!(path_data, "M10 10 L20 20");
        } else {
            panic!("expected Vector node");
        }
    }

    #[test]
    fn set_asset_ref_valid_image() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_image("i", "old.png"));
        assert!(sg.set_asset_ref(id, "new.png".to_string()));
        if let SceneNodeKind::Image { asset_ref } = &sg.get(id).expect("node").kind {
            assert_eq!(asset_ref, "new.png");
        } else {
            panic!("expected Image node");
        }
    }

    #[test]
    fn set_blend_mode_valid_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        assert!(sg.set_blend_mode(id, BlendMode::Add));
        assert_eq!(sg.get(id).expect("node").blend_mode, BlendMode::Add);
    }

    #[test]
    fn set_clip_mode_valid_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        assert!(sg.set_clip_mode(id, ClipMode::Scissor));
        assert_eq!(sg.get(id).expect("node").clip_mode, ClipMode::Scissor);
    }

    #[test]
    fn set_effects_valid_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        let effects = vec![Effect::Blur { radius: 4.0 }];
        assert!(sg.set_effects(id, effects.clone()));
        assert_eq!(sg.get(id).expect("node").effects, effects);
    }

    #[test]
    fn set_fill_gradient_valid_node() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        assert!(sg.set_fill_gradient(id, None));
        assert!(sg.get(id).expect("node").fill_gradient.is_none());
    }

    // -----------------------------------------------------------------------
    // 2. set_* on non-existent node (returns false)
    // -----------------------------------------------------------------------

    #[test]
    fn set_bounds_nonexistent() {
        let mut sg = SceneGraph::new();
        assert!(!sg.set_bounds(bogus_id(), BoundingBox::new(0.0, 0.0, 1.0, 1.0)));
    }

    #[test]
    fn set_fill_nonexistent() {
        let mut sg = SceneGraph::new();
        assert!(!sg.set_fill(bogus_id(), None));
    }

    #[test]
    fn set_text_content_nonexistent() {
        let mut sg = SceneGraph::new();
        assert!(!sg.set_text_content(bogus_id(), "x".to_string()));
    }

    #[test]
    fn set_transform_nonexistent() {
        let mut sg = SceneGraph::new();
        assert!(!sg.set_transform(bogus_id(), Transform2D::identity()));
    }

    #[test]
    fn set_rotation_nonexistent() {
        let mut sg = SceneGraph::new();
        assert!(!sg.set_rotation(bogus_id(), 1.0));
    }

    #[test]
    fn set_scale_nonexistent() {
        let mut sg = SceneGraph::new();
        assert!(!sg.set_scale(bogus_id(), 2.0, 2.0));
    }

    #[test]
    fn set_name_nonexistent() {
        let mut sg = SceneGraph::new();
        assert!(!sg.set_name(bogus_id(), "x".to_string()));
    }

    #[test]
    fn set_scroll_offset_nonexistent() {
        let mut sg = SceneGraph::new();
        assert!(!sg.set_scroll_offset(bogus_id(), 10.0, 10.0));
    }

    #[test]
    fn set_scroll_offset_clamped_nonexistent() {
        let mut sg = SceneGraph::new();
        assert!(!sg.set_scroll_offset_clamped(bogus_id(), 10.0, 10.0));
    }

    // -----------------------------------------------------------------------
    // 3. Boundary values
    // -----------------------------------------------------------------------

    #[test]
    fn set_opacity_clamps_to_0_1() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));

        sg.set_opacity(id, -0.5);
        assert_eq!(sg.get(id).expect("node").opacity, 0.0);

        sg.set_opacity(id, 2.0);
        assert_eq!(sg.get(id).expect("node").opacity, 1.0);
    }

    #[test]
    fn set_font_size_negative_stored_as_is() {
        // mutations.rs does not clamp font_size; the value is stored raw.
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_text("t", "hi", 12.0));
        assert!(sg.set_font_size(id, -5.0));
        if let SceneNodeKind::Text { font_size, .. } = &sg.get(id).expect("node").kind {
            assert_eq!(*font_size, -5.0);
        } else {
            panic!("expected Text node");
        }
    }

    #[test]
    fn set_stroke_width_zero() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        assert!(sg.set_stroke_width(id, 0.0));
        assert_eq!(sg.get(id).expect("node").stroke_width, 0.0);
    }

    // -----------------------------------------------------------------------
    // 4. Type-specific mutations on wrong node types
    // -----------------------------------------------------------------------

    #[test]
    fn set_text_content_on_frame_returns_false() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        assert!(!sg.set_text_content(id, "nope".to_string()));
    }

    #[test]
    fn set_font_size_on_vector_returns_false() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_vector("v", "M0 0"));
        assert!(!sg.set_font_size(id, 20.0));
    }

    #[test]
    fn set_corner_radius_on_text_returns_false() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_text("t", "hi", 12.0));
        assert!(!sg.set_corner_radius(id, [4.0; 4]));
    }

    #[test]
    fn set_path_data_on_image_returns_false() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_image("i", "pic.png"));
        assert!(!sg.set_path_data(id, "M0 0".to_string()));
    }

    #[test]
    fn set_asset_ref_on_frame_returns_false() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        assert!(!sg.set_asset_ref(id, "pic.png".to_string()));
    }

    // -----------------------------------------------------------------------
    // 5. Dirty flag propagation
    // -----------------------------------------------------------------------

    #[test]
    fn set_fill_marks_dirty_style() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        sg.clear_all_dirty();

        sg.set_fill(id, Some(Color::new(1.0, 0.0, 0.0, 1.0)));
        let node = sg.get(id).expect("node");
        assert!(node.dirty.contains(DirtyFlags::STYLE));
        assert!(!node.dirty.contains(DirtyFlags::GEOMETRY));
    }

    #[test]
    fn set_bounds_marks_dirty_geometry() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        sg.clear_all_dirty();

        sg.set_bounds(id, BoundingBox::new(5.0, 5.0, 50.0, 50.0));
        let node = sg.get(id).expect("node");
        assert!(node.dirty.contains(DirtyFlags::GEOMETRY));
    }

    #[test]
    fn set_text_content_marks_dirty_text() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_text("t", "hi", 12.0));
        sg.clear_all_dirty();

        sg.set_text_content(id, "bye".to_string());
        let node = sg.get(id).expect("node");
        assert!(node.dirty.contains(DirtyFlags::TEXT));
    }

    #[test]
    fn set_effects_marks_dirty_effects() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        sg.clear_all_dirty();

        sg.set_effects(id, vec![Effect::Blur { radius: 2.0 }]);
        let node = sg.get(id).expect("node");
        assert!(node.dirty.contains(DirtyFlags::EFFECTS));
    }

    #[test]
    fn set_clip_mode_marks_dirty_clip() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        sg.clear_all_dirty();

        sg.set_clip_mode(id, ClipMode::Stencil);
        let node = sg.get(id).expect("node");
        assert!(node.dirty.contains(DirtyFlags::CLIP));
    }

    #[test]
    fn set_transform_marks_dirty_transform_and_propagates_up() {
        let mut sg = SceneGraph::new();
        let parent_id = sg.add_root(make_frame("parent"));
        let child = make_frame("child");
        let child_id = child.id;
        sg.add_child(parent_id, child);
        sg.clear_all_dirty();

        sg.set_transform(child_id, Transform2D::identity());

        let child_node = sg.get(child_id).expect("child");
        assert!(child_node.dirty.contains(DirtyFlags::TRANSFORM));

        let parent_node = sg.get(parent_id).expect("parent");
        assert!(
            parent_node.dirty.contains(DirtyFlags::CHILDREN),
            "parent should have CHILDREN dirty after child mutation"
        );
    }

    #[test]
    fn set_name_does_not_set_dirty_flags() {
        let mut sg = SceneGraph::new();
        let id = sg.add_root(make_frame("f"));
        sg.clear_all_dirty();

        sg.set_name(id, "renamed".to_string());
        let node = sg.get(id).expect("node");
        assert!(
            node.dirty.is_clean(),
            "set_name should not set any dirty flags"
        );
    }

    // -----------------------------------------------------------------------
    // 6. Spatial index sync for set_bounds and set_transform
    // -----------------------------------------------------------------------

    #[test]
    fn set_bounds_syncs_spatial_index() {
        let mut sg = SceneGraph::new();
        let node = SceneNode::new(
            NodeId::new(),
            "f".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 10.0, 10.0),
        );
        let id = sg.add_root(node);

        // Before move: the node is at (0,0)-(10,10).
        assert!(sg.spatial().query_point(5.0, 5.0).contains(&id));
        assert!(!sg.spatial().query_point(500.0, 500.0).contains(&id));

        // Move the node to (490,490)-(590,590).
        sg.set_bounds(id, BoundingBox::new(490.0, 490.0, 100.0, 100.0));

        assert!(
            sg.spatial().query_point(500.0, 500.0).contains(&id),
            "spatial index should find node at new position"
        );
        assert!(
            !sg.spatial().query_point(5.0, 5.0).contains(&id),
            "spatial index should not find node at old position"
        );
    }

    #[test]
    fn set_transform_marks_descendants_dirty() {
        let mut sg = SceneGraph::new();
        let root_id = sg.add_root(make_frame("root"));
        let child = make_frame("child");
        let child_id = child.id;
        sg.add_child(root_id, child);
        let grandchild = make_frame("grandchild");
        let grandchild_id = grandchild.id;
        sg.add_child(child_id, grandchild);
        sg.clear_all_dirty();

        // Mutate the root transform.
        sg.set_transform(root_id, Transform2D::identity());

        // Both child and grandchild should have TRANSFORM dirty (downward propagation).
        let child_node = sg.get(child_id).expect("child");
        assert!(
            child_node.dirty.contains(DirtyFlags::TRANSFORM),
            "child should have TRANSFORM dirty after parent transform change"
        );

        let gc_node = sg.get(grandchild_id).expect("grandchild");
        assert!(
            gc_node.dirty.contains(DirtyFlags::TRANSFORM),
            "grandchild should have TRANSFORM dirty after ancestor transform change"
        );
    }
}
