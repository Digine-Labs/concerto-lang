# 21 - Ledger System

## Overview

AI models can generate hallucinated or imprecise queries when retrieving stored knowledge. The **Ledger** is a fault-tolerant knowledge store designed to handle this reality. Unlike the standard `hashmap` (a typed key-value store for pipeline state), the Ledger uses a document model with **identifiers** (descriptive sentences), **keys** (tags), and **values** (string content), combined with similarity-based querying on identifiers and case-insensitive exact matching on keys.

The Ledger is a **first-class language construct** declared with the `ledger` keyword. It is NOT a tool, MCP, or external service -- it is native to Concerto. However, tools CAN be built on top of a Ledger to let models request knowledge retrieval.

## Data Model

Each Ledger entry consists of three fields:

| Field | Type | Description |
|-------|------|-------------|
| `identifier` | `String` | A descriptive sentence (1-3 sentences). Serves as a human-readable description of the value. Used for similarity-based querying. |
| `keys` | `Array<String>` | An array of tags/labels for the entry. Used for exact (case-insensitive) querying. |
| `value` | `String` | The stored content. Can be data, instructions, file references, or any textual payload. |

### Constraints

| Constraint | Limit | Rationale |
|------------|-------|-----------|
| Identifier max length | 512 characters | Enough for 2-3 descriptive sentences |
| Keys array max size | 32 elements | Reasonable tag count per entry |
| Key max length | 128 characters | Single tag/label |
| Value max length | 65,536 characters (64 KB) | Large enough for data or file path references |

## Declaration

Use the `ledger` keyword at module scope:

```concerto
ledger knowledge: Ledger = Ledger::new();
ledger contracts: Ledger = Ledger::new();
ledger agent_memory: Ledger = Ledger::new();
```

The Ledger type is not generic -- all entries use the fixed `(String, Array<String>, String)` data model. This is intentional: the Ledger is designed for string-based knowledge that models interact with via natural language, not for arbitrary typed data (use `hashmap` for that).

## Insertion

```concerto
knowledge.insert(
    "Uniswap contract addresses on Ethereum mainnet.",
    ["Uniswap", "Contract Address", "Ethereum", "Dex", "Dex contracts"],
    "Pool: 0x12312313, Router: 0x456456456, Factory: 0x789789789",
);

knowledge.insert(
    "AAVE contract addresses on Ethereum mainnet.",
    ["AAVE", "Contract Address", "Ethereum", "Lending", "Money market contracts"],
    "Pool: 0x756745, MoneyMarket: 0x7890789, FlashLoan: 0xabc123",
);
```

### Signature

```
insert(identifier: String, keys: Array<String>, value: String) -> Nil
```

Inserting an entry with an identifier that already exists **replaces** the existing entry (upsert semantics). Identifier comparison for upsert is exact (case-sensitive).

## Querying

All query methods return `Array<LedgerEntry>` where:

```concerto
struct LedgerEntry {
    identifier: String,
    keys: Array<String>,
    value: String,
}
```

An empty array is returned when no entries match.

### Query Builder

Queries use a builder pattern via the `query()` method:

```concerto
let results = knowledge.query().from_identifier("Uniswap");
let results = knowledge.query().from_key("Ethereum");
let results = knowledge.query().from_any_keys(["Dex", "Lending"]);
let results = knowledge.query().from_exact_keys(["Dex", "Lending"]);
```

The compiler lowers `ledger.query().from_X(args)` to a direct method call `ledger.query_X(args)` on the LedgerRef. The `query()` intermediate exists for ergonomic chaining in source code.

### from_identifier(text: String) -> Array\<LedgerEntry\>

Searches identifiers using **word-level containment**. The query text and each identifier are tokenized into words (split on whitespace and punctuation). An entry matches if **every word** in the query text appears in the entry's identifier (case-insensitive).

```concerto
// Given the two entries above:

knowledge.query().from_identifier("Uniswap");
// Returns: [first entry] -- "uniswap" matches word in first identifier

knowledge.query().from_identifier("contract addresses");
// Returns: [first, second] -- both identifiers contain "contract" AND "addresses"

knowledge.query().from_identifier("AAVE Ethereum");
// Returns: [second] -- only second identifier has both "aave" AND "ethereum"

knowledge.query().from_identifier("Solana");
// Returns: [] -- no identifier contains "solana"
```

**Algorithm:**
1. Tokenize the query string into lowercase words (split on whitespace, strip punctuation)
2. For each entry, tokenize the identifier into lowercase words
3. Entry matches if every query word appears in the identifier's word set
4. Return all matching entries

### from_key(key: String) -> Array\<LedgerEntry\>

Searches keys using **exact case-insensitive matching**. An entry matches if any of its keys exactly equals the query key (case-insensitive).

```concerto
knowledge.query().from_key("Ethereum");
// Returns: [first, second] -- both have "Ethereum" key

knowledge.query().from_key("Contract");
// Returns: [] -- no key is exactly "Contract" ("Contract Address" does NOT match)

knowledge.query().from_key("dex");
// Returns: [first] -- "Dex" matches case-insensitively
```

### from_any_keys(keys: Array\<String\>) -> Array\<LedgerEntry\>

An entry matches if it has **at least one** key that matches any of the query keys (OR semantics, case-insensitive).

```concerto
knowledge.query().from_any_keys(["Dex", "Lending"]);
// Returns: [first, second] -- first has "Dex", second has "Lending"

knowledge.query().from_any_keys(["Solana", "Polygon"]);
// Returns: [] -- no entries have either key
```

### from_exact_keys(keys: Array\<String\>) -> Array\<LedgerEntry\>

An entry matches if it has **all** of the query keys (AND semantics, case-insensitive).

```concerto
knowledge.query().from_exact_keys(["Dex", "Lending"]);
// Returns: [] -- no single entry has BOTH "Dex" AND "Lending"

knowledge.query().from_exact_keys(["Ethereum", "Contract Address"]);
// Returns: [first, second] -- both entries have both keys

knowledge.query().from_exact_keys(["AAVE", "Lending"]);
// Returns: [second] -- only second has both
```

## Mutation

### Update by Identifier

```concerto
knowledge.update(
    "Uniswap contract addresses on Ethereum mainnet.",
    value: "Pool: 0xNEWADDR, Router: 0xNEWROUTER",
);
```

Updates the value of the entry with the matching identifier (exact match). Returns `Bool` indicating whether an entry was found and updated.

### Update Keys

```concerto
knowledge.update_keys(
    "Uniswap contract addresses on Ethereum mainnet.",
    ["Uniswap", "Contract Address", "Ethereum", "Dex", "AMM"],
);
```

Replaces the keys of an existing entry. Returns `Bool`.

### Delete

```concerto
knowledge.delete("Uniswap contract addresses on Ethereum mainnet.");
// Deletes the entry with this exact identifier
```

Returns `Bool` indicating whether an entry was found and removed.

## Utility Methods

```concerto
let count = knowledge.len();          // Number of entries
let empty = knowledge.is_empty();     // Bool
knowledge.clear();                    // Remove all entries

let all = knowledge.entries();        // Array<LedgerEntry> -- all entries
let ids = knowledge.identifiers();    // Array<String> -- all identifiers
```

## Scoping

Like databases, Ledgers support scoped views for multi-model isolation:

```concerto
ledger shared_knowledge: Ledger = Ledger::new();

let defi_scope = shared_knowledge.scope("defi");
let nft_scope = shared_knowledge.scope("nft");

// Inserts are namespaced -- defi_scope and nft_scope are isolated
defi_scope.insert(
    "Uniswap pools",
    ["Uniswap", "DEX"],
    "Pool: 0x123...",
);

nft_scope.insert(
    "OpenSea collections",
    ["OpenSea", "NFT"],
    "Top collections: ...",
);

// Queries only see entries in the scope
defi_scope.query().from_key("Uniswap");  // Returns the Uniswap entry
nft_scope.query().from_key("Uniswap");   // Returns [] -- not in this scope
```

## Integration with Models

The Ledger is a Concerto-level construct, not a model tool. However, you can build tools that expose Ledger queries to models:

```concerto
ledger knowledge: Ledger = Ledger::new();

@describe("Search the knowledge base by topic")
@param("query", "A topic or keyword to search for")
tool KnowledgeLookup {
    fn search(query: String) -> String {
        let results = knowledge.query().from_identifier(query);
        if results.is_empty() {
            "No knowledge found for: ${query}"
        } else {
            let mut output = "";
            for entry in results {
                output = output + "--- ${entry.identifier} ---\n${entry.value}\n\n";
            }
            output
        }
    }
}

model ResearchModel {
    provider: openai,
    base: "gpt-4o",
    tools: [KnowledgeLookup],
    system_prompt: "You are a research assistant. Use the KnowledgeLookup tool to find relevant information.",
}
```

In this pattern:
1. The Ledger is populated by Concerto code (not the model)
2. The model can request lookups via the tool
3. The tool bridges the model's natural language to the Ledger's query API
4. The Ledger's fault-tolerant matching handles imprecise model queries

### Assigning Ledger Scopes to Models

```concerto
ledger harness_knowledge: Ledger = Ledger::new();

model Analyst {
    provider: openai,
    base: "gpt-4o",
    knowledge: harness_knowledge.scope("analyst"),
}
```

The `knowledge` field on models binds a scoped Ledger view, allowing models to have isolated knowledge namespaces.

## What Ledger Is Not

- **Not a tool or MCP**: The Ledger is a language construct, not an external service. Models do not call the Ledger directly -- Concerto code does. Tools can be built as bridges.
- **Not a replacement for `hashmap`**: The `hashmap` keyword provides typed key-value storage for pipeline state management. The Ledger provides fault-tolerant knowledge retrieval with similarity matching. Use `hashmap` for exact-key state; use `ledger` for knowledge that models need to query.
- **Not a vector database**: The Ledger uses word-level containment matching, not embedding-based similarity. It is lightweight and runs in-memory without external dependencies.

## Compilation

### Keyword and AST

The `ledger` keyword is recognized by the lexer. The parser produces a `LedgerDecl` AST node:

```
LedgerDecl {
    name: String,
    span: Span,
}
```

### Semantic Analysis

- Ledger names must be unique at module scope
- Validates that `query()` chains use valid terminal methods
- Validates `insert` argument types (String, Array\<String\>, String)

### IR Generation

Ledger declarations produce entries in the `ledgers` IR section:

```json
{
    "ledgers": [
        { "name": "knowledge" }
    ]
}
```

Method calls on LedgerRef values use `CALL_METHOD` with the ledger name resolved at load time.

### Runtime

The VM holds a ledger store:

```
ledgers: HashMap<String, Vec<LedgerEntry>>
```

Where `LedgerEntry` is:

```rust
struct LedgerEntry {
    identifier: String,
    keys: Vec<String>,
    value: String,
}
```

Query methods perform in-memory scanning with the matching algorithms described above.

## Operations Summary

| Operation | Signature | Description |
|-----------|-----------|-------------|
| `insert(id, keys, value)` | `(String, Array<String>, String) -> Nil` | Insert or upsert an entry |
| `query().from_identifier(text)` | `(String) -> Array<LedgerEntry>` | Word-containment search on identifiers |
| `query().from_key(key)` | `(String) -> Array<LedgerEntry>` | Exact case-insensitive key match |
| `query().from_any_keys(keys)` | `(Array<String>) -> Array<LedgerEntry>` | Match entries with any of the keys (OR) |
| `query().from_exact_keys(keys)` | `(Array<String>) -> Array<LedgerEntry>` | Match entries with all keys (AND) |
| `update(id, value:)` | `(String, String) -> Bool` | Update value by identifier |
| `update_keys(id, keys)` | `(String, Array<String>) -> Bool` | Replace keys by identifier |
| `delete(id)` | `(String) -> Bool` | Delete entry by identifier |
| `len()` | `() -> Int` | Number of entries |
| `is_empty()` | `() -> Bool` | Check if empty |
| `clear()` | `() -> Nil` | Remove all entries |
| `entries()` | `() -> Array<LedgerEntry>` | Get all entries |
| `identifiers()` | `() -> Array<String>` | Get all identifiers |
| `scope(prefix)` | `(String) -> Ledger` | Create namespaced view |