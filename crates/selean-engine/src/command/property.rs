//! Property commands for the undo/redo system.
//!
//! Each command wraps a single property mutation on a scene node, capturing the
//! old value on execute so it can be restored on undo.

use selean_common::types::NodeId;

use super::traits::Command;
use crate::scene::clip::ClipMode;
use crate::scene::transform::Transform2D;
use crate::scene::{
    BlendMode, BoundingBox, Color, Effect, FontStyle, Gradient, SceneGraph, SceneNodeKind,
    TextAlign,
};

/// Generates a property command struct and its `Command` impl.
///
/// The macro takes:
/// - `$name`: struct name (e.g., `SetFillCommand`)
/// - `$value_ty`: the property type (e.g., `Option<Color>`)
/// - `$desc`: human-readable description string
/// - `$getter`: closure `|node| -> T` to read the current value
/// - `$setter`: method name on `SceneGraph` to set the value
macro_rules! define_property_command {
    (
        $name:ident,
        $value_ty:ty,
        $desc:expr,
        $getter:expr,
        $setter:ident
    ) => {
        #[doc = concat!("Command to ", $desc, " on a scene node.")]
        #[derive(Debug)]
        pub struct $name {
            node_id: NodeId,
            new_value: $value_ty,
            old_value: Option<$value_ty>,
        }

        impl $name {
            /// Creates a new command targeting the given node.
            #[must_use]
            pub fn new(node_id: NodeId, new_value: $value_ty) -> Self {
                Self {
                    node_id,
                    new_value,
                    old_value: None,
                }
            }
        }

        impl Command for $name {
            fn execute(&mut self, scene: &mut SceneGraph) -> bool {
                let getter_fn: fn(&crate::scene::SceneNode) -> $value_ty = $getter;
                let Some(node) = scene.get(self.node_id) else {
                    return false;
                };
                self.old_value = Some(getter_fn(node));
                scene.$setter(self.node_id, self.new_value.clone())
            }

            fn undo(&mut self, scene: &mut SceneGraph) -> bool {
                let Some(old) = self.old_value.take() else {
                    return false;
                };
                scene.$setter(self.node_id, old)
            }

            fn description(&self) -> &str {
                $desc
            }
        }
    };
}

// --- Scroll offset needs a special setter signature (x, y instead of [f32; 2]) ---

/// Generates a property command for scroll offset, which uses `set_scroll_offset(id, x, y)`.
macro_rules! define_scroll_offset_command {
    (
        $name:ident,
        $desc:expr
    ) => {
        #[doc = concat!("Command to ", $desc, " on a scene node.")]
        #[derive(Debug)]
        pub struct $name {
            node_id: NodeId,
            new_value: [f32; 2],
            old_value: Option<[f32; 2]>,
        }

        impl $name {
            /// Creates a new command targeting the given node.
            #[must_use]
            pub fn new(node_id: NodeId, new_value: [f32; 2]) -> Self {
                Self {
                    node_id,
                    new_value,
                    old_value: None,
                }
            }
        }

        impl Command for $name {
            fn execute(&mut self, scene: &mut SceneGraph) -> bool {
                let Some(node) = scene.get(self.node_id) else {
                    return false;
                };
                self.old_value = Some(node.scroll_offset);
                scene.set_scroll_offset(self.node_id, self.new_value[0], self.new_value[1])
            }

            fn undo(&mut self, scene: &mut SceneGraph) -> bool {
                let Some(old) = self.old_value.take() else {
                    return false;
                };
                scene.set_scroll_offset(self.node_id, old[0], old[1])
            }

            fn description(&self) -> &str {
                $desc
            }
        }
    };
}

// 10 macro-generated property commands (simple getter/setter pairs).
define_property_command!(
    SetBoundsCommand,
    BoundingBox,
    "Set Bounds",
    |node| node.bounds,
    set_bounds
);

define_property_command!(
    SetFillCommand,
    Option<Color>,
    "Set Fill",
    |node| node.fill,
    set_fill
);

define_property_command!(
    SetFillGradientCommand,
    Option<Gradient>,
    "Set Fill Gradient",
    |node| node.fill_gradient.clone(),
    set_fill_gradient
);

define_property_command!(
    SetEffectsCommand,
    Vec<Effect>,
    "Set Effects",
    |node| node.effects.clone(),
    set_effects
);

define_property_command!(
    SetStrokeCommand,
    Option<Color>,
    "Set Stroke",
    |node| node.stroke,
    set_stroke
);

define_property_command!(
    SetStrokeWidthCommand,
    f32,
    "Set Stroke Width",
    |node| node.stroke_width,
    set_stroke_width
);

define_property_command!(
    SetOpacityCommand,
    f32,
    "Set Opacity",
    |node| node.opacity,
    set_opacity
);

define_property_command!(
    SetVisibleCommand,
    bool,
    "Set Visible",
    |node| node.visible,
    set_visible
);

define_property_command!(
    SetNameCommand,
    String,
    "Set Name",
    |node| node.name.clone(),
    set_name
);

define_property_command!(
    SetBlendModeCommand,
    BlendMode,
    "Set Blend Mode",
    |node| node.blend_mode,
    set_blend_mode
);

define_property_command!(
    SetClipModeCommand,
    ClipMode,
    "Set Clip Mode",
    |node| node.clip_mode,
    set_clip_mode
);

define_property_command!(
    SetTransformCommand,
    Transform2D,
    "Set Transform",
    |node| node.local_transform,
    set_transform
);

// Scroll offset has a different setter signature.
define_scroll_offset_command!(SetScrollOffsetCommand, "Set Scroll Offset");

// --- Kind-specific property commands (macro-generated) ---

/// Generates a kind-specific property command that pattern-matches on a
/// [`SceneNodeKind`] variant to read the old value before applying a mutation.
///
/// Unlike [`define_property_command!`], which accesses top-level `SceneNode`
/// fields, this macro reaches into the `kind` enum.
macro_rules! define_kind_property_command {
    (
        $name:ident,
        $value_ty:ty,
        $desc:expr,
        $kind_pat:pat,
        $getter:expr,
        $setter:ident
    ) => {
        #[doc = concat!("Command to ", $desc, " on a scene node.")]
        #[derive(Debug)]
        #[allow(clippy::option_option)]
        pub struct $name {
            node_id: NodeId,
            new_value: $value_ty,
            old_value: Option<$value_ty>,
        }

        impl $name {
            /// Creates a new command targeting the given node.
            #[must_use]
            pub fn new(node_id: NodeId, new_value: $value_ty) -> Self {
                Self {
                    node_id,
                    new_value,
                    old_value: None,
                }
            }
        }

        impl Command for $name {
            fn execute(&mut self, scene: &mut SceneGraph) -> bool {
                let Some(node) = scene.get(self.node_id) else {
                    return false;
                };
                let $kind_pat = node.kind else {
                    return false;
                };
                self.old_value = Some($getter);
                scene.$setter(self.node_id, self.new_value.clone())
            }

            fn undo(&mut self, scene: &mut SceneGraph) -> bool {
                let Some(old) = self.old_value.take() else {
                    return false;
                };
                scene.$setter(self.node_id, old)
            }

            fn description(&self) -> &str {
                $desc
            }
        }
    };
}

define_kind_property_command!(
    SetTextContentCommand,
    String,
    "Set Text Content",
    SceneNodeKind::Text { ref content, .. },
    content.clone(),
    set_text_content
);

define_kind_property_command!(
    SetFontSizeCommand,
    f32,
    "Set Font Size",
    SceneNodeKind::Text { font_size, .. },
    font_size,
    set_font_size
);

define_kind_property_command!(
    SetPathDataCommand,
    String,
    "Set Path Data",
    SceneNodeKind::Vector { ref path_data },
    path_data.clone(),
    set_path_data
);

define_kind_property_command!(
    SetAssetRefCommand,
    String,
    "Set Asset Ref",
    SceneNodeKind::Image { ref asset_ref },
    asset_ref.clone(),
    set_asset_ref
);

define_kind_property_command!(
    SetCornerRadiusCommand,
    [f32; 4],
    "Set Corner Radius",
    SceneNodeKind::Frame { corner_radius },
    corner_radius,
    set_corner_radius
);

define_kind_property_command!(
    SetFontFamilyCommand,
    String,
    "Set Font Family",
    SceneNodeKind::Text { ref font_family, .. },
    font_family.clone(),
    set_font_family
);

define_kind_property_command!(
    SetFontWeightCommand,
    u16,
    "Set Font Weight",
    SceneNodeKind::Text { font_weight, .. },
    font_weight,
    set_font_weight
);

define_kind_property_command!(
    SetFontStyleCommand,
    FontStyle,
    "Set Font Style",
    SceneNodeKind::Text { font_style, .. },
    font_style,
    set_font_style
);

define_kind_property_command!(
    SetTextAlignCommand,
    TextAlign,
    "Set Text Align",
    SceneNodeKind::Text { text_align, .. },
    text_align,
    set_text_align
);

define_kind_property_command!(
    SetLineHeightCommand,
    f32,
    "Set Line Height",
    SceneNodeKind::Text { line_height, .. },
    line_height,
    set_line_height
);

define_kind_property_command!(
    SetTextColorCommand,
    Option<Color>,
    "Set Text Color",
    SceneNodeKind::Text { text_color, .. },
    text_color,
    set_text_color
);

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use crate::scene::{BoundingBox, GradientStop, SceneNode, SceneNodeKind};

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

    // --- Macro-generated command roundtrip tests ---

    #[test]
    fn set_bounds_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let new_bounds = BoundingBox::new(10.0, 20.0, 200.0, 150.0);
        let mut cmd = SetBoundsCommand::new(id, new_bounds);

        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).unwrap().bounds, new_bounds);

        assert!(cmd.undo(&mut scene));
        assert_eq!(
            scene.get(id).unwrap().bounds,
            BoundingBox::new(0.0, 0.0, 100.0, 100.0)
        );
    }

    #[test]
    fn set_fill_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let color = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        let mut cmd = SetFillCommand::new(id, color);

        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).unwrap().fill, color);

        assert!(cmd.undo(&mut scene));
        assert!(scene.get(id).unwrap().fill.is_none());
    }

    #[test]
    fn set_fill_gradient_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let grad = Some(Gradient::Linear {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
            stops: vec![
                GradientStop {
                    position: 0.0,
                    color: Color::new(1.0, 0.0, 0.0, 1.0),
                },
                GradientStop {
                    position: 1.0,
                    color: Color::new(0.0, 0.0, 1.0, 1.0),
                },
            ],
        });
        let mut cmd = SetFillGradientCommand::new(id, grad.clone());
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).unwrap().fill_gradient, grad);

        assert!(cmd.undo(&mut scene));
        assert!(scene.get(id).unwrap().fill_gradient.is_none());
    }

    #[test]
    fn set_fill_gradient_none_roundtrip() {
        let mut scene = SceneGraph::new();
        let mut node = make_frame("A");
        node.fill_gradient = Some(Gradient::Radial {
            center: [0.5, 0.5],
            radius: 1.0,
            stops: vec![],
        });
        let id = scene.add_root(node);

        let mut cmd = SetFillGradientCommand::new(id, None);
        assert!(cmd.execute(&mut scene));
        assert!(scene.get(id).unwrap().fill_gradient.is_none());

        assert!(cmd.undo(&mut scene));
        assert!(scene.get(id).unwrap().fill_gradient.is_some());
    }

    #[test]
    fn set_stroke_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let color = Some(Color::new(0.0, 1.0, 0.0, 1.0));
        let mut cmd = SetStrokeCommand::new(id, color);

        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).unwrap().stroke, color);

        assert!(cmd.undo(&mut scene));
        assert!(scene.get(id).unwrap().stroke.is_none());
    }

    #[test]
    fn set_stroke_width_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let mut cmd = SetStrokeWidthCommand::new(id, 5.0);
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).unwrap().stroke_width, 5.0);

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.get(id).unwrap().stroke_width, 0.0);
    }

    #[test]
    fn set_opacity_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let mut cmd = SetOpacityCommand::new(id, 0.5);
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).unwrap().opacity, 0.5);

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.get(id).unwrap().opacity, 1.0);
    }

    #[test]
    fn set_visible_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let mut cmd = SetVisibleCommand::new(id, false);
        assert!(cmd.execute(&mut scene));
        assert!(!scene.get(id).unwrap().visible);

        assert!(cmd.undo(&mut scene));
        assert!(scene.get(id).unwrap().visible);
    }

    #[test]
    fn set_name_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("Original");
        let id = scene.add_root(node);

        let mut cmd = SetNameCommand::new(id, "Renamed".to_string());
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).unwrap().name, "Renamed");

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.get(id).unwrap().name, "Original");
    }

    #[test]
    fn set_blend_mode_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let mut cmd = SetBlendModeCommand::new(id, BlendMode::Add);
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).unwrap().blend_mode, BlendMode::Add);

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.get(id).unwrap().blend_mode, BlendMode::Normal);
    }

    #[test]
    fn set_clip_mode_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let mut cmd = SetClipModeCommand::new(id, ClipMode::Scissor);
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).unwrap().clip_mode, ClipMode::Scissor);

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.get(id).unwrap().clip_mode, ClipMode::None);
    }

    #[test]
    fn set_transform_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let t = Transform2D::translation(10.0, 20.0);
        let mut cmd = SetTransformCommand::new(id, t);
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).unwrap().local_transform, t);

        assert!(cmd.undo(&mut scene));
        assert!(scene.get(id).unwrap().local_transform.is_identity());
    }

    #[test]
    fn set_scroll_offset_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let mut cmd = SetScrollOffsetCommand::new(id, [15.0, 25.0]);
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).unwrap().scroll_offset, [15.0, 25.0]);

        assert!(cmd.undo(&mut scene));
        assert_eq!(scene.get(id).unwrap().scroll_offset, [0.0, 0.0]);
    }

    // --- Kind-specific command tests ---

    #[test]
    fn set_text_content_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_text("T", "Hello", 16.0);
        let id = scene.add_root(node);

        let mut cmd = SetTextContentCommand::new(id, "World".to_string());
        assert!(cmd.execute(&mut scene));
        if let SceneNodeKind::Text { ref content, .. } = scene.get(id).unwrap().kind {
            assert_eq!(content, "World");
        } else {
            panic!("Expected Text node");
        }

        assert!(cmd.undo(&mut scene));
        if let SceneNodeKind::Text { ref content, .. } = scene.get(id).unwrap().kind {
            assert_eq!(content, "Hello");
        } else {
            panic!("Expected Text node");
        }
    }

    #[test]
    fn set_font_size_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_text("T", "Hello", 16.0);
        let id = scene.add_root(node);

        let mut cmd = SetFontSizeCommand::new(id, 24.0);
        assert!(cmd.execute(&mut scene));
        if let SceneNodeKind::Text { font_size, .. } = scene.get(id).unwrap().kind {
            assert_eq!(font_size, 24.0);
        } else {
            panic!("Expected Text node");
        }

        assert!(cmd.undo(&mut scene));
        if let SceneNodeKind::Text { font_size, .. } = scene.get(id).unwrap().kind {
            assert_eq!(font_size, 16.0);
        } else {
            panic!("Expected Text node");
        }
    }

    #[test]
    fn set_path_data_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_vector("V", "M0 0 L10 10");
        let id = scene.add_root(node);

        let mut cmd = SetPathDataCommand::new(id, "M0 0 L20 20".to_string());
        assert!(cmd.execute(&mut scene));
        if let SceneNodeKind::Vector { ref path_data } = scene.get(id).unwrap().kind {
            assert_eq!(path_data, "M0 0 L20 20");
        } else {
            panic!("Expected Vector node");
        }

        assert!(cmd.undo(&mut scene));
        if let SceneNodeKind::Vector { ref path_data } = scene.get(id).unwrap().kind {
            assert_eq!(path_data, "M0 0 L10 10");
        } else {
            panic!("Expected Vector node");
        }
    }

    #[test]
    fn set_asset_ref_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_image("I", "old.png");
        let id = scene.add_root(node);

        let mut cmd = SetAssetRefCommand::new(id, "new.png".to_string());
        assert!(cmd.execute(&mut scene));
        if let SceneNodeKind::Image { ref asset_ref } = scene.get(id).unwrap().kind {
            assert_eq!(asset_ref, "new.png");
        } else {
            panic!("Expected Image node");
        }

        assert!(cmd.undo(&mut scene));
        if let SceneNodeKind::Image { ref asset_ref } = scene.get(id).unwrap().kind {
            assert_eq!(asset_ref, "old.png");
        } else {
            panic!("Expected Image node");
        }
    }

    // --- Error cases ---

    #[test]
    fn kind_specific_wrong_node_kind_returns_false() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let mut cmd = SetTextContentCommand::new(id, "Hello".to_string());
        assert!(!cmd.execute(&mut scene));

        let mut cmd = SetFontSizeCommand::new(id, 24.0);
        assert!(!cmd.execute(&mut scene));

        let mut cmd = SetPathDataCommand::new(id, "M0 0".to_string());
        assert!(!cmd.execute(&mut scene));

        let mut cmd = SetAssetRefCommand::new(id, "img.png".to_string());
        assert!(!cmd.execute(&mut scene));
    }

    #[test]
    fn execute_on_missing_node_returns_false() {
        let mut scene = SceneGraph::new();
        let missing = NodeId::new();

        let mut cmd = SetFillCommand::new(missing, Some(Color::WHITE));
        assert!(!cmd.execute(&mut scene));
    }

    #[test]
    fn undo_without_execute_returns_false() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A");
        let id = scene.add_root(node);

        let mut cmd = SetFillCommand::new(id, Some(Color::WHITE));
        assert!(!cmd.undo(&mut scene));
    }

    // --- New command tests ---

    #[test]
    fn set_corner_radius_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_frame("F");
        let id = scene.add_root(node);

        let mut cmd = SetCornerRadiusCommand::new(id, [8.0, 8.0, 8.0, 8.0]);
        assert!(cmd.execute(&mut scene));
        if let SceneNodeKind::Frame { corner_radius } = scene.get(id).unwrap().kind {
            assert_eq!(corner_radius, [8.0; 4]);
        } else {
            panic!("Expected Frame node");
        }

        assert!(cmd.undo(&mut scene));
        if let SceneNodeKind::Frame { corner_radius } = scene.get(id).unwrap().kind {
            assert_eq!(corner_radius, [0.0; 4]);
        } else {
            panic!("Expected Frame node");
        }
    }

    #[test]
    fn set_font_family_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_text("T", "Hello", 16.0);
        let id = scene.add_root(node);

        let mut cmd = SetFontFamilyCommand::new(id, "Roboto".to_string());
        assert!(cmd.execute(&mut scene));
        if let SceneNodeKind::Text {
            ref font_family, ..
        } = scene.get(id).unwrap().kind
        {
            assert_eq!(font_family, "Roboto");
        } else {
            panic!("Expected Text node");
        }

        assert!(cmd.undo(&mut scene));
        if let SceneNodeKind::Text {
            ref font_family, ..
        } = scene.get(id).unwrap().kind
        {
            assert_eq!(font_family, "Inter");
        } else {
            panic!("Expected Text node");
        }
    }

    #[test]
    fn set_font_weight_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_text("T", "Hello", 16.0);
        let id = scene.add_root(node);

        let mut cmd = SetFontWeightCommand::new(id, 700);
        assert!(cmd.execute(&mut scene));
        if let SceneNodeKind::Text { font_weight, .. } = scene.get(id).unwrap().kind {
            assert_eq!(font_weight, 700);
        } else {
            panic!("Expected Text node");
        }

        assert!(cmd.undo(&mut scene));
        if let SceneNodeKind::Text { font_weight, .. } = scene.get(id).unwrap().kind {
            assert_eq!(font_weight, 400);
        } else {
            panic!("Expected Text node");
        }
    }

    #[test]
    fn set_font_style_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_text("T", "Hello", 16.0);
        let id = scene.add_root(node);

        let mut cmd = SetFontStyleCommand::new(id, FontStyle::Italic);
        assert!(cmd.execute(&mut scene));
        if let SceneNodeKind::Text { font_style, .. } = scene.get(id).unwrap().kind {
            assert_eq!(font_style, FontStyle::Italic);
        } else {
            panic!("Expected Text node");
        }

        assert!(cmd.undo(&mut scene));
        if let SceneNodeKind::Text { font_style, .. } = scene.get(id).unwrap().kind {
            assert_eq!(font_style, FontStyle::Normal);
        } else {
            panic!("Expected Text node");
        }
    }

    #[test]
    fn set_text_align_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_text("T", "Hello", 16.0);
        let id = scene.add_root(node);

        let mut cmd = SetTextAlignCommand::new(id, TextAlign::Center);
        assert!(cmd.execute(&mut scene));
        if let SceneNodeKind::Text { text_align, .. } = scene.get(id).unwrap().kind {
            assert_eq!(text_align, TextAlign::Center);
        } else {
            panic!("Expected Text node");
        }

        assert!(cmd.undo(&mut scene));
        if let SceneNodeKind::Text { text_align, .. } = scene.get(id).unwrap().kind {
            assert_eq!(text_align, TextAlign::Left);
        } else {
            panic!("Expected Text node");
        }
    }

    #[test]
    fn set_line_height_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_text("T", "Hello", 16.0);
        let id = scene.add_root(node);

        let mut cmd = SetLineHeightCommand::new(id, 1.8);
        assert!(cmd.execute(&mut scene));
        if let SceneNodeKind::Text { line_height, .. } = scene.get(id).unwrap().kind {
            assert!((line_height - 1.8).abs() < f32::EPSILON);
        } else {
            panic!("Expected Text node");
        }

        assert!(cmd.undo(&mut scene));
        if let SceneNodeKind::Text { line_height, .. } = scene.get(id).unwrap().kind {
            assert!((line_height - 1.2).abs() < f32::EPSILON);
        } else {
            panic!("Expected Text node");
        }
    }

    #[test]
    fn set_text_color_execute_and_undo() {
        let mut scene = SceneGraph::new();
        let node = make_text("T", "Hello", 16.0);
        let id = scene.add_root(node);

        let red = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        let mut cmd = SetTextColorCommand::new(id, red);
        assert!(cmd.execute(&mut scene));
        if let SceneNodeKind::Text { text_color, .. } = scene.get(id).unwrap().kind {
            assert_eq!(text_color, red);
        } else {
            panic!("Expected Text node");
        }

        assert!(cmd.undo(&mut scene));
        if let SceneNodeKind::Text { text_color, .. } = scene.get(id).unwrap().kind {
            assert!(text_color.is_none());
        } else {
            panic!("Expected Text node");
        }
    }
}
