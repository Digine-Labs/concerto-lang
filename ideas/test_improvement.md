# Test Improvement Plan

## Current State

- **536 tests** (10 manifest + 250 compiler + 238 runtime + 38 integration)
- All tests are Rust `#[test]` unit/integration tests
- No snapshot testing, no fuzzing, no cross-platform CI, no benchmark tracking
- Concerto is Turing complete (while/loop + recursion + dynamic arrays/maps + if/else/match)

## Research Summary

Analysis of how Rust (rustc), Cairo, and Solidity compilers are tested, plus JavaScript (Test262), Python, Go, and TypeScript conformance strategies.

### Key Findings

| Compiler | Test Count | Primary Method | Key Tools |
|----------|-----------|----------------|-----------|
| Rust (rustc) | ~35,000-40,000 | UI snapshot tests (`.stderr` files) | compiletest, `--bless`, Miri, Crater, cargo-fuzz |
| Cairo | Thousands | Snapshot tests (`test_data/` dirs) | `CAIRO_FIX_TESTS=1`, `cairo-test` binary |
| Solidity | ~8,000-15,000 | Syntax tests + semantic EVM execution | `isoltest`, OSS-Fuzz, evmone, external project tests |
| TypeScript | ~70,000 baselines | Snapshot baselines (`.errors.txt`, `.types`) | Custom runner, baseline regeneration |
| JavaScript (Test262) | ~48,000 | Conformance suite with YAML metadata | Engine-agnostic, spec-section linked |

### Universal Patterns

1. **Snapshot/golden file testing** is the dominant pattern across all mature compilers
2. **Auto-update mechanisms** are essential (`--bless`, `CAIRO_FIX_TESTS=1`, `isoltest --update`)
3. **One test per error diagnostic** — every error the compiler can produce should have a test
4. **Fuzzing** finds bugs hand-written tests miss (Solidity found hundreds via OSS-Fuzz)
5. **External/ecosystem testing** validates real-world compatibility (Rust's Crater, Solidity's external project tests)
6. **Each pipeline stage tested independently** — not just final output

---

## Phase A: High Impact (Immediate)

### A1. Snapshot Testing for Compiler Diagnostics

Add `.stderr` golden files for compiler error output. When the compiler produces an error, the test captures the full diagnostic and compares against the stored snapshot.

**How it works:**
- Test `.conc` files live in `tests/snapshots/` with corresponding `.stderr` files
- A test runner compiles the `.conc` file and compares stderr output against the `.stderr` file
- `--bless` flag regenerates `.stderr` files when output intentionally changes
- Output normalization: replace absolute paths with `$DIR`, normalize line endings

**What Rust does:** 19,000+ UI tests in `tests/ui/`, each with a `.stderr` snapshot. `./x test --bless` auto-updates.

**What Cairo does:** `test_data/` directories with extensionless files. `CAIRO_FIX_TESTS=1` to regenerate.

**Example layout:**
```
tests/snapshots/
  errors/
    undefined_variable.conc        # Source that triggers error
    undefined_variable.stderr      # Expected compiler output
    type_mismatch.conc
    type_mismatch.stderr
    missing_return_type.conc
    missing_return_type.stderr
  warnings/
    unused_variable.conc
    unused_variable.stderr
```

### A2. Snapshot Testing for IR Output

Store expected `.conc-ir` JSON output for small programs. Catches regressions in IR generation (opcode changes, constant pool layout, new fields).

**Example layout:**
```
tests/snapshots/
  ir/
    hello_emit.conc               # Source
    hello_emit.conc-ir            # Expected IR JSON
    function_call.conc
    function_call.conc-ir
```

### A3. Compile All Examples in CI

All 18 example projects should compile cleanly on every commit. Currently only verified manually.

**Implementation:** A CI step that runs `concertoc --check` on every `examples/*/src/main.conc`.

### A4. Fuzz Targets for Lexer and Parser

Add `cargo-fuzz` targets so random/mutated input never panics the compiler — only produces clean errors.

**Targets:**
```
fuzz/fuzz_targets/
  fuzz_lexer.rs      # bytes -> tokenize, should never panic
  fuzz_parser.rs     # bytes -> parse, should never panic
  fuzz_semantic.rs   # bytes -> full compile, should never panic
  fuzz_ir_loader.rs  # bytes -> IR load, should never panic
```

**Seed corpus:** Existing `.conc` files from `tests/fixtures/` and `examples/`.

---

## Phase B: Hardening

### B1. Negative Test Coverage (One Per Error Diagnostic)

For every error the compiler can emit, create a `.conc` file that triggers it. Track coverage against the set of all possible diagnostics.

**Approach:** Grep for all `self.diagnostics.error(...)` calls in the compiler. For each unique error message pattern, ensure a test exists.

**Current gap estimate:** The compiler likely has 50-100 distinct error diagnostics. Many probably have tests already via unit tests, but not via snapshot tests that also verify the error message text and span.

### B2. Edge Case and Stress Tests

| Test | What It Catches |
|------|-----------------|
| 999/1000/1001 recursive calls | Stack overflow handling at exact boundary |
| `9223372036854775807 + 1` (INT64_MAX + 1) | Integer overflow behavior |
| `let s = "a" * 1000000` (huge string) | String handling limits |
| 10,000-line generated program | Compiler performance/memory |
| 1000-deep nested `if` | Parser stack depth |
| Empty program (no fn main) | Edge case: no entry point |
| Program with only comments | Lexer edge case |
| Unicode emoji in strings | Lexer robustness |
| Null bytes in source | Lexer safety |
| Zero-length identifiers | Parser safety |
| Self-referential type aliases | Semantic cycle detection |
| 1000-element array literal | Codegen/runtime handling |
| Mutually recursive functions at max depth | Call stack unwinding |

### B3. Cross-Platform CI

GitHub Actions matrix: `ubuntu-latest`, `macos-latest`, `windows-latest`.

Run `cargo test --workspace` and `cargo clippy --workspace` on all three.

### B4. Turing Completeness Demonstrations

Add example programs that formally demonstrate computational completeness:

1. **Ackermann function** — proves Concerto exceeds primitive recursive power (cannot be computed with only bounded loops). A(3, 4) = 125 is tractable; A(4, 2) is theoretically possible but impractical.
2. **Brainfuck interpreter** — gold standard proof of Turing completeness by simulation. Requires: dynamic array tape, instruction loop, loop bracket matching, conditionals.
3. **Quicksort** — recursion + array partitioning + computed indexing.
4. **Fibonacci (both recursive and iterative)** — demonstrates both computation models work.

These would live in `examples/turing_completeness/` with a README explaining their significance.

---

## Phase C: Production Quality (Ongoing)

### C1. Grammar-Based Program Generator

A Rust tool that walks the Concerto grammar and generates random valid `.conc` programs. Used for:
- Fuzzing: compile thousands of random programs, none should crash
- Stress testing: generate programs with extreme nesting, long identifiers, many declarations
- Differential testing: compare debug vs release build outputs

**How CSmith works (C):** Generates random C programs with defined behavior, compiles with multiple compilers, compares outputs. Found hundreds of bugs in GCC and LLVM.

**How jsfunfuzz works (JS):** Grammar-driven JS generation. Found 2,800+ bugs in SpiderMonkey.

**For Concerto:** Generate random programs with agents, tools, schemas, pipelines, functions, control flow. Every generated program should either compile cleanly or produce a clean error — never crash.

### C2. Mutation Testing

Run `cargo-mutants` on the compiler and runtime. The tool modifies source code (change `+` to `-`, remove `if` branches, etc.) and verifies that at least one test catches each mutation.

**Metric:** Mutation kill rate. Aim for >80%. Low kill rates reveal areas where tests exist but don't actually verify behavior.

### C3. Spec-Section Coverage Tracking

Annotate tests with spec section references:
```rust
#[test]
fn test_while_loop_break_value() {
    // spec: 05-control-flow, section: while-loops
    ...
}
```

Generate a coverage report showing which spec sections have tests and which don't. With 30 spec files, this reveals gaps.

### C4. Benchmark Tracking

Use `criterion.rs` for reproducible benchmarks:
- **Compile time:** Time to compile `hello_agent` example
- **Runtime:** Time to execute a pipeline with N stages
- **Lexer throughput:** Tokens per second on large input
- **IR loading:** Time to deserialize a large `.conc-ir`

Track across commits. Flag >10% regressions.

### C5. Concerto-Level Test Suite (`concerto test` dogfooding)

Write tests for Concerto IN Concerto using the `@test`/`mock` system. This dogfoods the testing framework and builds a conformance suite in the language itself.

**Categories:**
```concerto
// tests/conformance/arithmetic.conc
@test fn integer_addition() { assert_eq(2 + 3, 5); }
@test fn integer_overflow_wraps() { ... }
@test fn float_precision() { assert_eq(0.1 + 0.2 != 0.3, true); }

// tests/conformance/control_flow.conc
@test fn while_loop_basic() { ... }
@test fn for_loop_array() { ... }
@test fn match_enum_variants() { ... }

// tests/conformance/agents.conc
@test fn mock_agent_execute() { mock Agent { response: "hi" } ... }
@test fn mock_agent_schema_validation() { ... }
```

### C6. External Program Regression Testing

As the Concerto ecosystem grows, maintain a set of known-good Concerto programs and recompile them against every compiler change.

**What Solidity does:** Maintains forks of Gnosis, OpenZeppelin, etc. Compiles them against the development compiler. Any regression blocks the release.

**What Rust does (Crater):** Rebuilds every crate on crates.io with the new compiler. Results posted on PRs.

**For Concerto:** Start with the 18 example projects. As users write programs, collect (with permission) a corpus for regression testing.

---

## Priority Matrix

| Priority | Strategy | Effort | Impact | Phase |
|----------|----------|--------|--------|-------|
| 1 | Snapshot testing for diagnostics (`.stderr`) | Medium | Critical | A |
| 2 | Snapshot testing for IR output (`.conc-ir`) | Medium | High | A |
| 3 | Compile all examples in CI | Low | High | A |
| 4 | Fuzz targets (lexer, parser, IR loader) | Low | High | A |
| 5 | One negative test per error diagnostic | Medium | High | B |
| 6 | Edge case / stress tests | Medium | High | B |
| 7 | Cross-platform CI (macOS + Windows) | Low | Medium | B |
| 8 | Turing completeness examples (Ackermann, BF) | Low | Medium | B |
| 9 | Grammar-based program generator | High | High | C |
| 10 | Mutation testing (cargo-mutants) | Low | Medium | C |
| 11 | Spec-section coverage tracking | Low | Low | C |
| 12 | Benchmark tracking (criterion) | Low | Medium | C |
| 13 | Concerto-level conformance suite | Medium | High | C |
| 14 | External program regression testing | Low | Medium | C |

## Target Metrics

| Metric | Current | Phase A Target | Phase C Target |
|--------|---------|---------------|----------------|
| Total Rust tests | 536 | 700+ | 1500+ |
| Snapshot tests | 0 | 100+ | 500+ |
| Fuzz targets | 0 | 4 | 6+ |
| Error diagnostic coverage | Unknown | 50%+ | 90%+ |
| Platform CI | 1 (Linux) | 3 (Linux/macOS/Windows) | 3+ |
| Concerto conformance tests | 0 | 0 | 200+ |
| Mutation kill rate | Unknown | Unknown | >80% |
| Spec section coverage | Unknown | 50%+ | 90%+ |

## References

- [Rust Compiler Testing Guide](https://rustc-dev-guide.rust-lang.org/tests/intro.html)
- [Rust UI Tests](https://rustc-dev-guide.rust-lang.org/tests/ui.html)
- [Cairo CONTRIBUTING.md](https://github.com/starkware-libs/cairo/blob/main/docs/CONTRIBUTING.md)
- [Solidity Contributing Docs](https://docs.soliditylang.org/en/latest/contributing.html)
- [Solidity Fuzz Testing Blog](https://www.soliditylang.org/blog/2021/02/10/an-introduction-to-soliditys-fuzz-testing-approach/)
- [How Rust is Tested (Brian Anderson)](https://brson.github.io/2017/07/10/how-rust-is-tested)
- [Snapshot Testing for Compilers (Adrian Sampson)](https://www.cs.cornell.edu/~asampson/blog/turnt.html)
- [Test262 (ECMAScript conformance)](https://github.com/tc39/test262)
- [CSmith: Random C Program Generator](https://github.com/csmith-project/csmith)
- [jsfunfuzz: JavaScript Engine Fuzzer](https://github.com/MozillaSecurity/funfuzz)
- [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)
- [cargo-mutants](https://github.com/sourcefrog/cargo-mutants)
