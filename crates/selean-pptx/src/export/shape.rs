//! Converts `SceneNode` to OOXML shape XML.

use selean_engine::scene::{FontStyle, SceneNode, SceneNodeKind, TextAlign};

use crate::coord::{color_to_ooxml_hex, px_to_emu, px_to_ooxml_font_size};

/// Converts a scene node to OOXML shape XML (`<p:sp>` element).
pub fn node_to_shape_xml(node: &SceneNode, shape_id: u32) -> String {
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
        } => build_text_shape_xml(
            node,
            shape_id,
            content,
            *font_size,
            font_family,
            *font_weight,
            *font_style,
            *text_align,
            text_color.as_ref(),
        ),
        SceneNodeKind::Frame { .. } => build_frame_shape_xml(node, shape_id),
        _ => build_fallback_shape_xml(node, shape_id),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_text_shape_xml(
    node: &SceneNode,
    shape_id: u32,
    content: &str,
    font_size: f32,
    font_family: &str,
    font_weight: u16,
    font_style: FontStyle,
    text_align: TextAlign,
    text_color: Option<&selean_engine::scene::Color>,
) -> String {
    let x = px_to_emu(node.bounds.x);
    let y = px_to_emu(node.bounds.y);
    let cx = px_to_emu(node.bounds.width);
    let cy = px_to_emu(node.bounds.height);
    let sz = px_to_ooxml_font_size(font_size);
    let bold = if font_weight >= 700 { r#" b="1""# } else { "" };
    let italic = if font_style == FontStyle::Italic {
        r#" i="1""#
    } else {
        ""
    };
    let algn = match text_align {
        TextAlign::Left => "l",
        TextAlign::Center => "ctr",
        TextAlign::Right => "r",
        TextAlign::Justify => "just",
    };

    let fill_xml = node
        .fill
        .map(|c| {
            format!(
                "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
                color_to_ooxml_hex(&c)
            )
        })
        .unwrap_or_default();

    let text_color_xml = text_color
        .map(|c| {
            format!(
                "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
                color_to_ooxml_hex(c)
            )
        })
        .unwrap_or_default();

    let font_xml = format!(r#"<a:latin typeface="{font_family}"/>"#);

    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{shape_id}" name="{name}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{fill_xml}</p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:pPr algn="{algn}"/><a:r><a:rPr lang="en-US" sz="{sz}"{bold}{italic}>{text_color_xml}{font_xml}</a:rPr><a:t>{content}</a:t></a:r></a:p></p:txBody></p:sp>"#,
        name = xml_escape(&node.name),
        content = xml_escape(content),
    )
}

fn build_frame_shape_xml(node: &SceneNode, shape_id: u32) -> String {
    let x = px_to_emu(node.bounds.x);
    let y = px_to_emu(node.bounds.y);
    let cx = px_to_emu(node.bounds.width);
    let cy = px_to_emu(node.bounds.height);

    let fill_xml = match node.fill {
        Some(c) => format!(
            "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            color_to_ooxml_hex(&c)
        ),
        None => "<a:noFill/>".to_string(),
    };

    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{shape_id}" name="{name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{fill_xml}</p:spPr></p:sp>"#,
        name = xml_escape(&node.name),
    )
}

fn build_fallback_shape_xml(node: &SceneNode, shape_id: u32) -> String {
    let x = px_to_emu(node.bounds.x);
    let y = px_to_emu(node.bounds.y);
    let cx = px_to_emu(node.bounds.width);
    let cy = px_to_emu(node.bounds.height);

    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{shape_id}" name="{name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr></p:sp>"#,
        name = xml_escape(&node.name),
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use selean_common::types::NodeId;
    use selean_engine::scene::{BoundingBox, Color};

    #[test]
    fn frame_to_shape_xml() {
        let node = SceneNode::new(
            NodeId::new(),
            "Box".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 50.0),
        );
        let xml = node_to_shape_xml(&node, 2);
        assert!(xml.contains("p:sp"));
        assert!(xml.contains("Box"));
        assert!(xml.contains("noFill"));
    }

    #[test]
    fn frame_with_fill_to_xml() {
        let mut node = SceneNode::new(
            NodeId::new(),
            "Filled".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 50.0),
        );
        node.fill = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        let xml = node_to_shape_xml(&node, 2);
        assert!(xml.contains("srgbClr"));
        assert!(xml.contains("FF0000"));
        assert!(!xml.contains("noFill"));
    }

    #[test]
    fn text_node_to_xml() {
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
        let xml = node_to_shape_xml(&node, 3);
        assert!(xml.contains("Hello"));
        assert!(xml.contains("txBox"));
        assert!(xml.contains(r#"b="1""#));
        assert!(xml.contains(r#"algn="ctr""#));
        assert!(xml.contains(r#"typeface="Arial""#));
    }

    #[test]
    fn text_node_italic_to_xml() {
        let node = SceneNode::new(
            NodeId::new(),
            "Italic".to_string(),
            SceneNodeKind::Text {
                content: "Fancy".to_string(),
                font_size: 16.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Italic,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(0.0, 0.0, 100.0, 30.0),
        );
        let xml = node_to_shape_xml(&node, 2);
        assert!(xml.contains(r#"i="1""#));
        assert!(!xml.contains(r#"b="1""#));
    }

    #[test]
    fn text_node_with_color_to_xml() {
        let node = SceneNode::new(
            NodeId::new(),
            "Colored".to_string(),
            SceneNodeKind::Text {
                content: "Red text".to_string(),
                font_size: 16.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: Some(Color::new(1.0, 0.0, 0.0, 1.0)),
            },
            BoundingBox::new(0.0, 0.0, 100.0, 30.0),
        );
        let xml = node_to_shape_xml(&node, 2);
        assert!(xml.contains("FF0000"));
    }

    #[test]
    fn xml_escapes_special_chars() {
        let node = SceneNode::new(
            NodeId::new(),
            "A & B <C>".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 50.0, 50.0),
        );
        let xml = node_to_shape_xml(&node, 2);
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
        let xml = node_to_shape_xml(&node, 2);
        assert!(xml.contains("Photo"));
        assert!(xml.contains("noFill"));
    }

    #[test]
    fn fallback_for_vector_node() {
        let node = SceneNode::new(
            NodeId::new(),
            "Arrow".to_string(),
            SceneNodeKind::Vector {
                path_data: "M0 0 L10 10".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 50.0, 50.0),
        );
        let xml = node_to_shape_xml(&node, 2);
        assert!(xml.contains("Arrow"));
        assert!(xml.contains("noFill"));
    }

    #[test]
    fn fallback_for_group_node() {
        let node = SceneNode::new(
            NodeId::new(),
            "Container".to_string(),
            SceneNodeKind::Group,
            BoundingBox::new(0.0, 0.0, 200.0, 200.0),
        );
        let xml = node_to_shape_xml(&node, 2);
        assert!(xml.contains("Container"));
    }
}
