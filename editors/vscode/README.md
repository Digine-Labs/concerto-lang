# Concerto Language for VS Code

Syntax highlighting and language support for the [Concerto](https://github.com/Digine-Labs/concerto-lang) programming language.

## Features

- Syntax highlighting for all Concerto language constructs
- Bracket matching and auto-closing
- Comment toggling (`Ctrl+/` for line, `Shift+Alt+A` for block)
- Indentation support

### Highlighted Elements

- **Keywords**: `let`, `fn`, `agent`, `tool`, `schema`, `pipeline`, `stage`, `match`, `if`/`else`, `try`/`catch`/`throw`, `async`/`await`, and more
- **AI constructs**: `agent`, `tool`, `schema`, `pipeline`, `stage`, `db`, `ledger`, `mcp`, `emit`
- **Types**: `Int`, `Float`, `String`, `Bool`, `Array<T>`, `Map<K,V>`, `Option<T>`, `Result<T,E>`, AI types
- **String interpolation**: `"Hello, ${name}!"` with full expression highlighting inside `${}`
- **Decorators**: `@describe(...)`, `@param(...)`, `@retry(...)`, `@timeout(...)`, `@log(...)`
- **Numbers**: decimal, hex (`0xFF`), binary (`0b1010`), octal (`0o77`), float, scientific notation
- **Comments**: line (`//`), doc (`///`), and nestable block (`/* ... */`)
- **Raw strings**: `r#"..."#`
- **Multi-line strings**: `"""..."""`
- **Standard library**: `std::math::sqrt`, `std::json::parse`, etc.

## Installation

### From Source (Development)

Symlink the extension into your VS Code extensions directory:

```bash
# Linux / macOS
ln -s /path/to/concerto-lang/editors/vscode ~/.vscode/extensions/concerto-lang-0.1.0

# Or copy it
cp -r /path/to/concerto-lang/editors/vscode ~/.vscode/extensions/concerto-lang-0.1.0
```

Then reload VS Code (`Ctrl+Shift+P` > "Developer: Reload Window").

## File Association

The extension automatically associates `.conc` files with the Concerto language. Open any `.conc` file and syntax highlighting will be applied.

## License

MIT
