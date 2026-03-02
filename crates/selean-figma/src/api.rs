//! Serde structs for the Figma REST API response.
//!
//! Models the JSON returned by `GET /v1/files/:key?geometry=paths`.
//! Uses liberal `#[serde(default)]` for resilience against missing optional
//! fields and `#[serde(deny_unknown_fields)]` is intentionally omitted to
//! tolerate new fields Figma may add.

use serde::Deserialize;

/// Top-level response from `GET /v1/files/:key`.
#[derive(Debug, Deserialize)]
pub struct FigmaFileResponse {
    /// File name as displayed in Figma.
    #[serde(default)]
    pub name: String,
    /// Root document node containing canvases (pages).
    pub document: FigmaNode,
}

/// A node in the Figma document tree.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FigmaNode {
    /// Node ID (e.g. "0:1").
    #[serde(default)]
    pub id: String,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// Node type (DOCUMENT, CANVAS, FRAME, TEXT, etc.).
    #[serde(rename = "type", default)]
    pub node_type: String,
    /// Whether the node is visible. Defaults to `true`.
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Node opacity (0.0 to 1.0).
    #[serde(default = "default_one")]
    pub opacity: f32,
    /// Absolute bounding box in global pixel coordinates.
    #[serde(default)]
    pub absolute_bounding_box: Option<FigmaRect>,
    /// Child nodes.
    #[serde(default)]
    pub children: Vec<FigmaNode>,
    /// Fill paints.
    #[serde(default)]
    pub fills: Vec<FigmaPaint>,
    /// Stroke paints.
    #[serde(default)]
    pub strokes: Vec<FigmaPaint>,
    /// Stroke weight in pixels.
    #[serde(default)]
    pub stroke_weight: f32,
    /// Uniform corner radius (overridden by `rectangle_corner_radii`).
    #[serde(default)]
    pub corner_radius: f32,
    /// Per-corner radii `[top-left, top-right, bottom-right, bottom-left]`.
    #[serde(default)]
    pub rectangle_corner_radii: Option<[f32; 4]>,
    /// Text content (TEXT nodes only).
    #[serde(default)]
    pub characters: Option<String>,
    /// Text style properties (TEXT nodes only).
    #[serde(default)]
    pub style: Option<FigmaTextStyle>,
    /// Fill geometry paths (VECTOR-like nodes). Requires `?geometry=paths`.
    #[serde(default)]
    pub fill_geometry: Vec<FigmaPath>,
    /// Image reference hash (nodes with IMAGE fill type).
    #[serde(default)]
    pub image_ref: Option<String>,
}

/// Axis-aligned bounding rectangle from the Figma API.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct FigmaRect {
    /// Left edge x coordinate.
    #[serde(default)]
    pub x: f32,
    /// Top edge y coordinate.
    #[serde(default)]
    pub y: f32,
    /// Width in pixels.
    #[serde(default)]
    pub width: f32,
    /// Height in pixels.
    #[serde(default)]
    pub height: f32,
}

/// A fill or stroke paint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FigmaPaint {
    /// Paint type: `SOLID`, `GRADIENT_LINEAR`, `IMAGE`, etc.
    #[serde(rename = "type", default)]
    pub paint_type: String,
    /// Color (SOLID paints).
    #[serde(default)]
    pub color: Option<FigmaColor>,
    /// Opacity of this paint layer.
    #[serde(default = "default_one")]
    pub opacity: f32,
    /// Whether this paint is visible.
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Image reference hash (IMAGE paints).
    #[serde(default)]
    pub image_ref: Option<String>,
}

/// RGBA color with components in [0.0, 1.0].
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct FigmaColor {
    /// Red component.
    #[serde(default)]
    pub r: f32,
    /// Green component.
    #[serde(default)]
    pub g: f32,
    /// Blue component.
    #[serde(default)]
    pub b: f32,
    /// Alpha component.
    #[serde(default = "default_one")]
    pub a: f32,
}

/// Text style properties from the Figma API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FigmaTextStyle {
    /// Font family name.
    #[serde(default)]
    pub font_family: Option<String>,
    /// Font PostScript name (used for italic detection).
    #[serde(default)]
    pub font_post_script_name: Option<String>,
    /// Font weight (100-900).
    #[serde(default)]
    pub font_weight: Option<u16>,
    /// Font size in pixels.
    #[serde(default)]
    pub font_size: Option<f32>,
    /// Text horizontal alignment: "LEFT", "CENTER", "RIGHT", "JUSTIFIED".
    #[serde(default)]
    pub text_align_horizontal: Option<String>,
    /// Line height in pixels (absolute value).
    #[serde(default)]
    pub line_height_px: Option<f32>,
}

/// A geometry path from `fillGeometry`.
#[derive(Debug, Clone, Deserialize)]
pub struct FigmaPath {
    /// SVG path data string.
    #[serde(default)]
    pub path: String,
}

fn default_true() -> bool {
    true
}

fn default_one() -> f32 {
    1.0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_minimal_file_response() {
        let json = r#"{"name":"Test","document":{"id":"0:0","name":"Document","type":"DOCUMENT"}}"#;
        let resp: FigmaFileResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.name, "Test");
        assert_eq!(resp.document.node_type, "DOCUMENT");
    }

    #[test]
    fn deserialize_node_defaults() {
        let json = r#"{"type":"FRAME"}"#;
        let node: FigmaNode = serde_json::from_str(json).unwrap();
        assert!(node.visible);
        assert_eq!(node.opacity, 1.0);
        assert!(node.children.is_empty());
        assert!(node.fills.is_empty());
        assert!(node.absolute_bounding_box.is_none());
        assert!(node.characters.is_none());
        assert!(node.style.is_none());
    }

    #[test]
    fn deserialize_node_with_bounds() {
        let json = r#"{
            "type": "RECTANGLE",
            "absoluteBoundingBox": {"x": 10.0, "y": 20.0, "width": 100.0, "height": 50.0}
        }"#;
        let node: FigmaNode = serde_json::from_str(json).unwrap();
        let bb = node.absolute_bounding_box.unwrap();
        assert_eq!(bb.x, 10.0);
        assert_eq!(bb.y, 20.0);
        assert_eq!(bb.width, 100.0);
        assert_eq!(bb.height, 50.0);
    }

    #[test]
    fn deserialize_solid_fill() {
        let json = r#"{
            "type": "SOLID",
            "color": {"r": 1.0, "g": 0.0, "b": 0.5, "a": 0.8},
            "opacity": 0.9,
            "visible": true
        }"#;
        let paint: FigmaPaint = serde_json::from_str(json).unwrap();
        assert_eq!(paint.paint_type, "SOLID");
        let c = paint.color.unwrap();
        assert_eq!(c.r, 1.0);
        assert_eq!(c.g, 0.0);
        assert_eq!(c.b, 0.5);
        assert_eq!(c.a, 0.8);
        assert_eq!(paint.opacity, 0.9);
    }

    #[test]
    fn deserialize_text_style() {
        let json = r#"{
            "fontFamily": "Inter",
            "fontPostScriptName": "Inter-BoldItalic",
            "fontWeight": 700,
            "fontSize": 24.0,
            "textAlignHorizontal": "CENTER",
            "lineHeightPx": 32.0
        }"#;
        let style: FigmaTextStyle = serde_json::from_str(json).unwrap();
        assert_eq!(style.font_family.as_deref(), Some("Inter"));
        assert_eq!(
            style.font_post_script_name.as_deref(),
            Some("Inter-BoldItalic")
        );
        assert_eq!(style.font_weight, Some(700));
        assert_eq!(style.font_size, Some(24.0));
        assert_eq!(style.text_align_horizontal.as_deref(), Some("CENTER"));
        assert_eq!(style.line_height_px, Some(32.0));
    }

    #[test]
    fn deserialize_fill_geometry() {
        let json = r#"{"path": "M 0 0 L 100 0 L 100 100 Z"}"#;
        let path: FigmaPath = serde_json::from_str(json).unwrap();
        assert_eq!(path.path, "M 0 0 L 100 0 L 100 100 Z");
    }

    #[test]
    fn unknown_fields_tolerated() {
        let json = r#"{
            "type": "FRAME",
            "someNewFigmaField": true,
            "anotherField": [1, 2, 3]
        }"#;
        let node: FigmaNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.node_type, "FRAME");
    }

    #[test]
    fn missing_optional_fields_default() {
        let json = r#"{"type": "TEXT"}"#;
        let node: FigmaNode = serde_json::from_str(json).unwrap();
        assert!(node.characters.is_none());
        assert!(node.style.is_none());
        assert_eq!(node.corner_radius, 0.0);
        assert!(node.rectangle_corner_radii.is_none());
        assert!(node.image_ref.is_none());
    }

    #[test]
    fn deserialize_rectangle_corner_radii() {
        let json = r#"{
            "type": "RECTANGLE",
            "rectangleCornerRadii": [4.0, 8.0, 12.0, 16.0]
        }"#;
        let node: FigmaNode = serde_json::from_str(json).unwrap();
        let radii = node.rectangle_corner_radii.unwrap();
        assert_eq!(radii, [4.0, 8.0, 12.0, 16.0]);
    }

    #[test]
    fn deserialize_full_tree() {
        let json = r#"{
            "name": "My File",
            "document": {
                "id": "0:0",
                "name": "Document",
                "type": "DOCUMENT",
                "children": [
                    {
                        "id": "0:1",
                        "name": "Page 1",
                        "type": "CANVAS",
                        "children": [
                            {
                                "id": "1:1",
                                "name": "Frame 1",
                                "type": "FRAME",
                                "absoluteBoundingBox": {"x": 0, "y": 0, "width": 1920, "height": 1080},
                                "fills": [{"type": "SOLID", "color": {"r": 1, "g": 1, "b": 1, "a": 1}}],
                                "children": [
                                    {
                                        "id": "1:2",
                                        "name": "Hello",
                                        "type": "TEXT",
                                        "characters": "Hello World",
                                        "absoluteBoundingBox": {"x": 100, "y": 200, "width": 300, "height": 40},
                                        "style": {
                                            "fontFamily": "Inter",
                                            "fontSize": 24.0,
                                            "fontWeight": 400
                                        }
                                    }
                                ]
                            }
                        ]
                    }
                ]
            }
        }"#;
        let resp: FigmaFileResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.name, "My File");
        assert_eq!(resp.document.children.len(), 1);
        let canvas = &resp.document.children[0];
        assert_eq!(canvas.node_type, "CANVAS");
        assert_eq!(canvas.children.len(), 1);
        let frame = &canvas.children[0];
        assert_eq!(frame.node_type, "FRAME");
        assert_eq!(frame.children.len(), 1);
        let text = &frame.children[0];
        assert_eq!(text.node_type, "TEXT");
        assert_eq!(text.characters.as_deref(), Some("Hello World"));
    }

    #[test]
    fn deserialize_image_paint() {
        let json = r#"{
            "type": "IMAGE",
            "imageRef": "abc123",
            "visible": true,
            "opacity": 1.0
        }"#;
        let paint: FigmaPaint = serde_json::from_str(json).unwrap();
        assert_eq!(paint.paint_type, "IMAGE");
        assert_eq!(paint.image_ref.as_deref(), Some("abc123"));
    }
}
