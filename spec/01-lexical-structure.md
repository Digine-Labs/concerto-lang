# 01 - Lexical Structure

## Overview

This specification defines the lexical structure of Concerto -- the rules for how source text is broken into tokens. The lexer (tokenizer) is the first stage of the compiler pipeline.

## Character Set

Concerto source files are UTF-8 encoded. Identifiers support ASCII alphanumeric characters and underscores. String literals support full Unicode.

## Whitespace

Whitespace (spaces, tabs, newlines, carriage returns) is insignificant except as a token separator. Concerto is not indentation-sensitive.

```concerto
// These are equivalent:
let x = 5;
let   x   =   5  ;
```

## Comments

### Line Comments

```concerto
// This is a line comment
let x = 5; // Inline comment
```

### Block Comments

```concerto
/* This is a block comment */

/*
 * Multi-line block comment
 * with conventional formatting
 */

/* Block comments /* can nest */ safely */
```

### Doc Comments

```concerto
/// This is a doc comment for the following item.
/// It supports markdown formatting.
///
/// # Examples
/// ```
/// let x = greet("World");
/// ```
fn greet(name: String) -> String {
    "Hello ${name}!"
}
```

## Keywords

The following identifiers are reserved as keywords and cannot be used as variable or function names:

### Declaration Keywords
| Keyword | Purpose |
|---------|---------|
| `let` | Variable binding (immutable) |
| `mut` | Mutable modifier |
| `const` | Compile-time constant |
| `fn` | Function declaration |
| `agent` | Agent definition |
| `tool` | Tool definition |
| `struct` | Struct type definition |
| `enum` | Enum type definition |
| `trait` | Trait definition |
| `impl` | Implementation block |
| `schema` | Output schema definition |
| `type` | Type alias |

### Visibility Keywords
| Keyword | Purpose |
|---------|---------|
| `pub` | Public visibility modifier |

### Module Keywords
| Keyword | Purpose |
|---------|---------|
| `use` | Import declaration |
| `mod` | Module declaration |

### Control Flow Keywords
| Keyword | Purpose |
|---------|---------|
| `if` | Conditional branch |
| `else` | Alternative branch |
| `match` | Pattern matching |
| `for` | Iterator loop |
| `while` | Conditional loop |
| `loop` | Infinite loop |
| `break` | Exit loop |
| `continue` | Skip to next iteration |
| `return` | Return from function |

### Error Handling Keywords
| Keyword | Purpose |
|---------|---------|
| `try` | Begin try block |
| `catch` | Catch error |
| `throw` | Throw error |

### Async Keywords
| Keyword | Purpose |
|---------|---------|
| `async` | Async function modifier |
| `await` | Await async result |

### AI-Specific Keywords
| Keyword | Purpose |
|---------|---------|
| `emit` | Output to emit channel |
| `pipeline` | Pipeline definition |
| `stage` | Pipeline stage |
| `hashmap` | HashMap declaration |
| `connect` | LLM provider connection |
| `with` | Attach configuration |

### Value Keywords
| Keyword | Purpose |
|---------|---------|
| `true` | Boolean true |
| `false` | Boolean false |
| `nil` | Null/absent value |
| `self` | Current instance reference |

### Operator Keywords
| Keyword | Purpose |
|---------|---------|
| `as` | Type casting |
| `in` | Membership/iteration |

## Identifiers

Identifiers name variables, functions, types, agents, tools, and modules.

```
identifier = [a-zA-Z_][a-zA-Z0-9_]*
```

### Conventions
- Variables and functions: `snake_case` (e.g., `my_variable`, `process_data`)
- Types, agents, tools, schemas: `PascalCase` (e.g., `Classifier`, `FileConnector`, `OutputSchema`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `MAX_RETRIES`, `DEFAULT_MODEL`)
- Modules: `snake_case` (e.g., `agents::classifier`)

## Literals

### Integer Literals

```concerto
let decimal = 42;
let negative = -17;
let with_underscores = 1_000_000;  // Underscores for readability
let hex = 0xFF;
let binary = 0b1010;
let octal = 0o77;
```

All integer literals are `Int` (64-bit signed).

### Float Literals

```concerto
let pi = 3.14159;
let negative = -2.5;
let scientific = 1.5e10;
let small = 2.3e-4;
let with_underscores = 1_000.50;
```

All float literals are `Float` (64-bit IEEE 754).

### String Literals

#### Basic Strings (double-quoted)

```concerto
let greeting = "Hello, World!";
let escaped = "Line 1\nLine 2\tTabbed";
let unicode = "Unicode: \u{1F600}";
```

#### Escape Sequences

| Sequence | Meaning |
|----------|---------|
| `\\` | Backslash |
| `\"` | Double quote |
| `\n` | Newline |
| `\r` | Carriage return |
| `\t` | Tab |
| `\0` | Null character |
| `\u{XXXX}` | Unicode code point |

#### String Interpolation

```concerto
let name = "World";
let greeting = "Hello, ${name}!";                    // Simple variable
let computed = "Result: ${2 + 2}";                   // Expression
let method = "Upper: ${name.to_upper()}";            // Method call
let nested = "Agent says: ${agent.execute(prompt)?}"; // Complex expression
```

Interpolation uses `${}` syntax. Any valid expression can appear inside the braces.

#### Multi-line Strings (triple-quoted)

```concerto
let prompt = """
    You are a document classifier.
    Given the following document, classify it into one of these categories:
    - Legal
    - Technical
    - Financial
    - General

    Document: ${document}
    """;
```

Multi-line strings:
- Begin and end with `"""`
- Leading whitespace is trimmed based on the indentation of the closing `"""`
- Support interpolation with `${}`

#### Raw Strings

```concerto
let regex_pattern = r#"(\d+)\s+(\w+)"#;
let json_template = r#"{"key": "value", "nested": {"a": 1}}"#;
let prompt_with_quotes = r##"Say "hello" to the user"##;
```

Raw strings:
- Begin with `r#"` and end with `"#`
- No escape sequences are processed
- No interpolation
- Additional `#` characters can be used for strings containing `"#`: `r##"..."##`

### Boolean Literals

```concerto
let yes = true;
let no = false;
```

### Nil Literal

```concerto
let nothing = nil;
```

### Array Literals

```concerto
let numbers = [1, 2, 3, 4, 5];
let strings = ["hello", "world"];
let mixed_not_allowed = [1, "two"];  // Compile error: array must be homogeneous
let empty: Array<Int> = [];
let nested = [[1, 2], [3, 4]];
```

### Map Literals

```concerto
let config = {
    "model": "gpt-4o",
    "temperature": 0.7,
    "max_tokens": 1000,
};
let empty: Map<String, Int> = {};
```

### Tuple Literals

```concerto
let pair = (1, "hello");
let triple = (true, 42, "world");
```

## Operators

### Arithmetic Operators
| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Addition / String concatenation | `a + b` |
| `-` | Subtraction / Negation | `a - b`, `-x` |
| `*` | Multiplication | `a * b` |
| `/` | Division | `a / b` |
| `%` | Modulo (remainder) | `a % b` |

### Comparison Operators
| Operator | Description | Example |
|----------|-------------|---------|
| `==` | Equal | `a == b` |
| `!=` | Not equal | `a != b` |
| `<` | Less than | `a < b` |
| `>` | Greater than | `a > b` |
| `<=` | Less than or equal | `a <= b` |
| `>=` | Greater than or equal | `a >= b` |

### Logical Operators
| Operator | Description | Example |
|----------|-------------|---------|
| `&&` | Logical AND (short-circuit) | `a && b` |
| `\|\|` | Logical OR (short-circuit) | `a \|\| b` |
| `!` | Logical NOT | `!a` |

### Assignment Operators
| Operator | Description | Example |
|----------|-------------|---------|
| `=` | Assignment | `x = 5` |
| `+=` | Add-assign | `x += 1` |
| `-=` | Subtract-assign | `x -= 1` |
| `*=` | Multiply-assign | `x *= 2` |
| `/=` | Divide-assign | `x /= 2` |
| `%=` | Modulo-assign | `x %= 3` |

### Special Operators
| Operator | Description | Example |
|----------|-------------|---------|
| `\|>` | Pipe (pass left result as first arg to right) | `x \|> f() \|> g()` |
| `?` | Error propagation (early return on Err) | `result?` |
| `??` | Nil coalescing (default if nil) | `value ?? default` |
| `..` | Range (exclusive end) | `1..10` |
| `..=` | Range (inclusive end) | `1..=10` |
| `->` | Return type annotation | `fn f() -> Int` |
| `=>` | Match arm / lambda body | `Ok(x) => x` |
| `::` | Path separator (module/associated) | `std::json::parse` |
| `.` | Member access | `obj.field` |
| `@` | Decorator | `@retry(max: 3)` |

## Delimiters

| Delimiter | Purpose |
|-----------|---------|
| `(` `)` | Grouping, function parameters, tuple |
| `{` `}` | Blocks, map literals, struct/agent/tool bodies |
| `[` `]` | Array literals, index access |
| `,` | Separator in lists |
| `;` | Statement terminator |
| `:` | Type annotation, map key-value separator |

## Token Precedence (Disambiguation)

When a sequence of characters could be tokenized multiple ways, the lexer applies these rules:

1. **Longest match**: The lexer always takes the longest possible token
2. **Keyword priority**: Keywords take precedence over identifiers (e.g., `let` is always a keyword, never an identifier)
3. **Numeric prefix**: A digit always starts a numeric literal
4. **String prefix**: `r#"` starts a raw string, `"""` starts a multi-line string

## Source Position Tracking

Every token carries source position information:
- **File path**: Which source file the token came from
- **Line number**: 1-based line number
- **Column number**: 1-based column (byte offset within line)
- **Byte offset**: 0-based absolute byte position in source

This information is preserved through the compiler pipeline for error reporting and IR source maps.
