# Idea: Rename `agent` → `model` and `host` → `agent` [IMPLEMENTED]

## The Problem

Concerto's identity is **orchestrating AI agents and building harnesses**. But our naming is backwards:

| Current keyword | What it actually is | Industry term |
|---|---|---|
| `agent` | A configured LLM endpoint (OpenAI, Anthropic, Ollama) with a system prompt | **Model** |
| `host` | An external autonomous system with its own tools, memory, decision-making | **Agent** |

When someone writes `agent Greeter { ... }`, they're not defining an agent — they're configuring an LLM model endpoint. The "agent" has no autonomy; it just receives a prompt and returns a completion.

Meanwhile, the things connected via `host` (Claude Code, custom systems) ARE actual agents — they have tools, memory, can take multi-step actions autonomously. Yet we call them "hosts", which sounds like infrastructure.

This creates confusion:
- Users expect `agent` to mean something autonomous — it doesn't
- The `host` keyword obscures that these ARE the agents being orchestrated
- The `tool` construct exists to give raw LLM endpoints capabilities — but hosted agents already have tools built in

## The Proposal

### Rename 1: `agent` → `model`

**Before:**
```concerto
agent Greeter {
    system_prompt: "You are a friendly greeter."
    model: "gpt-4"
    connection: openai
}

let response = Greeter.execute("Hello!");
```

**After:**
```concerto
model Greeter {
    system_prompt: "You are a friendly greeter."
    model: "gpt-4"
    connection: openai
}

let response = Greeter.execute("Hello!");
```

This is honest about what it is: a **model definition** — an LLM endpoint with configuration. You're not creating an agent; you're configuring how to talk to a model.

Note: the `model:` field inside the block has a naming collision with the keyword. Options:
- Rename the field to `name:` — `model Greeter { name: "gpt-4", ... }`
- Keep as-is — Rust handles this fine (keyword vs field context), and it reads naturally: "this model uses model gpt-4"
- Rename the field to `base:` — `model Greeter { base: "gpt-4", ... }` (as in "base model")

### Rename 2: `host` → `agent`

**Before:**
```concerto
host ClaudeCode {
    connector: "claude_code"
}

let result = listen ClaudeCode.execute("Refactor this code") {
    "progress" => |update| { emit("status", update); }
    "result" => |final| { final }
};
```

**After:**
```concerto
agent ClaudeCode {
    connector: "claude_code"
}

let result = listen ClaudeCode.execute("Refactor this code") {
    "progress" => |update| { emit("status", update); }
    "result" => |final| { final }
};
```

Now the keyword matches reality: Claude Code IS an agent. It has tools (file system, terminal, web search), memory, and autonomous decision-making. Concerto orchestrates it.

### Concerto.toml changes

```toml
# Before
[hosts.claude_code]
command = "claude"
args = ["--json"]

# After
[agents.claude_code]
command = "claude"
args = ["--json"]
```

`[connections.*]` stays unchanged — connections define API endpoints, which models reference.

## Alternative Names Considered

### For current `agent` (raw LLM connection):

| Name | Example | Pros | Cons |
|---|---|---|---|
| **`model`** | `model Greeter { ... }` | Industry standard, clear, short | Field collision with `model: "gpt-4"` |
| **`llm`** | `llm Greeter { ... }` | Explicit, very short, reads well | Acronym as keyword feels unusual |
| `endpoint` | `endpoint Greeter { ... }` | Technical accuracy | Too infrastructure-y, verbose |
| `responder` | `responder Greeter { ... }` | Describes behavior | Not industry standard |
| `completion` | `completion Greeter { ... }` | Maps to API concept | Too narrow (not all uses are completions) |

**Recommendation: `model`** — universally understood, concise, accurate.

### For current `host` (external agent system):

| Name | Example | Pros | Cons |
|---|---|---|---|
| **`agent`** | `agent ClaudeCode { ... }` | Industry standard, freed up by rename | Requires careful refactor sequencing |
| `service` | `service ClaudeCode { ... }` | SOA familiarity | Too generic, doesn't convey AI autonomy |
| `delegate` | `delegate ClaudeCode { ... }` | Implies task delegation pattern | Non-standard, might confuse |
| `worker` | `worker ClaudeCode { ... }` | Implies task execution | Too low-level, no autonomy connotation |
| `peer` | `peer ClaudeCode { ... }` | Implies equal collaboration | Unclear, academic |

**Recommendation: `agent`** — it's the correct term. These ARE agents. Concerto orchestrates agents.

## How This Clarifies the `tool` Question

With this rename, the role of `tool` becomes clear:

- **`model`** needs tools — raw LLMs have no capabilities without them
- **`agent`** has its own tools — Claude Code can read files, run commands, etc.

```concerto
// Tools are for models (raw LLM connections)
tool WebSearch {
    @describe("Search the web")
    fn search(self, query: String) -> String { ... }
}

model Researcher {
    system_prompt: "You research topics thoroughly."
    model: "gpt-4"
    connection: openai
    tools: [WebSearch]  // model needs tools to be useful
}

// Agents already have tools — no tool definition needed
agent ClaudeCode {
    connector: "claude_code"
    // Claude Code has its own tools: file system, terminal, etc.
}
```

This makes `tool` a model-level concern, not a language-level one. It explains why tools feel unnecessary for orchestration — because orchestrated agents don't need them.

## Semantic Alignment After Rename

| Concept | Keyword | Purpose |
|---|---|---|
| Raw LLM endpoint | `model` | Configure an LLM with system prompt, tools, schema |
| Autonomous system | `agent` | Connect to an external agent (Claude Code, custom) |
| Model capabilities | `tool` | Give raw models function-calling abilities |
| External tools | `mcp` | Connect MCP servers (tool providers for models) |
| Conversation state | `memory` | Sliding window memory for models or agents |
| Task orchestration | `pipeline` | Chain models and agents into workflows |
| Structured output | `schema` | Validate model responses against types |
| Data store | `hashmap`/`ledger` | State management for orchestration logic |
| Bidirectional comms | `listen` | Stream messages from agents |
| Event system | `emit` | Output events from orchestration logic |

Everything reads naturally now:
- "This **model** uses GPT-4 with these **tools**"
- "This **agent** is Claude Code — **listen** to it work"
- "This **pipeline** sends data through a **model**, then an **agent**"

## Scope of Changes

### Compiler (sequential, must be done in order)

1. **Lexer**: `TokenKind::Agent` → `TokenKind::Model`, `TokenKind::Host` → `TokenKind::Agent`
2. **AST**: `AgentDecl` → `ModelDecl`, `HostDecl` → `AgentDecl` (rename HostDecl first to avoid collision)
3. **Parser**: `parse_agent_decl` → `parse_model_decl`, `parse_host_decl` → `parse_agent_decl`
4. **Semantic**: `SymbolKind::Agent` → `SymbolKind::Model`, `SymbolKind::Host` → `SymbolKind::Agent`, `Type::AgentRef` → `Type::ModelRef`, `Type::HostRef` → `Type::AgentRef`
5. **Codegen**: `generate_agent` → `generate_model`, `generate_host` → `generate_agent`

### IR Types

6. **IrAgent** → `IrModel`, **IrHost** → `IrAgent`, **IrAgentConfig** → `IrModelConfig`
7. **IrModule** fields: `agents` → `models`, `hosts` → `agents`

### Runtime

8. **Value**: `AgentRef` → `ModelRef`, `HostRef` → `AgentRef`, `AgentBuilder` → `ModelBuilder`
9. **BuilderSourceKind**: `Agent` → `Model`, `Host` → `Agent`
10. **VM dispatch**: All CALL_METHOD routing for ModelRef/AgentRef
11. **IR Loader**: `LoadedModule` field renames

### Everything else

12. **Specs**: 07-agents.md → 07-models.md, 26-hosts.md → 26-agents.md, plus 24, 27, 29
13. **Examples**: 16 projects with keyword updates
14. **Tests**: ~100 tests with name/fixture updates
15. **Concerto.toml**: `[hosts.*]` → `[agents.*]`
16. **CLAUDE.md, STATUS.md, README.md**: Documentation updates

### Collision Management Strategy

The rename creates a temporary collision: old `agent` and new `agent` (from `host`).

**Resolution**: Rename in two passes:
1. **Pass 1**: Rename `agent` → `model` everywhere (frees the `agent` keyword)
2. **Pass 2**: Rename `host` → `agent` everywhere (no collision now)

Each pass is a single commit, keeping the codebase compilable between steps.

## What Stays the Same

- `tool` keyword — unchanged, but now clearly a model-level construct
- `mcp` keyword — unchanged, provides tools to models
- `memory` keyword — unchanged, works with both models and agents
- `listen` keyword — unchanged, but now reads better: "listen to the agent"
- `pipeline`/`stage` — unchanged
- `schema` — unchanged
- `emit` — unchanged
- `hashmap`/`ledger` — unchanged
- All opcodes — same opcodes, renamed dispatch targets
- Wire protocol (NDJSON) — unchanged
- Builder pattern — same pattern, renamed types

## Open Questions

1. **`model` field collision**: `model Greeter { model: "gpt-4" }` — keep, rename to `name:`, or `base:`?
2. **`mock` keyword in tests**: `mock Agent { ... }` → `mock Model { ... }`? This actually reads better — you're mocking a model's response
3. **AgentBuilder naming**: Keep as generic `Builder`, or split into `ModelBuilder`/`AgentBuilder`?
4. **Backward compatibility**: Should we version the IR format, or is this a clean break? (pre-1.0, clean break seems fine)
5. **`@test` decorator**: Tests that mock agents — does the syntax change? `mock Model { ... }` vs `mock Agent { ... }`

## Recommendation

Do this rename. It aligns Concerto's keywords with industry terminology and its own purpose:

> Concerto is a language for orchestrating **agents**. You configure **models** (LLM endpoints) and connect **agents** (autonomous systems), then compose them into **pipelines**.

The refactor is large (~50 files, ~100 tests) but mechanical — mostly search-and-replace with careful collision management. Pre-1.0 is the right time to get naming right.
