# 24 - Model Memory

## Overview

Concerto models are stateless by default: each `.execute(prompt)` call sends only the system prompt and current user prompt to the LLM. For multi-turn conversations, iterative refinement, and context accumulation, models need **memory** -- a persistent conversation history that is included in subsequent LLM requests.

**Memory** is a first-class language construct declared with the `memory` keyword. It stores an ordered list of chat messages (role + content pairs) and is attached to model executions via the builder pattern.

## Declaration

```concerto
memory conversation: Memory = Memory::new();
memory chat_log: Memory = Memory::new(50);  // sliding window: keep last 50 messages
```

The `memory` keyword declares a named conversation history store. Like `hashmap` and `ledger`, it is a top-level declaration.

### Constructor

- `Memory::new()` -- empty memory with no size limit
- `Memory::new(N)` -- empty memory with sliding window of N messages. When exceeded, oldest messages are dropped.

## Usage with Models

### Builder Pattern

Memory is attached to model execution via `with_memory()`, which returns an intermediate **ModelBuilder** value:

```concerto
// Auto-append mode (default): prompt + response saved to memory after execution
let result = Model.with_memory(conversation).execute(prompt);

// Manual mode: nothing is auto-appended, user controls memory contents
let result = Model.with_memory(conversation, auto: false).execute(prompt);
```

### Execution Semantics

When `Model.with_memory(m).execute(prompt)` runs:

1. Retrieve stored messages from memory `m`
2. Build `ChatRequest.messages` as: `[system_prompt] + [memory_messages] + [user_prompt]`
3. Send to LLM provider
4. If auto-append is enabled (default):
   - Append `{ role: "user", content: prompt }` to memory
   - Append `{ role: "assistant", content: response.text }` to memory
5. Return response

If the memory has a `max` limit and appending would exceed it, the oldest messages are dropped (FIFO).

### With Schema Validation

```concerto
let result = Model.with_memory(conversation).execute_with_schema<OutputType>(prompt);
```

Memory works identically with schema validation. On retry (schema mismatch), the retry prompt replaces the last user message -- memory is not corrupted by failed validation attempts.

### Chaining with Other Builders

```concerto
let result = Model
    .with_memory(conversation)
    .with_tools([Calculator])
    .execute(prompt);
```

Builder methods are composable and order-independent.

## Direct Memory API

Memory also exposes methods for manual manipulation:

```concerto
// Append a message manually
conversation.append("user", "Hello");
conversation.append("assistant", "Hi there!");
conversation.append("system", "You are now in debug mode.");

// Read messages
let msgs = conversation.messages();     // -> Array<Message>
let last5 = conversation.last(5);       // -> Array<Message> (last N)
let count = conversation.len();         // -> Int

// Clear all messages
conversation.clear();
```

### Message Type

Each message is a struct with `role` and `content` fields:

```concerto
// Message is a built-in struct type
// { role: String, content: String }
let msg = conversation.messages()[0];
emit("role", msg.role);        // "user"
emit("content", msg.content);  // "Hello"
```

## Type System

| Type | Description |
|------|-------------|
| `Memory` | Memory type (for type annotations) |
| `MemoryRef` | Runtime reference to a named memory store |
| `Message` | Chat message struct `{ role: String, content: String }` |

## ModelBuilder Value

The `with_memory()` method (and `with_tools()`, `with_context()`) returns a **ModelBuilder** -- a transient value that accumulates execution configuration before the final `.execute()` call.

```concerto
// This is the builder chain:
Model                           // ModelRef
    .with_memory(conversation)  // -> ModelBuilder { memory: "conversation", ... }
    .with_tools([T])            // -> ModelBuilder { memory: "conversation", tools: ["T"], ... }
    .execute(prompt)            // -> Result<Response, String>
```

ModelBuilder methods:
- `with_memory(memory_ref)` -- attach memory (auto-append: true)
- `with_memory(memory_ref, auto: false)` -- attach memory without auto-append
- `with_tools(tool_array)` -- add dynamic tools (see spec/25)
- `without_tools()` -- exclude model's default tools (see spec/25)
- `with_context(value)` -- pass context data for agents (see spec/26)
- `execute(prompt)` -- execute and return `Result<Response, String>`
- `execute_with_schema<T>(prompt)` -- execute with schema validation

## Compilation

### Keyword and AST

The `memory` keyword is added to the lexer. The parser produces a `MemoryDecl` AST node:

```
MemoryDecl {
    name: String,
    type_ann: TypeAnnotation,   // Memory
    initializer: Expr,          // Memory::new() or Memory::new(N)
    span: Span,
}
```

### Semantic Analysis

- `Memory` is registered as a built-in type
- `Memory::new()` is a recognized constructor
- `with_memory()` argument must be a `MemoryRef`
- Memory names are resolved in scope

### IR Generation

```json
{
  "memories": [
    { "name": "conversation", "max_messages": null },
    { "name": "chat_log", "max_messages": 50 }
  ]
}
```

## Runtime

### MemoryStore

The runtime maintains a `MemoryStore` -- a map from memory names to message lists:

```
MemoryStore {
    memories: HashMap<String, Vec<ChatMessage>>
}
```

Methods: `init`, `append`, `get_messages`, `last_n`, `len`, `clear`.

### VM Integration

- `Value::MemoryRef(name)` -- reference to a memory store
- `Value::ModelBuilder { ... }` -- transient builder with accumulated config
- Memory methods dispatched via `exec_call_method` on `MemoryRef`
- Builder methods dispatched via `exec_call_method` on `ModelRef` and `ModelBuilder`
- `build_chat_request` extended to inject memory messages between system prompt and user prompt

## Examples

### Multi-Turn Conversation

```concerto
memory chat: Memory = Memory::new();

model Assistant {
    provider: openai,
    base: "gpt-4o",
    system_prompt: "You are a helpful assistant.",
}

fn main() {
    let r1 = Assistant.with_memory(chat).execute("What is Rust?");
    let r2 = Assistant.with_memory(chat).execute("How does its borrow checker work?");
    // r2's request includes the full conversation history

    emit("history_length", chat.len());  // 4 (2 user + 2 assistant messages)
}
```

### Manual Memory Control

```concerto
memory context: Memory = Memory::new();

// Pre-seed memory with context
context.append("system", "The user is working on a Rust project.");
context.append("user", "I have a Vec<String> that I need to sort.");
context.append("assistant", "You can use .sort() for in-place sorting.");

// Continue conversation with pre-seeded context
let result = Model.with_memory(context).execute("Now how do I deduplicate it?");
```

### Sliding Window

```concerto
memory recent: Memory = Memory::new(20);  // keep last 20 messages

// After many calls, only the last 20 messages are sent to the LLM
for i in 0..100 {
    Model.with_memory(recent).execute("Question ${i}");
}
emit("memory_size", recent.len());  // 20
```
