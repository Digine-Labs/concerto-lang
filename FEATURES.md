# Concerto Features

Concerto is a purpose-built programming language for orchestrating AI systems. Instead of treating agent workflows as framework glue code, Concerto makes agents, schemas, tools, pipelines, memory, and runtime integration first-class language constructs.

## Why Concerto Exists

Most AI stacks today are built as libraries inside general-purpose languages. That works for quick prototypes, but production harnesses usually accumulate fragmented prompt strings, ad-hoc validation, and orchestration logic spread across app code, framework code, and infrastructure config.

Concerto addresses this by providing:

- A dedicated language for agent orchestration.
- Static analysis + explicit runtime semantics.
- A compile target (`.conc-ir`) with a dedicated VM runtime.

## Core Differentiators

### 1. AI-Orchestration as Native Syntax

Concerto includes first-class constructs for:

- `agent`: model/provider/system-prompt/tool-aware execution units.
- `schema`: typed structured outputs for LLM responses.
- `tool` and `mcp`: typed tool interfaces and external capability integration.
- `pipeline` / `stage`: declarative multi-step orchestration.
- `hashmap`: typed state store for workflow state.
- `ledger`: fault-tolerant knowledge store for imprecise AI retrieval queries.
- `emit`: runtime integration boundary (output + bidirectional host calls).

This reduces boilerplate that is otherwise hand-built in framework code.

### 2. Reliability by Design

Concerto emphasizes correctness and failure visibility:

- Static type system with inference, generics, unions, and pattern matching.
- Structured output validation through `execute_with_schema<T>()`.
- Retry/error handling patterns built into the language model (`Result`, `?`, `try/catch/throw`).
- Decorators such as `@retry`, `@timeout`, `@log` on agents/stages.
- Pipeline lifecycle events (`pipeline:start`, `stage_start`, `stage_complete`, `error`, `complete`).

### 3. Configuration/Code Separation

`Concerto.toml` is mandatory and carries project/runtime configuration:

- Connection/provider definitions.
- API key environment-variable references.
- MCP transport configuration.
- Provider model aliasing and retry/rate-limit settings.

Language source remains focused on orchestration logic rather than deployment plumbing.

### 4. Compiler + IR + Runtime Architecture

Concerto compiles `.conc` source to JSON IR (`.conc-ir`) and executes it on a dedicated runtime VM.

Benefits:

- Inspectable intermediate representation.
- Clear compile-time vs runtime responsibility boundaries.
- Easier host-language embedding and integration.

### 5. Practical Integration Boundary

The emit system provides a clean host contract:

- One-way emits for telemetry and outputs.
- Bidirectional emits for approvals, external systems, and human-in-the-loop steps.
- Tool implementations can remain inside Concerto or delegate safely to host handlers.

## Code Examples

### Typed Agent Execution (`agent` + `schema`)

```rust
schema Classification {
    label: "legal" | "technical" | "financial" | "general",
    confidence: Float,
    reasoning: String,
}

agent Classifier {
    provider: openai,
    model: "gpt-4o-mini",
    temperature: 0.2,
    system_prompt: "Classify documents and return valid JSON.",
}

fn main() {
    let prompt = "Classify this report: ${report}";
    let result = Classifier.execute_with_schema<Classification>(prompt);

    match result {
        Ok(v) => emit("classification", v),
        Err(e) => emit("error", e.message),
    }
}
```

### Declarative Multi-Step Workflows (`pipeline` + `stage`)

```rust
pipeline DocumentFlow {
    stage extract(doc: String) -> String {
        let raw = Extractor.execute(doc)?;
        raw.text
    }

    @retry(max: 2, backoff: "linear")
    stage classify(text: String) -> Classification {
        Classifier.execute_with_schema<Classification>(text)?
    }

    stage summarize(c: Classification) -> String {
        let out = Summarizer.execute("Summarize ${c.label}: ${c.reasoning}")?;
        out.text
    }
}

fn main() {
    let result = DocumentFlow.run(input_document);
    match result {
        Ok(v) => emit("summary", v),
        Err(e) => emit("pipeline_error", e.message),
    }
}
```

### Fault-Tolerant Knowledge Retrieval (`ledger`)

```rust
ledger knowledge: Ledger = Ledger::new();

fn seed() {
    knowledge.insert(
        "Uniswap contract addresses on Ethereum mainnet.",
        ["Uniswap", "Contract Address", "Ethereum", "Dex"],
        "Pool: 0x123..., Router: 0x456...",
    );
}

fn main() {
    seed();
    let by_identifier = knowledge.query().from_identifier("contract addresses");
    let by_key = knowledge.query().from_key("Ethereum");

    emit("identifier_hits", by_identifier);
    emit("key_hits", by_key);
}
```

### Tool-to-Host Bridge (`tool` + bidirectional `emit`)

```rust
tool FileReader {
    description: "Read files from host filesystem via emit bridge",

    @describe("Read a file by path")
    @param("path", "Absolute or relative file path")
    pub fn read(self, path: String) -> Result<String, ToolError> {
        let result = await emit("tool:file:read", { "path": path });
        match result {
            Ok(content) => Ok(content),
            Err(e) => Err(ToolError::new("read failed: ${e}")),
        }
    }
}
```

### Project-Level Configuration (`Concerto.toml`)

```rust
[project]
name = "support-triage-harness"
version = "0.1.0"
entry = "src/main.conc"

[connections.openai]
provider = "openai"
api_key_env = "OPENAI_API_KEY"
default_model = "gpt-4o-mini"

[connections.openai.retry]
max_attempts = 3
backoff = "exponential"
```

### Host Runtime Integration (Rust)

```rust
use concerto_runtime::{run_file, Value};

fn main() {
    let mut vm = run_file("build/main.conc-ir").expect("load failed");
    vm.set_emit_handler(|channel: &str, payload: &Value| {
        println!("[emit:{}] {}", channel, payload.display_string());
    });
    vm.execute().expect("execution failed");
}
```

## Complete Feature Surface (By Capability)

| Capability Area | What Concerto Provides |
|---|---|
| Language Fundamentals | Rust-like syntax, strong typing, expressions/control flow, modules/imports, async-oriented constructs |
| AI Primitives | Agents, schema-constrained execution, typed response objects, tool calling |
| Tooling Model | Compiler-enforced tool descriptions/params, typed MCP interfaces, permission-aware tool attachment |
| Orchestration | Pipelines/stages, branching, retries/timeouts, stage-level error handling, pipeline events |
| Memory | Typed key-value `hashmap` plus semantic/tolerant `ledger` retrieval model |
| Error Model | `Result<T,E>`, propagation (`?`), `try/catch`, explicit runtime errors |
| Interop | Host API, emit bridge, tool FFI patterns, MCP runtime integration |
| Runtime | IR loader, stack-based VM, schema validator, provider abstraction, observability hooks |
| Project Workflow | Spec-driven design, `Concerto.toml` manifests, `concerto init` scaffolding |
| Standard Library | `std::json`, `std::http`, `std::fs`, `std::env`, `std::collections`, `std::time`, `std::math`, `std::string`, etc. |

## Concerto vs LangChain-Style Libraries

| Concern | Concerto | LangChain / Library-First Stacks |
|---|---|---|
| Primary abstraction | Language-level orchestration primitives | Framework APIs inside host language |
| Type safety | Compile-time language typing + semantic analysis | Typically host-language typing (often partial/dynamic at orchestration boundaries) |
| Structured output guarantees | Native schema construct + runtime schema validator | Available, but often optional and framework-pattern-dependent |
| Workflow semantics | First-class `pipeline/stage`, runtime lifecycle events | Chain/graph abstractions vary by framework and app architecture |
| Config separation | Mandatory project manifest (`Concerto.toml`) | Often mixed across env vars, code, framework config |
| Artifact portability | Compile to `.conc-ir` runnable by dedicated VM | Usually execute directly in app runtime |
| Host boundary | Explicit emit protocol and tool bridge | Embedded in app code; boundary is usually implicit |
| Knowledge model | Built-in Ledger for tolerant retrieval patterns | Usually assembled via external vector DB + app/framework glue |

LangChain-like stacks remain excellent for rapid experimentation and ecosystem breadth. Concerto is strongest when teams want a dedicated orchestration language with explicit semantics, tighter reliability controls, and cleaner production boundaries.

## Concerto vs Other Approaches

| Approach | Typical Strength | Typical Tradeoff vs Concerto |
|---|---|---|
| Handwritten Python/TypeScript orchestration | Full flexibility, familiar ecosystem | More custom glue, less standardized semantics, weaker portability of orchestration logic |
| Workflow engines (Temporal/Airflow/etc.) | Durable scheduling, retries, distributed ops | Not AI-language-native; agent/schema/tool semantics must be layered manually |
| Prompt-only scripts | Fastest to start | Poor maintainability, weak typing/validation, difficult scaling to multi-agent systems |

## Why Teams Choose Concerto

- They need reliable structured outputs and explicit failure handling.
- They need complex multi-agent harnesses without framework sprawl.
- They want orchestration logic to be readable as a language, not scattered patterns.
- They want a clear host/runtime boundary for integration and governance.
- They want reproducible project structure via manifest + scaffold conventions.

## Best Fit

Concerto is a strong fit for production-oriented AI harnesses where correctness, maintainability, and orchestration clarity matter more than ad-hoc scripting speed.
