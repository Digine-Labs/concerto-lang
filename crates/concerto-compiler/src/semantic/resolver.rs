use concerto_common::{DiagnosticBag, Span};

use crate::ast::nodes::*;

use super::scope::{ScopeKind, ScopeStack, Symbol, SymbolKind};
use super::type_checker;
use super::types::Type;

/// Two-pass name resolver, type checker, and structural validator.
///
/// Pass 1 collects top-level declarations into the global scope so that
/// forward references work.  Pass 2 walks all bodies, resolving names,
/// checking types, and validating control flow.
pub struct Resolver {
    scopes: ScopeStack,
    diagnostics: DiagnosticBag,
    /// The return type of the enclosing function (if any).
    current_function_return: Option<Type>,
    /// Whether we are inside an `async fn`.
    in_async: bool,
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver {
    pub fn new() -> Self {
        let mut r = Self {
            scopes: ScopeStack::new(),
            diagnostics: DiagnosticBag::new(),
            current_function_return: None,
            in_async: false,
        };
        r.register_builtins();
        r
    }

    /// Run both passes and return accumulated diagnostics.
    pub fn resolve(mut self, program: &Program) -> DiagnosticBag {
        // Pass 1: populate global scope with top-level names.
        self.collect_declarations(program);
        // Pass 2: resolve bodies.
        self.resolve_program(program);
        self.diagnostics
    }

    // ====================================================================
    // Built-in symbols
    // ====================================================================

    fn register_builtins(&mut self) {
        let builtins: Vec<(&str, SymbolKind, Type)> = vec![
            (
                "emit",
                SymbolKind::Function,
                Type::Function {
                    params: vec![Type::String, Type::Any],
                    return_type: Box::new(Type::Nil),
                },
            ),
            (
                "print",
                SymbolKind::Function,
                Type::Function {
                    params: vec![Type::Any],
                    return_type: Box::new(Type::Nil),
                },
            ),
            (
                "println",
                SymbolKind::Function,
                Type::Function {
                    params: vec![Type::Any],
                    return_type: Box::new(Type::Nil),
                },
            ),
            // Option / Result constructors
            (
                "Some",
                SymbolKind::Function,
                Type::Function {
                    params: vec![Type::Any],
                    return_type: Box::new(Type::Option(Box::new(Type::Any))),
                },
            ),
            ("None", SymbolKind::Variable, Type::Option(Box::new(Type::Any))),
            (
                "Ok",
                SymbolKind::Function,
                Type::Function {
                    params: vec![Type::Any],
                    return_type: Box::new(Type::Result(
                        Box::new(Type::Any),
                        Box::new(Type::Any),
                    )),
                },
            ),
            (
                "Err",
                SymbolKind::Function,
                Type::Function {
                    params: vec![Type::Any],
                    return_type: Box::new(Type::Result(
                        Box::new(Type::Any),
                        Box::new(Type::Any),
                    )),
                },
            ),
            // Runtime built-in functions
            (
                "env",
                SymbolKind::Function,
                Type::Function {
                    params: vec![Type::String],
                    return_type: Box::new(Type::String),
                },
            ),
            // Common type constructors (used as namespaces via path expressions)
            ("ToolError", SymbolKind::Struct, Type::Named("ToolError".to_string())),
            ("Database", SymbolKind::Struct, Type::Named("Database".to_string())),
            ("Ledger", SymbolKind::Struct, Type::Named("Ledger".to_string())),
            ("std", SymbolKind::Module, Type::Any),
        ];

        for (name, kind, ty) in builtins {
            let _ = self.scopes.define(Symbol {
                name: name.to_string(),
                kind,
                ty,
                mutable: false,
                defined_at: Span::dummy(),
                used: true, // never warn about built-ins
                is_public: true,
            });
        }
    }

    // ====================================================================
    // Pass 1: collect top-level declarations
    // ====================================================================

    fn collect_declarations(&mut self, program: &Program) {
        for decl in &program.declarations {
            match decl {
                Declaration::Function(f) => self.declare_function_symbol(f),
                Declaration::Connect(c) => {
                    self.define_symbol(
                        &c.name,
                        SymbolKind::Connection,
                        Type::Named(c.name.clone()),
                        false,
                        false,
                        c.span.clone(),
                    );
                }
                Declaration::Agent(a) => {
                    self.define_symbol(
                        &a.name,
                        SymbolKind::Agent,
                        Type::AgentRef,
                        false,
                        false,
                        a.span.clone(),
                    );
                }
                Declaration::Tool(t) => {
                    self.define_symbol(
                        &t.name,
                        SymbolKind::Tool,
                        Type::Named(t.name.clone()),
                        false,
                        false,
                        t.span.clone(),
                    );
                }
                Declaration::Schema(s) => {
                    self.define_symbol(
                        &s.name,
                        SymbolKind::Schema,
                        Type::Named(s.name.clone()),
                        false,
                        false,
                        s.span.clone(),
                    );
                }
                Declaration::Pipeline(p) => {
                    self.define_symbol(
                        &p.name,
                        SymbolKind::Pipeline,
                        Type::Named(p.name.clone()),
                        false,
                        false,
                        p.span.clone(),
                    );
                }
                Declaration::Struct(s) => {
                    self.define_symbol(
                        &s.name,
                        SymbolKind::Struct,
                        Type::Named(s.name.clone()),
                        false,
                        false,
                        s.span.clone(),
                    );
                }
                Declaration::Enum(e) => {
                    self.define_symbol(
                        &e.name,
                        SymbolKind::Enum,
                        Type::Named(e.name.clone()),
                        false,
                        false,
                        e.span.clone(),
                    );
                }
                Declaration::Trait(t) => {
                    self.define_symbol(
                        &t.name,
                        SymbolKind::Trait,
                        Type::Named(t.name.clone()),
                        false,
                        false,
                        t.span.clone(),
                    );
                }
                Declaration::Const(c) => {
                    let ty = Type::from_annotation(&c.type_ann);
                    self.define_symbol(
                        &c.name,
                        SymbolKind::Const,
                        ty,
                        false,
                        false,
                        c.span.clone(),
                    );
                }
                Declaration::TypeAlias(t) => {
                    self.define_symbol(
                        &t.name,
                        SymbolKind::TypeAlias,
                        Type::from_annotation(&t.type_ann),
                        false,
                        false,
                        t.span.clone(),
                    );
                }
                Declaration::Db(d) => {
                    self.define_symbol(
                        &d.name,
                        SymbolKind::Database,
                        Type::from_annotation(&d.type_ann),
                        false,
                        false,
                        d.span.clone(),
                    );
                }
                Declaration::Ledger(l) => {
                    self.define_symbol(
                        &l.name,
                        SymbolKind::Ledger,
                        Type::LedgerRef,
                        false,
                        false,
                        l.span.clone(),
                    );
                }
                Declaration::Mcp(m) => {
                    self.define_symbol(
                        &m.name,
                        SymbolKind::Mcp,
                        Type::Named(m.name.clone()),
                        false,
                        false,
                        m.span.clone(),
                    );
                }
                Declaration::Module(m) => {
                    self.define_symbol(
                        &m.name,
                        SymbolKind::Module,
                        Type::Named(m.name.clone()),
                        false,
                        false,
                        m.span.clone(),
                    );
                }
                Declaration::Impl(_) | Declaration::Use(_) => {}
            }
        }
    }

    fn declare_function_symbol(&mut self, f: &FunctionDecl) {
        let return_type = f
            .return_type
            .as_ref()
            .map(Type::from_annotation)
            .unwrap_or(Type::Nil);
        let params: Vec<Type> = f
            .params
            .iter()
            .map(|p| {
                p.type_ann
                    .as_ref()
                    .map(Type::from_annotation)
                    .unwrap_or(Type::Unknown)
            })
            .collect();
        let ty = Type::Function {
            params,
            return_type: Box::new(return_type),
        };
        self.define_symbol(
            &f.name,
            SymbolKind::Function,
            ty,
            false,
            f.is_public,
            f.span.clone(),
        );
    }

    // ====================================================================
    // Pass 2: resolve bodies
    // ====================================================================

    fn resolve_program(&mut self, program: &Program) {
        for decl in &program.declarations {
            self.resolve_declaration(decl);
        }
    }

    fn resolve_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Function(f) => self.resolve_function(f),
            Declaration::Connect(c) => self.resolve_config_fields(&c.fields),
            Declaration::Agent(a) => self.resolve_config_fields(&a.fields),
            Declaration::Tool(t) => {
                self.resolve_config_fields(&t.fields);
                for method in &t.methods {
                    // Tool methods are implicitly async (they may use await emit)
                    self.resolve_tool_method(method);
                }
            }
            Declaration::Schema(s) => {
                for field in &s.fields {
                    if let Some(ref default) = field.default {
                        self.resolve_expr(default);
                    }
                }
            }
            Declaration::Pipeline(p) => {
                for stage in &p.stages {
                    self.resolve_stage(stage);
                }
            }
            Declaration::Struct(s) => {
                for field in &s.fields {
                    if let Some(ref default) = field.default {
                        self.resolve_expr(default);
                    }
                }
            }
            Declaration::Enum(_) => {}
            Declaration::Trait(t) => {
                for method in &t.methods {
                    self.resolve_function(method);
                }
            }
            Declaration::Impl(i) => {
                // Verify the target type exists.
                if self.scopes.lookup(&i.target).is_none() {
                    self.diagnostics.error(
                        format!("undefined type `{}` in impl block", i.target),
                        i.methods
                            .first()
                            .map(|m| m.span.clone())
                            .unwrap_or_else(Span::dummy),
                    );
                } else if let Some(sym) = self.scopes.lookup_mut(&i.target) {
                    sym.used = true;
                }
                for method in &i.methods {
                    self.resolve_function(method);
                }
            }
            Declaration::Const(c) => self.resolve_expr(&c.value),
            Declaration::Db(d) => self.resolve_expr(&d.initializer),
            Declaration::Ledger(l) => self.resolve_expr(&l.initializer),
            Declaration::Mcp(m) => self.resolve_config_fields(&m.fields),
            Declaration::Use(_) | Declaration::Module(_) | Declaration::TypeAlias(_) => {}
        }
    }

    fn resolve_function(&mut self, func: &FunctionDecl) {
        let Some(ref body) = func.body else {
            return;
        };

        let return_type = func
            .return_type
            .as_ref()
            .map(Type::from_annotation)
            .unwrap_or(Type::Nil);

        let prev_return = self.current_function_return.take();
        let prev_async = self.in_async;
        self.current_function_return = Some(return_type);
        self.in_async = func.is_async;

        self.scopes.push(ScopeKind::Function);

        // Declare `self` if present.
        if func.self_param != SelfParam::None {
            self.define_symbol(
                "self",
                SymbolKind::Parameter,
                Type::Unknown,
                func.self_param == SelfParam::Mutable,
                false,
                func.span.clone(),
            );
        }

        // Declare parameters.
        for param in &func.params {
            let ty = param
                .type_ann
                .as_ref()
                .map(Type::from_annotation)
                .unwrap_or(Type::Unknown);
            self.define_symbol(
                &param.name,
                SymbolKind::Parameter,
                ty,
                false,
                false,
                param.span.clone(),
            );
        }

        self.resolve_block(body);

        let scope_idx = self.scopes.pop();
        self.emit_unused_warnings(scope_idx);

        self.current_function_return = prev_return;
        self.in_async = prev_async;
    }

    /// Resolve a tool method - implicitly async (may use `await emit`).
    /// Also marks `self` as used to suppress warnings.
    fn resolve_tool_method(&mut self, func: &FunctionDecl) {
        let Some(ref body) = func.body else {
            return;
        };

        let return_type = func
            .return_type
            .as_ref()
            .map(Type::from_annotation)
            .unwrap_or(Type::Nil);

        let prev_return = self.current_function_return.take();
        let prev_async = self.in_async;
        self.current_function_return = Some(return_type);
        // Tool methods are implicitly async
        self.in_async = true;

        self.scopes.push(ScopeKind::Function);

        // Declare `self` if present and mark as used
        if func.self_param != SelfParam::None {
            self.define_symbol(
                "self",
                SymbolKind::Parameter,
                Type::Unknown,
                func.self_param == SelfParam::Mutable,
                false,
                func.span.clone(),
            );
            // Mark self as used to avoid warnings
            if let Some(sym) = self.scopes.lookup_mut("self") {
                sym.used = true;
            }
        }

        // Declare parameters.
        for param in &func.params {
            let ty = param
                .type_ann
                .as_ref()
                .map(Type::from_annotation)
                .unwrap_or(Type::Unknown);
            self.define_symbol(
                &param.name,
                SymbolKind::Parameter,
                ty,
                false,
                false,
                param.span.clone(),
            );
        }

        self.resolve_block(body);

        let scope_idx = self.scopes.pop();
        self.emit_unused_warnings(scope_idx);

        self.current_function_return = prev_return;
        self.in_async = prev_async;
    }

    fn resolve_stage(&mut self, stage: &StageDecl) {
        // Stages implicitly return Result<T, Error> where T is the declared return type.
        // This allows ? operator usage inside stages.
        let return_type = stage
            .return_type
            .as_ref()
            .map(|ann| {
                Type::Result(
                    Box::new(Type::from_annotation(ann)),
                    Box::new(Type::Named("Error".to_string())),
                )
            })
            .unwrap_or(Type::Any);

        let prev_return = self.current_function_return.take();
        let prev_async = self.in_async;
        // Stages are implicitly async (they call agents)
        self.current_function_return = Some(return_type);
        self.in_async = true;

        self.scopes.push(ScopeKind::Function);
        for param in &stage.params {
            let ty = param
                .type_ann
                .as_ref()
                .map(Type::from_annotation)
                .unwrap_or(Type::Unknown);
            self.define_symbol(
                &param.name,
                SymbolKind::Parameter,
                ty,
                false,
                false,
                param.span.clone(),
            );
        }
        self.resolve_block(&stage.body);
        let scope_idx = self.scopes.pop();
        self.emit_unused_warnings(scope_idx);

        self.current_function_return = prev_return;
        self.in_async = prev_async;
    }

    fn resolve_config_fields(&mut self, fields: &[ConfigField]) {
        for field in fields {
            self.resolve_expr(&field.value);
        }
    }

    // ====================================================================
    // Blocks and statements
    // ====================================================================

    fn resolve_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.resolve_stmt(stmt);
        }
        if let Some(ref tail) = block.tail_expr {
            self.resolve_expr(tail);
        }
    }

    fn resolve_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(s) => self.resolve_let(s),
            Stmt::Expr(s) => self.resolve_expr(&s.expr),
            Stmt::Return(s) => self.resolve_return(s),
            Stmt::Break(s) => self.resolve_break(s),
            Stmt::Continue(s) => self.resolve_continue(s),
            Stmt::Throw(s) => self.resolve_throw(s),
        }
    }

    fn resolve_let(&mut self, stmt: &LetStmt) {
        // Resolve the initializer *before* adding the binding to the scope.
        let ty = if let Some(ref init) = stmt.initializer {
            self.resolve_expr(init);
            if let Some(ref ann) = stmt.type_ann {
                Type::from_annotation(ann)
            } else {
                self.infer_expr_type(init)
            }
        } else if let Some(ref ann) = stmt.type_ann {
            Type::from_annotation(ann)
        } else {
            Type::Unknown
        };

        self.define_symbol(
            &stmt.name,
            SymbolKind::Variable,
            ty,
            stmt.mutable,
            false,
            stmt.span.clone(),
        );
    }

    fn resolve_return(&mut self, stmt: &ReturnStmt) {
        if !self.scopes.in_function() {
            self.diagnostics
                .error("`return` outside of function", stmt.span.clone());
        }
        if let Some(ref val) = stmt.value {
            self.resolve_expr(val);
        }
    }

    fn resolve_break(&mut self, stmt: &BreakStmt) {
        if !self.scopes.in_loop() {
            self.diagnostics
                .error("`break` outside of loop", stmt.span.clone());
        }
        if let Some(ref val) = stmt.value {
            self.resolve_expr(val);
        }
    }

    fn resolve_continue(&mut self, stmt: &ContinueStmt) {
        if !self.scopes.in_loop() {
            self.diagnostics
                .error("`continue` outside of loop", stmt.span.clone());
        }
    }

    fn resolve_throw(&mut self, stmt: &ThrowStmt) {
        if !self.scopes.in_function() {
            self.diagnostics
                .error("`throw` outside of function", stmt.span.clone());
        } else if let Some(ref ret_ty) = self.current_function_return {
            if !ret_ty.is_result() && !matches!(ret_ty, Type::Unknown | Type::Any | Type::Error) {
                self.diagnostics.error(
                    "`throw` can only be used in functions returning Result<T, E>",
                    stmt.span.clone(),
                );
            }
        }
        self.resolve_expr(&stmt.value);
    }

    // ====================================================================
    // Expressions
    // ====================================================================

    fn resolve_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Literal(_) => {}

            ExprKind::Identifier(name) => {
                if let Some(sym) = self.scopes.lookup_mut(name) {
                    sym.used = true;
                } else {
                    self.diagnostics.error(
                        format!("undefined variable `{}`", name),
                        expr.span.clone(),
                    );
                }
            }

            ExprKind::Binary { left, op, right } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
                let left_ty = self.infer_expr_type(left);
                let right_ty = self.infer_expr_type(right);
                if let Err(msg) = type_checker::check_binary_op(&left_ty, *op, &right_ty) {
                    self.diagnostics.error(msg, expr.span.clone());
                }
            }

            ExprKind::Unary { op, operand } => {
                self.resolve_expr(operand);
                let ty = self.infer_expr_type(operand);
                if let Err(msg) = type_checker::check_unary_op(*op, &ty) {
                    self.diagnostics.error(msg, expr.span.clone());
                }
            }

            ExprKind::Call { callee, args } => {
                self.resolve_expr(callee);
                for arg in args {
                    self.resolve_expr(arg);
                }
            }

            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.resolve_expr(condition);
                let cond_ty = self.infer_expr_type(condition);
                if !matches!(cond_ty, Type::Bool | Type::Unknown | Type::Any | Type::Error) {
                    self.diagnostics.error(
                        format!(
                            "condition must be Bool, got {}",
                            cond_ty.display_name()
                        ),
                        condition.span.clone(),
                    );
                }
                self.scopes.push(ScopeKind::Block);
                self.resolve_block(then_branch);
                let idx = self.scopes.pop();
                self.emit_unused_warnings(idx);

                if let Some(ref eb) = else_branch {
                    match eb {
                        ElseBranch::Block(b) => {
                            self.scopes.push(ScopeKind::Block);
                            self.resolve_block(b);
                            let idx = self.scopes.pop();
                            self.emit_unused_warnings(idx);
                        }
                        ElseBranch::ElseIf(e) => self.resolve_expr(e),
                    }
                }
            }

            ExprKind::Block(block) => {
                self.scopes.push(ScopeKind::Block);
                self.resolve_block(block);
                let idx = self.scopes.pop();
                self.emit_unused_warnings(idx);
            }

            ExprKind::Assign { target, value, .. } => {
                self.resolve_expr(target);
                self.resolve_expr(value);
                self.check_assign_target(target);
            }

            ExprKind::FieldAccess { object, .. } => {
                self.resolve_expr(object);
            }

            ExprKind::MethodCall { object, args, .. } => {
                self.resolve_expr(object);
                for arg in args {
                    self.resolve_expr(arg);
                }
            }

            ExprKind::Index { object, index } => {
                self.resolve_expr(object);
                self.resolve_expr(index);
            }

            ExprKind::Array(elems) => {
                for elem in elems {
                    self.resolve_expr(elem);
                }
            }

            ExprKind::Map(entries) => {
                for (key, val) in entries {
                    self.resolve_expr(key);
                    self.resolve_expr(val);
                }
            }

            ExprKind::Grouping(inner) => self.resolve_expr(inner),

            ExprKind::Match { scrutinee, arms } => {
                self.resolve_expr(scrutinee);
                for arm in arms {
                    self.scopes.push(ScopeKind::Block);
                    self.resolve_pattern(&arm.pattern);
                    if let Some(ref guard) = arm.guard {
                        self.resolve_expr(guard);
                    }
                    self.resolve_expr(&arm.body);
                    let idx = self.scopes.pop();
                    self.emit_unused_warnings(idx);
                }
            }

            ExprKind::TryCatch { body, catches } => {
                self.scopes.push(ScopeKind::Block);
                self.resolve_block(body);
                let idx = self.scopes.pop();
                self.emit_unused_warnings(idx);

                for catch in catches {
                    self.scopes.push(ScopeKind::Block);
                    if let Some(ref binding) = catch.binding {
                        self.define_symbol(
                            binding,
                            SymbolKind::Variable,
                            Type::Unknown,
                            false,
                            false,
                            catch.span.clone(),
                        );
                    }
                    self.resolve_block(&catch.body);
                    let idx = self.scopes.pop();
                    self.emit_unused_warnings(idx);
                }
            }

            ExprKind::For {
                pattern,
                iterable,
                body,
            } => {
                self.resolve_expr(iterable);
                self.scopes.push(ScopeKind::Loop);
                self.resolve_pattern(pattern);
                self.resolve_block(body);
                let idx = self.scopes.pop();
                self.emit_unused_warnings(idx);
            }

            ExprKind::While { condition, body } => {
                self.resolve_expr(condition);
                let cond_ty = self.infer_expr_type(condition);
                if !matches!(cond_ty, Type::Bool | Type::Unknown | Type::Any | Type::Error) {
                    self.diagnostics.error(
                        format!(
                            "while condition must be Bool, got {}",
                            cond_ty.display_name()
                        ),
                        condition.span.clone(),
                    );
                }
                self.scopes.push(ScopeKind::Loop);
                self.resolve_block(body);
                let idx = self.scopes.pop();
                self.emit_unused_warnings(idx);
            }

            ExprKind::Loop { body } => {
                self.scopes.push(ScopeKind::Loop);
                self.resolve_block(body);
                let idx = self.scopes.pop();
                self.emit_unused_warnings(idx);
            }

            ExprKind::Closure { params, body, .. } => {
                self.scopes.push(ScopeKind::Function);
                for param in params {
                    let ty = param
                        .type_ann
                        .as_ref()
                        .map(Type::from_annotation)
                        .unwrap_or(Type::Unknown);
                    self.define_symbol(
                        &param.name,
                        SymbolKind::Parameter,
                        ty,
                        false,
                        false,
                        param.span.clone(),
                    );
                }
                self.resolve_expr(body);
                let idx = self.scopes.pop();
                self.emit_unused_warnings(idx);
            }

            ExprKind::Pipe { left, right } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }

            ExprKind::Propagate(inner) => {
                self.resolve_expr(inner);
                if let Some(ref ret_ty) = self.current_function_return {
                    if !ret_ty.is_result()
                        && !ret_ty.is_option()
                        && !matches!(ret_ty, Type::Unknown | Type::Any | Type::Error)
                    {
                        self.diagnostics.error(
                            "`?` operator can only be used in functions returning Result or Option",
                            expr.span.clone(),
                        );
                    }
                } else {
                    self.diagnostics.error(
                        "`?` operator can only be used inside a function",
                        expr.span.clone(),
                    );
                }
            }

            ExprKind::NilCoalesce { left, right } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }

            ExprKind::Range { start, end, .. } => {
                if let Some(ref s) = start {
                    self.resolve_expr(s);
                }
                if let Some(ref e) = end {
                    self.resolve_expr(e);
                }
            }

            ExprKind::Cast { expr: inner, .. } => {
                self.resolve_expr(inner);
            }

            ExprKind::Path(segments) => {
                if let Some(first) = segments.first() {
                    if let Some(sym) = self.scopes.lookup_mut(first) {
                        sym.used = true;
                    } else {
                        self.diagnostics.error(
                            format!("undefined name `{}`", first),
                            expr.span.clone(),
                        );
                    }
                }
            }

            ExprKind::Await(inner) => {
                self.resolve_expr(inner);
                if !self.in_async {
                    self.diagnostics.error(
                        "`.await` can only be used in async functions",
                        expr.span.clone(),
                    );
                }
            }

            ExprKind::Tuple(elems) => {
                for elem in elems {
                    self.resolve_expr(elem);
                }
            }

            ExprKind::StructLiteral { name, fields } => {
                if let Some(first) = name.first() {
                    if let Some(sym) = self.scopes.lookup_mut(first) {
                        sym.used = true;
                    } else {
                        self.diagnostics.error(
                            format!("undefined type `{}`", first),
                            expr.span.clone(),
                        );
                    }
                }
                for f in fields {
                    self.resolve_expr(&f.value);
                }
            }

            ExprKind::StringInterpolation(parts) => {
                for part in parts {
                    if let StringPart::Expr(ref e) = part {
                        self.resolve_expr(e);
                    }
                }
            }

            ExprKind::Return(value) => {
                if let Some(val) = value {
                    self.resolve_expr(val);
                }
                if !self.scopes.in_function() {
                    self.diagnostics.error(
                        "`return` outside of function",
                        expr.span.clone(),
                    );
                }
            }
        }
    }

    // ====================================================================
    // Patterns  (introduce bindings into current scope)
    // ====================================================================

    fn resolve_pattern(&mut self, pattern: &Pattern) {
        match &pattern.kind {
            PatternKind::Wildcard | PatternKind::Rest => {}
            PatternKind::Literal(_) => {}
            PatternKind::Identifier(name) => {
                if name != "_" {
                    self.define_symbol(
                        name,
                        SymbolKind::Variable,
                        Type::Unknown,
                        false,
                        false,
                        pattern.span.clone(),
                    );
                }
            }
            PatternKind::Tuple(pats) => {
                for p in pats {
                    self.resolve_pattern(p);
                }
            }
            PatternKind::Struct { path, fields, .. } => {
                if let Some(first) = path.first() {
                    if let Some(sym) = self.scopes.lookup_mut(first) {
                        sym.used = true;
                    } else {
                        self.diagnostics.error(
                            format!("undefined type `{}`", first),
                            pattern.span.clone(),
                        );
                    }
                }
                for f in fields {
                    if let Some(ref p) = f.pattern {
                        self.resolve_pattern(p);
                    } else {
                        // Shorthand `Point { x }` — x is both field name and binding.
                        self.define_symbol(
                            &f.name,
                            SymbolKind::Variable,
                            Type::Unknown,
                            false,
                            false,
                            f.span.clone(),
                        );
                    }
                }
            }
            PatternKind::Enum { path, fields } => {
                if let Some(first) = path.first() {
                    if let Some(sym) = self.scopes.lookup_mut(first) {
                        sym.used = true;
                    }
                    // Don't error for unknown first segments — they might be
                    // built-in enum constructors (Some, Ok, Err, ...).
                }
                for p in fields {
                    self.resolve_pattern(p);
                }
            }
            PatternKind::Array { elements, .. } => {
                for p in elements {
                    self.resolve_pattern(p);
                }
            }
            PatternKind::Or(pats) => {
                for p in pats {
                    self.resolve_pattern(p);
                }
            }
            PatternKind::Binding { name, pattern } => {
                self.define_symbol(
                    name,
                    SymbolKind::Variable,
                    Type::Unknown,
                    false,
                    false,
                    pattern.span.clone(),
                );
                self.resolve_pattern(pattern);
            }
            PatternKind::Range { start, end, .. } => {
                self.resolve_expr(start);
                self.resolve_expr(end);
            }
        }
    }

    // ====================================================================
    // Basic type inference  (best-effort; returns Unknown when unsure)
    // ====================================================================

    fn infer_expr_type(&self, expr: &Expr) -> Type {
        match &expr.kind {
            ExprKind::Literal(lit) => match lit {
                Literal::Int(_) => Type::Int,
                Literal::Float(_) => Type::Float,
                Literal::String(_) => Type::String,
                Literal::Bool(_) => Type::Bool,
                Literal::Nil => Type::Nil,
            },
            ExprKind::Identifier(name) => self
                .scopes
                .lookup(name)
                .map(|s| s.ty.clone())
                .unwrap_or(Type::Error),
            ExprKind::Binary { left, op, right } => {
                let l = self.infer_expr_type(left);
                let r = self.infer_expr_type(right);
                type_checker::check_binary_op(&l, *op, &r).unwrap_or(Type::Error)
            }
            ExprKind::Unary { op, operand } => {
                let ty = self.infer_expr_type(operand);
                type_checker::check_unary_op(*op, &ty).unwrap_or(Type::Error)
            }
            ExprKind::Call { callee, .. } => {
                let callee_ty = self.infer_expr_type(callee);
                match callee_ty {
                    Type::Function { return_type, .. } => *return_type,
                    _ => Type::Unknown,
                }
            }
            ExprKind::If { then_branch, .. } => then_branch
                .tail_expr
                .as_ref()
                .map(|e| self.infer_expr_type(e))
                .unwrap_or(Type::Nil),
            ExprKind::Block(block) => block
                .tail_expr
                .as_ref()
                .map(|e| self.infer_expr_type(e))
                .unwrap_or(Type::Nil),
            ExprKind::Array(elems) => {
                let inner = elems
                    .first()
                    .map(|e| self.infer_expr_type(e))
                    .unwrap_or(Type::Unknown);
                Type::Array(Box::new(inner))
            }
            ExprKind::StringInterpolation(_) => Type::String,
            ExprKind::Grouping(inner) => self.infer_expr_type(inner),
            ExprKind::Propagate(inner) => {
                let inner_ty = self.infer_expr_type(inner);
                match inner_ty {
                    Type::Result(ok, _) => *ok,
                    Type::Option(inner) => *inner,
                    _ => Type::Unknown,
                }
            }
            ExprKind::NilCoalesce { left, right } => {
                let left_ty = self.infer_expr_type(left);
                match left_ty {
                    Type::Option(inner) => *inner,
                    _ => self.infer_expr_type(right),
                }
            }
            ExprKind::Range { .. } => Type::Named("Range".into()),
            ExprKind::Cast { target, .. } => Type::from_annotation(target),
            ExprKind::Tuple(elems) => {
                Type::Tuple(elems.iter().map(|e| self.infer_expr_type(e)).collect())
            }
            ExprKind::Map(_) => Type::Map(Box::new(Type::String), Box::new(Type::Unknown)),
            _ => Type::Unknown,
        }
    }

    // ====================================================================
    // Mutability checking
    // ====================================================================

    fn check_assign_target(&mut self, target: &Expr) {
        match &target.kind {
            ExprKind::Identifier(name) => {
                if let Some(sym) = self.scopes.lookup(name) {
                    if !sym.mutable && sym.kind == SymbolKind::Variable {
                        self.diagnostics.error(
                            format!("cannot assign to immutable variable `{}`", name),
                            target.span.clone(),
                        );
                    } else if sym.kind == SymbolKind::Const {
                        self.diagnostics.error(
                            format!("cannot assign to constant `{}`", name),
                            target.span.clone(),
                        );
                    }
                }
            }
            // Field access / index: allowed (the object itself controls mutability,
            // which would need full type info to check).
            ExprKind::FieldAccess { .. } | ExprKind::Index { .. } => {}
            _ => {
                self.diagnostics
                    .error("invalid assignment target", target.span.clone());
            }
        }
    }

    // ====================================================================
    // Helpers
    // ====================================================================

    fn define_symbol(
        &mut self,
        name: &str,
        kind: SymbolKind,
        ty: Type,
        mutable: bool,
        is_public: bool,
        span: Span,
    ) {
        let symbol = Symbol {
            name: name.to_string(),
            kind,
            ty,
            mutable,
            defined_at: span.clone(),
            used: false,
            is_public,
        };
        if let Err(prev_span) = self.scopes.define(symbol) {
            self.diagnostics.report(
                concerto_common::Diagnostic::error(format!(
                    "duplicate definition of `{}`",
                    name
                ))
                .with_span(span)
                .with_related(prev_span, "previously defined here"),
            );
        }
    }

    /// Emit unused-variable warnings for the scope at `scope_idx`.
    fn emit_unused_warnings(&mut self, scope_idx: usize) {
        // Collect first to avoid overlapping borrows on self.
        let unused: Vec<(String, Span)> = self
            .scopes
            .get_scope(scope_idx)
            .symbols
            .values()
            .filter(|sym| {
                !sym.used
                    && !sym.name.starts_with('_')
                    && matches!(sym.kind, SymbolKind::Variable | SymbolKind::Parameter)
            })
            .map(|sym| (sym.name.clone(), sym.defined_at.clone()))
            .collect();

        for (name, span) in unused {
            self.diagnostics
                .warning(format!("unused variable `{}`", name), span);
        }
    }
}

// ========================================================================
// Tests
// ========================================================================

#[cfg(test)]
mod tests {
    use crate::lexer::Lexer;
    use crate::parser;
    use concerto_common::Severity;

    /// Helper: parse source, run resolver, return diagnostics.
    fn analyze(source: &str) -> Vec<(Severity, String)> {
        let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
        assert!(!lex_diags.has_errors(), "lexer errors: {:?}", lex_diags);
        let (program, parse_diags) = parser::Parser::new(tokens).parse();
        assert!(
            !parse_diags.has_errors(),
            "parser errors: {:?}",
            parse_diags
        );
        let diags = super::Resolver::new().resolve(&program);
        diags
            .into_diagnostics()
            .into_iter()
            .map(|d| (d.severity, d.message))
            .collect()
    }

    fn errors(source: &str) -> Vec<String> {
        analyze(source)
            .into_iter()
            .filter(|(s, _)| *s == Severity::Error)
            .map(|(_, m)| m)
            .collect()
    }

    fn warnings(source: &str) -> Vec<String> {
        analyze(source)
            .into_iter()
            .filter(|(s, _)| *s == Severity::Warning)
            .map(|(_, m)| m)
            .collect()
    }

    // -- Name resolution --

    #[test]
    fn undefined_variable() {
        let errs = errors("fn main() { let x = y; }");
        assert!(errs.iter().any(|e| e.contains("undefined variable `y`")));
    }

    #[test]
    fn defined_variable_ok() {
        let errs = errors("fn main() { let x = 5; let y = x; }");
        assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    }

    #[test]
    fn forward_reference_functions() {
        let errs = errors(
            r#"
            fn main() { helper(); }
            fn helper() { }
            "#,
        );
        assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    }

    #[test]
    fn duplicate_top_level() {
        let errs = errors(
            r#"
            fn foo() { }
            fn foo() { }
            "#,
        );
        assert!(errs.iter().any(|e| e.contains("duplicate definition")));
    }

    // -- Control flow --

    #[test]
    fn break_outside_loop() {
        let errs = errors("fn main() { break; }");
        assert!(errs.iter().any(|e| e.contains("`break` outside of loop")));
    }

    #[test]
    fn break_inside_loop_ok() {
        let errs = errors("fn main() { loop { break; } }");
        assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    }

    #[test]
    fn continue_outside_loop() {
        let errs = errors("fn main() { continue; }");
        assert!(
            errs.iter()
                .any(|e| e.contains("`continue` outside of loop"))
        );
    }

    #[test]
    fn return_outside_function() {
        // At top level, return is invalid. But we can't write bare `return`
        // at top level in our grammar; it must be in a function. This test
        // just verifies the happy path works.
        let errs = errors("fn main() { return; }");
        assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    }

    // -- Type checking --

    #[test]
    fn if_condition_not_bool() {
        let errs = errors("fn main() { if 42 { } }");
        assert!(errs.iter().any(|e| e.contains("condition must be Bool")));
    }

    #[test]
    fn while_condition_not_bool() {
        let errs = errors("fn main() { while 1 { } }");
        assert!(
            errs.iter()
                .any(|e| e.contains("while condition must be Bool"))
        );
    }

    #[test]
    fn binary_type_mismatch() {
        let errs = errors(r#"fn main() { let x = "hello" - 1; }"#);
        assert!(
            errs.iter()
                .any(|e| e.contains("operator '-' cannot be applied"))
        );
    }

    #[test]
    fn logical_requires_bool() {
        let errs = errors("fn main() { let x = 1 && 2; }");
        assert!(
            errs.iter()
                .any(|e| e.contains("operator '&&' requires Bool operands"))
        );
    }

    #[test]
    fn unary_not_on_int() {
        let errs = errors("fn main() { let x = !42; }");
        assert!(
            errs.iter()
                .any(|e| e.contains("operator '!' requires Bool operand"))
        );
    }

    // -- Mutability --

    #[test]
    fn assign_to_immutable() {
        let errs = errors(
            r#"
            fn main() {
                let x = 5;
                x = 10;
            }
            "#,
        );
        assert!(
            errs.iter()
                .any(|e| e.contains("cannot assign to immutable variable"))
        );
    }

    #[test]
    fn assign_to_mutable_ok() {
        let errs = errors(
            r#"
            fn main() {
                let mut x = 5;
                x = 10;
            }
            "#,
        );
        assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    }

    // -- Async / Await --

    #[test]
    fn await_outside_async() {
        let errs = errors("fn main() { let x = my_fn().await; }");
        assert!(
            errs.iter()
                .any(|e| e.contains("`.await` can only be used in async"))
        );
    }

    // -- Propagation --

    #[test]
    fn propagate_outside_function() {
        // We can't easily test this because ? must be inside fn body in our grammar.
        // Instead test ? in a non-Result function.
        let errs = errors("fn main() { let x = foo()?; }");
        assert!(
            errs.iter()
                .any(|e| e.contains("`?` operator can only be used in functions returning"))
        );
    }

    #[test]
    fn propagate_in_result_fn_ok() {
        let errs = errors(
            r#"
            fn main() -> Result<Int, String> { let x = foo()?; }
            "#,
        );
        // Should not error about ? usage (foo undefined is a separate error).
        assert!(
            !errs
                .iter()
                .any(|e| e.contains("`?` operator can only be used"))
        );
    }

    // -- Throw --

    #[test]
    fn throw_in_non_result_fn() {
        let errs = errors(
            r#"
            fn main() {
                throw "error";
            }
            "#,
        );
        assert!(
            errs.iter()
                .any(|e| e.contains("`throw` can only be used in functions returning Result"))
        );
    }

    #[test]
    fn throw_in_result_fn_ok() {
        let errs = errors(
            r#"
            fn main() -> Result<Int, String> {
                throw "error";
            }
            "#,
        );
        assert!(
            !errs
                .iter()
                .any(|e| e.contains("`throw` can only be used"))
        );
    }

    // -- Unused variables --

    #[test]
    fn unused_variable_warning() {
        let warns = warnings("fn main() { let x = 5; }");
        assert!(warns.iter().any(|w| w.contains("unused variable `x`")));
    }

    #[test]
    fn underscore_prefix_no_warning() {
        let warns = warnings("fn main() { let _x = 5; }");
        assert!(
            !warns.iter().any(|w| w.contains("unused variable")),
            "got warnings: {:?}",
            warns
        );
    }

    #[test]
    fn used_variable_no_warning() {
        let warns = warnings(
            r#"
            fn main() {
                let x = 5;
                let y = x;
                emit("out", y);
            }
            "#,
        );
        assert!(
            !warns.iter().any(|w| w.contains("unused variable")),
            "got warnings: {:?}",
            warns
        );
    }

    // -- Built-ins --

    #[test]
    fn emit_is_builtin() {
        let errs = errors(r#"fn main() { emit("channel", 42); }"#);
        assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    }

    #[test]
    fn some_none_ok_err_builtins() {
        let errs = errors(
            r#"
            fn main() {
                let a = Some(1);
                let b = None;
                let c = Ok(1);
                let d = Err("fail");
            }
            "#,
        );
        assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    }

    // -- Scoping --

    #[test]
    fn block_scoping() {
        let errs = errors(
            r#"
            fn main() {
                {
                    let inner = 5;
                }
                let y = inner;
            }
            "#,
        );
        assert!(
            errs.iter()
                .any(|e| e.contains("undefined variable `inner`"))
        );
    }

    #[test]
    fn for_loop_scoping() {
        // Loop variable should be available inside body.
        let errs = errors(
            r#"
            fn main() {
                for item in [1, 2, 3] {
                    let x = item;
                }
            }
            "#,
        );
        assert!(errs.is_empty(), "unexpected errors: {:?}", errs);
    }
}
