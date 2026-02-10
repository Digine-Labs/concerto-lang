use concerto_common::ir::*;
use concerto_common::ir_opcodes::Opcode;

use crate::ast::*;

use super::constant_pool::ConstantPool;

// ============================================================================
// Code Generator
// ============================================================================

/// Lowers AST to IR instructions.
pub struct CodeGenerator {
    module_name: String,
    source_file: String,
    pool: ConstantPool,
    functions: Vec<IrFunction>,
    models: Vec<IrModel>,
    tools: Vec<IrTool>,
    schemas: Vec<IrSchema>,
    connections: Vec<IrConnection>,
    hashmaps: Vec<IrHashMap>,
    agents: Vec<IrAgent>,
    ledgers: Vec<IrLedger>,
    memories: Vec<IrMemory>,
    pipelines: Vec<IrPipeline>,
    listens: Vec<IrListen>,
    tests: Vec<IrTest>,
    types: Vec<IrType>,
    closure_counter: usize,
    listen_counter: usize,
    /// `use` import aliases: short name → full qualified path (e.g. "parse" → "std::json::parse")
    use_aliases: std::collections::HashMap<String, String>,
}

impl CodeGenerator {
    pub fn new(module_name: impl Into<String>, source_file: impl Into<String>) -> Self {
        Self {
            module_name: module_name.into(),
            source_file: source_file.into(),
            pool: ConstantPool::new(),
            functions: Vec::new(),
            models: Vec::new(),
            tools: Vec::new(),
            schemas: Vec::new(),
            connections: Vec::new(),
            hashmaps: Vec::new(),
            agents: Vec::new(),
            ledgers: Vec::new(),
            memories: Vec::new(),
            pipelines: Vec::new(),
            listens: Vec::new(),
            tests: Vec::new(),
            types: Vec::new(),
            closure_counter: 0,
            listen_counter: 0,
            use_aliases: std::collections::HashMap::new(),
        }
    }

    /// Add connections from Concerto.toml manifest into the IR.
    /// Called before `generate()` to embed external connection configs.
    pub fn add_manifest_connections(&mut self, connections: Vec<IrConnection>) {
        self.connections.extend(connections);
    }

    /// Embed agent configs from Concerto.toml manifest into IR agents.
    /// Called after `generate()` to merge TOML configs into agent declarations.
    pub fn embed_manifest_agents(
        ir: &mut IrModule,
        manifest_agents: &std::collections::HashMap<String, concerto_common::manifest::AgentConfig>,
    ) {
        for ag in &mut ir.agents {
            if let Some(cfg) = manifest_agents.get(&ag.connector) {
                ag.command = cfg.command.clone();
                ag.args = cfg.args.clone();
                ag.env = cfg.env.clone();
                ag.working_dir = cfg.working_dir.clone();
                ag.params = cfg.params.clone();
                // Use TOML timeout as fallback if not set in source
                if ag.timeout.is_none() {
                    ag.timeout = cfg.timeout;
                }
            }
        }
    }

    /// Generate IR from a parsed program.
    pub fn generate(mut self, program: &Program) -> IrModule {
        for decl in &program.declarations {
            self.generate_declaration(decl);
        }

        IrModule {
            version: "0.1.0".to_string(),
            module: self.module_name.clone(),
            source_file: self.source_file.clone(),
            constants: self.pool.into_constants(),
            types: self.types,
            functions: self.functions,
            models: self.models,
            tools: self.tools,
            schemas: self.schemas,
            connections: self.connections,
            hashmaps: self.hashmaps,
            ledgers: self.ledgers,
            memories: self.memories,
            agents: self.agents,
            pipelines: self.pipelines,
            listens: self.listens,
            tests: self.tests,
            source_map: None,
            metadata: IrMetadata {
                compiler_version: "0.1.0".to_string(),
                compiled_at: String::new(),
                optimization_level: 0,
                debug_info: true,
                entry_point: "main".to_string(),
            },
        }
    }

    // ========================================================================
    // Declaration dispatch
    // ========================================================================

    fn generate_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Function(f) => {
                if f.decorators.iter().any(|d| d.name == "test") {
                    self.generate_test_from_fn(f);
                } else {
                    self.generate_function(f);
                }
            }
            Declaration::Model(a) => self.generate_model(a),
            Declaration::Tool(t) => self.generate_tool(t),
            Declaration::Schema(s) => self.generate_schema(s),
            Declaration::Pipeline(p) => self.generate_pipeline(p),
            Declaration::Struct(s) => self.generate_struct_decl(s),
            Declaration::Enum(e) => self.generate_enum_decl(e),
            Declaration::Impl(i) => self.generate_impl(i),
            Declaration::Trait(t) => self.generate_trait_decl(t),
            Declaration::Const(c) => self.generate_const(c),
            Declaration::HashMap(d) => self.generate_hashmap(d),
            Declaration::Ledger(l) => self.generate_ledger(l),
            Declaration::Memory(m) => self.generate_memory(m),
            Declaration::Mcp(m) => self.generate_mcp(m),
            Declaration::Agent(h) => self.generate_agent(h),
            // Use: record alias mapping for identifier substitution; no IR emitted.
            Declaration::Use(u) => {
                let full_path = u.path.join("::");
                let local_name = u
                    .alias
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| u.path.last().cloned().unwrap_or_default());
                if !local_name.is_empty() {
                    self.use_aliases.insert(local_name, full_path);
                }
            }
            // Module, TypeAlias are compile-time only; no IR emitted.
            Declaration::Module(_) | Declaration::TypeAlias(_) => {}
        }
    }

    // ========================================================================
    // Functions & methods
    // ========================================================================

    fn generate_function(&mut self, func: &FunctionDecl) {
        if let Some(ir_func) = self.compile_function(func, &func.name) {
            self.functions.push(ir_func);
        }
    }

    /// Compile a function declaration into an IrFunction.
    /// Returns None if the function has no body (e.g. trait/mcp signatures).
    fn compile_function(&mut self, func: &FunctionDecl, name: &str) -> Option<IrFunction> {
        let body = func.body.as_ref()?;
        let mut ctx = FunctionCtx::new();

        for param in &func.params {
            ctx.add_local(&param.name);
        }

        self.generate_block(body, &mut ctx);

        if !ctx.last_is_return() {
            if body.tail_expr.is_none() {
                let idx = self.pool.add_nil();
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(idx.into())),
                    span: Some([func.span.end.line, func.span.end.column]),
                    ..default_instruction()
                });
            }
            ctx.emit(IrInstruction {
                op: Opcode::Return,
                span: Some([func.span.end.line, func.span.end.column]),
                ..default_instruction()
            });
        }

        let visibility = if func.is_public { "public" } else { "private" };

        Some(IrFunction {
            name: name.to_string(),
            module: self.module_name.clone(),
            visibility: visibility.to_string(),
            params: func
                .params
                .iter()
                .map(|p| IrParam {
                    name: p.name.clone(),
                    param_type: serde_json::Value::String(
                        p.type_ann
                            .as_ref()
                            .map(format_type)
                            .unwrap_or_else(|| "any".to_string()),
                    ),
                })
                .collect(),
            return_type: func
                .return_type
                .as_ref()
                .map(|t| serde_json::Value::String(format_type(t)))
                .unwrap_or(serde_json::Value::String("nil".to_string())),
            is_async: func.is_async,
            locals: ctx.locals,
            instructions: ctx.instructions,
        })
    }

    // ========================================================================
    // Blocks
    // ========================================================================

    /// Generate a block. If the block has a tail expression, its value is
    /// left on the stack. If not, nothing extra is pushed.
    fn generate_block(&mut self, block: &Block, ctx: &mut FunctionCtx) {
        for stmt in &block.stmts {
            self.generate_stmt(stmt, ctx);
        }
        if let Some(ref tail) = block.tail_expr {
            self.generate_expr(tail, ctx);
        }
    }

    /// Generate a block in void context: all values (including tail) are
    /// discarded. Used for loop bodies where the loop value comes from
    /// break, not the block.
    fn generate_block_void(&mut self, block: &Block, ctx: &mut FunctionCtx) {
        for stmt in &block.stmts {
            self.generate_stmt(stmt, ctx);
        }
        if let Some(ref tail) = block.tail_expr {
            self.generate_expr(tail, ctx);
            ctx.emit(IrInstruction {
                op: Opcode::Pop,
                span: Some([tail.span.start.line, tail.span.start.column]),
                ..default_instruction()
            });
        }
    }

    // ========================================================================
    // Statements
    // ========================================================================

    fn generate_stmt(&mut self, stmt: &Stmt, ctx: &mut FunctionCtx) {
        match stmt {
            Stmt::Let(s) => self.generate_let(s, ctx),
            Stmt::Expr(s) => {
                self.generate_expr(&s.expr, ctx);
                ctx.emit(IrInstruction {
                    op: Opcode::Pop,
                    span: Some([s.span.start.line, s.span.start.column]),
                    ..default_instruction()
                });
            }
            Stmt::Return(s) => self.generate_return(s, ctx),
            Stmt::Break(s) => self.generate_break(s, ctx),
            Stmt::Continue(s) => self.generate_continue(s, ctx),
            Stmt::Throw(s) => self.generate_throw(s, ctx),
            Stmt::Mock(m) => self.generate_mock(m, ctx),
        }
    }

    fn generate_let(&mut self, stmt: &LetStmt, ctx: &mut FunctionCtx) {
        ctx.add_local(&stmt.name);
        let span = Some([stmt.span.start.line, stmt.span.start.column]);

        if let Some(ref init) = stmt.initializer {
            self.generate_expr(init, ctx);
        } else {
            let idx = self.pool.add_nil();
            ctx.emit(IrInstruction {
                op: Opcode::LoadConst,
                arg: Some(serde_json::Value::Number(idx.into())),
                span,
                ..default_instruction()
            });
        }

        ctx.emit(IrInstruction {
            op: Opcode::StoreLocal,
            name: Some(stmt.name.clone()),
            span,
            ..default_instruction()
        });
    }

    fn generate_return(&mut self, stmt: &ReturnStmt, ctx: &mut FunctionCtx) {
        let span = Some([stmt.span.start.line, stmt.span.start.column]);
        if let Some(ref value) = stmt.value {
            self.generate_expr(value, ctx);
        } else {
            let idx = self.pool.add_nil();
            ctx.emit(IrInstruction {
                op: Opcode::LoadConst,
                arg: Some(serde_json::Value::Number(idx.into())),
                span,
                ..default_instruction()
            });
        }
        ctx.emit(IrInstruction {
            op: Opcode::Return,
            span,
            ..default_instruction()
        });
    }

    fn generate_break(&mut self, stmt: &BreakStmt, ctx: &mut FunctionCtx) {
        let span = Some([stmt.span.start.line, stmt.span.start.column]);

        // Store break value (or nil) into the loop result variable.
        if let Some(result_var) = ctx.loop_result_var() {
            if let Some(ref value) = stmt.value {
                self.generate_expr(value, ctx);
            } else {
                let idx = self.pool.add_nil();
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(idx.into())),
                    span,
                    ..default_instruction()
                });
            }
            ctx.emit(IrInstruction {
                op: Opcode::StoreLocal,
                name: Some(result_var),
                span,
                ..default_instruction()
            });
            let patch = ctx.emit_placeholder(Opcode::Jump, span);
            ctx.add_break_patch(patch);
        }
    }

    fn generate_continue(&mut self, _stmt: &ContinueStmt, ctx: &mut FunctionCtx) {
        let span = None;
        let patch = ctx.emit_placeholder(Opcode::Jump, span);
        ctx.add_continue_patch(patch);
    }

    fn generate_throw(&mut self, stmt: &ThrowStmt, ctx: &mut FunctionCtx) {
        let span = Some([stmt.span.start.line, stmt.span.start.column]);
        self.generate_expr(&stmt.value, ctx);
        ctx.emit(IrInstruction {
            op: Opcode::Throw,
            span,
            ..default_instruction()
        });
    }

    // ========================================================================
    // Expressions
    // ========================================================================

    fn generate_expr(&mut self, expr: &Expr, ctx: &mut FunctionCtx) {
        let span = Some([expr.span.start.line, expr.span.start.column]);
        match &expr.kind {
            ExprKind::Literal(lit) => {
                let idx = match lit {
                    Literal::Int(v) => self.pool.add_int(*v),
                    Literal::Float(v) => self.pool.add_float(*v),
                    Literal::String(v) => self.pool.add_string(v),
                    Literal::Bool(v) => self.pool.add_bool(*v),
                    Literal::Nil => self.pool.add_nil(),
                };
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(idx.into())),
                    span,
                    ..default_instruction()
                });
            }

            ExprKind::Identifier(name) => {
                // Substitute use-alias with full qualified path if present
                let resolved = self.use_aliases.get(name).cloned().unwrap_or_else(|| name.clone());
                ctx.emit(IrInstruction {
                    op: Opcode::LoadLocal,
                    name: Some(resolved),
                    span,
                    ..default_instruction()
                });
            }

            ExprKind::Binary { left, op, right } => {
                // Short-circuit evaluation for logical operators
                if *op == BinaryOp::And {
                    // left && right: if left is false, skip right
                    self.generate_expr(left, ctx);
                    ctx.emit(IrInstruction {
                        op: Opcode::Dup,
                        span,
                        ..default_instruction()
                    });
                    let jump_end = ctx.emit_placeholder(Opcode::JumpIfFalse, span);
                    ctx.emit(IrInstruction {
                        op: Opcode::Pop,
                        span,
                        ..default_instruction()
                    });
                    self.generate_expr(right, ctx);
                    ctx.patch_jump(jump_end);
                    return;
                }
                if *op == BinaryOp::Or {
                    // left || right: if left is true, skip right
                    self.generate_expr(left, ctx);
                    ctx.emit(IrInstruction {
                        op: Opcode::Dup,
                        span,
                        ..default_instruction()
                    });
                    let jump_end = ctx.emit_placeholder(Opcode::JumpIfTrue, span);
                    ctx.emit(IrInstruction {
                        op: Opcode::Pop,
                        span,
                        ..default_instruction()
                    });
                    self.generate_expr(right, ctx);
                    ctx.patch_jump(jump_end);
                    return;
                }

                self.generate_expr(left, ctx);
                self.generate_expr(right, ctx);
                let opcode = match op {
                    BinaryOp::Add => Opcode::Add,
                    BinaryOp::Sub => Opcode::Sub,
                    BinaryOp::Mul => Opcode::Mul,
                    BinaryOp::Div => Opcode::Div,
                    BinaryOp::Mod => Opcode::Mod,
                    BinaryOp::Eq => Opcode::Eq,
                    BinaryOp::Neq => Opcode::Neq,
                    BinaryOp::Lt => Opcode::Lt,
                    BinaryOp::Gt => Opcode::Gt,
                    BinaryOp::Lte => Opcode::Lte,
                    BinaryOp::Gte => Opcode::Gte,
                    BinaryOp::And | BinaryOp::Or => unreachable!(),
                };
                ctx.emit(IrInstruction {
                    op: opcode,
                    span,
                    ..default_instruction()
                });
            }

            ExprKind::Unary { op, operand } => {
                self.generate_expr(operand, ctx);
                let opcode = match op {
                    UnaryOp::Neg => Opcode::Neg,
                    UnaryOp::Not => Opcode::Not,
                };
                ctx.emit(IrInstruction {
                    op: opcode,
                    span,
                    ..default_instruction()
                });
            }

            ExprKind::Call { callee, args } => {
                // emit() special form
                if let ExprKind::Identifier(name) = &callee.kind {
                    if name == "emit" && args.len() == 2 {
                        self.generate_expr(&args[0], ctx);
                        self.generate_expr(&args[1], ctx);
                        ctx.emit(IrInstruction {
                            op: Opcode::Emit,
                            span,
                            ..default_instruction()
                        });
                        let idx = self.pool.add_nil();
                        ctx.emit(IrInstruction {
                            op: Opcode::LoadConst,
                            arg: Some(serde_json::Value::Number(idx.into())),
                            span,
                            ..default_instruction()
                        });
                        return;
                    }
                }

                // Regular function call
                for arg in args {
                    self.generate_expr(arg, ctx);
                }
                self.generate_expr(callee, ctx);
                ctx.emit(IrInstruction {
                    op: Opcode::Call,
                    argc: Some(args.len() as u32),
                    span,
                    ..default_instruction()
                });
            }

            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.generate_expr(condition, ctx);
                let jump_to_else = ctx.emit_placeholder(Opcode::JumpIfFalse, span);

                self.generate_block(then_branch, ctx);
                if then_branch.tail_expr.is_none() {
                    let idx = self.pool.add_nil();
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadConst,
                        arg: Some(serde_json::Value::Number(idx.into())),
                        span,
                        ..default_instruction()
                    });
                }

                let jump_to_end = ctx.emit_placeholder(Opcode::Jump, span);
                ctx.patch_jump(jump_to_else);

                match else_branch {
                    Some(ElseBranch::Block(block)) => {
                        self.generate_block(block, ctx);
                        if block.tail_expr.is_none() {
                            let idx = self.pool.add_nil();
                            ctx.emit(IrInstruction {
                                op: Opcode::LoadConst,
                                arg: Some(serde_json::Value::Number(idx.into())),
                                span,
                                ..default_instruction()
                            });
                        }
                    }
                    Some(ElseBranch::ElseIf(else_if_expr)) => {
                        self.generate_expr(else_if_expr, ctx);
                    }
                    None => {
                        let idx = self.pool.add_nil();
                        ctx.emit(IrInstruction {
                            op: Opcode::LoadConst,
                            arg: Some(serde_json::Value::Number(idx.into())),
                            span,
                            ..default_instruction()
                        });
                    }
                }

                ctx.patch_jump(jump_to_end);
            }

            ExprKind::Block(block) => {
                self.generate_block(block, ctx);
                if block.tail_expr.is_none() {
                    let idx = self.pool.add_nil();
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadConst,
                        arg: Some(serde_json::Value::Number(idx.into())),
                        span,
                        ..default_instruction()
                    });
                }
            }

            ExprKind::Assign { target, value, op } => {
                self.generate_assign(target, value, *op, ctx, span);
            }

            ExprKind::FieldAccess { object, field } => {
                self.generate_expr(object, ctx);
                ctx.emit(IrInstruction {
                    op: Opcode::FieldGet,
                    name: Some(field.clone()),
                    span,
                    ..default_instruction()
                });
            }

            ExprKind::MethodCall {
                object,
                method,
                type_args,
                args,
            } => {
                self.generate_expr(object, ctx);
                for arg in args {
                    self.generate_expr(arg, ctx);
                }
                // Extract schema name from type args (first type arg, if any)
                let schema = type_args.first().and_then(|ta| {
                    if let crate::ast::types::TypeKind::Named(name) = &ta.kind {
                        Some(name.clone())
                    } else {
                        None
                    }
                });
                ctx.emit(IrInstruction {
                    op: Opcode::CallMethod,
                    name: Some(method.clone()),
                    argc: Some(args.len() as u32),
                    schema,
                    span,
                    ..default_instruction()
                });
            }

            ExprKind::Index { object, index } => {
                self.generate_expr(object, ctx);
                self.generate_expr(index, ctx);
                ctx.emit(IrInstruction {
                    op: Opcode::IndexGet,
                    span,
                    ..default_instruction()
                });
            }

            ExprKind::Array(elements) => {
                for elem in elements {
                    self.generate_expr(elem, ctx);
                }
                ctx.emit(IrInstruction {
                    op: Opcode::BuildArray,
                    count: Some(elements.len() as u32),
                    span,
                    ..default_instruction()
                });
            }

            ExprKind::Map(entries) => {
                for (key, value) in entries {
                    self.generate_expr(key, ctx);
                    self.generate_expr(value, ctx);
                }
                ctx.emit(IrInstruction {
                    op: Opcode::BuildMap,
                    count: Some(entries.len() as u32),
                    span,
                    ..default_instruction()
                });
            }

            ExprKind::Grouping(inner) => {
                self.generate_expr(inner, ctx);
            }

            // --- Loops ---
            ExprKind::While { condition, body } => {
                self.generate_while(condition, body, ctx, span);
            }

            ExprKind::Loop { body } => {
                self.generate_loop(body, ctx, span);
            }

            ExprKind::For {
                pattern,
                iterable,
                body,
            } => {
                self.generate_for(pattern, iterable, body, ctx, span);
            }

            // --- Match ---
            ExprKind::Match { scrutinee, arms } => {
                self.generate_match(scrutinee, arms, ctx, span);
            }

            // --- Error handling ---
            ExprKind::TryCatch { body, catches } => {
                self.generate_try_catch(body, catches, ctx, span);
            }

            ExprKind::Propagate(inner) => {
                self.generate_expr(inner, ctx);
                ctx.emit(IrInstruction {
                    op: Opcode::Propagate,
                    span,
                    ..default_instruction()
                });
            }

            // --- Closures ---
            ExprKind::Closure {
                params,
                return_type,
                body,
            } => {
                self.generate_closure(params, return_type, body, ctx, span);
            }

            // --- Pipe ---
            ExprKind::Pipe { left, right } => {
                self.generate_pipe(left, right, ctx, span);
            }

            // --- Nil coalesce ---
            ExprKind::NilCoalesce { left, right } => {
                self.generate_nil_coalesce(left, right, ctx, span);
            }

            // --- String interpolation ---
            ExprKind::StringInterpolation(parts) => {
                self.generate_string_interpolation(parts, ctx, span);
            }

            // --- Range ---
            ExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                match start {
                    Some(s) => self.generate_expr(s, ctx),
                    None => {
                        let idx = self.pool.add_nil();
                        ctx.emit(IrInstruction {
                            op: Opcode::LoadConst,
                            arg: Some(serde_json::Value::Number(idx.into())),
                            span,
                            ..default_instruction()
                        });
                    }
                }
                match end {
                    Some(e) => self.generate_expr(e, ctx),
                    None => {
                        let idx = self.pool.add_nil();
                        ctx.emit(IrInstruction {
                            op: Opcode::LoadConst,
                            arg: Some(serde_json::Value::Number(idx.into())),
                            span,
                            ..default_instruction()
                        });
                    }
                }
                let incl_idx = self.pool.add_bool(*inclusive);
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(incl_idx.into())),
                    span,
                    ..default_instruction()
                });
                ctx.emit(IrInstruction {
                    op: Opcode::BuildRange,
                    span,
                    ..default_instruction()
                });
            }

            // --- Cast ---
            ExprKind::Cast { expr, target } => {
                self.generate_expr(expr, ctx);
                ctx.emit(IrInstruction {
                    op: Opcode::Cast,
                    type_name: Some(format_type(target)),
                    span,
                    ..default_instruction()
                });
            }

            // --- Path ---
            ExprKind::Path(segments) => {
                let full_path = segments.join("::");
                ctx.emit(IrInstruction {
                    op: Opcode::LoadGlobal,
                    name: Some(full_path),
                    span,
                    ..default_instruction()
                });
            }

            // --- Await ---
            ExprKind::Await(inner) => {
                self.generate_expr(inner, ctx);
                ctx.emit(IrInstruction {
                    op: Opcode::Await,
                    span,
                    ..default_instruction()
                });
            }

            // --- Tuple ---
            ExprKind::Tuple(elements) => {
                for elem in elements {
                    self.generate_expr(elem, ctx);
                }
                ctx.emit(IrInstruction {
                    op: Opcode::BuildArray,
                    count: Some(elements.len() as u32),
                    span,
                    ..default_instruction()
                });
            }

            // --- Struct literal ---
            ExprKind::StructLiteral { name, fields } => {
                let type_name = name.join("::");
                let field_names: Vec<serde_json::Value> = fields
                    .iter()
                    .map(|f| serde_json::Value::String(f.name.clone()))
                    .collect();
                for field in fields {
                    self.generate_expr(&field.value, ctx);
                }
                ctx.emit(IrInstruction {
                    op: Opcode::BuildStruct,
                    type_name: Some(type_name),
                    count: Some(fields.len() as u32),
                    arg: Some(serde_json::Value::Array(field_names)),
                    span,
                    ..default_instruction()
                });
            }

            // --- Listen expression ---
            ExprKind::Listen { call, handlers } => {
                self.generate_listen(call, handlers, ctx, span);
            }

            // --- Return expression (in expression position, e.g., match arms) ---
            ExprKind::Return(value) => {
                if let Some(val) = value {
                    self.generate_expr(val, ctx);
                } else {
                    let nil_idx = self.pool.add_nil();
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadConst,
                        arg: Some(serde_json::Value::Number(nil_idx.into())),
                        span,
                        ..default_instruction()
                    });
                }
                ctx.emit(IrInstruction {
                    op: Opcode::Return,
                    span,
                    ..default_instruction()
                });
                // Push nil as the "value" of this expression (unreachable, but keeps stack balanced)
                let nil_idx = self.pool.add_nil();
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(nil_idx.into())),
                    span,
                    ..default_instruction()
                });
            }
        }
    }

    // ========================================================================
    // Assignment
    // ========================================================================

    fn generate_assign(
        &mut self,
        target: &Expr,
        value: &Expr,
        op: AssignOp,
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        match &target.kind {
            ExprKind::Identifier(name) => {
                match op {
                    AssignOp::Assign => {
                        self.generate_expr(value, ctx);
                    }
                    _ => {
                        ctx.emit(IrInstruction {
                            op: Opcode::LoadLocal,
                            name: Some(name.clone()),
                            span,
                            ..default_instruction()
                        });
                        self.generate_expr(value, ctx);
                        ctx.emit(IrInstruction {
                            op: compound_assign_opcode(op),
                            span,
                            ..default_instruction()
                        });
                    }
                }
                ctx.emit(IrInstruction {
                    op: Opcode::StoreLocal,
                    name: Some(name.clone()),
                    span,
                    ..default_instruction()
                });
                // Assignment expression value
                ctx.emit(IrInstruction {
                    op: Opcode::LoadLocal,
                    name: Some(name.clone()),
                    span,
                    ..default_instruction()
                });
            }

            ExprKind::FieldAccess { object, field } => {
                self.generate_expr(object, ctx);
                if op != AssignOp::Assign {
                    ctx.emit(IrInstruction {
                        op: Opcode::Dup,
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::FieldGet,
                        name: Some(field.clone()),
                        span,
                        ..default_instruction()
                    });
                    self.generate_expr(value, ctx);
                    ctx.emit(IrInstruction {
                        op: compound_assign_opcode(op),
                        span,
                        ..default_instruction()
                    });
                } else {
                    self.generate_expr(value, ctx);
                }
                ctx.emit(IrInstruction {
                    op: Opcode::FieldSet,
                    name: Some(field.clone()),
                    span,
                    ..default_instruction()
                });
                // Assignment expression yields nil
                let idx = self.pool.add_nil();
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(idx.into())),
                    span,
                    ..default_instruction()
                });
            }

            ExprKind::Index { object, index } => {
                if op == AssignOp::Assign {
                    self.generate_expr(object, ctx);
                    self.generate_expr(index, ctx);
                    self.generate_expr(value, ctx);
                } else {
                    // Compound index assignment: use temp locals
                    let tmp_obj = ctx.fresh_local("$obj");
                    let tmp_idx = ctx.fresh_local("$idx");

                    self.generate_expr(object, ctx);
                    ctx.emit(IrInstruction {
                        op: Opcode::StoreLocal,
                        name: Some(tmp_obj.clone()),
                        span,
                        ..default_instruction()
                    });
                    self.generate_expr(index, ctx);
                    ctx.emit(IrInstruction {
                        op: Opcode::StoreLocal,
                        name: Some(tmp_idx.clone()),
                        span,
                        ..default_instruction()
                    });

                    // Read current value
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp_obj.clone()),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp_idx.clone()),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::IndexGet,
                        span,
                        ..default_instruction()
                    });
                    self.generate_expr(value, ctx);
                    ctx.emit(IrInstruction {
                        op: compound_assign_opcode(op),
                        span,
                        ..default_instruction()
                    });

                    // Store: need [collection, index, value]
                    let tmp_val = ctx.fresh_local("$val");
                    ctx.emit(IrInstruction {
                        op: Opcode::StoreLocal,
                        name: Some(tmp_val.clone()),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp_obj),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp_idx),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp_val),
                        span,
                        ..default_instruction()
                    });
                }
                ctx.emit(IrInstruction {
                    op: Opcode::IndexSet,
                    span,
                    ..default_instruction()
                });
                let idx = self.pool.add_nil();
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(idx.into())),
                    span,
                    ..default_instruction()
                });
            }

            _ => {
                // Invalid assignment target (caught by semantic analysis)
                let idx = self.pool.add_nil();
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(idx.into())),
                    span,
                    ..default_instruction()
                });
            }
        }
    }

    // ========================================================================
    // Loops
    // ========================================================================

    fn generate_while(
        &mut self,
        condition: &Expr,
        body: &Block,
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        let result_var = ctx.fresh_local("$loop");
        let nil_idx = self.pool.add_nil();

        // Init loop result to nil
        ctx.emit(IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::Value::Number(nil_idx.into())),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::StoreLocal,
            name: Some(result_var.clone()),
            span,
            ..default_instruction()
        });

        let loop_start = ctx.current_ip();
        ctx.push_loop(result_var.clone());

        self.generate_expr(condition, ctx);
        let jump_end = ctx.emit_placeholder(Opcode::JumpIfFalse, span);

        self.generate_block_void(body, ctx);

        // Patch continue -> loop_start (re-evaluate condition)
        let loop_ctx = ctx.pop_loop();
        for patch in &loop_ctx.continue_patches {
            ctx.instructions[*patch].offset = Some(loop_start as i32);
        }

        ctx.emit(IrInstruction {
            op: Opcode::Jump,
            offset: Some(loop_start as i32),
            span,
            ..default_instruction()
        });

        let loop_end = ctx.current_ip();
        ctx.patch_jump(jump_end);
        for patch in loop_ctx.break_patches {
            ctx.instructions[patch].offset = Some(loop_end as i32);
        }

        // Push loop result
        ctx.emit(IrInstruction {
            op: Opcode::LoadLocal,
            name: Some(result_var),
            span,
            ..default_instruction()
        });
    }

    fn generate_loop(&mut self, body: &Block, ctx: &mut FunctionCtx, span: Option<[u32; 2]>) {
        let result_var = ctx.fresh_local("$loop");
        let nil_idx = self.pool.add_nil();

        ctx.emit(IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::Value::Number(nil_idx.into())),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::StoreLocal,
            name: Some(result_var.clone()),
            span,
            ..default_instruction()
        });

        let loop_start = ctx.current_ip();
        ctx.push_loop(result_var.clone());

        self.generate_block_void(body, ctx);

        let loop_ctx = ctx.pop_loop();
        for patch in &loop_ctx.continue_patches {
            ctx.instructions[*patch].offset = Some(loop_start as i32);
        }

        ctx.emit(IrInstruction {
            op: Opcode::Jump,
            offset: Some(loop_start as i32),
            span,
            ..default_instruction()
        });

        let loop_end = ctx.current_ip();
        for patch in loop_ctx.break_patches {
            ctx.instructions[patch].offset = Some(loop_end as i32);
        }

        ctx.emit(IrInstruction {
            op: Opcode::LoadLocal,
            name: Some(result_var),
            span,
            ..default_instruction()
        });
    }

    fn generate_for(
        &mut self,
        pattern: &Pattern,
        iterable: &Expr,
        body: &Block,
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        let result_var = ctx.fresh_local("$loop");
        let coll_var = ctx.fresh_local("$coll");
        let idx_var = ctx.fresh_local("$idx");
        let nil_idx = self.pool.add_nil();

        // Init result
        ctx.emit(IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::Value::Number(nil_idx.into())),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::StoreLocal,
            name: Some(result_var.clone()),
            span,
            ..default_instruction()
        });

        // Store collection
        self.generate_expr(iterable, ctx);
        ctx.emit(IrInstruction {
            op: Opcode::StoreLocal,
            name: Some(coll_var.clone()),
            span,
            ..default_instruction()
        });

        // Init index = 0
        let zero_idx = self.pool.add_int(0);
        ctx.emit(IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::Value::Number(zero_idx.into())),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::StoreLocal,
            name: Some(idx_var.clone()),
            span,
            ..default_instruction()
        });

        // Loop start: check index < len
        let loop_start = ctx.current_ip();
        ctx.push_loop(result_var.clone());

        ctx.emit(IrInstruction {
            op: Opcode::LoadLocal,
            name: Some(idx_var.clone()),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::LoadLocal,
            name: Some(coll_var.clone()),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::CallMethod,
            name: Some("len".to_string()),
            argc: Some(0),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::Lt,
            span,
            ..default_instruction()
        });
        let jump_end = ctx.emit_placeholder(Opcode::JumpIfFalse, span);

        // Get current element
        ctx.emit(IrInstruction {
            op: Opcode::LoadLocal,
            name: Some(coll_var.clone()),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::LoadLocal,
            name: Some(idx_var.clone()),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::IndexGet,
            span,
            ..default_instruction()
        });

        // Bind element to pattern
        self.emit_pattern_bind(pattern, ctx, span);

        // Body
        self.generate_block_void(body, ctx);

        // Increment step (continue jumps here)
        let increment_ip = ctx.current_ip();

        let one_idx = self.pool.add_int(1);
        ctx.emit(IrInstruction {
            op: Opcode::LoadLocal,
            name: Some(idx_var.clone()),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::Value::Number(one_idx.into())),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::Add,
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::StoreLocal,
            name: Some(idx_var),
            span,
            ..default_instruction()
        });

        let loop_ctx = ctx.pop_loop();
        // continue -> increment step
        for patch in &loop_ctx.continue_patches {
            ctx.instructions[*patch].offset = Some(increment_ip as i32);
        }

        ctx.emit(IrInstruction {
            op: Opcode::Jump,
            offset: Some(loop_start as i32),
            span,
            ..default_instruction()
        });

        let loop_end = ctx.current_ip();
        ctx.patch_jump(jump_end);
        for patch in loop_ctx.break_patches {
            ctx.instructions[patch].offset = Some(loop_end as i32);
        }

        ctx.emit(IrInstruction {
            op: Opcode::LoadLocal,
            name: Some(result_var),
            span,
            ..default_instruction()
        });
    }

    // ========================================================================
    // Match
    // ========================================================================

    fn generate_match(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        // Evaluate scrutinee, store in temp
        let scrutinee_var = ctx.fresh_local("$match");
        self.generate_expr(scrutinee, ctx);
        ctx.emit(IrInstruction {
            op: Opcode::StoreLocal,
            name: Some(scrutinee_var.clone()),
            span,
            ..default_instruction()
        });

        let mut end_patches = Vec::new();

        for arm in arms {
            // Check pattern
            ctx.emit(IrInstruction {
                op: Opcode::LoadLocal,
                name: Some(scrutinee_var.clone()),
                span,
                ..default_instruction()
            });
            self.emit_pattern_check(&arm.pattern, ctx, span);
            let jump_next = ctx.emit_placeholder(Opcode::JumpIfFalse, span);

            // Bind pattern variables
            ctx.emit(IrInstruction {
                op: Opcode::LoadLocal,
                name: Some(scrutinee_var.clone()),
                span,
                ..default_instruction()
            });
            self.emit_pattern_bind(&arm.pattern, ctx, span);

            // Check guard if present
            if let Some(ref guard) = arm.guard {
                self.generate_expr(guard, ctx);
                let guard_fail = ctx.emit_placeholder(Opcode::JumpIfFalse, span);
                // Guard passed: generate body
                self.generate_expr(&arm.body, ctx);
                let end_patch = ctx.emit_placeholder(Opcode::Jump, span);
                end_patches.push(end_patch);
                // Guard failed: push nil and fall through
                ctx.patch_jump(guard_fail);
                ctx.patch_jump(jump_next);
                continue;
            }

            // Generate arm body
            self.generate_expr(&arm.body, ctx);

            let end_patch = ctx.emit_placeholder(Opcode::Jump, span);
            end_patches.push(end_patch);

            ctx.patch_jump(jump_next);
        }

        // No arm matched: push nil as fallback
        let idx = self.pool.add_nil();
        ctx.emit(IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::Value::Number(idx.into())),
            span,
            ..default_instruction()
        });

        for patch in end_patches {
            ctx.patch_jump(patch);
        }
    }

    // ========================================================================
    // Pattern matching
    // ========================================================================

    /// Emit code that checks if the value on top of the stack matches the
    /// pattern. Consumes the value, leaves Bool on the stack.
    fn emit_pattern_check(
        &mut self,
        pattern: &Pattern,
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        match &pattern.kind {
            PatternKind::Wildcard | PatternKind::Identifier(_) | PatternKind::Rest => {
                // Always matches. Pop the value, push true.
                ctx.emit(IrInstruction {
                    op: Opcode::Pop,
                    span,
                    ..default_instruction()
                });
                let idx = self.pool.add_bool(true);
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(idx.into())),
                    span,
                    ..default_instruction()
                });
            }

            PatternKind::Literal(lit) => {
                let idx = match lit {
                    Literal::Int(v) => self.pool.add_int(*v),
                    Literal::Float(v) => self.pool.add_float(*v),
                    Literal::String(v) => self.pool.add_string(v),
                    Literal::Bool(v) => self.pool.add_bool(*v),
                    Literal::Nil => self.pool.add_nil(),
                };
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(idx.into())),
                    span,
                    ..default_instruction()
                });
                ctx.emit(IrInstruction {
                    op: Opcode::Eq,
                    span,
                    ..default_instruction()
                });
            }

            PatternKind::Or(patterns) => {
                // Save value to temp, check each alternative
                let tmp = ctx.fresh_local("$or_pat");
                ctx.emit(IrInstruction {
                    op: Opcode::StoreLocal,
                    name: Some(tmp.clone()),
                    span,
                    ..default_instruction()
                });

                let mut success_patches = Vec::new();
                for sub_pat in patterns {
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp.clone()),
                        span,
                        ..default_instruction()
                    });
                    self.emit_pattern_check(sub_pat, ctx, span);
                    let jump_success = ctx.emit_placeholder(Opcode::JumpIfTrue, span);
                    success_patches.push(jump_success);
                }

                // None matched: push false
                let false_idx = self.pool.add_bool(false);
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(false_idx.into())),
                    span,
                    ..default_instruction()
                });
                let skip_true = ctx.emit_placeholder(Opcode::Jump, span);

                // Success label
                let success_ip = ctx.current_ip();
                for patch in success_patches {
                    ctx.instructions[patch].offset = Some(success_ip as i32);
                }
                let true_idx = self.pool.add_bool(true);
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(true_idx.into())),
                    span,
                    ..default_instruction()
                });

                ctx.patch_jump(skip_true);
            }

            PatternKind::Range {
                start,
                end,
                inclusive,
            } => {
                // Check: start <= value [<= end | < end]
                let tmp = ctx.fresh_local("$range_pat");
                ctx.emit(IrInstruction {
                    op: Opcode::StoreLocal,
                    name: Some(tmp.clone()),
                    span,
                    ..default_instruction()
                });

                // start <= value
                self.generate_expr(start, ctx);
                ctx.emit(IrInstruction {
                    op: Opcode::LoadLocal,
                    name: Some(tmp.clone()),
                    span,
                    ..default_instruction()
                });
                ctx.emit(IrInstruction {
                    op: Opcode::Lte,
                    span,
                    ..default_instruction()
                });
                let fail_start = ctx.emit_placeholder(Opcode::JumpIfFalse, span);

                // value <= end (or < end)
                ctx.emit(IrInstruction {
                    op: Opcode::LoadLocal,
                    name: Some(tmp),
                    span,
                    ..default_instruction()
                });
                self.generate_expr(end, ctx);
                ctx.emit(IrInstruction {
                    op: if *inclusive { Opcode::Lte } else { Opcode::Lt },
                    span,
                    ..default_instruction()
                });
                let skip_false = ctx.emit_placeholder(Opcode::Jump, span);

                // start check failed
                ctx.patch_jump(fail_start);
                let false_idx = self.pool.add_bool(false);
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(false_idx.into())),
                    span,
                    ..default_instruction()
                });

                ctx.patch_jump(skip_false);
            }

            PatternKind::Binding { pattern, .. } => {
                // Check inner pattern (binding happens in emit_pattern_bind)
                self.emit_pattern_check(pattern, ctx, span);
            }

            PatternKind::Enum { path, .. } => {
                // Handle core tagged unions used pervasively in Concerto.
                // This prevents `Ok(...)` from matching `Err(...)` (and vice-versa).
                let variant = path.last().map(|s| s.as_str()).unwrap_or("");
                let check = match variant {
                    "Ok" => Some(("Result", "is_ok")),
                    "Err" => Some(("Result", "is_err")),
                    "Some" => Some(("Option", "is_some")),
                    "None" => Some(("Option", "is_none")),
                    _ => None,
                };

                if let Some((type_name, method)) = check {
                    let tmp = ctx.fresh_local("$enum_check");
                    let false_idx = self.pool.add_bool(false);

                    // Save scrutinee for repeated checks.
                    ctx.emit(IrInstruction {
                        op: Opcode::StoreLocal,
                        name: Some(tmp.clone()),
                        span,
                        ..default_instruction()
                    });

                    // Fast type guard to avoid method-call type errors.
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp.clone()),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::CheckType,
                        type_name: Some(type_name.to_string()),
                        span,
                        ..default_instruction()
                    });
                    let type_mismatch = ctx.emit_placeholder(Opcode::JumpIfFalse, span);

                    // Variant predicate check (`is_ok`/`is_err`/`is_some`/`is_none`).
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::CallMethod,
                        name: Some(method.to_string()),
                        argc: Some(0),
                        span,
                        ..default_instruction()
                    });
                    let done = ctx.emit_placeholder(Opcode::Jump, span);

                    // Type mismatch => no match.
                    let mismatch_ip = ctx.current_ip();
                    ctx.instructions[type_mismatch].offset = Some(mismatch_ip as i32);
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadConst,
                        arg: Some(serde_json::Value::Number(false_idx.into())),
                        span,
                        ..default_instruction()
                    });

                    ctx.patch_jump(done);
                } else {
                    // User-defined enum: compare with LoadGlobal path.
                    let full_path = path.join("::");
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadGlobal,
                        name: Some(full_path),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::Eq,
                        span,
                        ..default_instruction()
                    });
                }
            }

            PatternKind::Tuple(elements) => {
                self.emit_sequence_pattern_check(elements, None, ctx, span);
            }

            PatternKind::Array { elements, .. } => {
                self.emit_sequence_pattern_check(elements, None, ctx, span);
            }

            PatternKind::Struct { path, fields, .. } => {
                let tmp = ctx.fresh_local("$struct_pat");
                let false_idx = self.pool.add_bool(false);

                // Save value to temp
                ctx.emit(IrInstruction {
                    op: Opcode::StoreLocal,
                    name: Some(tmp.clone()),
                    span,
                    ..default_instruction()
                });

                // Check type name matches
                ctx.emit(IrInstruction {
                    op: Opcode::LoadLocal,
                    name: Some(tmp.clone()),
                    span,
                    ..default_instruction()
                });
                let struct_type_name = path.join("::");
                ctx.emit(IrInstruction {
                    op: Opcode::CheckType,
                    type_name: Some(struct_type_name),
                    span,
                    ..default_instruction()
                });
                let fail_type = ctx.emit_placeholder(Opcode::JumpIfFalse, span);

                // Check each field sub-pattern
                let mut fail_patches = vec![fail_type];
                for field in fields {
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp.clone()),
                        span,
                        ..default_instruction()
                    });
                    let field_idx = self.pool.add_string(&field.name);
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadConst,
                        arg: Some(serde_json::Value::Number(field_idx.into())),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::IndexGet,
                        span,
                        ..default_instruction()
                    });
                    if let Some(ref pat) = field.pattern {
                        self.emit_pattern_check(pat, ctx, span);
                    } else {
                        // Just binding, always matches — pop value, push true
                        ctx.emit(IrInstruction {
                            op: Opcode::Pop,
                            span,
                            ..default_instruction()
                        });
                        let true_idx = self.pool.add_bool(true);
                        ctx.emit(IrInstruction {
                            op: Opcode::LoadConst,
                            arg: Some(serde_json::Value::Number(true_idx.into())),
                            span,
                            ..default_instruction()
                        });
                    }
                    let fail_field = ctx.emit_placeholder(Opcode::JumpIfFalse, span);
                    fail_patches.push(fail_field);
                }

                // All checks passed: push true
                let true_idx = self.pool.add_bool(true);
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(true_idx.into())),
                    span,
                    ..default_instruction()
                });
                let skip_false = ctx.emit_placeholder(Opcode::Jump, span);

                // Fail label: push false
                let fail_ip = ctx.current_ip();
                for patch in fail_patches {
                    ctx.instructions[patch].offset = Some(fail_ip as i32);
                }
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(false_idx.into())),
                    span,
                    ..default_instruction()
                });

                ctx.patch_jump(skip_false);
            }
        }
    }

    /// Emit pattern check for tuple/array patterns (both are Value::Array at runtime).
    /// Value on top of stack. Consumes value, pushes Bool.
    fn emit_sequence_pattern_check(
        &mut self,
        elements: &[Pattern],
        _rest: Option<&str>,
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        let tmp = ctx.fresh_local("$seq_pat");
        let false_idx = self.pool.add_bool(false);

        // Save value to temp
        ctx.emit(IrInstruction {
            op: Opcode::StoreLocal,
            name: Some(tmp.clone()),
            span,
            ..default_instruction()
        });

        // Check it's an Array
        ctx.emit(IrInstruction {
            op: Opcode::LoadLocal,
            name: Some(tmp.clone()),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::CheckType,
            type_name: Some("Array".to_string()),
            span,
            ..default_instruction()
        });
        let fail_type = ctx.emit_placeholder(Opcode::JumpIfFalse, span);

        // Check length matches
        ctx.emit(IrInstruction {
            op: Opcode::LoadLocal,
            name: Some(tmp.clone()),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::CallMethod,
            name: Some("len".to_string()),
            argc: Some(0),
            span,
            ..default_instruction()
        });
        let len_idx = self.pool.add_int(elements.len() as i64);
        ctx.emit(IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::Value::Number(len_idx.into())),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::Eq,
            span,
            ..default_instruction()
        });
        let fail_len = ctx.emit_placeholder(Opcode::JumpIfFalse, span);

        // Check each element sub-pattern
        let mut fail_patches = vec![fail_type, fail_len];
        for (i, pat) in elements.iter().enumerate() {
            ctx.emit(IrInstruction {
                op: Opcode::LoadLocal,
                name: Some(tmp.clone()),
                span,
                ..default_instruction()
            });
            let idx = self.pool.add_int(i as i64);
            ctx.emit(IrInstruction {
                op: Opcode::LoadConst,
                arg: Some(serde_json::Value::Number(idx.into())),
                span,
                ..default_instruction()
            });
            ctx.emit(IrInstruction {
                op: Opcode::IndexGet,
                span,
                ..default_instruction()
            });
            self.emit_pattern_check(pat, ctx, span);
            let fail_elem = ctx.emit_placeholder(Opcode::JumpIfFalse, span);
            fail_patches.push(fail_elem);
        }

        // All checks passed: push true
        let true_idx = self.pool.add_bool(true);
        ctx.emit(IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::Value::Number(true_idx.into())),
            span,
            ..default_instruction()
        });
        let skip_false = ctx.emit_placeholder(Opcode::Jump, span);

        // Fail label: push false
        let fail_ip = ctx.current_ip();
        for patch in fail_patches {
            ctx.instructions[patch].offset = Some(fail_ip as i32);
        }
        ctx.emit(IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::Value::Number(false_idx.into())),
            span,
            ..default_instruction()
        });

        ctx.patch_jump(skip_false);
    }

    /// Emit code that binds pattern variables from the value on top of the
    /// stack. Consumes the value.
    fn emit_pattern_bind(
        &mut self,
        pattern: &Pattern,
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        match &pattern.kind {
            PatternKind::Wildcard | PatternKind::Literal(_) | PatternKind::Rest => {
                ctx.emit(IrInstruction {
                    op: Opcode::Pop,
                    span,
                    ..default_instruction()
                });
            }

            PatternKind::Identifier(name) => {
                ctx.add_local(name);
                ctx.emit(IrInstruction {
                    op: Opcode::StoreLocal,
                    name: Some(name.clone()),
                    span,
                    ..default_instruction()
                });
            }

            PatternKind::Binding { name, pattern } => {
                ctx.emit(IrInstruction {
                    op: Opcode::Dup,
                    span,
                    ..default_instruction()
                });
                ctx.add_local(name);
                ctx.emit(IrInstruction {
                    op: Opcode::StoreLocal,
                    name: Some(name.clone()),
                    span,
                    ..default_instruction()
                });
                self.emit_pattern_bind(pattern, ctx, span);
            }

            PatternKind::Tuple(patterns) => {
                let tmp = ctx.fresh_local("$tup");
                ctx.emit(IrInstruction {
                    op: Opcode::StoreLocal,
                    name: Some(tmp.clone()),
                    span,
                    ..default_instruction()
                });
                for (i, pat) in patterns.iter().enumerate() {
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp.clone()),
                        span,
                        ..default_instruction()
                    });
                    let idx = self.pool.add_int(i as i64);
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadConst,
                        arg: Some(serde_json::Value::Number(idx.into())),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::IndexGet,
                        span,
                        ..default_instruction()
                    });
                    self.emit_pattern_bind(pat, ctx, span);
                }
            }

            PatternKind::Struct {
                fields, has_rest, ..
            } => {
                let tmp = ctx.fresh_local("$struct_pat");
                ctx.emit(IrInstruction {
                    op: Opcode::StoreLocal,
                    name: Some(tmp.clone()),
                    span,
                    ..default_instruction()
                });
                for field in fields {
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp.clone()),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::FieldGet,
                        name: Some(field.name.clone()),
                        span,
                        ..default_instruction()
                    });
                    if let Some(ref pat) = field.pattern {
                        self.emit_pattern_bind(pat, ctx, span);
                    } else {
                        // Shorthand: `name` binds to `name`
                        ctx.add_local(&field.name);
                        ctx.emit(IrInstruction {
                            op: Opcode::StoreLocal,
                            name: Some(field.name.clone()),
                            span,
                            ..default_instruction()
                        });
                    }
                }
                let _ = has_rest; // rest is handled by not binding remaining fields
            }

            PatternKind::Enum { path, fields } => {
                // For enum destructuring, the inner values are positional
                let tmp = ctx.fresh_local("$enum_pat");
                ctx.emit(IrInstruction {
                    op: Opcode::StoreLocal,
                    name: Some(tmp.clone()),
                    span,
                    ..default_instruction()
                });
                let _ = path; // type checking already validated
                for (i, pat) in fields.iter().enumerate() {
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp.clone()),
                        span,
                        ..default_instruction()
                    });
                    let idx = self.pool.add_int(i as i64);
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadConst,
                        arg: Some(serde_json::Value::Number(idx.into())),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::IndexGet,
                        span,
                        ..default_instruction()
                    });
                    self.emit_pattern_bind(pat, ctx, span);
                }
            }

            PatternKind::Array { elements, rest } => {
                let tmp = ctx.fresh_local("$arr_pat");
                ctx.emit(IrInstruction {
                    op: Opcode::StoreLocal,
                    name: Some(tmp.clone()),
                    span,
                    ..default_instruction()
                });
                for (i, pat) in elements.iter().enumerate() {
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadLocal,
                        name: Some(tmp.clone()),
                        span,
                        ..default_instruction()
                    });
                    let idx = self.pool.add_int(i as i64);
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadConst,
                        arg: Some(serde_json::Value::Number(idx.into())),
                        span,
                        ..default_instruction()
                    });
                    ctx.emit(IrInstruction {
                        op: Opcode::IndexGet,
                        span,
                        ..default_instruction()
                    });
                    self.emit_pattern_bind(pat, ctx, span);
                }
                let _ = rest; // rest binding handled by runtime
            }

            PatternKind::Or(patterns) => {
                // Or patterns bind the same variables; use first match
                if let Some(first) = patterns.first() {
                    self.emit_pattern_bind(first, ctx, span);
                } else {
                    ctx.emit(IrInstruction {
                        op: Opcode::Pop,
                        span,
                        ..default_instruction()
                    });
                }
            }

            PatternKind::Range { .. } => {
                // Range patterns don't bind (just match)
                ctx.emit(IrInstruction {
                    op: Opcode::Pop,
                    span,
                    ..default_instruction()
                });
            }
        }
    }

    // ========================================================================
    // Try/catch
    // ========================================================================

    fn generate_try_catch(
        &mut self,
        body: &Block,
        catches: &[CatchClause],
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        // TRY_BEGIN -> catch_start
        let try_begin = ctx.emit_placeholder(Opcode::TryBegin, span);

        self.generate_block(body, ctx);
        if body.tail_expr.is_none() {
            let idx = self.pool.add_nil();
            ctx.emit(IrInstruction {
                op: Opcode::LoadConst,
                arg: Some(serde_json::Value::Number(idx.into())),
                span,
                ..default_instruction()
            });
        }

        ctx.emit(IrInstruction {
            op: Opcode::TryEnd,
            span,
            ..default_instruction()
        });
        let jump_past_catch = ctx.emit_placeholder(Opcode::Jump, span);

        // Patch TRY_BEGIN to catch start
        ctx.patch_jump(try_begin);

        // Catch blocks
        let mut catch_exits = Vec::new();
        for catch in catches {
            ctx.emit(IrInstruction {
                op: Opcode::Catch,
                type_name: catch.error_type.as_ref().map(format_type),
                span: Some([catch.span.start.line, catch.span.start.column]),
                ..default_instruction()
            });

            // Bind error variable if present
            if let Some(ref binding) = catch.binding {
                ctx.add_local(binding);
                ctx.emit(IrInstruction {
                    op: Opcode::StoreLocal,
                    name: Some(binding.clone()),
                    span: Some([catch.span.start.line, catch.span.start.column]),
                    ..default_instruction()
                });
            } else {
                ctx.emit(IrInstruction {
                    op: Opcode::Pop,
                    span: Some([catch.span.start.line, catch.span.start.column]),
                    ..default_instruction()
                });
            }

            self.generate_block(&catch.body, ctx);
            if catch.body.tail_expr.is_none() {
                let idx = self.pool.add_nil();
                ctx.emit(IrInstruction {
                    op: Opcode::LoadConst,
                    arg: Some(serde_json::Value::Number(idx.into())),
                    span: Some([catch.span.start.line, catch.span.start.column]),
                    ..default_instruction()
                });
            }

            // Jump past all remaining catches after this body completes
            let catch_exit = ctx.emit_placeholder(Opcode::Jump, span);
            catch_exits.push(catch_exit);
        }

        // Patch all exit jumps (catch body exits + try-success exit) to here
        for exit in catch_exits {
            ctx.patch_jump(exit);
        }
        ctx.patch_jump(jump_past_catch);
    }

    // ========================================================================
    // Listen expression
    // ========================================================================

    fn generate_listen(
        &mut self,
        call: &Expr,
        handlers: &[ListenHandler],
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        // Extract agent name and args from the call expression.
        // Expected: MethodCall { object: Identifier(agent), method: "execute", args: [prompt] }
        let (agent_name, args) = match &call.kind {
            ExprKind::MethodCall {
                object,
                method: _,
                args,
                ..
            } => {
                let name = match &object.kind {
                    ExprKind::Identifier(name) => name.clone(),
                    _ => "unknown".to_string(),
                };
                (name, args)
            }
            _ => {
                // Fallback: push the call expression result and use "unknown" agent
                self.generate_expr(call, ctx);
                ("unknown".to_string(), &vec![] as &Vec<Expr>)
            }
        };

        // Push call args onto the stack (typically just the prompt)
        for arg in args {
            self.generate_expr(arg, ctx);
        }

        // Compile each handler body as instruction block (pipeline stage pattern)
        let ir_handlers: Vec<IrListenHandler> = handlers
            .iter()
            .map(|h| {
                let mut handler_ctx = FunctionCtx::new();
                handler_ctx.add_local(&h.param.name);
                self.generate_block(&h.body, &mut handler_ctx);

                // Ensure handler body has a RETURN
                if h.body.tail_expr.is_none() {
                    let nil_idx = self.pool.add_nil();
                    handler_ctx.emit(IrInstruction {
                        op: Opcode::LoadConst,
                        arg: Some(serde_json::Value::Number(nil_idx.into())),
                        span: None,
                        ..default_instruction()
                    });
                }
                handler_ctx.emit(IrInstruction {
                    op: Opcode::Return,
                    span: None,
                    ..default_instruction()
                });

                let param_type = h
                    .param
                    .type_ann
                    .as_ref()
                    .map(|t| serde_json::Value::String(format!("{t:?}")))
                    .unwrap_or(serde_json::Value::String("any".to_string()));

                IrListenHandler {
                    message_type: h.message_type.clone(),
                    param: IrParam {
                        name: h.param.name.clone(),
                        param_type,
                    },
                    instructions: handler_ctx.instructions,
                }
            })
            .collect();

        // Create IrListen definition
        let listen_name = format!("$listen_{}", self.listen_counter);
        self.listen_counter += 1;

        self.listens.push(IrListen {
            name: listen_name.clone(),
            agent: agent_name.clone(),
            handlers: ir_handlers,
        });

        // Emit ListenBegin instruction
        ctx.emit(IrInstruction {
            op: Opcode::ListenBegin,
            name: Some(agent_name),
            arg: Some(serde_json::Value::String(listen_name)),
            argc: Some(args.len() as u32),
            span,
            ..default_instruction()
        });
    }

    // ========================================================================
    // Closures
    // ========================================================================

    fn generate_closure(
        &mut self,
        params: &[Param],
        return_type: &Option<TypeAnnotation>,
        body: &Expr,
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        let closure_name = format!("$closure_{}", self.closure_counter);
        self.closure_counter += 1;

        let mut closure_ctx = FunctionCtx::new();
        for param in params {
            closure_ctx.add_local(&param.name);
        }

        // Generate closure body
        match &body.kind {
            ExprKind::Block(block) => {
                self.generate_block(block, &mut closure_ctx);
                if block.tail_expr.is_none() {
                    let idx = self.pool.add_nil();
                    closure_ctx.emit(IrInstruction {
                        op: Opcode::LoadConst,
                        arg: Some(serde_json::Value::Number(idx.into())),
                        span,
                        ..default_instruction()
                    });
                }
            }
            _ => {
                self.generate_expr(body, &mut closure_ctx);
            }
        }

        if !closure_ctx.last_is_return() {
            closure_ctx.emit(IrInstruction {
                op: Opcode::Return,
                span,
                ..default_instruction()
            });
        }

        self.functions.push(IrFunction {
            name: closure_name.clone(),
            module: self.module_name.clone(),
            visibility: "private".to_string(),
            params: params
                .iter()
                .map(|p| IrParam {
                    name: p.name.clone(),
                    param_type: serde_json::Value::String(
                        p.type_ann
                            .as_ref()
                            .map(format_type)
                            .unwrap_or_else(|| "any".to_string()),
                    ),
                })
                .collect(),
            return_type: return_type
                .as_ref()
                .map(|t| serde_json::Value::String(format_type(t)))
                .unwrap_or(serde_json::Value::String("any".to_string())),
            is_async: false,
            locals: closure_ctx.locals,
            instructions: closure_ctx.instructions,
        });

        // Push closure reference
        let idx = self.pool.add_string(&closure_name);
        ctx.emit(IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::Value::Number(idx.into())),
            span,
            ..default_instruction()
        });
    }

    // ========================================================================
    // Pipe
    // ========================================================================

    fn generate_pipe(
        &mut self,
        left: &Expr,
        right: &Expr,
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        // Pipe: left |> right
        // If right is a call `f(args...)`, becomes `f(left, args...)`
        // If right is an identifier/path, becomes `right(left)`
        match &right.kind {
            ExprKind::Call { callee, args } => {
                self.generate_expr(left, ctx);
                for arg in args {
                    self.generate_expr(arg, ctx);
                }
                self.generate_expr(callee, ctx);
                ctx.emit(IrInstruction {
                    op: Opcode::Call,
                    argc: Some((args.len() + 1) as u32),
                    span,
                    ..default_instruction()
                });
            }
            ExprKind::MethodCall {
                object,
                method,
                type_args,
                args,
            } => {
                // left |> obj.method(args) => obj.method(left, args)
                self.generate_expr(object, ctx);
                self.generate_expr(left, ctx);
                for arg in args {
                    self.generate_expr(arg, ctx);
                }
                let schema = type_args.first().and_then(|ta| {
                    if let crate::ast::types::TypeKind::Named(name) = &ta.kind {
                        Some(name.clone())
                    } else {
                        None
                    }
                });
                ctx.emit(IrInstruction {
                    op: Opcode::CallMethod,
                    name: Some(method.clone()),
                    argc: Some((args.len() + 1) as u32),
                    schema,
                    span,
                    ..default_instruction()
                });
            }
            _ => {
                // Treat right as a function: right(left)
                self.generate_expr(left, ctx);
                self.generate_expr(right, ctx);
                ctx.emit(IrInstruction {
                    op: Opcode::Call,
                    argc: Some(1),
                    span,
                    ..default_instruction()
                });
            }
        }
    }

    // ========================================================================
    // Nil coalesce
    // ========================================================================

    fn generate_nil_coalesce(
        &mut self,
        left: &Expr,
        right: &Expr,
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        // left ?? right
        // If left is not nil (or None), use left; otherwise use right.
        // NilCoalescePrep normalizes Option(None)→Nil, Option(Some(v))→v.
        self.generate_expr(left, ctx);
        ctx.emit(IrInstruction {
            op: Opcode::NilCoalescePrep,
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::Dup,
            span,
            ..default_instruction()
        });
        let nil_idx = self.pool.add_nil();
        ctx.emit(IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::Value::Number(nil_idx.into())),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::Neq,
            span,
            ..default_instruction()
        });
        // Stack: [left_val, (left != nil)]
        let jump_keep = ctx.emit_placeholder(Opcode::JumpIfTrue, span);
        // left is nil: pop it and evaluate right
        ctx.emit(IrInstruction {
            op: Opcode::Pop,
            span,
            ..default_instruction()
        });
        self.generate_expr(right, ctx);
        let jump_end = ctx.emit_placeholder(Opcode::Jump, span);
        // left is not nil: keep it
        ctx.patch_jump(jump_keep);
        ctx.patch_jump(jump_end);
    }

    // ========================================================================
    // String interpolation
    // ========================================================================

    fn generate_string_interpolation(
        &mut self,
        parts: &[StringPart],
        ctx: &mut FunctionCtx,
        span: Option<[u32; 2]>,
    ) {
        if parts.is_empty() {
            let idx = self.pool.add_string("");
            ctx.emit(IrInstruction {
                op: Opcode::LoadConst,
                arg: Some(serde_json::Value::Number(idx.into())),
                span,
                ..default_instruction()
            });
            return;
        }

        let mut first = true;
        for part in parts {
            match part {
                StringPart::Literal(s) => {
                    let idx = self.pool.add_string(s);
                    ctx.emit(IrInstruction {
                        op: Opcode::LoadConst,
                        arg: Some(serde_json::Value::Number(idx.into())),
                        span,
                        ..default_instruction()
                    });
                }
                StringPart::Expr(expr) => {
                    self.generate_expr(expr, ctx);
                    // Runtime coerces to string via Add
                }
            }
            if !first {
                ctx.emit(IrInstruction {
                    op: Opcode::Add,
                    span,
                    ..default_instruction()
                });
            }
            first = false;
        }
    }

    // ========================================================================
    // Declaration lowering
    // ========================================================================

    fn generate_model(&mut self, model: &ModelDecl) {
        let mut connection = String::new();
        let mut base = None;
        let mut temperature = None;
        let mut max_tokens = None;
        let mut system_prompt = None;
        let mut timeout = None;
        let mut tools = Vec::new();
        let mut memory = None;

        for field in &model.fields {
            match field.name.as_str() {
                "provider" => {
                    if let ExprKind::Identifier(name) = &field.value.kind {
                        connection = name.clone();
                    }
                }
                "base" => {
                    if let ExprKind::Literal(Literal::String(s)) = &field.value.kind {
                        base = Some(s.clone());
                    }
                }
                "temperature" => {
                    if let ExprKind::Literal(Literal::Float(f)) = &field.value.kind {
                        temperature = Some(*f);
                    }
                }
                "max_tokens" => {
                    if let ExprKind::Literal(Literal::Int(n)) = &field.value.kind {
                        max_tokens = Some(*n as u32);
                    }
                }
                "system_prompt" => {
                    if let ExprKind::Literal(Literal::String(s)) = &field.value.kind {
                        system_prompt = Some(s.clone());
                    }
                }
                "timeout" => {
                    if let ExprKind::Literal(Literal::Int(n)) = &field.value.kind {
                        timeout = Some(*n as u32);
                    }
                }
                "tools" => {
                    if let ExprKind::Array(elems) = &field.value.kind {
                        for elem in elems {
                            if let ExprKind::Identifier(name) = &elem.kind {
                                tools.push(name.clone());
                            }
                        }
                    }
                }
                "memory" => {
                    if let ExprKind::Identifier(name) = &field.value.kind {
                        memory = Some(name.clone());
                    }
                }
                _ => {}
            }
        }

        self.models.push(IrModel {
            name: model.name.clone(),
            module: self.module_name.clone(),
            connection,
            config: IrModelConfig {
                base,
                temperature,
                max_tokens,
                system_prompt,
                timeout,
            },
            tools,
            memory,
            decorators: model.decorators.iter().map(lower_decorator).collect(),
            methods: Vec::new(),
        });
    }

    fn generate_tool(&mut self, tool: &ToolDecl) {
        let methods: Vec<IrFunction> = tool
            .methods
            .iter()
            .filter_map(|m| self.compile_function(m, &m.name))
            .collect();

        // Generate tool schemas from @describe/@param decorators
        let tool_schemas = self.generate_tool_schemas(&tool.name, &tool.methods);

        self.tools.push(IrTool {
            name: tool.name.clone(),
            module: self.module_name.clone(),
            methods,
            tool_schemas,
        });
    }

    /// Generate JSON Schema entries for each tool method from @describe/@param decorators.
    fn generate_tool_schemas(
        &self,
        tool_name: &str,
        methods: &[FunctionDecl],
    ) -> Vec<ToolSchemaEntry> {
        methods
            .iter()
            .filter_map(|m| {
                // Extract @describe
                let description = m.decorators.iter().find_map(|d| {
                    if d.name == "describe" {
                        d.args.first().and_then(|a| match a {
                            DecoratorArg::Positional(expr) => {
                                if let ExprKind::Literal(Literal::String(s)) = &expr.kind {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })
                    } else {
                        None
                    }
                })?;

                // Extract @param decorators: @param("name", "description")
                let param_descriptions: Vec<(String, String)> = m
                    .decorators
                    .iter()
                    .filter(|d| d.name == "param")
                    .filter_map(|d| {
                        let mut args_iter = d.args.iter();
                        let name = args_iter.next().and_then(|a| match a {
                            DecoratorArg::Positional(expr) => {
                                if let ExprKind::Literal(Literal::String(s)) = &expr.kind {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })?;
                        let desc = args_iter
                            .next()
                            .and_then(|a| match a {
                                DecoratorArg::Positional(expr) => {
                                    if let ExprKind::Literal(Literal::String(s)) = &expr.kind {
                                        Some(s.clone())
                                    } else {
                                        None
                                    }
                                }
                                _ => None,
                            })
                            .unwrap_or_default();
                        Some((name, desc))
                    })
                    .collect();

                // Build JSON Schema for parameters (skip `self` param)
                let mut properties = serde_json::Map::new();
                let mut required = Vec::new();
                for param in &m.params {
                    let param_desc = param_descriptions
                        .iter()
                        .find(|(n, _)| n == &param.name)
                        .map(|(_, d)| d.as_str())
                        .unwrap_or("");

                    let json_type = param
                        .type_ann
                        .as_ref()
                        .map(concerto_type_to_json_schema)
                        .unwrap_or_else(|| serde_json::json!({ "type": "string" }));

                    let mut prop = json_type;
                    if !param_desc.is_empty() {
                        if let Some(obj) = prop.as_object_mut() {
                            obj.insert(
                                "description".to_string(),
                                serde_json::Value::String(param_desc.to_string()),
                            );
                        }
                    }
                    properties.insert(param.name.clone(), prop);
                    let is_optional_type = param
                        .type_ann
                        .as_ref()
                        .is_some_and(is_option_type_annotation);
                    if param.default.is_none() && !is_optional_type {
                        required.push(serde_json::Value::String(param.name.clone()));
                    }
                }

                let parameters = serde_json::json!({
                    "type": "object",
                    "properties": properties,
                    "required": required,
                });

                Some(ToolSchemaEntry {
                    method_name: format!("{}::{}", tool_name, m.name),
                    description,
                    parameters,
                })
            })
            .collect()
    }

    fn generate_schema(&mut self, schema: &SchemaDecl) {
        use super::super::ast::types::TypeKind;
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for field in &schema.fields {
            let prop = match &field.type_ann.kind {
                TypeKind::Union(variants) => {
                    // String literal union -> JSON schema enum
                    let enum_vals: Vec<serde_json::Value> = variants
                        .iter()
                        .filter_map(|v| {
                            if let TypeKind::StringLiteral(s) = &v.kind {
                                Some(serde_json::Value::String(s.clone()))
                            } else {
                                None
                            }
                        })
                        .collect();
                    serde_json::json!({ "type": "string", "enum": enum_vals })
                }
                _ => serde_json::json!({ "type": format_type(&field.type_ann) }),
            };
            properties.insert(field.name.clone(), prop);
            if !field.is_optional {
                required.push(serde_json::Value::String(field.name.clone()));
            }
        }

        let json_schema = serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required,
        });

        self.schemas.push(IrSchema {
            name: schema.name.clone(),
            json_schema,
            validation_mode: "strict".to_string(),
        });
    }

    fn generate_pipeline(&mut self, pipeline: &PipelineDecl) {
        let stages: Vec<IrPipelineStage> = pipeline
            .stages
            .iter()
            .map(|stage| {
                let mut ctx = FunctionCtx::new();
                for param in &stage.params {
                    ctx.add_local(&param.name);
                }
                self.generate_block(&stage.body, &mut ctx);
                if !ctx.last_is_return() {
                    if stage.body.tail_expr.is_none() {
                        let idx = self.pool.add_nil();
                        ctx.emit(IrInstruction {
                            op: Opcode::LoadConst,
                            arg: Some(serde_json::Value::Number(idx.into())),
                            span: None,
                            ..default_instruction()
                        });
                    }
                    ctx.emit(IrInstruction {
                        op: Opcode::Return,
                        span: None,
                        ..default_instruction()
                    });
                }

                IrPipelineStage {
                    name: stage.name.clone(),
                    params: stage
                        .params
                        .iter()
                        .map(|p| IrParam {
                            name: p.name.clone(),
                            param_type: p
                                .type_ann
                                .as_ref()
                                .map(|t| serde_json::Value::String(format_type(t)))
                                .unwrap_or(serde_json::Value::String("any".to_string())),
                        })
                        .collect(),
                    input_type: stage
                        .params
                        .first()
                        .and_then(|p| p.type_ann.as_ref())
                        .map(|t| serde_json::Value::String(format_type(t)))
                        .unwrap_or(serde_json::Value::String("any".to_string())),
                    output_type: stage
                        .return_type
                        .as_ref()
                        .map(|t| serde_json::Value::String(format_type(t)))
                        .unwrap_or(serde_json::Value::String("any".to_string())),
                    decorators: stage.decorators.iter().map(lower_decorator).collect(),
                    instructions: ctx.instructions,
                }
            })
            .collect();

        self.pipelines.push(IrPipeline {
            name: pipeline.name.clone(),
            stages,
            input_type: pipeline
                .input_param
                .as_ref()
                .and_then(|p| p.type_ann.as_ref())
                .map(|t| serde_json::Value::String(format_type(t))),
            output_type: pipeline
                .return_type
                .as_ref()
                .map(|t| serde_json::Value::String(format_type(t))),
        });
    }

    fn generate_struct_decl(&mut self, s: &StructDecl) {
        self.types.push(IrType {
            name: s.name.clone(),
            kind: "struct".to_string(),
            fields: s
                .fields
                .iter()
                .map(|f| IrTypeField {
                    name: f.name.clone(),
                    field_type: serde_json::Value::String(format_type(&f.type_ann)),
                    required: Some(!f.is_optional),
                })
                .collect(),
            variants: Vec::new(),
        });
    }

    fn generate_enum_decl(&mut self, e: &EnumDecl) {
        self.types.push(IrType {
            name: e.name.clone(),
            kind: "enum".to_string(),
            fields: Vec::new(),
            variants: e
                .variants
                .iter()
                .map(|v| IrEnumVariant {
                    name: v.name.clone(),
                    data: match &v.kind {
                        EnumVariantKind::Unit => Vec::new(),
                        EnumVariantKind::Tuple(types) => types
                            .iter()
                            .enumerate()
                            .map(|(i, t)| IrTypeField {
                                name: format!("_{}", i),
                                field_type: serde_json::Value::String(format_type(t)),
                                required: Some(true),
                            })
                            .collect(),
                        EnumVariantKind::Struct(fields) => fields
                            .iter()
                            .map(|f| IrTypeField {
                                name: f.name.clone(),
                                field_type: serde_json::Value::String(format_type(&f.type_ann)),
                                required: Some(!f.is_optional),
                            })
                            .collect(),
                    },
                })
                .collect(),
        });
    }

    fn generate_impl(&mut self, imp: &ImplDecl) {
        for method in &imp.methods {
            let qualified_name = if let Some(ref trait_name) = imp.trait_name {
                format!("{}::{}::{}", imp.target, trait_name, method.name)
            } else {
                format!("{}::{}", imp.target, method.name)
            };
            if let Some(ir_func) = self.compile_function(method, &qualified_name) {
                self.functions.push(ir_func);
            }
        }
    }

    fn generate_trait_decl(&mut self, t: &TraitDecl) {
        // Compile default implementations as functions
        for method in &t.methods {
            if method.body.is_some() {
                let qualified_name = format!("{}::{}", t.name, method.name);
                if let Some(ir_func) = self.compile_function(method, &qualified_name) {
                    self.functions.push(ir_func);
                }
            }
        }
    }

    fn generate_const(&mut self, c: &ConstDecl) {
        // Constants are compiled as global-level store instructions.
        // For the IR, we emit them as a special init function.
        let mut ctx = FunctionCtx::new();
        let span = Some([c.span.start.line, c.span.start.column]);

        self.generate_expr(&c.value, &mut ctx);
        ctx.emit(IrInstruction {
            op: Opcode::StoreGlobal,
            name: Some(c.name.clone()),
            span,
            ..default_instruction()
        });
        let idx = self.pool.add_nil();
        ctx.emit(IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::Value::Number(idx.into())),
            span,
            ..default_instruction()
        });
        ctx.emit(IrInstruction {
            op: Opcode::Return,
            span,
            ..default_instruction()
        });

        self.functions.push(IrFunction {
            name: format!("$const_{}", c.name),
            module: self.module_name.clone(),
            visibility: "private".to_string(),
            params: Vec::new(),
            return_type: serde_json::Value::String("nil".to_string()),
            is_async: false,
            locals: ctx.locals,
            instructions: ctx.instructions,
        });
    }

    fn generate_hashmap(&mut self, hm: &HashMapDecl) {
        use super::super::ast::types::TypeKind;
        let (key_type, value_type) = match &hm.type_ann.kind {
            TypeKind::Generic { name, args }
                if (name == "Map" || name == "HashMap") && args.len() == 2 =>
            {
                (format_type(&args[0]), format_type(&args[1]))
            }
            _ => ("any".to_string(), "any".to_string()),
        };

        self.hashmaps.push(IrHashMap {
            name: hm.name.clone(),
            key_type,
            value_type,
            persistence: None,
        });
    }

    fn generate_ledger(&mut self, ledger: &LedgerDecl) {
        self.ledgers.push(IrLedger {
            name: ledger.name.clone(),
        });
    }

    fn generate_memory(&mut self, mem: &MemoryDecl) {
        // Extract max_messages from initializer if it's Memory::new(max: N)
        let max_messages = self.extract_memory_max(&mem.initializer);
        self.memories.push(IrMemory {
            name: mem.name.clone(),
            max_messages,
        });
    }

    /// Try to extract `max` argument from `Memory::new(max: N)` initializer.
    fn extract_memory_max(&self, expr: &Expr) -> Option<u32> {
        // The initializer is a Call to Memory::new() possibly with named args
        if let ExprKind::Call { args, .. } = &expr.kind {
            for arg in args {
                // Named args in decorators are parsed differently, but for regular
                // function calls the parser doesn't produce named args.
                // For now, if there's a single int arg, treat it as max.
                if let ExprKind::Literal(Literal::Int(n)) = &arg.kind {
                    return Some(*n as u32);
                }
            }
        }
        None
    }

    fn generate_mcp(&mut self, mcp: &McpDecl) {
        let mut config = serde_json::Map::new();
        config.insert(
            "type".to_string(),
            serde_json::Value::String("mcp".to_string()),
        );
        for field in &mcp.fields {
            config.insert(field.name.clone(), expr_to_json(&field.value));
        }

        // Record MCP tool signatures in config
        let tool_sigs: Vec<serde_json::Value> = mcp
            .methods
            .iter()
            .map(|m| {
                serde_json::json!({
                    "name": m.name,
                    "params": m.params.iter().map(|p| serde_json::json!({
                        "name": p.name,
                        "type": p.type_ann.as_ref().map(format_type).unwrap_or_else(|| "any".to_string()),
                    })).collect::<Vec<_>>(),
                    "return_type": m.return_type.as_ref().map(format_type).unwrap_or_else(|| "any".to_string()),
                })
            })
            .collect();
        config.insert("tools".to_string(), serde_json::Value::Array(tool_sigs));

        self.connections.push(IrConnection {
            name: mcp.name.clone(),
            config: serde_json::Value::Object(config),
        });
    }

    // ========================================================================
    // Agent declaration
    // ========================================================================

    fn generate_agent(&mut self, ag: &AgentDecl) {
        let connector = ag
            .fields
            .iter()
            .find(|f| f.name == "connector")
            .map(|f| expr_to_string_value(&f.value))
            .unwrap_or_default();
        let input_format = ag
            .fields
            .iter()
            .find(|f| f.name == "input_format")
            .map(|f| expr_to_string_value(&f.value))
            .unwrap_or_else(|| "text".to_string());
        let output_format = ag
            .fields
            .iter()
            .find(|f| f.name == "output_format")
            .map(|f| expr_to_string_value(&f.value))
            .unwrap_or_else(|| "text".to_string());
        let timeout = ag
            .fields
            .iter()
            .find(|f| f.name == "timeout")
            .and_then(|f| expr_to_u32(&f.value));
        let decorators = ag.decorators.iter().map(lower_decorator).collect();
        self.agents.push(IrAgent {
            name: ag.name.clone(),
            connector,
            input_format,
            output_format,
            timeout,
            decorators,
            command: None,
            args: None,
            env: None,
            working_dir: None,
            params: None,
        });
    }

    fn generate_test_from_fn(&mut self, func: &FunctionDecl) {
        let Some(ref body) = func.body else {
            return;
        };

        // Extract description from @test("description") or use function name
        let description = func
            .decorators
            .iter()
            .find(|d| d.name == "test")
            .and_then(|d| d.args.first())
            .and_then(|arg| match arg {
                DecoratorArg::Positional(expr) => {
                    if let ExprKind::Literal(Literal::String(s)) = &expr.kind {
                        Some(s.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .unwrap_or_else(|| func.name.clone());

        // Check for @expect_fail decorator
        let expect_fail_decorator = func.decorators.iter().find(|d| d.name == "expect_fail");
        let expect_fail = expect_fail_decorator.is_some();
        let expect_fail_message = expect_fail_decorator.and_then(|d| {
            d.args.first().and_then(|arg| match arg {
                DecoratorArg::Positional(expr) => {
                    if let ExprKind::Literal(Literal::String(s)) = &expr.kind {
                        Some(s.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
        });

        let mut ctx = FunctionCtx::new();

        self.generate_block(body, &mut ctx);

        // Ensure trailing return
        if body.tail_expr.is_none() {
            let nil_idx = self.pool.add_nil();
            ctx.emit(IrInstruction {
                op: Opcode::LoadConst,
                arg: Some(serde_json::Value::Number(nil_idx.into())),
                span: Some([func.span.end.line, func.span.end.column]),
                ..default_instruction()
            });
        }
        ctx.emit(IrInstruction {
            op: Opcode::Return,
            span: Some([func.span.end.line, func.span.end.column]),
            ..default_instruction()
        });

        self.tests.push(IrTest {
            description,
            instructions: ctx.instructions,
            expect_fail,
            expect_fail_message,
        });
    }

    fn generate_mock(&mut self, mock: &MockStmt, ctx: &mut FunctionCtx) {
        // Build mock config from fields
        let mut config = serde_json::Map::new();
        for field in &mock.fields {
            config.insert(field.name.clone(), expr_to_json(&field.value));
        }

        ctx.emit(IrInstruction {
            op: Opcode::MockModel,
            name: Some(mock.model_name.clone()),
            arg: Some(serde_json::Value::Object(config)),
            span: Some([mock.span.start.line, mock.span.start.column]),
            ..default_instruction()
        });
    }
}

// ============================================================================
// FunctionCtx
// ============================================================================

/// Loop context for break/continue support.
struct LoopCtx {
    break_patches: Vec<usize>,
    continue_patches: Vec<usize>,
    result_var: String,
}

/// Per-function context for instruction emission.
struct FunctionCtx {
    instructions: Vec<IrInstruction>,
    locals: Vec<String>,
    loop_stack: Vec<LoopCtx>,
    temp_counter: usize,
}

impl FunctionCtx {
    fn new() -> Self {
        Self {
            instructions: Vec::new(),
            locals: Vec::new(),
            loop_stack: Vec::new(),
            temp_counter: 0,
        }
    }

    fn add_local(&mut self, name: &str) {
        if !self.locals.contains(&name.to_string()) {
            self.locals.push(name.to_string());
        }
    }

    /// Allocate a fresh temporary local variable name.
    fn fresh_local(&mut self, prefix: &str) -> String {
        let name = format!("{}_{}", prefix, self.temp_counter);
        self.temp_counter += 1;
        self.add_local(&name);
        name
    }

    fn emit(&mut self, instr: IrInstruction) {
        self.instructions.push(instr);
    }

    fn current_ip(&self) -> usize {
        self.instructions.len()
    }

    /// Emit a jump placeholder, returning the index for later patching.
    fn emit_placeholder(&mut self, op: Opcode, span: Option<[u32; 2]>) -> usize {
        let idx = self.instructions.len();
        self.instructions.push(IrInstruction {
            op,
            offset: Some(0), // placeholder
            span,
            ..default_instruction()
        });
        idx
    }

    /// Patch a jump instruction to point to the current instruction index.
    fn patch_jump(&mut self, placeholder_idx: usize) {
        let target = self.instructions.len() as i32;
        self.instructions[placeholder_idx].offset = Some(target);
    }

    fn last_is_return(&self) -> bool {
        self.instructions
            .last()
            .is_some_and(|i| i.op == Opcode::Return)
    }

    fn push_loop(&mut self, result_var: String) {
        self.loop_stack.push(LoopCtx {
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
            result_var,
        });
    }

    fn pop_loop(&mut self) -> LoopCtx {
        self.loop_stack.pop().expect("no loop to pop")
    }

    /// Get the result variable name of the current loop, if any.
    fn loop_result_var(&self) -> Option<String> {
        self.loop_stack.last().map(|l| l.result_var.clone())
    }

    fn add_break_patch(&mut self, patch: usize) {
        if let Some(loop_ctx) = self.loop_stack.last_mut() {
            loop_ctx.break_patches.push(patch);
        }
    }

    fn add_continue_patch(&mut self, patch: usize) {
        if let Some(loop_ctx) = self.loop_stack.last_mut() {
            loop_ctx.continue_patches.push(patch);
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn default_instruction() -> IrInstruction {
    IrInstruction {
        op: Opcode::Pop, // will be overwritten
        arg: None,
        name: None,
        model: None,
        method: None,
        schema: None,
        tool: None,
        hashmap_name: None,
        type_name: None,
        argc: None,
        offset: None,
        count: None,
        span: None,
    }
}

fn compound_assign_opcode(op: AssignOp) -> Opcode {
    match op {
        AssignOp::AddAssign => Opcode::Add,
        AssignOp::SubAssign => Opcode::Sub,
        AssignOp::MulAssign => Opcode::Mul,
        AssignOp::DivAssign => Opcode::Div,
        AssignOp::ModAssign => Opcode::Mod,
        AssignOp::Assign => unreachable!(),
    }
}

fn format_type(ty: &super::super::ast::types::TypeAnnotation) -> String {
    use super::super::ast::types::TypeKind;
    match &ty.kind {
        TypeKind::Named(name) => name.clone(),
        TypeKind::Generic { name, args } => {
            let arg_strs: Vec<String> = args.iter().map(format_type).collect();
            format!("{}<{}>", name, arg_strs.join(", "))
        }
        TypeKind::Tuple(elems) => {
            let elem_strs: Vec<String> = elems.iter().map(format_type).collect();
            format!("({})", elem_strs.join(", "))
        }
        TypeKind::Function {
            params,
            return_type,
        } => {
            let param_strs: Vec<String> = params.iter().map(format_type).collect();
            format!(
                "fn({}) -> {}",
                param_strs.join(", "),
                format_type(return_type)
            )
        }
        TypeKind::Union(variants) => {
            let variant_strs: Vec<String> = variants.iter().map(format_type).collect();
            variant_strs.join(" | ")
        }
        TypeKind::StringLiteral(s) => format!("\"{}\"", s),
        TypeKind::Inferred => "any".to_string(),
    }
}

/// Convert a Concerto type annotation to a JSON Schema value.
fn concerto_type_to_json_schema(
    ty: &super::super::ast::types::TypeAnnotation,
) -> serde_json::Value {
    use super::super::ast::types::TypeKind;
    match &ty.kind {
        TypeKind::Named(name) => match name.as_str() {
            "Int" => serde_json::json!({ "type": "integer" }),
            "Float" => serde_json::json!({ "type": "number" }),
            "String" => serde_json::json!({ "type": "string" }),
            "Bool" => serde_json::json!({ "type": "boolean" }),
            _ => serde_json::json!({ "type": "object" }),
        },
        TypeKind::Generic { name, args } => match name.as_str() {
            "Array" => {
                let items = args
                    .first()
                    .map(concerto_type_to_json_schema)
                    .unwrap_or(serde_json::json!({ "type": "string" }));
                serde_json::json!({ "type": "array", "items": items })
            }
            "Map" => serde_json::json!({ "type": "object" }),
            "Option" => {
                // Optional types are handled at the property level (not in required)
                args.first()
                    .map(concerto_type_to_json_schema)
                    .unwrap_or(serde_json::json!({ "type": "string" }))
            }
            _ => serde_json::json!({ "type": "object" }),
        },
        TypeKind::Union(variants) => {
            let enum_vals: Vec<serde_json::Value> = variants
                .iter()
                .filter_map(|v| {
                    if let TypeKind::StringLiteral(s) = &v.kind {
                        Some(serde_json::Value::String(s.clone()))
                    } else {
                        None
                    }
                })
                .collect();
            if !enum_vals.is_empty() {
                serde_json::json!({ "type": "string", "enum": enum_vals })
            } else {
                serde_json::json!({ "type": "string" })
            }
        }
        _ => serde_json::json!({ "type": "string" }),
    }
}

fn is_option_type_annotation(ty: &super::super::ast::types::TypeAnnotation) -> bool {
    use super::super::ast::types::TypeKind;
    matches!(&ty.kind, TypeKind::Generic { name, .. } if name == "Option")
}

fn expr_to_json(expr: &Expr) -> serde_json::Value {
    match &expr.kind {
        ExprKind::Literal(lit) => match lit {
            Literal::Int(v) => serde_json::json!(*v),
            Literal::Float(v) => serde_json::json!(*v),
            Literal::String(v) => serde_json::json!(v),
            Literal::Bool(v) => serde_json::json!(*v),
            Literal::Nil => serde_json::Value::Null,
        },
        ExprKind::Identifier(name) => serde_json::json!(name),
        ExprKind::Array(elems) => {
            let vals: Vec<serde_json::Value> = elems.iter().map(expr_to_json).collect();
            serde_json::Value::Array(vals)
        }
        _ => serde_json::Value::Null,
    }
}

/// Extract a string from a literal String expression or an identifier name.
fn expr_to_string_value(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Literal(Literal::String(s)) => s.clone(),
        ExprKind::Identifier(name) => name.clone(),
        _ => String::new(),
    }
}

/// Extract a u32 from a literal Int expression.
fn expr_to_u32(expr: &Expr) -> Option<u32> {
    match &expr.kind {
        ExprKind::Literal(Literal::Int(n)) => Some(*n as u32),
        _ => None,
    }
}

fn lower_decorator(d: &Decorator) -> IrDecorator {
    let args = if d.args.is_empty() {
        None
    } else {
        let args_json: Vec<serde_json::Value> = d
            .args
            .iter()
            .map(|a| match a {
                DecoratorArg::Positional(expr) => expr_to_json(expr),
                DecoratorArg::Named { name, value, .. } => {
                    serde_json::json!({ name.clone(): expr_to_json(value) })
                }
            })
            .collect();
        Some(serde_json::Value::Array(args_json))
    };
    IrDecorator {
        name: d.name.clone(),
        args,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn compile(source: &str) -> IrModule {
        let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
        assert!(!lex_diags.has_errors());
        let (program, parse_diags) = Parser::new(tokens).parse();
        assert!(!parse_diags.has_errors());
        CodeGenerator::new("test", "test.conc").generate(&program)
    }

    #[test]
    fn generates_ir_module() {
        let ir = compile("fn main() {}");
        assert_eq!(ir.version, "0.1.0");
        assert_eq!(ir.module, "test");
        assert_eq!(ir.functions.len(), 1);
        assert_eq!(ir.functions[0].name, "main");
    }

    #[test]
    fn constant_pool_deduplication() {
        let ir = compile(r#"fn main() { let x = 42; let y = 42; }"#);
        let int_constants: Vec<_> = ir
            .constants
            .iter()
            .filter(|c| c.const_type == "int")
            .collect();
        assert_eq!(int_constants.len(), 1);
    }

    #[test]
    fn let_generates_store() {
        let ir = compile("fn main() { let x = 5; }");
        let main = &ir.functions[0];
        assert!(main.locals.contains(&"x".to_string()));
        assert!(main
            .instructions
            .iter()
            .any(|i| i.op == Opcode::StoreLocal && i.name.as_deref() == Some("x")));
    }

    #[test]
    fn binary_expr_generates_ops() {
        let ir = compile("fn main() { let z = 1 + 2 * 3; }");
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::Mul));
        assert!(ops.contains(&Opcode::Add));
        let mul_pos = ops.iter().position(|o| *o == Opcode::Mul).unwrap();
        let add_pos = ops.iter().position(|o| *o == Opcode::Add).unwrap();
        assert!(mul_pos < add_pos);
    }

    #[test]
    fn if_generates_jumps() {
        let ir = compile(
            r#"
            fn main() {
                let x = 5;
                if x > 3 {
                    let y = 1;
                }
            }
        "#,
        );
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::JumpIfFalse));
        assert!(ops.contains(&Opcode::Jump));
    }

    #[test]
    fn emit_generates_emit_instruction() {
        let ir = compile(r#"fn main() { emit("result", 42); }"#);
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::Emit));
    }

    #[test]
    fn function_with_return() {
        let ir = compile("fn add(a: Int, b: Int) -> Int { return a + b; }");
        let func = &ir.functions[0];
        assert_eq!(func.params.len(), 2);
        assert_eq!(
            func.return_type,
            serde_json::Value::String("Int".to_string())
        );
        assert!(func.instructions.iter().any(|i| i.op == Opcode::Return));
    }

    #[test]
    fn milestone_program_generates_valid_ir() {
        let ir = compile(
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
        assert_eq!(ir.functions.len(), 1);
        let main = &ir.functions[0];
        assert!(main.locals.contains(&"x".to_string()));
        assert!(main.locals.contains(&"y".to_string()));

        let json = serde_json::to_string_pretty(&ir).unwrap();
        assert!(json.contains("\"version\": \"0.1.0\""));
        assert!(json.contains("\"name\": \"main\""));
    }

    #[test]
    fn ir_serializes_to_json() {
        let ir = compile("fn main() { let x = 42; }");
        let json = serde_json::to_string(&ir).unwrap();
        let ir2: IrModule = serde_json::from_str(&json).unwrap();
        assert_eq!(ir2.version, ir.version);
        assert_eq!(ir2.functions.len(), ir.functions.len());
    }

    // --- New tests for Step 11 ---

    #[test]
    fn while_loop_generates_jumps() {
        let ir = compile(
            r#"
            fn main() {
                let mut x = 0;
                while x < 10 {
                    x += 1;
                }
            }
        "#,
        );
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::JumpIfFalse));
        assert!(ops.contains(&Opcode::Jump));
        assert!(ops.contains(&Opcode::Lt));
    }

    #[test]
    fn loop_generates_unconditional_jump() {
        let ir = compile(
            r#"
            fn main() {
                let mut x = 0;
                loop {
                    x += 1;
                    if x > 5 { break; }
                }
            }
        "#,
        );
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        // Should have a backward jump for the loop
        let jump_count = ops.iter().filter(|o| **o == Opcode::Jump).count();
        assert!(jump_count >= 2); // At least loop-back and break jump
    }

    #[test]
    fn for_loop_generates_iteration() {
        let ir = compile(
            r#"
            fn main() {
                for x in [1, 2, 3] {
                    emit("val", x);
                }
            }
        "#,
        );
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::BuildArray));
        assert!(ops.contains(&Opcode::CallMethod)); // len()
        assert!(ops.contains(&Opcode::Lt));
        assert!(ops.contains(&Opcode::IndexGet));
        assert!(ops.contains(&Opcode::Emit));
    }

    #[test]
    fn throw_generates_throw_op() {
        let ir = compile(
            r#"
            fn fail() {
                throw "error";
            }
        "#,
        );
        let func = &ir.functions[0];
        let ops: Vec<Opcode> = func.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::Throw));
    }

    #[test]
    fn try_catch_generates_error_handling() {
        let ir = compile(
            r#"
            fn main() {
                try {
                    let x = 1;
                } catch {
                    let y = 2;
                }
            }
        "#,
        );
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::TryBegin));
        assert!(ops.contains(&Opcode::TryEnd));
        assert!(ops.contains(&Opcode::Catch));
    }

    #[test]
    fn propagate_generates_op() {
        let ir = compile(
            r#"
            fn main() -> Result<Int, String> {
                let x = some_func()?;
                return x;
            }
        "#,
        );
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::Propagate));
    }

    #[test]
    fn closure_generates_function() {
        let ir = compile(
            r#"
            fn main() {
                let f = |x: Int| x + 1;
            }
        "#,
        );
        // Should have main + closure function
        assert!(ir.functions.len() >= 2);
        assert!(ir.functions.iter().any(|f| f.name.starts_with("$closure_")));
    }

    #[test]
    fn pipe_generates_call() {
        let ir = compile(
            r#"
            fn double(x: Int) -> Int { return x * 2; }
            fn main() {
                let result = 5 |> double;
            }
        "#,
        );
        let main = ir.functions.iter().find(|f| f.name == "main").unwrap();
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::Call));
    }

    #[test]
    fn nil_coalesce_generates_jumps() {
        let ir = compile(
            r#"
            fn main() {
                let x = nil ?? 42;
            }
        "#,
        );
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::Dup));
        assert!(ops.contains(&Opcode::Neq));
        assert!(ops.contains(&Opcode::JumpIfTrue));
    }

    #[test]
    fn string_interpolation_generates_concat() {
        let ir = compile(
            r#"
            fn main() {
                let name = "world";
                let msg = "hello ${name}!";
            }
        "#,
        );
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::Add)); // string concat
    }

    #[test]
    fn match_generates_pattern_checks() {
        let ir = compile(
            r#"
            fn main() {
                let x = 5;
                match x {
                    1 => emit("one", 1),
                    _ => emit("other", x),
                }
            }
        "#,
        );
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::Eq)); // literal pattern check
        assert!(ops.contains(&Opcode::JumpIfFalse)); // arm skip
    }

    #[test]
    fn struct_literal_generates_build_struct() {
        let ir = compile(
            r#"
            struct Point { x: Int, y: Int }
            fn main() {
                let p = Point { x: 1, y: 2 };
            }
        "#,
        );
        let main = ir.functions.iter().find(|f| f.name == "main").unwrap();
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::BuildStruct));
        // Verify struct type is registered
        assert!(ir
            .types
            .iter()
            .any(|t| t.name == "Point" && t.kind == "struct"));
    }

    #[test]
    fn enum_generates_type() {
        let ir = compile(
            r#"
            enum Color {
                Red,
                Green,
                Blue,
            }
            fn main() {}
        "#,
        );
        let color_type = ir.types.iter().find(|t| t.name == "Color").unwrap();
        assert_eq!(color_type.kind, "enum");
        assert_eq!(color_type.variants.len(), 3);
    }

    #[test]
    fn agent_generates_ir_agent() {
        let ir = compile(
            r#"
            model MyAgent {
                provider: openai,
                base: "gpt-4o",
            }
            fn main() {}
        "#,
        );
        assert_eq!(ir.models.len(), 1);
        assert_eq!(ir.models[0].name, "MyAgent");
        assert_eq!(ir.models[0].connection, "openai");
        assert_eq!(ir.models[0].config.base, Some("gpt-4o".to_string()));
    }

    #[test]
    fn tool_generates_ir_tool() {
        let ir = compile(
            r#"
            tool Calculator {
                description: "A calculator",
                @describe("adds two numbers")
                pub fn add(self, a: Int, b: Int) -> Int {
                    return a + b;
                }
            }
            fn main() {}
        "#,
        );
        assert_eq!(ir.tools.len(), 1);
        assert_eq!(ir.tools[0].name, "Calculator");
        assert_eq!(ir.tools[0].methods.len(), 1);
        assert_eq!(ir.tools[0].methods[0].name, "add");
    }

    #[test]
    fn schema_generates_json_schema() {
        let ir = compile(
            r#"
            schema UserInput {
                name: String,
                age?: Int,
            }
            fn main() {}
        "#,
        );
        assert_eq!(ir.schemas.len(), 1);
        assert_eq!(ir.schemas[0].name, "UserInput");
        let schema = &ir.schemas[0].json_schema;
        assert!(schema["properties"]["name"].is_object());
        // name is required, age is optional
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("name")));
        assert!(!required.contains(&serde_json::json!("age")));
    }

    #[test]
    fn pipeline_generates_stages() {
        let ir = compile(
            r#"
            pipeline TextProcess {
                stage parse(input: String) -> Int {
                    return 42;
                }
                stage format(n: Int) -> String {
                    return "done";
                }
            }
            fn main() {}
        "#,
        );
        assert_eq!(ir.pipelines.len(), 1);
        assert_eq!(ir.pipelines[0].name, "TextProcess");
        assert_eq!(ir.pipelines[0].stages.len(), 2);
        assert_eq!(ir.pipelines[0].stages[0].name, "parse");
        assert_eq!(ir.pipelines[0].stages[1].name, "format");
    }

    #[test]
    fn impl_generates_qualified_functions() {
        let ir = compile(
            r#"
            struct Foo { x: Int }
            impl Foo {
                pub fn get_x(self) -> Int {
                    return self.x;
                }
            }
            fn main() {}
        "#,
        );
        assert!(ir.functions.iter().any(|f| f.name == "Foo::get_x"));
    }

    #[test]
    fn cast_generates_cast_op() {
        let ir = compile(
            r#"
            fn main() {
                let x = 42 as Float;
            }
        "#,
        );
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::Cast));
    }

    #[test]
    fn await_generates_await_op() {
        let ir = compile(
            r#"
            async fn fetch() -> String { return "data"; }
            async fn main() {
                let data = fetch().await;
            }
        "#,
        );
        let main = ir.functions.iter().find(|f| f.name == "main").unwrap();
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::Await));
    }

    #[test]
    fn tuple_generates_build_array() {
        let ir = compile(
            r#"
            fn main() {
                let t = (1, 2, 3);
            }
        "#,
        );
        let main = &ir.functions[0];
        let build_ops: Vec<_> = main
            .instructions
            .iter()
            .filter(|i| i.op == Opcode::BuildArray)
            .collect();
        assert!(!build_ops.is_empty());
    }

    #[test]
    fn range_generates_build_range() {
        let ir = compile(
            r#"
            fn main() {
                let r = 1..10;
            }
        "#,
        );
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::BuildRange));
    }

    #[test]
    fn path_generates_load_global() {
        let ir = compile(
            r#"
            fn main() {
                let x = std::io::read;
            }
        "#,
        );
        let main = &ir.functions[0];
        assert!(main
            .instructions
            .iter()
            .any(|i| i.op == Opcode::LoadGlobal && i.name.as_deref() == Some("std::io::read")));
    }

    #[test]
    fn tool_schema_generation() {
        let ir = compile(
            r#"
            tool Calculator {
                @describe("Add two numbers")
                @param("a", "First number")
                @param("b", "Second number")
                pub fn add(self, a: Int, b: Int) -> Int {
                    a + b
                }
            }
            fn main() {}
        "#,
        );
        assert_eq!(ir.tools.len(), 1);
        let tool = &ir.tools[0];
        assert_eq!(tool.name, "Calculator");
        assert_eq!(tool.tool_schemas.len(), 1);

        let schema = &tool.tool_schemas[0];
        assert_eq!(schema.method_name, "Calculator::add");
        assert_eq!(schema.description, "Add two numbers");

        let params = schema.parameters.as_object().unwrap();
        assert_eq!(params["type"], "object");
        let props = params["properties"].as_object().unwrap();
        assert!(props.contains_key("a"));
        assert!(props.contains_key("b"));
        assert_eq!(props["a"]["type"], "integer");
        assert_eq!(props["a"]["description"], "First number");
        assert_eq!(props["b"]["type"], "integer");
    }

    #[test]
    fn tool_schema_option_param_not_required() {
        let ir = compile(
            r#"
            tool Searcher {
                description: "Search utility",

                @describe("Search with optional limit")
                @param("query", "Search query")
                @param("tags", "Tag filters")
                @param("limit", "Optional max results")
                pub fn search(self, query: String, tags: Array<String>, limit: Option<Int>) -> String {
                    query
                }
            }
            fn main() {}
        "#,
        );
        assert_eq!(ir.tools.len(), 1);
        let schema = &ir.tools[0].tool_schemas[0];
        assert_eq!(schema.method_name, "Searcher::search");

        let params = schema.parameters.as_object().unwrap();
        let props = params["properties"].as_object().unwrap();
        assert_eq!(props["query"]["type"], "string");
        assert_eq!(props["tags"]["type"], "array");
        assert_eq!(props["tags"]["items"]["type"], "string");
        assert_eq!(props["limit"]["type"], "integer");

        let required = params["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::Value::String("query".to_string())));
        assert!(required.contains(&serde_json::Value::String("tags".to_string())));
        assert!(!required.contains(&serde_json::Value::String("limit".to_string())));
    }

    #[test]
    fn memory_declaration_ir() {
        let ir = compile(
            r#"
            memory conv: Memory = Memory::new();
            fn main() {}
        "#,
        );
        assert_eq!(ir.memories.len(), 1);
        assert_eq!(ir.memories[0].name, "conv");
        assert!(ir.memories[0].max_messages.is_none());
    }

    #[test]
    fn memory_with_max_ir() {
        let ir = compile(
            r#"
            memory conv: Memory = Memory::new(50);
            fn main() {}
        "#,
        );
        assert_eq!(ir.memories.len(), 1);
        assert_eq!(ir.memories[0].name, "conv");
        assert_eq!(ir.memories[0].max_messages, Some(50));
    }

    // ========================================================================
    // Listen codegen tests
    // ========================================================================

    #[test]
    fn listen_generates_ir_listen() {
        let ir = compile(
            r#"
            agent MyAgent {
                connector: "test_agent",
            }
            fn main() {
                let result = listen MyAgent.execute("do work") {
                    "progress" => |msg| {
                        emit("log", msg);
                    },
                };
            }
        "#,
        );
        // Should have generated a listen definition
        assert_eq!(ir.listens.len(), 1);
        assert_eq!(ir.listens[0].name, "$listen_0");
        assert_eq!(ir.listens[0].agent, "MyAgent");
        assert_eq!(ir.listens[0].handlers.len(), 1);
        assert_eq!(ir.listens[0].handlers[0].message_type, "progress");
        assert_eq!(ir.listens[0].handlers[0].param.name, "msg");
    }

    #[test]
    fn listen_emits_listen_begin_opcode() {
        let ir = compile(
            r#"
            agent MyAgent {
                connector: "test_agent",
            }
            fn main() {
                let result = listen MyAgent.execute("prompt") {
                    "progress" => |msg| {
                        emit("log", msg);
                    },
                };
            }
        "#,
        );
        let main = &ir.functions[0];
        let ops: Vec<Opcode> = main.instructions.iter().map(|i| i.op).collect();
        assert!(ops.contains(&Opcode::ListenBegin));
        // Verify the ListenBegin instruction has correct fields
        let listen_inst = main
            .instructions
            .iter()
            .find(|i| i.op == Opcode::ListenBegin)
            .unwrap();
        assert_eq!(listen_inst.name.as_deref(), Some("MyAgent"));
        assert_eq!(
            listen_inst.arg.as_ref().and_then(|a| a.as_str()),
            Some("$listen_0")
        );
        assert_eq!(listen_inst.argc, Some(1)); // one arg (the prompt)
    }

    #[test]
    fn listen_handler_instructions_have_return() {
        let ir = compile(
            r#"
            agent MyAgent {
                connector: "test_agent",
            }
            fn main() {
                let result = listen MyAgent.execute("prompt") {
                    "question" => |q| {
                        "answer"
                    },
                };
            }
        "#,
        );
        // Handler instructions should end with RETURN
        let handler = &ir.listens[0].handlers[0];
        let last_op = handler.instructions.last().unwrap().op;
        assert_eq!(last_op, Opcode::Return);
    }

    #[test]
    fn listen_multiple_handlers_ir() {
        let ir = compile(
            r#"
            agent MyAgent {
                connector: "test_agent",
            }
            fn main() {
                let result = listen MyAgent.execute("prompt") {
                    "progress" => |p| {
                        emit("log", p);
                    },
                    "question" => |q| {
                        "answer"
                    },
                };
            }
        "#,
        );
        assert_eq!(ir.listens[0].handlers.len(), 2);
        assert_eq!(ir.listens[0].handlers[0].message_type, "progress");
        assert_eq!(ir.listens[0].handlers[1].message_type, "question");
    }
}
