//! Command-line interface for the Astra toolchain
//!
//! Provides commands: fmt, check, test, run, package

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::interpreter::{
    Capabilities, ConsoleCapability, EnvCapability, FixedClock, Interpreter, SeededRand,
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

        /// Treat warnings as errors (like strict mypy / ruff enforcement)
        #[arg(long)]
        strict: bool,
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

    /// Start interactive REPL
    Repl,

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
            Command::Check { paths, strict } => {
                run_check(&paths, strict, cli.json)?;
            }
            Command::Test { filter, seed } => {
                run_test(filter.as_deref(), seed, cli.json)?;
            }
            Command::Run { file, args } => {
                run_program(&file, &args)?;
            }
            Command::Repl => {
                run_repl()?;
            }
            Command::Package { output, target } => {
                run_package(&output, &target)?;
            }
        }

        Ok(())
    }
}

fn run_fmt(paths: &[PathBuf], check: bool, _json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut files_formatted = 0;
    let mut files_changed = 0;

    for path in paths {
        if path.is_file() && path.extension().is_some_and(|ext| ext == "astra") {
            match fmt_file(path, check)? {
                FmtResult::Unchanged => files_formatted += 1,
                FmtResult::Changed => {
                    files_formatted += 1;
                    files_changed += 1;
                }
                FmtResult::Error => {}
            }
        } else if path.is_dir() {
            for entry in walkdir(path)? {
                if entry.extension().is_some_and(|ext| ext == "astra") {
                    match fmt_file(&entry, check)? {
                        FmtResult::Unchanged => files_formatted += 1,
                        FmtResult::Changed => {
                            files_formatted += 1;
                            files_changed += 1;
                        }
                        FmtResult::Error => {}
                    }
                }
            }
        }
    }

    if check {
        if files_changed > 0 {
            println!(
                "{} file(s) would be reformatted ({} checked)",
                files_changed, files_formatted
            );
            std::process::exit(1);
        } else {
            println!("{} file(s) already formatted", files_formatted);
        }
    } else {
        println!(
            "Formatted {} file(s) ({} changed)",
            files_formatted, files_changed
        );
    }

    Ok(())
}

enum FmtResult {
    Unchanged,
    Changed,
    Error,
}

fn fmt_file(path: &PathBuf, check: bool) -> Result<FmtResult, Box<dyn std::error::Error>> {
    let source =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

    let source_file = SourceFile::new(path.clone(), source.clone());
    let lexer = Lexer::new(&source_file);
    let mut parser = AstraParser::new(lexer, source_file.clone());

    let module = match parser.parse_module() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Parse error in {:?}:\n{}", path, e.format_text(&source));
            return Ok(FmtResult::Error);
        }
    };

    let mut formatter = crate::formatter::Formatter::new();
    let formatted = formatter.format_module(&module);

    if formatted == source {
        return Ok(FmtResult::Unchanged);
    }

    if check {
        println!("Would reformat: {:?}", path);
        Ok(FmtResult::Changed)
    } else {
        std::fs::write(path, &formatted)
            .map_err(|e| format!("Failed to write {:?}: {}", path, e))?;
        println!("Formatted: {:?}", path);
        Ok(FmtResult::Changed)
    }
}

/// Counts returned from checking a single file
struct CheckCounts {
    errors: usize,
    warnings: usize,
}

fn run_check(
    paths: &[PathBuf],
    strict: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut total_errors = 0;
    let mut total_warnings = 0;
    let mut files_checked = 0;

    for path in paths {
        if path.is_file() && path.extension().is_some_and(|ext| ext == "astra") {
            files_checked += 1;
            let counts = check_file(path, json)?;
            total_errors += counts.errors;
            total_warnings += counts.warnings;
        } else if path.is_dir() {
            for entry in walkdir(path)? {
                if entry.extension().is_some_and(|ext| ext == "astra") {
                    files_checked += 1;
                    let counts = check_file(&entry, json)?;
                    total_errors += counts.errors;
                    total_warnings += counts.warnings;
                }
            }
        }
    }

    let has_issues = total_errors > 0 || (strict && total_warnings > 0);

    if has_issues {
        let mut parts = Vec::new();
        if total_errors > 0 {
            parts.push(format!("{} error(s)", total_errors));
        }
        if total_warnings > 0 {
            if strict {
                parts.push(format!(
                    "{} warning(s) [treated as errors with --strict]",
                    total_warnings
                ));
            } else {
                parts.push(format!("{} warning(s)", total_warnings));
            }
        }
        eprintln!(
            "\nChecked {} file(s), found {}",
            files_checked,
            parts.join(", ")
        );
        std::process::exit(1);
    } else if total_warnings > 0 {
        println!(
            "Checked {} file(s), no errors ({} warning(s))",
            files_checked, total_warnings
        );
    } else {
        println!("Checked {} file(s), no errors found", files_checked);
    }

    Ok(())
}

fn check_file(path: &PathBuf, json: bool) -> Result<CheckCounts, Box<dyn std::error::Error>> {
    let source =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

    let source_file = SourceFile::new(path.clone(), source.clone());
    let lexer = Lexer::new(&source_file);
    let mut parser = AstraParser::new(lexer, source_file.clone());
    match parser.parse_module() {
        Ok(module) => {
            // Run type checking (includes exhaustiveness + effect + lint enforcement)
            let mut checker = crate::typechecker::TypeChecker::new();
            let type_result = checker.check_module(&module);

            // Always retrieve all diagnostics (errors + warnings)
            let all_diags = checker.diagnostics();
            let errors: Vec<_> = all_diags
                .diagnostics()
                .iter()
                .filter(|d| d.is_error())
                .collect();
            let warnings: Vec<_> = all_diags
                .diagnostics()
                .iter()
                .filter(|d| matches!(d.severity, crate::diagnostics::Severity::Warning))
                .collect();

            // Display errors (from Err result or from diagnostics bag)
            if let Err(ref bag) = type_result {
                if json {
                    println!("{}", bag.to_json());
                } else {
                    eprintln!("{}", bag.format_text(&source));
                }
            }

            // Display warnings
            if !warnings.is_empty() {
                for w in &warnings {
                    if json {
                        println!("{}", w.to_json());
                    } else {
                        eprintln!("{}", w.to_human_readable(&source));
                    }
                }
            }

            Ok(CheckCounts {
                errors: errors.len(),
                warnings: warnings.len(),
            })
        }
        Err(e) => {
            if json {
                println!("{}", e.to_json());
            } else {
                eprintln!("Error in {:?}:\n{}", path, e.format_text(&source));
            }
            Ok(CheckCounts {
                errors: e.len(),
                warnings: 0,
            })
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
        .filter(|p| p.extension().is_some_and(|ext| ext == "astra"))
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
                eprintln!("Parse error in {:?}:\n{}", path, e.format_text(&source));
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

                // Build capabilities from using clause
                let capabilities = build_test_capabilities(&test.using);

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

            // P5.1: Property-based tests
            if let Item::Property(prop) = item {
                if let Some(f) = filter {
                    if !prop.name.contains(f) {
                        continue;
                    }
                }

                total_tests += 1;
                let num_iterations = 100;
                let seed = _seed.unwrap_or(42);
                let mut all_passed = true;

                for i in 0..num_iterations {
                    let iter_seed = seed.wrapping_add(i);
                    let mut capabilities = build_test_capabilities(&prop.using);
                    capabilities.rand = Some(Box::new(SeededRand::new(iter_seed)));

                    let mut interpreter = Interpreter::with_capabilities(capabilities);
                    if let Err(e) = interpreter.load_module(&module) {
                        eprintln!("  FAIL: {} (iteration {}) - {}", prop.name, i, e);
                        all_passed = false;
                        break;
                    }

                    if let Err(e) = interpreter.eval_block(&prop.body) {
                        eprintln!(
                            "  FAIL: {} (iteration {}, seed {}) - {}",
                            prop.name, i, iter_seed, e
                        );
                        all_passed = false;
                        break;
                    }
                }

                if all_passed {
                    println!("  PASS: {} ({} iterations)", prop.name, num_iterations);
                    passed += 1;
                } else {
                    failed += 1;
                }
            }
        }
    }

    println!(
        "\n{} tests: {} passed, {} failed",
        total_tests, passed, failed
    );

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

/// Build capabilities for a test based on its `using effects(...)` clause.
///
/// Supports:
/// - `Rand = Rand.seeded(<seed>)` -> SeededRand
/// - `Clock = Clock.fixed(<time>)` -> FixedClock
/// - Console is always provided (captured)
fn build_test_capabilities(using: &Option<crate::parser::ast::UsingClause>) -> Capabilities {
    let mut capabilities = Capabilities {
        console: Some(Box::new(TestConsole::new())),
        ..Default::default()
    };

    if let Some(clause) = using {
        for binding in &clause.bindings {
            match binding.effect.as_str() {
                "Rand" => {
                    // Expect: Rand.seeded(<int>)
                    if let Some(seed) = extract_method_int_arg(&binding.value, "Rand", "seeded") {
                        capabilities.rand = Some(Box::new(SeededRand::new(seed as u64)));
                    }
                }
                "Clock" => {
                    // Expect: Clock.fixed(<int>)
                    if let Some(time) = extract_method_int_arg(&binding.value, "Clock", "fixed") {
                        capabilities.clock = Some(Box::new(FixedClock::new(time)));
                    }
                }
                _ => {
                    // Unknown effect binding - ignore for now
                }
            }
        }
    }

    capabilities
}

/// Extract an integer argument from a method call expression like `Foo.bar(42)`.
fn extract_method_int_arg(
    expr: &crate::parser::ast::Expr,
    expected_receiver: &str,
    expected_method: &str,
) -> Option<i64> {
    use crate::parser::ast::Expr;

    if let Expr::MethodCall {
        receiver,
        method,
        args,
        ..
    } = expr
    {
        if method == expected_method {
            if let Expr::Ident { name, .. } = receiver.as_ref() {
                if name == expected_receiver {
                    if let Some(Expr::IntLit { value, .. }) = args.first() {
                        return Some(*value);
                    }
                }
            }
        }
    }
    None
}

fn run_program(file: &PathBuf, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // Read the source file
    let source = std::fs::read_to_string(file)
        .map_err(|e| format!("Failed to read file {:?}: {}", file, e))?;

    // Parse the source
    let source_file = SourceFile::new(file.clone(), source.clone());
    let lexer = Lexer::new(&source_file);
    let mut parser = AstraParser::new(lexer, source_file.clone());
    let module = parser
        .parse_module()
        .map_err(|e| format!("Parse error:\n{}", e.format_text(&source)))?;

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
    // Add the file's parent directory as a search path for imports
    if let Some(parent) = file.parent() {
        interpreter.add_search_path(parent.to_path_buf());
    }
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

fn run_repl() -> Result<(), Box<dyn std::error::Error>> {
    use crate::interpreter::format_value;
    use std::io::{self, Write};

    println!("Astra REPL v0.1.0");
    println!("Type expressions to evaluate. Use :quit to exit, :help for help.");
    println!();

    // Keep track of definitions (accumulated source)
    let mut definitions = String::from("module repl\n");
    let mut line_num = 0u32;

    let make_interpreter = || {
        Interpreter::with_capabilities(Capabilities {
            console: Some(Box::new(RealConsole)),
            ..Default::default()
        })
    };

    loop {
        print!("astra> ");
        io::stdout().flush()?;

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // Handle REPL commands
        match input {
            ":quit" | ":q" | ":exit" => break,
            ":help" | ":h" => {
                println!("Commands:");
                println!("  :quit, :q    Exit the REPL");
                println!("  :help, :h    Show this help");
                println!("  :clear       Clear all definitions");
                println!();
                println!("Enter expressions, let bindings, or function definitions.");
                continue;
            }
            ":clear" => {
                definitions = String::from("module repl\n");
                println!("Cleared.");
                continue;
            }
            _ => {}
        }

        line_num += 1;

        // Try to evaluate as expression first: wrap in a temp main function
        let expr_source = format!("{}\nfn __repl__() -> Unit {{ {} }}", definitions, input);

        let source_file = SourceFile::new(PathBuf::from("repl.astra"), expr_source.clone());
        let lexer = Lexer::new(&source_file);
        let mut parser = AstraParser::new(lexer, source_file.clone());

        match parser.parse_module() {
            Ok(module) => {
                let mut interp = make_interpreter();
                if let Err(e) = interp.load_module(&module) {
                    eprintln!("Error: {}", e);
                    continue;
                }
                if let Some(func) = interp.env.lookup("__repl__").cloned() {
                    match interp.call_function(func, vec![]) {
                        Ok(value) => {
                            if !matches!(value, crate::interpreter::Value::Unit) {
                                println!("{}", format_value(&value));
                            }
                        }
                        Err(e) if e.is_return => {
                            if let Some(val) = e.early_return {
                                println!("{}", format_value(&val));
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                        }
                    }
                }
            }
            Err(_) => {
                // Try as a top-level definition (fn, type, enum)
                let def_source = format!("{}\n{}", definitions, input);
                let source_file2 = SourceFile::new(PathBuf::from("repl.astra"), def_source.clone());
                let lexer2 = Lexer::new(&source_file2);
                let mut parser2 = AstraParser::new(lexer2, source_file2.clone());

                match parser2.parse_module() {
                    Ok(_) => {
                        definitions = def_source;
                        println!("Defined. ({} definitions)", line_num);
                    }
                    Err(e) => {
                        eprintln!("{}", e.format_text(&def_source));
                    }
                }
            }
        }
    }

    println!("Bye!");
    Ok(())
}

fn run_package(output: &PathBuf, target: &str) -> Result<(), Box<dyn std::error::Error>> {
    // P7.2: Basic package command
    println!("Packaging project...");

    // Read project manifest
    let manifest_path = std::env::current_dir()?.join("astra.toml");
    if !manifest_path.exists() {
        return Err(
            "No astra.toml found in current directory. Run `astra init` to create one.".into(),
        );
    }

    let manifest_content = std::fs::read_to_string(&manifest_path)?;
    println!("  Found manifest: astra.toml");

    // Collect all .astra source files
    let current_dir = std::env::current_dir()?;
    let source_files = walkdir(&current_dir)?
        .into_iter()
        .filter(|p| p.extension().is_some_and(|ext| ext == "astra"))
        .collect::<Vec<_>>();

    println!("  Found {} source files", source_files.len());

    // Validate all files parse and type-check
    let mut errors = 0;
    for file in &source_files {
        let source = std::fs::read_to_string(file)?;
        let source_file = SourceFile::new(file.clone(), source.clone());
        let lexer = Lexer::new(&source_file);
        let mut parser = AstraParser::new(lexer, source_file.clone());
        match parser.parse_module() {
            Ok(module) => {
                let mut checker = crate::typechecker::TypeChecker::new();
                if checker.check_module(&module).is_err() {
                    eprintln!("  Type error in {:?}", file);
                    errors += 1;
                }
            }
            Err(_) => {
                eprintln!("  Parse error in {:?}", file);
                errors += 1;
            }
        }
    }

    if errors > 0 {
        return Err(format!("{} file(s) have errors. Fix them before packaging.", errors).into());
    }

    // Create output directory
    std::fs::create_dir_all(output)?;

    // Copy source files to output
    for file in &source_files {
        let relative = file.strip_prefix(&current_dir).unwrap_or(file);
        let dest = output.join(relative);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(file, &dest)?;
    }

    // Copy manifest
    std::fs::copy(&manifest_path, output.join("astra.toml"))?;

    // Write package metadata
    let metadata = format!(
        "# Astra Package\n# Target: {}\n# Manifest:\n{}\n",
        target, manifest_content
    );
    std::fs::write(output.join("PACKAGE.md"), metadata)?;

    println!("  Package created at {:?} (target: {})", output, target);
    println!("  {} files packaged successfully", source_files.len());

    Ok(())
}
