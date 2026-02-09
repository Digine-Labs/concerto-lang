# Bug Report: Spec-Promised String Indexing and `Array.get(index)` Are Not Implemented

## Status: OPEN (2026-02-09)

## Summary
Spec examples/documentation promise:
- `string[index]` access
- safe array `.get(index)` returning `Option<T>`

Both compile but fail at runtime.

## Severity
Medium-High (core expression ergonomics mismatch between spec and runtime behavior).

## Date
2026-02-09

## Affected Components
- `spec/04-operators-and-expressions.md`
- `crates/concerto-runtime/src/value.rs`
- `crates/concerto-runtime/src/vm.rs`

## Reproduction A: String indexing

```concerto
fn main() {
    let s = "abc";
    emit("mid", s[1]);
}
```

Observed:

- Compiler: `No errors found.`
- Runtime: `type error: cannot index String with Int`

Expected:

- String byte/char index value per spec.

## Reproduction B: `Array.get(index)`

```concerto
fn main() {
    let arr = [10, 20, 30];
    emit("v", arr.get(1));
}
```

Observed:

- Compiler: `No errors found.`
- Runtime: `type error: no method 'get' on Array`

Expected:

- `Some(20)` / `None` behavior as documented.

## Root Cause
- `Value::index_get` has no `String` + `Int` case.
- Array method dispatch supports only `len` and `is_empty`; no `get` method implementation.

## Impact
- Language users following docs hit runtime failures.
- Safe indexing guidance in docs is currently unusable.

## Workaround
Use manual bounds checks + direct array indexing and avoid string index access.

## Suggested Fix
Implement:
- `String` index path in `index_get`
- `Array.get(Int) -> Option<T>` method in array method dispatch
- regression tests matching spec examples.
