//! Converts Figma API types into Selean engine types.
//!
//! The main entry point is [`file_to_document`], which maps CANVAS children
//! to pages and iteratively converts nodes. Absolute bounding boxes from
//! Figma are converted to local coordinates by subtracting the parent's
//! absolute position.

use selean_common::types::{NodeId, PageId};
use selean_engine::persistence::{Document, Page};
use selean_engine::scene::{
    BoundingBox, Color, FontStyle, SceneGraph, SceneNode, SceneNodeKind, TextAlign,
};

use crate::FigmaError;
use crate::api::{FigmaFileResponse, FigmaNode, FigmaNodeType, FigmaTextStyle};
use crate::color::{figma_text_align, first_image_ref, first_solid_color};

/// Extracted text properties from a Figma node's style.
struct TextProps {
    font_size: f32,
    font_family: String,
    font_weight: u16,
    font_style: FontStyle,
    text_align: TextAlign,
    line_height: f32,
    text_color: Option<Color>,
}

/// A pending node conversion task for the iterative traversal stack.
struct ConvertTask<'a> {
    node: &'a FigmaNode,
    parent_id: Option<NodeId>,
    parent_abs_pos: (f32, f32),
}

/// Converts a Figma file response into a Selean `Document`.
///
/// Each CANVAS child of the root document becomes a page.
///
/// # Errors
///
/// Returns `FigmaError::InvalidFile` if there are no CANVAS children.
pub fn file_to_document(response: &FigmaFileResponse) -> Result<Document, FigmaError> {
    let canvases: Vec<&FigmaNode> = response
        .document
        .children
        .iter()
        .filter(|n| n.node_type == FigmaNodeType::Canvas)
        .collect();

    if canvases.is_empty() {
        return Err(FigmaError::InvalidFile(
            "no CANVAS children in document".to_string(),
        ));
    }

    let pages: Vec<Page> = canvases.iter().map(|c| canvas_to_page(c)).collect();
    Ok(Document::from_pages(pages))
}

/// Converts a Figma CANVAS node into a Selean `Page`.
///
/// Page dimensions are computed as the bounding union of all descendant nodes.
/// If no descendants have bounds, defaults to 1920x1080.
fn canvas_to_page(canvas: &FigmaNode) -> Page {
    let mut scene = SceneGraph::new();

    convert_nodes(&canvas.children, &mut scene);

    let (width, height) = compute_page_dimensions(&canvas.children);

    Page::with_scene(PageId::new(), &canvas.name, width, height, scene)
}

/// Iteratively converts Figma nodes and their children into scene nodes.
///
/// Uses an explicit stack instead of recursion. Children are pushed in
/// reverse order to maintain correct DFS processing order.
fn convert_nodes(root_children: &[FigmaNode], scene: &mut SceneGraph) {
    let mut stack: Vec<ConvertTask> = root_children
        .iter()
        .rev()
        .map(|n| ConvertTask {
            node: n,
            parent_id: None,
            parent_abs_pos: (0.0, 0.0),
        })
        .collect();

    while let Some(task) = stack.pop() {
        let Some(scene_node) = figma_node_to_scene_node(task.node, task.parent_abs_pos) else {
            continue;
        };

        let node_id = scene_node.id;

        if let Some(pid) = task.parent_id {
            scene.add_child(pid, scene_node);
        } else {
            scene.add_root(scene_node);
        }

        // Determine absolute position for children.
        let child_parent_pos = task
            .node
            .absolute_bounding_box
            .map_or(task.parent_abs_pos, |bb| (bb.x, bb.y));

        // Only process children of container types.
        if is_container(&task.node.node_type) {
            for child in task.node.children.iter().rev() {
                stack.push(ConvertTask {
                    node: child,
                    parent_id: Some(node_id),
                    parent_abs_pos: child_parent_pos,
                });
            }
        }
    }
}

/// Computes page dimensions from the bounding union of all descendant nodes.
///
/// Walks the full tree iteratively. Handles negative coordinates by extending
/// the page origin to `min(min_coord, 0)`. Falls back to 1920x1080 if no
/// descendants have bounds.
fn compute_page_dimensions(root_children: &[FigmaNode]) -> (f32, f32) {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut found = false;

    let mut stack: Vec<&FigmaNode> = root_children.iter().collect();

    while let Some(node) = stack.pop() {
        if let Some(ref bb) = node.absolute_bounding_box {
            found = true;
            min_x = min_x.min(bb.x);
            min_y = min_y.min(bb.y);
            max_x = max_x.max(bb.x + bb.width);
            max_y = max_y.max(bb.y + bb.height);
        }
        for child in &node.children {
            stack.push(child);
        }
    }

    if !found {
        return (1920.0, 1080.0);
    }

    // Page origin is min(min_coord, 0) to encompass negative positions.
    let page_left = min_x.min(0.0);
    let page_top = min_y.min(0.0);
    (max_x - page_left, max_y - page_top)
}

/// Returns `true` if the node type can contain children in the scene graph.
fn is_container(node_type: &FigmaNodeType) -> bool {
    matches!(
        node_type,
        FigmaNodeType::Frame
            | FigmaNodeType::Group
            | FigmaNodeType::Component
            | FigmaNodeType::ComponentSet
            | FigmaNodeType::Instance
    )
}

/// Converts a single Figma node into a Selean `SceneNode`.
///
/// Returns `None` for unsupported node types (DOCUMENT, CANVAS, Unknown, etc.).
fn figma_node_to_scene_node(node: &FigmaNode, parent_abs_pos: (f32, f32)) -> Option<SceneNode> {
    let kind = map_node_kind(node)?;
    let bounds = compute_local_bounds(node, parent_abs_pos);

    let mut scene_node = SceneNode::new(NodeId::new(), node.name.clone(), kind, bounds);

    scene_node.fill = first_solid_color(&node.fills);
    scene_node.stroke = first_solid_color(&node.strokes);
    scene_node.stroke_width = node.stroke_weight;
    scene_node.opacity = node.opacity;
    scene_node.visible = node.visible;

    Some(scene_node)
}

/// Maps a Figma node type to a `SceneNodeKind`.
///
/// Returns `None` for node types that have no scene graph representation.
fn map_node_kind(node: &FigmaNode) -> Option<SceneNodeKind> {
    // Check for image: node-level `imageRef` or IMAGE fill paint.
    if let Some(img_ref) = find_image_ref(node) {
        return Some(SceneNodeKind::Image { asset_ref: img_ref });
    }

    match node.node_type {
        FigmaNodeType::Frame
        | FigmaNodeType::Rectangle
        | FigmaNodeType::Component
        | FigmaNodeType::ComponentSet
        | FigmaNodeType::Instance => Some(SceneNodeKind::Frame {
            corner_radius: resolve_corner_radii(node),
        }),
        FigmaNodeType::Text => {
            let props = extract_text_props(node);
            Some(SceneNodeKind::Text {
                content: node.characters.clone().unwrap_or_default(),
                font_size: props.font_size,
                font_family: props.font_family,
                font_weight: props.font_weight,
                font_style: props.font_style,
                text_align: props.text_align,
                line_height: props.line_height,
                text_color: props.text_color,
            })
        }
        FigmaNodeType::Vector
        | FigmaNodeType::BooleanOperation
        | FigmaNodeType::Line
        | FigmaNodeType::RegularPolygon
        | FigmaNodeType::Star
        | FigmaNodeType::Ellipse => {
            let path_data = node
                .fill_geometry
                .first()
                .map_or_else(String::new, |p| p.path.clone());
            Some(SceneNodeKind::Vector { path_data })
        }
        FigmaNodeType::Group => Some(SceneNodeKind::Group),
        _ => {
            tracing::trace!(node_type = ?node.node_type, name = %node.name, "skipping unsupported node type");
            None
        }
    }
}

/// Resolves corner radii from per-corner or uniform values.
fn resolve_corner_radii(node: &FigmaNode) -> [f32; 4] {
    node.rectangle_corner_radii
        .unwrap_or([node.corner_radius; 4])
}

/// Computes local-space bounds by subtracting the parent's absolute position.
fn compute_local_bounds(node: &FigmaNode, parent_abs_pos: (f32, f32)) -> BoundingBox {
    node.absolute_bounding_box
        .map_or(BoundingBox::new(0.0, 0.0, 0.0, 0.0), |bb| {
            BoundingBox::new(
                bb.x - parent_abs_pos.0,
                bb.y - parent_abs_pos.1,
                bb.width,
                bb.height,
            )
        })
}

/// Extracts text properties from a Figma node's style.
fn extract_text_props(node: &FigmaNode) -> TextProps {
    let style = node.style.as_ref();

    let font_size = style.and_then(|s| s.font_size).unwrap_or(16.0);

    let font_family = style
        .and_then(|s| s.font_family.clone())
        .unwrap_or_else(|| "Inter".to_string());

    let font_weight = style.and_then(|s| s.font_weight).unwrap_or(400);

    let font_style = style.map_or(FontStyle::Normal, detect_italic);

    let text_align = style
        .and_then(|s| s.text_align_horizontal.as_deref())
        .map_or(TextAlign::Left, figma_text_align);

    let line_height = style
        .and_then(|s| s.line_height_px)
        .and_then(|lh_px| {
            if font_size > 0.0 {
                Some(lh_px / font_size)
            } else {
                None
            }
        })
        .unwrap_or(1.2);

    let text_color = first_solid_color(&node.fills);

    TextProps {
        font_size,
        font_family,
        font_weight,
        font_style,
        text_align,
        line_height,
        text_color,
    }
}

/// Detects italic font style from the PostScript name.
///
/// Checks for "italic" or "oblique" (case-insensitive) in
/// `fontPostScriptName`.
#[must_use]
pub fn detect_italic(style: &FigmaTextStyle) -> FontStyle {
    if let Some(ref psname) = style.font_post_script_name {
        let lower = psname.to_lowercase();
        if lower.contains("italic") || lower.contains("oblique") {
            return FontStyle::Italic;
        }
    }
    FontStyle::Normal
}

/// Checks both the node's `imageRef` field and IMAGE-type fills for an image reference.
#[must_use]
pub fn find_image_ref(node: &FigmaNode) -> Option<String> {
    if let Some(ref img_ref) = node.image_ref {
        return Some(img_ref.clone());
    }
    first_image_ref(&node.fills)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::api::*;

    fn make_node(node_type: FigmaNodeType) -> FigmaNode {
        FigmaNode {
            id: "1:1".to_string(),
            name: "Test".to_string(),
            node_type,
            absolute_bounding_box: Some(FigmaRect {
                x: 100.0,
                y: 200.0,
                width: 300.0,
                height: 150.0,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn frame_node_mapping() {
        let node = make_node(FigmaNodeType::Frame);
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(matches!(sn.kind, SceneNodeKind::Frame { .. }));
        assert_eq!(sn.bounds.x, 100.0);
        assert_eq!(sn.bounds.y, 200.0);
    }

    #[test]
    fn rectangle_node_mapping() {
        let node = make_node(FigmaNodeType::Rectangle);
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(matches!(sn.kind, SceneNodeKind::Frame { .. }));
    }

    #[test]
    fn component_node_mapping() {
        let node = make_node(FigmaNodeType::Component);
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(matches!(sn.kind, SceneNodeKind::Frame { .. }));
    }

    #[test]
    fn instance_node_mapping() {
        let node = make_node(FigmaNodeType::Instance);
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(matches!(sn.kind, SceneNodeKind::Frame { .. }));
    }

    #[test]
    fn text_node_mapping() {
        let node = FigmaNode {
            characters: Some("Hello".to_string()),
            style: Some(FigmaTextStyle {
                font_family: Some("Roboto".to_string()),
                font_weight: Some(700),
                font_size: Some(24.0),
                text_align_horizontal: Some("CENTER".to_string()),
                line_height_px: Some(32.0),
                ..Default::default()
            }),
            ..make_node(FigmaNodeType::Text)
        };

        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        match &sn.kind {
            SceneNodeKind::Text {
                content,
                font_size,
                font_family,
                font_weight,
                text_align,
                ..
            } => {
                assert_eq!(content, "Hello");
                assert_eq!(*font_size, 24.0);
                assert_eq!(font_family, "Roboto");
                assert_eq!(*font_weight, 700);
                assert_eq!(*text_align, TextAlign::Center);
            }
            _ => panic!("expected Text kind"),
        }
    }

    #[test]
    fn text_node_defaults() {
        let node = FigmaNode {
            characters: None,
            style: None,
            ..make_node(FigmaNodeType::Text)
        };

        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        match &sn.kind {
            SceneNodeKind::Text {
                content,
                font_size,
                font_family,
                font_weight,
                font_style,
                text_align,
                line_height,
                ..
            } => {
                assert_eq!(content, "");
                assert_eq!(*font_size, 16.0);
                assert_eq!(font_family, "Inter");
                assert_eq!(*font_weight, 400);
                assert_eq!(*font_style, FontStyle::Normal);
                assert_eq!(*text_align, TextAlign::Left);
                assert!((*line_height - 1.2).abs() < 1e-6);
            }
            _ => panic!("expected Text kind"),
        }
    }

    #[test]
    fn vector_node_mapping() {
        let node = FigmaNode {
            fill_geometry: vec![FigmaPath {
                path: "M 0 0 L 100 100".to_string(),
            }],
            ..make_node(FigmaNodeType::Vector)
        };
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        match &sn.kind {
            SceneNodeKind::Vector { path_data } => {
                assert_eq!(path_data, "M 0 0 L 100 100");
            }
            _ => panic!("expected Vector kind"),
        }
    }

    #[test]
    fn ellipse_maps_to_vector() {
        let node = make_node(FigmaNodeType::Ellipse);
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(matches!(sn.kind, SceneNodeKind::Vector { .. }));
    }

    #[test]
    fn group_node_mapping() {
        let node = make_node(FigmaNodeType::Group);
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(matches!(sn.kind, SceneNodeKind::Group));
    }

    #[test]
    fn unsupported_type_returns_none() {
        let node = make_node(FigmaNodeType::Unknown);
        assert!(figma_node_to_scene_node(&node, (0.0, 0.0)).is_none());
    }

    #[test]
    fn local_bounds_from_absolute() {
        let node = make_node(FigmaNodeType::Frame);
        let sn = figma_node_to_scene_node(&node, (50.0, 100.0)).unwrap();
        assert_eq!(sn.bounds.x, 50.0);
        assert_eq!(sn.bounds.y, 100.0);
        assert_eq!(sn.bounds.width, 300.0);
        assert_eq!(sn.bounds.height, 150.0);
    }

    #[test]
    fn hierarchy_with_coordinate_conversion() {
        let parent = FigmaNode {
            id: "1:1".to_string(),
            name: "Parent".to_string(),
            node_type: FigmaNodeType::Frame,
            absolute_bounding_box: Some(FigmaRect {
                x: 100.0,
                y: 200.0,
                width: 400.0,
                height: 300.0,
            }),
            children: vec![FigmaNode {
                id: "1:2".to_string(),
                name: "Child".to_string(),
                node_type: FigmaNodeType::Rectangle,
                absolute_bounding_box: Some(FigmaRect {
                    x: 150.0,
                    y: 250.0,
                    width: 100.0,
                    height: 50.0,
                }),
                ..Default::default()
            }],
            ..Default::default()
        };

        let mut scene = SceneGraph::new();
        convert_nodes(&[parent], &mut scene);

        // Parent at (100, 200) relative to canvas origin
        // Child at (150, 250) absolute -> (50, 50) local relative to parent
        assert_eq!(scene.len(), 2);

        let root_ids: Vec<_> = scene.roots().to_vec();
        assert_eq!(root_ids.len(), 1);

        let parent_node = scene.get(root_ids[0]).unwrap();
        assert_eq!(parent_node.bounds.x, 100.0);
        assert_eq!(parent_node.bounds.y, 200.0);
        assert_eq!(parent_node.children.len(), 1);

        let child_node = scene.get(parent_node.children[0]).unwrap();
        assert_eq!(child_node.bounds.x, 50.0);
        assert_eq!(child_node.bounds.y, 50.0);
    }

    #[test]
    fn three_level_nesting_coordinate_chain() {
        let grandchild = FigmaNode {
            id: "1:3".to_string(),
            name: "Grandchild".to_string(),
            node_type: FigmaNodeType::Rectangle,
            absolute_bounding_box: Some(FigmaRect {
                x: 170.0,
                y: 280.0,
                width: 50.0,
                height: 50.0,
            }),
            ..Default::default()
        };

        let child = FigmaNode {
            id: "1:2".to_string(),
            name: "Middle".to_string(),
            node_type: FigmaNodeType::Group,
            absolute_bounding_box: Some(FigmaRect {
                x: 150.0,
                y: 250.0,
                width: 200.0,
                height: 200.0,
            }),
            children: vec![grandchild],
            ..Default::default()
        };

        let root = FigmaNode {
            id: "1:1".to_string(),
            name: "Root".to_string(),
            node_type: FigmaNodeType::Frame,
            absolute_bounding_box: Some(FigmaRect {
                x: 100.0,
                y: 200.0,
                width: 400.0,
                height: 300.0,
            }),
            children: vec![child],
            ..Default::default()
        };

        let mut scene = SceneGraph::new();
        convert_nodes(&[root], &mut scene);

        assert_eq!(scene.len(), 3);

        let root_ids: Vec<_> = scene.roots().to_vec();
        let root_node = scene.get(root_ids[0]).unwrap();
        // Root: abs (100, 200), parent_abs (0, 0) -> local (100, 200)
        assert_eq!(root_node.bounds.x, 100.0);
        assert_eq!(root_node.bounds.y, 200.0);

        let mid_node = scene.get(root_node.children[0]).unwrap();
        // Middle: abs (150, 250), parent_abs (100, 200) -> local (50, 50)
        assert_eq!(mid_node.bounds.x, 50.0);
        assert_eq!(mid_node.bounds.y, 50.0);

        let leaf_node = scene.get(mid_node.children[0]).unwrap();
        // Grandchild: abs (170, 280), parent_abs (150, 250) -> local (20, 30)
        assert_eq!(leaf_node.bounds.x, 20.0);
        assert_eq!(leaf_node.bounds.y, 30.0);
    }

    #[test]
    fn invisible_node_preserved() {
        let node = FigmaNode {
            id: "1:1".to_string(),
            name: "Hidden".to_string(),
            node_type: FigmaNodeType::Frame,
            visible: false,
            opacity: 0.5,
            absolute_bounding_box: Some(FigmaRect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 50.0,
            }),
            ..Default::default()
        };

        let mut scene = SceneGraph::new();
        convert_nodes(&[node], &mut scene);

        assert_eq!(scene.len(), 1);
        let root_ids: Vec<_> = scene.roots().to_vec();
        let sn = scene.get(root_ids[0]).unwrap();
        assert!(!sn.visible);
        assert_eq!(sn.opacity, 0.5);
    }

    #[test]
    fn negative_coordinates_page_dimensions() {
        let children = vec![
            FigmaNode {
                absolute_bounding_box: Some(FigmaRect {
                    x: -100.0,
                    y: -50.0,
                    width: 200.0,
                    height: 100.0,
                }),
                ..make_node(FigmaNodeType::Frame)
            },
            FigmaNode {
                absolute_bounding_box: Some(FigmaRect {
                    x: 300.0,
                    y: 200.0,
                    width: 100.0,
                    height: 100.0,
                }),
                ..make_node(FigmaNodeType::Frame)
            },
        ];
        let (w, h) = compute_page_dimensions(&children);
        // min_x = -100, max_x = 400, page_left = -100, width = 400 - (-100) = 500
        // min_y = -50, max_y = 300, page_top = -50, height = 300 - (-50) = 350
        assert_eq!(w, 500.0);
        assert_eq!(h, 350.0);
    }

    #[test]
    fn empty_file_returns_error() {
        let response = FigmaFileResponse {
            name: "Empty".to_string(),
            document: FigmaNode {
                node_type: FigmaNodeType::Document,
                ..Default::default()
            },
        };
        let result = file_to_document(&response);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no CANVAS"));
    }

    #[test]
    fn detect_italic_from_postscript_name() {
        let style = FigmaTextStyle {
            font_family: Some("Inter".to_string()),
            font_post_script_name: Some("Inter-BoldItalic".to_string()),
            font_weight: Some(700),
            font_size: Some(16.0),
            ..Default::default()
        };
        assert_eq!(detect_italic(&style), FontStyle::Italic);
    }

    #[test]
    fn detect_italic_oblique() {
        let style = FigmaTextStyle {
            font_post_script_name: Some("SomeFont-Oblique".to_string()),
            ..Default::default()
        };
        assert_eq!(detect_italic(&style), FontStyle::Italic);
    }

    #[test]
    fn detect_italic_normal() {
        let style = FigmaTextStyle {
            font_post_script_name: Some("Inter-Regular".to_string()),
            ..Default::default()
        };
        assert_eq!(detect_italic(&style), FontStyle::Normal);
    }

    #[test]
    fn detect_italic_no_postscript() {
        let style = FigmaTextStyle::default();
        assert_eq!(detect_italic(&style), FontStyle::Normal);
    }

    #[test]
    fn image_ref_from_node_field() {
        let node = FigmaNode {
            image_ref: Some("img_abc".to_string()),
            ..make_node(FigmaNodeType::Frame)
        };
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(
            matches!(sn.kind, SceneNodeKind::Image { ref asset_ref } if asset_ref == "img_abc")
        );
    }

    #[test]
    fn image_ref_from_fill_paint() {
        let node = FigmaNode {
            fills: vec![FigmaPaint {
                paint_type: "IMAGE".to_string(),
                image_ref: Some("img_from_fill".to_string()),
                ..Default::default()
            }],
            ..make_node(FigmaNodeType::Rectangle)
        };
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(
            matches!(sn.kind, SceneNodeKind::Image { ref asset_ref } if asset_ref == "img_from_fill")
        );
    }

    #[test]
    fn corner_radii_per_corner() {
        let node = FigmaNode {
            rectangle_corner_radii: Some([4.0, 8.0, 12.0, 16.0]),
            corner_radius: 99.0,
            ..make_node(FigmaNodeType::Rectangle)
        };
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        match sn.kind {
            SceneNodeKind::Frame { corner_radius } => {
                assert_eq!(corner_radius, [4.0, 8.0, 12.0, 16.0]);
            }
            _ => panic!("expected Frame"),
        }
    }

    #[test]
    fn corner_radii_uniform_fallback() {
        let node = FigmaNode {
            corner_radius: 10.0,
            ..make_node(FigmaNodeType::Rectangle)
        };
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        match sn.kind {
            SceneNodeKind::Frame { corner_radius } => {
                assert_eq!(corner_radius, [10.0, 10.0, 10.0, 10.0]);
            }
            _ => panic!("expected Frame"),
        }
    }

    #[test]
    fn fill_and_stroke_extraction() {
        let node = FigmaNode {
            fills: vec![FigmaPaint {
                paint_type: "SOLID".to_string(),
                color: Some(FigmaColor {
                    r: 1.0,
                    ..Default::default()
                }),
                ..Default::default()
            }],
            strokes: vec![FigmaPaint {
                paint_type: "SOLID".to_string(),
                color: Some(FigmaColor {
                    b: 1.0,
                    ..Default::default()
                }),
                ..Default::default()
            }],
            stroke_weight: 2.0,
            opacity: 0.8,
            ..make_node(FigmaNodeType::Frame)
        };

        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert_eq!(sn.fill.unwrap().r, 1.0);
        assert_eq!(sn.stroke.unwrap().b, 1.0);
        assert_eq!(sn.stroke_width, 2.0);
        assert_eq!(sn.opacity, 0.8);
    }

    #[test]
    fn page_dimensions_from_children() {
        let children = vec![
            FigmaNode {
                absolute_bounding_box: Some(FigmaRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 200.0,
                }),
                ..make_node(FigmaNodeType::Frame)
            },
            FigmaNode {
                absolute_bounding_box: Some(FigmaRect {
                    x: 500.0,
                    y: 300.0,
                    width: 200.0,
                    height: 100.0,
                }),
                ..make_node(FigmaNodeType::Frame)
            },
        ];
        let (w, h) = compute_page_dimensions(&children);
        assert_eq!(w, 700.0); // 500 + 200
        assert_eq!(h, 400.0); // 300 + 100
    }

    #[test]
    fn page_dimensions_from_nested_descendants() {
        // A frame with a deeply nested child that extends beyond the parent.
        let children = vec![FigmaNode {
            absolute_bounding_box: Some(FigmaRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            }),
            children: vec![FigmaNode {
                absolute_bounding_box: Some(FigmaRect {
                    x: 0.0,
                    y: 0.0,
                    width: 500.0,
                    height: 400.0,
                }),
                ..make_node(FigmaNodeType::Rectangle)
            }],
            ..make_node(FigmaNodeType::Frame)
        }];
        let (w, h) = compute_page_dimensions(&children);
        // The nested child extends to (500, 400), so page should encompass it.
        assert_eq!(w, 500.0);
        assert_eq!(h, 400.0);
    }

    #[test]
    fn page_dimensions_default_when_no_bounds() {
        let (w, h) = compute_page_dimensions(&[]);
        assert_eq!(w, 1920.0);
        assert_eq!(h, 1080.0);
    }

    #[test]
    fn text_color_from_fill() {
        let node = FigmaNode {
            characters: Some("Colored".to_string()),
            fills: vec![FigmaPaint {
                paint_type: "SOLID".to_string(),
                color: Some(FigmaColor {
                    g: 0.5,
                    b: 1.0,
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..make_node(FigmaNodeType::Text)
        };

        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        match &sn.kind {
            SceneNodeKind::Text { text_color, .. } => {
                let tc = text_color.unwrap();
                assert_eq!(tc.g, 0.5);
                assert_eq!(tc.b, 1.0);
            }
            _ => panic!("expected Text kind"),
        }
    }

    #[test]
    fn line_height_ratio_from_px() {
        let node = FigmaNode {
            characters: Some("LH".to_string()),
            style: Some(FigmaTextStyle {
                font_size: Some(20.0),
                line_height_px: Some(30.0),
                ..Default::default()
            }),
            ..make_node(FigmaNodeType::Text)
        };

        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        match &sn.kind {
            SceneNodeKind::Text { line_height, .. } => {
                assert!((*line_height - 1.5).abs() < 1e-6);
            }
            _ => panic!("expected Text kind"),
        }
    }
}
