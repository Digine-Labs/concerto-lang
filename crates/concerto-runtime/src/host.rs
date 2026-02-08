use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
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
        }
    }

    /// Ensure the subprocess is running. Spawns if not connected.
    fn ensure_connected(&mut self) -> Result<()> {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_)) => self.child = None, // exited, need respawn
                Ok(None) => return Ok(()),        // still running
                Err(_) => self.child = None,
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

        let child = cmd.spawn().map_err(|e| {
            RuntimeError::CallError(format!(
                "Failed to spawn host '{}' (command: {}): {}",
                self.name, self.command, e
            ))
        })?;

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
        let stdout = child.stdout.as_mut().ok_or_else(|| {
            RuntimeError::CallError(format!("Host '{}' stdout not available", self.name))
        })?;

        let mut reader = BufReader::new(stdout);
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

    /// Shutdown the host process.
    pub fn shutdown(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;
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
}
