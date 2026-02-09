use crate::ast::*;
use crate::lexer::token::TokenKind;

use super::Parser;

impl Parser {
    /// Parse a statement within a block.
    pub(super) fn parse_statement(&mut self) -> Option<Stmt> {
        match self.peek() {
            TokenKind::Let => self.parse_let_stmt(),
            TokenKind::Return => self.parse_return_stmt(),
            TokenKind::Break => self.parse_break_stmt(),
            TokenKind::Continue => self.parse_continue_stmt(),
            TokenKind::Throw => self.parse_throw_stmt(),
            TokenKind::Mock => self.parse_mock_stmt(),
            _ => self.parse_expr_stmt(),
        }
    }

    /// Parse `let [mut] name [: Type] [= expr];`
    fn parse_let_stmt(&mut self) -> Option<Stmt> {
        let start = self.current_span();
        self.advance(); // consume 'let'

        let mutable = self.eat(TokenKind::Mut);

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        // Optional type annotation
        let type_ann = if self.eat(TokenKind::Colon) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        // Optional initializer
        let initializer = if self.eat(TokenKind::Equal) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        self.expect(TokenKind::Semicolon)?;
        let span = start.merge(&self.previous_span());

        Some(Stmt::Let(LetStmt {
            name,
            mutable,
            type_ann,
            initializer,
            span,
        }))
    }

    /// Parse `return [expr];`
    fn parse_return_stmt(&mut self) -> Option<Stmt> {
        let start = self.current_span();
        self.advance(); // consume 'return'

        let value = if self.peek() != TokenKind::Semicolon {
            Some(self.parse_expression()?)
        } else {
            None
        };

        self.expect(TokenKind::Semicolon)?;
        let span = start.merge(&self.previous_span());

        Some(Stmt::Return(ReturnStmt { value, span }))
    }

    /// Parse `break [expr];`
    fn parse_break_stmt(&mut self) -> Option<Stmt> {
        let start = self.current_span();
        self.advance(); // consume 'break'

        // Optional value expression (for loop expressions)
        let value = if self.peek() != TokenKind::Semicolon && self.peek() != TokenKind::RightBrace {
            Some(self.parse_expression()?)
        } else {
            None
        };

        self.expect(TokenKind::Semicolon)?;
        let span = start.merge(&self.previous_span());

        Some(Stmt::Break(BreakStmt {
            label: None,
            value,
            span,
        }))
    }

    /// Parse `continue;`
    fn parse_continue_stmt(&mut self) -> Option<Stmt> {
        let start = self.current_span();
        self.advance(); // consume 'continue'

        self.expect(TokenKind::Semicolon)?;
        let span = start.merge(&self.previous_span());

        Some(Stmt::Continue(ContinueStmt { label: None, span }))
    }

    /// Parse `throw expr;`
    fn parse_throw_stmt(&mut self) -> Option<Stmt> {
        let start = self.current_span();
        self.advance(); // consume 'throw'

        let value = self.parse_expression()?;

        self.expect(TokenKind::Semicolon)?;
        let span = start.merge(&self.previous_span());

        Some(Stmt::Throw(ThrowStmt { value, span }))
    }

    /// Parse `mock AgentName { response: "...", }`.
    fn parse_mock_stmt(&mut self) -> Option<Stmt> {
        let start = self.current_span();
        self.advance(); // consume 'mock'

        let name_token = self.expect(TokenKind::Identifier)?;
        let agent_name = name_token.lexeme.clone();

        let fields = self.parse_config_fields()?;
        let span = start.merge(&self.previous_span());

        Some(Stmt::Mock(MockStmt {
            agent_name,
            fields,
            span,
        }))
    }

    /// Parse an expression statement: `expr;`
    fn parse_expr_stmt(&mut self) -> Option<Stmt> {
        let start = self.current_span();
        let expr = self.parse_expression()?;
        self.expect(TokenKind::Semicolon)?;
        let span = start.merge(&self.previous_span());
        Some(Stmt::Expr(ExprStmt { expr, span }))
    }

    /// Parse a block: `{ stmt* [expr] }`
    ///
    /// If the last item is an expression without a trailing semicolon,
    /// it becomes the block's tail expression (its value).
    ///
    /// Block-ending expressions (if, match, for, while, loop, try/catch)
    /// do not require a semicolon when used as statements.
    pub(super) fn parse_block(&mut self) -> Option<Block> {
        let start = self.current_span();
        self.expect(TokenKind::LeftBrace)?;

        let mut stmts = Vec::new();
        let mut tail_expr: Option<Box<Expr>> = None;

        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            match self.peek() {
                // Statement keywords
                TokenKind::Let
                | TokenKind::Return
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Throw
                | TokenKind::Mock => {
                    if let Some(stmt) = self.parse_statement() {
                        stmts.push(stmt);
                    } else {
                        self.synchronize();
                    }
                }
                _ => {
                    // Could be expression statement or tail expression
                    let expr = match self.parse_expression() {
                        Some(e) => e,
                        None => {
                            self.synchronize();
                            continue;
                        }
                    };

                    if self.eat(TokenKind::Semicolon) {
                        // Expression statement with explicit semicolon
                        let span = expr.span.clone();
                        stmts.push(Stmt::Expr(ExprStmt { expr, span }));
                    } else if self.peek() == TokenKind::RightBrace {
                        // Tail expression (no semicolon before closing brace)
                        tail_expr = Some(Box::new(expr));
                    } else if is_block_expr(&expr) {
                        // Block-ending expressions don't need semicolons as statements
                        let span = expr.span.clone();
                        stmts.push(Stmt::Expr(ExprStmt { expr, span }));
                    } else {
                        // Missing semicolon
                        let span = self.current_span();
                        self.diagnostics.error(
                            format!("expected ';' or '}}', found {:?}", self.peek()),
                            span,
                        );
                        let expr_span = expr.span.clone();
                        stmts.push(Stmt::Expr(ExprStmt {
                            expr,
                            span: expr_span,
                        }));
                        self.synchronize();
                    }
                }
            }
        }

        self.expect(TokenKind::RightBrace)?;
        let span = start.merge(&self.previous_span());

        Some(Block {
            stmts,
            tail_expr,
            span,
        })
    }
}

/// Check if an expression ends with a block (and thus doesn't need `;`
/// when used as a statement in a block).
fn is_block_expr(expr: &Expr) -> bool {
    matches!(
        expr.kind,
        ExprKind::If { .. }
            | ExprKind::Block(_)
            | ExprKind::Match { .. }
            | ExprKind::For { .. }
            | ExprKind::While { .. }
            | ExprKind::Loop { .. }
            | ExprKind::TryCatch { .. }
    )
}
