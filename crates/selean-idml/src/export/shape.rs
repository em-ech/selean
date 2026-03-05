//! Converts `SceneNode` to IDML element XML.
//!
//! Each node maps to an IDML page item element: Frame -> Rectangle,
//! Text -> `TextFrame`, Vector/Image/Group -> Rectangle fallback.

use std::fmt::Write as FmtWrite;

use selean_common::xml::xml_escape;
use selean_engine::scene::{FontStyle, SceneNode, SceneNodeKind, TextAlign};

use crate::coord::{color_to_idml_rgb, px_to_idml_font_size, px_to_pt};

/// Converts a scene node to an IDML element XML string.
///
/// `element_id` is a unique identifier for the element within the spread.
/// `story_id` is provided for text nodes and links to a separate story file.
/// `half_w` and `half_h` are half the page dimensions in points for coordinate
/// conversion from top-left to spread-center origin.
pub fn node_to_idml_element(
    node: &SceneNode,
    element_id: &str,
    story_id: Option<&str>,
    half_w: f32,
    half_h: f32,
) -> String {
    match &node.kind {
        SceneNodeKind::Text { .. } => {
            build_text_frame_xml(node, element_id, story_id.unwrap_or(""), half_w, half_h)
        }
        _ => build_rectangle_xml(node, element_id, half_w, half_h),
    }
}

/// Builds a story XML file for a text node.
///
/// Returns `(story_id, story_xml)`.
pub fn build_story_xml(node: &SceneNode, story_id: &str) -> String {
    let (content, font_size, font_family, font_weight, font_style, text_align, text_color) =
        match &node.kind {
            SceneNodeKind::Text {
                content,
                font_size,
                font_family,
                font_weight,
                font_style,
                text_align,
                text_color,
                ..
            } => (
                content.as_str(),
                *font_size,
                font_family.as_str(),
                *font_weight,
                *font_style,
                *text_align,
                *text_color,
            ),
            _ => return String::new(),
        };

    let pt_size = px_to_idml_font_size(font_size);
    let font_style_attr = match (font_weight >= 700, font_style == FontStyle::Italic) {
        (true, true) => r#" FontStyle="Bold Italic""#,
        (true, false) => r#" FontStyle="Bold""#,
        (false, true) => r#" FontStyle="Italic""#,
        (false, false) => r#" FontStyle="Regular""#,
    };

    let justification = match text_align {
        TextAlign::Left => "LeftAlign",
        TextAlign::Center => "CenterAlign",
        TextAlign::Right => "RightAlign",
        TextAlign::Justify => "FullyJustified",
    };

    let fill_color_attr = r#" FillColor="Color/Black""#;
    let _ = text_color;

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<idPkg:Story xmlns:idPkg="http://ns.adobe.com/AdobeInDesign/idml/1.0/packaging" DOMVersion="7.0">
    <Story Self="{story_id}" AppliedTOCStyle="n" TrackChanges="false" StoryTitle="" AppliedNamedGrid="n">
        <ParagraphStyleRange AppliedParagraphStyle="ParagraphStyle/$ID/NormalParagraphStyle" Justification="{justification}">
            <CharacterStyleRange AppliedCharacterStyle="CharacterStyle/$ID/[No character style]" PointSize="{pt_size}"{font_style_attr} AppliedFont="{font_family}"{fill_color_attr}>
                <Content>{content}</Content>
            </CharacterStyleRange>
        </ParagraphStyleRange>
    </Story>
</idPkg:Story>"#,
        content = xml_escape(content),
        font_family = xml_escape(font_family),
    )
}

fn build_rectangle_xml(node: &SceneNode, element_id: &str, half_w: f32, half_h: f32) -> String {
    let path_geometry = bounds_to_path_geometry(node, half_w, half_h);
    let transform = build_item_transform(node, half_w, half_h);
    let fill_xml = build_fill_xml(node);
    let name = xml_escape(&node.name);

    format!(
        r#"<Rectangle Self="{element_id}" Name="{name}" ItemTransform="{transform}" StrokeWeight="{stroke_w}">
    {fill_xml}<Properties>
        <PathGeometry>
            <GeometryPathType PathOpen="false">
                <PathPointArray>
{path_geometry}                </PathPointArray>
            </GeometryPathType>
        </PathGeometry>
    </Properties>
</Rectangle>"#,
        stroke_w = node.stroke_width,
    )
}

fn build_text_frame_xml(
    node: &SceneNode,
    element_id: &str,
    story_id: &str,
    half_w: f32,
    half_h: f32,
) -> String {
    let path_geometry = bounds_to_path_geometry(node, half_w, half_h);
    let transform = build_item_transform(node, half_w, half_h);
    let fill_xml = build_fill_xml(node);
    let name = xml_escape(&node.name);

    format!(
        r#"<TextFrame Self="{element_id}" Name="{name}" ParentStory="{story_id}" ItemTransform="{transform}">
    {fill_xml}<Properties>
        <PathGeometry>
            <GeometryPathType PathOpen="false">
                <PathPointArray>
{path_geometry}                </PathPointArray>
            </GeometryPathType>
        </PathGeometry>
    </Properties>
</TextFrame>"#
    )
}

/// Generates `PathPointType` elements from node bounds.
///
/// Converts pixel bounds to spread-center points. Each corner becomes a
/// `PathPointType` with anchor = left = right (straight edges).
fn bounds_to_path_geometry(node: &SceneNode, half_w: f32, half_h: f32) -> String {
    let x = px_to_pt(node.bounds.x) - half_w;
    let y = px_to_pt(node.bounds.y) - half_h;
    let w = px_to_pt(node.bounds.width);
    let h = px_to_pt(node.bounds.height);

    let corners = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];

    let mut xml = String::new();
    for (cx, cy) in &corners {
        let _ = writeln!(
            xml,
            "                    <PathPointType Anchor=\"{cx} {cy}\" LeftDirection=\"{cx} {cy}\" RightDirection=\"{cx} {cy}\"/>"
        );
    }
    xml
}

fn build_item_transform(node: &SceneNode, _half_w: f32, _half_h: f32) -> String {
    // Identity transform; position is encoded in PathGeometry
    let _ = node;
    "1 0 0 1 0 0".to_string()
}

fn build_fill_xml(node: &SceneNode) -> String {
    match node.fill {
        Some(c) => {
            let (r, g, b) = color_to_idml_rgb(&c);
            format!("<FillColor type=\"enumeration\">Color/RGB {r} {g} {b}</FillColor>\n    ")
        }
        None => String::new(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use selean_common::types::NodeId;
    use selean_engine::scene::{BoundingBox, Color};

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
    fn rectangle_to_xml() {
        let node = make_frame("Box", 100.0, 100.0, 200.0, 150.0);
        let xml = node_to_idml_element(&node, "rect_1", None, 306.0, 396.0);
        assert!(xml.contains("Rectangle"));
        assert!(xml.contains("Box"));
        assert!(xml.contains("PathPointType"));
    }

    #[test]
    fn rectangle_with_fill_to_xml() {
        let mut node = make_frame("Filled", 0.0, 0.0, 100.0, 50.0);
        node.fill = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        let xml = node_to_idml_element(&node, "rect_2", None, 306.0, 396.0);
        assert!(xml.contains("FillColor"));
        assert!(xml.contains("1.000000"));
    }

    #[test]
    fn text_frame_to_xml() {
        let node = SceneNode::new(
            NodeId::new(),
            "Title".to_string(),
            SceneNodeKind::Text {
                content: "Hello".to_string(),
                font_size: 32.0,
                font_family: "Arial".to_string(),
                font_weight: 700,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Center,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(10.0, 20.0, 200.0, 50.0),
        );
        let xml = node_to_idml_element(&node, "tf_1", Some("story_1"), 306.0, 396.0);
        assert!(xml.contains("TextFrame"));
        assert!(xml.contains(r#"ParentStory="story_1""#));
        assert!(xml.contains("Title"));
    }

    #[test]
    fn build_story_xml_for_text() {
        let node = SceneNode::new(
            NodeId::new(),
            "Title".to_string(),
            SceneNodeKind::Text {
                content: "Hello IDML".to_string(),
                font_size: 32.0,
                font_family: "Helvetica".to_string(),
                font_weight: 700,
                font_style: FontStyle::Italic,
                text_align: TextAlign::Center,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(0.0, 0.0, 200.0, 50.0),
        );
        let xml = build_story_xml(&node, "story_1");
        assert!(xml.contains("story_1"));
        assert!(xml.contains("Hello IDML"));
        assert!(xml.contains(r#"FontStyle="Bold Italic""#));
        assert!(xml.contains(r#"Justification="CenterAlign""#));
        assert!(xml.contains("Helvetica"));
    }

    #[test]
    fn build_story_xml_for_non_text_returns_empty() {
        let node = make_frame("Box", 0.0, 0.0, 100.0, 100.0);
        let xml = build_story_xml(&node, "story_x");
        assert!(xml.is_empty());
    }

    #[test]
    fn xml_escapes_special_chars() {
        let node = make_frame("A & B <C>", 0.0, 0.0, 50.0, 50.0);
        let xml = node_to_idml_element(&node, "rect_3", None, 306.0, 396.0);
        assert!(xml.contains("A &amp; B &lt;C&gt;"));
    }

    #[test]
    fn fallback_for_image_node() {
        let node = SceneNode::new(
            NodeId::new(),
            "Photo".to_string(),
            SceneNodeKind::Image {
                asset_ref: "img.png".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        let xml = node_to_idml_element(&node, "rect_4", None, 306.0, 396.0);
        assert!(xml.contains("Rectangle"));
        assert!(xml.contains("Photo"));
    }

    #[test]
    fn bounds_to_path_geometry_produces_four_points() {
        let node = make_frame("Test", 0.0, 0.0, 100.0, 100.0);
        let geom = bounds_to_path_geometry(&node, 306.0, 396.0);
        let count = geom.matches("PathPointType").count();
        assert_eq!(count, 4);
    }
}
