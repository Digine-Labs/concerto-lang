use crate::ast::*;
use crate::lexer::token::TokenKind;

use super::Parser;

impl Parser {
    /// Parse a top-level declaration.
    pub(super) fn parse_declaration(&mut self) -> Option<Declaration> {
        // Collect any leading decorators
        let decorators = self.parse_decorators();

        match self.peek() {
            TokenKind::Fn | TokenKind::Async => self.parse_function_decl(false, decorators),
            TokenKind::Pub => {
                self.advance(); // consume 'pub'
                match self.peek() {
                    TokenKind::Fn | TokenKind::Async => self.parse_function_decl(true, decorators),
                    _ => {
                        let span = self.current_span();
                        self.diagnostics
                            .error("expected 'fn' or 'async fn' after 'pub'", span);
                        None
                    }
                }
            }
            TokenKind::Agent => self.parse_agent_decl(decorators),
            TokenKind::Tool => self.parse_tool_decl(),
            TokenKind::Schema => self.parse_schema_decl(decorators),
            TokenKind::Pipeline => self.parse_pipeline_decl(),
            TokenKind::Struct => self.parse_struct_decl(),
            TokenKind::Enum => self.parse_enum_decl(),
            TokenKind::Trait => self.parse_trait_decl(),
            TokenKind::Impl => self.parse_impl_decl(),
            TokenKind::Use => self.parse_use_decl(),
            TokenKind::Mod => self.parse_module_decl(),
            TokenKind::Const => self.parse_const_decl(),
            TokenKind::Type => self.parse_type_alias_decl(),
            TokenKind::HashMap => self.parse_hashmap_decl(),
            TokenKind::Ledger => self.parse_ledger_decl(),
            TokenKind::Memory => self.parse_memory_decl(),
            TokenKind::Mcp => self.parse_mcp_decl(),
            TokenKind::Host => self.parse_host_decl(decorators),
            _ => {
                let span = self.current_span();
                self.diagnostics.error(
                    format!("expected declaration, found {:?}", self.peek()),
                    span,
                );
                None
            }
        }
    }

    // ========================================================================
    // Decorators: @name or @name(args)
    // ========================================================================

    /// Parse zero or more decorators.
    fn parse_decorators(&mut self) -> Vec<Decorator> {
        let mut decorators = Vec::new();
        while self.peek() == TokenKind::At {
            if let Some(dec) = self.parse_decorator() {
                decorators.push(dec);
            }
        }
        decorators
    }

    /// Parse a single decorator: `@name` or `@name(args)`.
    fn parse_decorator(&mut self) -> Option<Decorator> {
        let start = self.current_span();
        self.expect(TokenKind::At)?;

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        let mut args = Vec::new();
        if self.eat(TokenKind::LeftParen) {
            if self.peek() != TokenKind::RightParen {
                loop {
                    let arg = self.parse_decorator_arg()?;
                    args.push(arg);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RightParen)?;
        }

        let span = start.merge(&self.previous_span());
        Some(Decorator { name, args, span })
    }

    /// Parse a single decorator argument (positional or named).
    fn parse_decorator_arg(&mut self) -> Option<DecoratorArg> {
        // Peek ahead: if Identifier followed by Colon, it's named
        if self.peek() == TokenKind::Identifier && self.peek_next() == TokenKind::Colon {
            let start = self.current_span();
            let name_token = self.advance().clone();
            let name = name_token.lexeme.clone();
            self.advance(); // consume ':'
            let value = self.parse_expression()?;
            let span = start.merge(&value.span);
            Some(DecoratorArg::Named { name, value, span })
        } else {
            let expr = self.parse_expression()?;
            Some(DecoratorArg::Positional(expr))
        }
    }

    // ========================================================================
    // Function declaration
    // ========================================================================

    /// Parse `[pub] [async] fn name(params) [-> Type] { body }`
    fn parse_function_decl(
        &mut self,
        is_public: bool,
        decorators: Vec<Decorator>,
    ) -> Option<Declaration> {
        let start = if is_public {
            self.previous_span()
        } else if let Some(first) = decorators.first() {
            first.span.clone()
        } else {
            self.current_span()
        };

        let is_async = self.eat(TokenKind::Async);
        self.expect(TokenKind::Fn)?;

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        // Parameters (with self handling)
        self.expect(TokenKind::LeftParen)?;
        let (self_param, params) = self.parse_method_param_list()?;
        self.expect(TokenKind::RightParen)?;

        // Optional return type
        let return_type = if self.eat(TokenKind::Arrow) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        // Body (required for top-level functions)
        let body = Some(self.parse_block()?);
        let span = start.merge(&self.previous_span());

        Some(Declaration::Function(FunctionDecl {
            name,
            decorators,
            self_param,
            params,
            return_type,
            body,
            is_public,
            is_async,
            span,
        }))
    }

    /// Parse a method inside a tool/trait/impl/mcp block.
    /// `require_body` controls whether `{ ... }` is mandatory or `;` is also accepted.
    fn parse_method(
        &mut self,
        decorators: Vec<Decorator>,
        require_body: bool,
    ) -> Option<FunctionDecl> {
        let start = if let Some(first) = decorators.first() {
            first.span.clone()
        } else {
            self.current_span()
        };

        let is_public = self.eat(TokenKind::Pub);
        let is_async = self.eat(TokenKind::Async);
        self.expect(TokenKind::Fn)?;

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::LeftParen)?;
        let (self_param, params) = self.parse_method_param_list()?;
        self.expect(TokenKind::RightParen)?;

        let return_type = if self.eat(TokenKind::Arrow) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let body = if require_body {
            Some(self.parse_block()?)
        } else if self.peek() == TokenKind::LeftBrace {
            // Optional body (trait default impl)
            Some(self.parse_block()?)
        } else {
            // Signature only (mcp, trait without default)
            self.expect(TokenKind::Semicolon)?;
            None
        };

        let span = start.merge(&self.previous_span());
        Some(FunctionDecl {
            name,
            decorators,
            self_param,
            params,
            return_type,
            body,
            is_public,
            is_async,
            span,
        })
    }

    // ========================================================================
    // Parameter lists (with self handling)
    // ========================================================================

    /// Parse a parameter list that may start with `self` or `mut self`.
    fn parse_method_param_list(&mut self) -> Option<(SelfParam, Vec<Param>)> {
        if self.peek() == TokenKind::RightParen {
            return Some((SelfParam::None, Vec::new()));
        }

        // Check for self / mut self
        let self_param = if self.peek() == TokenKind::SelfKw {
            self.advance(); // consume 'self'
            if self.peek() == TokenKind::Comma {
                self.advance(); // consume ','
            }
            SelfParam::Immutable
        } else if self.peek() == TokenKind::Mut && self.peek_next() == TokenKind::SelfKw {
            self.advance(); // consume 'mut'
            self.advance(); // consume 'self'
            if self.peek() == TokenKind::Comma {
                self.advance(); // consume ','
            }
            SelfParam::Mutable
        } else {
            SelfParam::None
        };

        let mut params = Vec::new();
        if self.peek() == TokenKind::RightParen {
            return Some((self_param, params));
        }

        loop {
            let param = self.parse_param()?;
            params.push(param);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        Some((self_param, params))
    }

    /// Parse a single parameter: `name: Type [= default]`
    fn parse_param(&mut self) -> Option<Param> {
        let start = self.current_span();
        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        // Optional type annotation
        let type_ann = if self.eat(TokenKind::Colon) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        // Optional default value
        let default = if self.eat(TokenKind::Equal) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        let span = start.merge(&self.previous_span());
        Some(Param {
            name,
            type_ann,
            default,
            span,
        })
    }

    /// Parse a type annotation.
    /// Handles named types, generics, string literals, and union types.
    /// Union types are only parsed when the first element is a string literal,
    /// to avoid ambiguity with closure parameter `|` delimiters.
    pub(super) fn parse_type_annotation(&mut self) -> Option<TypeAnnotation> {
        let first = self.parse_single_type_annotation()?;

        // Check for union type: "literal" | "literal" | ...
        // Only enter union parsing when the first element is a string literal,
        // because `|` is ambiguous with closure param delimiters for named types.
        if matches!(first.kind, types::TypeKind::StringLiteral(_)) && self.peek() == TokenKind::Pipe
        {
            let start = first.span.clone();
            let mut variants = vec![first];
            while self.eat(TokenKind::Pipe) {
                variants.push(self.parse_single_type_annotation()?);
            }
            let span = start.merge(&self.previous_span());
            return Some(TypeAnnotation {
                kind: types::TypeKind::Union(variants),
                span,
            });
        }

        Some(first)
    }

    /// Parse a single type annotation (without union).
    fn parse_single_type_annotation(&mut self) -> Option<TypeAnnotation> {
        let start = self.current_span();

        // String literal type: "legal", "technical", etc.
        if self.peek() == TokenKind::StringLiteral {
            let token = self.advance().clone();
            return Some(TypeAnnotation {
                kind: types::TypeKind::StringLiteral(token.lexeme.clone()),
                span: token.span,
            });
        }

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        // Check for generic arguments: Type<A, B>
        if self.peek() == TokenKind::Less {
            self.advance(); // consume '<'
            let mut args = Vec::new();
            loop {
                let arg = self.parse_type_annotation()?;
                args.push(arg);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::Greater)?;
            let span = start.merge(&self.previous_span());
            Some(TypeAnnotation {
                kind: types::TypeKind::Generic { name, args },
                span,
            })
        } else {
            let span = start.merge(&self.previous_span());
            Some(TypeAnnotation {
                kind: types::TypeKind::Named(name),
                span,
            })
        }
    }

    // ========================================================================
    // Config block helpers
    // ========================================================================

    /// Parse a config block body: `{ name: expr, ... }`.
    fn parse_config_fields(&mut self) -> Option<Vec<ConfigField>> {
        self.expect(TokenKind::LeftBrace)?;
        let mut fields = Vec::new();

        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            let field = self.parse_config_field()?;
            fields.push(field);
            self.eat(TokenKind::Comma); // trailing comma optional
        }

        self.expect(TokenKind::RightBrace)?;
        Some(fields)
    }

    /// Parse a single config field: `name: expr`
    fn parse_config_field(&mut self) -> Option<ConfigField> {
        let start = self.current_span();
        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();
        self.expect(TokenKind::Colon)?;
        let value = self.parse_expression()?;
        let span = start.merge(&self.previous_span());
        Some(ConfigField { name, value, span })
    }

    /// Parse typed fields for struct/schema: `{ [pub] name[?]: Type [= default], ... }`
    fn parse_typed_fields(&mut self) -> Option<Vec<FieldDecl>> {
        self.expect(TokenKind::LeftBrace)?;
        let mut fields = Vec::new();

        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            let field = self.parse_field_decl()?;
            fields.push(field);
            self.eat(TokenKind::Comma); // trailing comma optional
        }

        self.expect(TokenKind::RightBrace)?;
        Some(fields)
    }

    /// Parse a single typed field: `[pub] name[?]: Type [= default]`
    fn parse_field_decl(&mut self) -> Option<FieldDecl> {
        let start = self.current_span();
        let is_public = self.eat(TokenKind::Pub);

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        // Optional `?` for schema optional fields
        let is_optional = self.eat(TokenKind::Question);

        self.expect(TokenKind::Colon)?;
        let type_ann = self.parse_type_annotation()?;

        // Optional default value
        let default = if self.eat(TokenKind::Equal) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        let span = start.merge(&self.previous_span());
        Some(FieldDecl {
            name,
            type_ann,
            default,
            is_public,
            is_optional,
            span,
        })
    }

    // ========================================================================
    // agent declaration
    // ========================================================================

    /// Parse `[decorators] agent Name { fields... }`
    fn parse_agent_decl(&mut self, decorators: Vec<Decorator>) -> Option<Declaration> {
        let start = if let Some(first) = decorators.first() {
            first.span.clone()
        } else {
            self.current_span()
        };
        self.advance(); // consume 'agent'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        let fields = self.parse_config_fields()?;
        let span = start.merge(&self.previous_span());

        Some(Declaration::Agent(AgentDecl {
            name,
            decorators,
            fields,
            span,
        }))
    }

    // ========================================================================
    // tool declaration
    // ========================================================================

    /// Parse `tool Name { description: "...", fields..., methods... }`
    fn parse_tool_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'tool'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::LeftBrace)?;

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            // Methods start with decorators (@describe, @param) or `pub` or `fn`
            if self.peek() == TokenKind::At
                || self.peek() == TokenKind::Pub
                || self.peek() == TokenKind::Fn
                || self.peek() == TokenKind::Async
            {
                let method_decorators = self.parse_decorators();
                let method = self.parse_method(method_decorators, true)?;
                methods.push(method);
            } else {
                // Config field
                let field = self.parse_config_field()?;
                fields.push(field);
                self.eat(TokenKind::Comma);
            }
        }

        self.expect(TokenKind::RightBrace)?;
        let span = start.merge(&self.previous_span());

        Some(Declaration::Tool(ToolDecl {
            name,
            fields,
            methods,
            span,
        }))
    }

    // ========================================================================
    // schema declaration
    // ========================================================================

    /// Parse `[decorators] schema Name { typed fields... }`
    fn parse_schema_decl(&mut self, decorators: Vec<Decorator>) -> Option<Declaration> {
        let start = if let Some(first) = decorators.first() {
            first.span.clone()
        } else {
            self.current_span()
        };
        self.advance(); // consume 'schema'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        let fields = self.parse_typed_fields()?;
        let span = start.merge(&self.previous_span());

        Some(Declaration::Schema(SchemaDecl {
            name,
            decorators,
            fields,
            span,
        }))
    }

    // ========================================================================
    // pipeline declaration
    // ========================================================================

    /// Parse `pipeline Name { stage ... stage ... }`
    fn parse_pipeline_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'pipeline'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::LeftBrace)?;
        let mut stages = Vec::new();

        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            let stage_decorators = self.parse_decorators();
            let stage = self.parse_stage(stage_decorators)?;
            stages.push(stage);
        }

        self.expect(TokenKind::RightBrace)?;
        let span = start.merge(&self.previous_span());

        Some(Declaration::Pipeline(PipelineDecl { name, stages, span }))
    }

    /// Parse `[decorators] stage name(params) [-> Type] { body }`
    fn parse_stage(&mut self, decorators: Vec<Decorator>) -> Option<StageDecl> {
        let start = if let Some(first) = decorators.first() {
            first.span.clone()
        } else {
            self.current_span()
        };
        self.expect(TokenKind::Stage)?;

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::LeftParen)?;
        let params = self.parse_param_list()?;
        self.expect(TokenKind::RightParen)?;

        let return_type = if self.eat(TokenKind::Arrow) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        let span = start.merge(&self.previous_span());

        Some(StageDecl {
            name,
            decorators,
            params,
            return_type,
            body,
            span,
        })
    }

    /// Parse a simple comma-separated parameter list (no self).
    fn parse_param_list(&mut self) -> Option<Vec<Param>> {
        let mut params = Vec::new();
        if self.peek() == TokenKind::RightParen {
            return Some(params);
        }

        loop {
            let param = self.parse_param()?;
            params.push(param);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        Some(params)
    }

    // ========================================================================
    // struct declaration
    // ========================================================================

    /// Parse `struct Name { fields... }`
    fn parse_struct_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'struct'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        let fields = self.parse_typed_fields()?;
        let span = start.merge(&self.previous_span());

        Some(Declaration::Struct(StructDecl { name, fields, span }))
    }

    // ========================================================================
    // enum declaration
    // ========================================================================

    /// Parse `enum Name { Variant, Variant(T), Variant { f: T }, ... }`
    fn parse_enum_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'enum'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::LeftBrace)?;
        let mut variants = Vec::new();

        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            let variant = self.parse_enum_variant()?;
            variants.push(variant);
            self.eat(TokenKind::Comma); // trailing comma optional
        }

        self.expect(TokenKind::RightBrace)?;
        let span = start.merge(&self.previous_span());

        Some(Declaration::Enum(EnumDecl {
            name,
            variants,
            span,
        }))
    }

    /// Parse a single enum variant.
    fn parse_enum_variant(&mut self) -> Option<EnumVariant> {
        let start = self.current_span();
        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        let kind = if self.peek() == TokenKind::LeftParen {
            // Tuple variant: Variant(Type1, Type2)
            self.advance(); // consume '('
            let mut types = Vec::new();
            if self.peek() != TokenKind::RightParen {
                loop {
                    let ty = self.parse_type_annotation()?;
                    types.push(ty);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RightParen)?;
            EnumVariantKind::Tuple(types)
        } else if self.peek() == TokenKind::LeftBrace {
            // Struct variant: Variant { field: Type, ... }
            let fields = self.parse_typed_fields()?;
            EnumVariantKind::Struct(fields)
        } else {
            EnumVariantKind::Unit
        };

        let span = start.merge(&self.previous_span());
        Some(EnumVariant { name, kind, span })
    }

    // ========================================================================
    // trait declaration
    // ========================================================================

    /// Parse `trait Name { fn method(self) -> Type; ... }`
    fn parse_trait_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'trait'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::LeftBrace)?;
        let mut methods = Vec::new();

        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            let decorators = self.parse_decorators();
            let method = self.parse_method(decorators, false)?;
            methods.push(method);
        }

        self.expect(TokenKind::RightBrace)?;
        let span = start.merge(&self.previous_span());

        Some(Declaration::Trait(TraitDecl {
            name,
            methods,
            span,
        }))
    }

    // ========================================================================
    // impl declaration
    // ========================================================================

    /// Parse `impl [Trait for] Type { methods... }`
    fn parse_impl_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'impl'

        let first_name_token = self.expect(TokenKind::Identifier)?;
        let first_name = first_name_token.lexeme.clone();

        // Check for `Trait for Type` pattern
        let (trait_name, target) = if self.peek() == TokenKind::For {
            self.advance(); // consume 'for'
            let target_token = self.expect(TokenKind::Identifier)?;
            let target = target_token.lexeme.clone();
            (Some(first_name), target)
        } else {
            (None, first_name)
        };

        self.expect(TokenKind::LeftBrace)?;
        let mut methods = Vec::new();

        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            let decorators = self.parse_decorators();
            let method = self.parse_method(decorators, true)?;
            methods.push(method);
        }

        self.expect(TokenKind::RightBrace)?;
        let span = start.merge(&self.previous_span());

        Some(Declaration::Impl(ImplDecl {
            target,
            trait_name,
            methods,
            span,
        }))
    }

    // ========================================================================
    // use declaration
    // ========================================================================

    /// Parse `use path::to::item [as alias];`
    fn parse_use_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'use'

        let mut path = Vec::new();
        let name_token = self.expect(TokenKind::Identifier)?;
        path.push(name_token.lexeme.clone());

        while self.eat(TokenKind::ColonColon) {
            let segment = self.expect(TokenKind::Identifier)?;
            path.push(segment.lexeme.clone());
        }

        let alias = if self.eat(TokenKind::As) {
            let alias_token = self.expect(TokenKind::Identifier)?;
            Some(alias_token.lexeme.clone())
        } else {
            None
        };

        self.expect(TokenKind::Semicolon)?;
        let span = start.merge(&self.previous_span());

        Some(Declaration::Use(UseDecl { path, alias, span }))
    }

    // ========================================================================
    // module declaration
    // ========================================================================

    /// Parse `mod name { ... }` or `mod name;`
    fn parse_module_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'mod'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        let body = if self.peek() == TokenKind::LeftBrace {
            self.advance(); // consume '{'
            let mut declarations = Vec::new();
            while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
                match self.parse_declaration() {
                    Some(decl) => declarations.push(decl),
                    None => self.synchronize(),
                }
            }
            self.expect(TokenKind::RightBrace)?;
            Some(declarations)
        } else {
            self.expect(TokenKind::Semicolon)?;
            None
        };

        let span = start.merge(&self.previous_span());
        Some(Declaration::Module(ModuleDecl { name, body, span }))
    }

    // ========================================================================
    // const declaration
    // ========================================================================

    /// Parse `const NAME: Type = value;`
    fn parse_const_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'const'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::Colon)?;
        let type_ann = self.parse_type_annotation()?;
        self.expect(TokenKind::Equal)?;
        let value = self.parse_expression()?;
        self.expect(TokenKind::Semicolon)?;

        let span = start.merge(&self.previous_span());
        Some(Declaration::Const(ConstDecl {
            name,
            type_ann,
            value,
            span,
        }))
    }

    // ========================================================================
    // type alias declaration
    // ========================================================================

    /// Parse `type Name = Type;`
    fn parse_type_alias_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'type'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::Equal)?;
        let type_ann = self.parse_type_annotation()?;
        self.expect(TokenKind::Semicolon)?;

        let span = start.merge(&self.previous_span());
        Some(Declaration::TypeAlias(TypeAliasDecl {
            name,
            type_ann,
            span,
        }))
    }

    // ========================================================================
    // hashmap declaration
    // ========================================================================

    /// Parse `hashmap name: Type = expr;`
    fn parse_hashmap_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'hashmap'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::Colon)?;
        let type_ann = self.parse_type_annotation()?;
        self.expect(TokenKind::Equal)?;
        let initializer = self.parse_expression()?;
        self.expect(TokenKind::Semicolon)?;

        let span = start.merge(&self.previous_span());
        Some(Declaration::HashMap(HashMapDecl {
            name,
            type_ann,
            initializer,
            span,
        }))
    }

    // ========================================================================
    // ledger declaration
    // ========================================================================

    /// Parse `ledger name: Ledger = Ledger::new();`
    fn parse_ledger_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'ledger'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::Colon)?;
        let type_ann = self.parse_type_annotation()?;
        self.expect(TokenKind::Equal)?;
        let initializer = self.parse_expression()?;
        self.expect(TokenKind::Semicolon)?;

        let span = start.merge(&self.previous_span());
        Some(Declaration::Ledger(LedgerDecl {
            name,
            type_ann,
            initializer,
            span,
        }))
    }

    // ========================================================================
    // memory declaration
    // ========================================================================

    /// Parse `memory name: Memory = Memory::new();`
    fn parse_memory_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'memory'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::Colon)?;
        let type_ann = self.parse_type_annotation()?;
        self.expect(TokenKind::Equal)?;
        let initializer = self.parse_expression()?;
        self.expect(TokenKind::Semicolon)?;

        let span = start.merge(&self.previous_span());
        Some(Declaration::Memory(MemoryDecl {
            name,
            type_ann,
            initializer,
            span,
        }))
    }

    // ========================================================================
    // mcp declaration
    // ========================================================================

    /// Parse `mcp Name { config..., fn declarations... }`
    fn parse_mcp_decl(&mut self) -> Option<Declaration> {
        let start = self.current_span();
        self.advance(); // consume 'mcp'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        self.expect(TokenKind::LeftBrace)?;

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while self.peek() != TokenKind::RightBrace && !self.is_at_end() {
            // Methods start with decorators or `fn`
            if self.peek() == TokenKind::At
                || self.peek() == TokenKind::Fn
                || self.peek() == TokenKind::Async
            {
                let method_decorators = self.parse_decorators();
                // MCP methods have no body (signature only)
                let method = self.parse_method(method_decorators, false)?;
                methods.push(method);
            } else {
                // Config field
                let field = self.parse_config_field()?;
                fields.push(field);
                self.eat(TokenKind::Comma);
            }
        }

        self.expect(TokenKind::RightBrace)?;
        let span = start.merge(&self.previous_span());

        Some(Declaration::Mcp(McpDecl {
            name,
            fields,
            methods,
            span,
        }))
    }

    // ========================================================================
    // host declaration
    // ========================================================================

    /// Parse `[decorators] host Name { fields... }`
    fn parse_host_decl(&mut self, decorators: Vec<Decorator>) -> Option<Declaration> {
        let start = if let Some(first) = decorators.first() {
            first.span.clone()
        } else {
            self.current_span()
        };
        self.advance(); // consume 'host'

        let name_token = self.expect(TokenKind::Identifier)?;
        let name = name_token.lexeme.clone();

        let fields = self.parse_config_fields()?;
        let span = start.merge(&self.previous_span());

        Some(Declaration::Host(HostDecl {
            name,
            decorators,
            fields,
            span,
        }))
    }
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

    fn parse_with_errors(source: &str) -> (Program, concerto_common::DiagnosticBag) {
        let (tokens, _) = Lexer::new(source, "test.conc").tokenize();
        Parser::new(tokens).parse()
    }

    // ===== agent =====

    #[test]
    fn parse_agent_decl() {
        let prog = parse(
            r#"
            agent Classifier {
                provider: openai,
                model: "gpt-4o",
                temperature: 0.3,
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Agent(a) => {
                assert_eq!(a.name, "Classifier");
                assert_eq!(a.fields.len(), 3);
                assert!(a.decorators.is_empty());
            }
            other => panic!("expected Agent, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_agent_with_decorators() {
        let prog = parse(
            r#"
            @timeout(seconds: 60)
            @log(channel: "debug")
            agent MyAgent {
                provider: openai,
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Agent(a) => {
                assert_eq!(a.name, "MyAgent");
                assert_eq!(a.decorators.len(), 2);
                assert_eq!(a.decorators[0].name, "timeout");
                assert_eq!(a.decorators[1].name, "log");
            }
            other => panic!("expected Agent, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== tool =====

    #[test]
    fn parse_tool_decl() {
        let prog = parse(
            r#"
            tool Calculator {
                description: "A calculator",

                @describe("Add two numbers")
                @param("a", "First number")
                @param("b", "Second number")
                pub fn add(self, a: Float, b: Float) -> Float {
                    return a + b;
                }
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Tool(t) => {
                assert_eq!(t.name, "Calculator");
                assert_eq!(t.fields.len(), 1);
                assert_eq!(t.fields[0].name, "description");
                assert_eq!(t.methods.len(), 1);
                assert_eq!(t.methods[0].name, "add");
                assert!(t.methods[0].is_public);
                assert_eq!(t.methods[0].self_param, SelfParam::Immutable);
                assert_eq!(t.methods[0].decorators.len(), 3);
                assert_eq!(t.methods[0].decorators[0].name, "describe");
                assert_eq!(t.methods[0].decorators[1].name, "param");
                assert_eq!(t.methods[0].params.len(), 2); // a, b (self is separate)
            }
            other => panic!("expected Tool, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_tool_mut_self() {
        let prog = parse(
            r#"
            tool Counter {
                description: "Counts things",

                @describe("Increment the counter")
                pub fn increment(mut self) -> Int {
                    return 1;
                }
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Tool(t) => {
                assert_eq!(t.methods[0].self_param, SelfParam::Mutable);
            }
            other => panic!("expected Tool, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== schema =====

    #[test]
    fn parse_schema_decl() {
        let prog = parse(
            r#"
            schema Analysis {
                file_count: Int,
                summary: String,
                tags: Array<String>,
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Schema(s) => {
                assert_eq!(s.name, "Analysis");
                assert_eq!(s.fields.len(), 3);
                assert_eq!(s.fields[0].name, "file_count");
                assert_eq!(s.fields[1].name, "summary");
                assert_eq!(s.fields[2].name, "tags");
            }
            other => panic!("expected Schema, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_schema_optional_field() {
        let prog = parse(
            r#"
            schema Config {
                name: String,
                debug?: Bool,
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Schema(s) => {
                assert!(!s.fields[0].is_optional);
                assert!(s.fields[1].is_optional);
            }
            other => panic!("expected Schema, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_schema_with_defaults() {
        let prog = parse(
            r#"
            schema Settings {
                retries: Int = 3,
                verbose: Bool = false,
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Schema(s) => {
                assert!(s.fields[0].default.is_some());
                assert!(s.fields[1].default.is_some());
            }
            other => panic!("expected Schema, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== pipeline =====

    #[test]
    fn parse_pipeline_decl() {
        let prog = parse(
            r#"
            pipeline DataPipeline {
                stage extract(url: String) -> String {
                    return url;
                }
                stage transform(data: String) -> Int {
                    return 42;
                }
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Pipeline(p) => {
                assert_eq!(p.name, "DataPipeline");
                assert_eq!(p.stages.len(), 2);
                assert_eq!(p.stages[0].name, "extract");
                assert_eq!(p.stages[0].params.len(), 1);
                assert!(p.stages[0].return_type.is_some());
                assert_eq!(p.stages[1].name, "transform");
            }
            other => panic!("expected Pipeline, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== struct =====

    #[test]
    fn parse_struct_decl() {
        let prog = parse(
            r#"
            struct Point {
                x: Float,
                y: Float,
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Struct(s) => {
                assert_eq!(s.name, "Point");
                assert_eq!(s.fields.len(), 2);
                assert_eq!(s.fields[0].name, "x");
                assert_eq!(s.fields[1].name, "y");
            }
            other => panic!("expected Struct, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_struct_pub_fields() {
        let prog = parse(
            r#"
            struct Config {
                pub name: String,
                secret: String,
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Struct(s) => {
                assert!(s.fields[0].is_public);
                assert!(!s.fields[1].is_public);
            }
            other => panic!("expected Struct, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== enum =====

    #[test]
    fn parse_enum_unit_variants() {
        let prog = parse(
            r#"
            enum Color {
                Red,
                Green,
                Blue,
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Enum(e) => {
                assert_eq!(e.name, "Color");
                assert_eq!(e.variants.len(), 3);
                assert_eq!(e.variants[0].name, "Red");
                assert!(matches!(e.variants[0].kind, EnumVariantKind::Unit));
            }
            other => panic!("expected Enum, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_enum_tuple_variants() {
        let prog = parse(
            r#"
            enum Shape {
                Circle(Float),
                Rectangle(Float, Float),
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Enum(e) => {
                assert_eq!(e.variants.len(), 2);
                match &e.variants[0].kind {
                    EnumVariantKind::Tuple(types) => assert_eq!(types.len(), 1),
                    _ => panic!("expected tuple variant"),
                }
                match &e.variants[1].kind {
                    EnumVariantKind::Tuple(types) => assert_eq!(types.len(), 2),
                    _ => panic!("expected tuple variant"),
                }
            }
            other => panic!("expected Enum, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_enum_struct_variant() {
        let prog = parse(
            r#"
            enum Message {
                Quit,
                Text { content: String },
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Enum(e) => {
                assert!(matches!(e.variants[0].kind, EnumVariantKind::Unit));
                match &e.variants[1].kind {
                    EnumVariantKind::Struct(fields) => {
                        assert_eq!(fields.len(), 1);
                        assert_eq!(fields[0].name, "content");
                    }
                    _ => panic!("expected struct variant"),
                }
            }
            other => panic!("expected Enum, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== trait =====

    #[test]
    fn parse_trait_decl() {
        let prog = parse(
            r#"
            trait Printable {
                fn to_string(self) -> String;
                fn print(self) {
                    return;
                }
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Trait(t) => {
                assert_eq!(t.name, "Printable");
                assert_eq!(t.methods.len(), 2);
                // First method: signature only (no body)
                assert_eq!(t.methods[0].name, "to_string");
                assert!(t.methods[0].body.is_none());
                assert_eq!(t.methods[0].self_param, SelfParam::Immutable);
                // Second method: default implementation
                assert_eq!(t.methods[1].name, "print");
                assert!(t.methods[1].body.is_some());
            }
            other => panic!("expected Trait, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== impl =====

    #[test]
    fn parse_impl_decl() {
        let prog = parse(
            r#"
            impl Point {
                pub fn new(x: Float, y: Float) -> Point {
                    return x;
                }
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Impl(i) => {
                assert_eq!(i.target, "Point");
                assert!(i.trait_name.is_none());
                assert_eq!(i.methods.len(), 1);
                assert_eq!(i.methods[0].name, "new");
                assert!(i.methods[0].is_public);
            }
            other => panic!("expected Impl, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_impl_trait_for_type() {
        let prog = parse(
            r#"
            impl Printable for Point {
                fn to_string(self) -> String {
                    return "point";
                }
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Impl(i) => {
                assert_eq!(i.target, "Point");
                assert_eq!(i.trait_name.as_deref(), Some("Printable"));
                assert_eq!(i.methods.len(), 1);
            }
            other => panic!("expected Impl, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== use =====

    #[test]
    fn parse_use_decl() {
        let prog = parse("use std::json::parse;");
        match &prog.declarations[0] {
            Declaration::Use(u) => {
                assert_eq!(u.path, vec!["std", "json", "parse"]);
                assert!(u.alias.is_none());
            }
            other => panic!("expected Use, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_use_with_alias() {
        let prog = parse("use std::collections::HashMap as Map;");
        match &prog.declarations[0] {
            Declaration::Use(u) => {
                assert_eq!(u.path, vec!["std", "collections", "HashMap"]);
                assert_eq!(u.alias.as_deref(), Some("Map"));
            }
            other => panic!("expected Use, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== mod =====

    #[test]
    fn parse_module_decl_external() {
        let prog = parse("mod helpers;");
        match &prog.declarations[0] {
            Declaration::Module(m) => {
                assert_eq!(m.name, "helpers");
                assert!(m.body.is_none());
            }
            other => panic!("expected Module, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_module_decl_inline() {
        let prog = parse(
            r#"
            mod utils {
                fn helper() {}
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Module(m) => {
                assert_eq!(m.name, "utils");
                let body = m.body.as_ref().unwrap();
                assert_eq!(body.len(), 1);
                matches!(&body[0], Declaration::Function(_));
            }
            other => panic!("expected Module, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== const =====

    #[test]
    fn parse_const_decl() {
        let prog = parse("const MAX_RETRIES: Int = 3;");
        match &prog.declarations[0] {
            Declaration::Const(c) => {
                assert_eq!(c.name, "MAX_RETRIES");
                // Type is Int
                match &c.type_ann.kind {
                    types::TypeKind::Named(name) => assert_eq!(name, "Int"),
                    _ => panic!("expected named type"),
                }
            }
            other => panic!("expected Const, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== type alias =====

    #[test]
    fn parse_type_alias_decl() {
        let prog = parse("type StringList = Array<String>;");
        match &prog.declarations[0] {
            Declaration::TypeAlias(t) => {
                assert_eq!(t.name, "StringList");
                match &t.type_ann.kind {
                    types::TypeKind::Generic { name, args } => {
                        assert_eq!(name, "Array");
                        assert_eq!(args.len(), 1);
                    }
                    _ => panic!("expected generic type"),
                }
            }
            other => panic!(
                "expected TypeAlias, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    // ===== hashmap =====

    #[test]
    fn parse_hashmap_decl() {
        let prog = parse(r#"hashmap cache: Map<String, String> = empty_map();"#);
        match &prog.declarations[0] {
            Declaration::HashMap(d) => {
                assert_eq!(d.name, "cache");
                match &d.type_ann.kind {
                    types::TypeKind::Generic { name, args } => {
                        assert_eq!(name, "Map");
                        assert_eq!(args.len(), 2);
                    }
                    _ => panic!("expected generic type"),
                }
            }
            other => panic!("expected HashMap, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== mcp =====

    #[test]
    fn parse_mcp_decl() {
        let prog = parse(
            r#"
            mcp GitHubServer {
                transport: "stdio",
                command: "npx github-mcp",

                @describe("Search repos")
                @param("query", "Search query")
                fn search(query: String) -> String;

                @describe("Get file")
                @param("path", "File path")
                fn get_file(path: String) -> String;
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Mcp(m) => {
                assert_eq!(m.name, "GitHubServer");
                assert_eq!(m.fields.len(), 2);
                assert_eq!(m.fields[0].name, "transport");
                assert_eq!(m.fields[1].name, "command");
                assert_eq!(m.methods.len(), 2);
                // MCP methods have no body
                assert_eq!(m.methods[0].name, "search");
                assert!(m.methods[0].body.is_none());
                assert_eq!(m.methods[0].decorators.len(), 2);
                assert_eq!(m.methods[0].self_param, SelfParam::None);
                assert_eq!(m.methods[1].name, "get_file");
                assert!(m.methods[1].body.is_none());
            }
            other => panic!("expected Mcp, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== decorators =====

    #[test]
    fn parse_decorator_no_args() {
        let prog = parse(
            r#"
            @cache
            agent Cached {
                provider: openai,
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Agent(a) => {
                assert_eq!(a.decorators.len(), 1);
                assert_eq!(a.decorators[0].name, "cache");
                assert!(a.decorators[0].args.is_empty());
            }
            other => panic!("expected Agent, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_decorator_positional_args() {
        let prog = parse(
            r#"
            tool MyTool {
                description: "test",

                @describe("A method")
                @param("x", "The x value")
                pub fn method(self, x: Int) -> Int {
                    return x;
                }
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Tool(t) => {
                let desc = &t.methods[0].decorators[0];
                assert_eq!(desc.name, "describe");
                assert_eq!(desc.args.len(), 1);
                match &desc.args[0] {
                    DecoratorArg::Positional(expr) => {
                        matches!(&expr.kind, ExprKind::Literal(Literal::String(_)));
                    }
                    _ => panic!("expected positional arg"),
                }
                let param = &t.methods[0].decorators[1];
                assert_eq!(param.name, "param");
                assert_eq!(param.args.len(), 2);
            }
            other => panic!("expected Tool, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_decorator_named_args() {
        let prog = parse(
            r#"
            @timeout(seconds: 30)
            agent TimedAgent {
                provider: openai,
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Agent(a) => {
                let dec = &a.decorators[0];
                assert_eq!(dec.name, "timeout");
                assert_eq!(dec.args.len(), 1);
                match &dec.args[0] {
                    DecoratorArg::Named { name, .. } => assert_eq!(name, "seconds"),
                    _ => panic!("expected named arg"),
                }
            }
            other => panic!("expected Agent, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== mixed declarations =====

    #[test]
    fn parse_multiple_declarations() {
        let prog = parse(
            r#"
            use std::json;
            const MAX: Int = 100;
            fn main() {}
        "#,
        );
        assert_eq!(prog.declarations.len(), 3);
        assert!(matches!(&prog.declarations[0], Declaration::Use(_)));
        assert!(matches!(&prog.declarations[1], Declaration::Const(_)));
        assert!(matches!(&prog.declarations[2], Declaration::Function(_)));
    }

    // ===== agent with tools array (config field with array expression) =====

    #[test]
    fn parse_agent_with_tools_array() {
        let prog = parse(
            r#"
            agent FileAnalyzer {
                provider: openai,
                model: "gpt-4o",
                tools: [FileManager, Calculator],
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Agent(a) => {
                assert_eq!(a.name, "FileAnalyzer");
                assert_eq!(a.fields.len(), 3);
                assert_eq!(a.fields[2].name, "tools");
                // The value should be an array expression
                matches!(&a.fields[2].value.kind, ExprKind::Array(_));
            }
            other => panic!("expected Agent, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== async function =====

    #[test]
    fn parse_async_function() {
        let prog = parse("async fn fetch_data() -> String { return \"data\"; }");
        match &prog.declarations[0] {
            Declaration::Function(f) => {
                assert!(f.is_async);
                assert_eq!(f.name, "fetch_data");
            }
            other => panic!("expected Function, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_pub_async_function() {
        let prog = parse("pub async fn handler() {}");
        match &prog.declarations[0] {
            Declaration::Function(f) => {
                assert!(f.is_public);
                assert!(f.is_async);
                assert_eq!(f.name, "handler");
            }
            other => panic!("expected Function, got {:?}", std::mem::discriminant(other)),
        }
    }

    // ===== error recovery =====

    #[test]
    fn parse_unknown_decl_recovers() {
        let (prog, diags) = parse_with_errors("blah blah\nfn main() {}");
        assert!(diags.has_errors());
        // Should recover and parse the function
        assert!(prog
            .declarations
            .iter()
            .any(|d| matches!(d, Declaration::Function(_))));
    }

    // ===== Step 12: Union types =====

    #[test]
    fn parse_schema_with_union_type() {
        let prog = parse(
            r#"
            schema Classification {
                category: "legal" | "technical" | "financial" | "general",
                confidence: Float,
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Schema(s) => {
                assert_eq!(s.name, "Classification");
                assert_eq!(s.fields.len(), 2);
                // First field should have a union type
                match &s.fields[0].type_ann.kind {
                    crate::ast::types::TypeKind::Union(variants) => {
                        assert_eq!(variants.len(), 4);
                        match &variants[0].kind {
                            crate::ast::types::TypeKind::StringLiteral(s) => {
                                assert_eq!(s, "legal");
                            }
                            _ => panic!("expected string literal"),
                        }
                    }
                    _ => panic!("expected union type"),
                }
                // Second field should be a named type
                match &s.fields[1].type_ann.kind {
                    crate::ast::types::TypeKind::Named(name) => assert_eq!(name, "Float"),
                    _ => panic!("expected named type"),
                }
            }
            other => panic!("expected Schema, got {:?}", std::mem::discriminant(other)),
        }
    }

    #[test]
    fn parse_string_literal_type() {
        let prog = parse(
            r#"
            schema Mode {
                value: "on" | "off",
            }
        "#,
        );
        match &prog.declarations[0] {
            Declaration::Schema(s) => match &s.fields[0].type_ann.kind {
                crate::ast::types::TypeKind::Union(variants) => {
                    assert_eq!(variants.len(), 2);
                }
                _ => panic!("expected union type"),
            },
            other => panic!("expected Schema, got {:?}", std::mem::discriminant(other)),
        }
    }
}
