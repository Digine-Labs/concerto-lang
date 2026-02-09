# 11 - LLM Connections

## Overview

The `connect` block defines how Concerto's runtime communicates with LLM providers. Connections are declared at module scope and referenced by models. The runtime handles the actual HTTP communication, authentication, retry logic, and rate limiting.

## Connection Declaration

```concerto
connect connection_name {
    api_key: env("API_KEY_VAR"),
    base_url: "https://api.provider.com/v1",
    default_model: "model-id",
    // ... additional configuration
}
```

### Full Example

```concerto
connect openai {
    api_key: env("OPENAI_API_KEY"),
    base_url: "https://api.openai.com/v1",
    default_model: "gpt-4o",
    timeout: 60,
    retry: {
        max_attempts: 3,
        backoff: "exponential",
        initial_delay_ms: 1000,
    },
    rate_limit: {
        requests_per_minute: 60,
        tokens_per_minute: 150000,
    },
}

connect anthropic {
    api_key: env("ANTHROPIC_API_KEY"),
    default_model: "claude-sonnet-4-20250514",
    timeout: 60,
}

connect local_ollama {
    base_url: "http://localhost:11434/v1",
    default_model: "llama3.1",
    // No api_key needed for local Ollama
}
```

## Connection Fields

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `api_key` | String | API authentication key (use `env()` for secrets) |

**Note**: `api_key` is optional for local providers (Ollama) where no authentication is needed.

### Optional Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `base_url` | String | Provider default | API endpoint URL |
| `default_model` | String | Provider default | Default model for models using this connection |
| `timeout` | Int | 30 | Request timeout in seconds |
| `retry` | Object | No retries | Retry policy configuration |
| `rate_limit` | Object | No limit | Rate limiting configuration |
| `headers` | Map\<String, String\> | `{}` | Additional HTTP headers |
| `organization` | String | None | Organization ID (OpenAI) |
| `project` | String | None | Project ID (OpenAI) |

### Retry Configuration

```concerto
retry: {
    max_attempts: 3,              // Total attempts (including initial)
    backoff: "exponential",       // "none", "linear", "exponential"
    initial_delay_ms: 1000,       // Delay before first retry
    max_delay_ms: 30000,          // Maximum delay cap
    retry_on: ["rate_limit", "server_error", "timeout"],  // Which errors trigger retry
}
```

### Rate Limit Configuration

```concerto
rate_limit: {
    requests_per_minute: 60,      // Max requests per minute
    tokens_per_minute: 150000,    // Max tokens per minute
    concurrent_requests: 10,      // Max concurrent in-flight requests
}
```

When rate limits are reached, the runtime queues requests and processes them when capacity is available.

## Supported Providers

### OpenAI

```concerto
connect openai {
    api_key: env("OPENAI_API_KEY"),
    default_model: "gpt-4o",
}
```

Supported models: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `o1`, `o1-mini`, etc.

### Anthropic

```concerto
connect anthropic {
    api_key: env("ANTHROPIC_API_KEY"),
    default_model: "claude-sonnet-4-20250514",
}
```

Supported models: `claude-opus-4-20250514`, `claude-sonnet-4-20250514`, `claude-haiku-4-5-20251001`, etc.

### Google (Gemini)

```concerto
connect google {
    api_key: env("GOOGLE_API_KEY"),
    default_model: "gemini-2.0-flash",
}
```

### Ollama (Local)

```concerto
connect ollama {
    base_url: "http://localhost:11434/v1",
    default_model: "llama3.1",
}
```

### Custom (OpenAI-Compatible)

Any API that follows the OpenAI chat completions format:

```concerto
connect custom_provider {
    api_key: env("CUSTOM_API_KEY"),
    base_url: "https://my-api.example.com/v1",
    default_model: "my-model",
}
```

## Environment Variables

The `env()` function reads environment variables at runtime. This is the recommended way to handle secrets.

```concerto
let key = env("OPENAI_API_KEY");       // Returns String
let key = env("MISSING_VAR");          // Runtime error if not set

// With default
let key = env("OPTIONAL_VAR") ?? "default_value";
```

**Security**: API keys should never be hardcoded in `.conc` files. Always use `env()`.

## Using Connections in Agents

Agents reference connections by name:

```concerto
connect openai {
    api_key: env("OPENAI_API_KEY"),
    default_model: "gpt-4o",
}

model FastClassifier {
    provider: openai,           // References the connection above
    base: "gpt-4o-mini",      // Override the connection's default model
    temperature: 0.1,
}

model DeepAnalyzer {
    provider: openai,           // Same connection, different model
    base: "gpt-4o",
    temperature: 0.7,
    max_tokens: 4000,
}
```

## Model Aliasing

Create named aliases for model identifiers, enabling easy switching:

```concerto
connect openai {
    api_key: env("OPENAI_API_KEY"),
    models: {
        "fast": "gpt-4o-mini",
        "smart": "gpt-4o",
        "reasoning": "o1",
    },
}

model Classifier {
    provider: openai,
    base: "fast",              // Resolves to "gpt-4o-mini"
}

model Researcher {
    provider: openai,
    base: "smart",             // Resolves to "gpt-4o"
}
```

## Runtime Override

The host application can override connection settings without modifying Concerto code:

```
// Host (Rust) side:
runtime.override_connection("openai", ConnectionConfig {
    api_key: "sk-test-key",
    base_url: "http://localhost:8080/mock-api",  // Point to mock server for testing
    ..Default::default()
});
```

This is essential for:
- **Testing**: Point to mock LLM servers
- **Staging**: Use different API keys per environment
- **Proxying**: Route through a gateway for logging/cost control

## Streaming Support

Connections support streaming responses for real-time output:

```concerto
model StreamingAssistant {
    provider: openai,
    base: "gpt-4o",
}

// Stream response chunks
let stream = StreamingAssistant.stream("Write a long essay about AI.")?;
let mut full_text = "";
for chunk in stream {
    full_text += chunk.text;
    emit("stream_chunk", chunk.text);  // Real-time output to host
}
emit("stream_complete", full_text);
```

## Token Tracking

The runtime automatically tracks token usage per connection and per model:

```concerto
// After model calls, token usage is tracked automatically
let response = Classifier.execute(prompt)?;

// Access via runtime metrics (host API)
// runtime.get_metrics().tokens_by_connection("openai")
// runtime.get_metrics().tokens_by_model("Classifier")
// runtime.get_metrics().estimated_cost("openai")

// Or emit token info from within Concerto
emit("tokens", {
    "input": response.tokens_in,
    "output": response.tokens_out,
    "model": response.model,
});
```

## Multiple Providers in One Program

A Concerto program can use multiple providers simultaneously:

```concerto
connect openai {
    api_key: env("OPENAI_API_KEY"),
    default_model: "gpt-4o-mini",
}

connect anthropic {
    api_key: env("ANTHROPIC_API_KEY"),
    default_model: "claude-sonnet-4-20250514",
}

// Use the best model for each task
model QuickClassifier {
    provider: openai,
    base: "gpt-4o-mini",       // Fast, cheap
}

model DeepAnalyzer {
    provider: anthropic,
    base: "claude-sonnet-4-20250514",  // Thorough analysis
}

fn main() {
    let classification = QuickClassifier.execute_with_schema<Category>(doc)?;

    if classification.needs_deep_analysis {
        let analysis = DeepAnalyzer.execute(doc)?;
        emit("result", analysis.text);
    } else {
        emit("result", classification);
    }
}
```
