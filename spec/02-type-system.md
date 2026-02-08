# 02 - Type System

## Overview

Concerto uses a **static type system with local type inference**. Types are checked at compile time to catch errors before execution. The type system balances safety with ergonomics -- most types can be inferred, reducing annotation burden while maintaining compile-time guarantees.

## Primitive Types

### Int

64-bit signed integer. Represents whole numbers.

```concerto
let count: Int = 42;
let negative = -17;        // Inferred as Int
let max = 9_223_372_036_854_775_807;  // i64 max
```

### Float

64-bit IEEE 754 floating point. Represents decimal numbers.

```concerto
let pi: Float = 3.14159;
let temp = -40.0;          // Inferred as Float
let scientific = 1.5e10;
```

### String

UTF-8 encoded string. Immutable by value, supports interpolation.

```concerto
let greeting: String = "Hello, World!";
let name = "Concerto";     // Inferred as String
let interpolated = "Welcome to ${name}!";
```

### Bool

Boolean value. Can be `true` or `false`.

```concerto
let active: Bool = true;
let done = false;           // Inferred as Bool
```

### Nil

Represents the absence of a value. Used as the return type for functions that don't return anything meaningful.

```concerto
let nothing: Nil = nil;

fn log_message(msg: String) -> Nil {
    emit("log", msg);
}

// Nil return type can be omitted:
fn log_message(msg: String) {
    emit("log", msg);
}
```

## Compound Types

### Array\<T\>

Ordered, homogeneous collection. All elements must be the same type.

```concerto
let numbers: Array<Int> = [1, 2, 3, 4, 5];
let names = ["Alice", "Bob", "Charlie"];  // Inferred as Array<String>
let empty: Array<Float> = [];
let nested: Array<Array<Int>> = [[1, 2], [3, 4]];
```

**Operations:**
```concerto
let len = numbers.len();           // 5
let first = numbers[0];            // 1
let slice = numbers[1..3];         // [2, 3]
numbers.push(6);                   // Requires mut
numbers.pop();                     // Removes and returns last
let found = numbers.contains(3);   // true
let mapped = numbers.map(|x| x * 2);  // [2, 4, 6, 8, 10]
let filtered = numbers.filter(|x| x > 3);  // [4, 5]
let sum = numbers.reduce(0, |acc, x| acc + x);  // 15

for (i, item) in numbers.enumerate() {
    // i: Int, item: Int
}
```

### Map\<K, V\>

Key-value collection. Keys must be `String`, `Int`, or `Bool` (hashable types).

```concerto
let config: Map<String, String> = {
    "model": "gpt-4o",
    "provider": "openai",
};

let scores = {
    "Alice": 95,
    "Bob": 87,
};  // Inferred as Map<String, Int>
```

**Operations:**
```concerto
let model = config["model"];            // "gpt-4o" (panics if missing)
let model = config.get("model");        // Option<String>: Some("gpt-4o")
config.set("temperature", "0.7");       // Requires mut
config.delete("provider");              // Requires mut
let has = config.has("model");          // true
let keys = config.keys();              // Array<String>
let values = config.values();          // Array<String>
let size = config.len();               // Number of entries
```

### Tuple\<T1, T2, ...\>

Fixed-size, heterogeneous collection. Elements are accessed by position.

```concerto
let pair: (Int, String) = (42, "hello");
let triple = (true, 3.14, "world");    // Inferred as (Bool, Float, String)

// Access by position
let first = pair.0;    // 42
let second = pair.1;   // "hello"

// Destructuring
let (num, text) = pair;
```

### Option\<T\>

Represents a value that may or may not be present. Replaces null/undefined.

```concerto
let found: Option<String> = Some("hello");
let missing: Option<String> = None;

// Pattern matching (preferred)
match found {
    Some(value) => emit("found", value),
    None => emit("not_found", nil),
}

// Unwrap with default
let value = found ?? "default";   // "hello"
let value = missing ?? "default"; // "default"

// Unwrap (panics if None)
let value = found.unwrap();      // "hello"
let value = missing.unwrap();    // PANIC!

// Map over Option
let upper = found.map(|s| s.to_upper());  // Some("HELLO")
```

### Result\<T, E\>

Represents either a success value or an error. The primary error-handling type.

```concerto
let success: Result<Int, String> = Ok(42);
let failure: Result<Int, String> = Err("something went wrong");

// Pattern matching
match success {
    Ok(value) => emit("result", value),
    Err(e) => emit("error", e),
}

// Error propagation with ?
fn process() -> Result<String, AgentError> {
    let response = agent.execute(prompt)?;  // Returns early if Err
    let parsed = parse_response(response)?;
    Ok(parsed)
}

// Map and chain
let doubled = success.map(|x| x * 2);     // Ok(84)
let chained = success.and_then(|x| {
    if x > 0 { Ok(x) } else { Err("must be positive") }
});
```

## AI-Specific Types

These types are unique to Concerto and provide first-class support for AI orchestration.

### Prompt

A typed prompt string with optional metadata. Used to provide compile-time awareness that a value is intended as an LLM prompt.

```concerto
let simple: Prompt = Prompt::new("What is your name?");

let configured = Prompt::new("Classify this document: ${doc}")
    .with_model("gpt-4o")
    .with_temperature(0.2)
    .with_max_tokens(500);

// String automatically coerces to Prompt where expected:
let response = agent.execute("What is your name?")?;
```

### Response

LLM response containing the raw text, parsed content, and metadata.

```concerto
let response: Response = agent.execute(prompt)?;

let text = response.text;             // Raw response string
let tokens_in = response.tokens_in;   // Input token count
let tokens_out = response.tokens_out;  // Output token count
let model = response.model;           // Model that generated response
let provider = response.provider;     // Provider name
let latency_ms = response.latency_ms; // Response time in milliseconds
```

### Schema\<T\>

Represents an expected output structure. Used with `execute_with_schema` to validate and parse LLM responses.

```concerto
schema Classification {
    label: String,
    confidence: Float,
    reasoning: String,
}

// Usage:
let result: Classification = agent.execute_with_schema<Classification>(prompt)?;
// result is typed as Classification -- runtime validated the response
```

See [12-schema-validation.md](12-schema-validation.md) for full schema specification.

### Message

A chat message with role and content. Used for multi-turn conversations.

```concerto
let msg: Message = Message {
    role: "user",
    content: "Hello, how are you?",
};

let system_msg = Message::system("You are a helpful assistant.");
let user_msg = Message::user("What is 2 + 2?");
let assistant_msg = Message::assistant("2 + 2 equals 4.");
```

### ToolCall

Represents a tool invocation request from an LLM.

```concerto
// Typically received in agent hook methods:
fn on_tool_call(call: ToolCall) -> Result<String, ToolError> {
    let name = call.name;            // Tool function name
    let args = call.arguments;       // Map<String, Any>
    let id = call.id;               // Unique call ID
    // ...
}
```

### AgentRef

Reference to a running agent instance. Used when agents are dynamically instantiated.

```concerto
let agent_ref: AgentRef = Classifier.spawn();
let response = agent_ref.execute(prompt)?;
agent_ref.shutdown();
```

### HashMapRef

Reference to an in-memory hashmap. Used for passing hashmap references to agents.

```concerto
hashmap my_db: HashMap<String, String> = HashMap::new();
let db_ref: HashMapRef = my_db.as_ref();
```

## User-Defined Types

### Struct

Product types with named fields.

```concerto
struct Point {
    x: Float,
    y: Float,
}

struct User {
    pub name: String,
    pub email: String,
    age: Int,           // Private by default
}

// Instantiation
let p = Point { x: 1.0, y: 2.0 };
let user = User { name: "Alice", email: "alice@example.com", age: 30 };

// Access
let name = user.name;

// With methods via impl
impl Point {
    pub fn distance(self, other: Point) -> Float {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn origin() -> Point {
        Point { x: 0.0, y: 0.0 }
    }
}

let d = p.distance(Point::origin());
```

### Enum

Sum types / tagged unions. Each variant can optionally carry data.

```concerto
// Simple enum (no data)
enum Direction {
    North,
    South,
    East,
    West,
}

// Enum with data
enum Shape {
    Circle(Float),                  // radius
    Rectangle(Float, Float),        // width, height
    Triangle { a: Float, b: Float, c: Float },  // named fields
}

// Usage with pattern matching
let shape = Shape::Circle(5.0);
match shape {
    Shape::Circle(r) => r * r * 3.14159,
    Shape::Rectangle(w, h) => w * h,
    Shape::Triangle { a, b, c } => {
        let s = (a + b + c) / 2.0;
        (s * (s - a) * (s - b) * (s - c)).sqrt()
    },
}

// Error types as enums
enum ProcessError {
    InvalidInput(String),
    AgentFailed(AgentError),
    Timeout,
}
```

### Trait

Interfaces / capability contracts. Types implement traits to declare capabilities.

```concerto
trait Describable {
    fn describe(self) -> String;

    // Default implementation
    fn summary(self) -> String {
        let desc = self.describe();
        if desc.len() > 50 {
            "${desc[0..50]}..."
        } else {
            desc
        }
    }
}

impl Describable for User {
    fn describe(self) -> String {
        "${self.name} (${self.email})"
    }
}

// Trait bounds in functions
fn print_description<T: Describable>(item: T) {
    emit("description", item.describe());
}
```

## Type Inference

Concerto uses **local type inference** -- types can be inferred within function bodies but must be explicitly annotated in function signatures and struct/agent definitions.

```concerto
// Inferred:
let x = 5;              // Int
let y = 3.14;           // Float
let name = "hello";     // String
let items = [1, 2, 3];  // Array<Int>
let pair = (1, "two");  // (Int, String)

// Must be annotated:
fn add(a: Int, b: Int) -> Int {  // Parameters and return type required
    a + b
}

struct Config {
    model: String,      // Field types required
    temperature: Float,
}
```

## Generics

Concerto supports basic parametric polymorphism for container types, schemas, and functions.

```concerto
// Generic function
fn first<T>(items: Array<T>) -> Option<T> {
    if items.len() > 0 {
        Some(items[0])
    } else {
        None
    }
}

// Generic with trait bounds
fn process<T: Describable>(item: T) -> String {
    item.describe()
}

// Multiple bounds
fn complex<T: Describable + Serializable>(item: T) -> String {
    // ...
}
```

## Type Coercion

Concerto is strict about types with minimal implicit coercion:

| From | To | Allowed? |
|------|-----|----------|
| `Int` | `Float` | Yes (implicit, widening) |
| `Float` | `Int` | No (explicit `as Int` required -- truncates) |
| `String` | `Prompt` | Yes (implicit, where Prompt is expected) |
| Any | `String` | No (use `to_string()` method) |
| `T` | `Option<T>` | Yes (wraps in `Some`) |

Explicit casting:
```concerto
let f = 42 as Float;     // 42.0
let i = 3.7 as Int;      // 3 (truncated)
```

## Type Aliases

Create shorthand names for complex types:

```concerto
type AgentResult = Result<Response, AgentError>;
type StringMap = Map<String, String>;
type Handler = fn(String) -> AgentResult;
```

## The `Any` Type

A special escape-hatch type that can hold any value. Use sparingly -- it bypasses compile-time type checking.

```concerto
let value: Any = 42;
let value: Any = "hello";
let value: Any = [1, 2, 3];

// Must cast to use:
let num = value as Int;  // Runtime check, panics if wrong type
```

`Any` is primarily used:
- In `Map<String, Any>` for heterogeneous maps (like JSON objects)
- In database operations where value types vary
- At the FFI boundary with host language
