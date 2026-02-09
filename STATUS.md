# STATUS.md - Concerto Language Project Ledger

> **Last updated**: 2026-02-09

## Current Focus

**Phase 1–7e**: COMPLETE. See sections below.
**Direct Run**: COMPLETE. `concerto run file.conc` compiles and executes in one step. 515 tests total (10 manifest + 240 compiler + 238 runtime + 27 integration).

---

## Recent Development Log

### 2026-02-09 - Bidirectional Host Middleware Example

| Task | Status | Notes |
|------|--------|-------|
| Add self-contained bidirectional host middleware example | Done | New project: `examples/bidirectional_host_middleware/` (`Concerto.toml`, `src/main.conc`, `README.md`) |
| Add local mock host middleware process | Done | `examples/bidirectional_host_middleware/host/mock_external_agent.sh` implements NDJSON host→Concerto and response parsing Concerto→host |
| Verify example compile and runtime execution | Done | Compiled with `concertoc` and ran with `concerto run`; observed progress/question/approval/result flow |
| Fix host_streaming connector/manifest alignment | Done | Updated `examples/host_streaming/Concerto.toml` to `[hosts.claude_code]` with `transport` and timeout |

### 2026-02-08 - Advanced Example Projects

| Task | Status | Notes |
|------|--------|-------|
| Add ledger trial/error harness example | Done | `examples/ledger_trial_error_harness/src/main.conc` + manifest |
| Add low-model schema retry/fallback example | Done | `examples/schema_retry_fallback/src/main.conc` + manifest |
| Add multi-agent quality scoring loop example | Done | `examples/multi_agent_quality_loop/src/main.conc` + manifest |
| Add pipeline + ledger refinement example | Done | `examples/pipeline_refinement_with_ledger/src/main.conc` + manifest |
| Compile verification for all new examples | Done | All four compile successfully with `concertoc` |
| Bug report: builtin `len(...)` unresolved in semantic analysis | Done | `bugs/2026-02-08-len-builtin-unresolved.md` |
| Redesign quality loop example with first-class pipeline | Done | `examples/multi_agent_quality_loop/src/main.conc` now uses `pipeline MemoQualityPipeline` (`prepare -> iterate -> finalize`) |
| Pipeline runtime smoke verification | Done | Compiled+ran `/tmp/pipeline_runtime_smoke.conc` and observed lifecycle emits + final result (`18`) |
| New idea: iterative pipeline loop primitive | Done | `ideas/pipeline_iterative_loops.md` |
| New idea: attempt chains and recovery blocks | Done | `ideas/attempt_chains_and_recovery_blocks.md` |
| New idea: consensus and critic loops | Done | `ideas/consensus_and_critic_loops.md` |
| New idea: harness contracts and assertions | Done | `ideas/harness_contracts_and_assertions.md` |
| New idea: checkpoints and human approval gates | Done | `ideas/checkpoints_and_human_approval_gates.md` |
| Add language positioning/features document | Done | `FEATURES.md` with full feature map + Concerto vs LangChain-style comparison |
| Expand FEATURES examples with colorful snippets | Done | Added multi-feature code examples in `FEATURES.md` using `rust` fences for rendering |
| Add spec-24 example project (`agent_memory_conversation`) | Done | New project: `examples/agent_memory_conversation/` (`Concerto.toml` + `src/main.conc`) |
| Add spec-25 example project (`dynamic_tool_binding`) | Done | New project: `examples/dynamic_tool_binding/` (`Concerto.toml` + `src/main.conc`) |
| Strengthen integration coverage for memory/tool builder flows | Done | Added `e2e_agent_with_memory_builder_auto_and_manual_modes` and `e2e_dynamic_tool_binding_builder_paths` |
| Strengthen VM request composition tests for spec 24/25 | Done | Added unit tests for memory message injection ordering, static+dynamic tool schema merge/dedup, and `without_tools()` semantics |
| Fix runtime tool refs for `with_tools([ToolName])` | Done | `VM::new` now registers tool references in globals so identifier arrays resolve correctly |
| Fix compiler host IR emission after `IrHost` expansion | Done | `generate_host()` now initializes manifest-embedded host fields (`command`, `args`, `env`, `working_dir`) |
| Fix tool schema requiredness for `Option<T>` params | Done | Codegen now excludes `Option<T>` tool parameters from JSON Schema `required`; compiler test added |

---

## Phase 1: Foundation

| Task | Status | Notes |
|------|--------|-------|
| CLAUDE.md | Done | Project reference with mandatory update rule |
| STATUS.md | Done | This file |
| .gitignore | Done | Rust/Cargo, IDE, OS, .env, .conc-ir artifacts |
| README.md | Done | Project overview, example code, architecture |
| spec/00-overview.md | Done | Design philosophy, goals, compilation model |
| spec/01-lexical-structure.md | Done | Tokens, keywords, literals, operators, comments |
| spec/02-type-system.md | Done | Primitives, compounds, AI types, generics |
| spec/03-variables-and-bindings.md | Done | let, mut, const, destructuring, scoping |
| spec/04-operators-and-expressions.md | Done | All operators, pipe, precedence table |
| spec/05-control-flow.md | Done | if/else, match, loops, pipeline/stage |
| spec/06-functions.md | Done | fn, async fn, closures, defaults, doc comments |
| spec/07-agents.md | Done | Agent definition, execution, composition |
| spec/08-tools.md | Done | Tool definition, bindings, permissions |
| spec/09-memory-and-databases.md | Done | hashmap, HashMap<K,V>, scoping, queries |
| spec/10-emit-system.md | Done | emit(), channels, bidirectional, host API |
| spec/11-llm-connections.md | Done | connect blocks, providers, streaming |
| spec/12-schema-validation.md | Done | schema, validation modes, retry |
| spec/13-error-handling.md | Done | Result/?, try/catch, error hierarchy |
| spec/14-modules-and-imports.md | Done | use, pub, mod, std:: library |
| spec/15-concurrency-and-pipelines.md | Done | async/await, pipeline/stage, parallel |
| spec/16-ir-specification.md | Done | IR format, instruction set, sections |
| spec/17-runtime-engine.md | Done | VM architecture, components, host API |
| spec/18-compiler-pipeline.md | Done | Lexer, parser, AST, semantic, IR gen |
| spec/19-standard-library.md | Done | std:: modules with function signatures |
| spec/20-interop-and-ffi.md | Done | Host bindings, FFI, WASM, MCP |
| examples/hello_agent.conc | Done | Minimal agent example |
| examples/multi_agent_pipeline.conc | Done | Multi-stage pipeline example |
| examples/tool_usage.conc | Done | Tool definition and usage example |

## Phase 2: Compiler Implementation

| Task | Status | Notes |
|------|--------|-------|
| Cargo project scaffolding | Done | 5-crate workspace: common, compiler, concertoc, runtime (stub), concerto (stub) |
| Token types and lexer (core) | Done | 42 keywords, operators, literals, comments (nested block), 16 tests |
| AST node definitions (core) | Done | Program, FunctionDecl, LetStmt, ExprStmt, ReturnStmt, 11 ExprKind variants, Visitor |
| Parser (core) | Done | Recursive descent + Pratt parsing, 16 tests. fn, let, if/else, call, binary/unary, arrays, maps |
| IR generator (core) | Done | Constant pool with dedup, 59 opcodes, all core constructs lower to IR, 9 tests |
| Compiler CLI (`concertoc`) | Done | --emit-tokens, --emit-ast, --check, -o output. Compiles .conc -> .conc-ir JSON |
| Compiler test suite (core) | Done | 43 unit tests total, clippy clean, milestone program compiles end-to-end |
| Lexer - full coverage | Done | String interpolation (mode stack), multi-line strings, raw strings, hex/binary/octal ints, unicode escapes, `mcp` keyword, 42 keywords total, 74 tests |
| Parser - all declarations | Done | 16 declaration types (fn, connect, agent, tool, schema, pipeline, struct, enum, trait, impl, use, mod, const, type, hashmap, mcp), decorators (@name(args)), self/mut self params, 107 tests total |
| Parser - all statements & expressions | Done | for/while/loop, match (all pattern types), try/catch/throw, closures, pipe (|>), ? propagation, ?? nil coalesce, range (.., ..=), cast (as), path (::), .await, tuples, struct literals, string interpolation, break/continue, 143 tests total |
| Semantic analysis | Done | Name resolution (forward refs, scoping), type checking (operators, conditions, inference), control flow validation (break/continue in loops, return in fns, ?/throw in Result fns, .await in async), mutability checking, declaration validation (agent provider, tool description, @describe), unused variable warnings, built-ins (emit, print, env, Some/None/Ok/Err, ToolError, HashMap, std), 216 tests total |
| IR generator - full coverage | Done | All 16 declaration types lowered (agent, tool, schema, connect, pipeline, struct, enum, impl, trait, const, hashmap, mcp). All statement types (break w/ value, continue, throw). All 29 expression types (while/for/loop with break/continue, match with pattern check+bind, try/catch/throw, closures, pipe rewrite, ? propagation, ?? nil coalesce, range, cast, path, .await, tuples, struct literals, string interpolation). Loop result variables, pattern matching (literal/wildcard/identifier/or/range/binding/tuple/struct/enum/array), 216 tests total |
| Integration & polish | Done | All 3 examples compile end-to-end. Parser fixes: prefix `await expr`, `return` as expression (match arms), union types (`"a" \| "b"`). Semantic fixes: tool methods implicitly async, pipeline stages implicitly async with Result return, `self` not warned unused in tools. 222 tests total, clippy clean |
| Generic method calls | Done | Parser: `method<Type>(args)` parsed as MethodCall with type_args (lookahead disambiguates from comparison). AST: type_args field on MethodCall. Codegen: schema field on CALL_METHOD. 225 compiler tests total |

## Phase 3: Runtime Implementation

### Phase 3a: Core VM (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| Value system | Done | 15 Value variants (Int, Float, String, Bool, Nil, Array, Map, Struct, Result, Option, Function, AgentRef, SchemaRef, HashMapRef, PipelineRef). Arithmetic with type promotion, string coercion, comparisons, truthiness, field/index access. 16 tests |
| IR loader/decoder | Done | LoadedModule from JSON IR. Constants conversion, function/agent/tool/schema/connection/hashmap/pipeline tables. Qualified tool method registration. 2 tests |
| VM execution loop | Done | Stack-based dispatch of all 59 opcodes. CallFrame with locals HashMap. CALL convention (args then callee), CALL_METHOD (object then args). LOAD_LOCAL falls back to globals and function names. Max call depth 1000. 10 tests |
| Agent mock system | Done | Mock execute() returns Response struct with text/tokens/model. Mock execute_with_schema() populates fields from JSON Schema properties. Schema name passed via CALL_METHOD instruction |
| HashMap stubs | Done | In-memory KV (HashMap<String, HashMap<String, Value>>). set/get/has/delete operations via CALL_METHOD and HASHMAP_* opcodes |
| Emit channel system | Done | EMIT opcode pops channel + payload, invokes handler callback. Custom emit handler via set_emit_handler(). Default prints `[emit:channel] value` |
| Built-in functions | Done | Ok, Err, Some, None, env, print, println, len, typeof, panic, ToolError::new. Dispatched via $builtin_ prefix. 8 tests |
| Runtime host API | Done | lib.rs: run_file(path), VM::new(), VM::execute(), VM::set_emit_handler() |
| Runtime CLI (`concerto`) | Done | `concerto run <file.conc-ir> [--debug]`. Loads module, creates VM, executes, prints errors |
| Runtime test suite | Done | 36 tests: value arithmetic/comparison/truthiness/access, IR loading, VM opcodes (add, store/load, jumps, emit, calls, propagate, build_map, nil coalesce) |

### Phase 3b: Agent & Tool System (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| Try/catch exception handling | Done | TryFrame stack with catch_pc/call_depth/stack_height. Typed catch with skip logic. Propagate routes through try/catch. 7 tests |
| IndexSet, CheckType, Cast | Done | IndexSet (Array/Map), CheckType, Cast (Int/Float/String/Bool). 7 tests |
| Tool method dispatch | Done | ToolRegistry with per-tool state. CallTool via qualified function lookup. 2 tests |
| LlmProvider trait + deps | Done | tokio, reqwest (blocking), jsonschema. MockProvider + ConnectionManager. 3 tests |
| OpenAI + Anthropic providers | Done | HTTP providers with tool call support. Provider factory with auto-detection. 12 tests |
| Wire providers into VM | Done | ConnectionManager from IR connections. Agent calls use real providers with MockProvider fallback |
| Schema validation engine | Done | jsonschema crate validation, Concerto type normalization, retry prompt, json_to_value. 7 tests |
| Integration testing | Done | 299 tests (225 compiler + 74 runtime), clippy clean. Examples run with MockProvider |

### Phase 3c: Pipeline & Polish (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| IR Fix: IrPipelineStage.params | Done | Added params field to IR, compiler emits actual stage param names, runtime uses them |
| Decorator runtime | Done | decorator.rs: @retry (exponential/linear/none backoff), @timeout (seconds), @log (emit agent:log). 9 tests |
| Pipeline lifecycle events | Done | pipeline:start, pipeline:stage_start, pipeline:stage_complete, pipeline:error, pipeline:complete emits with duration tracking |
| Pipeline error handling | Done | Stage @retry/@timeout decorators, Result unwrapping, error short-circuit |
| Async foundations | Done | Value::Thunk variant. SpawnAsync creates thunk, Await resolves synchronously, AwaitAll collects. 4 tests |
| MCP client | Done | mcp.rs: McpClient (stdio JSON-RPC 2.0), McpRegistry, tool discovery, tool schemas for LLM. 8 tests |
| VM nested execution fix | Done | run_loop_until(stop_depth) prevents pipeline stages from executing caller's instructions |
| Mock provider enum fix | Done | mock_json_from_schema respects JSON Schema enum constraints |
| Integration testing | Done | 320 tests (225 compiler + 95 runtime), clippy clean. All 3 examples run end-to-end with full pipeline lifecycle |

### Phase 3d: Ledger System

| Task | Status | Notes |
|------|--------|-------|
| spec/21-ledger.md | Done | Full specification: data model, query API, compilation, runtime |
| Compiler: `ledger` keyword + parser | Done | Lexer keyword, LedgerDecl AST node, parser, semantic validation (SymbolKind::Ledger, Type::LedgerRef) |
| Compiler: IR generation | Done | IrLedger struct, `ledgers` IR section, generate_ledger() in codegen |
| Runtime: LedgerRef value + store | Done | LedgerEntry struct, LedgerStore (in-memory Vec), Value::LedgerRef variant |
| Runtime: insert/delete/update | Done | Upsert insert, delete, update value, update_keys — all via CALL_METHOD dispatch |
| Runtime: from_identifier query | Done | Word-tokenization + case-insensitive ALL-words containment matching |
| Runtime: from_key / from_any_keys / from_exact_keys | Done | Exact case-insensitive key matching (single, OR, AND semantics) |
| Runtime: scoping | Done | Namespaced ledger views via "name::prefix" convention in LedgerStore |
| Runtime: query builder pattern | Done | query() returns same LedgerRef, from_* performs query — two CALL_METHOD dispatches |
| Ledger test suite | Done | 18 unit tests covering all query modes, mutations, edge cases, scoping |

## Phase 4: Standard Library (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| stdlib scaffold + VM integration | Done | stdlib/mod.rs router, vm.rs std:: dispatch in exec_call + collections dispatch in exec_call_method, lib.rs pub mod stdlib |
| std::math | Done | abs, min, max, clamp, round, floor, ceil, pow, sqrt, random, random_int. 16 tests |
| std::string | Done | split, join, trim, trim_start, trim_end, replace, to_upper, to_lower, contains, starts_with, ends_with, substring, len, repeat, reverse, parse_int, parse_float. 16 tests |
| std::env | Done | get (Option), require (Result), all (Map), has (Bool). 6 tests |
| std::time | Done | now (ISO 8601), now_ms (epoch millis), sleep. Manual Gregorian calendar arithmetic. 6 tests |
| std::json | Done | parse (reuses SchemaValidator::json_to_value), stringify (reuses Value::to_json), stringify_pretty, is_valid. 8 tests |
| std::fmt | Done | format ({} sequential replacement), pad_left, pad_right, truncate, indent. 8 tests |
| std::log | Done | info, warn, error, debug — eprintln!("[LEVEL]"). 5 tests |
| std::fs | Done | read_file, write_file, append_file, exists, list_dir, remove_file, file_size. 8 tests |
| std::collections | Done | Set/Queue/Stack as Value::Struct with immutable semantics. Constructors + 20 methods. 12 tests |
| std::http | Done | get, post, put, delete, request — uses reqwest::blocking. HttpResponse struct. 6 tests |
| std::crypto | Done | sha256 (sha2), md5 (md-5), uuid (uuid v4), random_bytes. 5 tests |
| std::prompt | Done | template (${var} substitution), from_file (with optional vars), count_tokens (word heuristic). 5 tests |

## Phase 5: Integration and Polish (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| Runtime robustness | Done | Replaced 5 unwraps in vm.rs with proper error returns, implemented HashMapQuery with closure-based filtering, error on unknown functions (was Nil), mock provider fallback warning, removed dead code (stack_base), added call_stack_depth() |
| Compiler error quality | Done | ariadne colored error output (replaces manual eprintln), --quiet flag (compiler + runtime), --emit-ir flag, 7 error suggestions (.with_suggestion on common diagnostics) |
| Integration test suite | Done | 15 end-to-end compile-to-run tests in tests/integration.rs (arithmetic, strings, if/else, match, for/while loops, functions, pipe, structs, try/catch, Result, hashmap, stdlib, recursion) |
| CLI polish | Done | Long help text (--help), --emit-ir (print IR JSON to stdout), --quiet (suppress warnings/emits) |
| README & docs | Done | Installation, getting started, stdlib table, ledger in features, license MIT |
| Example programs verified | Done | All 3 examples compile and run end-to-end |

## Phase 6: Project Manifest & Scaffolding

This phase introduces `Concerto.toml` as the mandatory project manifest and adds the `concerto init` scaffolding command. The `connect` keyword is removed from the language — connection config moves entirely to TOML. See [spec/22-project-manifest.md](spec/22-project-manifest.md) and [spec/23-project-scaffolding.md](spec/23-project-scaffolding.md).

### Step 1: Concerto.toml Loader (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| Add `toml` crate dependency | Done | Workspace-level dep for TOML parsing |
| Create `concerto-common/src/manifest.rs` | Done | `ConcertoManifest` struct: `[project]`, `[connections.*]`, `[mcp.*]` sections. `load_manifest(path)` and `find_manifest(source_dir)` (walk-up search). Validation of required fields per provider type |
| Manifest unit tests | Done | 10 tests: parse valid TOML, missing fields, unknown provider, walk-up discovery, IR config conversion |

### Step 2: Remove `connect` Keyword (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| Lexer: remove `connect` keyword | Done | Removed from keyword list (42→41 keywords). `connect` becomes a regular identifier |
| Parser: remove `ConnectDecl` parsing | Done | Removed `parse_connect_declaration()` and dispatch |
| AST: remove `ConnectDecl` variant | Done | Removed from `Declaration` enum and visitor |
| Semantic: remove connect name registration | Done | Resolver no longer registers connect blocks. Connection names come from manifest |
| Codegen: remove connect IR generation | Done | Removed `generate_connect()`. `IrConnection` still exists (populated from TOML) |
| Update existing tests | Done | Removed/updated tests using `connect` blocks. 227 compiler tests |

### Step 3: Wire Manifest into Compiler (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| `concertoc` reads Concerto.toml | Done | Find manifest from source file dir (walk-up), parse, validate. NotFound is OK |
| Semantic: register TOML connections | Done | `analyze_with_connections()` registers manifest connection names as `SymbolKind::Connection` |
| Codegen: embed TOML connections in IR | Done | `add_manifest_connections()` extends connections Vec from manifest |

### Step 4: Update Runtime for TOML-sourced Connections (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| ConnectionManager: use `provider` field | Done | Explicit `provider` field from TOML config. Fallback to name-based heuristics for legacy |
| Ollama support | Done | No API key needed, default localhost:11434/v1 |
| `resolve_api_key` TOML format | Done | Handles `api_key_env` field (TOML) + `api_key` (legacy). 5 new tests |

### Step 5: `concerto init` Command (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| Add `Init` subcommand to CLI | Done | `concerto init <name> [-p openai|anthropic|ollama]` |
| Generate Concerto.toml | Done | Provider-specific templates with `[project]` + `[connections.*]` |
| Generate src/main.conc | Done | Hello-world agent program matching the chosen provider |
| Generate .gitignore | Done | `*.conc-ir` and `.env` |
| Overwrite protection | Done | Error if `Concerto.toml` already exists |
| Output formatting | Done | Print created files + "Get started" instructions (provider-specific) |

### Step 6: Restructure Examples & Update Docs (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| Restructure examples into project dirs | Done | `examples/hello_agent/`, `examples/tool_usage/`, `examples/multi_agent_pipeline/` — each with `Concerto.toml` + `src/main.conc` |
| Remove `connect` blocks from examples | Done | All 3 `.conc` sources updated, old flat files + `.conc-ir` artifacts removed |
| Verify examples compile & run | Done | All 3 examples compile and run end-to-end with mock providers |
| Update CLAUDE.md | Done | Updated keyword list (41), directory structure, design decisions, manifest in architecture |
| Update STATUS.md | Done | This update |

## Phase 7: Agent Memory, Dynamic Tool Binding, and Hosts

This phase adds three interrelated features that extend agent execution capabilities. All three share a builder pattern (`with_memory`, `with_tools`, `with_context`) that chains before `.execute()`. See [spec/24-agent-memory.md](spec/24-agent-memory.md), [spec/25-dynamic-tool-binding.md](spec/25-dynamic-tool-binding.md), and [spec/26-hosts.md](spec/26-hosts.md).

### Phase 7a: Agent Memory (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| spec/24-agent-memory.md | Done | Memory keyword, with_memory() builder, auto-append, sliding window |
| Compiler: `memory` keyword + parser | Done | Lexer keyword, MemoryDecl AST node, parser, semantic (SymbolKind::Memory, Type::MemoryRef) |
| Compiler: IR generation | Done | IrMemory struct, `memories` IR section, generate_memory() |
| Runtime: MemoryRef value + store | Done | Value::MemoryRef, MemoryStore (Vec<ChatMessage> per memory), memory.rs |
| Runtime: AgentBuilder value | Done | Value::AgentBuilder (shared transient value for all three features) |
| Runtime: with_memory() + execute() | Done | Builder dispatch, modified build_chat_request with memory injection |
| Runtime: Memory direct API | Done | append, messages, last, clear, len methods on MemoryRef |
| Tests + integration | Done | Unit tests for memory store, integration test for agent with memory |

### Phase 7b: Dynamic Tool Binding (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| spec/25-dynamic-tool-binding.md | Done | with_tools()/without_tools() builder, tool schema generation |
| Compiler: tool schema generation | Done | Extract @describe/@param decorators → ToolSchemaEntry in codegen |
| IR: ToolSchemaEntry on IrTool | Done | method_name, description, parameters (JSON Schema) |
| Runtime: with_tools()/without_tools() | Done | Builder methods, dynamic tool resolution, merged schemas in ChatRequest |
| Tests + integration | Done | Unit + integration tests |

### Phase 7c: Hosts (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| spec/26-hosts.md | Done | Host keyword, stdio transport, stateful subprocess, TOML config |
| TOML manifest: HostConfig | Done | [hosts.*] section in Concerto.toml |
| Compiler: `host` keyword + parser | Done | Lexer keyword, HostDecl AST node, parser, semantic (SymbolKind::Host, Type::HostRef) |
| Compiler: IR generation | Done | IrHost struct, `hosts` IR section, generate_host() |
| Runtime: host.rs + HostRegistry | Done | HostClient (subprocess mgmt, stdio I/O), HostRegistry |
| Runtime: HostRef value + VM dispatch | Done | Value::HostRef, with_context() builder, execute dispatch |
| Tests + integration | Done | Unit + integration tests |

### Phase 7d: Documentation Update (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| Update CLAUDE.md | Done | New keywords, types, value variants, design decisions |
| Update STATUS.md | Done | Phase 7 completion status |
| Update FEATURES.md | Done | New features documented |
| Update VS Code extension | Done | memory/host keywords in tmLanguage.json |
| Update memory files | Done | Project memory updated with Phase 7 details |

### Phase 7e: Host Streaming (COMPLETE)

Bidirectional host streaming via `listen` expression with typed handlers and NDJSON wire protocol. Enables agent-supervises-agent pattern: a Concerto agent answers the host's questions autonomously. See [spec/27-host-streaming.md](spec/27-host-streaming.md).

| Task | Status | Notes |
|------|--------|-------|
| spec/27-host-streaming.md | Done | Listen syntax, NDJSON wire protocol, handler semantics, schemas |
| IR types: IrListenHandler, IrListen | Done | Following IrPipelineStage pattern. ListenBegin opcode (60) |
| Compiler: `listen` keyword | Done | Lexer keyword, 43 keywords total |
| Compiler: AST (ListenHandler, ExprKind::Listen) | Done | 31 ExprKind variants |
| Compiler: parse_listen_expr | Done | Handler match-arm syntax with closure-style params |
| Compiler: semantic analysis | Done | Schema validation for typed handler params, scoped resolution |
| Compiler: codegen (generate_listen) | Done | Handler bodies as instruction blocks (not closures), ListenBegin emit |
| Runtime: IR loader listens | Done | listens HashMap on LoadedModule |
| Runtime: HostClient streaming | Done | Persistent BufReader, read_message(), write_response(), write_prompt_streaming() |
| Runtime: VM listen loop | Done | exec_listen_begin(), run_listen_loop() with handler dispatch via run_loop_until |
| Parser tests | Done | 4 tests: single/multiple handlers, typed/untyped params |
| Codegen tests | Done | 4 tests: IR listen generation, ListenBegin opcode, handler instructions |
| Host streaming tests | Done | 4 tests: NDJSON parsing, plain text fallback, response format, get_client_mut |
| Integration tests | Done | 3 tests: compile+load, VM execution, bidirectional handler |
| Example: host_streaming | Done | `examples/host_streaming/` with Concerto.toml + src/main.conc |
| Example: bidirectional_host_middleware | Done | `examples/bidirectional_host_middleware/` with local host middleware script for full bidirectional testing |
| Update CLAUDE.md | Done | Listen docs, keywords, design decision #27 |
| Update STATUS.md | Done | This section |

### Direct Run (COMPLETE)

`concerto run file.conc` compiles in-memory and executes directly — no intermediate `.conc-ir` file needed. `.conc-ir` files still supported for pre-compiled programs.

| Task | Status | Notes |
|------|--------|-------|
| Add concerto-compiler dep to CLI | Done | concerto crate now depends on both compiler and runtime |
| Implement compile_source() | Done | In-memory compile pipeline: lex → parse → semantic → codegen → LoadedModule |
| Extension detection (is_source_file) | Done | `.conc` → direct run, `.conc-ir` → legacy IR load |
| Manifest integration | Done | find_and_load_manifest() for connections/hosts in direct run path |
| Error formatting | Done | Simple text diagnostics with file:line:col format |
| Update help text | Done | CLI docs reflect `.conc` as primary input, init shows simpler workflow |
| Integration tests | Done | 3 tests: basic program, stdlib calls, agent with MockProvider |
| Update CLAUDE.md | Done | Design decision #28, CLI description updated |

### Future (deferred)

| Task | Status | Notes |
|------|--------|-------|
| Performance benchmarks | Not Started | VM execution performance |
| Documentation site | Not Started | Language guide and API docs |
| VS Code extension | Done | Syntax highlighting for .conc |
| Package registry design | Not Started | Future: sharing .conc packages |

---

## Decisions Log

| # | Decision | Date | Rationale |
|---|----------|------|-----------|
| 1 | Rust for implementation | 2026-02-07 | Performance, strong type system, aligns with syntax |
| 2 | JSON-based IR format | 2026-02-07 | Human-readable, debuggable; binary deferred |
| 3 | Stack-based VM | 2026-02-07 | Simpler to implement, well-understood model |
| 4 | Bidirectional emit system | 2026-02-07 | Human-in-the-loop patterns, host tool execution |
| 5 | First-class pipeline/stage | 2026-02-07 | Declarative multi-agent workflow orchestration |
| 6 | Comprehensive specs before code | 2026-02-07 | Validate design before committing to implementation |
| 7 | Static typing with inference | 2026-02-07 | Compile-time safety with reduced annotation burden |
| 8 | Dual error model (Result + try/catch) | 2026-02-07 | Functional and imperative styles both supported |
| 9 | `@describe`/`@param` decorators for tools | 2026-02-07 | Compiler-enforced tool descriptions replace fragile `///` doc comments; descriptions are language grammar, not comments |
| 10 | First-class `mcp` construct | 2026-02-07 | MCP tool interfaces declared in source with typed fn signatures; compile-time type checking + runtime schema validation |
| 11 | Generic method call syntax | 2026-02-08 | `method<Type>(args)` parsed with lookahead disambiguation from comparison operators; type args passed as schema on CALL_METHOD |
| 12 | Phase 3a mock-first | 2026-02-08 | No async runtime (tokio) in Phase 3a; AWAIT is no-op; agents return mock responses; enables full end-to-end testing without HTTP |
| 13 | First-class `ledger` keyword | 2026-02-08 | Fault-tolerant knowledge store for AI agents. Separate from `hashmap` (exact-key state). Identifier + Keys + Value model with word-containment similarity and case-insensitive key matching. First-class keyword for compiler integration |
| 14 | Synchronous LlmProvider trait | 2026-02-08 | Uses reqwest::blocking for Phase 3b simplicity. Async deferred to Phase 3c. tokio added now for CLI + future async needs |
| 15 | Trait-based provider with MockProvider fallback | 2026-02-08 | MockProvider auto-selected when no API key. Existing tests unchanged. Real providers require env API keys |
| 16 | Schema type normalization at runtime | 2026-02-08 | Compiler emits Concerto types (String, Int, Array<T>). Runtime normalizes to JSON Schema types (string, integer, array) before validation |
| 17 | `Concerto.toml` project manifest | 2026-02-08 | Connections defined in TOML (like Cargo.toml), not in source code. Compiler embeds connection config into IR at compile time. `connect` keyword removed |
| 18 | `concerto init` scaffolding | 2026-02-08 | Creates project structure (Concerto.toml + src/main.conc + .gitignore). Supports openai/anthropic/ollama. Generates working hello-world agent |
| 19 | Agent Memory with builder pattern | 2026-02-08 | `memory` keyword + `with_memory()` builder. Auto-append by default, opt-out via `auto: false`. Sliding window via `max: N`. Messages injected between system_prompt and user_prompt in ChatRequest |
| 20 | Dynamic tool binding | 2026-02-08 | `with_tools()` ADDS to agent's static tools, `without_tools()` strips all. Compile-time tool schema generation from `@describe`/`@param` decorators. Tool call execution loop deferred |
| 21 | Hosts as external agent connectors | 2026-02-08 | `host` keyword for external agent systems (Claude Code, Cursor). Stdio transport, stateful subprocess. TOML `[hosts.*]` config. Same builder interface as agents |
| 22 | Shared AgentBuilder value type | 2026-02-08 | Transient `Value::AgentBuilder` accumulates config (memory, tools, context) via method chaining. Shared across Agent/Host `.with_*().execute()` pattern |
| 23 | Bidirectional host streaming | 2026-02-09 | `listen` expression with typed handlers and NDJSON wire protocol. Handler bodies compiled as instruction blocks (pipeline stage pattern). `result`/`error` messages are terminal. Unhandled messages emitted to `listen:unhandled` |
| 24 | Direct run | 2026-02-09 | `concerto run file.conc` compiles in-memory and executes directly. Extension detection chooses path. `.conc-ir` still supported. No intermediate file I/O |

## Open Questions

- Package manager / registry: needed for v1?
- WASM compilation target: priority level?
- ~~MCP protocol integration depth~~ RESOLVED: First-class `mcp` construct with typed tool interfaces (Decision #10)
- Debugger: step-through debugging in v1 or deferred?
- LSP (Language Server Protocol): priority for IDE support?
