# Repository Guidelines

## Project Structure & Module Organization
This repository is a Rust workspace for the Concerto language toolchain.

- `crates/concerto-common`: shared types (IR, diagnostics, spans, manifest).
- `crates/concerto-compiler`: lexer, parser, semantic analysis, IR generation.
- `crates/concertoc`: compiler CLI (`.conc` -> `.conc-ir`).
- `crates/concerto-runtime`: VM/runtime, providers, schema/tool systems, stdlib.
- `crates/concerto`: runtime CLI (`concerto run <file.conc-ir>`).
- `spec/`: language source of truth.
- `examples/`: sample `.conc` programs.
- `tests/fixtures/`: shared test inputs.

## Agent Workflow (Mandatory)
When working in this repository, treat `STATUS.md`, `CLAUDE.md`, `spec/`, and `ideas/` as a coordinated system.

- Start each task by reading `STATUS.md` (especially **Current Focus** and relevant phase tables) to understand what is complete, in progress, and next.
- Use `STATUS.md` as a project ledger: after feature work or bug fixes, update statuses, notes, and progress entries in the affected phase.
- Keep `CLAUDE.md` and `STATUS.md` in sync. If behavior, architecture, semantics, or major implementation details change, update both files in the same change set.
- Follow spec-first development: if semantics change, update or add files in `spec/` before or alongside implementation.
- While implementing any feature or fix, evaluate whether a language-level improvement is worth proposing; if yes, create an idea doc in `ideas/` (no A/B test required).
- Before adding a new idea, check whether it is already captured in `ideas/` or promoted into `spec/`; if already covered, link to the existing artifact instead of duplicating it.
- Keep idea proposals aligned to Concerto’s purpose: orchestrating AI agents with strong typing, composable pipelines, reliable runtime behavior, and practical developer workflows.

## Build, Test, and Development Commands
Run all commands from repository root.

- `cargo build --workspace`: build all crates.
- `cargo build --release`: produce release binaries in `target/release/`.
- `cargo test --workspace`: run unit + integration tests.
- `cargo test -p concerto-runtime --test integration`: run end-to-end runtime integration tests.
- `cargo run -p concertoc -- examples/hello_agent.conc`: compile an example program.
- `cargo run -p concerto -- run examples/hello_agent.conc-ir`: execute compiled IR.
- `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`: formatting and lint gate.

## Coding Style & Naming Conventions
Follow standard Rust conventions and keep code clippy-clean.

- Formatting: `rustfmt` (4-space indentation, no manual alignment tweaks).
- Naming: `snake_case` for modules/functions/files, `UpperCamelCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants.
- Error handling: prefer `Result<T, E>` and explicit error enums (`thiserror`) over panics.
- Keep compiler/runtime behavior aligned with `spec/` before merging.

## Testing Guidelines
- Put focused unit tests in `#[cfg(test)]` modules near implementation.
- Keep cross-crate behavior checks in integration tests (for example `crates/concerto-runtime/tests/integration.rs`).
- Name tests by behavior (for example `e2e_result_propagation`, `e2e_pipe_operator`).
- Always run `cargo test --workspace` before opening a PR.

## Commit & Pull Request Guidelines
Git history shows a component-prefix style; use descriptive messages:

- Preferred format: `<component>: <description>` (for example `compiler: validate union type narrowing`).
- Use clear component prefixes such as `compiler`, `runtime`, `spec`, `docs`, `phaseX`.
- PRs should include: what changed, why, impacted crates/spec sections, and exact validation commands run.
- If semantics or behavior changes, update `spec/`, `STATUS.md`, and `CLAUDE.md` in the same PR.
- If a task reveals a strong future enhancement, add/update an `ideas/*.md` note and reference it in the PR summary.

## Security & Configuration Tips
- Do not commit API keys or provider secrets.
- Keep credentials in environment variables (for example `OPENAI_API_KEY`) and reference them via manifest/source configuration.
