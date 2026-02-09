# Bug Report: Bare `None` Match Pattern Is Parsed as Identifier Binding

## Status: OPEN (2026-02-09)

## Summary
A bare `None` pattern is parsed as `PatternKind::Identifier("None")` instead of enum unit variant, so it behaves like a catch-all binding.

## Severity
High (control-flow semantics are wrong in `match` on `Option`).

## Date
2026-02-09

## Affected Components
- `crates/concerto-compiler/src/parser/expressions.rs`
- `crates/concerto-compiler/src/semantic/resolver.rs`

## Reproduction
Source:

```concerto
fn main() {
    let out = match Some(7) {
        None => "none",
        Some(v) => "some",
    };
    emit("out", out);
}
```

Commands:

```bash
cargo run -q -p concertoc -- --check /tmp/concerto-audit/bug_none_pattern_catchall.conc
timeout 10s cargo run -q -p concerto -- run /tmp/concerto-audit/bug_none_pattern_catchall.conc
```

Observed output:

- Compiler warning: `unused variable 'None'`
- Runtime: `[emit:out] none`

Expected runtime output: `some`.

## Root Cause
- `parse_identifier_pattern()` returns `PatternKind::Identifier` for single-segment identifiers unless followed by tuple/struct syntax.
- Unit variants like `None` are not recognized as enum patterns in that path.

## Impact
- `match` arms with `None` can silently become catch-all binds.
- Branch ordering and correctness for `Option` handling becomes unreliable.

## Workaround
Use wildcard fallback (`_`) and place `Some(...)` arms explicitly, avoiding bare `None` patterns for correctness-sensitive code.

## Suggested Fix
Treat known enum unit variants (`None`, and ideally symbol-resolved unit variants) as `PatternKind::Enum` in parser/semantic pipeline, and add parser+runtime regression tests.
