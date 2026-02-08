use reqwest::blocking::Client;

use crate::error::{RuntimeError, Result};
use crate::provider::{ChatRequest, ChatResponse, LlmProvider, ToolCallRequest};

/// OpenAI-compatible LLM provider.
///
/// Works with OpenAI API and any compatible endpoint (e.g., Together, Groq, local LLMs).
pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        OpenAiProvider {
            client: Client::new(),
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com".to_string()),
        }
    }

    /// Build the JSON request body for the OpenAI chat completions API.
    pub fn build_request_body(request: &ChatRequest) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|m| {
                let mut msg = serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                });
                if let Some(ref id) = m.tool_call_id {
                    msg["tool_call_id"] = serde_json::json!(id);
                }
                msg
            })
            .collect();

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max);
        }

        if let Some(ref tools) = request.tools {
            let tool_defs: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tool_defs);
        }

        if let Some(ref rf) = request.response_format {
            if rf.format_type == "json_schema" {
                if let Some(ref schema) = rf.json_schema {
                    body["response_format"] = serde_json::json!({
                        "type": "json_schema",
                        "json_schema": {
                            "name": "response",
                            "schema": schema,
                            "strict": true,
                        }
                    });
                }
            } else if rf.format_type == "json_object" {
                body["response_format"] = serde_json::json!({"type": "json_object"});
            }
        }

        body
    }

    /// Parse the JSON response from the OpenAI API.
    pub fn parse_response(json: &serde_json::Value) -> Result<ChatResponse> {
        let choice = json
            .get("choices")
            .and_then(|c| c.get(0))
            .ok_or_else(|| RuntimeError::CallError("no choices in OpenAI response".into()))?;

        let message = choice
            .get("message")
            .ok_or_else(|| RuntimeError::CallError("no message in OpenAI choice".into()))?;

        let text = message
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        let model = json
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown")
            .to_string();

        let usage = json.get("usage");
        let tokens_in = usage
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|t| t.as_i64())
            .unwrap_or(0);
        let tokens_out = usage
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|t| t.as_i64())
            .unwrap_or(0);

        // Parse tool calls if present
        let tool_calls = message
            .get("tool_calls")
            .and_then(|tc| tc.as_array())
            .map(|calls| {
                calls
                    .iter()
                    .filter_map(|call| {
                        let id = call.get("id")?.as_str()?.to_string();
                        let function = call.get("function")?;
                        let function_name = function.get("name")?.as_str()?.to_string();
                        let arguments: serde_json::Value = function
                            .get("arguments")
                            .and_then(|a| a.as_str())
                            .and_then(|s| serde_json::from_str(s).ok())
                            .unwrap_or(serde_json::json!({}));
                        Some(ToolCallRequest {
                            id,
                            function_name,
                            arguments,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(ChatResponse {
            text,
            tokens_in,
            tokens_out,
            model,
            tool_calls,
        })
    }
}

impl LlmProvider for OpenAiProvider {
    fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/v1/chat/completions", self.base_url.trim_end_matches('/'));
        let body = Self::build_request_body(&request);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| RuntimeError::CallError(format!("OpenAI HTTP error: {}", e)))?;

        let status = response.status();
        let response_text = response
            .text()
            .map_err(|e| RuntimeError::CallError(format!("OpenAI read error: {}", e)))?;

        if !status.is_success() {
            return Err(RuntimeError::CallError(format!(
                "OpenAI API error ({}): {}",
                status, response_text
            )));
        }

        let json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| RuntimeError::CallError(format!("OpenAI JSON parse error: {}", e)))?;

        Self::parse_response(&json)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{ChatMessage, ResponseFormat, ToolSchema};

    #[test]
    fn build_request_basic() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
                tool_call_id: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
            tools: None,
            response_format: None,
        };
        let body = OpenAiProvider::build_request_body(&request);
        assert_eq!(body["model"], "gpt-4");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "Hello");
        assert_eq!(body["temperature"], 0.7);
        assert_eq!(body["max_tokens"], 100);
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn build_request_with_tools() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            tools: Some(vec![ToolSchema {
                name: "get_weather".to_string(),
                description: "Get weather for a city".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "city": {"type": "string"}
                    }
                }),
            }]),
            response_format: None,
        };
        let body = OpenAiProvider::build_request_body(&request);
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["function"]["name"], "get_weather");
    }

    #[test]
    fn build_request_with_json_schema() {
        let schema = serde_json::json!({"type": "object", "properties": {"name": {"type": "string"}}});
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            tools: None,
            response_format: Some(ResponseFormat {
                format_type: "json_schema".to_string(),
                json_schema: Some(schema.clone()),
            }),
        };
        let body = OpenAiProvider::build_request_body(&request);
        assert_eq!(body["response_format"]["type"], "json_schema");
        assert_eq!(body["response_format"]["json_schema"]["schema"], schema);
    }

    #[test]
    fn parse_response_basic() {
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello there!"
                }
            }],
            "model": "gpt-4-0613",
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5
            }
        });
        let response = OpenAiProvider::parse_response(&json).unwrap();
        assert_eq!(response.text, "Hello there!");
        assert_eq!(response.model, "gpt-4-0613");
        assert_eq!(response.tokens_in, 10);
        assert_eq!(response.tokens_out, 5);
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn parse_response_with_tool_calls() {
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_123",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"city\": \"Paris\"}"
                        }
                    }]
                }
            }],
            "model": "gpt-4",
            "usage": {"prompt_tokens": 15, "completion_tokens": 8}
        });
        let response = OpenAiProvider::parse_response(&json).unwrap();
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id, "call_123");
        assert_eq!(response.tool_calls[0].function_name, "get_weather");
        assert_eq!(response.tool_calls[0].arguments["city"], "Paris");
    }

    #[test]
    fn parse_response_no_choices_errors() {
        let json = serde_json::json!({"error": "something went wrong"});
        let result = OpenAiProvider::parse_response(&json);
        assert!(result.is_err());
    }
}
