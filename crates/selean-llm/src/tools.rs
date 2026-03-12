//! Tool definitions and execution mapping.
//!
//! [`all_tools`] returns the complete set of tool schemas for the Claude API.
//! [`map_tool_call`] converts a tool name + JSON args into `CommandDescriptor`
//! values ready for execution through the shared mutation pipeline.

use selean_common::types::NodeId;
use selean_engine::command::CommandDescriptor;
use selean_engine::scene::{
    BlendMode, BoundingBox, ClipMode, Color, Effect, FontStyle, Gradient, GradientStop, SceneNode,
    SceneNodeKind, TextAlign,
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

/// Type alias for tool mapper functions that convert JSON args to command descriptors.
type ToolMapper = fn(&serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError>;

/// A unified tool entry combining definition, category, and optional mapper.
///
/// Adding a new tool requires a single entry in [`all_entries`]. The category
/// and mapper are co-located with the definition, eliminating the need to
/// update separate match statements.
pub struct ToolEntry {
    /// The tool definition sent to the Claude API.
    pub definition: ToolDefinition,
    /// Dispatch category for the WASM bridge.
    pub category: ToolCategory,
    /// For `Mutation` tools, the function that converts args to `CommandDescriptor` values.
    /// `None` for tools whose execution is handled directly by the WASM bridge
    /// (ReadOnly, Page, Align, Rotation, Group, ZOrder).
    pub mapper: Option<ToolMapper>,
}

/// Returns the unified tool registry. Each entry defines a tool's schema,
/// category, and optional arg-to-command mapper in a single location.
#[must_use]
pub fn all_entries() -> Vec<ToolEntry> {
    vec![
        // --- Read-only tools ---
        ToolEntry {
            definition: tool_get_scene_summary(),
            category: ToolCategory::ReadOnly,
            mapper: None,
        },
        ToolEntry {
            definition: tool_get_node(),
            category: ToolCategory::ReadOnly,
            mapper: None,
        },
        ToolEntry {
            definition: tool_query_nodes(),
            category: ToolCategory::ReadOnly,
            mapper: None,
        },
        ToolEntry {
            definition: tool_get_pages(),
            category: ToolCategory::ReadOnly,
            mapper: None,
        },
        // --- Page tools ---
        ToolEntry {
            definition: tool_add_page(),
            category: ToolCategory::Page,
            mapper: None,
        },
        ToolEntry {
            definition: tool_remove_page(),
            category: ToolCategory::Page,
            mapper: None,
        },
        ToolEntry {
            definition: tool_set_active_page(),
            category: ToolCategory::Page,
            mapper: None,
        },
        // --- Special-category tools ---
        ToolEntry {
            definition: tool_align_nodes(),
            category: ToolCategory::Align,
            mapper: None,
        },
        ToolEntry {
            definition: tool_set_rotation(),
            category: ToolCategory::Rotation,
            mapper: None,
        },
        ToolEntry {
            definition: tool_group_nodes(),
            category: ToolCategory::Group,
            mapper: None,
        },
        ToolEntry {
            definition: tool_ungroup_node(),
            category: ToolCategory::Group,
            mapper: None,
        },
        ToolEntry {
            definition: tool_move_to_front(),
            category: ToolCategory::ZOrder,
            mapper: None,
        },
        ToolEntry {
            definition: tool_move_to_back(),
            category: ToolCategory::ZOrder,
            mapper: None,
        },
        ToolEntry {
            definition: tool_move_forward(),
            category: ToolCategory::ZOrder,
            mapper: None,
        },
        ToolEntry {
            definition: tool_move_backward(),
            category: ToolCategory::ZOrder,
            mapper: None,
        },
        // --- Mutation tools (with mappers) ---
        ToolEntry {
            definition: tool_set_fill(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_fill),
        },
        ToolEntry {
            definition: tool_set_bounds(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_bounds),
        },
        ToolEntry {
            definition: tool_set_text(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_text),
        },
        ToolEntry {
            definition: tool_set_opacity(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_opacity),
        },
        ToolEntry {
            definition: tool_set_visible(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_visible),
        },
        ToolEntry {
            definition: tool_set_name(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_name),
        },
        ToolEntry {
            definition: tool_set_stroke(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_stroke),
        },
        ToolEntry {
            definition: tool_set_blend_mode(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_blend_mode),
        },
        ToolEntry {
            definition: tool_create_node(),
            category: ToolCategory::Mutation,
            mapper: Some(map_create_node),
        },
        ToolEntry {
            definition: tool_delete_node(),
            category: ToolCategory::Mutation,
            mapper: Some(map_delete_node),
        },
        ToolEntry {
            definition: tool_set_corner_radius(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_corner_radius),
        },
        ToolEntry {
            definition: tool_set_font_family(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_font_family),
        },
        ToolEntry {
            definition: tool_set_font_weight(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_font_weight),
        },
        ToolEntry {
            definition: tool_set_text_align(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_text_align),
        },
        ToolEntry {
            definition: tool_set_line_height(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_line_height),
        },
        ToolEntry {
            definition: tool_set_text_color(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_text_color),
        },
        ToolEntry {
            definition: tool_add_child_node(),
            category: ToolCategory::Mutation,
            mapper: Some(map_add_child_node),
        },
        ToolEntry {
            definition: tool_reparent_node(),
            category: ToolCategory::Mutation,
            mapper: Some(map_reparent_node),
        },
        ToolEntry {
            definition: tool_reorder_children(),
            category: ToolCategory::Mutation,
            mapper: Some(map_reorder_children),
        },
        ToolEntry {
            definition: tool_set_font_style(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_font_style),
        },
        ToolEntry {
            definition: tool_set_clip_mode(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_clip_mode),
        },
        ToolEntry {
            definition: tool_set_linear_gradient(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_linear_gradient),
        },
        ToolEntry {
            definition: tool_set_radial_gradient(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_radial_gradient),
        },
        ToolEntry {
            definition: tool_set_drop_shadow(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_drop_shadow),
        },
        ToolEntry {
            definition: tool_set_blur(),
            category: ToolCategory::Mutation,
            mapper: Some(map_set_blur),
        },
        ToolEntry {
            definition: tool_remove_effects(),
            category: ToolCategory::Mutation,
            mapper: Some(map_remove_effects),
        },
    ]
}

/// Returns all tool definitions for the Claude API.
#[must_use]
pub fn all_tools() -> Vec<ToolDefinition> {
    all_entries().into_iter().map(|e| e.definition).collect()
}

/// Categorizes how a tool call should be dispatched by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    /// Read-only query. Does not mutate the scene.
    ReadOnly,
    /// Page-level mutation (add/remove/switch page). Operates on `Document` directly.
    Page,
    /// Alignment tool. Requires direct scene access.
    Align,
    /// Rotation tool. Requires scene access for center computation.
    Rotation,
    /// Grouping tool (group/ungroup). Requires scene access for hierarchy.
    Group,
    /// Z-order tool (move forward/backward/front/back). Requires scene access.
    ZOrder,
    /// Standard mutation. Produces `CommandDescriptor` values.
    Mutation,
}

/// Returns the dispatch category for the given tool name.
///
/// Derived from the unified [`all_entries`] registry. Unknown tools default
/// to `Mutation` for backward compatibility.
#[must_use]
pub fn tool_category(name: &str) -> ToolCategory {
    all_entries()
        .iter()
        .find(|e| e.definition.name == name)
        .map(|e| e.category)
        .unwrap_or(ToolCategory::Mutation)
}

/// Returns `true` if the given tool name is read-only.
#[must_use]
pub fn is_read_only_tool(name: &str) -> bool {
    tool_category(name) == ToolCategory::ReadOnly
}

/// Returns `true` if the given tool name is a page-level mutation.
#[must_use]
pub fn is_page_tool(name: &str) -> bool {
    tool_category(name) == ToolCategory::Page
}

/// Returns `true` if the given tool name is the alignment tool.
#[must_use]
pub fn is_align_tool(name: &str) -> bool {
    tool_category(name) == ToolCategory::Align
}

/// Returns `true` if the given tool name is the rotation tool.
#[must_use]
pub fn is_rotation_tool(name: &str) -> bool {
    tool_category(name) == ToolCategory::Rotation
}

/// Returns `true` if the given tool name is a grouping tool.
#[must_use]
pub fn is_group_tool(name: &str) -> bool {
    tool_category(name) == ToolCategory::Group
}

/// Returns `true` if the given tool name is a z-order tool.
#[must_use]
pub fn is_z_order_tool(name: &str) -> bool {
    tool_category(name) == ToolCategory::ZOrder
}

/// Maps a tool call (name + JSON args) to a list of `CommandDescriptor` values.
///
/// Derived from the unified [`all_entries`] registry. Read-only and
/// special-category tools (whose mapper is `None`) return an empty vec;
/// the caller handles them directly.
///
/// # Errors
///
/// Returns `ToolCallError` if the tool name is unknown or args are invalid.
pub fn map_tool_call(
    name: &str,
    args: &serde_json::Value,
) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let entry = all_entries()
        .into_iter()
        .find(|e| e.definition.name == name);

    let Some(entry) = entry else {
        return Err(ToolCallError::UnknownTool(name.to_string()));
    };

    match entry.mapper {
        Some(mapper) => mapper(args),
        None => Ok(vec![]),
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
        &["Frame", "Text", "Group", "Image", "Vector"],
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
    .optional_string_param("asset_ref", "Asset reference (required if kind is Image)")
    .optional_string_param("path_data", "SVG path data (required if kind is Vector)")
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
        &["Frame", "Text", "Group", "Image", "Vector"],
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
    .optional_string_param("asset_ref", "Asset reference (required if kind is Image)")
    .optional_string_param("path_data", "SVG path data (required if kind is Vector)")
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

/// Gets a required f32 field clamped to `[min, max]`.
fn get_f32_clamped(
    args: &serde_json::Value,
    field: &str,
    min: f32,
    max: f32,
) -> Result<f32, ToolCallError> {
    get_f32(args, field).map(|v| v.clamp(min, max))
}

#[allow(clippy::cast_possible_truncation)]
fn get_optional_f32(args: &serde_json::Value, field: &str) -> Option<f32> {
    args.get(field)
        .and_then(serde_json::Value::as_f64)
        .map(|v| v as f32)
}

/// Gets an optional f32 field clamped to `[min, max]`.
fn get_optional_f32_clamped(
    args: &serde_json::Value,
    field: &str,
    min: f32,
    max: f32,
) -> Option<f32> {
    get_optional_f32(args, field).map(|v| v.clamp(min, max))
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
    let r = get_optional_f32_clamped(args, &color_field(prefix, "r"), 0.0, 1.0)?;
    let g = get_optional_f32_clamped(args, &color_field(prefix, "g"), 0.0, 1.0).unwrap_or(0.0);
    let b = get_optional_f32_clamped(args, &color_field(prefix, "b"), 0.0, 1.0).unwrap_or(0.0);
    let a = get_optional_f32_clamped(args, &color_field(prefix, "a"), 0.0, 1.0).unwrap_or(1.0);
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
    if let Some(font_size) = get_optional_f32_clamped(args, "font_size", 1.0, 1000.0) {
        cmds.push(CommandDescriptor::SetFontSize { node_id, font_size });
    }
    Ok(cmds)
}

fn map_set_opacity(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let opacity = get_f32_clamped(args, "opacity", 0.0, 1.0)?;
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
    if let Some(width) = get_optional_f32_clamped(args, "width", 0.0, f32::MAX) {
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

fn parse_node_kind(
    args: &serde_json::Value,
    kind_str: &str,
) -> Result<SceneNodeKind, ToolCallError> {
    let corner_radius =
        get_optional_f32_clamped(args, "corner_radius", 0.0, f32::MAX).unwrap_or(0.0);
    match kind_str {
        "Frame" => Ok(SceneNodeKind::Frame {
            corner_radius: [corner_radius; 4],
        }),
        "Text" => {
            let content = get_string(args, "text_content").unwrap_or_else(|_| String::new());
            let font_size =
                get_optional_f32_clamped(args, "font_size", 1.0, 1000.0).unwrap_or(16.0);
            Ok(SceneNodeKind::Text {
                content,
                font_size,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            })
        }
        "Image" => {
            let asset_ref = get_string(args, "asset_ref").unwrap_or_default();
            Ok(SceneNodeKind::Image { asset_ref })
        }
        "Vector" => {
            let path_data = get_string(args, "path_data").unwrap_or_default();
            Ok(SceneNodeKind::Vector { path_data })
        }
        "Group" => Ok(SceneNodeKind::Group),
        _ => Err(ToolCallError::InvalidArgs(format!(
            "unknown node kind: {kind_str}"
        ))),
    }
}

/// Builds a `SceneNode` from the common create/add-child arguments.
fn build_node_from_args(args: &serde_json::Value) -> Result<SceneNode, ToolCallError> {
    let name = get_string(args, "name")?;
    let kind_str = get_string(args, "kind")?;
    let x = get_f32(args, "x")?;
    let y = get_f32(args, "y")?;
    let width = get_f32(args, "width")?;
    let height = get_f32(args, "height")?;

    let kind = parse_node_kind(args, &kind_str)?;

    let mut node = SceneNode::new(
        NodeId::new(),
        name,
        kind,
        BoundingBox::new(x, y, width, height),
    );
    node.fill = parse_optional_color(args, "fill");
    Ok(node)
}

fn map_create_node(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node = build_node_from_args(args)?;
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
    let base = get_f32_clamped(args, "radius", 0.0, f32::MAX)?;
    let tl = get_optional_f32_clamped(args, "top_left", 0.0, f32::MAX).unwrap_or(base);
    let tr = get_optional_f32_clamped(args, "top_right", 0.0, f32::MAX).unwrap_or(base);
    let br = get_optional_f32_clamped(args, "bottom_right", 0.0, f32::MAX).unwrap_or(base);
    let bl = get_optional_f32_clamped(args, "bottom_left", 0.0, f32::MAX).unwrap_or(base);
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
    let weight = get_f32_clamped(args, "font_weight", 1.0, 1000.0)?;
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
    let line_height = get_f32_clamped(args, "line_height", 0.1, 10.0)?;
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
    let node = build_node_from_args(args)?;
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

fn tool_set_font_style() -> ToolDefinition {
    ToolBuilder::new("set_font_style", "Set the font style of a Text node.")
        .string_param("node_id", "The UUID of the Text node to modify")
        .enum_param("font_style", "Font style", &["Normal", "Italic"])
        .build()
}

fn tool_set_clip_mode() -> ToolDefinition {
    ToolBuilder::new(
        "set_clip_mode",
        "Set the clip mode of a node, controlling how children are clipped to the node's bounds.",
    )
    .string_param("node_id", "The UUID of the node to modify")
    .enum_param(
        "clip_mode",
        "The clipping mode to apply",
        &["None", "Scissor", "Stencil", "ShaderRect"],
    )
    .build()
}

fn map_set_font_style(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let style_str = get_string(args, "font_style")?;
    let font_style: FontStyle = style_str.parse().map_err(ToolCallError::InvalidArgs)?;
    Ok(vec![CommandDescriptor::SetFontStyle {
        node_id,
        font_style,
    }])
}

fn map_set_clip_mode(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let mode_str = get_string(args, "clip_mode")?;
    let clip_mode = match mode_str.as_str() {
        "None" => ClipMode::None,
        "Scissor" => ClipMode::Scissor,
        "Stencil" => ClipMode::Stencil,
        "ShaderRect" => ClipMode::ShaderRect,
        _ => {
            return Err(ToolCallError::InvalidArgs(format!(
                "unknown clip mode: {mode_str}"
            )));
        }
    };
    Ok(vec![CommandDescriptor::SetClipMode { node_id, clip_mode }])
}

fn tool_align_nodes() -> ToolDefinition {
    ToolBuilder::new(
        "align_nodes",
        "Align or distribute multiple nodes. Requires at least 2 node IDs. Alignment adjusts node positions without changing their sizes.",
    )
    .array_param(
        "node_ids",
        "Array of node UUID strings to align (minimum 2)",
    )
    .enum_param(
        "alignment",
        "The alignment or distribution to apply",
        &[
            "Left",
            "Right",
            "Top",
            "Bottom",
            "CenterH",
            "CenterV",
            "DistributeH",
            "DistributeV",
        ],
    )
    .build()
}

// --- Page management tool definitions ---

fn tool_get_pages() -> ToolDefinition {
    ToolBuilder::new(
        "get_pages",
        "List all pages in the document with their names, dimensions, and node counts. Use this to understand the multi-page structure before navigating or modifying pages.",
    )
    .build()
}

fn tool_add_page() -> ToolDefinition {
    ToolBuilder::new(
        "add_page",
        "Add a new blank page to the document. Returns the new page's ID.",
    )
    .string_param("name", "Name for the new page (e.g. 'Slide 2')")
    .number_param("width", "Page width in pixels (e.g. 1920)")
    .number_param("height", "Page height in pixels (e.g. 1080)")
    .build()
}

fn tool_remove_page() -> ToolDefinition {
    ToolBuilder::new(
        "remove_page",
        "Remove a page from the document by its ID. Cannot remove the last remaining page.",
    )
    .string_param("page_id", "The UUID of the page to remove")
    .build()
}

fn tool_set_active_page() -> ToolDefinition {
    ToolBuilder::new(
        "set_active_page",
        "Switch the active page being edited. All subsequent scene operations will apply to this page.",
    )
    .string_param("page_id", "The UUID of the page to make active")
    .build()
}

fn tool_move_to_front() -> ToolDefinition {
    ToolBuilder::new(
        "move_to_front",
        "Move a node to the front of the layer order (drawn on top of all siblings).",
    )
    .string_param("node_id", "The UUID of the node to move to front")
    .build()
}

fn tool_move_to_back() -> ToolDefinition {
    ToolBuilder::new(
        "move_to_back",
        "Move a node to the back of the layer order (drawn behind all siblings).",
    )
    .string_param("node_id", "The UUID of the node to move to back")
    .build()
}

fn tool_move_forward() -> ToolDefinition {
    ToolBuilder::new(
        "move_forward",
        "Move a node one step forward in the layer order.",
    )
    .string_param("node_id", "The UUID of the node to move forward")
    .build()
}

fn tool_move_backward() -> ToolDefinition {
    ToolBuilder::new(
        "move_backward",
        "Move a node one step backward in the layer order.",
    )
    .string_param("node_id", "The UUID of the node to move backward")
    .build()
}

fn tool_group_nodes() -> ToolDefinition {
    ToolBuilder::new(
        "group_nodes",
        "Group multiple nodes into a single Group node. The group's bounds become the union of all selected nodes. Child order is preserved.",
    )
    .array_param("node_ids", "Array of node ID strings to group together")
    .build()
}

fn tool_ungroup_node() -> ToolDefinition {
    ToolBuilder::new(
        "ungroup_node",
        "Dissolve a Group node, reparenting its children to the group's parent at the group's position in the layer order.",
    )
    .string_param("node_id", "The UUID of the Group node to ungroup")
    .build()
}

fn tool_set_linear_gradient() -> ToolDefinition {
    ToolBuilder::new(
        "set_linear_gradient",
        "Set a linear gradient fill on a node. Specify start and end points as fractions of node bounds (0.0-1.0) and an array of color stops. Each stop has a position (0.0-1.0) and RGBA color. Maximum 4 stops.",
    )
    .string_param("node_id", "The UUID of the node to modify")
    .number_param("start_x", "Gradient start X as fraction of width (0.0-1.0)")
    .number_param("start_y", "Gradient start Y as fraction of height (0.0-1.0)")
    .number_param("end_x", "Gradient end X as fraction of width (0.0-1.0)")
    .number_param("end_y", "Gradient end Y as fraction of height (0.0-1.0)")
    .array_param("stops", "Array of color stops: [{position, r, g, b, a}]")
    .build()
}

fn tool_set_radial_gradient() -> ToolDefinition {
    ToolBuilder::new(
        "set_radial_gradient",
        "Set a radial gradient fill on a node. Specify center point as fractions of node bounds (0.0-1.0), radius (1.0 = half smaller dimension), and an array of color stops. Each stop has a position (0.0-1.0) and RGBA color. Maximum 4 stops.",
    )
    .string_param("node_id", "The UUID of the node to modify")
    .number_param("center_x", "Gradient center X as fraction of width (0.0-1.0)")
    .number_param("center_y", "Gradient center Y as fraction of height (0.0-1.0)")
    .number_param("radius", "Gradient radius (1.0 = half the smaller dimension)")
    .array_param("stops", "Array of color stops: [{position, r, g, b, a}]")
    .build()
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::redundant_closure_for_method_calls
)]
fn parse_gradient_stops(args: &serde_json::Value) -> Result<Vec<GradientStop>, ToolCallError> {
    let stops_val = args
        .get("stops")
        .ok_or_else(|| ToolCallError::InvalidArgs("missing 'stops' array".to_string()))?;
    let stops_arr = stops_val
        .as_array()
        .ok_or_else(|| ToolCallError::InvalidArgs("'stops' must be an array".to_string()))?;

    let mut stops = Vec::with_capacity(stops_arr.len().min(4));
    for (i, stop_val) in stops_arr.iter().take(4).enumerate() {
        let position = stop_val
            .get("position")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ToolCallError::InvalidArgs(format!("stop {i}: missing 'position'")))?
            as f32;
        let r = stop_val
            .get("r")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ToolCallError::InvalidArgs(format!("stop {i}: missing 'r'")))?
            as f32;
        let g = stop_val
            .get("g")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ToolCallError::InvalidArgs(format!("stop {i}: missing 'g'")))?
            as f32;
        let b = stop_val
            .get("b")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ToolCallError::InvalidArgs(format!("stop {i}: missing 'b'")))?
            as f32;
        let a = stop_val.get("a").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;

        stops.push(GradientStop {
            position: position.clamp(0.0, 1.0),
            color: Color::new(
                r.clamp(0.0, 1.0),
                g.clamp(0.0, 1.0),
                b.clamp(0.0, 1.0),
                a.clamp(0.0, 1.0),
            ),
        });
    }
    Ok(stops)
}

fn map_set_linear_gradient(
    args: &serde_json::Value,
) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let start_x = get_f32(args, "start_x")?;
    let start_y = get_f32(args, "start_y")?;
    let end_x = get_f32(args, "end_x")?;
    let end_y = get_f32(args, "end_y")?;
    let stops = parse_gradient_stops(args)?;

    Ok(vec![CommandDescriptor::SetFillGradient {
        node_id,
        fill_gradient: Some(Gradient::Linear {
            start: [start_x, start_y],
            end: [end_x, end_y],
            stops,
        }),
    }])
}

fn map_set_radial_gradient(
    args: &serde_json::Value,
) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let center_x = get_f32(args, "center_x")?;
    let center_y = get_f32(args, "center_y")?;
    let radius = get_f32(args, "radius")?;
    let stops = parse_gradient_stops(args)?;

    Ok(vec![CommandDescriptor::SetFillGradient {
        node_id,
        fill_gradient: Some(Gradient::Radial {
            center: [center_x, center_y],
            radius,
            stops,
        }),
    }])
}

fn tool_set_drop_shadow() -> ToolDefinition {
    ToolBuilder::new(
        "set_drop_shadow",
        "Add or replace a drop shadow effect on a node. Specify shadow color (RGBA), x/y offset in pixels, and blur radius.",
    )
    .string_param("node_id", "The UUID of the node to modify")
    .number_param("offset_x", "Shadow horizontal offset in pixels")
    .number_param("offset_y", "Shadow vertical offset in pixels")
    .number_param("blur_radius", "Shadow blur radius in pixels (0 = sharp)")
    .optional_number_param("r", "Shadow red component (0.0-1.0), defaults to 0.0")
    .optional_number_param("g", "Shadow green component (0.0-1.0), defaults to 0.0")
    .optional_number_param("b", "Shadow blue component (0.0-1.0), defaults to 0.0")
    .optional_number_param("a", "Shadow alpha component (0.0-1.0), defaults to 0.5")
    .build()
}

fn tool_set_blur() -> ToolDefinition {
    ToolBuilder::new(
        "set_blur",
        "Add or replace a blur effect on a node. Specify the blur radius in pixels.",
    )
    .string_param("node_id", "The UUID of the node to modify")
    .number_param("radius", "Blur radius in pixels")
    .build()
}

fn tool_remove_effects() -> ToolDefinition {
    ToolBuilder::new(
        "remove_effects",
        "Remove all effects (drop shadow, blur) from a node.",
    )
    .string_param("node_id", "The UUID of the node to clear effects from")
    .build()
}

#[allow(clippy::cast_possible_truncation)]
fn map_set_drop_shadow(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let offset_x = get_f32(args, "offset_x")?;
    let offset_y = get_f32(args, "offset_y")?;
    let blur_radius = get_f32(args, "blur_radius")?;
    let r = args
        .get("r")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0) as f32;
    let g = args
        .get("g")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0) as f32;
    let b = args
        .get("b")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0) as f32;
    let a = args
        .get("a")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.5) as f32;

    Ok(vec![CommandDescriptor::SetEffects {
        node_id,
        effects: vec![Effect::DropShadow {
            color: Color::new(r, g, b, a),
            offset_x,
            offset_y,
            blur_radius,
        }],
    }])
}

fn map_set_blur(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    let radius = get_f32(args, "radius")?;

    Ok(vec![CommandDescriptor::SetEffects {
        node_id,
        effects: vec![Effect::Blur { radius }],
    }])
}

fn map_remove_effects(args: &serde_json::Value) -> Result<Vec<CommandDescriptor>, ToolCallError> {
    let node_id = parse_node_id(args, "node_id")?;
    Ok(vec![CommandDescriptor::SetEffects {
        node_id,
        effects: vec![],
    }])
}

fn tool_set_rotation() -> ToolDefinition {
    ToolBuilder::new(
        "set_rotation",
        "Set the rotation angle of a node in degrees. Rotates around the node's bounding box center.",
    )
    .string_param("node_id", "The UUID of the node to rotate")
    .number_param("angle_degrees", "Rotation angle in degrees (0-360)")
    .build()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn all_tools_returns_expected_count() {
        let tools = all_tools();
        assert_eq!(tools.len(), 41);
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
        assert!(is_read_only_tool("get_pages"));
        assert!(!is_read_only_tool("set_fill"));
        assert!(!is_read_only_tool("create_node"));
        assert!(!is_read_only_tool("add_page"));
        assert!(!is_read_only_tool("nonexistent"));
    }

    #[test]
    fn is_page_tool_identifies_page_mutations() {
        assert!(is_page_tool("add_page"));
        assert!(is_page_tool("remove_page"));
        assert!(is_page_tool("set_active_page"));
        assert!(!is_page_tool("get_pages"));
        assert!(!is_page_tool("set_fill"));
        assert!(!is_page_tool("nonexistent"));
    }

    #[test]
    fn tool_category_classifies_all_categories() {
        assert_eq!(tool_category("get_scene_summary"), ToolCategory::ReadOnly);
        assert_eq!(tool_category("get_node"), ToolCategory::ReadOnly);
        assert_eq!(tool_category("query_nodes"), ToolCategory::ReadOnly);
        assert_eq!(tool_category("get_pages"), ToolCategory::ReadOnly);
        assert_eq!(tool_category("add_page"), ToolCategory::Page);
        assert_eq!(tool_category("remove_page"), ToolCategory::Page);
        assert_eq!(tool_category("set_active_page"), ToolCategory::Page);
        assert_eq!(tool_category("align_nodes"), ToolCategory::Align);
        assert_eq!(tool_category("set_rotation"), ToolCategory::Rotation);
        assert_eq!(tool_category("group_nodes"), ToolCategory::Group);
        assert_eq!(tool_category("ungroup_node"), ToolCategory::Group);
        assert_eq!(tool_category("move_to_front"), ToolCategory::ZOrder);
        assert_eq!(tool_category("move_to_back"), ToolCategory::ZOrder);
        assert_eq!(tool_category("move_forward"), ToolCategory::ZOrder);
        assert_eq!(tool_category("move_backward"), ToolCategory::ZOrder);
        assert_eq!(tool_category("set_fill"), ToolCategory::Mutation);
        assert_eq!(tool_category("create_node"), ToolCategory::Mutation);
        assert_eq!(tool_category("nonexistent"), ToolCategory::Mutation);
    }

    #[test]
    fn map_read_tools_return_empty_vec() {
        for name in &["get_scene_summary", "get_node", "query_nodes"] {
            let result = map_tool_call(name, &serde_json::json!({}));
            assert!(result.is_ok());
            assert!(result.unwrap().is_empty());
        }
    }

    #[test]
    fn page_tool_definitions_have_correct_params() {
        let tools = all_tools();
        let get_pages = tools.iter().find(|t| t.name == "get_pages").unwrap();
        assert!(get_pages.input_schema.required.is_empty());

        let add_page = tools.iter().find(|t| t.name == "add_page").unwrap();
        assert!(add_page.input_schema.required.contains(&"name".to_string()));
        assert!(
            add_page
                .input_schema
                .required
                .contains(&"width".to_string())
        );
        assert!(
            add_page
                .input_schema
                .required
                .contains(&"height".to_string())
        );

        let remove_page = tools.iter().find(|t| t.name == "remove_page").unwrap();
        assert!(
            remove_page
                .input_schema
                .required
                .contains(&"page_id".to_string())
        );

        let set_active = tools.iter().find(|t| t.name == "set_active_page").unwrap();
        assert!(
            set_active
                .input_schema
                .required
                .contains(&"page_id".to_string())
        );
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

    #[test]
    fn map_create_node_image_kind() {
        let args = serde_json::json!({
            "name": "Photo",
            "kind": "Image",
            "x": 0.0, "y": 0.0, "width": 400.0, "height": 300.0,
            "asset_ref": "img_12345"
        });
        let cmds = map_tool_call("create_node", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::AddRoot { node } => {
                assert_eq!(node.name, "Photo");
                match &node.kind {
                    SceneNodeKind::Image { asset_ref } => {
                        assert_eq!(asset_ref, "img_12345");
                    }
                    _ => panic!("expected Image kind"),
                }
            }
            _ => panic!("expected AddRoot"),
        }
    }

    #[test]
    fn map_create_node_vector_kind() {
        let args = serde_json::json!({
            "name": "Arrow",
            "kind": "Vector",
            "x": 10.0, "y": 10.0, "width": 100.0, "height": 50.0,
            "path_data": "M 0 0 L 100 50"
        });
        let cmds = map_tool_call("create_node", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::AddRoot { node } => match &node.kind {
                SceneNodeKind::Vector { path_data } => {
                    assert_eq!(path_data, "M 0 0 L 100 50");
                }
                _ => panic!("expected Vector kind"),
            },
            _ => panic!("expected AddRoot"),
        }
    }

    #[test]
    fn map_add_child_node_image_kind() {
        let parent_id = test_node_id();
        let args = serde_json::json!({
            "parent_id": parent_id.to_string(),
            "name": "Child Image",
            "kind": "Image",
            "x": 0.0, "y": 0.0, "width": 200.0, "height": 150.0,
            "asset_ref": "img_child"
        });
        let cmds = map_tool_call("add_child_node", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::AddChild {
                parent_id: pid,
                node,
            } => {
                assert_eq!(*pid, parent_id);
                match &node.kind {
                    SceneNodeKind::Image { asset_ref } => {
                        assert_eq!(asset_ref, "img_child");
                    }
                    _ => panic!("expected Image kind"),
                }
            }
            _ => panic!("expected AddChild"),
        }
    }

    #[test]
    fn map_set_font_style_valid() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "font_style": "Italic"
        });
        let cmds = map_tool_call("set_font_style", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetFontStyle { font_style, .. } => {
                assert_eq!(*font_style, FontStyle::Italic);
            }
            _ => panic!("expected SetFontStyle"),
        }
    }

    #[test]
    fn map_set_font_style_invalid() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "font_style": "Bold"
        });
        let result = map_tool_call("set_font_style", &args);
        assert!(result.is_err());
    }

    #[test]
    fn map_set_clip_mode_valid() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "clip_mode": "Scissor"
        });
        let cmds = map_tool_call("set_clip_mode", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetClipMode { clip_mode, .. } => {
                assert_eq!(*clip_mode, ClipMode::Scissor);
            }
            _ => panic!("expected SetClipMode"),
        }
    }

    #[test]
    fn map_set_clip_mode_invalid() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "clip_mode": "InvalidMode"
        });
        let result = map_tool_call("set_clip_mode", &args);
        assert!(result.is_err());
    }

    // --- Gradient tool tests ---

    #[test]
    fn map_set_linear_gradient() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "start_x": 0.0,
            "start_y": 0.0,
            "end_x": 1.0,
            "end_y": 1.0,
            "stops": [
                {"position": 0.0, "r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0},
                {"position": 1.0, "r": 0.0, "g": 0.0, "b": 1.0, "a": 1.0}
            ]
        });
        let cmds = map_tool_call("set_linear_gradient", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetFillGradient {
                node_id,
                fill_gradient,
            } => {
                assert_eq!(*node_id, id);
                let grad = fill_gradient.as_ref().unwrap();
                match grad {
                    Gradient::Linear { start, end, stops } => {
                        assert_eq!(*start, [0.0, 0.0]);
                        assert_eq!(*end, [1.0, 1.0]);
                        assert_eq!(stops.len(), 2);
                        assert_eq!(stops[0].position, 0.0);
                        assert_eq!(stops[1].position, 1.0);
                    }
                    Gradient::Radial { .. } => panic!("expected Linear gradient"),
                }
            }
            _ => panic!("expected SetFillGradient"),
        }
    }

    #[test]
    fn map_set_radial_gradient() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "center_x": 0.5,
            "center_y": 0.5,
            "radius": 1.0,
            "stops": [
                {"position": 0.0, "r": 1.0, "g": 1.0, "b": 1.0},
                {"position": 1.0, "r": 0.0, "g": 0.0, "b": 0.0}
            ]
        });
        let cmds = map_tool_call("set_radial_gradient", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetFillGradient { fill_gradient, .. } => {
                let grad = fill_gradient.as_ref().unwrap();
                match grad {
                    Gradient::Radial {
                        center,
                        radius,
                        stops,
                    } => {
                        assert_eq!(*center, [0.5, 0.5]);
                        assert_eq!(*radius, 1.0);
                        assert_eq!(stops.len(), 2);
                        // Alpha defaults to 1.0 when omitted
                        assert_eq!(stops[0].color.a, 1.0);
                    }
                    Gradient::Linear { .. } => panic!("expected Radial gradient"),
                }
            }
            _ => panic!("expected SetFillGradient"),
        }
    }

    #[test]
    fn map_set_linear_gradient_missing_stops() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "start_x": 0.0,
            "start_y": 0.0,
            "end_x": 1.0,
            "end_y": 1.0
        });
        let result = map_tool_call("set_linear_gradient", &args);
        assert!(result.is_err());
    }

    #[test]
    fn map_set_linear_gradient_invalid_stop() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "start_x": 0.0,
            "start_y": 0.0,
            "end_x": 1.0,
            "end_y": 1.0,
            "stops": [{"position": 0.0}]
        });
        let result = map_tool_call("set_linear_gradient", &args);
        assert!(result.is_err());
    }

    #[test]
    fn map_set_linear_gradient_caps_at_four_stops() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "start_x": 0.0,
            "start_y": 0.0,
            "end_x": 1.0,
            "end_y": 0.0,
            "stops": [
                {"position": 0.0, "r": 1.0, "g": 0.0, "b": 0.0},
                {"position": 0.25, "r": 0.0, "g": 1.0, "b": 0.0},
                {"position": 0.5, "r": 0.0, "g": 0.0, "b": 1.0},
                {"position": 0.75, "r": 1.0, "g": 1.0, "b": 0.0},
                {"position": 1.0, "r": 0.0, "g": 1.0, "b": 1.0}
            ]
        });
        let cmds = map_tool_call("set_linear_gradient", &args).unwrap();
        match &cmds[0] {
            CommandDescriptor::SetFillGradient { fill_gradient, .. } => {
                if let Gradient::Linear { stops, .. } = fill_gradient.as_ref().unwrap() {
                    assert_eq!(stops.len(), 4);
                } else {
                    panic!("expected Linear");
                }
            }
            _ => panic!("expected SetFillGradient"),
        }
    }

    #[test]
    fn map_set_radial_gradient_missing_radius() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "center_x": 0.5,
            "center_y": 0.5,
            "stops": [{"position": 0.0, "r": 1.0, "g": 0.0, "b": 0.0}]
        });
        let result = map_tool_call("set_radial_gradient", &args);
        assert!(result.is_err());
    }

    #[test]
    fn map_set_drop_shadow_with_defaults() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "offset_x": 4.0,
            "offset_y": 4.0,
            "blur_radius": 8.0
        });
        let cmds = map_tool_call("set_drop_shadow", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetEffects { node_id, effects } => {
                assert_eq!(*node_id, id);
                assert_eq!(effects.len(), 1);
                match &effects[0] {
                    Effect::DropShadow {
                        color,
                        offset_x,
                        offset_y,
                        blur_radius,
                    } => {
                        assert!((color.r).abs() < f32::EPSILON);
                        assert!((color.a - 0.5).abs() < f32::EPSILON);
                        assert!((*offset_x - 4.0).abs() < f32::EPSILON);
                        assert!((*offset_y - 4.0).abs() < f32::EPSILON);
                        assert!((*blur_radius - 8.0).abs() < f32::EPSILON);
                    }
                    Effect::Blur { .. } => panic!("expected DropShadow"),
                }
            }
            _ => panic!("expected SetEffects"),
        }
    }

    #[test]
    fn map_set_drop_shadow_with_color() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "offset_x": 2.0,
            "offset_y": 2.0,
            "blur_radius": 4.0,
            "r": 1.0, "g": 0.0, "b": 0.0, "a": 0.8
        });
        let cmds = map_tool_call("set_drop_shadow", &args).unwrap();
        match &cmds[0] {
            CommandDescriptor::SetEffects { effects, .. } => match &effects[0] {
                Effect::DropShadow { color, .. } => {
                    assert!((color.r - 1.0).abs() < f32::EPSILON);
                    assert!((color.a - 0.8).abs() < f32::EPSILON);
                }
                Effect::Blur { .. } => panic!("expected DropShadow"),
            },
            _ => panic!("expected SetEffects"),
        }
    }

    #[test]
    fn map_set_drop_shadow_missing_offset() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "blur_radius": 8.0
        });
        let result = map_tool_call("set_drop_shadow", &args);
        assert!(result.is_err());
    }

    #[test]
    fn map_set_blur() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "radius": 10.0
        });
        let cmds = map_tool_call("set_blur", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetEffects { node_id, effects } => {
                assert_eq!(*node_id, id);
                assert_eq!(effects.len(), 1);
                match &effects[0] {
                    Effect::Blur { radius } => {
                        assert!((*radius - 10.0).abs() < f32::EPSILON);
                    }
                    Effect::DropShadow { .. } => panic!("expected Blur"),
                }
            }
            _ => panic!("expected SetEffects"),
        }
    }

    #[test]
    fn map_set_blur_missing_radius() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string() });
        let result = map_tool_call("set_blur", &args);
        assert!(result.is_err());
    }

    #[test]
    fn map_remove_effects() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string() });
        let cmds = map_tool_call("remove_effects", &args).unwrap();
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            CommandDescriptor::SetEffects { node_id, effects } => {
                assert_eq!(*node_id, id);
                assert!(effects.is_empty());
            }
            _ => panic!("expected SetEffects"),
        }
    }

    #[test]
    fn opacity_clamped_to_unit_range() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string(), "opacity": 2.5 });
        let cmds = map_tool_call("set_opacity", &args).unwrap();
        match &cmds[0] {
            CommandDescriptor::SetOpacity { opacity, .. } => assert_eq!(*opacity, 1.0),
            _ => panic!("expected SetOpacity"),
        }

        let args = serde_json::json!({ "node_id": id.to_string(), "opacity": -1.0 });
        let cmds = map_tool_call("set_opacity", &args).unwrap();
        match &cmds[0] {
            CommandDescriptor::SetOpacity { opacity, .. } => assert_eq!(*opacity, 0.0),
            _ => panic!("expected SetOpacity"),
        }
    }

    #[test]
    fn color_channels_clamped_to_unit_range() {
        let id = test_node_id();
        let args = serde_json::json!({
            "node_id": id.to_string(),
            "r": 2.0, "g": -0.5, "b": 0.5, "a": 3.0
        });
        let cmds = map_tool_call("set_fill", &args).unwrap();
        match &cmds[0] {
            CommandDescriptor::SetFill { fill: Some(c), .. } => {
                assert_eq!(c.r, 1.0);
                assert_eq!(c.g, 0.0);
                assert_eq!(c.b, 0.5);
                assert_eq!(c.a, 1.0);
            }
            _ => panic!("expected SetFill with color"),
        }
    }

    #[test]
    fn font_weight_clamped() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string(), "font_weight": 2000.0 });
        let cmds = map_tool_call("set_font_weight", &args).unwrap();
        match &cmds[0] {
            CommandDescriptor::SetFontWeight { font_weight, .. } => {
                assert_eq!(*font_weight, 1000);
            }
            _ => panic!("expected SetFontWeight"),
        }
    }

    #[test]
    fn corner_radius_clamped_non_negative() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string(), "radius": -5.0 });
        let cmds = map_tool_call("set_corner_radius", &args).unwrap();
        match &cmds[0] {
            CommandDescriptor::SetCornerRadius { corner_radius, .. } => {
                assert_eq!(*corner_radius, [0.0, 0.0, 0.0, 0.0]);
            }
            _ => panic!("expected SetCornerRadius"),
        }
    }

    #[test]
    fn line_height_clamped() {
        let id = test_node_id();
        let args = serde_json::json!({ "node_id": id.to_string(), "line_height": -1.0 });
        let cmds = map_tool_call("set_line_height", &args).unwrap();
        match &cmds[0] {
            CommandDescriptor::SetLineHeight { line_height, .. } => {
                assert_eq!(*line_height, 0.1);
            }
            _ => panic!("expected SetLineHeight"),
        }

        let args = serde_json::json!({ "node_id": id.to_string(), "line_height": 50.0 });
        let cmds = map_tool_call("set_line_height", &args).unwrap();
        match &cmds[0] {
            CommandDescriptor::SetLineHeight { line_height, .. } => {
                assert_eq!(*line_height, 10.0);
            }
            _ => panic!("expected SetLineHeight"),
        }
    }
}
