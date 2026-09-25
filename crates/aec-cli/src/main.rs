use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use std::path::{Path, PathBuf};

use aec_ast::TopLevelItem;

mod loader;
mod render;
use aec_cli::package;
use loader::{load_program, LoadedProgram};
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
    /// Create an apm.toml manifest
    Init {
        name: Option<String>,
    },
    /// Add a local path dependency to apm.toml
    Add {
        name: String,
        path: PathBuf,
    },
    /// Remove a local dependency from apm.toml
    Remove {
        name: String,
    },
    /// Resolve local dependencies and write apm.lock
    Install,
    /// List local dependencies
    List,
    /// Print the local dependency tree
    Tree,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { file } => cmd_check(&file),
        Commands::Ast { file } => cmd_ast(&file),
        Commands::Run { file, entry, cli: force_cli } => cmd_run(&file, &entry, force_cli),
        Commands::Init { name } => cmd_init(name.as_deref()),
        Commands::Add { name, path } => cmd_add(&name, &path),
        Commands::Remove { name } => cmd_remove(&name),
        Commands::Install => cmd_install(),
        Commands::List => cmd_list(),
        Commands::Tree => cmd_tree(),
    }
}

fn cmd_init(name: Option<&str>) -> Result<()> {
    let root = std::env::current_dir()?;
    let manifest = package::init(&root, name)?;
    println!("{} {}", "✅ Initialized".green().bold(), manifest.name);
    Ok(())
}

fn cmd_add(name: &str, path: &std::path::Path) -> Result<()> {
    let root = std::env::current_dir()?;
    package::add_dependency(&root, name, path)?;
    if root.join(package::LOCK_FILE).exists() || root.join(package::LEGACY_LOCK_FILE).exists() {
        package::install(&root)?;
    }
    println!("{} {} -> {}", "✅ Added".green().bold(), name, path.display());
    Ok(())
}

fn cmd_remove(name: &str) -> Result<()> {
    let root = std::env::current_dir()?;
    package::remove_dependency(&root, name)?;
    if root.join(package::LOCK_FILE).exists() || root.join(package::LEGACY_LOCK_FILE).exists() {
        package::install(&root)?;
    }
    println!("{} {}", "✅ Removed".green().bold(), name);
    Ok(())
}

fn cmd_install() -> Result<()> {
    let root = std::env::current_dir()?;
    let lockfile = package::install(&root)?;
    println!("{} {} dependencies", "✅ Installed".green().bold(), lockfile.entries.len());
    for entry in lockfile.entries {
        println!("   {}@{} -> {}", entry.name, entry.version, entry.path);
    }
    Ok(())
}

fn cmd_list() -> Result<()> {
    let root = std::env::current_dir()?;
    for dependency in package::list_dependencies(&root)? {
        println!("{}@{} -> {}", dependency.name, dependency.version, dependency.path);
    }
    Ok(())
}

fn cmd_tree() -> Result<()> {
    let root = std::env::current_dir()?;
    println!("{}", package::dependency_tree(&root)?);
    Ok(())
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

fn cmd_check(file: &Path) -> Result<()> {
    println!("{} {}", "→ Checking".cyan().bold(), file.display());
    match load_program(file) {
        Ok(loaded) => {
            let program = &loaded.program;
            println!();
            println!("{}", "✅ Parse successful!".green().bold());
            println!();
            println!("   {} {}", "Agent:".bold(), program.header.name.name.yellow());
            println!("   {} {}", "Items:".bold(), program.items.len());

            let ui_count = program
                .items
                .iter()
                .enumerate()
                .filter(|(index, item)| {
                    program.item_modules.get(*index).cloned().flatten().is_none()
                        && matches!(item, TopLevelItem::Ui(_))
                })
                .count();
            if ui_count > 0 {
                println!("   {} {}", "UI blocks:".bold(), ui_count.to_string().cyan());
            }

            if !run_type_check(&loaded) {
                std::process::exit(1);
            }

            println!();
            Ok(())
        }
        Err(err) => {
            println!();
            println!("{}", "❌ Load failed!".red().bold());
            println!();
            println!("   {err:#}");
            println!();
            std::process::exit(1);
        }
    }
}

/// Runs and prints the type check. Returns `false` when there is any error.
fn run_type_check(loaded: &LoadedProgram) -> bool {
    let diags = aec_check::Checker::new().check_program_with_origins(&loaded.program);
    let errors = diags.iter().filter(|(_, diag)| diag.is_error()).count();
    let warnings = diags.len() - errors;

    if !diags.is_empty() {
        for (item_index, d) in &diags {
            println!();
            let source = &loaded.sources[loaded.origins[*item_index]];
            println!("   {}", source.path.display());
            let level = if d.is_error() {
                Level::Error
            } else {
                Level::Warning
            };
            print_diag(&source.text, level, &d.message, d.span);
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

fn cmd_ast(file: &Path) -> Result<()> {
    println!("{} {}", "→ Parsing".cyan().bold(), file.display());

    match load_program(file) {
        Ok(loaded) => {
            println!();
            println!("{:#?}", loaded.program);
            Ok(())
        }
        Err(err) => {
            println!();
            println!("{}", "❌ Load failed!".red().bold());
            println!();
            println!("   {err:#}");
            println!();
            std::process::exit(1);
        }
    }
}

fn cmd_run(file: &Path, entry: &str, force_cli: bool) -> Result<()> {
    println!("{} {}", "→ Running".cyan().bold(), file.display());
    println!();

    let loaded = match load_program(file) {
        Ok(p) => p,
        Err(err) => {
            println!("{}", "❌ Load failed!".red().bold());
            println!();
            println!("   {err:#}");
            println!();
            std::process::exit(1);
        }
    };
    let program = &loaded.program;

    // Gate: a program with type errors must not execute.
    if !run_type_check(&loaded) {
        std::process::exit(1);
    }

    // Is there a UI declaration?
    let ui_decls: Vec<_> = program
        .items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            if program.item_modules.get(index).cloned().flatten().is_none() {
                if let TopLevelItem::Ui(ui) = item {
                    return Some(ui);
                }
            }
            None
        })
        .collect();

    if ui_decls.len() > 1 {
        println!("{} {} UI declarations", "❌".red().bold(), ui_decls.len());
        println!("   only one UI declaration can be run");
        std::process::exit(1);
    }

    if !ui_decls.is_empty() && !force_cli {
        // There is a UI -> open a native window
        println!("{}", "🎨 UI detected — opening native window...".cyan().bold());
        println!();

        let ui = ui_decls[0];
        match aec_ui::run_ui(program, ui) {
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

        if let Err(err) = interp.run(program) {
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
