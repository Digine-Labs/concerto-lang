use std::collections::HashMap;

use concerto_common::ir::IrConnection;

use crate::error::{RuntimeError, Result};

// ============================================================================
// LLM Provider trait and types
// ============================================================================

/// A chat message in a conversation.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub tool_call_id: Option<String>,
}

/// A tool call requested by the LLM.
#[derive(Debug, Clone)]
pub struct ToolCallRequest {
    pub id: String,
    pub function_name: String,
    pub arguments: serde_json::Value,
}

/// Tool schema sent to the LLM for function calling.
#[derive(Debug, Clone)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Response format specification (e.g., JSON Schema for structured output).
#[derive(Debug, Clone)]
pub struct ResponseFormat {
    pub format_type: String,
    pub json_schema: Option<serde_json::Value>,
}

/// A request to an LLM provider.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub tools: Option<Vec<ToolSchema>>,
    pub response_format: Option<ResponseFormat>,
}

/// A response from an LLM provider.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub text: String,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub model: String,
    pub tool_calls: Vec<ToolCallRequest>,
}

/// Trait for LLM provider implementations.
///
/// Synchronous in Phase 3b (uses reqwest::blocking internally).
/// Will become async in Phase 3c.
pub trait LlmProvider: Send + Sync {
    fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse>;
}

// ============================================================================
// Mock Provider (for testing and when no API key is set)
// ============================================================================

/// A mock LLM provider that returns deterministic responses.
/// Used in tests and when no real connection is configured.
pub struct MockProvider;

impl LlmProvider for MockProvider {
    fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse> {
        let prompt = request
            .messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();
        let truncated = if prompt.len() > 50 {
            &prompt[..50]
        } else {
            &prompt
        };

        // If response_format is json_schema, return mock JSON matching the schema
        let text = if let Some(ref rf) = request.response_format {
            if let Some(ref schema) = rf.json_schema {
                mock_json_from_schema(schema)
            } else {
                format!("[mock response to: {}]", truncated)
            }
        } else {
            format!("[mock response to: {}]", truncated)
        };

        Ok(ChatResponse {
            text,
            tokens_in: prompt.len() as i64,
            tokens_out: 42,
            model: request.model,
            tool_calls: vec![],
        })
    }
}

/// Generate mock JSON from a JSON Schema.
fn mock_json_from_schema(schema: &serde_json::Value) -> String {
    let mut result = serde_json::Map::new();
    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        for (name, prop_schema) in props {
            // Check for enum constraint first — pick the first allowed value
            if let Some(enum_vals) = prop_schema.get("enum").and_then(|e| e.as_array()) {
                if let Some(first) = enum_vals.first() {
                    result.insert(name.clone(), first.clone());
                    continue;
                }
            }

            let field_type = prop_schema
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("string");
            let mock_value = match field_type {
                "string" | "String" => serde_json::Value::String(format!("[mock {}]", name)),
                "integer" | "int" | "Int" => serde_json::json!(0),
                "number" | "float" | "Float" => serde_json::json!(0.0),
                "boolean" | "bool" | "Bool" => serde_json::json!(false),
                t if t.starts_with("Array<") || t == "array" => {
                    serde_json::json!([format!("[mock {} item]", name)])
                }
                _ => serde_json::Value::String(format!("[mock {}]", name)),
            };
            result.insert(name.clone(), mock_value);
        }
    }
    serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
}

// ============================================================================
// Connection Manager
// ============================================================================

/// Manages LLM provider instances, one per connection name.
pub struct ConnectionManager {
    providers: HashMap<String, Box<dyn LlmProvider>>,
    fallback: MockProvider,
}

impl ConnectionManager {
    /// Create a ConnectionManager from loaded IR connections.
    /// For each connection, attempts to resolve API key from env and
    /// create the appropriate provider. Falls back to MockProvider.
    pub fn from_connections(connections: &HashMap<String, IrConnection>) -> Self {
        let mut providers: HashMap<String, Box<dyn LlmProvider>> = HashMap::new();

        for (name, conn) in connections {
            match create_provider(conn) {
                Ok(provider) => {
                    providers.insert(name.clone(), provider);
                }
                Err(_) => {
                    // No API key or invalid config — will fall back to mock
                }
            }
        }

        ConnectionManager {
            providers,
            fallback: MockProvider,
        }
    }

    /// Get the provider for a connection name.
    /// Returns the fallback MockProvider if no real provider is configured.
    pub fn get_provider(&self, name: &str) -> &dyn LlmProvider {
        self.providers
            .get(name)
            .map(|p| p.as_ref())
            .unwrap_or(&self.fallback)
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        ConnectionManager {
            providers: HashMap::new(),
            fallback: MockProvider,
        }
    }
}

/// Create a provider from an IR connection config.
/// Returns Err if no API key is available.
fn create_provider(conn: &IrConnection) -> Result<Box<dyn LlmProvider>> {
    let config = &conn.config;

    // Try to resolve API key (may be env("VAR") or direct string)
    let api_key = resolve_api_key(config)?;
    if api_key.is_empty() {
        return Err(RuntimeError::CallError("no API key for connection".into()));
    }

    let base_url = config
        .get("base_url")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Detect provider type from connection name or base_url
    let is_anthropic = conn.name == "anthropic"
        || base_url
            .as_deref()
            .is_some_and(|u| u.contains("anthropic"));

    if is_anthropic {
        Ok(Box::new(
            crate::providers::anthropic::AnthropicProvider::new(api_key, base_url),
        ))
    } else {
        // Default to OpenAI-compatible
        Ok(Box::new(
            crate::providers::openai::OpenAiProvider::new(api_key, base_url),
        ))
    }
}

/// Resolve an API key from connection config.
/// The config may have `api_key: "sk-..."` (direct) or
/// `api_key: {"$env": "OPENAI_API_KEY"}` (env reference).
fn resolve_api_key(config: &serde_json::Value) -> Result<String> {
    match config.get("api_key") {
        Some(serde_json::Value::String(key)) => Ok(key.clone()),
        Some(obj) if obj.get("$env").is_some() => {
            let var_name = obj["$env"]
                .as_str()
                .ok_or_else(|| RuntimeError::CallError("$env must be a string".into()))?;
            std::env::var(var_name).map_err(|_| {
                RuntimeError::CallError(format!("env var '{}' not set", var_name))
            })
        }
        _ => Err(RuntimeError::CallError("no api_key in connection config".into())),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_provider_returns_response() {
        let provider = MockProvider;
        let request = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Hello world".to_string(),
                tool_call_id: None,
            }],
            temperature: None,
            max_tokens: None,
            tools: None,
            response_format: None,
        };
        let response = provider.chat_completion(request).unwrap();
        assert!(response.text.contains("Hello world"));
        assert_eq!(response.model, "test-model");
        assert!(response.tokens_in > 0);
    }

    #[test]
    fn mock_provider_json_schema_response() {
        let provider = MockProvider;
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" },
                "count": { "type": "integer" }
            }
        });
        let request = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "test".to_string(),
                tool_call_id: None,
            }],
            temperature: None,
            max_tokens: None,
            tools: None,
            response_format: Some(ResponseFormat {
                format_type: "json_schema".to_string(),
                json_schema: Some(schema),
            }),
        };
        let response = provider.chat_completion(request).unwrap();
        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&response.text).unwrap();
        assert!(parsed.get("message").is_some());
        assert!(parsed.get("count").is_some());
    }

    #[test]
    fn connection_manager_defaults_to_mock() {
        let manager = ConnectionManager::default();
        let provider = manager.get_provider("nonexistent");
        let request = ChatRequest {
            model: "test".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            tools: None,
            response_format: None,
        };
        // Should work (mock provider)
        let result = provider.chat_completion(request);
        assert!(result.is_ok());
    }
}
