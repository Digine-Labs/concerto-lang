# 09 - Memory and HashMaps

## Overview

Concerto provides an in-memory key-value hashmap system for model coordination and harness state management. HashMaps act as a **shared state store** -- models can read and write data, enabling communication between pipeline stages and maintaining context across interactions.

> **Note**: For fault-tolerant knowledge retrieval with similarity-based querying (designed for AI models that may issue imprecise queries), see the [Ledger System](21-ledger.md). HashMaps (`hashmap`) are for exact-key typed state management; Ledgers (`ledger`) are for tagged knowledge with fuzzy matching.

## HashMap Declaration

Use the `hashmap` keyword to declare a hashmap at module scope.

```concerto
hashmap my_database: HashMap<String, String> = HashMap::new();
hashmap user_store: HashMap<String, UserProfile> = HashMap::new();
hashmap counters: HashMap<String, Int> = HashMap::new();
```

### Typed HashMaps

HashMaps are generic over key and value types:

```concerto
// String keys, String values (most common)
hashmap config: HashMap<String, String> = HashMap::new();

// String keys, structured values
hashmap profiles: HashMap<String, UserProfile> = HashMap::new();

// String keys, Any values (flexible but less type-safe)
hashmap general: HashMap<String, Any> = HashMap::new();

// Int keys for ordered data
hashmap log: HashMap<Int, LogEntry> = HashMap::new();
```

## Basic Operations

### Set (Write)

```concerto
hashmap store: HashMap<String, String> = HashMap::new();

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
hashmap entries: HashMap<String, Int> = HashMap::new();
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

Scopes create namespaced views of a hashmap. This allows multiple models to share one physical hashmap while operating on isolated key spaces.

```concerto
hashmap shared: HashMap<String, String> = HashMap::new();

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
hashmap harness_db: HashMap<String, Any> = HashMap::new();

model Classifier {
    provider: openai,
    base: "gpt-4o",
    memory: harness_db.scope("classifier"),
    // Agent reads/writes to "classifier:*" keys
}

model Summarizer {
    provider: openai,
    base: "gpt-4o",
    memory: harness_db.scope("summarizer"),
    // Agent reads/writes to "summarizer:*" keys
}

// Both models share the same physical hashmap but have isolated namespaces
```

## Reactive Events

Subscribe to hashmap changes for event-driven patterns.

```concerto
hashmap state: HashMap<String, String> = HashMap::new();

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

HashMaps are safe to access from concurrent model executions. The runtime provides:

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

By default, hashmaps are ephemeral (lost when runtime exits). The host can configure persistence:

```concerto
hashmap persistent_store: HashMap<String, String> = HashMap::new()
    .with_persistence("./data/store.json");

// Data is auto-saved on changes and loaded on startup
```

This is handled by the runtime -- the host configures which hashmaps persist and where.

## HashMap as Pipeline State Tracker

The primary use case for hashmaps in Concerto is as a **pipeline state tracker** -- a shared store that tracks the progress and results of an AI orchestration pipeline. For knowledge storage with fault-tolerant querying, see the [Ledger System](21-ledger.md).

```concerto
hashmap state_tracker: HashMap<String, Any> = HashMap::new();

pipeline InvoiceProcessor {
    stage extract(invoice: String) -> ExtractionResult {
        let result = Extractor.execute_with_schema<ExtractionResult>(invoice)?;

        // Record in state tracker
        state_tracker.set("extraction", result);
        state_tracker.set("extraction_time", std::time::now());

        result
    }

    stage validate(extraction: ExtractionResult) -> ValidationResult {
        let prev = state_tracker.get("extraction").unwrap();
        let result = Validator.execute_with_schema<ValidationResult>(
            "Validate: ${prev}"
        )?;

        state_tracker.set("validation", result);
        state_tracker.set("validation_time", std::time::now());

        result
    }

    stage route(validation: ValidationResult) -> String {
        // State tracker contains full history for decision making
        let extraction = state_tracker.get("extraction").unwrap();

        match validation.status {
            "approved" => {
                state_tracker.set("final_status", "approved");
                ApprovalAgent.execute(extraction)?
            },
            "rejected" => {
                state_tracker.set("final_status", "rejected");
                RejectionAgent.execute(extraction)?
            },
            _ => {
                state_tracker.set("final_status", "manual_review");
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
| `scope(prefix)` | `(String) -> HashMap<K, V>` | Create namespaced view |
| `set_many(entries)` | `(Map<K, V>) -> Nil` | Atomic batch write |
| `get_many(keys)` | `(Array<K>) -> Map<K, Option<V>>` | Batch read |
| `on_change(key, callback)` | `(K, fn(V, V)) -> Nil` | Watch key changes |
| `on_any_change(callback)` | `(fn(K, V, V)) -> Nil` | Watch all changes |
