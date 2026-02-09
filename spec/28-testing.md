# 28. Testing

## Overview

Concerto provides a `@test` decorator for writing tests within `.conc` source files. Test functions are automatically skipped during normal execution (`concerto run`) and only run when explicitly requested (`concerto test`). This design ensures zero runtime overhead and seamless integration with the language's existing decorator system (`@retry`, `@timeout`, `@log`).

Testing AI model programs requires deterministic behavior. The `mock` keyword allows replacing model execution with fixed responses, enabling reliable, repeatable tests without requiring API keys or network access.

## Test Functions

A test is a regular function decorated with `@test`:

```concerto
@test
fn description_of_what_is_tested() {
    // test body - same semantics as a function body
}
```

- `@test` functions must not have parameters, `self`, or a return type.
- Test bodies support all statements and expressions valid in function bodies.
- Each test runs in an isolated VM instance (no shared state between tests).
- `@test` functions **cannot be called from non-test code** (compile-time error).
- `@test` functions are emitted to `IrModule.tests`, not `IrModule.functions` — the runtime cannot find or invoke them outside the test runner.

### Optional Description

By default the function name is used as the test description. An explicit description can be provided as a string argument to `@test`:

```concerto
@test("auth :: login validation")
fn auth_login() {
    assert(len("admin") > 0);
}
```

### Examples

```concerto
@test
fn basic_arithmetic() {
    let x = 2 + 3;
    assert_eq(x, 5);
}

@test
fn string_operations() {
    let s = "hello world";
    assert_eq(len(s), 11);
    assert_ne(s, "goodbye");
}

@test
fn error_handling() {
    let result = Ok(42);
    assert(result.is_ok());

    let err = Err("something went wrong");
    assert(err.is_err());
}
```

## Expected Failures

The `@expect_fail` decorator marks a test that is expected to fail. This is useful for documenting known bugs or verifying that certain operations correctly produce errors.

```concerto
@test
@expect_fail
fn panicking_test() {
    panic("this should panic");
}

@test
@expect_fail("assertion failed")
fn specific_failure_message() {
    assert_eq(1, 2);
}
```

- `@expect_fail` without arguments: any error counts as a pass.
- `@expect_fail("message")`: the error message must contain the specified string.
- If an `@expect_fail` test passes (no error), it is reported as **FAIL** ("expected failure but test passed").
- `@expect_fail` can only be used on `@test` functions (compile-time error otherwise).

## Assertion Functions

Assertion functions are global built-ins, available everywhere (not just in test functions).

| Function | Description |
|----------|-------------|
| `assert(condition)` | Fails if `condition` is falsy |
| `assert(condition, message)` | Fails with custom message if `condition` is falsy |
| `assert_eq(left, right)` | Fails if `left != right`, displays both values |
| `assert_ne(left, right)` | Fails if `left == right`, displays both values |

When an assertion fails, it throws an error that stops the current test and reports it as failed.

### Failure Messages

```
assertion failed: expected truthy value, got false
assertion failed: 3 != 5
assertion failed: "hello" == "hello" (expected not equal)
assertion failed: length should be 5   // custom message
```

## Mock Declarations

The `mock` keyword creates a mock override for a model within a test function. When a mocked model's `execute()` or `execute_with_schema()` is called, it returns the configured response instead of making a real LLM API call.

```concerto
@test
fn model_returns_greeting() {
    mock ModelName {
        response: '{"message": "Hello!"}',
    }

    let result = ModelName.execute("Say hello");
    // result is Ok(Response { text: '{"message": "Hello!"}', ... })
}
```

### Mock Fields

| Field | Type | Description |
|-------|------|-------------|
| `response` | String | The text response the model returns |
| `error` | String | Simulate an error (returns Err) |

### Mock with Schema Validation

When `execute_with_schema<T>()` is called on a mocked model, the mock response string is parsed as JSON and converted to the schema type:

```concerto
schema Greeting {
    message: String,
    language: String,
}

model Greeter {
    provider: openai,
    base: "gpt-4o",
    system_prompt: "You are a greeter.",
}

@test
fn structured_greeting() {
    mock Greeter {
        response: '{"message": "Bonjour!", "language": "French"}',
    }

    let result = Greeter.execute_with_schema<Greeting>("Say hello in French");
    match result {
        Ok(greeting) => {
            assert_eq(greeting.message, "Bonjour!");
            assert_eq(greeting.language, "French");
        },
        Err(e) => assert(false, "expected Ok"),
    }
}
```

### Mock Error Simulation

```concerto
@test
fn handles_model_error() {
    mock MyModel {
        error: "API rate limit exceeded",
    }

    let result = MyModel.execute("do something");
    assert(result.is_err());
}
```

### Rules

- `mock` is only valid inside `@test` functions (compile-time error otherwise).
- The mocked name must reference a declared model.
- Multiple models can be mocked in the same test.
- Mocks are scoped to their test — they do not leak between tests.

## Emit Capture

The `test_emits()` built-in returns all emits captured during the current test as an array:

```concerto
@test
fn emit_capture() {
    emit("greeting", "hello");
    emit("farewell", "goodbye");

    let emits = test_emits();
    assert_eq(len(emits), 2);
    assert_eq(emits[0].channel, "greeting");
    assert_eq(emits[0].payload, "hello");
    assert_eq(emits[1].channel, "farewell");
}
```

Each element in the returned array is a struct with `channel` (String) and `payload` (the emitted value) fields.

## Test Groups

Tests can be logically grouped using `::` in the description string:

```concerto
@test("auth :: login succeeds with valid credentials")
fn auth_login() {
    // ...
}

@test("auth :: login fails with bad password")
fn auth_bad_password() {
    // ...
}

@test("auth :: signup creates new user")
fn auth_signup() {
    // ...
}
```

The `--filter` flag matches against the full description, enabling group-level filtering:

```bash
concerto test src/main.conc --filter "auth"
```

## CLI

```bash
concerto test                          # Run all tests in src/main.conc (or entry file)
concerto test src/main.conc            # Run tests in specific file
concerto test --filter "arithmetic"    # Run tests matching pattern
concerto test --quiet                  # Show only summary
concerto test --debug                  # Show error details on failure
```

### Output Format

```
running 5 tests

  PASS  basic_arithmetic
  PASS  string_operations
  FAIL  model_returns_greeting
  PASS  emit_capture
  PASS  panicking_test (expected failure)

test result: FAILED. 4 passed, 1 failed

failures:
  model_returns_greeting -- assertion failed: "Hi!" != "Hello!"
```

## Interaction with `concerto run`

- `concerto run src/main.conc` compiles and executes `fn main()` only. `@test` functions are compiled into the IR tests section but never executed.
- `concerto test src/main.conc` compiles and executes only `@test` functions. `fn main()` is not executed.
- A file with only `@test` functions (no `fn main()`) is valid for `concerto test` but will error on `concerto run`.

## Enforcement

Test function isolation is enforced at two layers:

1. **Compile-time**: The semantic analyzer registers `@test` functions as `SymbolKind::TestFunction`. Calling a `TestFunction` from non-test code produces a compile error: `"cannot call test function 'name' from non-test code"`.
2. **IR-level**: `@test` functions are emitted to `IrModule.tests`, not `IrModule.functions`. The VM cannot find or invoke them during normal execution.

## Testing Patterns

### Testing Tool Functions

Tools contain deterministic logic that can be tested directly:

```concerto
tool Calculator {
    description: "Basic math operations",

    @describe("Add two numbers")
    @param("a", "First number")
    @param("b", "Second number")
    pub fn add(self, a: Int, b: Int) -> Int {
        return a + b;
    }
}

@test
fn calculator_addition() {
    assert_eq(2 + 3, 5);
}
```

### Testing with Emit Verification

```concerto
fn process(data: String) {
    emit("status", "processing");
    emit("result", data);
}

@test
fn process_emits_correct_events() {
    process("hello");
    let emits = test_emits();
    assert_eq(len(emits), 2);
    assert_eq(emits[0].channel, "status");
    assert_eq(emits[1].channel, "result");
    assert_eq(emits[1].payload, "hello");
}
```

### Testing Model Pipelines

```concerto
@test
fn review_pipeline_with_mocks() {
    mock Analyzer {
        response: '{"issues": 0, "summary": "All clear"}',
    }
    mock Reviewer {
        response: '{"approved": true}',
    }

    let analysis = Analyzer.execute("Review this code");
    assert(analysis.is_ok());
}
```
