# Bug Report: Structural/User Enum Match Patterns Degenerate to Always-True

## Status: OPEN (2026-02-09)

## Summary
Pattern checks for tuple/struct/array patterns and user-defined enum variants are currently stubbed as unconditional `true`, so first matching-arm order is broken.

## Severity
Critical (match semantics are incorrect for major pattern classes).

## Date
2026-02-09

## Affected Components
- `crates/concerto-compiler/src/codegen/emitter.rs`

## Reproduction A: Tuple pattern

```concerto
fn main() {
    let pair = (1, 2);
    let out = match pair {
        (3, 4) => "wrong-first-arm",
        (1, 2) => "correct-arm",
        _ => "fallback",
    };
    emit("out", out);
}
```

Observed: `[emit:out] wrong-first-arm`.

## Reproduction B: User enum pattern

```concerto
enum Mode { A, B }

fn main() {
    let m = Mode::A;
    let out = match m {
        Mode::B => "wrong-first-arm",
        Mode::A => "correct-arm",
        _ => "fallback",
    };
    emit("out", out);
}
```

Observed: `[emit:out] wrong-first-arm`.

## Root Cause
In pattern-check codegen:
- Non-core enum patterns (`Option`/`Result` are special-cased) fall back to unconditional `true`.
- Tuple/struct/array patterns also fall back to unconditional `true`.

`generate_match()` trusts these checks, so the first such arm wins even when structurally incompatible.

## Impact
- `match` control flow is incorrect for common structured patterns.
- Exhaustive logic and branch ordering become unreliable.

## Workaround
Avoid structural and user-enum patterns in correctness-critical paths; prefer explicit primitive guards until fixed.

## Suggested Fix
Implement real structural/variant checks for tuple, struct, array, and user-defined enum patterns in codegen/runtime.
