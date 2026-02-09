# Bug Report: `try/catch` Control Flow Is Corrupted for Multi-Catch and Typed Mismatch Cases

## Status: OPEN (2026-02-09)

## Summary
Two severe `try/catch` issues were observed:

1. **Matched catch falls through into later catches** (last catch wins).
2. **Typed catch mismatch with no fallback swallows error and corrupts stack state**.

## Severity
Critical (error handling semantics and stack discipline are broken).

## Date
2026-02-09

## Affected Components
- `crates/concerto-compiler/src/codegen/emitter.rs`
- `crates/concerto-runtime/src/vm.rs`

## Reproduction A: Matched catch falls through

```concerto
fn f() -> Result<String, String> {
    let out = try {
        throw "boom";
        "after"
    } catch String(e) {
        "first"
    } catch {
        "second"
    };
    Ok(out)
}

fn main() {
    emit("f", f());
}
```

Observed output:

- `[emit:f] Ok(second)`

Expected output:

- `Ok(first)`

## Reproduction B: Typed mismatch without fallback swallows error

```concerto
fn f() -> Result<Int, String> {
    let out = try {
        throw "boom";
        1
    } catch Int(e) {
        2
    };
    Ok(out)
}

fn main() {
    emit("f", f());
}
```

Observed output:

- `[emit:boom] nil`

Expected behavior:

- uncaught error propagation (or `Err("boom")` in surrounding Result flow), not silent swallow.

## Root Cause
- `generate_try_catch()` emits catch blocks linearly without jump-to-end after each catch body, so execution can flow into subsequent catches.
- Runtime `Catch` mismatch path (`skip_catch_body`) skips to next `Catch`/`Jump`, but if no matching catch exists, original error value is left on stack and not rethrown, causing stack corruption in subsequent expressions.

## Impact
- Multi-catch semantics are wrong.
- Error mismatch paths can silently produce incorrect values.
- Stack corruption affects unrelated code after catch.

## Workaround
Avoid multiple catches and typed-only catches without a final catch-all until control-flow generation/runtime matching is fixed.

## Suggested Fix
- Codegen: emit jump-to-end after each catch body.
- Runtime: when no catch matches, rethrow original error rather than falling through with stale stack state.
- Add integration tests for both repro classes above.
