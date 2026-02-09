# Bug Report: `use`/`mod` Declarations Are Parsed but Not Functionally Wired

## Status: OPEN (2026-02-09)

## Summary
`use` and `mod` syntax compiles, but imported/module members are not resolved for execution. Missing module files are also accepted without diagnostics.

## Severity
High (documented module/import workflow is non-functional).

## Date
2026-02-09

## Affected Components
- `spec/14-modules-and-imports.md`
- `crates/concerto-compiler/src/semantic/resolver.rs`
- `crates/concerto-compiler/src/codegen/emitter.rs`
- `crates/concerto-runtime/src/vm.rs`

## Reproduction A: `use` import cannot be called

```concerto
use std::json::parse;

fn main() {
    let x = parse("{}");
    emit("x", x);
}
```

Observed:

- Compiler error: `undefined variable 'parse'`

Expected:

- `parse` should resolve via `use`.

## Reproduction B: Inline module function call compiles, then runtime name error

```concerto
mod diagnostics {
    pub fn label() -> String {
        "inline"
    }
}

fn main() {
    emit("label", diagnostics::label());
}
```

Observed:

- Compiler: `No errors found.`
- Runtime: `name error: undefined variable 'diagnostics::label'`

## Reproduction C: Missing external module file accepted

```concerto
mod does_not_exist;

fn main() {
    emit("ok", true);
}
```

Observed:

- Compiler: `No errors found.`
- Runtime executes normally.

Expected:

- compile-time module resolution error.

## Root Cause
- Resolver registers module symbols but does not resolve/use module/import contents in pass 2.
- Codegen explicitly no-ops on `Declaration::Use` and `Declaration::Module`.
- Runtime path loading treats `name::path` as callable function reference without guaranteed backing function, deferring failure to runtime call sites.

## Impact
- `use` and module declarations are effectively documentation-only syntax.
- Path calls compile but fail at runtime.
- Missing module files are not detected.

## Workaround
Use fully qualified builtins directly where available (`std::...`) and avoid relying on `use`/`mod` for executable member resolution.

## Suggested Fix
Implement real module/import resolution in semantic + codegen phases (including file loading for `mod name;`) and validate path symbol existence at compile time.
