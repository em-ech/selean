//! Serializable command descriptors for cross-boundary communication.
//!
//! [`CommandDescriptor`] is the shared wire format for mutations across all tracks:
//! React UI edits, LLM tool calls, and file imports. Each variant mirrors a
//! concrete `Command` type in this crate.

use selean_common::types::NodeId;
use serde::{Deserialize, Serialize};

use crate::scene::{
    BlendMode, BoundingBox, ClipMode, Color, Effect, FontStyle, Gradient, SceneNode, SceneNodeKind,
    TextAlign, Transform2D,
};

use super::{
    AddChildCommand, AddRootCommand, Command, RemoveNodeCommand, ReorderChildrenCommand,
    ReorderRootsCommand, ReparentCommand, ReparentToRootCommand, SetAssetRefCommand,
    SetBlendModeCommand,
    SetBoundsCommand, SetClipModeCommand, SetCornerRadiusCommand, SetEffectsCommand,
    SetFillCommand, SetFillGradientCommand, SetFontFamilyCommand, SetFontSizeCommand,
    SetFontStyleCommand, SetFontWeightCommand, SetLineHeightCommand, SetNameCommand,
    SetOpacityCommand, SetPathDataCommand, SetScrollOffsetCommand, SetStrokeCommand,
    SetStrokeWidthCommand, SetTextAlignCommand, SetTextColorCommand, SetTextContentCommand,
    SetTransformCommand, SetVisibleCommand,
};

/// A serializable description of a scene graph mutation.
///
/// This is the single wire format shared by all mutation sources (UI, LLM, import).
/// Deserialized from JSON, then converted to a `Box<dyn Command>` for execution
/// through `CommandHistory`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum CommandDescriptor {
    /// Set the bounding box of a node.
    SetBounds {
        /// Target node.
        node_id: NodeId,
        /// New bounds.
        bounds: BoundingBox,
    },
    /// Set or clear the fill color of a node.
    SetFill {
        /// Target node.
        node_id: NodeId,
        /// New fill color, or null to clear.
        fill: Option<Color>,
    },
    /// Set or clear the gradient fill of a node.
    SetFillGradient {
        /// Target node.
        node_id: NodeId,
        /// New gradient fill, or null to clear.
        fill_gradient: Option<Gradient>,
    },
    /// Set or clear the stroke color of a node.
    SetStroke {
        /// Target node.
        node_id: NodeId,
        /// New stroke color, or null to clear.
        stroke: Option<Color>,
    },
    /// Set the stroke width of a node.
    SetStrokeWidth {
        /// Target node.
        node_id: NodeId,
        /// New stroke width.
        width: f32,
    },
    /// Set the opacity of a node.
    SetOpacity {
        /// Target node.
        node_id: NodeId,
        /// New opacity in [0.0, 1.0].
        opacity: f32,
    },
    /// Set the visibility of a node.
    SetVisible {
        /// Target node.
        node_id: NodeId,
        /// New visibility state.
        visible: bool,
    },
    /// Set the name of a node.
    SetName {
        /// Target node.
        node_id: NodeId,
        /// New name.
        name: String,
    },
    /// Set the text content of a Text node.
    SetTextContent {
        /// Target node (must be Text kind).
        node_id: NodeId,
        /// New text content.
        content: String,
    },
    /// Set the font size of a Text node.
    SetFontSize {
        /// Target node (must be Text kind).
        node_id: NodeId,
        /// New font size.
        font_size: f32,
    },
    /// Add a new root node to the scene.
    AddRoot {
        /// The node to add.
        node: SceneNode,
    },
    /// Add a child node under a parent.
    AddChild {
        /// Parent node to attach to.
        parent_id: NodeId,
        /// The child node to add.
        node: SceneNode,
    },
    /// Remove a node (and its subtree) from the scene.
    RemoveNode {
        /// Node to remove.
        node_id: NodeId,
    },
    /// Set the blend mode of a node.
    SetBlendMode {
        /// Target node.
        node_id: NodeId,
        /// New blend mode.
        blend_mode: BlendMode,
    },
    /// Set the clip mode of a node.
    SetClipMode {
        /// Target node.
        node_id: NodeId,
        /// New clip mode.
        clip_mode: ClipMode,
    },
    /// Set the local transform of a node.
    SetTransform {
        /// Target node.
        node_id: NodeId,
        /// New transform as `[a, b, c, d, tx, ty]`.
        transform: [f32; 6],
    },
    /// Set the scroll offset of a node.
    SetScrollOffset {
        /// Target node.
        node_id: NodeId,
        /// New scroll offset `[x, y]`.
        offset: [f32; 2],
    },
    /// Set the SVG path data of a Vector node.
    SetPathData {
        /// Target node (must be Vector kind).
        node_id: NodeId,
        /// New SVG path data string.
        path_data: String,
    },
    /// Set the asset reference of an Image node.
    SetAssetRef {
        /// Target node (must be Image kind).
        node_id: NodeId,
        /// New asset reference string.
        asset_ref: String,
    },
    /// Set the corner radius of a Frame node.
    SetCornerRadius {
        /// Target node (must be Frame kind).
        node_id: NodeId,
        /// New corner radius `[tl, tr, br, bl]`.
        corner_radius: [f32; 4],
    },
    /// Set the font family of a Text node.
    SetFontFamily {
        /// Target node (must be Text kind).
        node_id: NodeId,
        /// New font family name.
        font_family: String,
    },
    /// Set the font weight of a Text node.
    SetFontWeight {
        /// Target node (must be Text kind).
        node_id: NodeId,
        /// New font weight (100-900).
        font_weight: u16,
    },
    /// Set the font style of a Text node.
    SetFontStyle {
        /// Target node (must be Text kind).
        node_id: NodeId,
        /// New font style.
        font_style: FontStyle,
    },
    /// Set the text alignment of a Text node.
    SetTextAlign {
        /// Target node (must be Text kind).
        node_id: NodeId,
        /// New text alignment.
        text_align: TextAlign,
    },
    /// Set the line height of a Text node.
    SetLineHeight {
        /// Target node (must be Text kind).
        node_id: NodeId,
        /// New line height multiplier.
        line_height: f32,
    },
    /// Set the text color of a Text node.
    SetTextColor {
        /// Target node (must be Text kind).
        node_id: NodeId,
        /// New text color, or null to clear.
        text_color: Option<Color>,
    },
    /// Reparent a node to a new parent.
    Reparent {
        /// Node to move.
        node_id: NodeId,
        /// New parent node ID.
        new_parent_id: NodeId,
    },
    /// Reorder children of a node.
    ReorderChildren {
        /// Parent node whose children to reorder.
        parent_id: NodeId,
        /// New order of child IDs.
        new_order: Vec<NodeId>,
    },
    /// Set the effects list of a node.
    SetEffects {
        /// Target node.
        node_id: NodeId,
        /// New effects list.
        effects: Vec<Effect>,
    },
    /// Move a node to become a root (detach from parent).
    ReparentToRoot {
        /// Node to move to root level.
        node_id: NodeId,
    },
    /// Reorder root nodes.
    ReorderRoots {
        /// New order of root node IDs.
        new_order: Vec<NodeId>,
    },
}

impl CommandDescriptor {
    /// Converts this descriptor into a boxed `Command` ready for execution.
    #[must_use]
    pub fn into_command(self) -> Box<dyn Command> {
        match self {
            Self::SetBounds { node_id, bounds } => Box::new(SetBoundsCommand::new(node_id, bounds)),
            Self::SetFill { node_id, fill } => Box::new(SetFillCommand::new(node_id, fill)),
            Self::SetFillGradient {
                node_id,
                fill_gradient,
            } => Box::new(SetFillGradientCommand::new(node_id, fill_gradient)),
            Self::SetStroke { node_id, stroke } => Box::new(SetStrokeCommand::new(node_id, stroke)),
            Self::SetStrokeWidth { node_id, width } => {
                Box::new(SetStrokeWidthCommand::new(node_id, width))
            }
            Self::SetOpacity { node_id, opacity } => {
                Box::new(SetOpacityCommand::new(node_id, opacity))
            }
            Self::SetVisible { node_id, visible } => {
                Box::new(SetVisibleCommand::new(node_id, visible))
            }
            Self::SetName { node_id, name } => Box::new(SetNameCommand::new(node_id, name)),
            Self::SetTextContent { node_id, content } => {
                Box::new(SetTextContentCommand::new(node_id, content))
            }
            Self::SetFontSize { node_id, font_size } => {
                Box::new(SetFontSizeCommand::new(node_id, font_size))
            }
            Self::AddRoot { node } => Box::new(AddRootCommand::new(node)),
            Self::AddChild { parent_id, node } => Box::new(AddChildCommand::new(parent_id, node)),
            Self::RemoveNode { node_id } => Box::new(RemoveNodeCommand::new(node_id)),
            Self::SetBlendMode {
                node_id,
                blend_mode,
            } => Box::new(SetBlendModeCommand::new(node_id, blend_mode)),
            Self::SetClipMode { node_id, clip_mode } => {
                Box::new(SetClipModeCommand::new(node_id, clip_mode))
            }
            Self::SetTransform { node_id, transform } => Box::new(SetTransformCommand::new(
                node_id,
                Transform2D::from_raw(transform),
            )),
            Self::SetScrollOffset { node_id, offset } => {
                Box::new(SetScrollOffsetCommand::new(node_id, offset))
            }
            Self::SetPathData { node_id, path_data } => {
                Box::new(SetPathDataCommand::new(node_id, path_data))
            }
            Self::SetAssetRef { node_id, asset_ref } => {
                Box::new(SetAssetRefCommand::new(node_id, asset_ref))
            }
            Self::SetCornerRadius {
                node_id,
                corner_radius,
            } => Box::new(SetCornerRadiusCommand::new(node_id, corner_radius)),
            Self::SetFontFamily {
                node_id,
                font_family,
            } => Box::new(SetFontFamilyCommand::new(node_id, font_family)),
            Self::SetFontWeight {
                node_id,
                font_weight,
            } => Box::new(SetFontWeightCommand::new(node_id, font_weight)),
            Self::SetFontStyle {
                node_id,
                font_style,
            } => Box::new(SetFontStyleCommand::new(node_id, font_style)),
            Self::SetTextAlign {
                node_id,
                text_align,
            } => Box::new(SetTextAlignCommand::new(node_id, text_align)),
            Self::SetLineHeight {
                node_id,
                line_height,
            } => Box::new(SetLineHeightCommand::new(node_id, line_height)),
            Self::SetTextColor {
                node_id,
                text_color,
            } => Box::new(SetTextColorCommand::new(node_id, text_color)),
            Self::Reparent {
                node_id,
                new_parent_id,
            } => Box::new(ReparentCommand::new(node_id, new_parent_id)),
            Self::ReorderChildren {
                parent_id,
                new_order,
            } => Box::new(ReorderChildrenCommand::new(parent_id, new_order)),
            Self::SetEffects { node_id, effects } => {
                Box::new(SetEffectsCommand::new(node_id, effects))
            }
            Self::ReparentToRoot { node_id } => Box::new(ReparentToRootCommand::new(node_id)),
            Self::ReorderRoots { new_order } => Box::new(ReorderRootsCommand::new(new_order)),
        }
    }
}

/// Helper to create a Frame `SceneNode` from minimal parameters.
///
/// Used by both the demo scene builder and command descriptors that need
/// to construct nodes from JSON.
#[must_use]
pub fn create_frame_node(
    name: &str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    fill: Option<Color>,
    corner_radius: [f32; 4],
) -> SceneNode {
    let mut node = SceneNode::new(
        NodeId::new(),
        name.to_string(),
        SceneNodeKind::Frame { corner_radius },
        BoundingBox::new(x, y, width, height),
    );
    node.fill = fill;
    node
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::scene::GradientStop;

    fn sample_node_id() -> NodeId {
        NodeId::new()
    }

    #[test]
    fn set_bounds_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetBounds {
            node_id: id,
            bounds: BoundingBox::new(10.0, 20.0, 100.0, 50.0),
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_fill_with_color_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetFill {
            node_id: id,
            fill: Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_fill_null_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetFill {
            node_id: id,
            fill: None,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_fill_gradient_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetFillGradient {
            node_id: id,
            fill_gradient: Some(Gradient::Linear {
                start: [0.0, 0.0],
                end: [1.0, 1.0],
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
            }),
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_fill_gradient_null_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetFillGradient {
            node_id: id,
            fill_gradient: None,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_opacity_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetOpacity {
            node_id: id,
            opacity: 0.5,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_stroke_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetStroke {
            node_id: id,
            stroke: Some(Color::BLACK),
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_name_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetName {
            node_id: id,
            name: "My Node".to_string(),
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_text_content_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetTextContent {
            node_id: id,
            content: "Hello world".to_string(),
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn add_root_roundtrip() {
        let node = create_frame_node("Test", 0.0, 0.0, 100.0, 100.0, None, [0.0; 4]);
        let desc = CommandDescriptor::AddRoot { node };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn remove_node_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::RemoveNode { node_id: id };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn add_child_roundtrip() {
        let parent_id = sample_node_id();
        let node = create_frame_node("Child", 10.0, 10.0, 50.0, 50.0, None, [0.0; 4]);
        let desc = CommandDescriptor::AddChild { parent_id, node };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_visible_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetVisible {
            node_id: id,
            visible: false,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_font_size_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetFontSize {
            node_id: id,
            font_size: 24.0,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_stroke_width_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetStrokeWidth {
            node_id: id,
            width: 2.5,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_blend_mode_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetBlendMode {
            node_id: id,
            blend_mode: BlendMode::Multiply,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_clip_mode_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetClipMode {
            node_id: id,
            clip_mode: ClipMode::Scissor,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_transform_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetTransform {
            node_id: id,
            transform: [1.0, 0.0, 0.0, 1.0, 10.0, 20.0],
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_scroll_offset_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetScrollOffset {
            node_id: id,
            offset: [100.0, 200.0],
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_path_data_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetPathData {
            node_id: id,
            path_data: "M 0 0 L 100 100".to_string(),
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_asset_ref_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetAssetRef {
            node_id: id,
            asset_ref: "images/logo.png".to_string(),
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn reparent_roundtrip() {
        let node_id = sample_node_id();
        let parent_id = sample_node_id();
        let desc = CommandDescriptor::Reparent {
            node_id,
            new_parent_id: parent_id,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn reorder_children_roundtrip() {
        let parent_id = sample_node_id();
        let child1 = sample_node_id();
        let child2 = sample_node_id();
        let desc = CommandDescriptor::ReorderChildren {
            parent_id,
            new_order: vec![child2, child1],
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn reorder_roots_roundtrip() {
        let r1 = sample_node_id();
        let r2 = sample_node_id();
        let desc = CommandDescriptor::ReorderRoots {
            new_order: vec![r2, r1],
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn tagged_json_format() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetOpacity {
            node_id: id,
            opacity: 0.75,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        assert!(json.contains("\"type\":\"SetOpacity\""));
    }

    #[test]
    fn into_command_set_bounds_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetBounds {
            node_id: id,
            bounds: BoundingBox::new(10.0, 20.0, 200.0, 100.0),
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));

        let n = scene.get(id).expect("node exists");
        assert!((n.bounds.x - 10.0).abs() < f32::EPSILON);
        assert!((n.bounds.width - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn into_command_set_fill_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetFill {
            node_id: id,
            fill: Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));

        let n = scene.get(id).expect("node exists");
        assert!(n.fill.is_some());
        assert!((n.fill.expect("has fill").r - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn into_command_add_root_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("Root", 0.0, 0.0, 100.0, 100.0, None, [0.0; 4]);
        let id = node.id;

        let desc = CommandDescriptor::AddRoot { node };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert!(scene.get(id).is_some());
    }

    #[test]
    fn into_command_remove_node_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::RemoveNode { node_id: id };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert!(scene.get(id).is_none());
    }

    #[test]
    fn into_command_set_fill_undo_restores() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetFill {
            node_id: id,
            fill: Some(Color::WHITE),
        };
        let mut cmd = desc.into_command();
        cmd.execute(&mut scene);
        assert!(scene.get(id).expect("exists").fill.is_some());

        cmd.undo(&mut scene);
        assert!(scene.get(id).expect("exists").fill.is_none());
    }

    #[test]
    fn into_command_set_blend_mode_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetBlendMode {
            node_id: id,
            blend_mode: BlendMode::Multiply,
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert_eq!(
            scene.get(id).expect("exists").blend_mode,
            BlendMode::Multiply
        );
    }

    #[test]
    fn into_command_set_clip_mode_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetClipMode {
            node_id: id,
            clip_mode: ClipMode::Stencil,
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).expect("exists").clip_mode, ClipMode::Stencil);
    }

    #[test]
    fn into_command_set_transform_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let raw = [2.0, 0.0, 0.0, 2.0, 5.0, 10.0];
        let desc = CommandDescriptor::SetTransform {
            node_id: id,
            transform: raw,
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert_eq!(*scene.get(id).expect("exists").local_transform.raw(), raw);
    }

    #[test]
    fn into_command_set_scroll_offset_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetScrollOffset {
            node_id: id,
            offset: [15.0, 25.0],
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).expect("exists").scroll_offset, [15.0, 25.0]);
    }

    // --- New descriptor roundtrip tests ---

    #[test]
    fn set_corner_radius_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetCornerRadius {
            node_id: id,
            corner_radius: [4.0, 8.0, 12.0, 16.0],
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_font_family_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetFontFamily {
            node_id: id,
            font_family: "Roboto".to_string(),
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_font_weight_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetFontWeight {
            node_id: id,
            font_weight: 700,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_font_style_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetFontStyle {
            node_id: id,
            font_style: FontStyle::Italic,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_text_align_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetTextAlign {
            node_id: id,
            text_align: TextAlign::Center,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_line_height_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetLineHeight {
            node_id: id,
            line_height: 1.5,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_text_color_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetTextColor {
            node_id: id,
            text_color: Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_text_color_null_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetTextColor {
            node_id: id,
            text_color: None,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_effects_roundtrip() {
        use crate::scene::Effect;
        let id = sample_node_id();
        let desc = CommandDescriptor::SetEffects {
            node_id: id,
            effects: vec![Effect::DropShadow {
                color: Color::new(0.0, 0.0, 0.0, 0.5),
                offset_x: 4.0,
                offset_y: 4.0,
                blur_radius: 8.0,
            }],
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn set_effects_empty_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::SetEffects {
            node_id: id,
            effects: vec![],
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn into_command_set_effects_executes() {
        use crate::scene::Effect;
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let effects = vec![Effect::Blur { radius: 10.0 }];
        let desc = CommandDescriptor::SetEffects {
            node_id: id,
            effects: effects.clone(),
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).expect("exists").effects, effects);
    }

    #[test]
    fn into_command_set_effects_undo_restores() {
        use crate::scene::Effect;
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetEffects {
            node_id: id,
            effects: vec![Effect::Blur { radius: 10.0 }],
        };
        let mut cmd = desc.into_command();
        cmd.execute(&mut scene);
        assert!(!scene.get(id).expect("exists").effects.is_empty());

        cmd.undo(&mut scene);
        assert!(scene.get(id).expect("exists").effects.is_empty());
    }

    #[test]
    fn reparent_to_root_roundtrip() {
        let id = sample_node_id();
        let desc = CommandDescriptor::ReparentToRoot { node_id: id };
        let json = serde_json::to_string(&desc).expect("serialize");
        let back: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, back);
    }

    #[test]
    fn into_command_set_opacity_executes_and_undoes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);
        assert_eq!(scene.get(id).expect("exists").opacity, 1.0);

        let desc = CommandDescriptor::SetOpacity {
            node_id: id,
            opacity: 0.3,
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert!((scene.get(id).expect("exists").opacity - 0.3).abs() < f32::EPSILON);

        cmd.undo(&mut scene);
        assert!((scene.get(id).expect("exists").opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn into_command_set_visible_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetVisible {
            node_id: id,
            visible: false,
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert!(!scene.get(id).expect("exists").visible);
    }

    #[test]
    fn into_command_set_name_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("Old", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetName {
            node_id: id,
            name: "New Name".to_string(),
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.get(id).expect("exists").name, "New Name");
    }

    #[test]
    fn into_command_set_stroke_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetStroke {
            node_id: id,
            stroke: Some(Color::BLACK),
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert!(scene.get(id).expect("exists").stroke.is_some());
    }

    #[test]
    fn into_command_set_stroke_width_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetStrokeWidth {
            node_id: id,
            width: 3.0,
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert!((scene.get(id).expect("exists").stroke_width - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn into_command_add_child_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let parent = create_frame_node("Parent", 0.0, 0.0, 200.0, 200.0, None, [0.0; 4]);
        let parent_id = parent.id;
        scene.add_root(parent);

        let child = create_frame_node("Child", 10.0, 10.0, 50.0, 50.0, None, [0.0; 4]);
        let child_id = child.id;
        let desc = CommandDescriptor::AddChild {
            parent_id,
            node: child,
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert!(scene.get(child_id).is_some());
        assert_eq!(scene.children(parent_id).unwrap().len(), 1);
    }

    #[test]
    fn into_command_set_corner_radius_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetCornerRadius {
            node_id: id,
            corner_radius: [5.0, 10.0, 15.0, 20.0],
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        match &scene.get(id).expect("exists").kind {
            SceneNodeKind::Frame { corner_radius } => {
                assert_eq!(*corner_radius, [5.0, 10.0, 15.0, 20.0]);
            }
            _ => panic!("expected Frame"),
        }
    }

    #[test]
    fn into_command_set_text_content_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = SceneNode::new(
            NodeId::new(),
            "T".to_string(),
            SceneNodeKind::Text {
                content: "old".to_string(),
                font_size: 16.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(0.0, 0.0, 100.0, 30.0),
        );
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetTextContent {
            node_id: id,
            content: "new text".to_string(),
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        match &scene.get(id).expect("exists").kind {
            SceneNodeKind::Text { content, .. } => assert_eq!(content, "new text"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn into_command_reparent_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let parent_a = create_frame_node("A", 0.0, 0.0, 100.0, 100.0, None, [0.0; 4]);
        let parent_b = create_frame_node("B", 0.0, 0.0, 100.0, 100.0, None, [0.0; 4]);
        let child = create_frame_node("C", 10.0, 10.0, 50.0, 50.0, None, [0.0; 4]);
        let a_id = parent_a.id;
        let b_id = parent_b.id;
        let c_id = child.id;
        scene.add_root(parent_a);
        scene.add_root(parent_b);
        scene.add_child(a_id, child);
        assert_eq!(scene.children(a_id).unwrap().len(), 1);

        let desc = CommandDescriptor::Reparent {
            node_id: c_id,
            new_parent_id: b_id,
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert_eq!(scene.children(a_id).unwrap().len(), 0);
        assert_eq!(scene.children(b_id).unwrap().len(), 1);
    }

    #[test]
    fn into_command_nonexistent_node_returns_false() {
        let mut scene = crate::scene::SceneGraph::new();
        let fake_id = sample_node_id();
        let desc = CommandDescriptor::SetOpacity {
            node_id: fake_id,
            opacity: 0.5,
        };
        let mut cmd = desc.into_command();
        assert!(!cmd.execute(&mut scene));
    }

    #[test]
    fn into_command_set_fill_gradient_executes() {
        let mut scene = crate::scene::SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetFillGradient {
            node_id: id,
            fill_gradient: Some(Gradient::Linear {
                start: [0.0, 0.0],
                end: [1.0, 1.0],
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
            }),
        };
        let mut cmd = desc.into_command();
        assert!(cmd.execute(&mut scene));
        assert!(scene.get(id).expect("exists").fill_gradient.is_some());
    }
}
