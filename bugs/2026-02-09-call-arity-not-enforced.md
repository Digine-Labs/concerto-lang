# Bug Report: Function Call Arity Is Not Enforced (Missing Args Fail Late, Extra Args Ignored)

## Status: OPEN (2026-02-09)

## Summary
Compiler does not validate function call arity against declaration parameters.

- Missing arguments compile and fail at runtime.
- Extra arguments compile and are silently ignored.

## Severity
High (core function contract is unchecked; runtime behavior is misleading).

## Date
2026-02-09

## Affected Components
- `crates/concerto-compiler/src/semantic/resolver.rs`
- `crates/concerto-runtime/src/vm.rs`

## Reproduction A: Missing argument

```concerto
fn add(a: Int, b: Int) -> Int {
    a + b
}

fn main() {
    emit("v", add(1));
}
```

Observed:

- Compiler: `No errors found.`
- Runtime: `name error: undefined variable 'b'`

Expected: compile-time arity mismatch error.

## Reproduction B: Extra argument

```concerto
fn add(a: Int, b: Int) -> Int {
    a + b
}

fn main() {
    emit("v", add(1, 2, 3));
}
```

Observed:

- Compiler: `No errors found.`
- Runtime: `[emit:v] 3` (third arg silently ignored)

Expected: compile-time arity mismatch error.

## Root Cause
- Semantic call handling resolves callee and args but does not validate argument count against function signature.
- Runtime frame argument binding effectively drops extras and leaves missing parameters unbound.

## Impact
- Function signatures are not reliable contracts.
- Bugs surface at runtime or are silently hidden.

## Workaround
Manually audit call sites for parameter count until semantic arity checks are added.

## Suggested Fix
Add compile-time argument-count validation for function and method calls, with optional support for defaults/variadics only where explicitly defined.
