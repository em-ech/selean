//! Tool definitions and execution mapping.
//!
//! [`all_tools`] returns the complete set of tool schemas for the Claude API.
//! [`map_tool_call`] converts a tool name + JSON args into `CommandDescriptor`
//! values ready for execution through the shared mutation pipeline.

use selean_common::types::NodeId;
use selean_engine::command::CommandDescriptor;
use selean_engine::scene::{
    BlendMode, BoundingBox, Color, FontStyle, SceneNode, SceneNodeKind, TextAlign,
};

use crate::schema::{ToolBuilder, ToolDefinition};

/// Error returned when a tool call cannot be mapped to commands.
#[derive(Debug, thiserror::Error)]
pub enum ToolCallError {
    /// The tool name is not recognized.
    #[error("unknown tool: {0}")]
    UnknownTool(String),
    /// The tool arguments failed to parse.
    #[error("invalid arguments: {0}")]
    InvalidArgs(String),
    /// A referenced node ID is not a valid UUID.
    #[error("invalid node ID: {0}")]
    InvalidNodeId(String),
}

/// Returns all tool definitions for the Claude API.
#[must_use]
pub fn all_tools() -> Vec<ToolDefinition> {
    vec![
        tool_get_scene_summary(),
        tool_get_node(),
        tool_query_nodes(),
        tool_set_fill(),
        tool_set_bounds(),
        tool_set_text(),
        tool_set_opacity(),
        tool_set_visible(),
        tool_set_name(),
        tool_set_stroke(),
        tool_set_blend_mode(),
        tool_create_node(),
        tool_delete_node(),
        tool_set_corner_radius(),
        tool_set_font_family(),
        tool_set_font_weight(),
        tool_set_text_align(),
        tool_set_line_height(),
        tool_set_text_color(),
        tool_add_child_node(),
        tool_reparent_node(),
        tool_reorder_children(),
    ]
}

/// Returns `true` if the given tool name is read-only (queries scene state
/// without producing mutations).
#[must_use]
pub fn is_read_only_tool(name: &str) -> bool {
    matches!(name, "get_scene_summary" | "get_node" | "query_nodes")
}

/// Maps a tool call (name + JSON args) to a list of `CommandDescriptor` values.
///
/// Read-only tools (`get_scene_summary`, `get_node`, `query_nodes`) return an empty
/// vec because they do not mutate the scene. The caller should handle read
/// results separately.
///
/// # Errors
///
/// Returns `ToolCallError` if the tool name is unknown or args are invalid.
pub fn map_tool_call(
    name: &str,
    args: &serde_json::Value,
) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    if is_read_only_tool(name) {
        return Ok(vec![]);
    }
    match name {
        "set_fill" => map_set_fill(args),
        "set_bounds" => map_set_bounds(args),
        "set_text" => map_set_text(args),
        "set_opacity" => map_set_opacity(args),
        "set_visible" => map_set_visible(args),
        "set_name" => map_set_name(args),
        "set_stroke" => map_set_stroke(args),
        "set_blend_mode" => map_set_blend_mode(args),
        "create_node" => map_create_node(args),
        "delete_node" => map_delete_node(args),
        "set_corner_radius" => map_set_corner_radius(args),
        "set_font_family" => map_set_font_family(args),
        "set_font_weight" => map_set_font_weight(args),
        "set_text_align" => map_set_text_align(args),
        "set_line_height" => map_set_line_height(args),
        "set_text_color" => map_set_text_color(args),
        "add_child_node" => map_add_child_node(args),
        "reparent_node" => map_reparent_node(args),
        "reorder_children" => map_reorder_children(args),
        _ => Err(ToolCallError::UnknownTool(name.to_string())),
    }
}

// --- Tool definitions ---

fn tool_get_scene_summary() -> ToolDefinition {
    ToolBuilder::new(
        "get_scene_summary",
        "Get a summary of the current scene including all nodes, their types, positions, and hierarchy. Use this to understand the current state of the design before making changes.",
    )
    .build()
}

fn tool_get_node() -> ToolDefinition {
    ToolBuilder::new(
        "get_node",
        "Get detailed properties of a specific node by ID, including fill, stroke, opacity, bounds, text content, and hierarchy.",
    )
    .string_param("node_id", "The UUID of the node to inspect")
    .build()
}

fn tool_query_nodes() -> ToolDefinition {
    ToolBuilder::new(
        "query_nodes",
        "Search for nodes by name pattern or kind. Returns matching node IDs and names.",
    )
    .optional_string_param(
        "name_pattern",
        "Substring to match against node names (case-insensitive)",
    )
    .optional_string_param(
        "kind",
        "Filter by node kind: Frame, Text, Image, Vector, or Group",
    )
    .build()
}

fn tool_set_fill() -> ToolDefinition {
    ToolBuilder::new(
        "set_fill",
        "Set the fill color of a node. Colors are specified as RGBA values from 0.0 to 1.0. Set to null to remove the fill.",
    )
    .string_param("node_id", "The UUID of the node to modify")
    .optional_number_param("r", "Red component (0.0-1.0). Omit all color params to clear fill.")
    .optional_number_param("g", "Green component (0.0-1.0)")
    .optional_number_param("b", "Blue component (0.0-1.0)")
    .optional_number_param("a", "Alpha component (0.0-1.0), defaults to 1.0")
    .build()
}

fn tool_set_bounds() -> ToolDefinition {
    ToolBuilder::new("set_bounds", "Set the position and size of a node.")
        .string_param("node_id", "The UUID of the node to modify")
        .number_param("x", "Left edge x coordinate in logical pixels")
        .number_param("y", "Top edge y coordinate in logical pixels")
        .number_param("width", "Width in logical pixels")
        .number_param("height", "Height in logical pixels")
        .build()
}

fn tool_set_text() -> ToolDefinition {
    ToolBuilder::new(
        "set_text",
        "Set the text content and optionally the font size of a Text node.",
    )
    .string_param("node_id", "The UUID of the Text node to modify")
    .string_param("content", "The new text content")
    .optional_number_param(
        "font_size",
        "Font size in logical pixels (unchanged if omitted)",
    )
    .build()
}

fn tool_set_opacity() -> ToolDefinition {
    ToolBuilder::new(
        "set_opacity",
        "Set the opacity of a node. 1.0 is fully opaque, 0.0 is fully transparent.",
    )
    .string_param("node_id", "The UUID of the node to modify")
    .number_param("opacity", "Opacity value from 0.0 to 1.0")
    .build()
}

fn tool_set_visible() -> ToolDefinition {
    ToolBuilder::new(
        "set_visible",
        "Show or hide a node. Hidden nodes and their children are not rendered.",
    )
    .string_param("node_id", "The UUID of the node to modify")
    .boolean_param("visible", "true to show, false to hide")
    .build()
}

fn tool_set_name() -> ToolDefinition {
    ToolBuilder::new("set_name", "Rename a node.")
        .string_param("node_id", "The UUID of the node to rename")
        .string_param("name", "The new name for the node")
        .build()
}

fn tool_set_stroke() -> ToolDefinition {
    ToolBuilder::new(
        "set_stroke",
        "Set the stroke (outline) color and width of a node. Omit color params to clear the stroke.",
    )
    .string_param("node_id", "The UUID of the node to modify")
    .optional_number_param("r", "Red component (0.0-1.0). Omit all color params to clear stroke.")
    .optional_number_param("g", "Green component (0.0-1.0)")
    .optional_number_param("b", "Blue component (0.0-1.0)")
    .optional_number_param("a", "Alpha component (0.0-1.0), defaults to 1.0")
    .optional_number_param("width", "Stroke width in logical pixels")
    .build()
}

fn tool_set_blend_mode() -> ToolDefinition {
    ToolBuilder::new(
        "set_blend_mode",
        "Set the blend mode of a node, controlling how it composites with elements behind it.",
    )
    .string_param("node_id", "The UUID of the node to modify")
    .enum_param(
        "blend_mode",
        "The blend mode to apply",
        &[
            "Normal",
            "Add",
            "Multiply",
            "Screen",
            "Overlay",
            "Darken",
            "Lighten",
            "ColorDodge",
            "ColorBurn",
            "HardLight",
            "SoftLight",
            "Difference",
            "Exclusion",
        ],
    )
    .build()
}

fn tool_create_node() -> ToolDefinition {
    ToolBuilder::new(
        "create_node",
        "Create a new node and add it to the scene as a root element. Returns the new node's ID.",
    )
    .string_param("name", "Human-readable name for the node")
    .enum_param(
        "kind",
        "The visual type of the node",
        &["Frame", "Text", "Group"],
    )
    .number_param("x", "Left edge x coordinate")
    .number_param("y", "Top edge y coordinate")
    .number_param("width", "Width in logical pixels")
    .number_param("height", "Height in logical pixels")
    .optional_number_param("fill_r", "Fill red component (0.0-1.0)")
    .optional_number_param("fill_g", "Fill green component (0.0-1.0)")
    .optional_number_param("fill_b", "Fill blue component (0.0-1.0)")
    .optional_number_param("fill_a", "Fill alpha component (0.0-1.0)")
    .optional_string_param("text_content", "Text content (required if kind is Text)")
    .optional_number_param("font_size", "Font size (for Text nodes, default 16.0)")
    .optional_number_param(
        "corner_radius",
        "Corner radius for Frame nodes (default 0.0)",
    )
    .build()
}

fn tool_delete_node() -> ToolDefinition {
    ToolBuilder::new(
        "delete_node",
        "Remove a node and its entire subtree from the scene.",
    )
    .string_param("node_id", "The UUID of the node to delete")
    .build()
}

fn tool_set_corner_radius() -> ToolDefinition {
    ToolBuilder::new(
        "set_corner_radius",
        "Set the corner radius of a Frame node. Provide four values for each corner (top-left, top-right, bottom-right, bottom-left), or a single value to set all corners equally.",
    )
    .string_param("node_id", "The UUID of the Frame node to modify")
    .number_param("radius", "Corner radius in logical pixels (sets all four corners)")
    .optional_number_param("top_left", "Top-left corner radius (overrides radius)")
    .optional_number_param("top_right", "Top-right corner radius (overrides radius)")
    .optional_number_param("bottom_right", "Bottom-right corner radius (overrides radius)")
    .optional_number_param("bottom_left", "Bottom-left corner radius (overrides radius)")
    .build()
}

fn tool_set_font_family() -> ToolDefinition {
    ToolBuilder::new("set_font_family", "Set the font family of a Text node.")
        .string_param("node_id", "The UUID of the Text node to modify")
        .string_param(
            "font_family",
            "Font family name (e.g. 'Inter', 'Roboto', 'Arial')",
        )
        .build()
}

fn tool_set_font_weight() -> ToolDefinition {
    ToolBuilder::new(
        "set_font_weight",
        "Set the font weight of a Text node. Common values: 100 (Thin), 300 (Light), 400 (Regular), 500 (Medium), 600 (SemiBold), 700 (Bold), 800 (ExtraBold), 900 (Black).",
    )
    .string_param("node_id", "The UUID of the Text node to modify")
    .number_param("font_weight", "Font weight from 100 to 900")
    .build()
}

fn tool_set_text_align() -> ToolDefinition {
    ToolBuilder::new("set_text_align", "Set the text alignment of a Text node.")
        .string_param("node_id", "The UUID of the Text node to modify")
        .enum_param(
            "text_align",
            "Text alignment",
            &["Left", "Center", "Right", "Justify"],
        )
        .build()
}

fn tool_set_line_height() -> ToolDefinition {
    ToolBuilder::new(
        "set_line_height",
        "Set the line height multiplier of a Text node. 1.0 means single-spaced, 1.5 is one-and-a-half spacing, 2.0 is double-spaced.",
    )
    .string_param("node_id", "The UUID of the Text node to modify")
    .number_param("line_height", "Line height multiplier (e.g. 1.2)")
    .build()
}

fn tool_set_text_color() -> ToolDefinition {
    ToolBuilder::new(
        "set_text_color",
        "Set the text color of a Text node. This is separate from the node's fill color and only affects text rendering. Omit color params to clear (falls back to fill).",
    )
    .string_param("node_id", "The UUID of the Text node to modify")
    .optional_number_param("r", "Red component (0.0-1.0). Omit all color params to clear.")
    .optional_number_param("g", "Green component (0.0-1.0)")
    .optional_number_param("b", "Blue component (0.0-1.0)")
    .optional_number_param("a", "Alpha component (0.0-1.0), defaults to 1.0")
    .build()
}

fn tool_add_child_node() -> ToolDefinition {
    ToolBuilder::new(
        "add_child_node",
        "Create a new node as a child of an existing node. Returns the new node's ID.",
    )
    .string_param("parent_id", "The UUID of the parent node")
    .string_param("name", "Human-readable name for the new node")
    .enum_param(
        "kind",
        "The visual type of the node",
        &["Frame", "Text", "Group"],
    )
    .number_param("x", "Left edge x coordinate")
    .number_param("y", "Top edge y coordinate")
    .number_param("width", "Width in logical pixels")
    .number_param("height", "Height in logical pixels")
    .optional_number_param("fill_r", "Fill red component (0.0-1.0)")
    .optional_number_param("fill_g", "Fill green component (0.0-1.0)")
    .optional_number_param("fill_b", "Fill blue component (0.0-1.0)")
    .optional_number_param("fill_a", "Fill alpha component (0.0-1.0)")
    .optional_string_param("text_content", "Text content (required if kind is Text)")
    .optional_number_param("font_size", "Font size (for Text nodes, default 16.0)")
    .optional_number_param(
        "corner_radius",
        "Corner radius for Frame nodes (default 0.0)",
    )
    .build()
}

fn tool_reparent_node() -> ToolDefinition {
    ToolBuilder::new(
        "reparent_node",
        "Move a node to be a child of a different parent node.",
    )
    .string_param("node_id", "The UUID of the node to move")
    .string_param("new_parent_id", "The UUID of the new parent node")
    .build()
}

fn tool_reorder_children() -> ToolDefinition {
    ToolBuilder::new(
        "reorder_children",
        "Reorder the children of a node. Provide the full list of child IDs in the desired order.",
    )
    .string_param(
        "parent_id",
        "The UUID of the parent node whose children to reorder",
    )
    .array_param(
        "new_order",
        "Array of child node UUIDs in the desired order",
    )
    .build()
}

// --- Argument mapping ---

fn parse_node_id(args: &serde_json::Value, field: &str) -> Result<NodeId, ToolCallError> {
    let id_str = args
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ToolCallError::InvalidArgs(format!("missing {field}")))?;
    let uuid = uuid::Uuid::parse_str(id_str)
        .map_err(|_| ToolCallError::InvalidNodeId(id_str.to_string()))?;
    Ok(NodeId::from_uuid(uuid))
}

#[allow(clippy::cast_possible_truncation)]
fn get_f32(args: &serde_json::Value, field: &str) -> Result<f32, ToolCallError> {
    args.get(field)
        .and_then(serde_json::Value::as_f64)
        .map(|v| v as f32)
        .ok_or_else(|| ToolCallError::InvalidArgs(format!("missing or invalid {field}")))
}

#[allow(clippy::cast_possible_truncation)]
fn get_optional_f32(args: &serde_json::Value, field: &str) -> Option<f32> {
    args.get(field)
        .and_then(serde_json::Value::as_f64)
        .map(|v| v as f32)
}

fn get_string(args: &serde_json::Value, field: &str) -> Result<String, ToolCallError> {
    args.get(field)
        .and_then(serde_json::Value::as_str)
        .map(String::from)
        .ok_or_else(|| ToolCallError::InvalidArgs(format!("missing {field}")))
}

fn color_field(prefix: &str, channel: &str) -> String {
    if prefix.is_empty() {
        channel.to_string()
    } else {
        format!("{prefix}_{channel}")
    }
}

fn parse_optional_color(args: &serde_json::Value, prefix: &str) -> Option<Color> {
    let r = get_optional_f32(args, &color_field(prefix, "r"))?;
    let g = get_optional_f32(args, &color_field(prefix, "g")).unwrap_or(0.0);
    let b = get_optional_f32(args, &color_field(prefix, "b")).unwrap_or(0.0);
    let a = get_optional_f32(args, &color_field(prefix, "a")).unwrap_or(1.0);
    Some(Color::new(r, g, b, a))
}

fn map_set_fill(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let fill = parse_optional_color(args, "");
    Ok(vec![CommandDescriptor::SetFill { node_id, fill }])
}

fn map_set_bounds(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let x = get_f32(args, "x")?;
    let y = get_f32(args, "y")?;
    let width = get_f32(args, "width")?;
    let height = get_f32(args, "height")?;
    Ok(vec![CommandDescriptor::SetBounds {
        node_id,
        bounds: BoundingBox::new(x, y, width, height),
    }])
}

fn map_set_text(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let content = get_string(args, "content")?;
    let mut cmds = vec![CommandDescriptor::SetTextContent { node_id, content }];
    if let Some(font_size) = get_optional_f32(args, "font_size") {
        cmds.push(CommandDescriptor::SetFontSize { node_id, font_size });
    }
    Ok(cmds)
}

fn map_set_opacity(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let opacity = get_f32(args, "opacity")?;
    Ok(vec![CommandDescriptor::SetOpacity { node_id, opacity }])
}

fn map_set_visible(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let visible = args
        .get("visible")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| ToolCallError::InvalidArgs("missing visible".to_string()))?;
    Ok(vec![CommandDescriptor::SetVisible { node_id, visible }])
}

fn map_set_name(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let name = get_string(args, "name")?;
    Ok(vec![CommandDescriptor::SetName { node_id, name }])
}

fn map_set_stroke(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let stroke = parse_optional_color(args, "");
    let mut cmds = vec![CommandDescriptor::SetStroke { node_id, stroke }];
    if let Some(width) = get_optional_f32(args, "width") {
        cmds.push(CommandDescriptor::SetStrokeWidth { node_id, width });
    }
    Ok(cmds)
}

fn map_set_blend_mode(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let mode_str = get_string(args, "blend_mode")?;
    let blend_mode: BlendMode = mode_str.parse().map_err(ToolCallError::InvalidArgs)?;
    Ok(vec![CommandDescriptor::SetBlendMode {
        node_id,
        blend_mode,
    }])
}

fn map_create_node(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let name = get_string(args, "name")?;
    let kind_str = get_string(args, "kind")?;
    let x = get_f32(args, "x")?;
    let y = get_f32(args, "y")?;
    let width = get_f32(args, "width")?;
    let height = get_f32(args, "height")?;

    let corner_radius = get_optional_f32(args, "corner_radius").unwrap_or(0.0);

    let kind = match kind_str.as_str() {
        "Frame" => SceneNodeKind::Frame {
            corner_radius: [corner_radius; 4],
        },
        "Text" => {
            let content = get_string(args, "text_content").unwrap_or_else(|_| String::new());
            let font_size = get_optional_f32(args, "font_size").unwrap_or(16.0);
            SceneNodeKind::Text {
                content,
                font_size,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            }
        }
        "Group" => SceneNodeKind::Group,
        _ => {
            return Err(ToolCallError::InvalidArgs(format!(
                "unknown node kind: {kind_str}"
            )));
        }
    };

    let mut node = SceneNode::new(
        NodeId::new(),
        name,
        kind,
        BoundingBox::new(x, y, width, height),
    );
    node.fill = parse_optional_color(args, "fill");

    Ok(vec![CommandDescriptor::AddRoot { node }])
}

fn map_delete_node(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    Ok(vec![CommandDescriptor::RemoveNode { node_id }])
}

fn map_set_corner_radius(
    args: &serde_json::Value,
) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let base = get_f32(args, "radius")?;
    let tl = get_optional_f32(args, "top_left").unwrap_or(base);
    let tr = get_optional_f32(args, "top_right").unwrap_or(base);
    let br = get_optional_f32(args, "bottom_right").unwrap_or(base);
    let bl = get_optional_f32(args, "bottom_left").unwrap_or(base);
    Ok(vec![CommandDescriptor::SetCornerRadius {
        node_id,
        corner_radius: [tl, tr, br, bl],
    }])
}

fn map_set_font_family(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let font_family = get_string(args, "font_family")?;
    Ok(vec![CommandDescriptor::SetFontFamily {
        node_id,
        font_family,
    }])
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn map_set_font_weight(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let weight = get_f32(args, "font_weight")?;
    Ok(vec![CommandDescriptor::SetFontWeight {
        node_id,
        font_weight: weight as u16,
    }])
}

fn map_set_text_align(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let align_str = get_string(args, "text_align")?;
    let text_align: TextAlign = align_str.parse().map_err(ToolCallError::InvalidArgs)?;
    Ok(vec![CommandDescriptor::SetTextAlign {
        node_id,
        text_align,
    }])
}

fn map_set_line_height(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let line_height = get_f32(args, "line_height")?;
    Ok(vec![CommandDescriptor::SetLineHeight {
        node_id,
        line_height,
    }])
}

fn map_set_text_color(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let text_color = parse_optional_color(args, "");
    Ok(vec![CommandDescriptor::SetTextColor {
        node_id,
        text_color,
    }])
}

fn map_add_child_node(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let parent_id = parse_node_id(args, "parent_id")?;
    let name = get_string(args, "name")?;
    let kind_str = get_string(args, "kind")?;
    let x = get_f32(args, "x")?;
    let y = get_f32(args, "y")?;
    let width = get_f32(args, "width")?;
    let height = get_f32(args, "height")?;

    let corner_radius = get_optional_f32(args, "corner_radius").unwrap_or(0.0);

    let kind = match kind_str.as_str() {
        "Frame" => SceneNodeKind::Frame {
            corner_radius: [corner_radius; 4],
        },
        "Text" => {
            let content = get_string(args, "text_content").unwrap_or_else(|_| String::new());
            let font_size = get_optional_f32(args, "font_size").unwrap_or(16.0);
            SceneNodeKind::Text {
                content,
                font_size,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            }
        }
        "Group" => SceneNodeKind::Group,
        _ => {
            return Err(ToolCallError::InvalidArgs(format!(
                "unknown node kind: {kind_str}"
            )));
        }
    };

    let mut node = SceneNode::new(
        NodeId::new(),
        name,
        kind,
        BoundingBox::new(x, y, width, height),
    );
    node.fill = parse_optional_color(args, "fill");

    Ok(vec![CommandDescriptor::AddChild { parent_id, node }])
}

fn map_reparent_node(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let new_parent_id = parse_node_id(args, "new_parent_id")?;
    Ok(vec![CommandDescriptor::Reparent {
        node_id,
        new_parent_id,
    }])
}

fn map_reorder_children(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let parent_id = parse_node_id(args, "parent_id")?;
    let order_arr = args
        .get("new_order")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ToolCallError::InvalidArgs("missing new_order array".to_string()))?;

    let mut new_order = Vec::with_capacity(order_arr.len());
    for item in order_arr {
        let id_str = item.as_str().ok_or_else(|| {
            ToolCallError::InvalidArgs("new_order items must be strings".to_string())
        })?;
        let uuid = uuid::Uuid::parse_str(id_str)
            .map_err(|_| ToolCallError::InvalidNodeId(id_str.to_string()))?;
        new_order.push(NodeId::from_uuid(uuid));
    }

    Ok(vec![CommandDescriptor::ReorderChildren {
        parent_id,
        new_order,
    }])
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn all_tools_returns_expected_count() {
        let tools = all_tools();
        assert_eq!(tools.len(), 22);
    }

    #[test]
    fn all_tool_names_are_unique() {
        let tools = all_tools();
        let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate tool names found");
    }

    #[test]
    fn all_tools_have_descriptions() {
        for tool in all_tools() {
            assert!(
                !tool.description.is_empty(),
                "tool {} missing description",
                tool.name
            );
        }
    }

    #[test]
    fn all_tools_schema_type_is_object() {
        for tool in all_tools() {
            assert_eq!(
                tool.input_schema.schema_type, "object",
                "tool {} has wrong schema type",
                tool.name
            );
        }
    }

    #[test]
    fn all_tools_serialize_to_valid_json() {
        for tool in all_tools() {
            let json = serde_json::to_string(&tool);
            assert!(
                json.is_ok(),
                "tool {} failed to serialize: {:?}",
                tool.name,
                json.err()
            );
        }
    }

    #[test]
    fn map_unknown_tool_returns_error() {
        let result = map_tool_call("nonexistent", &serde_json::json!({}));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolCallError::UnknownTool(_)));
    }

    #[test]
    fn is_read_only_identifies_query_tools() {
        assert!(is_read_only_tool("get_scene_summary"));
        assert!(is_read_only_tool("get_node"));
        assert!(is_read_only_tool("query_nodes"));
        assert!(!is_read_only_tool("set_fill"));
        assert!(!is_read_only_tool("create_node"));
        assert!(!is_read_only_tool("nonexistent"));
    }

    #[test]
    fn map_read_tools_return_empty_vec() {
        for name in &["get_scene_summary", "get_node", "query_nodes"] {
            let result = map_tool_call(name, &serde_json::json!({}));
            assert!(result.is_ok());
            assert!(result.unwrap().is_empty());
        }
    }

    fn test_node_id() -> NodeId {
        NodeId::new()
    }

    #[test]
    fn map_set_fill_with_color() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "r": 1.0,
            "g": 0.5,
            "b": 0.0,
            "a": 0.8
        });
        let cmds = map_tool_call("set_fill", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetFill { node_id, fill } => {
                assert_eq!(*node_id, id);
                let c = fill.unwrap();
                assert!((c.r - 1.0).abs() < f32::EPSILON);
                assert!((c.g - 0.5).abs() < f32::EPSILON);
                assert!((c.a - 0.8).abs() < f32::EPSILON);
            }
            _ => panic!("expected SetFill"),
        }
    }

    #[test]
    fn map_set_fill_clear() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string() });
        let cmds = map_tool_call("set_fill", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetFill { fill, .. } => assert!(fill.is_none()),
            _ => panic!("expected SetFill"),
        }
    }

    #[test]
    fn map_set_bounds() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "x": 10.0, "y": 20.0, "width": 300.0, "height": 200.0
        });
        let cmds = map_tool_call("set_bounds", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetBounds { bounds, .. } => {
                assert!((bounds.x - 10.0).abs() < f32::EPSILON);
                assert!((bounds.width - 300.0).abs() < f32::EPSILON);
            }
            _ => panic!("expected SetBounds"),
        }
    }

    #[test]
    fn map_set_text_content_only() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "content": "Hello"
        });
        let cmds = map_tool_call("set_text", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetTextContent { content, .. } => {
                assert_eq!(content, "Hello");
            }
            _ => panic!("expected SetTextContent"),
        }
    }

    #[test]
    fn map_set_text_with_font_size() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "content": "Big Text",
            "font_size": 48.0
        });
        let cmds = map_tool_call("set_text", &args).unwrap();
        assert_eq!(cmds.len(), 2);
        assert!(matches!(&cmds[0], CommandDescriptor::SetTextContent { .. }));
        match &cmds[1] {
            CommandDescriptor::SetFontSize { font_size, .. } => {
                assert!((font_size - 48.0).abs() < f32::EPSILON);
            }
            _ => panic!("expected SetFontSize"),
        }
    }

    #[test]
    fn map_set_opacity() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string(), "opacity": 0.5 });
        let cmds = map_tool_call("set_opacity", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetOpacity { opacity, .. } => {
                assert!((opacity - 0.5).abs() < f32::EPSILON);
            }
            _ => panic!("expected SetOpacity"),
        }
    }

    #[test]
    fn map_set_visible() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string(), "visible": false });
        let cmds = map_tool_call("set_visible", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetVisible { visible, .. } => assert!(!visible),
            _ => panic!("expected SetVisible"),
        }
    }

    #[test]
    fn map_set_name() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string(), "name": "New Name" });
        let cmds = map_tool_call("set_name", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetName { name, .. } => assert_eq!(name, "New Name"),
            _ => panic!("expected SetName"),
        }
    }

    #[test]
    fn map_set_stroke_with_color_and_width() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0,
            "width": 2.0
        });
        let cmds = map_tool_call("set_stroke", &args).unwrap();
        assert_eq!(cmds.len(), 2);
        assert!(matches!(&cmds[0], CommandDescriptor::SetStroke { .. }));
        match &cmds[1] {
            CommandDescriptor::SetStrokeWidth { width, .. } => {
                assert!((width - 2.0).abs() < f32::EPSILON);
            }
            _ => panic!("expected SetStrokeWidth"),
        }
    }

    #[test]
    fn map_set_blend_mode_valid() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string(), "blend_mode": "Multiply" });
        let cmds = map_tool_call("set_blend_mode", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetBlendMode { blend_mode, .. } => {
                assert_eq!(*blend_mode, BlendMode::Multiply);
            }
            _ => panic!("expected SetBlendMode"),
        }
    }

    #[test]
    fn map_set_blend_mode_invalid() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string(), "blend_mode": "NotAMode" });
        let result = map_tool_call("set_blend_mode", &args);
        assert!(result.is_err());
    }

    #[test]
    fn map_create_frame_node() {
        let args = serde_json::json!({
            "name": "New Frame",
            "kind": "Frame",
            "x": 50.0, "y": 50.0, "width": 200.0, "height": 100.0,
            "fill_r": 0.2, "fill_g": 0.4, "fill_b": 0.8, "fill_a": 1.0,
            "corner_radius": 8.0
        });
        let cmds = map_tool_call("create_node", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::AddRoot { node } => {
                assert_eq!(node.name, "New Frame");
                assert!(node.fill.is_some());
                match &node.kind {
                    SceneNodeKind::Frame { corner_radius } => {
                        assert_eq!(*corner_radius, [8.0; 4]);
                    }
                    _ => panic!("expected Frame kind"),
                }
            }
            _ => panic!("expected AddRoot"),
        }
    }

    #[test]
    fn map_create_text_node() {
        let args = serde_json::json!({
            "name": "Title",
            "kind": "Text",
            "x": 0.0, "y": 0.0, "width": 400.0, "height": 50.0,
            "text_content": "Hello World",
            "font_size": 32.0
        });
        let cmds = map_tool_call("create_node", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::AddRoot { node } => match &node.kind {
                SceneNodeKind::Text {
                    content, font_size, ..
                } => {
                    assert_eq!(content, "Hello World");
                    assert!((font_size - 32.0).abs() < f32::EPSILON);
                }
                _ => panic!("expected Text kind"),
            },
            _ => panic!("expected AddRoot"),
        }
    }

    #[test]
    fn map_create_node_invalid_kind() {
        let args = serde_json::json!({
            "name": "Bad",
            "kind": "Polygon",
            "x": 0.0, "y": 0.0, "width": 50.0, "height": 50.0
        });
        let result = map_tool_call("create_node", &args);
        assert!(result.is_err());
    }

    #[test]
    fn map_delete_node() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string() });
        let cmds = map_tool_call("delete_node", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::RemoveNode { node_id } => assert_eq!(*node_id, id),
            _ => panic!("expected RemoveNode"),
        }
    }

    #[test]
    fn map_set_fill_invalid_node_id() {
        let args = serde_json::json!({ "node_id": "not-a-uuid", "r": 1.0 });
        let result = map_tool_call("set_fill", &args);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ToolCallError::InvalidNodeId(_)
        ));
    }

    #[test]
    fn map_set_fill_missing_node_id() {
        let args = serde_json::json!({ "r": 1.0 });
        let result = map_tool_call("set_fill", &args);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolCallError::InvalidArgs(_)));
    }

    #[test]
    fn map_set_bounds_missing_field() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string(), "x": 10.0 });
        let result = map_tool_call("set_bounds", &args);
        assert!(result.is_err());
    }

    #[test]
    fn map_set_corner_radius_uniform() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "radius": 12.0
        });
        let cmds = map_tool_call("set_corner_radius", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetCornerRadius { corner_radius, .. } => {
                assert_eq!(*corner_radius, [12.0, 12.0, 12.0, 12.0]);
            }
            _ => panic!("expected SetCornerRadius"),
        }
    }

    #[test]
    fn map_set_corner_radius_individual() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "radius": 0.0,
            "top_left": 4.0,
            "top_right": 8.0,
            "bottom_right": 12.0,
            "bottom_left": 16.0
        });
        let cmds = map_tool_call("set_corner_radius", &args).unwrap();
        match &cmds[0] {
            CommandDescriptor::SetCornerRadius { corner_radius, .. } => {
                assert_eq!(*corner_radius, [4.0, 8.0, 12.0, 16.0]);
            }
            _ => panic!("expected SetCornerRadius"),
        }
    }

    #[test]
    fn map_set_font_family() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "font_family": "Roboto"
        });
        let cmds = map_tool_call("set_font_family", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetFontFamily { font_family, .. } => {
                assert_eq!(font_family, "Roboto");
            }
            _ => panic!("expected SetFontFamily"),
        }
    }

    #[test]
    fn map_set_font_weight() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "font_weight": 700.0
        });
        let cmds = map_tool_call("set_font_weight", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetFontWeight { font_weight, .. } => {
                assert_eq!(*font_weight, 700);
            }
            _ => panic!("expected SetFontWeight"),
        }
    }

    #[test]
    fn map_set_text_align_valid() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "text_align": "Center"
        });
        let cmds = map_tool_call("set_text_align", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetTextAlign { text_align, .. } => {
                assert_eq!(*text_align, TextAlign::Center);
            }
            _ => panic!("expected SetTextAlign"),
        }
    }

    #[test]
    fn map_set_text_align_invalid() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "text_align": "Invalid"
        });
        let result = map_tool_call("set_text_align", &args);
        assert!(result.is_err());
    }

    #[test]
    fn map_set_line_height() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "line_height": 1.5
        });
        let cmds = map_tool_call("set_line_height", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetLineHeight { line_height, .. } => {
                assert!((line_height - 1.5).abs() < f32::EPSILON);
            }
            _ => panic!("expected SetLineHeight"),
        }
    }

    #[test]
    fn map_set_text_color_with_color() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0
        });
        let cmds = map_tool_call("set_text_color", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetTextColor { text_color, .. } => {
                assert!(text_color.is_some());
            }
            _ => panic!("expected SetTextColor"),
        }
    }

    #[test]
    fn map_set_text_color_clear() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string() });
        let cmds = map_tool_call("set_text_color", &args).unwrap();
        match &cmds[0] {
            CommandDescriptor::SetTextColor { text_color, .. } => {
                assert!(text_color.is_none());
            }
            _ => panic!("expected SetTextColor"),
        }
    }

    #[test]
    fn map_add_child_node() {
        let parent_id = test_node_id();
        let args = serde_json::json!({
            "parent_id": parent_id.to_string(),
            "name": "Child Frame",
            "kind": "Frame",
            "x": 10.0, "y": 10.0, "width": 50.0, "height": 50.0
        });
        let cmds = map_tool_call("add_child_node", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::AddChild {
                parent_id: pid,
                node,
            } => {
                assert_eq!(*pid, parent_id);
                assert_eq!(node.name, "Child Frame");
            }
            _ => panic!("expected AddChild"),
        }
    }

    #[test]
    fn map_reparent_node() {
        let node_id = test_node_id();
        let new_parent_id = test_node_id();
        let args = serde_json::json!({
            "node_id": node_id.to_string(),
            "new_parent_id": new_parent_id.to_string()
        });
        let cmds = map_tool_call("reparent_node", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::Reparent {
                node_id: actual_node,
                new_parent_id: actual_parent,
            } => {
                assert_eq!(*actual_node, node_id);
                assert_eq!(*actual_parent, new_parent_id);
            }
            _ => panic!("expected Reparent"),
        }
    }

    #[test]
    fn map_reorder_children() {
        let parent_id = test_node_id();
        let child1 = test_node_id();
        let child2 = test_node_id();
        let args = serde_json::json!({
            "parent_id": parent_id.to_string(),
            "new_order": [child2.to_string(), child1.to_string()]
        });
        let cmds = map_tool_call("reorder_children", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::ReorderChildren {
                parent_id: pid,
                new_order,
            } => {
                assert_eq!(*pid, parent_id);
                assert_eq!(new_order.len(), 2);
                assert_eq!(new_order[0], child2);
                assert_eq!(new_order[1], child1);
            }
            _ => panic!("expected ReorderChildren"),
        }
    }

    #[test]
    fn map_reorder_children_invalid_ids() {
        let parent_id = test_node_id();
        let args = serde_json::json!({
            "parent_id": parent_id.to_string(),
            "new_order": ["not-a-uuid"]
        });
        let result = map_tool_call("reorder_children", &args);
        assert!(result.is_err());
    }
}
