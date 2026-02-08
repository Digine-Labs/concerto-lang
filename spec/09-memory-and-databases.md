# 09 - Memory and Databases

## Overview

Concerto provides an in-memory key-value database system for agent coordination and harness state management. Databases act as a **shared ledger** -- agents can read and write data, enabling communication between pipeline stages and maintaining context across interactions.

## Database Declaration

Use the `db` keyword to declare a database at module scope.

```concerto
db my_database: Database<String, String> = Database::new();
db user_store: Database<String, UserProfile> = Database::new();
db counters: Database<String, Int> = Database::new();
```

### Typed Databases

Databases are generic over key and value types:

```concerto
// String keys, String values (most common)
db config: Database<String, String> = Database::new();

// String keys, structured values
db profiles: Database<String, UserProfile> = Database::new();

// String keys, Any values (flexible but less type-safe)
db general: Database<String, Any> = Database::new();

// Int keys for ordered data
db log: Database<Int, LogEntry> = Database::new();
```

## Basic Operations

### Set (Write)

```concerto
db store: Database<String, String> = Database::new();

store.set("user_query", "What is the weather?");
store.set("classification", "weather_inquiry");
store.set("confidence", "0.95");
```

### Get (Read)

```concerto
let query = store.get("user_query");  // Option<String>

match query {
    Some(value) => emit("query", value),
    None => emit("error", "Key not found"),
}

// Or with nil coalescing
let query = store.get("user_query") ?? "no query";
```

### Delete

```concerto
store.delete("temporary_key");
```

### Has (Check existence)

```concerto
if store.has("user_query") {
    let query = store.get("user_query").unwrap();
    process(query);
}
```

### Keys and Values

```concerto
let all_keys = store.keys();       // Array<String>
let all_values = store.values();   // Array<String>
let count = store.len();           // Int
let empty = store.is_empty();      // Bool
```

### Clear

```concerto
store.clear();  // Remove all entries
```

## Querying

### Filter Query

```concerto
db entries: Database<String, Int> = Database::new();
entries.set("score_alice", 95);
entries.set("score_bob", 87);
entries.set("score_charlie", 72);
entries.set("count_total", 3);

// Query by key pattern
let scores = entries.query(|key, value| {
    key.starts_with("score_") && value > 80
});
// Returns: Map<String, Int> = { "score_alice": 95, "score_bob": 87 }
```

### Find First

```concerto
let top_scorer = entries.find(|key, value| value > 90);
// Returns: Option<(String, Int)> = Some(("score_alice", 95))
```

### Iterate

```concerto
for (key, value) in entries {
    emit("entry", { "key": key, "value": value });
}
```

## Scoping

Scopes create namespaced views of a database. This allows multiple agents to share one physical database while operating on isolated key spaces.

```concerto
db shared: Database<String, String> = Database::new();

// Create scoped views
let agent_a_view = shared.scope("agent_a");
let agent_b_view = shared.scope("agent_b");
let global_view = shared.scope("global");

// Writes go to scoped keys internally
agent_a_view.set("result", "classified as legal");
// Internally stores as: "agent_a:result" -> "classified as legal"

agent_b_view.set("result", "summarized successfully");
// Internally stores as: "agent_b:result" -> "summarized successfully"

// Reads are scoped
let a_result = agent_a_view.get("result"); // "classified as legal"
let b_result = agent_b_view.get("result"); // "summarized successfully"

// Global scope can see everything
global_view.set("status", "processing");
let status = global_view.get("status"); // "processing"
```

### Nested Scopes

```concerto
let pipeline_scope = shared.scope("pipeline_1");
let stage_scope = pipeline_scope.scope("stage_extract");
// Keys prefixed with: "pipeline_1:stage_extract:"

stage_scope.set("output", extracted_text);
```

### Assigning Scoped Views to Agents

```concerto
db harness_db: Database<String, Any> = Database::new();

agent Classifier {
    provider: openai,
    model: "gpt-4o",
    memory: harness_db.scope("classifier"),
    // Agent reads/writes to "classifier:*" keys
}

agent Summarizer {
    provider: openai,
    model: "gpt-4o",
    memory: harness_db.scope("summarizer"),
    // Agent reads/writes to "summarizer:*" keys
}

// Both agents share the same physical database but have isolated namespaces
```

## Reactive Events

Subscribe to database changes for event-driven patterns.

```concerto
db state: Database<String, String> = Database::new();

// Watch for changes to specific key
state.on_change("status", |old_value, new_value| {
    emit("status_changed", {
        "from": old_value,
        "to": new_value,
    });
});

// Watch for any change
state.on_any_change(|key, old_value, new_value| {
    emit("db_update", {
        "key": key,
        "old": old_value,
        "new": new_value,
    });
});

state.set("status", "processing");
// Triggers on_change callback
```

## Concurrency Safety

Databases are safe to access from concurrent agent executions. The runtime provides:

1. **Atomic operations**: `set`, `get`, `delete` are atomic
2. **Read-write consistency**: reads always return the latest written value
3. **No deadlocks**: lock-free implementation for simple operations
4. **Batch operations**: `set_many` and `get_many` for atomic multi-key operations

```concerto
// Atomic batch write
store.set_many({
    "step_1_result": result_1,
    "step_2_result": result_2,
    "status": "complete",
});

// Atomic batch read
let values = store.get_many(["step_1_result", "step_2_result"]);
// Returns: Map<String, Option<V>>
```

## Persistence (Optional)

By default, databases are ephemeral (lost when runtime exits). The host can configure persistence:

```concerto
db persistent_store: Database<String, String> = Database::new()
    .with_persistence("./data/store.json");

// Data is auto-saved on changes and loaded on startup
```

This is handled by the runtime -- the host configures which databases persist and where.

## Database as Harness Ledger

The primary use case for databases in Concerto is as a **harness ledger** -- a shared state that tracks the progress and results of an AI orchestration pipeline.

```concerto
db ledger: Database<String, Any> = Database::new();

pipeline InvoiceProcessor {
    stage extract(invoice: String) -> ExtractionResult {
        let result = Extractor.execute_with_schema<ExtractionResult>(invoice)?;

        // Record in ledger
        ledger.set("extraction", result);
        ledger.set("extraction_time", std::time::now());

        result
    }

    stage validate(extraction: ExtractionResult) -> ValidationResult {
        let prev = ledger.get("extraction").unwrap();
        let result = Validator.execute_with_schema<ValidationResult>(
            "Validate: ${prev}"
        )?;

        ledger.set("validation", result);
        ledger.set("validation_time", std::time::now());

        result
    }

    stage route(validation: ValidationResult) -> String {
        // Ledger contains full history for decision making
        let extraction = ledger.get("extraction").unwrap();

        match validation.status {
            "approved" => {
                ledger.set("final_status", "approved");
                ApprovalAgent.execute(extraction)?
            },
            "rejected" => {
                ledger.set("final_status", "rejected");
                RejectionAgent.execute(extraction)?
            },
            _ => {
                ledger.set("final_status", "manual_review");
                emit("manual_review", extraction);
                "Sent to manual review"
            },
        }
    }
}
```

## Operations Summary

| Operation | Signature | Description |
|-----------|-----------|-------------|
| `set(key, value)` | `(K, V) -> Nil` | Store a value |
| `get(key)` | `(K) -> Option<V>` | Retrieve a value |
| `delete(key)` | `(K) -> Bool` | Delete entry, returns true if existed |
| `has(key)` | `(K) -> Bool` | Check key existence |
| `keys()` | `() -> Array<K>` | Get all keys |
| `values()` | `() -> Array<V>` | Get all values |
| `len()` | `() -> Int` | Entry count |
| `is_empty()` | `() -> Bool` | Check if empty |
| `clear()` | `() -> Nil` | Remove all entries |
| `query(predicate)` | `(fn(K, V) -> Bool) -> Map<K, V>` | Filter entries |
| `find(predicate)` | `(fn(K, V) -> Bool) -> Option<(K, V)>` | Find first match |
| `scope(prefix)` | `(String) -> Database<K, V>` | Create namespaced view |
| `set_many(entries)` | `(Map<K, V>) -> Nil` | Atomic batch write |
| `get_many(keys)` | `(Array<K>) -> Map<K, Option<V>>` | Batch read |
| `on_change(key, callback)` | `(K, fn(V, V)) -> Nil` | Watch key changes |
| `on_any_change(callback)` | `(fn(K, V, V)) -> Nil` | Watch all changes |
