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
