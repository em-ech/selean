//! JSX code generation from a scene graph.
//!
//! Performs a DFS walk of the [`SceneGraph`], emitting React component code
//! with Tailwind CSS classes for each node.

use selean_engine::scene::{SceneGraph, SceneNode, SceneNodeKind};

use crate::tailwind;

/// Renders a complete React component for a scene graph.
///
/// The component wraps all root nodes in a `relative` container div
/// sized to the page dimensions.
#[must_use]
pub fn render_scene(scene: &SceneGraph, page_name: &str, width: f32, height: f32) -> String {
    let component_name = sanitize_component_name(page_name);
    let children_jsx = render_root_nodes(scene, 2);

    format!(
        "export default function {component_name}() {{\n\
         {INDENT}return (\n\
         {INDENT}{INDENT}<div className=\"relative w-[{w}px] h-[{h}px]\">\n\
         {children_jsx}\
         {INDENT}{INDENT}</div>\n\
         {INDENT});\n\
         }}\n",
        w = format_f32(width),
        h = format_f32(height),
    )
}

/// Renders all root nodes of a scene graph at the given indent level.
fn render_root_nodes(scene: &SceneGraph, base_indent: usize) -> String {
    let mut out = String::new();
    for &root_id in scene.roots() {
        if let Some(node) = scene.get(root_id) {
            if node.visible {
                out.push_str(&render_node(node, scene, base_indent + 1));
            }
        }
    }
    out
}

/// Recursively renders a single node and its children as JSX.
#[must_use]
pub fn render_node(node: &SceneNode, scene: &SceneGraph, indent: usize) -> String {
    if !node.visible {
        return String::new();
    }

    match &node.kind {
        SceneNodeKind::Frame { .. } => render_frame_node(node, scene, indent),
        SceneNodeKind::Text { content, .. } => render_text_node(node, content, indent),
        SceneNodeKind::Image { asset_ref } => render_image_node(node, asset_ref, indent),
        SceneNodeKind::Vector { path_data } => render_vector_node(node, path_data, indent),
        SceneNodeKind::Group => render_group_node(node, scene, indent),
    }
}

/// Renders a `Frame` node as a `<div>` with children.
fn render_frame_node(node: &SceneNode, scene: &SceneGraph, indent: usize) -> String {
    let classes = tailwind::node_classes(node);
    let style_attr = build_style_attr(node);
    let pad = indent_str(indent);
    let children = render_children(node, scene, indent + 1);

    if children.is_empty() {
        format!("{pad}<div className=\"{classes}\"{style_attr} />\n")
    } else {
        format!(
            "{pad}<div className=\"{classes}\"{style_attr}>\n\
             {children}\
             {pad}</div>\n"
        )
    }
}

/// Renders a `Text` node as a `<p>` or heading element.
fn render_text_node(node: &SceneNode, content: &str, indent: usize) -> String {
    let classes = tailwind::node_classes(node);
    let style_attr = build_style_attr(node);
    let pad = indent_str(indent);
    let tag = text_tag(node);
    let escaped = escape_jsx(content);
    format!("{pad}<{tag} className=\"{classes}\"{style_attr}>{escaped}</{tag}>\n")
}

/// Renders an `Image` node as an `<img>` tag.
fn render_image_node(node: &SceneNode, asset_ref: &str, indent: usize) -> String {
    let classes = tailwind::node_classes(node);
    let style_attr = build_style_attr(node);
    let pad = indent_str(indent);
    let alt = escape_jsx(&node.name);
    format!("{pad}<img src=\"{asset_ref}\" alt=\"{alt}\" className=\"{classes}\"{style_attr} />\n")
}

/// Renders a `Vector` node as an inline SVG.
fn render_vector_node(node: &SceneNode, path_data: &str, indent: usize) -> String {
    let classes = tailwind::node_classes(node);
    let style_attr = build_style_attr(node);
    let pad = indent_str(indent);
    let b = &node.bounds;
    let fill = node
        .fill
        .as_ref()
        .map_or_else(|| "none".to_string(), tailwind::color_to_hex);

    format!(
        "{pad}<svg className=\"{classes}\"{style_attr} viewBox=\"0 0 {w} {h}\">\n\
         {pad}  <path d=\"{path_data}\" fill=\"{fill}\" />\n\
         {pad}</svg>\n",
        w = format_f32(b.width),
        h = format_f32(b.height),
    )
}

/// Renders a `Group` node as a `<div>` wrapper.
fn render_group_node(node: &SceneNode, scene: &SceneGraph, indent: usize) -> String {
    let classes = tailwind::node_classes(node);
    let style_attr = build_style_attr(node);
    let pad = indent_str(indent);
    let children = render_children(node, scene, indent + 1);

    if children.is_empty() {
        format!("{pad}<div className=\"{classes}\"{style_attr} />\n")
    } else {
        format!(
            "{pad}<div className=\"{classes}\"{style_attr}>\n\
             {children}\
             {pad}</div>\n"
        )
    }
}

/// Renders all visible children of a node.
fn render_children(parent: &SceneNode, scene: &SceneGraph, indent: usize) -> String {
    let mut out = String::new();
    for &child_id in &parent.children {
        if let Some(child) = scene.get(child_id) {
            if child.visible {
                out.push_str(&render_node(child, scene, indent));
            }
        }
    }
    out
}

/// Chooses a text HTML tag based on font size.
fn text_tag(node: &SceneNode) -> &'static str {
    if let SceneNodeKind::Text { font_size, .. } = &node.kind {
        match font_size {
            s if *s >= 40.0 => "h1",
            s if *s >= 32.0 => "h2",
            s if *s >= 24.0 => "h3",
            s if *s >= 20.0 => "h4",
            s if *s >= 18.0 => "h5",
            s if *s >= 16.0 => "h6",
            _ => "p",
        }
    } else {
        "p"
    }
}

/// Builds an inline `style` attribute for gradient fills.
///
/// Returns an empty string if no inline style is needed.
fn build_style_attr(node: &SceneNode) -> String {
    if let Some(style) = tailwind::gradient_style(node) {
        format!(" style={{{{\"{style}\"}}}}")
    } else {
        String::new()
    }
}

/// Converts a page name to a valid `PascalCase` React component name.
#[must_use]
pub fn sanitize_component_name(name: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for ch in name.chars() {
        if ch.is_alphanumeric() {
            if capitalize_next {
                for upper in ch.to_uppercase() {
                    result.push(upper);
                }
                capitalize_next = false;
            } else {
                result.push(ch);
            }
        } else {
            capitalize_next = true;
        }
    }

    // Component names must start with an uppercase letter.
    if result.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        result.insert(0, 'P');
    }

    if result.is_empty() {
        return "Untitled".to_string();
    }

    result
}

/// Escapes special characters for JSX text content.
fn escape_jsx(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('{', "&#123;")
        .replace('}', "&#125;")
}

/// Generates an indentation string of `n` levels (2 spaces per level).
fn indent_str(n: usize) -> String {
    "  ".repeat(n)
}

/// Two-space indent constant used in template literals.
const INDENT: &str = "  ";

/// Formats an f32 as a clean string.
fn format_f32(v: f32) -> String {
    if (v - v.round()).abs() < 0.001 {
        #[allow(clippy::cast_possible_truncation)]
        return format!("{}", v.round() as i32);
    }
    format!("{v:.1}")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use selean_common::types::NodeId;
    use selean_engine::scene::{BoundingBox, Color, FontStyle, SceneNodeKind, TextAlign};

    use super::*;

    /// Returns the list of visible root `NodeId`s for a scene graph.
    fn visible_root_ids(scene: &SceneGraph) -> Vec<NodeId> {
        scene
            .roots()
            .iter()
            .copied()
            .filter(|id| scene.get(*id).is_some_and(|n| n.visible))
            .collect()
    }

    fn make_scene_with_frame() -> SceneGraph {
        let mut scene = SceneGraph::new();
        let mut node = SceneNode::new(
            NodeId::new(),
            "Box".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(10.0, 20.0, 200.0, 100.0),
        );
        node.fill = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        scene.add_root(node);
        scene
    }

    #[test]
    fn empty_scene_renders_valid_component() {
        let scene = SceneGraph::new();
        let output = render_scene(&scene, "Empty Page", 800.0, 600.0);
        assert!(output.contains("function EmptyPage()"));
        assert!(output.contains("w-[800px]"));
        assert!(output.contains("h-[600px]"));
    }

    #[test]
    fn single_frame_renders_div() {
        let scene = make_scene_with_frame();
        let output = render_scene(&scene, "Test", 800.0, 600.0);
        assert!(output.contains("<div className="));
        assert!(output.contains("bg-[#ff0000]"));
    }

    #[test]
    fn text_node_renders_with_content() {
        let mut scene = SceneGraph::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Title".to_string(),
            SceneNodeKind::Text {
                content: "Hello World".to_string(),
                font_size: 32.0,
                font_family: "Inter".to_string(),
                font_weight: 700,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Center,
                line_height: 1.5,
                text_color: Some(Color::BLACK),
            },
            BoundingBox::new(0.0, 0.0, 400.0, 60.0),
        );
        scene.add_root(node);
        let output = render_scene(&scene, "Test", 800.0, 600.0);
        assert!(output.contains("Hello World"));
        assert!(output.contains("<h2")); // font_size 32 -> h2
        assert!(output.contains("font-[700]"));
    }

    #[test]
    fn image_node_renders_img_tag() {
        let mut scene = SceneGraph::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Photo".to_string(),
            SceneNodeKind::Image {
                asset_ref: "/images/hero.png".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 400.0, 300.0),
        );
        scene.add_root(node);
        let output = render_scene(&scene, "Test", 800.0, 600.0);
        assert!(output.contains("<img"));
        assert!(output.contains("src=\"/images/hero.png\""));
        assert!(output.contains("alt=\"Photo\""));
    }

    #[test]
    fn vector_node_renders_svg() {
        let mut scene = SceneGraph::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Arrow".to_string(),
            SceneNodeKind::Vector {
                path_data: "M0 0 L100 50 L0 100 Z".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        scene.add_root(node);
        let output = render_scene(&scene, "Test", 800.0, 600.0);
        assert!(output.contains("<svg"));
        assert!(output.contains("<path"));
        assert!(output.contains("M0 0 L100 50 L0 100 Z"));
    }

    #[test]
    fn nested_frame_with_child_text() {
        let mut scene = SceneGraph::new();
        let parent_id = NodeId::new();
        let parent = SceneNode::new(
            parent_id,
            "Container".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [8.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 400.0, 200.0),
        );
        scene.add_root(parent);
        let child = SceneNode::new(
            NodeId::new(),
            "Label".to_string(),
            SceneNodeKind::Text {
                content: "Nested text".to_string(),
                font_size: 14.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(10.0, 10.0, 200.0, 30.0),
        );
        scene.add_child(parent_id, child);
        let output = render_scene(&scene, "Nested", 800.0, 600.0);
        assert!(output.contains("Nested text"));
        // Parent div should contain child
        assert!(output.contains("</div>"));
    }

    #[test]
    fn multiple_root_nodes() {
        let mut scene = SceneGraph::new();
        let n1 = SceneNode::new(
            NodeId::new(),
            "A".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        let n2 = SceneNode::new(
            NodeId::new(),
            "B".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(200.0, 0.0, 100.0, 100.0),
        );
        scene.add_root(n1);
        scene.add_root(n2);
        let output = render_scene(&scene, "Multi", 800.0, 600.0);
        // Should have two self-closing divs (no children)
        let div_count = output.matches("<div className=").count();
        // 1 wrapper + 2 root nodes = 3 divs with className
        assert_eq!(div_count, 3);
    }

    // --- sanitize_component_name tests ---

    #[test]
    fn sanitize_simple_name() {
        assert_eq!(sanitize_component_name("Page 1"), "Page1");
    }

    #[test]
    fn sanitize_special_chars() {
        assert_eq!(sanitize_component_name("my-page_v2!"), "MyPageV2");
    }

    #[test]
    fn sanitize_starts_with_number() {
        assert_eq!(sanitize_component_name("1st page"), "P1stPage");
    }

    #[test]
    fn sanitize_empty_name() {
        assert_eq!(sanitize_component_name(""), "Untitled");
    }

    #[test]
    fn sanitize_all_special() {
        assert_eq!(sanitize_component_name("---"), "Untitled");
    }

    // --- escape_jsx tests ---

    #[test]
    fn escape_jsx_special_chars() {
        assert_eq!(escape_jsx("<script>"), "&lt;script&gt;");
        assert_eq!(escape_jsx("a & b"), "a &amp; b");
        assert_eq!(escape_jsx("{value}"), "&#123;value&#125;");
    }

    // --- text_tag tests ---

    #[test]
    fn text_tag_h1_for_large_font() {
        let node = SceneNode::new(
            NodeId::new(),
            "T".to_string(),
            SceneNodeKind::Text {
                content: String::new(),
                font_size: 48.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(0.0, 0.0, 200.0, 60.0),
        );
        assert_eq!(text_tag(&node), "h1");
    }

    #[test]
    fn text_tag_p_for_small_font() {
        let node = SceneNode::new(
            NodeId::new(),
            "T".to_string(),
            SceneNodeKind::Text {
                content: String::new(),
                font_size: 12.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(0.0, 0.0, 200.0, 20.0),
        );
        assert_eq!(text_tag(&node), "p");
    }

    #[test]
    fn invisible_root_node_skipped() {
        let mut scene = SceneGraph::new();
        let mut node = SceneNode::new(
            NodeId::new(),
            "Hidden".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        node.visible = false;
        scene.add_root(node);
        let ids = visible_root_ids(&scene);
        assert!(ids.is_empty());
    }
}
