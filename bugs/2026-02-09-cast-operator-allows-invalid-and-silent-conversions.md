# Bug Report: `as` Cast Allows Unsupported Conversions and Silently Fails

## Status: OPEN (2026-02-09)

## Summary
Spec restricts `as` casts to compatible conversions (`Int`/`Float`, `Any -> T` runtime-checked). Current compiler/runtime accept broader casts and silently preserve original value on failed parse.

## Severity
High (type soundness and spec compliance issue).

## Date
2026-02-09

## Affected Components
- `spec/04-operators-and-expressions.md`
- `spec/02-type-system.md`
- `crates/concerto-compiler/src/semantic/resolver.rs`
- `crates/concerto-runtime/src/vm.rs`

## Reproduction A: Invalid cast compiles and yields wrong runtime type

```concerto
fn main() {
    let x = "abc" as Int;
    emit("typeof_x", typeof(x));
    emit("x", x);
}
```

Observed:

- Compiler: `No errors found.`
- Runtime: `typeof_x = String`, `x = abc`

Expected: compile error or runtime cast failure.

## Reproduction B: Unsupported cast path accepted

```concerto
fn main() {
    let s = 42 as String;
    emit("typeof_s", typeof(s));
    emit("s", s);
}
```

Observed: cast succeeds to string.

Expected per spec: string conversion should use method-based APIs, not `as`.

## Root Cause
- Semantic resolver emits no cast compatibility diagnostics.
- Type inference assumes cast target type regardless source compatibility.
- Runtime `exec_cast()` allows extra conversion paths and for parse failures returns original value instead of raising error.

## Impact
- Static type assumptions can diverge from runtime values.
- Invalid casts silently pass through, hiding bugs.

## Workaround
Avoid `as` outside `Int`/`Float` conversions; use explicit parse/format methods.

## Suggested Fix
- Enforce cast compatibility in semantic analysis.
- Make invalid runtime casts fail explicitly (no silent passthrough).
- Add regression tests for disallowed cast pairs and failed `Any -> T` assertions.
