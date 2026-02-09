# 00 - Language Overview

## Design Philosophy

Concerto is built on the principle of **"Orchestration as Code"** -- making AI orchestration a first-class programming concern rather than an afterthought bolted onto existing languages through libraries.

### Core Beliefs

1. **AI harnesses deserve their own language.** General-purpose languages and library-based approaches (LangChain, CrewAI) force developers to work around fundamental mismatches between language abstractions and AI orchestration patterns. Concerto removes this friction by making models, agents, tools, memory, and structured output native constructs.

2. **Type safety prevents costly runtime failures.** When an LLM returns unexpected output, the error shouldn't propagate silently. Concerto's static type system and schema validation catch structural mismatches at the earliest possible moment.

3. **Prompts are code, not strings.** Prompt engineering involves composition, templating, conditional logic, and validation. Concerto treats prompts as first-class values with dedicated syntax for interpolation, multi-line templates, and raw strings.

4. **Orchestration patterns should be declarative.** Multi-model pipelines, branching logic, retry strategies, and error recovery should be expressed as clear, readable code -- not buried in callback hell or framework configuration files.

5. **External integration through a clean boundary.** Concerto doesn't try to be a general-purpose runtime. It communicates with the outside world through the **emit system**, providing a clear, typed contract between the AI harness and the host application.

## Goals

- Provide a purpose-built language for designing complex AI orchestration harnesses
- Enable type-safe LLM interactions with compile-time validation where possible
- Support multi-model orchestration with first-class pipeline constructs
- Offer a clean integration boundary via the bidirectional emit system
- Keep syntax familiar to Rust/TypeScript developers (low learning curve for systems programmers)
- Produce human-readable IR for debugging and inspection

## Non-Goals

- Concerto is **not** a general-purpose systems programming language
- Concerto does **not** compete with Python/JS for ML model training or data science
- Concerto does **not** handle LLM API communication directly -- the runtime provides this via connection managers
- Concerto does **not** include a GUI framework, web server, or database ORM
- Concerto does **not** generate native machine code (it targets an interpreted IR)

## Target Use Cases

1. **Complex orchestration harnesses** -- Multi-step workflows where models collaborate, share memory, and produce structured outputs
2. **Structured output extraction** -- Parsing LLM responses into typed schemas with validation and retry logic
3. **Tool-augmented LLM workflows** -- Defining tools that models can invoke, with parameter schemas auto-generated from type signatures
4. **Human-in-the-loop pipelines** -- Using bidirectional emit to pause execution, request human input, and resume
5. **Multi-provider orchestration** -- Routing different tasks to different LLM providers based on cost, speed, or capability
6. **Model testing and simulation** -- Writing test harnesses that validate model behavior with mock LLM responses

## Inspirations

| Language/Tool | What Concerto Borrows |
|---------------|----------------------|
| **Rust** | Syntax style, `Result<T,E>`, pattern matching, `?` operator, trait system, module system |
| **Go** | Simplicity of concurrency model, clear error handling philosophy |
| **TypeScript** | Template literal strings, interface-like schemas, developer ergonomics |
| **Elixir** | Pipe operator `\|>`, pipeline-oriented design |
| **LangChain** | Agent/tool/chain concepts (but as language primitives, not library patterns) |

## Compilation Model

```
Source (.conc) --> Compiler --> IR (.conc-ir) --> Runtime --> Execution + Emits
```

1. **Source files** (`.conc`) are written by developers in Concerto syntax
2. The **compiler** (`concertoc`) transforms source into IR through lexing, parsing, type checking, and IR generation
3. The **IR** (`.conc-ir`) is a JSON-based intermediate representation containing instructions, type info, model/agent/tool definitions, and source maps
4. The **runtime** (`concerto`) loads IR, executes it on a stack-based VM, manages LLM connections, and emits outputs
5. **Emits** are the primary output mechanism -- the host application listens for emit events to integrate with the Concerto harness

## Hello Model

The simplest useful Concerto program:

```concerto
connect openai {
    api_key: env("OPENAI_API_KEY"),
    default_model: "gpt-4o-mini",
}

model Greeter {
    provider: openai,
    system_prompt: "You are a friendly assistant.",
}

fn main() {
    let response = Greeter.execute("Hello! What is your name?")?;
    emit("response", response.text);
}
```

This program:
1. Establishes an OpenAI connection
2. Defines a `Greeter` model with a system prompt
3. Sends a prompt to the LLM
4. Emits the response text for the host application to consume
