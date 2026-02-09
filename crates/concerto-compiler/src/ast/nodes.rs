use concerto_common::Span;

use super::types::TypeAnnotation;

// ============================================================================
// Program (top-level)
// ============================================================================

/// A complete Concerto program.
#[derive(Debug, Clone)]
pub struct Program {
    pub declarations: Vec<Declaration>,
    pub span: Span,
}

// ============================================================================
// Decorators
// ============================================================================

/// A decorator: `@name` or `@name(args)`.
///
/// ```concerto
/// @describe("Search the web for information")
/// @param("query", "The search query string")
/// @timeout(seconds: 60)
/// ```
#[derive(Debug, Clone)]
pub struct Decorator {
    pub name: String,
    pub args: Vec<DecoratorArg>,
    pub span: Span,
}

/// A single argument in a decorator.
#[derive(Debug, Clone)]
pub enum DecoratorArg {
    /// Positional argument: `@describe("text")`
    Positional(Expr),
    /// Named argument: `@timeout(seconds: 60)`
    Named {
        name: String,
        value: Expr,
        span: Span,
    },
}

// ============================================================================
// Declarations
// ============================================================================

/// A top-level declaration.
#[derive(Debug, Clone)]
pub enum Declaration {
    Function(FunctionDecl),
    Agent(AgentDecl),
    Tool(ToolDecl),
    Schema(SchemaDecl),
    Pipeline(PipelineDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplDecl),
    Use(UseDecl),
    Module(ModuleDecl),
    Const(ConstDecl),
    TypeAlias(TypeAliasDecl),
    HashMap(HashMapDecl),
    Ledger(LedgerDecl),
    Memory(MemoryDecl),
    Mcp(McpDecl),
    Host(HostDecl),
}

// ============================================================================
// Function declaration
// ============================================================================

/// A function declaration (also used for methods in tool/trait/impl/mcp blocks).
///
/// ```concerto
/// @describe("Do something")
/// pub async fn name(self, param: Type) -> ReturnType { body }
/// ```
#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub name: String,
    pub decorators: Vec<Decorator>,
    pub self_param: SelfParam,
    pub params: Vec<Param>,
    pub return_type: Option<TypeAnnotation>,
    pub body: Option<Block>,
    pub is_public: bool,
    pub is_async: bool,
    pub span: Span,
}

/// Whether a method has a `self` parameter and its mutability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfParam {
    None,
    Immutable, // self
    Mutable,   // mut self
}

/// A function parameter.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub type_ann: Option<TypeAnnotation>,
    pub default: Option<Expr>,
    pub span: Span,
}

// ============================================================================
// Agent declaration
// ============================================================================

/// ```concerto
/// @log(channel: "debug")
/// agent Name {
///     provider: openai,
///     model: "gpt-4o",
///     tools: [Tool1, Tool2],
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AgentDecl {
    pub name: String,
    pub decorators: Vec<Decorator>,
    pub fields: Vec<ConfigField>,
    pub span: Span,
}

// ============================================================================
// Tool declaration
// ============================================================================

/// ```concerto
/// tool Name {
///     description: "...",
///     @describe("...") @param("x", "...")
///     pub fn method(self, x: Int) -> Result<Int, ToolError> { ... }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ToolDecl {
    pub name: String,
    pub fields: Vec<ConfigField>,
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

// ============================================================================
// Schema declaration
// ============================================================================

/// ```concerto
/// schema Name {
///     field: Type,
///     optional?: Type,
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SchemaDecl {
    pub name: String,
    pub decorators: Vec<Decorator>,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

// ============================================================================
// Pipeline / Stage declaration
// ============================================================================

/// ```concerto
/// pipeline Name {
///     stage step1(input: String) -> Int { ... }
///     stage step2(n: Int) -> String { ... }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PipelineDecl {
    pub name: String,
    pub stages: Vec<StageDecl>,
    pub span: Span,
    /// Optional pipeline-level input parameter (e.g., `pipeline P(input: String)`).
    pub input_param: Option<Param>,
    /// Optional pipeline-level return type (e.g., `pipeline P(...) -> Output`).
    pub return_type: Option<TypeAnnotation>,
}

/// A single stage within a pipeline.
#[derive(Debug, Clone)]
pub struct StageDecl {
    pub name: String,
    pub decorators: Vec<Decorator>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeAnnotation>,
    pub body: Block,
    pub span: Span,
}

// ============================================================================
// Struct declaration
// ============================================================================

/// ```concerto
/// struct Name {
///     pub field: Type,
///     other: Type,
/// }
/// ```
#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

// ============================================================================
// Enum declaration
// ============================================================================

/// ```concerto
/// enum Name {
///     Unit,
///     Tuple(Int, String),
///     Struct { field: Type },
/// }
/// ```
#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub kind: EnumVariantKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum EnumVariantKind {
    Unit,
    Tuple(Vec<TypeAnnotation>),
    Struct(Vec<FieldDecl>),
}

// ============================================================================
// Trait declaration
// ============================================================================

/// ```concerto
/// trait Name {
///     fn method(self) -> Type;
///     fn with_default(self) -> Type { ... }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TraitDecl {
    pub name: String,
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

// ============================================================================
// Impl declaration
// ============================================================================

/// ```concerto
/// impl Type { pub fn method(self) { ... } }
/// impl Trait for Type { fn method(self) { ... } }
/// ```
#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub target: String,
    pub trait_name: Option<String>,
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

// ============================================================================
// Use / Module declarations
// ============================================================================

/// `use path::to::item [as alias];`
#[derive(Debug, Clone)]
pub struct UseDecl {
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub span: Span,
}

/// `mod name { ... }` or `mod name;`
#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub name: String,
    pub body: Option<Vec<Declaration>>,
    pub span: Span,
}

// ============================================================================
// Const / Type alias declarations
// ============================================================================

/// `const NAME: Type = value;`
#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub type_ann: TypeAnnotation,
    pub value: Expr,
    pub span: Span,
}

/// `type Name = Type;`
#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub name: String,
    pub type_ann: TypeAnnotation,
    pub span: Span,
}

// ============================================================================
// HashMap declaration
// ============================================================================

/// `hashmap name: Type = expr;`
#[derive(Debug, Clone)]
pub struct HashMapDecl {
    pub name: String,
    pub type_ann: TypeAnnotation,
    pub initializer: Expr,
    pub span: Span,
}

// ============================================================================
// Ledger declaration
// ============================================================================

/// `ledger name: Ledger = Ledger::new();`
#[derive(Debug, Clone)]
pub struct LedgerDecl {
    pub name: String,
    pub type_ann: TypeAnnotation,
    pub initializer: Expr,
    pub span: Span,
}

// ============================================================================
// Memory declaration
// ============================================================================

/// `memory name: Memory = Memory::new();`
#[derive(Debug, Clone)]
pub struct MemoryDecl {
    pub name: String,
    pub type_ann: TypeAnnotation,
    pub initializer: Expr,
    pub span: Span,
}

// ============================================================================
// MCP declaration
// ============================================================================

/// ```concerto
/// mcp ServerName {
///     transport: "stdio",
///     command: "npx ...",
///     @describe("...") fn tool_name(p: Type) -> Result<T, E>;
/// }
/// ```
#[derive(Debug, Clone)]
pub struct McpDecl {
    pub name: String,
    pub fields: Vec<ConfigField>,
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

// ============================================================================
// Host declaration
// ============================================================================

/// ```concerto
/// host ClaudeCode {
///     connector: claude_code,
///     input_format: "text",
///     output_format: "json",
///     timeout: 300,
/// }
/// ```
#[derive(Debug, Clone)]
pub struct HostDecl {
    pub name: String,
    pub decorators: Vec<Decorator>,
    pub fields: Vec<ConfigField>,
    pub span: Span,
}

// ============================================================================
// Shared field types
// ============================================================================

/// A key-value config field: `name: expr` (used in agent, tool, mcp).
#[derive(Debug, Clone)]
pub struct ConfigField {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

/// A typed field: `[pub] name[?]: Type [= default]` (used in struct, schema).
#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub name: String,
    pub type_ann: TypeAnnotation,
    pub default: Option<Expr>,
    pub is_public: bool,
    pub is_optional: bool,
    pub span: Span,
}

// ============================================================================
// Patterns (used in match arms, for loops, destructuring)
// ============================================================================

/// A pattern node.
#[derive(Debug, Clone)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

/// All pattern variants.
#[derive(Debug, Clone)]
pub enum PatternKind {
    /// Wildcard: `_`
    Wildcard,

    /// Literal: `42`, `"hello"`, `true`, `nil`
    Literal(Literal),

    /// Identifier (variable binding): `x`
    Identifier(String),

    /// Tuple destructure: `(a, b, c)`
    Tuple(Vec<Pattern>),

    /// Struct destructure: `Point { x, y }` or `Point { x: a, .. }`
    Struct {
        path: Vec<String>,
        fields: Vec<PatternField>,
        has_rest: bool,
    },

    /// Enum variant: `Some(v)`, `Shape::Circle(r)`, `Direction::North`
    Enum {
        path: Vec<String>,
        fields: Vec<Pattern>,
    },

    /// Array destructure: `[first, second, ..rest]`
    Array {
        elements: Vec<Pattern>,
        rest: Option<String>,
    },

    /// Or pattern: `A | B | C`
    Or(Vec<Pattern>),

    /// Binding with pattern: `n @ 1..=5`
    Binding { name: String, pattern: Box<Pattern> },

    /// Range pattern: `1..=5` (only in match)
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
    },

    /// Rest pattern: `..` (used in struct/array contexts)
    Rest,
}

/// A field in a struct pattern: `x` (shorthand) or `x: pattern`.
#[derive(Debug, Clone)]
pub struct PatternField {
    pub name: String,
    pub pattern: Option<Pattern>,
    pub span: Span,
}

/// A match arm: `pattern [if guard] => body`.
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

/// A handler arm within a listen expression.
/// `"message_type" => |param: Type| { body }`
#[derive(Debug, Clone)]
pub struct ListenHandler {
    pub message_type: String,
    pub param: Param,
    pub body: Block,
    pub span: Span,
}

/// A catch clause: `catch [ErrorType(binding)] { body }`.
#[derive(Debug, Clone)]
pub struct CatchClause {
    pub error_type: Option<TypeAnnotation>,
    pub binding: Option<String>,
    pub body: Block,
    pub span: Span,
}

/// A part of a string interpolation.
#[derive(Debug, Clone)]
pub enum StringPart {
    Literal(String),
    Expr(Box<Expr>),
}

/// A field in a struct literal: `name: expr`.
#[derive(Debug, Clone)]
pub struct StructLiteralField {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

// ============================================================================
// Statements
// ============================================================================

/// A statement within a block.
#[derive(Debug, Clone)]
pub enum Stmt {
    Let(LetStmt),
    Expr(ExprStmt),
    Return(ReturnStmt),
    Break(BreakStmt),
    Continue(ContinueStmt),
    Throw(ThrowStmt),
    Mock(MockStmt),
}

/// `let [mut] name [: Type] = expr;`
#[derive(Debug, Clone)]
pub struct LetStmt {
    pub name: String,
    pub mutable: bool,
    pub type_ann: Option<TypeAnnotation>,
    pub initializer: Option<Expr>,
    pub span: Span,
}

/// An expression used as a statement (e.g., `foo();`)
#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}

/// `return [expr];`
#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

/// `break [value];` or `break 'label;`
#[derive(Debug, Clone)]
pub struct BreakStmt {
    pub label: Option<String>,
    pub value: Option<Expr>,
    pub span: Span,
}

/// `continue;` or `continue 'label;`
#[derive(Debug, Clone)]
pub struct ContinueStmt {
    pub label: Option<String>,
    pub span: Span,
}

/// `throw expr;` (sugar for `return Err(expr)`)
#[derive(Debug, Clone)]
pub struct ThrowStmt {
    pub value: Expr,
    pub span: Span,
}

/// A mock statement: `mock AgentName { response: "...", }`.
/// Only valid inside test blocks.
#[derive(Debug, Clone)]
pub struct MockStmt {
    pub agent_name: String,
    pub fields: Vec<ConfigField>,
    pub span: Span,
}

// ============================================================================
// Expressions
// ============================================================================

/// An expression node.
#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// All expression variants.
#[derive(Debug, Clone)]
pub enum ExprKind {
    /// A literal value: `42`, `3.14`, `"hello"`, `true`, `nil`
    Literal(Literal),

    /// A variable reference: `x`, `foo`
    Identifier(String),

    /// Binary operation: `a + b`, `x == y`
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },

    /// Unary operation: `-x`, `!flag`
    Unary { op: UnaryOp, operand: Box<Expr> },

    /// Function or method call: `foo(a, b)`, `emit("ch", val)`
    Call { callee: Box<Expr>, args: Vec<Expr> },

    /// `if condition { then } else { else }`
    If {
        condition: Box<Expr>,
        then_branch: Block,
        else_branch: Option<ElseBranch>,
    },

    /// A block expression: `{ stmts... expr? }`
    Block(Block),

    /// Assignment: `x = 5`, `x += 1`
    Assign {
        target: Box<Expr>,
        op: AssignOp,
        value: Box<Expr>,
    },

    /// Field access: `obj.field`
    FieldAccess { object: Box<Expr>, field: String },

    /// Method call: `obj.method(args)` or `obj.method<Type>(args)`
    MethodCall {
        object: Box<Expr>,
        method: String,
        type_args: Vec<super::types::TypeAnnotation>,
        args: Vec<Expr>,
    },

    /// Index access: `arr[0]`
    Index { object: Box<Expr>, index: Box<Expr> },

    /// Array literal: `[1, 2, 3]`
    Array(Vec<Expr>),

    /// Map literal: `{ "key": value }`
    Map(Vec<(Expr, Expr)>),

    /// Grouping (parenthesized expression, transparent after parsing).
    Grouping(Box<Expr>),

    // === New expression variants (Step 9) ===
    /// Match expression: `match expr { pattern => body, ... }`
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },

    /// Try/catch expression: `try { body } catch ErrorType(e) { handler }`
    TryCatch {
        body: Block,
        catches: Vec<CatchClause>,
    },

    /// For loop: `for pattern in iterable { body }`
    For {
        pattern: Pattern,
        iterable: Box<Expr>,
        body: Block,
    },

    /// While loop: `while condition { body }`
    While { condition: Box<Expr>, body: Block },

    /// Infinite loop: `loop { body }`
    Loop { body: Block },

    /// Closure: `|params| expr` or `|params| -> Type { block }`
    Closure {
        params: Vec<Param>,
        return_type: Option<TypeAnnotation>,
        body: Box<Expr>,
    },

    /// Pipe: `expr |> func(args)`
    Pipe { left: Box<Expr>, right: Box<Expr> },

    /// Error propagation: `expr?`
    Propagate(Box<Expr>),

    /// Nil coalesce: `expr ?? default`
    NilCoalesce { left: Box<Expr>, right: Box<Expr> },

    /// Range: `start..end` or `start..=end`
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },

    /// Type cast: `expr as Type`
    Cast {
        expr: Box<Expr>,
        target: TypeAnnotation,
    },

    /// Path expression: `a::b::c`
    Path(Vec<String>),

    /// Await: `expr.await`
    Await(Box<Expr>),

    /// Tuple: `(a, b, c)` or `(a,)` or `()`
    Tuple(Vec<Expr>),

    /// Struct literal: `Point { x: 1, y: 2 }`
    StructLiteral {
        name: Vec<String>,
        fields: Vec<StructLiteralField>,
    },

    /// String interpolation: `"Hello ${name}!"`
    StringInterpolation(Vec<StringPart>),

    /// Return expression: `return expr` (used in expression position, e.g., match arms)
    Return(Option<Box<Expr>>),

    /// Listen expression: `listen Host.execute("prompt") { "type" => |p| { ... }, ... }`
    Listen {
        call: Box<Expr>,
        handlers: Vec<ListenHandler>,
    },
}

/// A literal value.
#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Nil,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Comparison
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    // Logical
    And,
    Or,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

/// Assignment operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,    // =
    AddAssign, // +=
    SubAssign, // -=
    MulAssign, // *=
    DivAssign, // /=
    ModAssign, // %=
}

// ============================================================================
// Block
// ============================================================================

/// A block of statements, optionally ending with a trailing expression
/// (whose value becomes the block's value).
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    /// Trailing expression (no semicolon) -- the block's value.
    pub tail_expr: Option<Box<Expr>>,
    pub span: Span,
}

/// An else branch can be another block or an else-if.
#[derive(Debug, Clone)]
pub enum ElseBranch {
    Block(Block),
    ElseIf(Box<Expr>), // The inner Expr is an ExprKind::If
}
