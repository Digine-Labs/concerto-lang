# Concerto Language

**A programming language for orchestrating AI agents.**

Concerto provides a Rust-like syntax purpose-built for designing complex AI harnesses. Define agents, tools, memory databases, and multi-step pipelines as code -- then let the Concerto runtime handle LLM connections, structured output validation, and external integration.

## Why Concerto?

Prompt engineering is time-consuming. LangChain-style libraries are task-specific. Concerto gives you a **full programming language** to orchestrate agents -- with type safety, error handling, and composable pipelines.

```rust
connect openai {
    api_key: env("OPENAI_API_KEY"),
    default_model: "gpt-4o",
}

schema Classification {
    label: String,
    confidence: Float,
    reasoning: String,
}

agent Classifier {
    provider: openai,
    model: "gpt-4o",
    temperature: 0.2,
    system_prompt: "You are a document classifier.",
}

fn main() {
    let document = "The quarterly earnings report shows a 15% increase...";
    let prompt = "Classify this document: ${document}";

    let result = Classifier.execute_with_schema<Classification>(prompt);

    match result {
        Ok(classification) => emit("result", classification),
        Err(e) => emit("error", e.message),
    }
}
```

## Key Features

- **Agents as first-class constructs** -- Define LLM-powered agents with model, provider, temperature, system prompt, tools, and memory
- **Schema validation** -- Define expected output structures; runtime validates and auto-retries on mismatch
- **First-class pipelines** -- `pipeline`/`stage` keywords for declarative multi-agent workflows
- **Bidirectional emit system** -- Runtime outputs that enable programmatic integration with host applications
- **In-memory databases** -- Map-like storage for agent query design and harness state (ledger)
- **Tools** -- Define tools as classes that agents can invoke, with auto-generated parameter schemas
- **Strong type system** -- Static typing with inference, including AI-specific types (`Prompt`, `Response`, `Schema<T>`)
- **Dual error handling** -- `Result<T,E>` with `?` propagation and `try`/`catch` for flexibility
- **Async-native** -- All agent execution is async; parallel execution with `await (a, b, c)`
- **Pipe operator** -- `prompt |> agent.execute() |> parse_schema(Output) |> emit("result")`
- **LLM provider agnostic** -- `connect` blocks for OpenAI, Anthropic, Google, Ollama, or custom providers

## Architecture

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
 |  RUNTIME  |  Stack-based VM with agent, tool, memory, and emit systems
 +-----------+
       |
       v
 Emits (programmatic output to host application)
```

## Example: Multi-Agent Pipeline

```rust
pipeline DocumentProcessor {
    stage extract(doc: String) {
        let text = Extractor.execute(doc)?;
        text
    }

    stage classify(text: String) {
        let result = Classifier.execute_with_schema<Classification>(text)?;
        result
    }

    stage route(classification: Classification) {
        match classification.label {
            "legal" => LegalAgent.execute(classification)?,
            "technical" => TechAgent.execute(classification)?,
            _ => DefaultAgent.execute(classification)?,
        }
    }
}

fn main() {
    let result = DocumentProcessor.run(input_document);
    match result {
        Ok(output) => emit("processed", output),
        Err(e) => emit("error", e),
    }
}
```

## Installation

```bash
git clone https://github.com/Digine-Labs/concerto-lang.git
cd concerto-lang
cargo build --release
```

Binaries will be in `target/release/`:
- `concertoc` -- Compiler (`.conc` -> `.conc-ir`)
- `concerto` -- Runtime (executes `.conc-ir`)

## Getting Started

1. Write a Concerto program:

```concerto
// hello.conc
fn main() {
    emit("greeting", "Hello from Concerto!");
}
```

2. Compile it:

```bash
concertoc hello.conc
```

3. Run it:

```bash
concerto run hello.conc-ir
```

## Standard Library

| Module | Functions | Description |
|--------|-----------|-------------|
| `std::math` | abs, min, max, clamp, round, floor, ceil, pow, sqrt, random, random_int | Numeric operations |
| `std::string` | split, join, trim, replace, to_upper, to_lower, contains, substring, len, repeat, reverse, parse_int, parse_float | String manipulation |
| `std::env` | get, require, all, has | Environment variables |
| `std::time` | now, now_ms, sleep | Time and ISO 8601 |
| `std::json` | parse, stringify, stringify_pretty, is_valid | JSON serialization |
| `std::fmt` | format, pad_left, pad_right, truncate, indent | Text formatting |
| `std::log` | info, warn, error, debug | Structured logging |
| `std::fs` | read_file, write_file, append_file, exists, list_dir, remove_file, file_size | File system |
| `std::collections` | Set, Queue, Stack (+ 20 methods) | Data structures |
| `std::http` | get, post, put, delete, request | HTTP client |
| `std::crypto` | sha256, md5, uuid, random_bytes | Cryptography |
| `std::prompt` | template, from_file, count_tokens | Prompt utilities |

## Documentation

Language specifications are in the [spec/](spec/) directory:

- [Language Overview](spec/00-overview.md)
- [Lexical Structure](spec/01-lexical-structure.md)
- [Type System](spec/02-type-system.md)
- [Variables and Bindings](spec/03-variables-and-bindings.md)
- [Operators and Expressions](spec/04-operators-and-expressions.md)
- [Control Flow](spec/05-control-flow.md)
- [Functions](spec/06-functions.md)
- [Agents](spec/07-agents.md)
- [Tools](spec/08-tools.md)
- [Memory and Databases](spec/09-memory-and-databases.md)
- [Emit System](spec/10-emit-system.md)
- [LLM Connections](spec/11-llm-connections.md)
- [Schema Validation](spec/12-schema-validation.md)
- [Error Handling](spec/13-error-handling.md)
- [Modules and Imports](spec/14-modules-and-imports.md)
- [Concurrency and Pipelines](spec/15-concurrency-and-pipelines.md)
- [IR Specification](spec/16-ir-specification.md)
- [Runtime Engine](spec/17-runtime-engine.md)
- [Compiler Pipeline](spec/18-compiler-pipeline.md)
- [Standard Library](spec/19-standard-library.md)
- [Interop and FFI](spec/20-interop-and-ffi.md)
- [Ledger System](spec/21-ledger.md)

## Project Status

See [STATUS.md](STATUS.md) for detailed project tracking.

**Current:** Phase 5 (Integration and Polish) -- 458 tests, clippy clean.

## License

MIT
