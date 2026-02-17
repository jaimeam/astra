//! Command-line interface for the Astra toolchain
//!
//! Provides commands: fmt, check, test, run, package

use clap::{Parser, Subcommand};
use std::collections::HashSet;
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

        /// Watch for file changes and re-check automatically
        #[arg(long)]
        watch: bool,
    },

    /// Run tests
    Test {
        /// Filter tests by name
        #[arg()]
        filter: Option<String>,

        /// Random seed for deterministic tests
        #[arg(long)]
        seed: Option<u64>,

        /// Watch for file changes and re-run tests automatically
        #[arg(long)]
        watch: bool,
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

    /// Auto-apply diagnostic fix suggestions
    Fix {
        /// Files or directories to fix
        #[arg(default_value = ".")]
        paths: Vec<PathBuf>,

        /// Only apply fixes for specific error/warning codes (e.g., W0001,E1002)
        #[arg(long)]
        only: Option<String>,

        /// Show what would be fixed without modifying files
        #[arg(long)]
        dry_run: bool,
    },

    /// Explain an error or warning code in detail
    Explain {
        /// Error code to explain (e.g., E1001, W0001)
        code: String,
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
                watch,
            } => {
                if watch {
                    run_watch_check(&paths, strict, no_cache, cli.json)?;
                } else {
                    run_check(&paths, strict, no_cache, cli.json)?;
                }
            }
            Command::Test {
                filter,
                seed,
                watch,
            } => {
                if watch {
                    run_watch_test(filter.as_deref(), seed, cli.json)?;
                } else {
                    run_test(filter.as_deref(), seed, cli.json)?;
                }
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
            Command::Fix {
                paths,
                only,
                dry_run,
            } => {
                run_fix(&paths, only.as_deref(), dry_run, cli.json)?;
            }
            Command::Explain { code } => {
                run_explain(&code)?;
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

/// Run `astra check` in watch mode — re-check on file changes.
fn run_watch_check(
    paths: &[PathBuf],
    strict: bool,
    no_cache: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;
    use std::time::Duration;

    println!("Watching for changes... (Ctrl+C to stop)\n");

    // Run initial check
    let _ = run_check(paths, strict, no_cache, json);

    let (tx, rx) = mpsc::channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                    let _ = tx.send(());
                }
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(100)),
    )?;

    // Watch all provided paths
    for path in paths {
        let watch_path = if path.is_file() {
            path.parent().unwrap_or(path).to_path_buf()
        } else {
            path.clone()
        };
        watcher.watch(&watch_path, RecursiveMode::Recursive)?;
    }

    // Debounce: collect events and re-run after quiet period
    loop {
        // Wait for first event
        rx.recv()?;

        // Drain any accumulated events (debounce 200ms)
        while rx.recv_timeout(Duration::from_millis(200)).is_ok() {}

        // Clear screen and re-run
        print!("\x1B[2J\x1B[H"); // ANSI clear screen
        println!("File changed — re-checking...\n");
        let _ = run_check(paths, strict, no_cache, json);
        println!("\nWatching for changes... (Ctrl+C to stop)");
    }
}

/// Run `astra test` in watch mode — re-run tests on file changes.
fn run_watch_test(
    filter: Option<&str>,
    seed: Option<u64>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;
    use std::time::Duration;

    println!("Watching for changes... (Ctrl+C to stop)\n");

    // Run initial tests
    let _ = run_test(filter, seed, json);

    let (tx, rx) = mpsc::channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                    let _ = tx.send(());
                }
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(100)),
    )?;

    // Watch current directory
    let cwd = std::env::current_dir()?;
    watcher.watch(&cwd, RecursiveMode::Recursive)?;

    loop {
        rx.recv()?;
        while rx.recv_timeout(Duration::from_millis(200)).is_ok() {}

        print!("\x1B[2J\x1B[H");
        println!("File changed — re-running tests...\n");
        let _ = run_test(filter, seed, json);
        println!("\nWatching for changes... (Ctrl+C to stop)");
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

fn run_fix(
    paths: &[PathBuf],
    only: Option<&str>,
    dry_run: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse --only filter into a set of codes
    let code_filter: Option<HashSet<&str>> = only.map(|codes| codes.split(',').collect());

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

    let mut total_fixes = 0;
    let mut files_fixed = 0;

    for file_path in &astra_files {
        let source = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {:?}: {}", file_path, e))?;

        // Parse and check
        let source_file = SourceFile::new(file_path.clone(), source.clone());
        let lexer = Lexer::new(&source_file);
        let mut parser = AstraParser::new(lexer, source_file.clone());
        let module = match parser.parse_module() {
            Ok(m) => m,
            Err(_) => continue, // Skip files with parse errors (can't fix syntax errors)
        };

        let mut checker = crate::typechecker::TypeChecker::new();
        let _type_result = checker.check_module(&module);
        let all_diags = checker.diagnostics();

        // Collect all edits from suggestions, grouped by file
        let mut edits: Vec<crate::diagnostics::Edit> = Vec::new();
        for diag in all_diags.diagnostics() {
            // Apply code filter if specified
            if let Some(ref filter) = code_filter {
                if !filter.contains(diag.code.as_str()) {
                    continue;
                }
            }

            for suggestion in &diag.suggestions {
                for edit in &suggestion.edits {
                    edits.push(edit.clone());
                }
            }
        }

        if edits.is_empty() {
            continue;
        }

        // Sort edits by start position (descending) so we apply from end to start
        // This avoids offset invalidation
        edits.sort_by(|a, b| b.span.start.cmp(&a.span.start));

        // Deduplicate edits at the same span
        edits.dedup_by(|a, b| a.span.start == b.span.start && a.span.end == b.span.end);

        let fix_count = edits.len();
        let mut fixed_source = source.clone();

        for edit in &edits {
            // Apply edit: replace bytes from span.start..span.end with replacement
            if edit.span.start <= fixed_source.len() && edit.span.end <= fixed_source.len() {
                fixed_source.replace_range(edit.span.start..edit.span.end, &edit.replacement);
            }
        }

        if fixed_source != source {
            total_fixes += fix_count;
            files_fixed += 1;

            if dry_run {
                if json {
                    println!(
                        "{{\"file\":{},\"fixes\":{}}}",
                        serde_json::to_string(&file_path.display().to_string()).unwrap_or_default(),
                        fix_count
                    );
                } else {
                    println!(
                        "Would fix {} issue(s) in {:?}",
                        fix_count,
                        file_path.display()
                    );
                }
            } else {
                std::fs::write(file_path, &fixed_source)
                    .map_err(|e| format!("Failed to write {:?}: {}", file_path, e))?;
                if json {
                    println!(
                        "{{\"file\":{},\"fixes\":{}}}",
                        serde_json::to_string(&file_path.display().to_string()).unwrap_or_default(),
                        fix_count
                    );
                } else {
                    println!("Fixed {} issue(s) in {:?}", fix_count, file_path.display());
                }
            }
        }
    }

    if dry_run {
        println!(
            "\nDry run: {} fix(es) would be applied across {} file(s)",
            total_fixes, files_fixed
        );
    } else if total_fixes > 0 {
        println!(
            "\nApplied {} fix(es) across {} file(s)",
            total_fixes, files_fixed
        );
    } else {
        println!("No auto-fixable issues found");
    }

    Ok(())
}

fn run_explain(code: &str) -> Result<(), Box<dyn std::error::Error>> {
    let explanation = get_error_explanation(code);
    match explanation {
        Some(text) => {
            println!("{}", text);
        }
        None => {
            eprintln!("Unknown error code: {}", code);
            eprintln!();
            eprintln!("Valid error codes:");
            eprintln!("  E0xxx  Syntax/parsing errors (E0001-E0011)");
            eprintln!("  E1xxx  Type errors (E1001-E1016)");
            eprintln!("  E2xxx  Effect errors (E2001-E2007)");
            eprintln!("  E3xxx  Contract violations (E3001-E3005)");
            eprintln!("  E4xxx  Runtime errors (E4001-E4008)");
            eprintln!("  W0xxx  Warnings (W0001-W0007)");
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Get a detailed explanation for an error code.
fn get_error_explanation(code: &str) -> Option<String> {
    let explanation = match code {
        // Syntax errors
        "E0001" => {
            r#"E0001: Unexpected token

The parser encountered a token that doesn't fit the expected grammar.

Example:
  fn add(a Int) -> Int {  # expected ':', found 'Int'
    a
  }

Fix: Add the missing punctuation or correct the syntax.
"#
        }
        "E0002" => {
            r#"E0002: Unterminated string literal

A string literal was opened with `"` but never closed.

Example:
  let s = "hello

Fix: Close the string with a matching `"`.
"#
        }
        "E0003" => {
            r#"E0003: Invalid number literal

A number literal contains invalid characters.

Example:
  let n = 123abc

Fix: Ensure numbers contain only digits (and optionally one `.` for floats).
"#
        }
        "E0004" => {
            r#"E0004: Missing closing delimiter

An opening bracket, brace, or parenthesis was not closed.

Example:
  fn foo() {
    let x = (1 + 2
  }

Fix: Add the matching closing delimiter `)`, `]`, or `}`.
"#
        }
        "E0005" => {
            r#"E0005: Invalid identifier

An identifier contains invalid characters or starts incorrectly.

Fix: Identifiers must start with a letter or underscore, followed by
letters, digits, or underscores.
"#
        }
        "E0006" => {
            r#"E0006: Reserved keyword used as identifier

A reserved keyword cannot be used as a variable or function name.

Example:
  let match = 5  # 'match' is reserved

Fix: Choose a different name that isn't a keyword.
"#
        }
        "E0007" => {
            r#"E0007: Invalid escape sequence

A string contains an unrecognized escape sequence.

Example:
  let s = "hello\q"  # \q is not valid

Valid escape sequences: \n, \r, \t, \\, \"

Fix: Use a valid escape sequence or remove the backslash.
"#
        }
        "E0008" => {
            r#"E0008: Unexpected end of file

The parser reached the end of the file while still expecting more tokens.

Example:
  fn foo() {

Fix: Ensure all blocks and expressions are properly completed.
"#
        }
        "E0009" => {
            r#"E0009: Invalid module declaration

The module declaration is malformed.

Example:
  module    # missing module name

Fix: Provide a valid module path like `module my_project.utils`.
"#
        }
        "E0010" => {
            r#"E0010: Duplicate module declaration

Two modules with the same name exist in the project.

Fix: Rename one of the modules to avoid the conflict.
"#
        }
        "E0011" => {
            r#"E0011: Module not found

An import refers to a module that doesn't exist.

Example:
  import std.nonexistent

Fix: Check the module name and ensure it exists. Available stdlib modules:
  std.core, std.list, std.math, std.option, std.result, std.string,
  std.collections, std.json, std.io, std.iter, std.error, std.prelude
"#
        }

        // Type errors
        "E1001" => {
            r#"E1001: Type mismatch

The type of an expression doesn't match what was expected.

Example:
  fn add(a: Int, b: Int) -> Int {
    "hello"  # expected Int, got Text
  }

Fix: Ensure the expression has the expected type. The compiler often
suggests the correct type in the error message.
"#
        }
        "E1002" => {
            r#"E1002: Unknown identifier

A name was used that hasn't been defined in the current scope.

Example:
  fn foo() -> Int {
    bar  # 'bar' is not defined
  }

Fix: Define the variable before use, check for typos, or import the
necessary module. The compiler may suggest similar names.

This diagnostic often includes an auto-fix suggestion that can be
applied with `astra fix`.
"#
        }
        "E1003" => {
            r#"E1003: Missing type annotation

A type annotation is required but was not provided.

Fix: Add an explicit type annotation where the compiler indicates.
"#
        }
        "E1004" => {
            r#"E1004: Non-exhaustive match

A `match` expression doesn't cover all possible cases.

Example:
  match opt {
    Some(x) => x
    # missing: None => ...
  }

Fix: Add the missing patterns. The compiler lists which cases are
missing. This diagnostic may include an auto-fix suggestion.
"#
        }
        "E1005" => {
            r#"E1005: Duplicate field

A record type or literal has the same field name twice.

Fix: Remove or rename the duplicate field.
"#
        }
        "E1006" => {
            r#"E1006: Unknown field

A field name was used that doesn't exist in the record type.

Fix: Check the field name for typos and verify it exists in the type definition.
"#
        }
        "E1007" => {
            r#"E1007: Wrong argument count

A function was called with the wrong number of arguments.

Example:
  fn add(a: Int, b: Int) -> Int { a + b }
  add(1)  # expected 2 args, got 1

Fix: Provide the correct number of arguments.
"#
        }
        "E1008" => {
            r#"E1008: Cannot infer type

The compiler cannot determine the type of an expression.

Fix: Add an explicit type annotation to help the compiler.
"#
        }
        "E1009" => {
            r#"E1009: Recursive type

A type definition is recursive in a way that creates an infinite type.

Fix: Use an enum with a base case to break the recursion (e.g., a linked list).
"#
        }
        "E1010" => {
            r#"E1010: Invalid type application

Type arguments were applied to a type that doesn't accept them.

Fix: Remove the type arguments or check the type definition.
"#
        }
        "E1011" => {
            r#"E1011: Duplicate type

A type with this name is already defined.

Fix: Rename one of the type definitions.
"#
        }
        "E1012" => {
            r#"E1012: Unknown type

A type name was used that hasn't been defined.

Fix: Define the type, check for typos, or import the necessary module.
"#
        }
        "E1013" => {
            r#"E1013: Expected function

A non-function value was called as if it were a function.

Fix: Ensure the expression being called is actually a function.
"#
        }
        "E1014" => {
            r#"E1014: Expected record

An expression was used in a record context but isn't a record type.

Fix: Ensure the expression is a record type with the expected fields.
"#
        }
        "E1015" => {
            r#"E1015: Expected enum

An expression was used in an enum context but isn't an enum type.

Fix: Ensure the expression is an enum type with the expected variants.
"#
        }
        "E1016" => {
            r#"E1016: Trait constraint not satisfied

A generic function requires a type to implement a trait, but the
concrete type used at the call site doesn't.

Example:
  fn sort[T: Ord](items: List[T]) -> List[T] { ... }
  sort(["a", "b"])  # Text doesn't implement Ord

Fix: Use a type that implements the required trait, or add an
`impl TraitName for YourType` block.
"#
        }

        // Effect errors
        "E2001" => {
            r#"E2001: Effect not declared

A function uses an effect that isn't listed in its `effects(...)` clause.

Example:
  fn greet() {        # missing effects(Console)
    println("hello")  # uses Console effect
  }

Fix: Add the missing effect to the function's effects clause.
This diagnostic includes an auto-fix suggestion.
"#
        }
        "E2002" => {
            r#"E2002: Unknown effect

An effect name was used that doesn't exist.

Fix: Check the effect name. Built-in effects: Console, Fs, Net, Clock, Rand, Env.
"#
        }
        "E2003" => {
            r#"E2003: Capability not available

A function requires an effect capability that isn't provided at runtime.

Fix: Ensure the capability is provided when running the program, or mock
it in test contexts with `using effects(EffectName = ...)`.
"#
        }
        "E2004" => {
            r#"E2004: Effectful call in pure context

An effectful function was called from a function that doesn't declare effects.

Fix: Add the necessary effects to the calling function's signature.
"#
        }
        "E2005" => {
            r#"E2005: Effect mismatch

The effects used by a function don't match its declaration.

Fix: Update the effects clause to match actual usage.
"#
        }
        "E2006" => {
            r#"E2006: Effect not mockable

An effect was used in a test's `using effects(...)` clause that can't be mocked.

Fix: Use the correct mock constructor (e.g., Clock.fixed(100), Rand.seeded(42)).
"#
        }
        "E2007" => {
            r#"E2007: Invalid capability injection

A capability was injected incorrectly in a `using effects(...)` clause.

Fix: Check the syntax and use the correct constructor.
"#
        }

        // Contract errors
        "E3001" => {
            r#"E3001: Precondition violation

A function's `requires` contract was violated at call time.

Example:
  fn divide(a: Int, b: Int) -> Int
    requires b != 0
  { a / b }

  divide(10, 0)  # E3001: requires b != 0

Fix: Ensure the arguments satisfy the precondition before calling.
"#
        }
        "E3002" => {
            r#"E3002: Postcondition violation

A function's `ensures` contract was violated on return.

Fix: The function's implementation doesn't satisfy its contract. Fix the
implementation to ensure the return value meets the postcondition.
"#
        }
        "E3003" => {
            r#"E3003: Invariant violation

A type's invariant was violated during construction.

Example:
  type Positive = Int invariant self > 0
  let p: Positive = -5  # E3003: invariant self > 0 violated

Fix: Ensure the value satisfies the type's invariant.
"#
        }
        "E3004" => {
            r#"E3004: Invalid contract expression

A contract expression (requires/ensures/invariant) is malformed.

Fix: Ensure the contract is a valid boolean expression.
"#
        }
        "E3005" => {
            r#"E3005: Contract binding unavailable

A contract references a variable that isn't in scope.

Fix: Only reference function parameters in `requires`, and `result` plus
parameters in `ensures`.
"#
        }

        // Runtime errors
        "E4001" => {
            r#"E4001: Division by zero

An integer or float division by zero was attempted.

Fix: Check that the divisor is non-zero before dividing.
Use a `requires` contract to enforce this statically.
"#
        }
        "E4002" => {
            r#"E4002: Index out of bounds

A list or string was accessed with an index outside its valid range.

Fix: Ensure the index is within bounds (0 to len-1).
"#
        }
        "E4003" => {
            r#"E4003: Contract violation

A contract check failed at runtime (general).

Fix: See E3001-E3003 for specific contract violation types.
"#
        }
        "E4004" => {
            r#"E4004: Resource limit exceeded

A resource limit (memory, recursion depth, etc.) was exceeded.

Fix: Reduce the size of the computation or optimize the algorithm.
"#
        }
        "E4005" => {
            r#"E4005: Capability denied

An effect capability was requested but not available.

Fix: Ensure the required capability is provided. When running with
`astra run`, all capabilities are available. In tests, mock them with
`using effects(...)`.
"#
        }
        "E4006" => {
            r#"E4006: Integer overflow

An integer operation overflowed the 64-bit range.

Fix: Use smaller values or check for overflow before the operation.
"#
        }
        "E4007" => {
            r#"E4007: Stack overflow

Too many nested function calls caused a stack overflow.

Fix: Use tail recursion (the compiler optimizes tail-recursive calls
automatically) or convert to an iterative approach.
"#
        }
        "E4008" => {
            r#"E4008: Assertion failed

An `assert` expression evaluated to false.

Example:
  assert(x > 0, "x must be positive")

Fix: Ensure the asserted condition holds, or fix the logic that
produces the incorrect value.
"#
        }

        // Warnings
        "W0001" => {
            r#"W0001: Unused variable

A variable was defined but never used.

Example:
  let x = 42  # x is never used

Fix: Remove the variable, or prefix its name with `_` to indicate
it's intentionally unused.

This warning includes an auto-fix suggestion (`astra fix`).
"#
        }
        "W0002" => {
            r#"W0002: Unused import

An import statement brings a name into scope that is never used.

Fix: Remove the unused import.

This warning includes an auto-fix suggestion (`astra fix`).
"#
        }
        "W0003" => {
            r#"W0003: Unreachable code

Code after a `return` statement can never be executed.

Example:
  fn foo() -> Int {
    return 42
    let x = 10  # unreachable
  }

Fix: Remove the unreachable code or restructure the control flow.
"#
        }
        "W0004" => {
            r#"W0004: Deprecated

A deprecated feature or function is being used.

Fix: Use the recommended replacement. Check the deprecation notice
for migration guidance.
"#
        }
        "W0005" => {
            r#"W0005: Wildcard match

A match expression uses a wildcard `_` pattern. While valid, this may
hide missing cases when new variants are added to an enum.

Fix: Consider matching all variants explicitly for better exhaustiveness.
"#
        }
        "W0006" => {
            r#"W0006: Shadowed binding

A new variable shadows an existing binding with the same name.

Fix: Rename the inner variable to avoid confusion, or prefix the
outer one with `_` if it's intentionally replaced.
"#
        }
        "W0007" => {
            r#"W0007: Redundant type annotation

A type annotation is provided but matches the inferred type exactly.

Fix: Remove the type annotation to reduce visual noise, or keep it
for documentation purposes.
"#
        }
        "W0008" => {
            r#"W0008: Unused function

A private function is defined but never called within its module.

Example:
  fn unused_helper() -> Int {
    42
  }

  fn main() -> Int {
    0  # unused_helper is never called
  }

Fix: Remove the function, prefix its name with `_` to indicate it's
intentionally unused, or make it `public` if it's part of the module's API.
"#
        }
        _ => return None,
    };
    Some(explanation.to_string())
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

    #[test]
    fn test_explain_known_code() {
        let result = get_error_explanation("E1001");
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("Type mismatch"));
    }

    #[test]
    fn test_explain_unknown_code() {
        let result = get_error_explanation("E9999");
        assert!(result.is_none());
    }

    #[test]
    fn test_explain_all_error_codes() {
        // Verify all documented error codes have explanations
        let codes = [
            "E0001", "E0002", "E0003", "E0004", "E0005", "E0006", "E0007", "E0008", "E0009",
            "E0010", "E0011", "E1001", "E1002", "E1003", "E1004", "E1005", "E1006", "E1007",
            "E1008", "E1009", "E1010", "E1011", "E1012", "E1013", "E1014", "E1015", "E1016",
            "E2001", "E2002", "E2003", "E2004", "E2005", "E2006", "E2007", "E3001", "E3002",
            "E3003", "E3004", "E3005", "E4001", "E4002", "E4003", "E4004", "E4005", "E4006",
            "E4007", "E4008", "W0001", "W0002", "W0003", "W0004", "W0005", "W0006", "W0007",
            "W0008",
        ];
        for code in &codes {
            assert!(
                get_error_explanation(code).is_some(),
                "Missing explanation for {}",
                code
            );
        }
    }

    #[test]
    fn test_explain_warning_codes() {
        let result = get_error_explanation("W0001");
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("Unused variable"));
    }
}
