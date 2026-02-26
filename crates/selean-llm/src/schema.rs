//! Tool definition schema types matching the Claude API format.
//!
//! These types mirror the `tool` objects sent to the Claude API. They are
//! serialized to JSON when building the API request.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A tool definition for the Claude API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    /// Unique tool name (e.g. `set_fill`).
    pub name: String,
    /// Human-readable description shown to the model.
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: InputSchema,
}

/// JSON Schema object describing tool input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputSchema {
    /// Always "object" for Claude tools.
    #[serde(rename = "type")]
    pub schema_type: String,
    /// Property definitions.
    pub properties: BTreeMap<String, ToolParameter>,
    /// Required property names.
    pub required: Vec<String>,
}

/// A single parameter in the tool schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolParameter {
    /// JSON Schema type.
    #[serde(rename = "type")]
    pub param_type: ToolParameterType,
    /// Human-readable description.
    pub description: String,
    /// Allowed values for enum parameters.
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    /// Nested properties for object types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, ToolParameter>>,
    /// Item schema for array types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<ToolParameter>>,
}

/// JSON Schema type tag.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolParameterType {
    /// String value.
    String,
    /// Numeric value.
    Number,
    /// Integer value.
    Integer,
    /// Boolean value.
    Boolean,
    /// Object value.
    Object,
    /// Array value.
    Array,
}

/// Builder for constructing [`ToolDefinition`] values.
pub struct ToolBuilder {
    name: String,
    description: String,
    properties: BTreeMap<String, ToolParameter>,
    required: Vec<String>,
}

impl ToolBuilder {
    /// Start building a tool with the given name and description.
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            properties: BTreeMap::new(),
            required: Vec::new(),
        }
    }

    /// Add a required string parameter.
    pub fn string_param(mut self, name: &str, description: &str) -> Self {
        self.properties.insert(
            name.to_string(),
            ToolParameter {
                param_type: ToolParameterType::String,
                description: description.to_string(),
                enum_values: None,
                properties: None,
                items: None,
            },
        );
        self.required.push(name.to_string());
        self
    }

    /// Add a required number parameter.
    pub fn number_param(mut self, name: &str, description: &str) -> Self {
        self.properties.insert(
            name.to_string(),
            ToolParameter {
                param_type: ToolParameterType::Number,
                description: description.to_string(),
                enum_values: None,
                properties: None,
                items: None,
            },
        );
        self.required.push(name.to_string());
        self
    }

    /// Add an optional number parameter.
    pub fn optional_number_param(mut self, name: &str, description: &str) -> Self {
        self.properties.insert(
            name.to_string(),
            ToolParameter {
                param_type: ToolParameterType::Number,
                description: description.to_string(),
                enum_values: None,
                properties: None,
                items: None,
            },
        );
        self
    }

    /// Add a required boolean parameter.
    pub fn boolean_param(mut self, name: &str, description: &str) -> Self {
        self.properties.insert(
            name.to_string(),
            ToolParameter {
                param_type: ToolParameterType::Boolean,
                description: description.to_string(),
                enum_values: None,
                properties: None,
                items: None,
            },
        );
        self.required.push(name.to_string());
        self
    }

    /// Add a required enum parameter (string with allowed values).
    pub fn enum_param(mut self, name: &str, description: &str, values: &[&str]) -> Self {
        self.properties.insert(
            name.to_string(),
            ToolParameter {
                param_type: ToolParameterType::String,
                description: description.to_string(),
                enum_values: Some(values.iter().map(|s| (*s).to_string()).collect()),
                properties: None,
                items: None,
            },
        );
        self.required.push(name.to_string());
        self
    }

    /// Add an optional string parameter.
    pub fn optional_string_param(mut self, name: &str, description: &str) -> Self {
        self.properties.insert(
            name.to_string(),
            ToolParameter {
                param_type: ToolParameterType::String,
                description: description.to_string(),
                enum_values: None,
                properties: None,
                items: None,
            },
        );
        self
    }

    /// Consume the builder and produce a [`ToolDefinition`].
    pub fn build(self) -> ToolDefinition {
        ToolDefinition {
            name: self.name,
            description: self.description,
            input_schema: InputSchema {
                schema_type: "object".to_string(),
                properties: self.properties,
                required: self.required,
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn tool_builder_produces_valid_schema() {
        let tool = ToolBuilder::new("test_tool", "A test tool")
            .string_param("name", "The name")
            .number_param("value", "A number")
            .build();

        assert_eq!(tool.name, "test_tool");
        assert_eq!(tool.input_schema.schema_type, "object");
        assert_eq!(tool.input_schema.properties.len(), 2);
        assert_eq!(tool.input_schema.required.len(), 2);
        assert!(tool.input_schema.required.contains(&"name".to_string()));
        assert!(tool.input_schema.required.contains(&"value".to_string()));
    }

    #[test]
    fn tool_builder_optional_params_not_required() {
        let tool = ToolBuilder::new("opt_tool", "Optional params")
            .string_param("required_field", "Must provide")
            .optional_string_param("optional_field", "May provide")
            .build();

        assert_eq!(tool.input_schema.required.len(), 1);
        assert_eq!(tool.input_schema.required[0], "required_field");
        assert_eq!(tool.input_schema.properties.len(), 2);
    }

    #[test]
    fn tool_builder_enum_param() {
        let tool = ToolBuilder::new("enum_tool", "Enum param")
            .enum_param("mode", "The mode", &["fast", "slow"])
            .build();

        let param = &tool.input_schema.properties["mode"];
        assert_eq!(
            param.enum_values.as_ref().map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn tool_definition_roundtrip() {
        let tool = ToolBuilder::new("roundtrip", "Test roundtrip")
            .string_param("id", "Node ID")
            .number_param("x", "X position")
            .boolean_param("visible", "Is visible")
            .build();

        let json = serde_json::to_string(&tool).expect("serialize");
        let back: ToolDefinition = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(tool, back);
    }

    #[test]
    fn schema_type_is_always_object() {
        let tool = ToolBuilder::new("simple", "Simple tool")
            .string_param("a", "Param A")
            .build();

        assert_eq!(tool.input_schema.schema_type, "object");
    }
}
