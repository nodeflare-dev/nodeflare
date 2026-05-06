//! MCP Protocol Runtime
//!
//! Types and utilities for working with the MCP (Model Context Protocol)

use serde::{Deserialize, Serialize};

/// MCP Server manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub tools: Vec<ToolDefinition>,
    pub resources: Option<Vec<ResourceDefinition>>,
    pub prompts: Option<Vec<PromptDefinition>>,
}

/// Tool definition from MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Option<serde_json::Value>,
}

/// Resource definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

/// Prompt definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDefinition {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Option<Vec<PromptArgument>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: Option<bool>,
}

/// JSON-RPC Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: serde_json::Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

/// MCP Method names
pub mod methods {
    pub const INITIALIZE: &str = "initialize";
    pub const LIST_TOOLS: &str = "tools/list";
    pub const CALL_TOOL: &str = "tools/call";
    pub const LIST_RESOURCES: &str = "resources/list";
    pub const READ_RESOURCE: &str = "resources/read";
    pub const LIST_PROMPTS: &str = "prompts/list";
    pub const GET_PROMPT: &str = "prompts/get";
}

/// Parse tools from a list tools response
pub fn parse_tools_response(response: &JsonRpcResponse) -> Option<Vec<ToolDefinition>> {
    response.result.as_ref().and_then(|result| {
        result
            .get("tools")
            .and_then(|tools| serde_json::from_value(tools.clone()).ok())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tools() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            result: Some(serde_json::json!({
                "tools": [
                    {
                        "name": "read_file",
                        "description": "Read a file",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" }
                            }
                        }
                    }
                ]
            })),
            error: None,
        };

        let tools = parse_tools_response(&response).unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "read_file");
    }
}
