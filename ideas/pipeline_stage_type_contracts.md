# Pipeline Stage Type Contracts

## Problem

Pipelines currently rely on per-stage signatures, but the compiler does not strictly enforce the full stage-to-stage contract.

Current gaps:

- Stage return type annotation is optional (`stage ... [-> Type]`), and missing annotations are only warnings.
- No dedicated compile-time check guarantees that `stage[i]` output is assignable to `stage[i+1]` input.
- Type mismatches can survive compilation and fail at runtime in later stages.

Example of risky flow:

```concerto
pipeline BuildFlow {
    stage parse(input: String) -> Int {
        42
    }

    stage render(parsed: String) -> String {
        // expects String, but previous stage returned Int
        "ok"
    }
}
```

## Proposal

Introduce strict pipeline contract checking in semantic analysis.

### 1) Enforce adjacent stage compatibility

For every neighboring pair of stages:

- `prev_output_type` must be assignable/coercible to `next_input_type`.
- If not assignable, emit a compile error with both stage names and both types.

Diagnostic shape:

```text
error: pipeline `BuildFlow` stage type mismatch
  = stage `parse` returns `Int`
  = stage `render` expects `String`
  = help: align stage signatures or insert a conversion stage
```

### 2) Require explicit stage output types (strict mode)

Promote missing stage return annotations from warning to error under strict mode.

- Default path can start as warning for migration.
- Long-term target: all stages explicitly declare output type.

### 3) Validate stage body against declared output

Ensure stage body tail expression / return paths conform to declared `-> Type`.

- If a stage is declared `-> Summary`, all successful return paths must resolve to `Summary`.
- Preserve existing `Result`-style propagation semantics (`?`) while checking success payload type.

### 4) Optional pipeline-level signature (future syntax)

Consider adding top-level pipeline contract syntax:

```concerto
pipeline BuildFlow(input: String) -> Output {
    ...
}
```

Compiler verifies:

- First stage input matches pipeline input type.
- Last stage output matches pipeline output type.

This improves readability and catches drift when stages are edited later.

## Migration Strategy

1. Add stage adjacency checks as hard errors immediately (high confidence, low breakage).
2. Keep missing stage return type as warning for one release cycle.
3. Promote missing stage return type to error (or gate with `--strict-pipelines`).
4. Optionally add pipeline-level signature syntax after core checks are stable.

## Why This Matters

- Prevents late runtime failures in orchestration graphs.
- Makes pipeline refactors safe (type errors surface at compile time).
- Improves trust in multi-stage AI workflows where shape drift is common.
- Aligns Concerto with its type-safety goals for production harnesses.
