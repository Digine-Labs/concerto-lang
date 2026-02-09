# 07 - Models

## Overview

Models are the **core abstraction** of Concerto. A model encapsulates an LLM-powered entity with a base model, provider, system prompt, attached tools, memory hashmap, and execution methods. Models are defined with a class-like syntax and provide typed interfaces for interacting with LLMs.

## Model Definition

```concerto
model ModelName {
    // Required fields
    provider: connection_name,

    // Optional configuration fields
    base: "model-id",
    temperature: 0.7,
    max_tokens: 1000,
    system_prompt: "System prompt text",

    // Attachments
    memory: hashmap_ref,
    tools: [Tool1, Tool2],

    // Behavior configuration
    retry_policy: { max_attempts: 3, backoff: "exponential" },
    timeout: 30,
}
```

### Full Example

```concerto
connect openai {
    api_key: env("OPENAI_API_KEY"),
    default_model: "gpt-4o",
}

hashmap shared_memory: HashMap<String, String> = HashMap::new();

model DocumentClassifier {
    provider: openai,
    base: "gpt-4o",
    temperature: 0.2,
    max_tokens: 500,
    system_prompt: """
        You are a document classifier. Given a document, classify it into
        one of the following categories: legal, technical, financial, general.

        Always respond with valid JSON matching the requested schema.
        """,

    memory: shared_memory,
    tools: [FileConnector, HttpTool],

    retry_policy: { max_attempts: 3, backoff: "exponential" },
    timeout: 30,
}
```

## Model Fields

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `provider` | identifier | Reference to a `connect` block |

### Optional Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `base` | String | Provider's default | LLM model ID |
| `temperature` | Float | Provider default | Sampling temperature (0.0 - 2.0) |
| `max_tokens` | Int | Provider default | Maximum response tokens |
| `system_prompt` | String | None | System message prepended to every call |
| `memory` | HashMapRef | None | Attached in-memory hashmap |
| `tools` | Array of Tool types | `[]` | Tools available to the model |
| `retry_policy` | Object | No retries | Retry configuration |
| `timeout` | Int | 30 | Timeout in seconds per call |
| `top_p` | Float | Provider default | Nucleus sampling parameter |
| `stop_sequences` | Array\<String\> | `[]` | Stop generation sequences |

## Execution Methods

Models have built-in methods for interacting with the LLM.

### `execute(prompt)` -- Single Prompt

Sends a single prompt to the LLM and returns the raw response.

```concerto
let response: Result<Response, AgentError> = DocumentClassifier.execute(
    "Classify this document: ${document_text}"
);

match response {
    Ok(r) => emit("result", r.text),
    Err(e) => emit("error", e.message),
}
```

The `Response` type contains:
```concerto
// Response fields (read-only)
response.text         // String -- raw response text
response.tokens_in    // Int -- input token count
response.tokens_out   // Int -- output token count
response.model        // String -- model used
response.provider     // String -- provider name
response.latency_ms   // Int -- response time in milliseconds
```

### `execute_with_schema<T>(prompt)` -- Structured Output

Sends a prompt and validates the response against a schema. Automatically retries on schema mismatch.

```concerto
schema Classification {
    label: String,
    confidence: Float,
    reasoning: String,
}

let result: Result<Classification, AgentError> =
    DocumentClassifier.execute_with_schema<Classification>(
        "Classify: ${document_text}"
    );

match result {
    Ok(classification) => {
        emit("classification", {
            "label": classification.label,
            "confidence": classification.confidence,
        });
    },
    Err(e) => emit("error", e.message),
}
```

Schema validation behavior:
1. Send prompt to LLM (include schema description in the prompt automatically)
2. Parse response as JSON
3. Validate against the schema type `T`
4. If validation fails and retries remain, re-prompt with error feedback
5. Return `Ok(T)` on success or `Err(SchemaError)` after all retries exhausted

### `chat(messages)` -- Multi-Turn Conversation

Sends a list of messages for multi-turn conversation context.

```concerto
let messages = [
    Message::system("You are a helpful coding assistant."),
    Message::user("Write a function to sort an array."),
];

let response = CodingAssistant.chat(messages)?;

// Continue the conversation
let mut conversation = messages;
conversation.push(Message::assistant(response.text));
conversation.push(Message::user("Now add error handling."));

let followup = CodingAssistant.chat(conversation)?;
```

### `stream(prompt)` -- Streaming Response

Returns an async iterator of response chunks for real-time output.

```concerto
let chunks = DocumentClassifier.stream("Classify: ${document_text}");

let mut full_response = "";
for chunk in chunks.await {
    full_response += chunk.text;
    emit("stream_chunk", chunk.text);
}

emit("stream_complete", full_response);
```

## Custom Model Methods

Models can define custom methods via `impl` blocks:

```concerto
impl DocumentClassifier {
    /// Classify a document with confidence threshold
    pub fn classify_with_threshold(
        self,
        text: String,
        min_confidence: Float,
    ) -> Result<Classification, AgentError> {
        let result = self.execute_with_schema<Classification>(
            "Classify: ${text}"
        )?;

        if result.confidence < min_confidence {
            Err(AgentError::new(
                "Low confidence: ${result.confidence} < ${min_confidence}"
            ))
        } else {
            Ok(result)
        }
    }

    /// Batch classify multiple documents
    pub async fn batch_classify(
        self,
        documents: Array<String>,
    ) -> Array<Result<Classification, AgentError>> {
        let mut results = [];
        for doc in documents {
            results.push(self.classify_with_threshold(doc, 0.8));
        }
        results
    }
}
```

## Model Composition

Models can call other models, enabling complex orchestration patterns.

```concerto
model Orchestrator {
    provider: openai,
    base: "gpt-4o",
    system_prompt: "You coordinate document processing workflows.",
}

model Extractor {
    provider: openai,
    base: "gpt-4o-mini",
    system_prompt: "You extract key information from documents.",
}

model Summarizer {
    provider: anthropic,
    base: "claude-sonnet-4-20250514",
    system_prompt: "You write concise summaries.",
}

impl Orchestrator {
    pub async fn process_document(self, doc: String) -> Result<String, AgentError> {
        // Step 1: Extract key information
        let extraction = Extractor.execute(
            "Extract key entities and facts from: ${doc}"
        ).await?;

        // Step 2: Summarize
        let summary = Summarizer.execute(
            "Summarize these findings: ${extraction.text}"
        ).await?;

        Ok(summary.text)
    }
}
```

## Decorators

Decorators modify model behavior without changing the model definition.

### `@retry`

```concerto
@retry(max: 5, backoff: "exponential", delay_ms: 1000)
model UnreliableClassifier {
    provider: openai,
    base: "gpt-4o-mini",
    // ...
}
```

### `@timeout`

```concerto
@timeout(seconds: 60)
model LongRunningModel {
    provider: openai,
    base: "gpt-4o",
    // ...
}
```

### `@log`

Logs all model calls (prompt, response, timing) to the emit system.

```concerto
@log(channel: "model_log")
model Classifier {
    provider: openai,
    base: "gpt-4o",
    // ...
}
// Every call emits to "model_log" channel with full request/response details
```

### `@cache`

Caches responses for identical prompts during execution.

```concerto
@cache(ttl_seconds: 300)
model CachedLookup {
    provider: openai,
    base: "gpt-4o-mini",
    // ...
}
// Identical prompts within 5 minutes return cached response
```

### Combining Decorators

```concerto
@retry(max: 3)
@timeout(seconds: 30)
@log(channel: "debug")
model ProductionClassifier {
    provider: openai,
    base: "gpt-4o",
    temperature: 0.1,
    system_prompt: "You are a production document classifier.",
}
```

## Model Lifecycle

### 1. Definition Phase (Compile Time)
- Model struct is defined with fields and methods
- Type checking validates field types and method signatures
- Tool compatibility is verified

### 2. Initialization Phase (Runtime Start)
- Connection to LLM provider is established
- Memory hashmap reference is bound
- Tool registry is populated
- Model instance is ready for execution

### 3. Execution Phase (Runtime)
- Methods called on the model send prompts to LLM
- Responses are received, parsed, and returned
- Tools are invoked when the LLM requests them
- Memory hashmap is read/written during execution
- Emits are produced as configured

### 4. Teardown Phase (Runtime End)
- Open connections are closed gracefully
- Pending async operations are resolved or cancelled
- Memory hashmaps can be serialized if configured

## Dynamic Model Instantiation

For cases where model configuration is determined at runtime:

```concerto
fn create_model(base_model: String, temperature: Float) -> ModelRef {
    model DynamicModel {
        provider: openai,
        base: base_model,
        temperature: temperature,
        system_prompt: "You are a flexible assistant.",
    }

    DynamicModel.spawn()
}

let m = create_model("gpt-4o", 0.5);
let response = m.execute("Hello")?;
m.shutdown();
```

## Model Best Practices

1. **Use specific system prompts** -- Tell the model exactly what role it plays
2. **Prefer `execute_with_schema`** -- Structured output is more reliable than parsing raw text
3. **Set low temperature for classification** -- Use 0.1-0.3 for deterministic tasks
4. **Use `@retry` for production** -- LLM calls can fail intermittently
5. **Attach only needed tools** -- More tools = more confusion for the LLM
6. **Use different base models for different tasks** -- Fast model for simple tasks, powerful model for complex reasoning
7. **Set timeouts** -- Prevent hanging on slow LLM responses
