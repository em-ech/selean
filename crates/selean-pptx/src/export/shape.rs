//! Converts `SceneNode` to OOXML shape XML.

use selean_common::xml::xml_escape;
use selean_engine::scene::{Effect, FontStyle, Gradient, SceneNode, SceneNodeKind, TextAlign};

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

    let fill_xml = if node.fill_gradient.is_some() || node.fill.is_some() {
        build_fill_xml(node)
    } else {
        String::new()
    };

    let effects_xml = build_effects_xml(&node.effects);

    let text_color_xml = text_color
        .map(|c| {
            format!(
                "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
                color_to_ooxml_hex(c)
            )
        })
        .unwrap_or_default();

    let font_xml = format!(r#"<a:latin typeface="{}"/>"#, xml_escape(font_family));

    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{shape_id}" name="{name}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{fill_xml}{effects_xml}</p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:pPr algn="{algn}"/><a:r><a:rPr lang="en-US" sz="{sz}"{bold}{italic}>{text_color_xml}{font_xml}</a:rPr><a:t>{content}</a:t></a:r></a:p></p:txBody></p:sp>"#,
        name = xml_escape(&node.name),
        content = xml_escape(content),
    )
}

fn build_frame_shape_xml(node: &SceneNode, shape_id: u32) -> String {
    let x = px_to_emu(node.bounds.x);
    let y = px_to_emu(node.bounds.y);
    let cx = px_to_emu(node.bounds.width);
    let cy = px_to_emu(node.bounds.height);

    let fill_xml = build_fill_xml(node);
    let effects_xml = build_effects_xml(&node.effects);

    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{shape_id}" name="{name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{fill_xml}{effects_xml}</p:spPr></p:sp>"#,
        name = xml_escape(&node.name),
    )
}

/// Generates the fill XML for a shape, preferring gradient over solid.
fn build_fill_xml(node: &SceneNode) -> String {
    if let Some(ref gradient) = node.fill_gradient {
        return gradient_to_ooxml(gradient);
    }
    match node.fill {
        Some(c) => format!(
            "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            color_to_ooxml_hex(&c)
        ),
        None => "<a:noFill/>".to_string(),
    }
}

/// Converts an engine Gradient to OOXML `<a:gradFill>` XML.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn gradient_to_ooxml(gradient: &Gradient) -> String {
    match gradient {
        Gradient::Linear { start, end, stops } => {
            let gs_list = gradient_stops_to_ooxml(stops);
            let angle = gradient_points_to_ooxml_angle(*start, *end);
            format!(
                "<a:gradFill><a:gsLst>{gs_list}</a:gsLst><a:lin ang=\"{angle}\" scaled=\"1\"/></a:gradFill>"
            )
        }
        Gradient::Radial { stops, .. } => {
            let gs_list = gradient_stops_to_ooxml(stops);
            // Radial: use path fill type
            format!(
                "<a:gradFill><a:gsLst>{gs_list}</a:gsLst><a:path path=\"circle\"><a:fillToRect l=\"50000\" t=\"50000\" r=\"50000\" b=\"50000\"/></a:path></a:gradFill>"
            )
        }
    }
}

/// Converts gradient stops to OOXML `<a:gs>` elements.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn gradient_stops_to_ooxml(stops: &[selean_engine::scene::GradientStop]) -> String {
    use std::fmt::Write;
    let mut result = String::new();
    for s in stops {
        let pos = (s.position * 100_000.0).round() as i64;
        let hex = color_to_ooxml_hex(&s.color);
        let _ = write!(
            result,
            "<a:gs pos=\"{pos}\"><a:srgbClr val=\"{hex}\"/></a:gs>"
        );
    }
    result
}

/// Converts gradient start/end points to OOXML angle (60,000ths of a degree).
///
/// OOXML convention: 0 = left-to-right, 90 = top-to-bottom.
#[allow(clippy::cast_possible_truncation)]
fn gradient_points_to_ooxml_angle(start: [f32; 2], end: [f32; 2]) -> i64 {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let radians = dy.atan2(dx);
    let degrees = radians.to_degrees();
    let normalized = if degrees < 0.0 {
        degrees + 360.0
    } else {
        degrees
    };
    (normalized * 60_000.0).round() as i64
}

/// Generates `<a:effectLst>` XML for a node's effects.
///
/// Returns an empty string if the node has no exportable effects.
#[allow(clippy::cast_possible_truncation)]
fn build_effects_xml(effects: &[Effect]) -> String {
    let mut inner = String::new();
    for effect in effects {
        if let Effect::DropShadow {
            color,
            offset_x,
            offset_y,
            blur_radius,
        } = effect
        {
            use std::fmt::Write;
            let blur_emu = (*blur_radius * 12_700.0).round() as i64;
            let dist = (offset_x * offset_x + offset_y * offset_y).sqrt();
            let dist_emu = (dist * 12_700.0).round() as i64;
            let dir_deg = offset_y.atan2(*offset_x).to_degrees();
            let dir_normalized = if dir_deg < 0.0 {
                dir_deg + 360.0
            } else {
                dir_deg
            };
            let dir_60k = (dir_normalized * 60_000.0).round() as i64;
            let hex = color_to_ooxml_hex(color);
            let _ = write!(
                inner,
                "<a:outerShdw blurRad=\"{blur_emu}\" dist=\"{dist_emu}\" dir=\"{dir_60k}\"><a:srgbClr val=\"{hex}\"/></a:outerShdw>"
            );
        }
    }
    if inner.is_empty() {
        String::new()
    } else {
        format!("<a:effectLst>{inner}</a:effectLst>")
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use selean_common::types::NodeId;
    use selean_engine::scene::{BoundingBox, Color, Effect};

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
    fn frame_with_gradient_to_xml() {
        use selean_engine::scene::GradientStop;
        let mut node = SceneNode::new(
            NodeId::new(),
            "Gradient".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 50.0),
        );
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
        let xml = node_to_shape_xml(&node, 2);
        assert!(xml.contains("gradFill"));
        assert!(xml.contains("FF0000"));
        assert!(xml.contains("0000FF"));
        assert!(xml.contains("a:lin"));
        assert!(!xml.contains("noFill"));
        assert!(!xml.contains("solidFill"));
    }

    #[test]
    fn gradient_takes_priority_over_solid_fill() {
        use selean_engine::scene::GradientStop;
        let mut node = SceneNode::new(
            NodeId::new(),
            "Both".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 50.0),
        );
        node.fill = Some(Color::new(0.0, 1.0, 0.0, 1.0));
        node.fill_gradient = Some(Gradient::Linear {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
            stops: vec![GradientStop {
                position: 0.0,
                color: Color::new(1.0, 0.0, 0.0, 1.0),
            }],
        });
        let xml = node_to_shape_xml(&node, 2);
        assert!(xml.contains("gradFill"));
        assert!(!xml.contains("solidFill"));
    }

    #[test]
    fn frame_with_drop_shadow_to_xml() {
        let mut node = SceneNode::new(
            NodeId::new(),
            "Shadow".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 50.0),
        );
        node.effects = vec![Effect::DropShadow {
            color: Color::new(0.0, 0.0, 0.0, 1.0),
            offset_x: 0.0,
            offset_y: 3.0,
            blur_radius: 4.0,
        }];
        let xml = node_to_shape_xml(&node, 2);
        assert!(xml.contains("effectLst"));
        assert!(xml.contains("outerShdw"));
        // blurRad = 4 * 12700 = 50800
        assert!(xml.contains("blurRad=\"50800\""));
        assert!(xml.contains("000000"));
    }

    #[test]
    fn frame_no_effects_no_effect_lst() {
        let node = SceneNode::new(
            NodeId::new(),
            "NoFx".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 50.0),
        );
        let xml = node_to_shape_xml(&node, 2);
        assert!(!xml.contains("effectLst"));
        assert!(!xml.contains("outerShdw"));
    }

    #[test]
    fn text_with_drop_shadow_to_xml() {
        let mut node = SceneNode::new(
            NodeId::new(),
            "ShadowText".to_string(),
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
            BoundingBox::new(0.0, 0.0, 100.0, 30.0),
        );
        node.effects = vec![Effect::DropShadow {
            color: Color::new(1.0, 0.0, 0.0, 1.0),
            offset_x: 3.0,
            offset_y: 0.0,
            blur_radius: 2.0,
        }];
        let xml = node_to_shape_xml(&node, 2);
        assert!(xml.contains("effectLst"));
        assert!(xml.contains("outerShdw"));
        assert!(xml.contains("FF0000"));
    }

    #[test]
    fn blur_effect_not_exported() {
        let mut node = SceneNode::new(
            NodeId::new(),
            "BlurOnly".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 50.0),
        );
        node.effects = vec![Effect::Blur { radius: 10.0 }];
        let xml = node_to_shape_xml(&node, 2);
        // Blur has no OOXML equivalent for outerShdw, so effectLst should not appear
        assert!(!xml.contains("effectLst"));
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
