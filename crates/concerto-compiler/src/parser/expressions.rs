use crate::ast::*;
use crate::lexer::token::TokenKind;

use super::Parser;

// ============================================================================
// Binding powers for Pratt parsing
// ============================================================================
//
// Higher values bind tighter. (left_bp, right_bp): left < right = left-assoc.
//
// Spec precedence (1=tightest → 14=loosest) mapped to binding powers:
//
//  Spec 14: Assignment  =, +=, etc.     (2, 1) right-assoc
//  Spec 12: Pipe        |>              (5, 6) left-assoc
//  Spec 11: NilCoalesce ??              (7, 8) left-assoc
//  Spec 10: Logical OR  ||              (9, 10) left-assoc
//  Spec 9:  Logical AND &&              (11, 12) left-assoc
//  Spec 8:  Equality    ==, !=          (13, 14) left-assoc
//  Spec 7:  Comparison  <, >, <=, >=    (15, 16) left-assoc
//  Spec 6:  Range       .., ..=         (17, 18)
//  Spec 5:  Additive    +, -            (19, 20) left-assoc
//  Spec 4:  Multiplicative *, /, %      (21, 22) left-assoc
//  Spec 3:  Cast        as              (23, 24) left-assoc
//  Spec 2:  Prefix      !, - (unary)    25
//  Spec 1:  Postfix     (), [], ., ::   handled in parse_postfix
//  Spec 13: Propagate   ? (postfix)     bp = 3 (above assignment, below pipe)

fn infix_binding_power(kind: TokenKind) -> Option<(u8, u8)> {
    match kind {
        // Assignment (right-associative)
        TokenKind::Equal
        | TokenKind::PlusEqual
        | TokenKind::MinusEqual
        | TokenKind::StarEqual
        | TokenKind::SlashEqual
        | TokenKind::PercentEqual => Some((2, 1)),

        // Pipe (left-associative)
        TokenKind::PipeGreater => Some((5, 6)),

        // Nil coalesce (left-associative)
        TokenKind::QuestionQuestion => Some((7, 8)),

        // Logical OR (left-associative)
        TokenKind::PipePipe => Some((9, 10)),

        // Logical AND (left-associative)
        TokenKind::AmpAmp => Some((11, 12)),

        // Equality (left-associative)
        TokenKind::EqualEqual | TokenKind::BangEqual => Some((13, 14)),

        // Comparison (left-associative)
        TokenKind::Less | TokenKind::Greater | TokenKind::LessEqual | TokenKind::GreaterEqual => {
            Some((15, 16))
        }

        // Range
        TokenKind::DotDot | TokenKind::DotDotEqual => Some((17, 18)),

        // Addition / Subtraction (left-associative)
        TokenKind::Plus | TokenKind::Minus => Some((19, 20)),

        // Multiplication / Division / Modulo (left-associative)
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Some((21, 22)),

        // Type cast (left-associative)
        TokenKind::As => Some((23, 24)),

        _ => None,
    }
}

/// Prefix binding power for unary operators.
fn prefix_binding_power(kind: TokenKind) -> Option<u8> {
    match kind {
        TokenKind::Minus | TokenKind::Bang => Some(25),
        _ => None,
    }
}

/// Postfix `?` binding power (above assignment, below pipe).
const PROPAGATE_BP: u8 = 3;

impl Parser {
    /// Parse an expression using Pratt parsing.
    pub(super) fn parse_expression(&mut self) -> Option<Expr> {
        self.parse_expr_bp(0)
    }

    /// Core Pratt parser: parse an expression with a minimum binding power.
    fn parse_expr_bp(&mut self, min_bp: u8) -> Option<Expr> {
        // Parse prefix (unary or primary)
        let mut lhs = self.parse_prefix()?;

        loop {
            // Check for postfix (call, member access, index, .await)
            lhs = self.parse_postfix(lhs);

            let op_kind = self.peek();

            // Postfix ? operator (error propagation)
            if op_kind == TokenKind::Question {
                if PROPAGATE_BP < min_bp {
                    break;
                }
                self.advance(); // consume '?'
                let span = lhs.span.merge(&self.previous_span());
                lhs = Expr::new(ExprKind::Propagate(Box::new(lhs)), span);
                continue;
            }

            // Check for infix operator
            if let Some((left_bp, right_bp)) = infix_binding_power(op_kind) {
                if left_bp < min_bp {
                    break;
                }
                self.advance(); // consume operator

                // Handle assignment operators
                if let Some(assign_op) = token_to_assign_op(op_kind) {
                    let rhs = self.parse_expr_bp(right_bp)?;
                    let span = lhs.span.merge(&rhs.span);
                    lhs = Expr::new(
                        ExprKind::Assign {
                            target: Box::new(lhs),
                            op: assign_op,
                            value: Box::new(rhs),
                        },
                        span,
                    );
                    continue;
                }

                // Pipe operator
                if op_kind == TokenKind::PipeGreater {
                    let rhs = self.parse_expr_bp(right_bp)?;
                    let span = lhs.span.merge(&rhs.span);
                    lhs = Expr::new(
                        ExprKind::Pipe {
                            left: Box::new(lhs),
                            right: Box::new(rhs),
                        },
                        span,
                    );
                    continue;
                }

                // Nil coalesce operator
                if op_kind == TokenKind::QuestionQuestion {
                    let rhs = self.parse_expr_bp(right_bp)?;
                    let span = lhs.span.merge(&rhs.span);
                    lhs = Expr::new(
                        ExprKind::NilCoalesce {
                            left: Box::new(lhs),
                            right: Box::new(rhs),
                        },
                        span,
                    );
                    continue;
                }

                // Range operators
                if op_kind == TokenKind::DotDot || op_kind == TokenKind::DotDotEqual {
                    let inclusive = op_kind == TokenKind::DotDotEqual;
                    // Right side is optional (e.g. `5..`)
                    let end = if can_start_expression(self.peek()) {
                        Some(Box::new(self.parse_expr_bp(right_bp)?))
                    } else {
                        None
                    };
                    let span = if let Some(ref e) = end {
                        lhs.span.merge(&e.span)
                    } else {
                        lhs.span.merge(&self.previous_span())
                    };
                    lhs = Expr::new(
                        ExprKind::Range {
                            start: Some(Box::new(lhs)),
                            end,
                            inclusive,
                        },
                        span,
                    );
                    continue;
                }

                // Type cast: `expr as Type`
                if op_kind == TokenKind::As {
                    let target = self.parse_type_annotation()?;
                    let span = lhs.span.merge(&target.span);
                    lhs = Expr::new(
                        ExprKind::Cast {
                            expr: Box::new(lhs),
                            target,
                        },
                        span,
                    );
                    continue;
                }

                // Regular binary operator
                let bin_op = token_to_binary_op(op_kind).unwrap();
                let rhs = self.parse_expr_bp(right_bp)?;
                let span = lhs.span.merge(&rhs.span);
                lhs = Expr::new(
                    ExprKind::Binary {
                        left: Box::new(lhs),
                        op: bin_op,
                        right: Box::new(rhs),
                    },
                    span,
                );
            } else {
                break;
            }
        }

        Some(lhs)
    }

    /// Parse a prefix expression (unary or primary).
    fn parse_prefix(&mut self) -> Option<Expr> {
        let kind = self.peek();

        // Unary prefix
        if let Some(bp) = prefix_binding_power(kind) {
            let start = self.current_span();
            self.advance(); // consume operator
            let op = match kind {
                TokenKind::Minus => UnaryOp::Neg,
                TokenKind::Bang => UnaryOp::Not,
                _ => unreachable!(),
            };
            let operand = self.parse_expr_bp(bp)?;
            let span = start.merge(&operand.span);
            return Some(Expr::new(
                ExprKind::Unary {
                    op,
                    operand: Box::new(operand),
                },
                span,
            ));
        }

        // Range prefix: `..end` or `..=end`
        if kind == TokenKind::DotDot || kind == TokenKind::DotDotEqual {
            let start = self.current_span();
            let inclusive = kind == TokenKind::DotDotEqual;
            self.advance();
            let end = if can_start_expression(self.peek()) {
                Some(Box::new(self.parse_expr_bp(18)?)) // use range right_bp
            } else {
                None
            };
            let span = if let Some(ref e) = end {
                start.merge(&e.span)
            } else {
                start
            };
            return Some(Expr::new(
                ExprKind::Range {
                    start: None,
                    end,
                    inclusive,
                },
                span,
            ));
        }

        // Primary
        self.parse_primary()
    }

    /// Parse a primary expression.
    fn parse_primary(&mut self) -> Option<Expr> {
        let start = self.current_span();
        match self.peek() {
            // Integer literal
            TokenKind::IntLiteral => {
                let token = self.advance().clone();
                let clean = token.lexeme.replace('_', "");
                let value: i64 = if clean.starts_with("0x") || clean.starts_with("0X") {
                    i64::from_str_radix(&clean[2..], 16).unwrap_or_else(|_| {
                        self.diagnostics.error(
                            format!("invalid hex literal '{}'", token.lexeme),
                            token.span.clone(),
                        );
                        0
                    })
                } else if clean.starts_with("0b") || clean.starts_with("0B") {
                    i64::from_str_radix(&clean[2..], 2).unwrap_or_else(|_| {
                        self.diagnostics.error(
                            format!("invalid binary literal '{}'", token.lexeme),
                            token.span.clone(),
                        );
                        0
                    })
                } else if clean.starts_with("0o") || clean.starts_with("0O") {
                    i64::from_str_radix(&clean[2..], 8).unwrap_or_else(|_| {
                        self.diagnostics.error(
                            format!("invalid octal literal '{}'", token.lexeme),
                            token.span.clone(),
                        );
                        0
                    })
                } else {
                    clean.parse().unwrap_or_else(|_| {
                        self.diagnostics.error(
                            format!("invalid integer literal '{}'", token.lexeme),
                            token.span.clone(),
                        );
                        0
                    })
                };
                Some(Expr::new(
                    ExprKind::Literal(Literal::Int(value)),
                    token.span,
                ))
            }

            // Float literal
            TokenKind::FloatLiteral => {
                let token = self.advance().clone();
                let clean = token.lexeme.replace('_', "");
                let value: f64 = clean.parse().unwrap_or_else(|_| {
                    self.diagnostics.error(
                        format!("invalid float literal '{}'", token.lexeme),
                        token.span.clone(),
                    );
                    0.0
                });
                Some(Expr::new(
                    ExprKind::Literal(Literal::Float(value)),
                    token.span,
                ))
            }

            // String literal
            TokenKind::StringLiteral => {
                let token = self.advance().clone();
                Some(Expr::new(
                    ExprKind::Literal(Literal::String(token.lexeme.clone())),
                    token.span,
                ))
            }

            // String interpolation: "Hello ${name}!"
            TokenKind::InterpolStringStart => self.parse_string_interpolation(),

            // Boolean literals
            TokenKind::True => {
                let token = self.advance().clone();
                Some(Expr::new(
                    ExprKind::Literal(Literal::Bool(true)),
                    token.span,
                ))
            }
            TokenKind::False => {
                let token = self.advance().clone();
                Some(Expr::new(
                    ExprKind::Literal(Literal::Bool(false)),
                    token.span,
                ))
            }

            // Nil
            TokenKind::Nil => {
                let token = self.advance().clone();
                Some(Expr::new(ExprKind::Literal(Literal::Nil), token.span))
            }

            // Identifier (with path and struct literal support)
            // `self` and `emit` are keywords that can appear as identifiers in expressions
            TokenKind::Identifier | TokenKind::Emit | TokenKind::SelfKw => {
                self.parse_identifier_or_path()
            }

            // Grouping or tuple: (expr) or (expr, expr, ...)
            TokenKind::LeftParen => self.parse_grouping_or_tuple(),

            // Array literal: [expr, expr, ...]
            TokenKind::LeftBracket => {
                self.advance(); // consume '['
                let mut elements = Vec::new();
                if self.peek() != TokenKind::RightBracket {
                    loop {
                        let elem = self.parse_expression()?;
                        elements.push(elem);
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                        if self.peek() == TokenKind::RightBracket {
                            break; // trailing comma
                        }
                    }
                }
                self.expect(TokenKind::RightBracket)?;
                let span = start.merge(&self.previous_span());
                Some(Expr::new(ExprKind::Array(elements), span))
            }

            // Block expression or map literal
            TokenKind::LeftBrace => {
                if self.is_map_literal() {
                    self.parse_map_literal(start)
                } else {
                    let block = self.parse_block()?;
                    let span = block.span.clone();
                    Some(Expr::new(ExprKind::Block(block), span))
                }
            }

            // If expression
            TokenKind::If => self.parse_if_expr(),

            // Match expression
            TokenKind::Match => self.parse_match_expr(),

            // For loop expression
            TokenKind::For => self.parse_for_expr(),

            // While loop expression
            TokenKind::While => self.parse_while_expr(),

            // Loop expression
            TokenKind::Loop => self.parse_loop_expr(),

            // Try/catch expression
            TokenKind::Try => self.parse_try_catch_expr(),

            // Prefix await: `await expr`
            TokenKind::Await => {
                self.advance(); // consume 'await'
                let operand = self.parse_expr_bp(25)?; // tight binding (same as prefix unary)
                let span = start.merge(&operand.span);
                Some(Expr::new(ExprKind::Await(Box::new(operand)), span))
            }

            // Return as expression (for match arms, closures, etc.): `return expr`
            TokenKind::Return => {
                self.advance(); // consume 'return'
                                // Check if there's a value to return
                let value = if can_start_expression(self.peek()) {
                    Some(Box::new(self.parse_expression()?))
                } else {
                    None
                };
                let span = if let Some(ref v) = value {
                    start.merge(&v.span)
                } else {
                    start
                };
                Some(Expr::new(ExprKind::Return(value), span))
            }

            // Listen expression: listen Agent.execute("prompt") { "type" => |param| { ... }, ... }
            TokenKind::Listen => self.parse_listen_expr(),

            // Closure: |params| expr
            TokenKind::Pipe => self.parse_closure(),

            // Closure with empty params: || expr
            TokenKind::PipePipe => self.parse_closure_empty_params(),

            _ => {
                let span = self.current_span();
                self.diagnostics.error(
                    format!("expected expression, found {:?}", self.peek()),
                    span.clone(),
                );
                None
            }
        }
    }

    // ========================================================================
    // Postfix operations
    // ========================================================================

    /// Parse postfix operations: calls, member access, indexing, .await.
    fn parse_postfix(&mut self, mut expr: Expr) -> Expr {
        loop {
            match self.peek() {
                // Function call: expr(args)
                TokenKind::LeftParen => {
                    self.advance(); // consume '('
                    let args = self.parse_arg_list();
                    if self.expect(TokenKind::RightParen).is_none() {
                        return expr;
                    }
                    let span = expr.span.merge(&self.previous_span());
                    expr = Expr::new(
                        ExprKind::Call {
                            callee: Box::new(expr),
                            args,
                        },
                        span,
                    );
                }

                // Member access, method call, or .await
                TokenKind::Dot => {
                    self.advance(); // consume '.'

                    // Check for .await
                    if self.peek() == TokenKind::Await {
                        self.advance(); // consume 'await'
                        let span = expr.span.merge(&self.previous_span());
                        expr = Expr::new(ExprKind::Await(Box::new(expr)), span);
                        continue;
                    }

                    if self.peek() != TokenKind::Identifier {
                        let span = self.current_span();
                        self.diagnostics
                            .error("expected identifier after '.'", span);
                        return expr;
                    }
                    let field = self.advance().clone();
                    let field_name = field.lexeme.clone();

                    // Check if it's a method call (possibly with generic type args)
                    if self.peek() == TokenKind::LeftParen {
                        self.advance(); // consume '('
                        let args = self.parse_arg_list();
                        if self.expect(TokenKind::RightParen).is_none() {
                            return expr;
                        }
                        let span = expr.span.merge(&self.previous_span());
                        expr = Expr::new(
                            ExprKind::MethodCall {
                                object: Box::new(expr),
                                method: field_name,
                                type_args: vec![],
                                args,
                            },
                            span,
                        );
                    } else if self.peek() == TokenKind::Less {
                        // Could be generic method call: method<Type>(args)
                        // Lookahead to disambiguate from comparison: need <Ident>(
                        if self.is_generic_method_call() {
                            self.advance(); // consume '<'
                            let type_args = self.parse_type_arg_list();
                            if self.expect(TokenKind::Greater).is_none() {
                                return expr;
                            }
                            if self.expect(TokenKind::LeftParen).is_none() {
                                return expr;
                            }
                            let args = self.parse_arg_list();
                            if self.expect(TokenKind::RightParen).is_none() {
                                return expr;
                            }
                            let span = expr.span.merge(&self.previous_span());
                            expr = Expr::new(
                                ExprKind::MethodCall {
                                    object: Box::new(expr),
                                    method: field_name,
                                    type_args,
                                    args,
                                },
                                span,
                            );
                        } else {
                            // Not a generic call — fall through to field access
                            let span = expr.span.merge(&field.span);
                            expr = Expr::new(
                                ExprKind::FieldAccess {
                                    object: Box::new(expr),
                                    field: field_name,
                                },
                                span,
                            );
                        }
                    } else {
                        let span = expr.span.merge(&field.span);
                        expr = Expr::new(
                            ExprKind::FieldAccess {
                                object: Box::new(expr),
                                field: field_name,
                            },
                            span,
                        );
                    }
                }

                // Index: expr[index]
                TokenKind::LeftBracket => {
                    self.advance(); // consume '['
                    let index = match self.parse_expression() {
                        Some(e) => e,
                        None => return expr,
                    };
                    if self.expect(TokenKind::RightBracket).is_none() {
                        return expr;
                    }
                    let span = expr.span.merge(&self.previous_span());
                    expr = Expr::new(
                        ExprKind::Index {
                            object: Box::new(expr),
                            index: Box::new(index),
                        },
                        span,
                    );
                }

                _ => break,
            }
        }
        expr
    }

    /// Parse a comma-separated argument list (positional only for now).
    fn parse_arg_list(&mut self) -> Vec<Expr> {
        let mut args = Vec::new();
        if self.peek() == TokenKind::RightParen {
            return args;
        }
        while let Some(arg) = self.parse_expression() {
            args.push(arg);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        args
    }

    /// Lookahead to determine if `<` starts a generic type argument list
    /// for a method call (e.g., `method<Type>(args)`), as opposed to a
    /// comparison operator. Checks pattern: `< Identifier [, Identifier]* > (`
    fn is_generic_method_call(&self) -> bool {
        // Current token should be '<'
        if self.peek() != TokenKind::Less {
            return false;
        }
        // Scan ahead: < Type [, Type]* > (
        let mut offset = 1; // skip '<'
        loop {
            // Expect an identifier (type name)
            if self.peek_at(offset) != TokenKind::Identifier {
                return false;
            }
            offset += 1;
            // After identifier, expect ',' (more types) or '>' (end)
            match self.peek_at(offset) {
                TokenKind::Comma => {
                    offset += 1; // skip ',' and continue to next type
                }
                TokenKind::Greater => {
                    offset += 1; // skip '>'
                                 // Must be followed by '(' to be a method call
                    return self.peek_at(offset) == TokenKind::LeftParen;
                }
                _ => return false,
            }
        }
    }

    /// Parse generic type arguments: `Type [, Type]*` (between `<` and `>`).
    /// The `<` has already been consumed.
    fn parse_type_arg_list(&mut self) -> Vec<crate::ast::types::TypeAnnotation> {
        let mut type_args = Vec::new();
        if self.peek() == TokenKind::Greater {
            return type_args;
        }
        while let Some(ty) = self.parse_type_annotation() {
            type_args.push(ty);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        type_args
    }

    // ========================================================================
    // Control flow expressions
    // ========================================================================

    /// Parse an if expression: `if cond { ... } [else { ... }]`
    fn parse_if_expr(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // consume 'if'

        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;

        let else_branch = if self.eat(TokenKind::Else) {
            if self.peek() == TokenKind::If {
                let else_if = self.parse_if_expr()?;
                Some(ElseBranch::ElseIf(Box::new(else_if)))
            } else {
                let block = self.parse_block()?;
                Some(ElseBranch::Block(block))
            }
        } else {
            None
        };

        let end = self.previous_span();
        let span = start.merge(&end);
        Some(Expr::new(
            ExprKind::If {
                condition: Box::new(condition),
                then_branch,
                else_branch,
            },
            span,
        ))
    }

    /// Parse a match expression: `match expr { pattern => body, ... }`
    fn parse_match_expr(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // consume 'match'

        let scrutinee = self.parse_expression()?;
        self.expect(TokenKind::LeftBrace)?;

        let mut arms = Vec::new();
        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            let arm = self.parse_match_arm()?;
            arms.push(arm);
            // Comma is optional between arms (allows trailing comma)
            self.eat(TokenKind::Comma);
        }

        self.expect(TokenKind::RightBrace)?;
        let span = start.merge(&self.previous_span());
        Some(Expr::new(
            ExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span,
        ))
    }

    /// Parse a single match arm: `pattern [if guard] => body`
    fn parse_match_arm(&mut self) -> Option<MatchArm> {
        let start = self.current_span();
        let pattern = self.parse_pattern()?;

        // Optional guard clause
        let guard = if self.eat(TokenKind::If) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        self.expect(TokenKind::FatArrow)?; // =>

        // Arm body is an expression (can be a block expression)
        let body = self.parse_expression()?;

        let span = start.merge(&body.span);
        Some(MatchArm {
            pattern,
            guard,
            body,
            span,
        })
    }

    /// Parse a for loop expression: `for pattern in iterable { body }`
    fn parse_for_expr(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // consume 'for'

        let pattern = self.parse_pattern()?;
        self.expect(TokenKind::In)?;
        let iterable = self.parse_expression()?;
        let body = self.parse_block()?;

        let span = start.merge(&self.previous_span());
        Some(Expr::new(
            ExprKind::For {
                pattern,
                iterable: Box::new(iterable),
                body,
            },
            span,
        ))
    }

    /// Parse a while loop expression: `while condition { body }`
    fn parse_while_expr(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // consume 'while'

        let condition = self.parse_expression()?;
        let body = self.parse_block()?;

        let span = start.merge(&self.previous_span());
        Some(Expr::new(
            ExprKind::While {
                condition: Box::new(condition),
                body,
            },
            span,
        ))
    }

    /// Parse an infinite loop expression: `loop { body }`
    fn parse_loop_expr(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // consume 'loop'

        let body = self.parse_block()?;

        let span = start.merge(&self.previous_span());
        Some(Expr::new(ExprKind::Loop { body }, span))
    }

    /// Parse a try/catch expression:
    /// `try { body } catch [ErrorType(binding)] { handler }`
    fn parse_try_catch_expr(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // consume 'try'

        let body = self.parse_block()?;

        let mut catches = Vec::new();
        while self.eat(TokenKind::Catch) {
            let catch_start = self.previous_span();

            // Optional error type and binding: catch ErrorType(binding)
            let (error_type, binding) = if self.peek() == TokenKind::LeftBrace {
                // Bare catch: catch { ... }
                (None, None)
            } else {
                // Typed catch: catch ErrorType(binding) { ... }
                let ty = self.parse_type_annotation()?;
                let bind = if self.eat(TokenKind::LeftParen) {
                    let name = self.expect(TokenKind::Identifier)?.lexeme.clone();
                    self.expect(TokenKind::RightParen)?;
                    Some(name)
                } else {
                    None
                };
                (Some(ty), bind)
            };

            let catch_body = self.parse_block()?;
            let catch_span = catch_start.merge(&self.previous_span());
            catches.push(CatchClause {
                error_type,
                binding,
                body: catch_body,
                span: catch_span,
            });
        }

        let span = start.merge(&self.previous_span());
        Some(Expr::new(ExprKind::TryCatch { body, catches }, span))
    }

    // ========================================================================
    // Listen expression
    // ========================================================================

    /// Parse a listen expression:
    /// `listen Agent.execute("prompt") { "type" => |param| { body }, ... }`
    fn parse_listen_expr(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // consume 'listen'

        // Parse the agent call expression (e.g., ClaudeCode.execute("prompt"))
        let call = self.parse_expression()?;

        self.expect(TokenKind::LeftBrace)?;

        let mut handlers = Vec::new();
        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            let handler = self.parse_listen_handler()?;
            handlers.push(handler);
            self.eat(TokenKind::Comma); // optional trailing comma
        }

        self.expect(TokenKind::RightBrace)?;
        let span = start.merge(&self.previous_span());
        Some(Expr::new(
            ExprKind::Listen {
                call: Box::new(call),
                handlers,
            },
            span,
        ))
    }

    /// Parse a single listen handler: `"message_type" => |param: Type| { body }`
    fn parse_listen_handler(&mut self) -> Option<ListenHandler> {
        let start = self.current_span();

        // Parse message type string literal: "progress", "question", etc.
        let type_token = self.expect(TokenKind::StringLiteral)?;
        let message_type = type_token
            .lexeme
            .trim_matches('"')
            .to_string();

        self.expect(TokenKind::FatArrow)?; // =>

        // Parse closure-like parameter: |param: Type|
        self.expect(TokenKind::Pipe)?; // opening |

        let param = self.parse_param()?;

        self.expect(TokenKind::Pipe)?; // closing |

        // Parse handler body block
        let body = self.parse_block()?;

        let span = start.merge(&self.previous_span());
        Some(ListenHandler {
            message_type,
            param,
            body,
            span,
        })
    }

    // ========================================================================
    // Closures
    // ========================================================================

    /// Parse a closure: `|params| expr` or `|params| -> Type { block }`
    fn parse_closure(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // consume '|'

        let mut params = Vec::new();
        if self.peek() != TokenKind::Pipe {
            loop {
                let param_start = self.current_span();
                let name_token = self.expect(TokenKind::Identifier)?;
                let name = name_token.lexeme.clone();

                let type_ann = if self.eat(TokenKind::Colon) {
                    Some(self.parse_type_annotation()?)
                } else {
                    None
                };

                let param_span = param_start.merge(&self.previous_span());
                params.push(Param {
                    name,
                    type_ann,
                    default: None,
                    span: param_span,
                });

                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }

        self.expect(TokenKind::Pipe)?; // closing '|'

        // Optional return type
        let return_type = if self.eat(TokenKind::Arrow) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        // Body: block or single expression
        let body = if self.peek() == TokenKind::LeftBrace {
            let block = self.parse_block()?;
            let span = block.span.clone();
            Expr::new(ExprKind::Block(block), span)
        } else {
            self.parse_expression()?
        };

        let span = start.merge(&body.span);
        Some(Expr::new(
            ExprKind::Closure {
                params,
                return_type,
                body: Box::new(body),
            },
            span,
        ))
    }

    /// Parse a closure with empty params: `|| expr`
    fn parse_closure_empty_params(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // consume '||'

        let return_type = if self.eat(TokenKind::Arrow) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let body = if self.peek() == TokenKind::LeftBrace {
            let block = self.parse_block()?;
            let span = block.span.clone();
            Expr::new(ExprKind::Block(block), span)
        } else {
            self.parse_expression()?
        };

        let span = start.merge(&body.span);
        Some(Expr::new(
            ExprKind::Closure {
                params: Vec::new(),
                return_type,
                body: Box::new(body),
            },
            span,
        ))
    }

    // ========================================================================
    // Identifier, path, and struct literal
    // ========================================================================

    /// Parse an identifier, potentially followed by :: path or { struct literal.
    fn parse_identifier_or_path(&mut self) -> Option<Expr> {
        let start = self.current_span();
        let token = self.advance().clone();
        let name = token.lexeme.clone();

        // Check for :: path
        if self.peek() == TokenKind::ColonColon {
            let mut segments = vec![name];
            while self.eat(TokenKind::ColonColon) {
                let seg = self.expect(TokenKind::Identifier)?;
                segments.push(seg.lexeme.clone());
            }

            // Check for struct literal: Path { field: value }
            if self.peek() == TokenKind::LeftBrace && self.is_struct_literal_ahead() {
                return self.parse_struct_literal(segments, start);
            }

            // Check for enum constructor: Path(args) - handled by normal postfix call
            let span = start.merge(&self.previous_span());
            return Some(Expr::new(ExprKind::Path(segments), span));
        }

        // Check for struct literal: Ident { field: value }
        if self.peek() == TokenKind::LeftBrace && self.is_struct_literal_ahead() {
            return self.parse_struct_literal(vec![name], start);
        }

        Some(Expr::new(ExprKind::Identifier(name), token.span))
    }

    /// Heuristic: check if the upcoming `{` starts a struct literal.
    /// A struct literal has `{ ident:` or `{ }` pattern.
    fn is_struct_literal_ahead(&self) -> bool {
        let after_brace = self.pos + 1;
        if let Some(token) = self.tokens.get(after_brace) {
            // Empty struct: Ident {}
            if token.kind == TokenKind::RightBrace {
                return true;
            }
            // Struct with fields: Ident { field: ... }
            if token.kind == TokenKind::Identifier {
                if let Some(next) = self.tokens.get(after_brace + 1) {
                    return next.kind == TokenKind::Colon;
                }
            }
        }
        false
    }

    /// Parse a struct literal: `Name { field: value, ... }`
    fn parse_struct_literal(
        &mut self,
        name: Vec<String>,
        start: concerto_common::Span,
    ) -> Option<Expr> {
        self.advance(); // consume '{'
        let mut fields = Vec::new();

        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            let field_start = self.current_span();
            let field_name = self.expect(TokenKind::Identifier)?.lexeme.clone();
            self.expect(TokenKind::Colon)?;
            let value = self.parse_expression()?;
            let field_span = field_start.merge(&value.span);
            fields.push(StructLiteralField {
                name: field_name,
                value,
                span: field_span,
            });
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        self.expect(TokenKind::RightBrace)?;
        let span = start.merge(&self.previous_span());
        Some(Expr::new(ExprKind::StructLiteral { name, fields }, span))
    }

    // ========================================================================
    // Tuple / grouping
    // ========================================================================

    /// Parse `(expr)` (grouping) or `(expr, expr, ...)` (tuple) or `()` (empty tuple).
    fn parse_grouping_or_tuple(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // consume '('

        // Empty tuple: ()
        if self.peek() == TokenKind::RightParen {
            self.advance();
            let span = start.merge(&self.previous_span());
            return Some(Expr::new(ExprKind::Tuple(Vec::new()), span));
        }

        let first = self.parse_expression()?;

        if self.eat(TokenKind::Comma) {
            // Tuple: (expr, ...)
            let mut elements = vec![first];
            if self.peek() != TokenKind::RightParen {
                loop {
                    elements.push(self.parse_expression()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                    if self.peek() == TokenKind::RightParen {
                        break; // trailing comma
                    }
                }
            }
            self.expect(TokenKind::RightParen)?;
            let span = start.merge(&self.previous_span());
            Some(Expr::new(ExprKind::Tuple(elements), span))
        } else {
            // Grouping: (expr)
            self.expect(TokenKind::RightParen)?;
            let span = start.merge(&self.previous_span());
            Some(Expr::new(ExprKind::Grouping(Box::new(first)), span))
        }
    }

    // ========================================================================
    // String interpolation
    // ========================================================================

    /// Parse string interpolation: `"Hello ${name}!"`
    fn parse_string_interpolation(&mut self) -> Option<Expr> {
        let start = self.current_span();
        let first_token = self.advance().clone();
        let mut parts = Vec::new();

        parts.push(StringPart::Literal(first_token.lexeme.clone()));

        loop {
            // Parse the interpolated expression
            let expr = self.parse_expression()?;
            parts.push(StringPart::Expr(Box::new(expr)));

            match self.peek() {
                TokenKind::InterpolStringMid => {
                    let mid = self.advance().clone();
                    parts.push(StringPart::Literal(mid.lexeme.clone()));
                }
                TokenKind::InterpolStringEnd => {
                    let end_token = self.advance().clone();
                    parts.push(StringPart::Literal(end_token.lexeme.clone()));
                    break;
                }
                _ => {
                    self.diagnostics
                        .error("expected string continuation or end", self.current_span());
                    break;
                }
            }
        }

        let span = start.merge(&self.previous_span());
        Some(Expr::new(ExprKind::StringInterpolation(parts), span))
    }

    // ========================================================================
    // Map literal
    // ========================================================================

    /// Heuristic: check if the upcoming `{` starts a map literal.
    /// A map starts with `{ string_key : ...`.
    fn is_map_literal(&self) -> bool {
        if self.peek() == TokenKind::LeftBrace {
            let after_brace = self.pos + 1;
            if let Some(token) = self.tokens.get(after_brace) {
                if token.kind == TokenKind::StringLiteral {
                    if let Some(colon) = self.tokens.get(after_brace + 1) {
                        return colon.kind == TokenKind::Colon;
                    }
                }
            }
        }
        false
    }

    /// Parse a map literal: `{ "key": value, ... }`
    fn parse_map_literal(&mut self, start: concerto_common::Span) -> Option<Expr> {
        self.advance(); // consume '{'
        let mut entries = Vec::new();

        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            let key = self.parse_expression()?;
            self.expect(TokenKind::Colon)?;
            let value = self.parse_expression()?;
            entries.push((key, value));
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        self.expect(TokenKind::RightBrace)?;
        let span = start.merge(&self.previous_span());
        Some(Expr::new(ExprKind::Map(entries), span))
    }

    // ========================================================================
    // Pattern parsing
    // ========================================================================

    /// Parse a pattern (used in match arms, for loops).
    /// Handles or-patterns: `pattern | pattern | ...`
    pub(super) fn parse_pattern(&mut self) -> Option<Pattern> {
        let start = self.current_span();
        let first = self.parse_single_pattern()?;

        // Check for or-pattern: pattern | pattern | ...
        if self.peek() == TokenKind::Pipe {
            let mut patterns = vec![first];
            while self.eat(TokenKind::Pipe) {
                patterns.push(self.parse_single_pattern()?);
            }
            let span = start.merge(&self.previous_span());
            return Some(Pattern {
                kind: PatternKind::Or(patterns),
                span,
            });
        }

        Some(first)
    }

    /// Parse a single pattern (without or-pattern).
    fn parse_single_pattern(&mut self) -> Option<Pattern> {
        let start = self.current_span();

        match self.peek() {
            // Wildcard: _
            TokenKind::Identifier if self.current().lexeme == "_" => {
                self.advance();
                Some(Pattern {
                    kind: PatternKind::Wildcard,
                    span: start,
                })
            }

            // Identifier, path, enum variant, or struct destructure
            TokenKind::Identifier => self.parse_identifier_pattern(),

            // Literal patterns
            TokenKind::IntLiteral
            | TokenKind::FloatLiteral
            | TokenKind::StringLiteral
            | TokenKind::True
            | TokenKind::False
            | TokenKind::Nil => self.parse_literal_pattern(),

            // Negative literal pattern: -42
            TokenKind::Minus => {
                self.advance();
                let mut pat = self.parse_literal_pattern()?;
                match &mut pat.kind {
                    PatternKind::Literal(Literal::Int(v)) => *v = -*v,
                    PatternKind::Literal(Literal::Float(v)) => *v = -*v,
                    _ => {
                        self.diagnostics.error(
                            "expected numeric literal after '-' in pattern",
                            start.clone(),
                        );
                    }
                }
                pat.span = start.merge(&pat.span);
                Some(pat)
            }

            // Tuple pattern: (a, b, c)
            TokenKind::LeftParen => {
                self.advance(); // consume '('
                let mut patterns = Vec::new();
                if self.peek() != TokenKind::RightParen {
                    loop {
                        patterns.push(self.parse_pattern()?);
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                        if self.peek() == TokenKind::RightParen {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RightParen)?;
                let span = start.merge(&self.previous_span());
                Some(Pattern {
                    kind: PatternKind::Tuple(patterns),
                    span,
                })
            }

            // Array pattern: [first, second, ..rest]
            TokenKind::LeftBracket => {
                self.advance(); // consume '['
                let mut elements = Vec::new();
                let mut rest = None;
                while self.peek() != TokenKind::RightBracket && !self.is_at_end() {
                    if self.peek() == TokenKind::DotDot {
                        self.advance(); // consume '..'
                        if self.peek() == TokenKind::Identifier {
                            rest = Some(self.advance().lexeme.clone());
                        }
                        break;
                    }
                    elements.push(self.parse_pattern()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RightBracket)?;
                let span = start.merge(&self.previous_span());
                Some(Pattern {
                    kind: PatternKind::Array { elements, rest },
                    span,
                })
            }

            // Rest pattern: ..
            TokenKind::DotDot => {
                self.advance();
                Some(Pattern {
                    kind: PatternKind::Rest,
                    span: start,
                })
            }

            _ => {
                self.diagnostics
                    .error(format!("expected pattern, found {:?}", self.peek()), start);
                None
            }
        }
    }

    /// Parse an identifier pattern, potentially with path, enum constructor, or struct destructure.
    fn parse_identifier_pattern(&mut self) -> Option<Pattern> {
        let start = self.current_span();
        let name_token = self.advance().clone();
        let name = name_token.lexeme.clone();

        // Check for binding: n @ pattern
        if self.peek() == TokenKind::At {
            self.advance(); // consume '@'
            let inner = self.parse_single_pattern()?;
            let span = start.merge(&inner.span);
            return Some(Pattern {
                kind: PatternKind::Binding {
                    name,
                    pattern: Box::new(inner),
                },
                span,
            });
        }

        // Check for path: Ident::Ident
        let mut path = vec![name];
        while self.eat(TokenKind::ColonColon) {
            let seg = self.expect(TokenKind::Identifier)?;
            path.push(seg.lexeme.clone());
        }

        // Enum tuple variant: Path(patterns)
        if self.peek() == TokenKind::LeftParen {
            self.advance(); // consume '('
            let mut fields = Vec::new();
            if self.peek() != TokenKind::RightParen {
                loop {
                    fields.push(self.parse_pattern()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                    if self.peek() == TokenKind::RightParen {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RightParen)?;
            let span = start.merge(&self.previous_span());
            return Some(Pattern {
                kind: PatternKind::Enum { path, fields },
                span,
            });
        }

        // Struct destructure: Path { fields }
        if self.peek() == TokenKind::LeftBrace {
            self.advance(); // consume '{'
            let mut fields = Vec::new();
            let mut has_rest = false;
            while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
                if self.peek() == TokenKind::DotDot {
                    self.advance();
                    has_rest = true;
                    // Allow trailing comma after ..
                    self.eat(TokenKind::Comma);
                    break;
                }
                let field_start = self.current_span();
                let field_name = self.expect(TokenKind::Identifier)?.lexeme.clone();

                let pattern = if self.eat(TokenKind::Colon) {
                    Some(self.parse_pattern()?)
                } else {
                    None
                };

                let field_span = field_start.merge(&self.previous_span());
                fields.push(PatternField {
                    name: field_name,
                    pattern,
                    span: field_span,
                });

                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RightBrace)?;
            let span = start.merge(&self.previous_span());
            return Some(Pattern {
                kind: PatternKind::Struct {
                    path,
                    fields,
                    has_rest,
                },
                span,
            });
        }

        // Simple identifier binding (single-segment path)
        if path.len() == 1 {
            let name = path.into_iter().next().unwrap();
            // `None` is a unit enum variant, not a variable binding
            if name == "None" {
                return Some(Pattern {
                    kind: PatternKind::Enum {
                        path: vec![name],
                        fields: Vec::new(),
                    },
                    span: name_token.span,
                });
            }
            Some(Pattern {
                kind: PatternKind::Identifier(name),
                span: name_token.span,
            })
        } else {
            // Multi-segment path as enum unit variant (e.g., Direction::North)
            let span = start.merge(&self.previous_span());
            Some(Pattern {
                kind: PatternKind::Enum {
                    path,
                    fields: Vec::new(),
                },
                span,
            })
        }
    }

    /// Parse a literal pattern.
    fn parse_literal_pattern(&mut self) -> Option<Pattern> {
        let start = self.current_span();
        match self.peek() {
            TokenKind::IntLiteral => {
                let token = self.advance().clone();
                let clean = token.lexeme.replace('_', "");
                let value: i64 = clean.parse().unwrap_or(0);
                Some(Pattern {
                    kind: PatternKind::Literal(Literal::Int(value)),
                    span: token.span,
                })
            }
            TokenKind::FloatLiteral => {
                let token = self.advance().clone();
                let clean = token.lexeme.replace('_', "");
                let value: f64 = clean.parse().unwrap_or(0.0);
                Some(Pattern {
                    kind: PatternKind::Literal(Literal::Float(value)),
                    span: token.span,
                })
            }
            TokenKind::StringLiteral => {
                let token = self.advance().clone();
                Some(Pattern {
                    kind: PatternKind::Literal(Literal::String(token.lexeme.clone())),
                    span: token.span,
                })
            }
            TokenKind::True => {
                let token = self.advance().clone();
                Some(Pattern {
                    kind: PatternKind::Literal(Literal::Bool(true)),
                    span: token.span,
                })
            }
            TokenKind::False => {
                let token = self.advance().clone();
                Some(Pattern {
                    kind: PatternKind::Literal(Literal::Bool(false)),
                    span: token.span,
                })
            }
            TokenKind::Nil => {
                let token = self.advance().clone();
                Some(Pattern {
                    kind: PatternKind::Literal(Literal::Nil),
                    span: token.span,
                })
            }
            _ => {
                self.diagnostics.error(
                    format!("expected literal pattern, found {:?}", self.peek()),
                    start,
                );
                None
            }
        }
    }
}

// ============================================================================
// Helper functions
// ============================================================================

fn token_to_binary_op(kind: TokenKind) -> Option<BinaryOp> {
    match kind {
        TokenKind::Plus => Some(BinaryOp::Add),
        TokenKind::Minus => Some(BinaryOp::Sub),
        TokenKind::Star => Some(BinaryOp::Mul),
        TokenKind::Slash => Some(BinaryOp::Div),
        TokenKind::Percent => Some(BinaryOp::Mod),
        TokenKind::EqualEqual => Some(BinaryOp::Eq),
        TokenKind::BangEqual => Some(BinaryOp::Neq),
        TokenKind::Less => Some(BinaryOp::Lt),
        TokenKind::Greater => Some(BinaryOp::Gt),
        TokenKind::LessEqual => Some(BinaryOp::Lte),
        TokenKind::GreaterEqual => Some(BinaryOp::Gte),
        TokenKind::AmpAmp => Some(BinaryOp::And),
        TokenKind::PipePipe => Some(BinaryOp::Or),
        _ => None,
    }
}

fn token_to_assign_op(kind: TokenKind) -> Option<AssignOp> {
    match kind {
        TokenKind::Equal => Some(AssignOp::Assign),
        TokenKind::PlusEqual => Some(AssignOp::AddAssign),
        TokenKind::MinusEqual => Some(AssignOp::SubAssign),
        TokenKind::StarEqual => Some(AssignOp::MulAssign),
        TokenKind::SlashEqual => Some(AssignOp::DivAssign),
        TokenKind::PercentEqual => Some(AssignOp::ModAssign),
        _ => None,
    }
}

/// Check if a token kind can start an expression.
fn can_start_expression(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::IntLiteral
            | TokenKind::FloatLiteral
            | TokenKind::StringLiteral
            | TokenKind::InterpolStringStart
            | TokenKind::True
            | TokenKind::False
            | TokenKind::Nil
            | TokenKind::Identifier
            | TokenKind::Emit
            | TokenKind::LeftParen
            | TokenKind::LeftBracket
            | TokenKind::LeftBrace
            | TokenKind::If
            | TokenKind::Match
            | TokenKind::For
            | TokenKind::While
            | TokenKind::Loop
            | TokenKind::Try
            | TokenKind::Pipe
            | TokenKind::PipePipe
            | TokenKind::Minus
            | TokenKind::Bang
            | TokenKind::Await
            | TokenKind::Return
            | TokenKind::Listen
    )
}

#[cfg(test)]
mod tests {
    use crate::ast::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn parse(source: &str) -> Program {
        let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
        assert!(
            !lex_diags.has_errors(),
            "lex errors: {:?}",
            lex_diags.diagnostics()
        );
        let (program, parse_diags) = Parser::new(tokens).parse();
        assert!(
            !parse_diags.has_errors(),
            "parse errors: {:?}",
            parse_diags.diagnostics()
        );
        program
    }

    /// Extract a FunctionDecl from the first declaration.
    fn get_fn(prog: &Program) -> &FunctionDecl {
        match &prog.declarations[0] {
            Declaration::Function(f) => f,
            other => panic!("expected Function, got {:?}", std::mem::discriminant(other)),
        }
    }

    /// Get the body block of a function (unwraps Option).
    fn body(f: &FunctionDecl) -> &Block {
        f.body.as_ref().expect("function should have a body")
    }

    // =====================================================================
    // Existing tests (preserved from Step 4+)
    // =====================================================================

    #[test]
    fn parse_empty_function() {
        let prog = parse("fn main() {}");
        assert_eq!(prog.declarations.len(), 1);
        let f = get_fn(&prog);
        assert_eq!(f.name, "main");
        assert!(f.params.is_empty());
        assert!(!f.is_public);
        assert!(!f.is_async);
    }

    #[test]
    fn parse_let_with_init() {
        let prog = parse("fn main() { let x = 5; }");
        let f = get_fn(&prog);
        let b = body(f);
        assert_eq!(b.stmts.len(), 1);
        match &b.stmts[0] {
            Stmt::Let(s) => {
                assert_eq!(s.name, "x");
                assert!(!s.mutable);
                assert!(s.initializer.is_some());
            }
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_let_mut() {
        let prog = parse("fn main() { let mut y = 10; }");
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => {
                assert_eq!(s.name, "y");
                assert!(s.mutable);
            }
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_binary_expression() {
        let prog = parse("fn main() { let z = 1 + 2 * 3; }");
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => {
                let init = s.initializer.as_ref().unwrap();
                match &init.kind {
                    ExprKind::Binary { op, .. } => {
                        assert_eq!(*op, BinaryOp::Add);
                    }
                    _ => panic!("expected binary expression"),
                }
            }
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_if_expression() {
        let prog = parse("fn main() { if x > 0 { 1; } else { 2; } }");
        let f = get_fn(&prog);
        let b = body(f);
        assert_eq!(b.stmts.len(), 0);
        assert!(b.tail_expr.is_some());
        matches!(&b.tail_expr.as_ref().unwrap().kind, ExprKind::If { .. });
    }

    #[test]
    fn parse_function_call() {
        let prog = parse("fn main() { emit(\"result\", 42); }");
        let f = get_fn(&prog);
        let b = body(f);
        assert_eq!(b.stmts.len(), 1);
        match &b.stmts[0] {
            Stmt::Expr(s) => match &s.expr.kind {
                ExprKind::Call { callee, args } => {
                    matches!(&callee.kind, ExprKind::Identifier(name) if name == "emit");
                    assert_eq!(args.len(), 2);
                }
                _ => panic!("expected call expression"),
            },
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parse_method_call() {
        let prog = parse("fn main() { obj.method(a, b); }");
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Expr(s) => match &s.expr.kind {
                ExprKind::MethodCall { method, args, .. } => {
                    assert_eq!(method, "method");
                    assert_eq!(args.len(), 2);
                }
                _ => panic!("expected method call"),
            },
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parse_generic_method_call() {
        let prog = parse("fn main() { obj.method<Type>(a, b); }");
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Expr(s) => match &s.expr.kind {
                ExprKind::MethodCall {
                    method,
                    type_args,
                    args,
                    ..
                } => {
                    assert_eq!(method, "method");
                    assert_eq!(type_args.len(), 1);
                    assert_eq!(args.len(), 2);
                    // Verify the type arg is "Type"
                    if let crate::ast::types::TypeKind::Named(name) = &type_args[0].kind {
                        assert_eq!(name, "Type");
                    } else {
                        panic!("expected Named type arg");
                    }
                }
                _ => panic!("expected method call, got {:?}", s.expr.kind),
            },
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parse_generic_method_call_multiple_type_args() {
        let prog = parse("fn main() { obj.method<A, B>(x); }");
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Expr(s) => match &s.expr.kind {
                ExprKind::MethodCall {
                    method,
                    type_args,
                    args,
                    ..
                } => {
                    assert_eq!(method, "method");
                    assert_eq!(type_args.len(), 2);
                    assert_eq!(args.len(), 1);
                }
                _ => panic!("expected method call"),
            },
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parse_less_than_not_generic() {
        // `a.b < c` should parse as comparison, not generic method
        let prog = parse("fn main() { let x = a.b < c; }");
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(l) => match &l.initializer.as_ref().unwrap().kind {
                ExprKind::Binary { op, .. } => {
                    assert_eq!(op, &BinaryOp::Lt);
                }
                _ => panic!("expected binary expression"),
            },
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_field_access() {
        let prog = parse("fn main() { let x = obj.field; }");
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::FieldAccess { field, .. } => {
                    assert_eq!(field, "field");
                }
                _ => panic!("expected field access"),
            },
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_unary() {
        let prog = parse("fn main() { let x = -5; let y = !true; }");
        let f = get_fn(&prog);
        let b = body(f);
        assert_eq!(b.stmts.len(), 2);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Unary { op, .. } => assert_eq!(*op, UnaryOp::Neg),
                _ => panic!("expected unary"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_return_value() {
        let prog = parse("fn add(a: Int, b: Int) -> Int { return a + b; }");
        let f = get_fn(&prog);
        assert_eq!(f.name, "add");
        assert_eq!(f.params.len(), 2);
        assert!(f.return_type.is_some());
    }

    #[test]
    fn parse_array_literal() {
        let prog = parse("fn main() { let arr = [1, 2, 3]; }");
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Array(elems) => assert_eq!(elems.len(), 3),
                _ => panic!("expected array"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_nested_if_else_if() {
        let prog = parse(
            r#"
            fn main() {
                if x > 0 {
                    1;
                } else if x < 0 {
                    2;
                } else {
                    0;
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        assert_eq!(b.stmts.len(), 0);
        assert!(b.tail_expr.is_some());
    }

    #[test]
    fn parse_map_literal() {
        let prog = parse(r#"fn main() { let m = { "a": 1, "b": 2 }; }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Map(entries) => assert_eq!(entries.len(), 2),
                _ => panic!(
                    "expected map, got {:?}",
                    s.initializer.as_ref().unwrap().kind
                ),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_pub_function() {
        let prog = parse("pub fn greet() {}");
        let f = get_fn(&prog);
        assert!(f.is_public);
        assert_eq!(f.name, "greet");
    }

    #[test]
    fn parse_index_expression() {
        let prog = parse("fn main() { let x = arr[0]; }");
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Index { .. } => {}
                _ => panic!("expected index expression"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_milestone_program() {
        let prog = parse(
            r#"
            fn main() {
                let x = 5;
                let y = x + 3;
                if y > 7 {
                    emit("result", y);
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        assert_eq!(f.name, "main");
        assert_eq!(b.stmts.len(), 2);
        assert!(b.tail_expr.is_some());
    }

    // =====================================================================
    // New tests: Match expressions
    // =====================================================================

    #[test]
    fn parse_match_basic() {
        let prog = parse(
            r#"
            fn main() {
                match x {
                    1 => "one",
                    2 => "two",
                    _ => "other",
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        assert!(b.tail_expr.is_some());
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::Match { arms, .. } => {
                assert_eq!(arms.len(), 3);
                assert!(matches!(arms[2].pattern.kind, PatternKind::Wildcard));
            }
            _ => panic!("expected match expression"),
        }
    }

    #[test]
    fn parse_match_with_guard() {
        let prog = parse(
            r#"
            fn main() {
                match x {
                    n if n > 0 => "positive",
                    _ => "non-positive",
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::Match { arms, .. } => {
                assert!(arms[0].guard.is_some());
                assert!(matches!(arms[0].pattern.kind, PatternKind::Identifier(ref n) if n == "n"));
            }
            _ => panic!("expected match"),
        }
    }

    #[test]
    fn parse_match_enum_patterns() {
        let prog = parse(
            r#"
            fn main() {
                match result {
                    Ok(value) => value,
                    Err(e) => 0,
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                match &arms[0].pattern.kind {
                    PatternKind::Enum { path, fields } => {
                        assert_eq!(path, &vec!["Ok".to_string()]);
                        assert_eq!(fields.len(), 1);
                    }
                    _ => panic!("expected enum pattern"),
                }
            }
            _ => panic!("expected match"),
        }
    }

    #[test]
    fn parse_match_or_pattern() {
        let prog = parse(
            r#"
            fn main() {
                match status {
                    "active" | "enabled" => true,
                    _ => false,
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::Match { arms, .. } => match &arms[0].pattern.kind {
                PatternKind::Or(pats) => assert_eq!(pats.len(), 2),
                _ => panic!("expected or pattern"),
            },
            _ => panic!("expected match"),
        }
    }

    #[test]
    fn parse_match_struct_pattern() {
        let prog = parse(
            r#"
            fn main() {
                match point {
                    Point { x, y } => x + y,
                    Point { x: a, .. } => a,
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                match &arms[0].pattern.kind {
                    PatternKind::Struct {
                        path,
                        fields,
                        has_rest,
                    } => {
                        assert_eq!(path, &vec!["Point".to_string()]);
                        assert_eq!(fields.len(), 2);
                        assert!(!has_rest);
                    }
                    _ => panic!("expected struct pattern"),
                }
                match &arms[1].pattern.kind {
                    PatternKind::Struct {
                        has_rest, fields, ..
                    } => {
                        assert!(*has_rest);
                        assert_eq!(fields.len(), 1);
                        assert!(fields[0].pattern.is_some()); // x: a
                    }
                    _ => panic!("expected struct pattern"),
                }
            }
            _ => panic!("expected match"),
        }
    }

    #[test]
    fn parse_match_tuple_pattern() {
        let prog = parse(
            r#"
            fn main() {
                match pair {
                    (0, y) => y,
                    (x, 0) => x,
                    _ => 0,
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::Match { arms, .. } => {
                assert_eq!(arms.len(), 3);
                match &arms[0].pattern.kind {
                    PatternKind::Tuple(pats) => assert_eq!(pats.len(), 2),
                    _ => panic!("expected tuple pattern"),
                }
            }
            _ => panic!("expected match"),
        }
    }

    #[test]
    fn parse_match_binding_pattern() {
        let prog = parse(
            r#"
            fn main() {
                match x {
                    n @ 42 => n,
                    _ => 0,
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::Match { arms, .. } => match &arms[0].pattern.kind {
                PatternKind::Binding { name, pattern } => {
                    assert_eq!(name, "n");
                    assert!(matches!(
                        pattern.kind,
                        PatternKind::Literal(Literal::Int(42))
                    ));
                }
                _ => panic!("expected binding pattern"),
            },
            _ => panic!("expected match"),
        }
    }

    // =====================================================================
    // New tests: For/While/Loop
    // =====================================================================

    #[test]
    fn parse_for_loop() {
        let prog = parse(
            r#"
            fn main() {
                for item in items {
                    emit("item", item);
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        assert!(b.tail_expr.is_some());
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::For { pattern, .. } => {
                assert!(matches!(pattern.kind, PatternKind::Identifier(ref n) if n == "item"));
            }
            _ => panic!("expected for loop"),
        }
    }

    #[test]
    fn parse_for_with_tuple_pattern() {
        let prog = parse(
            r#"
            fn main() {
                for (key, value) in pairs {
                    emit("kv", key);
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::For { pattern, .. } => match &pattern.kind {
                PatternKind::Tuple(pats) => assert_eq!(pats.len(), 2),
                _ => panic!("expected tuple pattern"),
            },
            _ => panic!("expected for loop"),
        }
    }

    #[test]
    fn parse_while_loop() {
        let prog = parse(
            r#"
            fn main() {
                while x > 0 {
                    x = x - 1;
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        assert!(b.tail_expr.is_some());
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::While { condition, .. } => {
                assert!(matches!(condition.kind, ExprKind::Binary { .. }));
            }
            _ => panic!("expected while loop"),
        }
    }

    #[test]
    fn parse_loop() {
        let prog = parse(
            r#"
            fn main() {
                loop {
                    emit("tick", 1);
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        assert!(b.tail_expr.is_some());
        assert!(matches!(
            b.tail_expr.as_ref().unwrap().kind,
            ExprKind::Loop { .. }
        ));
    }

    // =====================================================================
    // New tests: Break / Continue / Throw
    // =====================================================================

    #[test]
    fn parse_break_continue() {
        let prog = parse(
            r#"
            fn main() {
                loop {
                    break;
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        // The loop is a tail expression
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::Loop { body } => {
                assert_eq!(body.stmts.len(), 1);
                assert!(matches!(body.stmts[0], Stmt::Break(_)));
            }
            _ => panic!("expected loop"),
        }
    }

    #[test]
    fn parse_break_with_value() {
        let prog = parse(
            r#"
            fn main() {
                loop {
                    break 42;
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::Loop { body } => match &body.stmts[0] {
                Stmt::Break(s) => assert!(s.value.is_some()),
                _ => panic!("expected break"),
            },
            _ => panic!("expected loop"),
        }
    }

    #[test]
    fn parse_continue_stmt() {
        let prog = parse(
            r#"
            fn main() {
                while true {
                    continue;
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::While { body, .. } => {
                assert_eq!(body.stmts.len(), 1);
                assert!(matches!(body.stmts[0], Stmt::Continue(_)));
            }
            _ => panic!("expected while"),
        }
    }

    #[test]
    fn parse_throw_stmt() {
        let prog = parse(
            r#"
            fn main() {
                throw ToolError("something went wrong");
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        assert_eq!(b.stmts.len(), 1);
        assert!(matches!(b.stmts[0], Stmt::Throw(_)));
    }

    // =====================================================================
    // New tests: Try/Catch
    // =====================================================================

    #[test]
    fn parse_try_catch() {
        let prog = parse(
            r#"
            fn main() {
                try {
                    risky_call();
                } catch {
                    handle_error();
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        assert!(b.tail_expr.is_some());
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::TryCatch { catches, .. } => {
                assert_eq!(catches.len(), 1);
                assert!(catches[0].error_type.is_none()); // bare catch
            }
            _ => panic!("expected try/catch"),
        }
    }

    #[test]
    fn parse_try_catch_typed() {
        let prog = parse(
            r#"
            fn main() {
                try {
                    risky_call();
                } catch ToolError(e) {
                    handle_error();
                } catch {
                    fallback();
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::TryCatch { catches, .. } => {
                assert_eq!(catches.len(), 2);
                assert!(catches[0].error_type.is_some());
                assert_eq!(catches[0].binding.as_deref(), Some("e"));
                assert!(catches[1].error_type.is_none());
            }
            _ => panic!("expected try/catch"),
        }
    }

    // =====================================================================
    // New tests: Closures
    // =====================================================================

    #[test]
    fn parse_closure_single_param() {
        let prog = parse(r#"fn main() { let f = |x| x + 1; }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Closure {
                    params,
                    return_type,
                    ..
                } => {
                    assert_eq!(params.len(), 1);
                    assert_eq!(params[0].name, "x");
                    assert!(return_type.is_none());
                }
                _ => panic!("expected closure"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_closure_typed_params() {
        let prog = parse(r#"fn main() { let f = |a: Int, b: Int| -> Int { a + b }; }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Closure {
                    params,
                    return_type,
                    ..
                } => {
                    assert_eq!(params.len(), 2);
                    assert!(params[0].type_ann.is_some());
                    assert!(return_type.is_some());
                }
                _ => panic!("expected closure"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_closure_empty_params() {
        let prog = parse(r#"fn main() { let f = || 42; }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Closure { params, .. } => {
                    assert!(params.is_empty());
                }
                _ => panic!("expected closure"),
            },
            _ => panic!("expected let"),
        }
    }

    // =====================================================================
    // New tests: Pipe, Propagate, NilCoalesce
    // =====================================================================

    #[test]
    fn parse_pipe_operator() {
        let prog = parse(r#"fn main() { let x = data |> transform() |> format(); }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Pipe { left, .. } => {
                    // Left-associative: (data |> transform()) |> format()
                    assert!(matches!(left.kind, ExprKind::Pipe { .. }));
                }
                _ => panic!("expected pipe"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_propagate_operator() {
        let prog = parse(r#"fn main() { let x = risky()?; }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Propagate(inner) => {
                    assert!(matches!(inner.kind, ExprKind::Call { .. }));
                }
                _ => panic!(
                    "expected propagate, got {:?}",
                    s.initializer.as_ref().unwrap().kind
                ),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_nil_coalesce() {
        let prog = parse(r#"fn main() { let x = a ?? b ?? c; }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::NilCoalesce { left, .. } => {
                    // Left-associative: (a ?? b) ?? c
                    assert!(matches!(left.kind, ExprKind::NilCoalesce { .. }));
                }
                _ => panic!("expected nil coalesce"),
            },
            _ => panic!("expected let"),
        }
    }

    // =====================================================================
    // New tests: Range, Cast, Path
    // =====================================================================

    #[test]
    fn parse_range_exclusive() {
        let prog = parse(r#"fn main() { let r = 0..10; }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Range {
                    start,
                    end,
                    inclusive,
                } => {
                    assert!(start.is_some());
                    assert!(end.is_some());
                    assert!(!inclusive);
                }
                _ => panic!("expected range"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_range_inclusive() {
        let prog = parse(r#"fn main() { let r = 0..=10; }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Range { inclusive, .. } => {
                    assert!(inclusive);
                }
                _ => panic!("expected range"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_type_cast() {
        let prog = parse(r#"fn main() { let x = 42 as Float; }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Cast { target, .. } => {
                    assert!(matches!(target.kind, TypeKind::Named(ref n) if n == "Float"));
                }
                _ => panic!("expected cast"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_path_expression() {
        let prog = parse(r#"fn main() { let x = std::json::parse(text); }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Call { callee, args } => {
                    match &callee.kind {
                        ExprKind::Path(segments) => {
                            assert_eq!(segments, &vec!["std", "json", "parse"]);
                        }
                        _ => panic!("expected path callee"),
                    }
                    assert_eq!(args.len(), 1);
                }
                _ => panic!("expected call"),
            },
            _ => panic!("expected let"),
        }
    }

    // =====================================================================
    // New tests: Await, Tuple, StructLiteral
    // =====================================================================

    #[test]
    fn parse_await_expr() {
        let prog = parse(r#"fn main() { let x = my_agent.execute().await; }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Await(inner) => {
                    assert!(matches!(inner.kind, ExprKind::MethodCall { .. }));
                }
                _ => panic!("expected await"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_tuple_expr() {
        let prog = parse(r#"fn main() { let t = (1, "two", true); }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Tuple(elems) => assert_eq!(elems.len(), 3),
                _ => panic!("expected tuple"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_empty_tuple() {
        let prog = parse(r#"fn main() { let t = (); }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Tuple(elems) => assert!(elems.is_empty()),
                _ => panic!("expected empty tuple"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_struct_literal() {
        let prog = parse(r#"fn main() { let p = Point { x: 1, y: 2 }; }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::StructLiteral { name, fields } => {
                    assert_eq!(name, &vec!["Point".to_string()]);
                    assert_eq!(fields.len(), 2);
                    assert_eq!(fields[0].name, "x");
                    assert_eq!(fields[1].name, "y");
                }
                _ => panic!("expected struct literal"),
            },
            _ => panic!("expected let"),
        }
    }

    // =====================================================================
    // New tests: Complex/combined expressions
    // =====================================================================

    #[test]
    fn parse_await_with_propagate() {
        let prog = parse(r#"fn main() { let x = my_agent.execute().await?; }"#);
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Propagate(inner) => {
                    assert!(matches!(inner.kind, ExprKind::Await(_)));
                }
                _ => panic!("expected propagate(await)"),
            },
            _ => panic!("expected let"),
        }
    }

    #[test]
    fn parse_for_with_range() {
        let prog = parse(
            r#"
            fn main() {
                for i in 0..10 {
                    emit("i", i);
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::For { iterable, .. } => {
                assert!(matches!(iterable.kind, ExprKind::Range { .. }));
            }
            _ => panic!("expected for loop"),
        }
    }

    #[test]
    fn parse_block_ending_no_semicolon() {
        // Control flow expressions that end with } don't need ; as statements
        let prog = parse(
            r#"
            fn main() {
                for x in items {
                    emit("x", x);
                }
                let y = 5;
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        // for loop is a statement (ExprStmt), followed by let
        assert_eq!(b.stmts.len(), 2);
    }

    #[test]
    fn parse_match_with_block_arms() {
        let prog = parse(
            r#"
            fn main() {
                match status {
                    "ok" => {
                        let x = 1;
                        x
                    },
                    _ => 0,
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                assert!(matches!(arms[0].body.kind, ExprKind::Block(_)));
            }
            _ => panic!("expected match"),
        }
    }

    #[test]
    fn parse_match_path_variant() {
        let prog = parse(
            r#"
            fn main() {
                match shape {
                    Shape::Circle(r) => r * r,
                    Shape::Rectangle(w, h) => w * h,
                }
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.tail_expr.as_ref().unwrap().kind {
            ExprKind::Match { arms, .. } => match &arms[0].pattern.kind {
                PatternKind::Enum { path, fields } => {
                    assert_eq!(path, &vec!["Shape".to_string(), "Circle".to_string()]);
                    assert_eq!(fields.len(), 1);
                }
                _ => panic!("expected enum pattern"),
            },
            _ => panic!("expected match"),
        }
    }

    // =====================================================================
    // Step 12: New tests for prefix await, return expression, union types
    // =====================================================================

    #[test]
    fn parse_prefix_await() {
        let prog = parse(
            r#"
            fn main() {
                let result = await emit("channel", 42);
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Await(inner) => {
                    assert!(matches!(inner.kind, ExprKind::Call { .. }));
                }
                _ => panic!("expected await expression"),
            },
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_prefix_await_chained() {
        // Prefix await with method call and propagation
        let prog = parse(
            r#"
            fn main() {
                let x = await foo.bar(1, 2);
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => {
                assert!(matches!(
                    s.initializer.as_ref().unwrap().kind,
                    ExprKind::Await(_)
                ));
            }
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_return_in_match_arm() {
        let prog = parse(
            r#"
            fn main() {
                let x = match op {
                    "add" => a + b,
                    _ => return Err("unknown"),
                };
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(s) => match &s.initializer.as_ref().unwrap().kind {
                ExprKind::Match { arms, .. } => {
                    // First arm: normal expression
                    assert!(matches!(arms[0].body.kind, ExprKind::Binary { .. }));
                    // Second arm: return expression
                    assert!(matches!(arms[1].body.kind, ExprKind::Return(Some(_))));
                }
                _ => panic!("expected match"),
            },
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_return_no_value_in_match() {
        let prog = parse(
            r#"
            fn main() {
                match x {
                    0 => return,
                    _ => 42,
                };
            }
        "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Expr(s) => match &s.expr.kind {
                ExprKind::Match { arms, .. } => {
                    assert!(matches!(arms[0].body.kind, ExprKind::Return(None)));
                }
                _ => panic!("expected match"),
            },
            _ => panic!("expected expr statement"),
        }
    }

    // ========================================================================
    // Listen expression tests
    // ========================================================================

    #[test]
    fn parse_listen_single_handler() {
        let prog = parse(
            r#"
            fn main() {
                let result = listen MyAgent.execute("prompt") {
                    "progress" => |msg| {
                        emit("log", msg);
                    },
                };
            }
            "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(l) => match &l.initializer.as_ref().unwrap().kind {
                ExprKind::Listen { call, handlers } => {
                    // Verify it's a method call on the agent
                    match &call.kind {
                        ExprKind::MethodCall { object, method, args, .. } => {
                            assert!(matches!(&object.kind, ExprKind::Identifier(name) if name == "MyAgent"));
                            assert_eq!(method, "execute");
                            assert_eq!(args.len(), 1);
                        }
                        _ => panic!("expected method call in listen"),
                    }
                    assert_eq!(handlers.len(), 1);
                    assert_eq!(handlers[0].message_type, "progress");
                    assert_eq!(handlers[0].param.name, "msg");
                }
                _ => panic!("expected Listen expression"),
            },
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_listen_multiple_handlers() {
        let prog = parse(
            r#"
            fn main() {
                let result = listen Agent.execute("do work") {
                    "progress" => |p| {
                        emit("prog", p);
                    },
                    "question" => |q| {
                        "answer"
                    },
                    "approval" => |req| {
                        "yes"
                    },
                };
            }
            "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(l) => match &l.initializer.as_ref().unwrap().kind {
                ExprKind::Listen { handlers, .. } => {
                    assert_eq!(handlers.len(), 3);
                    assert_eq!(handlers[0].message_type, "progress");
                    assert_eq!(handlers[0].param.name, "p");
                    assert_eq!(handlers[1].message_type, "question");
                    assert_eq!(handlers[1].param.name, "q");
                    assert_eq!(handlers[2].message_type, "approval");
                    assert_eq!(handlers[2].param.name, "req");
                }
                _ => panic!("expected Listen expression"),
            },
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_listen_typed_handler() {
        let prog = parse(
            r#"
            fn main() {
                let result = listen Agent.execute("prompt") {
                    "question" => |q: AgentQuestion| {
                        "answer"
                    },
                };
            }
            "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(l) => match &l.initializer.as_ref().unwrap().kind {
                ExprKind::Listen { handlers, .. } => {
                    assert_eq!(handlers.len(), 1);
                    assert_eq!(handlers[0].param.name, "q");
                    // Verify type annotation is present
                    assert!(handlers[0].param.type_ann.is_some());
                }
                _ => panic!("expected Listen expression"),
            },
            _ => panic!("expected let statement"),
        }
    }

    #[test]
    fn parse_listen_no_trailing_comma() {
        let prog = parse(
            r#"
            fn main() {
                let result = listen Agent.execute("prompt") {
                    "progress" => |msg| {
                        emit("log", msg);
                    }
                };
            }
            "#,
        );
        let f = get_fn(&prog);
        let b = body(f);
        match &b.stmts[0] {
            Stmt::Let(l) => match &l.initializer.as_ref().unwrap().kind {
                ExprKind::Listen { handlers, .. } => {
                    assert_eq!(handlers.len(), 1);
                }
                _ => panic!("expected Listen expression"),
            },
            _ => panic!("expected let statement"),
        }
    }
}
