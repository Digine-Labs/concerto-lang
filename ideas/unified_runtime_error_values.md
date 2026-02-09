# Unified Runtime Error Values

## Problem

Runtime `Result` errors are currently inconsistent across subsystems:

- Agent/host/stdlib paths often return `Err(String)`.
- Tool paths use structured `ToolError` values.
- Language examples/spec snippets frequently assume `Err(e).message`, which fails when `e` is a `String`.

This mismatch causes avoidable runtime failures in normal error branches (for example: `cannot access field 'message' on String`).

## Proposal

Introduce a first-class runtime error value shape for all recoverable errors:

```concerto
struct RuntimeErrorValue {
    kind: String,
    message: String,
    source?: String,
    code?: String,
    details?: Map<String, Any>,
}
```

Then standardize all `Result<_, E>` returns to use structured errors (`AgentError`, `JsonError`, `HttpError`, etc.) that at minimum expose `.message`.

## Migration Strategy

1. Add an adapter layer in runtime that wraps existing `String` errors into a structured error object.
2. Keep backward compatibility by allowing string coercion (`emit("error", e)` still works).
3. Update stdlib and agent/host execution to emit typed errors natively.
4. Add conformance tests that verify `Err(e).message` works across stdlib + agent + host flows.

## Why This Matters

- Removes a common class of runtime failures in error handling code.
- Aligns runtime behavior with spec/examples that model typed errors.
- Makes debugging and observability better (`kind`, `source`, `code`, `details`).
