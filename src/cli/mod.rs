//! Command-line interface for the Astra toolchain
//!
//! Provides commands: fmt, check, test, run, package

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::diagnostics::DiagnosticBag;
use crate::interpreter::{
    Capabilities, ConsoleCapability, EnvCapability, Interpreter,
};
use crate::parser::{Lexer, Parser as AstraParser, SourceFile};

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

fn run_check(paths: &[PathBuf], json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut total_errors = 0;
    let mut files_checked = 0;

    for path in paths {
        if path.is_file() && path.extension().map_or(false, |ext| ext == "astra") {
            files_checked += 1;
            let errors = check_file(path, json)?;
            total_errors += errors;
        } else if path.is_dir() {
            // Recursively check all .astra files
            for entry in walkdir(path)? {
                if entry.extension().map_or(false, |ext| ext == "astra") {
                    files_checked += 1;
                    let errors = check_file(&entry, json)?;
                    total_errors += errors;
                }
            }
        }
    }

    if total_errors > 0 {
        eprintln!("\nChecked {} file(s), found {} error(s)", files_checked, total_errors);
        std::process::exit(1);
    } else {
        println!("Checked {} file(s), no errors found", files_checked);
    }

    Ok(())
}

fn check_file(path: &PathBuf, json: bool) -> Result<usize, Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

    let source_file = SourceFile::new(path.clone(), source.clone());
    let lexer = Lexer::new(&source_file);
    let mut parser = AstraParser::new(lexer, source_file.clone());
    match parser.parse_module() {
        Ok(module) => {
            // Run type checking (includes exhaustiveness + effect enforcement)
            let mut checker = crate::typechecker::TypeChecker::new();
            match checker.check_module(&module) {
                Ok(()) => Ok(0),
                Err(bag) => {
                    if json {
                        println!("{}", bag.to_json());
                    } else {
                        eprintln!("Error in {:?}:\n{}", path, bag.format_text(&source));
                    }
                    Ok(bag.len())
                }
            }
        }
        Err(e) => {
            let bag = DiagnosticBag::from(e);
            if json {
                println!("{}", bag.to_json());
            } else {
                eprintln!("Error in {:?}:\n{}", path, bag.format_text(&source));
            }
            Ok(bag.len())
        }
    }
}

/// Simple recursive directory walker
fn walkdir(path: &PathBuf) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut results = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_file() {
            results.push(entry_path);
        } else if entry_path.is_dir() {
            results.extend(walkdir(&entry_path)?);
        }
    }
    Ok(results)
}

fn run_test(
    filter: Option<&str>,
    _seed: Option<u64>,
    _json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::parser::ast::Item;

    // Find all .astra files in current directory
    let current_dir = std::env::current_dir()?;
    let files = walkdir(&current_dir)?;
    let astra_files: Vec<_> = files
        .into_iter()
        .filter(|p| p.extension().map_or(false, |ext| ext == "astra"))
        .collect();

    let mut total_tests = 0;
    let mut passed = 0;
    let mut failed = 0;

    for path in astra_files {
        let source = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

        let source_file = SourceFile::new(path.clone(), source.clone());
        let lexer = Lexer::new(&source_file);
        let mut parser = AstraParser::new(lexer, source_file.clone());

        let module = match parser.parse_module() {
            Ok(m) => m,
            Err(e) => {
                let bag = DiagnosticBag::from(e);
                eprintln!("Parse error in {:?}:\n{}", path, bag.format_text(&source));
                continue;
            }
        };

        // Find and run all test blocks
        for item in &module.items {
            if let Item::Test(test) = item {
                // Apply filter if specified
                if let Some(f) = filter {
                    if !test.name.contains(f) {
                        continue;
                    }
                }

                total_tests += 1;

                // Run the test
                let console = Box::new(TestConsole::new());
                let capabilities = Capabilities {
                    console: Some(console),
                    ..Default::default()
                };

                let mut interpreter = Interpreter::with_capabilities(capabilities);
                // Load the module functions first
                if let Err(e) = interpreter.load_module(&module) {
                    eprintln!("  FAIL: {} - {}", test.name, e);
                    failed += 1;
                    continue;
                }

                // Run the test block
                match interpreter.eval_block(&test.body) {
                    Ok(_) => {
                        println!("  PASS: {}", test.name);
                        passed += 1;
                    }
                    Err(e) => {
                        eprintln!("  FAIL: {} - {}", test.name, e);
                        failed += 1;
                    }
                }
            }
        }
    }

    println!("\n{} tests: {} passed, {} failed", total_tests, passed, failed);

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Test console that captures output
struct TestConsole {
    output: std::cell::RefCell<Vec<String>>,
}

impl TestConsole {
    fn new() -> Self {
        Self {
            output: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl ConsoleCapability for TestConsole {
    fn print(&self, text: &str) {
        self.output.borrow_mut().push(text.to_string());
    }

    fn println(&self, text: &str) {
        self.output.borrow_mut().push(format!("{}\n", text));
    }

    fn read_line(&self) -> Option<String> {
        None
    }
}

fn run_program(file: &PathBuf, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Read the source file
    let source = std::fs::read_to_string(file)
        .map_err(|e| format!("Failed to read file {:?}: {}", file, e))?;

    // Parse the source
    let source_file = SourceFile::new(file.clone(), source.clone());
    let lexer = Lexer::new(&source_file);
    let mut parser = AstraParser::new(lexer, source_file.clone());
    let module = parser.parse_module().map_err(|e| {
        let bag = DiagnosticBag::from(e);
        format!("Parse error:\n{}", bag.format_text(&source))
    })?;

    // Set up capabilities
    let console = Box::new(RealConsole);
    let env_cap = Box::new(RealEnv::new(args.to_vec()));

    let capabilities = Capabilities {
        console: Some(console),
        env: Some(env_cap),
        ..Default::default()
    };

    // Create interpreter and run
    let mut interpreter = Interpreter::with_capabilities(capabilities);
    match interpreter.eval_module(&module) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Runtime error: {}", e).into()),
    }
}

/// Real console capability that prints to stdout
struct RealConsole;

impl ConsoleCapability for RealConsole {
    fn print(&self, text: &str) {
        print!("{}", text);
    }

    fn println(&self, text: &str) {
        println!("{}", text);
    }

    fn read_line(&self) -> Option<String> {
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) => None, // EOF
            Ok(_) => Some(line.trim_end().to_string()),
            Err(_) => None,
        }
    }
}

/// Real environment capability
struct RealEnv {
    args: Vec<String>,
}

impl RealEnv {
    fn new(args: Vec<String>) -> Self {
        Self { args }
    }
}

impl EnvCapability for RealEnv {
    fn get(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn args(&self) -> Vec<String> {
        self.args.clone()
    }
}

fn run_package(output: &PathBuf, target: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Packaging to {:?} (target: {})...", output, target);
    // TODO: Implement packaging
    Ok(())
}
