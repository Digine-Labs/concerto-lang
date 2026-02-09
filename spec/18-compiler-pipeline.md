# 18 - Compiler Pipeline

## Overview

The Concerto compiler (`concertoc`) transforms `.conc` source files into `.conc-ir` files through a five-stage pipeline: Lexer, Parser, AST construction, Semantic Analysis, and IR Generation.

```
Source (.conc) -> Lexer -> Tokens -> Parser -> AST -> Semantic Analysis -> Typed AST -> IR Generator -> IR (.conc-ir)
```

## Stage 1: Lexer (Tokenization)

The lexer scans source text character by character, producing a stream of tokens.

### Input
Raw UTF-8 source text.

### Output
A sequence of `Token` values.

### Token Structure

```
struct Token {
    kind: TokenKind,     // What type of token
    lexeme: String,      // The actual text
    span: Span,          // Source position
}

struct Span {
    file: String,        // Source file path
    start: Position,     // Start position
    end: Position,       // End position
}

struct Position {
    line: u32,           // 1-based line number
    column: u32,         // 1-based column number
    offset: u32,         // 0-based byte offset
}
```

### Token Kinds

```
enum TokenKind {
    // Literals
    IntLiteral,          // 42, 0xFF, 0b1010
    FloatLiteral,        // 3.14, 1.5e10
    StringLiteral,       // "hello"
    RawStringLiteral,    // r#"raw"#
    MultiLineString,     // """multi"""
    BoolLiteral,         // true, false
    NilLiteral,          // nil

    // Identifiers and Keywords
    Identifier,          // variable_name, TypeName
    // Keywords (one variant per keyword):
    Let, Mut, Const, Fn, Model, Agent, Tool, Pub, Use, Mod,
    If, Else, Match, For, While, Loop, Break, Continue, Return,
    Try, Catch, Throw, Emit, Await, Async,
    Pipeline, Stage, Schema, Db, Connect,
    Self_, Impl, Trait, Enum, Struct, As, In, With,
    True, False, Nil, Type,

    // Operators
    Plus, Minus, Star, Slash, Percent,       // + - * / %
    EqualEqual, BangEqual,                    // == !=
    Less, Greater, LessEqual, GreaterEqual,  // < > <= >=
    AmpAmp, PipePipe, Bang,                  // && || !
    Equal, PlusEqual, MinusEqual,            // = += -=
    StarEqual, SlashEqual, PercentEqual,     // *= /= %=
    Arrow, FatArrow,                         // -> =>
    ColonColon, Dot, DotDot, DotDotEqual,   // :: . .. ..=
    Pipe, PipeGreater,                       // | |>
    Question, QuestionQuestion,              // ? ??
    At,                                      // @

    // Delimiters
    LeftParen, RightParen,                   // ( )
    LeftBrace, RightBrace,                   // { }
    LeftBracket, RightBracket,               // [ ]
    Comma, Semicolon, Colon,                 // , ; :

    // Special
    StringInterpolationStart,  // ${ within a string
    StringInterpolationEnd,    // } closing interpolation

    // Meta
    DocComment,          // /// doc comment
    EOF,                 // End of file
}
```

### Lexer Behavior

1. **Skip whitespace and regular comments** (line `//` and block `/* */`)
2. **Preserve doc comments** (`///`) as tokens for the parser
3. **Handle string interpolation** by emitting a sequence of tokens:
   - `"Hello, ${name}!"` becomes: `StringLiteral("Hello, ")`, `StringInterpolationStart`, `Identifier("name")`, `StringInterpolationEnd`, `StringLiteral("!")`
4. **Track source positions** for every token
5. **Report errors** for unterminated strings, invalid escape sequences, malformed numbers

### Error Recovery

On lexer errors, emit an error diagnostic and skip to the next recognizable token boundary. This allows the parser to continue and report multiple errors.

## Stage 2: Parser

The parser consumes the token stream and produces an Abstract Syntax Tree (AST).

### Parser Type
Recursive descent with **Pratt parsing** for expression precedence.

### Grammar (Simplified)

```
program         = declaration*
declaration     = connect_decl | db_decl | model_decl | agent_decl | tool_decl | schema_decl
                | pipeline_decl | fn_decl | struct_decl | enum_decl | trait_decl
                | impl_decl | use_decl | mod_decl | const_decl | type_alias

fn_decl         = decorator* "pub"? "async"? "fn" IDENT "(" params? ")" ("->" type)? block
model_decl      = decorator* "pub"? "model" IDENT "{" model_fields "}"
tool_decl       = "pub"? "tool" IDENT ("impl" IDENT)? "{" tool_body "}"
schema_decl     = "pub"? "schema" IDENT ("<" type_params ">")? "{" schema_fields "}"
connect_decl    = "connect" IDENT "{" connect_fields "}"
hashmap_decl    = "hashmap" IDENT ":" type "=" expr ";"
pipeline_decl   = "pipeline" IDENT "{" stage_decl+ "}"
struct_decl     = "pub"? "struct" IDENT "{" struct_fields "}"
enum_decl       = "pub"? "enum" IDENT "{" enum_variants "}"
trait_decl      = "pub"? "trait" IDENT "{" trait_methods "}"
impl_decl       = "impl" IDENT ("for" IDENT)? "{" fn_decl* "}"
use_decl        = "use" path ("::*" | "::{" import_list "}")? ";"
mod_decl        = "pub"? "mod" IDENT (";" | block)

stage_decl      = decorator* "stage" IDENT "(" params? ")" ("->" type)? block

statement       = let_stmt | expr_stmt | for_stmt | while_stmt | loop_stmt
                | if_expr | match_expr | try_expr | return_stmt | break_stmt
                | continue_stmt | emit_stmt

let_stmt        = "let" "mut"? pattern (":" type)? ("=" expr)? ";"
expr_stmt       = expr ";"
for_stmt        = "for" pattern "in" expr block
while_stmt      = "while" expr block
loop_stmt       = "loop" block
return_stmt     = "return" expr? ";"
break_stmt      = "break" (LABEL)? expr? ";"
continue_stmt   = "continue" (LABEL)? ";"
emit_stmt       = "emit" "(" expr ("," expr)? ")" ";"

expr            = assignment
assignment      = pipe_expr (("=" | "+=" | "-=" | "*=" | "/=" | "%=") expr)?
pipe_expr       = nil_coalesce ("|>" nil_coalesce)*
nil_coalesce    = logical_or ("??" logical_or)*
logical_or      = logical_and ("||" logical_and)*
logical_and     = equality ("&&" equality)*
equality        = comparison (("==" | "!=") comparison)*
comparison      = range (("<" | ">" | "<=" | ">=") range)*
range           = addition ((".." | "..=") addition)?
addition        = multiplication (("+" | "-") multiplication)*
multiplication  = unary (("*" | "/" | "%") unary)*
unary           = ("!" | "-") unary | postfix
postfix         = primary (call | index | member | "?" | ".await")*
primary         = literal | IDENT | "(" expr ")" | if_expr | match_expr
                | block | array_literal | map_literal | closure | try_expr

call            = "(" args? ")"
index           = "[" expr "]"
member          = "." IDENT | "::" IDENT
```

### Pratt Parsing for Expressions

The parser uses Pratt parsing (top-down operator precedence) for expressions. Each operator has a binding power that determines precedence:

```
fn parse_expression(min_bp: u8) -> Expr {
    let mut lhs = parse_prefix();  // Unary or primary

    while let Some(op) = peek_operator() {
        let (left_bp, right_bp) = operator_binding_power(op);
        if left_bp < min_bp { break; }
        advance();
        let rhs = parse_expression(right_bp);
        lhs = Expr::Binary(op, lhs, rhs);
    }

    lhs
}
```

### Error Recovery

The parser uses **synchronization** to recover from errors:
1. On error, emit diagnostic with source span
2. Skip tokens until a synchronization point (`;`, `}`, keyword)
3. Continue parsing from the synchronization point
4. Report all errors at the end (don't stop at first error)

## Stage 3: AST (Abstract Syntax Tree)

### Node Types

```
// Top-level declarations
Program { declarations: Vec<Declaration> }

enum Declaration {
    Function(FunctionDecl),
    Model(ModelDecl),
    Tool(ToolDecl),
    Schema(SchemaDecl),
    Connect(ConnectDecl),
    HashMap(HashMapDecl),
    Pipeline(PipelineDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplDecl),
    Use(UseDecl),
    Module(ModuleDecl),
    Const(ConstDecl),
    TypeAlias(TypeAliasDecl),
}

// Statements
enum Statement {
    Let(LetStmt),
    Expression(ExprStmt),
    For(ForStmt),
    While(WhileStmt),
    Loop(LoopStmt),
    Return(ReturnStmt),
    Break(BreakStmt),
    Continue(ContinueStmt),
    Emit(EmitStmt),
}

// Expressions
enum Expression {
    Literal(LiteralExpr),       // 42, "hello", true, nil
    Identifier(IdentExpr),       // variable_name
    Binary(BinaryExpr),          // a + b, a && b
    Unary(UnaryExpr),            // !x, -x
    Call(CallExpr),              // f(x, y)
    MethodCall(MethodCallExpr),  // obj.method(x)
    FieldAccess(FieldExpr),      // obj.field
    Index(IndexExpr),            // arr[0]
    If(IfExpr),                  // if c { a } else { b }
    Match(MatchExpr),            // match x { ... }
    Block(BlockExpr),            // { ... }
    Array(ArrayExpr),            // [1, 2, 3]
    Map(MapExpr),                // { "key": "value" }
    Tuple(TupleExpr),            // (a, b)
    Closure(ClosureExpr),        // |x| x + 1
    Pipe(PipeExpr),              // x |> f()
    Try(TryExpr),                // try { ... } catch { ... }
    Await(AwaitExpr),            // x.await
    Propagate(PropagateExpr),    // x?
    NilCoalesce(NilCoalesceExpr), // x ?? y
    StringInterpolation(InterpExpr), // "Hello ${name}"
    Range(RangeExpr),            // 1..10
    Cast(CastExpr),              // x as Float
    Path(PathExpr),              // std::json::parse
    Assign(AssignExpr),          // x = 5, x += 1
}
```

Every AST node carries a `Span` for error reporting.

## Stage 4: Semantic Analysis

Transforms the untyped AST into a **typed AST** by resolving names, checking types, and validating program semantics.

### Passes

#### 1. Name Resolution
- Build symbol tables for each scope
- Resolve identifiers to their declarations
- Resolve module paths (`use` imports)
- Detect undefined variables, functions, types
- Detect duplicate declarations in the same scope

#### 2. Type Checking
- Infer types for `let` bindings without annotations
- Check that expressions match expected types
- Verify function parameter and return types
- Check operator type compatibility
- Verify model field types
- Check schema field types
- Validate generic type parameters

#### 3. Scope Analysis
- Verify variable usage (used before initialization?)
- Check mutability (writing to immutable binding?)
- Detect unused variables (warning)
- Validate loop labels
- Check `break`/`continue` inside loops
- Check `return` inside functions
- Check `?` inside functions returning Result/Option

#### 4. Model / Agent / Tool / Schema Validation
- Verify models reference valid connections
- Verify models reference valid tools
- Verify models reference valid databases
- Validate schema fields are valid types
- Verify tool method signatures
- Check pipeline stage type compatibility (output of N matches input of N+1)

#### 5. Import Resolution
- Resolve `use` paths to source files
- Verify imported items are `pub`
- Detect circular dependencies
- Build dependency graph

### Error Reporting

Semantic errors include:
- Source span (file, line, column)
- Error message
- Suggestions (did-you-mean for typos)
- Related spans (where the conflicting declaration is)

```
error[E0301]: Type mismatch
  --> main.conc:15:20
   |
15 |     let x: Int = "hello";
   |            ---   ^^^^^^^ expected Int, found String
   |            |
   |            type annotation here
```

## Stage 5: IR Generation

Transforms the typed AST into IR instructions.

### Process

1. **Constant extraction**: Collect all literals into the constant pool
2. **Function lowering**: Convert each function body to IR instructions
3. **Model registration**: Serialize model definitions to IR model section
4. **Tool registration**: Serialize tool definitions with JSON schemas
5. **Schema compilation**: Convert schemas to JSON Schema format
6. **Pipeline lowering**: Convert pipeline stages to function-like instruction sequences
7. **Source map generation**: Record instruction-to-source mappings
8. **Output**: Write complete IR to JSON file

### Expression Lowering Example

Source:
```concerto
let result = a + b * c;
```

IR instructions:
```
LOAD_LOCAL "a"
LOAD_LOCAL "b"
LOAD_LOCAL "c"
MUL
ADD
STORE_LOCAL "result"
```

### Control Flow Lowering

Source:
```concerto
if condition {
    do_a();
} else {
    do_b();
}
```

IR instructions:
```
LOAD_LOCAL "condition"
JUMP_IF_FALSE else_label    // Jump to else if false
CALL "do_a" 0
JUMP end_label              // Skip else
// else_label:
CALL "do_b" 0
// end_label:
```

## Compiler CLI

```
# Compile a .conc file to IR
concertoc main.conc -o main.conc-ir

# Compile with debug info
concertoc main.conc -o main.conc-ir --debug

# Check for errors without generating IR
concertoc main.conc --check

# Show AST (debug)
concertoc main.conc --emit-ast

# Show tokens (debug)
concertoc main.conc --emit-tokens

# Compile and run (convenience -- invokes runtime)
concertoc main.conc --run
```

## Optimization (Future)

Planned optimizations (not in v1):
- **Constant folding**: Evaluate constant expressions at compile time
- **Dead code elimination**: Remove unreachable code
- **Inlining**: Inline small functions
- **Pipeline fusion**: Merge adjacent stages that don't need intermediate storage
