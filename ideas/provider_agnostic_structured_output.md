# Provider-Agnostic Structured Output

## The Problem

Concerto's `execute_with_schema<T>()` currently assumes all LLM providers support OpenAI's
`response_format: { type: "json_schema", json_schema: {...} }` API shape. This is false.

### Current Behavior by Provider

| Provider | What happens with `response_format` | Structured output quality |
|----------|-------------------------------------|--------------------------|
| OpenAI | Sent as `response_format.json_schema` with `strict: true` — native constraint | Guaranteed valid JSON |
| Anthropic | **Silently ignored** — `build_request_body()` never reads it | Pure luck + retry loop |
| Ollama | Depends on whether model is OpenAI-compatible | Varies by model |
| Mock | Generates mock JSON from schema | Always works (testing) |

When a Concerto user writes:

```concerto
agent writer uses claude_conn {
    model: "claude-sonnet-4-5-20250929",
}

let result = writer.execute_with_schema<Summary>("Summarize this");
```

The schema is **never sent to Claude's API**. The runtime just sends a plain prompt, hopes the
LLM returns valid JSON, validates after the fact with `jsonschema`, and retries up to 3 times
by pasting the schema into the prompt text. This is fragile and wastes tokens.

### Root Cause

The `ResponseFormat` struct is modeled after OpenAI's API:

```rust
pub struct ResponseFormat {
    pub format_type: String,        // "json_schema" or "json_object"
    pub json_schema: Option<serde_json::Value>,
}
```

Each provider's `build_request_body()` is responsible for translating this into its native API
format — but only OpenAI actually does it. The `LlmProvider` trait has no abstraction for
"structured output capability," so there's no way to know what a provider supports.

### Why This Matters

1. **Silent degradation** — Users get no warning that their schema constraint is being ignored
2. **Token waste** — Without API-level constraints, retries burn tokens on malformed responses
3. **Inconsistent behavior** — Same Concerto code works reliably on OpenAI, flaky on Anthropic
4. **Provider lock-in** — Defeats the purpose of Concerto abstracting over providers


---

## Key Points

1. **Not all providers have equivalent structured output APIs.** OpenAI has `response_format`
   with strict JSON schema. Anthropic has no direct equivalent — the closest is using tool_use
   as a structured output channel, or system prompt prefill. Ollama/local models vary wildly.

2. **The `LlmProvider` trait is too thin.** It's just `chat_completion(ChatRequest) -> ChatResponse`.
   There's no way to query provider capabilities (supports structured output? supports tools?
   supports streaming?). The runtime can't adapt its strategy per provider.

3. **Schema validation is the only reliable layer.** The post-response `SchemaValidator::validate()`
   plus retry loop is actually the universal fallback. But it's inefficient when a provider *does*
   support native constraints.

4. **The retry prompt is a reasonable fallback but not a strategy.** Pasting the schema into the
   prompt works ~80% of the time with capable models, but it's not deterministic. Some models
   produce markdown-wrapped JSON, extra commentary, or subtly wrong types.

5. **Provider-specific structured output mechanisms:**
   - **OpenAI**: `response_format: { type: "json_schema", json_schema: { schema, strict: true } }`
   - **Anthropic**: Use `tool_use` with a single tool whose `input_schema` is the target schema.
     The LLM is forced to call the tool with conforming input. Extract from tool_use block.
   - **Google Gemini**: `generationConfig.responseSchema` with `responseMimeType: "application/json"`
   - **Ollama**: `format: "json"` (basic) or `format: { schema }` (newer versions)
   - **Mistral**: `response_format: { type: "json_object" }` (no strict schema, just JSON mode)
   - **Groq**: Follows OpenAI format for compatible models
   - **Local/GGUF**: Some support grammar-based constraints (GBNF), most don't

6. **The problem extends beyond structured output.** Tool calling, streaming, vision, system
   prompts — every provider has slightly different API shapes. Concerto needs a general
   strategy for provider abstraction, not just a structured-output fix.


---

## Brainstorm: Language & Runtime Features for Universal Provider Support

### A. Provider Capability System

Add a capability model to the runtime so the VM knows what each provider supports and can
choose the best strategy automatically.

```
// Possible runtime-internal capability flags (not exposed to user)
capabilities:
  - structured_output: native | tool_trick | prompt_only
  - tool_calling: native | none
  - vision: native | none
  - streaming: native | none
```

The runtime picks the best structured output strategy per provider:
- `native` → use provider's API-level constraint (OpenAI response_format, Gemini responseSchema)
- `tool_trick` → wrap schema as a fake tool, extract from tool_use response (Anthropic)
- `prompt_only` → inject schema into system prompt, validate + retry (universal fallback)

**Language impact: None.** This is purely a runtime optimization. Concerto code stays the same,
the runtime just gets smarter about how it enforces the schema per provider. This should be the
minimum viable fix.

### B. `output` Block on Agents (Declarative Schema Binding)

Instead of specifying the schema at the call site (`execute_with_schema<T>`), bind it at the
agent definition level:

```concerto
agent summarizer uses my_llm {
    model: "gpt-4",
    system_prompt: "You summarize articles.",
    output: Summary,           // <-- always produce Summary-shaped output
}

// Now execute() always returns Result<Summary, Error>
let result = summarizer.execute("Summarize this");
```

**Advantages:**
- The compiler knows at definition time that this agent always produces structured output
- Can generate provider-specific request building at compile time
- `execute()` return type is statically known — no need for `execute_with_schema` variant
- Cleaner API: agent's contract is fully declared in one place

**The generic `execute_with_schema<T>` still exists** for dynamic/ad-hoc schema use. But for
agents with a known output contract, `output:` is the idiomatic way.

### C. `format` Strategy Field (User Control Over Enforcement)

Let users declare *how* they want structured output enforced:

```concerto
agent writer uses claude_conn {
    model: "claude-sonnet-4-5-20250929",
    output: Summary,
    format: "auto",            // let runtime pick best strategy (default)
    // format: "strict",       // fail if provider can't do native constraint
    // format: "tool",         // force tool_use trick
    // format: "prompt",       // only use prompt-based enforcement
}
```

**Why this matters:**
- `"auto"` (default) — runtime picks the best available strategy. Zero config for most users.
- `"strict"` — for production pipelines where you need guaranteed schema conformance. Fails
  fast if the provider can't enforce it natively rather than silently degrading.
- `"tool"` — explicit opt-in to the tool_use trick. Useful when you know the provider supports
  tools but not response_format.
- `"prompt"` — for local models or providers with no structured output support. Relies entirely
  on prompt engineering + post-validation.

### D. `@schema` Decorator for Prompt Injection

A decorator that automatically injects the schema definition into the system prompt or user
prompt, so even prompt-only providers get schema context:

```concerto
@schema(inject: "system")    // inject schema description into system prompt
agent writer uses local_llm {
    model: "llama3",
    output: Summary,
}
```

The compiler would generate a schema description block:

```
You must respond with valid JSON matching this exact schema:
{
  "title": "string",
  "points": ["string"],
  "score": "integer"
}
Do not include any text outside the JSON object.
```

This gets prepended/appended to the system prompt at compile time (embedded in IR) or at
runtime (before sending the request). Useful for models that don't support any form of
constrained generation but are capable enough to follow JSON instructions.

### E. Response Transform Pipeline (Post-Processing)

Allow users to define a transform chain that processes the raw LLM response before schema
validation:

```concerto
agent writer uses some_llm {
    output: Summary,
    transform: [
        strip_markdown,        // remove ```json ... ``` wrappers
        extract_json,          // find first { ... } in response
        // custom fn also possible
    ],
}
```

**Why:** Many models (especially local ones) wrap JSON in markdown code blocks, add preamble
text like "Here's the JSON:", or append commentary after the JSON. A transform pipeline would
handle these common patterns before validation, reducing false retry loops.

Built-in transforms could include:
- `strip_markdown` — remove code fences
- `extract_json` — regex-find the first `{...}` or `[...]` block
- `trim` — remove leading/trailing whitespace
- Custom function reference for user-defined transforms

### F. Provider Adapter Trait (Extensible Provider System)

Redesign `LlmProvider` to be more capability-aware:

```rust
pub trait LlmProvider: Send + Sync {
    fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse>;

    // New: capability queries
    fn supports_structured_output(&self) -> StructuredOutputSupport;
    fn supports_tool_calling(&self) -> bool;

    // New: provider-specific structured output preparation
    fn prepare_structured_request(
        &self,
        request: ChatRequest,
        schema: &serde_json::Value,
    ) -> ChatRequest {
        // Default: just attach response_format (OpenAI-style)
        // Anthropic overrides: convert to tool_use trick
        // Local model overrides: inject into prompt
        request
    }

    // New: provider-specific response extraction
    fn extract_structured_response(
        &self,
        response: &ChatResponse,
    ) -> Option<String> {
        // Default: return response.text
        // Anthropic override: extract from tool_use block
        Some(response.text.clone())
    }
}

enum StructuredOutputSupport {
    Native,       // OpenAI, Gemini — API-level constraint
    ToolTrick,    // Anthropic — can be faked via tool_use
    PromptOnly,   // Local models — no API support, prompt + validate
    None,         // Provider doesn't support it at all
}
```

**This is the most architecturally clean solution.** The VM doesn't need provider-specific
logic — it calls `prepare_structured_request` and `extract_structured_response`, and each
provider handles its own details.

### G. `schema` as a Tool (The Anthropic Pattern as a First-Class Concept)

The Anthropic "tool trick" works by defining a fake tool whose input_schema is the desired
output schema, then extracting the structured data from the tool_use response. This pattern
could be elevated to a first-class concept:

```concerto
// Under the hood, when targeting Anthropic:
// The runtime registers Summary as a "tool" with input_schema = Summary's JSON Schema
// The LLM "calls" this tool, producing structured output
// The runtime intercepts the tool_use and extracts the data
```

This would be invisible to the user — purely a runtime strategy. But understanding this pattern
is key to making `execute_with_schema` work on Anthropic.

### H. Multi-Provider Schema Negotiation

For pipelines that use multiple providers across stages, the runtime could negotiate schema
enforcement per-stage based on which provider each agent uses:

```concerto
pipeline review_pipeline(input: String) {
    // Stage 1: OpenAI agent — uses native response_format
    stage draft = drafter.execute_with_schema<Draft>(input);

    // Stage 2: Anthropic agent — runtime auto-switches to tool_use trick
    stage review = reviewer.execute_with_schema<Review>(draft.text);

    // Stage 3: Local model — runtime injects schema into prompt
    stage score = scorer.execute_with_schema<Score>(review.feedback);
}
```

Same Concerto code, three different enforcement strategies, all transparent to the user.


---

## Recommended Implementation Path

### Phase 1: Fix the immediate bug (runtime only, no language changes)
1. Add `supports_structured_output()` to `LlmProvider` trait
2. Implement the tool_use trick in `AnthropicProvider::build_request_body()`
3. Add `extract_structured_response()` to handle tool_use extraction
4. Log a warning when falling back to prompt-only enforcement
5. This fixes the silent degradation with zero Concerto language changes

### Phase 2: `output` block on agents (language feature)
1. Spec the `output:` config field in `spec/07-agents.md`
2. Add to compiler (parser, AST, semantic, codegen)
3. Runtime uses it to auto-apply schema on every `execute()` call
4. `execute_with_schema<T>` remains for ad-hoc/dynamic use

### Phase 3: Response transforms and format strategy (language feature)
1. Spec `format:` field and built-in transforms
2. Implement transform pipeline in runtime
3. Add `@schema(inject: "system")` decorator

### Phase 4: Full provider adapter redesign
1. Redesign `LlmProvider` trait with capability model
2. Add `prepare_structured_request` / `extract_structured_response`
3. Each provider self-describes its capabilities
4. Runtime adapts strategy automatically


---

## Open Questions

1. **Should the tool_use trick be the default for Anthropic, or should users opt in?**
   The trick is well-established but it changes the request shape (adds a tool, potentially
   changes stop_reason). Auto-applying it is convenient but might surprise users debugging
   their API calls.

2. **How do we handle providers we don't know about?** If someone connects a custom
   OpenAI-compatible endpoint, should we assume OpenAI capabilities? Should the manifest
   allow declaring provider capabilities?

3. **What about streaming + structured output?** OpenAI streams structured output token by
   token. Anthropic streams tool_use differently. This compounds the provider abstraction
   challenge.

4. **Should schemas be optional or required for agents?** If we add `output:` to agents,
   should some agents be "unstructured by default" (return raw text) and others "structured
   by default"? Or should all agents always have a schema?

5. **Grammar-based constraints for local models?** Some local inference engines (llama.cpp,
   vLLM) support GBNF grammars that can enforce JSON schemas at the token level. Should
   Concerto support compiling schemas to GBNF for these backends?
