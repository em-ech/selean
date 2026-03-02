//! Converts Figma API types into Selean engine types.
//!
//! The main entry point is [`file_to_document`], which maps CANVAS children
//! to pages and recursively converts nodes. Absolute bounding boxes from
//! Figma are converted to local coordinates by subtracting the parent's
//! absolute position.

use selean_common::types::{NodeId, PageId};
use selean_engine::persistence::{Document, Page};
use selean_engine::scene::{
    BoundingBox, Color, FontStyle, SceneGraph, SceneNode, SceneNodeKind, TextAlign,
};

use crate::FigmaError;
use crate::api::{FigmaFileResponse, FigmaNode, FigmaTextStyle};
use crate::color::{figma_text_align, first_image_ref, first_solid_color};

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
        .filter(|n| n.node_type == "CANVAS")
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
/// Page dimensions are computed as the bounding union of all child nodes.
/// If no children have bounds, defaults to 1920x1080.
fn canvas_to_page(canvas: &FigmaNode) -> Page {
    let mut scene = SceneGraph::new();

    for child in &canvas.children {
        convert_node_recursive(child, &mut scene, None, (0.0, 0.0));
    }

    let (width, height) = compute_page_dimensions(&canvas.children);

    Page::with_scene(PageId::new(), &canvas.name, width, height, scene)
}

/// Computes page dimensions from the union of all children's bounding boxes.
///
/// Returns (width, height). Falls back to 1920x1080 if no children have bounds.
fn compute_page_dimensions(children: &[FigmaNode]) -> (f32, f32) {
    let mut union = BoundingBox::new(0.0, 0.0, 0.0, 0.0);

    for child in children {
        if let Some(ref bb) = child.absolute_bounding_box {
            let child_bb = BoundingBox::new(bb.x, bb.y, bb.width, bb.height);
            union = union.union(&child_bb);
        }
    }

    if union.is_empty() {
        (1920.0, 1080.0)
    } else {
        (union.right(), union.bottom())
    }
}

/// Recursively converts a Figma node and its children into scene nodes.
///
/// `parent_abs_pos` is the absolute (x, y) of the parent node. For root-level
/// children (direct children of a CANVAS), this is (0, 0). For nested children
/// inside a FRAME/GROUP, this is the parent's absolute bounding box origin.
fn convert_node_recursive(
    node: &FigmaNode,
    scene: &mut SceneGraph,
    parent_id: Option<NodeId>,
    parent_abs_pos: (f32, f32),
) {
    let Some(scene_node) = figma_node_to_scene_node(node, parent_abs_pos) else {
        return;
    };

    let node_id = scene_node.id;

    if let Some(pid) = parent_id {
        scene.add_child(pid, scene_node);
    } else {
        scene.add_root(scene_node);
    }

    // Determine absolute position for children.
    let child_parent_pos = node
        .absolute_bounding_box
        .map_or(parent_abs_pos, |bb| (bb.x, bb.y));

    // Only recurse into container types.
    if is_container(&node.node_type) {
        for child in &node.children {
            convert_node_recursive(child, scene, Some(node_id), child_parent_pos);
        }
    }
}

/// Returns `true` if the node type can contain children in the scene graph.
fn is_container(node_type: &str) -> bool {
    matches!(
        node_type,
        "FRAME" | "GROUP" | "COMPONENT" | "COMPONENT_SET" | "INSTANCE"
    )
}

/// Converts a single Figma node into a Selean `SceneNode`.
///
/// Returns `None` for unsupported or invisible node types (DOCUMENT, CANVAS,
/// SECTION, SLICE, etc.).
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
    // Check for image: node-level imageRef or IMAGE fill paint.
    if let Some(ref img_ref) = find_image_ref(node) {
        return Some(SceneNodeKind::Image {
            asset_ref: img_ref.clone(),
        });
    }

    match node.node_type.as_str() {
        "FRAME" | "RECTANGLE" | "COMPONENT" | "COMPONENT_SET" | "INSTANCE" => {
            Some(SceneNodeKind::Frame {
                corner_radius: resolve_corner_radii(node),
            })
        }
        "TEXT" => {
            let (
                font_size,
                font_family,
                font_weight,
                font_style,
                text_align,
                line_height,
                text_color,
            ) = extract_text_props(node);
            Some(SceneNodeKind::Text {
                content: node.characters.clone().unwrap_or_default(),
                font_size,
                font_family,
                font_weight,
                font_style,
                text_align,
                line_height,
                text_color,
            })
        }
        "VECTOR" | "BOOLEAN_OPERATION" | "LINE" | "REGULAR_POLYGON" | "STAR" | "ELLIPSE" => {
            let path_data = node
                .fill_geometry
                .first()
                .map_or_else(String::new, |p| p.path.clone());
            Some(SceneNodeKind::Vector { path_data })
        }
        "GROUP" => Some(SceneNodeKind::Group),
        _ => {
            tracing::trace!(node_type = %node.node_type, name = %node.name, "skipping unsupported node type");
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
///
/// Returns (`font_size`, `font_family`, `font_weight`, `font_style`, `text_align`, `line_height`, `text_color`).
fn extract_text_props(
    node: &FigmaNode,
) -> (f32, String, u16, FontStyle, TextAlign, f32, Option<Color>) {
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

    (
        font_size,
        font_family,
        font_weight,
        font_style,
        text_align,
        line_height,
        text_color,
    )
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

    fn make_node(node_type: &str) -> FigmaNode {
        FigmaNode {
            id: "1:1".to_string(),
            name: "Test".to_string(),
            node_type: node_type.to_string(),
            visible: true,
            opacity: 1.0,
            absolute_bounding_box: Some(FigmaRect {
                x: 100.0,
                y: 200.0,
                width: 300.0,
                height: 150.0,
            }),
            children: vec![],
            fills: vec![],
            strokes: vec![],
            stroke_weight: 0.0,
            corner_radius: 0.0,
            rectangle_corner_radii: None,
            characters: None,
            style: None,
            fill_geometry: vec![],
            image_ref: None,
        }
    }

    #[test]
    fn frame_node_mapping() {
        let node = make_node("FRAME");
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(matches!(sn.kind, SceneNodeKind::Frame { .. }));
        assert_eq!(sn.bounds.x, 100.0);
        assert_eq!(sn.bounds.y, 200.0);
    }

    #[test]
    fn rectangle_node_mapping() {
        let node = make_node("RECTANGLE");
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(matches!(sn.kind, SceneNodeKind::Frame { .. }));
    }

    #[test]
    fn component_node_mapping() {
        let node = make_node("COMPONENT");
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(matches!(sn.kind, SceneNodeKind::Frame { .. }));
    }

    #[test]
    fn instance_node_mapping() {
        let node = make_node("INSTANCE");
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(matches!(sn.kind, SceneNodeKind::Frame { .. }));
    }

    #[test]
    fn text_node_mapping() {
        let mut node = make_node("TEXT");
        node.characters = Some("Hello".to_string());
        node.style = Some(FigmaTextStyle {
            font_family: Some("Roboto".to_string()),
            font_post_script_name: None,
            font_weight: Some(700),
            font_size: Some(24.0),
            text_align_horizontal: Some("CENTER".to_string()),
            line_height_px: Some(32.0),
        });

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
        let mut node = make_node("TEXT");
        node.characters = None;
        node.style = None;

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
        let mut node = make_node("VECTOR");
        node.fill_geometry = vec![FigmaPath {
            path: "M 0 0 L 100 100".to_string(),
        }];
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
        let node = make_node("ELLIPSE");
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(matches!(sn.kind, SceneNodeKind::Vector { .. }));
    }

    #[test]
    fn group_node_mapping() {
        let node = make_node("GROUP");
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(matches!(sn.kind, SceneNodeKind::Group));
    }

    #[test]
    fn unsupported_type_returns_none() {
        let node = make_node("SLICE");
        assert!(figma_node_to_scene_node(&node, (0.0, 0.0)).is_none());
    }

    #[test]
    fn local_bounds_from_absolute() {
        let node = make_node("FRAME");
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
            node_type: "FRAME".to_string(),
            visible: true,
            opacity: 1.0,
            absolute_bounding_box: Some(FigmaRect {
                x: 100.0,
                y: 200.0,
                width: 400.0,
                height: 300.0,
            }),
            children: vec![FigmaNode {
                id: "1:2".to_string(),
                name: "Child".to_string(),
                node_type: "RECTANGLE".to_string(),
                visible: true,
                opacity: 1.0,
                absolute_bounding_box: Some(FigmaRect {
                    x: 150.0,
                    y: 250.0,
                    width: 100.0,
                    height: 50.0,
                }),
                children: vec![],
                fills: vec![],
                strokes: vec![],
                stroke_weight: 0.0,
                corner_radius: 0.0,
                rectangle_corner_radii: None,
                characters: None,
                style: None,
                fill_geometry: vec![],
                image_ref: None,
            }],
            fills: vec![],
            strokes: vec![],
            stroke_weight: 0.0,
            corner_radius: 0.0,
            rectangle_corner_radii: None,
            characters: None,
            style: None,
            fill_geometry: vec![],
            image_ref: None,
        };

        let mut scene = SceneGraph::new();
        convert_node_recursive(&parent, &mut scene, None, (0.0, 0.0));

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
    fn empty_file_returns_error() {
        let response = FigmaFileResponse {
            name: "Empty".to_string(),
            document: FigmaNode {
                id: "0:0".to_string(),
                name: "Document".to_string(),
                node_type: "DOCUMENT".to_string(),
                visible: true,
                opacity: 1.0,
                absolute_bounding_box: None,
                children: vec![],
                fills: vec![],
                strokes: vec![],
                stroke_weight: 0.0,
                corner_radius: 0.0,
                rectangle_corner_radii: None,
                characters: None,
                style: None,
                fill_geometry: vec![],
                image_ref: None,
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
            text_align_horizontal: None,
            line_height_px: None,
        };
        assert_eq!(detect_italic(&style), FontStyle::Italic);
    }

    #[test]
    fn detect_italic_oblique() {
        let style = FigmaTextStyle {
            font_family: None,
            font_post_script_name: Some("SomeFont-Oblique".to_string()),
            font_weight: None,
            font_size: None,
            text_align_horizontal: None,
            line_height_px: None,
        };
        assert_eq!(detect_italic(&style), FontStyle::Italic);
    }

    #[test]
    fn detect_italic_normal() {
        let style = FigmaTextStyle {
            font_family: None,
            font_post_script_name: Some("Inter-Regular".to_string()),
            font_weight: None,
            font_size: None,
            text_align_horizontal: None,
            line_height_px: None,
        };
        assert_eq!(detect_italic(&style), FontStyle::Normal);
    }

    #[test]
    fn detect_italic_no_postscript() {
        let style = FigmaTextStyle {
            font_family: None,
            font_post_script_name: None,
            font_weight: None,
            font_size: None,
            text_align_horizontal: None,
            line_height_px: None,
        };
        assert_eq!(detect_italic(&style), FontStyle::Normal);
    }

    #[test]
    fn image_ref_from_node_field() {
        let mut node = make_node("FRAME");
        node.image_ref = Some("img_abc".to_string());
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(
            matches!(sn.kind, SceneNodeKind::Image { ref asset_ref } if asset_ref == "img_abc")
        );
    }

    #[test]
    fn image_ref_from_fill_paint() {
        let mut node = make_node("RECTANGLE");
        node.fills = vec![FigmaPaint {
            paint_type: "IMAGE".to_string(),
            color: None,
            opacity: 1.0,
            visible: true,
            image_ref: Some("img_from_fill".to_string()),
        }];
        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        assert!(
            matches!(sn.kind, SceneNodeKind::Image { ref asset_ref } if asset_ref == "img_from_fill")
        );
    }

    #[test]
    fn corner_radii_per_corner() {
        let mut node = make_node("RECTANGLE");
        node.rectangle_corner_radii = Some([4.0, 8.0, 12.0, 16.0]);
        node.corner_radius = 99.0;
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
        let mut node = make_node("RECTANGLE");
        node.corner_radius = 10.0;
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
        let mut node = make_node("FRAME");
        node.fills = vec![FigmaPaint {
            paint_type: "SOLID".to_string(),
            color: Some(FigmaColor {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
            opacity: 1.0,
            visible: true,
            image_ref: None,
        }];
        node.strokes = vec![FigmaPaint {
            paint_type: "SOLID".to_string(),
            color: Some(FigmaColor {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            }),
            opacity: 1.0,
            visible: true,
            image_ref: None,
        }];
        node.stroke_weight = 2.0;
        node.opacity = 0.8;

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
                ..make_node("FRAME")
            },
            FigmaNode {
                absolute_bounding_box: Some(FigmaRect {
                    x: 500.0,
                    y: 300.0,
                    width: 200.0,
                    height: 100.0,
                }),
                ..make_node("FRAME")
            },
        ];
        let (w, h) = compute_page_dimensions(&children);
        assert_eq!(w, 700.0); // 500 + 200
        assert_eq!(h, 400.0); // 300 + 100
    }

    #[test]
    fn page_dimensions_default_when_no_bounds() {
        let (w, h) = compute_page_dimensions(&[]);
        assert_eq!(w, 1920.0);
        assert_eq!(h, 1080.0);
    }

    #[test]
    fn text_color_from_fill() {
        let mut node = make_node("TEXT");
        node.characters = Some("Colored".to_string());
        node.fills = vec![FigmaPaint {
            paint_type: "SOLID".to_string(),
            color: Some(FigmaColor {
                r: 0.0,
                g: 0.5,
                b: 1.0,
                a: 1.0,
            }),
            opacity: 1.0,
            visible: true,
            image_ref: None,
        }];

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
        let mut node = make_node("TEXT");
        node.characters = Some("LH".to_string());
        node.style = Some(FigmaTextStyle {
            font_family: None,
            font_post_script_name: None,
            font_weight: None,
            font_size: Some(20.0),
            text_align_horizontal: None,
            line_height_px: Some(30.0),
        });

        let sn = figma_node_to_scene_node(&node, (0.0, 0.0)).unwrap();
        match &sn.kind {
            SceneNodeKind::Text { line_height, .. } => {
                assert!((*line_height - 1.5).abs() < 1e-6);
            }
            _ => panic!("expected Text kind"),
        }
    }
}
