use aec_cli::package;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "apm")]
#[command(version = "0.1.0")]
#[command(about = "APM — the AEC package manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create an apm.toml manifest
    Init {
        name: Option<String>,
    },
    /// Add a local path dependency
    Add {
        name: String,
        path: PathBuf,
    },
    /// Remove a local dependency
    Remove {
        name: String,
    },
    /// Resolve dependencies and write apm.lock
    Install,
    /// List local dependencies
    List,
    /// Print the dependency tree
    Tree,
}

fn main() -> Result<()> {
    let root = std::env::current_dir()?;
    match Cli::parse().command {
        Commands::Init { name } => {
            let manifest = package::init(&root, name.as_deref())?;
            println!("initialized {}", manifest.name);
        }
        Commands::Add { name, path } => {
            package::add_dependency(&root, &name, &path)?;
            if root.join(package::LOCK_FILE).exists() || root.join(package::LEGACY_LOCK_FILE).exists() {
                package::install(&root)?;
            }
            println!("added {} -> {}", name, path.display());
        }
        Commands::Remove { name } => {
            package::remove_dependency(&root, &name)?;
            if root.join(package::LOCK_FILE).exists() || root.join(package::LEGACY_LOCK_FILE).exists() {
                package::install(&root)?;
            }
            println!("removed {}", name);
        }
        Commands::Install => {
            let lockfile = package::install(&root)?;
            println!("installed {} dependencies", lockfile.entries.len());
            for entry in lockfile.entries {
                println!("{}@{} -> {}", entry.name, entry.version, entry.path);
            }
        }
        Commands::List => {
            for dependency in package::list_dependencies(&root)? {
                println!("{}@{} -> {}", dependency.name, dependency.version, dependency.path);
            }
        }
        Commands::Tree => println!("{}", package::dependency_tree(&root)?),
    }
    Ok(())
}
