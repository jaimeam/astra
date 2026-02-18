//! Handler for the `astra test` subcommand.

use crate::interpreter::{Capabilities, FixedClock, Interpreter, MockConsole, SeededRand};
use crate::parser::{Lexer, Parser as AstraParser, SourceFile};

use super::{configure_search_paths, json_escape, walkdir};

pub(crate) fn run_test(
    filter: Option<&str>,
    seed: Option<u64>,
    json: bool,
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

    // P4/P6: Collect results for JSON output
    let mut json_results: Vec<String> = Vec::new();

    for path in astra_files {
        let source = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

        let source_file = SourceFile::new(path.clone(), source.clone());
        let lexer = Lexer::new(&source_file);
        let mut parser = AstraParser::new(lexer, source_file.clone());

        let module = match parser.parse_module() {
            Ok(m) => m,
            Err(e) => {
                if !json {
                    eprintln!("Parse error in {:?}:\n{}", path, e.format_text(&source));
                }
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
                    if json {
                        json_results.push(format!(
                            "{{\"name\":{},\"file\":{},\"status\":\"fail\",\"error\":{}}}",
                            json_escape(&test.name),
                            json_escape(&path.display().to_string()),
                            json_escape(&e.to_string())
                        ));
                    } else {
                        eprintln!("  FAIL: {} - {}", test.name, e);
                    }
                    failed += 1;
                    continue;
                }

                // Run the test block
                match interpreter.eval_block(&test.body) {
                    Ok(_) => {
                        if json {
                            json_results.push(format!(
                                "{{\"name\":{},\"file\":{},\"status\":\"pass\"}}",
                                json_escape(&test.name),
                                json_escape(&path.display().to_string())
                            ));
                        } else {
                            println!("  PASS: {}", test.name);
                        }
                        passed += 1;
                    }
                    Err(e) => {
                        if json {
                            json_results.push(format!(
                                "{{\"name\":{},\"file\":{},\"status\":\"fail\",\"error\":{}}}",
                                json_escape(&test.name),
                                json_escape(&path.display().to_string()),
                                json_escape(&e.to_string())
                            ));
                        } else {
                            eprintln!("  FAIL: {} - {}", test.name, e);
                        }
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
                let mut fail_msg = String::new();

                for i in 0..num_iterations {
                    let iter_seed = seed.wrapping_add(i);
                    let mut capabilities = build_test_capabilities(&prop.using);
                    capabilities.rand = Some(Box::new(SeededRand::new(iter_seed)));

                    let mut interpreter = Interpreter::with_capabilities(capabilities);
                    configure_search_paths(&mut interpreter, path.parent());
                    if let Err(e) = interpreter.load_module(&module) {
                        fail_msg = format!("iteration {}: {}", i, e);
                        if !json {
                            eprintln!("  FAIL: {} (iteration {}) - {}", prop.name, i, e);
                        }
                        all_passed = false;
                        break;
                    }

                    if let Err(e) = interpreter.eval_block(&prop.body) {
                        fail_msg = format!("iteration {}, seed {}: {}", i, iter_seed, e);
                        if !json {
                            eprintln!(
                                "  FAIL: {} (iteration {}, seed {}) - {}",
                                prop.name, i, iter_seed, e
                            );
                        }
                        all_passed = false;
                        break;
                    }
                }

                if all_passed {
                    if json {
                        json_results.push(format!(
                            "{{\"name\":{},\"file\":{},\"status\":\"pass\",\"iterations\":{}}}",
                            json_escape(&prop.name),
                            json_escape(&path.display().to_string()),
                            num_iterations
                        ));
                    } else {
                        println!("  PASS: {} ({} iterations)", prop.name, num_iterations);
                    }
                    passed += 1;
                } else {
                    if json {
                        json_results.push(format!(
                            "{{\"name\":{},\"file\":{},\"status\":\"fail\",\"error\":{}}}",
                            json_escape(&prop.name),
                            json_escape(&path.display().to_string()),
                            json_escape(&fail_msg)
                        ));
                    }
                    failed += 1;
                }
            }
        }
    }

    if json {
        println!(
            "{{\"total\":{},\"passed\":{},\"failed\":{},\"results\":[{}]}}",
            total_tests,
            passed,
            failed,
            json_results.join(",")
        );
    } else {
        println!(
            "\n{} tests: {} passed, {} failed",
            total_tests, passed, failed
        );
    }

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Run `astra test` in watch mode -- re-run tests on file changes.
pub(crate) fn run_watch_test(
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
pub(super) fn build_test_capabilities(
    using: &Option<crate::parser::ast::UsingClause>,
) -> Capabilities {
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
pub(super) fn extract_method_int_arg(
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
pub(super) fn extract_call_int_arg(
    expr: &crate::parser::ast::Expr,
    expected_fn: &str,
) -> Option<i64> {
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
