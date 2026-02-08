/// Source position within a file (1-based line/column, 0-based byte offset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Position {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub column: u32,
    /// 0-based byte offset from start of file.
    pub offset: u32,
}

/// A range in source code, from `start` to `end` in a given file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Span {
    /// Source file path (interned or shared in practice).
    pub file: String,
    /// Start position (inclusive).
    pub start: Position,
    /// End position (exclusive).
    pub end: Position,
}

impl Span {
    pub fn new(file: impl Into<String>, start: Position, end: Position) -> Self {
        Self {
            file: file.into(),
            start,
            end,
        }
    }

    /// Create a dummy span for compiler-generated nodes.
    pub fn dummy() -> Self {
        Self {
            file: String::new(),
            start: Position::default(),
            end: Position::default(),
        }
    }

    /// Merge two spans into one that covers both (same file assumed).
    pub fn merge(&self, other: &Span) -> Span {
        let start = if self.start.offset <= other.start.offset {
            self.start
        } else {
            other.start
        };
        let end = if self.end.offset >= other.end.offset {
            self.end
        } else {
            other.end
        };
        Span {
            file: self.file.clone(),
            start,
            end,
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.file, self.start.line, self.start.column
        )
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}
