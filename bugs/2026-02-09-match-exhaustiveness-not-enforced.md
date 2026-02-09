# Bug Report: Match Exhaustiveness Is Not Enforced; Non-Matches Fall Back to `nil`

## Status: OPEN (2026-02-09)

## Summary
Spec states matches must be exhaustive, but compiler accepts non-exhaustive `match` expressions and codegen silently inserts `nil` fallback when no arm matches.

## Severity
High (control-flow and value soundness issue).

## Date
2026-02-09

## Affected Components
- `spec/05-control-flow.md`
- `crates/concerto-compiler/src/semantic/resolver.rs`
- `crates/concerto-compiler/src/codegen/emitter.rs`

## Reproduction
```concerto
fn main() {
    let out = match 2 {
        1 => "one",
    };
    emit("out", out);
}
```

Observed:

- Compiler: `No errors found.`
- Runtime: `[emit:out] nil`

Expected: compile-time exhaustiveness error.

## Root Cause
- Semantic analysis has no exhaustiveness validation pass for `match`.
- `generate_match()` unconditionally emits a fallback `nil` when no arm matches.

## Impact
- Missing branches are silently converted to `nil` values.
- Bugs in decision logic may go undetected.

## Workaround
Always include explicit wildcard arm (`_ => ...`) in every `match`.

## Suggested Fix
Implement exhaustiveness checks in semantic analysis and reserve `nil` fallback only for explicitly optional/dynamic match contexts.
