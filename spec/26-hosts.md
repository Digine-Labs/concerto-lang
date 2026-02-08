# 26 - Hosts

## Overview

A **Host** is a connector between the Concerto runtime and an external AI agent system. Unlike agents (which use LLM API endpoints), hosts wrap full external AI systems -- tools like Claude Code, Cursor, Devin, or custom AI services -- that have their own reasoning, tool usage, and state management.

From Concerto code's perspective, a host looks similar to an agent: you send prompts and receive responses. But underneath, the host manages a subprocess (via stdio transport) that runs the external agent system.

### Key Distinctions

| Concept | What It Is | Transport | State |
|---------|-----------|-----------|-------|
| **Agent** | Concerto-defined, powered by LLM API | HTTP to provider | Stateless per-call |
| **Host** | Adapter to external agent system | Stdio subprocess | Stateful (process stays alive) |
| **MCP** | Tool protocol | Stdio/HTTP | Stateless tools |
| **Provider** | API endpoint config (in TOML) | HTTP | N/A (config only) |

A host IS an agent in the sense that it thinks and acts, but it is NOT defined in Concerto -- it is an external system wrapped by a protocol adapter.

## Declaration

```concerto
host ClaudeCode {
    connector: claude_code,      // references [hosts.claude_code] in Concerto.toml
    input_format: "text",        // "text" | "json"
    output_format: "json",       // "text" | "json"
    timeout: 300,                // seconds (default: 120)
}
```

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `connector` | identifier | yes | Name of the `[hosts.X]` section in `Concerto.toml` |
| `input_format` | string | no | How prompts are sent: `"text"` (default) or `"json"` |
| `output_format` | string | no | How responses are parsed: `"text"` (default) or `"json"` |
| `timeout` | int | no | Per-call timeout in seconds (default: 120) |

## TOML Configuration

Host connections are defined in `Concerto.toml` under `[hosts.*]` sections:

```toml
[hosts.claude_code]
transport = "stdio"
command = "claude"
args = ["--print", "--output-format", "json"]
timeout = 300
env = { CLAUDE_MODEL = "claude-sonnet-4-20250514" }
working_dir = "."

[hosts.cursor]
transport = "stdio"
command = "cursor"
args = ["--cli", "--no-ui"]
timeout = 120

[hosts.custom_agent]
transport = "stdio"
command = "python"
args = ["my_agent.py"]
env = { AGENT_MODE = "production" }
```

### TOML Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `transport` | string | yes | Transport protocol: `"stdio"` |
| `command` | string | yes | Command to spawn the external process |
| `args` | array | no | Command arguments |
| `timeout` | int | no | Default timeout in seconds |
| `env` | table | no | Environment variables for the subprocess |
| `working_dir` | string | no | Working directory for the subprocess |

## Usage

### Basic Execution

```concerto
let result = ClaudeCode.execute("Write a function that sorts an array")?;
emit("output", result);
```

### With Schema Validation

```concerto
schema CodeOutput {
    code: String,
    language: String,
    explanation: String,
}

let result = ClaudeCode.execute_with_schema<CodeOutput>(prompt)?;
emit("code", result.code);
```

Schema validation works identically to agents: the response is parsed as JSON, validated against the schema, and retried on mismatch (up to 3 times).

### With Memory

```concerto
memory code_session: Memory = Memory::new();

// First call
let r1 = ClaudeCode.with_memory(code_session).execute("Create a new Rust project")?;

// Second call -- host has its own state, but memory tracks concerto-side history
let r2 = ClaudeCode.with_memory(code_session).execute("Add a sort function")?;
```

When `with_memory()` is used on a stateful host, the memory is for **Concerto-side record-keeping** (logging, inspection, replay). The host itself maintains its own internal conversation state.

### With Context

```concerto
// Pass additional context data to the host
let ctx = {
    files: ["src/main.rs", "src/lib.rs"],
    project: "my-app",
};
let result = ClaudeCode.with_context(ctx).execute("Fix the compilation error")?;
```

For `input_format: "json"`, the context is merged into the JSON payload sent to the host. For `input_format: "text"`, the context is serialized and appended to the prompt.

### Composing Builders

```concerto
let result = ClaudeCode
    .with_memory(session)
    .with_context({ files: ["src/main.rs"] })
    .execute(prompt)?;
```

## Execution Model

### Stdio Transport

The host spawns a subprocess and communicates via stdin/stdout:

1. **Connect** (lazy, on first use): Spawn subprocess via `Command::new(config.command)`
2. **Execute**: Write input to stdin, read output from stdout
3. **Stateful**: Process stays alive between calls. The host tracks its own conversation state.
4. **Shutdown**: Process is killed when the VM is dropped

### Input Formats

**Text format** (`input_format: "text"`):
```
→ stdin: "Write a function that sorts an array\n"
← stdout: "Here's a sort function:\n\nfn sort<T: Ord>(...\n"
```

The runtime writes the prompt as a line to stdin and reads until EOF or a delimiter.

**JSON format** (`input_format: "json"`):
```json
→ stdin: {"prompt": "Write a sort function", "context": {"files": ["src/main.rs"]}}
← stdout: {"text": "fn sort...", "metadata": {"files_modified": ["src/sort.rs"]}}
```

The runtime writes a JSON object to stdin (one line) and reads a JSON object from stdout.

### Output Parsing

For `output_format: "text"`:
- Response is returned as `Value::String`
- For `execute_with_schema<T>()`, the text is parsed as JSON

For `output_format: "json"`:
- Response is parsed as JSON
- The `"text"` field is extracted as the primary response
- Additional fields are available as a map

### Delimiter Protocol

To support multi-turn communication over a persistent stdio connection, the host uses a **line-delimited** protocol:

- Input: single JSON/text line terminated by `\n`
- Output: single JSON/text line terminated by `\n`
- The runtime reads exactly one line per execution

For multi-line responses (common with code generation), the host should:
- In text mode: use a delimiter marker (e.g., `\x04` ETX or empty line after output)
- In JSON mode: output is always a single JSON line (newlines in content are escaped)

## Edge Cases

### Host Process Crashes

If the subprocess exits unexpectedly (non-zero exit code, signal), the runtime:
1. Returns `Err("Host 'name' process exited with code N")`
2. On the next `.execute()` call, re-spawns the process

### Timeout

If the host doesn't respond within the configured timeout:
1. The subprocess is killed (SIGTERM, then SIGKILL after 5s)
2. Returns `Err("Host 'name' timed out after N seconds")`
3. On the next call, re-spawns the process

### Host's Own Tools

External agent systems (like Claude Code) have their own tool capabilities (file read/write, bash execution, etc.). These tools are **invisible to Concerto** -- the host manages them internally. Concerto only sees the final output.

This is a deliberate design choice: Concerto orchestrates at the agent level, not the tool level, for hosts.

### Memory vs Host State

Stateful hosts maintain their own conversation context. When `with_memory()` is used:
- **Memory**: Concerto-side log of prompts/responses (for inspection, replay, passing to other agents)
- **Host state**: Internal to the subprocess (Concerto doesn't see or manage it)

These are independent. Memory is useful for logging and cross-agent context sharing even when the host is stateful.

### Error Handling

```concerto
match ClaudeCode.execute(prompt) {
    Ok(response) => emit("output", response),
    Err(e) => {
        // e could be: "Host 'ClaudeCode' process exited with code 1"
        //             "Host 'ClaudeCode' timed out after 300 seconds"
        //             "Host 'ClaudeCode' output parsing failed: invalid JSON"
        emit("error", e);
    }
}
```

## Type System

| Type | Description |
|------|-------------|
| `HostRef` | Runtime reference to a named host |

Hosts share the `AgentBuilder` pattern with agents -- `with_memory()`, `with_context()`, and `execute()` all work through the same builder mechanism.

## Compilation

### Keyword and AST

The `host` keyword is added to the lexer. The parser produces a `HostDecl` AST node (same structure as `AgentDecl` -- config fields with name/value pairs).

### Semantic Analysis

- `host` declarations are registered as `SymbolKind::Host`
- `connector` field must reference a valid `[hosts.*]` section in the manifest
- `input_format` and `output_format` must be `"text"` or `"json"`
- `timeout` must be a positive integer

### IR Generation

```json
{
  "hosts": [
    {
      "name": "ClaudeCode",
      "connector": "claude_code",
      "input_format": "text",
      "output_format": "json",
      "timeout": 300,
      "decorators": []
    }
  ]
}
```

## Runtime

### HostClient

```
HostClient {
    name: String,
    child: Child,               // spawned subprocess
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    input_format: HostFormat,   // Text | Json
    output_format: HostFormat,
    timeout: Duration,
    connected: bool,
}
```

Methods: `connect`, `execute`, `execute_json`, `shutdown`, `is_alive`.

### HostRegistry

```
HostRegistry {
    clients: HashMap<String, HostClient>,
    configs: HashMap<String, HostConfig>,
}
```

The registry lazily connects to hosts on first use and manages their lifecycle.

### VM Integration

- `Value::HostRef(name)` in globals for each declared host
- `exec_call_method` on `HostRef` dispatches `execute`, `with_memory`, `with_context`
- `exec_call_method` on `AgentBuilder { source_kind: Host }` dispatches `execute`, `execute_with_schema`

## Examples

### Using Claude Code as a Host

```concerto
host ClaudeCode {
    connector: claude_code,
    input_format: "text",
    output_format: "text",
    timeout: 300,
}

agent Architect {
    provider: openai,
    model: "gpt-4o",
    system_prompt: "You are a software architect. Design systems and delegate implementation.",
}

fn main() {
    // Architect designs the approach
    let design = Architect.execute("Design a REST API for a todo app")?;

    // Claude Code implements it
    let code = ClaudeCode.execute("Implement this design: ${design}")?;

    emit("result", code);
}
```

### Multi-Host Orchestration

```concerto
host ClaudeCode {
    connector: claude_code,
    output_format: "json",
    timeout: 300,
}

host TestRunner {
    connector: test_runner,
    output_format: "json",
    timeout: 60,
}

schema TestResult {
    passed: Bool,
    failures: Array<String>,
}

fn main() {
    let code = ClaudeCode.execute("Write a sorting algorithm in Rust")?;

    let result = TestRunner.execute_with_schema<TestResult>(
        "Run tests on this code: ${code}"
    )?;

    if !result.passed {
        // Feed failures back to Claude Code
        let fix = ClaudeCode.execute(
            "Fix these test failures: ${result.failures}"
        )?;
    }
}
```

### Host with Context

```concerto
host ClaudeCode {
    connector: claude_code,
    input_format: "json",
    output_format: "json",
    timeout: 300,
}

fn main() {
    let ctx = {
        files: ["src/main.rs", "src/lib.rs"],
        task: "refactor",
    };

    let result = ClaudeCode
        .with_context(ctx)
        .execute("Refactor the error handling to use thiserror")?;
}
```

## Future Extensions

- **HTTP transport**: REST API endpoints for cloud-hosted agent services
- **WebSocket transport**: Bidirectional streaming for real-time agent interaction
- **Host capabilities declaration**: Describe what a host can do (file access, code execution, etc.) for compile-time validation
- **Host federation**: Multiple Concerto runtimes coordinating hosts across machines
- **Host-to-Concerto callbacks**: Hosts requesting information from the Concerto runtime mid-execution via the emit system
