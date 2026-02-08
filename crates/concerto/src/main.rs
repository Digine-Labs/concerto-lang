use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use clap::Parser;
use concerto_runtime::{LoadedModule, VM};

/// Concerto language runtime — executes compiled .conc-ir files.
#[derive(Parser)]
#[command(
    name = "concerto",
    version,
    about,
    long_about = "Concerto language runtime.\n\nExecutes compiled .conc-ir files produced by the Concerto compiler (concertoc).\n\nExamples:\n  concerto run hello.conc-ir            Run a compiled program\n  concerto run hello.conc-ir --debug    Run with debug output\n  concerto run hello.conc-ir --quiet    Run without emit output\n  concerto init my-project              Create a new Concerto project"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Execute a compiled .conc-ir file
    Run {
        /// Path to the .conc-ir file
        input: PathBuf,

        /// Enable debug output (show stack trace on error)
        #[arg(long)]
        debug: bool,

        /// Suppress emit output
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Run {
            input,
            debug,
            quiet,
        } => {
            let path = input.to_string_lossy().to_string();

            let module = match LoadedModule::load_from_file(&path) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("error: failed to load IR: {}", e);
                    process::exit(1);
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

        Command::Init { name, provider } => {
            if let Err(msg) = run_init(&name, &provider) {
                eprintln!("{}", msg);
                process::exit(1);
            }
        }
    }
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

    println!("  concertoc src/main.conc");
    println!("  concerto run src/main.conc-ir");

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
