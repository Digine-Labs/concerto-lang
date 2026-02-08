use concerto_common::Span;
use std::fmt;

/// A single token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, lexeme: impl Into<String>, span: Span) -> Self {
        Self {
            kind,
            lexeme: lexeme.into(),
            span,
        }
    }

    pub fn eof(span: Span) -> Self {
        Self {
            kind: TokenKind::Eof,
            lexeme: String::new(),
            span,
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}({})", self.kind, self.lexeme)
    }
}

/// All token kinds in the Concerto language.
///
/// See spec/01-lexical-structure.md and spec/18-compiler-pipeline.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // === Literals ===
    IntLiteral,
    FloatLiteral,
    StringLiteral,
    BoolLiteral,
    NilLiteral,

    // === Identifiers ===
    Identifier,

    // === Keywords ===
    Let,
    Mut,
    Const,
    Fn,
    Agent,
    Tool,
    Pub,
    Use,
    Mod,
    If,
    Else,
    Match,
    For,
    While,
    Loop,
    Break,
    Continue,
    Return,
    Try,
    Catch,
    Throw,
    Emit,
    Await,
    Async,
    Pipeline,
    Stage,
    Schema,
    HashMap,
    Host,
    Ledger,
    Memory,
    SelfKw,
    Impl,
    Trait,
    Enum,
    Struct,
    As,
    In,
    With,
    True,
    False,
    Nil,
    Type,
    Mcp,

    // === Operators ===
    Plus,          // +
    Minus,         // -
    Star,          // *
    Slash,         // /
    Percent,       // %
    EqualEqual,    // ==
    BangEqual,     // !=
    Less,          // <
    Greater,       // >
    LessEqual,     // <=
    GreaterEqual,  // >=
    AmpAmp,        // &&
    PipePipe,      // ||
    Bang,          // !
    Equal,         // =
    PlusEqual,     // +=
    MinusEqual,    // -=
    StarEqual,     // *=
    SlashEqual,    // /=
    PercentEqual,  // %=
    Arrow,         // ->
    FatArrow,      // =>
    ColonColon,    // ::
    Dot,           // .
    DotDot,        // ..
    DotDotEqual,   // ..=
    Pipe,          // |
    PipeGreater,   // |>
    Question,      // ?
    QuestionQuestion, // ??
    At,            // @

    // === Delimiters ===
    LeftParen,     // (
    RightParen,    // )
    LeftBrace,     // {
    RightBrace,    // }
    LeftBracket,   // [
    RightBracket,  // ]
    Comma,         // ,
    Semicolon,     // ;
    Colon,         // :

    // === String interpolation ===
    /// Start of an interpolated string: text from `"` up to first `${`.
    InterpolStringStart,
    /// Middle of an interpolated string: text between `}` and next `${`.
    InterpolStringMid,
    /// End of an interpolated string: text from `}` to closing `"`.
    InterpolStringEnd,

    // === Special ===
    DocComment,
    Eof,
}

impl TokenKind {
    /// Try to match an identifier string to a keyword.
    pub fn keyword_from_str(s: &str) -> Option<TokenKind> {
        match s {
            "let" => Some(TokenKind::Let),
            "mut" => Some(TokenKind::Mut),
            "const" => Some(TokenKind::Const),
            "fn" => Some(TokenKind::Fn),
            "agent" => Some(TokenKind::Agent),
            "tool" => Some(TokenKind::Tool),
            "pub" => Some(TokenKind::Pub),
            "use" => Some(TokenKind::Use),
            "mod" => Some(TokenKind::Mod),
            "if" => Some(TokenKind::If),
            "else" => Some(TokenKind::Else),
            "match" => Some(TokenKind::Match),
            "for" => Some(TokenKind::For),
            "while" => Some(TokenKind::While),
            "loop" => Some(TokenKind::Loop),
            "break" => Some(TokenKind::Break),
            "continue" => Some(TokenKind::Continue),
            "return" => Some(TokenKind::Return),
            "try" => Some(TokenKind::Try),
            "catch" => Some(TokenKind::Catch),
            "throw" => Some(TokenKind::Throw),
            "emit" => Some(TokenKind::Emit),
            "await" => Some(TokenKind::Await),
            "async" => Some(TokenKind::Async),
            "pipeline" => Some(TokenKind::Pipeline),
            "stage" => Some(TokenKind::Stage),
            "schema" => Some(TokenKind::Schema),
            "hashmap" => Some(TokenKind::HashMap),
            "host" => Some(TokenKind::Host),
            "ledger" => Some(TokenKind::Ledger),
            "memory" => Some(TokenKind::Memory),
            "self" => Some(TokenKind::SelfKw),
            "impl" => Some(TokenKind::Impl),
            "trait" => Some(TokenKind::Trait),
            "enum" => Some(TokenKind::Enum),
            "struct" => Some(TokenKind::Struct),
            "as" => Some(TokenKind::As),
            "in" => Some(TokenKind::In),
            "with" => Some(TokenKind::With),
            "true" => Some(TokenKind::True),
            "false" => Some(TokenKind::False),
            "nil" => Some(TokenKind::Nil),
            "type" => Some(TokenKind::Type),
            "mcp" => Some(TokenKind::Mcp),
            _ => None,
        }
    }
}
