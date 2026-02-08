# 23 - Project Scaffolding (concerto init)

## Overview

The `concerto init` command scaffolds a new Concerto project with a working directory structure, a `Concerto.toml` manifest, a hello-world agent program, and a `.gitignore` file. It provides a zero-friction entry point for new users.

## Command Syntax

```bash
# Create a new project in a new directory
concerto init <name>

# Initialize in the current directory
concerto init .
```

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `name` | Yes | Project directory name, or `.` for current directory |

### Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--provider <name>` | `-p` | Default LLM provider: `openai` (default), `anthropic`, `ollama` |

## Scaffolded Structure

```
<name>/
  Concerto.toml
  src/
    main.conc
  .gitignore
```

Three files, one subdirectory. Minimal and opinionated.

## Generated Files

### Concerto.toml

Generated based on the `--provider` flag (default: `openai`).

#### With `--provider openai` (default)

```toml
[project]
name = "<name>"
version = "0.1.0"
entry = "src/main.conc"

[connections.openai]
provider = "openai"
api_key_env = "OPENAI_API_KEY"
default_model = "gpt-4o-mini"
```

#### With `--provider anthropic`

```toml
[project]
name = "<name>"
version = "0.1.0"
entry = "src/main.conc"

[connections.anthropic]
provider = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
default_model = "claude-sonnet-4-20250514"
```

#### With `--provider ollama`

```toml
[project]
name = "<name>"
version = "0.1.0"
entry = "src/main.conc"

[connections.local]
provider = "ollama"
base_url = "http://localhost:11434/v1"
default_model = "llama3.1"
```

### src/main.conc

A minimal but complete agent program that demonstrates schema-validated LLM output. The `provider:` name matches the TOML connection name.

```concerto
schema Greeting {
    message: String,
    language: String,
}

agent Greeter {
    provider: <connection_name>,
    model: "<default_model>",
    temperature: 0.7,
    system_prompt: "You are a friendly multilingual greeter. Always respond with valid JSON.",
}

fn main() {
    let result = Greeter.execute_with_schema<Greeting>(
        "Say hello in French. Return JSON with 'message' and 'language' fields."
    );

    match result {
        Ok(greeting) => emit("greeting", {
            "message": greeting.message,
            "language": greeting.language,
        }),
        Err(e) => emit("error", e.message),
    }
}
```

Where `<connection_name>` and `<default_model>` are substituted based on the provider flag:

| `--provider` | `connection_name` | `default_model` |
|--------------|-------------------|-----------------|
| `openai` | `openai` | `gpt-4o-mini` |
| `anthropic` | `anthropic` | `claude-sonnet-4-20250514` |
| `ollama` | `local` | `llama3.1` |

### .gitignore

```
# Compiled IR
*.conc-ir

# Environment secrets
.env
```

## Behavior

### Name Inference

- `concerto init my-project` — creates a `my-project/` directory, uses `"my-project"` as `[project].name`
- `concerto init .` — does not create a directory, uses the current directory's name as `[project].name`

### No Overwrite

If the target directory already contains a `Concerto.toml`, the command fails:

```
error: Concerto project already exists in this directory
  = help: remove Concerto.toml to reinitialize, or use a different directory
```

The check is specifically for `Concerto.toml`, not for an empty directory. Files like `README.md` or `.git/` are fine.

### Output

On success, print what was created and next steps:

```
Created Concerto project "my-project"
  Concerto.toml
  src/main.conc
  .gitignore

Get started:
  cd my-project
  export OPENAI_API_KEY="your-key"
  concertoc src/main.conc
  concerto run src/main.conc-ir
```

The `export` line adapts to the provider:
- OpenAI: `export OPENAI_API_KEY="your-key"`
- Anthropic: `export ANTHROPIC_API_KEY="your-key"`
- Ollama: (omitted — no key needed, but prints `ollama serve` reminder)

### Directory Creation

- `concerto init my-project` — creates `my-project/` if it doesn't exist. If it exists and is empty (or has no `Concerto.toml`), uses it as-is.
- `concerto init .` — never creates a directory. Initializes in the current working directory.

## CLI Integration

The `init` subcommand is added to the `concerto` binary (the runtime CLI), not `concertoc` (the compiler CLI). This aligns with Cargo's model where `cargo init` is on the main tool.

```rust
#[derive(clap::Subcommand)]
enum Command {
    /// Execute a compiled .conc-ir file
    Run { ... },

    /// Create a new Concerto project
    Init {
        /// Project name or '.' for current directory
        name: String,

        /// Default LLM provider
        #[arg(short, long, default_value = "openai")]
        provider: String,
    },
}
```

## Error Cases

| Condition | Error Message |
|-----------|--------------|
| `Concerto.toml` exists | `error: Concerto project already exists in this directory` |
| Invalid provider flag | `error: unknown provider '<name>'. Valid providers: openai, anthropic, ollama` |
| Cannot create directory | `error: failed to create directory '<name>': <os error>` |
| Cannot write file | `error: failed to write '<path>': <os error>` |

## Implementation Scope

- **CLI only**: Add `Init` variant to `Command` enum in `crates/concerto/src/main.rs`
- **No new crates**: Pure `std::fs` file generation
- **No template engine**: String formatting with `format!()` is sufficient for 3 files
- **Depends on**: spec/22 (Concerto.toml format) for the generated manifest structure

## Relationship to Other Specs

- **Depends on**: spec/22 (Project Manifest) — generated `Concerto.toml` follows that spec
- **Extends**: spec/17 (Runtime Engine) — new CLI subcommand on the runtime binary
