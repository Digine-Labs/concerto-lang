# 16 - IR Specification

## Overview

The Intermediate Representation (IR) is the bridge between the Concerto compiler and runtime. The compiler outputs IR; the runtime executes it. The IR is a JSON-based, stack-machine instruction set designed for readability, debuggability, and ease of implementation.

## IR Format

IR files use the `.conc-ir` extension and contain a JSON object with the following top-level structure:

```json
{
    "version": "0.1.0",
    "module": "main",
    "source_file": "main.conc",
    "constants": [...],
    "types": [...],
    "functions": [...],
    "agents": [...],
    "tools": [...],
    "mcp_connections": [...],
    "schemas": [...],
    "connections": [...],
    "databases": [...],
    "pipelines": [...],
    "source_map": {...},
    "metadata": {...}
}
```

## IR Sections

### Version

```json
{
    "version": "0.1.0"
}
```

Semantic versioning for IR format compatibility. The runtime checks this before execution.

### Constants

Literal values extracted from the source code into a constant pool.

```json
{
    "constants": [
        { "index": 0, "type": "string", "value": "Hello, World!" },
        { "index": 1, "type": "int", "value": 42 },
        { "index": 2, "type": "float", "value": 3.14159 },
        { "index": 3, "type": "bool", "value": true },
        { "index": 4, "type": "nil", "value": null },
        { "index": 5, "type": "string", "value": "Classify this document: " }
    ]
}
```

### Types

Type definitions used in the program.

```json
{
    "types": [
        {
            "name": "Classification",
            "kind": "schema",
            "fields": [
                { "name": "label", "type": "string", "required": true },
                { "name": "confidence", "type": "float", "required": true },
                { "name": "reasoning", "type": "string", "required": true }
            ]
        },
        {
            "name": "ProcessError",
            "kind": "enum",
            "variants": [
                { "name": "InvalidInput", "data": [{ "type": "string" }] },
                { "name": "Timeout", "data": [] }
            ]
        },
        {
            "name": "UserProfile",
            "kind": "struct",
            "fields": [
                { "name": "name", "type": "string" },
                { "name": "age", "type": "int" }
            ]
        }
    ]
}
```

### Functions

Compiled function bodies containing IR instructions.

```json
{
    "functions": [
        {
            "name": "main",
            "module": "main",
            "visibility": "private",
            "params": [],
            "return_type": "nil",
            "is_async": false,
            "locals": ["response", "result"],
            "instructions": [
                { "op": "LOAD_CONST", "arg": 5, "span": [1, 0] },
                { "op": "CALL_AGENT", "agent": "Classifier", "method": "execute", "argc": 1, "span": [2, 0] },
                { "op": "STORE_LOCAL", "name": "response", "span": [2, 0] },
                { "op": "LOAD_LOCAL", "name": "response", "span": [3, 0] },
                { "op": "LOAD_CONST", "arg": 0, "span": [3, 0] },
                { "op": "EMIT", "span": [3, 0] },
                { "op": "RETURN", "span": [4, 0] }
            ]
        },
        {
            "name": "classify",
            "module": "main",
            "visibility": "public",
            "params": [{ "name": "text", "type": "string" }],
            "return_type": { "result": ["Classification", "AgentError"] },
            "is_async": true,
            "locals": ["response", "parsed"],
            "instructions": [...]
        }
    ]
}
```

### Agents

Agent definitions with their configuration.

```json
{
    "agents": [
        {
            "name": "Classifier",
            "module": "main",
            "connection": "openai",
            "config": {
                "model": "gpt-4o",
                "temperature": 0.2,
                "max_tokens": 500,
                "system_prompt": "You are a document classifier.",
                "timeout": 30
            },
            "tools": ["FileConnector"],
            "memory": "shared_memory",
            "decorators": [
                { "name": "retry", "args": { "max": 3, "backoff": "exponential" } }
            ],
            "methods": [
                {
                    "name": "classify_with_threshold",
                    "params": [{ "name": "text", "type": "string" }, { "name": "min_confidence", "type": "float" }],
                    "return_type": { "result": ["Classification", "AgentError"] },
                    "instructions": [...]
                }
            ]
        }
    ]
}
```

### Tools

Tool definitions with method signatures. Tool-level `description` and method-level `description`/`param_descriptions` come from the `description:` field and `@describe`/`@param` decorators respectively.

```json
{
    "tools": [
        {
            "name": "FileConnector",
            "module": "main",
            "description": "Read, write, and list files on the filesystem",
            "fields": [
                { "name": "base_path", "type": "string", "default": "." }
            ],
            "methods": [
                {
                    "name": "read_file",
                    "description": "Read the contents of a file at the given path",
                    "params": [
                        { "name": "path", "type": "string", "description": "Absolute or relative file path to read" }
                    ],
                    "return_type": { "result": ["string", "ToolError"] },
                    "instructions": [...]
                }
            ]
        }
    ]
}
```

### MCP Connections

MCP server declarations with typed tool interfaces. These are compiled from `mcp` blocks in the source. The runtime uses this section to connect to MCP servers and validate their tool schemas.

```json
{
    "mcp_connections": [
        {
            "name": "GitHubServer",
            "module": "main",
            "transport": "stdio",
            "command": "npx -y @modelcontextprotocol/server-github",
            "url": null,
            "tools": [
                {
                    "name": "search_repositories",
                    "description": "Search GitHub repositories by query",
                    "params": [
                        { "name": "query", "type": "string", "description": "Search query string" },
                        { "name": "max_results", "type": "int", "default": 10, "description": "Maximum number of results to return" }
                    ],
                    "return_type": { "result": ["array:Repository", "ToolError"] }
                },
                {
                    "name": "get_file_contents",
                    "description": "Get the contents of a file in a repository",
                    "params": [
                        { "name": "owner", "type": "string", "description": "Repository owner username" },
                        { "name": "repo", "type": "string", "description": "Repository name" },
                        { "name": "path", "type": "string", "description": "File path within the repository" }
                    ],
                    "return_type": { "result": ["string", "ToolError"] }
                }
            ]
        }
    ]
}
```

### Schemas

Schema definitions with JSON Schema representation.

```json
{
    "schemas": [
        {
            "name": "Classification",
            "json_schema": {
                "type": "object",
                "properties": {
                    "label": { "type": "string" },
                    "confidence": { "type": "number" },
                    "reasoning": { "type": "string" }
                },
                "required": ["label", "confidence", "reasoning"]
            },
            "validation_mode": "strict"
        }
    ]
}
```

### Connections

Provider connection configurations.

```json
{
    "connections": [
        {
            "name": "openai",
            "config": {
                "api_key_env": "OPENAI_API_KEY",
                "base_url": "https://api.openai.com/v1",
                "default_model": "gpt-4o",
                "timeout": 60,
                "retry": { "max_attempts": 3, "backoff": "exponential" }
            }
        }
    ]
}
```

### Databases

In-memory database declarations.

```json
{
    "databases": [
        {
            "name": "shared_memory",
            "key_type": "string",
            "value_type": "any",
            "persistence": null
        }
    ]
}
```

### Pipelines

Pipeline definitions with stage sequences.

```json
{
    "pipelines": [
        {
            "name": "DocumentProcessor",
            "stages": [
                {
                    "name": "extract",
                    "input_type": "string",
                    "output_type": "string",
                    "instructions": [...]
                },
                {
                    "name": "classify",
                    "input_type": "string",
                    "output_type": "Classification",
                    "decorators": [{ "name": "timeout", "args": { "seconds": 30 } }],
                    "instructions": [...]
                }
            ]
        }
    ]
}
```

## Instruction Set

### Stack Operations

| Opcode | Args | Description |
|--------|------|-------------|
| `PUSH` | value | Push immediate value onto stack |
| `POP` | - | Pop top of stack |
| `DUP` | - | Duplicate top of stack |
| `SWAP` | - | Swap top two stack values |

### Constants

| Opcode | Args | Description |
|--------|------|-------------|
| `LOAD_CONST` | index | Push constant from pool onto stack |

### Variables

| Opcode | Args | Description |
|--------|------|-------------|
| `LOAD_LOCAL` | name | Push local variable value onto stack |
| `STORE_LOCAL` | name | Pop stack top and store in local variable |
| `LOAD_GLOBAL` | name | Push global/module variable onto stack |
| `STORE_GLOBAL` | name | Store in global/module variable |

### Arithmetic

| Opcode | Args | Description |
|--------|------|-------------|
| `ADD` | - | Pop two, push sum |
| `SUB` | - | Pop two, push difference |
| `MUL` | - | Pop two, push product |
| `DIV` | - | Pop two, push quotient |
| `MOD` | - | Pop two, push remainder |
| `NEG` | - | Pop one, push negation |

### Comparison

| Opcode | Args | Description |
|--------|------|-------------|
| `EQ` | - | Pop two, push equality result (Bool) |
| `NEQ` | - | Pop two, push inequality result |
| `LT` | - | Pop two, push less-than result |
| `GT` | - | Pop two, push greater-than result |
| `LTE` | - | Pop two, push less-or-equal result |
| `GTE` | - | Pop two, push greater-or-equal result |

### Logical

| Opcode | Args | Description |
|--------|------|-------------|
| `AND` | - | Pop two, push logical AND |
| `OR` | - | Pop two, push logical OR |
| `NOT` | - | Pop one, push logical NOT |

### Control Flow

| Opcode | Args | Description |
|--------|------|-------------|
| `JUMP` | offset | Unconditional jump to instruction offset |
| `JUMP_IF_TRUE` | offset | Jump if stack top is true (pops) |
| `JUMP_IF_FALSE` | offset | Jump if stack top is false (pops) |
| `RETURN` | - | Return from function (stack top is return value) |

### Function Calls

| Opcode | Args | Description |
|--------|------|-------------|
| `CALL` | name, argc | Call function with argc args from stack |
| `CALL_METHOD` | name, argc | Call method on stack-top object |
| `CALL_NATIVE` | name, argc | Call native/built-in function |

### Agent Operations

| Opcode | Args | Description |
|--------|------|-------------|
| `CALL_AGENT` | agent, method, argc | Call agent method (prompt on stack) |
| `CALL_AGENT_SCHEMA` | agent, schema, argc | Call agent with schema validation |
| `CALL_AGENT_STREAM` | agent, argc | Call agent in streaming mode |
| `CALL_AGENT_CHAT` | agent, argc | Call agent with message history |

### Tool Operations

| Opcode | Args | Description |
|--------|------|-------------|
| `CALL_TOOL` | tool, method, argc | Invoke tool method |

### Database Operations

| Opcode | Args | Description |
|--------|------|-------------|
| `DB_GET` | db_name | Get value (key on stack, pushes Option) |
| `DB_SET` | db_name | Set value (key and value on stack) |
| `DB_DELETE` | db_name | Delete entry (key on stack) |
| `DB_HAS` | db_name | Check existence (key on stack, pushes Bool) |
| `DB_QUERY` | db_name | Query with predicate (closure on stack) |

### Emit

| Opcode | Args | Description |
|--------|------|-------------|
| `EMIT` | - | Fire-and-forget emit (channel and payload on stack) |
| `EMIT_AWAIT` | - | Bidirectional emit (channel and payload on stack, pushes response) |

### Error Handling

| Opcode | Args | Description |
|--------|------|-------------|
| `TRY_BEGIN` | catch_offset | Mark start of try block, register catch handler |
| `TRY_END` | - | Mark end of try block (no error occurred) |
| `CATCH` | error_type | Begin catch block for specific error type |
| `THROW` | - | Throw error (error value on stack) |
| `PROPAGATE` | - | `?` operator: unwrap Ok or return Err |

### Object / Array / Map

| Opcode | Args | Description |
|--------|------|-------------|
| `BUILD_ARRAY` | count | Pop count values, push Array |
| `BUILD_MAP` | count | Pop count key-value pairs, push Map |
| `BUILD_STRUCT` | type_name, count | Pop count field values, push Struct |
| `FIELD_GET` | name | Pop object, push field value |
| `FIELD_SET` | name | Pop object and value, set field |
| `INDEX_GET` | - | Pop collection and index, push value |
| `INDEX_SET` | - | Pop collection, index, and value; set index |

### Type Operations

| Opcode | Args | Description |
|--------|------|-------------|
| `CHECK_TYPE` | type_name | Pop value, push Bool (is instance?) |
| `CAST` | type_name | Pop value, push casted value (or error) |

### Async Operations

| Opcode | Args | Description |
|--------|------|-------------|
| `AWAIT` | - | Await async value on stack top |
| `AWAIT_ALL` | count | Await count async values, push tuple of results |
| `SPAWN_ASYNC` | - | Spawn async task from closure on stack |

## Source Maps

The IR includes source maps for mapping instructions back to source code positions for error reporting and debugging:

```json
{
    "source_map": {
        "file": "main.conc",
        "mappings": [
            { "instruction": 0, "line": 15, "column": 4 },
            { "instruction": 1, "line": 16, "column": 4 },
            { "instruction": 5, "line": 17, "column": 4 }
        ]
    }
}
```

Each instruction carries a `span` field with `[line, column]` for inline reference. The source map provides the full mapping for tools that need it.

## IR Metadata

```json
{
    "metadata": {
        "compiler_version": "0.1.0",
        "compiled_at": "2026-02-07T10:30:00Z",
        "optimization_level": 0,
        "debug_info": true,
        "entry_point": "main"
    }
}
```

## IR Versioning

The runtime checks the IR `version` field for compatibility:
- **Major version mismatch**: refuse to execute
- **Minor version mismatch**: warn but execute (backward compatible)
- **Patch version mismatch**: silent (no behavioral change)
