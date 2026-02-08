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

        agent Assistant {
            provider: openai,
            model: "gpt-4o-mini",
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

        agent Worker {
            provider: openai,
            model: "gpt-4o-mini",
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
fn e2e_host_declaration() {
    // Host declarations compile and load successfully.
    // We can't execute without a real subprocess, but verify the pipeline works.
    let source = r#"
        host EchoHost {
            connector: "echo_service",
            input_format: "text",
            output_format: "text",
            timeout: 30,
        }

        fn main() {
            emit("has_host", "yes");
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

    // Verify host appears in IR
    assert_eq!(ir.hosts.len(), 1);
    assert_eq!(ir.hosts[0].name, "EchoHost");
    assert_eq!(ir.hosts[0].connector, "echo_service");
    assert_eq!(ir.hosts[0].input_format, "text");
    assert_eq!(ir.hosts[0].output_format, "text");
    assert_eq!(ir.hosts[0].timeout, Some(30));

    // Load and execute
    let json = serde_json::to_string(&ir).expect("IR serialization failed");
    let ir_module: concerto_common::ir::IrModule =
        serde_json::from_str(&json).expect("IR deserialization failed");
    let module = LoadedModule::from_ir(ir_module).expect("IR loading failed");

    let mut vm = VM::new(module);
    let result = vm.execute();
    assert!(result.is_ok());
}
