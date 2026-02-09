# Bug Report: `for` Loop Iterable Type Is Not Validated at Compile Time

## Status: OPEN (2026-02-09)

## Summary
`for` loops accept non-iterable values during compilation. Runtime then fails when generated loop logic calls collection methods (e.g., `len`) on invalid types.

## Severity
Medium-High (late runtime failure for statically-detectable misuse).

## Date
2026-02-09

## Affected Components
- `spec/05-control-flow.md`
- `crates/concerto-compiler/src/semantic/resolver.rs`
- `crates/concerto-compiler/src/codegen/emitter.rs`
- `crates/concerto-runtime/src/vm.rs`

## Reproduction
```concerto
fn main() {
    for n in 42 {
        emit("n", n);
    }
}
```

Observed:

- Compiler: `No errors found.`
- Runtime: `runtime error: type error: no method 'len' on Int`

Expected: compile-time error indicating loop target is not iterable.

## Root Cause
- Semantic resolver visits `for` iterable expression but does not validate iterable type.
- For-loop codegen assumes iterables implement `len` and index semantics.
- Runtime enforces this assumption dynamically and fails for `Int`.

## Impact
- Invalid loops compile successfully and fail only at runtime.

## Workaround
Ensure loop targets are explicit arrays/maps/ranges by manual inspection.

## Suggested Fix
Add compile-time iterable type checks for `for` expressions and emit targeted diagnostics.
