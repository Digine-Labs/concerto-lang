# Bug Report: ModelBuilder Method Argument Contracts (`with_tools`, `without_tools`) Are Not Enforced

## Status: OPEN (2026-02-09)

## Summary
Spec requires strict builder contracts:
- `with_tools(array of tool/MCP refs)`
- `without_tools()` takes no args

Current behavior accepts invalid values and extra args without diagnostics.

## Severity
Medium-High (API contracts not enforced; invalid configuration is silently accepted).

## Date
2026-02-09

## Affected Components
- `spec/25-dynamic-tool-binding.md`
- `crates/concerto-compiler/src/semantic/resolver.rs`
- `crates/concerto-runtime/src/vm.rs`

## Reproduction
```concerto
model M {
    provider: openai,
    base: "gpt-4o-mini",
}

fn main() {
    let a = M.with_tools([123, true, "x"]).execute("hi");
    let b = M.without_tools(123).execute("hi");
    emit("a", a.is_ok());
    emit("b", b.is_ok());
}
```

Observed:

- Compiler: `No errors found.`
- Runtime: both calls succeed (`true` under mock provider)

Expected:

- compile-time/type error for invalid `with_tools` element types
- compile-time/arity error for `without_tools(123)`

## Root Cause
- No semantic validation exists for builder method signatures.
- Runtime `with_tools` accepts any array element and coerces via `display_string`.
- Runtime `without_tools` ignores extra arguments.

## Impact
- Invalid tool selections are silently ignored.
- Builder API contracts are not reliable.

## Workaround
Only pass identifier references (`[ToolName, McpServerName]`) and call `without_tools()` with zero args.

## Suggested Fix
Implement semantic checks for builder method arity and argument element types per spec; fail fast on invalid inputs.
