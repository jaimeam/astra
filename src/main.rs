//! Astra CLI - The Astra programming language toolchain

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "astra")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output diagnostics as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Suppress non-error output
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Show verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Format Astra source files
    Fmt {
        /// Files to format (formats all if none specified)
        #[arg(value_name = "FILES")]
        files: Vec<PathBuf>,

        /// Check formatting without modifying files
        #[arg(long)]
        check: bool,
    },

    /// Check source files for errors
    Check {
        /// Files to check (checks all if none specified)
        #[arg(value_name = "FILES")]
        files: Vec<PathBuf>,
    },

    /// Run tests
    Test {
        /// Filter tests by name
        #[arg(value_name = "FILTER")]
        filter: Option<String>,

        /// Random seed for property tests
        #[arg(long)]
        seed: Option<u64>,
    },

    /// Run an Astra program
    Run {
        /// The file to run
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Arguments to pass to the program
        #[arg(value_name = "ARGS")]
        args: Vec<String>,

        /// Random seed for deterministic execution
        #[arg(long)]
        seed: Option<u64>,
    },

    /// Package the project for distribution
    Package {
        /// Output directory
        #[arg(short, long, default_value = "dist")]
        output: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Fmt { files, check } => astra::cli::fmt::run(files, check, cli.json),
        Commands::Check { files } => astra::cli::check::run(files, cli.json),
        Commands::Test { filter, seed } => astra::cli::test::run(filter, seed, cli.json),
        Commands::Run { file, args, seed } => astra::cli::run::run(file, args, seed),
        Commands::Package { output } => astra::cli::package::run(output),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if !cli.quiet {
                eprintln!("Error: {}", e);
            }
            ExitCode::FAILURE
        }
    }
}
