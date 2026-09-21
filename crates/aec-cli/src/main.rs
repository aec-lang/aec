use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use std::fs;
use std::path::PathBuf;

use aec_ast::TopLevelItem;

mod render;
use render::{render as render_diag, Level};

#[derive(Parser)]
#[command(name = "aec")]
#[command(version = "0.1.0")]
#[command(about = "AEC — Agent Easy Creator", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check if an AEC file parses correctly
    Check {
        file: PathBuf,
    },
    /// Print the AST of an AEC file
    Ast {
        file: PathBuf,
    },
    /// Run an AEC file
    Run {
        file: PathBuf,
        /// Function to call (default: main)
        #[arg(short, long, default_value = "main")]
        entry: String,
        /// Force terminal mode (ignore UI)
        #[arg(long)]
        cli: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { file } => cmd_check(&file),
        Commands::Ast { file } => cmd_ast(&file),
        Commands::Run { file, entry, cli: force_cli } => cmd_run(&file, &entry, force_cli),
    }
}

/// A readable message for a parse error (without a duplicated prefix).
fn parse_error_message(kind: &aec_ast::ParseErrorKind) -> String {
    match kind {
        aec_ast::ParseErrorKind::BuildError { message } => message.clone(),
        other => other.to_string(),
    }
}

/// Prints one diagnostic with its source snippet and caret.
fn print_diag(source: &str, level: Level, message: &str, span: aec_ast::Span) {
    let block = render_diag(source, level, message, span);
    for (i, line) in block.lines().enumerate() {
        if i == 0 {
            let text = format!("   {}", line);
            match level {
                Level::Error => println!("{}", text.red()),
                Level::Warning => println!("{}", text.yellow()),
            }
        } else {
            println!("   {}", line.dimmed());
        }
    }
}

fn cmd_check(file: &PathBuf) -> Result<()> {
    println!("{} {}", "→ Checking".cyan().bold(), file.display());

    let source = fs::read_to_string(file)?;
    match aec_parser::parse(&source) {
        Ok(program) => {
            println!();
            println!("{}", "✅ Parse successful!".green().bold());
            println!();
            println!("   {} {}", "Agent:".bold(), program.header.name.name.yellow());
            println!("   {} {}", "Items:".bold(), program.items.len());

            let ui_count = program.items.iter()
                .filter(|i| matches!(i, TopLevelItem::Ui(_)))
                .count();
            if ui_count > 0 {
                println!("   {} {}", "UI blocks:".bold(), ui_count.to_string().cyan());
            }

            if !run_type_check(&source, &program) {
                std::process::exit(1);
            }

            println!();
            Ok(())
        }
        Err(err) => {
            println!();
            println!("{}", "❌ Parse failed!".red().bold());
            println!();
            print_diag(&source, Level::Error, &parse_error_message(&err.kind), err.span);
            println!();
            std::process::exit(1);
        }
    }
}

/// Runs and prints the type check. Returns `false` when there is any error.
fn run_type_check(source: &str, program: &aec_ast::Program) -> bool {
    let diags = aec_check::check_program(program);
    let errors = diags.iter().filter(|d| d.is_error()).count();
    let warnings = diags.len() - errors;

    if !diags.is_empty() {
        for d in &diags {
            println!();
            let level = if d.is_error() {
                Level::Error
            } else {
                Level::Warning
            };
            print_diag(source, level, &d.message, d.span);
        }
    }

    if errors > 0 {
        println!();
        println!(
            "{}",
            format!(
                "❌ Type check failed! ({} error(s), {} warning(s))",
                errors, warnings
            )
            .red()
            .bold()
        );
        false
    } else if warnings > 0 {
        println!();
        println!(
            "{}",
            format!("⚠️  Type check passed with {} warning(s)", warnings)
                .yellow()
                .bold()
        );
        true
    } else {
        println!();
        println!("{}", "✅ Type check passed".green());
        true
    }
}

fn cmd_ast(file: &PathBuf) -> Result<()> {
    println!("{} {}", "→ Parsing".cyan().bold(), file.display());

    let source = fs::read_to_string(file)?;
    match aec_parser::parse(&source) {
        Ok(program) => {
            println!();
            println!("{:#?}", program);
            Ok(())
        }
        Err(err) => {
            println!();
            println!("{}", "❌ Parse failed!".red().bold());
            println!();
            print_diag(&source, Level::Error, &parse_error_message(&err.kind), err.span);
            println!();
            std::process::exit(1);
        }
    }
}

fn cmd_run(file: &PathBuf, entry: &str, force_cli: bool) -> Result<()> {
    println!("{} {}", "→ Running".cyan().bold(), file.display());
    println!();

    let source = fs::read_to_string(file)?;
    let program = match aec_parser::parse(&source) {
        Ok(p) => p,
        Err(err) => {
            println!("{}", "❌ Parse failed!".red().bold());
            println!();
            print_diag(&source, Level::Error, &parse_error_message(&err.kind), err.span);
            println!();
            std::process::exit(1);
        }
    };

    // Gate: a program with type errors must not execute.
    if !run_type_check(&source, &program) {
        std::process::exit(1);
    }

    // Is there a UI declaration?
    let ui_decls: Vec<_> = program.items.iter()
        .filter_map(|i| {
            if let TopLevelItem::Ui(u) = i { Some(u) } else { None }
        })
        .collect();

    if !ui_decls.is_empty() && !force_cli {
        // There is a UI -> open a native window
        println!("{}", "🎨 UI detected — opening native window...".cyan().bold());
        println!();

        let ui = ui_decls[0];
        match aec_ui::run_ui(&program, ui) {
            Ok(_) => {
                println!();
                println!("{}", "✅ Window closed".green().bold());
                Ok(())
            }
            Err(e) => {
                println!("{}", "❌ UI error!".red().bold());
                println!();
                println!("   {}", e.to_string().red());
                std::process::exit(1);
            }
        }
    } else {
        // CLI mode
        let mut interp = aec_runtime::Interpreter::new();

        if let Err(err) = interp.run(&program) {
            println!("{}", "❌ Runtime init failed!".red().bold());
            println!();
            println!("   {}", err.to_string().red());
            std::process::exit(1);
        }

        match interp.call_function(entry, vec![], aec_ast::Span::dummy()) {
            Ok(value) => {
                println!();
                println!("{}", "✅ Program finished!".green().bold());
                if !matches!(value, aec_runtime::Value::None) {
                    println!();
                    println!("   {} {}", "Result:".bold(), value.to_string().yellow());
                }
                println!();
                Ok(())
            }
            Err(err) => {
                println!();
                println!("{}", "❌ Runtime error!".red().bold());
                println!();
                println!("   {}", err.to_string().red());
                println!();
                std::process::exit(1);
            }
        }
    }
}
