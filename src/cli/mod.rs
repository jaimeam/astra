//! Command-line interface for the Astra toolchain
//!
//! Provides commands: fmt, check, test, run, package

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::interpreter::{
    Capabilities, ClockCapability, ConsoleCapability, EnvCapability, FixedClock, FsCapability,
    Interpreter, MockConsole, NetCapability, RandCapability, SeededRand, Value,
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

        /// Bypass the incremental cache (re-check all files)
        #[arg(long)]
        no_cache: bool,
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

    /// Initialize a new Astra project
    Init {
        /// Project name (defaults to current directory name)
        #[arg()]
        name: Option<String>,

        /// Create a library project (no main function)
        #[arg(long)]
        lib: bool,
    },

    /// Generate API documentation from doc comments
    Doc {
        /// Files or directories to document
        #[arg(default_value = ".")]
        paths: Vec<PathBuf>,

        /// Output directory for generated docs
        #[arg(long, short, default_value = "docs/api")]
        output: PathBuf,

        /// Output format (markdown or html)
        #[arg(long, default_value = "markdown")]
        format: String,
    },

    /// Start Language Server Protocol server (for IDE integration)
    Lsp,
}

impl Cli {
    /// Run the CLI
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::parse();

        match cli.command {
            Command::Fmt { paths, check } => {
                run_fmt(&paths, check, cli.json)?;
            }
            Command::Check {
                paths,
                strict,
                no_cache,
            } => {
                run_check(&paths, strict, no_cache, cli.json)?;
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
            Command::Init { name, lib } => {
                run_init(name.as_deref(), lib)?;
            }
            Command::Doc {
                paths,
                output,
                format,
            } => {
                run_doc(&paths, &output, &format)?;
            }
            Command::Lsp => {
                crate::lsp::run_server()?;
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
    /// Serialized JSON of each diagnostic (for caching)
    diagnostic_jsons: Vec<String>,
}

fn run_check(
    paths: &[PathBuf],
    strict: bool,
    no_cache: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::cache::{self, CachedFileResult, CheckCache};

    let project_root = cache::find_project_root(
        paths
            .first()
            .map(|p| p.as_path())
            .unwrap_or_else(|| std::path::Path::new(".")),
    );
    let mut cache = if no_cache {
        CheckCache::default()
    } else {
        CheckCache::load(&project_root)
    };

    let mut total_errors = 0;
    let mut total_warnings = 0;
    let mut files_checked = 0;
    let mut files_cached = 0;

    // Collect all .astra files
    let mut astra_files = Vec::new();
    for path in paths {
        if path.is_file() && path.extension().is_some_and(|ext| ext == "astra") {
            astra_files.push(path.clone());
        } else if path.is_dir() {
            for entry in walkdir(path)? {
                if entry.extension().is_some_and(|ext| ext == "astra") {
                    astra_files.push(entry);
                }
            }
        }
    }

    for file_path in &astra_files {
        let source = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {:?}: {}", file_path, e))?;
        let content_hash = cache::hash_content(&source);

        // Try cache lookup
        if !no_cache {
            if let Some(cached) = cache.lookup(file_path, content_hash) {
                files_checked += 1;
                files_cached += 1;
                total_errors += cached.errors;
                total_warnings += cached.warnings;
                // Re-display cached diagnostics
                if json {
                    for d in &cached.diagnostics {
                        println!("{}", d);
                    }
                } else if !cached.diagnostics.is_empty() {
                    for d_json in &cached.diagnostics {
                        // Parse back and display as human-readable
                        if let Ok(d) =
                            serde_json::from_str::<crate::diagnostics::Diagnostic>(d_json)
                        {
                            eprintln!("{}", d.to_human_readable(&source));
                        }
                    }
                }
                continue;
            }
        }

        files_checked += 1;
        let counts = check_file(file_path, json)?;

        // Store result in cache
        cache.store(
            file_path,
            CachedFileResult {
                content_hash,
                errors: counts.errors,
                warnings: counts.warnings,
                diagnostics: counts.diagnostic_jsons,
            },
        );

        total_errors += counts.errors;
        total_warnings += counts.warnings;
    }

    // Save cache (unless --no-cache)
    if !no_cache {
        cache.prune();
        if let Err(e) = cache.save(&project_root) {
            eprintln!("Warning: failed to save check cache: {}", e);
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
        let cache_note = if files_cached > 0 {
            format!(" ({} cached)", files_cached)
        } else {
            String::new()
        };
        println!(
            "Checked {} file(s){}, no errors ({} warning(s))",
            files_checked, cache_note, total_warnings
        );
    } else {
        let cache_note = if files_cached > 0 {
            format!(" ({} cached)", files_cached)
        } else {
            String::new()
        };
        println!(
            "Checked {} file(s){}, no errors found",
            files_checked, cache_note
        );
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
            let _type_result = checker.check_module(&module);

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

            // Collect JSON for caching
            let diagnostic_jsons: Vec<String> = all_diags
                .diagnostics()
                .iter()
                .map(|d| d.to_json())
                .collect();

            // Display all diagnostics from the checker's bag
            let all = all_diags.diagnostics();
            if !all.is_empty() {
                for d in all {
                    if json {
                        println!("{}", d.to_json());
                    } else {
                        eprintln!("{}", d.to_human_readable(&source));
                    }
                }
            }

            Ok(CheckCounts {
                errors: errors.len(),
                warnings: warnings.len(),
                diagnostic_jsons,
            })
        }
        Err(e) => {
            let diagnostic_jsons: Vec<String> =
                e.diagnostics().iter().map(|d| d.to_json()).collect();
            if json {
                println!("{}", e.to_json());
            } else {
                eprintln!("Error in {:?}:\n{}", path, e.format_text(&source));
            }
            Ok(CheckCounts {
                errors: e.len(),
                warnings: 0,
                diagnostic_jsons,
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
    seed: Option<u64>,
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
                configure_search_paths(&mut interpreter, path.parent());
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
                let seed = seed.unwrap_or(42);
                let mut all_passed = true;

                for i in 0..num_iterations {
                    let iter_seed = seed.wrapping_add(i);
                    let mut capabilities = build_test_capabilities(&prop.using);
                    capabilities.rand = Some(Box::new(SeededRand::new(iter_seed)));

                    let mut interpreter = Interpreter::with_capabilities(capabilities);
                    configure_search_paths(&mut interpreter, path.parent());
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

/// Configure standard search paths for module resolution.
///
/// Adds the following search paths in order:
/// 1. The given base directory (usually the source file's parent)
/// 2. The current working directory
/// 3. The executable's directory (for finding stdlib relative to the binary)
fn configure_search_paths(interpreter: &mut Interpreter, base_dir: Option<&std::path::Path>) {
    // Add the base directory first (usually the source file's parent)
    if let Some(base) = base_dir {
        interpreter.add_search_path(base.to_path_buf());
    }

    // Add the current working directory
    if let Ok(cwd) = std::env::current_dir() {
        interpreter.add_search_path(cwd.clone());

        // Also check for astra.toml to find the project root
        // Walk up from cwd to find it
        let mut dir = cwd.as_path();
        loop {
            if dir.join("astra.toml").exists() {
                interpreter.add_search_path(dir.to_path_buf());
                break;
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => break,
            }
        }
    }

    // Add the executable's directory as a search path
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            interpreter.add_search_path(exe_dir.to_path_buf());
        }
    }
}

/// Mock filesystem capability for tests
struct MockFs;

impl crate::interpreter::FsCapability for MockFs {
    fn read(&self, _path: &str) -> Result<String, String> {
        Ok("mocked content".to_string())
    }

    fn write(&self, _path: &str, _content: &str) -> Result<(), String> {
        Ok(())
    }

    fn exists(&self, _path: &str) -> bool {
        true
    }
}

/// Mock network capability for tests
struct MockNet;

impl crate::interpreter::NetCapability for MockNet {
    fn get(&self, _url: &str) -> Result<crate::interpreter::Value, String> {
        Ok(crate::interpreter::Value::Text(
            "mocked response".to_string(),
        ))
    }

    fn post(&self, _url: &str, _body: &str) -> Result<crate::interpreter::Value, String> {
        Ok(crate::interpreter::Value::Text(
            "mocked response".to_string(),
        ))
    }
}

/// Build capabilities for a test based on its `using effects(...)` clause.
///
/// Supports:
/// - `Rand = Rand.seeded(<seed>)` or `Rand = seeded_rand(<seed>)` -> SeededRand
/// - `Clock = Clock.fixed(<time>)` -> FixedClock
/// - `Fs = mock_fs` or `Fs = ...` -> MockFs
/// - `Net = mock_net` or `Net = ...` -> MockNet
/// - `Console = ...` -> MockConsole (always provided)
fn build_test_capabilities(using: &Option<crate::parser::ast::UsingClause>) -> Capabilities {
    let mut capabilities = Capabilities {
        console: Some(Box::new(MockConsole::new())),
        ..Default::default()
    };

    if let Some(clause) = using {
        for binding in &clause.bindings {
            match binding.effect.as_str() {
                "Rand" => {
                    // Expect: Rand.seeded(<int>) or seeded_rand(<int>)
                    if let Some(seed) = extract_method_int_arg(&binding.value, "Rand", "seeded") {
                        capabilities.rand = Some(Box::new(SeededRand::new(seed as u64)));
                    } else if let Some(seed) = extract_call_int_arg(&binding.value, "seeded_rand") {
                        capabilities.rand = Some(Box::new(SeededRand::new(seed as u64)));
                    } else {
                        // Default seeded rand with seed 42
                        capabilities.rand = Some(Box::new(SeededRand::new(42)));
                    }
                }
                "Clock" => {
                    // Expect: Clock.fixed(<int>)
                    if let Some(time) = extract_method_int_arg(&binding.value, "Clock", "fixed") {
                        capabilities.clock = Some(Box::new(FixedClock::new(time)));
                    }
                }
                "Fs" => {
                    // Provide mock filesystem
                    capabilities.fs = Some(Box::new(MockFs));
                }
                "Net" => {
                    // Provide mock network
                    capabilities.net = Some(Box::new(MockNet));
                }
                "Console" => {
                    // Console is always provided (already set above)
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

/// Extract an integer argument from a function call expression like `foo(42)`.
fn extract_call_int_arg(expr: &crate::parser::ast::Expr, expected_fn: &str) -> Option<i64> {
    use crate::parser::ast::Expr;

    if let Expr::Call { func, args, .. } = expr {
        if let Expr::Ident { name, .. } = func.as_ref() {
            if name == expected_fn {
                if let Some(Expr::IntLit { value, .. }) = args.first() {
                    return Some(*value);
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

    // Set up capabilities — provide all real capabilities for `astra run`
    let capabilities = Capabilities {
        console: Some(Box::new(RealConsole)),
        env: Some(Box::new(RealEnv::new(args.to_vec()))),
        fs: Some(Box::new(RealFs)),
        net: Some(Box::new(RealNet)),
        clock: Some(Box::new(RealClock)),
        rand: Some(Box::new(RealRand::new())),
    };

    // Create interpreter and run
    let mut interpreter = Interpreter::with_capabilities(capabilities);
    configure_search_paths(&mut interpreter, file.parent());
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

/// Real filesystem capability that performs actual I/O
struct RealFs;

impl FsCapability for RealFs {
    fn read(&self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read '{}': {}", path, e))
    }

    fn write(&self, path: &str, content: &str) -> Result<(), String> {
        std::fs::write(path, content).map_err(|e| format!("Failed to write '{}': {}", path, e))
    }

    fn exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
}

/// Real network capability using ureq for HTTP
struct RealNet;

impl NetCapability for RealNet {
    fn get(&self, url: &str) -> Result<Value, String> {
        match ureq::get(url).call() {
            Ok(response) => {
                let body = response
                    .into_string()
                    .map_err(|e| format!("Failed to read response body: {}", e))?;
                Ok(Value::Text(body))
            }
            Err(e) => Err(format!("HTTP GET failed: {}", e)),
        }
    }

    fn post(&self, url: &str, body: &str) -> Result<Value, String> {
        match ureq::post(url).send_string(body) {
            Ok(response) => {
                let body = response
                    .into_string()
                    .map_err(|e| format!("Failed to read response body: {}", e))?;
                Ok(Value::Text(body))
            }
            Err(e) => Err(format!("HTTP POST failed: {}", e)),
        }
    }
}

/// Real clock capability using system time
struct RealClock;

impl ClockCapability for RealClock {
    fn now(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    fn sleep(&self, millis: u64) {
        std::thread::sleep(std::time::Duration::from_millis(millis));
    }
}

/// Real random capability using system randomness
struct RealRand {
    seed: std::cell::Cell<u64>,
}

impl RealRand {
    fn new() -> Self {
        // Seed from system time for non-deterministic randomness
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self {
            seed: std::cell::Cell::new(seed),
        }
    }

    fn next(&self) -> u64 {
        let mut x = self.seed.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.seed.set(x);
        x
    }
}

impl RandCapability for RealRand {
    fn int(&self, min: i64, max: i64) -> i64 {
        if max <= min {
            return min;
        }
        let range = (max - min + 1) as u64;
        let r = self.next() % range;
        min + r as i64
    }

    fn bool(&self) -> bool {
        self.next().is_multiple_of(2)
    }

    fn float(&self) -> f64 {
        (self.next() as f64) / (u64::MAX as f64)
    }
}

fn run_doc(
    paths: &[PathBuf],
    output: &PathBuf,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Collect all .astra files
    let mut astra_files = Vec::new();
    for path in paths {
        if path.is_file() && path.extension().is_some_and(|ext| ext == "astra") {
            astra_files.push(path.clone());
        } else if path.is_dir() {
            for entry in walkdir(path)? {
                if entry.extension().is_some_and(|ext| ext == "astra") {
                    astra_files.push(entry);
                }
            }
        }
    }

    if astra_files.is_empty() {
        println!("No .astra files found");
        return Ok(());
    }

    std::fs::create_dir_all(output)?;

    let mut module_docs = Vec::new();

    for file_path in &astra_files {
        let source = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {:?}: {}", file_path, e))?;

        let source_file = SourceFile::new(file_path.clone(), source.clone());
        let lexer = Lexer::new(&source_file);
        let mut parser = AstraParser::new(lexer, source_file.clone());

        let module = match parser.parse_module() {
            Ok(m) => m,
            Err(_) => continue, // Skip files with parse errors
        };

        let doc = generate_module_doc(&module, &source, file_path);
        if !doc.is_empty() {
            let module_name = module.name.segments.join(".");
            let ext = if format == "html" { "html" } else { "md" };
            let out_file = output.join(format!("{}.{}", module_name, ext));

            let content = if format == "html" {
                markdown_to_html(&doc)
            } else {
                doc.clone()
            };

            std::fs::write(&out_file, &content)?;
            module_docs.push((module_name, out_file));
        }
    }

    // Generate index
    let ext = if format == "html" { "html" } else { "md" };
    let index_path = output.join(format!("index.{}", ext));
    let mut index = String::new();
    index.push_str("# API Documentation\n\n");
    for (name, path) in &module_docs {
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        index.push_str(&format!("- [{}]({})\n", name, filename));
    }

    let index_content = if format == "html" {
        markdown_to_html(&index)
    } else {
        index
    };
    std::fs::write(&index_path, index_content)?;

    println!(
        "Generated documentation for {} module(s) in {:?}",
        module_docs.len(),
        output
    );
    Ok(())
}

/// Generate documentation for a single module.
fn generate_module_doc(
    module: &crate::parser::ast::Module,
    source: &str,
    file_path: &std::path::Path,
) -> String {
    use crate::parser::ast::*;

    let mut doc = String::new();
    let module_name = module.name.segments.join(".");
    doc.push_str(&format!("# Module `{}`\n\n", module_name));
    doc.push_str(&format!(
        "Source: `{}`\n\n",
        file_path.file_name().unwrap_or_default().to_string_lossy()
    ));

    // Extract module-level doc comments (## lines before any items)
    let module_doc = extract_doc_comment(source, 1);
    if !module_doc.is_empty() {
        doc.push_str(&module_doc);
        doc.push_str("\n\n");
    }

    // Collect items by category
    let mut functions = Vec::new();
    let mut types = Vec::new();
    let mut enums = Vec::new();
    let mut traits = Vec::new();
    let mut effects = Vec::new();

    for item in &module.items {
        match item {
            Item::FnDef(def) => functions.push(def),
            Item::TypeDef(def) => types.push(def),
            Item::EnumDef(def) => enums.push(def),
            Item::TraitDef(def) => traits.push(def),
            Item::EffectDef(def) => effects.push(def),
            _ => {}
        }
    }

    // Document types
    if !types.is_empty() {
        doc.push_str("## Types\n\n");
        for def in &types {
            let doc_comment = extract_doc_comment(source, def.span.start_line);
            doc.push_str(&format!("### `type {}`\n\n", def.name));
            doc.push_str(&format!(
                "```astra\ntype {} = {}\n```\n\n",
                def.name,
                format_type_expr_for_doc(&def.value)
            ));
            if !doc_comment.is_empty() {
                doc.push_str(&doc_comment);
                doc.push_str("\n\n");
            }
        }
    }

    // Document enums
    if !enums.is_empty() {
        doc.push_str("## Enums\n\n");
        for def in &enums {
            let doc_comment = extract_doc_comment(source, def.span.start_line);
            doc.push_str(&format!("### `enum {}`\n\n", def.name));
            doc.push_str("```astra\nenum ");
            doc.push_str(&def.name);
            doc.push_str(" {\n");
            for v in &def.variants {
                if v.fields.is_empty() {
                    doc.push_str(&format!("  {}\n", v.name));
                } else {
                    let fields: Vec<String> = v
                        .fields
                        .iter()
                        .map(|f| format!("{}: {}", f.name, format_type_expr_for_doc(&f.ty)))
                        .collect();
                    doc.push_str(&format!("  {}({})\n", v.name, fields.join(", ")));
                }
            }
            doc.push_str("}\n```\n\n");
            if !doc_comment.is_empty() {
                doc.push_str(&doc_comment);
                doc.push_str("\n\n");
            }
        }
    }

    // Document traits
    if !traits.is_empty() {
        doc.push_str("## Traits\n\n");
        for def in &traits {
            let doc_comment = extract_doc_comment(source, def.span.start_line);
            doc.push_str(&format!("### `trait {}`\n\n", def.name));
            doc.push_str("```astra\ntrait ");
            doc.push_str(&def.name);
            doc.push_str(" {\n");
            for m in &def.methods {
                let params: Vec<String> = m
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, format_type_expr_for_doc(&p.ty)))
                    .collect();
                let ret = m
                    .return_type
                    .as_ref()
                    .map(|t| format!(" -> {}", format_type_expr_for_doc(t)))
                    .unwrap_or_default();
                doc.push_str(&format!("  fn {}({}){}\n", m.name, params.join(", "), ret));
            }
            doc.push_str("}\n```\n\n");
            if !doc_comment.is_empty() {
                doc.push_str(&doc_comment);
                doc.push_str("\n\n");
            }
        }
    }

    // Document effects
    if !effects.is_empty() {
        doc.push_str("## Effects\n\n");
        for def in &effects {
            let doc_comment = extract_doc_comment(source, def.span.start_line);
            doc.push_str(&format!("### `effect {}`\n\n", def.name));
            if !doc_comment.is_empty() {
                doc.push_str(&doc_comment);
                doc.push_str("\n\n");
            }
        }
    }

    // Document functions
    let public_fns: Vec<_> = functions
        .iter()
        .filter(|f| matches!(f.visibility, Visibility::Public))
        .collect();
    let private_fns: Vec<_> = functions
        .iter()
        .filter(|f| matches!(f.visibility, Visibility::Private))
        .collect();

    if !public_fns.is_empty() {
        doc.push_str("## Public Functions\n\n");
        for def in &public_fns {
            doc.push_str(&format_fn_doc(def, source));
        }
    }

    if !private_fns.is_empty() {
        doc.push_str("## Functions\n\n");
        for def in &private_fns {
            doc.push_str(&format_fn_doc(def, source));
        }
    }

    doc
}

/// Format documentation for a single function.
fn format_fn_doc(def: &crate::parser::ast::FnDef, source: &str) -> String {
    let mut doc = String::new();
    let doc_comment = extract_doc_comment(source, def.span.start_line);

    let type_params_str = if def.type_params.is_empty() {
        String::new()
    } else {
        format!("[{}]", def.type_params.join(", "))
    };

    let params_str: Vec<String> = def
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, format_type_expr_for_doc(&p.ty)))
        .collect();

    let ret_str = def
        .return_type
        .as_ref()
        .map(|t| format!(" -> {}", format_type_expr_for_doc(t)))
        .unwrap_or_default();

    let effects_str = if def.effects.is_empty() {
        String::new()
    } else {
        format!(" effects({})", def.effects.join(", "))
    };

    doc.push_str(&format!("### `{}`\n\n", def.name));
    doc.push_str(&format!(
        "```astra\nfn {}{}({}){}{}\n```\n\n",
        def.name,
        type_params_str,
        params_str.join(", "),
        ret_str,
        effects_str
    ));
    if !doc_comment.is_empty() {
        doc.push_str(&doc_comment);
        doc.push_str("\n\n");
    }
    doc
}

/// Extract doc comments (`##` lines) immediately before the given line.
fn extract_doc_comment(source: &str, item_line: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    if item_line == 0 || item_line > lines.len() {
        return String::new();
    }

    let mut doc_lines = Vec::new();
    let mut line_idx = item_line.saturating_sub(2); // 0-indexed, line before the item

    // Walk backwards collecting ## doc comment lines
    loop {
        if line_idx >= lines.len() {
            break;
        }
        let line = lines[line_idx].trim();
        if let Some(comment) = line.strip_prefix("##") {
            doc_lines.push(comment.trim().to_string());
        } else {
            break;
        }
        if line_idx == 0 {
            break;
        }
        line_idx -= 1;
    }

    doc_lines.reverse();
    doc_lines.join("\n")
}

/// Format a TypeExpr for documentation output.
fn format_type_expr_for_doc(ty: &crate::parser::ast::TypeExpr) -> String {
    use crate::parser::ast::TypeExpr;
    match ty {
        TypeExpr::Named { name, args, .. } => {
            if args.is_empty() {
                name.clone()
            } else {
                let args_str: Vec<String> = args.iter().map(format_type_expr_for_doc).collect();
                format!("{}[{}]", name, args_str.join(", "))
            }
        }
        TypeExpr::Record { fields, .. } => {
            let fields_str: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", f.name, format_type_expr_for_doc(&f.ty)))
                .collect();
            format!("{{ {} }}", fields_str.join(", "))
        }
        TypeExpr::Function {
            params,
            ret,
            effects,
            ..
        } => {
            let params_str: Vec<String> = params.iter().map(format_type_expr_for_doc).collect();
            let effects_str = if effects.is_empty() {
                String::new()
            } else {
                format!(" effects({})", effects.join(", "))
            };
            format!(
                "({}) -> {}{}",
                params_str.join(", "),
                format_type_expr_for_doc(ret),
                effects_str
            )
        }
        TypeExpr::Tuple { elements, .. } => {
            let elems_str: Vec<String> = elements.iter().map(format_type_expr_for_doc).collect();
            format!("({})", elems_str.join(", "))
        }
    }
}

/// Basic markdown-to-HTML conversion for the `--format html` option.
fn markdown_to_html(md: &str) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\">\n");
    html.push_str("<title>Astra API Docs</title>\n");
    html.push_str("<style>body{font-family:sans-serif;max-width:800px;margin:0 auto;padding:20px}");
    html.push_str("pre{background:#f4f4f4;padding:12px;overflow-x:auto}");
    html.push_str("code{background:#f4f4f4;padding:2px 4px}</style>\n");
    html.push_str("</head><body>\n");

    let mut in_code_block = false;
    for line in md.lines() {
        if line.starts_with("```") {
            if in_code_block {
                html.push_str("</code></pre>\n");
                in_code_block = false;
            } else {
                html.push_str("<pre><code>");
                in_code_block = true;
            }
        } else if in_code_block {
            html.push_str(&line.replace('<', "&lt;").replace('>', "&gt;"));
            html.push('\n');
        } else if let Some(heading) = line.strip_prefix("### ") {
            html.push_str(&format!("<h3>{}</h3>\n", heading));
        } else if let Some(heading) = line.strip_prefix("## ") {
            html.push_str(&format!("<h2>{}</h2>\n", heading));
        } else if let Some(heading) = line.strip_prefix("# ") {
            html.push_str(&format!("<h1>{}</h1>\n", heading));
        } else if let Some(item) = line.strip_prefix("- ") {
            html.push_str(&format!("<li>{}</li>\n", item));
        } else if line.is_empty() {
            html.push_str("<br>\n");
        } else {
            html.push_str(&format!("<p>{}</p>\n", line));
        }
    }

    html.push_str("</body></html>\n");
    html
}

fn run_init(name: Option<&str>, lib: bool) -> Result<(), Box<dyn std::error::Error>> {
    let project_name = match name {
        Some(n) => n.to_string(),
        None => {
            let cwd = std::env::current_dir()?;
            cwd.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("my_project")
                .to_string()
        }
    };

    // Determine project root
    let project_dir = if name.is_some() {
        let dir = std::env::current_dir()?.join(&project_name);
        std::fs::create_dir_all(&dir)?;
        dir
    } else {
        std::env::current_dir()?
    };

    // Create src directory
    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    // Write astra.toml
    let manifest = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
description = ""
authors = []
license = "MIT"

[build]
target = "interpreter"

[lint]
level = "warn"
"#,
        name = project_name
    );
    std::fs::write(project_dir.join("astra.toml"), manifest)?;

    // Write main source file
    if lib {
        let lib_source = format!(
            r#"module {name}

## A library module for {name}.

public fn greet(who: Text) -> Text {{
  "Hello, ${{who}}!"
}}

test "greet works" {{
  assert_eq(greet("world"), "Hello, world!")
}}
"#,
            name = project_name
        );
        std::fs::write(src_dir.join("lib.astra"), lib_source)?;
    } else {
        let main_source = format!(
            r#"module {name}

fn main() effects(Console) {{
  println("Hello from {name}!")
}}

test "hello works" {{
  assert true
}}
"#,
            name = project_name
        );
        std::fs::write(src_dir.join("main.astra"), main_source)?;
    }

    // Write .gitignore
    let gitignore = "# Astra build artifacts\n/build/\n/.astra-cache/\n";
    std::fs::write(project_dir.join(".gitignore"), gitignore)?;

    if name.is_some() {
        println!("Created new Astra project '{}'", project_name);
        println!("  cd {}", project_name);
    } else {
        println!("Initialized Astra project '{}'", project_name);
    }

    if lib {
        println!("  astra test          # Run tests");
        println!("  astra check         # Type check");
    } else {
        println!("  astra run src/main.astra   # Run the program");
        println!("  astra test                 # Run tests");
        println!("  astra check                # Type check");
    }

    Ok(())
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
        let mut interp = Interpreter::with_capabilities(Capabilities {
            console: Some(Box::new(RealConsole)),
            ..Default::default()
        });
        configure_search_paths(&mut interp, None);
        interp
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Span;
    use crate::parser::ast::{Expr, NodeId};

    fn dummy_span() -> Span {
        Span::new(std::path::PathBuf::from("test.astra"), 0, 0, 1, 1, 1, 1)
    }

    #[test]
    fn test_extract_call_int_arg_match() {
        let expr = Expr::Call {
            id: NodeId::new(),
            span: dummy_span(),
            func: Box::new(Expr::Ident {
                id: NodeId::new(),
                span: dummy_span(),
                name: "seeded_rand".to_string(),
            }),
            args: vec![Expr::IntLit {
                id: NodeId::new(),
                span: dummy_span(),
                value: 42,
            }],
        };
        assert_eq!(extract_call_int_arg(&expr, "seeded_rand"), Some(42));
    }

    #[test]
    fn test_extract_call_int_arg_wrong_name() {
        let expr = Expr::Call {
            id: NodeId::new(),
            span: dummy_span(),
            func: Box::new(Expr::Ident {
                id: NodeId::new(),
                span: dummy_span(),
                name: "other_fn".to_string(),
            }),
            args: vec![Expr::IntLit {
                id: NodeId::new(),
                span: dummy_span(),
                value: 42,
            }],
        };
        assert_eq!(extract_call_int_arg(&expr, "seeded_rand"), None);
    }

    #[test]
    fn test_extract_method_int_arg_match() {
        let expr = Expr::MethodCall {
            id: NodeId::new(),
            span: dummy_span(),
            receiver: Box::new(Expr::Ident {
                id: NodeId::new(),
                span: dummy_span(),
                name: "Clock".to_string(),
            }),
            method: "fixed".to_string(),
            args: vec![Expr::IntLit {
                id: NodeId::new(),
                span: dummy_span(),
                value: 1000,
            }],
        };
        assert_eq!(extract_method_int_arg(&expr, "Clock", "fixed"), Some(1000));
    }

    #[test]
    fn test_extract_method_int_arg_wrong_receiver() {
        let expr = Expr::MethodCall {
            id: NodeId::new(),
            span: dummy_span(),
            receiver: Box::new(Expr::Ident {
                id: NodeId::new(),
                span: dummy_span(),
                name: "Other".to_string(),
            }),
            method: "fixed".to_string(),
            args: vec![Expr::IntLit {
                id: NodeId::new(),
                span: dummy_span(),
                value: 1000,
            }],
        };
        assert_eq!(extract_method_int_arg(&expr, "Clock", "fixed"), None);
    }

    #[test]
    fn test_build_test_capabilities_default() {
        let caps = build_test_capabilities(&None);
        assert!(caps.console.is_some());
        assert!(caps.rand.is_none());
        assert!(caps.clock.is_none());
    }

    #[test]
    fn test_configure_search_paths() {
        let mut interpreter = Interpreter::new();
        configure_search_paths(&mut interpreter, None);
        assert!(!interpreter.search_paths.is_empty());
    }
}
