# STATUS.md - Concerto Language Project Ledger

> **Last updated**: 2026-02-08

## Current Focus

**Phase 1: Foundation** - COMPLETE. All specs, docs, and examples written.
**Phase 2: Compiler Implementation** - COMPLETE. All 12 steps done + generic method call fix. All 3 example programs compile end-to-end to IR. 225 tests, clippy clean.
**Phase 3a: Runtime Core** - COMPLETE. Stack-based VM with all opcodes, mock agent system, database stubs, emit channels. All 3 examples compile AND run end-to-end. 261 tests total, clippy clean.
**Phase 3b: Agent & Tool System** - COMPLETE. Try/catch exception handling, real LLM providers (OpenAI + Anthropic), schema validation with retry, tool method dispatch. 299 tests total, clippy clean.
**Phase 3c: Pipeline & Polish** - COMPLETE. Decorator runtime (@retry/@timeout/@log), full pipeline lifecycle events, async foundations (Thunk), MCP client (stdio transport), run_loop_until for nested execution. 320 tests total, clippy clean, all 3 examples run end-to-end.
**Phase 3d: Ledger System** - COMPLETE. First-class `ledger` keyword across full compiler+runtime stack. LedgerStore with word-containment identifier queries, case-insensitive key queries (single/OR/AND), mutations, scoping. 338 tests total, clippy clean, all 3 examples run end-to-end.
**Phase 4: Standard Library** - COMPLETE. All 12 std:: modules implemented (math, string, env, time, json, fmt, log, fs, collections, http, crypto, prompt). 102 new stdlib tests. VM dispatch for std:: function calls and collection method calls. 440 tests total (225 compiler + 215 runtime), clippy clean, all 3 examples run end-to-end.
**Phase 5: Integration and Polish** - COMPLETE. Runtime robustness (replaced 5 unwraps, implemented DbQuery, error on unknown fn, mock provider warning). Compiler error quality (ariadne colored output, --quiet/--emit-ir flags, 7 error suggestions). 15 end-to-end integration tests. CLI polish (help text, long_about). README/docs updates. 458 tests total (228 compiler + 215 runtime + 15 integration), clippy clean.
**Phase 6: Project Manifest & Scaffolding** - NEXT. Introduce `Concerto.toml` as mandatory project manifest. Remove `connect` keyword from language. Add `concerto init` scaffolding command. See [spec/22](spec/22-project-manifest.md) and [spec/23](spec/23-project-scaffolding.md).

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
| spec/09-memory-and-databases.md | Done | db, Database<K,V>, scoping, queries |
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
| Parser - all declarations | Done | 16 declaration types (fn, connect, agent, tool, schema, pipeline, struct, enum, trait, impl, use, mod, const, type, db, mcp), decorators (@name(args)), self/mut self params, 107 tests total |
| Parser - all statements & expressions | Done | for/while/loop, match (all pattern types), try/catch/throw, closures, pipe (|>), ? propagation, ?? nil coalesce, range (.., ..=), cast (as), path (::), .await, tuples, struct literals, string interpolation, break/continue, 143 tests total |
| Semantic analysis | Done | Name resolution (forward refs, scoping), type checking (operators, conditions, inference), control flow validation (break/continue in loops, return in fns, ?/throw in Result fns, .await in async), mutability checking, declaration validation (agent provider, tool description, @describe), unused variable warnings, built-ins (emit, print, env, Some/None/Ok/Err, ToolError, Database, std), 216 tests total |
| IR generator - full coverage | Done | All 16 declaration types lowered (agent, tool, schema, connect, pipeline, struct, enum, impl, trait, const, db, mcp). All statement types (break w/ value, continue, throw). All 29 expression types (while/for/loop with break/continue, match with pattern check+bind, try/catch/throw, closures, pipe rewrite, ? propagation, ?? nil coalesce, range, cast, path, .await, tuples, struct literals, string interpolation). Loop result variables, pattern matching (literal/wildcard/identifier/or/range/binding/tuple/struct/enum/array), 216 tests total |
| Integration & polish | Done | All 3 examples compile end-to-end. Parser fixes: prefix `await expr`, `return` as expression (match arms), union types (`"a" \| "b"`). Semantic fixes: tool methods implicitly async, pipeline stages implicitly async with Result return, `self` not warned unused in tools. 222 tests total, clippy clean |
| Generic method calls | Done | Parser: `method<Type>(args)` parsed as MethodCall with type_args (lookahead disambiguates from comparison). AST: type_args field on MethodCall. Codegen: schema field on CALL_METHOD. 225 compiler tests total |

## Phase 3: Runtime Implementation

### Phase 3a: Core VM (COMPLETE)

| Task | Status | Notes |
|------|--------|-------|
| Value system | Done | 15 Value variants (Int, Float, String, Bool, Nil, Array, Map, Struct, Result, Option, Function, AgentRef, SchemaRef, DatabaseRef, PipelineRef). Arithmetic with type promotion, string coercion, comparisons, truthiness, field/index access. 16 tests |
| IR loader/decoder | Done | LoadedModule from JSON IR. Constants conversion, function/agent/tool/schema/connection/database/pipeline tables. Qualified tool method registration. 2 tests |
| VM execution loop | Done | Stack-based dispatch of all 59 opcodes. CallFrame with locals HashMap. CALL convention (args then callee), CALL_METHOD (object then args). LOAD_LOCAL falls back to globals and function names. Max call depth 1000. 10 tests |
| Agent mock system | Done | Mock execute() returns Response struct with text/tokens/model. Mock execute_with_schema() populates fields from JSON Schema properties. Schema name passed via CALL_METHOD instruction |
| Database stubs | Done | In-memory KV (HashMap<String, HashMap<String, Value>>). set/get/has/delete operations via CALL_METHOD and DB_* opcodes |
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
| Runtime robustness | Done | Replaced 5 unwraps in vm.rs with proper error returns, implemented DbQuery with closure-based filtering, error on unknown functions (was Nil), mock provider fallback warning, removed dead code (stack_base), added call_stack_depth() |
| Compiler error quality | Done | ariadne colored error output (replaces manual eprintln), --quiet flag (compiler + runtime), --emit-ir flag, 7 error suggestions (.with_suggestion on common diagnostics) |
| Integration test suite | Done | 15 end-to-end compile-to-run tests in tests/integration.rs (arithmetic, strings, if/else, match, for/while loops, functions, pipe, structs, try/catch, Result, database, stdlib, recursion) |
| CLI polish | Done | Long help text (--help), --emit-ir (print IR JSON to stdout), --quiet (suppress warnings/emits) |
| README & docs | Done | Installation, getting started, stdlib table, ledger in features, license MIT |
| Example programs verified | Done | All 3 examples compile and run end-to-end |

## Phase 6: Project Manifest & Scaffolding

This phase introduces `Concerto.toml` as the mandatory project manifest and adds the `concerto init` scaffolding command. The `connect` keyword is removed from the language — connection config moves entirely to TOML. See [spec/22-project-manifest.md](spec/22-project-manifest.md) and [spec/23-project-scaffolding.md](spec/23-project-scaffolding.md).

### Step 1: Concerto.toml Loader

| Task | Status | Notes |
|------|--------|-------|
| Add `toml` crate dependency | Not Started | Workspace-level dep for TOML parsing |
| Create `concerto-common/src/manifest.rs` | Not Started | `ConcertoManifest` struct: `[project]`, `[connections.*]`, `[mcp.*]` sections. `load_manifest(path)` and `find_manifest(source_dir)` (walk-up search). Validation of required fields per provider type |
| Manifest unit tests | Not Started | Parse valid TOML, missing fields, unknown provider, walk-up discovery |

### Step 2: Remove `connect` Keyword

| Task | Status | Notes |
|------|--------|-------|
| Lexer: remove `connect` keyword | Not Started | Remove from keyword list (42→41 keywords). `connect` becomes a regular identifier |
| Parser: remove `ConnectDecl` parsing | Not Started | Remove `parse_connect_declaration()` and related AST nodes |
| AST: remove `ConnectDecl` variant | Not Started | Remove from `Declaration` enum, remove `ConnectField` types |
| Semantic: remove connect name registration | Not Started | Resolver no longer registers connect block names in global scope |
| Codegen: remove connect IR generation | Not Started | Remove `generate_connect()`, `IrConnection` still exists (populated from TOML now) |
| Update existing tests | Not Started | Remove/update tests that use `connect` blocks. Compiler tests that parse connect syntax need updating |

### Step 3: Wire Manifest into Compiler

| Task | Status | Notes |
|------|--------|-------|
| `concertoc` reads Concerto.toml | Not Started | Find manifest from source file dir, parse, validate. Error if not found |
| Semantic: validate `provider:` against TOML | Not Started | Resolver loads connection names from manifest, checks agent `provider:` fields |
| Codegen: embed TOML connections in IR | Not Started | `connections` IR section populated from manifest instead of `connect` blocks |
| MCP: merge TOML config with source interfaces | Not Started | `mcp_connections` IR section gets transport/command/url from TOML, typed tools from source |
| Compiler error messages | Not Started | Unknown provider, missing manifest, MCP name mismatch warnings |

### Step 4: Update Runtime for TOML-sourced Connections

| Task | Status | Notes |
|------|--------|-------|
| IR connection format: add `provider` field | Not Started | `IrConnection.config` now includes explicit `provider` type string from TOML |
| ConnectionManager: use `provider` field | Not Started | Provider factory uses explicit `provider` field instead of guessing from name |
| MCP registry: TOML config fields | Not Started | McpClient reads transport/command/url from IR (sourced from TOML) |
| Runtime integration tests | Not Started | End-to-end tests with TOML-based connections |

### Step 5: `concerto init` Command

| Task | Status | Notes |
|------|--------|-------|
| Add `Init` subcommand to CLI | Not Started | `concerto init <name> [--provider openai\|anthropic\|ollama]` |
| Generate Concerto.toml | Not Started | Provider-specific template with `[project]` + `[connections.*]` |
| Generate src/main.conc | Not Started | Hello-world agent program matching the chosen provider |
| Generate .gitignore | Not Started | `*.conc-ir` and `.env` |
| Overwrite protection | Not Started | Error if `Concerto.toml` already exists |
| Output formatting | Not Started | Print created files + "Get started" instructions |
| Init tests | Not Started | Test each provider template, overwrite check, name inference from `.` |

### Step 6: Restructure Examples & Update Docs

| Task | Status | Notes |
|------|--------|-------|
| Restructure examples into project dirs | Not Started | Flat `examples/*.conc` → proper project layout: `examples/hello_agent/Concerto.toml` + `examples/hello_agent/src/main.conc` (same for multi_agent_pipeline, tool_usage). Remove old flat files + `.conc-ir` artifacts |
| Create Concerto.toml per example | Not Started | hello_agent: openai only. multi_agent_pipeline: openai + anthropic. tool_usage: openai + `[mcp.GitHubServer]` with stdio transport |
| Remove `connect` blocks from examples | Not Started | Delete `connect openai { ... }` / `connect anthropic { ... }` from all 3 `.conc` sources |
| Remove MCP connection fields from tool_usage | Not Started | `mcp GitHubServer` keeps typed fn signatures, loses `transport`/`command` fields (now in TOML) |
| Update integration tests | Not Started | Integration tests need Concerto.toml or manifest injection for programs that use agents/connections |
| Update spec/11 (LLM connections) | Not Started | Mark as superseded by spec/22, add cross-reference |
| Update spec/07 (Agents) | Not Started | Remove `connect` from agent examples, reference Concerto.toml |
| Update spec/20 (Interop/MCP) | Not Started | MCP connection config references Concerto.toml |
| Update CLAUDE.md | Not Started | New keyword count (41), manifest in architecture, new spec files, updated examples dir structure |
| Update README.md | Not Started | Getting started with `concerto init` + Concerto.toml workflow, add spec/22 and spec/23 to docs list |

### Future (deferred)

| Task | Status | Notes |
|------|--------|-------|
| Performance benchmarks | Not Started | VM execution performance |
| Documentation site | Not Started | Language guide and API docs |
| VS Code extension | Not Started | Syntax highlighting for .conc |
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
| 13 | First-class `ledger` keyword | 2026-02-08 | Fault-tolerant knowledge store for AI agents. Separate from `db` (exact-key state). Identifier + Keys + Value model with word-containment similarity and case-insensitive key matching. First-class keyword for compiler integration |
| 14 | Synchronous LlmProvider trait | 2026-02-08 | Uses reqwest::blocking for Phase 3b simplicity. Async deferred to Phase 3c. tokio added now for CLI + future async needs |
| 15 | Trait-based provider with MockProvider fallback | 2026-02-08 | MockProvider auto-selected when no API key. Existing tests unchanged. Real providers require env API keys |
| 16 | Schema type normalization at runtime | 2026-02-08 | Compiler emits Concerto types (String, Int, Array<T>). Runtime normalizes to JSON Schema types (string, integer, array) before validation |

## Open Questions

- Package manager / registry: needed for v1?
- WASM compilation target: priority level?
- ~~MCP protocol integration depth~~ RESOLVED: First-class `mcp` construct with typed tool interfaces (Decision #10)
- Debugger: step-through debugging in v1 or deferred?
- LSP (Language Server Protocol): priority for IDE support?
