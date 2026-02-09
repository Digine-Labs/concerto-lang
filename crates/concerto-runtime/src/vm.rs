use std::collections::HashMap;

use concerto_common::ir::IrInstruction;
use concerto_common::ir_opcodes::Opcode;

use crate::builtins;
use crate::error::{Result, RuntimeError};
use crate::agent::AgentRegistry;
use crate::ir_loader::LoadedModule;
use crate::ledger::LedgerStore;
use crate::mcp::McpRegistry;
use crate::memory::MemoryStore;
use crate::provider::{ChatMessage, ChatRequest, ConnectionManager};
use crate::schema::SchemaValidator;
use crate::tool::ToolRegistry;
use crate::value::Value;

const MAX_CALL_DEPTH: usize = 1000;

// ============================================================================
// Call Frame
// ============================================================================

struct CallFrame {
    function_name: String,
    instructions: Vec<IrInstruction>,
    pc: usize,
    locals: HashMap<String, Value>,
}

// ============================================================================
// Try Frame (exception handler)
// ============================================================================

/// An active exception handler installed by TRY_BEGIN.
struct TryFrame {
    /// PC of the first CATCH instruction to jump to on error.
    catch_pc: usize,
    /// call_stack depth at TRY_BEGIN (for unwinding nested calls).
    call_depth: usize,
    /// Stack height at TRY_BEGIN (for cleanup on error).
    stack_height: usize,
}

// ============================================================================
// VM
// ============================================================================

/// The Concerto stack-based virtual machine.
pub struct VM {
    module: LoadedModule,
    stack: Vec<Value>,
    call_stack: Vec<CallFrame>,
    globals: HashMap<String, Value>,
    /// In-memory hashmaps (hashmap_name -> key -> value).
    hashmaps: HashMap<String, HashMap<String, Value>>,
    /// Ledger store (fault-tolerant knowledge stores).
    ledger_store: LedgerStore,
    /// Memory store (conversation history).
    memory_store: MemoryStore,
    /// Exception handler stack for try/catch.
    try_stack: Vec<TryFrame>,
    /// Tool instance state registry.
    tool_registry: ToolRegistry,
    /// LLM connection manager (real providers or mock fallback).
    connection_manager: ConnectionManager,
    /// MCP server connections (stdio transport).
    mcp_registry: McpRegistry,
    /// Agent connections (external agent systems).
    agent_registry: AgentRegistry,
    /// Emit handler callback.
    #[allow(clippy::type_complexity)]
    emit_handler: Box<dyn Fn(&str, &Value)>,
    /// Mock model responses (model_name -> mock response text).
    mock_models: HashMap<String, MockConfig>,
    /// Captured emits during test execution.
    test_emits: Vec<(String, Value)>,
    /// Whether to capture emits for test_emits() built-in.
    test_capture_emits: bool,
}

/// Mock configuration for a model.
#[derive(Clone)]
struct MockConfig {
    response: Option<String>,
    error: Option<String>,
}

impl VM {
    /// Create a new VM from a loaded module.
    pub fn new(module: LoadedModule) -> Self {
        let mut globals = HashMap::new();

        // Register models as ModelRef values
        for name in module.models.keys() {
            globals.insert(name.clone(), Value::ModelRef(name.clone()));
        }

        // Register schemas as SchemaRef values
        for name in module.schemas.keys() {
            globals.insert(name.clone(), Value::SchemaRef(name.clone()));
        }

        // Register tool references so `[ToolName]` works in with_tools().
        for name in module.tools.keys() {
            globals.insert(name.clone(), Value::Function(name.clone()));
        }

        // Register MCP references so `[McpServerName]` works in with_tools().
        for (name, conn) in &module.connections {
            let is_mcp = conn
                .config
                .get("type")
                .and_then(|v| v.as_str())
                .is_some_and(|t| t == "mcp");
            if is_mcp {
                globals.insert(name.clone(), Value::Function(name.clone()));
            }
        }

        // Register hashmaps as HashMapRef values
        for name in module.hashmaps.keys() {
            globals.insert(name.clone(), Value::HashMapRef(name.clone()));
        }

        // Register pipelines as PipelineRef values
        for name in module.pipelines.keys() {
            globals.insert(name.clone(), Value::PipelineRef(name.clone()));
        }

        // Register built-in functions
        globals.insert("Ok".to_string(), Value::Function("$builtin_ok".to_string()));
        globals.insert(
            "Err".to_string(),
            Value::Function("$builtin_err".to_string()),
        );
        globals.insert(
            "Some".to_string(),
            Value::Function("$builtin_some".to_string()),
        );
        globals.insert("None".to_string(), Value::Option(None));
        globals.insert(
            "env".to_string(),
            Value::Function("$builtin_env".to_string()),
        );
        globals.insert(
            "print".to_string(),
            Value::Function("$builtin_print".to_string()),
        );
        globals.insert(
            "println".to_string(),
            Value::Function("$builtin_println".to_string()),
        );
        globals.insert(
            "len".to_string(),
            Value::Function("$builtin_len".to_string()),
        );
        globals.insert(
            "typeof".to_string(),
            Value::Function("$builtin_typeof".to_string()),
        );
        globals.insert(
            "panic".to_string(),
            Value::Function("$builtin_panic".to_string()),
        );

        // Register assertion built-ins
        globals.insert(
            "assert".to_string(),
            Value::Function("$builtin_assert".to_string()),
        );
        globals.insert(
            "assert_eq".to_string(),
            Value::Function("$builtin_assert_eq".to_string()),
        );
        globals.insert(
            "assert_ne".to_string(),
            Value::Function("$builtin_assert_ne".to_string()),
        );
        globals.insert(
            "test_emits".to_string(),
            Value::Function("$builtin_test_emits".to_string()),
        );

        // Register path-based constructors (e.g., ToolError::new)
        globals.insert(
            "ToolError::new".to_string(),
            Value::Function("$builtin_tool_error_new".to_string()),
        );

        // Initialize hashmaps
        let mut hashmaps = HashMap::new();
        for name in module.hashmaps.keys() {
            hashmaps.insert(name.clone(), HashMap::new());
        }

        // Initialize ledger store
        let mut ledger_store = LedgerStore::new();
        for name in module.ledgers.keys() {
            ledger_store.init_ledger(name);
            globals.insert(name.clone(), Value::LedgerRef(name.clone()));
        }

        // Initialize memory store
        let mut memory_store = MemoryStore::new();
        for (name, ir_mem) in &module.memories {
            memory_store.init_memory(name, ir_mem.max_messages);
            globals.insert(name.clone(), Value::MemoryRef(name.clone()));
        }

        // Initialize tool registry
        let mut tool_registry = ToolRegistry::new();
        for name in module.tools.keys() {
            tool_registry.register_tool(name);
        }

        // Initialize connection manager from IR connections
        let connection_manager = ConnectionManager::from_connections(&module.connections);

        // Initialize MCP registry from IR connections (type: "mcp")
        let mcp_registry = McpRegistry::from_connections(&module.connections);

        // Initialize agent registry from IR agents
        let mut agent_registry = AgentRegistry::new();
        for ir_agent in module.agents.values() {
            agent_registry.register(ir_agent);
            globals.insert(ir_agent.name.clone(), Value::AgentRef(ir_agent.name.clone()));
        }

        VM {
            module,
            stack: Vec::with_capacity(256),
            call_stack: Vec::with_capacity(64),
            globals,
            hashmaps,
            ledger_store,
            memory_store,
            try_stack: Vec::new(),
            tool_registry,
            connection_manager,
            mcp_registry,
            agent_registry,
            emit_handler: Box::new(|channel, payload| {
                println!("[emit:{}] {}", channel, payload);
            }),
            mock_models: HashMap::new(),
            test_emits: Vec::new(),
            test_capture_emits: false,
        }
    }

    /// Set a custom emit handler.
    pub fn set_emit_handler(&mut self, handler: impl Fn(&str, &Value) + 'static) {
        self.emit_handler = Box::new(handler);
    }

    /// Get the name of the currently executing function (for error reporting).
    pub fn current_function_name(&self) -> &str {
        self.call_stack
            .last()
            .map(|f| f.function_name.as_str())
            .unwrap_or("<none>")
    }

    /// Get the current call stack depth (useful for debug output).
    pub fn call_stack_depth(&self) -> usize {
        self.call_stack.len()
    }

    /// Execute the module starting from the entry point.
    pub fn execute(&mut self) -> Result<Value> {
        let entry = self.module.entry_point.clone();
        let func = self
            .module
            .functions
            .get(&entry)
            .ok_or_else(|| RuntimeError::NameError(entry.clone()))?
            .clone();

        self.push_frame(
            func.name.clone(),
            func.instructions.clone(),
            vec![],
            &func.params,
        )?;
        self.run_loop()
    }

    /// Execute a single test in the current VM instance.
    ///
    /// Clears mock/emit state, enables emit capture, pushes a test frame,
    /// and runs the test instructions.
    pub fn run_test(&mut self, test: &concerto_common::ir::IrTest) -> Result<Value> {
        // Clear per-test state
        self.mock_models.clear();
        self.test_emits.clear();
        self.test_capture_emits = true;

        // Push a call frame for the test
        self.push_frame(
            format!("test:{}", test.description),
            test.instructions.clone(),
            vec![],
            &[],
        )?;

        self.run_loop()
    }

    // ========================================================================
    // Stack operations
    // ========================================================================

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    fn pop(&mut self) -> Result<Value> {
        self.stack.pop().ok_or(RuntimeError::StackUnderflow)
    }

    fn peek(&self) -> Result<&Value> {
        self.stack.last().ok_or(RuntimeError::StackUnderflow)
    }

    // ========================================================================
    // Call frame management
    // ========================================================================

    fn push_frame(
        &mut self,
        function_name: String,
        instructions: Vec<IrInstruction>,
        args: Vec<Value>,
        params: &[concerto_common::ir::IrParam],
    ) -> Result<()> {
        if self.call_stack.len() >= MAX_CALL_DEPTH {
            return Err(RuntimeError::StackOverflow(MAX_CALL_DEPTH));
        }

        let mut locals = HashMap::new();
        for (param, arg) in params.iter().zip(args) {
            locals.insert(param.name.clone(), arg);
        }

        self.call_stack.push(CallFrame {
            function_name,
            instructions,
            pc: 0,
            locals,
        });
        Ok(())
    }

    /// Resolve a thunk by calling the named function synchronously.
    fn resolve_thunk(&mut self, function: &str, args: Vec<Value>) -> Result<Value> {
        if let Some(func) = self.module.functions.get(function).cloned() {
            let stop_depth = self.call_stack.len();
            self.push_frame(
                func.name.clone(),
                func.instructions.clone(),
                args,
                &func.params,
            )?;
            self.run_loop_until(stop_depth)
        } else {
            Err(RuntimeError::CallError(format!(
                "thunk references unknown function: {}",
                function
            )))
        }
    }

    // ========================================================================
    // Main execution loop
    // ========================================================================

    fn run_loop(&mut self) -> Result<Value> {
        self.run_loop_until(0)
    }

    /// Execute instructions until the call stack depth returns to `stop_depth`.
    /// When `stop_depth` is 0, runs until the call stack is empty (top-level).
    fn run_loop_until(&mut self, stop_depth: usize) -> Result<Value> {
        loop {
            // Check if call stack is empty or returned to caller's depth
            if self.call_stack.is_empty() {
                return Ok(self.stack.pop().unwrap_or(Value::Nil));
            }
            if self.call_stack.len() <= stop_depth {
                return Ok(self.stack.pop().unwrap_or(Value::Nil));
            }

            // Check if we've run past the end of instructions (implicit return nil)
            let at_end = {
                let frame = self.call_stack.last().ok_or_else(|| {
                    RuntimeError::CallError("internal: call stack unexpectedly empty".into())
                })?;
                frame.pc >= frame.instructions.len()
            };
            if at_end {
                self.call_stack.pop();
                if self.call_stack.is_empty() || self.call_stack.len() <= stop_depth {
                    return Ok(Value::Nil);
                }
                self.push(Value::Nil);
                continue;
            }

            // Fetch and advance
            let inst = {
                let frame = self.call_stack.last_mut().ok_or_else(|| {
                    RuntimeError::CallError("internal: call stack unexpectedly empty".into())
                })?;
                let inst = frame.instructions[frame.pc].clone();
                frame.pc += 1;
                inst
            };

            // Dispatch
            match inst.op {
                // === Stack ops ===
                Opcode::LoadConst => self.exec_load_const(&inst)?,
                Opcode::LoadLocal => self.exec_load_local(&inst)?,
                Opcode::StoreLocal => self.exec_store_local(&inst)?,
                Opcode::LoadGlobal => self.exec_load_global(&inst)?,
                Opcode::StoreGlobal => self.exec_store_global(&inst)?,
                Opcode::Pop => {
                    self.pop()?;
                }
                Opcode::Dup => {
                    let val = self.peek()?.clone();
                    self.push(val);
                }
                Opcode::Swap => {
                    let a = self.pop()?;
                    let b = self.pop()?;
                    self.push(a);
                    self.push(b);
                }
                Opcode::Push => {
                    // PUSH with immediate value in arg
                    let val = inst
                        .arg
                        .as_ref()
                        .map(json_to_value)
                        .unwrap_or(Ok(Value::Nil))?;
                    self.push(val);
                }

                // === Arithmetic ===
                Opcode::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.add(&b)?);
                }
                Opcode::Sub => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.sub(&b)?);
                }
                Opcode::Mul => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.mul(&b)?);
                }
                Opcode::Div => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.div(&b)?);
                }
                Opcode::Mod => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.modulo(&b)?);
                }
                Opcode::Neg => {
                    let a = self.pop()?;
                    self.push(a.neg()?);
                }

                // === Comparison ===
                Opcode::Eq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.eq_val(&b));
                }
                Opcode::Neq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.neq_val(&b));
                }
                Opcode::Lt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.lt(&b)?);
                }
                Opcode::Gt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.gt(&b)?);
                }
                Opcode::Lte => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.lte(&b)?);
                }
                Opcode::Gte => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.gte(&b)?);
                }

                // === Logical ===
                Opcode::And => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.and(&b)?);
                }
                Opcode::Or => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.or(&b)?);
                }
                Opcode::Not => {
                    let a = self.pop()?;
                    self.push(a.not()?);
                }

                // === Control flow ===
                Opcode::Jump => {
                    let target = inst
                        .offset
                        .ok_or_else(|| RuntimeError::LoadError("JUMP missing offset".into()))?
                        as usize;
                    self.call_stack
                        .last_mut()
                        .ok_or_else(|| {
                            RuntimeError::CallError("internal: jump with empty call stack".into())
                        })?
                        .pc = target;
                }
                Opcode::JumpIfTrue => {
                    let target = inst.offset.ok_or_else(|| {
                        RuntimeError::LoadError("JUMP_IF_TRUE missing offset".into())
                    })? as usize;
                    let cond = self.pop()?;
                    if cond.is_truthy() {
                        self.call_stack
                            .last_mut()
                            .ok_or_else(|| {
                                RuntimeError::CallError(
                                    "internal: jump with empty call stack".into(),
                                )
                            })?
                            .pc = target;
                    }
                }
                Opcode::JumpIfFalse => {
                    let target = inst.offset.ok_or_else(|| {
                        RuntimeError::LoadError("JUMP_IF_FALSE missing offset".into())
                    })? as usize;
                    let cond = self.pop()?;
                    if !cond.is_truthy() {
                        self.call_stack
                            .last_mut()
                            .ok_or_else(|| {
                                RuntimeError::CallError(
                                    "internal: jump with empty call stack".into(),
                                )
                            })?
                            .pc = target;
                    }
                }
                Opcode::Return => {
                    let return_val = self.pop()?;
                    self.call_stack.pop();
                    if self.call_stack.is_empty() || self.call_stack.len() <= stop_depth {
                        return Ok(return_val);
                    }
                    self.push(return_val);
                }

                // === Function calls ===
                Opcode::Call => self.exec_call(&inst)?,
                Opcode::CallMethod => self.exec_call_method(&inst)?,
                Opcode::CallNative => self.exec_call_native(&inst)?,

                // === Emit ===
                Opcode::Emit => self.exec_emit()?,
                Opcode::EmitAwait => {
                    // Phase 3a: treat as fire-and-forget emit, push Nil as response
                    self.exec_emit()?;
                    self.push(Value::Nil);
                }

                // === Error handling ===
                Opcode::Propagate => self.exec_propagate()?,
                Opcode::Throw => {
                    let val = self.pop()?;
                    self.exec_throw(val)?;
                }
                Opcode::TryBegin => {
                    let catch_pc = inst
                        .offset
                        .ok_or_else(|| RuntimeError::LoadError("TRY_BEGIN missing offset".into()))?
                        as usize;
                    self.try_stack.push(TryFrame {
                        catch_pc,
                        call_depth: self.call_stack.len(),
                        stack_height: self.stack.len(),
                    });
                }
                Opcode::TryEnd => {
                    self.try_stack.pop();
                }
                Opcode::Catch => {
                    // Catch instruction reached during normal flow means
                    // we need to check if the error type matches.
                    // The error value is on top of the stack (pushed by exec_throw).
                    let error_type = inst.type_name.as_deref();

                    if let Some(expected_type) = error_type {
                        // Typed catch: check if error matches
                        let error_val = self.peek()?;
                        let actual_type = error_val.type_name().to_string();
                        if actual_type != expected_type {
                            // Type doesn't match — skip this catch body.
                            // Scan forward to find the next CATCH or the JUMP
                            // that exits all catch blocks.
                            self.skip_catch_body()?;
                            continue;
                        }
                    }
                    // Type matches or catch-all: let execution continue
                    // into the StoreLocal/Pop + catch body instructions.
                }

                // === Object/Array/Map ===
                Opcode::FieldGet => self.exec_field_get(&inst)?,
                Opcode::FieldSet => self.exec_field_set(&inst)?,
                Opcode::IndexGet => self.exec_index_get()?,
                Opcode::IndexSet => self.exec_index_set()?,
                Opcode::BuildArray => self.exec_build_array(&inst)?,
                Opcode::BuildMap => self.exec_build_map(&inst)?,
                Opcode::BuildStruct => self.exec_build_struct(&inst)?,

                // === Type operations ===
                Opcode::CheckType => self.exec_check_type(&inst)?,
                Opcode::Cast => self.exec_cast(&inst)?,

                // === Async ===
                Opcode::Await => {
                    let val = self.pop()?;
                    match val {
                        Value::Thunk { function, args } => {
                            // Resolve thunk by calling the function synchronously
                            let result = self.resolve_thunk(&function, args)?;
                            self.push(result);
                        }
                        other => {
                            // Not a thunk — value is already resolved
                            self.push(other);
                        }
                    }
                }
                Opcode::AwaitAll => {
                    let count = inst.count.unwrap_or(0) as usize;
                    let mut values = Vec::with_capacity(count);
                    for _ in 0..count {
                        values.push(self.pop()?);
                    }
                    values.reverse();

                    // Resolve any thunks sequentially
                    let mut results = Vec::with_capacity(values.len());
                    for val in values {
                        match val {
                            Value::Thunk { function, args } => {
                                results.push(self.resolve_thunk(&function, args)?);
                            }
                            other => {
                                results.push(other);
                            }
                        }
                    }
                    self.push(Value::Array(results));
                }
                Opcode::ListenBegin => self.exec_listen_begin(&inst)?,

                Opcode::MockModel => {
                    // Install mock response for a model during test execution
                    let model_name = inst
                        .name
                        .as_ref()
                        .ok_or_else(|| {
                            RuntimeError::LoadError("MOCK_AGENT missing name".into())
                        })?
                        .clone();
                    let config = inst.arg.as_ref().and_then(|v| v.as_object());
                    let response = config
                        .and_then(|c| c.get("response"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let error = config
                        .and_then(|c| c.get("error"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    self.mock_models
                        .insert(model_name, MockConfig { response, error });
                }

                Opcode::SpawnAsync => {
                    // Create a deferred computation (Thunk).
                    // The callee (function ref) is on the stack top.
                    let callee = self.pop()?;
                    match callee {
                        Value::Function(name) => {
                            self.push(Value::Thunk {
                                function: name,
                                args: vec![],
                            });
                        }
                        other => {
                            // Not a function — just pass through
                            self.push(other);
                        }
                    }
                }

                // === Model operations (dispatched through CALL_METHOD) ===
                Opcode::CallModel
                | Opcode::CallModelSchema
                | Opcode::CallModelStream
                | Opcode::CallModelChat => {
                    // These should be dispatched through CALL_METHOD in our codegen,
                    // but handle them here as a fallback
                    let argc = inst.argc.unwrap_or(0) as usize;
                    let mut args = Vec::with_capacity(argc);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    let model_name = inst.model.as_deref().unwrap_or("unknown");
                    let method = inst.method.as_deref().unwrap_or("execute");
                    let result =
                        self.call_model_method(model_name, method, args, inst.schema.as_deref())?;
                    self.push(result);
                }

                // === Tool operations ===
                Opcode::CallTool => self.exec_call_tool(&inst)?,

                // === HashMap operations ===
                Opcode::HashMapGet => self.exec_hashmap_get(&inst)?,
                Opcode::HashMapSet => self.exec_hashmap_set(&inst)?,
                Opcode::HashMapDelete => self.exec_hashmap_delete(&inst)?,
                Opcode::HashMapHas => self.exec_hashmap_has(&inst)?,
                Opcode::HashMapQuery => {
                    let predicate = self.pop()?;
                    let hashmap_name = inst.hashmap_name.as_ref().ok_or_else(|| {
                        RuntimeError::LoadError("HASH_MAP_QUERY missing hashmap_name".into())
                    })?;
                    let entries: Vec<(String, Value)> = self
                        .hashmaps
                        .get(hashmap_name)
                        .map(|hm| hm.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                        .unwrap_or_default();

                    let mut results = Vec::new();
                    match &predicate {
                        Value::Function(fn_name) => {
                            for (key, value) in entries {
                                if let Some(func) = self.module.functions.get(fn_name).cloned() {
                                    let args = vec![Value::String(key.clone()), value.clone()];
                                    let stop_depth = self.call_stack.len();
                                    self.push_frame(
                                        func.name.clone(),
                                        func.instructions.clone(),
                                        args,
                                        &func.params,
                                    )?;
                                    let result = self.run_loop_until(stop_depth)?;
                                    if result.is_truthy() {
                                        results.push((key, value));
                                    }
                                }
                            }
                        }
                        _ => {
                            return Err(RuntimeError::TypeError(
                                "hashmap.query() predicate must be a function".into(),
                            ));
                        }
                    }
                    self.push(Value::Map(results));
                }

                // === Nil Coalesce ===
                Opcode::NilCoalescePrep => {
                    let val = self.pop()?;
                    match val {
                        Value::Option(None) => self.push(Value::Nil),
                        Value::Option(Some(inner)) => self.push(*inner),
                        other => self.push(other),
                    }
                }

                // === Range ===
                Opcode::BuildRange => {
                    let inclusive = self.pop()?;
                    let end = self.pop()?;
                    let start = self.pop()?;
                    let start_i = match &start {
                        Value::Int(i) => *i,
                        Value::Nil => 0,
                        _ => {
                            return Err(RuntimeError::TypeError(format!(
                                "range start must be Int, got {}",
                                start.type_name()
                            )));
                        }
                    };
                    let end_i = match &end {
                        Value::Int(i) => *i,
                        Value::Nil => i64::MAX,
                        _ => {
                            return Err(RuntimeError::TypeError(format!(
                                "range end must be Int, got {}",
                                end.type_name()
                            )));
                        }
                    };
                    let inclusive_b = match &inclusive {
                        Value::Bool(b) => *b,
                        _ => false,
                    };
                    self.push(Value::Range {
                        start: start_i,
                        end: end_i,
                        inclusive: inclusive_b,
                    });
                }
            }
        }
    }

    // ========================================================================
    // Instruction implementations
    // ========================================================================

    fn exec_load_const(&mut self, inst: &IrInstruction) -> Result<()> {
        let idx = inst
            .arg
            .as_ref()
            .and_then(|v| v.as_u64())
            .ok_or_else(|| RuntimeError::LoadError("LOAD_CONST missing arg".into()))?
            as usize;
        let value = self
            .module
            .constants
            .get(idx)
            .ok_or_else(|| {
                RuntimeError::LoadError(format!("constant index {} out of bounds", idx))
            })?
            .clone();
        self.push(value);
        Ok(())
    }

    fn exec_load_local(&mut self, inst: &IrInstruction) -> Result<()> {
        let name = inst
            .name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("LOAD_LOCAL missing name".into()))?;

        // Check local variables in current frame
        if let Some(frame) = self.call_stack.last() {
            if let Some(value) = frame.locals.get(name) {
                self.push(value.clone());
                return Ok(());
            }
        }

        // Fall back to globals (models, schemas, builtins, etc.)
        if let Some(value) = self.globals.get(name) {
            self.push(value.clone());
            return Ok(());
        }

        // Check if it's a user-defined function name
        if self.module.functions.contains_key(name) {
            self.push(Value::Function(name.clone()));
            return Ok(());
        }

        // Check if it's a path-based name like "std::time::now"
        // These resolve to global functions
        if name.contains("::") {
            self.push(Value::Function(name.clone()));
            return Ok(());
        }

        Err(RuntimeError::NameError(name.clone()))
    }

    fn exec_store_local(&mut self, inst: &IrInstruction) -> Result<()> {
        let name = inst
            .name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("STORE_LOCAL missing name".into()))?;
        let value = self.pop()?;
        let frame = self
            .call_stack
            .last_mut()
            .ok_or_else(|| RuntimeError::CallError("no call frame".into()))?;
        frame.locals.insert(name.clone(), value);
        Ok(())
    }

    fn exec_load_global(&mut self, inst: &IrInstruction) -> Result<()> {
        let name = inst
            .name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("LOAD_GLOBAL missing name".into()))?;

        if let Some(value) = self.globals.get(name) {
            self.push(value.clone());
            return Ok(());
        }

        // Path-based globals resolve to function references
        self.push(Value::Function(name.clone()));
        Ok(())
    }

    fn exec_store_global(&mut self, inst: &IrInstruction) -> Result<()> {
        let name = inst
            .name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("STORE_GLOBAL missing name".into()))?;
        let value = self.pop()?;
        self.globals.insert(name.clone(), value);
        Ok(())
    }

    fn exec_call(&mut self, inst: &IrInstruction) -> Result<()> {
        let argc = inst.argc.unwrap_or(0) as usize;

        // Pop callee (it's on top of stack, above the args)
        let callee = self.pop()?;

        // Pop args (pushed before callee, so pop in reverse)
        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        match callee {
            Value::Function(name) => {
                if name == "$builtin_test_emits" {
                    // test_emits() needs VM state — handle directly
                    let emits: Vec<Value> = self
                        .test_emits
                        .iter()
                        .map(|(ch, payload)| {
                            let mut fields = HashMap::new();
                            fields.insert("channel".to_string(), Value::String(ch.clone()));
                            fields.insert("payload".to_string(), payload.clone());
                            Value::Struct {
                                type_name: "Emit".to_string(),
                                fields,
                            }
                        })
                        .collect();
                    self.push(Value::Array(emits));
                } else if name.starts_with("$builtin_") {
                    let result = builtins::call_builtin(&name, args)?;
                    self.push(result);
                } else if let Some(func) = self.module.functions.get(&name).cloned() {
                    self.push_frame(
                        func.name.clone(),
                        func.instructions.clone(),
                        args,
                        &func.params,
                    )?;
                    // Execution continues in run_loop reading from new frame
                } else if name.starts_with("std::") {
                    let result = crate::stdlib::call_stdlib(&name, args)?;
                    self.push(result);
                } else {
                    return Err(RuntimeError::NameError(name));
                }
            }
            // If someone calls a ModelRef directly, treat as execute
            Value::ModelRef(model_name) => {
                let result = self.call_model_method(&model_name, "execute", args, None)?;
                self.push(result);
            }
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "cannot call value of type {}",
                    callee.type_name()
                )));
            }
        }
        Ok(())
    }

    fn exec_call_method(&mut self, inst: &IrInstruction) -> Result<()> {
        let method = inst
            .name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("CALL_METHOD missing name".into()))?
            .clone();
        let argc = inst.argc.unwrap_or(0) as usize;

        // Pop args (on top of stack)
        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        // Pop object (below args)
        let object = self.pop()?;

        let schema = inst.schema.clone();
        let result = match &object {
            Value::ModelRef(model_name) => match method.as_str() {
                "with_memory" | "with_tools" | "without_tools" => {
                    self.model_ref_to_builder(model_name, &method, args)?
                }
                _ => self.call_model_method(model_name, &method, args, schema.as_deref())?,
            },
            Value::HashMapRef(hashmap_name) => {
                self.call_hashmap_method(hashmap_name, &method, args)?
            }
            Value::LedgerRef(ledger_name) => self.call_ledger_method(ledger_name, &method, args)?,
            Value::MemoryRef(memory_name) => self.call_memory_method(memory_name, &method, args)?,
            Value::AgentRef(agent_name) => match method.as_str() {
                "with_memory" | "with_tools" | "without_tools" | "with_context" => {
                    self.agent_ref_to_builder(agent_name, &method, args)?
                }
                "execute" => self.call_agent_execute(agent_name, args, None)?,
                "execute_with_schema" => {
                    self.call_agent_execute(agent_name, args, schema.as_deref())?
                }
                _ => {
                    return Err(RuntimeError::TypeError(format!(
                        "no method '{}' on Agent",
                        method
                    )))
                }
            },
            Value::ModelBuilder { .. } => {
                self.call_model_builder_method(object, &method, args, schema.as_deref())?
            }
            Value::PipelineRef(pipeline_name) => {
                self.call_pipeline_method(pipeline_name, &method, args)?
            }
            Value::Result { is_ok, value } => Self::call_result_method(*is_ok, value, &method)?,
            Value::Option(opt) => Self::call_option_method(opt, &method)?,
            Value::String(s) => Self::call_string_method(s, &method, args)?,
            Value::Array(arr) => Self::call_array_method(arr, &method, args)?,
            Value::Range { start, end, inclusive } => {
                Self::call_range_method(*start, *end, *inclusive, &method)?
            }
            Value::Struct { ref type_name, .. }
                if type_name == "Set" || type_name == "Queue" || type_name == "Stack" =>
            {
                crate::stdlib::collections::call_collection_method(object, &method, args)?
            }
            _ => {
                // Try to find a qualified function (Type::method)
                let type_name = object.type_name().to_string();
                let qualified = format!("{}::{}", type_name, method);
                if let Some(func) = self.module.functions.get(&qualified).cloned() {
                    // Prepend `self` to args
                    let mut full_args = vec![object];
                    full_args.extend(args);
                    self.push_frame(
                        func.name.clone(),
                        func.instructions.clone(),
                        full_args,
                        &func.params,
                    )?;
                    return Ok(());
                }
                return Err(RuntimeError::TypeError(format!(
                    "no method '{}' on {}",
                    method, type_name
                )));
            }
        };
        self.push(result);
        Ok(())
    }

    fn exec_call_native(&mut self, inst: &IrInstruction) -> Result<()> {
        // CALL_NATIVE uses 'name' for the function name and 'argc' for arg count
        let name = inst
            .name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("CALL_NATIVE missing name".into()))?;
        let argc = inst.argc.unwrap_or(0) as usize;

        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        // Map to builtin
        let builtin_name = format!("$builtin_{}", name);
        let result = builtins::call_builtin(&builtin_name, args).unwrap_or(Value::Nil);
        self.push(result);
        Ok(())
    }

    fn exec_emit(&mut self) -> Result<()> {
        let payload = self.pop()?;
        let channel = self.pop()?;

        let channel_str = match &channel {
            Value::String(s) => s.clone(),
            _ => channel.display_string(),
        };

        // Capture emits during test execution
        if self.test_capture_emits {
            self.test_emits
                .push((channel_str.clone(), payload.clone()));
        }

        (self.emit_handler)(&channel_str, &payload);
        Ok(())
    }

    fn exec_propagate(&mut self) -> Result<()> {
        let value = self.pop()?;
        match value {
            Value::Result { is_ok: true, value } => {
                // Unwrap Ok
                self.push(*value);
                Ok(())
            }
            Value::Result {
                is_ok: false,
                value,
            } => {
                // Propagate Err — try to catch it via try/catch first
                self.exec_throw(*value)
            }
            Value::Option(Some(inner)) => {
                // Unwrap Some
                self.push(*inner);
                Ok(())
            }
            Value::Option(None) => {
                // Early return with None — pop current frame, push None for caller
                self.call_stack.pop();
                self.push(Value::Option(None));
                Ok(())
            }
            other => {
                // Not a Result or Option — runtime error
                Err(RuntimeError::TypeError(format!(
                    "? operator requires Result or Option, got {}",
                    other.type_name()
                )))
            }
        }
    }

    /// Handle a throw: unwind to the nearest try/catch handler, or return
    /// an unhandled error if none exists.
    fn exec_throw(&mut self, error_val: Value) -> Result<()> {
        if let Some(try_frame) = self.try_stack.pop() {
            // Unwind call stack to the frame that owns the try block
            while self.call_stack.len() > try_frame.call_depth {
                self.call_stack.pop();
            }
            // Restore stack height (discard any values pushed during try body)
            self.stack.truncate(try_frame.stack_height);
            // Push the error value for the catch block to consume
            self.push(error_val);
            // Jump to the catch handler
            if let Some(frame) = self.call_stack.last_mut() {
                frame.pc = try_frame.catch_pc;
            }
            Ok(())
        } else {
            Err(RuntimeError::UnhandledThrow(error_val.display_string()))
        }
    }

    /// Skip past the current catch body to find the next CATCH or exit JUMP.
    /// Called when a typed catch doesn't match the error type.
    fn skip_catch_body(&mut self) -> Result<()> {
        let frame = self
            .call_stack
            .last_mut()
            .ok_or_else(|| RuntimeError::CallError("no call frame".into()))?;

        // Scan forward looking for the next CATCH or JUMP instruction.
        // The codegen emits: CATCH -> StoreLocal/Pop -> body -> [next CATCH | end]
        // We need to skip to the next CATCH at the same nesting level, or
        // to the JUMP that exits all catch blocks.
        let mut depth = 0;
        loop {
            if frame.pc >= frame.instructions.len() {
                break;
            }
            let next_op = frame.instructions[frame.pc].op;
            match next_op {
                Opcode::TryBegin => depth += 1,
                Opcode::Catch if depth == 0 => {
                    // Found the next catch block at the same level — stop here.
                    // The main loop will process this CATCH instruction.
                    break;
                }
                Opcode::TryEnd if depth > 0 => depth -= 1,
                Opcode::Jump if depth == 0 => {
                    // This is the exit jump past all catch blocks.
                    // Execute it (the main loop will handle it on next iteration).
                    break;
                }
                _ => {}
            }
            frame.pc += 1;
        }
        Ok(())
    }

    fn exec_field_get(&mut self, inst: &IrInstruction) -> Result<()> {
        let name = inst
            .name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("FIELD_GET missing name".into()))?;
        let object = self.pop()?;
        let value = object.field_get(name)?;
        self.push(value);
        Ok(())
    }

    fn exec_field_set(&mut self, inst: &IrInstruction) -> Result<()> {
        let name = inst
            .name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("FIELD_SET missing name".into()))?;
        let value = self.pop()?;
        let mut object = self.pop()?;
        match &mut object {
            Value::Struct { fields, .. } => {
                fields.insert(name.clone(), value);
            }
            Value::Map(pairs) => {
                if let Some(pair) = pairs.iter_mut().find(|(k, _)| k == name) {
                    pair.1 = value;
                } else {
                    pairs.push((name.clone(), value));
                }
            }
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "cannot set field '{}' on {}",
                    name,
                    object.type_name()
                )));
            }
        }
        self.push(object);
        Ok(())
    }

    fn exec_index_get(&mut self) -> Result<()> {
        let index = self.pop()?;
        let object = self.pop()?;
        let value = object.index_get(&index)?;
        self.push(value);
        Ok(())
    }

    fn exec_index_set(&mut self) -> Result<()> {
        let value = self.pop()?;
        let index = self.pop()?;
        let mut collection = self.pop()?;
        match (&mut collection, &index) {
            (Value::Array(arr), Value::Int(i)) => {
                let idx = *i as usize;
                if idx < arr.len() {
                    arr[idx] = value;
                } else {
                    return Err(RuntimeError::IndexError {
                        index: *i,
                        len: arr.len(),
                    });
                }
            }
            (Value::Map(pairs), Value::String(key)) => {
                if let Some(pair) = pairs.iter_mut().find(|(k, _)| k == key) {
                    pair.1 = value;
                } else {
                    pairs.push((key.clone(), value));
                }
            }
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "cannot index-set on {} with {}",
                    collection.type_name(),
                    index.type_name()
                )));
            }
        }
        self.push(collection);
        Ok(())
    }

    fn exec_check_type(&mut self, inst: &IrInstruction) -> Result<()> {
        let val = self.pop()?;
        let target_type = inst.type_name.as_deref().unwrap_or("");
        let matches = match target_type {
            "Int" => matches!(val, Value::Int(_)),
            "Float" => matches!(val, Value::Float(_)),
            "String" => matches!(val, Value::String(_)),
            "Bool" => matches!(val, Value::Bool(_)),
            "Nil" => matches!(val, Value::Nil),
            "Array" => matches!(val, Value::Array(_)),
            "Map" => matches!(val, Value::Map(_)),
            name => val.type_name() == name,
        };
        self.push(Value::Bool(matches));
        Ok(())
    }

    fn exec_cast(&mut self, inst: &IrInstruction) -> Result<()> {
        let val = self.pop()?;
        let target = inst.type_name.as_deref().unwrap_or("");
        let result = match target {
            "Int" => match &val {
                Value::Int(_) => Ok(val),
                Value::Float(f) => Ok(Value::Int(*f as i64)),
                Value::String(s) => s.parse::<i64>().map(Value::Int).map_err(|_| {
                    format!("cannot cast String \"{}\" to Int", s)
                }),
                Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
                _ => Err(format!("cannot cast {} to Int", val.type_name())),
            },
            "Float" => match &val {
                Value::Float(_) => Ok(val),
                Value::Int(i) => Ok(Value::Float(*i as f64)),
                Value::String(s) => s.parse::<f64>().map(Value::Float).map_err(|_| {
                    format!("cannot cast String \"{}\" to Float", s)
                }),
                _ => Err(format!("cannot cast {} to Float", val.type_name())),
            },
            "String" => Ok(Value::String(val.display_string())),
            "Bool" => Ok(Value::Bool(val.is_truthy())),
            _ => Err(format!("unsupported cast target type '{}'", target)),
        };
        match result {
            Ok(v) => {
                self.push(v);
                Ok(())
            }
            Err(msg) => self.exec_throw(Value::String(msg)),
        }
    }

    fn exec_build_array(&mut self, inst: &IrInstruction) -> Result<()> {
        let count = inst.count.unwrap_or(0) as usize;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.pop()?);
        }
        values.reverse();
        self.push(Value::Array(values));
        Ok(())
    }

    fn exec_build_map(&mut self, inst: &IrInstruction) -> Result<()> {
        let count = inst.count.unwrap_or(0) as usize;
        let mut pairs = Vec::with_capacity(count);
        for _ in 0..count {
            let value = self.pop()?;
            let key = self.pop()?;
            let key_str = match key {
                Value::String(s) => s,
                _ => key.display_string(),
            };
            pairs.push((key_str, value));
        }
        pairs.reverse();
        self.push(Value::Map(pairs));
        Ok(())
    }

    fn exec_build_struct(&mut self, inst: &IrInstruction) -> Result<()> {
        let count = inst.count.unwrap_or(0) as usize;
        let type_name = inst
            .type_name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("BUILD_STRUCT missing type_name".into()))?
            .clone();

        // Field names from arg (JSON array of strings)
        let field_names: Vec<String> = inst
            .arg
            .as_ref()
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.pop()?);
        }
        values.reverse();

        let mut fields = HashMap::new();
        for (name, value) in field_names.into_iter().zip(values) {
            fields.insert(name, value);
        }

        self.push(Value::Struct { type_name, fields });
        Ok(())
    }

    // ========================================================================
    // HashMap operations
    // ========================================================================

    fn exec_hashmap_get(&mut self, inst: &IrInstruction) -> Result<()> {
        let hashmap_name = inst
            .hashmap_name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("HASH_MAP_GET missing hashmap_name".into()))?;
        let key = self.pop()?;
        let key_str = key.display_string();

        let value = self
            .hashmaps
            .get(hashmap_name)
            .and_then(|hm| hm.get(&key_str))
            .cloned();

        match value {
            Some(v) => self.push(Value::Option(Some(Box::new(v)))),
            None => self.push(Value::Option(None)),
        }
        Ok(())
    }

    fn exec_hashmap_set(&mut self, inst: &IrInstruction) -> Result<()> {
        let hashmap_name = inst
            .hashmap_name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("HASH_MAP_SET missing hashmap_name".into()))?;
        let value = self.pop()?;
        let key = self.pop()?;
        let key_str = key.display_string();

        self.hashmaps
            .entry(hashmap_name.clone())
            .or_default()
            .insert(key_str, value);
        self.push(Value::Nil);
        Ok(())
    }

    fn exec_hashmap_delete(&mut self, inst: &IrInstruction) -> Result<()> {
        let hashmap_name = inst.hashmap_name.as_ref().ok_or_else(|| {
            RuntimeError::LoadError("HASH_MAP_DELETE missing hashmap_name".into())
        })?;
        let key = self.pop()?;
        let key_str = key.display_string();

        if let Some(hm) = self.hashmaps.get_mut(hashmap_name) {
            hm.remove(&key_str);
        }
        self.push(Value::Nil);
        Ok(())
    }

    fn exec_hashmap_has(&mut self, inst: &IrInstruction) -> Result<()> {
        let hashmap_name = inst
            .hashmap_name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("HASH_MAP_HAS missing hashmap_name".into()))?;
        let key = self.pop()?;
        let key_str = key.display_string();

        let exists = self
            .hashmaps
            .get(hashmap_name)
            .map(|hm| hm.contains_key(&key_str))
            .unwrap_or(false);

        self.push(Value::Bool(exists));
        Ok(())
    }

    // ========================================================================
    // Model method dispatch (provider-based)
    // ========================================================================

    fn call_model_method(
        &self,
        model_name: &str,
        method: &str,
        args: Vec<Value>,
        schema_name: Option<&str>,
    ) -> Result<Value> {
        // Check for mock override first
        if let Some(mock) = self.mock_models.get(model_name) {
            return self.call_mock_model(model_name, method, mock.clone(), schema_name);
        }

        let model_def = self
            .module
            .models
            .get(model_name)
            .ok_or_else(|| RuntimeError::NameError(format!("unknown model: {}", model_name)))?;

        // Parse decorator configs from model definition
        let retry_config = crate::decorator::find_decorator(&model_def.decorators, "retry")
            .map(crate::decorator::parse_retry);
        let timeout_config = crate::decorator::find_decorator(&model_def.decorators, "timeout")
            .map(crate::decorator::parse_timeout);
        let has_log = crate::decorator::find_decorator(&model_def.decorators, "log").is_some();

        let max_attempts = retry_config.as_ref().map(|r| r.max_attempts).unwrap_or(1);

        match method {
            "execute" => {
                let prompt = args.into_iter().next().unwrap_or(Value::Nil);
                let prompt_str = prompt.display_string();
                let mut last_error = String::new();

                for attempt in 0..max_attempts {
                    let start = std::time::Instant::now();
                    let request = self.build_chat_request(model_def, &prompt_str, None);
                    let provider = self.connection_manager.get_provider(&model_def.connection);

                    match provider.chat_completion(request) {
                        Ok(chat_response) => {
                            // Check timeout
                            if let Some(ref tc) = timeout_config {
                                let elapsed = start.elapsed();
                                if elapsed > std::time::Duration::from_secs(tc.seconds) {
                                    last_error = format!(
                                        "timeout exceeded ({}s > {}s)",
                                        elapsed.as_secs(),
                                        tc.seconds
                                    );
                                    if attempt + 1 < max_attempts {
                                        if let Some(ref rc) = retry_config {
                                            std::thread::sleep(crate::decorator::backoff_delay(
                                                &rc.backoff,
                                                attempt,
                                            ));
                                        }
                                        continue;
                                    }
                                    return Ok(Value::Result {
                                        is_ok: false,
                                        value: Box::new(Value::String(last_error)),
                                    });
                                }
                            }

                            // @log decorator
                            if has_log {
                                (self.emit_handler)(
                                    "model:log",
                                    &Value::Map(vec![
                                        (
                                            "model".to_string(),
                                            Value::String(model_name.to_string()),
                                        ),
                                        (
                                            "method".to_string(),
                                            Value::String("execute".to_string()),
                                        ),
                                        ("attempt".to_string(), Value::Int((attempt + 1) as i64)),
                                        (
                                            "tokens_in".to_string(),
                                            Value::Int(chat_response.tokens_in),
                                        ),
                                        (
                                            "tokens_out".to_string(),
                                            Value::Int(chat_response.tokens_out),
                                        ),
                                    ]),
                                );
                            }

                            let response = Self::chat_response_to_value(&chat_response);
                            return Ok(Value::Result {
                                is_ok: true,
                                value: Box::new(response),
                            });
                        }
                        Err(e) => {
                            last_error = e.to_string();
                            if attempt + 1 < max_attempts {
                                if let Some(ref rc) = retry_config {
                                    std::thread::sleep(crate::decorator::backoff_delay(
                                        &rc.backoff,
                                        attempt,
                                    ));
                                }
                            }
                        }
                    }
                }

                // All retries exhausted
                Ok(Value::Result {
                    is_ok: false,
                    value: Box::new(Value::String(format!(
                        "model '{}' failed after {} attempts: {}",
                        model_name, max_attempts, last_error
                    ))),
                })
            }
            "execute_with_schema" => {
                let prompt = args.into_iter().next().unwrap_or(Value::Nil);
                let prompt_str = prompt.display_string();

                // Look up schema for structured output
                let schema = schema_name.and_then(|n| self.module.schemas.get(n));

                if let Some(schema) = schema {
                    let response_format = Some(crate::provider::ResponseFormat {
                        format_type: "json_schema".to_string(),
                        json_schema: Some(schema.json_schema.clone()),
                    });

                    let mut last_error = String::new();

                    // Outer retry loop: @retry decorator (catches provider errors)
                    for attempt in 0..max_attempts {
                        let start = std::time::Instant::now();
                        let provider = self.connection_manager.get_provider(&model_def.connection);
                        let mut current_prompt = prompt_str.clone();

                        // Inner retry loop: schema validation retry
                        let mut schema_result = None;
                        for schema_attempt in 0..SchemaValidator::max_retries() {
                            let rf = if schema_attempt == 0 {
                                response_format.clone()
                            } else {
                                current_prompt =
                                    SchemaValidator::retry_prompt(&prompt_str, &last_error, schema);
                                response_format.clone()
                            };

                            let request = self.build_chat_request(model_def, &current_prompt, rf);
                            match provider.chat_completion(request) {
                                Ok(chat_response) => {
                                    // Check timeout
                                    if let Some(ref tc) = timeout_config {
                                        let elapsed = start.elapsed();
                                        if elapsed > std::time::Duration::from_secs(tc.seconds) {
                                            last_error = format!(
                                                "timeout exceeded ({}s > {}s)",
                                                elapsed.as_secs(),
                                                tc.seconds
                                            );
                                            break;
                                        }
                                    }

                                    match SchemaValidator::validate(&chat_response.text, schema) {
                                        Ok(validated) => {
                                            if has_log {
                                                (self.emit_handler)(
                                                    "model:log",
                                                    &Value::Map(vec![
                                                        (
                                                            "model".to_string(),
                                                            Value::String(model_name.to_string()),
                                                        ),
                                                        (
                                                            "method".to_string(),
                                                            Value::String(
                                                                "execute_with_schema".to_string(),
                                                            ),
                                                        ),
                                                        (
                                                            "attempt".to_string(),
                                                            Value::Int((attempt + 1) as i64),
                                                        ),
                                                        (
                                                            "schema_attempt".to_string(),
                                                            Value::Int((schema_attempt + 1) as i64),
                                                        ),
                                                        (
                                                            "tokens_in".to_string(),
                                                            Value::Int(chat_response.tokens_in),
                                                        ),
                                                        (
                                                            "tokens_out".to_string(),
                                                            Value::Int(chat_response.tokens_out),
                                                        ),
                                                    ]),
                                                );
                                            }
                                            schema_result = Some(validated);
                                            break;
                                        }
                                        Err(e) => {
                                            last_error = e.to_string();
                                        }
                                    }
                                }
                                Err(e) => {
                                    last_error = e.to_string();
                                    break; // Provider error — exit inner loop, let outer retry handle it
                                }
                            }
                        }

                        if let Some(validated) = schema_result {
                            return Ok(Value::Result {
                                is_ok: true,
                                value: Box::new(validated),
                            });
                        }

                        // Inner loop didn't succeed — try outer retry
                        if attempt + 1 < max_attempts {
                            if let Some(ref rc) = retry_config {
                                std::thread::sleep(crate::decorator::backoff_delay(
                                    &rc.backoff,
                                    attempt,
                                ));
                            }
                        }
                    }

                    // All retries exhausted
                    Ok(Value::Result {
                        is_ok: false,
                        value: Box::new(Value::String(format!(
                            "schema validation failed after {} attempts: {}",
                            max_attempts * SchemaValidator::max_retries() as u32,
                            last_error
                        ))),
                    })
                } else {
                    // No schema found — just do a regular execute
                    let request = self.build_chat_request(model_def, &prompt_str, None);
                    let provider = self.connection_manager.get_provider(&model_def.connection);
                    let chat_response = provider.chat_completion(request)?;
                    let response = Self::chat_response_to_value(&chat_response);
                    Ok(Value::Result {
                        is_ok: true,
                        value: Box::new(response),
                    })
                }
            }
            _ => Err(RuntimeError::CallError(format!(
                "unknown model method: {}.{}",
                model_name, method
            ))),
        }
    }

    /// Handle a mocked model call, returning a fixed response or error.
    fn call_mock_model(
        &self,
        model_name: &str,
        method: &str,
        mock: MockConfig,
        schema_name: Option<&str>,
    ) -> Result<Value> {
        match method {
            "execute" | "execute_with_schema" => {
                // Check for error mock
                if let Some(err_msg) = &mock.error {
                    return Ok(Value::Result {
                        is_ok: false,
                        value: Box::new(Value::String(err_msg.clone())),
                    });
                }

                let response_text = mock.response.clone().unwrap_or_default();

                // For execute_with_schema, parse the response as JSON and convert to struct
                if method == "execute_with_schema" {
                    if let Some(schema) = schema_name {
                        if let Some(ir_schema) = self.module.schemas.get(schema) {
                            match SchemaValidator::validate(&response_text, ir_schema) {
                                Ok(validated) => {
                                    return Ok(Value::Result {
                                        is_ok: true,
                                        value: Box::new(validated),
                                    });
                                }
                                Err(e) => {
                                    return Ok(Value::Result {
                                        is_ok: false,
                                        value: Box::new(Value::String(format!(
                                            "mock schema validation failed: {}",
                                            e
                                        ))),
                                    });
                                }
                            }
                        }
                    }
                }

                // Build a Response struct matching the real model response shape
                let mut fields = HashMap::new();
                fields.insert("text".to_string(), Value::String(response_text));
                fields.insert("model".to_string(), Value::String("mock".to_string()));
                fields.insert("tokens_in".to_string(), Value::Int(0));
                fields.insert("tokens_out".to_string(), Value::Int(0));
                Ok(Value::Result {
                    is_ok: true,
                    value: Box::new(Value::Struct {
                        type_name: "Response".to_string(),
                        fields,
                    }),
                })
            }
            _ => Err(RuntimeError::TypeError(format!(
                "mock model '{}' does not support method '{}'",
                model_name, method
            ))),
        }
    }

    /// Build a ChatRequest from model config and prompt.
    fn build_chat_request(
        &self,
        model_def: &concerto_common::ir::IrModel,
        prompt: &str,
        response_format: Option<crate::provider::ResponseFormat>,
    ) -> ChatRequest {
        self.build_chat_request_with_memory(model_def, prompt, response_format, None)
    }

    fn build_chat_request_with_memory(
        &self,
        model_def: &concerto_common::ir::IrModel,
        prompt: &str,
        response_format: Option<crate::provider::ResponseFormat>,
        memory_name: Option<&str>,
    ) -> ChatRequest {
        self.build_chat_request_full(model_def, prompt, response_format, memory_name, &[], false)
    }

    fn build_chat_request_full(
        &self,
        model_def: &concerto_common::ir::IrModel,
        prompt: &str,
        response_format: Option<crate::provider::ResponseFormat>,
        memory_name: Option<&str>,
        extra_tools: &[String],
        exclude_default_tools: bool,
    ) -> ChatRequest {
        let mut messages = Vec::new();

        // System prompt from model config
        if let Some(ref sys) = model_def.config.system_prompt {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: sys.clone(),
                tool_call_id: None,
            });
        }

        // Inject memory messages (between system prompt and user prompt)
        if let Some(mem_name) = memory_name {
            if let Ok(mem_msgs) = self.memory_store.messages(mem_name) {
                messages.extend(mem_msgs);
            }
        }

        // User prompt
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
            tool_call_id: None,
        });

        // Collect tool schemas
        let mut tool_schemas = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        // Static tools from model definition (unless excluded)
        if !exclude_default_tools {
            for tool_ref in &model_def.tools {
                if self.mcp_registry.has_server(tool_ref) {
                    for schema in self.mcp_registry.get_tool_schemas(tool_ref) {
                        if seen_names.insert(schema.name.clone()) {
                            tool_schemas.push(schema);
                        }
                    }
                }
            }
        }

        // Dynamic tools from extra_tools
        for tool_name in extra_tools {
            // Check if it's a Concerto tool with schemas
            if let Some(ir_tool) = self.module.tools.get(tool_name) {
                for entry in &ir_tool.tool_schemas {
                    if seen_names.insert(entry.method_name.clone()) {
                        tool_schemas.push(crate::provider::ToolSchema {
                            name: entry.method_name.clone(),
                            description: entry.description.clone(),
                            parameters: entry.parameters.clone(),
                        });
                    }
                }
            }
            // Check if it's an MCP server
            else if self.mcp_registry.has_server(tool_name) {
                for schema in self.mcp_registry.get_tool_schemas(tool_name) {
                    if seen_names.insert(schema.name.clone()) {
                        tool_schemas.push(schema);
                    }
                }
            }
        }

        let tools = if tool_schemas.is_empty() {
            None
        } else {
            Some(tool_schemas)
        };

        ChatRequest {
            model: model_def
                .config
                .base
                .clone()
                .unwrap_or_else(|| "gpt-4".to_string()),
            messages,
            temperature: model_def.config.temperature,
            max_tokens: model_def.config.max_tokens,
            tools,
            response_format,
        }
    }

    /// Convert a ChatResponse into a Value::Struct { type_name: "Response", ... }.
    fn chat_response_to_value(response: &crate::provider::ChatResponse) -> Value {
        let mut fields = HashMap::new();
        fields.insert("text".to_string(), Value::String(response.text.clone()));
        fields.insert("tokens_in".to_string(), Value::Int(response.tokens_in));
        fields.insert("tokens_out".to_string(), Value::Int(response.tokens_out));
        fields.insert("model".to_string(), Value::String(response.model.clone()));
        Value::Struct {
            type_name: "Response".to_string(),
            fields,
        }
    }

    // ========================================================================
    // Tool method dispatch
    // ========================================================================

    fn exec_call_tool(&mut self, inst: &IrInstruction) -> Result<()> {
        let tool_name = inst
            .tool
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("CALL_TOOL missing tool name".into()))?;
        let method_name = inst
            .name
            .as_ref()
            .ok_or_else(|| RuntimeError::LoadError("CALL_TOOL missing method name".into()))?;
        let argc = inst.argc.unwrap_or(0) as usize;

        // Pop arguments
        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        // Look up the qualified function "Tool::method"
        let qualified = format!("{}::{}", tool_name, method_name);
        if let Some(func) = self.module.functions.get(&qualified).cloned() {
            // Get tool self value
            let self_val = self.tool_registry.get_self_value(tool_name);

            // Prepend self to args
            let mut full_args = vec![self_val];
            full_args.extend(args);

            // Push frame and execute
            self.push_frame(
                func.name.clone(),
                func.instructions.clone(),
                full_args,
                &func.params,
            )?;
        } else {
            return Err(RuntimeError::CallError(format!(
                "unknown tool method: {}::{}",
                tool_name, method_name
            )));
        }
        Ok(())
    }

    // ========================================================================
    // HashMap method dispatch (for CALL_METHOD on HashMapRef)
    // ========================================================================

    fn call_hashmap_method(
        &mut self,
        hashmap_name: &str,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        match method {
            "set" => {
                if args.len() >= 2 {
                    let key = args[0].display_string();
                    let value = args[1].clone();
                    self.hashmaps
                        .entry(hashmap_name.to_string())
                        .or_default()
                        .insert(key, value);
                }
                Ok(Value::Nil)
            }
            "get" => {
                let key = args
                    .into_iter()
                    .next()
                    .map(|v| v.display_string())
                    .unwrap_or_default();
                let value = self
                    .hashmaps
                    .get(hashmap_name)
                    .and_then(|hm| hm.get(&key))
                    .cloned();
                match value {
                    Some(v) => Ok(Value::Option(Some(Box::new(v)))),
                    None => Ok(Value::Option(None)),
                }
            }
            "has" => {
                let key = args
                    .into_iter()
                    .next()
                    .map(|v| v.display_string())
                    .unwrap_or_default();
                let exists = self
                    .hashmaps
                    .get(hashmap_name)
                    .map(|hm| hm.contains_key(&key))
                    .unwrap_or(false);
                Ok(Value::Bool(exists))
            }
            "delete" => {
                let key = args
                    .into_iter()
                    .next()
                    .map(|v| v.display_string())
                    .unwrap_or_default();
                if let Some(hm) = self.hashmaps.get_mut(hashmap_name) {
                    hm.remove(&key);
                }
                Ok(Value::Nil)
            }
            _ => Err(RuntimeError::CallError(format!(
                "unknown hashmap method: {}.{}",
                hashmap_name, method
            ))),
        }
    }

    // ========================================================================
    // Ledger method dispatch (for CALL_METHOD on LedgerRef)
    // ========================================================================

    fn call_ledger_method(
        &mut self,
        ledger_name: &str,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        match method {
            "insert" => {
                // insert(identifier: String, keys: Array<String>, value: String) -> Nil
                if args.len() >= 3 {
                    let identifier = args[0].display_string();
                    let keys = match &args[1] {
                        Value::Array(arr) => arr.iter().map(|v| v.display_string()).collect(),
                        _ => vec![args[1].display_string()],
                    };
                    let value = args[2].display_string();
                    self.ledger_store
                        .insert(ledger_name, identifier, keys, value);
                }
                Ok(Value::Nil)
            }
            "delete" => {
                // delete(identifier: String) -> Bool
                let identifier = args
                    .into_iter()
                    .next()
                    .map(|v| v.display_string())
                    .unwrap_or_default();
                Ok(Value::Bool(
                    self.ledger_store.delete(ledger_name, &identifier),
                ))
            }
            "update" => {
                // update(identifier: String, value: String) -> Bool
                if args.len() >= 2 {
                    let identifier = args[0].display_string();
                    let value = args[1].display_string();
                    Ok(Value::Bool(self.ledger_store.update(
                        ledger_name,
                        &identifier,
                        value,
                    )))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            "update_keys" => {
                // update_keys(identifier: String, keys: Array<String>) -> Bool
                if args.len() >= 2 {
                    let identifier = args[0].display_string();
                    let keys = match &args[1] {
                        Value::Array(arr) => arr.iter().map(|v| v.display_string()).collect(),
                        _ => vec![args[1].display_string()],
                    };
                    Ok(Value::Bool(self.ledger_store.update_keys(
                        ledger_name,
                        &identifier,
                        keys,
                    )))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            "query" => {
                // query() -> LedgerRef (identity, for method chaining)
                Ok(Value::LedgerRef(ledger_name.to_string()))
            }
            "from_identifier" => {
                // from_identifier(text: String) -> Array<LedgerEntry>
                let text = args
                    .into_iter()
                    .next()
                    .map(|v| v.display_string())
                    .unwrap_or_default();
                let entries: Vec<Value> = self
                    .ledger_store
                    .query_from_identifier(ledger_name, &text)
                    .into_iter()
                    .map(|e| e.to_value())
                    .collect();
                Ok(Value::Array(entries))
            }
            "from_key" => {
                // from_key(key: String) -> Array<LedgerEntry>
                let key = args
                    .into_iter()
                    .next()
                    .map(|v| v.display_string())
                    .unwrap_or_default();
                let entries: Vec<Value> = self
                    .ledger_store
                    .query_from_key(ledger_name, &key)
                    .into_iter()
                    .map(|e| e.to_value())
                    .collect();
                Ok(Value::Array(entries))
            }
            "from_any_keys" => {
                // from_any_keys(keys: Array<String>) -> Array<LedgerEntry>
                let keys: Vec<String> = match args.into_iter().next() {
                    Some(Value::Array(arr)) => arr.iter().map(|v| v.display_string()).collect(),
                    Some(v) => vec![v.display_string()],
                    None => vec![],
                };
                let entries: Vec<Value> = self
                    .ledger_store
                    .query_from_any_keys(ledger_name, &keys)
                    .into_iter()
                    .map(|e| e.to_value())
                    .collect();
                Ok(Value::Array(entries))
            }
            "from_exact_keys" => {
                // from_exact_keys(keys: Array<String>) -> Array<LedgerEntry>
                let keys: Vec<String> = match args.into_iter().next() {
                    Some(Value::Array(arr)) => arr.iter().map(|v| v.display_string()).collect(),
                    Some(v) => vec![v.display_string()],
                    None => vec![],
                };
                let entries: Vec<Value> = self
                    .ledger_store
                    .query_from_exact_keys(ledger_name, &keys)
                    .into_iter()
                    .map(|e| e.to_value())
                    .collect();
                Ok(Value::Array(entries))
            }
            "len" => Ok(Value::Int(self.ledger_store.len(ledger_name) as i64)),
            "is_empty" => Ok(Value::Bool(self.ledger_store.is_empty(ledger_name))),
            "clear" => {
                self.ledger_store.clear(ledger_name);
                Ok(Value::Nil)
            }
            "entries" => {
                let entries: Vec<Value> = self
                    .ledger_store
                    .entries(ledger_name)
                    .iter()
                    .map(|e| e.to_value())
                    .collect();
                Ok(Value::Array(entries))
            }
            "identifiers" => {
                let ids: Vec<Value> = self
                    .ledger_store
                    .identifiers(ledger_name)
                    .into_iter()
                    .map(|id| Value::String(id.to_string()))
                    .collect();
                Ok(Value::Array(ids))
            }
            "scope" => {
                // scope(prefix: String) -> LedgerRef("name::prefix")
                let prefix = args
                    .into_iter()
                    .next()
                    .map(|v| v.display_string())
                    .unwrap_or_default();
                let scoped_name = format!("{}::{}", ledger_name, prefix);
                // Initialize the scoped ledger if it doesn't exist
                self.ledger_store.init_ledger(&scoped_name);
                Ok(Value::LedgerRef(scoped_name))
            }
            _ => Err(RuntimeError::CallError(format!(
                "unknown ledger method: {}.{}",
                ledger_name, method
            ))),
        }
    }

    // ========================================================================
    // Pipeline method dispatch
    // ========================================================================

    fn call_pipeline_method(
        &mut self,
        pipeline_name: &str,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        match method {
            "run" => {
                let pipeline = self
                    .module
                    .pipelines
                    .get(pipeline_name)
                    .ok_or_else(|| {
                        RuntimeError::NameError(format!("unknown pipeline: {}", pipeline_name))
                    })?
                    .clone();

                let mut input = args.into_iter().next().unwrap_or(Value::Nil);
                let pipeline_start = std::time::Instant::now();

                // Emit pipeline:start
                (self.emit_handler)(
                    "pipeline:start",
                    &Value::Map(vec![
                        ("name".to_string(), Value::String(pipeline_name.to_string())),
                        (
                            "stages".to_string(),
                            Value::Int(pipeline.stages.len() as i64),
                        ),
                    ]),
                );

                for stage in &pipeline.stages {
                    let stage_start = std::time::Instant::now();

                    // Emit pipeline:stage_start
                    (self.emit_handler)(
                        "pipeline:stage_start",
                        &Value::Map(vec![
                            (
                                "pipeline".to_string(),
                                Value::String(pipeline_name.to_string()),
                            ),
                            ("stage".to_string(), Value::String(stage.name.clone())),
                        ]),
                    );

                    // Parse stage decorators
                    let retry_config = crate::decorator::find_decorator(&stage.decorators, "retry")
                        .map(crate::decorator::parse_retry);
                    let timeout_config =
                        crate::decorator::find_decorator(&stage.decorators, "timeout")
                            .map(crate::decorator::parse_timeout);

                    let max_attempts = retry_config.as_ref().map(|r| r.max_attempts).unwrap_or(1);
                    let mut last_error = String::new();
                    let mut stage_result = None;

                    for attempt in 0..max_attempts {
                        let stop_depth = self.call_stack.len();
                        self.push_frame(
                            format!("{}::{}", pipeline_name, stage.name),
                            stage.instructions.clone(),
                            vec![input.clone()],
                            &stage.params,
                        )?;

                        match self.run_loop_until(stop_depth) {
                            Ok(val) => {
                                // Check timeout
                                if let Some(ref tc) = timeout_config {
                                    let elapsed = stage_start.elapsed();
                                    if elapsed > std::time::Duration::from_secs(tc.seconds) {
                                        last_error = format!(
                                            "stage '{}' timed out ({}ms > {}s)",
                                            stage.name,
                                            elapsed.as_millis(),
                                            tc.seconds
                                        );
                                        if attempt + 1 < max_attempts {
                                            if let Some(ref rc) = retry_config {
                                                std::thread::sleep(
                                                    crate::decorator::backoff_delay(
                                                        &rc.backoff,
                                                        attempt,
                                                    ),
                                                );
                                            }
                                            continue;
                                        }
                                        break;
                                    }
                                }

                                // Unwrap Result if stage returned one
                                let output = match val {
                                    Value::Result { is_ok: true, value } => *value,
                                    Value::Result {
                                        is_ok: false,
                                        value,
                                    } => {
                                        last_error = value.display_string();
                                        if attempt + 1 < max_attempts {
                                            if let Some(ref rc) = retry_config {
                                                std::thread::sleep(
                                                    crate::decorator::backoff_delay(
                                                        &rc.backoff,
                                                        attempt,
                                                    ),
                                                );
                                            }
                                            continue;
                                        }
                                        break;
                                    }
                                    other => other,
                                };

                                stage_result = Some(output);
                                break;
                            }
                            Err(e) => {
                                last_error = e.to_string();
                                if attempt + 1 < max_attempts {
                                    if let Some(ref rc) = retry_config {
                                        std::thread::sleep(crate::decorator::backoff_delay(
                                            &rc.backoff,
                                            attempt,
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    match stage_result {
                        Some(output) => {
                            let stage_duration = stage_start.elapsed().as_millis() as i64;
                            // Emit pipeline:stage_complete
                            (self.emit_handler)(
                                "pipeline:stage_complete",
                                &Value::Map(vec![
                                    (
                                        "pipeline".to_string(),
                                        Value::String(pipeline_name.to_string()),
                                    ),
                                    ("stage".to_string(), Value::String(stage.name.clone())),
                                    ("duration_ms".to_string(), Value::Int(stage_duration)),
                                ]),
                            );
                            input = output;
                        }
                        None => {
                            // Emit pipeline:error
                            (self.emit_handler)(
                                "pipeline:error",
                                &Value::Map(vec![
                                    (
                                        "pipeline".to_string(),
                                        Value::String(pipeline_name.to_string()),
                                    ),
                                    ("stage".to_string(), Value::String(stage.name.clone())),
                                    ("error".to_string(), Value::String(last_error.clone())),
                                ]),
                            );
                            return Ok(Value::Result {
                                is_ok: false,
                                value: Box::new(Value::String(format!(
                                    "pipeline '{}' failed at stage '{}': {}",
                                    pipeline_name, stage.name, last_error
                                ))),
                            });
                        }
                    }
                }

                let total_duration = pipeline_start.elapsed().as_millis() as i64;
                // Emit pipeline:complete
                (self.emit_handler)(
                    "pipeline:complete",
                    &Value::Map(vec![
                        ("name".to_string(), Value::String(pipeline_name.to_string())),
                        ("duration_ms".to_string(), Value::Int(total_duration)),
                    ]),
                );

                Ok(Value::Result {
                    is_ok: true,
                    value: Box::new(input),
                })
            }
            _ => Err(RuntimeError::CallError(format!(
                "unknown pipeline method: {}.{}",
                pipeline_name, method
            ))),
        }
    }

    // ========================================================================
    // Memory method dispatch
    // ========================================================================

    fn call_memory_method(
        &mut self,
        memory_name: &str,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        match method {
            "append" => {
                let role = args.first().map(|v| v.display_string()).unwrap_or_default();
                let content = args.get(1).map(|v| v.display_string()).unwrap_or_default();
                self.memory_store.append(memory_name, &role, &content)?;
                Ok(Value::Nil)
            }
            "messages" => self.memory_store.messages_to_value(memory_name),
            "last" => {
                let count = match args.first() {
                    Some(Value::Int(n)) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "memory.last() requires an Int argument".into(),
                        ))
                    }
                };
                self.memory_store.last_to_value(memory_name, count)
            }
            "len" => {
                let len = self.memory_store.len(memory_name)?;
                Ok(Value::Int(len as i64))
            }
            "clear" => {
                self.memory_store.clear(memory_name)?;
                Ok(Value::Nil)
            }
            _ => Err(RuntimeError::TypeError(format!(
                "no method '{}' on MemoryRef",
                method
            ))),
        }
    }

    // ========================================================================
    // Agent method dispatch
    // ========================================================================

    /// Create a ModelBuilder from an AgentRef for chaining.
    fn agent_ref_to_builder(
        &self,
        agent_name: &str,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        let mut builder = Value::ModelBuilder {
            source_name: agent_name.to_string(),
            source_kind: crate::value::BuilderSourceKind::Agent,
            memory: None,
            memory_auto_append: true,
            extra_tools: Vec::new(),
            exclude_default_tools: false,
            context: None,
        };
        self.apply_builder_method(&mut builder, method, args)?;
        Ok(builder)
    }

    /// Execute a prompt on an agent directly (without builder).
    fn call_agent_execute(
        &mut self,
        agent_name: &str,
        args: Vec<Value>,
        schema_name: Option<&str>,
    ) -> Result<Value> {
        let prompt = args.into_iter().next().unwrap_or(Value::Nil);
        let prompt_str = prompt.display_string();

        let result_text = self.agent_registry.execute(agent_name, &prompt_str, None)?;

        // If schema validation requested
        if let Some(sname) = schema_name {
            if let Some(schema) = self.module.schemas.get(sname) {
                let schema = schema.clone();
                match SchemaValidator::validate(&result_text, &schema) {
                    Ok(validated) => Ok(Value::Result {
                        is_ok: true,
                        value: Box::new(validated),
                    }),
                    Err(_) => Ok(Value::Result {
                        is_ok: false,
                        value: Box::new(Value::String(result_text)),
                    }),
                }
            } else {
                Ok(Value::Result {
                    is_ok: false,
                    value: Box::new(Value::String(format!("unknown schema: {}", sname))),
                })
            }
        } else {
            // Wrap in Result
            Ok(Value::Result {
                is_ok: true,
                value: Box::new(Value::String(result_text)),
            })
        }
    }

    // ========================================================================
    // Listen (bidirectional agent streaming)
    // ========================================================================

    /// Execute a ListenBegin opcode: send prompt to agent, then enter message loop.
    fn exec_listen_begin(&mut self, inst: &IrInstruction) -> Result<()> {
        let agent_name = inst.name.as_deref().ok_or_else(|| {
            RuntimeError::CallError("ListenBegin missing agent name".into())
        })?;
        let listen_name = inst
            .arg
            .as_ref()
            .and_then(|a| a.as_str())
            .ok_or_else(|| {
                RuntimeError::CallError("ListenBegin missing listen definition reference".into())
            })?;
        let argc = inst.argc.unwrap_or(0) as usize;

        // Pop args (prompt, etc.) from stack
        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        let prompt = args
            .into_iter()
            .next()
            .unwrap_or(Value::Nil)
            .display_string();

        // Look up listen definition
        let listen = self
            .module
            .listens
            .get(listen_name)
            .ok_or_else(|| {
                RuntimeError::CallError(format!(
                    "Listen definition '{}' not found",
                    listen_name
                ))
            })?
            .clone();

        // Send prompt to agent
        self.agent_registry
            .get_client_mut(agent_name)?
            .write_prompt_streaming(&prompt, None)?;

        // Emit lifecycle event
        (self.emit_handler)(
            "listen:start",
            &Value::Map(vec![
                ("agent".to_string(), Value::String(agent_name.to_string())),
                ("listen".to_string(), Value::String(listen_name.to_string())),
            ]),
        );

        // Run the listen loop
        let result = self.run_listen_loop(agent_name, &listen);

        // Emit completion event
        match &result {
            Ok(_) => {
                (self.emit_handler)(
                    "listen:complete",
                    &Value::Map(vec![
                        ("agent".to_string(), Value::String(agent_name.to_string())),
                    ]),
                );
            }
            Err(e) => {
                (self.emit_handler)(
                    "listen:error",
                    &Value::Map(vec![
                        ("agent".to_string(), Value::String(agent_name.to_string())),
                        ("error".to_string(), Value::String(e.to_string())),
                    ]),
                );
            }
        }

        let value = result?;
        // Wrap in Result
        self.push(Value::Result {
            is_ok: true,
            value: Box::new(value),
        });
        Ok(())
    }

    /// Run the listen loop: read NDJSON messages from agent, dispatch to handlers.
    fn run_listen_loop(
        &mut self,
        agent_name: &str,
        listen: &concerto_common::ir::IrListen,
    ) -> Result<Value> {
        use crate::schema::SchemaValidator;

        loop {
            // Read next message from agent
            let msg = self
                .agent_registry
                .get_client_mut(agent_name)?
                .read_message()?;

            let msg = match msg {
                Some(m) => m,
                None => {
                    // Agent exited without sending result/error
                    return Err(RuntimeError::CallError(format!(
                        "Agent '{}' exited without sending a result",
                        agent_name
                    )));
                }
            };

            // Extract message type
            let msg_type = msg
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("result")
                .to_string();

            match msg_type.as_str() {
                "result" => {
                    // Terminal: convert remaining fields to Value and return
                    let mut result_obj = msg.clone();
                    if let Some(obj) = result_obj.as_object_mut() {
                        obj.remove("type");
                    }
                    return Ok(SchemaValidator::json_to_value(&result_obj));
                }
                "error" => {
                    // Terminal: return as error
                    let error_msg = msg
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown agent error")
                        .to_string();
                    return Err(RuntimeError::CallError(format!(
                        "Agent '{}' error: {}",
                        agent_name, error_msg
                    )));
                }
                _ => {
                    // Find matching handler
                    let handler = listen.handlers.iter().find(|h| h.message_type == msg_type);

                    if let Some(handler) = handler {
                        // Build handler parameter value:
                        // Remove "type" field, convert rest to Value
                        let mut param_obj = msg.clone();
                        if let Some(obj) = param_obj.as_object_mut() {
                            obj.remove("type");
                        }
                        let param_value = SchemaValidator::json_to_value(&param_obj);

                        // Execute handler via push_frame + run_loop_until (pipeline stage pattern)
                        let stop_depth = self.call_stack.len();
                        let handler_name =
                            format!("$listen_handler_{}", handler.message_type);
                        self.push_frame(
                            handler_name,
                            handler.instructions.clone(),
                            vec![param_value],
                            std::slice::from_ref(&handler.param),
                        )?;
                        let handler_result = self.run_loop_until(stop_depth)?;

                        // If handler returned a non-Nil value, send response back to agent
                        if handler_result != Value::Nil {
                            let response_json = serde_json::json!({
                                "type": "response",
                                "in_reply_to": msg_type,
                                "value": handler_result.to_json()
                            });
                            self.agent_registry
                                .get_client_mut(agent_name)?
                                .write_response(&response_json)?;
                        }
                    } else {
                        // No handler — emit unhandled and continue
                        (self.emit_handler)(
                            "listen:unhandled",
                            &SchemaValidator::json_to_value(&msg),
                        );
                    }
                }
            }
        }
    }

    // ========================================================================
    // ModelBuilder creation and dispatch
    // ========================================================================

    /// Convert a ModelRef + builder method call into a Value::ModelBuilder.
    fn model_ref_to_builder(
        &self,
        model_name: &str,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        use crate::value::BuilderSourceKind;
        let mut builder = Value::ModelBuilder {
            source_name: model_name.to_string(),
            source_kind: BuilderSourceKind::Model,
            memory: None,
            memory_auto_append: true,
            extra_tools: Vec::new(),
            exclude_default_tools: false,
            context: None,
        };
        self.apply_builder_method(&mut builder, method, args)?;
        Ok(builder)
    }

    /// Apply a builder method to a ModelBuilder.
    fn apply_builder_method(
        &self,
        builder: &mut Value,
        method: &str,
        args: Vec<Value>,
    ) -> Result<()> {
        if let Value::ModelBuilder {
            ref mut memory,
            ref mut memory_auto_append,
            ref mut extra_tools,
            ref mut exclude_default_tools,
            ref mut context,
            ..
        } = builder
        {
            match method {
                "with_memory" => {
                    match args.first() {
                        Some(Value::MemoryRef(name)) => {
                            *memory = Some(name.clone());
                        }
                        _ => {
                            return Err(RuntimeError::TypeError(
                                "with_memory() requires a MemoryRef argument".into(),
                            ))
                        }
                    }
                    // Check for auto: false named arg (second arg as Bool)
                    if let Some(Value::Bool(false)) = args.get(1) {
                        *memory_auto_append = false;
                    }
                }
                "with_tools" => match args.first() {
                    Some(Value::Array(tools)) => {
                        for tool in tools {
                            match tool {
                                Value::String(name) => extra_tools.push(name.clone()),
                                Value::Function(name) => extra_tools.push(name.clone()),
                                other => extra_tools.push(other.display_string()),
                            }
                        }
                    }
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "with_tools() requires an Array argument".into(),
                        ))
                    }
                },
                "without_tools" => {
                    *exclude_default_tools = true;
                }
                "with_context" => {
                    if let Some(val) = args.into_iter().next() {
                        *context = Some(Box::new(val));
                    }
                }
                _ => {
                    return Err(RuntimeError::TypeError(format!(
                        "unknown builder method '{}'",
                        method
                    )))
                }
            }
        }
        Ok(())
    }

    /// Dispatch a method call on a ModelBuilder value.
    fn call_model_builder_method(
        &mut self,
        builder: Value,
        method: &str,
        args: Vec<Value>,
        schema_name: Option<&str>,
    ) -> Result<Value> {
        match method {
            "with_memory" | "with_tools" | "without_tools" | "with_context" => {
                let mut new_builder = builder;
                self.apply_builder_method(&mut new_builder, method, args)?;
                Ok(new_builder)
            }
            "execute" => self.execute_model_builder(builder, args, None),
            "execute_with_schema" => self.execute_model_builder(builder, args, schema_name),
            _ => Err(RuntimeError::TypeError(format!(
                "no method '{}' on ModelBuilder",
                method
            ))),
        }
    }

    /// Execute a model call using the accumulated builder configuration.
    fn execute_model_builder(
        &mut self,
        builder: Value,
        args: Vec<Value>,
        schema_name: Option<&str>,
    ) -> Result<Value> {
        if let Value::ModelBuilder {
            source_name,
            source_kind,
            memory,
            memory_auto_append,
            extra_tools,
            exclude_default_tools,
            context,
        } = &builder
        {
            let source_name = source_name.clone();
            let source_kind = source_kind.clone();
            let memory = memory.clone();
            let memory_auto_append = *memory_auto_append;
            let extra_tools = extra_tools.clone();
            let exclude_default_tools = *exclude_default_tools;
            let context = context.clone();

            let prompt = args.into_iter().next().unwrap_or(Value::Nil);
            let prompt_str = prompt.display_string();

            // Check for mock override on model builders
            if matches!(source_kind, crate::value::BuilderSourceKind::Model) {
                if let Some(mock) = self.mock_models.get(&source_name).cloned() {
                    let method = if schema_name.is_some() {
                        "execute_with_schema"
                    } else {
                        "execute"
                    };
                    return self.call_mock_model(&source_name, method, mock, schema_name);
                }
            }

            // Dispatch based on source kind
            let (result_text, response_meta) = match source_kind {
                crate::value::BuilderSourceKind::Model => {
                    let model_def = self
                        .module
                        .models
                        .get(&source_name)
                        .ok_or_else(|| {
                            RuntimeError::NameError(format!("unknown model: {}", source_name))
                        })?
                        .clone();

                    let response_format = schema_name.map(|schema| {
                        let json_schema = self
                            .module
                            .schemas
                            .get(schema)
                            .map(|s| s.json_schema.clone())
                            .unwrap_or(serde_json::json!({}));
                        crate::provider::ResponseFormat {
                            format_type: "json_schema".to_string(),
                            json_schema: Some(json_schema),
                        }
                    });

                    let request = self.build_chat_request_full(
                        &model_def,
                        &prompt_str,
                        response_format,
                        memory.as_deref(),
                        &extra_tools,
                        exclude_default_tools,
                    );

                    let provider = self.connection_manager.get_provider(&model_def.connection);
                    let chat_response = provider.chat_completion(request)?;
                    (
                        chat_response.text,
                        Some((
                            chat_response.model,
                            chat_response.tokens_in,
                            chat_response.tokens_out,
                        )),
                    )
                }
                crate::value::BuilderSourceKind::Agent => (
                    self.agent_registry
                        .execute(&source_name, &prompt_str, context.as_deref())?,
                    None,
                ),
            };

            // Auto-append to memory if enabled
            if memory_auto_append {
                if let Some(ref mem_name) = memory {
                    self.memory_store.append(mem_name, "user", &prompt_str)?;
                    self.memory_store
                        .append(mem_name, "assistant", &result_text)?;
                }
            }

            // If schema validation requested, validate the response
            if let Some(sname) = schema_name {
                if let Some(schema) = self.module.schemas.get(sname) {
                    let schema = schema.clone();
                    match SchemaValidator::validate(&result_text, &schema) {
                        Ok(validated) => Ok(Value::Result {
                            is_ok: true,
                            value: Box::new(validated),
                        }),
                        Err(_) => Ok(Value::Result {
                            is_ok: false,
                            value: Box::new(Value::String(result_text)),
                        }),
                    }
                } else {
                    Ok(Value::Result {
                        is_ok: false,
                        value: Box::new(Value::String(format!("unknown schema: {}", sname))),
                    })
                }
            } else if matches!(source_kind, crate::value::BuilderSourceKind::Agent) {
                // Agents return Result<String>
                Ok(Value::Result {
                    is_ok: true,
                    value: Box::new(Value::String(result_text)),
                })
            } else {
                // Models return Response struct
                let (model, tokens_in, tokens_out) =
                    response_meta.unwrap_or_else(|| (String::new(), 0, 0));
                let mut fields = std::collections::HashMap::new();
                fields.insert("text".to_string(), Value::String(result_text));
                fields.insert("model".to_string(), Value::String(model));
                fields.insert("tokens_in".to_string(), Value::Int(tokens_in));
                fields.insert("tokens_out".to_string(), Value::Int(tokens_out));
                Ok(Value::Result {
                    is_ok: true,
                    value: Box::new(Value::Struct {
                        type_name: "Response".to_string(),
                        fields,
                    }),
                })
            }
        } else {
            Err(RuntimeError::TypeError("expected ModelBuilder".into()))
        }
    }

    // ========================================================================
    // Type method dispatch
    // ========================================================================

    fn call_result_method(is_ok: bool, value: &Value, method: &str) -> Result<Value> {
        match method {
            "unwrap" => {
                if is_ok {
                    Ok(value.clone())
                } else {
                    Err(RuntimeError::UnhandledThrow(format!(
                        "called unwrap() on Err({})",
                        value
                    )))
                }
            }
            "is_ok" => Ok(Value::Bool(is_ok)),
            "is_err" => Ok(Value::Bool(!is_ok)),
            _ => Err(RuntimeError::TypeError(format!(
                "no method '{}' on Result",
                method
            ))),
        }
    }

    fn call_option_method(opt: &Option<Box<Value>>, method: &str) -> Result<Value> {
        match method {
            "unwrap" => match opt {
                Some(v) => Ok(*v.clone()),
                None => Err(RuntimeError::UnhandledThrow(
                    "called unwrap() on None".to_string(),
                )),
            },
            "is_some" => Ok(Value::Bool(opt.is_some())),
            "is_none" => Ok(Value::Bool(opt.is_none())),
            _ => Err(RuntimeError::TypeError(format!(
                "no method '{}' on Option",
                method
            ))),
        }
    }

    fn call_string_method(s: &str, method: &str, args: Vec<Value>) -> Result<Value> {
        match method {
            "len" => Ok(Value::Int(s.len() as i64)),
            "contains" => {
                let substr = args
                    .into_iter()
                    .next()
                    .map(|v| v.display_string())
                    .unwrap_or_default();
                Ok(Value::Bool(s.contains(&substr)))
            }
            "trim" => Ok(Value::String(s.trim().to_string())),
            "to_uppercase" => Ok(Value::String(s.to_uppercase())),
            "to_lowercase" => Ok(Value::String(s.to_lowercase())),
            _ => Err(RuntimeError::TypeError(format!(
                "no method '{}' on String",
                method
            ))),
        }
    }

    fn call_array_method(arr: &[Value], method: &str, _args: Vec<Value>) -> Result<Value> {
        match method {
            "len" => Ok(Value::Int(arr.len() as i64)),
            "is_empty" => Ok(Value::Bool(arr.is_empty())),
            _ => Err(RuntimeError::TypeError(format!(
                "no method '{}' on Array",
                method
            ))),
        }
    }

    fn call_range_method(start: i64, end: i64, inclusive: bool, method: &str) -> Result<Value> {
        match method {
            "len" => {
                let len = if inclusive { end - start + 1 } else { end - start };
                Ok(Value::Int(len.max(0)))
            }
            "is_empty" => {
                let len = if inclusive { end - start + 1 } else { end - start };
                Ok(Value::Bool(len <= 0))
            }
            _ => Err(RuntimeError::TypeError(format!(
                "no method '{}' on Range",
                method
            ))),
        }
    }
}

/// Convert a JSON value to a runtime Value (used for PUSH immediate values).
fn json_to_value(json: &serde_json::Value) -> Result<Value> {
    match json {
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err(RuntimeError::LoadError("invalid number in PUSH".into()))
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(s.clone())),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Null => Ok(Value::Nil),
        _ => Err(RuntimeError::LoadError(format!(
            "unsupported PUSH value: {}",
            json
        ))),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir_loader::LoadedModule;
    use concerto_common::ir::*;
    use std::sync::{Arc, Mutex};

    /// Helper: create a minimal IR module from instructions
    fn make_module(instructions: Vec<IrInstruction>) -> IrModule {
        IrModule {
            version: "0.1.0".to_string(),
            module: "test".to_string(),
            source_file: "test.conc".to_string(),
            constants: vec![],
            types: vec![],
            functions: vec![IrFunction {
                name: "main".to_string(),
                module: "test".to_string(),
                visibility: "private".to_string(),
                params: vec![],
                return_type: serde_json::json!("nil"),
                is_async: false,
                locals: vec![],
                instructions,
            }],
            models: vec![],
            tools: vec![],
            schemas: vec![],
            connections: vec![],
            hashmaps: vec![],
            ledgers: vec![],
            memories: vec![],
            agents: vec![],
            listens: vec![],
            pipelines: vec![],
            tests: vec![],
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

    fn inst(op: Opcode) -> IrInstruction {
        IrInstruction {
            op,
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

    fn inst_const(idx: u32) -> IrInstruction {
        IrInstruction {
            op: Opcode::LoadConst,
            arg: Some(serde_json::json!(idx)),
            ..inst(Opcode::LoadConst)
        }
    }

    fn inst_store(name: &str) -> IrInstruction {
        IrInstruction {
            op: Opcode::StoreLocal,
            name: Some(name.to_string()),
            ..inst(Opcode::StoreLocal)
        }
    }

    fn inst_load(name: &str) -> IrInstruction {
        IrInstruction {
            op: Opcode::LoadLocal,
            name: Some(name.to_string()),
            ..inst(Opcode::LoadLocal)
        }
    }

    #[test]
    fn simple_add_and_return() {
        let mut module = make_module(vec![
            inst_const(0),
            inst_const(1),
            inst(Opcode::Add),
            inst(Opcode::Return),
        ]);
        module.constants = vec![
            IrConstant {
                index: 0,
                const_type: "int".to_string(),
                value: serde_json::json!(5),
            },
            IrConstant {
                index: 1,
                const_type: "int".to_string(),
                value: serde_json::json!(3),
            },
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Int(8));
    }

    #[test]
    fn store_and_load_local() {
        let mut module = make_module(vec![
            inst_const(0),
            inst_store("x"),
            inst_load("x"),
            inst(Opcode::Return),
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "int".to_string(),
            value: serde_json::json!(42),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn conditional_jump() {
        // if true { return 1 } else { return 2 }
        let mut module = make_module(vec![
            inst_const(0), // 0: push true
            IrInstruction {
                op: Opcode::JumpIfFalse,
                offset: Some(4),
                ..inst(Opcode::JumpIfFalse)
            }, // 1: jump to 4 if false
            inst_const(1), // 2: push 1
            inst(Opcode::Return), // 3: return 1
            inst_const(2), // 4: push 2
            inst(Opcode::Return), // 5: return 2
        ]);
        module.constants = vec![
            IrConstant {
                index: 0,
                const_type: "bool".to_string(),
                value: serde_json::json!(true),
            },
            IrConstant {
                index: 1,
                const_type: "int".to_string(),
                value: serde_json::json!(1),
            },
            IrConstant {
                index: 2,
                const_type: "int".to_string(),
                value: serde_json::json!(2),
            },
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn emit_fires_handler() {
        let mut module = make_module(vec![
            inst_const(0), // channel "result"
            inst_const(1), // value 8
            inst(Opcode::Emit),
            inst_const(2), // nil
            inst(Opcode::Return),
        ]);
        module.constants = vec![
            IrConstant {
                index: 0,
                const_type: "string".to_string(),
                value: serde_json::json!("result"),
            },
            IrConstant {
                index: 1,
                const_type: "int".to_string(),
                value: serde_json::json!(8),
            },
            IrConstant {
                index: 2,
                const_type: "nil".to_string(),
                value: serde_json::Value::Null,
            },
        ];

        let emits: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let emits_clone = emits.clone();

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        vm.set_emit_handler(move |channel, payload| {
            emits_clone
                .lock()
                .unwrap()
                .push((channel.to_string(), payload.display_string()));
        });

        vm.execute().unwrap();

        let emits = emits.lock().unwrap();
        assert_eq!(emits.len(), 1);
        assert_eq!(emits[0].0, "result");
        assert_eq!(emits[0].1, "8");
    }

    #[test]
    fn function_call_and_return() {
        // main() calls add(3, 5) which returns 8
        let module = IrModule {
            version: "0.1.0".to_string(),
            module: "test".to_string(),
            source_file: "test.conc".to_string(),
            constants: vec![
                IrConstant {
                    index: 0,
                    const_type: "int".to_string(),
                    value: serde_json::json!(3),
                },
                IrConstant {
                    index: 1,
                    const_type: "int".to_string(),
                    value: serde_json::json!(5),
                },
            ],
            types: vec![],
            functions: vec![
                IrFunction {
                    name: "main".to_string(),
                    module: "test".to_string(),
                    visibility: "private".to_string(),
                    params: vec![],
                    return_type: serde_json::json!("nil"),
                    is_async: false,
                    locals: vec![],
                    instructions: vec![
                        inst_const(0),    // push 3
                        inst_const(1),    // push 5
                        inst_load("add"), // push fn ref
                        IrInstruction {
                            op: Opcode::Call,
                            argc: Some(2),
                            ..inst(Opcode::Call)
                        },
                        inst(Opcode::Return),
                    ],
                },
                IrFunction {
                    name: "add".to_string(),
                    module: "test".to_string(),
                    visibility: "private".to_string(),
                    params: vec![
                        IrParam {
                            name: "a".to_string(),
                            param_type: serde_json::json!("Int"),
                        },
                        IrParam {
                            name: "b".to_string(),
                            param_type: serde_json::json!("Int"),
                        },
                    ],
                    return_type: serde_json::json!("Int"),
                    is_async: false,
                    locals: vec!["a".to_string(), "b".to_string()],
                    instructions: vec![
                        inst_load("a"),
                        inst_load("b"),
                        inst(Opcode::Add),
                        inst(Opcode::Return),
                    ],
                },
            ],
            models: vec![],
            tools: vec![],
            schemas: vec![],
            connections: vec![],
            hashmaps: vec![],
            ledgers: vec![],
            memories: vec![],
            agents: vec![],
            listens: vec![],
            pipelines: vec![],
            tests: vec![],
            source_map: None,
            metadata: IrMetadata {
                compiler_version: "0.1.0".to_string(),
                compiled_at: String::new(),
                optimization_level: 0,
                debug_info: true,
                entry_point: "main".to_string(),
            },
        };

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Int(8));
    }

    #[test]
    fn builtin_ok_err() {
        let mut module = make_module(vec![
            inst_const(0),   // push 42
            inst_load("Ok"), // push Ok fn
            IrInstruction {
                op: Opcode::Call,
                argc: Some(1),
                ..inst(Opcode::Call)
            },
            inst(Opcode::Return),
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "int".to_string(),
            value: serde_json::json!(42),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(
            result,
            Value::Result {
                is_ok: true,
                value: Box::new(Value::Int(42))
            }
        );
    }

    #[test]
    fn propagate_unwraps_ok() {
        let mut module = make_module(vec![
            inst_const(0),   // push 42
            inst_load("Ok"), // push Ok fn
            IrInstruction {
                op: Opcode::Call,
                argc: Some(1),
                ..inst(Opcode::Call)
            },
            inst(Opcode::Propagate), // unwrap Ok -> 42
            inst(Opcode::Return),
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "int".to_string(),
            value: serde_json::json!(42),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn propagate_errors_on_err() {
        let mut module = make_module(vec![
            inst_const(0),    // push "oops"
            inst_load("Err"), // push Err fn
            IrInstruction {
                op: Opcode::Call,
                argc: Some(1),
                ..inst(Opcode::Call)
            },
            inst(Opcode::Propagate), // propagate Err
            inst(Opcode::Return),
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "string".to_string(),
            value: serde_json::json!("oops"),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute();
        assert!(result.is_err());
    }

    #[test]
    fn build_map() {
        let mut module = make_module(vec![
            inst_const(0), // "a"
            inst_const(1), // 1
            inst_const(2), // "b"
            inst_const(3), // 2
            IrInstruction {
                op: Opcode::BuildMap,
                count: Some(2),
                ..inst(Opcode::BuildMap)
            },
            inst(Opcode::Return),
        ]);
        module.constants = vec![
            IrConstant {
                index: 0,
                const_type: "string".to_string(),
                value: serde_json::json!("a"),
            },
            IrConstant {
                index: 1,
                const_type: "int".to_string(),
                value: serde_json::json!(1),
            },
            IrConstant {
                index: 2,
                const_type: "string".to_string(),
                value: serde_json::json!("b"),
            },
            IrConstant {
                index: 3,
                const_type: "int".to_string(),
                value: serde_json::json!(2),
            },
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        match result {
            Value::Map(pairs) => {
                assert_eq!(pairs.len(), 2);
                assert_eq!(pairs[0], ("a".to_string(), Value::Int(1)));
                assert_eq!(pairs[1], ("b".to_string(), Value::Int(2)));
            }
            _ => panic!("expected Map, got {:?}", result),
        }
    }

    #[test]
    fn dup_and_neq_for_nil_coalesce() {
        // Simulates: value ?? "default"
        // DUP, LOAD_CONST nil, NEQ, JUMP_IF_TRUE skip, POP, LOAD_CONST default
        let mut module = make_module(vec![
            inst_const(0),     // 0: push nil (the value)
            inst(Opcode::Dup), // 1: dup
            inst_const(0),     // 2: push nil
            inst(Opcode::Neq), // 3: neq -> false (nil != nil is false)
            IrInstruction {
                op: Opcode::JumpIfTrue,
                offset: Some(7), // 4: if true (value != nil), jump to 7 (return)
                ..inst(Opcode::JumpIfTrue)
            },
            inst(Opcode::Pop),    // 5: pop the original nil
            inst_const(1),        // 6: push "default"
            inst(Opcode::Return), // 7: return
        ]);
        module.constants = vec![
            IrConstant {
                index: 0,
                const_type: "nil".to_string(),
                value: serde_json::Value::Null,
            },
            IrConstant {
                index: 1,
                const_type: "string".to_string(),
                value: serde_json::json!("default"),
            },
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::String("default".to_string()));
    }

    // ========================================================================
    // Try/Catch tests
    // ========================================================================

    #[test]
    fn try_catch_no_error() {
        // try { 42 } catch { 0 }
        // Try body succeeds → TRY_END pops handler → JUMP past catch → return 42
        let mut module = make_module(vec![
            // 0: TRY_BEGIN offset=4 (catch starts at instruction 4)
            IrInstruction {
                op: Opcode::TryBegin,
                offset: Some(4),
                ..inst(Opcode::TryBegin)
            },
            inst_const(0),        // 1: push 42
            inst(Opcode::TryEnd), // 2: pop try frame
            IrInstruction {
                // 3: JUMP past catch to 8
                op: Opcode::Jump,
                offset: Some(8),
                ..inst(Opcode::Jump)
            },
            // 4: CATCH (catch-all)
            IrInstruction {
                op: Opcode::Catch,
                ..inst(Opcode::Catch)
            },
            inst(Opcode::Pop), // 5: discard error
            inst_const(1),     // 6: push 0
            // (fall through to 7, which doesn't exist, but we jumped past)
            // 7: (this is instruction index 7, but we jump to 8)
            inst(Opcode::Return), // 7: return (catch path would return 0)
            inst(Opcode::Return), // 8: return (try path returns 42)
        ]);
        module.constants = vec![
            IrConstant {
                index: 0,
                const_type: "int".to_string(),
                value: serde_json::json!(42),
            },
            IrConstant {
                index: 1,
                const_type: "int".to_string(),
                value: serde_json::json!(0),
            },
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn try_catch_throw_caught() {
        // try { throw "oops"; 42 } catch { 99 }
        // Throw in try body → catch block runs → returns 99
        let mut module = make_module(vec![
            // 0: TRY_BEGIN offset=5
            IrInstruction {
                op: Opcode::TryBegin,
                offset: Some(5),
                ..inst(Opcode::TryBegin)
            },
            inst_const(0),        // 1: push "oops"
            inst(Opcode::Throw),  // 2: throw
            inst(Opcode::TryEnd), // 3: (unreachable)
            IrInstruction {
                // 4: JUMP past catch to 8
                op: Opcode::Jump,
                offset: Some(8),
                ..inst(Opcode::Jump)
            },
            // 5: CATCH
            IrInstruction {
                op: Opcode::Catch,
                ..inst(Opcode::Catch)
            },
            inst(Opcode::Pop),    // 6: discard error
            inst_const(1),        // 7: push 99
            inst(Opcode::Return), // 8: return
        ]);
        module.constants = vec![
            IrConstant {
                index: 0,
                const_type: "string".to_string(),
                value: serde_json::json!("oops"),
            },
            IrConstant {
                index: 1,
                const_type: "int".to_string(),
                value: serde_json::json!(99),
            },
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Int(99));
    }

    #[test]
    fn try_catch_with_binding() {
        // try { throw "error msg" } catch(e) { return e }
        let mut module = make_module(vec![
            // 0: TRY_BEGIN offset=5
            IrInstruction {
                op: Opcode::TryBegin,
                offset: Some(5),
                ..inst(Opcode::TryBegin)
            },
            inst_const(0),        // 1: push "error msg"
            inst(Opcode::Throw),  // 2: throw
            inst(Opcode::TryEnd), // 3: (unreachable)
            IrInstruction {
                // 4: JUMP past catch to 8
                op: Opcode::Jump,
                offset: Some(8),
                ..inst(Opcode::Jump)
            },
            // 5: CATCH
            IrInstruction {
                op: Opcode::Catch,
                ..inst(Opcode::Catch)
            },
            inst_store("e"),      // 6: store error in 'e'
            inst_load("e"),       // 7: load 'e'
            inst(Opcode::Return), // 8: return
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "string".to_string(),
            value: serde_json::json!("error msg"),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::String("error msg".to_string()));
    }

    #[test]
    fn try_catch_propagate_in_try() {
        // try { Err("fail")? } catch(e) { return e }
        // Propagate Err inside try → caught by catch
        let mut module = make_module(vec![
            // 0: TRY_BEGIN offset=6
            IrInstruction {
                op: Opcode::TryBegin,
                offset: Some(6),
                ..inst(Opcode::TryBegin)
            },
            inst_const(0),    // 1: push "fail"
            inst_load("Err"), // 2: load Err builtin
            IrInstruction {
                // 3: call Err("fail")
                op: Opcode::Call,
                argc: Some(1),
                ..inst(Opcode::Call)
            },
            inst(Opcode::Propagate), // 4: ? — should trigger catch
            inst(Opcode::TryEnd),    // 5: (unreachable)
            // 6: CATCH
            IrInstruction {
                op: Opcode::Catch,
                ..inst(Opcode::Catch)
            },
            inst_store("e"),      // 7: store error
            inst_load("e"),       // 8: load error
            inst(Opcode::Return), // 9: return
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "string".to_string(),
            value: serde_json::json!("fail"),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::String("fail".to_string()));
    }

    #[test]
    fn try_catch_nested() {
        // try { try { throw "inner" } catch { throw "outer" } } catch(e) { return e }
        let mut module = make_module(vec![
            // 0: outer TRY_BEGIN offset=10
            IrInstruction {
                op: Opcode::TryBegin,
                offset: Some(10),
                ..inst(Opcode::TryBegin)
            },
            // 1: inner TRY_BEGIN offset=5
            IrInstruction {
                op: Opcode::TryBegin,
                offset: Some(5),
                ..inst(Opcode::TryBegin)
            },
            inst_const(0),        // 2: push "inner"
            inst(Opcode::Throw),  // 3: throw inner
            inst(Opcode::TryEnd), // 4: (unreachable)
            // 5: inner CATCH
            IrInstruction {
                op: Opcode::Catch,
                ..inst(Opcode::Catch)
            },
            inst(Opcode::Pop),    // 6: discard inner error
            inst_const(1),        // 7: push "outer"
            inst(Opcode::Throw),  // 8: throw outer (from inner catch body)
            inst(Opcode::TryEnd), // 9: (unreachable — outer try end)
            // 10: outer CATCH
            IrInstruction {
                op: Opcode::Catch,
                ..inst(Opcode::Catch)
            },
            inst_store("e"),      // 11: store error in 'e'
            inst_load("e"),       // 12: load 'e'
            inst(Opcode::Return), // 13: return
        ]);
        module.constants = vec![
            IrConstant {
                index: 0,
                const_type: "string".to_string(),
                value: serde_json::json!("inner"),
            },
            IrConstant {
                index: 1,
                const_type: "string".to_string(),
                value: serde_json::json!("outer"),
            },
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::String("outer".to_string()));
    }

    #[test]
    fn throw_without_try_is_unhandled() {
        // throw "oops" — no try/catch, should be an error
        let mut module = make_module(vec![
            inst_const(0),
            inst(Opcode::Throw),
            inst(Opcode::Return),
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "string".to_string(),
            value: serde_json::json!("oops"),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("oops"));
    }

    #[test]
    fn try_catch_from_function_call() {
        // try { failing_fn() } catch(e) { return e }
        // where failing_fn throws "boom"
        let module = IrModule {
            version: "0.1.0".to_string(),
            module: "test".to_string(),
            source_file: "test.conc".to_string(),
            constants: vec![IrConstant {
                index: 0,
                const_type: "string".to_string(),
                value: serde_json::json!("boom"),
            }],
            types: vec![],
            functions: vec![
                IrFunction {
                    name: "main".to_string(),
                    module: "test".to_string(),
                    visibility: "private".to_string(),
                    params: vec![],
                    return_type: serde_json::json!("nil"),
                    is_async: false,
                    locals: vec![],
                    instructions: vec![
                        // 0: TRY_BEGIN offset=5
                        IrInstruction {
                            op: Opcode::TryBegin,
                            offset: Some(5),
                            ..inst(Opcode::TryBegin)
                        },
                        inst_load("failing_fn"), // 1: load fn ref
                        IrInstruction {
                            // 2: call failing_fn()
                            op: Opcode::Call,
                            argc: Some(0),
                            ..inst(Opcode::Call)
                        },
                        inst(Opcode::TryEnd), // 3: (unreachable if fn throws)
                        IrInstruction {
                            // 4: JUMP past catch to 8
                            op: Opcode::Jump,
                            offset: Some(8),
                            ..inst(Opcode::Jump)
                        },
                        // 5: CATCH
                        IrInstruction {
                            op: Opcode::Catch,
                            ..inst(Opcode::Catch)
                        },
                        inst_store("e"),      // 6: store error
                        inst_load("e"),       // 7: load error
                        inst(Opcode::Return), // 8: return
                    ],
                },
                IrFunction {
                    name: "failing_fn".to_string(),
                    module: "test".to_string(),
                    visibility: "private".to_string(),
                    params: vec![],
                    return_type: serde_json::json!("nil"),
                    is_async: false,
                    locals: vec![],
                    instructions: vec![
                        inst_const(0),       // 0: push "boom"
                        inst(Opcode::Throw), // 1: throw "boom"
                    ],
                },
            ],
            models: vec![],
            tools: vec![],
            schemas: vec![],
            connections: vec![],
            hashmaps: vec![],
            ledgers: vec![],
            memories: vec![],
            agents: vec![],
            listens: vec![],
            pipelines: vec![],
            tests: vec![],
            source_map: None,
            metadata: IrMetadata {
                compiler_version: "0.1.0".to_string(),
                compiled_at: String::new(),
                optimization_level: 0,
                debug_info: true,
                entry_point: "main".to_string(),
            },
        };

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::String("boom".to_string()));
    }

    // ========================================================================
    // IndexSet tests
    // ========================================================================

    #[test]
    fn index_set_array() {
        // arr = [10, 20, 30]; arr[1] = 99; return arr
        let mut module = make_module(vec![
            inst_const(0),
            inst_const(1),
            inst_const(2),
            IrInstruction {
                op: Opcode::BuildArray,
                count: Some(3),
                ..inst(Opcode::BuildArray)
            },
            inst_store("arr"),
            // arr[1] = 99
            inst_load("arr"),
            inst_const(3), // index: 1
            inst_const(4), // value: 99
            inst(Opcode::IndexSet),
            inst_store("arr"), // store mutated arr back
            inst_load("arr"),
            inst(Opcode::Return),
        ]);
        module.constants = vec![
            IrConstant {
                index: 0,
                const_type: "int".to_string(),
                value: serde_json::json!(10),
            },
            IrConstant {
                index: 1,
                const_type: "int".to_string(),
                value: serde_json::json!(20),
            },
            IrConstant {
                index: 2,
                const_type: "int".to_string(),
                value: serde_json::json!(30),
            },
            IrConstant {
                index: 3,
                const_type: "int".to_string(),
                value: serde_json::json!(1),
            },
            IrConstant {
                index: 4,
                const_type: "int".to_string(),
                value: serde_json::json!(99),
            },
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(
            result,
            Value::Array(vec![Value::Int(10), Value::Int(99), Value::Int(30)])
        );
    }

    #[test]
    fn index_set_map() {
        // map = {"a": 1}; map["a"] = 99; return map
        let mut module = make_module(vec![
            inst_const(0),
            inst_const(1),
            IrInstruction {
                op: Opcode::BuildMap,
                count: Some(1),
                ..inst(Opcode::BuildMap)
            },
            inst_store("map"),
            // map["a"] = 99
            inst_load("map"),
            inst_const(0), // key "a"
            inst_const(2), // value 99
            inst(Opcode::IndexSet),
            inst_store("map"),
            inst_load("map"),
            inst(Opcode::Return),
        ]);
        module.constants = vec![
            IrConstant {
                index: 0,
                const_type: "string".to_string(),
                value: serde_json::json!("a"),
            },
            IrConstant {
                index: 1,
                const_type: "int".to_string(),
                value: serde_json::json!(1),
            },
            IrConstant {
                index: 2,
                const_type: "int".to_string(),
                value: serde_json::json!(99),
            },
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        match result {
            Value::Map(pairs) => {
                assert_eq!(pairs.len(), 1);
                assert_eq!(pairs[0], ("a".to_string(), Value::Int(99)));
            }
            _ => panic!("expected Map"),
        }
    }

    // ========================================================================
    // CheckType tests
    // ========================================================================

    #[test]
    fn check_type_matching() {
        let mut module = make_module(vec![
            inst_const(0), // push 42
            IrInstruction {
                op: Opcode::CheckType,
                type_name: Some("Int".to_string()),
                ..inst(Opcode::CheckType)
            },
            inst(Opcode::Return),
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "int".to_string(),
            value: serde_json::json!(42),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn check_type_mismatch() {
        let mut module = make_module(vec![
            inst_const(0), // push "hello"
            IrInstruction {
                op: Opcode::CheckType,
                type_name: Some("Int".to_string()),
                ..inst(Opcode::CheckType)
            },
            inst(Opcode::Return),
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "string".to_string(),
            value: serde_json::json!("hello"),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    // ========================================================================
    // Cast tests
    // ========================================================================

    #[test]
    fn cast_int_to_float() {
        let mut module = make_module(vec![
            inst_const(0), // push 42
            IrInstruction {
                op: Opcode::Cast,
                type_name: Some("Float".to_string()),
                ..inst(Opcode::Cast)
            },
            inst(Opcode::Return),
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "int".to_string(),
            value: serde_json::json!(42),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Float(42.0));
    }

    #[test]
    fn cast_float_to_int() {
        let mut module = make_module(vec![
            inst_const(0), // push 3.7
            IrInstruction {
                op: Opcode::Cast,
                type_name: Some("Int".to_string()),
                ..inst(Opcode::Cast)
            },
            inst(Opcode::Return),
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "float".to_string(),
            value: serde_json::json!(3.7),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn cast_to_string() {
        let mut module = make_module(vec![
            inst_const(0), // push 42
            IrInstruction {
                op: Opcode::Cast,
                type_name: Some("String".to_string()),
                ..inst(Opcode::Cast)
            },
            inst(Opcode::Return),
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "int".to_string(),
            value: serde_json::json!(42),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::String("42".to_string()));
    }

    #[test]
    fn build_chat_request_includes_memory_between_system_and_user_prompt() {
        let mut module = make_module(vec![inst(Opcode::Return)]);
        module.models = vec![IrModel {
            name: "Assistant".to_string(),
            module: "test".to_string(),
            connection: "openai".to_string(),
            config: IrModelConfig {
                base: Some("gpt-4o-mini".to_string()),
                temperature: Some(0.2),
                max_tokens: Some(256),
                system_prompt: Some("System prompt".to_string()),
                timeout: None,
            },
            tools: vec![],
            memory: None,
            decorators: vec![],
            methods: vec![],
        }];
        module.memories = vec![IrMemory {
            name: "conv".to_string(),
            max_messages: None,
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);

        vm.memory_store
            .append("conv", "user", "Earlier question")
            .unwrap();
        vm.memory_store
            .append("conv", "assistant", "Earlier answer")
            .unwrap();

        let model_def = vm.module.models.get("Assistant").unwrap().clone();
        let request =
            vm.build_chat_request_full(&model_def, "Current question", None, Some("conv"), &[], false);

        assert_eq!(request.messages.len(), 4);
        assert_eq!(request.messages[0].role, "system");
        assert_eq!(request.messages[0].content, "System prompt");
        assert_eq!(request.messages[1].role, "user");
        assert_eq!(request.messages[1].content, "Earlier question");
        assert_eq!(request.messages[2].role, "assistant");
        assert_eq!(request.messages[2].content, "Earlier answer");
        assert_eq!(request.messages[3].role, "user");
        assert_eq!(request.messages[3].content, "Current question");
    }

    #[test]
    fn build_chat_request_merges_and_deduplicates_static_and_dynamic_tools() {
        let mut module = make_module(vec![inst(Opcode::Return)]);
        module.models = vec![IrModel {
            name: "Worker".to_string(),
            module: "test".to_string(),
            connection: "openai".to_string(),
            config: IrModelConfig {
                base: Some("gpt-4o-mini".to_string()),
                temperature: None,
                max_tokens: None,
                system_prompt: None,
                timeout: None,
            },
            tools: vec!["Calculator".to_string()],
            memory: None,
            decorators: vec![],
            methods: vec![],
        }];
        module.tools = vec![
            IrTool {
                name: "Calculator".to_string(),
                module: "test".to_string(),
                methods: vec![],
                tool_schemas: vec![ToolSchemaEntry {
                    method_name: "Calculator::add".to_string(),
                    description: "Add integers".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "a": { "type": "integer" },
                            "b": { "type": "integer" }
                        },
                        "required": ["a", "b"]
                    }),
                }],
            },
            IrTool {
                name: "Formatter".to_string(),
                module: "test".to_string(),
                methods: vec![],
                tool_schemas: vec![ToolSchemaEntry {
                    method_name: "Formatter::up".to_string(),
                    description: "Uppercase text".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "text": { "type": "string" }
                        },
                        "required": ["text"]
                    }),
                }],
            },
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let vm = VM::new(loaded);

        let model_def = vm.module.models.get("Worker").unwrap().clone();
        let extra_tools = vec!["Calculator".to_string(), "Formatter".to_string()];
        let request = vm.build_chat_request_full(&model_def, "prompt", None, None, &extra_tools, false);

        let mut tool_names: Vec<String> = request
            .tools
            .unwrap_or_default()
            .into_iter()
            .map(|t| t.name)
            .collect();
        tool_names.sort();

        assert_eq!(tool_names, vec!["Calculator::add", "Formatter::up"]);
    }

    #[test]
    fn build_chat_request_without_tools_excludes_static_but_keeps_dynamic() {
        let mut module = make_module(vec![inst(Opcode::Return)]);
        module.models = vec![IrModel {
            name: "Worker".to_string(),
            module: "test".to_string(),
            connection: "openai".to_string(),
            config: IrModelConfig {
                base: Some("gpt-4o-mini".to_string()),
                temperature: None,
                max_tokens: None,
                system_prompt: None,
                timeout: None,
            },
            tools: vec!["Calculator".to_string()],
            memory: None,
            decorators: vec![],
            methods: vec![],
        }];
        module.tools = vec![
            IrTool {
                name: "Calculator".to_string(),
                module: "test".to_string(),
                methods: vec![],
                tool_schemas: vec![ToolSchemaEntry {
                    method_name: "Calculator::add".to_string(),
                    description: "Add integers".to_string(),
                    parameters: serde_json::json!({"type":"object","properties":{},"required":[]}),
                }],
            },
            IrTool {
                name: "Formatter".to_string(),
                module: "test".to_string(),
                methods: vec![],
                tool_schemas: vec![ToolSchemaEntry {
                    method_name: "Formatter::up".to_string(),
                    description: "Uppercase text".to_string(),
                    parameters: serde_json::json!({"type":"object","properties":{},"required":[]}),
                }],
            },
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let vm = VM::new(loaded);

        let model_def = vm.module.models.get("Worker").unwrap().clone();
        let extra_tools = vec!["Formatter".to_string()];
        let request = vm.build_chat_request_full(&model_def, "prompt", None, None, &extra_tools, true);

        let tool_names: Vec<String> = request
            .tools
            .unwrap_or_default()
            .into_iter()
            .map(|t| t.name)
            .collect();

        assert_eq!(tool_names, vec!["Formatter::up"]);
    }

    #[test]
    fn vm_registers_tool_refs_for_with_tools_arrays() {
        let mut module = make_module(vec![inst(Opcode::Return)]);
        module.tools = vec![IrTool {
            name: "Calculator".to_string(),
            module: "test".to_string(),
            methods: vec![],
            tool_schemas: vec![],
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let vm = VM::new(loaded);

        match vm.globals.get("Calculator") {
            Some(Value::Function(name)) => assert_eq!(name, "Calculator"),
            other => panic!("expected Function(\"Calculator\"), got {:?}", other),
        }
    }

    #[test]
    fn spawn_async_creates_thunk() {
        // LOAD_CONST "add_fn" (as function name), SPAWN_ASYNC → Thunk on stack
        let mut module = make_module(vec![
            inst_const(0), // push function name string
            inst(Opcode::Return),
        ]);
        // We'll push a Function value via LoadGlobal + SpawnAsync
        // Simpler: test that SpawnAsync with a Function creates a Thunk
        module.functions.push(IrFunction {
            name: "helper".to_string(),
            module: "test".to_string(),
            visibility: "private".to_string(),
            params: vec![],
            return_type: serde_json::json!("Int"),
            is_async: false,
            locals: vec![],
            instructions: vec![inst_const(1), inst(Opcode::Return)],
        });
        module.constants = vec![
            IrConstant {
                index: 0,
                const_type: "string".to_string(),
                value: serde_json::json!("helper"),
            },
            IrConstant {
                index: 1,
                const_type: "int".to_string(),
                value: serde_json::json!(42),
            },
        ];
        // main: LOAD_LOCAL "helper" (function ref), SPAWN_ASYNC, RETURN
        module.functions[0].instructions = vec![
            {
                let mut i = inst(Opcode::LoadLocal);
                i.name = Some("helper".to_string());
                i
            },
            inst(Opcode::SpawnAsync),
            inst(Opcode::Return),
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        match result {
            Value::Thunk { function, args } => {
                assert_eq!(function, "helper");
                assert!(args.is_empty());
            }
            _ => panic!("expected Thunk, got {:?}", result),
        }
    }

    #[test]
    fn await_resolves_thunk() {
        // Push Thunk for "helper" then Await → should call helper and get 42
        let mut module = make_module(vec![]);
        module.functions.push(IrFunction {
            name: "helper".to_string(),
            module: "test".to_string(),
            visibility: "private".to_string(),
            params: vec![],
            return_type: serde_json::json!("Int"),
            is_async: false,
            locals: vec![],
            instructions: vec![inst_const(0), inst(Opcode::Return)],
        });
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "int".to_string(),
            value: serde_json::json!(42),
        }];
        // main: LOAD_LOCAL "helper" (fn ref), SPAWN_ASYNC (→ thunk), AWAIT (→ 42), RETURN
        module.functions[0].instructions = vec![
            {
                let mut i = inst(Opcode::LoadLocal);
                i.name = Some("helper".to_string());
                i
            },
            inst(Opcode::SpawnAsync),
            inst(Opcode::Await),
            inst(Opcode::Return),
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn await_passthrough_non_thunk() {
        // Await on a non-thunk value passes it through
        let mut module = make_module(vec![
            inst_const(0),
            inst(Opcode::Await),
            inst(Opcode::Return),
        ]);
        module.constants = vec![IrConstant {
            index: 0,
            const_type: "string".to_string(),
            value: serde_json::json!("hello"),
        }];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::String("hello".to_string()));
    }

    #[test]
    fn await_all_collects_results() {
        // AwaitAll collects N values (including thunks) into an Array
        let mut module = make_module(vec![]);
        module.functions.push(IrFunction {
            name: "get_ten".to_string(),
            module: "test".to_string(),
            visibility: "private".to_string(),
            params: vec![],
            return_type: serde_json::json!("Int"),
            is_async: false,
            locals: vec![],
            instructions: vec![inst_const(0), inst(Opcode::Return)],
        });
        module.constants = vec![
            IrConstant {
                index: 0,
                const_type: "int".to_string(),
                value: serde_json::json!(10),
            },
            IrConstant {
                index: 1,
                const_type: "int".to_string(),
                value: serde_json::json!(20),
            },
        ];
        // main: push thunk for get_ten, push literal 20, AWAIT_ALL(2) → [10, 20]
        module.functions[0].instructions = vec![
            {
                let mut i = inst(Opcode::LoadLocal);
                i.name = Some("get_ten".to_string());
                i
            },
            inst(Opcode::SpawnAsync), // Thunk { function: "get_ten", args: [] }
            inst_const(1),            // 20
            {
                let mut i = inst(Opcode::AwaitAll);
                i.count = Some(2);
                i
            },
            inst(Opcode::Return),
        ];

        let loaded = LoadedModule::from_ir(module).unwrap();
        let mut vm = VM::new(loaded);
        let result = vm.execute().unwrap();
        assert_eq!(result, Value::Array(vec![Value::Int(10), Value::Int(20)]));
    }
}
