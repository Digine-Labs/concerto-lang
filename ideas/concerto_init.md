## `concerto init` - Project Scaffolding Command

### Problem

There's no standard way to start a new Concerto project. Developers must manually create files, figure out the directory structure, and write boilerplate. Every Concerto project needs a `Concerto.toml` manifest (see [concerto_toml_manifest.md](concerto_toml_manifest.md)) and at least one `.conc` source file with a `main()` function.

### Proposal

Add a `concerto init` subcommand to the `concerto` CLI that scaffolds a new project with a working hello-world agent program.

### Usage

```bash
# Create a new project in a new directory
concerto init my-project

# Initialize in the current directory
concerto init .
```

### Scaffolded Structure

```
my-project/
  Concerto.toml
  src/
    main.conc
  .gitignore
```

### Generated Files

**Concerto.toml:**
```toml
[project]
name = "my-project"
version = "0.1.0"
entry = "src/main.conc"

[connections.openai]
provider = "openai"
api_key_env = "OPENAI_API_KEY"
default_model = "gpt-4o-mini"
```

**src/main.conc:**
```concerto
schema Greeting {
    message: String,
    language: String,
}

agent Greeter {
    provider: openai,
    model: "gpt-4o-mini",
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

**.gitignore:**
```
*.conc-ir
.env
```

### Behavior Details

1. **Name inference**: `concerto init my-project` creates `my-project/` directory and uses "my-project" as `[project].name`. `concerto init .` uses the current directory name.
2. **No overwrite**: If target directory is non-empty (has a `Concerto.toml` already), error with message: `error: Concerto project already exists in this directory`.
3. **Output**: Print what was created:
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

### Implementation Scope

- **CLI only**: Add `Init` variant to the `Command` enum in `crates/concerto/src/main.rs`
- **No new crates**: Pure file generation with `std::fs` -- no templates engine needed
- **Depends on**: The Concerto.toml manifest idea (the generated TOML must match that spec)

### Open Questions for Plan Agent

1. Should `concerto init` live in the runtime CLI (`concerto`) or the compiler CLI (`concertoc`), or should there be a unified `concerto` CLI that merges both? (Currently they're separate binaries)
2. Should there be a `--provider` flag to scaffold with a different default provider? (e.g., `concerto init my-project --provider anthropic`)
3. Should `concerto init` also run `git init`?
