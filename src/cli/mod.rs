//! Command-line interface for the Astra toolchain
//!
//! Provides commands: fmt, check, test, run, package

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Astra - An LLM/Agent-native programming language
#[derive(Parser, Debug)]
#[command(name = "astra")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Output diagnostics as JSON
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Format Astra source files
    Fmt {
        /// Files or directories to format
        #[arg(default_value = ".")]
        paths: Vec<PathBuf>,

        /// Check formatting without modifying files
        #[arg(long)]
        check: bool,
    },

    /// Check for errors without running
    Check {
        /// Files or directories to check
        #[arg(default_value = ".")]
        paths: Vec<PathBuf>,
    },

    /// Run tests
    Test {
        /// Filter tests by name
        #[arg()]
        filter: Option<String>,

        /// Random seed for deterministic tests
        #[arg(long)]
        seed: Option<u64>,
    },

    /// Run an Astra program
    Run {
        /// File to run
        file: PathBuf,

        /// Arguments to pass to the program
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Create a distributable package
    Package {
        /// Output directory
        #[arg(long, short, default_value = "build")]
        output: PathBuf,

        /// Target format (wasm, native)
        #[arg(long, default_value = "wasm")]
        target: String,
    },
}

impl Cli {
    /// Run the CLI
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::parse();

        match cli.command {
            Command::Fmt { paths, check } => {
                run_fmt(&paths, check, cli.json)?;
            }
            Command::Check { paths } => {
                run_check(&paths, cli.json)?;
            }
            Command::Test { filter, seed } => {
                run_test(filter.as_deref(), seed, cli.json)?;
            }
            Command::Run { file, args } => {
                run_program(&file, &args)?;
            }
            Command::Package { output, target } => {
                run_package(&output, &target)?;
            }
        }

        Ok(())
    }
}

fn run_fmt(paths: &[PathBuf], check: bool, _json: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Formatting {} path(s)...", paths.len());
    if check {
        println!("(check mode - no files will be modified)");
    }
    // TODO: Implement formatting
    Ok(())
}

fn run_check(paths: &[PathBuf], _json: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Checking {} path(s)...", paths.len());
    // TODO: Implement checking
    Ok(())
}

fn run_test(
    filter: Option<&str>,
    seed: Option<u64>,
    _json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(f) = filter {
        println!("Running tests matching '{}'...", f);
    } else {
        println!("Running all tests...");
    }
    if let Some(s) = seed {
        println!("Using seed: {}", s);
    }
    // TODO: Implement test runner
    Ok(())
}

fn run_program(file: &PathBuf, _args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    println!("Running {:?}...", file);
    // TODO: Implement interpreter
    Ok(())
}

fn run_package(output: &PathBuf, target: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Packaging to {:?} (target: {})...", output, target);
    // TODO: Implement packaging
    Ok(())
}
