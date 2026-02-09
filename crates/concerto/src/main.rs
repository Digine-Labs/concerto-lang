use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use clap::Parser;
use concerto_runtime::{LoadedModule, VM};

/// Concerto language runtime — executes .conc source files or compiled .conc-ir files.
#[derive(Parser)]
#[command(
    name = "concerto",
    version,
    about,
    long_about = "Concerto language runtime.\n\nRuns Concerto programs from source (.conc) or compiled IR (.conc-ir) files.\nWhen given a .conc file, it compiles in-memory and executes directly.\n\nExamples:\n  concerto run src/main.conc            Compile and run in one step\n  concerto run hello.conc-ir            Run a pre-compiled program\n  concerto run src/main.conc --debug    Run with debug output\n  concerto run src/main.conc --quiet    Run without emit output\n  concerto test src/main.conc           Run tests in a source file\n  concerto test src/main.conc --filter \"auth\"  Run matching tests\n  concerto init my-project              Create a new Concerto project"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Execute a .conc source file or compiled .conc-ir file
    Run {
        /// Path to the .conc or .conc-ir file
        input: PathBuf,

        /// Enable debug output (show stack trace on error)
        #[arg(long)]
        debug: bool,

        /// Suppress emit output
        #[arg(short, long)]
        quiet: bool,
    },

    /// Run tests in a .conc source file
    Test {
        /// Path to the .conc file containing tests
        input: PathBuf,

        /// Only run tests matching this pattern
        #[arg(short, long)]
        filter: Option<String>,

        /// Show error details on failure
        #[arg(long)]
        debug: bool,

        /// Show only summary
        #[arg(short, long)]
        quiet: bool,
    },

    /// Create a new Concerto project
    Init {
        /// Project name or '.' for current directory
        name: String,

        /// Default LLM provider (openai, anthropic, ollama)
        #[arg(short, long, default_value = "openai")]
        provider: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Run {
            input,
            debug,
            quiet,
        } => {
            let path_str = input.to_string_lossy().to_string();

            let module = if is_source_file(&input) {
                // Direct run: compile .conc in-memory, then execute
                match compile_source(&input, quiet) {
                    Ok(m) => m,
                    Err(msg) => {
                        eprintln!("{}", msg);
                        process::exit(1);
                    }
                }
            } else {
                // Legacy path: load pre-compiled .conc-ir
                match LoadedModule::load_from_file(&path_str) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("error: failed to load IR: {}", e);
                        process::exit(1);
                    }
                }
            };

            let mut vm = VM::new(module);

            if quiet {
                vm.set_emit_handler(|_channel, _payload| {});
            }

            match vm.execute() {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("runtime error: {}", e);
                    if debug {
                        eprintln!("  in function: {}", vm.current_function_name());
                    }
                    process::exit(1);
                }
            }
        }

        Command::Test {
            input,
            filter,
            debug,
            quiet,
        } => {
            if let Err(code) = run_tests(&input, filter.as_deref(), debug, quiet) {
                process::exit(code);
            }
        }

        Command::Init { name, provider } => {
            if let Err(msg) = run_init(&name, &provider) {
                eprintln!("{}", msg);
                process::exit(1);
            }
        }
    }
}

// ============================================================================
// Direct .conc compilation
// ============================================================================

/// Check if the input file is a .conc source file (not .conc-ir).
fn is_source_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    ext == "conc"
}

/// Compile a .conc source file in-memory and return a LoadedModule.
fn compile_source(path: &Path, quiet: bool) -> Result<LoadedModule, String> {
    use concerto_common::ir::IrConnection;
    use concerto_common::manifest;
    use concerto_compiler::codegen::CodeGenerator;
    use concerto_compiler::lexer::Lexer;
    use concerto_compiler::parser;

    // Read source
    let source = fs::read_to_string(path)
        .map_err(|e| format!("error: could not read '{}': {}", path.display(), e))?;

    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Find and load Concerto.toml
    let abs_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let (connection_names, ir_connections, manifest_hosts) =
        match manifest::find_and_load_manifest(&abs_path) {
            Ok(m) => {
                let names: Vec<String> = m.connections.keys().cloned().collect();
                let ir_conns: Vec<IrConnection> = m
                    .connections
                    .iter()
                    .map(|(name, cfg)| IrConnection {
                        name: name.clone(),
                        config: cfg.to_ir_config(),
                    })
                    .collect();
                let hosts = m.hosts.clone();
                (names, ir_conns, hosts)
            }
            Err(manifest::ManifestError::NotFound(_)) => {
                (Vec::new(), Vec::new(), std::collections::HashMap::new())
            }
            Err(e) => return Err(format!("error: {}", e)),
        };

    // Lex
    let (tokens, lex_diags) = Lexer::new(&source, &file_name).tokenize();
    if lex_diags.has_errors() {
        let mut msg = String::new();
        for diag in lex_diags.diagnostics() {
            msg.push_str(&format_diagnostic(diag, &source, &file_name));
        }
        return Err(msg);
    }

    // Parse
    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    if parse_diags.has_errors() {
        let mut msg = String::new();
        for diag in parse_diags.diagnostics() {
            msg.push_str(&format_diagnostic(diag, &source, &file_name));
        }
        return Err(msg);
    }
    if !quiet {
        for diag in parse_diags.diagnostics() {
            if !diag.is_error() {
                eprint!("{}", format_diagnostic(diag, &source, &file_name));
            }
        }
    }

    // Semantic analysis
    let sem_diags =
        concerto_compiler::semantic::analyze_with_connections(&program, &connection_names);
    if sem_diags.has_errors() {
        let mut msg = String::new();
        for diag in sem_diags.diagnostics() {
            msg.push_str(&format_diagnostic(diag, &source, &file_name));
        }
        return Err(msg);
    }
    if !quiet {
        for diag in sem_diags.diagnostics() {
            if !diag.is_error() {
                eprint!("{}", format_diagnostic(diag, &source, &file_name));
            }
        }
    }

    // Codegen
    let module_name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut codegen = CodeGenerator::new(&module_name, &file_name);
    codegen.add_manifest_connections(ir_connections);
    let mut ir = codegen.generate(&program);

    if !manifest_hosts.is_empty() {
        CodeGenerator::embed_manifest_hosts(&mut ir, &manifest_hosts);
    }

    // Convert IR to LoadedModule in-memory (no file I/O)
    let json = serde_json::to_string(&ir)
        .map_err(|e| format!("error: failed to serialize IR: {}", e))?;
    let ir_module: concerto_common::ir::IrModule = serde_json::from_str(&json)
        .map_err(|e| format!("error: failed to deserialize IR: {}", e))?;
    LoadedModule::from_ir(ir_module)
        .map_err(|e| format!("error: failed to load IR module: {}", e))
}

/// Format a diagnostic as a simple text message (no ariadne colors in direct run).
fn format_diagnostic(
    diag: &concerto_common::Diagnostic,
    _source: &str,
    file_name: &str,
) -> String {
    let prefix = if diag.is_error() { "error" } else { "warning" };
    if let Some(ref span) = diag.span {
        let mut msg = format!(
            "{}:{}:{}: {}: {}\n",
            file_name, span.start.line, span.start.column, prefix, diag.message
        );
        if let Some(ref suggestion) = diag.suggestion {
            msg.push_str(&format!("  = help: {}\n", suggestion));
        }
        msg
    } else {
        let mut msg = format!("{}: {}\n", prefix, diag.message);
        if let Some(ref suggestion) = diag.suggestion {
            msg.push_str(&format!("  = help: {}\n", suggestion));
        }
        msg
    }
}

// ============================================================================
// concerto test
// ============================================================================

fn run_tests(input: &Path, filter: Option<&str>, debug: bool, quiet: bool) -> Result<(), i32> {
    // Compile source for tests (permissive — no entry point required)
    let module = match compile_source_for_tests(input, quiet) {
        Ok(m) => m,
        Err(msg) => {
            eprintln!("{}", msg);
            return Err(1);
        }
    };

    // Filter tests by description
    let tests: Vec<_> = module
        .tests
        .iter()
        .filter(|t| {
            if let Some(f) = filter {
                t.description.contains(f)
            } else {
                true
            }
        })
        .collect();

    if tests.is_empty() {
        if filter.is_some() {
            eprintln!("no tests matching filter");
        } else {
            eprintln!("no tests found");
        }
        return Err(1);
    }

    if !quiet {
        println!("running {} tests\n", tests.len());
    }

    let mut passed = 0;
    let mut failed = 0;
    let mut failures: Vec<(String, String)> = Vec::new();

    for test in &tests {
        // Each test gets a fresh VM instance
        let mut vm = VM::new(module.clone());

        // Suppress emit output during tests unless debug mode
        if !debug {
            vm.set_emit_handler(|_channel, _payload| {});
        }

        let result = vm.run_test(test);

        if test.expect_fail {
            match result {
                Ok(_) => {
                    // Expected failure but test passed
                    failed += 1;
                    if !quiet {
                        println!("  \x1b[31mFAIL\x1b[0m  {}", test.description);
                    }
                    failures.push((
                        test.description.clone(),
                        "expected failure but test passed".to_string(),
                    ));
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    if let Some(ref expected_msg) = test.expect_fail_message {
                        if err_msg.contains(expected_msg.as_str()) {
                            passed += 1;
                            if !quiet {
                                println!("  \x1b[32mPASS\x1b[0m  {} (expected failure)", test.description);
                            }
                        } else {
                            failed += 1;
                            if !quiet {
                                println!("  \x1b[31mFAIL\x1b[0m  {}", test.description);
                            }
                            failures.push((
                                test.description.clone(),
                                format!(
                                    "expected error containing '{}', got: {}",
                                    expected_msg, err_msg
                                ),
                            ));
                        }
                    } else {
                        // Any failure is acceptable
                        passed += 1;
                        if !quiet {
                            println!("  \x1b[32mPASS\x1b[0m  {} (expected failure)", test.description);
                        }
                    }
                }
            }
        } else {
            match result {
                Ok(_) => {
                    passed += 1;
                    if !quiet {
                        println!("  \x1b[32mPASS\x1b[0m  {}", test.description);
                    }
                }
                Err(e) => {
                    failed += 1;
                    let err_msg = e.to_string();
                    if !quiet {
                        println!("  \x1b[31mFAIL\x1b[0m  {}", test.description);
                    }
                    failures.push((test.description.clone(), err_msg));
                }
            }
        }
    }

    // Print summary
    println!();
    if failed == 0 {
        println!(
            "test result: \x1b[32mok\x1b[0m. {} passed, 0 failed",
            passed
        );
        Ok(())
    } else {
        println!(
            "test result: \x1b[31mFAILED\x1b[0m. {} passed, {} failed",
            passed, failed
        );
        println!("\nfailures:");
        for (desc, err) in &failures {
            println!("  {} -- {}", desc, err);
        }
        Err(1)
    }
}

/// Compile a .conc source file for test execution (permissive loading).
fn compile_source_for_tests(path: &Path, quiet: bool) -> Result<LoadedModule, String> {
    use concerto_common::ir::IrConnection;
    use concerto_common::manifest;
    use concerto_compiler::codegen::CodeGenerator;
    use concerto_compiler::lexer::Lexer;
    use concerto_compiler::parser;

    let source = fs::read_to_string(path)
        .map_err(|e| format!("error: could not read '{}': {}", path.display(), e))?;

    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let abs_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let (connection_names, ir_connections, manifest_hosts) =
        match manifest::find_and_load_manifest(&abs_path) {
            Ok(m) => {
                let names: Vec<String> = m.connections.keys().cloned().collect();
                let ir_conns: Vec<IrConnection> = m
                    .connections
                    .iter()
                    .map(|(name, cfg)| IrConnection {
                        name: name.clone(),
                        config: cfg.to_ir_config(),
                    })
                    .collect();
                let hosts = m.hosts.clone();
                (names, ir_conns, hosts)
            }
            Err(manifest::ManifestError::NotFound(_)) => {
                (Vec::new(), Vec::new(), std::collections::HashMap::new())
            }
            Err(e) => return Err(format!("error: {}", e)),
        };

    let (tokens, lex_diags) = Lexer::new(&source, &file_name).tokenize();
    if lex_diags.has_errors() {
        let mut msg = String::new();
        for diag in lex_diags.diagnostics() {
            msg.push_str(&format_diagnostic(diag, &source, &file_name));
        }
        return Err(msg);
    }

    let (program, parse_diags) = parser::Parser::new(tokens).parse();
    if parse_diags.has_errors() {
        let mut msg = String::new();
        for diag in parse_diags.diagnostics() {
            msg.push_str(&format_diagnostic(diag, &source, &file_name));
        }
        return Err(msg);
    }
    if !quiet {
        for diag in parse_diags.diagnostics() {
            if !diag.is_error() {
                eprint!("{}", format_diagnostic(diag, &source, &file_name));
            }
        }
    }

    let sem_diags =
        concerto_compiler::semantic::analyze_with_connections(&program, &connection_names);
    if sem_diags.has_errors() {
        let mut msg = String::new();
        for diag in sem_diags.diagnostics() {
            msg.push_str(&format_diagnostic(diag, &source, &file_name));
        }
        return Err(msg);
    }
    if !quiet {
        for diag in sem_diags.diagnostics() {
            if !diag.is_error() {
                eprint!("{}", format_diagnostic(diag, &source, &file_name));
            }
        }
    }

    let module_name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut codegen = CodeGenerator::new(&module_name, &file_name);
    codegen.add_manifest_connections(ir_connections);
    let mut ir = codegen.generate(&program);

    if !manifest_hosts.is_empty() {
        CodeGenerator::embed_manifest_hosts(&mut ir, &manifest_hosts);
    }

    let json = serde_json::to_string(&ir)
        .map_err(|e| format!("error: failed to serialize IR: {}", e))?;
    let ir_module: concerto_common::ir::IrModule = serde_json::from_str(&json)
        .map_err(|e| format!("error: failed to deserialize IR: {}", e))?;

    // Use permissive loading — test-only files may not have main()
    LoadedModule::from_ir_permissive(ir_module)
        .map_err(|e| format!("error: failed to load IR module: {}", e))
}

// ============================================================================
// concerto init
// ============================================================================

fn run_init(name: &str, provider: &str) -> Result<(), String> {
    // Validate provider
    let (conn_name, api_key_env, default_model, base_url) = match provider {
        "openai" => ("openai", Some("OPENAI_API_KEY"), "gpt-4o-mini", None),
        "anthropic" => (
            "anthropic",
            Some("ANTHROPIC_API_KEY"),
            "claude-sonnet-4-20250514",
            None,
        ),
        "ollama" => ("local", None, "llama3.1", Some("http://localhost:11434/v1")),
        _ => {
            return Err(format!(
                "error: unknown provider '{}'. Valid providers: openai, anthropic, ollama",
                provider
            ));
        }
    };

    // Determine project directory and name
    let (project_dir, project_name) = if name == "." {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("error: cannot determine current directory: {}", e))?;
        let dir_name = cwd
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        (cwd, dir_name)
    } else {
        (PathBuf::from(name), name.to_string())
    };

    // Check if Concerto.toml already exists
    if project_dir.join("Concerto.toml").exists() {
        return Err(
            "error: Concerto project already exists in this directory\n  = help: remove Concerto.toml to reinitialize, or use a different directory".to_string()
        );
    }

    // Create project directory if needed
    if name != "." {
        fs::create_dir_all(&project_dir)
            .map_err(|e| format!("error: failed to create directory '{}': {}", name, e))?;
    }

    // Create src/ directory
    fs::create_dir_all(project_dir.join("src"))
        .map_err(|e| format!("error: failed to create 'src/': {}", e))?;

    // Generate and write Concerto.toml
    let toml_content = generate_toml(
        &project_name,
        provider,
        conn_name,
        api_key_env,
        default_model,
        base_url,
    );
    write_file(&project_dir.join("Concerto.toml"), &toml_content)?;

    // Generate and write src/main.conc
    let main_content = generate_main_conc(conn_name, default_model);
    write_file(&project_dir.join("src/main.conc"), &main_content)?;

    // Generate and write .gitignore
    let gitignore_content = "# Compiled IR\n*.conc-ir\n\n# Environment secrets\n.env\n";
    write_file(&project_dir.join(".gitignore"), gitignore_content)?;

    // Print success message
    println!("Created Concerto project \"{}\"", project_name);
    println!("  Concerto.toml");
    println!("  src/main.conc");
    println!("  .gitignore");
    println!();
    println!("Get started:");

    if name != "." {
        println!("  cd {}", name);
    }

    match provider {
        "openai" => println!("  export OPENAI_API_KEY=\"your-key\""),
        "anthropic" => println!("  export ANTHROPIC_API_KEY=\"your-key\""),
        "ollama" => println!("  ollama serve"),
        _ => {}
    }

    println!("  concerto run src/main.conc");

    Ok(())
}

fn write_file(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content)
        .map_err(|e| format!("error: failed to write '{}': {}", path.display(), e))
}

fn generate_toml(
    project_name: &str,
    provider: &str,
    conn_name: &str,
    api_key_env: Option<&str>,
    default_model: &str,
    base_url: Option<&str>,
) -> String {
    let mut toml = format!(
        "[project]\nname = \"{}\"\nversion = \"0.1.0\"\nentry = \"src/main.conc\"\n",
        project_name
    );

    toml.push_str(&format!("\n[connections.{}]\n", conn_name));
    toml.push_str(&format!("provider = \"{}\"\n", provider));

    if let Some(key_env) = api_key_env {
        toml.push_str(&format!("api_key_env = \"{}\"\n", key_env));
    }

    if let Some(url) = base_url {
        toml.push_str(&format!("base_url = \"{}\"\n", url));
    }

    toml.push_str(&format!("default_model = \"{}\"\n", default_model));

    toml
}

fn generate_main_conc(conn_name: &str, default_model: &str) -> String {
    format!(
        r#"schema Greeting {{
    message: String,
    language: String,
}}

agent Greeter {{
    provider: {conn_name},
    model: "{default_model}",
    temperature: 0.7,
    system_prompt: "You are a friendly multilingual greeter. Always respond with valid JSON.",
}}

fn main() {{
    let result = Greeter.execute_with_schema<Greeting>(
        "Say hello in French. Return JSON with 'message' and 'language' fields."
    );

    match result {{
        Ok(greeting) => emit("greeting", {{
            "message": greeting.message,
            "language": greeting.language,
        }}),
        Err(e) => emit("error", e.message),
    }}
}}
"#,
        conn_name = conn_name,
        default_model = default_model,
    )
}
