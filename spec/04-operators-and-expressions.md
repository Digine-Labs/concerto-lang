# 04 - Operators and Expressions

## Overview

Concerto expressions evaluate to values. Nearly everything in Concerto is an expression -- including `if`/`else`, `match`, and blocks. This specification defines all operators, their semantics, and their precedence.

## Arithmetic Operators

Operate on `Int` and `Float` types. Mixed `Int`/`Float` operations promote `Int` to `Float`.

```concerto
let a = 10 + 3;      // 13 (Int)
let b = 10 - 3;      // 7 (Int)
let c = 10 * 3;      // 30 (Int)
let d = 10 / 3;      // 3 (Int -- integer division)
let e = 10 % 3;      // 1 (Int -- remainder)
let f = 10.0 / 3.0;  // 3.333... (Float)
let g = 10 + 2.5;    // 12.5 (Float -- Int promoted)
let h = -42;          // Negation (Int)
```

### Division Behavior
- `Int / Int` performs integer division (truncates toward zero)
- `Float / Float` performs floating-point division
- Division by zero: runtime panic for `Int`, returns `Inf` or `NaN` for `Float`

## String Operators

```concerto
// Concatenation
let greeting = "Hello" + ", " + "World";  // "Hello, World"

// Interpolation (preferred over concatenation)
let name = "World";
let greeting = "Hello, ${name}!";

// Repetition (method, not operator)
let line = "-".repeat(40);  // "----------------------------------------"
```

## Comparison Operators

Return `Bool`. Work on `Int`, `Float`, `String` (lexicographic), and `Bool`.

```concerto
let eq = (5 == 5);     // true
let neq = (5 != 3);    // true
let lt = (3 < 5);      // true
let gt = (5 > 3);      // true
let lte = (5 <= 5);    // true
let gte = (5 >= 3);    // true

// String comparison (lexicographic)
let before = ("apple" < "banana");  // true
```

## Logical Operators

Operate on `Bool` values. `&&` and `||` use short-circuit evaluation.

```concerto
let and = true && false;    // false (short-circuit: false not evaluated if left is false)
let or = true || false;     // true (short-circuit: true not evaluated if left is true)
let not = !true;            // false

// Common patterns
if response.is_ok() && response.unwrap().text.len() > 0 {
    // Safe because of short-circuit: unwrap() only called if is_ok()
}
```

## Assignment Operators

```concerto
let mut x = 10;
x = 20;       // Assignment
x += 5;       // Add-assign: x = x + 5 -> 25
x -= 3;       // Subtract-assign: x = x - 3 -> 22
x *= 2;       // Multiply-assign: x = x * 2 -> 44
x /= 4;       // Divide-assign: x = x / 4 -> 11
x %= 3;       // Modulo-assign: x = x % 3 -> 2
```

Assignment operators require a `mut` binding on the left side.

## Pipe Operator (`|>`)

The pipe operator passes the result of the left expression as the **first argument** to the right function call. This is one of Concerto's most important operators for readable model pipelines.

```concerto
// Without pipe:
let result = emit("result", parse_schema(model.execute(prompt)?)?);

// With pipe (much more readable):
let result = prompt
    |> model.execute()
    |> parse_schema<Classification>()
    |> emit("result");
```

### Semantics

```concerto
x |> f()          // Equivalent to: f(x)
x |> f(a, b)      // Equivalent to: f(x, a, b)
x |> obj.method() // Equivalent to: obj.method(x)
```

### Chaining

```concerto
// Multi-step model pipeline
let output = document
    |> extract_text()
    |> Classifier.execute_with_schema<Classification>()
    |> enrich_with_metadata()
    |> emit("classification");
```

### With Error Propagation

```concerto
let output = document
    |> extract_text()?        // ? works at each stage
    |> Classifier.execute()?
    |> parse_response()?
    |> emit("result");
```

## Error Propagation Operator (`?`)

The `?` operator unwraps a `Result<T, E>` or `Option<T>`:
- On `Ok(value)` or `Some(value)`: extracts the inner value
- On `Err(e)` or `None`: returns early from the enclosing function with the error

```concerto
fn process() -> Result<String, AgentError> {
    let response = model.execute(prompt)?;   // Returns Err early if execution fails
    let parsed = parse_schema(response)?;    // Returns Err early if parsing fails
    Ok(parsed.label)                         // Return success
}
```

The `?` operator can only be used inside functions that return `Result` or `Option`.

### Error Type Conversion

If the function's error type differs from the expression's error type, automatic conversion is attempted via a `From` trait implementation:

```concerto
fn process() -> Result<String, ProcessError> {
    let response = model.execute(prompt)?;  // AgentError -> ProcessError (via From)
    Ok(response.text)
}
```

## Nil Coalescing Operator (`??`)

Provides a default value when the left side is `None` (for `Option`) or `nil`.

```concerto
let name = config.get("name") ?? "default";
let timeout = settings.timeout ?? 30;

// Chaining
let value = primary.get(key) ?? secondary.get(key) ?? "fallback";
```

## Range Operators

Create range values for iteration and slicing.

```concerto
let exclusive = 0..10;     // 0, 1, 2, ..., 9 (exclusive end)
let inclusive = 0..=10;    // 0, 1, 2, ..., 10 (inclusive end)

// In for loops
for i in 0..5 {
    // i: 0, 1, 2, 3, 4
}

// In array slicing
let items = [10, 20, 30, 40, 50];
let slice = items[1..4];   // [20, 30, 40]
let slice = items[2..];    // [30, 40, 50] (to end)
let slice = items[..3];    // [10, 20, 30] (from start)
```

## Member Access

### Dot (`.`) -- Instance access

```concerto
let name = user.name;           // Field access
let len = items.len();          // Method call
let text = response.text;       // Property access
```

### Double colon (`::`) -- Path/associated access

```concerto
let p = Point::origin();             // Associated function (constructor)
let parsed = std::json::parse(text); // Module path
let circle = Shape::Circle(5.0);     // Enum variant
```

## Index Access

```concerto
let item = array[0];             // Array index (Int)
let value = map["key"];          // Map index (Key type)
let char = string[5];            // String byte index

// Nested
let cell = matrix[row][col];
```

Out-of-bounds array access causes a runtime panic. Use `.get(index)` for safe access returning `Option<T>`.

## Type Casting (`as`)

Explicit type conversion.

```concerto
let f = 42 as Float;       // 42.0
let i = 3.7 as Int;        // 3 (truncates toward zero)
let s = 42.to_string();    // "42" (method, not cast)
```

`as` is only valid between compatible types:
- `Int` <-> `Float`
- `Any` -> specific type (runtime checked, may panic)

## Grouping

Parentheses override precedence:

```concerto
let result = (2 + 3) * 4;  // 20, not 14
```

## Conditional Expression (`if`/`else`)

`if`/`else` is an expression that returns a value:

```concerto
let status = if score > 90 { "excellent" } else { "good" };
let abs_val = if x < 0 { -x } else { x };
```

## Match Expression

`match` is an expression:

```concerto
let label = match category {
    1 => "first",
    2 => "second",
    3 => "third",
    _ => "other",
};
```

## Block Expression

A block evaluates to its last expression:

```concerto
let result = {
    let a = compute();
    let b = transform(a);
    a + b   // No semicolon -> this is the block's value
};
```

## Function Call Expressions

```concerto
let result = process(input);                       // Regular call
let result = model.execute(prompt);                // Method call
let result = Classifier.execute_with_schema<T>(p); // Generic method
let result = std::json::parse(text);              // Qualified call
```

## Operator Precedence Table

From highest to lowest precedence:

| Precedence | Operator | Associativity | Description |
|------------|----------|---------------|-------------|
| 1 (highest) | `()` `[]` `.` `::` | Left | Grouping, index, member access, path |
| 2 | `!` `-` (unary) | Right (prefix) | Logical NOT, negation |
| 3 | `as` | Left | Type casting |
| 4 | `*` `/` `%` | Left | Multiplication, division, modulo |
| 5 | `+` `-` | Left | Addition, subtraction |
| 6 | `..` `..=` | None | Range |
| 7 | `<` `>` `<=` `>=` | Left | Comparison |
| 8 | `==` `!=` | Left | Equality |
| 9 | `&&` | Left | Logical AND |
| 10 | `\|\|` | Left | Logical OR |
| 11 | `??` | Left | Nil coalescing |
| 12 | `\|>` | Left | Pipe |
| 13 | `?` | Postfix | Error propagation |
| 14 | `=` `+=` `-=` `*=` `/=` `%=` | Right | Assignment |
| 15 (lowest) | `=>` | Right | Match arm / lambda |

## Expression vs Statement

In Concerto, statements are expressions followed by `;`. The semicolon discards the expression's value.

```concerto
let x = 5;              // Statement (value discarded)
let y = { 5 };          // Expression block, y = 5
let z = { 5; };         // Expression block with statement, z = Nil (value discarded by ;)
```
