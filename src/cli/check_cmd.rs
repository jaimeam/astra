//! Handler for the `astra check` subcommand.

use std::path::PathBuf;

use crate::parser::{Lexer, Parser as AstraParser, SourceFile};

use super::{configure_checker_search_paths, walkdir};

/// Counts returned from checking a single file
pub(super) struct CheckCounts {
    pub(super) errors: usize,
    pub(super) warnings: usize,
    /// Serialized JSON of each diagnostic (for caching)
    pub(super) diagnostic_jsons: Vec<String>,
}

pub(crate) fn run_check(
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

pub(super) fn check_file(
    path: &PathBuf,
    json: bool,
) -> Result<CheckCounts, Box<dyn std::error::Error>> {
    let source =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

    let source_file = SourceFile::new(path.clone(), source.clone());
    let lexer = Lexer::new(&source_file);
    let mut parser = AstraParser::new(lexer, source_file.clone());
    match parser.parse_module() {
        Ok(module) => {
            // Run type checking (includes exhaustiveness + effect + lint enforcement)
            let mut checker = crate::typechecker::TypeChecker::new();
            configure_checker_search_paths(&mut checker, path.parent());
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

/// Run `astra check` in watch mode -- re-check on file changes.
pub(crate) fn run_watch_check(
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
