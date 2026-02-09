# Bug Report: `&&` and `||` Eagerly Evaluate Both Sides (No Short-Circuit)

## Status: OPEN (2026-02-09)

## Summary
Spec states `&&` and `||` use short-circuit evaluation, but codegen/runtime eagerly evaluate both operands.

## Severity
High (semantic mismatch and unexpected side effects/errors).

## Date
2026-02-09

## Affected Components
- `spec/04-operators-and-expressions.md`
- `crates/concerto-compiler/src/codegen/emitter.rs`
- `crates/concerto-runtime/src/vm.rs`

## Reproduction
```concerto
fn side_true(label: String) -> Bool {
    emit("side_called", label);
    true
}

fn main() {
    let a = false && side_true("and_right");
    let b = true || side_true("or_right");
    emit("a", a);
    emit("b", b);
}
```

Observed:

- `[emit:side_called] and_right`
- `[emit:side_called] or_right`

Expected: neither side-effect emit should happen.

## Root Cause
- Binary expression codegen always emits left then right evaluation before applying opcode, including `And` and `Or`.
- VM executes `Opcode::And`/`Opcode::Or` on already-evaluated stack values.

## Impact
- Violates spec semantics.
- Can trigger unnecessary errors in right operand that should be skipped.
- Breaks common safety idioms relying on short-circuit guards.

## Workaround
Rewrite boolean expressions as explicit `if` chains to control evaluation order.

## Suggested Fix
Implement short-circuit lowering for logical operators (conditional jumps around right operand evaluation).
