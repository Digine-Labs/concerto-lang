use concerto_common::{Position, Span};

/// Low-level character reader over source text.
///
/// Tracks current position (line, column, byte offset) and provides
/// peek/advance primitives for the lexer.
pub struct Cursor<'src> {
    source: &'src str,
    file: String,
    chars: std::str::Chars<'src>,
    /// Byte offset of the *next* character to be consumed.
    offset: u32,
    line: u32,
    column: u32,
}

impl<'src> Cursor<'src> {
    pub fn new(source: &'src str, file: impl Into<String>) -> Self {
        Self {
            source,
            file: file.into(),
            chars: source.chars(),
            offset: 0,
            line: 1,
            column: 1,
        }
    }

    /// Current position in the source.
    pub fn position(&self) -> Position {
        Position {
            line: self.line,
            column: self.column,
            offset: self.offset,
        }
    }

    /// Peek at the next character without consuming it.
    pub fn peek(&self) -> Option<char> {
        self.chars.clone().next()
    }

    /// Peek at the character after the next one.
    pub fn peek_second(&self) -> Option<char> {
        let mut iter = self.chars.clone();
        iter.next();
        iter.next()
    }

    /// Consume and return the next character.
    pub fn advance(&mut self) -> Option<char> {
        let ch = self.chars.next()?;
        self.offset += ch.len_utf8() as u32;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    /// Consume the next character if it matches `expected`.
    pub fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// True if there are no more characters.
    pub fn is_eof(&self) -> bool {
        self.peek().is_none()
    }

    /// Slice the source from byte offset `start` to `end`.
    pub fn slice(&self, start: u32, end: u32) -> &'src str {
        &self.source[start as usize..end as usize]
    }

    /// Slice the source from byte offset `start` to the current offset.
    pub fn slice_from(&self, start: u32) -> &'src str {
        self.slice(start, self.offset)
    }

    /// Build a Span from a start position to the current position.
    pub fn span_from(&self, start: Position) -> Span {
        Span::new(self.file.clone(), start, self.position())
    }

    /// Consume characters while `predicate` returns true.
    pub fn eat_while(&mut self, predicate: impl Fn(char) -> bool) {
        while let Some(ch) = self.peek() {
            if predicate(ch) {
                self.advance();
            } else {
                break;
            }
        }
    }
}
