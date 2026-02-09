# CLAUDE.md - Concerto Language Project Reference

> **MANDATORY RULE: This file MUST be updated whenever features, logic, specifications, or architectural decisions are added or changed. No exception. Every PR and every change session must verify this file is current.**

## Project Identity

- **Name**: Concerto Language
- **Purpose**: A programming language with Rust-like syntax for designing and orchestrating AI agent harnesses
- **Repository**: concerto-lang (github.com/Digine-Labs/concerto-lang)
- **Implementation Language**: Rust
- **File Extension**: `.conc` (source), `.conc-ir` (compiled IR, JSON format)
- **Status Tracking**: See [STATUS.md](STATUS.md) for current project state

## Architecture Overview

Concerto has three core components:

```
 .conc Source Code
       |
       v
 +-----------+
 | COMPILER  |  Lexer -> Parser -> AST -> Semantic Analysis -> IR Generation
 +-----------+
       |
       v
  .conc-ir (JSON-based Intermediate Representation)
       |
       v
 +-----------+
 |  RUNTIME  |  IR Loader -> VM Execution Loop
 +-----------+
       |
       +---> Agent Registry (LLM connections, model configs)
       +---> Tool Registry (registered tools, permissions)
       +---> Memory Manager (in-memory hashmaps, scoping)
       +---> Emit Channel (bidirectional output system)
       +---> Schema Validator (structured output validation)
       +---> Async Executor (concurrent agent calls)
       +---> Host Registry (external agent system adapters, stdio transport)
```

### Compiler Pipeline

```
Source (.conc) -> Lexer -> Tokens -> Parser -> AST -> Semantic Analysis -> Typed AST -> IR Generator -> IR (.conc-ir)
```

1. **Lexer**: Character scanning, tokenization, source position tracking
2. **Parser**: Recursive descent with Pratt parsing for expressions
3. **AST**: Abstract syntax tree with source spans -- 17 declaration types (connect removed, added MemoryDecl, HostDecl), decorators, config/typed fields, self params, memory/host declarations, 31 ExprKind variants (incl. Return expr, Listen), ListenHandler struct, 11 PatternKind variants, 6 Stmt variants, union/string-literal type annotations
4. **Semantic Analysis**: Two-pass resolver (collect decls, then walk bodies) + declaration validator. Name resolution with forward references, basic type checking (operators, conditions), control flow validation (break/continue/return/?/throw/.await), mutability checking, unused variable warnings, built-in symbols (emit, print, env, Some/None/Ok/Err, ToolError, HashMap, Ledger, Memory, Host, std). Manifest-sourced connection names registered as `SymbolKind::Connection`. `SymbolKind::Memory` and `SymbolKind::Host` for memory/host declarations. Tool methods implicitly async, pipeline stages implicitly async with Result return type, `self` not warned unused in tool methods
5. **IR Generation**: Full coverage lowering of all 17 declaration types (connect removed — connections come from Concerto.toml; added memory, host), all 6 statement types, all 30 expression types. Includes loop control flow (break w/ value, continue via patches), match pattern compilation (check + bind phases), try/catch/throw, closures (compiled as separate functions), pipe rewrite, ? propagation, ?? nil coalesce, string interpolation concat, struct/enum/pipeline/agent/tool/schema/hashmap/ledger/mcp/memory/host lowering to IR sections, return expression in match arms, schema union types to JSON Schema enum. Manifest connections embedded into IR via `add_manifest_connections()`

### Runtime Pipeline

```
IR (.conc-ir) -> IR Loader -> VM Execution Loop -> Output (emits, return value)
```

1. **IR Loader**: JSON deserialization → `LoadedModule` (constants, functions, agents, schemas, connections, hashmaps, ledgers, pipelines, memories, hosts, listens). Converts constant types, builds lookup HashMaps, registers qualified tool methods
2. **VM**: Stack-based execution with `CallFrame`s (function_name, instructions, pc, locals HashMap). Max call depth 1000. All 59 opcodes dispatched. `TryFrame` stack for exception handling. `run_loop_until(stop_depth)` for nested execution (pipeline stages, thunks). `call_stack_depth()` public API. Unknown functions return `NameError` (not Nil). `HashMapQuery` uses closure-based filtering. `memory_store: MemoryStore` for agent conversation memory. `host_registry: HostRegistry` for external agent system adapters. Tool and MCP refs are registered in globals so `with_tools([ToolName, McpServer])` identifier arrays resolve at runtime
3. **Value System**: 20 variants (Int, Float, String, Bool, Nil, Array, Map, Struct, Result, Option, Function, AgentRef, SchemaRef, HashMapRef, LedgerRef, PipelineRef, Thunk, MemoryRef, HostRef, AgentBuilder). Arithmetic with Int/Float promotion, string coercion in add, comparisons, truthiness, field/index access
4. **CALL convention**: Args pushed first, callee pushed last. VM pops callee, then N args
5. **CALL_METHOD convention**: Object pushed first, then args. VM pops N args, then object. Method name from instruction `name` field, schema from `schema` field
6. **LOAD_LOCAL**: Checks locals → globals → module.functions → path-based names → error
7. **LLM Providers**: `LlmProvider` trait (sync). OpenAI + Anthropic HTTP providers (reqwest::blocking). `ConnectionManager` resolves from IR connections. Explicit `provider` field from Concerto.toml; fallback name-based heuristics for legacy. Ollama support (no API key, localhost default). `resolve_api_key()` handles `api_key` (direct/`$env` ref) and `api_key_env` (TOML format). `MockProvider` fallback when no API key
8. **Agent Execution**: `execute()` → ChatRequest → provider → Response. `execute_with_schema()` → json_schema format → SchemaValidator (retry up to 3x) → typed struct. Decorator support: @retry (backoff), @timeout, @log
9. **Schema Validation**: `SchemaValidator` (jsonschema crate). Normalizes Concerto types → JSON Schema types. Retry prompt with error feedback
10. **Tool Dispatch**: `ToolRegistry` per-tool state. `CallTool` → qualified function `Tool::method` with self
11. **Try/Catch**: `TryFrame` stack (catch_pc, call_depth, stack_height). Throw unwinds. Typed catch. Propagate (?) routes through try/catch
12. **HashMap**: In-memory KV (HashMap<String, HashMap<String, Value>>). set/get/has/delete
13. **Ledger**: LedgerStore (in-memory Vec<LedgerEntry> per ledger). Three-field document model (identifier, keys, value). Word-containment identifier queries (tokenize + case-insensitive ALL-match). Case-insensitive key queries (exact single, OR, AND). Mutations (insert/upsert, delete, update, update_keys). Scoping via `"name::prefix"` namespacing. Query builder: `query()` returns same `LedgerRef` for chaining. Returns `LedgerEntry` structs
14. **Emit**: Pops channel + payload, invokes callback. Custom handler via `set_emit_handler()`
15. **Built-ins**: Ok, Err, Some, None, env, print, println, len, typeof, panic, ToolError::new
16. **Decorators**: decorator.rs — @retry (max attempts, exponential/linear/none backoff), @timeout (seconds), @log (emit event). Applied to agents and pipeline stages
17. **Pipeline Lifecycle**: Full lifecycle emits (pipeline:start/stage_start/stage_complete/error/complete). Stage @retry/@timeout decorators. Result unwrapping. Error short-circuit. Duration tracking
18. **Async Foundations**: Thunk value (deferred computation). SpawnAsync creates thunk, Await resolves synchronously, AwaitAll collects results. True parallel execution deferred
19. **MCP Client**: mcp.rs — McpClient (stdio JSON-RPC 2.0 transport), McpRegistry (manages connections). Tool discovery via tools/list, tool schemas included in ChatRequest for LLM function calling
20. **Standard Library**: stdlib/ module with 12 sub-modules. VM dispatches `std::*` calls via `call_stdlib()`. Collections (Set/Queue/Stack) as Value::Struct with immutable method semantics, dispatched via exec_call_method. Modules: math (11 fns), string (17 fns), env (4 fns), time (3 fns), json (4 fns), fmt (5 fns), log (4 fns), fs (7 fns), collections (3 types + 20 methods), http (5 fns), crypto (4 fns), prompt (3 fns)
21. **Agent Memory**: MemoryStore (HashMap of MemoryInstance per memory name). Sliding window (configurable max_messages). Methods: append/messages/last/clear/len. Memory injected into ChatRequest between system_prompt and user_prompt. Auto-append mode for agent conversations
22. **AgentBuilder**: Transient builder pattern value (Value::AgentBuilder) for chaining with_memory/with_tools/with_context/execute. BuilderSourceKind (Agent | Host) enables shared builder interface for both agent and host execution. Agent builder `execute()` returns `Result<Response, String>` shape for consistency with direct agent calls
23. **Dynamic Tool Binding**: Compile-time ToolSchemaEntry generation from @describe/@param decorators. with_tools()/without_tools() on AgentBuilder for runtime tool selection. Merged tool schema resolution at execution time
24. **Hosts**: HostClient (stdio subprocess transport), HostRegistry (manages connections), HostFormat (Text|Json). Stateful long-running processes. execute/with_memory/with_context support via AgentBuilder. IrHost embeds TOML config from Concerto.toml
25. **Host Streaming**: `listen` expression for bidirectional NDJSON message loops. ListenBegin opcode dispatches to `exec_listen_begin()` + `run_listen_loop()`. Persistent BufReader for multi-message reads. Handler instructions compiled as instruction blocks (pipeline stage pattern). Non-nil handler returns sent back to host as `{"type":"response","in_reply_to":"...","value":"..."}`. Terminal messages: `result` (returns value) and `error` (returns error). Lifecycle emits: listen:start, listen:complete, listen:error, listen:unhandled

## Directory Structure

```
concerto-lang/
  CLAUDE.md              # This file - project reference (MUST stay updated)
  STATUS.md              # Project ledger - task tracking and decisions
  README.md              # Public-facing project description
  .gitignore             # Git ignore rules
  spec/                  # Language specifications (source of truth)
    00-overview.md       # Design philosophy, goals, compilation model
    01-lexical-structure.md  # Tokens, keywords, literals, operators
    02-type-system.md    # Primitives, compounds, AI-specific types
    03-variables-and-bindings.md  # let, mut, const, destructuring
    04-operators-and-expressions.md  # Arithmetic, pipe, error prop
    05-control-flow.md   # if/else, match, for/while/loop, pipeline
    06-functions.md      # fn, async fn, closures, doc comments
    07-agents.md         # Agent definition, execution, composition
    08-tools.md          # Tool definition, runtime bindings, permissions
    09-memory-and-databases.md  # hashmap keyword, HashMap<K,V>, scoping
    10-emit-system.md    # emit(), channels, bidirectional, host API
    11-llm-connections.md    # connect blocks, providers, config
    12-schema-validation.md  # schema keyword, validation modes
    13-error-handling.md     # Result/?, try/catch, error hierarchy
    14-modules-and-imports.md  # use, pub, mod, std:: library
    15-concurrency-and-pipelines.md  # async/await, pipeline/stage
    16-ir-specification.md   # IR format, instruction set, sections
    17-runtime-engine.md     # VM architecture, components, host API
    18-compiler-pipeline.md  # Lexer, parser, AST, semantic, IR gen
    19-standard-library.md   # std:: modules and functions
    20-interop-and-ffi.md    # Host bindings, FFI, WASM, MCP
    21-ledger.md             # Fault-tolerant knowledge store for AI agents
    22-project-manifest.md   # Concerto.toml manifest format
    23-project-scaffolding.md  # concerto init command
    27-host-streaming.md     # Bidirectional host streaming (listen expression)
  examples/              # Example projects (each has Concerto.toml + src/main.conc)
    hello_agent/         # Minimal agent example
    tool_usage/          # Tool definition and usage
    multi_agent_pipeline/  # Multi-stage pipeline with multiple providers
    agent_memory_conversation/ # Spec 24 memory conversation patterns
    dynamic_tool_binding/ # Spec 25 with_tools/without_tools composition
    host_streaming/      # Spec 27 bidirectional host streaming with listen
    bidirectional_host_middleware/ # Spec 27 end-to-end middleware test with local host process
  Cargo.toml             # Workspace root
  crates/
    concerto-common/     # Shared types (Span, Diagnostic, IR types, Opcodes, Manifest)
      src/lib.rs, span.rs, errors.rs, ir.rs, ir_opcodes.rs, manifest.rs
    concerto-compiler/   # Compiler library (lexer, parser, AST, semantic, codegen)
      src/
        lib.rs
        lexer/mod.rs, token.rs, cursor.rs, scanner.rs
        ast/mod.rs, nodes.rs, types.rs, visitor.rs
        parser/mod.rs, declarations.rs, statements.rs, expressions.rs
        semantic/mod.rs, scope.rs, types.rs, resolver.rs, type_checker.rs, validator.rs
        codegen/mod.rs, emitter.rs, constant_pool.rs
    concertoc/           # Compiler CLI binary
      src/main.rs
    concerto-runtime/    # Runtime library (Phase 4 complete)
      src/
        lib.rs, error.rs, value.rs, ir_loader.rs, vm.rs, builtins.rs
        ledger.rs        # LedgerStore (fault-tolerant knowledge store, word-containment queries)
        memory.rs        # MemoryStore (agent conversation memory, sliding window)
        host.rs          # HostClient (external agent system adapters, stdio transport)
        provider.rs      # LlmProvider trait, ChatRequest/Response, MockProvider, ConnectionManager
        providers/mod.rs, openai.rs, anthropic.rs  # HTTP LLM providers
        schema.rs        # SchemaValidator (jsonschema validation, type normalization, retry)
        tool.rs          # ToolRegistry (per-tool instance state)
        decorator.rs     # @retry/@timeout/@log decorator parsing and application
        mcp.rs           # MCP JSON-RPC client (stdio), McpRegistry, tool discovery
        stdlib/          # Standard library (12 modules, 87 functions)
          mod.rs         # Router: call_stdlib() dispatches by module path
          math.rs, string.rs, env.rs, time.rs, json.rs, fmt.rs
          log.rs, fs.rs, collections.rs, http.rs, crypto.rs, prompt.rs
    concerto-runtime/
      tests/
        integration.rs   # 15 end-to-end compile→run tests
    concerto/            # Runtime CLI binary (depends on both compiler + runtime)
      src/main.rs        # `concerto run` (direct .conc or .conc-ir) + `concerto init` (#[tokio::main])
  tests/
    fixtures/            # Test .conc source files
      minimal.conc       # Milestone program for end-to-end testing
```

## Conventions

### Workflow
- **Spec-first**: All language features are specified in `spec/` BEFORE implementation
- **spec/ is the source of truth** for language semantics
- **STATUS.md** tracks all tasks, phases, and decisions
- **CLAUDE.md** (this file) must be updated with every behavioral change

### Commit Messages
Format: `<component>: <description>`
```
compiler: add lexer for string literals
runtime: implement emit channel system
spec: add schema validation specification
docs: update README with installation guide
```

### Code Style (Rust)
- Follow standard Rust conventions (`rustfmt`, `clippy`)
- Use `Result<T, E>` for fallible operations
- Comprehensive error types with `thiserror`
- Tests alongside source in `#[cfg(test)]` modules

## Type System Quick Reference

### Primitive Types
| Type | Description |
|------|-------------|
| `Int` | 64-bit signed integer |
| `Float` | 64-bit floating point |
| `String` | UTF-8 string |
| `Bool` | `true` / `false` |
| `Nil` | Absence of value |

### Compound Types
| Type | Description |
|------|-------------|
| `Array<T>` | Ordered collection |
| `Map<K, V>` | Key-value pairs |
| `Tuple<T1, T2, ...>` | Fixed-size heterogeneous |
| `Option<T>` | `Some(value)` or `None` |
| `Result<T, E>` | `Ok(value)` or `Err(error)` |

### AI-Specific Types
| Type | Description |
|------|-------------|
| `Prompt` | Typed prompt string with metadata |
| `Response` | LLM response with text, tokens, model info |
| `Schema<T>` | Expected output schema for validation |
| `Message` | Chat message with role and content |
| `ToolCall` | Tool invocation request from LLM |
| `AgentRef` | Reference to running agent instance |
| `HashMapRef` | Reference to in-memory hash map |
| `LedgerRef` | Reference to fault-tolerant knowledge store |
| `MemoryRef` | Reference to conversation memory store |
| `HostRef` | Reference to external agent system adapter |

### User-Defined Types
- `struct` - Product types with named fields
- `enum` - Sum types / tagged unions
- `trait` - Interfaces / capability contracts

## Keyword Reference

```
let    mut    fn     agent   tool    pub     use     mod
if     else   match  for     while   loop    break   continue
return try    catch  throw   emit    await   async   pipeline
stage  schema hashmap self   impl    trait   enum    struct
as     in     with   true    false   nil     const   type
mcp    ledger memory  host   listen
```

## Built-in Functions

| Function | Description |
|----------|-------------|
| `emit(channel, payload)` | Output to named channel (bidirectional with `await`) |
| `env(name)` | Read environment variable |
| `print(value)` | Debug print to stdout |
| `panic(message)` | Unrecoverable error, halt execution |
| `typeof(value)` | Returns type name as string |
| `len(collection)` | Returns length of array, string, or map |

## Key Design Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Rust for implementation | Performance, strong type system, aligns with Rust-like syntax |
| 2 | JSON-based IR | Human-readable, debuggable; binary format deferred to optimization phase |
| 3 | Stack-based VM | Simpler to implement, well-understood execution model |
| 4 | Bidirectional emit | Enables human-in-the-loop patterns and external tool execution via host |
| 5 | First-class pipelines | `pipeline`/`stage` keywords for declarative multi-agent workflows |
| 6 | Dual error handling | `Result<T,E>` with `?` for functional style + `try`/`catch` for imperative |
| 7 | Static typing with inference | Catch errors at compile time, reduce annotation burden via inference |
| 8 | Async-only agent execution | All LLM calls are inherently async; forced async prevents footguns |
| 9 | Spec-first development | All features fully specified before implementation begins |
| 10 | `@describe`/`@param` for tool descriptions | Compiler-enforced decorators replace fragile doc comments; descriptions are language grammar, not comments |
| 11 | First-class `mcp` construct | MCP tool interfaces declared in source with typed signatures; compile-time type checking, runtime schema validation |
| 12 | Generic method call syntax | `method<Type>(args)` with lookahead disambiguation from comparison; type args as schema on CALL_METHOD |
| 13 | Phase 3a mock-first approach | No tokio/async in Phase 3a; AWAIT is no-op; agents return mock responses; full end-to-end without HTTP |
| 14 | First-class `ledger` keyword | Fault-tolerant knowledge store for AI agents. Separate from `hashmap` (exact-key state). Identifier + Keys + Value document model with word-containment similarity matching and case-insensitive tag queries |
| 15 | Synchronous LlmProvider trait | Uses reqwest::blocking for simplicity. Async deferred to Phase 3c. tokio added for CLI + future async |
| 16 | Trait-based provider with MockProvider fallback | MockProvider auto-selected when no API key. Real providers need env vars (OPENAI_API_KEY etc.) |
| 17 | Schema type normalization at runtime | Compiler emits Concerto types (String, Int, Array<T>). Runtime normalizes to JSON Schema types before jsonschema validation |
| 18 | run_loop_until(stop_depth) for nested execution | Pipeline stages and thunks call run_loop_until to prevent executing caller's instructions after RETURN |
| 19 | Thunk-based async foundations | SpawnAsync creates Value::Thunk (deferred computation), Await resolves synchronously. True parallel deferred |
| 20 | MCP stdio JSON-RPC transport | McpClient spawns subprocess, communicates via JSON-RPC 2.0 on stdin/stdout. Tool schemas fed to LLM ChatRequest |
| 21 | `Concerto.toml` project manifest | Connections defined in TOML (like Cargo.toml), not in source code. Compiler embeds connection config into IR at compile time. `connect` keyword removed |
| 22 | `concerto init` scaffolding | Creates project structure (Concerto.toml + src/main.conc + .gitignore). Supports openai/anthropic/ollama providers. Generates working hello-world agent |
| 23 | Agent Memory with sliding window | Auto-append by default. Memory injected into ChatRequest between system_prompt and user_prompt. Configurable max_messages for sliding window |
| 24 | AgentBuilder pattern for chained configuration | Shared transient value for Agent/Host. with_memory/with_tools/without_tools/with_context chain to .execute() |
| 25 | Compile-time tool schema generation | @describe/@param decorators → JSON Schema ToolSchemaEntry in IR. Dynamic binding at execution time |
| 26 | Hosts as external agent adapters | Stdio subprocess transport. Stateful processes. IrHost embeds TOML config. Same builder interface as agents |
| 27 | Bidirectional host streaming (`listen`) | `listen Host.execute("prompt") { "type" => \|param\| { body } }` for NDJSON message loops. Handler return values sent back to host. Persistent BufReader for multi-message streaming. `result`/`error` are terminal message types |
| 28 | Direct run (`concerto run file.conc`) | CLI compiles `.conc` in-memory and executes directly — no intermediate `.conc-ir` file. Detects extension to choose path. `.conc-ir` still supported for pre-compiled files |
