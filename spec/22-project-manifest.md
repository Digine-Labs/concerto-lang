# 22 - Project Manifest (Concerto.toml)

## Overview

Every Concerto project has a **Concerto.toml** manifest file at its root. This file declares project metadata, LLM provider connections, and MCP server configurations. It replaces inline `connect` blocks in `.conc` source files, cleanly separating infrastructure concerns (API keys, endpoints, models) from application logic (agents, tools, pipelines).

## Motivation

Inline `connect` blocks mix deployment configuration with program logic:

```concerto
// BAD: infrastructure details pollute source code
connect openai {
    api_key: env("OPENAI_API_KEY"),
    default_model: "gpt-4o-mini",
}
```

With `Concerto.toml`, source files contain only orchestration logic. Connections are resolved by name at compile time from the manifest. The same `.conc` program can target different providers in dev, staging, and production by swapping the manifest.

## File Location and Discovery

The compiler and runtime locate `Concerto.toml` by searching upward from the source file's directory:

```
my-project/
  Concerto.toml         <-- found here
  src/
    main.conc           <-- compiler starts here, walks up
    utils/
      helpers.conc
```

If no `Concerto.toml` is found, the compiler emits an error:

```
error: no Concerto.toml found
  = help: run 'concerto init' to create a project, or create Concerto.toml manually
```

## Manifest Structure

### [project] Section (Required)

Project metadata. Every manifest must have this section.

```toml
[project]
name = "my-agent-harness"       # Project name (kebab-case recommended)
version = "0.1.0"               # Semantic version
entry = "src/main.conc"         # Main source file (relative to manifest)
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | String | Yes | Project name |
| `version` | String | Yes | Semantic version string |
| `entry` | String | Yes | Path to main `.conc` source file |

### [connections.*] Sections (Optional)

LLM provider connections. Each connection has a unique name used as the TOML key.

```toml
[connections.openai]
provider = "openai"
api_key_env = "OPENAI_API_KEY"
default_model = "gpt-4o"

[connections.anthropic]
provider = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
default_model = "claude-sonnet-4-20250514"

[connections.local]
provider = "ollama"
base_url = "http://localhost:11434/v1"
default_model = "llama3.1"
# no api_key_env needed for local providers
```

#### Connection Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `provider` | String | Yes | -- | Provider type: `"openai"`, `"anthropic"`, `"google"`, `"ollama"`, `"custom"` |
| `api_key_env` | String | No* | -- | Name of the environment variable holding the API key |
| `base_url` | String | No | Provider default | API endpoint URL |
| `default_model` | String | No | Provider default | Default model for agents using this connection |
| `timeout` | Integer | No | 30 | Request timeout in seconds |
| `organization` | String | No | -- | Organization ID (OpenAI) |
| `project` | String | No | -- | Project ID (OpenAI) |

\* Required for cloud providers (openai, anthropic, google). Not required for local (ollama) or custom providers with no auth.

**Security**: `api_key_env` stores the *name* of the environment variable, not the secret itself. The runtime reads the actual key from the environment at execution time. Secrets never appear in the manifest or IR.

#### Retry Configuration

```toml
[connections.openai.retry]
max_attempts = 3                # Total attempts including initial (default: 1 = no retry)
backoff = "exponential"         # "none", "linear", "exponential" (default: "none")
initial_delay_ms = 1000         # Delay before first retry (default: 1000)
max_delay_ms = 30000            # Maximum delay cap (default: 30000)
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_attempts` | Integer | 1 | Total attempts (1 = no retry) |
| `backoff` | String | `"none"` | Backoff strategy |
| `initial_delay_ms` | Integer | 1000 | Initial delay in milliseconds |
| `max_delay_ms` | Integer | 30000 | Maximum delay cap |

#### Rate Limit Configuration

```toml
[connections.openai.rate_limit]
requests_per_minute = 60
tokens_per_minute = 150000
concurrent_requests = 10
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `requests_per_minute` | Integer | unlimited | Max requests per minute |
| `tokens_per_minute` | Integer | unlimited | Max tokens per minute |
| `concurrent_requests` | Integer | unlimited | Max concurrent in-flight requests |

#### Model Aliasing

Map friendly names to model identifiers, enabling easy switching:

```toml
[connections.openai.models]
fast = "gpt-4o-mini"
smart = "gpt-4o"
reasoning = "o1"
```

When an agent specifies `model: "fast"`, the runtime resolves it to `"gpt-4o-mini"` via this mapping.

### [mcp.*] Sections (Optional)

MCP server connection configurations. The name must match an `mcp` block name in source code.

```toml
[mcp.github]
transport = "stdio"
command = "npx -y @modelcontextprotocol/server-github"

[mcp.github.env]
GITHUB_TOKEN_ENV = "GITHUB_TOKEN"

[mcp.web_search]
transport = "sse"
url = "http://localhost:3000/mcp"
timeout = 30
```

#### MCP Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `transport` | String | Yes | `"stdio"` or `"sse"` |
| `command` | String | For stdio | Shell command to launch the server |
| `url` | String | For SSE | Server URL |
| `timeout` | Integer | No | Connection timeout in seconds (default: 30) |

#### MCP Environment Variables

```toml
[mcp.github.env]
GITHUB_TOKEN_ENV = "GITHUB_TOKEN"   # Env var name passed to server process
```

Environment entries are passed to the MCP server process. Values are env var names (like `api_key_env`), resolved at runtime.

## Impact on Source Files

### connect Keyword Removed

The `connect` keyword is **removed** from the language grammar. Source files no longer contain connection declarations:

```concerto
// BEFORE (removed):
connect openai {
    api_key: env("OPENAI_API_KEY"),
    default_model: "gpt-4o",
}

// AFTER: nothing — connections come from Concerto.toml
```

### Agent provider: Field Unchanged

Agents still reference connections by name. The name resolves from `Concerto.toml` instead of a source-level `connect` block:

```concerto
agent Classifier {
    provider: openai,           // resolves from [connections.openai]
    model: "gpt-4o-mini",
    system_prompt: "You are a classifier.",
}
```

### MCP Blocks Keep Typed Interfaces Only

`mcp` blocks in source retain their typed function signatures (for compile-time type checking) but lose connection fields (`transport`, `command`, `url`). Those move to `Concerto.toml`:

```concerto
// BEFORE:
mcp GitHubServer {
    transport: "stdio",
    command: "npx -y @modelcontextprotocol/server-github",

    @describe("Create a GitHub issue")
    fn create_issue(owner: String, repo: String, title: String) -> Result<Issue, ToolError>;
}

// AFTER:
mcp GitHubServer {
    // Connection config in [mcp.GitHubServer] in Concerto.toml
    // Source only declares the typed tool interface

    @describe("Create a GitHub issue")
    fn create_issue(owner: String, repo: String, title: String) -> Result<Issue, ToolError>;
}
```

The mapping between source `mcp` block names and TOML `[mcp.*]` keys is by **exact name match** (case-sensitive).

## Compilation Pipeline Changes

### Compiler Reads Concerto.toml

The compiler (`concertoc`) now:

1. Locates `Concerto.toml` by walking up from the source file
2. Parses the TOML manifest
3. Validates `[project]` section (name, version, entry)
4. Validates `[connections.*]` sections (required fields per provider type)
5. Validates `[mcp.*]` sections (transport-specific required fields)
6. Resolves agent `provider:` names against known connections — error if not found
7. Resolves `mcp` block names against `[mcp.*]` sections — warning if no matching config (may be provided at runtime)

### IR Embedding

The compiler **embeds** resolved connection and MCP configs into the `.conc-ir` file. This makes the IR self-contained — the runtime does not need to find or parse `Concerto.toml`.

The existing IR `connections` section is populated from TOML instead of from `connect` blocks:

```json
{
    "connections": [
        {
            "name": "openai",
            "config": {
                "provider": "openai",
                "api_key_env": "OPENAI_API_KEY",
                "base_url": "https://api.openai.com/v1",
                "default_model": "gpt-4o",
                "timeout": 60,
                "retry": { "max_attempts": 3, "backoff": "exponential" },
                "models": { "fast": "gpt-4o-mini", "smart": "gpt-4o" }
            }
        }
    ]
}
```

MCP connection details from TOML are merged into the IR `mcp_connections` section alongside the typed tool interfaces from source.

### Semantic Analysis Changes

The resolver no longer registers `connect` block names in the scope. Instead, it loads connection names from the parsed `Concerto.toml` and registers those. Agent `provider:` validation checks against TOML connection names.

## Runtime Changes

The runtime is **unchanged** in how it reads the IR. The `connections` and `mcp_connections` sections in the IR have the same structure — the only difference is that they were populated from TOML at compile time rather than from `connect` blocks.

The runtime still supports `override_connection()` from the host API for testing and staging overrides.

## Error Messages

### Missing Concerto.toml

```
error: no Concerto.toml found
  --> src/main.conc
  = help: run 'concerto init' to create a project, or create Concerto.toml manually
```

### Invalid Concerto.toml

```
error: invalid Concerto.toml: missing required field 'provider' in [connections.openai]
  --> Concerto.toml
```

### Unknown Provider Reference

```
error: agent `Classifier` references unknown connection `openai`
  --> src/main.conc:5:15
   |
 5 |     provider: openai,
   |               ^^^^^^
   = help: add [connections.openai] to Concerto.toml
```

### MCP Name Mismatch (Warning)

```
warning: mcp block `GitHubServer` has no matching [mcp.GitHubServer] in Concerto.toml
  --> src/main.conc:10:5
   |
10 | mcp GitHubServer {
   |     ^^^^^^^^^^^^
   = help: add [mcp.GitHubServer] to Concerto.toml, or provide config at runtime
```

## Complete Example

### Concerto.toml

```toml
[project]
name = "document-processor"
version = "0.1.0"
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
```

### src/main.conc

```concerto
schema Classification {
    category: "legal" | "technical" | "financial" | "general",
    confidence: Float,
}

agent Classifier {
    provider: openai,
    model: "fast",              // Resolves to "gpt-4o-mini" via [connections.openai.models]
    temperature: 0.1,
    system_prompt: "Classify documents. Respond with valid JSON.",
}

agent Analyst {
    provider: anthropic,
    model: "claude-sonnet-4-20250514",
    temperature: 0.3,
    system_prompt: "Analyze documents in detail.",
}

fn main() {
    let doc = "Revenue increased 15% year-over-year to $2.3 billion.";
    let classification = Classifier.execute_with_schema<Classification>(doc)?;
    emit("result", classification);
}
```

## Example Project Structure

With `Concerto.toml`, each example becomes a self-contained project directory. The current flat layout:

```
examples/
  hello_agent.conc
  hello_agent.conc-ir
  multi_agent_pipeline.conc
  multi_agent_pipeline.conc-ir
  tool_usage.conc
  tool_usage.conc-ir
```

Is restructured into proper project directories:

```
examples/
  hello_agent/
    Concerto.toml
    src/
      main.conc
  multi_agent_pipeline/
    Concerto.toml
    src/
      main.conc
  tool_usage/
    Concerto.toml
    src/
      main.conc
```

Each example has its own `Concerto.toml` with the connections that example needs. The `.conc-ir` files are build artifacts, not checked in (the project-level `.gitignore` already excludes `*.conc-ir`).

### hello_agent/Concerto.toml

```toml
[project]
name = "hello-agent"
version = "0.1.0"
entry = "src/main.conc"

[connections.openai]
provider = "openai"
api_key_env = "OPENAI_API_KEY"
default_model = "gpt-4o-mini"
```

### multi_agent_pipeline/Concerto.toml

```toml
[project]
name = "multi-agent-pipeline"
version = "0.1.0"
entry = "src/main.conc"

[connections.openai]
provider = "openai"
api_key_env = "OPENAI_API_KEY"
default_model = "gpt-4o"

[connections.anthropic]
provider = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
default_model = "claude-sonnet-4-20250514"
```

### tool_usage/Concerto.toml

```toml
[project]
name = "tool-usage"
version = "0.1.0"
entry = "src/main.conc"

[connections.openai]
provider = "openai"
api_key_env = "OPENAI_API_KEY"
default_model = "gpt-4o"

[mcp.GitHubServer]
transport = "stdio"
command = "npx -y @modelcontextprotocol/server-github"

[mcp.GitHubServer.env]
GITHUB_TOKEN_ENV = "GITHUB_TOKEN"
```

The corresponding `.conc` source files have their `connect` blocks removed and are moved to `src/main.conc` within each project directory. The `mcp GitHubServer` block in `tool_usage` keeps its typed function signatures but loses the `transport`/`command` fields.

## Backward Compatibility

This is a **breaking change**. Existing programs using `connect` blocks will fail to compile. Migration is straightforward:

1. Create `Concerto.toml` with `[project]` section
2. Move each `connect` block to a `[connections.*]` section
3. Replace `api_key: env("VAR")` with `api_key_env = "VAR"`
4. Move MCP `transport`/`command`/`url` fields to `[mcp.*]` sections
5. Delete `connect` blocks from `.conc` files

## Relationship to Other Specs

- **Replaces**: spec/11 (LLM Connections) — connection declarations move from source to TOML
- **Modifies**: spec/07 (Agents) — `provider:` still works, resolves from TOML
- **Modifies**: spec/20 (Interop) — MCP connection config moves to TOML
- **Modifies**: spec/16 (IR) — `connections` section populated from TOML at compile time
- **Modifies**: spec/18 (Compiler) — new TOML loading stage before semantic analysis
- **Enables**: spec/23 (Project Scaffolding) — `concerto init` generates `Concerto.toml`
