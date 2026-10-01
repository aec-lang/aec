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
    #[command(about = "Create an apm.toml manifest")]
    Init { name: Option<String> },
    #[command(about = "Add a local path or registry dependency")]
    Add { name: String, path: PathBuf },
    #[command(about = "Remove a local or registry dependency")]
    Remove { name: String },
    #[command(about = "Resolve dependencies and write apm.lock")]
    Install {
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long = "trust-key")]
        trust_keys: Vec<PathBuf>,
    },
    #[command(about = "List local dependencies")]
    List,
    #[command(about = "Print the dependency tree")]
    Tree,
    #[command(about = "Generate an Ed25519 PKCS#8 key pair")]
    Keygen {
        #[arg(long)]
        private_key: PathBuf,
        #[arg(long)]
        public_key: PathBuf,
    },
    #[command(about = "Initialize a local directory registry")]
    RegistryInit { registry: PathBuf },
    #[command(about = "Publish an immutable signed package version")]
    Publish {
        #[arg(long)]
        registry: PathBuf,
        #[arg(long)]
        private_key: PathBuf,
        #[arg(long)]
        package: Option<PathBuf>,
    },
    #[command(about = "Verify a signed package version with an explicit trust key")]
    Verify {
        #[arg(long)]
        registry: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long)]
        version: String,
        #[arg(long = "trust-key")]
        trust_keys: Vec<PathBuf>,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let root = std::env::current_dir()?;
    match Cli::parse().command {
        Commands::Init { name } => {
            let manifest = package::init(&root, name.as_deref())?;
            println!("initialized {}", manifest.name);
        }
        Commands::Add { name, path } => {
            package::add_dependency(&root, &name, &path)?;
            if package::has_lockfile(&root)? {
                package::refresh_lockfile(&root)?;
            }
            println!("added {} -> {}", name, path.display());
        }
        Commands::Remove { name } => {
            package::remove_dependency(&root, &name)?;
            if package::has_lockfile(&root)? {
                package::refresh_lockfile(&root)?;
            }
            println!("removed {}", name);
        }
        Commands::Install {
            registry,
            trust_keys,
        } => {
            let lockfile = package::install_with_options(
                &root,
                registry.as_deref(),
                &trust_keys,
            )?;
            println!("installed {} dependencies", lockfile.entries.len());
            for entry in lockfile.entries {
                println!("{}@{} -> {}", entry.name, entry.version, entry.path);
            }
        }
        Commands::List => {
            for dependency in package::list_dependencies(&root)? {
                println!(
                    "{}@{} -> {}",
                    dependency.name, dependency.version, dependency.path
                );
            }
        }
        Commands::Tree => println!("{}", package::dependency_tree(&root)?),
        Commands::Keygen {
            private_key,
            public_key,
        } => {
            package::generate_key_pair(&private_key, &public_key)?;
            println!("generated Ed25519 key pair");
        }
        Commands::RegistryInit { registry } => {
            package::init_registry(&registry)?;
            println!("initialized local registry {}", registry.display());
        }
        Commands::Publish {
            registry,
            private_key,
            package,
        } => {
            let package = package.as_deref().unwrap_or(&root);
            let published = package::publish_package(&registry, &private_key, package)?;
            println!(
                "published {}@{} tree {}",
                published.name, published.version, published.tree_digest
            );
        }
        Commands::Verify {
            registry,
            name,
            version,
            trust_keys,
        } => {
            let verified =
                package::verify_package_with_trust_keys(&registry, &name, &version, &trust_keys)?;
            println!(
                "verified {}@{} tree {}",
                verified.name, verified.version, verified.tree_digest
            );
        }
    }
    Ok(())
}
