# 20 - Interop and FFI

## Overview

Concerto is designed to be embedded in host applications. It communicates with the outside world through three mechanisms:

1. **Emit system** -- Primary output mechanism (bidirectional)
2. **Host API** -- Rust library for embedding the runtime
3. **Tool FFI** -- Tools implemented in the host language, registered at runtime

Concerto does NOT call external APIs directly. All external communication flows through the runtime, which the host application controls.

## Host Language Integration

### Rust (Primary)

The Concerto runtime is a Rust library (`concerto-runtime` crate). Host applications embed it directly.

```rust
use concerto_runtime::{Runtime, RuntimeConfig, Value};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let runtime = Runtime::new(RuntimeConfig::default());
    runtime.load_ir("harness.conc-ir")?;

    // Register emit handlers
    runtime.on("result", |value: Value| {
        println!("Got result: {}", value);
    });

    runtime.on_emit("tool:database_query", |payload: Value| async move {
        let query = payload.get_str("query")?;
        let results = my_database.execute(query).await?;
        Ok(Value::from(results))
    });

    runtime.execute().await?;

    Ok(())
}
```

### C FFI (For Other Languages)

A C-compatible FFI layer enables integration from any language that can call C functions.

```c
// C header (generated)
typedef struct ConcertoRuntime ConcertoRuntime;
typedef struct ConcertoValue ConcertoValue;

ConcertoRuntime* concerto_runtime_new(const char* config_json);
int concerto_runtime_load_ir(ConcertoRuntime* rt, const char* path);
int concerto_runtime_execute(ConcertoRuntime* rt);
void concerto_runtime_on_emit(ConcertoRuntime* rt, const char* channel, EmitCallback cb);
void concerto_runtime_free(ConcertoRuntime* rt);

// Value access
const char* concerto_value_as_string(ConcertoValue* val);
int64_t concerto_value_as_int(ConcertoValue* val);
double concerto_value_as_float(ConcertoValue* val);
```

### Python Bindings (Future)

Via PyO3 or the C FFI:

```python
from concerto import Runtime

runtime = Runtime()
runtime.load_ir("harness.conc-ir")

@runtime.on("result")
def handle_result(value):
    print(f"Got result: {value}")

@runtime.on_emit("tool:search")
async def handle_search(payload):
    results = await search_engine.query(payload["query"])
    return results

runtime.execute()
```

### Node.js / TypeScript Bindings (Future)

Via NAPI or the C FFI:

```typescript
import { Runtime } from 'concerto-runtime';

const runtime = new Runtime();
await runtime.loadIR('harness.conc-ir');

runtime.on('result', (value) => {
    console.log('Got result:', value);
});

runtime.onEmit('tool:api_call', async (payload) => {
    const response = await fetch(payload.url);
    return await response.json();
});

await runtime.execute();
```

## Tool FFI

Tools defined in Concerto can delegate to host implementations. This is the primary way to extend Concerto's capabilities beyond LLM interaction.

### Concerto Side

```concerto
tool DatabaseTool {
    description: "Execute SQL queries against the application database",

    @describe("Execute a SQL query and return results")
    @param("sql", "The SQL query string to execute")
    pub fn query(self, sql: String) -> Result<Array<Map<String, Any>>, ToolError> {
        let result = await emit("tool:db_query", { "sql": sql });
        match result {
            Ok(data) => Ok(data),
            Err(e) => Err(ToolError::new("Database query failed: ${e}")),
        }
    }

    @describe("Insert a record into a table")
    @param("table", "The target table name")
    @param("data", "Key-value pairs to insert as a row")
    pub fn insert(self, table: String, data: Map<String, Any>) -> Result<Bool, ToolError> {
        let result = await emit("tool:db_insert", {
            "table": table,
            "data": data,
        });
        match result {
            Ok(_) => Ok(true),
            Err(e) => Err(ToolError::new("Insert failed: ${e}")),
        }
    }
}
```

### Host Side (Rust)

```rust
runtime.on_emit("tool:db_query", |payload: Value| async move {
    let sql = payload.get_str("sql")
        .ok_or_else(|| anyhow!("Missing 'sql' field"))?;

    let rows = sqlx::query(sql)
        .fetch_all(&db_pool)
        .await?;

    let results: Vec<Value> = rows.iter()
        .map(|row| row_to_value(row))
        .collect();

    Ok(Value::Array(results))
});
```

### Native Tool Registration

For high-performance tools, the host can register native Rust functions directly:

```rust
runtime.register_native_tool("FileReader", "read_file", |args: Vec<Value>| async move {
    let path = args[0].as_str().ok_or("Expected string path")?;
    let content = tokio::fs::read_to_string(path).await?;
    Ok(Value::String(content))
});
```

## Data Serialization Boundary

All values crossing the Concerto-Host boundary are serialized as JSON-compatible types.

### Concerto to Host

| Concerto Type | Serialized As |
|---------------|--------------|
| `Int` | JSON number (integer) |
| `Float` | JSON number |
| `String` | JSON string |
| `Bool` | JSON boolean |
| `Nil` | JSON null |
| `Array<T>` | JSON array |
| `Map<K, V>` | JSON object |
| `Struct` | JSON object (field names as keys) |
| `Enum variant` | `{ "variant": "Name", "data": ... }` |
| `Option::Some(v)` | The value |
| `Option::None` | null |
| `Result::Ok(v)` | `{ "ok": value }` |
| `Result::Err(e)` | `{ "err": value }` |

### Host to Concerto

The same mapping applies in reverse. The runtime deserializes JSON values into Concerto `Value` instances.

## Runtime Configuration

The host configures the runtime behavior through `RuntimeConfig`:

```rust
struct RuntimeConfig {
    // Connection overrides (for testing, staging, etc.)
    connection_overrides: HashMap<String, ConnectionOverride>,

    // Tool permissions (enable/disable tools)
    tool_permissions: HashMap<String, ToolPermission>,

    // Emit mode (immediate or buffered)
    emit_mode: EmitMode,

    // Global execution timeout
    execution_timeout: Duration,

    // Debug mode (enable step-through, verbose logging)
    debug: bool,

    // File system sandbox root
    fs_root: Option<PathBuf>,

    // Environment variable whitelist (restrict which env vars Concerto can read)
    env_whitelist: Option<Vec<String>>,

    // Maximum memory for databases
    max_db_memory: usize,

    // Maximum concurrent model calls
    max_concurrent_models: usize,
}
```

## WASM Target (Future)

Compiling Concerto IR to WebAssembly for browser execution:

```
.conc -> Compiler -> .conc-ir -> WASM Compiler -> .wasm
```

### Browser Runtime

```javascript
import { ConcertoWasm } from 'concerto-wasm';

const runtime = await ConcertoWasm.load('harness.wasm');

runtime.on('result', (value) => {
    document.getElementById('output').textContent = JSON.stringify(value);
});

runtime.onEmit('tool:user_input', async (payload) => {
    return prompt(payload.message);
});

await runtime.execute();
```

### Considerations
- All LLM calls go through bidirectional emit (browser makes fetch calls)
- File system tools are unavailable (replaced by browser-specific tools)
- WASM runtime is a separate compilation target (not v1)

## MCP (Model Context Protocol) Integration

Concerto provides first-class language support for MCP servers through the `mcp` construct. Unlike runtime-only MCP discovery, Concerto requires tool interfaces to be declared in the source code so the compiler can type-check all tool usage at compile time.

See [spec/08-tools.md](08-tools.md) for the full `mcp` construct specification.

### Design Philosophy

MCP servers are treated as **typed interfaces** in Concerto, not opaque runtime services. The language must understand what tools an MCP server provides -- their names, parameter types, return types, and descriptions -- at compile time.

This means:
- The compiler can catch type errors in MCP tool usage before runtime
- IDE tooling can provide autocomplete for MCP tool parameters
- Refactoring tools can find all usages of an MCP tool
- The developer explicitly declares what they expect from an MCP server

### MCP Declaration Syntax

```concerto
mcp ServerName {
    // Connection configuration
    transport: "stdio",
    command: "npx -y @modelcontextprotocol/server-name",

    // Typed tool interfaces (no body -- server implements these)
    @describe("Description of tool")
    @param("param", "Description of param")
    fn tool_name(param: Type) -> Result<ReturnType, ToolError>;
}
```

### MCP Tool Schema Mapping

Concerto MCP declarations map bidirectionally to MCP tool schemas:

```concerto
mcp WebSearch {
    transport: "sse",
    url: "http://localhost:3000/mcp",

    @describe("Search the web for information")
    @param("query", "The search query string")
    @param("max_results", "Maximum number of results to return")
    fn search(query: String, max_results: Int = 10) -> Result<Array<SearchResult>, ToolError>;
}
```

Maps to/from MCP tool definition:
```json
{
    "name": "search",
    "description": "Search the web for information",
    "inputSchema": {
        "type": "object",
        "properties": {
            "query": { "type": "string", "description": "The search query string" },
            "max_results": { "type": "integer", "default": 10, "description": "Maximum number of results to return" }
        },
        "required": ["query"]
    }
}
```

### Runtime Validation

When the runtime connects to an MCP server, it performs validation:

1. **Tool existence** -- Verify the server provides all declared tools
2. **Schema compatibility** -- Check that the server's parameter schemas are compatible with the Concerto declarations
3. **Missing tools** -- Produce a clear runtime error if the server doesn't provide a declared tool:

```
Runtime error: MCP server 'GitHubServer' does not provide tool 'create_issue'
  --> main.conc:15:5
   |
15 |     fn create_issue(owner: String, ...) -> Result<Issue, ToolError>;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   = help: verify the MCP server version supports this tool
```

### MCP + Local Tools on Agents

Both local tools and MCP tools are interchangeable on model `tools:` arrays:

```concerto
model Researcher {
    provider: openai,
    base: "gpt-4o",
    tools: [
        FileConnector,     // Local tool (defined with `tool`)
        Calculator,        // Local tool (defined with `tool`)
        GitHubServer,      // MCP tool (defined with `mcp`)
    ],
}
```

The model doesn't distinguish between local and MCP tools -- both produce the same function schemas for the LLM. The runtime handles routing:
- Local tool calls -> execute Concerto method
- MCP tool calls -> forward to MCP server via configured transport

### Host-Side MCP Configuration

The host can override MCP connection settings at runtime:

```rust
runtime.configure_mcp("GitHubServer", McpConfig {
    // Override the command from the source
    command: Some("docker run ghcr.io/mcp-server-github".into()),
    // Add environment variables
    env: vec![("GITHUB_TOKEN", "ghp_...")],
    // Connection timeout
    timeout: Duration::from_secs(30),
});
```

This allows the same Concerto program to connect to different MCP server instances in development vs production.

## Security Model

### Principle of Least Privilege
- Concerto code has no direct access to the system
- All external operations go through the emit system or registered tools
- The host decides what tools are available and what permissions they have

### Layers of Security

1. **Compile-time**: Type checking, scope analysis
2. **Runtime tool permissions**: Host enables/disables specific tools
3. **File system sandboxing**: Restrict file access to specific directories
4. **Environment variable whitelisting**: Restrict env var access
5. **Network restrictions**: Host controls which URLs tools can access
6. **Resource limits**: Max memory, max concurrent calls, execution timeout

### Auditing

The runtime emits audit events for security-sensitive operations:
```
emit("audit:tool_call", { "tool": "ShellTool", "method": "exec", "args": {...} })
emit("audit:file_access", { "operation": "read", "path": "/data/input.txt" })
emit("audit:env_access", { "variable": "API_KEY" })
```

Host applications can monitor these for security logging.
