use std::path::PathBuf;
use std::process;

use clap::Parser;
use concerto_runtime::{LoadedModule, VM};

/// Concerto language runtime — executes compiled .conc-ir files.
#[derive(Parser)]
#[command(name = "concerto", version, about, long_about = "Concerto language runtime.\n\nExecutes compiled .conc-ir files produced by the Concerto compiler (concertoc).\n\nExamples:\n  concerto run hello.conc-ir            Run a compiled program\n  concerto run hello.conc-ir --debug    Run with debug output\n  concerto run hello.conc-ir --quiet    Run without emit output")]
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
    }
}
