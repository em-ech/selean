//! IDML page item parsing: converts spread elements to scene nodes.
//!
//! IDML page items include `Rectangle`, `TextFrame`, `Oval`, `Polygon`, and `Group`.
//! Each uses `PathGeometry > GeometryPathType > PathPointArray > PathPointType`
//! for shape definition, unlike PPTX which uses simple offset+extent.

use std::collections::HashMap;
use std::fmt::Write as FmtWrite;

use quick_xml::Reader;
use quick_xml::events::Event;

use selean_common::types::NodeId;
use selean_engine::scene::{BoundingBox, Color, FontStyle, SceneNode, SceneNodeKind, TextAlign};

use super::text::ParsedStory;
use crate::coord::{parse_idml_color, parse_item_transform, pt_to_px, spread_to_topleft};

/// A single path point from IDML's `PathPointType`.
#[derive(Debug, Clone, Copy)]
pub struct PathPoint {
    /// The anchor point (x, y) in spread coordinates (points).
    pub anchor: (f32, f32),
    /// Left direction handle for curves.
    pub left_direction: (f32, f32),
    /// Right direction handle for curves.
    pub right_direction: (f32, f32),
}

/// Parses all top-level page items from a spread XML fragment.
///
/// `stories` maps story Self IDs to parsed story data for text frame lookup.
/// `half_w` and `half_h` are half the page dimensions in points, used for
/// coordinate conversion from spread-center to top-left origin.
#[allow(clippy::implicit_hasher)]
pub fn parse_page_items(
    xml: &str,
    stories: &HashMap<String, ParsedStory>,
    half_w: f32,
    half_h: f32,
) -> Vec<SceneNode> {
    let mut nodes = Vec::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut in_element = false;
    let mut element_depth: u32 = 0;
    let mut element_xml = String::new();
    let mut element_tag = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let qname = e.name();
                let local = local_name(qname.as_ref());
                let is_page_item = matches!(
                    local,
                    b"Rectangle" | b"TextFrame" | b"Oval" | b"Polygon" | b"Group"
                );
                if is_page_item && !in_element {
                    in_element = true;
                    element_depth = 1;
                    element_xml.clear();
                    element_tag = String::from_utf8_lossy(local).to_string();
                    append_start_tag(&mut element_xml, e);
                } else if in_element {
                    element_depth += 1;
                    append_start_tag(&mut element_xml, e);
                }
            }
            Ok(Event::End(ref e)) => {
                if in_element {
                    append_end_tag(&mut element_xml, e);
                    element_depth -= 1;
                    if element_depth == 0 {
                        in_element = false;
                        nodes.push(parse_single_element(
                            &element_tag,
                            &element_xml,
                            stories,
                            half_w,
                            half_h,
                        ));
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                if in_element {
                    append_empty_tag(&mut element_xml, e);
                } else {
                    let qname = e.name();
                    let local = local_name(qname.as_ref());
                    let is_page_item = matches!(
                        local,
                        b"Rectangle" | b"TextFrame" | b"Oval" | b"Polygon" | b"Group"
                    );
                    if is_page_item {
                        let tag = String::from_utf8_lossy(local).to_string();
                        let mut xml_str = String::new();
                        append_empty_tag(&mut xml_str, e);
                        nodes.push(parse_single_element(
                            &tag, &xml_str, stories, half_w, half_h,
                        ));
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_element {
                    element_xml.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    nodes
}

#[allow(clippy::too_many_lines, clippy::similar_names)]
fn parse_single_element(
    tag: &str,
    xml: &str,
    stories: &HashMap<String, ParsedStory>,
    half_w: f32,
    half_h: f32,
) -> SceneNode {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut name = String::from("Shape");
    let mut parent_story_id: Option<String> = None;
    let mut fill_color: Option<Color> = None;
    let mut stroke_color: Option<Color> = None;
    let mut stroke_width: f32 = 0.0;
    let mut transform = [1.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut path_points: Vec<PathPoint> = Vec::new();
    let mut in_path_geometry = false;
    let mut in_fill_color = false;
    let mut in_stroke_color = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let qname = e.name();
                let local = local_name(qname.as_ref());
                match local {
                    b"Rectangle" | b"TextFrame" | b"Oval" | b"Polygon" | b"Group" => {
                        parse_element_attrs(
                            e,
                            &mut name,
                            &mut parent_story_id,
                            &mut transform,
                            &mut stroke_width,
                        );
                    }
                    b"PathGeometry" => in_path_geometry = true,
                    b"PathPointType" if in_path_geometry => {
                        if let Some(pp) = parse_path_point(e) {
                            path_points.push(pp);
                        }
                    }
                    b"FillColor" => in_fill_color = true,
                    b"StrokeColor" => in_stroke_color = true,
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let qname = e.name();
                let local = local_name(qname.as_ref());
                match local {
                    b"PathGeometry" => in_path_geometry = false,
                    b"FillColor" => in_fill_color = false,
                    b"StrokeColor" => in_stroke_color = false,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_fill_color || in_stroke_color {
                    if let Ok(text) = e.unescape() {
                        let trimmed = text.trim();
                        if let Some(c) = try_parse_color_value_list(trimmed) {
                            if in_fill_color {
                                fill_color = Some(c);
                            } else {
                                stroke_color = Some(c);
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    // Compute bounds from path points or transform
    let (geo_x, geo_y, geo_w, geo_h) = if path_points.len() >= 2 {
        path_points_to_bounds(&path_points)
    } else {
        (transform[4], transform[5], 100.0, 100.0)
    };

    // Convert from spread-center points to top-left pixels
    let (tl_x, tl_y) =
        spread_to_topleft(geo_x + transform[4], geo_y + transform[5], half_w, half_h);
    let pixel_x = pt_to_px(tl_x);
    let pixel_y = pt_to_px(tl_y);
    let pixel_w = pt_to_px(geo_w).max(1.0);
    let pixel_h = pt_to_px(geo_h).max(1.0);

    let bounds = BoundingBox::new(pixel_x, pixel_y, pixel_w, pixel_h);

    let kind = match tag {
        "TextFrame" => {
            let story = parent_story_id.as_deref().and_then(|id| stories.get(id));
            match story {
                Some(s) => SceneNodeKind::Text {
                    content: s.content.clone(),
                    font_size: s.font_size,
                    font_family: s.font_family.clone(),
                    font_weight: s.font_weight,
                    font_style: s.font_style,
                    text_align: s.text_align,
                    line_height: 1.2,
                    text_color: s.text_color,
                },
                None => SceneNodeKind::Text {
                    content: String::new(),
                    font_size: 16.0,
                    font_family: "Inter".to_string(),
                    font_weight: 400,
                    font_style: FontStyle::Normal,
                    text_align: TextAlign::Left,
                    line_height: 1.2,
                    text_color: None,
                },
            }
        }
        "Oval" | "Polygon" => {
            let path_data = if path_points.len() >= 2 {
                path_points_to_svg_path(&path_points, geo_x, geo_y)
            } else {
                String::new()
            };
            SceneNodeKind::Vector { path_data }
        }
        "Group" => SceneNodeKind::Group,
        _ => SceneNodeKind::Frame {
            corner_radius: [0.0; 4],
        },
    };

    let mut node = SceneNode::new(NodeId::new(), name, kind, bounds);
    node.fill = fill_color;
    node.stroke = stroke_color;
    node.stroke_width = stroke_width;
    node
}

fn parse_element_attrs(
    e: &quick_xml::events::BytesStart<'_>,
    name: &mut String,
    parent_story_id: &mut Option<String>,
    transform: &mut [f32; 6],
    stroke_width: &mut f32,
) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"Name" => {
                if let Ok(v) = std::str::from_utf8(&attr.value) {
                    *name = v.to_string();
                }
            }
            b"ParentStory" => {
                if let Ok(v) = std::str::from_utf8(&attr.value) {
                    *parent_story_id = Some(v.to_string());
                }
            }
            b"ItemTransform" => {
                if let Ok(v) = std::str::from_utf8(&attr.value) {
                    *transform = parse_item_transform(v);
                }
            }
            b"StrokeWeight" => {
                if let Ok(v) = std::str::from_utf8(&attr.value) {
                    *stroke_width = v.parse().unwrap_or(0.0);
                }
            }
            _ => {}
        }
    }
}

fn parse_path_point(e: &quick_xml::events::BytesStart<'_>) -> Option<PathPoint> {
    let mut anchor = None;
    let mut left_dir = None;
    let mut right_dir = None;

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"Anchor" => {
                if let Ok(v) = std::str::from_utf8(&attr.value) {
                    anchor = parse_point_pair(v);
                }
            }
            b"LeftDirection" => {
                if let Ok(v) = std::str::from_utf8(&attr.value) {
                    left_dir = parse_point_pair(v);
                }
            }
            b"RightDirection" => {
                if let Ok(v) = std::str::from_utf8(&attr.value) {
                    right_dir = parse_point_pair(v);
                }
            }
            _ => {}
        }
    }

    let a = anchor?;
    Some(PathPoint {
        anchor: a,
        left_direction: left_dir.unwrap_or(a),
        right_direction: right_dir.unwrap_or(a),
    })
}

fn parse_point_pair(s: &str) -> Option<(f32, f32)> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 2 {
        let x: f32 = parts[0].parse().ok()?;
        let y: f32 = parts[1].parse().ok()?;
        Some((x, y))
    } else {
        None
    }
}

/// Computes bounding box from path points in points.
///
/// Returns `(min_x, min_y, width, height)`.
pub fn path_points_to_bounds(points: &[PathPoint]) -> (f32, f32, f32, f32) {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for p in points {
        min_x = min_x.min(p.anchor.0);
        min_y = min_y.min(p.anchor.1);
        max_x = max_x.max(p.anchor.0);
        max_y = max_y.max(p.anchor.1);
    }

    (min_x, min_y, max_x - min_x, max_y - min_y)
}

/// Converts path points to an SVG path string, relative to the bounding box origin.
///
/// Straight segments use `L`, curved segments use `C`.
pub fn path_points_to_svg_path(points: &[PathPoint], origin_x: f32, origin_y: f32) -> String {
    if points.is_empty() {
        return String::new();
    }

    let mut d = String::new();
    let first = &points[0];
    let _ = write!(
        d,
        "M{} {}",
        first.anchor.0 - origin_x,
        first.anchor.1 - origin_y
    );

    for i in 1..points.len() {
        let prev = &points[i - 1];
        let curr = &points[i];
        let is_curve = !is_same_point(prev.right_direction, prev.anchor)
            || !is_same_point(curr.left_direction, curr.anchor);

        if is_curve {
            let _ = write!(
                d,
                " C{} {},{} {},{} {}",
                prev.right_direction.0 - origin_x,
                prev.right_direction.1 - origin_y,
                curr.left_direction.0 - origin_x,
                curr.left_direction.1 - origin_y,
                curr.anchor.0 - origin_x,
                curr.anchor.1 - origin_y,
            );
        } else {
            let _ = write!(
                d,
                " L{} {}",
                curr.anchor.0 - origin_x,
                curr.anchor.1 - origin_y
            );
        }
    }

    // Close path
    let last = &points[points.len() - 1];
    let is_curve = !is_same_point(last.right_direction, last.anchor)
        || !is_same_point(first.left_direction, first.anchor);
    if is_curve {
        let _ = write!(
            d,
            " C{} {},{} {},{} {}",
            last.right_direction.0 - origin_x,
            last.right_direction.1 - origin_y,
            first.left_direction.0 - origin_x,
            first.left_direction.1 - origin_y,
            first.anchor.0 - origin_x,
            first.anchor.1 - origin_y,
        );
    }
    d.push_str(" Z");
    d
}

fn is_same_point(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 0.001 && (a.1 - b.1).abs() < 0.001
}

fn try_parse_color_value_list(text: &str) -> Option<Color> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() == 3 {
        parse_idml_color(parts[0], parts[1], parts[2])
    } else {
        None
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

fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b':') {
        Some(pos) => &name[pos + 1..],
        None => name,
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn empty_stories() -> HashMap<String, ParsedStory> {
        HashMap::new()
    }

    #[test]
    fn parse_rectangle() {
        let xml = r#"<Spread>
            <Rectangle Self="rect_1" Name="BlueBox" ItemTransform="1 0 0 1 0 0">
                <Properties>
                    <PathGeometry>
                        <GeometryPathType PathOpen="false">
                            <PathPointArray>
                                <PathPointType Anchor="-100 -50" LeftDirection="-100 -50" RightDirection="-100 -50"/>
                                <PathPointType Anchor="100 -50" LeftDirection="100 -50" RightDirection="100 -50"/>
                                <PathPointType Anchor="100 50" LeftDirection="100 50" RightDirection="100 50"/>
                                <PathPointType Anchor="-100 50" LeftDirection="-100 50" RightDirection="-100 50"/>
                            </PathPointArray>
                        </GeometryPathType>
                    </PathGeometry>
                </Properties>
            </Rectangle>
        </Spread>"#;
        let nodes = parse_page_items(xml, &empty_stories(), 306.0, 396.0);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0].kind, SceneNodeKind::Frame { .. }));
        assert_eq!(nodes[0].name, "BlueBox");
    }

    #[test]
    fn parse_text_frame_with_story() {
        let mut stories = HashMap::new();
        stories.insert(
            "story_1".to_string(),
            ParsedStory {
                content: "Hello IDML".to_string(),
                font_size: 32.0,
                ..ParsedStory::default()
            },
        );

        let xml = r#"<Spread>
            <TextFrame Self="tf_1" Name="Title" ParentStory="story_1" ItemTransform="1 0 0 1 0 0">
                <Properties>
                    <PathGeometry>
                        <GeometryPathType PathOpen="false">
                            <PathPointArray>
                                <PathPointType Anchor="-50 -25" LeftDirection="-50 -25" RightDirection="-50 -25"/>
                                <PathPointType Anchor="50 -25" LeftDirection="50 -25" RightDirection="50 -25"/>
                                <PathPointType Anchor="50 25" LeftDirection="50 25" RightDirection="50 25"/>
                                <PathPointType Anchor="-50 25" LeftDirection="-50 25" RightDirection="-50 25"/>
                            </PathPointArray>
                        </GeometryPathType>
                    </PathGeometry>
                </Properties>
            </TextFrame>
        </Spread>"#;
        let nodes = parse_page_items(xml, &stories, 306.0, 396.0);
        assert_eq!(nodes.len(), 1);
        if let SceneNodeKind::Text {
            ref content,
            font_size,
            ..
        } = nodes[0].kind
        {
            assert_eq!(content, "Hello IDML");
            assert!((font_size - 32.0).abs() < 0.01);
        } else {
            panic!("expected Text node");
        }
    }

    #[test]
    fn parse_text_frame_without_story() {
        let xml = r#"<Spread>
            <TextFrame Self="tf_2" Name="Empty" ParentStory="missing_story" ItemTransform="1 0 0 1 0 0">
                <Properties>
                    <PathGeometry>
                        <GeometryPathType PathOpen="false">
                            <PathPointArray>
                                <PathPointType Anchor="-50 -25" LeftDirection="-50 -25" RightDirection="-50 -25"/>
                                <PathPointType Anchor="50 25" LeftDirection="50 25" RightDirection="50 25"/>
                            </PathPointArray>
                        </GeometryPathType>
                    </PathGeometry>
                </Properties>
            </TextFrame>
        </Spread>"#;
        let nodes = parse_page_items(xml, &empty_stories(), 306.0, 396.0);
        assert_eq!(nodes.len(), 1);
        if let SceneNodeKind::Text { ref content, .. } = nodes[0].kind {
            assert!(content.is_empty());
        } else {
            panic!("expected Text node");
        }
    }

    #[test]
    fn parse_oval() {
        let xml = r#"<Spread>
            <Oval Self="oval_1" Name="Circle" ItemTransform="1 0 0 1 0 0">
                <Properties>
                    <PathGeometry>
                        <GeometryPathType PathOpen="false">
                            <PathPointArray>
                                <PathPointType Anchor="0 -50" LeftDirection="-27.6 -50" RightDirection="27.6 -50"/>
                                <PathPointType Anchor="50 0" LeftDirection="50 -27.6" RightDirection="50 27.6"/>
                                <PathPointType Anchor="0 50" LeftDirection="27.6 50" RightDirection="-27.6 50"/>
                                <PathPointType Anchor="-50 0" LeftDirection="-50 27.6" RightDirection="-50 -27.6"/>
                            </PathPointArray>
                        </GeometryPathType>
                    </PathGeometry>
                </Properties>
            </Oval>
        </Spread>"#;
        let nodes = parse_page_items(xml, &empty_stories(), 306.0, 396.0);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0].kind, SceneNodeKind::Vector { .. }));
        if let SceneNodeKind::Vector { ref path_data } = nodes[0].kind {
            assert!(path_data.contains('C'), "oval should use curve commands");
        }
    }

    #[test]
    fn parse_multiple_items() {
        let xml = r#"<Spread>
            <Rectangle Self="r1" Name="Box1" ItemTransform="1 0 0 1 -100 -100">
                <Properties>
                    <PathGeometry>
                        <GeometryPathType PathOpen="false">
                            <PathPointArray>
                                <PathPointType Anchor="-50 -50" LeftDirection="-50 -50" RightDirection="-50 -50"/>
                                <PathPointType Anchor="50 50" LeftDirection="50 50" RightDirection="50 50"/>
                            </PathPointArray>
                        </GeometryPathType>
                    </PathGeometry>
                </Properties>
            </Rectangle>
            <Rectangle Self="r2" Name="Box2" ItemTransform="1 0 0 1 100 100">
                <Properties>
                    <PathGeometry>
                        <GeometryPathType PathOpen="false">
                            <PathPointArray>
                                <PathPointType Anchor="-50 -50" LeftDirection="-50 -50" RightDirection="-50 -50"/>
                                <PathPointType Anchor="50 50" LeftDirection="50 50" RightDirection="50 50"/>
                            </PathPointArray>
                        </GeometryPathType>
                    </PathGeometry>
                </Properties>
            </Rectangle>
        </Spread>"#;
        let nodes = parse_page_items(xml, &empty_stories(), 306.0, 396.0);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].name, "Box1");
        assert_eq!(nodes[1].name, "Box2");
    }

    #[test]
    fn path_points_to_bounds_rect() {
        let points = vec![
            PathPoint {
                anchor: (-100.0, -50.0),
                left_direction: (-100.0, -50.0),
                right_direction: (-100.0, -50.0),
            },
            PathPoint {
                anchor: (100.0, -50.0),
                left_direction: (100.0, -50.0),
                right_direction: (100.0, -50.0),
            },
            PathPoint {
                anchor: (100.0, 50.0),
                left_direction: (100.0, 50.0),
                right_direction: (100.0, 50.0),
            },
            PathPoint {
                anchor: (-100.0, 50.0),
                left_direction: (-100.0, 50.0),
                right_direction: (-100.0, 50.0),
            },
        ];
        let (x, y, w, h) = path_points_to_bounds(&points);
        assert!((x - (-100.0)).abs() < 0.001);
        assert!((y - (-50.0)).abs() < 0.001);
        assert!((w - 200.0).abs() < 0.001);
        assert!((h - 100.0).abs() < 0.001);
    }

    #[test]
    fn path_points_to_svg_straight() {
        let points = vec![
            PathPoint {
                anchor: (0.0, 0.0),
                left_direction: (0.0, 0.0),
                right_direction: (0.0, 0.0),
            },
            PathPoint {
                anchor: (100.0, 0.0),
                left_direction: (100.0, 0.0),
                right_direction: (100.0, 0.0),
            },
            PathPoint {
                anchor: (100.0, 100.0),
                left_direction: (100.0, 100.0),
                right_direction: (100.0, 100.0),
            },
        ];
        let path = path_points_to_svg_path(&points, 0.0, 0.0);
        assert!(path.starts_with("M0 0"));
        assert!(path.contains("L100 0"));
        assert!(path.contains("L100 100"));
        assert!(path.ends_with(" Z"));
    }

    #[test]
    fn path_points_to_svg_curve() {
        let points = vec![
            PathPoint {
                anchor: (0.0, -50.0),
                left_direction: (-27.6, -50.0),
                right_direction: (27.6, -50.0),
            },
            PathPoint {
                anchor: (50.0, 0.0),
                left_direction: (50.0, -27.6),
                right_direction: (50.0, 27.6),
            },
        ];
        let path = path_points_to_svg_path(&points, 0.0, -50.0);
        assert!(path.contains('C'), "should contain curve command");
    }

    #[test]
    fn parse_empty_spread() {
        let xml = r"<Spread></Spread>";
        let nodes = parse_page_items(xml, &empty_stories(), 306.0, 396.0);
        assert!(nodes.is_empty());
    }

    #[test]
    fn parse_point_pair_valid() {
        let result = parse_point_pair("10.5 -20.3");
        assert!(result.is_some());
        let (x, y) = result.unwrap();
        assert!((x - 10.5).abs() < 0.001);
        assert!((y - (-20.3)).abs() < 0.001);
    }

    #[test]
    fn parse_point_pair_invalid() {
        assert!(parse_point_pair("bad").is_none());
        assert!(parse_point_pair("10.5").is_none());
        assert!(parse_point_pair("").is_none());
    }
}
