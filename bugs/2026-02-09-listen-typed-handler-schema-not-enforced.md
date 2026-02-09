# Bug Report: `listen` Typed Handler Parameters Are Not Type-Validated

## Status: OPEN (2026-02-09)

## Summary
`listen` handler parameter annotations (e.g., `|msg: HostProgress|`) are not enforced by semantic analysis or runtime message validation.

## Severity
High (typed streaming contracts are ignored; invalid payloads silently flow through).

## Date
2026-02-09

## Affected Components
- `spec/27-agent-streaming.md`
- `crates/concerto-compiler/src/semantic/resolver.rs`
- `crates/concerto-compiler/src/codegen/emitter.rs`
- `crates/concerto-runtime/src/vm.rs`

## Reproduction
Agent emits:

```json
{"type":"progress","message":"hello","percent":"oops"}
{"type":"result","text":"done"}
```

Concerto source:

```concerto
schema HostProgress {
    message: String,
    percent: Int,
}

agent Fake {
    connector: "fake",
    output_format: "json",
    timeout: 5,
}

fn main() {
    let result = listen Fake.execute("go") {
        "progress" => |msg: HostProgress| {
            emit("pct_plus", msg.percent + 1);
        },
    };

    emit("result", result);
}
```

Observed:

- Compiler: `No errors found.`
- Runtime emits `[emit:pct_plus] oops1` and completes successfully.

Expected: payload should fail type/schema validation before handler logic.

## Root Cause
- Resolver binds listen handler param as `Type::Unknown`, ignoring annotation.
- Codegen stores handler `param_type` as debug-string metadata.
- Runtime `run_listen_loop()` strips `type` and converts JSON to generic Value map without validating against handler param type/schema.

## Impact
- Typed handler contracts are effectively documentation-only.
- Invalid agent payloads can trigger downstream logic bugs without explicit validation errors.

## Workaround
Treat all listen payloads as untyped and perform explicit manual validation inside handlers.

## Suggested Fix
- Preserve canonical handler parameter type metadata in IR.
- Validate incoming message payload against annotated schema/type before invoking handler.
- Surface structured listen type errors.
