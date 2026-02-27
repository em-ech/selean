//! Shape parsing from OOXML slide XML.

use quick_xml::Reader;
use quick_xml::events::Event;

use selean_common::types::NodeId;
use selean_engine::scene::{BoundingBox, Color, SceneNode, SceneNodeKind};

use super::text::parse_text_body;
use crate::coord::{emu_to_px, parse_ooxml_color};

/// Parses all shapes from a slide XML string.
///
/// Returns a list of `SceneNode` instances, one per `<p:sp>` shape found.
pub fn parse_shapes(slide_xml: &str) -> Vec<SceneNode> {
    let mut nodes = Vec::new();
    let mut reader = Reader::from_str(slide_xml);
    let mut buf = Vec::new();

    let mut in_sp = false;
    let mut sp_depth: u32 = 0;
    let mut shape_xml = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                let local = local_name(name.as_ref());
                if local == b"sp" && !in_sp {
                    in_sp = true;
                    sp_depth = 1;
                    shape_xml.clear();
                    append_start_tag(&mut shape_xml, e);
                } else if in_sp {
                    sp_depth += 1;
                    append_start_tag(&mut shape_xml, e);
                }
            }
            Ok(Event::End(ref e)) => {
                if in_sp {
                    append_end_tag(&mut shape_xml, e);
                    sp_depth -= 1;
                    let name = e.name();
                    let local = local_name(name.as_ref());
                    if sp_depth == 0 && local == b"sp" {
                        in_sp = false;
                        nodes.push(parse_single_shape(&shape_xml));
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                if in_sp {
                    append_empty_tag(&mut shape_xml, e);
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_sp {
                    shape_xml.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    nodes
}

/// Parses a single `<p:sp>` element XML fragment into a `SceneNode`.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn parse_single_shape(shape_xml: &str) -> SceneNode {
    let mut reader = Reader::from_str(shape_xml);
    let mut buf = Vec::new();

    let mut name = String::from("Shape");
    let mut off_x: i64 = 0;
    let mut off_y: i64 = 0;
    let mut ext_cx: i64 = 0;
    let mut ext_cy: i64 = 0;
    let mut fill_color: Option<Color> = None;
    let mut has_text_body = false;
    let mut in_txbody = false;
    let mut in_sppr = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let qname = e.name();
                let local = local_name(qname.as_ref());
                match local {
                    b"cNvPr" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                if let Ok(n) = std::str::from_utf8(&attr.value) {
                                    name = n.to_string();
                                }
                            }
                        }
                    }
                    b"spPr" => in_sppr = true,
                    b"txBody" => {
                        has_text_body = true;
                        in_txbody = true;
                    }
                    _ => {
                        parse_geometry_and_fill(
                            local,
                            e,
                            &mut off_x,
                            &mut off_y,
                            &mut ext_cx,
                            &mut ext_cy,
                            &mut fill_color,
                            in_sppr,
                            in_txbody,
                        );
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let qname = e.name();
                let local = local_name(qname.as_ref());
                match local {
                    b"cNvPr" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                if let Ok(n) = std::str::from_utf8(&attr.value) {
                                    name = n.to_string();
                                }
                            }
                        }
                    }
                    _ => {
                        parse_geometry_and_fill(
                            local,
                            e,
                            &mut off_x,
                            &mut off_y,
                            &mut ext_cx,
                            &mut ext_cy,
                            &mut fill_color,
                            in_sppr,
                            in_txbody,
                        );
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let qname = e.name();
                let local = local_name(qname.as_ref());
                match local {
                    b"spPr" => in_sppr = false,
                    b"txBody" => in_txbody = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    let x = emu_to_px(off_x);
    let y = emu_to_px(off_y);
    let width = emu_to_px(ext_cx);
    let height = emu_to_px(ext_cy);
    let bounds = BoundingBox::new(x, y, width, height);

    if has_text_body {
        let parsed = parse_text_body(shape_xml);
        let kind = SceneNodeKind::Text {
            content: parsed.content,
            font_size: parsed.font_size,
            font_family: parsed.font_family,
            font_weight: parsed.font_weight,
            font_style: parsed.font_style,
            text_align: parsed.text_align,
            line_height: 1.2,
            text_color: parsed.text_color,
        };
        let mut node = SceneNode::new(NodeId::new(), name, kind, bounds);
        node.fill = fill_color;
        node
    } else {
        let kind = SceneNodeKind::Frame {
            corner_radius: [0.0; 4],
        };
        let mut node = SceneNode::new(NodeId::new(), name, kind, bounds);
        node.fill = fill_color;
        node
    }
}

/// Parses offset, extent, and fill color from element attributes.
#[allow(clippy::too_many_arguments, clippy::similar_names)]
fn parse_geometry_and_fill(
    local: &[u8],
    e: &quick_xml::events::BytesStart<'_>,
    off_x: &mut i64,
    off_y: &mut i64,
    ext_cx: &mut i64,
    ext_cy: &mut i64,
    fill_color: &mut Option<Color>,
    in_sppr: bool,
    in_txbody: bool,
) {
    match local {
        b"off" => {
            for attr in e.attributes().flatten() {
                match attr.key.as_ref() {
                    b"x" => {
                        if let Ok(v) = std::str::from_utf8(&attr.value) {
                            *off_x = v.parse().unwrap_or(0);
                        }
                    }
                    b"y" => {
                        if let Ok(v) = std::str::from_utf8(&attr.value) {
                            *off_y = v.parse().unwrap_or(0);
                        }
                    }
                    _ => {}
                }
            }
        }
        b"ext" => {
            for attr in e.attributes().flatten() {
                match attr.key.as_ref() {
                    b"cx" => {
                        if let Ok(v) = std::str::from_utf8(&attr.value) {
                            *ext_cx = v.parse().unwrap_or(0);
                        }
                    }
                    b"cy" => {
                        if let Ok(v) = std::str::from_utf8(&attr.value) {
                            *ext_cy = v.parse().unwrap_or(0);
                        }
                    }
                    _ => {}
                }
            }
        }
        b"srgbClr" if in_sppr && !in_txbody => {
            for attr in e.attributes().flatten() {
                if attr.key.as_ref() == b"val" {
                    if let Ok(hex) = std::str::from_utf8(&attr.value) {
                        *fill_color = parse_ooxml_color(hex);
                    }
                }
            }
        }
        _ => {}
    }
}

fn append_start_tag(s: &mut String, e: &quick_xml::events::BytesStart<'_>) {
    s.push('<');
    s.push_str(&String::from_utf8_lossy(e.name().as_ref()));
    for attr in e.attributes().flatten() {
        s.push(' ');
        s.push_str(&String::from_utf8_lossy(attr.key.as_ref()));
        s.push_str("=\"");
        s.push_str(&String::from_utf8_lossy(&attr.value));
        s.push('"');
    }
    s.push('>');
}

fn append_empty_tag(s: &mut String, e: &quick_xml::events::BytesStart<'_>) {
    s.push('<');
    s.push_str(&String::from_utf8_lossy(e.name().as_ref()));
    for attr in e.attributes().flatten() {
        s.push(' ');
        s.push_str(&String::from_utf8_lossy(attr.key.as_ref()));
        s.push_str("=\"");
        s.push_str(&String::from_utf8_lossy(&attr.value));
        s.push('"');
    }
    s.push_str("/>");
}

fn append_end_tag(s: &mut String, e: &quick_xml::events::BytesEnd<'_>) {
    s.push_str("</");
    s.push_str(&String::from_utf8_lossy(e.name().as_ref()));
    s.push('>');
}

/// Extracts the local name from a potentially namespaced XML tag.
fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b':') {
        Some(pos) => &name[pos + 1..],
        None => name,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_slide() {
        let xml = r"<p:sld><p:cSld><p:spTree></p:spTree></p:cSld></p:sld>";
        let nodes = parse_shapes(xml);
        assert!(nodes.is_empty());
    }

    #[test]
    fn parse_rectangle_shape() {
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="Rectangle"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm>
                        <a:off x="914400" y="914400"/>
                        <a:ext cx="1828800" cy="914400"/>
                    </a:xfrm>
                    <a:solidFill><a:srgbClr val="FF0000"/></a:solidFill>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        let node = &nodes[0];
        assert!(matches!(node.kind, SceneNodeKind::Frame { .. }));
        // 914400 EMU = 96px
        assert!((node.bounds.x - 96.0).abs() < 1.0);
        assert!((node.bounds.width - 192.0).abs() < 1.0);
    }

    #[test]
    fn parse_shape_with_fill_color() {
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="Red Box"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm>
                        <a:off x="0" y="0"/>
                        <a:ext cx="914400" cy="914400"/>
                    </a:xfrm>
                    <a:solidFill><a:srgbClr val="FF0000"/></a:solidFill>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        let fill = nodes[0].fill.unwrap();
        assert!((fill.r - 1.0).abs() < 0.01);
        assert!(fill.g.abs() < 0.01);
    }

    #[test]
    fn parse_shape_with_text() {
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="Title"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm>
                        <a:off x="0" y="0"/>
                        <a:ext cx="914400" cy="457200"/>
                    </a:xfrm>
                </p:spPr>
                <p:txBody>
                    <a:p><a:r><a:t>Hello</a:t></a:r></a:p>
                </p:txBody>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0].kind, SceneNodeKind::Text { .. }));
        if let SceneNodeKind::Text { ref content, .. } = nodes[0].kind {
            assert_eq!(content, "Hello");
        }
    }

    #[test]
    fn parse_multiple_shapes() {
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="A"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm>
                </p:spPr>
            </p:sp>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="3" name="B"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm><a:off x="200" y="200"/><a:ext cx="300" cy="300"/></a:xfrm>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].name, "A");
        assert_eq!(nodes[1].name, "B");
    }

    #[test]
    fn parse_shape_name_preserved() {
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="My Custom Shape"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes[0].name, "My Custom Shape");
    }

    #[test]
    fn parse_shape_no_fill_defaults_none() {
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="NoFill"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert!(nodes[0].fill.is_none());
    }
}
