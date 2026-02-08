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
       +---> Memory Manager (in-memory databases, scoping)
       +---> Emit Channel (bidirectional output system)
       +---> Schema Validator (structured output validation)
       +---> Async Executor (concurrent agent calls)
```

### Compiler Pipeline

```
Source (.conc) -> Lexer -> Tokens -> Parser -> AST -> Semantic Analysis -> Typed AST -> IR Generator -> IR (.conc-ir)
```

1. **Lexer**: Character scanning, tokenization, source position tracking
2. **Parser**: Recursive descent with Pratt parsing for expressions
3. **AST**: Abstract syntax tree with source spans -- 16 declaration types, decorators, config/typed fields, self params, 30 ExprKind variants (incl. Return expr), 11 PatternKind variants, 6 Stmt variants, union/string-literal type annotations
4. **Semantic Analysis**: Two-pass resolver (collect decls, then walk bodies) + declaration validator. Name resolution with forward references, basic type checking (operators, conditions), control flow validation (break/continue/return/?/throw/.await), mutability checking, unused variable warnings, built-in symbols (emit, print, env, Some/None/Ok/Err, ToolError, Database, std). Tool methods implicitly async, pipeline stages implicitly async with Result return type, `self` not warned unused in tool methods
5. **IR Generation**: Full coverage lowering of all 16 declaration types, all 6 statement types, all 30 expression types. Includes loop control flow (break w/ value, continue via patches), match pattern compilation (check + bind phases), try/catch/throw, closures (compiled as separate functions), pipe rewrite, ? propagation, ?? nil coalesce, string interpolation concat, struct/enum/pipeline/agent/tool/schema/connect/db/mcp lowering to IR sections, return expression in match arms, schema union types to JSON Schema enum

### Runtime Pipeline

```
IR (.conc-ir) -> IR Loader -> Instruction Dispatcher -> Execution (with LLM, tools, memory, emits)
```

- Stack-based virtual machine
- JSON-based IR format (human-readable, debuggable)
- Async execution for LLM calls

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
    09-memory-and-databases.md  # db keyword, Database<K,V>, scoping
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
  examples/              # Example .conc programs
    hello_agent.conc     # Minimal agent example
    multi_agent_pipeline.conc  # Multi-stage pipeline
    tool_usage.conc      # Tool definition and usage
  Cargo.toml             # Workspace root
  crates/
    concerto-common/     # Shared types (Span, Diagnostic, IR types, Opcodes)
      src/lib.rs, span.rs, errors.rs, ir.rs, ir_opcodes.rs
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
    concerto-runtime/    # Runtime library (Phase 3 - stub)
      src/lib.rs
    concerto/            # Runtime CLI binary (Phase 3 - stub)
      src/main.rs
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
| `DatabaseRef` | Reference to in-memory database |

### User-Defined Types
- `struct` - Product types with named fields
- `enum` - Sum types / tagged unions
- `trait` - Interfaces / capability contracts

## Keyword Reference

```
let    mut    fn     agent   tool    pub     use     mod
if     else   match  for     while   loop    break   continue
return try    catch  throw   emit    await   async   pipeline
stage  schema db     connect self    impl    trait   enum
struct as     in     with    true    false   nil     const
type   mcp
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
