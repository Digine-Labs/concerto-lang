use super::nodes::*;

/// Visitor trait for walking the AST.
///
/// Default implementations walk children; override specific methods
/// to add behavior at particular node types.
pub trait Visitor {
    fn visit_program(&mut self, program: &Program) {
        for decl in &program.declarations {
            self.visit_declaration(decl);
        }
    }

    fn visit_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Function(f) => self.visit_function_decl(f),
            Declaration::Agent(a) => self.visit_agent_decl(a),
            Declaration::Tool(t) => self.visit_tool_decl(t),
            Declaration::Schema(s) => self.visit_schema_decl(s),
            Declaration::Pipeline(p) => self.visit_pipeline_decl(p),
            Declaration::Struct(s) => self.visit_struct_decl(s),
            Declaration::Enum(e) => self.visit_enum_decl(e),
            Declaration::Trait(t) => self.visit_trait_decl(t),
            Declaration::Impl(i) => self.visit_impl_decl(i),
            Declaration::Use(u) => self.visit_use_decl(u),
            Declaration::Module(m) => self.visit_module_decl(m),
            Declaration::Const(c) => self.visit_const_decl(c),
            Declaration::TypeAlias(t) => self.visit_type_alias_decl(t),
            Declaration::HashMap(d) => self.visit_hashmap_decl(d),
            Declaration::Ledger(l) => self.visit_ledger_decl(l),
            Declaration::Memory(m) => self.visit_memory_decl(m),
            Declaration::Mcp(m) => self.visit_mcp_decl(m),
            Declaration::Host(h) => self.visit_host_decl(h),
        }
    }

    fn visit_function_decl(&mut self, func: &FunctionDecl) {
        if let Some(ref body) = func.body {
            self.visit_block(body);
        }
    }

    fn visit_agent_decl(&mut self, decl: &AgentDecl) {
        for field in &decl.fields {
            self.visit_expr(&field.value);
        }
    }

    fn visit_tool_decl(&mut self, decl: &ToolDecl) {
        for field in &decl.fields {
            self.visit_expr(&field.value);
        }
        for method in &decl.methods {
            self.visit_function_decl(method);
        }
    }

    fn visit_schema_decl(&mut self, decl: &SchemaDecl) {
        for field in &decl.fields {
            if let Some(ref default) = field.default {
                self.visit_expr(default);
            }
        }
    }

    fn visit_pipeline_decl(&mut self, decl: &PipelineDecl) {
        for stage in &decl.stages {
            self.visit_block(&stage.body);
        }
    }

    fn visit_struct_decl(&mut self, decl: &StructDecl) {
        for field in &decl.fields {
            if let Some(ref default) = field.default {
                self.visit_expr(default);
            }
        }
    }

    fn visit_enum_decl(&mut self, _decl: &EnumDecl) {}

    fn visit_trait_decl(&mut self, decl: &TraitDecl) {
        for method in &decl.methods {
            self.visit_function_decl(method);
        }
    }

    fn visit_impl_decl(&mut self, decl: &ImplDecl) {
        for method in &decl.methods {
            self.visit_function_decl(method);
        }
    }

    fn visit_use_decl(&mut self, _decl: &UseDecl) {}

    fn visit_module_decl(&mut self, decl: &ModuleDecl) {
        if let Some(ref body) = decl.body {
            for inner_decl in body {
                self.visit_declaration(inner_decl);
            }
        }
    }

    fn visit_const_decl(&mut self, decl: &ConstDecl) {
        self.visit_expr(&decl.value);
    }

    fn visit_type_alias_decl(&mut self, _decl: &TypeAliasDecl) {}

    fn visit_hashmap_decl(&mut self, decl: &HashMapDecl) {
        self.visit_expr(&decl.initializer);
    }

    fn visit_ledger_decl(&mut self, decl: &LedgerDecl) {
        self.visit_expr(&decl.initializer);
    }

    fn visit_memory_decl(&mut self, decl: &MemoryDecl) {
        self.visit_expr(&decl.initializer);
    }

    fn visit_mcp_decl(&mut self, decl: &McpDecl) {
        for field in &decl.fields {
            self.visit_expr(&field.value);
        }
    }

    fn visit_host_decl(&mut self, decl: &HostDecl) {
        for decorator in &decl.decorators {
            for arg in &decorator.args {
                match arg {
                    DecoratorArg::Positional(expr) => self.visit_expr(expr),
                    DecoratorArg::Named { value, .. } => self.visit_expr(value),
                }
            }
        }
        for field in &decl.fields {
            self.visit_expr(&field.value);
        }
    }

    fn visit_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
        if let Some(ref tail) = block.tail_expr {
            self.visit_expr(tail);
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(s) => self.visit_let_stmt(s),
            Stmt::Expr(s) => self.visit_expr_stmt(s),
            Stmt::Return(s) => self.visit_return_stmt(s),
            Stmt::Break(s) => self.visit_break_stmt(s),
            Stmt::Continue(s) => self.visit_continue_stmt(s),
            Stmt::Throw(s) => self.visit_throw_stmt(s),
        }
    }

    fn visit_let_stmt(&mut self, stmt: &LetStmt) {
        if let Some(ref init) = stmt.initializer {
            self.visit_expr(init);
        }
    }

    fn visit_expr_stmt(&mut self, stmt: &ExprStmt) {
        self.visit_expr(&stmt.expr);
    }

    fn visit_return_stmt(&mut self, stmt: &ReturnStmt) {
        if let Some(ref val) = stmt.value {
            self.visit_expr(val);
        }
    }

    fn visit_break_stmt(&mut self, stmt: &BreakStmt) {
        if let Some(ref val) = stmt.value {
            self.visit_expr(val);
        }
    }

    fn visit_continue_stmt(&mut self, _stmt: &ContinueStmt) {}

    fn visit_throw_stmt(&mut self, stmt: &ThrowStmt) {
        self.visit_expr(&stmt.value);
    }

    fn visit_pattern(&mut self, pattern: &Pattern) {
        match &pattern.kind {
            PatternKind::Wildcard | PatternKind::Identifier(_) | PatternKind::Rest => {}
            PatternKind::Literal(_) => {}
            PatternKind::Tuple(pats) => {
                for p in pats {
                    self.visit_pattern(p);
                }
            }
            PatternKind::Struct { fields, .. } => {
                for f in fields {
                    if let Some(ref p) = f.pattern {
                        self.visit_pattern(p);
                    }
                }
            }
            PatternKind::Enum { fields, .. } => {
                for p in fields {
                    self.visit_pattern(p);
                }
            }
            PatternKind::Array { elements, .. } => {
                for p in elements {
                    self.visit_pattern(p);
                }
            }
            PatternKind::Or(pats) => {
                for p in pats {
                    self.visit_pattern(p);
                }
            }
            PatternKind::Binding { pattern, .. } => {
                self.visit_pattern(pattern);
            }
            PatternKind::Range { start, end, .. } => {
                self.visit_expr(start);
                self.visit_expr(end);
            }
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Literal(_) => {}
            ExprKind::Identifier(_) => {}
            ExprKind::Binary { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            ExprKind::Unary { operand, .. } => {
                self.visit_expr(operand);
            }
            ExprKind::Call { callee, args } => {
                self.visit_expr(callee);
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.visit_expr(condition);
                self.visit_block(then_branch);
                if let Some(ref eb) = else_branch {
                    match eb {
                        ElseBranch::Block(b) => self.visit_block(b),
                        ElseBranch::ElseIf(e) => self.visit_expr(e),
                    }
                }
            }
            ExprKind::Block(block) => self.visit_block(block),
            ExprKind::Assign { target, value, .. } => {
                self.visit_expr(target);
                self.visit_expr(value);
            }
            ExprKind::FieldAccess { object, .. } => {
                self.visit_expr(object);
            }
            ExprKind::MethodCall { object, args, .. } => {
                self.visit_expr(object);
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            ExprKind::Index { object, index } => {
                self.visit_expr(object);
                self.visit_expr(index);
            }
            ExprKind::Array(elems) => {
                for elem in elems {
                    self.visit_expr(elem);
                }
            }
            ExprKind::Map(entries) => {
                for (key, val) in entries {
                    self.visit_expr(key);
                    self.visit_expr(val);
                }
            }
            ExprKind::Grouping(inner) => self.visit_expr(inner),

            // New expression variants (Step 9)
            ExprKind::Match { scrutinee, arms } => {
                self.visit_expr(scrutinee);
                for arm in arms {
                    self.visit_pattern(&arm.pattern);
                    if let Some(ref guard) = arm.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_expr(&arm.body);
                }
            }
            ExprKind::TryCatch { body, catches } => {
                self.visit_block(body);
                for catch in catches {
                    self.visit_block(&catch.body);
                }
            }
            ExprKind::For {
                pattern,
                iterable,
                body,
            } => {
                self.visit_pattern(pattern);
                self.visit_expr(iterable);
                self.visit_block(body);
            }
            ExprKind::While { condition, body } => {
                self.visit_expr(condition);
                self.visit_block(body);
            }
            ExprKind::Loop { body } => {
                self.visit_block(body);
            }
            ExprKind::Closure { body, .. } => {
                self.visit_expr(body);
            }
            ExprKind::Pipe { left, right } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            ExprKind::Propagate(inner) => {
                self.visit_expr(inner);
            }
            ExprKind::NilCoalesce { left, right } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            ExprKind::Range { start, end, .. } => {
                if let Some(ref s) = start {
                    self.visit_expr(s);
                }
                if let Some(ref e) = end {
                    self.visit_expr(e);
                }
            }
            ExprKind::Cast { expr, .. } => {
                self.visit_expr(expr);
            }
            ExprKind::Path(_) => {}
            ExprKind::Await(inner) => {
                self.visit_expr(inner);
            }
            ExprKind::Tuple(elems) => {
                for elem in elems {
                    self.visit_expr(elem);
                }
            }
            ExprKind::StructLiteral { fields, .. } => {
                for f in fields {
                    self.visit_expr(&f.value);
                }
            }
            ExprKind::StringInterpolation(parts) => {
                for part in parts {
                    if let StringPart::Expr(ref e) = part {
                        self.visit_expr(e);
                    }
                }
            }
            ExprKind::Return(value) => {
                if let Some(val) = value {
                    self.visit_expr(val);
                }
            }
        }
    }
}
