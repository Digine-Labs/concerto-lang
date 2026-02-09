# Bug Report: Range Values Are Encoded as Arrays, Breaking Iteration and Slicing

## Status: OPEN (2026-02-09)

## Summary
Range syntax compiles, but runtime semantics are incorrect for both `for` iteration and array slicing with range indices.

## Severity
High (core syntax feature behaves incorrectly at runtime).

## Date
2026-02-09

## Affected Components
- `spec/04-operators-and-expressions.md`
- `spec/05-control-flow.md`
- `crates/concerto-compiler/src/codegen/emitter.rs`
- `crates/concerto-runtime/src/value.rs`

## Reproduction A: Range in `for`

```concerto
fn main() {
    let mut total = 0;
    for n in 1..=3 {
        total = total + n;
    }
    emit("total", total);
}
```

Observed:

- Compiler: `No errors found.`
- Runtime: `runtime error: type error: cannot add Int and Bool`

Expected: `total = 6`.

## Reproduction B: Range slicing

```concerto
fn main() {
    let items = [10, 20, 30, 40, 50];
    let slice = items[1..4];
    emit("slice", slice);
}
```

Observed: `runtime error: type error: cannot index Array with Array`.

Expected: `[20, 30, 40]`.

## Root Cause
- Range expression lowering emits a plain 3-element array `[start, end, inclusive]`.
- `for` loop lowering treats iterables generically as collections (`len` + index access), so it iterates those metadata elements rather than numeric range values.
- Runtime indexing has no `Array[Range]` path.

## Impact
- Documented range iteration and slicing semantics are unusable.
- Programs compile but fail/behave incorrectly at runtime.

## Workaround
Use explicit array literals or manual index loops instead of range-based iteration/slicing.

## Suggested Fix
Introduce a dedicated runtime range value representation and implement:
- range iterators for `for`
- slicing support in index operations
- regression tests for `..` and `..=` cases.
