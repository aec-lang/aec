use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use std::fs;
use std::path::PathBuf;

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
        /// Path to .aec file
        file: PathBuf,
    },
    /// Print the AST of an AEC file
    Ast {
        /// Path to .aec file
        file: PathBuf,
    },
    /// Run an AEC file
    Run {
        /// Path to .aec file
        file: PathBuf,
        /// Function to call (default: main)
        #[arg(short, long, default_value = "main")]
        entry: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { file } => cmd_check(&file),
        Commands::Ast { file } => cmd_ast(&file),
        Commands::Run { file, entry } => cmd_run(&file, &entry),
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
            println!();
            Ok(())
        }
        Err(err) => {
            println!();
            println!("{}", "❌ Parse failed!".red().bold());
            println!();
            println!("   {}", err.to_string().red());
            println!();
            std::process::exit(1);
        }
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
            println!("   {}", err.to_string().red());
            println!();
            std::process::exit(1);
        }
    }
}

fn cmd_run(file: &PathBuf, entry: &str) -> Result<()> {
    println!("{} {}", "→ Running".cyan().bold(), file.display());
    println!();

    let source = fs::read_to_string(file)?;
    let program = match aec_parser::parse(&source) {
        Ok(p) => p,
        Err(err) => {
            println!("{}", "❌ Parse failed!".red().bold());
            println!();
            println!("   {}", err.to_string().red());
            println!();
            std::process::exit(1);
        }
    };

    let mut interp = aec_runtime::Interpreter::new();

    // ثبت توابع
    if let Err(err) = interp.run(&program) {
        println!("{}", "❌ Runtime init failed!".red().bold());
        println!();
        println!("   {}", err.to_string().red());
        std::process::exit(1);
    }

    // فراخوانی تابع entry
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
