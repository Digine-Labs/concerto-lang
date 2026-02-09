# 05 - Control Flow

## Overview

Concerto provides familiar control flow constructs -- all of which are **expressions** that return values. This includes `if`/`else`, `match`, loops, and the AI-specific `pipeline`/`stage` construct.

## If / Else

Conditional branching. Always an expression -- the branches must return the same type when used as a value.

```concerto
// Statement form
if temperature > 100.0 {
    emit("warning", "Overheating!");
}

// With else
if score > 90 {
    emit("grade", "A");
} else {
    emit("grade", "B");
}

// Chained
if score > 90 {
    emit("grade", "A");
} else if score > 80 {
    emit("grade", "B");
} else if score > 70 {
    emit("grade", "C");
} else {
    emit("grade", "F");
}

// Expression form (returns a value)
let grade = if score > 90 { "A" }
            else if score > 80 { "B" }
            else if score > 70 { "C" }
            else { "F" };

// Condition must be Bool
if response.is_ok() {  // OK: Bool expression
    process(response.unwrap());
}
```

### Truthiness

Only `Bool` values are accepted in conditions. There is no implicit truthiness:

```concerto
let x = 5;
if x { }          // Compile error: expected Bool, got Int
if x != 0 { }     // OK: explicit comparison
if x > 0 { }      // OK

let s = "hello";
if s { }           // Compile error: expected Bool, got String
if s.len() > 0 { } // OK
```

## Match

Pattern matching with exhaustiveness checking. The compiler ensures all possible values are handled.

### Basic Pattern Matching

```concerto
let direction = Direction::North;

match direction {
    Direction::North => emit("heading", "north"),
    Direction::South => emit("heading", "south"),
    Direction::East => emit("heading", "east"),
    Direction::West => emit("heading", "west"),
}
```

### Value Patterns

```concerto
let status_code = 404;

let message = match status_code {
    200 => "OK",
    201 => "Created",
    400 => "Bad Request",
    404 => "Not Found",
    500 => "Internal Server Error",
    _ => "Unknown",  // Wildcard: catches everything else
};
```

### Destructuring Patterns

```concerto
// Result pattern
match model.execute(prompt) {
    Ok(response) => {
        emit("result", response.text);
    },
    Err(AgentError { message, .. }) => {
        emit("error", message);
    },
}

// Enum variant patterns
match shape {
    Shape::Circle(radius) => radius * radius * PI,
    Shape::Rectangle(w, h) => w * h,
    Shape::Triangle { a, b, c } => heron(a, b, c),
}

// Tuple patterns
match get_result() {
    (true, value) => use_value(value),
    (false, _) => emit("error", "failed"),
}

// Nested patterns
match response {
    Ok(Response { text, tokens_out, .. }) if tokens_out < 100 => {
        emit("short_response", text);
    },
    Ok(Response { text, .. }) => {
        emit("response", text);
    },
    Err(e) => emit("error", e.message),
}
```

### Pattern Guards

```concerto
match value {
    x if x > 100 => "large",
    x if x > 10 => "medium",
    x if x > 0 => "small",
    0 => "zero",
    _ => "negative",
}
```

### Multiple Patterns (OR)

```concerto
match status {
    "active" | "enabled" => true,
    "inactive" | "disabled" => false,
    _ => false,
}
```

### Binding with `@`

```concerto
match count {
    n @ 1..=5 => emit("few", n),
    n @ 6..=20 => emit("several", n),
    n => emit("many", n),
}
```

### Exhaustiveness

The compiler requires that all possible values are covered:

```concerto
enum Status { Active, Inactive, Pending }

match status {
    Status::Active => "active",
    Status::Inactive => "inactive",
    // Compile error: non-exhaustive pattern -- `Status::Pending` not covered
}

// Fix: add missing variants or use wildcard
match status {
    Status::Active => "active",
    _ => "other",
}
```

## For Loop

Iterate over collections, ranges, and iterators.

```concerto
// Range iteration
for i in 0..10 {
    emit("count", i);
}

// Array iteration
let items = ["apple", "banana", "cherry"];
for item in items {
    emit("fruit", item);
}

// With index (enumerate)
for (i, item) in items.enumerate() {
    emit("item", { "index": i, "value": item });
}

// Map iteration
let config = { "model": "gpt-4o", "temp": "0.7" };
for (key, value) in config {
    emit("config", { "key": key, "value": value });
}
```

### For with Destructuring

```concerto
let points = [Point { x: 1.0, y: 2.0 }, Point { x: 3.0, y: 4.0 }];
for Point { x, y } in points {
    emit("point", { "x": x, "y": y });
}
```

## While Loop

Loop while a condition is true.

```concerto
let mut attempts = 0;
let mut success = false;

while attempts < MAX_RETRIES && !success {
    match model.execute(prompt) {
        Ok(response) => {
            success = true;
            emit("result", response.text);
        },
        Err(_) => {
            attempts += 1;
            emit("retry", attempts);
        },
    }
}
```

## Loop (Infinite)

Unconditional loop. Must exit with `break`.

```concerto
let mut counter = 0;

loop {
    let response = model.execute(prompt)?;

    if response.text.contains("DONE") {
        break;
    }

    counter += 1;
    if counter > 100 {
        panic("Too many iterations");
    }
}
```

### Loop as Expression

`break` can carry a value, making `loop` an expression:

```concerto
let result = loop {
    let response = model.execute(prompt)?;
    let parsed = parse_schema<Output>(response);

    match parsed {
        Ok(output) => break output,  // Loop evaluates to this value
        Err(_) => continue,          // Try again
    }
};
```

## Break and Continue

### Break

Exits the innermost loop.

```concerto
for item in items {
    if item == "stop" {
        break;
    }
    process(item);
}
```

### Continue

Skips to the next iteration of the innermost loop.

```concerto
for item in items {
    if item == "skip" {
        continue;
    }
    process(item);
}
```

### Labeled Loops

For nested loops, labels allow breaking or continuing an outer loop:

```concerto
'outer: for row in matrix {
    for cell in row {
        if cell == target {
            emit("found", cell);
            break 'outer;   // Exit both loops
        }
    }
}

'retry: loop {
    for step in steps {
        if step.failed() {
            emit("retry", step.name);
            continue 'retry;  // Restart from outer loop
        }
        step.execute()?;
    }
    break;  // All steps succeeded
}
```

## Return

Explicitly return from the current function.

```concerto
fn classify(text: String) -> Result<String, AgentError> {
    if text.len() == 0 {
        return Err(AgentError::new("Empty input"));
    }

    let response = model.execute(text)?;
    Ok(response.text)   // Implicit return (last expression)
}
```

`return` is optional for the last expression in a function body. These are equivalent:

```concerto
fn add(a: Int, b: Int) -> Int {
    return a + b;   // Explicit return
}

fn add(a: Int, b: Int) -> Int {
    a + b           // Implicit return (no semicolon)
}
```

## Pipeline / Stage

First-class pipeline construct for multi-step model workflows. See [15-concurrency-and-pipelines.md](15-concurrency-and-pipelines.md) for the full specification.

```concerto
pipeline DocumentProcessor {
    stage extract(doc: String) -> String {
        Extractor.execute(doc)?
    }

    stage classify(text: String) -> Classification {
        Classifier.execute_with_schema<Classification>(text)?
    }

    stage route(result: Classification) -> String {
        match result.label {
            "legal" => LegalModel.execute(result)?,
            "technical" => TechModel.execute(result)?,
            _ => DefaultModel.execute(result)?,
        }
    }
}

// Execute the pipeline
let final_result = DocumentProcessor.run(input_document)?;
```

Key properties:
- Stages execute sequentially by default
- Output of one stage flows as input to the next
- Each stage can have its own error handling
- The pipeline returns the output of the last stage
