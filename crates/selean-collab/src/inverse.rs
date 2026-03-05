//! Compute the undo inverse of a [`CommandDescriptor`] against the current scene state.
//!
//! For property descriptors: reads the current value from the scene and produces
//! a descriptor that restores it. For hierarchy descriptors: snapshots structural
//! state so it can be restored.
//!
//! Returns `None` if the target node no longer exists (the operation is a no-op).

use selean_common::types::NodeId;
use selean_engine::command::CommandDescriptor;
use selean_engine::scene::{SceneGraph, SceneNodeKind};

/// Computes the inverse [`CommandDescriptor`] that would undo `descriptor`.
///
/// Reads the **current** state of the scene to determine what values to restore.
/// Returns `None` if the operation cannot be inverted (e.g., the target node
/// was deleted by a concurrent operation).
#[must_use]
pub fn compute_inverse(
    descriptor: &CommandDescriptor,
    scene: &SceneGraph,
) -> Option<CommandDescriptor> {
    match descriptor {
        CommandDescriptor::AddRoot { node } | CommandDescriptor::AddChild { node, .. } => {
            Some(CommandDescriptor::RemoveNode { node_id: node.id })
        }
        CommandDescriptor::RemoveNode { node_id } => inverse_remove(*node_id, scene),
        CommandDescriptor::Reparent { node_id, .. } => {
            let node = scene.get(*node_id)?;
            node.parent
                .map(|current_parent| CommandDescriptor::Reparent {
                    node_id: *node_id,
                    new_parent_id: current_parent,
                })
        }
        CommandDescriptor::ReorderChildren { parent_id, .. } => {
            let children = scene.children(*parent_id)?;
            Some(CommandDescriptor::ReorderChildren {
                parent_id: *parent_id,
                new_order: children.to_vec(),
            })
        }
        CommandDescriptor::ReparentToRoot { node_id } => {
            let node = scene.get(*node_id)?;
            node.parent
                .map(|current_parent| CommandDescriptor::Reparent {
                    node_id: *node_id,
                    new_parent_id: current_parent,
                })
        }
        CommandDescriptor::ReorderRoots { .. } => Some(CommandDescriptor::ReorderRoots {
            new_order: scene.roots().to_vec(),
        }),
        _ => inverse_property(descriptor, scene),
    }
}

/// Inverse for property descriptors: reads the current value and produces a
/// descriptor that restores it.
fn inverse_property(
    descriptor: &CommandDescriptor,
    scene: &SceneGraph,
) -> Option<CommandDescriptor> {
    match descriptor {
        CommandDescriptor::SetBounds { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetBounds {
                node_id: *node_id,
                bounds: node.bounds,
            })
        }
        CommandDescriptor::SetFill { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetFill {
                node_id: *node_id,
                fill: node.fill,
            })
        }
        CommandDescriptor::SetStroke { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetStroke {
                node_id: *node_id,
                stroke: node.stroke,
            })
        }
        CommandDescriptor::SetStrokeWidth { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetStrokeWidth {
                node_id: *node_id,
                width: node.stroke_width,
            })
        }
        CommandDescriptor::SetOpacity { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetOpacity {
                node_id: *node_id,
                opacity: node.opacity,
            })
        }
        CommandDescriptor::SetVisible { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetVisible {
                node_id: *node_id,
                visible: node.visible,
            })
        }
        CommandDescriptor::SetName { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetName {
                node_id: *node_id,
                name: node.name.clone(),
            })
        }
        CommandDescriptor::SetBlendMode { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetBlendMode {
                node_id: *node_id,
                blend_mode: node.blend_mode,
            })
        }
        CommandDescriptor::SetClipMode { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetClipMode {
                node_id: *node_id,
                clip_mode: node.clip_mode,
            })
        }
        CommandDescriptor::SetTransform { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetTransform {
                node_id: *node_id,
                transform: *node.local_transform.raw(),
            })
        }
        CommandDescriptor::SetScrollOffset { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetScrollOffset {
                node_id: *node_id,
                offset: node.scroll_offset,
            })
        }
        CommandDescriptor::SetFillGradient { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetFillGradient {
                node_id: *node_id,
                fill_gradient: node.fill_gradient.clone(),
            })
        }
        CommandDescriptor::SetEffects { node_id, .. } => {
            let node = scene.get(*node_id)?;
            Some(CommandDescriptor::SetEffects {
                node_id: *node_id,
                effects: node.effects.clone(),
            })
        }
        _ => inverse_kind_specific(descriptor, scene),
    }
}

/// Inverse for kind-specific descriptors (vector, image, frame properties).
fn inverse_kind_specific(
    descriptor: &CommandDescriptor,
    scene: &SceneGraph,
) -> Option<CommandDescriptor> {
    match descriptor {
        CommandDescriptor::SetPathData { node_id, .. } => {
            let node = scene.get(*node_id)?;
            match &node.kind {
                SceneNodeKind::Vector { path_data } => Some(CommandDescriptor::SetPathData {
                    node_id: *node_id,
                    path_data: path_data.clone(),
                }),
                _ => None,
            }
        }
        CommandDescriptor::SetAssetRef { node_id, .. } => {
            let node = scene.get(*node_id)?;
            match &node.kind {
                SceneNodeKind::Image { asset_ref } => Some(CommandDescriptor::SetAssetRef {
                    node_id: *node_id,
                    asset_ref: asset_ref.clone(),
                }),
                _ => None,
            }
        }
        CommandDescriptor::SetCornerRadius { node_id, .. } => {
            let node = scene.get(*node_id)?;
            match &node.kind {
                SceneNodeKind::Frame { corner_radius } => {
                    Some(CommandDescriptor::SetCornerRadius {
                        node_id: *node_id,
                        corner_radius: *corner_radius,
                    })
                }
                _ => None,
            }
        }
        _ => inverse_text_property(descriptor, scene),
    }
}

/// Inverse for text-specific property descriptors.
fn inverse_text_property(
    descriptor: &CommandDescriptor,
    scene: &SceneGraph,
) -> Option<CommandDescriptor> {
    match descriptor {
        CommandDescriptor::SetTextContent { node_id, .. } => {
            let node = scene.get(*node_id)?;
            match &node.kind {
                SceneNodeKind::Text { content, .. } => Some(CommandDescriptor::SetTextContent {
                    node_id: *node_id,
                    content: content.clone(),
                }),
                _ => None,
            }
        }
        CommandDescriptor::SetFontSize { node_id, .. } => {
            let node = scene.get(*node_id)?;
            match &node.kind {
                SceneNodeKind::Text { font_size, .. } => Some(CommandDescriptor::SetFontSize {
                    node_id: *node_id,
                    font_size: *font_size,
                }),
                _ => None,
            }
        }
        CommandDescriptor::SetFontFamily { node_id, .. } => {
            let node = scene.get(*node_id)?;
            match &node.kind {
                SceneNodeKind::Text { font_family, .. } => Some(CommandDescriptor::SetFontFamily {
                    node_id: *node_id,
                    font_family: font_family.clone(),
                }),
                _ => None,
            }
        }
        CommandDescriptor::SetFontWeight { node_id, .. } => {
            let node = scene.get(*node_id)?;
            match &node.kind {
                SceneNodeKind::Text { font_weight, .. } => Some(CommandDescriptor::SetFontWeight {
                    node_id: *node_id,
                    font_weight: *font_weight,
                }),
                _ => None,
            }
        }
        CommandDescriptor::SetFontStyle { node_id, .. } => {
            let node = scene.get(*node_id)?;
            match &node.kind {
                SceneNodeKind::Text { font_style, .. } => Some(CommandDescriptor::SetFontStyle {
                    node_id: *node_id,
                    font_style: *font_style,
                }),
                _ => None,
            }
        }
        CommandDescriptor::SetTextAlign { node_id, .. } => {
            let node = scene.get(*node_id)?;
            match &node.kind {
                SceneNodeKind::Text { text_align, .. } => Some(CommandDescriptor::SetTextAlign {
                    node_id: *node_id,
                    text_align: *text_align,
                }),
                _ => None,
            }
        }
        CommandDescriptor::SetLineHeight { node_id, .. } => {
            let node = scene.get(*node_id)?;
            match &node.kind {
                SceneNodeKind::Text { line_height, .. } => Some(CommandDescriptor::SetLineHeight {
                    node_id: *node_id,
                    line_height: *line_height,
                }),
                _ => None,
            }
        }
        CommandDescriptor::SetTextColor { node_id, .. } => {
            let node = scene.get(*node_id)?;
            match &node.kind {
                SceneNodeKind::Text { text_color, .. } => Some(CommandDescriptor::SetTextColor {
                    node_id: *node_id,
                    text_color: *text_color,
                }),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Inverse for `RemoveNode`: snapshot the node and produce `AddRoot` or `AddChild`.
fn inverse_remove(node_id: NodeId, scene: &SceneGraph) -> Option<CommandDescriptor> {
    let node = scene.get(node_id)?;
    let cloned = node.clone();
    match node.parent {
        Some(parent_id) => Some(CommandDescriptor::AddChild {
            parent_id,
            node: cloned,
        }),
        None => Some(CommandDescriptor::AddRoot { node: cloned }),
    }
}

/// Extracts the target `NodeId` from a descriptor, if it targets a specific node.
#[must_use]
pub fn target_node_id(descriptor: &CommandDescriptor) -> Option<NodeId> {
    match descriptor {
        CommandDescriptor::SetBounds { node_id, .. }
        | CommandDescriptor::SetFill { node_id, .. }
        | CommandDescriptor::SetFillGradient { node_id, .. }
        | CommandDescriptor::SetStroke { node_id, .. }
        | CommandDescriptor::SetStrokeWidth { node_id, .. }
        | CommandDescriptor::SetOpacity { node_id, .. }
        | CommandDescriptor::SetVisible { node_id, .. }
        | CommandDescriptor::SetName { node_id, .. }
        | CommandDescriptor::SetTextContent { node_id, .. }
        | CommandDescriptor::SetFontSize { node_id, .. }
        | CommandDescriptor::SetBlendMode { node_id, .. }
        | CommandDescriptor::SetClipMode { node_id, .. }
        | CommandDescriptor::SetTransform { node_id, .. }
        | CommandDescriptor::SetScrollOffset { node_id, .. }
        | CommandDescriptor::SetPathData { node_id, .. }
        | CommandDescriptor::SetAssetRef { node_id, .. }
        | CommandDescriptor::SetCornerRadius { node_id, .. }
        | CommandDescriptor::SetFontFamily { node_id, .. }
        | CommandDescriptor::SetFontWeight { node_id, .. }
        | CommandDescriptor::SetFontStyle { node_id, .. }
        | CommandDescriptor::SetTextAlign { node_id, .. }
        | CommandDescriptor::SetLineHeight { node_id, .. }
        | CommandDescriptor::SetTextColor { node_id, .. }
        | CommandDescriptor::SetEffects { node_id, .. }
        | CommandDescriptor::RemoveNode { node_id }
        | CommandDescriptor::Reparent { node_id, .. }
        | CommandDescriptor::ReparentToRoot { node_id } => Some(*node_id),
        CommandDescriptor::AddRoot { node } | CommandDescriptor::AddChild { node, .. } => {
            Some(node.id)
        }
        CommandDescriptor::ReorderChildren { parent_id, .. } => Some(*parent_id),
        CommandDescriptor::ReorderRoots { .. } => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use selean_engine::command::descriptor::create_frame_node;
    use selean_engine::scene::{
        BlendMode, BoundingBox, ClipMode, Color, FontStyle, SceneNode, SceneNodeKind, TextAlign,
    };

    fn text_node(name: &str, content: &str, font_size: f32) -> SceneNode {
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

    fn vector_node(name: &str, path: &str) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Vector {
                path_data: path.to_string(),
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        )
    }

    fn image_node(name: &str, asset: &str) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Image {
                asset_ref: asset.to_string(),
            },
            BoundingBox::new(0.0, 0.0, 200.0, 200.0),
        )
    }

    // --- Property inverse tests ---

    #[test]
    fn inverse_set_bounds() {
        let mut scene = SceneGraph::new();
        let node = create_frame_node("A", 10.0, 20.0, 100.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetBounds {
            node_id: id,
            bounds: BoundingBox::new(0.0, 0.0, 200.0, 100.0),
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetBounds { node_id, bounds } => {
                assert_eq!(node_id, id);
                assert_eq!(bounds.x, 10.0);
                assert_eq!(bounds.width, 100.0);
            }
            _ => panic!("expected SetBounds inverse"),
        }
    }

    #[test]
    fn inverse_set_fill() {
        let mut scene = SceneGraph::new();
        let mut node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        node.fill = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetFill {
            node_id: id,
            fill: Some(Color::new(0.0, 1.0, 0.0, 1.0)),
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetFill { fill, .. } => {
                let c = fill.unwrap();
                assert_eq!(c.r, 1.0);
                assert_eq!(c.g, 0.0);
            }
            _ => panic!("expected SetFill inverse"),
        }
    }

    #[test]
    fn inverse_set_fill_gradient() {
        use selean_engine::scene::{Gradient, GradientStop};

        let mut scene = SceneGraph::new();
        let mut node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        node.fill_gradient = Some(Gradient::Linear {
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
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetFillGradient {
            node_id: id,
            fill_gradient: None,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetFillGradient { fill_gradient, .. } => {
                assert!(fill_gradient.is_some());
            }
            _ => panic!("expected SetFillGradient inverse"),
        }
    }

    #[test]
    fn inverse_set_stroke() {
        let mut scene = SceneGraph::new();
        let mut node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        node.stroke = Some(Color::BLACK);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetStroke {
            node_id: id,
            stroke: None,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetStroke { stroke, .. } => {
                assert!(stroke.is_some());
            }
            _ => panic!("expected SetStroke inverse"),
        }
    }

    #[test]
    fn inverse_set_stroke_width() {
        let mut scene = SceneGraph::new();
        let mut node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        node.stroke_width = 3.0;
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetStrokeWidth {
            node_id: id,
            width: 5.0,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetStrokeWidth { width, .. } => assert_eq!(width, 3.0),
            _ => panic!("expected SetStrokeWidth inverse"),
        }
    }

    #[test]
    fn inverse_set_opacity() {
        let mut scene = SceneGraph::new();
        let mut node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        node.opacity = 0.5;
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetOpacity {
            node_id: id,
            opacity: 1.0,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetOpacity { opacity, .. } => assert_eq!(opacity, 0.5),
            _ => panic!("expected SetOpacity inverse"),
        }
    }

    #[test]
    fn inverse_set_visible() {
        let mut scene = SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetVisible {
            node_id: id,
            visible: false,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetVisible { visible, .. } => assert!(visible),
            _ => panic!("expected SetVisible inverse"),
        }
    }

    #[test]
    fn inverse_set_name() {
        let mut scene = SceneGraph::new();
        let node = create_frame_node("OldName", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetName {
            node_id: id,
            name: "NewName".to_string(),
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetName { name, .. } => assert_eq!(name, "OldName"),
            _ => panic!("expected SetName inverse"),
        }
    }

    #[test]
    fn inverse_set_text_content() {
        let mut scene = SceneGraph::new();
        let node = text_node("T", "Hello", 16.0);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetTextContent {
            node_id: id,
            content: "World".to_string(),
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetTextContent { content, .. } => assert_eq!(content, "Hello"),
            _ => panic!("expected SetTextContent inverse"),
        }
    }

    #[test]
    fn inverse_set_font_size() {
        let mut scene = SceneGraph::new();
        let node = text_node("T", "Hi", 16.0);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetFontSize {
            node_id: id,
            font_size: 24.0,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetFontSize { font_size, .. } => assert_eq!(font_size, 16.0),
            _ => panic!("expected SetFontSize inverse"),
        }
    }

    #[test]
    fn inverse_set_blend_mode() {
        let mut scene = SceneGraph::new();
        let mut node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        node.blend_mode = BlendMode::Multiply;
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetBlendMode {
            node_id: id,
            blend_mode: BlendMode::Screen,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetBlendMode { blend_mode, .. } => {
                assert_eq!(blend_mode, BlendMode::Multiply);
            }
            _ => panic!("expected SetBlendMode inverse"),
        }
    }

    #[test]
    fn inverse_set_clip_mode() {
        let mut scene = SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetClipMode {
            node_id: id,
            clip_mode: ClipMode::Scissor,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetClipMode { clip_mode, .. } => {
                assert_eq!(clip_mode, ClipMode::None);
            }
            _ => panic!("expected SetClipMode inverse"),
        }
    }

    #[test]
    fn inverse_set_transform() {
        let mut scene = SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetTransform {
            node_id: id,
            transform: [2.0, 0.0, 0.0, 2.0, 10.0, 20.0],
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetTransform { transform, .. } => {
                // Identity: [1, 0, 0, 1, 0, 0]
                assert_eq!(transform[0], 1.0);
                assert_eq!(transform[3], 1.0);
                assert_eq!(transform[4], 0.0);
            }
            _ => panic!("expected SetTransform inverse"),
        }
    }

    #[test]
    fn inverse_set_scroll_offset() {
        let mut scene = SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetScrollOffset {
            node_id: id,
            offset: [100.0, 200.0],
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetScrollOffset { offset, .. } => {
                assert_eq!(offset, [0.0, 0.0]);
            }
            _ => panic!("expected SetScrollOffset inverse"),
        }
    }

    #[test]
    fn inverse_set_path_data() {
        let mut scene = SceneGraph::new();
        let node = vector_node("V", "M 0 0 L 100 100");
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetPathData {
            node_id: id,
            path_data: "M 0 0 L 50 50".to_string(),
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetPathData { path_data, .. } => {
                assert_eq!(path_data, "M 0 0 L 100 100");
            }
            _ => panic!("expected SetPathData inverse"),
        }
    }

    #[test]
    fn inverse_set_asset_ref() {
        let mut scene = SceneGraph::new();
        let node = image_node("I", "old.png");
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetAssetRef {
            node_id: id,
            asset_ref: "new.png".to_string(),
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetAssetRef { asset_ref, .. } => {
                assert_eq!(asset_ref, "old.png");
            }
            _ => panic!("expected SetAssetRef inverse"),
        }
    }

    #[test]
    fn inverse_set_corner_radius() {
        let mut scene = SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [4.0, 4.0, 4.0, 4.0]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetCornerRadius {
            node_id: id,
            corner_radius: [8.0, 8.0, 8.0, 8.0],
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetCornerRadius { corner_radius, .. } => {
                assert_eq!(corner_radius, [4.0, 4.0, 4.0, 4.0]);
            }
            _ => panic!("expected SetCornerRadius inverse"),
        }
    }

    #[test]
    fn inverse_set_font_family() {
        let mut scene = SceneGraph::new();
        let node = text_node("T", "Hi", 16.0);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetFontFamily {
            node_id: id,
            font_family: "Roboto".to_string(),
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetFontFamily { font_family, .. } => {
                assert_eq!(font_family, "Inter");
            }
            _ => panic!("expected SetFontFamily inverse"),
        }
    }

    #[test]
    fn inverse_set_font_weight() {
        let mut scene = SceneGraph::new();
        let node = text_node("T", "Hi", 16.0);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetFontWeight {
            node_id: id,
            font_weight: 700,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetFontWeight { font_weight, .. } => {
                assert_eq!(font_weight, 400);
            }
            _ => panic!("expected SetFontWeight inverse"),
        }
    }

    #[test]
    fn inverse_set_font_style() {
        let mut scene = SceneGraph::new();
        let node = text_node("T", "Hi", 16.0);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetFontStyle {
            node_id: id,
            font_style: FontStyle::Italic,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetFontStyle { font_style, .. } => {
                assert_eq!(font_style, FontStyle::Normal);
            }
            _ => panic!("expected SetFontStyle inverse"),
        }
    }

    #[test]
    fn inverse_set_text_align() {
        let mut scene = SceneGraph::new();
        let node = text_node("T", "Hi", 16.0);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetTextAlign {
            node_id: id,
            text_align: TextAlign::Center,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetTextAlign { text_align, .. } => {
                assert_eq!(text_align, TextAlign::Left);
            }
            _ => panic!("expected SetTextAlign inverse"),
        }
    }

    #[test]
    fn inverse_set_line_height() {
        let mut scene = SceneGraph::new();
        let node = text_node("T", "Hi", 16.0);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetLineHeight {
            node_id: id,
            line_height: 2.0,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetLineHeight { line_height, .. } => {
                assert_eq!(line_height, 1.2);
            }
            _ => panic!("expected SetLineHeight inverse"),
        }
    }

    #[test]
    fn inverse_set_text_color() {
        let mut scene = SceneGraph::new();
        let node = text_node("T", "Hi", 16.0);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetTextColor {
            node_id: id,
            text_color: Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetTextColor { text_color, .. } => {
                assert!(text_color.is_none());
            }
            _ => panic!("expected SetTextColor inverse"),
        }
    }

    // --- Hierarchy inverse tests ---

    #[test]
    fn inverse_add_root() {
        let scene = SceneGraph::new();
        let node = create_frame_node("R", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;

        let desc = CommandDescriptor::AddRoot { node };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::RemoveNode { node_id } => assert_eq!(node_id, id),
            _ => panic!("expected RemoveNode inverse"),
        }
    }

    #[test]
    fn inverse_add_child() {
        let mut scene = SceneGraph::new();
        let parent = create_frame_node("P", 0.0, 0.0, 200.0, 200.0, None, [0.0; 4]);
        let parent_id = parent.id;
        scene.add_root(parent);

        let child = create_frame_node("C", 10.0, 10.0, 50.0, 50.0, None, [0.0; 4]);
        let child_id = child.id;

        let desc = CommandDescriptor::AddChild {
            parent_id,
            node: child,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::RemoveNode { node_id } => assert_eq!(node_id, child_id),
            _ => panic!("expected RemoveNode inverse"),
        }
    }

    #[test]
    fn inverse_remove_root_node() {
        let mut scene = SceneGraph::new();
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::RemoveNode { node_id: id };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::AddRoot { node } => assert_eq!(node.id, id),
            _ => panic!("expected AddRoot inverse for root removal"),
        }
    }

    #[test]
    fn inverse_remove_child_node() {
        let mut scene = SceneGraph::new();
        let parent = create_frame_node("P", 0.0, 0.0, 200.0, 200.0, None, [0.0; 4]);
        let parent_id = parent.id;
        scene.add_root(parent);

        let child = create_frame_node("C", 10.0, 10.0, 50.0, 50.0, None, [0.0; 4]);
        let child_id = child.id;
        scene.add_child(parent_id, child);

        let desc = CommandDescriptor::RemoveNode { node_id: child_id };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::AddChild {
                parent_id: pid,
                node,
            } => {
                assert_eq!(pid, parent_id);
                assert_eq!(node.id, child_id);
            }
            _ => panic!("expected AddChild inverse for child removal"),
        }
    }

    #[test]
    fn inverse_reparent() {
        let mut scene = SceneGraph::new();
        let parent_a = create_frame_node("A", 0.0, 0.0, 200.0, 200.0, None, [0.0; 4]);
        let parent_b = create_frame_node("B", 300.0, 0.0, 200.0, 200.0, None, [0.0; 4]);
        let first_parent_id = parent_a.id;
        let second_parent_id = parent_b.id;
        scene.add_root(parent_a);
        scene.add_root(parent_b);

        let child = create_frame_node("C", 10.0, 10.0, 50.0, 50.0, None, [0.0; 4]);
        let child_id = child.id;
        scene.add_child(first_parent_id, child);

        let desc = CommandDescriptor::Reparent {
            node_id: child_id,
            new_parent_id: second_parent_id,
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::Reparent {
                node_id,
                new_parent_id,
            } => {
                assert_eq!(node_id, child_id);
                assert_eq!(new_parent_id, first_parent_id);
            }
            _ => panic!("expected Reparent inverse"),
        }
    }

    #[test]
    fn inverse_reorder_children() {
        let mut scene = SceneGraph::new();
        let parent = create_frame_node("P", 0.0, 0.0, 200.0, 200.0, None, [0.0; 4]);
        let parent_id = parent.id;
        scene.add_root(parent);

        let c1 = create_frame_node("C1", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let c2 = create_frame_node("C2", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let c1_id = c1.id;
        let c2_id = c2.id;
        scene.add_child(parent_id, c1);
        scene.add_child(parent_id, c2);

        let desc = CommandDescriptor::ReorderChildren {
            parent_id,
            new_order: vec![c2_id, c1_id],
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::ReorderChildren {
                parent_id: pid,
                new_order,
            } => {
                assert_eq!(pid, parent_id);
                assert_eq!(new_order, vec![c1_id, c2_id]);
            }
            _ => panic!("expected ReorderChildren inverse"),
        }
    }

    #[test]
    fn inverse_reorder_roots() {
        let mut scene = SceneGraph::new();
        let r1 = create_frame_node("R1", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let r2 = create_frame_node("R2", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let r1_id = r1.id;
        let r2_id = r2.id;
        scene.add_root(r1);
        scene.add_root(r2);

        let desc = CommandDescriptor::ReorderRoots {
            new_order: vec![r2_id, r1_id],
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::ReorderRoots { new_order } => {
                assert_eq!(new_order, vec![r1_id, r2_id]);
            }
            _ => panic!("expected ReorderRoots inverse"),
        }
    }

    // --- Edge cases ---

    #[test]
    fn inverse_set_effects() {
        use selean_engine::scene::{Color, Effect};

        let mut scene = SceneGraph::new();
        let mut node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        node.effects = vec![Effect::DropShadow {
            color: Color::new(0.0, 0.0, 0.0, 0.5),
            offset_x: 4.0,
            offset_y: 4.0,
            blur_radius: 8.0,
        }];
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetEffects {
            node_id: id,
            effects: vec![],
        };
        let inv = compute_inverse(&desc, &scene).unwrap();
        match inv {
            CommandDescriptor::SetEffects { effects, .. } => {
                assert_eq!(effects.len(), 1);
            }
            _ => panic!("expected SetEffects inverse"),
        }
    }

    #[test]
    fn inverse_deleted_node_returns_none() {
        let scene = SceneGraph::new();
        let desc = CommandDescriptor::SetOpacity {
            node_id: NodeId::new(),
            opacity: 0.5,
        };
        assert!(compute_inverse(&desc, &scene).is_none());
    }

    #[test]
    fn inverse_text_content_on_frame_returns_none() {
        let mut scene = SceneGraph::new();
        let node = create_frame_node("F", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let desc = CommandDescriptor::SetTextContent {
            node_id: id,
            content: "X".to_string(),
        };
        assert!(compute_inverse(&desc, &scene).is_none());
    }

    #[test]
    fn inverse_remove_nonexistent_returns_none() {
        let scene = SceneGraph::new();
        let desc = CommandDescriptor::RemoveNode {
            node_id: NodeId::new(),
        };
        assert!(compute_inverse(&desc, &scene).is_none());
    }

    #[test]
    fn target_node_id_property() {
        let id = NodeId::new();
        let desc = CommandDescriptor::SetOpacity {
            node_id: id,
            opacity: 0.5,
        };
        assert_eq!(target_node_id(&desc), Some(id));
    }

    #[test]
    fn target_node_id_reorder_roots_is_none() {
        let desc = CommandDescriptor::ReorderRoots { new_order: vec![] };
        assert_eq!(target_node_id(&desc), None);
    }
}
