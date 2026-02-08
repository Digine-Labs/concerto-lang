use concerto_common::{DiagnosticBag, Position};

use super::cursor::Cursor;
use super::token::{Token, TokenKind};

/// String delimiter type for tracking multi-line vs regular strings.
#[derive(Debug, Clone, Copy)]
enum StringDelimiter {
    /// Regular double-quoted string: `"..."`
    Double,
    /// Triple-quoted multi-line string: `"""..."""`
    TripleDouble,
}

/// Lexer mode for string interpolation tracking.
#[derive(Debug, Clone, Copy)]
enum LexerMode {
    /// Inside a `${...}` interpolation expression within a string.
    Interpolation {
        brace_depth: u32,
        delimiter: StringDelimiter,
    },
}

/// Hand-written lexer for the Concerto language.
///
/// Supports: all 43 keywords, all operators, string interpolation,
/// multi-line strings, raw strings, hex/binary/octal integers,
/// unicode escapes, nested block comments, doc comments.
pub struct Lexer<'src> {
    cursor: Cursor<'src>,
    diagnostics: DiagnosticBag,
    mode_stack: Vec<LexerMode>,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str, file: impl Into<String>) -> Self {
        Self {
            cursor: Cursor::new(source, file),
            diagnostics: DiagnosticBag::new(),
            mode_stack: Vec::new(),
        }
    }

    /// Tokenize the entire source, returning all tokens and diagnostics.
    pub fn tokenize(mut self) -> (Vec<Token>, DiagnosticBag) {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token();
            let is_eof = token.kind == TokenKind::Eof;
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        (tokens, self.diagnostics)
    }

    /// Scan the next token.
    fn next_token(&mut self) -> Token {
        self.skip_whitespace_and_comments();

        if self.cursor.is_eof() {
            let pos = self.cursor.position();
            return Token::eof(self.cursor.span_from(pos));
        }

        let start = self.cursor.position();
        let ch = self.cursor.advance().unwrap();

        match ch {
            // === Delimiters ===
            '(' => self.make_token(TokenKind::LeftParen, start),
            ')' => self.make_token(TokenKind::RightParen, start),
            '{' => {
                // Track brace depth for string interpolation
                if let Some(LexerMode::Interpolation {
                    ref mut brace_depth,
                    ..
                }) = self.mode_stack.last_mut()
                {
                    *brace_depth += 1;
                }
                self.make_token(TokenKind::LeftBrace, start)
            }
            '}' => {
                // Check if this closes a string interpolation
                let continue_delimiter = match self.mode_stack.last() {
                    Some(LexerMode::Interpolation {
                        brace_depth: 0,
                        delimiter,
                    }) => Some(*delimiter),
                    _ => None,
                };

                if let Some(delimiter) = continue_delimiter {
                    // End of interpolation — resume string scanning
                    self.mode_stack.pop();
                    return self.scan_string_content(start, delimiter, false);
                }

                // Decrement brace depth if in interpolation
                if let Some(LexerMode::Interpolation {
                    ref mut brace_depth,
                    ..
                }) = self.mode_stack.last_mut()
                {
                    *brace_depth -= 1;
                }

                self.make_token(TokenKind::RightBrace, start)
            }
            '[' => self.make_token(TokenKind::LeftBracket, start),
            ']' => self.make_token(TokenKind::RightBracket, start),
            ',' => self.make_token(TokenKind::Comma, start),
            ';' => self.make_token(TokenKind::Semicolon, start),
            ':' => {
                if self.cursor.eat(':') {
                    self.make_token(TokenKind::ColonColon, start)
                } else {
                    self.make_token(TokenKind::Colon, start)
                }
            }
            '@' => self.make_token(TokenKind::At, start),

            // === Operators (multi-char disambiguation) ===
            '+' => {
                if self.cursor.eat('=') {
                    self.make_token(TokenKind::PlusEqual, start)
                } else {
                    self.make_token(TokenKind::Plus, start)
                }
            }
            '-' => {
                if self.cursor.eat('>') {
                    self.make_token(TokenKind::Arrow, start)
                } else if self.cursor.eat('=') {
                    self.make_token(TokenKind::MinusEqual, start)
                } else {
                    self.make_token(TokenKind::Minus, start)
                }
            }
            '*' => {
                if self.cursor.eat('=') {
                    self.make_token(TokenKind::StarEqual, start)
                } else {
                    self.make_token(TokenKind::Star, start)
                }
            }
            '/' => {
                if self.cursor.eat('=') {
                    self.make_token(TokenKind::SlashEqual, start)
                } else {
                    self.make_token(TokenKind::Slash, start)
                }
            }
            '%' => {
                if self.cursor.eat('=') {
                    self.make_token(TokenKind::PercentEqual, start)
                } else {
                    self.make_token(TokenKind::Percent, start)
                }
            }
            '=' => {
                if self.cursor.eat('=') {
                    self.make_token(TokenKind::EqualEqual, start)
                } else if self.cursor.eat('>') {
                    self.make_token(TokenKind::FatArrow, start)
                } else {
                    self.make_token(TokenKind::Equal, start)
                }
            }
            '!' => {
                if self.cursor.eat('=') {
                    self.make_token(TokenKind::BangEqual, start)
                } else {
                    self.make_token(TokenKind::Bang, start)
                }
            }
            '<' => {
                if self.cursor.eat('=') {
                    self.make_token(TokenKind::LessEqual, start)
                } else {
                    self.make_token(TokenKind::Less, start)
                }
            }
            '>' => {
                if self.cursor.eat('=') {
                    self.make_token(TokenKind::GreaterEqual, start)
                } else {
                    self.make_token(TokenKind::Greater, start)
                }
            }
            '&' => {
                if self.cursor.eat('&') {
                    self.make_token(TokenKind::AmpAmp, start)
                } else {
                    let span = self.cursor.span_from(start);
                    self.diagnostics
                        .error("unexpected character '&'; did you mean '&&'?", span.clone());
                    self.make_token_with_span(TokenKind::AmpAmp, start, span)
                }
            }
            '|' => {
                if self.cursor.eat('|') {
                    self.make_token(TokenKind::PipePipe, start)
                } else if self.cursor.eat('>') {
                    self.make_token(TokenKind::PipeGreater, start)
                } else {
                    self.make_token(TokenKind::Pipe, start)
                }
            }
            '?' => {
                if self.cursor.eat('?') {
                    self.make_token(TokenKind::QuestionQuestion, start)
                } else {
                    self.make_token(TokenKind::Question, start)
                }
            }
            '.' => {
                if self.cursor.eat('.') {
                    if self.cursor.eat('=') {
                        self.make_token(TokenKind::DotDotEqual, start)
                    } else {
                        self.make_token(TokenKind::DotDot, start)
                    }
                } else {
                    self.make_token(TokenKind::Dot, start)
                }
            }

            // === String literals ===
            '"' => self.scan_string(start),

            // === Number literals ===
            c if c.is_ascii_digit() => self.scan_number(start, c),

            // === Raw strings (must check before identifier fallback) ===
            'r' if self.cursor.peek() == Some('#') => self.scan_raw_string(start),

            // === Identifiers and keywords ===
            c if is_ident_start(c) => self.scan_identifier(start),

            _ => {
                let span = self.cursor.span_from(start);
                self.diagnostics
                    .error(format!("unexpected character '{}'", ch), span.clone());
                Token::new(TokenKind::Eof, ch.to_string(), span)
            }
        }
    }

    // ---------------------------------------------------------------
    // Whitespace & comments
    // ---------------------------------------------------------------

    /// Skip whitespace and comments.
    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            self.cursor.eat_while(|c| c.is_ascii_whitespace());

            // Check for comments
            if self.cursor.peek() == Some('/') {
                match self.cursor.peek_second() {
                    Some('/') => {
                        self.cursor.advance(); // first /
                        self.cursor.advance(); // second /

                        // Doc comment (///) — skip for now, emit as DocComment later
                        if self.cursor.peek() == Some('/')
                            && self.cursor.peek_second() != Some('/')
                        {
                            self.cursor.eat_while(|c| c != '\n');
                            continue;
                        }

                        // Regular line comment
                        self.cursor.eat_while(|c| c != '\n');
                        continue;
                    }
                    Some('*') => {
                        // Block comment (with nesting support)
                        self.cursor.advance(); // /
                        self.cursor.advance(); // *
                        self.skip_block_comment();
                        continue;
                    }
                    _ => {}
                }
            }

            break;
        }
    }

    /// Skip a block comment, supporting nesting.
    fn skip_block_comment(&mut self) {
        let mut depth: u32 = 1;
        while depth > 0 {
            match self.cursor.advance() {
                Some('/') if self.cursor.peek() == Some('*') => {
                    self.cursor.advance();
                    depth += 1;
                }
                Some('*') if self.cursor.peek() == Some('/') => {
                    self.cursor.advance();
                    depth -= 1;
                }
                Some(_) => {}
                None => {
                    let pos = self.cursor.position();
                    let span = self.cursor.span_from(pos);
                    self.diagnostics
                        .error("unterminated block comment", span);
                    return;
                }
            }
        }
    }

    // ---------------------------------------------------------------
    // String scanning
    // ---------------------------------------------------------------

    /// Entry point for string scanning after the opening `"` is consumed.
    fn scan_string(&mut self, start: Position) -> Token {
        // Check for multi-line string """
        if self.cursor.peek() == Some('"') && self.cursor.peek_second() == Some('"') {
            self.cursor.advance(); // second "
            self.cursor.advance(); // third "
            return self.scan_string_content(start, StringDelimiter::TripleDouble, true);
        }
        self.scan_string_content(start, StringDelimiter::Double, true)
    }

    /// Core string content scanner. Handles escape sequences, interpolation,
    /// and both regular (`"`) and multi-line (`"""`) delimiters.
    ///
    /// - `is_start`: true if this is the beginning of a string (opening `"` or `"""`).
    ///   false if resuming after a `}` that closed an interpolation.
    fn scan_string_content(
        &mut self,
        start: Position,
        delimiter: StringDelimiter,
        is_start: bool,
    ) -> Token {
        let mut value = String::new();

        loop {
            match self.cursor.advance() {
                // --- String interpolation: ${...} ---
                Some('$') if self.cursor.peek() == Some('{') => {
                    self.cursor.advance(); // consume '{'
                    self.mode_stack.push(LexerMode::Interpolation {
                        brace_depth: 0,
                        delimiter,
                    });
                    let kind = if is_start {
                        TokenKind::InterpolStringStart
                    } else {
                        TokenKind::InterpolStringMid
                    };
                    let span = self.cursor.span_from(start);
                    return Token::new(kind, value, span);
                }

                // --- Closing delimiter ---
                Some('"') => match delimiter {
                    StringDelimiter::Double => {
                        let kind = if is_start {
                            TokenKind::StringLiteral
                        } else {
                            TokenKind::InterpolStringEnd
                        };
                        let span = self.cursor.span_from(start);
                        return Token::new(kind, value, span);
                    }
                    StringDelimiter::TripleDouble => {
                        // Need two more " to close
                        if self.cursor.peek() == Some('"')
                            && self.cursor.peek_second() == Some('"')
                        {
                            self.cursor.advance(); // second "
                            self.cursor.advance(); // third "
                            let kind = if is_start {
                                TokenKind::StringLiteral
                            } else {
                                TokenKind::InterpolStringEnd
                            };
                            let span = self.cursor.span_from(start);
                            return Token::new(kind, value, span);
                        }
                        // Single/double " inside triple-quoted string is content
                        value.push('"');
                    }
                },

                // --- Escape sequences ---
                Some('\\') => match self.cursor.advance() {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some('r') => value.push('\r'),
                    Some('\\') => value.push('\\'),
                    Some('"') => value.push('"'),
                    Some('0') => value.push('\0'),
                    Some('$') => value.push('$'),
                    Some('u') => self.scan_unicode_escape(&mut value, start),
                    Some(c) => {
                        let span = self.cursor.span_from(start);
                        self.diagnostics.error(
                            format!("unknown escape sequence '\\{}'", c),
                            span,
                        );
                        value.push(c);
                    }
                    None => {
                        let span = self.cursor.span_from(start);
                        self.diagnostics
                            .error("unterminated string literal", span.clone());
                        return Token::new(TokenKind::StringLiteral, value, span);
                    }
                },

                // --- Newline in regular string is an error ---
                Some('\n') if matches!(delimiter, StringDelimiter::Double) => {
                    let span = self.cursor.span_from(start);
                    self.diagnostics.error(
                        "unterminated string literal (newline in string; use \"\"\" for multi-line)",
                        span.clone(),
                    );
                    return Token::new(TokenKind::StringLiteral, value, span);
                }

                // --- Regular character ---
                Some(c) => value.push(c),

                // --- EOF ---
                None => {
                    let span = self.cursor.span_from(start);
                    self.diagnostics
                        .error("unterminated string literal", span.clone());
                    return Token::new(TokenKind::StringLiteral, value, span);
                }
            }
        }
    }

    /// Parse a unicode escape sequence: `\u{XXXX}`.
    /// Called after `\u` has been consumed.
    fn scan_unicode_escape(&mut self, value: &mut String, string_start: Position) {
        if !self.cursor.eat('{') {
            let span = self.cursor.span_from(string_start);
            self.diagnostics
                .error("expected '{' after '\\u'", span);
            return;
        }

        let hex_start = self.cursor.position();
        self.cursor.eat_while(|c| c.is_ascii_hexdigit());
        let hex = self.cursor.slice_from(hex_start.offset);

        if !self.cursor.eat('}') {
            let span = self.cursor.span_from(string_start);
            self.diagnostics
                .error("expected '}' to close unicode escape", span);
            return;
        }

        if hex.is_empty() {
            let span = self.cursor.span_from(string_start);
            self.diagnostics.error("empty unicode escape", span);
            return;
        }

        match u32::from_str_radix(hex, 16) {
            Ok(code_point) => {
                if let Some(ch) = char::from_u32(code_point) {
                    value.push(ch);
                } else {
                    let span = self.cursor.span_from(string_start);
                    self.diagnostics.error(
                        format!("invalid unicode code point: U+{:04X}", code_point),
                        span,
                    );
                }
            }
            Err(_) => {
                let span = self.cursor.span_from(string_start);
                self.diagnostics
                    .error(format!("invalid hex in unicode escape: {}", hex), span);
            }
        }
    }

    /// Scan a raw string literal: `r#"..."#` (one or more `#`).
    /// Called after `r` has been consumed and cursor is at the first `#`.
    fn scan_raw_string(&mut self, start: Position) -> Token {
        // Count # characters
        let mut hash_count = 0u32;
        while self.cursor.eat('#') {
            hash_count += 1;
        }

        // Expect opening "
        if !self.cursor.eat('"') {
            let span = self.cursor.span_from(start);
            self.diagnostics
                .error("expected '\"' after raw string prefix", span.clone());
            return Token::new(TokenKind::StringLiteral, String::new(), span);
        }

        // Scan until " followed by hash_count # characters
        let mut value = String::new();
        loop {
            match self.cursor.advance() {
                Some('"') => {
                    // Check for matching # count
                    let mut found = 0u32;
                    while found < hash_count && self.cursor.peek() == Some('#') {
                        self.cursor.advance();
                        found += 1;
                    }
                    if found == hash_count {
                        // Closing delimiter found
                        let span = self.cursor.span_from(start);
                        return Token::new(TokenKind::StringLiteral, value, span);
                    }
                    // Not enough #s — the " and partial #s are content
                    value.push('"');
                    for _ in 0..found {
                        value.push('#');
                    }
                }
                Some(c) => value.push(c),
                None => {
                    let span = self.cursor.span_from(start);
                    self.diagnostics
                        .error("unterminated raw string literal", span.clone());
                    return Token::new(TokenKind::StringLiteral, value, span);
                }
            }
        }
    }

    // ---------------------------------------------------------------
    // Number scanning
    // ---------------------------------------------------------------

    /// Scan a number literal (integer or float).
    /// Supports decimal, hex (0x), binary (0b), octal (0o), underscores,
    /// floats with scientific notation.
    fn scan_number(&mut self, start: Position, first: char) -> Token {
        // Check for hex/binary/octal prefix
        if first == '0' {
            match self.cursor.peek() {
                Some('x') | Some('X') => {
                    self.cursor.advance(); // consume prefix
                    let digit_start = self.cursor.position();
                    self.cursor
                        .eat_while(|c| c.is_ascii_hexdigit() || c == '_');
                    if self.cursor.position().offset == digit_start.offset {
                        let span = self.cursor.span_from(start);
                        self.diagnostics
                            .error("expected hex digits after '0x'", span.clone());
                        return Token::new(TokenKind::IntLiteral, "0x", span);
                    }
                    let lexeme = self.cursor.slice_from(start.offset);
                    let span = self.cursor.span_from(start);
                    return Token::new(TokenKind::IntLiteral, lexeme, span);
                }
                Some('b') | Some('B') => {
                    self.cursor.advance();
                    let digit_start = self.cursor.position();
                    self.cursor
                        .eat_while(|c| c == '0' || c == '1' || c == '_');
                    if self.cursor.position().offset == digit_start.offset {
                        let span = self.cursor.span_from(start);
                        self.diagnostics
                            .error("expected binary digits after '0b'", span.clone());
                        return Token::new(TokenKind::IntLiteral, "0b", span);
                    }
                    let lexeme = self.cursor.slice_from(start.offset);
                    let span = self.cursor.span_from(start);
                    return Token::new(TokenKind::IntLiteral, lexeme, span);
                }
                Some('o') | Some('O') => {
                    self.cursor.advance();
                    let digit_start = self.cursor.position();
                    self.cursor
                        .eat_while(|c| ('0'..='7').contains(&c) || c == '_');
                    if self.cursor.position().offset == digit_start.offset {
                        let span = self.cursor.span_from(start);
                        self.diagnostics
                            .error("expected octal digits after '0o'", span.clone());
                        return Token::new(TokenKind::IntLiteral, "0o", span);
                    }
                    let lexeme = self.cursor.slice_from(start.offset);
                    let span = self.cursor.span_from(start);
                    return Token::new(TokenKind::IntLiteral, lexeme, span);
                }
                _ => {} // fall through to decimal
            }
        }

        // Decimal integer (continue consuming digits)
        self.cursor.eat_while(|c| c.is_ascii_digit() || c == '_');

        // Check for float (decimal point followed by digit)
        let is_float = self.cursor.peek() == Some('.')
            && self.cursor
                .peek_second()
                .is_some_and(|c| c.is_ascii_digit());

        if is_float {
            self.cursor.advance(); // consume '.'
            self.cursor.eat_while(|c| c.is_ascii_digit() || c == '_');

            // Scientific notation
            if matches!(self.cursor.peek(), Some('e' | 'E')) {
                self.cursor.advance();
                if matches!(self.cursor.peek(), Some('+' | '-')) {
                    self.cursor.advance();
                }
                self.cursor.eat_while(|c| c.is_ascii_digit() || c == '_');
            }

            let lexeme = self.cursor.slice_from(start.offset);
            let span = self.cursor.span_from(start);
            Token::new(TokenKind::FloatLiteral, lexeme, span)
        } else {
            let lexeme = self.cursor.slice_from(start.offset);
            let span = self.cursor.span_from(start);
            Token::new(TokenKind::IntLiteral, lexeme, span)
        }
    }

    // ---------------------------------------------------------------
    // Identifier / keyword scanning
    // ---------------------------------------------------------------

    /// Scan an identifier or keyword.
    fn scan_identifier(&mut self, start: Position) -> Token {
        self.cursor.eat_while(is_ident_continue);
        let lexeme = self.cursor.slice_from(start.offset);
        let span = self.cursor.span_from(start);

        let kind = match lexeme {
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "nil" => TokenKind::Nil,
            other => TokenKind::keyword_from_str(other).unwrap_or(TokenKind::Identifier),
        };

        Token::new(kind, lexeme, span)
    }

    // ---------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------

    /// Create a token using the slice from `start` to current position.
    fn make_token(&self, kind: TokenKind, start: Position) -> Token {
        let lexeme = self.cursor.slice_from(start.offset);
        let span = self.cursor.span_from(start);
        Token::new(kind, lexeme, span)
    }

    fn make_token_with_span(
        &self,
        kind: TokenKind,
        start: Position,
        span: concerto_common::Span,
    ) -> Token {
        let lexeme = self.cursor.slice_from(start.offset);
        Token::new(kind, lexeme, span)
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(source: &str) -> Vec<Token> {
        let (tokens, diags) = Lexer::new(source, "test.conc").tokenize();
        assert!(
            !diags.has_errors(),
            "unexpected errors: {:?}",
            diags.diagnostics()
        );
        tokens
    }

    fn lex_kinds(source: &str) -> Vec<TokenKind> {
        lex(source).into_iter().map(|t| t.kind).collect()
    }

    fn lex_with_errors(source: &str) -> (Vec<Token>, DiagnosticBag) {
        Lexer::new(source, "test.conc").tokenize()
    }

    // =====================================================================
    // Existing tests (preserved from core subset)
    // =====================================================================

    #[test]
    fn empty_source() {
        let kinds = lex_kinds("");
        assert_eq!(kinds, vec![TokenKind::Eof]);
    }

    #[test]
    fn simple_let() {
        let kinds = lex_kinds("let x = 5;");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Let,
                TokenKind::Identifier,
                TokenKind::Equal,
                TokenKind::IntLiteral,
                TokenKind::Semicolon,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn operators() {
        let kinds = lex_kinds("+ - * / % == != < > <= >= && || ! = |> ?? -> =>");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Slash,
                TokenKind::Percent,
                TokenKind::EqualEqual,
                TokenKind::BangEqual,
                TokenKind::Less,
                TokenKind::Greater,
                TokenKind::LessEqual,
                TokenKind::GreaterEqual,
                TokenKind::AmpAmp,
                TokenKind::PipePipe,
                TokenKind::Bang,
                TokenKind::Equal,
                TokenKind::PipeGreater,
                TokenKind::QuestionQuestion,
                TokenKind::Arrow,
                TokenKind::FatArrow,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn keywords() {
        let kinds = lex_kinds("fn let mut if else return emit true false nil");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Fn,
                TokenKind::Let,
                TokenKind::Mut,
                TokenKind::If,
                TokenKind::Else,
                TokenKind::Return,
                TokenKind::Emit,
                TokenKind::True,
                TokenKind::False,
                TokenKind::Nil,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn string_literal() {
        let tokens = lex(r#""hello world""#);
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme, "hello world");
    }

    #[test]
    fn string_escape_sequences() {
        let tokens = lex(r#""hello\nworld\t!""#);
        assert_eq!(tokens[0].lexeme, "hello\nworld\t!");
    }

    #[test]
    fn float_literal() {
        let tokens = lex("3.14");
        assert_eq!(tokens[0].kind, TokenKind::FloatLiteral);
        assert_eq!(tokens[0].lexeme, "3.14");
    }

    #[test]
    fn integer_with_underscores() {
        let tokens = lex("1_000_000");
        assert_eq!(tokens[0].kind, TokenKind::IntLiteral);
        assert_eq!(tokens[0].lexeme, "1_000_000");
    }

    #[test]
    fn line_comment_skipped() {
        let kinds = lex_kinds("x // comment\ny");
        assert_eq!(
            kinds,
            vec![TokenKind::Identifier, TokenKind::Identifier, TokenKind::Eof]
        );
    }

    #[test]
    fn block_comment_skipped() {
        let kinds = lex_kinds("x /* block */ y");
        assert_eq!(
            kinds,
            vec![TokenKind::Identifier, TokenKind::Identifier, TokenKind::Eof]
        );
    }

    #[test]
    fn nested_block_comment() {
        let kinds = lex_kinds("x /* outer /* inner */ still outer */ y");
        assert_eq!(
            kinds,
            vec![TokenKind::Identifier, TokenKind::Identifier, TokenKind::Eof]
        );
    }

    #[test]
    fn delimiters() {
        let kinds = lex_kinds("(){}[],;:");
        assert_eq!(
            kinds,
            vec![
                TokenKind::LeftParen,
                TokenKind::RightParen,
                TokenKind::LeftBrace,
                TokenKind::RightBrace,
                TokenKind::LeftBracket,
                TokenKind::RightBracket,
                TokenKind::Comma,
                TokenKind::Semicolon,
                TokenKind::Colon,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn dot_operators() {
        let kinds = lex_kinds(". .. ..=");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Dot,
                TokenKind::DotDot,
                TokenKind::DotDotEqual,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn compound_assignment() {
        let kinds = lex_kinds("+= -= *= /= %=");
        assert_eq!(
            kinds,
            vec![
                TokenKind::PlusEqual,
                TokenKind::MinusEqual,
                TokenKind::StarEqual,
                TokenKind::SlashEqual,
                TokenKind::PercentEqual,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn full_function() {
        let source = r#"
fn main() {
    let x = 5;
    let y = x + 3;
    if y > 7 {
        emit("result", y);
    }
}
"#;
        let kinds = lex_kinds(source);
        assert_eq!(
            kinds,
            vec![
                TokenKind::Fn,
                TokenKind::Identifier, // main
                TokenKind::LeftParen,
                TokenKind::RightParen,
                TokenKind::LeftBrace,
                TokenKind::Let,
                TokenKind::Identifier, // x
                TokenKind::Equal,
                TokenKind::IntLiteral, // 5
                TokenKind::Semicolon,
                TokenKind::Let,
                TokenKind::Identifier, // y
                TokenKind::Equal,
                TokenKind::Identifier, // x
                TokenKind::Plus,
                TokenKind::IntLiteral, // 3
                TokenKind::Semicolon,
                TokenKind::If,
                TokenKind::Identifier, // y
                TokenKind::Greater,
                TokenKind::IntLiteral, // 7
                TokenKind::LeftBrace,
                TokenKind::Emit,
                TokenKind::LeftParen,
                TokenKind::StringLiteral, // "result"
                TokenKind::Comma,
                TokenKind::Identifier, // y
                TokenKind::RightParen,
                TokenKind::Semicolon,
                TokenKind::RightBrace,
                TokenKind::RightBrace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn span_tracking() {
        let tokens = lex("let x = 5;");
        assert_eq!(tokens[0].span.start.line, 1);
        assert_eq!(tokens[0].span.start.column, 1);
        assert_eq!(tokens[1].span.start.line, 1);
        assert_eq!(tokens[1].span.start.column, 5);
    }

    // =====================================================================
    // New tests for Step 7: full coverage
    // =====================================================================

    // --- mcp keyword ---

    #[test]
    fn mcp_keyword() {
        let kinds = lex_kinds("mcp GitHubServer { }");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Mcp,
                TokenKind::Identifier,
                TokenKind::LeftBrace,
                TokenKind::RightBrace,
                TokenKind::Eof,
            ]
        );
    }

    // --- Hex/binary/octal integers ---

    #[test]
    fn hex_integer() {
        let tokens = lex("0xFF");
        assert_eq!(tokens[0].kind, TokenKind::IntLiteral);
        assert_eq!(tokens[0].lexeme, "0xFF");
    }

    #[test]
    fn hex_integer_uppercase() {
        let tokens = lex("0XAB");
        assert_eq!(tokens[0].kind, TokenKind::IntLiteral);
        assert_eq!(tokens[0].lexeme, "0XAB");
    }

    #[test]
    fn binary_integer() {
        let tokens = lex("0b1010");
        assert_eq!(tokens[0].kind, TokenKind::IntLiteral);
        assert_eq!(tokens[0].lexeme, "0b1010");
    }

    #[test]
    fn octal_integer() {
        let tokens = lex("0o77");
        assert_eq!(tokens[0].kind, TokenKind::IntLiteral);
        assert_eq!(tokens[0].lexeme, "0o77");
    }

    #[test]
    fn hex_with_underscores() {
        let tokens = lex("0xFF_FF");
        assert_eq!(tokens[0].kind, TokenKind::IntLiteral);
        assert_eq!(tokens[0].lexeme, "0xFF_FF");
    }

    #[test]
    fn binary_with_underscores() {
        let tokens = lex("0b1010_0101");
        assert_eq!(tokens[0].kind, TokenKind::IntLiteral);
        assert_eq!(tokens[0].lexeme, "0b1010_0101");
    }

    #[test]
    fn zero_followed_by_dot_is_float() {
        let tokens = lex("0.5");
        assert_eq!(tokens[0].kind, TokenKind::FloatLiteral);
        assert_eq!(tokens[0].lexeme, "0.5");
    }

    #[test]
    fn hex_missing_digits() {
        let (tokens, diags) = lex_with_errors("0x");
        assert!(diags.has_errors());
        assert_eq!(tokens[0].kind, TokenKind::IntLiteral);
    }

    // --- String interpolation ---

    #[test]
    fn string_interpolation_simple() {
        let tokens = lex(r#""Hello ${name}!""#);
        assert_eq!(tokens[0].kind, TokenKind::InterpolStringStart);
        assert_eq!(tokens[0].lexeme, "Hello ");
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
        assert_eq!(tokens[1].lexeme, "name");
        assert_eq!(tokens[2].kind, TokenKind::InterpolStringEnd);
        assert_eq!(tokens[2].lexeme, "!");
    }

    #[test]
    fn string_interpolation_at_start() {
        let tokens = lex(r#""${x} done""#);
        assert_eq!(tokens[0].kind, TokenKind::InterpolStringStart);
        assert_eq!(tokens[0].lexeme, "");
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
        assert_eq!(tokens[2].kind, TokenKind::InterpolStringEnd);
        assert_eq!(tokens[2].lexeme, " done");
    }

    #[test]
    fn string_interpolation_at_end() {
        let tokens = lex(r#""value: ${x}""#);
        assert_eq!(tokens[0].kind, TokenKind::InterpolStringStart);
        assert_eq!(tokens[0].lexeme, "value: ");
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
        assert_eq!(tokens[2].kind, TokenKind::InterpolStringEnd);
        assert_eq!(tokens[2].lexeme, "");
    }

    #[test]
    fn string_interpolation_multiple() {
        let tokens = lex(r#""${x} + ${y} = ${z}""#);
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::InterpolStringStart,
                TokenKind::Identifier, // x
                TokenKind::InterpolStringMid,
                TokenKind::Identifier, // y
                TokenKind::InterpolStringMid,
                TokenKind::Identifier, // z
                TokenKind::InterpolStringEnd,
                TokenKind::Eof,
            ]
        );
        assert_eq!(tokens[0].lexeme, "");       // before x
        assert_eq!(tokens[2].lexeme, " + ");    // between x and y
        assert_eq!(tokens[4].lexeme, " = ");    // between y and z
        assert_eq!(tokens[6].lexeme, "");        // after z
    }

    #[test]
    fn string_interpolation_with_expression() {
        let tokens = lex(r#""result: ${x + 3}""#);
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::InterpolStringStart,
                TokenKind::Identifier, // x
                TokenKind::Plus,
                TokenKind::IntLiteral, // 3
                TokenKind::InterpolStringEnd,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn string_interpolation_with_braces_in_expression() {
        // Braces inside interpolation (e.g., map literal) should be tracked
        let tokens = lex(r#""val: ${ {1: 2} }""#);
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::InterpolStringStart,
                TokenKind::LeftBrace,
                TokenKind::IntLiteral,
                TokenKind::Colon,
                TokenKind::IntLiteral,
                TokenKind::RightBrace,
                TokenKind::InterpolStringEnd,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn string_escaped_dollar() {
        // \$ prevents interpolation
        let tokens = lex(r#""price is \${50}""#);
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme, "price is ${50}");
    }

    #[test]
    fn string_dollar_without_brace() {
        // $ not followed by { is just a $
        let tokens = lex(r#""price is $50""#);
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme, "price is $50");
    }

    // --- Multi-line strings ---

    #[test]
    fn multiline_string_basic() {
        let tokens = lex("\"\"\"hello\nworld\"\"\"");
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme, "hello\nworld");
    }

    #[test]
    fn multiline_string_with_quotes() {
        let tokens = lex("\"\"\"she said \"hi\" to me\"\"\"");
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme, "she said \"hi\" to me");
    }

    #[test]
    fn multiline_string_empty() {
        let tokens = lex("\"\"\"\"\"\"");
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme, "");
    }

    #[test]
    fn multiline_string_with_interpolation() {
        let tokens = lex("\"\"\"hello ${name}!\"\"\"");
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::InterpolStringStart,
                TokenKind::Identifier,
                TokenKind::InterpolStringEnd,
                TokenKind::Eof,
            ]
        );
        assert_eq!(tokens[0].lexeme, "hello ");
        assert_eq!(tokens[2].lexeme, "!");
    }

    // --- Raw strings ---

    #[test]
    fn raw_string_basic() {
        let tokens = lex(r###"r#"hello\nworld"#"###);
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme, "hello\\nworld"); // No escape processing
    }

    #[test]
    fn raw_string_with_quotes() {
        let tokens = lex(r####"r##"say "hello" please"##"####);
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme, r#"say "hello" please"#);
    }

    #[test]
    fn raw_string_no_interpolation() {
        let tokens = lex(r###"r#"${not_interp}"#"###);
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme, "${not_interp}");
    }

    #[test]
    fn raw_string_with_inner_hash_quote() {
        // To include "# in content, use r##"..."##
        let tokens = lex(r####"r##"has "# inside"##"####);
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme, r##"has "# inside"##);
    }

    // --- Unicode escape ---

    #[test]
    fn unicode_escape_basic() {
        let tokens = lex(r#""\u{0041}""#);
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme, "A"); // U+0041 = 'A'
    }

    #[test]
    fn unicode_escape_emoji() {
        let tokens = lex(r#""\u{1F600}""#);
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[0].lexeme, "\u{1F600}");
    }

    #[test]
    fn unicode_escape_in_string() {
        let tokens = lex(r#""Hello \u{0041}\u{0042}""#);
        assert_eq!(tokens[0].lexeme, "Hello \u{0041}\u{0042}");
    }

    // --- Edge cases ---

    #[test]
    fn plain_string_no_interpolation() {
        // A string with no ${} should be a plain StringLiteral
        let tokens = lex(r#""hello world""#);
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
    }

    #[test]
    fn r_identifier_not_raw_string() {
        // 'r' not followed by '#' is just an identifier
        let kinds = lex_kinds("r + 1");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Identifier,
                TokenKind::Plus,
                TokenKind::IntLiteral,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn all_keywords_recognized() {
        let source = "let mut const fn agent tool pub use mod if else match for while loop break continue return try catch throw emit await async pipeline stage schema hashmap ledger self impl trait enum struct as in with true false nil type mcp";
        let tokens = lex(source);
        // All should be keywords (not Identifier), except true/false/nil which are literals
        for token in &tokens[..tokens.len() - 1] {
            assert_ne!(
                token.kind,
                TokenKind::Identifier,
                "expected keyword, got Identifier for {:?}",
                token.lexeme
            );
        }
        // 42 keywords + Eof (removed 'connect', added 'ledger' which was missing from test)
        assert_eq!(tokens.len(), 43);
    }
}
