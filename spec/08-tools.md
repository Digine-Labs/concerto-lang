# 08 - Tools

## Overview

Tools are capabilities that models can invoke during execution. They map to the LLM function-calling interface -- when an LLM decides it needs to use a tool, the runtime intercepts the request, executes the tool method, and returns the result to the LLM.

Tools come in two forms:
1. **Local tools** -- Defined and implemented in Concerto
2. **MCP tools** -- Declared in Concerto as typed interfaces, implemented by external MCP servers

Both forms are compile-time type-checked and interchangeable on model `tools:` arrays.

## Tool Definition

### Structure

```concerto
tool ToolName {
    description: "Human-readable description of what this tool does",

    // Optional state fields
    field_name: Type,

    // Tool methods (become available as LLM functions)
    @describe("Description of what this method does")
    @param("param_name", "Description of the parameter")
    pub fn method_name(self, param_name: Type) -> Result<ReturnType, ToolError> {
        // implementation
    }
}
```

### Required Elements

1. **`description:` field** -- Every tool MUST have a `description` field. This is the tool-level description sent to the LLM so it can decide when to use the tool. The compiler enforces this.

2. **`@describe` decorator** -- Every `pub fn` method MUST have a `@describe` decorator. This replaces doc comments (`///`) for tool method descriptions. The compiler emits an error (not a warning) if a tool method lacks `@describe`.

3. **`@param` decorators** -- Every parameter of a `pub fn` method MUST have a `@param` decorator. This provides the LLM with human-readable descriptions of each parameter.

### Why Not Doc Comments?

Doc comments (`///`) are ambiguous -- they serve as both developer documentation and LLM-facing descriptions. This creates problems:
- A developer might add a `///` comment above any function for documentation purposes, not intending it as an LLM tool description
- The boundary between "this is for developers" and "this is for the LLM" becomes unclear
- Missing descriptions silently degrade model performance rather than failing loudly

The `@describe`/`@param` decorators are **compiler-enforced** -- they are part of the language grammar, not comments. This makes tool descriptions a first-class, required part of tool definitions.

## Basic Example

```concerto
tool FileConnector {
    description: "Read, write, and list files on the filesystem",

    @describe("Read the contents of a file at the given path")
    @param("path", "Absolute or relative file path to read")
    pub fn read_file(self, path: String) -> Result<String, ToolError> {
        let content = emit("tool:read_file", { "path": path }).await;
        match content {
            Ok(data) => Ok(data),
            Err(e) => Err(ToolError::new("Failed to read file: ${path}")),
        }
    }

    @describe("Write content to a file, creating it if it doesn't exist")
    @param("path", "Absolute or relative file path to write to")
    @param("content", "The string content to write to the file")
    pub fn write_file(self, path: String, content: String) -> Result<Bool, ToolError> {
        let result = emit("tool:write_file", {
            "path": path,
            "content": content,
        }).await;
        match result {
            Ok(_) => Ok(true),
            Err(e) => Err(ToolError::new("Failed to write file: ${path}")),
        }
    }

    @describe("List all files in a directory")
    @param("directory", "Path to the directory to list")
    pub fn list_files(self, directory: String) -> Result<Array<String>, ToolError> {
        let result = emit("tool:list_files", { "directory": directory }).await;
        match result {
            Ok(files) => Ok(files),
            Err(e) => Err(ToolError::new("Failed to list directory: ${directory}")),
        }
    }
}
```

## Decorator-Based Function Descriptions

The `@describe` and `@param` decorators are compiled into the tool's function schema sent to the LLM:

```concerto
tool WebSearcher {
    description: "Search the web for information",

    @describe("Search the web for information about a topic. Returns a list of relevant search results with titles and snippets.")
    @param("query", "The search query string")
    @param("max_results", "Maximum number of results to return")
    pub fn search(self, query: String, max_results: Int = 5) -> Result<Array<SearchResult>, ToolError> {
        // ...
    }
}
```

The compiler extracts decorators and type signatures into the function schema:

```json
{
    "name": "search",
    "description": "Search the web for information about a topic. Returns a list of relevant search results with titles and snippets.",
    "parameters": {
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "The search query string"
            },
            "max_results": {
                "type": "integer",
                "default": 5,
                "description": "Maximum number of results to return"
            }
        },
        "required": ["query"]
    }
}
```

**Compiler enforcement rules**:
- Missing `@describe` on a `pub fn` in a tool -> compile error
- Missing `@param` for any parameter -> compile error
- `@param` referencing a non-existent parameter name -> compile error
- `@param` count mismatch with parameter list -> compile error (excluding `self`)

## Auto-Generated Parameter Schemas

Tool method type signatures are automatically converted to JSON Schema for the LLM function-calling API:

| Concerto Type | JSON Schema Type |
|---------------|-----------------|
| `String` | `"string"` |
| `Int` | `"integer"` |
| `Float` | `"number"` |
| `Bool` | `"boolean"` |
| `Array<T>` | `{ "type": "array", "items": ... }` |
| `Map<String, T>` | `{ "type": "object", "additionalProperties": ... }` |
| `Option<T>` | Schema for T (parameter becomes optional) |

Parameters with default values become optional in the schema.

## Tool State

Tools can maintain internal state across invocations within an execution.

```concerto
tool ConversationTracker {
    description: "Track conversation turns and history",
    turn_count: Int = 0,
    history: Array<String> = [],

    @describe("Record a conversation turn and return the turn number")
    @param("message", "The message content to record")
    pub fn record_turn(mut self, message: String) -> Result<Int, ToolError> {
        self.turn_count += 1;
        self.history.push(message);
        Ok(self.turn_count)
    }

    @describe("Get the full conversation history")
    pub fn get_history(self) -> Result<Array<String>, ToolError> {
        Ok(self.history)
    }
}
```

Note: `mut self` is required for methods that modify tool state.

## Runtime Bindings

Tools can be implemented in two ways:

### 1. Concerto Implementation (pure logic)

The tool logic is written entirely in Concerto:

```concerto
tool Calculator {
    description: "Evaluate mathematical expressions",

    @describe("Evaluate a mathematical expression")
    @param("expression", "The mathematical expression to evaluate")
    pub fn evaluate(self, expression: String) -> Result<Float, ToolError> {
        let result = eval_math(expression)?;
        Ok(result)
    }
}
```

### 2. Host-Bound Implementation (via emit to host)

The tool delegates to the host application through the emit system:

```concerto
tool DatabaseQuery {
    description: "Execute SQL queries against the application database",

    @describe("Execute a SQL query against the application database")
    @param("sql", "The SQL query string to execute")
    pub fn query(self, sql: String) -> Result<Array<Map<String, Any>>, ToolError> {
        let result = emit("tool:db_query", { "sql": sql }).await?;
        Ok(result)
    }
}
```

The host application registers handlers:
```
// Host side (Rust):
runtime.on_emit("tool:db_query", |payload| {
    let sql = payload["sql"].as_str();
    let results = database.query(sql);
    Ok(serialize(results))
});
```

## MCP Tool Integration

Concerto supports connecting to external MCP (Model Context Protocol) servers as a first-class language feature. MCP tool interfaces are declared in Concerto's type system so the compiler can type-check tool usage at compile time.

### The `mcp` Construct

```concerto
mcp ServerName {
    // Connection configuration
    transport: "stdio" | "sse",
    command: "npx -y @modelcontextprotocol/server-name",  // for stdio
    url: "http://localhost:3000/mcp",                      // for sse

    // Typed tool interface declarations (no body -- implemented by server)
    @describe("Description of what this tool does")
    @param("param_name", "Description of the parameter")
    fn tool_name(param_name: Type) -> Result<ReturnType, ToolError>;
}
```

### MCP Example

```concerto
mcp GitHubServer {
    transport: "stdio",
    command: "npx -y @modelcontextprotocol/server-github",

    @describe("Search GitHub repositories by query")
    @param("query", "Search query string")
    @param("max_results", "Maximum number of results to return")
    fn search_repositories(query: String, max_results: Int = 10) -> Result<Array<Repository>, ToolError>;

    @describe("Get the contents of a file in a repository")
    @param("owner", "Repository owner username")
    @param("repo", "Repository name")
    @param("path", "File path within the repository")
    fn get_file_contents(owner: String, repo: String, path: String) -> Result<String, ToolError>;

    @describe("Create a new issue in a repository")
    @param("owner", "Repository owner username")
    @param("repo", "Repository name")
    @param("title", "Issue title")
    @param("body", "Issue body content in markdown")
    fn create_issue(owner: String, repo: String, title: String, body: String) -> Result<Issue, ToolError>;
}
```

### Key Design Points

1. **No body on MCP fn declarations** -- These are interface declarations only. The implementation lives on the MCP server.

2. **No `self` parameter** -- MCP tools are stateless from Concerto's perspective. The server manages its own state.

3. **Compile-time type checking** -- The compiler validates that code using MCP tools passes the correct argument types and handles the return types correctly. This is the same type checking applied to local tools.

4. **Runtime validation** -- When the runtime connects to the MCP server, it validates that the server actually provides the declared tools with compatible schemas. Mismatches produce runtime errors with clear diagnostics.

5. **Transport configuration** -- `transport: "stdio"` launches a subprocess; `transport: "sse"` connects to an HTTP endpoint. Additional transport types can be added as the MCP spec evolves.

## Attaching Tools to Models

Both local tools and MCP tools attach to models uniformly via the `tools:` array:

```concerto
model ResearchAssistant {
    provider: openai,
    base: "gpt-4o",
    system_prompt: "You are a research assistant with web search, file access, and GitHub integration.",
    tools: [WebSearcher, FileConnector, GitHubServer],
}
```

When the model makes a tool call:
1. Runtime receives the tool call from the LLM
2. Runtime looks up the tool in the model's tool registry
3. **For local tools**: Runtime invokes the tool method with the provided arguments
4. **For MCP tools**: Runtime forwards the call to the MCP server via the configured transport
5. The tool method executes and returns a result
6. Runtime sends the result back to the LLM for continued generation

## Tool Permissions

The runtime can restrict which tool methods are actually available, providing a security layer:

```concerto
// In the host configuration:
// runtime.set_tool_permissions({
//     "FileConnector": {
//         "read_file": true,
//         "write_file": false,  // Disable write access
//     },
//     "ShellTool": false,  // Disable entirely
//     "GitHubServer": {
//         "search_repositories": true,
//         "create_issue": false,  // Disable write operations
//     },
// });
```

Tool methods that are disabled will not appear in the function schema sent to the LLM.

## Built-in Tools

Concerto provides several built-in tools in the standard library:

### `std::tools::HttpTool`

```concerto
use std::tools::HttpTool;

tool HttpTool {
    description: "Make HTTP requests",

    @describe("Make an HTTP GET request to a URL")
    @param("url", "The URL to send the GET request to")
    @param("headers", "Optional HTTP headers to include")
    pub fn get(self, url: String, headers: Map<String, String> = {}) -> Result<HttpResponse, ToolError>;

    @describe("Make an HTTP POST request with a JSON body")
    @param("url", "The URL to send the POST request to")
    @param("body", "The JSON body to include in the request")
    @param("headers", "Optional HTTP headers to include")
    pub fn post(self, url: String, body: Map<String, Any>, headers: Map<String, String> = {}) -> Result<HttpResponse, ToolError>;
}
```

### `std::tools::FileTool`

```concerto
use std::tools::FileTool;

tool FileTool {
    description: "Read, write, and check files on the filesystem",

    @describe("Read file contents as a string")
    @param("path", "The file path to read")
    pub fn read(self, path: String) -> Result<String, ToolError>;

    @describe("Write string content to a file")
    @param("path", "The file path to write to")
    @param("content", "The content to write")
    pub fn write(self, path: String, content: String) -> Result<Bool, ToolError>;

    @describe("Check if a file exists at the given path")
    @param("path", "The file path to check")
    pub fn exists(self, path: String) -> Result<Bool, ToolError>;
}
```

### `std::tools::ShellTool`

```concerto
use std::tools::ShellTool;

tool ShellTool {
    description: "Execute shell commands",

    @describe("Execute a shell command and return stdout")
    @param("command", "The shell command to execute")
    @param("timeout_ms", "Maximum execution time in milliseconds")
    pub fn exec(self, command: String, timeout_ms: Int = 30000) -> Result<String, ToolError>;
}
```

**Security note**: `ShellTool` is disabled by default in the runtime. The host must explicitly enable it.

## Tool Error Handling

Tool methods return `Result<T, ToolError>`. When a tool fails:

1. The error is sent back to the LLM as a tool result
2. The LLM can decide to retry, use a different approach, or report the error
3. The model's `on_tool_error` hook (if defined) is called

```concerto
impl ResearchAssistant {
    fn on_tool_error(self, tool_name: String, error: ToolError) -> ToolErrorAction {
        match tool_name {
            "WebSearcher" => ToolErrorAction::Retry(3),
            "FileConnector" => ToolErrorAction::ReportToLLM,
            _ => ToolErrorAction::Fail,
        }
    }
}
```

## Trait-Based Tool Interfaces

Tools can implement traits for shared behavior:

```concerto
trait Searchable {
    @describe("Search for items matching a query")
    @param("query", "The search query")
    fn search(self, query: String) -> Result<Array<SearchResult>, ToolError>;
}

tool WebSearcher impl Searchable {
    description: "Search the web",

    @describe("Search the web for information")
    @param("query", "The search query")
    pub fn search(self, query: String) -> Result<Array<SearchResult>, ToolError> {
        // Web search implementation
    }
}

tool DatabaseSearcher impl Searchable {
    description: "Search the database",

    @describe("Search the database for matching records")
    @param("query", "The search query")
    pub fn search(self, query: String) -> Result<Array<SearchResult>, ToolError> {
        // Database search implementation
    }
}
```
