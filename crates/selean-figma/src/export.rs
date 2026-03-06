//! Figma interchange export for the Selean design platform.
//!
//! Converts a Selean [`Document`] into a JSON interchange format that can be
//! consumed by the Selean Figma plugin to recreate designs in Figma.

use selean_common::types::NodeId;
use selean_engine::persistence::Document;
use selean_engine::scene::{
    Color, Effect, FontStyle, Gradient, SceneGraph, SceneNode, SceneNodeKind, TextAlign,
};
use serde::Serialize;

use crate::FigmaError;

/// Top-level interchange structure representing a full document.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FigmaInterchange {
    /// Document name.
    pub name: String,
    /// Pages in the document.
    pub pages: Vec<FigmaInterchangePage>,
}

/// A single page in the interchange format.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FigmaInterchangePage {
    /// Page display name.
    pub name: String,
    /// Page width in pixels.
    pub width: f32,
    /// Page height in pixels.
    pub height: f32,
    /// Root-level nodes on this page.
    pub children: Vec<FigmaInterchangeNode>,
}

/// A node in the interchange format, matching Figma's plugin API conventions.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FigmaInterchangeNode {
    /// Figma node type: FRAME, TEXT, RECTANGLE, GROUP, VECTOR, IMAGE.
    #[serde(rename = "type")]
    pub node_type: String,
    /// Display name.
    pub name: String,
    /// X position relative to parent.
    pub x: f32,
    /// Y position relative to parent.
    pub y: f32,
    /// Width in pixels.
    pub width: f32,
    /// Height in pixels.
    pub height: f32,
    /// Whether the node is visible.
    pub visible: bool,
    /// Opacity from 0.0 to 1.0.
    pub opacity: f32,
    /// Corner radii `[tl, tr, br, bl]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<[f32; 4]>,
    /// Fill paints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fills: Option<Vec<FigmaInterchangePaint>>,
    /// Stroke paints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strokes: Option<Vec<FigmaInterchangePaint>>,
    /// Stroke weight in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke_weight: Option<f32>,
    /// Visual effects (shadows, blurs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects: Option<Vec<FigmaInterchangeEffect>>,
    /// Text content (TEXT nodes only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub characters: Option<String>,
    /// Font size in pixels (TEXT nodes only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    /// Font family name (TEXT nodes only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    /// Font weight 100-900 (TEXT nodes only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
    /// Font style: "normal" or "italic" (TEXT nodes only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_style: Option<String>,
    /// Horizontal text alignment (TEXT nodes only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align_horizontal: Option<String>,
    /// Line height in pixels (TEXT nodes only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_height_px: Option<f32>,
    /// Text color (TEXT nodes only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<FigmaInterchangeColor>,
    /// SVG path data (VECTOR nodes only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_data: Option<String>,
    /// Asset reference (IMAGE nodes only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_ref: Option<String>,
    /// Child nodes (FRAME, GROUP only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<FigmaInterchangeNode>>,
}

/// An RGBA color in the interchange format.
#[derive(Debug, Serialize)]
pub struct FigmaInterchangeColor {
    /// Red component (0.0 to 1.0).
    pub r: f32,
    /// Green component (0.0 to 1.0).
    pub g: f32,
    /// Blue component (0.0 to 1.0).
    pub b: f32,
    /// Alpha component (0.0 to 1.0).
    pub a: f32,
}

/// A fill or stroke paint in the interchange format.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FigmaInterchangePaint {
    /// Paint type: `SOLID`, `GRADIENT_LINEAR`, `GRADIENT_RADIAL`.
    #[serde(rename = "type")]
    pub paint_type: String,
    /// Solid color (SOLID paints only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<FigmaInterchangeColor>,
    /// Gradient handle positions (gradient paints only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient_handle_positions: Option<Vec<FigmaInterchangeVector>>,
    /// Gradient color stops (gradient paints only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient_stops: Option<Vec<FigmaInterchangeGradientStop>>,
}

/// A 2D vector in the interchange format.
#[derive(Debug, Serialize)]
pub struct FigmaInterchangeVector {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

/// A gradient color stop in the interchange format.
#[derive(Debug, Serialize)]
pub struct FigmaInterchangeGradientStop {
    /// Position along the gradient axis (0.0 to 1.0).
    pub position: f32,
    /// Color at this stop.
    pub color: FigmaInterchangeColor,
}

/// A visual effect in the interchange format.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FigmaInterchangeEffect {
    /// Effect type: `DROP_SHADOW`, `LAYER_BLUR`.
    #[serde(rename = "type")]
    pub effect_type: String,
    /// Whether this effect is visible.
    pub visible: bool,
    /// Blur radius in pixels.
    pub radius: f32,
    /// Shadow color (`DROP_SHADOW` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<FigmaInterchangeColor>,
    /// Shadow offset (`DROP_SHADOW` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<FigmaInterchangeVector>,
}

/// Exports a Selean `Document` to Figma interchange JSON.
///
/// Walks each page and converts all scene graph nodes into the interchange
/// format that a Figma plugin can consume via `figma.createFrame()`, etc.
///
/// # Errors
///
/// Returns `FigmaError::Parse` if JSON serialization fails.
pub fn export_figma_interchange(doc: &Document) -> Result<String, FigmaError> {
    let pages: Vec<FigmaInterchangePage> = doc
        .pages()
        .iter()
        .map(|page| {
            let children = convert_root_nodes(&page.scene);
            FigmaInterchangePage {
                name: page.name.clone(),
                width: page.width,
                height: page.height,
                children,
            }
        })
        .collect();

    let interchange = FigmaInterchange {
        name: "Selean Export".to_string(),
        pages,
    };

    serde_json::to_string_pretty(&interchange).map_err(FigmaError::from)
}

/// Converts all root nodes in a scene graph to interchange nodes.
fn convert_root_nodes(scene: &SceneGraph) -> Vec<FigmaInterchangeNode> {
    scene
        .roots()
        .iter()
        .filter_map(|id| convert_node(scene, *id))
        .collect()
}

/// Converts a single scene node and its descendants to an interchange node.
fn convert_node(scene: &SceneGraph, id: NodeId) -> Option<FigmaInterchangeNode> {
    let node = scene.get(id)?;
    let node_type = map_kind_to_figma_type(&node.kind, scene.children(id));
    let fills = build_fills(node);
    let strokes = build_strokes(node);
    let effects = build_effects(&node.effects);
    let children = build_children(scene, node, &node_type);

    Some(FigmaInterchangeNode {
        node_type,
        name: node.name.clone(),
        x: node.bounds.x,
        y: node.bounds.y,
        width: node.bounds.width,
        height: node.bounds.height,
        visible: node.visible,
        opacity: node.opacity,
        corner_radius: extract_corner_radius(&node.kind),
        fills,
        strokes,
        stroke_weight: if node.stroke_width > 0.0 {
            Some(node.stroke_width)
        } else {
            None
        },
        effects,
        characters: extract_text_content(&node.kind),
        font_size: extract_font_size(&node.kind),
        font_family: extract_font_family(&node.kind),
        font_weight: extract_font_weight(&node.kind),
        font_style: extract_font_style(&node.kind),
        text_align_horizontal: extract_text_align(&node.kind),
        line_height_px: extract_line_height_px(&node.kind),
        text_color: extract_text_color(&node.kind),
        path_data: extract_path_data(&node.kind),
        asset_ref: extract_asset_ref(&node.kind),
        children,
    })
}

/// Maps a `SceneNodeKind` to a Figma node type string.
///
/// Frames with no children are exported as RECTANGLE for cleaner plugin mapping.
fn map_kind_to_figma_type(kind: &SceneNodeKind, children: Option<&[NodeId]>) -> String {
    match kind {
        SceneNodeKind::Frame { .. } => {
            let has_children = children.is_some_and(|c| !c.is_empty());
            if has_children { "FRAME" } else { "RECTANGLE" }.to_string()
        }
        SceneNodeKind::Text { .. } => "TEXT".to_string(),
        SceneNodeKind::Group => "GROUP".to_string(),
        SceneNodeKind::Vector { .. } => "VECTOR".to_string(),
        SceneNodeKind::Image { .. } => "IMAGE".to_string(),
    }
}

/// Builds fill paints from a node's solid fill and gradient fill.
fn build_fills(node: &SceneNode) -> Option<Vec<FigmaInterchangePaint>> {
    let mut fills = Vec::new();

    if let Some(ref gradient) = node.fill_gradient {
        fills.push(gradient_to_paint(gradient));
    } else if let Some(color) = node.fill {
        fills.push(solid_paint(color));
    }

    if fills.is_empty() { None } else { Some(fills) }
}

/// Builds stroke paints from a node's stroke color.
fn build_strokes(node: &SceneNode) -> Option<Vec<FigmaInterchangePaint>> {
    node.stroke.map(|color| vec![solid_paint(color)])
}

/// Builds interchange effects from engine effects.
fn build_effects(effects: &[Effect]) -> Option<Vec<FigmaInterchangeEffect>> {
    if effects.is_empty() {
        return None;
    }
    let converted: Vec<FigmaInterchangeEffect> = effects.iter().map(convert_effect).collect();
    Some(converted)
}

/// Recursively converts child nodes for container types (FRAME, GROUP).
fn build_children(
    scene: &SceneGraph,
    node: &SceneNode,
    figma_type: &str,
) -> Option<Vec<FigmaInterchangeNode>> {
    if figma_type != "FRAME" && figma_type != "GROUP" {
        return None;
    }
    let child_ids = scene.children(node.id)?;
    if child_ids.is_empty() {
        return None;
    }
    let children: Vec<FigmaInterchangeNode> = child_ids
        .iter()
        .filter_map(|cid| convert_node(scene, *cid))
        .collect();
    if children.is_empty() {
        None
    } else {
        Some(children)
    }
}

/// Converts an engine `Color` to an interchange color.
fn color_to_interchange(c: Color) -> FigmaInterchangeColor {
    FigmaInterchangeColor {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// Creates a SOLID paint from a color.
fn solid_paint(color: Color) -> FigmaInterchangePaint {
    FigmaInterchangePaint {
        paint_type: "SOLID".to_string(),
        color: Some(color_to_interchange(color)),
        gradient_handle_positions: None,
        gradient_stops: None,
    }
}

/// Converts an engine `Gradient` to an interchange paint.
fn gradient_to_paint(gradient: &Gradient) -> FigmaInterchangePaint {
    match gradient {
        Gradient::Linear { start, end, stops } => FigmaInterchangePaint {
            paint_type: "GRADIENT_LINEAR".to_string(),
            color: None,
            gradient_handle_positions: Some(vec![
                FigmaInterchangeVector {
                    x: start[0],
                    y: start[1],
                },
                FigmaInterchangeVector {
                    x: end[0],
                    y: end[1],
                },
            ]),
            gradient_stops: Some(convert_gradient_stops(stops)),
        },
        Gradient::Radial {
            center,
            radius,
            stops,
        } => FigmaInterchangePaint {
            paint_type: "GRADIENT_RADIAL".to_string(),
            color: None,
            gradient_handle_positions: Some(vec![
                FigmaInterchangeVector {
                    x: center[0],
                    y: center[1],
                },
                FigmaInterchangeVector {
                    x: center[0] + radius,
                    y: center[1],
                },
            ]),
            gradient_stops: Some(convert_gradient_stops(stops)),
        },
    }
}

/// Converts engine gradient stops to interchange gradient stops.
fn convert_gradient_stops(
    stops: &[selean_engine::scene::GradientStop],
) -> Vec<FigmaInterchangeGradientStop> {
    stops
        .iter()
        .map(|s| FigmaInterchangeGradientStop {
            position: s.position,
            color: color_to_interchange(s.color),
        })
        .collect()
}

/// Converts a single engine `Effect` to an interchange effect.
fn convert_effect(effect: &Effect) -> FigmaInterchangeEffect {
    match effect {
        Effect::DropShadow {
            color,
            offset_x,
            offset_y,
            blur_radius,
        } => FigmaInterchangeEffect {
            effect_type: "DROP_SHADOW".to_string(),
            visible: true,
            radius: *blur_radius,
            color: Some(color_to_interchange(*color)),
            offset: Some(FigmaInterchangeVector {
                x: *offset_x,
                y: *offset_y,
            }),
        },
        Effect::Blur { radius } => FigmaInterchangeEffect {
            effect_type: "LAYER_BLUR".to_string(),
            visible: true,
            radius: *radius,
            color: None,
            offset: None,
        },
    }
}

/// Extracts corner radius from a Frame kind, if non-zero.
fn extract_corner_radius(kind: &SceneNodeKind) -> Option<[f32; 4]> {
    if let SceneNodeKind::Frame { corner_radius } = kind {
        let all_zero = corner_radius.iter().all(|r| *r == 0.0);
        if all_zero { None } else { Some(*corner_radius) }
    } else {
        None
    }
}

/// Extracts text content from a Text kind.
fn extract_text_content(kind: &SceneNodeKind) -> Option<String> {
    if let SceneNodeKind::Text { content, .. } = kind {
        Some(content.clone())
    } else {
        None
    }
}

/// Extracts font size from a Text kind.
fn extract_font_size(kind: &SceneNodeKind) -> Option<f32> {
    if let SceneNodeKind::Text { font_size, .. } = kind {
        Some(*font_size)
    } else {
        None
    }
}

/// Extracts font family from a Text kind.
fn extract_font_family(kind: &SceneNodeKind) -> Option<String> {
    if let SceneNodeKind::Text { font_family, .. } = kind {
        Some(font_family.clone())
    } else {
        None
    }
}

/// Extracts font weight from a Text kind.
fn extract_font_weight(kind: &SceneNodeKind) -> Option<u16> {
    if let SceneNodeKind::Text { font_weight, .. } = kind {
        Some(*font_weight)
    } else {
        None
    }
}

/// Extracts font style as a Figma-compatible string from a Text kind.
fn extract_font_style(kind: &SceneNodeKind) -> Option<String> {
    if let SceneNodeKind::Text { font_style, .. } = kind {
        Some(
            match font_style {
                FontStyle::Normal => "normal",
                FontStyle::Italic => "italic",
            }
            .to_string(),
        )
    } else {
        None
    }
}

/// Extracts text alignment as a Figma-compatible string from a Text kind.
fn extract_text_align(kind: &SceneNodeKind) -> Option<String> {
    if let SceneNodeKind::Text { text_align, .. } = kind {
        Some(
            match text_align {
                TextAlign::Left => "LEFT",
                TextAlign::Center => "CENTER",
                TextAlign::Right => "RIGHT",
                TextAlign::Justify => "JUSTIFIED",
            }
            .to_string(),
        )
    } else {
        None
    }
}

/// Extracts line height in pixels from a Text kind.
///
/// Converts the engine's line height ratio to absolute pixels using font size.
fn extract_line_height_px(kind: &SceneNodeKind) -> Option<f32> {
    if let SceneNodeKind::Text {
        line_height,
        font_size,
        ..
    } = kind
    {
        Some(line_height * font_size)
    } else {
        None
    }
}

/// Extracts text color from a Text kind.
fn extract_text_color(kind: &SceneNodeKind) -> Option<FigmaInterchangeColor> {
    if let SceneNodeKind::Text { text_color, .. } = kind {
        text_color.map(color_to_interchange)
    } else {
        None
    }
}

/// Extracts SVG path data from a Vector kind.
fn extract_path_data(kind: &SceneNodeKind) -> Option<String> {
    if let SceneNodeKind::Vector { path_data } = kind {
        if path_data.is_empty() {
            None
        } else {
            Some(path_data.clone())
        }
    } else {
        None
    }
}

/// Extracts asset reference from an Image kind.
fn extract_asset_ref(kind: &SceneNodeKind) -> Option<String> {
    if let SceneNodeKind::Image { asset_ref } = kind {
        if asset_ref.is_empty() {
            None
        } else {
            Some(asset_ref.clone())
        }
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp, clippy::expect_used)]
mod tests {
    use super::*;
    use selean_common::types::{NodeId, PageId};
    use selean_engine::persistence::{Document, Page};
    use selean_engine::scene::{BoundingBox, GradientStop, SceneGraph, SceneNode};

    fn make_scene_node(name: &str, kind: SceneNodeKind, bounds: BoundingBox) -> SceneNode {
        SceneNode::new(NodeId::new(), name.to_string(), kind, bounds)
    }

    fn frame_kind(radii: [f32; 4]) -> SceneNodeKind {
        SceneNodeKind::Frame {
            corner_radius: radii,
        }
    }

    fn text_kind(content: &str) -> SceneNodeKind {
        SceneNodeKind::Text {
            content: content.to_string(),
            font_size: 24.0,
            font_family: "Roboto".to_string(),
            font_weight: 700,
            font_style: FontStyle::Italic,
            text_align: TextAlign::Center,
            line_height: 1.5,
            text_color: Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        }
    }

    fn parse_exported(doc: &Document) -> serde_json::Value {
        let json = export_figma_interchange(doc).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    fn single_page_doc(scene: SceneGraph) -> Document {
        let page = Page::with_scene(PageId::new(), "Test Page", 800.0, 600.0, scene);
        Document::from_pages(vec![page])
    }

    #[test]
    fn export_empty_document() {
        let doc = Document::new();
        let val = parse_exported(&doc);
        assert_eq!(val["name"], "Selean Export");
        assert_eq!(val["pages"].as_array().unwrap().len(), 1);
        let page = &val["pages"][0];
        assert_eq!(page["children"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn export_frame_node() {
        let mut scene = SceneGraph::new();
        let node = make_scene_node(
            "My Frame",
            frame_kind([0.0; 4]),
            BoundingBox::new(10.0, 20.0, 300.0, 200.0),
        );
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let child = &val["pages"][0]["children"][0];

        // Frame with no children becomes RECTANGLE.
        assert_eq!(child["type"], "RECTANGLE");
        assert_eq!(child["name"], "My Frame");
        assert_eq!(child["x"], 10.0);
        assert_eq!(child["y"], 20.0);
        assert_eq!(child["width"], 300.0);
        assert_eq!(child["height"], 200.0);
        assert_eq!(child["visible"], true);
        assert_eq!(child["opacity"], 1.0);
    }

    #[test]
    fn export_text_node() {
        let mut scene = SceneGraph::new();
        let node = make_scene_node(
            "Title",
            text_kind("Hello World"),
            BoundingBox::new(0.0, 0.0, 200.0, 40.0),
        );
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let child = &val["pages"][0]["children"][0];

        assert_eq!(child["type"], "TEXT");
        assert_eq!(child["characters"], "Hello World");
        assert_eq!(child["fontSize"], 24.0);
        assert_eq!(child["fontFamily"], "Roboto");
        assert_eq!(child["fontWeight"], 700);
        assert_eq!(child["fontStyle"], "italic");
        assert_eq!(child["textAlignHorizontal"], "CENTER");
        // lineHeightPx = 1.5 * 24.0 = 36.0
        assert_eq!(child["lineHeightPx"], 36.0);
        assert_eq!(child["textColor"]["r"], 1.0);
        assert_eq!(child["textColor"]["a"], 1.0);
    }

    #[test]
    fn export_vector_node() {
        let mut scene = SceneGraph::new();
        let node = make_scene_node(
            "Path",
            SceneNodeKind::Vector {
                path_data: "M 0 0 L 100 100".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let child = &val["pages"][0]["children"][0];

        assert_eq!(child["type"], "VECTOR");
        assert_eq!(child["pathData"], "M 0 0 L 100 100");
    }

    #[test]
    fn export_image_node() {
        let mut scene = SceneGraph::new();
        let node = make_scene_node(
            "Photo",
            SceneNodeKind::Image {
                asset_ref: "img_abc123".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 400.0, 300.0),
        );
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let child = &val["pages"][0]["children"][0];

        assert_eq!(child["type"], "IMAGE");
        assert_eq!(child["assetRef"], "img_abc123");
    }

    #[test]
    fn export_group_with_children() {
        let mut scene = SceneGraph::new();
        let group = make_scene_node(
            "Group 1",
            SceneNodeKind::Group,
            BoundingBox::new(0.0, 0.0, 500.0, 500.0),
        );
        let group_id = group.id;
        scene.add_root(group);

        let child = make_scene_node(
            "Child Rect",
            frame_kind([0.0; 4]),
            BoundingBox::new(10.0, 10.0, 100.0, 100.0),
        );
        scene.add_child(group_id, child);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let group_node = &val["pages"][0]["children"][0];

        assert_eq!(group_node["type"], "GROUP");
        assert_eq!(group_node["name"], "Group 1");
        let group_children = group_node["children"].as_array().unwrap();
        assert_eq!(group_children.len(), 1);
        assert_eq!(group_children[0]["name"], "Child Rect");
        assert_eq!(group_children[0]["type"], "RECTANGLE");
    }

    #[test]
    fn export_fills_and_strokes() {
        let mut scene = SceneGraph::new();
        let mut node = make_scene_node(
            "Styled",
            frame_kind([0.0; 4]),
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        node.fill = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        node.stroke = Some(Color::new(0.0, 0.0, 1.0, 0.8));
        node.stroke_width = 2.0;
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let child = &val["pages"][0]["children"][0];

        let fills = child["fills"].as_array().unwrap();
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0]["type"], "SOLID");
        assert_eq!(fills[0]["color"]["r"], 1.0);

        let strokes = child["strokes"].as_array().unwrap();
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0]["type"], "SOLID");
        assert_eq!(strokes[0]["color"]["b"], 1.0);
        // Alpha includes the 0.8 from the color.
        let stroke_alpha = strokes[0]["color"]["a"].as_f64().unwrap();
        assert!((stroke_alpha - 0.8).abs() < 1e-6);

        assert_eq!(child["strokeWeight"], 2.0);
    }

    #[test]
    fn export_gradient_fill() {
        let mut scene = SceneGraph::new();
        let mut node = make_scene_node(
            "Gradient",
            frame_kind([0.0; 4]),
            BoundingBox::new(0.0, 0.0, 200.0, 200.0),
        );
        node.fill_gradient = Some(Gradient::Linear {
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
        });
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let child = &val["pages"][0]["children"][0];

        let fills = child["fills"].as_array().unwrap();
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0]["type"], "GRADIENT_LINEAR");
        let positions = fills[0]["gradientHandlePositions"].as_array().unwrap();
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0]["x"], 0.0);
        assert_eq!(positions[1]["x"], 1.0);
        let stops = fills[0]["gradientStops"].as_array().unwrap();
        assert_eq!(stops.len(), 2);
        assert_eq!(stops[0]["position"], 0.0);
        assert_eq!(stops[0]["color"]["r"], 1.0);
        assert_eq!(stops[1]["position"], 1.0);
        assert_eq!(stops[1]["color"]["b"], 1.0);
    }

    #[test]
    fn export_radial_gradient() {
        let mut scene = SceneGraph::new();
        let mut node = make_scene_node(
            "Radial",
            frame_kind([0.0; 4]),
            BoundingBox::new(0.0, 0.0, 200.0, 200.0),
        );
        node.fill_gradient = Some(Gradient::Radial {
            center: [0.5, 0.5],
            radius: 0.5,
            stops: vec![GradientStop {
                position: 0.0,
                color: Color::WHITE,
            }],
        });
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let fills = val["pages"][0]["children"][0]["fills"].as_array().unwrap();
        assert_eq!(fills[0]["type"], "GRADIENT_RADIAL");
        let positions = fills[0]["gradientHandlePositions"].as_array().unwrap();
        assert_eq!(positions[0]["x"], 0.5);
        assert_eq!(positions[0]["y"], 0.5);
        // Second handle = center + radius along x-axis.
        assert_eq!(positions[1]["x"], 1.0);
        assert_eq!(positions[1]["y"], 0.5);
    }

    #[test]
    fn export_effects() {
        let mut scene = SceneGraph::new();
        let mut node = make_scene_node(
            "FX",
            frame_kind([0.0; 4]),
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        node.effects = vec![
            Effect::DropShadow {
                color: Color::new(0.0, 0.0, 0.0, 0.5),
                offset_x: 4.0,
                offset_y: 6.0,
                blur_radius: 8.0,
            },
            Effect::Blur { radius: 10.0 },
        ];
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let effects = val["pages"][0]["children"][0]["effects"]
            .as_array()
            .unwrap();
        assert_eq!(effects.len(), 2);

        assert_eq!(effects[0]["type"], "DROP_SHADOW");
        assert_eq!(effects[0]["radius"], 8.0);
        assert_eq!(effects[0]["color"]["a"], 0.5);
        assert_eq!(effects[0]["offset"]["x"], 4.0);
        assert_eq!(effects[0]["offset"]["y"], 6.0);

        assert_eq!(effects[1]["type"], "LAYER_BLUR");
        assert_eq!(effects[1]["radius"], 10.0);
        assert!(effects[1]["color"].is_null());
        assert!(effects[1]["offset"].is_null());
    }

    #[test]
    fn export_invisible_node() {
        let mut scene = SceneGraph::new();
        let mut node = make_scene_node(
            "Hidden",
            frame_kind([0.0; 4]),
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        node.visible = false;
        node.opacity = 0.3;
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let child = &val["pages"][0]["children"][0];
        assert_eq!(child["visible"], false);
        assert_eq!(child["opacity"], 0.3);
    }

    #[test]
    fn export_corner_radii() {
        let mut scene = SceneGraph::new();
        let node = make_scene_node(
            "Rounded",
            frame_kind([4.0, 8.0, 12.0, 16.0]),
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let radii = val["pages"][0]["children"][0]["cornerRadius"]
            .as_array()
            .unwrap();
        assert_eq!(radii.len(), 4);
        assert_eq!(radii[0], 4.0);
        assert_eq!(radii[1], 8.0);
        assert_eq!(radii[2], 12.0);
        assert_eq!(radii[3], 16.0);
    }

    #[test]
    fn export_zero_corner_radii_omitted() {
        let mut scene = SceneGraph::new();
        let node = make_scene_node(
            "Sharp",
            frame_kind([0.0; 4]),
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        assert!(val["pages"][0]["children"][0]["cornerRadius"].is_null());
    }

    #[test]
    fn export_frame_with_children_uses_frame_type() {
        let mut scene = SceneGraph::new();
        let parent = make_scene_node(
            "Container",
            frame_kind([0.0; 4]),
            BoundingBox::new(0.0, 0.0, 500.0, 500.0),
        );
        let parent_id = parent.id;
        scene.add_root(parent);

        let child = make_scene_node(
            "Inner",
            frame_kind([0.0; 4]),
            BoundingBox::new(10.0, 10.0, 50.0, 50.0),
        );
        scene.add_child(parent_id, child);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let container = &val["pages"][0]["children"][0];

        // Frame with children should be FRAME, not RECTANGLE.
        assert_eq!(container["type"], "FRAME");
        let children = container["children"].as_array().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["type"], "RECTANGLE");
    }

    #[test]
    fn export_multi_page_document() {
        let pages = vec![
            Page::with_scene(PageId::new(), "Page A", 800.0, 600.0, SceneGraph::new()),
            Page::with_scene(PageId::new(), "Page B", 1920.0, 1080.0, SceneGraph::new()),
        ];
        let doc = Document::from_pages(pages);
        let val = parse_exported(&doc);

        let exported_pages = val["pages"].as_array().unwrap();
        assert_eq!(exported_pages.len(), 2);
        assert_eq!(exported_pages[0]["name"], "Page A");
        assert_eq!(exported_pages[0]["width"], 800.0);
        assert_eq!(exported_pages[0]["height"], 600.0);
        assert_eq!(exported_pages[1]["name"], "Page B");
        assert_eq!(exported_pages[1]["width"], 1920.0);
    }

    #[test]
    fn export_gradient_takes_priority_over_solid() {
        let mut scene = SceneGraph::new();
        let mut node = make_scene_node(
            "Both",
            frame_kind([0.0; 4]),
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        // Set both solid and gradient. Gradient should win.
        node.fill = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        node.fill_gradient = Some(Gradient::Linear {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
            stops: vec![GradientStop {
                position: 0.0,
                color: Color::new(0.0, 1.0, 0.0, 1.0),
            }],
        });
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let fills = val["pages"][0]["children"][0]["fills"].as_array().unwrap();
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0]["type"], "GRADIENT_LINEAR");
    }

    #[test]
    fn export_no_stroke_omits_fields() {
        let mut scene = SceneGraph::new();
        let node = make_scene_node(
            "NoStroke",
            frame_kind([0.0; 4]),
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let child = &val["pages"][0]["children"][0];
        assert!(child["strokes"].is_null());
        assert!(child["strokeWeight"].is_null());
    }

    #[test]
    fn export_empty_path_data_omitted() {
        let mut scene = SceneGraph::new();
        let node = make_scene_node(
            "EmptyVec",
            SceneNodeKind::Vector {
                path_data: String::new(),
            },
            BoundingBox::new(0.0, 0.0, 50.0, 50.0),
        );
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        assert!(val["pages"][0]["children"][0]["pathData"].is_null());
    }

    #[test]
    fn export_empty_asset_ref_omitted() {
        let mut scene = SceneGraph::new();
        let node = make_scene_node(
            "EmptyImg",
            SceneNodeKind::Image {
                asset_ref: String::new(),
            },
            BoundingBox::new(0.0, 0.0, 50.0, 50.0),
        );
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        assert!(val["pages"][0]["children"][0]["assetRef"].is_null());
    }

    #[test]
    fn export_text_default_properties() {
        let mut scene = SceneGraph::new();
        let node = make_scene_node(
            "Default Text",
            SceneNodeKind::Text {
                content: "Hi".to_string(),
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
        scene.add_root(node);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);
        let child = &val["pages"][0]["children"][0];

        assert_eq!(child["fontStyle"], "normal");
        assert_eq!(child["textAlignHorizontal"], "LEFT");
        // lineHeightPx = 1.2 * 16 = 19.2
        let lh = child["lineHeightPx"].as_f64().unwrap();
        assert!((lh - 19.2).abs() < 0.01);
        assert!(child["textColor"].is_null());
    }

    #[test]
    fn export_roundtrip_fidelity() {
        // Build a document with varied nodes, export, and verify key properties.
        let mut scene = SceneGraph::new();

        let mut frame = make_scene_node(
            "Root Frame",
            frame_kind([10.0, 20.0, 30.0, 40.0]),
            BoundingBox::new(0.0, 0.0, 800.0, 600.0),
        );
        frame.fill = Some(Color::new(0.9, 0.9, 0.9, 1.0));
        frame.opacity = 0.95;
        let frame_id = frame.id;
        scene.add_root(frame);

        let text = make_scene_node(
            "Heading",
            text_kind("Welcome"),
            BoundingBox::new(50.0, 30.0, 200.0, 40.0),
        );
        scene.add_child(frame_id, text);

        let doc = single_page_doc(scene);
        let val = parse_exported(&doc);

        let root = &val["pages"][0]["children"][0];
        assert_eq!(root["type"], "FRAME");
        assert_eq!(root["name"], "Root Frame");
        assert_eq!(root["opacity"], 0.95);
        assert_eq!(root["cornerRadius"][0], 10.0);
        assert_eq!(root["cornerRadius"][3], 40.0);

        let heading = &root["children"].as_array().unwrap()[0];
        assert_eq!(heading["type"], "TEXT");
        assert_eq!(heading["characters"], "Welcome");
        assert_eq!(heading["fontFamily"], "Roboto");
        assert_eq!(heading["fontWeight"], 700);
    }
}
