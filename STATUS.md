# STATUS.md - Concerto Language Project Ledger

> **Last updated**: 2026-02-07

## Current Focus

**Phase 1: Foundation** - COMPLETE. All specs, docs, and examples written.
**Phase 2: Compiler Implementation** - COMPLETE. All 12 steps done. All 3 example programs compile end-to-end to IR. 222 tests, clippy clean.

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

## Phase 3: Runtime Implementation

| Task | Status | Notes |
|------|--------|-------|
| Value system | Not Started | Runtime value representation |
| IR loader/decoder | Not Started | JSON IR deserialization |
| VM execution loop | Not Started | Stack-based instruction dispatch |
| Agent registry | Not Started | Agent instances, lifecycle |
| Tool registry | Not Started | Tool registration, permissions |
| Connection manager | Not Started | LLM provider connections |
| Memory/database system | Not Started | In-memory KV with scoping |
| Emit channel system | Not Started | Bidirectional emit with host |
| Schema validator | Not Started | JSON Schema validation engine |
| Error handling frames | Not Started | try/catch frame stack |
| Async executor | Not Started | Concurrent agent calls |
| Runtime host API | Not Started | Rust library for embedding |
| Runtime CLI (`concerto`) | Not Started | CLI interface for execution |
| Runtime test suite | Not Started | Unit + integration tests |

## Phase 4: Standard Library

| Task | Status | Notes |
|------|--------|-------|
| std::json | Not Started | parse, stringify |
| std::http | Not Started | get, post, request |
| std::fs | Not Started | read_file, write_file, exists |
| std::env | Not Started | get, set, all |
| std::fmt | Not Started | format, pad, truncate |
| std::collections | Not Started | Set, Queue, Stack |
| std::time | Not Started | now, sleep, timestamp |
| std::math | Not Started | abs, min, max, round, random |
| std::string | Not Started | split, join, trim, replace |
| std::log | Not Started | info, warn, error, debug |
| std::prompt | Not Started | Prompt templates and utilities |
| std::crypto | Not Started | hash_sha256, uuid |

## Phase 5: Integration and Polish

| Task | Status | Notes |
|------|--------|-------|
| End-to-end pipeline test | Not Started | Compile + run full example |
| Example programs verified | Not Started | All examples compile and run |
| Error message quality | Not Started | Helpful, actionable errors |
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

## Open Questions

- Package manager / registry: needed for v1?
- WASM compilation target: priority level?
- ~~MCP protocol integration depth~~ RESOLVED: First-class `mcp` construct with typed tool interfaces (Decision #10)
- Debugger: step-through debugging in v1 or deferred?
- LSP (Language Server Protocol): priority for IDE support?
