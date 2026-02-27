//! LLM tool definitions and execution mapping for the Selean design platform.
//!
//! Provides [`ToolDefinition`] schemas matching the Claude API tool format and
//! [`map_tool_call`] to convert tool invocations into [`CommandDescriptor`] values
//! that flow through the shared mutation pipeline.

pub mod schema;
pub mod tools;

pub use schema::{ToolDefinition, ToolParameter, ToolParameterType};
pub use tools::{ToolCallError, all_tools, is_page_tool, is_read_only_tool, map_tool_call};
