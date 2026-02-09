# Bug Report: Match Error Binding Is Untyped, Allowing Invalid Field Access

## Status: OPEN (2026-02-09)

## Summary
In `match` over `Result<T, E>`, the binding in `Err(e)` is treated as unknown type. Invalid field access like `e.message` compiles even when `E = String`, then fails at runtime.

## Severity
High (type narrowing failure in a common error-handling path).

## Date
2026-02-09

## Affected Components
- `crates/concerto-compiler/src/semantic/resolver.rs`

## Reproduction
Source:

```concerto
fn fail() -> Result<Int, String> {
    Err("boom")
}

fn main() {
    match fail() {
        Ok(v) => emit("ok", v),
        Err(e) => emit("err_message", e.message),
    }
}
```

Commands:

```bash
cargo run -q -p concertoc -- --check /tmp/concerto-audit/bug_match_binding_field.conc
timeout 10s cargo run -q -p concerto -- run /tmp/concerto-audit/bug_match_binding_field.conc
```

Observed output:

- Compiler: `No errors found.`
- Runtime: `runtime error: type error: cannot access field 'message' on String`

## Expected Result
Compiler should reject `e.message` because `e` is `String` in `Err(e)`.

## Root Cause
- Pattern bindings are introduced as `Type::Unknown` in resolver.
- No variant-aware narrowing propagates `Result<T, E>` generic types into arm-local bindings.

## Impact
- Common `match` error handling can compile with invalid field access and fail only at runtime.

## Workaround
Treat `Err(e)` as opaque/error string in user code (`emit(..., e)`) instead of field access.

## Suggested Fix
Implement variant-aware type narrowing for pattern bindings (`Ok(v)`, `Err(e)`, `Some(v)`, `None`) and enforce field access/type checks inside arms.
