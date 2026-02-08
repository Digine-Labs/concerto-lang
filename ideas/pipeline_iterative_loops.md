## Iterative Pipeline Loop Primitive

### Problem
Quality-improvement workflows (draft -> judge -> revise until threshold) currently require embedding `while` loops inside a single `stage`, which makes pipeline graphs less declarative and harder to observe.

### Proposal
Add an iterative pipeline primitive that repeats one or more stages until a stop condition is met.

### Possible Syntax
```concerto
pipeline MemoFlow {
    stage draft(input: Topic) -> Draft { ... }
    stage judge(draft: Draft) -> Score { ... }

    repeat [draft, judge] until score.total >= 24 max_rounds 5

    stage finalize(score: Score) -> Output { ... }
}
```

Alternative expression-style form:
```concerto
stage refine(input: Draft) -> Draft {
    loop_stage(max: 5, until: score.total >= 24) {
        let score = Judge.execute_with_schema<Score>(input)?;
        if score.total < 24 {
            input = Rewriter.execute_with_schema<Draft>(input)?;
        }
    }
    input
}
```

### Why It Fits Concerto
- Concerto is orchestration-first; iterative quality gates are a core agent-harness pattern.
- Makes loop state and stopping criteria explicit for tooling/observability.
- Enables runtime-native metrics (`pipeline:iteration_start`, `pipeline:iteration_complete`, convergence stats).

### MVP Scope
1. New syntax for `repeat ... until ... max_rounds ...` in pipeline declarations.
2. Compiler lowering to existing control-flow IR.
3. Runtime events for each iteration and convergence status.
4. Validation errors for missing stop condition or unbounded iteration.
