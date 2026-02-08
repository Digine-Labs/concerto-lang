# 10 - Emit System

## Overview

The emit system is Concerto's **primary output mechanism** for external integration. When the runtime executes Concerto code, it communicates with the outside world through emits -- named output channels that carry typed payloads. The host application listens on these channels to receive data from the Concerto harness.

Concerto's emit system is **bidirectional**: emits can optionally await a response from the host, enabling human-in-the-loop patterns and external tool execution.

## Basic Emit

```concerto
emit("channel_name", payload);
```

### Examples

```concerto
// Emit a simple string
emit("log", "Processing started");

// Emit structured data
emit("classification", {
    "label": "legal",
    "confidence": 0.95,
    "model": "gpt-4o",
});

// Emit a variable
let result = agent.execute(prompt)?;
emit("response", result.text);

// Emit without payload
emit("heartbeat");
```

## Channels

Emits are organized by **channel names** -- string identifiers that the host application subscribes to.

### Built-in Channels

| Channel | Purpose | Payload |
|---------|---------|---------|
| `"log"` | General log messages | `String` |
| `"error"` | Error events | `String` or `{ message, code, details }` |
| `"warning"` | Warning events | `String` |
| `"debug"` | Debug output (verbose) | `Any` |
| `"result"` | Primary output results | `Any` |

### Custom Channels

Any string can be used as a channel name:

```concerto
emit("classification", result);
emit("token_usage", { "in": 150, "out": 42 });
emit("pipeline:stage_complete", { "stage": "extract", "duration_ms": 1200 });
emit("agent:tool_call", { "tool": "search", "query": "latest news" });
```

### Channel Naming Conventions

- Use lowercase with underscores for simple channels: `"log"`, `"error"`, `"result"`
- Use colon-separated namespaces for complex systems: `"pipeline:stage_1"`, `"agent:classifier"`
- Prefix tool channels with `"tool:"`: `"tool:read_file"`, `"tool:db_query"`

## Bidirectional Emit (Await)

The most powerful feature of the emit system. Using `await emit(...)`, Concerto code can send data to the host and **wait for a response**.

```concerto
// Send a question to the host and wait for an answer
let user_input = await emit("request:user_input", {
    "prompt": "Please provide the document to classify:",
    "type": "text",
});

// Use the response
let result = Classifier.execute(user_input)?;
emit("result", result.text);
```

### How It Works

1. Concerto code calls `await emit(channel, payload)`
2. Runtime suspends execution at this point
3. Runtime sends the payload to the host via the channel
4. Host processes the request and sends a response back
5. Runtime resumes execution with the response as the return value

### Host Side (Rust)

```
// Host (Rust) side:
runtime.on_emit("request:user_input", |payload| async {
    let prompt = payload["prompt"].as_str();
    // Display prompt to user, collect input...
    let user_response = get_user_input(prompt).await;
    Ok(user_response)
});
```

### Use Cases for Bidirectional Emit

#### Human-in-the-Loop

```concerto
let classification = Classifier.execute_with_schema<Classification>(doc)?;

if classification.confidence < 0.7 {
    // Low confidence -- ask a human
    let human_decision = await emit("human_review", {
        "document": doc,
        "ai_classification": classification,
        "message": "AI is uncertain. Please confirm the classification:",
    });

    emit("result", human_decision);
} else {
    emit("result", classification);
}
```

#### External Tool Execution

```concerto
// Tool delegates to host for actual execution
let file_content = await emit("tool:read_file", { "path": "/data/input.txt" });
let db_result = await emit("tool:db_query", { "sql": "SELECT * FROM users" });
```

#### Approval Gates

```concerto
pipeline CriticalWorkflow {
    stage analyze(input: String) -> Analysis {
        Analyzer.execute_with_schema<Analysis>(input)?
    }

    stage approve(analysis: Analysis) -> Bool {
        let approval = await emit("approval_required", {
            "analysis": analysis,
            "risk_level": analysis.risk,
        });
        approval == "approved"
    }

    stage execute(approved: Bool) -> String {
        if approved {
            Executor.execute("Proceed with plan")?
        } else {
            "Workflow cancelled by reviewer"
        }
    }
}
```

## Typed Emit (with Schema)

For type-safe external contracts, emits can declare their payload schema:

```concerto
schema ClassificationEmit {
    label: String,
    confidence: Float,
    model: String,
    timestamp: String,
}

// Compiler verifies the payload matches the schema
emit<ClassificationEmit>("classification", {
    "label": result.label,
    "confidence": result.confidence,
    "model": "gpt-4o",
    "timestamp": std::time::now(),
});
```

## Emit Buffering

By default, emits are sent immediately. The runtime can be configured for batch mode:

```concerto
// Runtime configuration (host side):
// runtime.set_emit_mode(EmitMode::Buffered { flush_interval_ms: 100 });
// runtime.set_emit_mode(EmitMode::Immediate);  // Default
```

In buffered mode:
- Emits accumulate in a buffer
- Buffer is flushed at the configured interval or when full
- `flush_emits()` can force immediate delivery

```concerto
// Force flush (useful before critical points)
emit("step_1_complete", result);
flush_emits();  // Ensure host receives before continuing
```

## Emit in Pipelines

Emits are commonly used to report pipeline progress:

```concerto
pipeline DataPipeline {
    stage fetch(url: String) -> String {
        emit("pipeline:progress", { "stage": "fetch", "status": "started" });
        let data = HttpTool.get(url)?;
        emit("pipeline:progress", { "stage": "fetch", "status": "complete" });
        data
    }

    stage process(data: String) -> ProcessedData {
        emit("pipeline:progress", { "stage": "process", "status": "started" });
        let result = Processor.execute_with_schema<ProcessedData>(data)?;
        emit("pipeline:progress", { "stage": "process", "status": "complete" });
        result
    }

    stage output(processed: ProcessedData) -> String {
        emit("pipeline:progress", { "stage": "output", "status": "started" });
        emit("pipeline:result", processed);
        emit("pipeline:progress", { "stage": "output", "status": "complete" });
        "Pipeline complete"
    }
}
```

## Emit Error Handling

Emit itself can fail (host not listening, channel error, timeout on bidirectional):

```concerto
// Fire-and-forget emit -- never fails (drops silently if no listener)
emit("log", "This is best-effort");

// Bidirectional emit can fail
let response = await emit("tool:external_api", request);
// Returns Result<Any, EmitError> -- handle the error

match await emit("request:approval", data) {
    Ok(response) => process(response),
    Err(EmitError::Timeout) => emit("error", "Approval timeout"),
    Err(EmitError::NoListener) => emit("error", "No approval handler configured"),
    Err(e) => emit("error", e.message),
}
```

## Host API Summary

The host application interacts with the emit system through the runtime API:

| Host Method | Description |
|-------------|-------------|
| `runtime.on(channel, handler)` | Subscribe to unidirectional emits |
| `runtime.on_emit(channel, async_handler)` | Subscribe to bidirectional emits (handler returns response) |
| `runtime.off(channel)` | Unsubscribe from a channel |
| `runtime.emit_to(channel, data)` | Send data into the runtime (for bidirectional response) |
| `runtime.list_channels()` | List all channels that have been emitted to |
| `runtime.set_emit_mode(mode)` | Configure immediate or buffered mode |
