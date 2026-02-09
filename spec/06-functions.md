# 06 - Functions

## Overview

Functions are the primary unit of code organization in Concerto. They support type-safe parameters, return types, default values, closures, and async execution.

## Function Declaration

```concerto
fn function_name(param1: Type1, param2: Type2) -> ReturnType {
    // body
}
```

### Basic Examples

```concerto
fn greet(name: String) -> String {
    "Hello, ${name}!"
}

fn add(a: Int, b: Int) -> Int {
    a + b
}

// No return value (returns Nil implicitly)
fn log(message: String) {
    emit("log", message);
}
```

### Parameter and Return Type Rules

- All parameters **must** have explicit type annotations
- Return type **must** be specified with `->` if the function returns a value
- Functions that return nothing omit the `-> Type` annotation (implicit `Nil`)
- The last expression without a semicolon is the implicit return value

## Visibility

Functions are private to their module by default. Use `pub` for public access.

```concerto
// Private -- only accessible within this module
fn helper(x: Int) -> Int {
    x * 2
}

// Public -- accessible from other modules
pub fn process(input: String) -> Result<String, ProcessError> {
    let doubled = helper(input.len());
    Ok("Processed ${doubled}")
}
```

## Default Parameters

Parameters can have default values. Parameters with defaults must come after required parameters.

```concerto
fn classify(
    text: String,
    model: String = "gpt-4o",
    temperature: Float = 0.3,
    max_retries: Int = 3,
) -> Result<Classification, AgentError> {
    // ...
}

// Call with defaults
let result = classify("Some document")?;

// Override specific defaults
let result = classify("Some document", temperature: 0.8)?;
```

## Named Arguments

When calling a function, arguments can be passed by name for clarity. Named arguments can appear in any order after positional arguments.

```concerto
fn create_model(
    name: String,
    model: String,
    provider: String,
    temperature: Float = 0.7,
    max_tokens: Int = 1000,
) -> ModelRef {
    // ...
}

// Positional
let m = create_model("Classifier", "gpt-4o", "openai");

// Named (clearer for many parameters)
let m = create_model(
    name: "Classifier",
    model: "gpt-4o",
    provider: "openai",
    temperature: 0.2,
    max_tokens: 2000,
);

// Mixed: positional first, then named
let m = create_model("Classifier", model: "gpt-4o", provider: "openai");
```

## Closures (Anonymous Functions)

Closures are anonymous functions that capture variables from their enclosing scope.

### Short Form

```concerto
let double = |x: Int| x * 2;
let add = |a: Int, b: Int| a + b;
let greet = |name: String| "Hello, ${name}!";
```

### Block Form

```concerto
let process = |text: String| -> Result<String, AgentError> {
    let response = m.execute(text)?;
    let parsed = parse_schema<Output>(response)?;
    Ok(parsed.label)
};
```

### Type Inference in Closures

Closure parameter types can often be inferred from context:

```concerto
let numbers = [1, 2, 3, 4, 5];

// Types inferred from Array<Int>
let doubled = numbers.map(|x| x * 2);        // x inferred as Int
let evens = numbers.filter(|x| x % 2 == 0);  // x inferred as Int
let sum = numbers.reduce(0, |acc, x| acc + x);
```

### Capturing Variables

Closures capture variables from their enclosing scope by reference (immutable) or by value (for owned types).

```concerto
let prefix = "Result";
let formatter = |value: String| "${prefix}: ${value}";

formatter("hello");  // "Result: hello"
```

## Async Functions

Functions that perform asynchronous operations (LLM calls, I/O) use the `async` keyword. They must be `await`ed at call sites.

```concerto
async fn fetch_classification(text: String) -> Result<Classification, AgentError> {
    let response = m.execute(text).await?;
    let parsed = parse_schema<Classification>(response)?;
    Ok(parsed)
}

// Calling async functions
async fn main() {
    let result = fetch_classification("Document text").await?;
    emit("classification", result);
}
```

### Await

The `.await` keyword suspends execution until the async operation completes.

```concerto
// Sequential await
let a = model_a.execute(prompt_a).await?;
let b = model_b.execute(prompt_b).await?;  // Waits for a to finish first

// Parallel await (both run concurrently)
let (a, b) = await (
    model_a.execute(prompt_a),
    model_b.execute(prompt_b),
);
```

See [15-concurrency-and-pipelines.md](15-concurrency-and-pipelines.md) for full async patterns.

## Doc Comments

Triple-slash `///` comments before a function generate documentation.

```concerto
/// Classifies a document into predefined categories.
///
/// Takes a document text and returns a classification with label,
/// confidence score, and reasoning.
///
/// # Arguments
/// - `text` - The document text to classify
/// - `categories` - Optional list of valid categories
///
/// # Returns
/// A `Classification` struct on success, or `AgentError` on failure.
///
/// # Examples
/// ```
/// let result = classify_document("Quarterly earnings report...")?;
/// assert(result.confidence > 0.8);
/// ```
pub fn classify_document(
    text: String,
    categories: Array<String> = [],
) -> Result<Classification, AgentError> {
    // ...
}
```

## Function Types

Functions can be stored in variables and passed as arguments.

```concerto
// Function type syntax: fn(ParamTypes) -> ReturnType
type Processor = fn(String) -> Result<String, AgentError>;
type Predicate = fn(Int) -> Bool;
type Callback = fn(String);

// Passing functions as arguments
fn apply_processor(text: String, processor: fn(String) -> String) -> String {
    processor(text)
}

let result = apply_processor("hello", |s| s.to_upper());

// Storing functions
let operations: Array<fn(Int) -> Int> = [
    |x| x + 1,
    |x| x * 2,
    |x| x * x,
];

for op in operations {
    emit("result", op(5));
}
```

## Recursion

Functions can call themselves. The compiler does not perform tail-call optimization (TCO) in v1.

```concerto
fn factorial(n: Int) -> Int {
    if n <= 1 { 1 }
    else { n * factorial(n - 1) }
}

// Recursive model conversation
fn multi_turn(
    m: ModelRef,
    messages: Array<Message>,
    max_turns: Int,
) -> Result<String, AgentError> {
    if max_turns == 0 {
        return Err(AgentError::new("Max turns exceeded"));
    }

    let response = m.chat(messages)?;

    if response.text.contains("FINAL_ANSWER") {
        Ok(response.text)
    } else {
        let mut updated = messages;
        updated.push(Message::assistant(response.text));
        updated.push(Message::user("Continue."));
        multi_turn(m, updated, max_turns - 1)
    }
}
```

## Method Syntax

Methods are functions defined inside `impl` blocks that take `self` as the first parameter.

```concerto
struct Counter {
    value: Int,
}

impl Counter {
    // Associated function (no self) -- called with ::
    pub fn new() -> Counter {
        Counter { value: 0 }
    }

    // Immutable method
    pub fn get(self) -> Int {
        self.value
    }

    // Mutable method
    pub fn increment(mut self) {
        self.value += 1;
    }

    // Method returning Self
    pub fn with_value(self, value: Int) -> Counter {
        Counter { value: value }
    }
}

// Usage
let mut counter = Counter::new();
counter.increment();
let v = counter.get();  // 1
```
