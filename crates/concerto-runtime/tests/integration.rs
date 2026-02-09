//! End-to-end integration tests: compile Concerto source → IR → VM → verify.

use std::sync::{Arc, Mutex};

use concerto_compiler::codegen::CodeGenerator;
use concerto_compiler::lexer::Lexer;
use concerto_compiler::parser;
use concerto_runtime::value::Value;
use concerto_runtime::{LoadedModule, VM};

/// Compile source, run through VM, return (final_value, collected_emits).
/// Each emit is (channel, display_string).
fn run_program(source: &str) -> (Value, Vec<(String, String)>) {
    // Lex
    let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
    assert!(
        !lex_diags.has_errors(),
        "lexer errors: {:?}",
        lex_diags.diagnostics()
    );

    // Parse
    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    assert!(
        !parse_diags.has_errors(),
        "parse errors: {:?}",
        parse_diags.diagnostics()
    );

    // Semantic analysis
    let sem_diags = concerto_compiler::semantic::analyze(&program);
    assert!(
        !sem_diags.has_errors(),
        "semantic errors: {:?}",
        sem_diags.diagnostics()
    );

    // Codegen
    let ir = CodeGenerator::new("test", "test.conc").generate(&program);

    // Load IR: serialize to JSON, deserialize to IrModule, then convert
    let json = serde_json::to_string(&ir).expect("IR serialization failed");
    let ir_module: concerto_common::ir::IrModule =
        serde_json::from_str(&json).expect("IR deserialization failed");
    let module = LoadedModule::from_ir(ir_module).expect("IR loading failed");

    // VM
    let emits: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let emits_clone = emits.clone();
    let mut vm = VM::new(module);
    vm.set_emit_handler(move |channel, payload| {
        emits_clone
            .lock()
            .unwrap()
            .push((channel.to_string(), payload.display_string()));
    });

    let result = vm.execute().expect("VM execution failed");
    let collected = emits.lock().unwrap().clone();
    (result, collected)
}

// =========================================================================
// Tests
// =========================================================================

#[test]
fn e2e_arithmetic() {
    let (_, emits) = run_program(
        r#"
        fn main() {
            let x = 10;
            let y = 3;
            emit("result", x + y * 2);
        }
        "#,
    );
    assert_eq!(emits.len(), 1);
    assert_eq!(emits[0].0, "result");
    assert_eq!(emits[0].1, "16");
}

#[test]
fn e2e_string_ops() {
    let (_, emits) = run_program(
        r#"
        fn main() {
            let name = "world";
            let greeting = "Hello, " + name + "!";
            emit("result", greeting);
        }
        "#,
    );
    assert_eq!(emits[0].1, "Hello, world!");
}

#[test]
fn e2e_if_else() {
    let (_, emits) = run_program(
        r#"
        fn main() {
            let x = 5;
            let result = if x > 3 { "big" } else { "small" };
            emit("result", result);
        }
        "#,
    );
    assert_eq!(emits[0].1, "big");
}

#[test]
fn e2e_match() {
    let (_, emits) = run_program(
        r#"
        fn main() {
            let x = 2;
            let result = match x {
                1 => "one",
                2 => "two",
                _ => "other",
            };
            emit("result", result);
        }
        "#,
    );
    assert_eq!(emits[0].1, "two");
}

#[test]
fn e2e_for_loop() {
    let (_, emits) = run_program(
        r#"
        fn main() {
            let mut sum = 0;
            for i in [1, 2, 3, 4, 5] {
                sum = sum + i;
            }
            emit("result", sum);
        }
        "#,
    );
    assert_eq!(emits[0].1, "15");
}

#[test]
fn e2e_while_loop() {
    let (_, emits) = run_program(
        r#"
        fn main() {
            let mut count = 0;
            while count < 5 {
                count = count + 1;
            }
            emit("result", count);
        }
        "#,
    );
    assert_eq!(emits[0].1, "5");
}

#[test]
fn e2e_functions() {
    let (_, emits) = run_program(
        r#"
        fn add(a: Int, b: Int) -> Int {
            return a + b;
        }

        fn main() {
            let result = add(3, 7);
            emit("result", result);
        }
        "#,
    );
    assert_eq!(emits[0].1, "10");
}

#[test]
fn e2e_pipe_operator() {
    let (_, emits) = run_program(
        r#"
        fn double(x: Int) -> Int {
            return x * 2;
        }

        fn main() {
            let result = 5 |> double;
            emit("result", result);
        }
        "#,
    );
    assert_eq!(emits[0].1, "10");
}

#[test]
fn e2e_structs() {
    let (_, emits) = run_program(
        r#"
        struct Point {
            x: Int,
            y: Int,
        }

        fn main() {
            let p = Point { x: 10, y: 20 };
            emit("x", p.x);
            emit("y", p.y);
        }
        "#,
    );
    assert_eq!(emits.len(), 2);
    assert_eq!(emits[0].1, "10");
    assert_eq!(emits[1].1, "20");
}

#[test]
fn e2e_try_catch() {
    let (_, emits) = run_program(
        r#"
        fn risky() -> Result<Int, String> {
            throw "oops";
        }

        fn main() {
            try {
                risky();
            } catch {
                emit("caught", "handled");
            }
        }
        "#,
    );
    assert_eq!(emits[0].0, "caught");
    assert_eq!(emits[0].1, "handled");
}

#[test]
fn e2e_result_propagation() {
    let (_, emits) = run_program(
        r#"
        fn safe_divide(a: Int, b: Int) -> Result<Int, String> {
            if b == 0 {
                return Err("division by zero");
            }
            return Ok(a / b);
        }

        fn main() {
            let r = safe_divide(10, 2);
            match r {
                Ok(v) => emit("result", v),
                Err(e) => emit("error", e),
            }
        }
        "#,
    );
    assert_eq!(emits[0].0, "result");
    assert_eq!(emits[0].1, "5");
}

#[test]
fn e2e_match_result_err_selects_err_arm() {
    let (_, emits) = run_program(
        r#"
        fn main() {
            let result = Err("boom");
            match result {
                Ok(v) => emit("branch", "ok"),
                Err(e) => emit("branch", "err"),
            }
        }
        "#,
    );
    assert_eq!(emits.len(), 1);
    assert_eq!(emits[0].0, "branch");
    assert_eq!(emits[0].1, "err");
}

#[test]
fn e2e_database() {
    let (_, emits) = run_program(
        r#"
        hashmap store: HashMap<String, Int> = HashMap::new();

        fn main() {
            store.set("count", 42);
            let val = store.get("count");
            emit("has", store.has("count"));
            emit("val", val);
            store.delete("count");
            emit("after_delete", store.has("count"));
        }
        "#,
    );
    assert_eq!(emits[0].1, "true");
    // hashmap.get returns Option, so displayed as Some(42)
    assert_eq!(emits[1].1, "Some(42)");
    assert_eq!(emits[2].1, "false");
}

#[test]
fn e2e_stdlib_math() {
    let (_, emits) = run_program(
        r#"
        fn main() {
            let a = std::math::abs(-5);
            let b = std::math::max(3, 7);
            let c = std::math::min(3, 7);
            emit("abs", a);
            emit("max", b);
            emit("min", c);
        }
        "#,
    );
    assert_eq!(emits[0].1, "5");
    assert_eq!(emits[1].1, "7");
    assert_eq!(emits[2].1, "3");
}

#[test]
fn e2e_stdlib_string() {
    let (_, emits) = run_program(
        r#"
        fn main() {
            let s = std::string::to_upper("hello");
            let t = std::string::trim("  hi  ");
            emit("upper", s);
            emit("trim", t);
        }
        "#,
    );
    assert_eq!(emits[0].1, "hello".to_uppercase());
    assert_eq!(emits[1].1, "hi");
}

#[test]
fn e2e_nested_calls() {
    let (_, emits) = run_program(
        r#"
        fn factorial(n: Int) -> Int {
            if n <= 1 {
                return 1;
            }
            return n * factorial(n - 1);
        }

        fn main() {
            emit("result", factorial(6));
        }
        "#,
    );
    assert_eq!(emits[0].1, "720");
}

#[test]
fn e2e_memory_declaration() {
    let (_, emits) = run_program(
        r#"
        memory conversation: Memory = Memory::new();

        fn main() {
            conversation.append("user", "Hello");
            conversation.append("assistant", "Hi there!");
            emit("len", conversation.len());
            let msgs = conversation.messages();
            emit("count", len(msgs));
            conversation.clear();
            emit("after_clear", conversation.len());
        }
        "#,
    );
    assert_eq!(emits[0].0, "len");
    assert_eq!(emits[0].1, "2");
    assert_eq!(emits[1].0, "count");
    assert_eq!(emits[1].1, "2");
    assert_eq!(emits[2].0, "after_clear");
    assert_eq!(emits[2].1, "0");
}

#[test]
fn e2e_memory_last() {
    let (_, emits) = run_program(
        r#"
        memory conv: Memory = Memory::new();

        fn main() {
            conv.append("user", "msg1");
            conv.append("assistant", "msg2");
            conv.append("user", "msg3");
            conv.append("assistant", "msg4");
            let last2 = conv.last(2);
            emit("last_count", len(last2));
        }
        "#,
    );
    assert_eq!(emits[0].1, "2");
}

#[test]
fn e2e_memory_sliding_window() {
    let (_, emits) = run_program(
        r#"
        memory conv: Memory = Memory::new(3);

        fn main() {
            conv.append("user", "msg1");
            conv.append("assistant", "msg2");
            conv.append("user", "msg3");
            conv.append("assistant", "msg4");
            emit("len", conv.len());
        }
        "#,
    );
    // After 4 appends with max 3, oldest is dropped
    assert_eq!(emits[0].1, "3");
}

#[test]
fn e2e_agent_with_memory_builder_auto_and_manual_modes() {
    let (_, emits) = run_program(
        r#"
        const openai: Int = 0;
        memory conv: Memory = Memory::new(3);

        model Assistant {
            provider: openai,
            base: "gpt-4o-mini",
            system_prompt: "You are a concise assistant.",
        }

        fn main() {
            let _r1 = Assistant.with_memory(conv).execute("prompt-1");
            let _r2 = Assistant.with_memory(conv).execute("prompt-2");

            emit("len_after_auto", conv.len());
            let msgs = conv.messages();
            emit("oldest_role", msgs[0].role);
            emit("latest_role", msgs[2].role);

            let r3 = Assistant.with_memory(conv, false).execute("prompt-3");
            emit("manual_ok", r3.is_ok());
            emit("len_after_manual", conv.len());
        }
        "#,
    );
    // Two auto-appending calls (4 messages) with max=3 keeps only the latest 3.
    assert_eq!(emits[0].0, "len_after_auto");
    assert_eq!(emits[0].1, "3");
    assert_eq!(emits[1].0, "oldest_role");
    assert_eq!(emits[1].1, "assistant");
    assert_eq!(emits[2].0, "latest_role");
    assert_eq!(emits[2].1, "assistant");
    assert_eq!(emits[3].0, "manual_ok");
    assert_eq!(emits[3].1, "true");
    assert_eq!(emits[4].0, "len_after_manual");
    assert_eq!(emits[4].1, "3");
}

#[test]
fn e2e_dynamic_tool_binding_builder_paths() {
    let (_, emits) = run_program(
        r#"
        const openai: Int = 0;
        memory conv: Memory = Memory::new();

        tool Calculator {
            description: "Simple arithmetic operations",

            @describe("Add two integers")
            @param("a", "First integer")
            @param("b", "Second integer")
            pub fn add(self, a: Int, b: Int) -> Int {
                a + b
            }
        }

        tool Formatter {
            description: "String formatting helper",

            @describe("Uppercase the input text")
            @param("text", "Input text")
            pub fn up(self, text: String) -> String {
                std::string::to_upper(text)
            }
        }

        model Worker {
            provider: openai,
            base: "gpt-4o-mini",
            system_prompt: "Use tools if needed.",
            tools: [Calculator],
        }

        fn main() {
            let base = Worker.execute("base");
            let plus_dynamic = Worker.with_tools([Formatter]).execute("plus");
            let stripped = Worker.without_tools().execute("stripped");
            let chained = Worker.without_tools().with_tools([Calculator, Formatter]).execute("chained");
            let combo = Worker.with_memory(conv).with_tools([Formatter]).execute("combo");

            emit("base_ok", base.is_ok());
            emit("plus_dynamic_ok", plus_dynamic.is_ok());
            emit("stripped_ok", stripped.is_ok());
            emit("chained_ok", chained.is_ok());
            emit("combo_ok", combo.is_ok());
            emit("combo_mem_len", conv.len());
        }
        "#,
    );
    assert_eq!(emits[0], ("base_ok".to_string(), "true".to_string()));
    assert_eq!(
        emits[1],
        ("plus_dynamic_ok".to_string(), "true".to_string())
    );
    assert_eq!(emits[2], ("stripped_ok".to_string(), "true".to_string()));
    assert_eq!(emits[3], ("chained_ok".to_string(), "true".to_string()));
    assert_eq!(emits[4], ("combo_ok".to_string(), "true".to_string()));
    assert_eq!(emits[5], ("combo_mem_len".to_string(), "2".to_string()));
}

#[test]
fn e2e_agent_declaration() {
    // Agent declarations compile and load successfully.
    // We can't execute without a real subprocess, but verify the pipeline works.
    let source = r#"
        agent EchoAgent {
            connector: "echo_service",
            input_format: "text",
            output_format: "text",
            timeout: 30,
        }

        fn main() {
            emit("has_agent", "yes");
        }
    "#;

    let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
    assert!(
        !lex_diags.has_errors(),
        "lexer errors: {:?}",
        lex_diags.diagnostics()
    );

    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    assert!(
        !parse_diags.has_errors(),
        "parse errors: {:?}",
        parse_diags.diagnostics()
    );

    let sem_diags = concerto_compiler::semantic::analyze(&program);
    assert!(
        !sem_diags.has_errors(),
        "semantic errors: {:?}",
        sem_diags.diagnostics()
    );

    let ir = CodeGenerator::new("test", "test.conc").generate(&program);

    // Verify agent appears in IR
    assert_eq!(ir.agents.len(), 1);
    assert_eq!(ir.agents[0].name, "EchoAgent");
    assert_eq!(ir.agents[0].connector, "echo_service");
    assert_eq!(ir.agents[0].input_format, "text");
    assert_eq!(ir.agents[0].output_format, "text");
    assert_eq!(ir.agents[0].timeout, Some(30));

    // Load and execute
    let json = serde_json::to_string(&ir).expect("IR serialization failed");
    let ir_module: concerto_common::ir::IrModule =
        serde_json::from_str(&json).expect("IR deserialization failed");
    let module = LoadedModule::from_ir(ir_module).expect("IR loading failed");

    let mut vm = VM::new(module);
    let result = vm.execute();
    assert!(result.is_ok());
}

// =========================================================================
// Listen expression tests
// =========================================================================

#[test]
fn e2e_listen_compiles_and_loads() {
    // Test that a listen expression compiles to valid IR and loads correctly
    let source = r#"
        agent StreamAgent {
            connector: "stream_agent",
        }
        fn main() {
            let result = listen StreamAgent.execute("do work") {
                "progress" => |msg| {
                    emit("log", msg);
                },
                "question" => |q| {
                    "yes"
                },
            };
        }
    "#;

    let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
    assert!(!lex_diags.has_errors());
    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    assert!(
        !parse_diags.has_errors(),
        "parse errors: {:?}",
        parse_diags.diagnostics()
    );
    let sem_diags = concerto_compiler::semantic::analyze(&program);
    assert!(
        !sem_diags.has_errors(),
        "semantic errors: {:?}",
        sem_diags.diagnostics()
    );

    let ir = CodeGenerator::new("test", "test.conc").generate(&program);

    // Verify listen IR structure
    assert_eq!(ir.listens.len(), 1);
    assert_eq!(ir.listens[0].agent, "StreamAgent");
    assert_eq!(ir.listens[0].handlers.len(), 2);
    assert_eq!(ir.listens[0].handlers[0].message_type, "progress");
    assert_eq!(ir.listens[0].handlers[1].message_type, "question");

    // Verify IR serialization round-trip
    let json = serde_json::to_string(&ir).expect("IR serialization failed");
    let ir_module: concerto_common::ir::IrModule =
        serde_json::from_str(&json).expect("IR deserialization failed");
    let module = LoadedModule::from_ir(ir_module).expect("IR loading failed");

    // Verify listen loaded into module
    assert!(module.listens.contains_key("$listen_0"));
}

#[test]
fn e2e_listen_vm_execution() {
    // Test actual VM execution with a mock agent that outputs NDJSON.
    // Uses printf to output progress + result messages.
    let source = r#"
        agent MockAgent {
            connector: "mock",
        }
        fn main() {
            let result = listen MockAgent.execute("do work") {
                "progress" => |msg| {
                    emit("progress_received", msg);
                },
            };
            emit("done", result);
        }
    "#;

    let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
    assert!(!lex_diags.has_errors());
    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    assert!(!parse_diags.has_errors(), "parse errors: {:?}", parse_diags.diagnostics());
    let sem_diags = concerto_compiler::semantic::analyze(&program);
    assert!(!sem_diags.has_errors(), "semantic errors: {:?}", sem_diags.diagnostics());

    let ir = CodeGenerator::new("test", "test.conc").generate(&program);
    let json = serde_json::to_string(&ir).expect("IR serialization failed");

    // Patch the agent command to use printf for mock NDJSON output
    let mut ir_module: concerto_common::ir::IrModule =
        serde_json::from_str(&json).expect("IR deserialization failed");
    // Set the agent command to printf which outputs NDJSON
    ir_module.agents[0].command = Some("printf".to_string());
    ir_module.agents[0].args = Some(vec![
        r#"{"type":"progress","message":"Working on it..."}\n{"type":"result","text":"All done"}\n"#.to_string(),
    ]);

    let module = LoadedModule::from_ir(ir_module).expect("IR loading failed");

    let emits: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let emits_clone = emits.clone();
    let mut vm = VM::new(module);
    vm.set_emit_handler(move |channel, payload| {
        emits_clone
            .lock()
            .unwrap()
            .push((channel.to_string(), payload.display_string()));
    });

    let result = vm.execute();
    assert!(result.is_ok(), "VM execution failed: {:?}", result.err());

    let collected = emits.lock().unwrap().clone();
    // Should have: listen:start, progress_received, listen:complete, done
    let channels: Vec<&str> = collected.iter().map(|(c, _)| c.as_str()).collect();
    assert!(channels.contains(&"listen:start"), "missing listen:start, got: {:?}", channels);
    assert!(channels.contains(&"progress_received"), "missing progress_received, got: {:?}", channels);
    assert!(channels.contains(&"listen:complete"), "missing listen:complete, got: {:?}", channels);
    assert!(channels.contains(&"done"), "missing done, got: {:?}", channels);
}

#[test]
fn e2e_listen_bidirectional() {
    // Test bidirectional communication: agent sends question, handler responds.
    // Uses a bash script via `bash -c` that reads stdin and writes to stdout.
    let source = r#"
        agent BidiAgent {
            connector: "bidi",
        }
        fn main() {
            let result = listen BidiAgent.execute("start") {
                "question" => |q| {
                    emit("got_question", q);
                    "yes"
                },
            };
            emit("final", result);
        }
    "#;

    let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
    assert!(!lex_diags.has_errors());
    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    assert!(!parse_diags.has_errors());
    let sem_diags = concerto_compiler::semantic::analyze(&program);
    assert!(!sem_diags.has_errors());

    let ir = CodeGenerator::new("test", "test.conc").generate(&program);
    let json = serde_json::to_string(&ir).expect("IR serialization failed");

    let mut ir_module: concerto_common::ir::IrModule =
        serde_json::from_str(&json).expect("IR deserialization failed");
    // Script: read prompt, send question, read response, send result
    ir_module.agents[0].command = Some("bash".to_string());
    ir_module.agents[0].args = Some(vec![
        "-c".to_string(),
        // Read the prompt line, emit a question, read back response, emit result
        r#"read prompt; echo '{"type":"question","question":"Approve?"}'; read response; echo '{"type":"result","text":"completed"}';"#.to_string(),
    ]);

    let module = LoadedModule::from_ir(ir_module).expect("IR loading failed");

    let emits: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let emits_clone = emits.clone();
    let mut vm = VM::new(module);
    vm.set_emit_handler(move |channel, payload| {
        emits_clone
            .lock()
            .unwrap()
            .push((channel.to_string(), payload.display_string()));
    });

    let result = vm.execute();
    assert!(result.is_ok(), "VM execution failed: {:?}", result.err());

    let collected = emits.lock().unwrap().clone();
    let channels: Vec<&str> = collected.iter().map(|(c, _)| c.as_str()).collect();
    assert!(channels.contains(&"got_question"), "missing got_question, got: {:?}", channels);
    assert!(channels.contains(&"final"), "missing final, got: {:?}", channels);
}

// =========================================================================
// Direct run tests (compile .conc in-memory → execute)
// =========================================================================

/// Helper: compile source to IrModule in-memory, load, return LoadedModule.
/// Mimics what `concerto run file.conc` does internally.
fn compile_and_load(source: &str) -> LoadedModule {
    let (tokens, lex_diags) = Lexer::new(source, "direct.conc").tokenize();
    assert!(!lex_diags.has_errors(), "lex errors: {:?}", lex_diags.diagnostics());

    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    assert!(!parse_diags.has_errors(), "parse errors: {:?}", parse_diags.diagnostics());

    let sem_diags = concerto_compiler::semantic::analyze(&program);
    assert!(!sem_diags.has_errors(), "semantic errors: {:?}", sem_diags.diagnostics());

    let ir = CodeGenerator::new("direct", "direct.conc").generate(&program);
    let json = serde_json::to_string(&ir).expect("IR serialize failed");
    let ir_module: concerto_common::ir::IrModule =
        serde_json::from_str(&json).expect("IR deserialize failed");
    LoadedModule::from_ir(ir_module).expect("LoadedModule failed")
}

#[test]
fn e2e_direct_run_basic_program() {
    // Verify the in-memory compile+run path works for a basic program
    let source = r#"
        fn main() {
            let x = 10 + 20;
            emit("result", x);
        }
    "#;

    let module = compile_and_load(source);
    let emits: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let emits_clone = emits.clone();
    let mut vm = VM::new(module);
    vm.set_emit_handler(move |channel, payload| {
        emits_clone.lock().unwrap().push((channel.to_string(), payload.display_string()));
    });

    let result = vm.execute();
    assert!(result.is_ok(), "execution failed: {:?}", result.err());

    let collected = emits.lock().unwrap().clone();
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].0, "result");
    assert_eq!(collected[0].1, "30");
}

#[test]
fn e2e_direct_run_with_stdlib() {
    // Verify stdlib calls work through direct run path
    let source = r#"
        fn main() {
            let s = "hello world";
            let upper = std::string::to_upper(s);
            emit("output", upper);
        }
    "#;

    let module = compile_and_load(source);
    let emits: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let emits_clone = emits.clone();
    let mut vm = VM::new(module);
    vm.set_emit_handler(move |channel, payload| {
        emits_clone.lock().unwrap().push((channel.to_string(), payload.display_string()));
    });

    let result = vm.execute();
    assert!(result.is_ok());
    let collected = emits.lock().unwrap().clone();
    assert_eq!(collected[0].1, "HELLO WORLD");
}

#[test]
fn e2e_direct_run_with_agent_mock() {
    // Verify agent execution works through direct run path (uses MockProvider)
    let source = r#"
        model TestBot {
            provider: openai,
            base: "gpt-4o-mini",
            system_prompt: "You are a test bot.",
        }

        fn main() {
            let result = TestBot.execute("Hello");
            match result {
                Ok(response) => emit("text", response.text),
                Err(e) => emit("error", e),
            }
        }
    "#;

    let (tokens, lex_diags) = Lexer::new(source, "agent.conc").tokenize();
    assert!(!lex_diags.has_errors());
    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    assert!(!parse_diags.has_errors());

    // Need to register "openai" as connection name for semantic analysis
    let sem_diags = concerto_compiler::semantic::analyze_with_connections(
        &program, &["openai".to_string()],
    );
    assert!(!sem_diags.has_errors(), "sem: {:?}", sem_diags.diagnostics());

    let mut codegen = CodeGenerator::new("agent_test", "agent.conc");
    codegen.add_manifest_connections(vec![concerto_common::ir::IrConnection {
        name: "openai".to_string(),
        config: serde_json::json!({
            "provider": "openai",
            "default_model": "gpt-4o-mini"
        }),
    }]);
    let ir = codegen.generate(&program);

    let json = serde_json::to_string(&ir).unwrap();
    let ir_module: concerto_common::ir::IrModule = serde_json::from_str(&json).unwrap();
    let module = LoadedModule::from_ir(ir_module).unwrap();

    let emits: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let emits_clone = emits.clone();
    let mut vm = VM::new(module);
    vm.set_emit_handler(move |channel, payload| {
        emits_clone.lock().unwrap().push((channel.to_string(), payload.display_string()));
    });

    let result = vm.execute();
    assert!(result.is_ok(), "execution failed: {:?}", result.err());

    let collected = emits.lock().unwrap().clone();
    assert_eq!(collected.len(), 1);
    // MockProvider returns a mock response, so we just check the channel
    assert_eq!(collected[0].0, "text");
}

// =========================================================================
// Testing system integration tests
// =========================================================================

/// Compile source for test mode (permissive loading), return LoadedModule.
fn compile_for_tests(source: &str) -> LoadedModule {
    compile_for_tests_with_connections(source, &[])
}

/// Compile source for test mode with connection names registered.
fn compile_for_tests_with_connections(source: &str, connections: &[&str]) -> LoadedModule {
    let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
    assert!(
        !lex_diags.has_errors(),
        "lexer errors: {:?}",
        lex_diags.diagnostics()
    );

    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    assert!(
        !parse_diags.has_errors(),
        "parse errors: {:?}",
        parse_diags.diagnostics()
    );

    let conn_names: Vec<String> = connections.iter().map(|s| s.to_string()).collect();
    let sem_diags =
        concerto_compiler::semantic::analyze_with_connections(&program, &conn_names);
    assert!(
        !sem_diags.has_errors(),
        "semantic errors: {:?}",
        sem_diags.diagnostics()
    );

    let ir = CodeGenerator::new("test", "test.conc").generate(&program);
    let json = serde_json::to_string(&ir).expect("IR serialization failed");
    let ir_module: concerto_common::ir::IrModule =
        serde_json::from_str(&json).expect("IR deserialization failed");
    LoadedModule::from_ir_permissive(ir_module).expect("IR loading failed")
}

#[test]
fn e2e_test_assert_passing() {
    let module = compile_for_tests(
        r#"
        @test
        fn basic_assertions() {
            assert(true);
            assert_eq(2 + 3, 5);
            assert_ne(1, 2);
            assert(true, "custom message");
        }
        "#,
    );

    assert_eq!(module.tests.len(), 1);
    let mut vm = VM::new(module.clone());
    vm.set_emit_handler(|_, _| {});
    let result = vm.run_test(&module.tests[0]);
    assert!(result.is_ok(), "test should pass: {:?}", result.err());
}

#[test]
fn e2e_test_assert_failing() {
    let module = compile_for_tests(
        r#"
        @test
        fn failing_assertion() {
            assert_eq(1, 2);
        }
        "#,
    );

    let mut vm = VM::new(module.clone());
    vm.set_emit_handler(|_, _| {});
    let result = vm.run_test(&module.tests[0]);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("1 != 2"),
        "error should show values: {}",
        err_msg
    );
}

#[test]
fn e2e_test_emit_capture() {
    let module = compile_for_tests(
        r#"
        @test
        fn emit_capture() {
            emit("greeting", "hello");
            emit("farewell", "goodbye");

            let emits = test_emits();
            assert_eq(len(emits), 2);
            assert_eq(emits[0].channel, "greeting");
            assert_eq(emits[0].payload, "hello");
            assert_eq(emits[1].channel, "farewell");
            assert_eq(emits[1].payload, "goodbye");
        }
        "#,
    );

    let mut vm = VM::new(module.clone());
    vm.set_emit_handler(|_, _| {});
    let result = vm.run_test(&module.tests[0]);
    assert!(result.is_ok(), "test should pass: {:?}", result.err());
}

#[test]
fn e2e_test_mock_agent() {
    let module = compile_for_tests_with_connections(
        r#"
        model Greeter {
            provider: openai,
            base: "gpt-4o",
            system_prompt: "You greet people.",
        }

        @test
        fn mock_agent_execute() {
            mock Greeter {
                response: "Hello from mock!",
            }

            let result = Greeter.execute("Hi");
            assert(result.is_ok());
        }
        "#,
        &["openai"],
    );

    assert_eq!(module.tests.len(), 1);
    let mut vm = VM::new(module.clone());
    vm.set_emit_handler(|_, _| {});
    let result = vm.run_test(&module.tests[0]);
    assert!(result.is_ok(), "test should pass: {:?}", result.err());
}

#[test]
fn e2e_test_mock_agent_error() {
    let module = compile_for_tests_with_connections(
        r#"
        model MyAgent {
            provider: openai,
            base: "gpt-4o",
            system_prompt: "test",
        }

        @test
        fn mock_agent_error() {
            mock MyAgent {
                error: "API rate limit exceeded",
            }

            let result = MyAgent.execute("do something");
            assert(result.is_err());
        }
        "#,
        &["openai"],
    );

    let mut vm = VM::new(module.clone());
    vm.set_emit_handler(|_, _| {});
    let result = vm.run_test(&module.tests[0]);
    assert!(result.is_ok(), "test should pass: {:?}", result.err());
}

#[test]
fn e2e_test_isolation() {
    // Each test gets fresh VM state — mocks and emits don't leak
    let module = compile_for_tests(
        r#"
        @test
        fn first_test_emits() {
            emit("ch1", "val1");
            let emits = test_emits();
            assert_eq(len(emits), 1);
        }

        @test
        fn second_test_sees_no_emits() {
            let emits = test_emits();
            assert_eq(len(emits), 0);
        }
        "#,
    );

    assert_eq!(module.tests.len(), 2);

    for test in &module.tests {
        let mut vm = VM::new(module.clone());
        vm.set_emit_handler(|_, _| {});
        let result = vm.run_test(test);
        assert!(
            result.is_ok(),
            "test '{}' should pass: {:?}",
            test.description,
            result.err()
        );
    }
}

#[test]
fn e2e_test_expect_fail() {
    let module = compile_for_tests(
        r#"
        @test
        @expect_fail
        fn should_panic() {
            panic("boom");
        }
        "#,
    );

    assert_eq!(module.tests.len(), 1);
    assert!(module.tests[0].expect_fail);
    assert!(module.tests[0].expect_fail_message.is_none());

    let mut vm = VM::new(module.clone());
    vm.set_emit_handler(|_, _| {});
    let result = vm.run_test(&module.tests[0]);
    // The test itself should fail (panic), but expect_fail means it's expected
    assert!(result.is_err(), "test should fail with panic");
}

#[test]
fn e2e_test_expect_fail_with_message() {
    let module = compile_for_tests(
        r#"
        @test
        @expect_fail("assertion failed")
        fn expects_specific_error() {
            assert_eq(1, 2);
        }
        "#,
    );

    assert_eq!(module.tests.len(), 1);
    assert!(module.tests[0].expect_fail);
    assert_eq!(
        module.tests[0].expect_fail_message.as_deref(),
        Some("assertion failed")
    );

    let mut vm = VM::new(module.clone());
    vm.set_emit_handler(|_, _| {});
    let result = vm.run_test(&module.tests[0]);
    assert!(result.is_err(), "test should fail with assertion error");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("assertion failed"),
        "error should contain expected message: {}",
        err_msg
    );
}

#[test]
fn e2e_test_expect_fail_unexpected_pass() {
    let module = compile_for_tests(
        r#"
        @test
        @expect_fail
        fn should_fail_but_passes() {
            assert(true);
        }
        "#,
    );

    assert_eq!(module.tests.len(), 1);
    assert!(module.tests[0].expect_fail);

    let mut vm = VM::new(module.clone());
    vm.set_emit_handler(|_, _| {});
    let result = vm.run_test(&module.tests[0]);
    // This test passes but was expected to fail — we just verify the IR metadata
    assert!(result.is_ok(), "test should pass (unexpectedly)");
}

#[test]
fn e2e_test_description_from_decorator() {
    let module = compile_for_tests(
        r#"
        @test("descriptive test name")
        fn my_test_fn() {
            assert(true);
        }
        "#,
    );

    assert_eq!(module.tests.len(), 1);
    assert_eq!(module.tests[0].description, "descriptive test name");
}

// =========================================================================
// Spec 29: Agent initialization params in IR
// =========================================================================

#[test]
fn e2e_agent_params_none_without_manifest() {
    // Agent declaration without manifest should have params: None in IR
    let source = r#"
        agent MyAgent {
            connector: "test_service",
            input_format: "json",
            output_format: "json",
            timeout: 60,
        }

        fn main() {
            emit("ok", "yes");
        }
    "#;

    let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
    assert!(!lex_diags.has_errors());
    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    assert!(!parse_diags.has_errors());
    let sem_diags = concerto_compiler::semantic::analyze(&program);
    assert!(!sem_diags.has_errors());

    let ir = CodeGenerator::new("test", "test.conc").generate(&program);
    assert_eq!(ir.agents.len(), 1);
    assert_eq!(ir.agents[0].name, "MyAgent");
    assert!(ir.agents[0].params.is_none(), "params should be None without manifest");
}

// =========================================================================
// Spec 30: Pipeline type contracts
// =========================================================================

#[test]
fn e2e_pipeline_with_signature_compiles() {
    // Pipeline with signature compiles and loads successfully
    let source = r#"
        pipeline TextPipeline(input: String) -> Int {
            stage parse(data: String) -> Int {
                return 42;
            }
        }

        fn main() {
            emit("ok", "compiled");
        }
    "#;

    let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
    assert!(!lex_diags.has_errors());
    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    assert!(!parse_diags.has_errors(), "parse errors: {:?}", parse_diags.diagnostics());
    let sem_diags = concerto_compiler::semantic::analyze(&program);
    assert!(!sem_diags.has_errors(), "semantic errors: {:?}", sem_diags.diagnostics());

    let ir = CodeGenerator::new("test", "test.conc").generate(&program);

    // Verify pipeline signature in IR
    assert_eq!(ir.pipelines.len(), 1);
    assert!(ir.pipelines[0].input_type.is_some(), "should have input_type");
    assert!(ir.pipelines[0].output_type.is_some(), "should have output_type");

    // Round-trip through JSON
    let json = serde_json::to_string(&ir).expect("IR serialization failed");
    let ir_module: concerto_common::ir::IrModule =
        serde_json::from_str(&json).expect("IR deserialization failed");
    let module = LoadedModule::from_ir(ir_module).expect("IR loading failed");

    let emits: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let emits_clone = emits.clone();
    let mut vm = VM::new(module);
    vm.set_emit_handler(move |channel, payload| {
        emits_clone.lock().unwrap().push((channel.to_string(), payload.display_string()));
    });

    let result = vm.execute();
    assert!(result.is_ok(), "execution failed: {:?}", result.err());
}

#[test]
fn e2e_pipeline_adjacency_type_error() {
    // Pipeline with mismatched stage types should produce a compile error
    let source = r#"
        pipeline Bad {
            stage a(x: String) -> Int {
                return 42;
            }
            stage b(y: String) -> String {
                return y;
            }
        }
        fn main() {}
    "#;

    let (tokens, lex_diags) = Lexer::new(source, "test.conc").tokenize();
    assert!(!lex_diags.has_errors());
    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    assert!(!parse_diags.has_errors());

    // Validator should catch type mismatch
    let validator_diags = concerto_compiler::semantic::validator::Validator::new().validate(&program);
    assert!(
        validator_diags.has_errors(),
        "should have type mismatch error"
    );
    let errors: Vec<_> = validator_diags
        .diagnostics()
        .iter()
        .filter(|d| d.severity == concerto_common::Severity::Error)
        .map(|d| d.message.clone())
        .collect();
    assert!(
        errors.iter().any(|e| e.contains("type mismatch")),
        "expected type mismatch error, got: {:?}",
        errors
    );
}
