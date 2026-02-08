use std::fs;
use std::path::PathBuf;
use std::process;

use clap::Parser;

use concerto_compiler::codegen::CodeGenerator;
use concerto_compiler::lexer::Lexer;
use concerto_compiler::parser;

/// Concerto language compiler.
///
/// Compiles .conc source files to .conc-ir (JSON IR) files.
#[derive(Parser)]
#[command(name = "concertoc", version, about)]
struct Cli {
    /// Input .conc source file.
    input: PathBuf,

    /// Output file path (default: <input>.conc-ir).
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Check for errors without generating IR.
    #[arg(long)]
    check: bool,

    /// Emit token stream to stdout (debug).
    #[arg(long = "emit-tokens")]
    emit_tokens: bool,

    /// Emit AST to stdout (debug).
    #[arg(long = "emit-ast")]
    emit_ast: bool,
}

fn main() {
    let cli = Cli::parse();

    // Read source file
    let source = match fs::read_to_string(&cli.input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "error: could not read '{}': {}",
                cli.input.display(),
                e
            );
            process::exit(1);
        }
    };

    let file_name = cli
        .input
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // === Lexer ===
    let (tokens, lex_diags) = Lexer::new(&source, &file_name).tokenize();

    if lex_diags.has_errors() {
        for diag in lex_diags.diagnostics() {
            print_diagnostic(diag, &source, &file_name);
        }
        process::exit(1);
    }

    if cli.emit_tokens {
        for token in &tokens {
            println!(
                "{:>4}:{:<3} {:?} {:?}",
                token.span.start.line,
                token.span.start.column,
                token.kind,
                token.lexeme,
            );
        }
        if cli.check {
            println!("\nNo lexer errors.");
        }
        return;
    }

    // === Parser ===
    let (program, parse_diags) = parser::Parser::new(tokens).parse();

    if parse_diags.has_errors() {
        for diag in parse_diags.diagnostics() {
            print_diagnostic(diag, &source, &file_name);
        }
        process::exit(1);
    }

    // Print warnings
    for diag in parse_diags.diagnostics() {
        if !diag.is_error() {
            print_diagnostic(diag, &source, &file_name);
        }
    }

    if cli.emit_ast {
        println!("{:#?}", program);
        return;
    }

    // === Semantic Analysis ===
    let sem_diags = concerto_compiler::semantic::analyze(&program);

    if sem_diags.has_errors() {
        for diag in sem_diags.diagnostics() {
            print_diagnostic(diag, &source, &file_name);
        }
        process::exit(1);
    }

    // Print semantic warnings
    for diag in sem_diags.diagnostics() {
        if !diag.is_error() {
            print_diagnostic(diag, &source, &file_name);
        }
    }

    if cli.check {
        println!("No errors found.");
        return;
    }

    // === IR Generation ===
    let module_name = cli
        .input
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let ir = CodeGenerator::new(&module_name, &file_name).generate(&program);

    let json = match serde_json::to_string_pretty(&ir) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("error: failed to serialize IR: {}", e);
            process::exit(1);
        }
    };

    // Determine output path
    let output_path = cli.output.unwrap_or_else(|| {
        let mut p = cli.input.clone();
        p.set_extension("conc-ir");
        p
    });

    match fs::write(&output_path, &json) {
        Ok(()) => {
            println!(
                "Compiled {} -> {} ({} bytes)",
                cli.input.display(),
                output_path.display(),
                json.len()
            );
        }
        Err(e) => {
            eprintln!(
                "error: could not write '{}': {}",
                output_path.display(),
                e
            );
            process::exit(1);
        }
    }
}

fn print_diagnostic(
    diag: &concerto_common::Diagnostic,
    source: &str,
    file_name: &str,
) {
    let prefix = if diag.is_error() { "error" } else { "warning" };

    if let Some(ref span) = diag.span {
        eprintln!(
            "{}: {}",
            prefix, diag.message
        );
        eprintln!(
            "  --> {}:{}:{}",
            file_name, span.start.line, span.start.column
        );

        // Show the source line
        if let Some(line) = source.lines().nth(span.start.line as usize - 1) {
            eprintln!("   |");
            eprintln!("{:>3} | {}", span.start.line, line);
            eprintln!(
                "   | {}{}",
                " ".repeat(span.start.column as usize - 1),
                "^".repeat(
                    (span.end.column.saturating_sub(span.start.column)).max(1) as usize
                )
            );
        }
    } else {
        eprintln!("{}: {}", prefix, diag.message);
    }

    if let Some(ref suggestion) = diag.suggestion {
        eprintln!("   = help: {}", suggestion);
    }

    eprintln!();
}
