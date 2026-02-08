use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The parsed Concerto.toml manifest.
#[derive(Debug, Clone)]
pub struct ConcertoManifest {
    pub project: ProjectSection,
    pub connections: HashMap<String, ConnectionConfig>,
    pub mcp: HashMap<String, McpConfig>,
    /// The directory containing the Concerto.toml file.
    pub root_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSection {
    pub name: String,
    pub version: String,
    pub entry: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionConfig {
    pub provider: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub timeout: Option<u32>,
    #[serde(default)]
    pub organization: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub retry: Option<RetryConfig>,
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,
    #[serde(default)]
    pub models: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RetryConfig {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_backoff")]
    pub backoff: String,
    #[serde(default = "default_initial_delay")]
    pub initial_delay_ms: u32,
    #[serde(default = "default_max_delay")]
    pub max_delay_ms: u32,
}

fn default_max_attempts() -> u32 {
    1
}
fn default_backoff() -> String {
    "none".to_string()
}
fn default_initial_delay() -> u32 {
    1000
}
fn default_max_delay() -> u32 {
    30000
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default)]
    pub requests_per_minute: Option<u32>,
    #[serde(default)]
    pub tokens_per_minute: Option<u32>,
    #[serde(default)]
    pub concurrent_requests: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpConfig {
    pub transport: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub timeout: Option<u32>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

/// Raw TOML structure for deserialization.
#[derive(Deserialize)]
struct RawManifest {
    project: ProjectSection,
    #[serde(default)]
    connections: HashMap<String, ConnectionConfig>,
    #[serde(default)]
    mcp: HashMap<String, McpConfig>,
}

/// Errors that can occur when loading a manifest.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("no Concerto.toml found (searched from {0})")]
    NotFound(String),
    #[error("failed to read Concerto.toml: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("invalid Concerto.toml: {0}")]
    ParseError(String),
    #[error("invalid Concerto.toml: missing required field 'provider' in [connections.{0}]")]
    MissingProvider(String),
    #[error("invalid Concerto.toml: connection '{0}' has provider '{1}' which requires 'api_key_env'")]
    MissingApiKeyEnv(String, String),
    #[error("invalid Concerto.toml: [mcp.{0}] transport 'stdio' requires 'command' field")]
    McpMissingCommand(String),
    #[error("invalid Concerto.toml: [mcp.{0}] transport 'sse' requires 'url' field")]
    McpMissingUrl(String),
    #[error("invalid Concerto.toml: [mcp.{0}] unknown transport '{1}' (expected 'stdio' or 'sse')")]
    McpUnknownTransport(String, String),
}

/// Walk up from `start_dir` looking for `Concerto.toml`.
/// Returns the path to the manifest file if found.
pub fn find_manifest(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir.to_path_buf();
    loop {
        let candidate = current.join("Concerto.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Load and validate a Concerto.toml manifest from a file path.
pub fn load_manifest(path: &Path) -> Result<ConcertoManifest, ManifestError> {
    let content = std::fs::read_to_string(path)?;
    let root_dir = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    parse_manifest(&content, root_dir)
}

/// Parse and validate a Concerto.toml manifest from a string.
pub fn parse_manifest(
    content: &str,
    root_dir: PathBuf,
) -> Result<ConcertoManifest, ManifestError> {
    let raw: RawManifest =
        toml::from_str(content).map_err(|e| ManifestError::ParseError(e.to_string()))?;

    // Validate connections
    for (name, conn) in &raw.connections {
        validate_connection(name, conn)?;
    }

    // Validate MCP configs
    for (name, mcp) in &raw.mcp {
        validate_mcp(name, mcp)?;
    }

    Ok(ConcertoManifest {
        project: raw.project,
        connections: raw.connections,
        mcp: raw.mcp,
        root_dir,
    })
}

/// Find and load the manifest starting from a source file's directory.
pub fn find_and_load_manifest(source_file: &Path) -> Result<ConcertoManifest, ManifestError> {
    let start_dir = source_file
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let manifest_path = find_manifest(start_dir)
        .ok_or_else(|| ManifestError::NotFound(start_dir.display().to_string()))?;
    load_manifest(&manifest_path)
}

fn validate_connection(name: &str, conn: &ConnectionConfig) -> Result<(), ManifestError> {
    // Validate provider-specific requirements
    let cloud_providers = ["openai", "anthropic", "google"];
    if cloud_providers.contains(&conn.provider.as_str()) && conn.api_key_env.is_none() {
        return Err(ManifestError::MissingApiKeyEnv(
            name.to_string(),
            conn.provider.clone(),
        ));
    }
    Ok(())
}

fn validate_mcp(name: &str, mcp: &McpConfig) -> Result<(), ManifestError> {
    match mcp.transport.as_str() {
        "stdio" => {
            if mcp.command.is_none() {
                return Err(ManifestError::McpMissingCommand(name.to_string()));
            }
        }
        "sse" => {
            if mcp.url.is_none() {
                return Err(ManifestError::McpMissingUrl(name.to_string()));
            }
        }
        other => {
            return Err(ManifestError::McpUnknownTransport(
                name.to_string(),
                other.to_string(),
            ));
        }
    }
    Ok(())
}

/// Convert a ConnectionConfig to the JSON config format used in IR.
impl ConnectionConfig {
    pub fn to_ir_config(&self) -> serde_json::Value {
        let mut config = serde_json::Map::new();
        config.insert(
            "provider".to_string(),
            serde_json::Value::String(self.provider.clone()),
        );
        if let Some(ref key_env) = self.api_key_env {
            config.insert(
                "api_key_env".to_string(),
                serde_json::Value::String(key_env.clone()),
            );
        }
        if let Some(ref url) = self.base_url {
            config.insert(
                "base_url".to_string(),
                serde_json::Value::String(url.clone()),
            );
        }
        if let Some(ref model) = self.default_model {
            config.insert(
                "default_model".to_string(),
                serde_json::Value::String(model.clone()),
            );
        }
        if let Some(timeout) = self.timeout {
            config.insert(
                "timeout".to_string(),
                serde_json::Value::Number(serde_json::Number::from(timeout)),
            );
        }
        if let Some(ref models) = self.models {
            let models_obj: serde_json::Map<String, serde_json::Value> = models
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            config.insert(
                "models".to_string(),
                serde_json::Value::Object(models_obj),
            );
        }
        if let Some(ref retry) = self.retry {
            let mut retry_obj = serde_json::Map::new();
            retry_obj.insert(
                "max_attempts".to_string(),
                serde_json::Value::Number(serde_json::Number::from(retry.max_attempts)),
            );
            retry_obj.insert(
                "backoff".to_string(),
                serde_json::Value::String(retry.backoff.clone()),
            );
            config.insert(
                "retry".to_string(),
                serde_json::Value::Object(retry_obj),
            );
        }
        serde_json::Value::Object(config)
    }
}

/// Convert an McpConfig to the JSON config format used in IR.
impl McpConfig {
    pub fn to_ir_config(&self) -> serde_json::Value {
        let mut config = serde_json::Map::new();
        config.insert(
            "type".to_string(),
            serde_json::Value::String("mcp".to_string()),
        );
        config.insert(
            "transport".to_string(),
            serde_json::Value::String(self.transport.clone()),
        );
        if let Some(ref cmd) = self.command {
            config.insert(
                "command".to_string(),
                serde_json::Value::String(cmd.clone()),
            );
        }
        if let Some(ref url) = self.url {
            config.insert(
                "url".to_string(),
                serde_json::Value::String(url.clone()),
            );
        }
        if let Some(timeout) = self.timeout {
            config.insert(
                "timeout".to_string(),
                serde_json::Value::Number(serde_json::Number::from(timeout)),
            );
        }
        serde_json::Value::Object(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_manifest() {
        let toml = r#"
[project]
name = "test-project"
version = "0.1.0"
entry = "src/main.conc"
"#;
        let manifest = parse_manifest(toml, PathBuf::from(".")).unwrap();
        assert_eq!(manifest.project.name, "test-project");
        assert_eq!(manifest.project.version, "0.1.0");
        assert_eq!(manifest.project.entry, "src/main.conc");
        assert!(manifest.connections.is_empty());
        assert!(manifest.mcp.is_empty());
    }

    #[test]
    fn parse_full_manifest() {
        let toml = r#"
[project]
name = "my-agent"
version = "1.0.0"
entry = "src/main.conc"

[connections.openai]
provider = "openai"
api_key_env = "OPENAI_API_KEY"
default_model = "gpt-4o"
timeout = 60

[connections.openai.retry]
max_attempts = 3
backoff = "exponential"

[connections.openai.models]
fast = "gpt-4o-mini"
smart = "gpt-4o"

[connections.anthropic]
provider = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
default_model = "claude-sonnet-4-20250514"

[connections.local]
provider = "ollama"
base_url = "http://localhost:11434/v1"
default_model = "llama3.1"

[mcp.github]
transport = "stdio"
command = "npx -y @modelcontextprotocol/server-github"

[mcp.github.env]
GITHUB_TOKEN_ENV = "GITHUB_TOKEN"

[mcp.web_search]
transport = "sse"
url = "http://localhost:3000/mcp"
timeout = 30
"#;
        let manifest = parse_manifest(toml, PathBuf::from("/project")).unwrap();
        assert_eq!(manifest.project.name, "my-agent");
        assert_eq!(manifest.connections.len(), 3);
        assert_eq!(manifest.mcp.len(), 2);

        // Check openai connection
        let openai = &manifest.connections["openai"];
        assert_eq!(openai.provider, "openai");
        assert_eq!(openai.api_key_env.as_deref(), Some("OPENAI_API_KEY"));
        assert_eq!(openai.default_model.as_deref(), Some("gpt-4o"));
        assert_eq!(openai.timeout, Some(60));
        assert!(openai.retry.is_some());
        let retry = openai.retry.as_ref().unwrap();
        assert_eq!(retry.max_attempts, 3);
        assert_eq!(retry.backoff, "exponential");
        assert!(openai.models.is_some());
        let models = openai.models.as_ref().unwrap();
        assert_eq!(models["fast"], "gpt-4o-mini");
        assert_eq!(models["smart"], "gpt-4o");

        // Check local connection (no api_key_env required)
        let local = &manifest.connections["local"];
        assert_eq!(local.provider, "ollama");
        assert!(local.api_key_env.is_none());

        // Check MCP configs
        let github = &manifest.mcp["github"];
        assert_eq!(github.transport, "stdio");
        assert_eq!(github.command.as_deref(), Some("npx -y @modelcontextprotocol/server-github"));
        assert!(github.env.is_some());

        let web = &manifest.mcp["web_search"];
        assert_eq!(web.transport, "sse");
        assert_eq!(web.url.as_deref(), Some("http://localhost:3000/mcp"));
        assert_eq!(web.timeout, Some(30));
    }

    #[test]
    fn missing_project_section_fails() {
        let toml = r#"
[connections.openai]
provider = "openai"
api_key_env = "KEY"
"#;
        let result = parse_manifest(toml, PathBuf::from("."));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid Concerto.toml"), "got: {}", err);
    }

    #[test]
    fn cloud_provider_missing_api_key_env() {
        let toml = r#"
[project]
name = "test"
version = "0.1.0"
entry = "src/main.conc"

[connections.openai]
provider = "openai"
default_model = "gpt-4o"
"#;
        let result = parse_manifest(toml, PathBuf::from("."));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("api_key_env"), "got: {}", err);
    }

    #[test]
    fn ollama_no_api_key_ok() {
        let toml = r#"
[project]
name = "test"
version = "0.1.0"
entry = "src/main.conc"

[connections.local]
provider = "ollama"
base_url = "http://localhost:11434/v1"
default_model = "llama3.1"
"#;
        let manifest = parse_manifest(toml, PathBuf::from(".")).unwrap();
        assert_eq!(manifest.connections["local"].provider, "ollama");
    }

    #[test]
    fn mcp_stdio_missing_command() {
        let toml = r#"
[project]
name = "test"
version = "0.1.0"
entry = "src/main.conc"

[mcp.github]
transport = "stdio"
"#;
        let result = parse_manifest(toml, PathBuf::from("."));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("command"), "got: {}", err);
    }

    #[test]
    fn mcp_sse_missing_url() {
        let toml = r#"
[project]
name = "test"
version = "0.1.0"
entry = "src/main.conc"

[mcp.web]
transport = "sse"
"#;
        let result = parse_manifest(toml, PathBuf::from("."));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("url"), "got: {}", err);
    }

    #[test]
    fn mcp_unknown_transport() {
        let toml = r#"
[project]
name = "test"
version = "0.1.0"
entry = "src/main.conc"

[mcp.bad]
transport = "websocket"
"#;
        let result = parse_manifest(toml, PathBuf::from("."));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("websocket"), "got: {}", err);
    }

    #[test]
    fn connection_to_ir_config() {
        let conn = ConnectionConfig {
            provider: "openai".to_string(),
            api_key_env: Some("OPENAI_API_KEY".to_string()),
            base_url: None,
            default_model: Some("gpt-4o".to_string()),
            timeout: Some(60),
            organization: None,
            project: None,
            retry: None,
            rate_limit: None,
            models: None,
        };
        let config = conn.to_ir_config();
        let obj = config.as_object().unwrap();
        assert_eq!(obj["provider"], "openai");
        assert_eq!(obj["api_key_env"], "OPENAI_API_KEY");
        assert_eq!(obj["default_model"], "gpt-4o");
        assert_eq!(obj["timeout"], 60);
    }

    #[test]
    fn find_manifest_walks_up() {
        // Create a temp directory structure
        let tmp = std::env::temp_dir().join("concerto_test_manifest");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("src/nested")).unwrap();
        std::fs::write(
            tmp.join("Concerto.toml"),
            "[project]\nname = \"test\"\nversion = \"0.1.0\"\nentry = \"src/main.conc\"\n",
        )
        .unwrap();

        // Search from nested subdir should find manifest at root
        let found = find_manifest(&tmp.join("src/nested"));
        assert!(found.is_some());
        assert_eq!(found.unwrap(), tmp.join("Concerto.toml"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
