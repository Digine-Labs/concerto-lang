# 19 - Standard Library

## Overview

Concerto's standard library (`std::`) provides commonly needed functionality. All standard library modules are available via `use std::module_name`. Standard library functions are implemented natively in the runtime (Rust), not in Concerto.

## std::json

JSON parsing and serialization.

```concerto
use std::json;

// Parse JSON string to a value
let data: Map<String, Any> = json::parse(json_string)?;
let array: Array<Any> = json::parse("[1, 2, 3]")?;

// Serialize value to JSON string
let json_str: String = json::stringify(data);
let pretty: String = json::stringify_pretty(data, indent: 2);

// Type-safe parsing with schema
let typed: Classification = json::parse_as<Classification>(json_string)?;
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `parse(s)` | `(String) -> Result<Any, JsonError>` | Parse JSON string to value |
| `parse_as<T>(s)` | `(String) -> Result<T, JsonError>` | Parse JSON to typed value |
| `stringify(v)` | `(Any) -> String` | Serialize value to JSON |
| `stringify_pretty(v, indent)` | `(Any, Int) -> String` | Pretty-print JSON |
| `is_valid(s)` | `(String) -> Bool` | Check if string is valid JSON |

## std::http

HTTP client for making web requests.

```concerto
use std::http;

// GET request
let response = http::get("https://api.example.com/data")?;
let body = response.body;
let status = response.status;

// POST request with JSON body
let response = http::post(
    "https://api.example.com/submit",
    body: { "key": "value" },
    headers: { "Authorization": "Bearer ${token}" },
)?;

// General request
let response = http::request(
    method: "PUT",
    url: "https://api.example.com/resource/1",
    body: { "updated": true },
    headers: { "Content-Type": "application/json" },
    timeout_ms: 5000,
)?;
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `get(url, headers?)` | `(String, Map?) -> Result<HttpResponse, HttpError>` | HTTP GET |
| `post(url, body?, headers?)` | `(String, Any?, Map?) -> Result<HttpResponse, HttpError>` | HTTP POST |
| `put(url, body?, headers?)` | `(String, Any?, Map?) -> Result<HttpResponse, HttpError>` | HTTP PUT |
| `delete(url, headers?)` | `(String, Map?) -> Result<HttpResponse, HttpError>` | HTTP DELETE |
| `request(method, url, body?, headers?, timeout_ms?)` | `(...) -> Result<HttpResponse, HttpError>` | General HTTP |

### HttpResponse

```concerto
struct HttpResponse {
    status: Int,                    // HTTP status code
    body: String,                   // Response body
    headers: Map<String, String>,   // Response headers
}
```

## std::fs

File system operations. All paths are relative to the runtime's working directory unless absolute.

```concerto
use std::fs;

let content = fs::read_file("data/input.txt")?;
fs::write_file("data/output.txt", processed_content)?;
let exists = fs::exists("data/input.txt");
let files = fs::list_dir("data/")?;
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `read_file(path)` | `(String) -> Result<String, FsError>` | Read file as UTF-8 string |
| `write_file(path, content)` | `(String, String) -> Result<Nil, FsError>` | Write string to file |
| `append_file(path, content)` | `(String, String) -> Result<Nil, FsError>` | Append to file |
| `exists(path)` | `(String) -> Bool` | Check if path exists |
| `list_dir(path)` | `(String) -> Result<Array<String>, FsError>` | List directory entries |
| `remove_file(path)` | `(String) -> Result<Nil, FsError>` | Delete a file |
| `file_size(path)` | `(String) -> Result<Int, FsError>` | Get file size in bytes |

**Security**: File system access is sandboxed. The host runtime configures allowed directories.

## std::env

Environment variable access.

```concerto
use std::env;

let api_key = env::get("OPENAI_API_KEY")?;
let debug = env::get("DEBUG") ?? "false";
let all_vars = env::all();
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `get(name)` | `(String) -> Option<String>` | Get environment variable |
| `require(name)` | `(String) -> Result<String, EnvError>` | Get or error if missing |
| `all()` | `() -> Map<String, String>` | Get all environment variables |
| `has(name)` | `(String) -> Bool` | Check if variable is set |

## std::fmt

String formatting utilities.

```concerto
use std::fmt;

let formatted = fmt::format("Hello, {}! You have {} messages.", ["Alice", 5]);
let padded = fmt::pad_left("42", 10, '0');   // "0000000042"
let truncated = fmt::truncate("Long text...", 8);  // "Long tex"
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `format(template, args)` | `(String, Array<Any>) -> String` | Positional format |
| `pad_left(s, width, char)` | `(String, Int, String) -> String` | Left-pad string |
| `pad_right(s, width, char)` | `(String, Int, String) -> String` | Right-pad string |
| `truncate(s, max_len)` | `(String, Int) -> String` | Truncate string |
| `indent(s, spaces)` | `(String, Int) -> String` | Indent each line |

## std::collections

Extended collection types beyond Array and Map.

```concerto
use std::collections::{Set, Queue, Stack};

let mut set: Set<String> = Set::new();
set.insert("apple");
set.insert("banana");
set.insert("apple");  // No duplicate
let has = set.contains("apple");  // true
let size = set.len();  // 2

let mut queue: Queue<String> = Queue::new();
queue.enqueue("first");
queue.enqueue("second");
let item = queue.dequeue();  // Some("first")

let mut stack: Stack<Int> = Stack::new();
stack.push(1);
stack.push(2);
let top = stack.pop();  // Some(2)
```

### Set\<T\>

| Function | Signature | Description |
|----------|-----------|-------------|
| `Set::new()` | `() -> Set<T>` | Create empty set |
| `insert(value)` | `(T) -> Bool` | Insert value, returns true if new |
| `remove(value)` | `(T) -> Bool` | Remove value, returns true if existed |
| `contains(value)` | `(T) -> Bool` | Check membership |
| `len()` | `() -> Int` | Number of elements |
| `union(other)` | `(Set<T>) -> Set<T>` | Set union |
| `intersection(other)` | `(Set<T>) -> Set<T>` | Set intersection |
| `difference(other)` | `(Set<T>) -> Set<T>` | Set difference |

### Queue\<T\>

| Function | Signature | Description |
|----------|-----------|-------------|
| `Queue::new()` | `() -> Queue<T>` | Create empty queue |
| `enqueue(value)` | `(T) -> Nil` | Add to back |
| `dequeue()` | `() -> Option<T>` | Remove from front |
| `peek()` | `() -> Option<T>` | View front without removing |
| `len()` | `() -> Int` | Number of elements |
| `is_empty()` | `() -> Bool` | Check if empty |

### Stack\<T\>

| Function | Signature | Description |
|----------|-----------|-------------|
| `Stack::new()` | `() -> Stack<T>` | Create empty stack |
| `push(value)` | `(T) -> Nil` | Push onto top |
| `pop()` | `() -> Option<T>` | Pop from top |
| `peek()` | `() -> Option<T>` | View top without removing |
| `len()` | `() -> Int` | Number of elements |
| `is_empty()` | `() -> Bool` | Check if empty |

## std::time

Time and delay utilities.

```concerto
use std::time;

let timestamp = time::now();              // ISO 8601 string
let epoch_ms = time::now_ms();            // Unix timestamp in milliseconds
time::sleep(1000);                        // Sleep 1000ms
let elapsed = time::measure(|| {          // Measure execution time
    agent.execute(prompt)
});  // Returns duration in ms
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `now()` | `() -> String` | ISO 8601 timestamp |
| `now_ms()` | `() -> Int` | Unix epoch milliseconds |
| `sleep(ms)` | `(Int) -> Nil` | Sleep for milliseconds |
| `measure(fn)` | `(fn() -> T) -> (T, Int)` | Execute and measure time in ms |

## std::math

Mathematical operations.

```concerto
use std::math;

let absolute = math::abs(-42);          // 42
let minimum = math::min(3, 7);          // 3
let maximum = math::max(3, 7);          // 7
let rounded = math::round(3.7);         // 4
let floored = math::floor(3.7);         // 3
let ceiled = math::ceil(3.2);           // 4
let power = math::pow(2, 10);           // 1024
let root = math::sqrt(16.0);           // 4.0
let rand = math::random();             // 0.0..1.0 Float
let rand_int = math::random_int(1, 100); // 1..100 Int
let clamped = math::clamp(150, 0, 100);  // 100
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs(x)` | `(Int\|Float) -> Int\|Float` | Absolute value |
| `min(a, b)` | `(T, T) -> T` | Minimum of two |
| `max(a, b)` | `(T, T) -> T` | Maximum of two |
| `clamp(x, min, max)` | `(T, T, T) -> T` | Clamp to range |
| `round(x)` | `(Float) -> Int` | Round to nearest |
| `floor(x)` | `(Float) -> Int` | Round down |
| `ceil(x)` | `(Float) -> Int` | Round up |
| `pow(base, exp)` | `(Int, Int) -> Int` | Power |
| `sqrt(x)` | `(Float) -> Float` | Square root |
| `random()` | `() -> Float` | Random 0.0..1.0 |
| `random_int(min, max)` | `(Int, Int) -> Int` | Random integer in range |

## std::string

String manipulation utilities (complement methods on String type).

```concerto
use std::string;

let parts = string::split("a,b,c", ",");      // ["a", "b", "c"]
let joined = string::join(["a", "b", "c"], ","); // "a,b,c"
let trimmed = string::trim("  hello  ");        // "hello"
let replaced = string::replace("hello world", "world", "Concerto");
let upper = string::to_upper("hello");          // "HELLO"
let lower = string::to_lower("HELLO");          // "hello"
let contains = string::contains("hello", "ell"); // true
let starts = string::starts_with("hello", "hel"); // true
let ends = string::ends_with("hello", "llo");    // true
let sub = string::substring("hello", 1, 4);      // "ell"
let len = string::len("hello");                   // 5
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `split(s, delimiter)` | `(String, String) -> Array<String>` | Split string |
| `join(parts, separator)` | `(Array<String>, String) -> String` | Join strings |
| `trim(s)` | `(String) -> String` | Trim whitespace |
| `trim_start(s)` | `(String) -> String` | Trim leading whitespace |
| `trim_end(s)` | `(String) -> String` | Trim trailing whitespace |
| `replace(s, from, to)` | `(String, String, String) -> String` | Replace all occurrences |
| `to_upper(s)` | `(String) -> String` | Uppercase |
| `to_lower(s)` | `(String) -> String` | Lowercase |
| `contains(s, sub)` | `(String, String) -> Bool` | Check containment |
| `starts_with(s, prefix)` | `(String, String) -> Bool` | Check prefix |
| `ends_with(s, suffix)` | `(String, String) -> Bool` | Check suffix |
| `substring(s, start, end)` | `(String, Int, Int) -> String` | Extract substring |
| `len(s)` | `(String) -> Int` | String length |
| `repeat(s, n)` | `(String, Int) -> String` | Repeat string |
| `reverse(s)` | `(String) -> String` | Reverse string |
| `parse_int(s)` | `(String) -> Result<Int, ParseError>` | Parse as integer |
| `parse_float(s)` | `(String) -> Result<Float, ParseError>` | Parse as float |

## std::log

Developer-facing logging (distinct from `emit` which is for host integration).

```concerto
use std::log;

log::info("Processing started");
log::warn("Low confidence: ${confidence}");
log::error("Agent call failed: ${error}");
log::debug("Response tokens: ${response.tokens_out}");
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `info(msg)` | `(String) -> Nil` | Info-level log |
| `warn(msg)` | `(String) -> Nil` | Warning-level log |
| `error(msg)` | `(String) -> Nil` | Error-level log |
| `debug(msg)` | `(String) -> Nil` | Debug-level log |

Logs are routed through the runtime's logging system. The host configures log levels and output destinations.

## std::prompt

Prompt template utilities for building complex prompts.

```concerto
use std::prompt;

// Template with variables
let template = prompt::template("""
    You are a ${role}.
    Given the following ${input_type}, perform ${task}.

    ${input_label}: ${input}

    Respond in ${format} format.
    """,
    {
        "role": "document classifier",
        "input_type": "document",
        "task": "classification",
        "input_label": "Document",
        "input": document_text,
        "format": "JSON",
    },
);
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `template(text, vars)` | `(String, Map<String, String>) -> String` | Fill template variables |
| `from_file(path, vars?)` | `(String, Map?) -> Result<String, FsError>` | Load prompt from file |
| `count_tokens(text, model?)` | `(String, String?) -> Int` | Estimate token count |

## std::crypto

Hashing and UUID generation.

```concerto
use std::crypto;

let hash = crypto::sha256("input data");
let id = crypto::uuid();  // "550e8400-e29b-41d4-a716-446655440000"
let hash_md5 = crypto::md5("input data");
```

### Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `sha256(input)` | `(String) -> String` | SHA-256 hash (hex) |
| `md5(input)` | `(String) -> String` | MD5 hash (hex) |
| `uuid()` | `() -> String` | Generate UUID v4 |
| `random_bytes(n)` | `(Int) -> String` | Random bytes (hex) |
