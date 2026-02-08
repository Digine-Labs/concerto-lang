use std::fs;
use std::path::PathBuf;
use std::process;

use ariadne::{Color, Label, Report, ReportKind, Source};
use clap::Parser;

use concerto_compiler::codegen::CodeGenerator;
use concerto_compiler::lexer::Lexer;
use concerto_compiler::parser;

/// Concerto language compiler.
///
/// Compiles .conc source files to .conc-ir (JSON IR) files.
#[derive(Parser)]
#[command(name = "concertoc", version, about, long_about = "Concerto language compiler.\n\nCompiles .conc source files into .conc-ir (JSON intermediate representation)\nfor execution by the Concerto runtime (concerto run).\n\nExamples:\n  concertoc hello.conc              Compile to hello.conc-ir\n  concertoc hello.conc -o out.ir    Compile to custom output path\n  concertoc hello.conc --check      Check for errors only\n  concertoc hello.conc --emit-ir    Print IR JSON to stdout")]
struct Cli {
    /// Input .conc source file.
    input: PathBuf,

    /// Output file path (default: <input>.conc-ir).
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Check for errors without generating IR.
    #[arg(long)]
    check: bool,

    /// Suppress warning output.
    #[arg(short, long)]
    quiet: bool,

    /// Emit IR JSON to stdout instead of writing to file.
    #[arg(long = "emit-ir")]
    emit_ir: bool,

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
    if !cli.quiet {
        for diag in parse_diags.diagnostics() {
            if !diag.is_error() {
                print_diagnostic(diag, &source, &file_name);
            }
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
    if !cli.quiet {
        for diag in sem_diags.diagnostics() {
            if !diag.is_error() {
                print_diagnostic(diag, &source, &file_name);
            }
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

    // --emit-ir: print JSON to stdout
    if cli.emit_ir {
        println!("{}", json);
        return;
    }

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
    let kind = if diag.is_error() {
        ReportKind::Error
    } else {
        ReportKind::Warning
    };

    if let Some(ref span) = diag.span {
        let start = span.start.offset as usize;
        let end = (span.end.offset as usize).max(start + 1);

        let color = if diag.is_error() {
            Color::Red
        } else {
            Color::Yellow
        };

        let mut report = Report::build(kind, file_name, start)
            .with_message(&diag.message)
            .with_label(
                Label::new((file_name, start..end))
                    .with_message(&diag.message)
                    .with_color(color),
            );

        for related in &diag.related {
            let rs = related.span.start.offset as usize;
            let re = (related.span.end.offset as usize).max(rs + 1);
            report = report.with_label(
                Label::new((file_name, rs..re))
                    .with_message(&related.message)
                    .with_color(Color::Blue),
            );
        }

        if let Some(ref suggestion) = diag.suggestion {
            report = report.with_help(suggestion);
        }

        report
            .finish()
            .eprint((file_name, Source::from(source)))
            .unwrap();
    } else {
        let prefix = if diag.is_error() { "error" } else { "warning" };
        eprintln!("{}: {}", prefix, diag.message);
        if let Some(ref suggestion) = diag.suggestion {
            eprintln!("   = help: {}", suggestion);
        }
        eprintln!();
    }
}
