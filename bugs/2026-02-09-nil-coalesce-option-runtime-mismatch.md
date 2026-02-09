# Bug Report: `??` Does Not Unwrap `Option`, Causing Runtime Type Errors

## Status: OPEN (2026-02-09)

## Summary
The language spec says `??` should coalesce both `None` and `nil`, but runtime/codegen currently only treats `nil` as nullish.

As a result, `Some(3) ?? 0` evaluates to `Some(3)` (an `Option`) instead of `3`, while semantic inference treats the expression as unwrapped.

## Severity
High (spec/runtime mismatch + type-safety breach that compiles but fails at runtime).

## Date
2026-02-09

## Affected Components
- `spec/04-operators-and-expressions.md`
- `crates/concerto-compiler/src/semantic/resolver.rs`
- `crates/concerto-compiler/src/codegen/emitter.rs`

## Reproduction
Source:

```concerto
fn main() {
    let threshold = Some(3) ?? 0;
    emit("threshold", threshold);
    emit("comparison", threshold >= 1);
}
```

Commands:

```bash
cargo run -q -p concertoc -- --check /tmp/concerto-audit/bug_nil_coalesce_option.conc
timeout 10s cargo run -q -p concerto -- run /tmp/concerto-audit/bug_nil_coalesce_option.conc
```

Observed output:

- Compiler: `No errors found.`
- Runtime:
  - `[emit:threshold] Some(3)`
  - `runtime error: type error: cannot compare Option >= Int`

## Expected Result
`threshold` should be `3` (unwrapped), and the comparison should execute without type error.

## Root Cause
- Spec explicitly says `??` handles `None` and `nil`.
- Resolver inference assumes Option unwrapping on coalesce (`ExprKind::NilCoalesce`) in `crates/concerto-compiler/src/semantic/resolver.rs`.
- Codegen emits a nil-only check (`left != nil`) in `generate_nil_coalesce()` in `crates/concerto-compiler/src/codegen/emitter.rs`, so `Option(None)` and `Option(Some(_))` are never unwrapped.

## Impact
- Runtime failures in valid-looking code.
- Type checker and runtime semantics diverge.
- Existing examples using `hashmap.get(...) ?? ...` can fail unpredictably.

## Workaround
Avoid `??` for `Option` until fixed; use `match` with explicit `Some(...)` and fallback arm.

## Suggested Fix
Implement coalescing semantics for `Option` in runtime/codegen (or introduce dedicated opcode), then add compiler+runtime regression tests for:
- `Some(v) ?? d` -> `v`
- `None ?? d` -> `d`
- `nil ?? d` -> `d`
