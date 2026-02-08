use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

use crate::error::{RuntimeError, Result};
use crate::value::Value;

/// A JSON-RPC 2.0 request.
#[derive(serde::Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 response.
#[derive(serde::Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)] // required for JSON-RPC protocol deserialization
    jsonrpc: String,
    #[allow(dead_code)] // required for JSON-RPC protocol deserialization
    id: u64,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

/// JSON-RPC error object.
#[derive(serde::Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[allow(dead_code)] // part of JSON-RPC error spec
    #[serde(default)]
    data: Option<serde_json::Value>,
}

/// MCP tool definition from the server's tools/list response.
#[derive(Debug, Clone)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// An active MCP client connected to a server process (stdio transport).
pub struct McpClient {
    name: String,
    child: Child,
    request_id: u64,
    tools: HashMap<String, McpToolDef>,
}

impl McpClient {
    /// Start an MCP server process and initialize the connection.
    pub fn connect(name: &str, config: &serde_json::Value) -> Result<Self> {
        let transport = config
            .get("transport")
            .and_then(|v| v.as_str())
            .unwrap_or("stdio");

        if transport != "stdio" {
            return Err(RuntimeError::CallError(format!(
                "MCP transport '{}' not supported (only stdio)", transport
            )));
        }

        let command = config
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                RuntimeError::CallError(format!("MCP '{}' missing 'command' field", name))
            })?;

        // Parse command string into program and args
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(RuntimeError::CallError("empty MCP command".into()));
        }

        let mut cmd = Command::new(parts[0]);
        for arg in &parts[1..] {
            cmd.arg(arg);
        }

        // Set environment variables from config
        if let Some(env_obj) = config.get("env").and_then(|v| v.as_object()) {
            for (key, val) in env_obj {
                if let Some(val_str) = val.as_str() {
                    cmd.env(key, val_str);
                }
            }
        }

        let child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                RuntimeError::CallError(format!(
                    "failed to start MCP server '{}': {}", name, e
                ))
            })?;

        let mut client = McpClient {
            name: name.to_string(),
            child,
            request_id: 0,
            tools: HashMap::new(),
        };

        // Initialize (MCP protocol handshake)
        client.initialize()?;

        // Discover tools
        client.discover_tools()?;

        Ok(client)
    }

    /// Send a JSON-RPC request and read the response.
    fn send_request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        self.request_id += 1;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: self.request_id,
            method: method.to_string(),
            params,
        };

        let stdin = self.child.stdin.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("MCP '{}': stdin not available", self.name))
        })?;

        let json = serde_json::to_string(&request).map_err(|e| {
            RuntimeError::CallError(format!("MCP '{}': serialize error: {}", self.name, e))
        })?;

        writeln!(stdin, "{}", json).map_err(|e| {
            RuntimeError::CallError(format!("MCP '{}': write error: {}", self.name, e))
        })?;
        stdin.flush().map_err(|e| {
            RuntimeError::CallError(format!("MCP '{}': flush error: {}", self.name, e))
        })?;

        // Read response line from stdout
        let stdout = self.child.stdout.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("MCP '{}': stdout not available", self.name))
        })?;

        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| {
            RuntimeError::CallError(format!("MCP '{}': read error: {}", self.name, e))
        })?;

        if line.is_empty() {
            return Err(RuntimeError::CallError(format!(
                "MCP '{}': server closed connection", self.name
            )));
        }

        let response: JsonRpcResponse = serde_json::from_str(&line).map_err(|e| {
            RuntimeError::CallError(format!("MCP '{}': response parse error: {}", self.name, e))
        })?;

        if let Some(error) = response.error {
            return Err(RuntimeError::CallError(format!(
                "MCP error ({}): {}",
                error.code, error.message
            )));
        }

        response.result.ok_or_else(|| {
            RuntimeError::CallError("MCP response missing result".into())
        })
    }

    /// MCP initialize handshake.
    fn initialize(&mut self) -> Result<()> {
        let _result = self.send_request(
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "concerto-runtime",
                    "version": "0.1.0"
                }
            })),
        )?;
        Ok(())
    }

    /// Discover available tools from the MCP server.
    fn discover_tools(&mut self) -> Result<()> {
        let result = self.send_request("tools/list", None)?;

        if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
            for tool in tools {
                let name = tool
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let description = tool
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();
                let input_schema = tool
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                self.tools.insert(
                    name.clone(),
                    McpToolDef {
                        name,
                        description,
                        input_schema,
                    },
                );
            }
        }
        Ok(())
    }

    /// Call a tool on the MCP server.
    pub fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<Value> {
        let result = self.send_request(
            "tools/call",
            Some(serde_json::json!({
                "name": tool_name,
                "arguments": arguments,
            })),
        )?;

        // MCP returns { content: [{ type: "text", text: "..." }] }
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            if let Some(first) = content.first() {
                if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                    // Try to parse as JSON
                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(text) {
                        return Ok(crate::schema::SchemaValidator::json_to_value(&json_val));
                    }
                    return Ok(Value::String(text.to_string()));
                }
            }
        }

        Ok(Value::Nil)
    }

    /// Get list of available tool names.
    pub fn tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get tool definitions for LLM function calling.
    pub fn get_tool_schemas(&self) -> Vec<crate::provider::ToolSchema> {
        self.tools
            .values()
            .map(|t| crate::provider::ToolSchema {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.input_schema.clone(),
            })
            .collect()
    }

    /// Check if the server provides a specific tool.
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Registry of active MCP connections.
pub struct McpRegistry {
    clients: HashMap<String, McpClient>,
}

impl McpRegistry {
    pub fn new() -> Self {
        McpRegistry {
            clients: HashMap::new(),
        }
    }

    /// Initialize MCP connections from IR connections that have type="mcp".
    pub fn from_connections(
        connections: &HashMap<String, concerto_common::ir::IrConnection>,
    ) -> Self {
        let mut registry = McpRegistry::new();

        for (name, conn) in connections {
            if conn.config.get("type").and_then(|v| v.as_str()) == Some("mcp") {
                match McpClient::connect(name, &conn.config) {
                    Ok(client) => {
                        registry.clients.insert(name.clone(), client);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to connect MCP '{}': {}", name, e);
                    }
                }
            }
        }

        registry
    }

    /// Call a tool on an MCP server.
    pub fn call_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<Value> {
        let client = self.clients.get_mut(server_name).ok_or_else(|| {
            RuntimeError::CallError(format!("MCP server '{}' not connected", server_name))
        })?;
        client.call_tool(tool_name, arguments)
    }

    /// Check if a server is connected.
    pub fn has_server(&self, name: &str) -> bool {
        self.clients.contains_key(name)
    }

    /// Get tool schemas from a specific MCP server for LLM function calling.
    pub fn get_tool_schemas(&self, server_name: &str) -> Vec<crate::provider::ToolSchema> {
        self.clients
            .get(server_name)
            .map(|client| client.get_tool_schemas())
            .unwrap_or_default()
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_registry_empty() {
        let registry = McpRegistry::new();
        assert!(!registry.has_server("anything"));
        assert!(registry.get_tool_schemas("anything").is_empty());
    }

    #[test]
    fn mcp_registry_skips_non_mcp_connections() {
        let mut connections = HashMap::new();
        connections.insert(
            "openai".to_string(),
            concerto_common::ir::IrConnection {
                name: "openai".to_string(),
                config: serde_json::json!({
                    "api_key": "test",
                    "default_model": "gpt-4"
                }),
            },
        );
        let registry = McpRegistry::from_connections(&connections);
        assert!(!registry.has_server("openai"));
    }

    #[test]
    fn mcp_registry_handles_missing_command() {
        let mut connections = HashMap::new();
        connections.insert(
            "test_mcp".to_string(),
            concerto_common::ir::IrConnection {
                name: "test_mcp".to_string(),
                config: serde_json::json!({
                    "type": "mcp",
                    "transport": "stdio"
                    // missing "command" field
                }),
            },
        );
        // Should not panic — logs a warning and skips
        let registry = McpRegistry::from_connections(&connections);
        assert!(!registry.has_server("test_mcp"));
    }

    #[test]
    fn mcp_registry_unsupported_transport() {
        let mut connections = HashMap::new();
        connections.insert(
            "test_sse".to_string(),
            concerto_common::ir::IrConnection {
                name: "test_sse".to_string(),
                config: serde_json::json!({
                    "type": "mcp",
                    "transport": "sse",
                    "url": "http://localhost:3000"
                }),
            },
        );
        let registry = McpRegistry::from_connections(&connections);
        assert!(!registry.has_server("test_sse"));
    }

    #[test]
    fn json_rpc_request_serialization() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "search",
                "arguments": {"query": "test"}
            })),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["method"], "tools/call");
        assert_eq!(parsed["params"]["name"], "search");
    }

    #[test]
    fn json_rpc_response_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"search","description":"Search repos","inputSchema":{}}]}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "search");
    }

    #[test]
    fn json_rpc_error_response() {
        let json = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert!(response.result.is_none());
        let error = response.error.unwrap();
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found");
    }

    #[test]
    fn call_tool_on_missing_server() {
        let mut registry = McpRegistry::new();
        let result = registry.call_tool("nonexistent", "search", serde_json::json!({}));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not connected"));
    }
}
