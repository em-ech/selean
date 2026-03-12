//! Scene graph query functions for the WASM boundary.
//!
//! These functions serialize scene state to JSON for consumption by the
//! React frontend. All return `String` (JSON) to cross the WASM FFI boundary.

use selean_common::types::NodeId;
use selean_engine::persistence::Document;
use selean_engine::renderer::Camera;
use selean_engine::scene::{SceneGraph, SceneNode};
use serde::{Deserialize, Serialize};

/// Serializable representation of a node for the frontend.
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node ID as a string.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Node kind tag ("Frame", "Text", "Image", "Vector", "Group").
    pub kind: String,
    /// Left edge x coordinate.
    pub x: f32,
    /// Top edge y coordinate.
    pub y: f32,
    /// Width in logical pixels.
    pub width: f32,
    /// Height in logical pixels.
    pub height: f32,
    /// Fill color (RGBA), if set.
    pub fill: Option<[f32; 4]>,
    /// Stroke color (RGBA), if set.
    pub stroke: Option<[f32; 4]>,
    /// Stroke width.
    pub stroke_width: f32,
    /// Opacity.
    pub opacity: f32,
    /// Visibility.
    pub visible: bool,
    /// Blend mode name.
    pub blend_mode: String,
    /// Clip mode name.
    pub clip_mode: String,
    /// Local transform as `[a, b, c, d, tx, ty]`.
    pub transform: [f32; 6],
    /// Scroll offset `[x, y]`.
    pub scroll_offset: [f32; 2],
    /// Corner radius `[tl, tr, br, bl]` (Frame kind only, zeros otherwise).
    pub corner_radius: [f32; 4],
    /// Text content (Text kind only).
    pub text_content: Option<String>,
    /// Font size (Text kind only).
    pub font_size: Option<f32>,
    /// Asset reference (Image kind only).
    pub asset_ref: Option<String>,
    /// SVG path data (Vector kind only).
    pub path_data: Option<String>,
    /// Font family (Text kind only).
    pub font_family: Option<String>,
    /// Font weight (Text kind only).
    pub font_weight: Option<u16>,
    /// Font style as string (Text kind only).
    pub font_style: Option<String>,
    /// Text alignment as string (Text kind only).
    pub text_align: Option<String>,
    /// Line height multiplier (Text kind only).
    pub line_height: Option<f32>,
    /// Text-specific color as RGBA (Text kind only).
    pub text_color: Option<[f32; 4]>,
    /// Child node IDs.
    pub children: Vec<String>,
    /// Parent node ID, if any.
    pub parent: Option<String>,
}

impl From<&SceneNode> for NodeInfo {
    #[allow(clippy::too_many_lines)]
    fn from(node: &SceneNode) -> Self {
        use selean_engine::scene::SceneNodeKind;

        let kind = node.kind.kind_tag();
        let (
            corner_radius,
            text_content,
            font_size,
            asset_ref,
            path_data,
            font_family,
            font_weight,
            font_style,
            text_align,
            line_height,
            text_color,
        ) = match &node.kind {
            SceneNodeKind::Frame { corner_radius } => (
                *corner_radius,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            SceneNodeKind::Text {
                content,
                font_size,
                font_family,
                font_weight,
                font_style,
                text_align,
                line_height,
                text_color,
            } => (
                [0.0; 4],
                Some(content.clone()),
                Some(*font_size),
                None,
                None,
                Some(font_family.clone()),
                Some(*font_weight),
                Some(font_style.to_string()),
                Some(text_align.to_string()),
                Some(*line_height),
                text_color.map(|c| [c.r, c.g, c.b, c.a]),
            ),
            SceneNodeKind::Image { asset_ref } => (
                [0.0; 4],
                None,
                None,
                Some(asset_ref.clone()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            SceneNodeKind::Vector { path_data } => (
                [0.0; 4],
                None,
                None,
                None,
                Some(path_data.clone()),
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            SceneNodeKind::Group => (
                [0.0; 4], None, None, None, None, None, None, None, None, None, None,
            ),
        };

        Self {
            id: node.id.to_string(),
            name: node.name.clone(),
            kind: kind.to_string(),
            x: node.bounds.x,
            y: node.bounds.y,
            width: node.bounds.width,
            height: node.bounds.height,
            fill: node.fill.map(|c| [c.r, c.g, c.b, c.a]),
            stroke: node.stroke.map(|c| [c.r, c.g, c.b, c.a]),
            stroke_width: node.stroke_width,
            opacity: node.opacity,
            visible: node.visible,
            blend_mode: node.blend_mode.as_str().to_string(),
            clip_mode: node.clip_mode.as_str().to_string(),
            transform: *node.local_transform.raw(),
            scroll_offset: node.scroll_offset,
            corner_radius,
            text_content,
            font_size,
            asset_ref,
            path_data,
            font_family,
            font_weight,
            font_style,
            text_align,
            line_height,
            text_color,
            children: node.children.iter().map(ToString::to_string).collect(),
            parent: node.parent.map(|id| id.to_string()),
        }
    }
}

/// Serializable representation of the entire scene for the frontend.
#[derive(Debug, Serialize, Deserialize)]
pub struct SceneInfo {
    /// All nodes in the scene.
    pub nodes: Vec<NodeInfo>,
    /// Root node IDs in order.
    pub roots: Vec<String>,
    /// Total node count.
    pub node_count: usize,
}

/// Returns JSON representation of a single node, or `null` if not found.
pub fn get_node_json(scene: &SceneGraph, node_id_str: &str) -> String {
    let Ok(uuid) = uuid::Uuid::parse_str(node_id_str) else {
        return "null".to_string();
    };
    let node_id = NodeId::from_uuid(uuid);
    match scene.get(node_id) {
        Some(node) => {
            let info = NodeInfo::from(node);
            serde_json::to_string(&info).unwrap_or_else(|_| "null".to_string())
        }
        None => "null".to_string(),
    }
}

/// Returns JSON array of selected node ID strings.
pub fn get_selected_ids_json(selected: &[NodeId]) -> String {
    let ids: Vec<String> = selected.iter().map(ToString::to_string).collect();
    serde_json::to_string(&ids).unwrap_or_else(|_| "[]".to_string())
}

/// Compact node summary for query results.
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeSummary {
    /// Node ID.
    pub id: String,
    /// Node name.
    pub name: String,
    /// Node kind tag.
    pub kind: String,
}

/// Searches nodes by optional name substring (case-insensitive) and optional kind.
/// Returns JSON array of `NodeSummary`.
pub fn query_nodes_json(
    scene: &SceneGraph,
    name_pattern: Option<&str>,
    kind: Option<&str>,
) -> String {
    let pattern_lower = name_pattern.map(str::to_lowercase);

    let results: Vec<NodeSummary> = scene
        .nodes()
        .values()
        .filter(|node| {
            if let Some(ref pat) = pattern_lower {
                if !node.name.to_lowercase().contains(pat) {
                    return false;
                }
            }
            if let Some(k) = &kind {
                if !node.kind.kind_tag().eq_ignore_ascii_case(k) {
                    return false;
                }
            }
            true
        })
        .map(|node| NodeSummary {
            id: node.id.to_string(),
            name: node.name.clone(),
            kind: node.kind.kind_tag().to_string(),
        })
        .collect();

    serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string())
}

/// Returns JSON representation of the entire scene graph.
pub fn get_scene_json(scene: &SceneGraph) -> String {
    let nodes: Vec<NodeInfo> = scene.nodes().values().map(NodeInfo::from).collect();
    let roots: Vec<String> = scene.roots().iter().map(ToString::to_string).collect();
    let info = SceneInfo {
        node_count: nodes.len(),
        nodes,
        roots,
    };
    serde_json::to_string(&info).unwrap_or_else(|_| "{}".to_string())
}

/// Serializable page metadata for the frontend.
#[derive(Debug, Serialize, Deserialize)]
pub struct PageInfo {
    /// Page ID as a string.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Page width.
    pub width: f32,
    /// Page height.
    pub height: f32,
    /// Number of nodes on this page.
    pub node_count: usize,
}

/// Returns JSON array of page metadata from a document.
pub fn get_pages_json(doc: &Document) -> String {
    let pages: Vec<PageInfo> = doc
        .pages()
        .iter()
        .map(|p| PageInfo {
            id: p.id.to_string(),
            name: p.name.clone(),
            width: p.width,
            height: p.height,
            node_count: p.scene.len(),
        })
        .collect();
    serde_json::to_string(&pages).unwrap_or_else(|_| "[]".to_string())
}

/// Serializable scene tree node for the layer panel.
#[derive(Debug, Serialize, Deserialize)]
pub struct TreeNode {
    /// Node ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Node kind tag.
    pub kind: String,
    /// Visibility.
    pub visible: bool,
    /// Recursive children.
    pub children: Vec<TreeNode>,
}

/// Builds a `TreeNode` recursively from the scene graph.
fn build_tree_node(scene: &SceneGraph, node_id: NodeId) -> Option<TreeNode> {
    let node = scene.get(node_id)?;
    let children = node
        .children
        .iter()
        .filter_map(|&cid| build_tree_node(scene, cid))
        .collect();
    Some(TreeNode {
        id: node.id.to_string(),
        name: node.name.clone(),
        kind: node.kind.kind_tag().to_string(),
        visible: node.visible,
        children,
    })
}

/// Returns JSON array of `TreeNode` from the scene roots.
pub fn get_scene_tree_json(scene: &SceneGraph) -> String {
    let tree: Vec<TreeNode> = scene
        .roots()
        .iter()
        .filter_map(|&rid| build_tree_node(scene, rid))
        .collect();
    serde_json::to_string(&tree).unwrap_or_else(|_| "[]".to_string())
}

/// Serializable bounding box for the selection overlay.
#[derive(Debug, Serialize, Deserialize)]
pub struct SelectionBounds {
    /// Node ID as a string.
    pub node_id: String,
    /// World-space x.
    pub x: f32,
    /// World-space y.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// Returns world-space bounds for each selected node as a `Vec`.
pub fn get_selected_bounds(scene: &SceneGraph, selected_ids: &[NodeId]) -> Vec<SelectionBounds> {
    selected_ids
        .iter()
        .filter_map(|&id| scene.get(id))
        .map(|node| {
            let world_bb = node.world_transform.transform_aabb(&node.bounds);
            SelectionBounds {
                node_id: node.id.to_string(),
                x: world_bb.x,
                y: world_bb.y,
                width: world_bb.width,
                height: world_bb.height,
            }
        })
        .collect()
}

/// Lightweight bounding box for snap guides: all non-selected nodes.
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeBoundsInfo {
    /// Node ID as a string.
    pub id: String,
    /// World-space x.
    pub x: f32,
    /// World-space y.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// Returns world-space bounds for all nodes except those in `excluded_ids`.
/// Used by `SnapGuides` to get alignment targets without full-scene serialization.
pub fn get_all_node_bounds(scene: &SceneGraph, excluded_ids: &[NodeId]) -> Vec<NodeBoundsInfo> {
    scene
        .nodes()
        .values()
        .filter(|node| !excluded_ids.contains(&node.id))
        .map(|node| {
            let world_bb = node.world_transform.transform_aabb(&node.bounds);
            NodeBoundsInfo {
                id: node.id.to_string(),
                x: world_bb.x,
                y: world_bb.y,
                width: world_bb.width,
                height: world_bb.height,
            }
        })
        .collect()
}

/// Returns JSON array of world-space bounds for each selected node.
pub fn get_selected_bounds_json(scene: &SceneGraph, selected_ids: &[NodeId]) -> String {
    let bounds: Vec<SelectionBounds> = selected_ids
        .iter()
        .filter_map(|&id| scene.get(id))
        .map(|node| {
            let world_bb = node.world_transform.transform_aabb(&node.bounds);
            SelectionBounds {
                node_id: node.id.to_string(),
                x: world_bb.x,
                y: world_bb.y,
                width: world_bb.width,
                height: world_bb.height,
            }
        })
        .collect();
    serde_json::to_string(&bounds).unwrap_or_else(|_| "[]".to_string())
}

/// Serializable camera state for the frontend overlay.
#[derive(Debug, Serialize, Deserialize)]
pub struct CameraInfo {
    /// Pan X in world space.
    pub pan_x: f32,
    /// Pan Y in world space.
    pub pan_y: f32,
    /// Zoom level.
    pub zoom: f32,
    /// Viewport width.
    pub viewport_width: f32,
    /// Viewport height.
    pub viewport_height: f32,
}

/// Returns camera info as a struct (for typed WASM returns).
pub fn get_camera_info(camera: &Camera) -> CameraInfo {
    let (pan_x, pan_y) = camera.pan();
    let (viewport_width, viewport_height) = camera.viewport_size();
    CameraInfo {
        pan_x,
        pan_y,
        zoom: camera.zoom(),
        viewport_width,
        viewport_height,
    }
}

/// Returns JSON representation of the camera state.
pub fn get_camera_json(camera: &Camera) -> String {
    let (pan_x, pan_y) = camera.pan();
    let (viewport_width, viewport_height) = camera.viewport_size();
    let info = CameraInfo {
        pan_x,
        pan_y,
        zoom: camera.zoom(),
        viewport_width,
        viewport_height,
    };
    serde_json::to_string(&info).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use selean_engine::scene::{BoundingBox, Color, SceneNode, SceneNodeKind};

    fn make_frame(name: &str, x: f32, y: f32, w: f32, h: f32) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(x, y, w, h),
        )
    }

    #[test]
    fn node_info_from_scene_node() {
        let mut node = make_frame("Test", 10.0, 20.0, 100.0, 50.0);
        node.fill = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        let info = NodeInfo::from(&node);
        assert_eq!(info.name, "Test");
        assert_eq!(info.kind, "Frame");
        assert!((info.x - 10.0).abs() < f32::EPSILON);
        assert!(info.fill.is_some());
        let fill = info.fill.unwrap();
        assert!((fill[0] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn get_node_json_returns_valid_json() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A", 0.0, 0.0, 50.0, 50.0);
        let id = node.id;
        scene.add_root(node);

        let json = get_node_json(&scene, &id.to_string());
        assert!(json.contains("\"name\":\"A\""));
        assert!(json.contains("\"kind\":\"Frame\""));
    }

    #[test]
    fn get_node_json_invalid_id_returns_null() {
        let scene = SceneGraph::new();
        let json = get_node_json(&scene, "not-a-uuid");
        assert_eq!(json, "null");
    }

    #[test]
    fn get_node_json_missing_id_returns_null() {
        let scene = SceneGraph::new();
        let id = NodeId::new();
        let json = get_node_json(&scene, &id.to_string());
        assert_eq!(json, "null");
    }

    #[test]
    fn get_selected_ids_json_empty() {
        let json = get_selected_ids_json(&[]);
        assert_eq!(json, "[]");
    }

    #[test]
    fn get_selected_ids_json_with_ids() {
        let ids = vec![NodeId::new(), NodeId::new()];
        let json = get_selected_ids_json(&ids);
        let parsed: Vec<String> = serde_json::from_str(&json).expect("valid json");
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn get_scene_json_empty_scene() {
        let scene = SceneGraph::new();
        let json = get_scene_json(&scene);
        let parsed: SceneInfo = serde_json::from_str(&json).expect("valid json");
        assert_eq!(parsed.node_count, 0);
        assert!(parsed.nodes.is_empty());
        assert!(parsed.roots.is_empty());
    }

    #[test]
    fn get_scene_json_with_nodes() {
        let mut scene = SceneGraph::new();
        scene.add_root(make_frame("A", 0.0, 0.0, 50.0, 50.0));
        scene.add_root(make_frame("B", 100.0, 0.0, 50.0, 50.0));

        let json = get_scene_json(&scene);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(parsed["node_count"], 2);
        assert_eq!(parsed["roots"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn node_info_group_kind() {
        let node = SceneNode::new(
            NodeId::new(),
            "Group1".to_string(),
            SceneNodeKind::Group,
            BoundingBox::new(0.0, 0.0, 0.0, 0.0),
        );
        let info = NodeInfo::from(&node);
        assert_eq!(info.kind, "Group");
    }

    #[test]
    fn node_info_text_kind() {
        use selean_engine::scene::{FontStyle, TextAlign};
        let node = SceneNode::new(
            NodeId::new(),
            "Label".to_string(),
            SceneNodeKind::Text {
                content: "Hello".to_string(),
                font_size: 16.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(0.0, 0.0, 100.0, 20.0),
        );
        let info = NodeInfo::from(&node);
        assert_eq!(info.kind, "Text");
    }

    #[test]
    fn node_info_no_fill_stroke() {
        let node = make_frame("Empty", 0.0, 0.0, 50.0, 50.0);
        let info = NodeInfo::from(&node);
        assert!(info.fill.is_none());
        assert!(info.stroke.is_none());
    }

    #[test]
    fn node_info_frame_corner_radius() {
        let node = SceneNode::new(
            NodeId::new(),
            "Rounded".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [4.0, 8.0, 12.0, 16.0],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        let info = NodeInfo::from(&node);
        assert_eq!(info.corner_radius, [4.0, 8.0, 12.0, 16.0]);
        assert!(info.text_content.is_none());
        assert!(info.font_size.is_none());
    }

    #[test]
    fn node_info_text_content_and_font_size() {
        use selean_engine::scene::{FontStyle, TextAlign};
        let node = SceneNode::new(
            NodeId::new(),
            "Label".to_string(),
            SceneNodeKind::Text {
                content: "Hello World".to_string(),
                font_size: 24.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(0.0, 0.0, 200.0, 30.0),
        );
        let info = NodeInfo::from(&node);
        assert_eq!(info.text_content.as_deref(), Some("Hello World"));
        assert_eq!(info.font_size, Some(24.0));
        assert_eq!(info.corner_radius, [0.0; 4]);
        assert_eq!(info.font_family.as_deref(), Some("Inter"));
        assert_eq!(info.font_weight, Some(400));
        assert_eq!(info.font_style.as_deref(), Some("Normal"));
        assert_eq!(info.text_align.as_deref(), Some("Left"));
        assert_eq!(info.line_height, Some(1.2));
        assert!(info.text_color.is_none());
    }

    #[test]
    fn node_info_image_asset_ref() {
        let node = SceneNode::new(
            NodeId::new(),
            "Photo".to_string(),
            SceneNodeKind::Image {
                asset_ref: "assets/photo.png".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 200.0, 150.0),
        );
        let info = NodeInfo::from(&node);
        assert_eq!(info.asset_ref.as_deref(), Some("assets/photo.png"));
        assert!(info.path_data.is_none());
    }

    #[test]
    fn node_info_vector_path_data() {
        let node = SceneNode::new(
            NodeId::new(),
            "Arrow".to_string(),
            SceneNodeKind::Vector {
                path_data: "M 0 0 L 50 50".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 50.0, 50.0),
        );
        let info = NodeInfo::from(&node);
        assert_eq!(info.path_data.as_deref(), Some("M 0 0 L 50 50"));
        assert!(info.asset_ref.is_none());
    }

    #[test]
    fn query_nodes_no_filter() {
        let mut scene = SceneGraph::new();
        scene.add_root(make_frame("A", 0.0, 0.0, 50.0, 50.0));
        scene.add_root(make_frame("B", 100.0, 0.0, 50.0, 50.0));

        let json = query_nodes_json(&scene, None, None);
        let results: Vec<NodeSummary> = serde_json::from_str(&json).expect("valid json");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_nodes_name_filter() {
        let mut scene = SceneGraph::new();
        scene.add_root(make_frame("Red Box", 0.0, 0.0, 50.0, 50.0));
        scene.add_root(make_frame("Blue Box", 100.0, 0.0, 50.0, 50.0));
        scene.add_root(make_frame("Green Circle", 200.0, 0.0, 50.0, 50.0));

        let json = query_nodes_json(&scene, Some("box"), None);
        let results: Vec<NodeSummary> = serde_json::from_str(&json).expect("valid json");
        assert_eq!(results.len(), 2);
        assert!(
            results
                .iter()
                .all(|n| n.name.to_lowercase().contains("box"))
        );
    }

    #[test]
    fn query_nodes_kind_filter() {
        let mut scene = SceneGraph::new();
        scene.add_root(make_frame("Box", 0.0, 0.0, 50.0, 50.0));
        scene.add_root(SceneNode::new(
            NodeId::new(),
            "Label".to_string(),
            SceneNodeKind::Text {
                content: "Hi".to_string(),
                font_size: 16.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: selean_engine::scene::FontStyle::Normal,
                text_align: selean_engine::scene::TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(0.0, 0.0, 100.0, 20.0),
        ));

        let json = query_nodes_json(&scene, None, Some("Text"));
        let results: Vec<NodeSummary> = serde_json::from_str(&json).expect("valid json");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "Text");
    }

    #[test]
    fn query_nodes_both_filters() {
        let mut scene = SceneGraph::new();
        scene.add_root(make_frame("Red Box", 0.0, 0.0, 50.0, 50.0));
        scene.add_root(make_frame("Blue Box", 100.0, 0.0, 50.0, 50.0));

        let json = query_nodes_json(&scene, Some("red"), Some("Frame"));
        let results: Vec<NodeSummary> = serde_json::from_str(&json).expect("valid json");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Red Box");
    }

    #[test]
    fn query_nodes_empty_result() {
        let scene = SceneGraph::new();
        let json = query_nodes_json(&scene, Some("nonexistent"), None);
        let results: Vec<NodeSummary> = serde_json::from_str(&json).expect("valid json");
        assert!(results.is_empty());
    }

    #[test]
    fn node_info_blend_and_clip_mode() {
        use selean_engine::scene::{BlendMode, ClipMode};
        let mut node = make_frame("Clipped", 0.0, 0.0, 100.0, 100.0);
        node.blend_mode = BlendMode::Multiply;
        node.clip_mode = ClipMode::Stencil;
        let info = NodeInfo::from(&node);
        assert_eq!(info.blend_mode, "Multiply");
        assert_eq!(info.clip_mode, "Stencil");
    }

    #[test]
    fn node_info_default_transform_and_scroll() {
        let node = make_frame("Default", 0.0, 0.0, 100.0, 100.0);
        let info = NodeInfo::from(&node);
        assert_eq!(info.transform, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        assert_eq!(info.scroll_offset, [0.0, 0.0]);
    }

    #[test]
    fn node_info_serializes_new_fields() {
        let mut node = make_frame("Full", 10.0, 20.0, 100.0, 50.0);
        node.fill = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        node.scroll_offset = [5.0, 10.0];
        let info = NodeInfo::from(&node);
        let json = serde_json::to_string(&info).expect("serialize");
        assert!(json.contains("\"blend_mode\":\"Normal\""));
        assert!(json.contains("\"clip_mode\":\"None\""));
        assert!(json.contains("\"scroll_offset\":[5.0,10.0]"));
        assert!(json.contains("\"corner_radius\":[0.0,0.0,0.0,0.0]"));
    }

    // --- Page and tree query tests ---

    #[test]
    fn get_pages_json_default_document() {
        let doc = Document::new();
        let json = get_pages_json(&doc);
        let pages: Vec<PageInfo> = serde_json::from_str(&json).expect("valid json");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].name, "Page 1");
        assert_eq!(pages[0].node_count, 0);
    }

    #[test]
    fn get_pages_json_multi_page() {
        let mut doc = Document::new();
        doc.active_page_mut()
            .scene
            .add_root(make_frame("A", 0.0, 0.0, 50.0, 50.0));
        doc.add_page("Page 2", 800.0, 600.0);

        let json = get_pages_json(&doc);
        let pages: Vec<PageInfo> = serde_json::from_str(&json).expect("valid json");
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].node_count, 1);
        assert_eq!(pages[1].node_count, 0);
        assert_eq!(pages[1].width, 800.0);
    }

    #[test]
    fn get_scene_tree_json_empty() {
        let scene = SceneGraph::new();
        let json = get_scene_tree_json(&scene);
        let tree: Vec<TreeNode> = serde_json::from_str(&json).expect("valid json");
        assert!(tree.is_empty());
    }

    #[test]
    fn get_scene_tree_json_flat() {
        let mut scene = SceneGraph::new();
        scene.add_root(make_frame("A", 0.0, 0.0, 50.0, 50.0));
        scene.add_root(make_frame("B", 100.0, 0.0, 50.0, 50.0));

        let json = get_scene_tree_json(&scene);
        let tree: Vec<TreeNode> = serde_json::from_str(&json).expect("valid json");
        assert_eq!(tree.len(), 2);
        assert!(tree[0].children.is_empty());
        assert!(tree[1].children.is_empty());
    }

    #[test]
    fn get_scene_tree_json_nested() {
        let mut scene = SceneGraph::new();
        let parent = make_frame("Parent", 0.0, 0.0, 200.0, 200.0);
        let pid = scene.add_root(parent);
        let child = make_frame("Child", 10.0, 10.0, 50.0, 50.0);
        scene.add_child(pid, child);

        let json = get_scene_tree_json(&scene);
        let tree: Vec<TreeNode> = serde_json::from_str(&json).expect("valid json");
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "Parent");
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].name, "Child");
    }

    #[test]
    fn get_selected_bounds_no_selection() {
        let scene = SceneGraph::new();
        let json = get_selected_bounds_json(&scene, &[]);
        let bounds: Vec<super::SelectionBounds> = serde_json::from_str(&json).expect("valid json");
        assert!(bounds.is_empty());
    }

    #[test]
    fn get_selected_bounds_single_node() {
        let mut scene = SceneGraph::new();
        let node = make_frame("A", 10.0, 20.0, 100.0, 50.0);
        let id = node.id;
        scene.add_root(node);

        let json = get_selected_bounds_json(&scene, &[id]);
        let bounds: Vec<super::SelectionBounds> = serde_json::from_str(&json).expect("valid json");
        assert_eq!(bounds.len(), 1);
        assert_eq!(bounds[0].node_id, id.to_string());
        assert!((bounds[0].x - 10.0).abs() < f32::EPSILON);
        assert!((bounds[0].width - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn get_selected_bounds_multi_node() {
        let mut scene = SceneGraph::new();
        let n1 = make_frame("A", 0.0, 0.0, 50.0, 50.0);
        let id1 = n1.id;
        scene.add_root(n1);
        let n2 = make_frame("B", 100.0, 100.0, 60.0, 40.0);
        let id2 = n2.id;
        scene.add_root(n2);

        let json = get_selected_bounds_json(&scene, &[id1, id2]);
        let bounds: Vec<super::SelectionBounds> = serde_json::from_str(&json).expect("valid json");
        assert_eq!(bounds.len(), 2);
    }

    #[test]
    fn get_selected_bounds_uses_world_transform() {
        use selean_engine::scene::Transform2D;

        let mut scene = SceneGraph::new();
        // Parent at (100, 200) with a child at local (10, 20).
        // World-space child bounds should be (110, 220).
        let parent = make_frame("Parent", 100.0, 200.0, 300.0, 300.0);
        let parent_id = parent.id;
        scene.add_root(parent);

        let child = make_frame("Child", 10.0, 20.0, 50.0, 30.0);
        let child_id = child.id;
        scene.add_child(parent_id, child);

        // Set a translation transform on the parent.
        scene.set_transform(parent_id, Transform2D::translation(100.0, 200.0));
        scene.recompute_world_transforms();

        let json = get_selected_bounds_json(&scene, &[child_id]);
        let bounds: Vec<super::SelectionBounds> = serde_json::from_str(&json).expect("valid json");
        assert_eq!(bounds.len(), 1);
        // Child world position: parent_transform * child_local = (100+10, 200+20) = (110, 220).
        assert!((bounds[0].x - 110.0).abs() < 0.1, "x was {}", bounds[0].x);
        assert!((bounds[0].y - 220.0).abs() < 0.1, "y was {}", bounds[0].y);
        assert!((bounds[0].width - 50.0).abs() < 0.1);
        assert!((bounds[0].height - 30.0).abs() < 0.1);
    }

    #[test]
    fn get_camera_json_defaults() {
        let camera = Camera::new(800.0, 600.0);
        let json = get_camera_json(&camera);
        let info: super::CameraInfo = serde_json::from_str(&json).expect("valid json");
        assert!((info.pan_x).abs() < f32::EPSILON);
        assert!((info.pan_y).abs() < f32::EPSILON);
        assert!((info.zoom - 1.0).abs() < f32::EPSILON);
        assert!((info.viewport_width - 800.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tree_node_includes_visibility() {
        let mut scene = SceneGraph::new();
        let mut node = make_frame("Hidden", 0.0, 0.0, 50.0, 50.0);
        node.visible = false;
        scene.add_root(node);

        let json = get_scene_tree_json(&scene);
        let tree: Vec<TreeNode> = serde_json::from_str(&json).expect("valid json");
        assert!(!tree[0].visible);
    }
}
