# Bidirectional Host Streaming & Multi-Message Protocols

## The Problem

Hosts are currently **fire-and-forget**: send a prompt via stdin, read one line from stdout, done.
But real external agent systems (Claude Code, Cursor, Devin, custom planning agents) don't work
like that. A single "task" produces a **stream of heterogeneous messages** over time:

```
Concerto sends:  "Refactor the auth module to use JWT"
                         |
Claude Code sends back:  { type: "progress", message: "Reading auth files..." }
                         { type: "progress", message: "Found 12 files to modify" }
                         { type: "question", id: "q1", question: "Should I also update the tests?" }
                              ← Concerto's Architect agent decides: "yes, update all tests"
                         { type: "progress", message: "Updating tests..." }
                         { type: "progress", percent: 75, message: "Running test suite" }
                         { type: "question", id: "q2", question: "Test X fails. Fix or skip?" }
                              ← Concerto's Architect agent decides: "fix it, here's the approach..."
                         { type: "result", text: "Refactored 12 files, all tests pass" }
```

The key difference from human-in-the-loop: **another agent answers the questions**. The host
is the worker, a Concerto-defined agent is the supervisor. No human needed in the loop — agents
orchestrate each other.

### Current Limitation

`HostClient::execute()` does `reader.read_line(&mut line)` — reads **one line** and returns.
Every message type (progress, question, result) gets the same treatment. The runtime has no way
to distinguish "this is an intermediate update" from "this is the final answer" from "this needs
a response back to the host."

### What This Breaks

1. **Planning agents** that ask clarifying questions mid-task — currently impossible to answer
2. **Progress reporting** — currently lost or treated as the final response
3. **Approval gates** — host can't pause and ask "should I proceed?" during execution
4. **Multi-step workflows** — host does step 1, reports back, does step 2, etc.
5. **Long-running tasks** — no heartbeats, no way to know if the host is alive or stuck


---

## Key Insight: The Emit System Is the Right Primitive

Concerto already has bidirectional emit (`await emit(channel, payload)` spec'd in
`spec/10-emit-system.md`). The architecture is:

- **Outbound**: Concerto → external world (fire-and-forget or await response)
- **Inbound**: External world → Concerto (response to an awaited emit)

The host problem is the **inverse**: the host is the one producing a stream of messages, some
of which need responses from Concerto. We need the same bidirectional pattern, but initiated
from the host side.

The solution is to make host communication a **message loop** that routes each message type
through the emit system, allowing Concerto code to declaratively handle each message kind.


---

## Design: Host Message Protocol

### The Wire Protocol

Hosts communicate via **newline-delimited JSON** (NDJSON). Each line from the host's stdout is
a message with a `type` field:

```json
{"type": "progress", "message": "Reading files...", "percent": 10}
{"type": "progress", "message": "Analyzing code...", "percent": 45}
{"type": "question", "id": "q1", "question": "Update tests too?", "options": ["yes", "no"]}
{"type": "result", "text": "Done. 12 files modified.", "metadata": {...}}
{"type": "error", "message": "Permission denied on /etc/config"}
{"type": "log", "level": "debug", "message": "Cache invalidated"}
```

### Message Types

| Type | Direction | Meaning | Requires Response? |
|------|-----------|---------|-------------------|
| `progress` | host → concerto | Status update | No |
| `question` | host → concerto | Needs input to continue | **Yes** |
| `approval` | host → concerto | Needs yes/no to proceed | **Yes** |
| `result` | host → concerto | Final output, task complete | No (terminates loop) |
| `error` | host → concerto | Error, task failed | No (terminates loop) |
| `log` | host → concerto | Debug/info message | No |
| `partial` | host → concerto | Incremental output (streaming) | No |

### How Concerto Responds

When the host sends a `question` or `approval`, the runtime writes a response to the host's
stdin:

```json
{"answer_to": "q1", "value": "yes, update all tests"}
```

The host reads this from its stdin and continues.


---

## Language Design: `on` Handlers for Host Messages

### Option A: `on` Block (Event Handler Syntax)

A new `on` construct that registers handlers for message types during host execution.
**The key pattern: a Concerto agent answers the host's questions.**

```concerto
host ClaudeCode {
    connector: claude_code,
    output_format: "json",
    timeout: 600,
}

// The supervisor — a Concerto-defined agent that makes decisions for the host
agent Supervisor uses openai {
    model: "gpt-4o",
    system_prompt: "You are a senior engineer supervising a coding agent.
        When asked questions about implementation decisions, give concise,
        decisive answers. Always prefer correctness over speed.",
}

fn main() {
    let result = ClaudeCode.execute("Refactor auth to use JWT") {
        on progress(msg) {
            emit("host:progress", msg);
        }

        on question(q) {
            // Claude Code asks: "Should I also update the tests?"
            // Instead of asking a human, the Supervisor agent decides
            let answer = Supervisor.execute(
                "The coding agent asks: ${q.question}\nContext: ${q.context}\nDecide and answer concisely."
            )?;
            answer.text  // sent back to Claude Code via stdin
        }

        on approval(req) {
            // Claude Code asks: "This will delete 3 files. Proceed?"
            // Supervisor evaluates the risk and decides
            let decision = Supervisor.execute(
                "Approve or reject this action: ${req.description}\nRisk: ${req.risk_level}"
            )?;
            if decision.text.contains("approve") { "yes" } else { "no" }
        }

        on log(entry) {
            if entry.level == "error" {
                emit("host:error", entry.message);
            }
        }
    };

    emit("output", result);
}
```

**Key semantics:**
- The `on` block is scoped to a single `execute()` call — handlers are active only during that
  execution
- Handlers that return a value send that value back to the host (bidirectional)
- Handlers that don't return anything are fire-and-forget (progress, log)
- The loop terminates when the host sends `type: "result"` or `type: "error"`
- The `result` message's payload becomes the return value of `execute()`
- **Agent calls inside handlers** are the natural pattern — one agent supervises another

**Compiler representation:**
- `on` blocks compile to closures associated with the host call
- Each handler becomes a function in IR, referenced by the host execute instruction
- The VM runs a message loop instead of a single read_line

### Option B: Callback Map (Simpler, No New Syntax)

Pass handlers as a map argument instead of new syntax:

```concerto
let handlers = {
    on_progress: fn(msg) {
        emit("host:progress", msg);
    },
    on_question: fn(q) {
        // Agent answers the host's question
        let answer = Supervisor.execute("Answer this: ${q.question}")?;
        answer.text
    },
    on_approval: fn(req) {
        let decision = Supervisor.execute("Approve? ${req.description}")?;
        decision.text
    },
};

let result = ClaudeCode.execute_streaming("Refactor auth to use JWT", handlers)?;
```

Pros: No new syntax, uses existing fn/closure mechanism.
Cons: Less readable, handler names are strings, no compile-time validation of message types.

### Option C: `stream` Keyword (Iterator-Style)

Treat host output as a stream and process messages in a for loop:

```concerto
let stream = ClaudeCode.stream("Refactor auth to use JWT");

for msg in stream {
    match msg.type {
        "progress" => emit("host:progress", msg),
        "question" => {
            // Supervisor agent answers the host's question
            let answer = Supervisor.execute("Answer: ${msg.question}")?;
            stream.respond(msg.id, answer.text);
        },
        "approval" => {
            let decision = Supervisor.execute("Approve? ${msg.description}")?;
            stream.respond(msg.id, decision.text);
        },
        "result" => {
            emit("output", msg.text);
            break;
        },
        "error" => {
            emit("error", msg.message);
            break;
        },
    }
}
```

Pros: Uses existing `for`/`match` syntax. Very explicit control flow.
Cons: Verbose. Every host interaction needs boilerplate. `stream.respond()` is a new concept.

### Option D: `listen` Block (Recommended)

A dedicated `listen` construct that creates a message loop with typed handlers. The central
pattern: **a Concerto agent supervises the host, answering its questions in real time**.

```concerto
host ClaudeCode {
    connector: claude_code,
    output_format: "json",
    timeout: 600,
}

agent Architect uses openai {
    model: "gpt-4o",
    system_prompt: "You are a senior architect supervising implementation agents.
        Make clear, decisive technical choices. Prefer simplicity.",
}

memory supervision_log: Memory = Memory::new();

fn main() {
    let result = listen ClaudeCode.execute("Refactor auth module to use JWT") {
        on "progress" => fn(msg) {
            emit("host:progress", msg);
        },

        on "question" => fn(q) {
            // Claude Code asks: "Should I use RS256 or HS256 for JWT signing?"
            // The Architect agent answers — no human needed
            let answer = Architect
                .with_memory(supervision_log)
                .execute("The implementation agent asks: ${q.question}\nDecide concisely.")?;
            answer.text  // sent back to Claude Code
        },

        on "approval" => fn(req) {
            // Claude Code asks: "This will modify 8 files. Proceed?"
            // Architect evaluates risk and decides
            let decision = Architect.execute(
                "Approve or reject: ${req.description} (risk: ${req.risk_level})"
            )?;
            if decision.text.contains("approve") { "yes" } else { "no" }
        },
    };

    // supervision_log now contains the full Q&A history between Architect and Claude Code
    emit("output", result);
    emit("supervision_trace", supervision_log.messages());
}
```

**Why `listen`:**
- Makes it clear this is a long-lived message loop, not a single call
- `on "type"` handlers are explicit about which messages they handle
- Return value from handler = response sent back to host
- `result`/`error` types are **implicitly terminal** — no need to handle them manually
- Unhandled message types get a default behavior (log + ignore)
- The whole expression returns the `result` payload or throws on `error`
- **Agent calls inside handlers** are the natural pattern — agent-supervises-agent


---

## Runtime Architecture

### Message Loop in HostClient

Replace `read_line` (single response) with a **message loop**:

```
HostClient::execute_streaming(prompt, handlers) -> Result<Value>
    1. Write prompt to stdin
    2. Loop:
        a. Read one JSON line from stdout
        b. Parse the "type" field
        c. Match against handlers:
           - "progress"/"log"/"partial" → call handler (fire-and-forget)
           - "question"/"approval"      → call handler, write response to stdin
           - "result"                   → return handler result or payload
           - "error"                    → return Err(...)
        d. Unknown type → log warning, continue
    3. Return the final result
```

### Integration with Emit System

The primary pattern is **agent-answers-agent**: the host asks a question, a Concerto-defined
agent answers it. But emit still plays a role for observability and human escalation:

```
External Agent (Claude Code)
    ↓ stdout: { type: "question", question: "Use RS256 or HS256?" }
Concerto Runtime (message loop)
    ↓ calls on "question" handler
Concerto Agent (Architect)
    ↓ LLM API call: "The coder asks: Use RS256 or HS256?"
    ↓ response: "Use RS256 — asymmetric keys are more secure for this use case"
Concerto Runtime
    ↓ stdin: { answer_to: "q1", value: "Use RS256 — ..." }
External Agent (continues working with RS256)
```

For observability, the handler can ALSO emit:

```concerto
on "question" => fn(q) {
    let answer = Architect.execute("Answer: ${q.question}")?;
    emit("supervision:qa", { question: q.question, answer: answer.text });
    answer.text  // sent back to host
},
```

For human escalation (rare, high-risk decisions):

```concerto
on "approval" => fn(req) {
    if req.risk_level == "critical" {
        // Escalate to human via emit
        await emit("human:approval", req)
    } else {
        // Agent handles routine approvals
        let decision = Supervisor.execute("Approve? ${req.description}")?;
        decision.text
    }
},
```

Three layers, with agents as the primary responders:
1. **External agent** (subprocess) ↔ **Concerto VM** (host message protocol)
2. **Concerto VM** ↔ **Concerto agents** (LLM API calls — the primary decision makers)
3. **Concerto VM** ↔ **Host application** via emit (observability, logging, human escalation)

### Handler Registration in VM

```rust
// New: handlers for streaming host execution
struct HostMessageHandler {
    message_type: String,
    handler_function: String,  // name of compiled handler function in IR
    is_bidirectional: bool,    // does the handler return a response?
}

// In execute_streaming:
fn execute_host_streaming(
    &mut self,
    host_name: &str,
    prompt: &str,
    handlers: Vec<HostMessageHandler>,
) -> Result<Value> {
    self.host_registry.write_prompt(host_name, prompt)?;

    loop {
        let msg = self.host_registry.read_message(host_name)?;
        let msg_type = msg.get("type").and_then(|t| t.as_str());

        match msg_type {
            Some("result") => return Ok(json_to_value(msg)),
            Some("error") => return Err(RuntimeError::from(msg)),
            Some(t) => {
                if let Some(handler) = handlers.iter().find(|h| h.message_type == t) {
                    let result = self.call_handler(&handler.handler_function, msg)?;
                    if handler.is_bidirectional {
                        self.host_registry.write_response(host_name, &result)?;
                    }
                }
                // else: unhandled message type, log and continue
            }
            None => continue, // malformed message
        }
    }
}
```


---

## Default Handlers & Fallback Behavior

Not every host interaction needs custom handlers. Sensible defaults:

| Message Type | Default Behavior (no handler registered) |
|-------------|----------------------------------------|
| `progress` | `emit("host:progress", msg)` |
| `question` | `emit("host:question", msg)` — if no emit listener, return `""` |
| `approval` | `emit("host:approval", msg)` — if no emit listener, return `"yes"` |
| `log` | `emit("host:log", msg)` |
| `partial` | Append to accumulator, available as `result.partial_output` |
| `result` | Terminate loop, return payload |
| `error` | Terminate loop, return Err |

With defaults, the simplest possible host call that handles streaming is still one line:

```concerto
// Uses all default handlers — progress emits, questions forwarded via emit, result returned
let result = ClaudeCode.execute("Build me an API")?;
```

But the real power is **agent-supervised execution**:

```concerto
// Supervisor agent answers all questions — fully autonomous, no human needed
let result = listen ClaudeCode.execute("Build me an API") {
    on "question" => fn(q) {
        // Route different question types to different specialist agents
        if q.question.contains("test") {
            let answer = QaLead.execute("Answer: ${q.question}")?;
            answer.text
        } else if q.question.contains("security") {
            let answer = SecurityReviewer.execute("Answer: ${q.question}")?;
            answer.text
        } else {
            let answer = Architect.execute("Answer: ${q.question}")?;
            answer.text
        }
    },
};
```

Or for cases where you DO want a human in the loop for certain decisions, you can mix agent
answers with emit-based human escalation:

```concerto
let result = listen ClaudeCode.execute("Deploy to production") {
    on "question" => fn(q) {
        // Agent handles routine questions
        let answer = Supervisor.execute("Answer: ${q.question}")?;
        answer.text
    },
    on "approval" => fn(req) {
        // But production approvals go to a human via emit
        await emit("human:approval", req)
    },
};
```


---

## Backward Compatibility

### Old-style hosts still work

If a host doesn't speak the message protocol (no `type` field), the runtime falls back to
the current behavior: read one line, return as string. Detection:

1. Read first line from stdout
2. Try to parse as JSON with `type` field
3. If it has `type` → enter message loop
4. If not → treat as single-response (current behavior)

### `execute()` vs `execute_streaming()` vs `listen`

| Method | Behavior | Returns |
|--------|----------|---------|
| `execute()` | Single response (current) | `Result<String>` |
| `execute()` with protocol-aware host | Auto message loop with default handlers | `Result<String>` (from `result` message) |
| `listen ... execute()` | Message loop with custom handlers | `Result<Value>` |

The key decision: should `execute()` auto-detect streaming, or should streaming require an
explicit `listen` block? Auto-detection is more ergonomic but could surprise users. Explicit
`listen` is safer but adds verbosity.

**Recommendation**: Auto-detect at the protocol level. If the host sends message-protocol JSON,
enter the loop with default handlers. If the user wants custom handlers, they use `listen`.
This means `execute()` Just Works for both old and new hosts.


---

## Schema for Host Messages

Define standard message schemas so the compiler can type-check handler parameters:

```concerto
// Built-in (compiler knows these, like Result/Option)
schema HostProgress {
    message: String,
    percent: Option<Int>,
    stage: Option<String>,
}

schema HostQuestion {
    id: String,
    question: String,
    options: Option<Array<String>>,
    context: Option<String>,
}

schema HostApproval {
    id: String,
    description: String,
    risk_level: Option<String>,
}

schema HostResult {
    text: String,
    metadata: Option<Map<String, String>>,
}

schema HostError {
    message: String,
    code: Option<String>,
    recoverable: Option<Bool>,
}
```

With these schemas, handler parameters are typed and the compiler checks field access:

```concerto
listen ClaudeCode.execute("Build an API") {
    on "question" => fn(q: HostQuestion) {
        // q.question, q.options, q.id are all typed — compiler validates access
        let context = "Question: ${q.question}";
        if q.options != nil {
            context = "${context}\nOptions: ${q.options}";
        }
        let answer = Architect.execute(context)?;
        answer.text
    },
    on "approval" => fn(req: HostApproval) {
        // req.risk_level is typed as Option<String>
        let answer = SecurityReviewer.execute(
            "Approve: ${req.description}, risk=${req.risk_level}"
        )?;
        answer.text
    },
};
```


---

## Advanced: Host-Initiated Tool Calls

Some external agents might want to call Concerto-defined tools mid-execution. For example,
Claude Code might need to query a knowledge base or database that only Concerto has access to:

```json
{"type": "tool_call", "id": "tc1", "tool": "lookup_user", "args": {"email": "user@example.com"}}
```

The Concerto runtime dispatches this to a registered Concerto tool, and an agent can enrich
the tool result before sending it back:

```concerto
tool UserDb {
    @describe("Look up a user by email")
    fn lookup_user(self, @param("email") email: String) -> String {
        let user = db.get(email)?;
        std::json::stringify(user)
    }
}

agent DataAnalyst uses openai {
    model: "gpt-4o",
    system_prompt: "You analyze data lookups and provide context.",
}

// Host calls Concerto tools, agent enriches the results
let result = listen ClaudeCode.execute("Find and deactivate inactive users") {
    on "tool_call" => fn(call) {
        // First, execute the actual tool
        let raw_result = dispatch_tool(call.tool, call.args);

        // Then, have an agent add context if needed
        if call.tool == "lookup_user" {
            let enriched = DataAnalyst.execute(
                "Tool returned: ${raw_result}\nAdd a brief note on whether this user looks inactive."
            )?;
            enriched.text
        } else {
            raw_result
        }
    },

    on "question" => fn(q) {
        let answer = Supervisor.execute("Answer: ${q.question}")?;
        answer.text
    },
};
```

This creates a **full duplex** channel: the host can call Concerto tools (enriched by agents),
and Concerto agents answer the host's questions. Each side has capabilities the other needs.


---

## The Core Pattern: Agent-Supervises-Agent

This is the central idea that makes bidirectional hosts powerful in Concerto. The pattern:

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

### Why This Is Powerful

1. **Full autonomy** — No human bottleneck. Agents run 24/7 without waiting for input.

2. **Separation of concerns** — The worker host is good at doing (coding, testing, deploying).
   The supervisor agent is good at deciding (architecture, risk, priorities). Each does what
   it's best at.

3. **Composable supervision** — Different supervisors for different question types. Route
   security questions to a security agent, design questions to an architect, test questions
   to a QA agent.

4. **Auditable** — Memory captures the full Q&A trace between agents. You can inspect every
   decision the supervisor made and why.

5. **Graceful escalation** — For high-risk decisions, the supervisor agent can itself escalate
   to a human via emit. Agents handle the routine, humans handle the exceptional.

### Full Example: Autonomous Code Review Pipeline

```concerto
host ClaudeCode {
    connector: claude_code,
    output_format: "json",
    timeout: 600,
}

agent Architect uses openai {
    model: "gpt-4o",
    system_prompt: "You are a principal engineer. You make architecture and design decisions.
        Be decisive. Prefer simplicity and correctness.",
}

agent SecurityReviewer uses anthropic {
    model: "claude-sonnet-4-5-20250929",
    system_prompt: "You are a security engineer. You evaluate code changes for vulnerabilities.
        Flag anything that touches auth, crypto, or user data.",
}

agent QaLead uses openai {
    model: "gpt-4o",
    system_prompt: "You are a QA lead. You decide testing strategy.
        Always require tests for new features. Allow skipping tests only for docs changes.",
}

memory decision_log: Memory = Memory::new();

schema ReviewResult {
    files_changed: Array<String>,
    tests_passed: Bool,
    summary: String,
}

fn route_question(q: HostQuestion) -> String {
    // Different agents handle different types of questions
    let context = "Question: ${q.question}\nOptions: ${q.options}";

    if q.question.contains("security") || q.question.contains("auth") || q.question.contains("token") {
        let answer = SecurityReviewer
            .with_memory(decision_log)
            .execute(context)?;
        answer.text
    } else if q.question.contains("test") || q.question.contains("coverage") {
        let answer = QaLead
            .with_memory(decision_log)
            .execute(context)?;
        answer.text
    } else {
        let answer = Architect
            .with_memory(decision_log)
            .execute(context)?;
        answer.text
    }
}

fn main() {
    let task = "Implement OAuth2 login with Google provider, including tests";

    let result = listen ClaudeCode.execute(task) {
        on "progress" => fn(msg) {
            emit("progress", msg);
        },

        on "question" => fn(q) {
            // Route to the right specialist agent
            route_question(q)
        },

        on "approval" => fn(req) {
            // High-risk actions: SecurityReviewer decides
            let decision = SecurityReviewer.execute(
                "Approve or reject this action: ${req.description}"
            )?;
            if decision.text.contains("approve") { "yes" } else { "no" }
        },
    };

    // Validate the output
    let validated = ClaudeCode.execute_with_schema<ReviewResult>(
        "Summarize what you just did as JSON"
    )?;

    emit("result", validated);
    emit("decisions", decision_log.messages());  // full agent decision trace
}
```

In this example:
- **Claude Code** does all the actual coding (reads files, writes code, runs tests)
- **Architect** answers design questions ("Should I use passport.js or custom middleware?")
- **SecurityReviewer** answers security questions and approves risky actions
- **QaLead** answers testing questions ("Skip integration tests for now?")
- **decision_log** captures every Q&A exchange for auditing
- **No human is involved** — the whole pipeline is agent-to-agent


---

## Interaction with Pipelines

Host streaming integrates naturally with pipeline stages. Each stage can have a different
supervisor agent — the architect designs, the QA lead oversees testing:

```concerto
agent Architect uses openai {
    model: "gpt-4o",
    system_prompt: "You are a software architect. Make design decisions.",
}

agent QaLead uses anthropic {
    model: "claude-sonnet-4-5-20250929",
    system_prompt: "You are a QA lead. Prioritize test coverage and correctness.",
}

pipeline BuildAndTest(spec: String) {
    // Stage 1: Claude Code implements, Architect supervises design questions
    stage implement = listen ClaudeCode.execute("Implement: ${spec}") {
        on "progress" => fn(msg) {
            emit("pipeline:progress", {
                stage: "implement",
                message: msg.message,
            });
        },
        on "question" => fn(q) {
            let answer = Architect.execute(
                "The coder asks: ${q.question}\nProject spec: ${spec}\nDecide."
            )?;
            answer.text
        },
    };

    // Stage 2: Test host runs tests, QA Lead supervises test decisions
    stage test = listen TestRunner.execute("Test: ${implement}") {
        on "question" => fn(q) {
            // QA Lead decides: "should I skip flaky test X?" -> "No, fix it"
            let answer = QaLead.execute("The test runner asks: ${q.question}")?;
            answer.text
        },
    };
}
```

Each pipeline stage can have its own supervision strategy — different agents, different
policies. The host does the work, the agents make the decisions.


---

## Implementation Considerations

### Threading Model

The message loop is inherently blocking (read from stdout, possibly await emit, write to stdin).
This is fine for the current synchronous runtime. For future async:
- Each `listen` block could be a tokio task
- Host stdout reading is naturally async (tokio::io::BufReader)
- Emit await is already designed to be async (currently sync placeholder)

### Timeout and Deadlock Prevention

Two timeout concerns:
1. **Host never sends `result`/`error`** — the message loop needs a global timeout (from the
   host's `timeout:` config)
2. **Host sends `question`, Concerto's emit handler never responds** — need per-question
   timeout with a default answer or error

```concerto
host ClaudeCode {
    connector: claude_code,
    timeout: 600,               // global timeout for entire execution
    question_timeout: 30,       // timeout for individual questions (new field)
    question_default: "skip",   // default answer if question times out (new field)
}
```

### Buffer Management

For long-running hosts producing many `progress` messages, the runtime should:
- Not accumulate all messages in memory
- Stream `progress` emits immediately
- Only buffer `partial` messages (for accumulating streaming text output)


---

## Open Questions

1. **Is `listen` a keyword or a function?** As a keyword, it needs compiler support (new AST
   node, semantic analysis, codegen). As a method (`.listen(handlers)` or `.execute_streaming()`),
   it's runtime-only but less ergonomic.

2. **Should handlers be closures or named functions?** Closures are more natural for inline
   handlers. Named functions allow reuse across multiple host calls. Both?

3. **What if the host and Concerto disagree on protocol?** If a host sends messages the Concerto
   code doesn't handle, or Concerto expects messages the host doesn't send — how to fail
   gracefully?

4. **Should hosts declare their message types in TOML?** E.g., `message_types = ["progress",
   "question", "result"]`. This would allow compile-time checking that handlers match what
   the host actually sends.

5. **How does this interact with `@retry` on hosts?** If a host execution fails mid-stream
   (after sending progress + questions), retrying means restarting the entire conversation.
   Should the memory/context from the failed attempt be forwarded to the retry?

6. **Partial output accumulation** — Should `partial` messages be automatically concatenated
   into the final result, or should the user explicitly accumulate them?

7. **Can multiple `listen` calls be active concurrently?** In an async context, two hosts
   might both be streaming. The runtime needs to multiplex message loops.
