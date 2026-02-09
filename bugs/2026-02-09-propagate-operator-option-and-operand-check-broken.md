# Bug Report: `?` Operator Is Unsound for `Option` and Non-Result Operands

## Status: OPEN (2026-02-09)

## Summary
The compiler allows `?` on expressions that are not `Result`/`Option`, and runtime propagation only handles `Result` (not `Option`). This creates major semantic divergence from spec and type inference.

## Severity
Critical (core error-handling operator is semantically incorrect).

## Date
2026-02-09

## Affected Components
- `spec/04-operators-and-expressions.md`
- `spec/13-error-handling.md`
- `crates/concerto-compiler/src/semantic/resolver.rs`
- `crates/concerto-runtime/src/vm.rs`

## Reproduction A: `?` on `Option` does not unwrap

```concerto
fn bump(v: Option<Int>) -> Option<Int> {
    let n = v?;
    Some(n + 1)
}

fn main() {
    emit("some", bump(Some(1)));
}
```

Observed:

- Compiler: `No errors found.`
- Runtime: `runtime error: type error: cannot add Option and Int`

Expected: `Some(2)`.

## Reproduction B: `?` on `None` in `Option` function does not early-return

```concerto
fn only_if_present(v: Option<Int>) -> Option<Int> {
    v?;
    Some(42)
}

fn main() {
    emit("none_case", only_if_present(None));
}
```

Observed: `[emit:none_case] Some(42)`.

Expected: `None`.

## Reproduction C: `?` on non-Result/non-Option compiles and no-ops

```concerto
fn f() -> Result<Int, String> {
    let x = 5?;
    Ok(x)
}
```

Observed: compiles and returns `Ok(5)`.

Expected: compile error.

## Root Cause
- Semantic pass only checks enclosing function return kind for `?`; it does not validate operand type (`Result`/`Option`).
- Type inference assumes `Option` unwrapping for `ExprKind::Propagate`, so static types drift from runtime behavior.
- Runtime `exec_propagate()` only handles `Value::Result`; `Option` and other values pass through unchanged.

## Impact
- Error propagation control flow is unreliable.
- Compiler and runtime semantics diverge.
- Invalid uses of `?` compile and execute silently.

## Workaround
Avoid `?` on `Option` in current runtime. Use explicit `match`/`return` branches for `Some`/`None`.

## Suggested Fix
- Semantic: require operand type to be `Result` or `Option`.
- Runtime: implement proper `Option` propagation semantics (`Some(v)` unwrap, `None` early-return).
- Add regression tests for all three repro categories.
