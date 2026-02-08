# Bug Report: `len` Builtin Is Not Recognized by Semantic Resolver

## Status: FIXED (2026-02-08)

## Summary
`len(...)` is documented and implemented as a builtin, but the compiler rejects it during semantic analysis with `undefined variable 'len'`.

Additionally, `typeof(...)` and `panic(...)` had the same issue — implemented in the runtime but missing from the semantic resolver's builtin registration.

## Severity
Medium (language feature mismatch between spec/runtime and compiler semantic pass).

## Date
2026-02-08

## Affected Components
- `crates/concerto-compiler/src/semantic/resolver.rs`
- `crates/concerto-runtime/src/builtins.rs`
- Spec/docs consistency (`spec/13-error-handling.md`, `CLAUDE.md` builtins list)

## Reproduction
Minimal source:

```concerto
fn main() {
    let xs = [1, 2, 3];
    if len(xs) > 0 {
        emit("ok", true);
    }
}
```

Compile command:

```bash
cargo run -q -p concertoc -- /tmp/len_repro.conc
```

## Actual Result
Compiler error:

- `undefined variable 'len'`

This occurs at semantic analysis before IR generation.

## Expected Result
`len(...)` should resolve as a builtin function and compile successfully, matching runtime behavior and documentation.

## Evidence
- Runtime has builtin support for `len` in `crates/concerto-runtime/src/builtins.rs`.
- Semantic resolver builtin registration appears to omit `len`, causing name resolution failure.

## Impact
- Any program using `len` cannot compile.
- Breaks alignment between language docs/spec and practical compiler behavior.
- Forces non-idiomatic workarounds in examples and user programs.

## Workaround
Avoid `len(...)` in current code and use alternative logic (for example context-empty checks or other available methods) until semantic resolver registration is fixed.

## Suggested Fix
Register `len` in compiler semantic builtin symbols (same layer that registers `emit`, `print`, `env`, `Ok`, `Err`, etc.) so resolver and runtime builtins are consistent.

## Resolution
**Fixed** by adding `len`, `typeof`, and `panic` to `register_builtins()` in `crates/concerto-compiler/src/semantic/resolver.rs`.

### Changes Made
1. **`resolver.rs`**: Added 3 builtin registrations with correct type signatures:
   - `len(Any) -> Int`
   - `typeof(Any) -> String`
   - `panic(Any) -> Nil`
2. **`resolver.rs` (tests)**: Added `len_typeof_panic_builtins` test verifying all three resolve without errors.

### Verification
- All 473 tests pass (228 compiler + 220 runtime + 15 integration + 10 manifest)
- All 7 examples compile and run successfully
- Clippy clean
- Minimal reproduction programs (`len(xs)`, `typeof(n)`) now compile and produce correct output
