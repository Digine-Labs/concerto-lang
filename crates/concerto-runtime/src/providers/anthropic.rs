use reqwest::blocking::Client;

use crate::error::{RuntimeError, Result};
use crate::provider::{ChatRequest, ChatResponse, LlmProvider, ToolCallRequest};

/// Anthropic LLM provider (Claude API).
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        AnthropicProvider {
            client: Client::new(),
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string()),
        }
    }

    /// Build the JSON request body for the Anthropic messages API.
    pub fn build_request_body(request: &ChatRequest) -> serde_json::Value {
        // Anthropic requires system prompt separate from messages.
        // Extract system messages and keep only user/assistant messages.
        let mut system_text = String::new();
        let mut messages: Vec<serde_json::Value> = Vec::new();

        for m in &request.messages {
            if m.role == "system" {
                if !system_text.is_empty() {
                    system_text.push('\n');
                }
                system_text.push_str(&m.content);
            } else if m.role == "tool" {
                // Anthropic uses tool_result content blocks
                messages.push(serde_json::json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": m.tool_call_id.as_deref().unwrap_or(""),
                        "content": m.content,
                    }]
                }));
            } else {
                messages.push(serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                }));
            }
        }

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
        });

        if !system_text.is_empty() {
            body["system"] = serde_json::json!(system_text);
        }

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        if let Some(ref tools) = request.tools {
            let tool_defs: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tool_defs);
        }

        body
    }

    /// Parse the JSON response from the Anthropic API.
    pub fn parse_response(json: &serde_json::Value) -> Result<ChatResponse> {
        let content = json
            .get("content")
            .and_then(|c| c.as_array())
            .ok_or_else(|| RuntimeError::CallError("no content in Anthropic response".into()))?;

        // Collect text blocks
        let mut text_parts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<ToolCallRequest> = Vec::new();

        for block in content {
            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match block_type {
                "text" => {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        text_parts.push(t.to_string());
                    }
                }
                "tool_use" => {
                    let id = block
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input = block
                        .get("input")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));
                    tool_calls.push(ToolCallRequest {
                        id,
                        function_name: name,
                        arguments: input,
                    });
                }
                _ => {}
            }
        }

        let model = json
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown")
            .to_string();

        let usage = json.get("usage");
        let tokens_in = usage
            .and_then(|u| u.get("input_tokens"))
            .and_then(|t| t.as_i64())
            .unwrap_or(0);
        let tokens_out = usage
            .and_then(|u| u.get("output_tokens"))
            .and_then(|t| t.as_i64())
            .unwrap_or(0);

        Ok(ChatResponse {
            text: text_parts.join(""),
            tokens_in,
            tokens_out,
            model,
            tool_calls,
        })
    }
}

impl LlmProvider for AnthropicProvider {
    fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let body = Self::build_request_body(&request);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| RuntimeError::CallError(format!("Anthropic HTTP error: {}", e)))?;

        let status = response.status();
        let response_text = response
            .text()
            .map_err(|e| RuntimeError::CallError(format!("Anthropic read error: {}", e)))?;

        if !status.is_success() {
            return Err(RuntimeError::CallError(format!(
                "Anthropic API error ({}): {}",
                status, response_text
            )));
        }

        let json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| RuntimeError::CallError(format!("Anthropic JSON parse error: {}", e)))?;

        Self::parse_response(&json)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{ChatMessage, ToolSchema};

    #[test]
    fn build_request_basic() {
        let request = ChatRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: "You are helpful.".to_string(),
                    tool_call_id: None,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: "Hello".to_string(),
                    tool_call_id: None,
                },
            ],
            temperature: Some(0.5),
            max_tokens: Some(200),
            tools: None,
            response_format: None,
        };
        let body = AnthropicProvider::build_request_body(&request);
        assert_eq!(body["model"], "claude-sonnet-4-5-20250929");
        assert_eq!(body["system"], "You are helpful.");
        // Only user message in messages array (system extracted)
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["temperature"], 0.5);
        assert_eq!(body["max_tokens"], 200);
    }

    #[test]
    fn build_request_with_tools() {
        let request = ChatRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "What's the weather?".to_string(),
                tool_call_id: None,
            }],
            temperature: None,
            max_tokens: None,
            tools: Some(vec![ToolSchema {
                name: "get_weather".to_string(),
                description: "Get weather".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            }]),
            response_format: None,
        };
        let body = AnthropicProvider::build_request_body(&request);
        assert_eq!(body["tools"][0]["name"], "get_weather");
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
    }

    #[test]
    fn build_request_default_max_tokens() {
        let request = ChatRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            tools: None,
            response_format: None,
        };
        let body = AnthropicProvider::build_request_body(&request);
        assert_eq!(body["max_tokens"], 4096);
    }

    #[test]
    fn parse_response_text() {
        let json = serde_json::json!({
            "content": [
                {"type": "text", "text": "Hello! How can I help?"}
            ],
            "model": "claude-sonnet-4-5-20250929",
            "usage": {
                "input_tokens": 12,
                "output_tokens": 8
            }
        });
        let response = AnthropicProvider::parse_response(&json).unwrap();
        assert_eq!(response.text, "Hello! How can I help?");
        assert_eq!(response.model, "claude-sonnet-4-5-20250929");
        assert_eq!(response.tokens_in, 12);
        assert_eq!(response.tokens_out, 8);
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn parse_response_with_tool_use() {
        let json = serde_json::json!({
            "content": [
                {"type": "text", "text": "Let me check the weather."},
                {
                    "type": "tool_use",
                    "id": "toolu_123",
                    "name": "get_weather",
                    "input": {"city": "London"}
                }
            ],
            "model": "claude-sonnet-4-5-20250929",
            "usage": {"input_tokens": 20, "output_tokens": 15}
        });
        let response = AnthropicProvider::parse_response(&json).unwrap();
        assert_eq!(response.text, "Let me check the weather.");
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id, "toolu_123");
        assert_eq!(response.tool_calls[0].function_name, "get_weather");
        assert_eq!(response.tool_calls[0].arguments["city"], "London");
    }

    #[test]
    fn parse_response_no_content_errors() {
        let json = serde_json::json!({"error": {"message": "invalid_api_key"}});
        let result = AnthropicProvider::parse_response(&json);
        assert!(result.is_err());
    }
}
