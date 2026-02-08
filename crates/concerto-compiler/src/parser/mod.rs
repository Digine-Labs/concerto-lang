mod declarations;
mod expressions;
mod statements;

use concerto_common::{DiagnosticBag, Span};

use crate::ast::*;
use crate::lexer::token::{Token, TokenKind};

/// Recursive descent parser for the Concerto language.
///
/// Uses Pratt parsing for expression precedence.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: DiagnosticBag,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            diagnostics: DiagnosticBag::new(),
        }
    }

    /// Parse the entire token stream into a Program.
    pub fn parse(mut self) -> (Program, DiagnosticBag) {
        let mut declarations = Vec::new();
        let start = self.current_span();

        while !self.is_at_end() {
            match self.parse_declaration() {
                Some(decl) => declarations.push(decl),
                None => {
                    // Error recovery: skip to next synchronization point
                    self.synchronize();
                }
            }
        }

        let end = self.current_span();
        let span = start.merge(&end);
        let program = Program {
            declarations,
            span,
        };
        (program, self.diagnostics)
    }

    // ========================================================================
    // Token manipulation helpers
    // ========================================================================

    /// Peek at the current token kind.
    fn peek(&self) -> TokenKind {
        self.tokens
            .get(self.pos)
            .map(|t| t.kind)
            .unwrap_or(TokenKind::Eof)
    }

    /// Peek at the next token kind (one ahead).
    fn peek_next(&self) -> TokenKind {
        self.tokens
            .get(self.pos + 1)
            .map(|t| t.kind)
            .unwrap_or(TokenKind::Eof)
    }

    /// Peek at a token kind N positions ahead.
    fn peek_at(&self, offset: usize) -> TokenKind {
        self.tokens
            .get(self.pos + offset)
            .map(|t| t.kind)
            .unwrap_or(TokenKind::Eof)
    }

    /// Get the current token.
    fn current(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .unwrap_or_else(|| self.tokens.last().unwrap())
    }

    /// Get the previous token (the one just consumed).
    fn previous(&self) -> &Token {
        &self.tokens[self.pos.saturating_sub(1)]
    }

    /// Advance past the current token and return it.
    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.pos += 1;
        }
        self.previous()
    }

    /// Consume a token of the expected kind, or report an error.
    fn expect(&mut self, kind: TokenKind) -> Option<&Token> {
        if self.peek() == kind {
            self.advance();
            Some(self.previous())
        } else {
            let span = self.current_span();
            self.diagnostics.error(
                format!("expected {:?}, found {:?}", kind, self.peek()),
                span,
            );
            None
        }
    }

    /// Consume if the current token matches, otherwise do nothing.
    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.peek() == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Check if the parser has reached EOF.
    fn is_at_end(&self) -> bool {
        self.peek() == TokenKind::Eof
    }

    /// Get the span of the current token.
    fn current_span(&self) -> Span {
        self.current().span.clone()
    }

    /// Get the span of the previous token.
    fn previous_span(&self) -> Span {
        self.previous().span.clone()
    }

    /// Error recovery: skip tokens until we find a synchronization point.
    fn synchronize(&mut self) {
        self.advance();
        while !self.is_at_end() {
            // After a semicolon, we're at a statement boundary
            if self.previous().kind == TokenKind::Semicolon {
                return;
            }
            // These token kinds start new declarations/statements
            match self.peek() {
                TokenKind::Fn
                | TokenKind::Pub
                | TokenKind::Let
                | TokenKind::If
                | TokenKind::Match
                | TokenKind::For
                | TokenKind::While
                | TokenKind::Loop
                | TokenKind::Return
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Throw
                | TokenKind::Try
                | TokenKind::Emit
                | TokenKind::Agent
                | TokenKind::Tool
                | TokenKind::Schema
                | TokenKind::Pipeline
                | TokenKind::Struct
                | TokenKind::Enum
                | TokenKind::Trait
                | TokenKind::Impl
                | TokenKind::Use
                | TokenKind::Mod
                | TokenKind::Const
                | TokenKind::Type
                | TokenKind::HashMap
                | TokenKind::Memory
                | TokenKind::Mcp
                | TokenKind::Host
                | TokenKind::At
                | TokenKind::RightBrace => return,
                _ => {
                    self.advance();
                }
            }
        }
    }
}
