# 13 - Error Handling

## Overview

Concerto provides a **dual error-handling model**: functional-style `Result<T, E>` with the `?` propagation operator, and imperative-style `try`/`catch`/`throw`. Both approaches are first-class and interoperable. The functional style is preferred for composable pipelines; the imperative style is available for cases where explicit error catching is clearer.

## Result Type

`Result<T, E>` represents either a success (`Ok(T)`) or a failure (`Err(E)`).

```concerto
let success: Result<Int, String> = Ok(42);
let failure: Result<Int, String> = Err("something went wrong");

// Pattern matching (most explicit)
match success {
    Ok(value) => emit("result", value),
    Err(e) => emit("error", e),
}
```

## Error Propagation (`?`)

The `?` operator unwraps `Ok` or returns early with `Err` from the enclosing function.

```concerto
fn process_document(doc: String) -> Result<Classification, AgentError> {
    let extracted = Extractor.execute(doc)?;            // Returns Err early if fails
    let classified = Classifier.execute(extracted.text)?; // Returns Err early if fails
    let parsed = parse_schema<Classification>(classified)?; // Returns Err early if fails
    Ok(parsed)
}
```

This is equivalent to:

```concerto
fn process_document(doc: String) -> Result<Classification, AgentError> {
    let extracted = match Extractor.execute(doc) {
        Ok(r) => r,
        Err(e) => return Err(e),
    };
    let classified = match Classifier.execute(extracted.text) {
        Ok(r) => r,
        Err(e) => return Err(e),
    };
    let parsed = match parse_schema<Classification>(classified) {
        Ok(r) => r,
        Err(e) => return Err(e),
    };
    Ok(parsed)
}
```

### `?` with Option

The `?` operator also works on `Option<T>`, converting `None` to an error:

```concerto
fn get_config_value(key: String) -> Result<String, ConfigError> {
    let value = config.get(key)?;  // Returns Err if None
    Ok(value)
}
```

## Try / Catch / Throw

Imperative error handling for cases where catching and handling errors at specific points is clearer.

### Basic Try/Catch

```concerto
try {
    let response = agent.execute(prompt)?;
    let parsed = parse_schema<Output>(response)?;
    emit("result", parsed);
} catch {
    emit("error", "An error occurred");
}
```

### Typed Catch Blocks

Catch specific error types:

```concerto
try {
    let response = agent.execute(prompt)?;
    let parsed = parse_schema<Output>(response)?;
    emit("result", parsed);
} catch AgentError(e) {
    emit("error", { "type": "agent", "message": e.message, "model": e.model });
} catch SchemaError(e) {
    emit("error", { "type": "schema", "message": e.message, "raw": e.raw_response });
} catch TimeoutError(e) {
    emit("error", { "type": "timeout", "seconds": e.timeout_seconds });
} catch {
    // Catch-all for any other error
    emit("error", { "type": "unknown" });
}
```

Catch blocks are matched in order. The first matching block handles the error. A bare `catch` at the end catches anything not caught by previous blocks.

### Throw

Explicitly throw an error:

```concerto
fn validate_input(text: String) -> Result<String, ProcessError> {
    if text.len() == 0 {
        throw ProcessError::InvalidInput("Empty input");
    }
    if text.len() > 100000 {
        throw ProcessError::InvalidInput("Input too large");
    }
    Ok(text)
}
```

`throw` is syntactic sugar for returning `Err`:

```concerto
throw SomeError("message");
// is equivalent to:
return Err(SomeError("message"));
```

### Try as Expression

`try` blocks are expressions that return `Result`:

```concerto
let result: Result<Classification, AgentError> = try {
    let response = agent.execute(prompt)?;
    parse_schema<Classification>(response)?
};
```

## Error Type Hierarchy

Concerto provides a built-in error type hierarchy for AI orchestration:

```
Error (base trait)
  |
  +-- AgentError           -- LLM call failures
  |     +-- ModelNotFound
  |     +-- RateLimited
  |     +-- ContentFiltered
  |     +-- InvalidResponse
  |
  +-- SchemaError          -- Output validation failures
  |     +-- ParseError
  |     +-- MissingField
  |     +-- TypeMismatch
  |     +-- EnumViolation
  |     +-- ValidationFailed
  |     +-- MaxRetriesExceeded
  |
  +-- ToolError            -- Tool execution failures
  |     +-- ToolNotFound
  |     +-- PermissionDenied
  |     +-- ExecutionFailed
  |
  +-- ConnectionError      -- Provider connection issues
  |     +-- AuthenticationFailed
  |     +-- NetworkError
  |     +-- ProviderUnavailable
  |
  +-- TimeoutError         -- Operation timeout
  |
  +-- DatabaseError        -- Memory/DB operation failures
  |     +-- KeyNotFound
  |     +-- TypeMismatch
  |
  +-- EmitError            -- Emit system failures
  |     +-- NoListener
  |     +-- Timeout
  |
  +-- RuntimeError         -- General runtime failures
```

### Accessing Error Fields

```concerto
match result {
    Err(AgentError { message, model, provider, .. }) => {
        emit("error", {
            "type": "agent_error",
            "message": message,
            "model": model,
            "provider": provider,
        });
    },
    // ...
}
```

## Custom Error Types

Define domain-specific errors using enums:

```concerto
enum ProcessError {
    InvalidInput(String),
    ClassificationFailed { reason: String, confidence: Float },
    PipelineAborted(String),
    MaxRetriesExceeded { attempts: Int },
}

impl ProcessError {
    pub fn message(self) -> String {
        match self {
            ProcessError::InvalidInput(msg) => "Invalid input: ${msg}",
            ProcessError::ClassificationFailed { reason, .. } => "Classification failed: ${reason}",
            ProcessError::PipelineAborted(msg) => "Pipeline aborted: ${msg}",
            ProcessError::MaxRetriesExceeded { attempts } => "Failed after ${attempts} attempts",
        }
    }
}
```

## Error Conversion (From trait)

Errors automatically convert between compatible types when using `?`:

```concerto
// Automatic: AgentError can convert to ProcessError
impl From<AgentError> for ProcessError {
    fn from(e: AgentError) -> ProcessError {
        ProcessError::ClassificationFailed {
            reason: e.message,
            confidence: 0.0,
        }
    }
}

fn process() -> Result<String, ProcessError> {
    // AgentError automatically converted to ProcessError via From
    let response = agent.execute(prompt)?;
    Ok(response.text)
}
```

## Error Context

Add context to errors for better debugging:

```concerto
fn process_batch(documents: Array<String>) -> Result<Array<Classification>, ProcessError> {
    let mut results = [];
    for (i, doc) in documents.enumerate() {
        let result = classify(doc)
            .context("Failed to process document ${i}")?;
        results.push(result);
    }
    Ok(results)
}

// Error message: "Failed to process document 3: Classification failed: invalid JSON response"
```

## Panic

For unrecoverable errors that should halt execution immediately:

```concerto
fn critical_operation() {
    if !system_check() {
        panic("Critical system check failed -- cannot continue");
    }
    // ... continue only if check passed
}
```

Panic:
- Immediately stops execution
- Emits to the `"panic"` channel with the message
- Is not catchable with `try`/`catch`
- Should be used sparingly -- prefer `Result` for recoverable errors

## Result Methods

```concerto
let result: Result<Int, String> = Ok(42);

// Transform success value
let doubled = result.map(|x| x * 2);              // Ok(84)

// Transform error value
let mapped_err = result.map_err(|e| "Error: ${e}"); // Ok(42) -- no change on Ok

// Chain operations
let chained = result.and_then(|x| {
    if x > 0 { Ok(x) } else { Err("must be positive") }
});

// Unwrap (panics on Err)
let value = result.unwrap();                        // 42

// Unwrap with default
let value = result.unwrap_or(0);                    // 42 (or 0 if Err)
let value = result.unwrap_or_else(|e| {
    emit("error", e);
    0
});

// Check status
let is_ok = result.is_ok();                         // true
let is_err = result.is_err();                       // false
```

## Common Patterns

### Retry with Backoff

```concerto
fn retry_agent_call(
    prompt: String,
    max_attempts: Int,
) -> Result<Response, AgentError> {
    let mut last_error: Option<AgentError> = None;

    for attempt in 1..=max_attempts {
        match agent.execute(prompt) {
            Ok(response) => return Ok(response),
            Err(e) => {
                last_error = Some(e);
                if attempt < max_attempts {
                    let delay = 1000 * (2 ** (attempt - 1));  // Exponential backoff
                    std::time::sleep(delay);
                }
            },
        }
    }

    Err(last_error.unwrap())
}
```

### Fallback Chain

```concerto
fn classify_with_fallback(doc: String) -> Result<Classification, AgentError> {
    // Try primary model
    match PrimaryClassifier.execute_with_schema<Classification>(doc) {
        Ok(result) => Ok(result),
        Err(primary_error) => {
            emit("warning", "Primary classifier failed, trying fallback");
            // Try fallback model
            match FallbackClassifier.execute_with_schema<Classification>(doc) {
                Ok(result) => Ok(result),
                Err(fallback_error) => {
                    Err(AgentError::new(
                        "Both classifiers failed. Primary: ${primary_error.message}, Fallback: ${fallback_error.message}"
                    ))
                },
            }
        },
    }
}
```

### Collect Results

```concerto
fn process_all(items: Array<String>) -> (Array<String>, Array<AgentError>) {
    let mut successes = [];
    let mut errors = [];

    for item in items {
        match agent.execute(item) {
            Ok(response) => successes.push(response.text),
            Err(e) => errors.push(e),
        }
    }

    (successes, errors)
}
```
