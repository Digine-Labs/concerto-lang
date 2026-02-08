# 25 - Dynamic Tool Binding

## Overview

Concerto agents declare their tools statically at definition time via the `tools` field. However, many orchestration patterns require **dynamic tool binding** -- adding or removing tools for specific executions without modifying the agent definition.

Dynamic tool binding uses the **AgentBuilder** pattern (shared with agent memory and hosts) to temporarily modify which tools are available to an agent for a single execution.

## Syntax

```concerto
// Add tools for this execution only (in addition to agent's static tools)
let result = Agent.with_tools([Calculator, FileManager]).execute(prompt);

// Remove agent's default tools for this execution
let result = Agent.without_tools().execute(prompt);

// Compose with memory
let result = Agent.with_memory(m).with_tools([Calculator]).execute(prompt);

// Compose multiple builder calls
let result = Agent
    .with_memory(conversation)
    .with_tools([SearchTool, DatabaseTool])
    .execute(prompt);
```

## Semantics

### `with_tools(tools: Array<ToolRef | McpRef>)`

Adds the specified tools' schemas to the `ChatRequest.tools` for this execution **in addition to** the agent's statically-defined tools.

Accepts an array containing:
- **Concerto Tool references** -- tools defined with the `tool` keyword
- **MCP server references** -- MCP servers defined in `Concerto.toml`

```concerto
tool Calculator {
    @describe("Add two numbers")
    pub fn add(@param("first number") a: Int, @param("second number") b: Int) -> Int {
        a + b
    }
}

agent Assistant {
    provider: openai,
    model: "gpt-4o",
    tools: [WebSearch],  // static MCP tool
}

// WebSearch + Calculator available for this call
let result = Assistant.with_tools([Calculator]).execute("What is 2+2?");
```

### `without_tools()`

Excludes the agent's statically-defined tools from this execution. The agent runs without any tool schemas in the request.

```concerto
// Agent has tools: [WebSearch, FileManager] but run without them
let result = Agent.without_tools().execute("Just answer from your knowledge.");
```

### Tool Resolution Order

When `AgentBuilder.execute()` runs:

1. **Static tools** (from agent definition) -- included unless `without_tools()` was called
2. **Dynamic tools** (from `with_tools()`) -- always included
3. All tool schemas are merged into `ChatRequest.tools`

If the same tool appears in both static and dynamic sets, it is included once (deduplication by name).

## Tool Schema Generation

### MCP Tools (Existing)

MCP tool schemas are already discovered at runtime via JSON-RPC `tools/list` and included in `ChatRequest`. No changes needed for MCP tools.

### Concerto Tool Schemas (New)

Currently, Concerto `tool` definitions have `@describe` and `@param` decorators but no JSON schemas are generated for LLM function calling. For dynamic tool binding, the **compiler generates tool schemas at compile time**.

For each `pub fn` method on a tool:

```concerto
tool Calculator {
    @describe("Add two numbers together")
    pub fn add(
        @param("The first number") a: Int,
        @param("The second number") b: Int,
    ) -> Int {
        a + b
    }
}
```

The compiler generates:

```json
{
  "tool_schemas": [
    {
      "method_name": "Calculator::add",
      "description": "Add two numbers together",
      "parameters": {
        "type": "object",
        "properties": {
          "a": { "type": "integer", "description": "The first number" },
          "b": { "type": "integer", "description": "The second number" }
        },
        "required": ["a", "b"]
      }
    }
  ]
}
```

This uses the existing `@describe`/`@param` decorators (already compiler-enforced on tool methods) and the type annotations to produce JSON Schema.

### Type Mapping for Tool Parameters

| Concerto Type | JSON Schema Type |
|---------------|-----------------|
| `Int` | `"integer"` |
| `Float` | `"number"` |
| `String` | `"string"` |
| `Bool` | `"boolean"` |
| `Array<T>` | `{ "type": "array", "items": ... }` |
| `Map<K,V>` | `{ "type": "object" }` |
| `Option<T>` | type of T (not in `required`) |

## Tool Call Execution Loop

When the LLM responds with `tool_calls` in its response, the runtime executes a tool call loop:

1. For each tool call in the response:
   - If Concerto tool: dispatch via the existing `CALL_TOOL` mechanism
   - If MCP tool: forward to the MCP server via `McpClient`
2. Append tool results as messages: `{ role: "tool", content: result_json, tool_call_id: id }`
3. Re-send to LLM with updated messages
4. Repeat until the LLM responds without tool calls (or max 10 iterations)

**Note:** The tool call execution loop is a significant enhancement. The initial implementation (Phase 7b MVP) includes tool schemas in the request. The full tool call loop is implemented as a follow-up within the same phase if time permits.

## Compilation

### IR Changes

The `IrTool` struct gains a `tool_schemas` field:

```json
{
  "tools": [
    {
      "name": "Calculator",
      "module": "main",
      "methods": [ ... ],
      "tool_schemas": [
        {
          "method_name": "Calculator::add",
          "description": "Add two numbers together",
          "parameters": { ... }
        }
      ]
    }
  ]
}
```

### Codegen

The `generate_tool()` function in `emitter.rs` is extended to:
1. Iterate over each method in the tool
2. Extract `@describe` decorator text
3. Extract `@param` decorators for each parameter
4. Build JSON Schema from parameter types
5. Emit `ToolSchemaEntry` objects

### Semantic Analysis

- `with_tools()` argument must be an array of tool/MCP references
- `without_tools()` takes no arguments
- Both return `AgentBuilder`

## Runtime

### Tool Schema Resolution

When building a `ChatRequest` with dynamic tools:

```
build_chat_request_with_builder(agent, prompt, builder, response_format):
    tool_schemas = []

    if !builder.exclude_default_tools:
        // Agent's static MCP tools
        for tool_ref in agent.tools:
            tool_schemas += mcp_registry.get_tool_schemas(tool_ref)

    // Dynamic tools from with_tools()
    for tool_name in builder.extra_tools:
        if mcp_registry.has_server(tool_name):
            tool_schemas += mcp_registry.get_tool_schemas(tool_name)
        elif module.tools.has(tool_name):
            tool_schemas += module.tools[tool_name].tool_schemas

    // Deduplicate by name
    deduplicate(tool_schemas)

    return ChatRequest { ..., tools: tool_schemas }
```

### VM Dispatch

- `AgentRef.with_tools(array)` -> creates `AgentBuilder` with `extra_tools` set
- `AgentRef.without_tools()` -> creates `AgentBuilder` with `exclude_default_tools: true`
- `AgentBuilder.with_tools(array)` -> extends `extra_tools` on existing builder
- `AgentBuilder.without_tools()` -> sets `exclude_default_tools: true` on existing builder

## Examples

### Adding a Specialized Tool

```concerto
tool Summarizer {
    @describe("Summarize text to a given length")
    pub fn summarize(
        @param("Text to summarize") text: String,
        @param("Maximum word count") max_words: Int,
    ) -> String {
        // implementation
    }
}

agent Researcher {
    provider: openai,
    model: "gpt-4o",
    tools: [WebSearch],
}

// Research with web search + summarization
let result = Researcher.with_tools([Summarizer]).execute("Research quantum computing");
```

### Tool-Free Execution

```concerto
agent Writer {
    provider: openai,
    model: "gpt-4o",
    tools: [WebSearch, Calculator],
}

// Creative writing -- no tools needed
let poem = Writer.without_tools().execute("Write a poem about the ocean");
```
