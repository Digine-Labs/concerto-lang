use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::Duration;

use concerto_common::ir::IrHost;

use crate::error::{Result, RuntimeError};
use crate::value::Value;

/// Format for host I/O.
#[derive(Debug, Clone, PartialEq)]
pub enum HostFormat {
    Text,
    Json,
}

impl HostFormat {
    fn from_str(s: &str) -> Self {
        match s {
            "json" => HostFormat::Json,
            _ => HostFormat::Text,
        }
    }
}

/// A client that manages communication with an external agent system via stdio.
pub struct HostClient {
    name: String,
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    working_dir: Option<String>,
    input_format: HostFormat,
    output_format: HostFormat,
    timeout: Duration,
    child: Option<Child>,
    /// Persistent buffered reader for streaming (taken from child.stdout).
    stdout_reader: Option<BufReader<ChildStdout>>,
}

impl std::fmt::Debug for HostClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostClient")
            .field("name", &self.name)
            .field("command", &self.command)
            .field("input_format", &self.input_format)
            .field("output_format", &self.output_format)
            .field("timeout", &self.timeout)
            .field("connected", &self.child.is_some())
            .finish()
    }
}

impl HostClient {
    /// Create from an IrHost (which has embedded TOML config).
    pub fn from_ir(ir_host: &IrHost) -> Self {
        let timeout_secs = ir_host.timeout.unwrap_or(120);
        Self {
            name: ir_host.name.clone(),
            command: ir_host.command.clone().unwrap_or_default(),
            args: ir_host.args.clone().unwrap_or_default(),
            env: ir_host.env.clone().unwrap_or_default(),
            working_dir: ir_host.working_dir.clone(),
            input_format: HostFormat::from_str(&ir_host.input_format),
            output_format: HostFormat::from_str(&ir_host.output_format),
            timeout: Duration::from_secs(timeout_secs as u64),
            child: None,
            stdout_reader: None,
        }
    }

    /// Ensure the subprocess is running. Spawns if not connected.
    fn ensure_connected(&mut self) -> Result<()> {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_)) => {
                    self.child = None;
                    self.stdout_reader = None;
                }
                Ok(None) => return Ok(()),
                Err(_) => {
                    self.child = None;
                    self.stdout_reader = None;
                }
            }
        }

        if self.command.is_empty() {
            return Err(RuntimeError::CallError(format!(
                "Host '{}' has no command configured (check Concerto.toml [hosts.{}])",
                self.name, self.name
            )));
        }

        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);
        for (key, val) in &self.env {
            cmd.env(key, val);
        }
        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            RuntimeError::CallError(format!(
                "Failed to spawn host '{}' (command: {}): {}",
                self.name, self.command, e
            ))
        })?;

        // Take stdout for the persistent BufReader
        let stdout = child.stdout.take().ok_or_else(|| {
            RuntimeError::CallError(format!("Host '{}' stdout not available", self.name))
        })?;
        self.stdout_reader = Some(BufReader::new(stdout));
        self.child = Some(child);
        Ok(())
    }

    /// Execute a prompt on the host and return the response.
    pub fn execute(&mut self, prompt: &str, context: Option<&Value>) -> Result<String> {
        self.ensure_connected()?;

        let child = self.child.as_mut().unwrap();
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("Host '{}' stdin not available", self.name))
        })?;

        // Write input
        let input = match self.input_format {
            HostFormat::Text => {
                if let Some(ctx) = context {
                    format!("{}\nContext: {}\n", prompt, ctx)
                } else {
                    format!("{}\n", prompt)
                }
            }
            HostFormat::Json => {
                let mut payload = serde_json::json!({ "prompt": prompt });
                if let Some(ctx) = context {
                    payload["context"] = ctx.to_json();
                }
                format!("{}\n", payload)
            }
        };

        stdin.write_all(input.as_bytes()).map_err(|e| {
            RuntimeError::CallError(format!("Failed to write to host '{}': {}", self.name, e))
        })?;
        stdin.flush().map_err(|e| {
            RuntimeError::CallError(format!("Failed to flush host '{}' stdin: {}", self.name, e))
        })?;

        // Read output (one line, blocking)
        let reader = self.stdout_reader.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("Host '{}' stdout not available", self.name))
        })?;

        let mut line = String::new();

        reader.read_line(&mut line).map_err(|e| {
            RuntimeError::CallError(format!("Host '{}' read error: {}", self.name, e))
        })?;

        if line.is_empty() {
            self.child = None;
            return Err(RuntimeError::CallError(format!(
                "Host '{}' process exited unexpectedly",
                self.name
            )));
        }

        let response = line.trim_end().to_string();

        // Parse based on output format
        match self.output_format {
            HostFormat::Text => Ok(response),
            HostFormat::Json => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                    if let Some(text) = json.get("text").and_then(|t| t.as_str()) {
                        Ok(text.to_string())
                    } else {
                        Ok(response)
                    }
                } else {
                    Ok(response)
                }
            }
        }
    }

    /// Send initial prompt for a streaming session (no read).
    pub fn write_prompt_streaming(&mut self, prompt: &str, context: Option<&Value>) -> Result<()> {
        self.ensure_connected()?;

        let child = self.child.as_mut().unwrap();
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("Host '{}' stdin not available", self.name))
        })?;

        let input = match self.input_format {
            HostFormat::Text => {
                if let Some(ctx) = context {
                    format!("{}\nContext: {}\n", prompt, ctx)
                } else {
                    format!("{}\n", prompt)
                }
            }
            HostFormat::Json => {
                let mut payload = serde_json::json!({ "prompt": prompt });
                if let Some(ctx) = context {
                    payload["context"] = ctx.to_json();
                }
                format!("{}\n", payload)
            }
        };

        stdin.write_all(input.as_bytes()).map_err(|e| {
            RuntimeError::CallError(format!("Failed to write to host '{}': {}", self.name, e))
        })?;
        stdin.flush().map_err(|e| {
            RuntimeError::CallError(format!("Failed to flush host '{}' stdin: {}", self.name, e))
        })?;

        Ok(())
    }

    /// Read one NDJSON message from host stdout.
    /// Returns None on EOF (host exited).
    /// Non-JSON lines are wrapped as `{"type": "result", "text": "<line>"}`.
    pub fn read_message(&mut self) -> Result<Option<serde_json::Value>> {
        loop {
            let reader = self.stdout_reader.as_mut().ok_or_else(|| {
                RuntimeError::CallError(format!("Host '{}' not connected", self.name))
            })?;

            let mut line = String::new();

            match reader.read_line(&mut line) {
                Ok(0) => {
                    // EOF
                    self.child = None;
                    self.stdout_reader = None;
                    return Ok(None);
                }
                Ok(_) => {
                    let trimmed = line.trim_end();
                    if trimmed.is_empty() {
                        // Skip empty lines, try next
                        continue;
                    }
                    // Try parsing as JSON
                    return match serde_json::from_str::<serde_json::Value>(trimmed) {
                        Ok(json) if json.is_object() && json.get("type").is_some() => {
                            Ok(Some(json))
                        }
                        _ => {
                            // Non-JSON or no "type" field: wrap as result
                            Ok(Some(serde_json::json!({
                                "type": "result",
                                "text": trimmed
                            })))
                        }
                    };
                }
                Err(e) => {
                    self.child = None;
                    self.stdout_reader = None;
                    return Err(RuntimeError::CallError(format!(
                        "Host '{}' read error: {}",
                        self.name, e
                    )));
                }
            }
        }
    }

    /// Write a response JSON line to host stdin.
    pub fn write_response(&mut self, response: &serde_json::Value) -> Result<()> {
        let child = self.child.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("Host '{}' not connected", self.name))
        })?;

        let stdin = child.stdin.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("Host '{}' stdin not available", self.name))
        })?;

        let line = format!("{}\n", serde_json::to_string(response).unwrap_or_default());
        stdin.write_all(line.as_bytes()).map_err(|e| {
            RuntimeError::CallError(format!("Failed to write response to host '{}': {}", self.name, e))
        })?;
        stdin.flush().map_err(|e| {
            RuntimeError::CallError(format!("Failed to flush host '{}' stdin: {}", self.name, e))
        })?;

        Ok(())
    }

    /// Shutdown the host process.
    pub fn shutdown(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;
        self.stdout_reader = None;
    }
}

impl Drop for HostClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Registry that manages named host clients.
#[derive(Debug, Default)]
pub struct HostRegistry {
    clients: HashMap<String, HostClient>,
}

impl HostRegistry {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    /// Register a host from an IrHost (which contains embedded TOML config).
    pub fn register(&mut self, ir_host: &IrHost) {
        let client = HostClient::from_ir(ir_host);
        self.clients.insert(ir_host.name.clone(), client);
    }

    /// Execute a prompt on a named host.
    pub fn execute(&mut self, name: &str, prompt: &str, context: Option<&Value>) -> Result<String> {
        let client = self
            .clients
            .get_mut(name)
            .ok_or_else(|| RuntimeError::CallError(format!("Host '{}' not registered", name)))?;
        client.execute(prompt, context)
    }

    /// Get a mutable reference to a named host client.
    pub fn get_client_mut(&mut self, name: &str) -> Result<&mut HostClient> {
        self.clients
            .get_mut(name)
            .ok_or_else(|| RuntimeError::CallError(format!("Host '{}' not registered", name)))
    }

    /// Check if a host is registered.
    pub fn has_host(&self, name: &str) -> bool {
        self.clients.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_format_from_str() {
        assert_eq!(HostFormat::from_str("text"), HostFormat::Text);
        assert_eq!(HostFormat::from_str("json"), HostFormat::Json);
        assert_eq!(HostFormat::from_str("other"), HostFormat::Text);
    }

    #[test]
    fn host_registry_empty() {
        let registry = HostRegistry::new();
        assert!(!registry.has_host("nonexistent"));
    }

    #[test]
    fn host_registry_register() {
        let mut registry = HostRegistry::new();
        let ir_host = IrHost {
            name: "TestHost".to_string(),
            connector: "test".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(60),
            decorators: vec![],
            command: Some("echo".to_string()),
            args: Some(vec!["hello".to_string()]),
            env: None,
            working_dir: None,
        };
        registry.register(&ir_host);
        assert!(registry.has_host("TestHost"));
        assert!(!registry.has_host("Other"));
    }

    #[test]
    fn host_execute_echo() {
        let mut registry = HostRegistry::new();
        let ir_host = IrHost {
            name: "EchoHost".to_string(),
            connector: "echo".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(5),
            decorators: vec![],
            command: Some("echo".to_string()),
            args: Some(vec!["hello world".to_string()]),
            env: None,
            working_dir: None,
        };
        registry.register(&ir_host);

        // `echo` just outputs its args and exits, so we read one line
        let result = registry.execute("EchoHost", "ignored", None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn host_get_client_mut() {
        let mut registry = HostRegistry::new();
        let ir_host = IrHost {
            name: "TestHost".to_string(),
            connector: "test".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(60),
            decorators: vec![],
            command: Some("echo".to_string()),
            args: Some(vec!["hello".to_string()]),
            env: None,
            working_dir: None,
        };
        registry.register(&ir_host);
        assert!(registry.get_client_mut("TestHost").is_ok());
        assert!(registry.get_client_mut("NonExistent").is_err());
    }

    #[test]
    fn host_read_message_ndjson() {
        // Use printf to output NDJSON lines, then EOF
        let ir_host = IrHost {
            name: "NdjsonHost".to_string(),
            connector: "test".to_string(),
            input_format: "text".to_string(),
            output_format: "json".to_string(),
            timeout: Some(5),
            decorators: vec![],
            command: Some("printf".to_string()),
            args: Some(vec![
                r#"{"type":"progress","message":"Working..."}\n{"type":"result","text":"Done"}\n"#
                    .to_string(),
            ]),
            env: None,
            working_dir: None,
        };
        let mut client = HostClient::from_ir(&ir_host);
        client.ensure_connected().unwrap();

        // First message: progress
        let msg1 = client.read_message().unwrap().unwrap();
        assert_eq!(msg1["type"], "progress");
        assert_eq!(msg1["message"], "Working...");

        // Second message: result
        let msg2 = client.read_message().unwrap().unwrap();
        assert_eq!(msg2["type"], "result");
        assert_eq!(msg2["text"], "Done");

        // EOF
        let msg3 = client.read_message().unwrap();
        assert!(msg3.is_none());
    }

    #[test]
    fn host_read_message_plain_text_fallback() {
        // Non-JSON lines should be wrapped as {"type": "result", "text": "..."}
        let ir_host = IrHost {
            name: "PlainHost".to_string(),
            connector: "test".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(5),
            decorators: vec![],
            command: Some("echo".to_string()),
            args: Some(vec!["plain text output".to_string()]),
            env: None,
            working_dir: None,
        };
        let mut client = HostClient::from_ir(&ir_host);
        client.ensure_connected().unwrap();

        let msg = client.read_message().unwrap().unwrap();
        assert_eq!(msg["type"], "result");
        assert_eq!(msg["text"], "plain text output");
    }

    #[test]
    fn host_write_response_format() {
        // Use `cat` as a pass-through host to verify response format
        let ir_host = IrHost {
            name: "CatHost".to_string(),
            connector: "test".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(5),
            decorators: vec![],
            command: Some("cat".to_string()),
            args: None,
            env: None,
            working_dir: None,
        };
        let mut client = HostClient::from_ir(&ir_host);
        client.ensure_connected().unwrap();

        // Write a response — it should be JSON serialized
        let response = serde_json::json!({
            "type": "response",
            "in_reply_to": "question",
            "value": "RS256"
        });
        let result = client.write_response(&response);
        assert!(result.is_ok());

        // cat echoes it back, so we can read it as a message
        let echoed = client.read_message().unwrap().unwrap();
        assert_eq!(echoed["type"], "response");
        assert_eq!(echoed["in_reply_to"], "question");
        assert_eq!(echoed["value"], "RS256");
    }
}
