# 27 - Bidirectional Host Streaming

## Overview

Hosts are external agent systems (Claude Code, Cursor, Devin) connected to Concerto via stdio subprocess transport. The current host execution model is **fire-and-forget**: send a prompt, read one line, done. Real external agents produce **streams of heterogeneous messages** over time — progress updates, questions needing answers, approval gates, and final results.

The **`listen` expression** creates a message loop with typed handlers, enabling bidirectional communication between Concerto and host processes. The key pattern is **agent-supervises-agent**: a Concerto-defined agent answers the host's questions autonomously.

### Host-as-Middleware Model

In practice, a host should be implemented as **middleware**:

- upstream: talks to an external agent system (Claude Code, Cursor, custom worker)
- downstream: talks to Concerto over NDJSON stdio (`question`/`approval` requests and `response` replies)

This keeps Concerto protocol handling isolated from vendor-specific APIs and lets teams swap external agent backends without changing Concerto orchestration code. A self-contained local middleware harness lives in `examples/bidirectional_host_middleware/`. A Claude Code-oriented reference adapter lives in `hosts/claude_code/`.

```
+------------------+         questions          +------------------+
|                  | ───────────────────────── > |                  |
|   Host (Worker)  |                             |  Agent (Decider) |
|   Claude Code    | < ─────────────────────── ─ |  Architect       |
|   Cursor, Devin  |         answers             |  QA Lead, etc.   |
+------------------+                             +------------------+
        |                                                |
        | does the work                                  | makes decisions
        | (reads files, writes code,                     | (design choices,
        |  runs tests, deploys)                          |  risk assessment,
        |                                                |  approval/rejection)
        v                                                v
   External System                               LLM API (OpenAI, etc.)
```

## Wire Protocol

Hosts communicate via **Newline-Delimited JSON** (NDJSON). Each line from the host's stdout is a JSON object with a `type` field.

### Message Format (Host → Concerto)

```json
{"type": "progress", "message": "Reading files...", "percent": 10}
{"type": "question", "question": "Use RS256 or HS256?", "context": "JWT signing"}
{"type": "approval", "description": "Delete 3 files", "risk_level": "medium"}
{"type": "result", "text": "Done. 12 files modified.", "files_changed": 12}
{"type": "error", "message": "Permission denied"}
{"type": "log", "level": "debug", "message": "Cache invalidated"}
```

All fields except `type` become the handler's parameter value (as a Map or typed Struct).

### Response Format (Concerto → Host)

When a handler returns a non-nil value, Concerto writes it back to the host's stdin:

```json
{"type": "response", "in_reply_to": "question", "value": "Use RS256"}
```

### Message Types

| Type | Direction | Meaning | Requires Response? |
|------|-----------|---------|-------------------|
| `progress` | host → concerto | Status update | No |
| `question` | host → concerto | Needs input to continue | **Yes** |
| `approval` | host → concerto | Needs yes/no to proceed | **Yes** |
| `result` | host → concerto | Final output, terminates loop | No |
| `error` | host → concerto | Error, terminates loop | No |
| `log` | host → concerto | Debug/info message | No |
| `partial` | host → concerto | Incremental output (streaming) | No |

### Terminal Types

`result` and `error` are **terminal** — they end the listen loop. All other types are non-terminal.

- When `result` is received: the listen expression returns `Ok(payload)` where payload contains all message fields except `type`
- When `error` is received: the listen expression returns `Err(message)`

### Plain Text Fallback

If a host outputs a line that is not valid JSON or lacks a `type` field, it is treated as:
```json
{"type": "result", "text": "<the line>"}
```
This preserves backward compatibility with non-streaming hosts.

## Message Schemas

Message types can be typed using Concerto's existing `schema` construct:

```concerto
schema HostProgress {
    message: String,
    percent?: Int,
    stage?: String,
}

schema HostQuestion {
    question: String,
    context?: String,
    options?: Array<String>,
}

schema HostApproval {
    description: String,
    risk_level?: String,
}
```

Schemas are optional — handlers can also receive untyped dynamic values.

## Listen Expression

### Syntax

```
listen <host-call-expr> {
    <string-literal> => |<param>[: <Type>]| { <body> },
    ...
}
```

### Semantics

The `listen` expression is a blocking message loop that:
1. Sends the initial prompt to the host
2. Reads messages from the host's stdout (NDJSON)
3. Dispatches each message to a matching handler by type string
4. Sends handler return values back to the host's stdin (for bidirectional types)
5. Terminates when a `result` or `error` message is received
6. Returns `Result<Value>` — Ok on result, Err on error

### Examples

#### Basic: Fire-and-forget progress + bidirectional questions

```concerto
host ClaudeCode {
    connector: claude_code,
    output_format: "json",
    timeout: 600,
}

agent Architect uses openai {
    model: "gpt-4o",
    system_prompt: "You are a senior architect. Make clear, decisive technical choices.",
}

let result = listen ClaudeCode.execute("Refactor auth module to use JWT") {
    "progress" => |msg: HostProgress| {
        emit("host:progress", msg);
    },
    "question" => |q: HostQuestion| {
        let answer = Architect.execute("Answer: ${q.question}")?;
        answer.text
    },
};
```

#### Specialist routing: Different agents for different question types

```concerto
agent SecurityReviewer uses anthropic {
    model: "claude-sonnet-4-5-20250929",
    system_prompt: "You are a security engineer. Evaluate code for vulnerabilities.",
}

agent QaLead uses openai {
    model: "gpt-4o",
    system_prompt: "You are a QA lead. Prioritize test coverage and correctness.",
}

let result = listen ClaudeCode.execute("Implement OAuth2 login") {
    "question" => |q: HostQuestion| {
        if q.question.contains("security") || q.question.contains("auth") {
            let answer = SecurityReviewer.execute("Answer: ${q.question}")?;
            answer.text
        } else if q.question.contains("test") {
            let answer = QaLead.execute("Answer: ${q.question}")?;
            answer.text
        } else {
            let answer = Architect.execute("Answer: ${q.question}")?;
            answer.text
        }
    },
    "approval" => |req: HostApproval| {
        let decision = SecurityReviewer.execute("Approve? ${req.description}")?;
        if decision.text.contains("approve") { "yes" } else { "no" }
    },
};
```

#### Untyped handlers (dynamic parameter)

```concerto
let result = listen ClaudeCode.execute("Build an API") {
    "progress" => |msg| {
        emit("host:progress", msg);
    },
    "question" => |q| {
        let answer = Architect.execute("Answer: ${q.question}")?;
        answer.text
    },
};
```

When no type annotation is provided, the handler parameter is a `Map<String, Value>` with all message fields except `type`.

### Handler Behavior

| Handler returns | Runtime action |
|----------------|---------------|
| Non-nil value (String, Struct, etc.) | Write response to host stdin (bidirectional) |
| Nil / no tail expression | Fire-and-forget — no response sent |

### Unhandled Messages

When a message type has no matching handler:
- The runtime emits a `listen:unhandled` event via the emit system
- The message is logged and the loop continues
- This is NOT an error — hosts may send informational types that Concerto doesn't need to process

### Error Handling

- Handler bodies can use `?` for error propagation (try/catch within handlers)
- If a handler throws an uncaught error, the listen loop terminates with that error
- The listen expression itself can be wrapped in try/catch at the call site
- Host process exit without `result`/`error` returns `Err("host exited without result")`

## Interaction with Existing Constructs

### Host Declaration (unchanged)

The host declaration remains the same. No protocol block is required — handler type annotations implicitly define the protocol.

```concerto
host ClaudeCode {
    connector: claude_code,
    output_format: "json",
    timeout: 600,
}
```

### execute() backward compatibility

The existing `execute()` method is unchanged. Code without `listen` continues to work:

```concerto
// Old style: fire-and-forget (still works)
let result = ClaudeCode.execute("Do something")?;
```

`listen` is purely additive — it creates a new code path for streaming communication.

### Memory

Hosts manage their own internal memory and context (e.g., Claude Code has its own conversation history). Concerto cannot inject memory into the host — it can only send prompts and handle responses. Memory declared in Concerto can still track the Concerto-side Q&A history for auditing:

```concerto
memory supervision_log: Memory = Memory::new();

let result = listen ClaudeCode.execute("Build API") {
    "question" => |q: HostQuestion| {
        let answer = Architect
            .with_memory(supervision_log)
            .execute("Answer: ${q.question}")?;
        answer.text
    },
};

// supervision_log now contains the Architect's Q&A decisions
emit("decisions", supervision_log.messages());
```

### Pipelines

Listen expressions can be used inside pipeline stages:

```concerto
pipeline BuildAndTest(spec: String) {
    stage implement = listen ClaudeCode.execute("Implement: ${spec}") {
        "question" => |q: HostQuestion| {
            let answer = Architect.execute("Answer: ${q.question}")?;
            answer.text
        },
    };

    stage test = listen TestRunner.execute("Test: ${implement}") {
        "question" => |q: HostQuestion| {
            let answer = QaLead.execute("Answer: ${q.question}")?;
            answer.text
        },
    };
}
```

## Implementation Notes

### Compilation

- `listen` is a keyword (added to lexer)
- `ListenHandler` is an AST node with `message_type: String`, `param: Param`, `body: Block`
- `ExprKind::Listen { call, handlers }` is an expression variant
- Each handler body compiles to an instruction block (same pattern as pipeline stages, NOT closures)
- Handler instruction blocks are stored in `IrListen.handlers[].instructions`
- The `ListenBegin` opcode triggers the VM message loop

### Runtime

- The VM's `run_listen_loop()` reads NDJSON messages and dispatches to handler instruction blocks via `run_loop_until(stop_depth)`
- This reuses the proven pipeline stage execution mechanism
- Handler return values are sent back to the host as JSON responses
- The loop terminates on `result`/`error` messages or host process exit

### Timeouts

The host's `timeout` config applies to the entire listen session. If the host doesn't send a terminal message within the timeout, the listen loop returns an error.
