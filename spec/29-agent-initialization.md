# 29. Host Initialization Arguments

## Overview

Hosts are **middleware applications** — typically TypeScript or Python projects — that sit between Concerto and external tools or agent systems. Concerto spawns the middleware process and communicates via stdio. The middleware internally manages whatever it wraps (Claude Code, a custom LLM pipeline, a code analysis tool, etc.).

```
Concerto Runtime  <--stdio-->  Host Middleware (TS/Python)  --->  Claude Code / LLM / Tool
```

Middleware often needs structured configuration at startup — working directories, model names, API endpoints, permission lists. Currently these must be encoded as positional CLI `args` in `Concerto.toml`, which is unstructured and fragile.

This spec adds a `[hosts.<name>.params]` TOML table for named initialization parameters, and a wire protocol `init` message to deliver them to the middleware at spawn time.

## Manifest Configuration

### Params Table

A `[hosts.<name>.params]` table in `Concerto.toml` defines named arguments passed to the host middleware at initialization:

```toml
# A TypeScript middleware that internally manages Claude Code
[hosts.claude_code_host]
transport = "stdio"
command = "npx"
args = ["ts-node", "hosts/claude-code-host/index.ts"]
timeout = 600

[hosts.claude_code_host.params]
work_dir = "/home/user/my-project"
model = "opus"
allowed_tools = ["read", "write", "bash"]

# A Python middleware that wraps a custom LLM pipeline
[hosts.researcher]
transport = "stdio"
command = "python"
args = ["hosts/researcher/main.py"]
timeout = 300

[hosts.researcher.params]
model = "gpt-4o"
api_endpoint = "https://api.openai.com/v1"
max_tokens = 4096
temperature = 0.7
search_provider = "tavily"
```

### Supported Value Types

Params support the full range of TOML types, serialized as JSON:

| TOML Type | JSON Type | Example |
|-----------|-----------|---------|
| String | string | `model = "opus"` |
| Integer | number | `max_tokens = 4096` |
| Float | number | `temperature = 0.7` |
| Boolean | boolean | `verbose = true` |
| Array | array | `allowed_tools = ["read", "write"]` |
| Inline table | object | `limits = { requests = 100, tokens = 50000 }` |

Nested objects and arrays are fully supported. The params table is serialized as `serde_json::Value` and passed through to the middleware as-is.

### No Source Changes

The `.conc` source does not change. Host declarations remain purely structural:

```concerto
host ClaudeCodeHost {
    connector: "claude_code_host",
    output_format: "json",
}
```

Params are a deployment concern (which directory, which model, which permissions) and belong in config, not in language source. This is consistent with how `[connections.*]` handles LLM provider config.

## Wire Protocol

### Init Message

When the runtime spawns a host middleware that has a `[hosts.<name>.params]` table, it sends an **init message** as the first NDJSON line before any prompts:

```
--> {"type": "init", "params": {"work_dir": "/home/user/my-project", "model": "opus", "allowed_tools": ["read", "write", "bash"]}}
```

The middleware must respond with an **init_ack** message:

```
<-- {"type": "init_ack"}
```

The full sequence:

```
--> {"type": "init", "params": {...}}
<-- {"type": "init_ack"}
--> {"type": "prompt", "text": "Refactor the auth module"}
<-- {"type": "progress", "message": "Analyzing codebase...", "percent": 10}
<-- {"type": "result", "text": "Done. Refactored 3 files."}
```

### Init Ack Fields

The `init_ack` response may optionally include metadata:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | `"init_ack"` | Yes | Identifies this as an init acknowledgement |
| `version` | String | No | Middleware protocol version |
| `capabilities` | Array\<String\> | No | Middleware capabilities (e.g., `["streaming", "cancel"]`) |

Example with metadata:

```json
{"type": "init_ack", "version": "1.2.0", "capabilities": ["streaming"]}
```

### Error Handling

If the middleware does **not** respond with `init_ack`:

- **Timeout**: If no response arrives within the host's configured `timeout` (or a default init timeout of 10 seconds), the runtime raises a hard error: `"host '<name>' did not acknowledge initialization"`. This ensures the middleware is params-aware.
- **Error response**: If the middleware responds with `{"type": "error", "message": "..."}` instead of `init_ack`, the runtime raises the error immediately.
- **No params**: If the host has no `[hosts.<name>.params]` table (or it is empty), the runtime skips the init message entirely and proceeds directly to prompts. This provides backwards compatibility with existing middleware.

### Init vs Prompt

| Aspect | Init | Prompt |
|--------|------|--------|
| When sent | Once, at process spawn | Each `execute()` / `listen` call |
| Purpose | Configure the middleware | Request work |
| Response | `init_ack` | `result` / `error` / streaming messages |
| Required | Only if `[params]` exists | Always |

## Compiler Integration

### IR Embedding

The compiler already embeds host TOML config (`command`, `args`, `env`, `working_dir`) into `IrHost` at compile time. The `params` table is added as a new field:

```rust
pub struct IrHost {
    pub name: String,
    pub connector: String,
    pub input_format: String,
    pub output_format: String,
    pub timeout: Option<u32>,
    pub decorators: Vec<IrDecorator>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub working_dir: Option<String>,
    // NEW: initialization params from [hosts.<name>.params]
    pub params: Option<serde_json::Value>,
}
```

The `embed_manifest_hosts()` function in the compiler merges the `params` table from TOML into the IR alongside existing fields.

### Manifest Parsing

The `HostConfig` struct in `manifest.rs` gains a `params` field:

```rust
pub struct HostConfig {
    pub transport: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub timeout: Option<u32>,
    pub env: Option<HashMap<String, String>>,
    pub working_dir: Option<String>,
    // NEW
    pub params: Option<serde_json::Value>,
}
```

TOML nested tables under `[hosts.<name>.params]` are automatically deserialized into `serde_json::Value` by the serde TOML parser.

## Runtime Behavior

### HostClient Changes

The `HostClient` in `host.rs` gains init support:

1. On `ensure_connected()`, after spawning the subprocess, check if `params` is `Some`.
2. If so, write the init message as the first NDJSON line.
3. Read the response line. If it is `{"type": "init_ack", ...}`, proceed normally.
4. If the response is an error or times out, return a `RuntimeError`.

```rust
fn send_init(&mut self, params: &serde_json::Value) -> Result<(), RuntimeError> {
    let init_msg = serde_json::json!({
        "type": "init",
        "params": params
    });
    self.write_line(&init_msg.to_string())?;

    let response = self.read_line_timeout(Duration::from_secs(10))?;
    let parsed: serde_json::Value = serde_json::from_str(&response)
        .map_err(|e| RuntimeError::host_error(format!("invalid init response: {}", e)))?;

    match parsed.get("type").and_then(|t| t.as_str()) {
        Some("init_ack") => Ok(()),
        Some("error") => {
            let msg = parsed.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
            Err(RuntimeError::host_error(format!("host init failed: {}", msg)))
        }
        _ => Err(RuntimeError::host_error("host did not acknowledge initialization")),
    }
}
```

### IrHost to HostClient Flow

```
IrHost.params (from IR)
    |
    v
HostRegistry.register() stores params alongside client
    |
    v
HostClient.ensure_connected() checks params
    |
    v
If params present: send_init() -> wait init_ack
    |
    v
Ready for execute() / listen prompts
```

## Middleware Implementation Guide

### TypeScript Example

```typescript
// hosts/claude-code-host/index.ts
import * as readline from 'readline';

const rl = readline.createInterface({ input: process.stdin });

async function main() {
    // Step 1: Read init message
    const initLine = await new Promise<string>(resolve =>
        rl.once('line', resolve)
    );
    const initMsg = JSON.parse(initLine);

    if (initMsg.type !== 'init') {
        console.error('Expected init message');
        process.exit(1);
    }

    const { work_dir, model, allowed_tools } = initMsg.params;

    // Step 2: Acknowledge
    console.log(JSON.stringify({ type: 'init_ack', version: '1.0.0' }));

    // Step 3: Handle prompts
    for await (const line of rl) {
        const msg = JSON.parse(line);
        if (msg.type === 'prompt') {
            // Use params to configure the tool execution
            const result = await runClaudeCode(msg.text, {
                cwd: work_dir,
                model,
                allowedTools: allowed_tools,
            });
            console.log(JSON.stringify({ type: 'result', text: result }));
        }
    }
}

main();
```

### Python Example

```python
# hosts/researcher/main.py
import json
import sys

def main():
    # Step 1: Read init message
    init_line = sys.stdin.readline().strip()
    init_msg = json.loads(init_line)

    assert init_msg["type"] == "init"
    params = init_msg["params"]

    # Step 2: Acknowledge
    print(json.dumps({"type": "init_ack"}), flush=True)

    # Step 3: Configure with params
    model = params["model"]
    api_endpoint = params["api_endpoint"]
    client = create_llm_client(api_endpoint)

    # Step 4: Handle prompts
    for line in sys.stdin:
        msg = json.loads(line.strip())
        if msg["type"] == "prompt":
            response = client.chat(msg["text"], model=model)
            print(json.dumps({"type": "result", "text": response}), flush=True)

if __name__ == "__main__":
    main()
```

## Use Cases

| Host Middleware | Param | Purpose |
|----------------|-------|---------|
| Claude Code Host | `work_dir` | Directory where Claude Code operates |
| Claude Code Host | `model` | Which Claude model the middleware passes to Claude |
| Claude Code Host | `allowed_tools` | Tool permissions for the Claude session |
| Python Researcher | `model` | LLM model name for the research pipeline |
| Python Researcher | `api_endpoint` | Custom API endpoint |
| Python Researcher | `search_provider` | Which search API to use (tavily, serper, etc.) |
| TS Code Reviewer | `repo_path` | Repository to analyze |
| TS Code Reviewer | `rules_config` | Path to lint/review rules |
| Data Processor | `batch_size` | Processing batch size |
| Data Processor | `output_format` | `"csv"` or `"json"` |

## Design Rationale

### Why TOML, Not Source

- **Deployment-specific**: Working directories, models, API endpoints vary per environment — they belong in config.
- **Middleware concern**: The `.conc` source defines agent orchestration. How the middleware configures itself is the middleware's business.
- **No language complexity**: No constructor syntax or parameterized host types needed in the grammar.
- **Consistent with connections**: LLM provider config (`api_key_env`, `default_model`) already lives in TOML.

### Why Hard Fail on Missing Ack

If the middleware doesn't acknowledge init, it likely isn't params-aware and will ignore the params silently. This leads to confusing runtime behavior (middleware uses wrong model, wrong directory, etc.). A hard fail surfaces the problem immediately.

### Alternatives Considered

1. **Constructor syntax in source** (`host Worker(dir: String) { ... }`) — rejected because these are middleware deployment config, not language semantics.
2. **Environment variables only** — works but is unstructured, hard to document per-host, and doesn't compose well with multiple hosts.
3. **Extending `args` array** — CLI args are positional and untyped; named params are clearer and middleware can parse them structurally.
