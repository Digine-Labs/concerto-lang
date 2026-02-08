## Concerto.toml - Project Manifest & External Connection Config

### Problem

Currently, LLM connections and MCP server configs are declared inline in `.conc` source files using `connect` and `mcp` blocks:

```concerto
connect openai {
    api_key: env("OPENAI_API_KEY"),
    default_model: "gpt-4o-mini",
}
```

This mixes infrastructure/deployment concerns (API keys, endpoints, models) with application logic (agents, tools, pipelines). Concerto source should focus purely on **algorithm and agent harness design** -- the "what" -- not the "how to connect."

### Proposal

Introduce `Concerto.toml` as a **mandatory project manifest** (like `Cargo.toml` for Rust). All LLM provider connections and MCP server configs move here. The `connect` keyword is **removed from the language grammar entirely**.

### Concerto.toml Structure

```toml
[project]
name = "my-agent-harness"
version = "0.1.0"
entry = "src/main.conc"          # main source file

[connections.openai]
provider = "openai"              # provider type (openai, anthropic, google, ollama, custom)
api_key_env = "OPENAI_API_KEY"   # env var name (NOT the key itself)
default_model = "gpt-4o"
timeout = 60

[connections.openai.retry]
max_attempts = 3
backoff = "exponential"

[connections.openai.rate_limit]
requests_per_minute = 60

[connections.anthropic]
provider = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
default_model = "claude-sonnet-4-20250514"

[connections.local]
provider = "ollama"
base_url = "http://localhost:11434/v1"
default_model = "llama3.1"
# no api_key_env needed for local

[connections.openai.models]      # model aliasing
fast = "gpt-4o-mini"
smart = "gpt-4o"
reasoning = "o1"

[mcp.github]
transport = "stdio"
command = "npx -y @modelcontextprotocol/server-github"
env = { GITHUB_TOKEN_ENV = "GITHUB_TOKEN" }

[mcp.web_search]
transport = "sse"
url = "http://localhost:3000/mcp"
timeout = 30
```

### Impact on .conc Source Files

**Before (current):**
```concerto
connect openai {
    api_key: env("OPENAI_API_KEY"),
    default_model: "gpt-4o-mini",
}

agent Greeter {
    provider: openai,
    model: "gpt-4o-mini",
    system_prompt: "You are a friendly greeter.",
}
```

**After (proposed):**
```concerto
// No connect block needed -- connections come from Concerto.toml

agent Greeter {
    provider: openai,            // resolves from [connections.openai] in Concerto.toml
    model: "gpt-4o-mini",
    system_prompt: "You are a friendly greeter.",
}
```

Agents still reference providers by name (`provider: openai`). The name resolves from `Concerto.toml` instead of a source-level `connect` block. **Zero syntax change for agents.**

MCP constructs in source keep their **typed tool interface declarations** (function signatures, @describe, @param) but lose their connection config fields (transport, command, url):

**Before:**
```concerto
mcp GitHubServer {
    transport: "stdio",
    command: "npx -y @modelcontextprotocol/server-github",

    @describe("Create a GitHub issue")
    fn create_issue(owner: String, repo: String, title: String) -> Result<Issue, ToolError>;
}
```

**After:**
```concerto
mcp GitHubServer {
    // transport/command/url come from [mcp.github] in Concerto.toml
    // source only declares the typed tool interface

    @describe("Create a GitHub issue")
    fn create_issue(owner: String, repo: String, title: String) -> Result<Issue, ToolError>;
}
```

### Design Decisions

1. **`connect` keyword removed entirely** from the language grammar. Clean break -- source files are pure logic.
2. **`Concerto.toml` is mandatory** for any project. The compiler/runtime looks for it in the project root (walks up from source file).
3. **Agent `provider:` field unchanged** -- same name-based reference, resolved from TOML at compile/load time.
4. **`api_key_env`** instead of `api_key: env(...)` -- TOML stores the env var *name*, runtime reads the actual value. Secrets never appear in config.
5. **MCP typed interfaces stay in source** -- only connection details (transport, command, url) move to TOML. The compiler still type-checks MCP tool usage.
6. **`[project]` section** provides metadata (name, version, entry point) making Concerto projects self-describing.

### Implementation Scope

This touches:
- **Spec**: Update spec/11 (connections), spec/20 (MCP interop), spec/07 (agents). New spec or appendix for Concerto.toml format.
- **Compiler**: Remove `connect` keyword/parser/AST/semantic/codegen. Add TOML loader. Validate `provider:` names against TOML connections. Strip connection fields from `mcp` blocks.
- **Runtime**: IR loader reads connection config from TOML (or a resolved config section in IR) instead of IR `connections` section. ConnectionManager adapts to TOML source.
- **CLI**: Both `concertoc` and `concerto` find and parse `Concerto.toml` from project root.
- **Examples**: Update all 3 examples. Remove `connect` blocks, add `Concerto.toml` alongside them.
- **IR format**: The `connections` section in `.conc-ir` may be populated from TOML at compile time, or the runtime reads TOML directly. Design choice for plan agent.

### Open Questions for Plan Agent

1. Should the compiler embed resolved TOML config into IR, or should the runtime read Concerto.toml independently at load time? (Tradeoff: self-contained IR vs. runtime flexibility)
2. How does `mcp` block in source map to `[mcp.name]` in TOML? By exact name match? Configurable mapping?
3. Should there be a `concerto init` command that scaffolds a Concerto.toml?
4. Error messages: what happens when agent references `provider: foo` but no `[connections.foo]` exists in TOML?
