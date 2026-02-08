# 12 - Schema Validation

## Overview

Schema validation is critical for reliable AI pipelines. LLMs return unstructured text, but Concerto programs need structured, typed data. The `schema` construct defines expected output structures. When used with `execute_with_schema`, the runtime validates LLM responses, automatically retries on mismatch, and returns typed values.

## Schema Definition

```concerto
schema SchemaName {
    field_name: Type,
    optional_field?: Type,
    field_with_default: Type = default_value,
}
```

### Basic Example

```concerto
schema Classification {
    label: String,
    confidence: Float,
    reasoning: String,
    categories: Array<String>,
}

// Usage
let result = Classifier.execute_with_schema<Classification>(prompt)?;
// result.label, result.confidence, etc. are all typed and validated
```

## Field Types

### Required Fields

```concerto
schema UserProfile {
    name: String,           // Must be present, must be a string
    age: Int,               // Must be present, must be an integer
    active: Bool,           // Must be present, must be a boolean
}
```

### Optional Fields

Use `?` after the field name. Optional fields may be absent from the response.

```concerto
schema SearchResult {
    title: String,          // Required
    url: String,            // Required
    description?: String,   // Optional -- may be missing
    score?: Float,          // Optional
}

// Access optional fields
match result.description {
    Some(desc) => emit("desc", desc),
    None => emit("desc", "No description"),
}

// Or with nil coalescing
let desc = result.description ?? "No description";
```

### Default Values

Fields with defaults are used when the LLM omits them:

```concerto
schema Config {
    model: String = "gpt-4o",
    temperature: Float = 0.7,
    max_retries: Int = 3,
    format: String = "json",
}
```

## Supported Field Types

| Type | JSON Representation |
|------|-------------------|
| `String` | `"string"` |
| `Int` | `123` (integer number) |
| `Float` | `1.5` (number) |
| `Bool` | `true` / `false` |
| `Array<T>` | `[...]` |
| `Map<String, T>` | `{...}` |
| `Option<T>` | value or null/absent |
| Nested schema | Nested object |

## Nested Schemas

Schemas can reference other schemas:

```concerto
schema Address {
    street: String,
    city: String,
    state: String,
    zip: String,
}

schema Person {
    name: String,
    age: Int,
    address: Address,               // Nested schema
    previous_addresses: Array<Address>,  // Array of schemas
}
```

## Enum Constraints

Constrain string fields to specific values:

```concerto
schema Classification {
    label: "legal" | "technical" | "financial" | "general",
    confidence: Float,
    sub_category?: String,
}
```

The runtime validates that `label` is exactly one of the specified values.

### Numeric Constraints

```concerto
schema Rating {
    score: Int,               // Any integer
    // Future: score: Int(1..=5),  // Integer between 1 and 5
}
```

## Array Constraints

```concerto
schema TaggedDocument {
    title: String,
    tags: Array<String>,        // Any number of string tags
    // Future: top_tags: Array<String, min: 1, max: 5>,  // 1-5 tags
}
```

## Validation Modes

### Strict Mode (Default)

All required fields must be present with correct types. Extra fields are ignored.

```concerto
schema StrictOutput {
    name: String,
    value: Int,
}

// Valid: {"name": "test", "value": 42, "extra": true}  -- extra field ignored
// Invalid: {"name": "test"}  -- missing required field "value"
// Invalid: {"name": "test", "value": "not_a_number"}  -- wrong type
```

### Partial Mode

Allows missing required fields (they become `Option<T>`):

```concerto
schema PartialOutput {
    @partial
    name: String,
    value: Int,
    description: String,
}

// Valid: {"name": "test"}  -- value and description become None
// All fields are now effectively Option<T>
```

### Coerce Mode

Attempts type coercion before failing:

```concerto
schema CoercedOutput {
    @coerce
    count: Int,
    ratio: Float,
    active: Bool,
}

// Valid: {"count": "42", "ratio": "3.14", "active": "true"}
// Coercions: "42" -> 42, "3.14" -> 3.14, "true" -> true
// Invalid: {"count": "not_a_number"}  -- coercion fails
```

## Custom Validators

Add custom validation logic with the `@validate` decorator:

```concerto
schema ValidatedOutput {
    score: Float,
    label: String,
    tags: Array<String>,
}

@validate(fn(output: ValidatedOutput) -> Result<Bool, String> {
    if output.score < 0.0 || output.score > 1.0 {
        Err("Score must be between 0.0 and 1.0")
    } else if output.tags.len() == 0 {
        Err("Must have at least one tag")
    } else {
        Ok(true)
    }
})
schema ValidatedOutput;
```

## Retry on Mismatch

When `execute_with_schema` receives a response that doesn't match the schema, it can automatically retry with error feedback:

```concerto
agent Classifier {
    provider: openai,
    model: "gpt-4o",
    retry_policy: { max_attempts: 3 },
}

schema Output {
    label: String,
    score: Float,
}

// Retry flow:
// 1. Send prompt + schema description to LLM
// 2. Receive response, attempt JSON parse
// 3. If parse fails: re-prompt with "Your response was not valid JSON. Please respond with valid JSON."
// 4. If schema validation fails: re-prompt with specific field errors
// 5. Repeat up to max_attempts
// 6. If all retries fail: return Err(SchemaError)

let result = Classifier.execute_with_schema<Output>(prompt)?;
```

### Custom Retry Messages

```concerto
let result = Classifier.execute_with_schema<Output>(
    prompt,
    retry_message: "Please provide a JSON response with 'label' (string) and 'score' (float between 0 and 1).",
)?;
```

## Provider-Native Structured Output

When the LLM provider supports native structured output (e.g., OpenAI JSON mode, Anthropic tool use), the runtime can leverage it:

```concerto
// The runtime automatically detects provider capabilities:
// - OpenAI: Uses response_format: { type: "json_schema", json_schema: {...} }
// - Anthropic: Uses tool_use with the schema as a tool parameter
// - Others: Includes schema description in the prompt text

let result = Classifier.execute_with_schema<Classification>(prompt)?;
// Runtime picks the best strategy based on the agent's provider
```

## Schema Composition

### Schema Extension (Embedding)

```concerto
schema BaseResult {
    status: String,
    timestamp: String,
}

schema ClassificationResult {
    base: BaseResult,              // Embed base fields
    label: String,
    confidence: Float,
}
```

### Generic Schemas

```concerto
schema ApiResponse<T> {
    success: Bool,
    data: T,
    error?: String,
}

// Usage
let result = agent.execute_with_schema<ApiResponse<Classification>>(prompt)?;
if result.success {
    emit("data", result.data);
} else {
    emit("error", result.error ?? "Unknown error");
}
```

## Schema to JSON Schema Compilation

Internally, the compiler converts Concerto schemas to JSON Schema for LLM communication:

```concerto
schema Classification {
    label: "legal" | "technical" | "financial",
    confidence: Float,
    reasoning: String,
    tags?: Array<String>,
}
```

Compiles to:
```json
{
    "type": "object",
    "properties": {
        "label": {
            "type": "string",
            "enum": ["legal", "technical", "financial"]
        },
        "confidence": {
            "type": "number"
        },
        "reasoning": {
            "type": "string"
        },
        "tags": {
            "type": "array",
            "items": { "type": "string" }
        }
    },
    "required": ["label", "confidence", "reasoning"]
}
```

## Error Types

```concerto
// SchemaError variants
enum SchemaError {
    ParseError(String),          // Response is not valid JSON
    MissingField(String),        // Required field absent
    TypeMismatch {               // Field has wrong type
        field: String,
        expected: String,
        actual: String,
    },
    EnumViolation {              // Value not in allowed set
        field: String,
        allowed: Array<String>,
        actual: String,
    },
    ValidationFailed(String),    // Custom validator failed
    MaxRetriesExceeded {         // All retry attempts failed
        attempts: Int,
        last_error: String,
    },
}
```
