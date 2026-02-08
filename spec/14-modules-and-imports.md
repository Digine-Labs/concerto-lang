# 14 - Modules and Imports

## Overview

Concerto uses a file-based module system. Each `.conc` file is a module. Modules organize code into namespaces, control visibility, and enable code reuse across files.

## File-Based Modules

Every `.conc` file is automatically a module. The module name is derived from the file name (without the `.conc` extension).

```
project/
  main.conc              -> module: main (entry point)
  classifier.conc        -> module: classifier
  agents/
    mod.conc             -> module: agents
    document.conc        -> module: agents::document
    research.conc        -> module: agents::research
  tools/
    mod.conc             -> module: tools
    file_connector.conc  -> module: tools::file_connector
    http_client.conc     -> module: tools::http_client
  schemas/
    mod.conc             -> module: schemas
    classification.conc  -> module: schemas::classification
```

## Module Declaration

### Directory Modules

A directory becomes a module when it contains a `mod.conc` file. The `mod.conc` file declares the submodules:

```concerto
// agents/mod.conc
pub mod document;    // Re-exports agents/document.conc as agents::document
pub mod research;    // Re-exports agents/research.conc as agents::research
```

### Inline Module Declaration

Modules can also be declared inline (less common):

```concerto
mod helpers {
    pub fn format_prompt(template: String, vars: Map<String, String>) -> String {
        // ...
    }
}

// Usage
let prompt = helpers::format_prompt(template, vars);
```

## Imports (`use`)

### Basic Import

```concerto
use agents::document::DocumentClassifier;
use schemas::classification::Classification;
use tools::file_connector::FileConnector;
```

### Multiple Imports from Same Module

```concerto
use agents::document::{DocumentClassifier, DocumentExtractor};
use schemas::classification::{Classification, Category, Confidence};
```

### Wildcard Import

Import all public items from a module:

```concerto
use agents::document::*;
// DocumentClassifier, DocumentExtractor, etc. are now in scope
```

**Note**: Wildcard imports are convenient but can cause name conflicts. Prefer explicit imports in larger projects.

### Aliased Import

Rename an import to avoid conflicts or improve clarity:

```concerto
use agents::document::DocumentClassifier as DocClassifier;
use agents::research::ResearchAgent as Researcher;

// Usage
let result = DocClassifier.execute(prompt)?;
let analysis = Researcher.execute(query)?;
```

### Nested Path Imports

```concerto
use std::{
    json::{parse, stringify},
    time::{now, sleep},
    collections::Set,
};
```

## Visibility

Items (functions, agents, tools, schemas, structs, enums, constants) are **private by default**. Use `pub` to make them accessible from other modules.

### Public Items

```concerto
// agents/classifier.conc

pub agent Classifier {
    // ...
}

pub schema ClassificationOutput {
    label: String,
    confidence: Float,
}

pub fn create_prompt(doc: String) -> String {
    "Classify the following document: ${doc}"
}

// Private -- only accessible within this module
fn internal_helper(text: String) -> String {
    text.trim().to_lower()
}

const INTERNAL_THRESHOLD: Float = 0.8;  // Private constant
pub const DEFAULT_MODEL: String = "gpt-4o";  // Public constant
```

### Public Fields

Struct and agent fields can individually be marked as public:

```concerto
pub struct Config {
    pub model: String,       // Public field
    pub temperature: Float,  // Public field
    api_key: String,         // Private field
}
```

## Standard Library (`std::`)

Concerto provides a standard library accessible via the `std::` prefix:

```concerto
use std::json;
use std::json::{parse, stringify};
use std::time::now;
use std::collections::Set;
use std::env::get as env_get;
```

### Standard Library Modules

| Module | Purpose |
|--------|---------|
| `std::json` | JSON parsing and serialization |
| `std::http` | HTTP client tools |
| `std::fs` | File system operations |
| `std::env` | Environment variables |
| `std::fmt` | String formatting |
| `std::collections` | Set, Queue, Stack |
| `std::time` | Time, sleep, timestamps |
| `std::math` | Math operations |
| `std::string` | String utilities |
| `std::log` | Logging (developer-facing, distinct from emit) |
| `std::prompt` | Prompt template utilities |
| `std::crypto` | Hashing, UUID generation |
| `std::tools` | Built-in tools (HttpTool, FileTool, ShellTool) |

See [19-standard-library.md](19-standard-library.md) for full API reference.

## Module Scope

Items defined at module scope (outside any function):

```concerto
// Module scope items:
use std::json;

const MAX_RETRIES: Int = 3;

connect openai {
    api_key: env("OPENAI_API_KEY"),
}

db shared_state: Database<String, Any> = Database::new();

agent Classifier { /* ... */ }
tool FileReader { /* ... */ }
schema Output { /* ... */ }

pub fn main() { /* ... */ }
```

Module-scope items are initialized when the module is loaded by the runtime.

## Entry Point

The program entry point is the `main` function in the root module:

```concerto
// main.conc

use agents::classifier::Classifier;
use schemas::output::ClassificationOutput;

fn main() {
    let result = Classifier.execute_with_schema<ClassificationOutput>(
        "Classify this document..."
    );
    match result {
        Ok(output) => emit("result", output),
        Err(e) => emit("error", e.message),
    }
}
```

The `main` function:
- Must be in the root module (file specified to the compiler)
- Takes no parameters
- Return type is optional (`Nil` by default, or `Result<Nil, Error>` for error propagation)

## Re-exports

Modules can re-export items from submodules:

```concerto
// agents/mod.conc
pub mod document;
pub mod research;

// Re-export commonly used items at the agents:: level
pub use document::DocumentClassifier;
pub use research::ResearchAgent;
```

This allows:
```concerto
// Instead of:
use agents::document::DocumentClassifier;

// Users can write:
use agents::DocumentClassifier;
```

## Circular Dependencies

Circular module dependencies are not allowed. The compiler will error if module A imports from module B and module B imports from module A:

```
// ERROR: Circular dependency detected:
// agents::classifier -> tools::validator -> agents::classifier
```

Solution: extract shared types into a separate module that both can import.

## Module Resolution Order

When the compiler encounters an import like `use foo::bar::Baz`:

1. Check if `foo` is `std` (standard library)
2. Look for `foo.conc` in the project root
3. Look for `foo/mod.conc` in the project root
4. Check `foo/bar.conc` for module `bar`
5. Check `foo/bar/mod.conc` for module `bar`
6. Look for `Baz` as a public item in the resolved module
7. Error if not found at any step
