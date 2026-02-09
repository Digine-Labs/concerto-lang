# Bug Report: Compiler Accepts Signature/Annotation Type Violations

## Status: OPEN (2026-02-09)

## Summary
Multiple core type-safety checks are missing in semantic analysis:
- `let` annotation compatibility is not validated.
- function call argument types are not validated against parameter types.
- function return values are not validated against declared return type.
- assignment type compatibility is not validated after declaration.

## Severity
Critical (language advertises type safety, but core type contracts are unchecked).

## Date
2026-02-09

## Affected Components
- `crates/concerto-compiler/src/semantic/resolver.rs`

## Reproduction
### 1) Function argument mismatch compiles

```concerto
fn add_one(x: Int) -> Int {
    x + 1
}

fn main() {
    let y = add_one("bad");
    emit("cmp", y >= 2);
}
```

### 2) Return type mismatch compiles

```concerto
fn score() -> Int {
    "not-an-int"
}

fn main() {
    let next = score();
    emit("cmp", next >= 1);
}
```

### 3) Assignment mismatch compiles

```concerto
fn main() {
    let mut x: Int = 1;
    x = "oops";
    emit("cmp", x >= 2);
}
```

Compiler output for all three: `No errors found.`

Runtime output for all three: `runtime error: type error: cannot compare String >= Int`.

## Expected Result
All three programs should fail semantic analysis before IR generation.

## Root Cause
- `resolve_let()` stores annotation/inferred type but never checks initializer assignability.
- `ExprKind::Call` handling resolves callee/args but performs no argument-vs-parameter type validation.
- `resolve_return()` does not validate return expression type against `current_function_return`.
- `check_assign_target()` enforces mutability only, not value type compatibility.

## Impact
- Programs with invalid types compile and fail at runtime.
- Type annotations and function signatures are not reliable guarantees.
- Erodes compiler trust and weakens IDE/static tooling value.

## Workaround
Manual defensive checks in program logic; avoid relying on static signature enforcement for now.

## Suggested Fix
Implement explicit assignability checks for:
- let initializers
- call-site arguments
- return statements / tail expressions
- assignments

Add semantic regression tests for each invalid pattern above.
