# 03 - Variables and Bindings

## Overview

Concerto uses `let` bindings for variables. Bindings are immutable by default; the `mut` keyword enables mutation. Constants are declared with `const` for compile-time known values.

## Immutable Bindings (`let`)

By default, all variable bindings are immutable. Once assigned, the value cannot be changed.

```concerto
let name = "Concerto";
let count = 42;
let items = [1, 2, 3];

name = "Other";   // Compile error: cannot assign to immutable binding `name`
items.push(4);    // Compile error: cannot mutate immutable binding `items`
```

## Mutable Bindings (`let mut`)

Use `mut` to create a binding that can be reassigned or mutated.

```concerto
let mut counter = 0;
counter = counter + 1;  // OK
counter += 1;           // OK

let mut items = [1, 2, 3];
items.push(4);          // OK -- mutating the collection
items = [5, 6, 7];     // OK -- reassigning entirely
```

**Design philosophy**: Immutable by default encourages safer code. Use `mut` only when necessary, signaling intent to the reader.

## Constants (`const`)

Compile-time constants. The value must be computable at compile time (no function calls, no runtime values).

```concerto
const MAX_RETRIES: Int = 3;
const DEFAULT_MODEL: String = "gpt-4o";
const PI: Float = 3.14159265358979;
const ENABLED: Bool = true;
```

Constants:
- Must have explicit type annotations
- Must be initialized with compile-time evaluable expressions
- Are always immutable
- Are conventionally `SCREAMING_SNAKE_CASE`
- Can be used anywhere a literal would be accepted
- Can be defined at module scope or inside functions

## Type Annotations

Type annotations are optional when the type can be inferred. Use the `: Type` syntax after the binding name.

```concerto
// Inferred (preferred when obvious)
let x = 5;
let name = "hello";
let items = [1, 2, 3];

// Explicit (required when ambiguous or for documentation)
let x: Int = 5;
let items: Array<String> = [];  // Empty collections need type annotation
let result: Result<String, AgentError>;  // Declared but not yet assigned (rare)
```

**When annotation is required:**
- Empty collections: `let items: Array<Int> = [];`
- Ambiguous numeric literals (if needed): `let x: Float = 5;`
- Function parameters and return types (always required)
- Struct and model field definitions (always required)

## Destructuring

### Tuple Destructuring

```concerto
let pair = (42, "hello");
let (num, text) = pair;     // num: Int = 42, text: String = "hello"

// Ignore a field with _
let (_, text) = pair;       // Only bind text

// Nested
let nested = ((1, 2), "outer");
let ((a, b), label) = nested;
```

### Struct Destructuring

```concerto
struct Point { x: Float, y: Float }

let p = Point { x: 1.0, y: 2.0 };
let Point { x, y } = p;           // x: Float = 1.0, y: Float = 2.0

// Rename during destructuring
let Point { x: horizontal, y: vertical } = p;

// Partial destructuring (ignore some fields)
let Point { x, .. } = p;          // Only bind x
```

### Array Destructuring

```concerto
let items = [1, 2, 3, 4, 5];
let [first, second, ..rest] = items;  // first: 1, second: 2, rest: [3, 4, 5]
let [head, ..] = items;              // head: 1
```

### In Match Arms

```concerto
match result {
    Ok(Response { text, tokens_out, .. }) => {
        emit("result", text);
        emit("tokens", tokens_out);
    },
    Err(AgentError { message, .. }) => {
        emit("error", message);
    },
}
```

## Shadowing

A new `let` binding in the same scope shadows the previous binding of the same name. This is allowed and useful for transforming values.

```concerto
let input = "  hello  ";
let input = input.trim();        // Shadows previous `input`
let input = input.to_upper();    // Shadows again

// Type can change when shadowing
let count = "42";                // String
let count = count.parse_int()?;  // Int (shadowed with different type)
```

Shadowing differs from mutation:
- Shadowing creates a **new binding** (the old value is not changed)
- Mutation changes the **existing value** in place
- Shadowing works with `let` (immutable); mutation requires `let mut`

## Block Scoping

Variables are scoped to the block `{}` in which they are defined.

```concerto
let x = 1;
{
    let y = 2;
    let x = 10;       // Shadows outer x within this block
    // x is 10, y is 2 here
}
// x is 1 here (outer binding restored)
// y is not accessible here -- compile error

if true {
    let inner = "visible only here";
}
// inner is not accessible here
```

## Block Expressions

Blocks are expressions that evaluate to their last expression (without a semicolon).

```concerto
let result = {
    let a = compute_a();
    let b = compute_b();
    a + b  // No semicolon -- this is the block's value
};

let category = if score > 90 {
    "excellent"
} else if score > 70 {
    "good"
} else {
    "needs improvement"
};
```

## Uninitialized Bindings

Bindings can be declared without initialization, but must be assigned before use.

```concerto
let result: String;

if condition {
    result = "yes";
} else {
    result = "no";
}

// result is now initialized and can be used
emit("result", result);
```

The compiler tracks initialization and will error if a binding is used before being assigned on all code paths.

## Binding Patterns Summary

| Pattern | Syntax | Mutability | When to Use |
|---------|--------|------------|-------------|
| Immutable | `let x = value;` | Immutable | Default -- most variables |
| Mutable | `let mut x = value;` | Mutable | Counters, accumulators, collections you modify |
| Constant | `const X: Type = value;` | Immutable | Compile-time known values, configuration constants |
| Shadowed | `let x = transform(x);` | New binding | Transforming a value step by step |
| Destructured | `let (a, b) = pair;` | Immutable | Unpacking tuples, structs, arrays |
