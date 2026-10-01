//! Shape parsing from OOXML slide XML.

use quick_xml::Reader;
use quick_xml::events::Event;

use selean_common::types::NodeId;
use selean_common::xml::{
    append_empty_tag, append_end_tag, append_general_ref, append_start_tag, local_name,
};
use selean_engine::scene::{
    BoundingBox, Color, Effect, Gradient, GradientStop, SceneNode, SceneNodeKind,
};

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
            Ok(Event::GeneralRef(ref e)) => {
                if in_sp {
                    append_general_ref(&mut shape_xml, e);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!("XML parse error in PPTX shape import: {e}");
                break;
            }
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
    let mut fill_gradient: Option<Gradient> = None;
    let mut has_text_body = false;
    let mut in_txbody = false;
    let mut in_sppr = false;
    let mut in_grad_fill = false;
    let mut grad_stops: Vec<GradientStop> = Vec::new();
    let mut grad_angle: f32 = 0.0;
    let mut current_gs_pos: Option<f32> = None;
    let mut effects: Vec<Effect> = Vec::new();
    let mut in_effect_lst = false;
    let mut in_outer_shdw = false;
    let mut shdw_blur_rad: f32 = 0.0;
    let mut shdw_dist: f32 = 0.0;
    let mut shdw_dir: f32 = 0.0;
    let mut shdw_color: Option<Color> = None;

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
                    b"effectLst" if in_sppr && !in_txbody => {
                        in_effect_lst = true;
                    }
                    b"outerShdw" if in_effect_lst => {
                        in_outer_shdw = true;
                        shdw_blur_rad = 0.0;
                        shdw_dist = 0.0;
                        shdw_dir = 0.0;
                        shdw_color = None;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"blurRad" => {
                                    if let Ok(v) = std::str::from_utf8(&attr.value) {
                                        let emu: f32 = v.parse().unwrap_or(0.0);
                                        shdw_blur_rad = emu / 12_700.0;
                                    }
                                }
                                b"dist" => {
                                    if let Ok(v) = std::str::from_utf8(&attr.value) {
                                        let emu: f32 = v.parse().unwrap_or(0.0);
                                        shdw_dist = emu / 12_700.0;
                                    }
                                }
                                b"dir" => {
                                    if let Ok(v) = std::str::from_utf8(&attr.value) {
                                        shdw_dir = v.parse::<f32>().unwrap_or(0.0);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    b"gradFill" if in_sppr && !in_txbody => {
                        in_grad_fill = true;
                        grad_stops.clear();
                        grad_angle = 0.0;
                    }
                    b"gs" if in_grad_fill => {
                        current_gs_pos = None;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"pos" {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    // OOXML pos is in 1/1000 percent (0-100000)
                                    let raw: f32 = v.parse().unwrap_or(0.0);
                                    current_gs_pos = Some(raw / 100_000.0);
                                }
                            }
                        }
                    }
                    _ => {
                        if in_outer_shdw && local == b"srgbClr" {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"val" {
                                    if let Ok(hex) = std::str::from_utf8(&attr.value) {
                                        shdw_color = parse_ooxml_color(hex);
                                    }
                                }
                            }
                        } else if in_grad_fill {
                            parse_gradient_color(local, e, &mut grad_stops, &mut current_gs_pos);
                        }
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
                    b"lin" if in_grad_fill => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"ang" {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    grad_angle = v.parse::<f32>().unwrap_or(0.0);
                                }
                            }
                        }
                    }
                    _ => {
                        if in_outer_shdw && local == b"srgbClr" {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"val" {
                                    if let Ok(hex) = std::str::from_utf8(&attr.value) {
                                        shdw_color = parse_ooxml_color(hex);
                                    }
                                }
                            }
                        } else if in_grad_fill {
                            parse_gradient_color(local, e, &mut grad_stops, &mut current_gs_pos);
                        }
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
                    b"outerShdw" if in_outer_shdw => {
                        in_outer_shdw = false;
                        let angle_rad = (shdw_dir / 60_000.0_f32).to_radians();
                        let offset_x = shdw_dist * angle_rad.cos();
                        let offset_y = shdw_dist * angle_rad.sin();
                        let color = shdw_color.unwrap_or(Color::new(0.0, 0.0, 0.0, 0.5));
                        effects.push(Effect::DropShadow {
                            color,
                            offset_x,
                            offset_y,
                            blur_radius: shdw_blur_rad,
                        });
                    }
                    b"effectLst" if in_effect_lst => {
                        in_effect_lst = false;
                    }
                    b"gradFill" if in_grad_fill => {
                        in_grad_fill = false;
                        if !grad_stops.is_empty() {
                            let stops: Vec<GradientStop> =
                                grad_stops.iter().take(4).copied().collect();
                            let (start, end) = ooxml_angle_to_gradient_points(grad_angle);
                            fill_gradient = Some(Gradient::Linear { start, end, stops });
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!("XML parse error in PPTX shape import: {e}");
                break;
            }
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
        node.fill_gradient = fill_gradient;
        node.effects = effects;
        node
    } else {
        let kind = SceneNodeKind::Frame {
            corner_radius: [0.0; 4],
        };
        let mut node = SceneNode::new(NodeId::new(), name, kind, bounds);
        node.fill = fill_color;
        node.fill_gradient = fill_gradient;
        node.effects = effects;
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

/// Parses an `srgbClr` element inside a gradient stop and pushes a `GradientStop`.
fn parse_gradient_color(
    local: &[u8],
    e: &quick_xml::events::BytesStart<'_>,
    stops: &mut Vec<GradientStop>,
    current_pos: &mut Option<f32>,
) {
    if local == b"srgbClr" {
        if let Some(pos) = current_pos.take() {
            for attr in e.attributes().flatten() {
                if attr.key.as_ref() == b"val" {
                    if let Ok(hex) = std::str::from_utf8(&attr.value) {
                        if let Some(color) = parse_ooxml_color(hex) {
                            stops.push(GradientStop {
                                position: pos,
                                color,
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Converts an OOXML gradient angle to start/end points in [0..1] space.
///
/// OOXML angle is in 60,000ths of a degree. 0 = left-to-right, 5400000 = top-to-bottom.
fn ooxml_angle_to_gradient_points(angle_60k: f32) -> ([f32; 2], [f32; 2]) {
    let degrees = angle_60k / 60_000.0;
    let radians = degrees.to_radians();
    let dx = radians.cos();
    let dy = radians.sin();
    let start = [0.5 - dx * 0.5, 0.5 - dy * 0.5];
    let end = [0.5 + dx * 0.5, 0.5 + dy * 0.5];
    (start, end)
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
    fn parse_shape_text_keeps_entity_references() {
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
                    <a:p><a:r><a:t>R&amp;D &lt;caf&#233;&gt;</a:t></a:r></a:p>
                </p:txBody>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        if let SceneNodeKind::Text { ref content, .. } = nodes[0].kind {
            assert_eq!(content, "R&D <caf\u{e9}>");
        } else {
            panic!("expected Text node");
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
    fn parse_shape_with_gradient_fill() {
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="GradBox"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm>
                        <a:off x="0" y="0"/>
                        <a:ext cx="914400" cy="914400"/>
                    </a:xfrm>
                    <a:gradFill>
                        <a:gsLst>
                            <a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs>
                            <a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs>
                        </a:gsLst>
                        <a:lin ang="5400000" scaled="1"/>
                    </a:gradFill>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        let node = &nodes[0];
        assert!(node.fill_gradient.is_some());
        match node.fill_gradient.as_ref().unwrap() {
            Gradient::Linear { stops, .. } => {
                assert_eq!(stops.len(), 2);
                assert!((stops[0].position - 0.0).abs() < 1e-6);
                assert!((stops[0].color.r - 1.0).abs() < 0.01);
                assert!((stops[1].position - 1.0).abs() < 1e-6);
                assert!((stops[1].color.b - 1.0).abs() < 0.01);
            }
            Gradient::Radial { .. } => panic!("expected Linear gradient"),
        }
    }

    #[test]
    fn parse_shape_gradient_angle_top_to_bottom() {
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="TopBot"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm>
                        <a:off x="0" y="0"/>
                        <a:ext cx="100" cy="100"/>
                    </a:xfrm>
                    <a:gradFill>
                        <a:gsLst>
                            <a:gs pos="0"><a:srgbClr val="FFFFFF"/></a:gs>
                            <a:gs pos="100000"><a:srgbClr val="000000"/></a:gs>
                        </a:gsLst>
                        <a:lin ang="5400000"/>
                    </a:gradFill>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        let grad = nodes[0].fill_gradient.as_ref().unwrap();
        match grad {
            Gradient::Linear { start, end, .. } => {
                // 5400000 = 90 degrees = top-to-bottom
                assert!((start[1] - 0.0).abs() < 0.01);
                assert!((end[1] - 1.0).abs() < 0.01);
            }
            Gradient::Radial { .. } => panic!("expected Linear"),
        }
    }

    #[test]
    fn parse_shape_with_drop_shadow() {
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="ShadowBox"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm>
                        <a:off x="0" y="0"/>
                        <a:ext cx="914400" cy="914400"/>
                    </a:xfrm>
                    <a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill>
                    <a:effectLst>
                        <a:outerShdw blurRad="50800" dist="38100" dir="5400000">
                            <a:srgbClr val="000000"/>
                        </a:outerShdw>
                    </a:effectLst>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].effects.len(), 1);
        match &nodes[0].effects[0] {
            Effect::DropShadow {
                color,
                offset_x,
                offset_y,
                blur_radius,
            } => {
                // blurRad 50800 / 12700 = 4.0
                assert!((*blur_radius - 4.0).abs() < 0.01);
                // dist 38100 / 12700 = 3.0, dir 5400000 = 90 degrees
                // offset_x = 3.0 * cos(90) ~= 0.0
                // offset_y = 3.0 * sin(90) ~= 3.0
                assert!(offset_x.abs() < 0.01);
                assert!((*offset_y - 3.0).abs() < 0.01);
                // color is black
                assert!(color.r.abs() < 0.01);
                assert!(color.g.abs() < 0.01);
                assert!(color.b.abs() < 0.01);
            }
            Effect::Blur { .. } => panic!("expected DropShadow"),
        }
    }

    #[test]
    fn parse_shape_shadow_with_empty_srgb_clr() {
        // srgbClr as self-closing tag (Empty event)
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="ShadowEmpty"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm>
                        <a:off x="0" y="0"/>
                        <a:ext cx="100" cy="100"/>
                    </a:xfrm>
                    <a:effectLst>
                        <a:outerShdw blurRad="25400" dist="0" dir="0">
                            <a:srgbClr val="FF0000"/>
                        </a:outerShdw>
                    </a:effectLst>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes[0].effects.len(), 1);
        match &nodes[0].effects[0] {
            Effect::DropShadow {
                color, blur_radius, ..
            } => {
                assert!((*blur_radius - 2.0).abs() < 0.01);
                assert!((color.r - 1.0).abs() < 0.01);
                assert!(color.g.abs() < 0.01);
            }
            Effect::Blur { .. } => panic!("expected DropShadow"),
        }
    }

    #[test]
    fn parse_shape_no_effects_defaults_empty() {
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="NoFx"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert!(nodes[0].effects.is_empty());
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

    // ---- Error path tests ----

    #[test]
    fn parse_shape_missing_shape_id_still_produces_node() {
        // cNvPr without an id attribute; the parser should not panic.
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr name="NoId"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "NoId");
    }

    #[test]
    fn parse_shape_invalid_color_hex_ignored() {
        // "ZZZZZZ" is not valid hex; fill should remain None.
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="BadColor"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm>
                    <a:solidFill><a:srgbClr val="ZZZZZZ"/></a:solidFill>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].fill.is_none());
    }

    #[test]
    fn parse_shape_short_color_hex_ignored() {
        // "FF00" is only 4 chars, not 6; should be silently ignored.
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="ShortHex"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm>
                    <a:solidFill><a:srgbClr val="FF00"/></a:solidFill>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].fill.is_none());
    }

    #[test]
    fn parse_shape_missing_transform_defaults_to_zero() {
        // No <a:xfrm> at all; offsets and extents should default to 0.
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="NoXfrm"/></p:nvSpPr>
                <p:spPr/>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].bounds.x.abs() < f32::EPSILON);
        assert!(nodes[0].bounds.y.abs() < f32::EPSILON);
        assert!(nodes[0].bounds.width.abs() < f32::EPSILON);
        assert!(nodes[0].bounds.height.abs() < f32::EPSILON);
    }

    #[test]
    fn parse_empty_slide_tree_no_shapes() {
        // A slide with an empty spTree; should produce zero nodes.
        let xml = r"<p:sld><p:cSld><p:spTree/></p:cSld></p:sld>";
        let nodes = parse_shapes(xml);
        assert!(nodes.is_empty());
    }

    #[test]
    fn parse_malformed_xml_does_not_panic() {
        // Completely broken XML; the parser should bail gracefully.
        let xml = r"<p:spTree><p:sp><broken<<>></p:sp>";
        let nodes = parse_shapes(xml);
        // May produce partial results or nothing, but must not panic.
        let _ = nodes;
    }

    #[test]
    fn parse_shape_negative_emu_coordinates() {
        // Negative EMU values should produce negative pixel values.
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="Negative"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm>
                        <a:off x="-914400" y="-457200"/>
                        <a:ext cx="1828800" cy="914400"/>
                    </a:xfrm>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        assert!((nodes[0].bounds.x - (-96.0)).abs() < 1.0);
        assert!((nodes[0].bounds.y - (-48.0)).abs() < 1.0);
        assert!((nodes[0].bounds.width - 192.0).abs() < 1.0);
    }

    #[test]
    fn parse_shape_overflow_emu_coordinates() {
        // Very large EMU values should not panic (precision loss is OK).
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="Huge"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm>
                        <a:off x="9223372036854775000" y="0"/>
                        <a:ext cx="0" cy="0"/>
                    </a:xfrm>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        // Just verifying it doesn't panic; the pixel value will be very large.
        assert!(nodes[0].bounds.x > 0.0);
    }

    #[test]
    fn parse_shape_missing_text_body_produces_frame() {
        // A shape with no <p:txBody> should produce a Frame, not Text.
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="JustFrame"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="914400"/></a:xfrm>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0].kind, SceneNodeKind::Frame { .. }));
    }

    #[test]
    fn parse_shape_empty_text_body_with_paragraph_produces_text() {
        // <txBody> with an empty paragraph; the Start event sets has_text_body.
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="EmptyText"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="457200"/></a:xfrm>
                </p:spPr>
                <p:txBody><a:p/></p:txBody>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0].kind, SceneNodeKind::Text { .. }));
        if let SceneNodeKind::Text { ref content, .. } = nodes[0].kind {
            assert!(content.is_empty());
        }
    }

    #[test]
    fn parse_shape_self_closing_text_body_produces_frame() {
        // A self-closing <p:txBody/> is an Empty event, not a Start event,
        // so has_text_body is never set and the shape becomes a Frame.
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="SelfClose"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="457200"/></a:xfrm>
                </p:spPr>
                <p:txBody/>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0].kind, SceneNodeKind::Frame { .. }));
    }

    #[test]
    fn parse_shape_unknown_elements_gracefully_ignored() {
        // Unknown child elements inside <p:sp> should be silently skipped.
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="WithUnknown"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm>
                    <a:customFill><a:unknownElement foo="bar"/></a:customFill>
                </p:spPr>
                <p:unknownSection>
                    <p:weirdChild attr="val"/>
                </p:unknownSection>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "WithUnknown");
    }

    #[test]
    fn parse_shape_non_numeric_emu_defaults_to_zero() {
        // Non-numeric values in coordinate attributes default to 0.
        let xml = r#"
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:cNvPr id="2" name="BadCoords"/></p:nvSpPr>
                <p:spPr>
                    <a:xfrm>
                        <a:off x="abc" y="def"/>
                        <a:ext cx="ghi" cy="jkl"/>
                    </a:xfrm>
                </p:spPr>
            </p:sp>
        </p:spTree>"#;
        let nodes = parse_shapes(xml);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].bounds.x.abs() < f32::EPSILON);
        assert!(nodes[0].bounds.y.abs() < f32::EPSILON);
        assert!(nodes[0].bounds.width.abs() < f32::EPSILON);
        assert!(nodes[0].bounds.height.abs() < f32::EPSILON);
    }
}
