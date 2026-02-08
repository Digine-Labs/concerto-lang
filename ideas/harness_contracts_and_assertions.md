## Harness Contracts and Runtime Assertions

### Problem

Schema validation guarantees structure, but not harness-level correctness:

- score is present but too low,
- required evidence is missing,
- pipeline outputs violate business rules.

Teams currently encode these checks manually with ad-hoc `if` branches and custom error strings.

### Proposal

Add typed contract/assertion primitives for harness invariants.

### Example

```concerto
contract IncidentQuality(report: IncidentReport) {
    require(report.confidence >= 7, "confidence must be >= 7");
    require(report.action_items.len() > 0, "at least one action item required");
    require(report.severity != "low" || report.summary.len() > 40, "low severity still needs context");
}

fn process(ticket: String) -> Result<IncidentReport, AgentError> {
    let report = Extractor.execute_with_schema<IncidentReport>(ticket)?;
    enforce IncidentQuality(report)?;
    Ok(report)
}
```

Stage-level shorthand:

```concerto
stage review(draft: Draft) -> Verdict
    ensures VerdictQuality
{
    Judge.execute_with_schema<Verdict>(draft)?
}
```

### Semantics

- `contract` is a reusable invariant block with typed parameters.
- `require` failures return a structured `ContractError` (message + failed condition id).
- `enforce ContractName(...)` evaluates contract at runtime.
- `ensures` auto-injects contract enforcement on stage/fn outputs.

### Why It Fits Concerto

- Encodes harness correctness rules as first-class language artifacts.
- Improves readability and reuse of quality gates across pipelines.
- Strengthens reliability without forcing imperative boilerplate.

### MVP Scope

1. Add `contract`, `require`, `enforce`, `ensures` syntax.
2. Type-check contract parameter bindings.
3. Emit runtime `contract:passed` / `contract:failed` events.
4. Add standard `ContractError` type for `try/catch` handling.

### Open Questions

1. Should `require` support severity levels (`warn` vs `error`)?
2. Should contract expressions allow helper function calls or remain pure?
3. Should `ensures` support multiple contracts with ordered evaluation?
