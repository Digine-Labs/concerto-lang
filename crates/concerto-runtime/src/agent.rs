use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use concerto_common::ir::IrAgent;

use crate::error::{Result, RuntimeError};
use crate::value::Value;

/// Format for agent I/O.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentFormat {
    Text,
    Json,
}

impl AgentFormat {
    fn from_str(s: &str) -> Self {
        match s {
            "json" => AgentFormat::Json,
            _ => AgentFormat::Text,
        }
    }
}

/// A client that manages communication with an external agent system via stdio.
pub struct AgentClient {
    name: String,
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    working_dir: Option<String>,
    input_format: AgentFormat,
    output_format: AgentFormat,
    timeout: Duration,
    child: Option<Child>,
    /// Persistent buffered reader for streaming (taken from child.stdout).
    stdout_reader: Option<BufReader<ChildStdout>>,
    /// Initialization params from [agents.<name>.params] in Concerto.toml.
    params: Option<serde_json::Value>,
}

impl std::fmt::Debug for AgentClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentClient")
            .field("name", &self.name)
            .field("command", &self.command)
            .field("input_format", &self.input_format)
            .field("output_format", &self.output_format)
            .field("timeout", &self.timeout)
            .field("connected", &self.child.is_some())
            .finish()
    }
}

impl AgentClient {
    /// Create from an IrAgent (which has embedded TOML config).
    pub fn from_ir(ir_agent: &IrAgent) -> Self {
        let timeout_secs = ir_agent.timeout.unwrap_or(120);
        Self {
            name: ir_agent.name.clone(),
            command: ir_agent.command.clone().unwrap_or_default(),
            args: ir_agent.args.clone().unwrap_or_default(),
            env: ir_agent.env.clone().unwrap_or_default(),
            working_dir: ir_agent.working_dir.clone(),
            input_format: AgentFormat::from_str(&ir_agent.input_format),
            output_format: AgentFormat::from_str(&ir_agent.output_format),
            timeout: Duration::from_secs(timeout_secs as u64),
            child: None,
            stdout_reader: None,
            params: ir_agent.params.clone(),
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
                "Agent '{}' has no command configured (check Concerto.toml [agents.{}])",
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
                "Failed to spawn agent '{}' (command: {}): {}",
                self.name, self.command, e
            ))
        })?;

        // Take stdout for the persistent BufReader
        let stdout = child.stdout.take().ok_or_else(|| {
            RuntimeError::CallError(format!("Agent '{}' stdout not available", self.name))
        })?;
        self.stdout_reader = Some(BufReader::new(stdout));
        self.child = Some(child);

        // Send init message if params are configured
        if let Some(ref params) = self.params {
            self.send_init(params.clone())?;
        }

        Ok(())
    }

    /// Send init message with params and wait for init_ack.
    fn send_init(&mut self, params: serde_json::Value) -> Result<()> {
        let init_msg = serde_json::json!({
            "type": "init",
            "params": params
        });

        // Write init message as first NDJSON line
        let child = self.child.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("Agent '{}' not connected for init", self.name))
        })?;
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("Agent '{}' stdin not available for init", self.name))
        })?;
        let line = format!("{}\n", init_msg);
        stdin.write_all(line.as_bytes()).map_err(|e| {
            RuntimeError::CallError(format!("Failed to write init to agent '{}': {}", self.name, e))
        })?;
        stdin.flush().map_err(|e| {
            RuntimeError::CallError(format!(
                "Failed to flush init to agent '{}': {}",
                self.name, e
            ))
        })?;

        // Read init_ack response (with timeout)
        let response_line = self.read_line_with_timeout().map_err(|e| {
            RuntimeError::CallError(format!(
                "Agent '{}' failed to read init_ack: {}",
                self.name, e
            ))
        })?;

        if response_line.is_empty() {
            return Err(RuntimeError::CallError(format!(
                "Agent '{}' exited before acknowledging initialization",
                self.name
            )));
        }

        let parsed: serde_json::Value =
            serde_json::from_str(response_line.trim_end()).map_err(|e| {
                RuntimeError::CallError(format!(
                    "Agent '{}' invalid init response: {}",
                    self.name, e
                ))
            })?;

        match parsed.get("type").and_then(|t| t.as_str()) {
            Some("init_ack") => Ok(()),
            Some("error") => {
                let msg = parsed
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown error");
                Err(RuntimeError::CallError(format!(
                    "Agent '{}' init failed: {}",
                    self.name, msg
                )))
            }
            _ => Err(RuntimeError::CallError(format!(
                "Agent '{}' did not acknowledge initialization",
                self.name
            ))),
        }
    }

    /// Execute a prompt on the agent and return the response.
    pub fn execute(&mut self, prompt: &str, context: Option<&Value>) -> Result<String> {
        self.ensure_connected()?;

        let child = self.child.as_mut().unwrap();
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("Agent '{}' stdin not available", self.name))
        })?;

        // Write input
        let input = match self.input_format {
            AgentFormat::Text => {
                if let Some(ctx) = context {
                    format!("{}\nContext: {}\n", prompt, ctx)
                } else {
                    format!("{}\n", prompt)
                }
            }
            AgentFormat::Json => {
                let mut payload = serde_json::json!({ "prompt": prompt });
                if let Some(ctx) = context {
                    payload["context"] = ctx.to_json();
                }
                format!("{}\n", payload)
            }
        };

        stdin.write_all(input.as_bytes()).map_err(|e| {
            RuntimeError::CallError(format!("Failed to write to agent '{}': {}", self.name, e))
        })?;
        stdin.flush().map_err(|e| {
            RuntimeError::CallError(format!("Failed to flush agent '{}' stdin: {}", self.name, e))
        })?;

        // Read output (one line, with timeout)
        let line = self.read_line_with_timeout()?;

        if line.is_empty() {
            self.child = None;
            return Err(RuntimeError::CallError(format!(
                "Agent '{}' process exited unexpectedly",
                self.name
            )));
        }

        let response = line.trim_end().to_string();

        // Parse based on output format
        match self.output_format {
            AgentFormat::Text => Ok(response),
            AgentFormat::Json => {
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
            RuntimeError::CallError(format!("Agent '{}' stdin not available", self.name))
        })?;

        let input = match self.input_format {
            AgentFormat::Text => {
                if let Some(ctx) = context {
                    format!("{}\nContext: {}\n", prompt, ctx)
                } else {
                    format!("{}\n", prompt)
                }
            }
            AgentFormat::Json => {
                let mut payload = serde_json::json!({ "prompt": prompt });
                if let Some(ctx) = context {
                    payload["context"] = ctx.to_json();
                }
                format!("{}\n", payload)
            }
        };

        stdin.write_all(input.as_bytes()).map_err(|e| {
            RuntimeError::CallError(format!("Failed to write to agent '{}': {}", self.name, e))
        })?;
        stdin.flush().map_err(|e| {
            RuntimeError::CallError(format!("Failed to flush agent '{}' stdin: {}", self.name, e))
        })?;

        Ok(())
    }

    /// Read one NDJSON message from agent stdout.
    /// Returns None on EOF (agent exited).
    /// Non-JSON lines are wrapped as `{"type": "result", "text": "<line>"}`.
    pub fn read_message(&mut self) -> Result<Option<serde_json::Value>> {
        loop {
            let line = match self.read_line_with_timeout() {
                Ok(line) => line,
                Err(e) => {
                    // Check if it's a timeout or disconnect
                    self.child = None;
                    self.stdout_reader = None;
                    return Err(e);
                }
            };

            if line.is_empty() {
                // EOF
                self.child = None;
                self.stdout_reader = None;
                return Ok(None);
            }

            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                // Skip empty lines, try next
                continue;
            }
            // Try parsing as JSON
            return match serde_json::from_str::<serde_json::Value>(trimmed) {
                Ok(json) if json.is_object() && json.get("type").is_some() => Ok(Some(json)),
                _ => {
                    // Non-JSON or no "type" field: wrap as result
                    Ok(Some(serde_json::json!({
                        "type": "result",
                        "text": trimmed
                    })))
                }
            };
        }
    }

    /// Write a response JSON line to agent stdin.
    pub fn write_response(&mut self, response: &serde_json::Value) -> Result<()> {
        let child = self.child.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("Agent '{}' not connected", self.name))
        })?;

        let stdin = child.stdin.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("Agent '{}' stdin not available", self.name))
        })?;

        let line = format!("{}\n", serde_json::to_string(response).unwrap_or_default());
        stdin.write_all(line.as_bytes()).map_err(|e| {
            RuntimeError::CallError(format!("Failed to write response to agent '{}': {}", self.name, e))
        })?;
        stdin.flush().map_err(|e| {
            RuntimeError::CallError(format!("Failed to flush agent '{}' stdin: {}", self.name, e))
        })?;

        Ok(())
    }

    /// Read a line from stdout with timeout enforcement.
    /// Takes ownership of the BufReader temporarily to move into thread.
    fn read_line_with_timeout(&mut self) -> Result<String> {
        let mut reader = self.stdout_reader.take().ok_or_else(|| {
            RuntimeError::CallError(format!("Agent '{}' stdout not available", self.name))
        })?;
        let timeout = self.timeout;
        let agent_name = self.name.clone();

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = reader.read_line(&mut line);
            // Send both the result and the reader back
            let _ = tx.send((result, line, reader));
        });

        match rx.recv_timeout(timeout) {
            Ok((read_result, line, reader)) => {
                // Restore the reader
                self.stdout_reader = Some(reader);
                read_result.map_err(|e| {
                    RuntimeError::CallError(format!("Agent '{}' read error: {}", agent_name, e))
                })?;
                Ok(line)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Timeout — kill the child process
                if let Some(ref mut child) = self.child {
                    let _ = child.kill();
                }
                self.child = None;
                // stdout_reader is already None (taken above), leave it
                Err(RuntimeError::CallError(format!(
                    "Agent '{}' timed out after {}s",
                    agent_name,
                    timeout.as_secs()
                )))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.child = None;
                Err(RuntimeError::CallError(format!(
                    "Agent '{}' read thread disconnected",
                    agent_name
                )))
            }
        }
    }

    /// Shutdown the agent process.
    pub fn shutdown(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;
        self.stdout_reader = None;
    }
}

impl Drop for AgentClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Registry that manages named agent clients.
#[derive(Debug, Default)]
pub struct AgentRegistry {
    clients: HashMap<String, AgentClient>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    /// Register an agent from an IrAgent (which contains embedded TOML config).
    pub fn register(&mut self, ir_agent: &IrAgent) {
        let client = AgentClient::from_ir(ir_agent);
        self.clients.insert(ir_agent.name.clone(), client);
    }

    /// Execute a prompt on a named agent.
    pub fn execute(&mut self, name: &str, prompt: &str, context: Option<&Value>) -> Result<String> {
        let client = self
            .clients
            .get_mut(name)
            .ok_or_else(|| RuntimeError::CallError(format!("Agent '{}' not registered", name)))?;
        client.execute(prompt, context)
    }

    /// Get a mutable reference to a named agent client.
    pub fn get_client_mut(&mut self, name: &str) -> Result<&mut AgentClient> {
        self.clients
            .get_mut(name)
            .ok_or_else(|| RuntimeError::CallError(format!("Agent '{}' not registered", name)))
    }

    /// Check if an agent is registered.
    pub fn has_agent(&self, name: &str) -> bool {
        self.clients.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_format_from_str() {
        assert_eq!(AgentFormat::from_str("text"), AgentFormat::Text);
        assert_eq!(AgentFormat::from_str("json"), AgentFormat::Json);
        assert_eq!(AgentFormat::from_str("other"), AgentFormat::Text);
    }

    #[test]
    fn agent_registry_empty() {
        let registry = AgentRegistry::new();
        assert!(!registry.has_agent("nonexistent"));
    }

    #[test]
    fn agent_registry_register() {
        let mut registry = AgentRegistry::new();
        let ir_agent = IrAgent {
            name: "TestAgent".to_string(),
            connector: "test".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(60),
            decorators: vec![],
            command: Some("echo".to_string()),
            args: Some(vec!["hello".to_string()]),
            env: None,
            working_dir: None,
            params: None,
        };
        registry.register(&ir_agent);
        assert!(registry.has_agent("TestAgent"));
        assert!(!registry.has_agent("Other"));
    }

    #[test]
    fn agent_execute_echo() {
        let mut registry = AgentRegistry::new();
        let ir_agent = IrAgent {
            name: "EchoAgent".to_string(),
            connector: "echo".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(5),
            decorators: vec![],
            command: Some("echo".to_string()),
            args: Some(vec!["hello world".to_string()]),
            env: None,
            working_dir: None,
            params: None,
        };
        registry.register(&ir_agent);

        // `echo` just outputs its args and exits, so we read one line
        let result = registry.execute("EchoAgent", "ignored", None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn agent_get_client_mut() {
        let mut registry = AgentRegistry::new();
        let ir_agent = IrAgent {
            name: "TestAgent".to_string(),
            connector: "test".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(60),
            decorators: vec![],
            command: Some("echo".to_string()),
            args: Some(vec!["hello".to_string()]),
            env: None,
            working_dir: None,
            params: None,
        };
        registry.register(&ir_agent);
        assert!(registry.get_client_mut("TestAgent").is_ok());
        assert!(registry.get_client_mut("NonExistent").is_err());
    }

    #[test]
    fn agent_read_message_ndjson() {
        // Use printf to output NDJSON lines, then EOF
        let ir_agent = IrAgent {
            name: "NdjsonAgent".to_string(),
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
            params: None,
        };
        let mut client = AgentClient::from_ir(&ir_agent);
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
    fn agent_init_sends_params_and_receives_ack() {
        // Use bash to read init message, echo init_ack, then echo result for execute
        let ir_agent = IrAgent {
            name: "InitAgent".to_string(),
            connector: "test".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(5),
            decorators: vec![],
            command: Some("bash".to_string()),
            args: Some(vec![
                "-c".to_string(),
                // Read init line, respond with init_ack, read prompt, echo response
                r#"read init_line; echo '{"type":"init_ack"}'; read prompt; echo "response_text""#.to_string(),
            ]),
            env: None,
            working_dir: None,
            params: Some(serde_json::json!({"model": "gpt-4o", "temperature": 0.5})),
        };
        let mut client = AgentClient::from_ir(&ir_agent);
        // ensure_connected should send init and get init_ack
        let result = client.ensure_connected();
        assert!(result.is_ok(), "init handshake failed: {:?}", result.err());

        // Now execute should work normally
        let exec_result = client.execute("hello", None);
        assert!(exec_result.is_ok(), "execute failed: {:?}", exec_result.err());
        assert_eq!(exec_result.unwrap(), "response_text");
    }

    #[test]
    fn agent_init_error_propagates() {
        // Agent responds with error instead of init_ack
        let ir_agent = IrAgent {
            name: "FailAgent".to_string(),
            connector: "test".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(5),
            decorators: vec![],
            command: Some("bash".to_string()),
            args: Some(vec![
                "-c".to_string(),
                r#"read init_line; echo '{"type":"error","message":"bad config"}'"#.to_string(),
            ]),
            env: None,
            working_dir: None,
            params: Some(serde_json::json!({"invalid": true})),
        };
        let mut client = AgentClient::from_ir(&ir_agent);
        let result = client.ensure_connected();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("bad config"), "got: {}", err_msg);
    }

    #[test]
    fn agent_no_params_skips_init() {
        // Agent without params should NOT send init message
        // Use echo which outputs immediately and exits — if init were sent, it would fail
        let ir_agent = IrAgent {
            name: "NoInitAgent".to_string(),
            connector: "test".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(5),
            decorators: vec![],
            command: Some("echo".to_string()),
            args: Some(vec!["hello".to_string()]),
            env: None,
            working_dir: None,
            params: None,
        };
        let mut client = AgentClient::from_ir(&ir_agent);
        assert!(client.params.is_none());
        // This should succeed without any init handshake
        let result = client.ensure_connected();
        assert!(result.is_ok());
    }

    #[test]
    fn agent_read_message_plain_text_fallback() {
        // Non-JSON lines should be wrapped as {"type": "result", "text": "..."}
        let ir_agent = IrAgent {
            name: "PlainAgent".to_string(),
            connector: "test".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(5),
            decorators: vec![],
            command: Some("echo".to_string()),
            args: Some(vec!["plain text output".to_string()]),
            env: None,
            working_dir: None,
            params: None,
        };
        let mut client = AgentClient::from_ir(&ir_agent);
        client.ensure_connected().unwrap();

        let msg = client.read_message().unwrap().unwrap();
        assert_eq!(msg["type"], "result");
        assert_eq!(msg["text"], "plain text output");
    }

    #[test]
    fn agent_write_response_format() {
        // Use `cat` as a pass-through agent to verify response format
        let ir_agent = IrAgent {
            name: "CatAgent".to_string(),
            connector: "test".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(5),
            decorators: vec![],
            command: Some("cat".to_string()),
            args: None,
            env: None,
            working_dir: None,
            params: None,
        };
        let mut client = AgentClient::from_ir(&ir_agent);
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

    #[test]
    fn agent_execute_timeout_enforced() {
        // Bug: timeout was stored but never enforced.
        // Agent sleeps 3s but timeout is 1s — should error before 3s.
        let ir_agent = IrAgent {
            name: "SlowAgent".to_string(),
            connector: "slow".to_string(),
            input_format: "text".to_string(),
            output_format: "text".to_string(),
            timeout: Some(1),
            decorators: vec![],
            command: Some("sleep".to_string()),
            args: Some(vec!["10".to_string()]),
            env: None,
            working_dir: None,
            params: None,
        };
        let mut client = AgentClient::from_ir(&ir_agent);
        client.ensure_connected().unwrap();

        let start = std::time::Instant::now();
        let result = client.read_line_with_timeout();
        let elapsed = start.elapsed();

        assert!(result.is_err(), "should timeout");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("timed out"), "error should mention timeout, got: {}", err);
        // Should complete well under 3s (timeout is 1s)
        assert!(elapsed.as_secs() < 3, "elapsed {}s, expected ~1s", elapsed.as_secs());
    }
}
