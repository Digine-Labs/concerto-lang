# 30. Pipeline Stage Type Contracts

## Overview

Pipelines are first-class constructs for multi-step agent workflows (see [spec 15](15-concurrency-and-pipelines.md)). Each stage receives the previous stage's output as input and produces a new output. Currently, the compiler does not enforce type compatibility between adjacent stages — mismatches survive compilation and fail at runtime.

This spec introduces strict compile-time type checking for pipeline stage adjacency, required stage return types, and an optional pipeline-level signature syntax.

## Problem

### Missing Adjacency Checks

The compiler validates each stage independently but does not verify that stage N's output type is compatible with stage N+1's input type:

```concerto
pipeline BuildFlow {
    stage parse(input: String) -> Int {
        42
    }

    stage render(parsed: String) -> String {
        // Compile succeeds, but runtime fails:
        // parse() returns Int, render() expects String
        "ok"
    }
}
```

### Optional Return Types

Stage return type annotations are currently optional (missing annotations produce warnings, not errors). This makes adjacency checking impossible when types are omitted.

### No Pipeline-Level Contract

There is no way to declare a pipeline's overall input/output contract. When stages are edited, the first-stage input or last-stage output can drift without warning.

## Stage Adjacency Type Checking

### Rule

For every pair of adjacent stages `(stage[i], stage[i+1])` in a pipeline:

- Let `output_type` be the declared return type of `stage[i]`.
- Let `input_type` be the declared parameter type of `stage[i+1]`'s first (and only) parameter.
- `output_type` must be **assignable** to `input_type`.

### Result Unwrapping

The runtime automatically unwraps `Result<T, E>` between stages — if a stage returns `Ok(value)`, the next stage receives `value` (not the `Result` wrapper). On `Err`, the pipeline short-circuits.

The adjacency check accounts for this:

- If `stage[i]` declares `-> Result<T, E>`, the effective output type for adjacency checking is `T` (the success payload).
- If `stage[i]` declares `-> T` (non-Result), the effective output type is `T`.

```concerto
pipeline DocFlow {
    // Returns Result<String, Error>, but runtime unwraps to String
    stage extract(doc: String) -> String {
        let response = Extractor.execute(doc)?;  // ? makes return type Result
        response.text
    }

    // Receives String (unwrapped from Result<String, Error>) -- OK
    stage classify(text: String) -> Classification {
        Classifier.execute_with_schema<Classification>(text)?
    }
}
```

### Assignability Rules

Type `A` is assignable to type `B` if:

| A | B | Assignable? |
|---|---|-------------|
| `T` | `T` | Yes (exact match) |
| `T` | `Any` | Yes (`Any` accepts everything) |
| `Any` | `T` | Yes (dynamic, defer to runtime) |
| `Unknown` | `T` | Yes (unresolved, skip check) |
| `T` | `Unknown` | Yes (unresolved, skip check) |
| `Named("X")` | `Named("X")` | Yes (same named type) |
| `Array<T>` | `Array<U>` | Yes if `T` assignable to `U` |
| `Map<K1,V1>` | `Map<K2,V2>` | Yes if `K1≈K2` and `V1≈V2` |
| `Result<T,E>` | `T` | Yes (after unwrap) |
| `Int` | `Float` | Yes (numeric promotion) |
| Otherwise | | No — compile error |

### Diagnostic

When a type mismatch is detected:

```text
error: pipeline `BuildFlow` stage type mismatch
  --> src/main.conc:8:5
  |
4 |     stage parse(input: String) -> Int {
  |                                    --- stage `parse` returns `Int`
  ...
8 |     stage render(parsed: String) -> String {
  |                  ^^^^^^^^^^^^^^^ stage `render` expects `String`
  |
  = help: align stage signatures or insert a conversion stage
```

### Implementation

In `Resolver::resolve_declaration()` for `Declaration::Pipeline(p)`:

```rust
// After resolving all stages individually:
for i in 0..p.stages.len() - 1 {
    let current = &p.stages[i];
    let next = &p.stages[i + 1];

    let output_type = current.return_type.as_ref()
        .map(Type::from_annotation)
        .unwrap_or(Type::Any);

    // Unwrap Result<T, E> to T (runtime does this between stages)
    let effective_output = match &output_type {
        Type::Result(inner, _) => inner.as_ref().clone(),
        other => other.clone(),
    };

    if let Some(first_param) = next.params.first() {
        let input_type = first_param.type_ann.as_ref()
            .map(Type::from_annotation)
            .unwrap_or(Type::Any);

        if !is_assignable(&effective_output, &input_type) {
            self.diagnostics.report(
                Diagnostic::error(format!(
                    "pipeline `{}` stage type mismatch: `{}` returns `{}` but `{}` expects `{}`",
                    p.name, current.name, effective_output.display_name(),
                    next.name, input_type.display_name()
                ))
                .with_span(next.params[0].span.clone())
                .with_suggestion("align stage signatures or insert a conversion stage"),
            );
        }
    }
}
```

## Required Stage Return Types

### Current Behavior

Missing stage return type annotations produce a **warning**:

```text
warning: stage `classify` in pipeline `DocFlow` has no return type annotation
```

### New Behavior

Missing stage return type annotations are promoted to **errors**:

```text
error: stage `classify` in pipeline `DocFlow` must have a return type annotation
  --> src/main.conc:10:5
  |
10|     stage classify(text: String) {
  |     ^^^^^ add `-> ReturnType` to this stage
  |
  = help: explicit return types are required for pipeline stage type checking
```

This change is necessary because adjacency checking requires knowing the output type of each stage. Without it, the compiler cannot verify the pipeline's data flow.

### Implementation

In `Validator::validate_pipeline()`, change the existing warning to an error:

```rust
for stage in &pipeline.stages {
    if stage.return_type.is_none() {
        self.diagnostics.error(
            format!(
                "stage `{}` in pipeline `{}` must have a return type annotation",
                stage.name, pipeline.name
            ),
            stage.span.clone(),
        );
    }
}
```

## Pipeline-Level Signature

### Syntax

Pipelines can optionally declare an overall input/output contract:

```concerto
pipeline DocumentProcessor(input: String) -> Summary {
    stage extract(doc: String) -> String {
        // ...
    }

    stage classify(text: String) -> Classification {
        // ...
    }

    stage summarize(classification: Classification) -> Summary {
        // ...
    }
}
```

The function-like syntax `pipeline Name(input: T) -> U` is consistent with `fn` and `stage` signatures.

### Semantics

When a pipeline-level signature is declared, the compiler verifies:

1. **First stage input**: The first stage's parameter type must be assignable from the pipeline's input type.
2. **Last stage output**: The last stage's return type must be assignable to the pipeline's declared output type.

```text
error: pipeline `DocumentProcessor` input type mismatch
  |
1 | pipeline DocumentProcessor(input: String) -> Summary {
  |                                   ^^^^^^ pipeline declares input `String`
2 |     stage extract(doc: Int) -> String {
  |                       ^^^ first stage expects `Int`
```

```text
error: pipeline `DocumentProcessor` output type mismatch
  |
1 | pipeline DocumentProcessor(input: String) -> Summary {
  |                                               ^^^^^^^ pipeline declares output `Summary`
  ...
10|     stage summarize(classification: Classification) -> String {
  |                                                        ^^^^^^ last stage returns `String`
```

### Pipeline Run Typing

With a pipeline signature, the `.run()` call gains compile-time type information:

```concerto
// Without signature: result type is Any
let result = MyPipeline.run(input);

// With signature pipeline MyPipeline(input: String) -> Summary:
// Compiler knows result is Result<Summary, Error>
let result = MyPipeline.run(input);
```

### AST Changes

Add optional fields to `PipelineDecl`:

```rust
pub struct PipelineDecl {
    pub name: String,
    pub stages: Vec<StageDecl>,
    pub span: Span,
    // NEW: optional pipeline-level signature
    pub input_param: Option<Param>,
    pub return_type: Option<TypeAnnotation>,
}
```

### Parser Changes

The parser recognizes the optional parameter list and return type after the pipeline name:

```
pipeline Name [ "(" param ")" ] [ "->" Type ] "{" stages "}"
```

If no parameter list is given, the existing syntax is preserved (backwards compatible).

### IR Changes

Add optional fields to `IrPipeline`:

```rust
pub struct IrPipeline {
    pub name: String,
    pub stages: Vec<IrPipelineStage>,
    // NEW: optional pipeline-level type contract
    pub input_type: Option<serde_json::Value>,
    pub output_type: Option<serde_json::Value>,
}
```

## Migration Strategy

### Phase 1: Adjacency Checks (Immediate)

- Add stage adjacency type checking as **hard errors**.
- High confidence, low breakage: existing code that has correct types passes; only genuinely mismatched pipelines fail.
- Result unwrapping is accounted for automatically.

### Phase 2: Required Return Types (Immediate)

- Promote missing stage return type annotation from warning to **error**.
- Any stage without `-> Type` must be annotated before compilation succeeds.
- This is prerequisite for meaningful adjacency checks.

### Phase 3: Pipeline-Level Signature (Next Release)

- Add optional `pipeline Name(input: T) -> U` syntax.
- First/last stage contract validation.
- Backwards compatible — existing pipelines without signatures continue to work.
- Run-time: pipeline input is validated against declared type before first stage.

## Examples

### Valid Pipeline (All Checks Pass)

```concerto
pipeline ReviewFlow(input: String) -> ReviewResult {
    stage analyze(code: String) -> Analysis {
        Analyzer.execute_with_schema<Analysis>(code)?
    }

    stage review(analysis: Analysis) -> ReviewResult {
        let prompt = "Review: ${analysis.summary}";
        Reviewer.execute_with_schema<ReviewResult>(prompt)?
    }
}
```

Checks:
- `analyze` returns `Analysis` (unwrapped from `Result<Analysis, Error>`)
- `review` expects `Analysis` -- match
- Pipeline input `String` matches first stage input `String` -- match
- Pipeline output `ReviewResult` matches last stage output `ReviewResult` -- match

### Invalid Pipeline (Adjacency Error)

```concerto
pipeline BadFlow {
    stage count(input: String) -> Int {
        len(input)
    }

    stage format(text: String) -> String {
        "Count: ${text}"
    }
}
```

Error: `count` returns `Int`, but `format` expects `String`.

### Invalid Pipeline (Signature Mismatch)

```concerto
pipeline Broken(input: Int) -> String {
    stage process(text: String) -> String {
        text
    }
}
```

Error: Pipeline declares input `Int`, but first stage expects `String`.

## Design Rationale

### Why Strict by Default

- Pipeline type mismatches are silent bugs that manifest at runtime in later stages — often in production with real data.
- Concerto's type-safety goals require catching these at compile time.
- The stage return type requirement is a small annotation burden with large safety payoff.

### Why Unwrap Result in Checks

- The runtime already unwraps `Result` between stages (short-circuiting on `Err`).
- Requiring stages to explicitly accept `Result<T, E>` would be redundant and verbose — every stage would need to destructure.
- Checking the success payload type matches what the next stage actually receives.

### Why Function-Style Pipeline Signature

- Consistent with `fn name(param: T) -> U` and `stage name(param: T) -> U`.
- Reads naturally: "this pipeline takes a String and produces a Summary".
- The parameter name is useful for documentation, even though `.run()` calls use positional args.
