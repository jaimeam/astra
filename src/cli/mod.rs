//! Command-line interface for the Astra toolchain
//!
//! Provides commands: fmt, check, test, run, package

mod check_cmd;
mod doc_cmd;
mod explain_cmd;
mod fmt_cmd;
mod init_cmd;
mod pkg_cmd;
mod repl_cmd;
pub(crate) mod run_cmd;
mod test_cmd;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::interpreter::Interpreter;

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

    /// v1.1: Package management commands
    Pkg {
        #[command(subcommand)]
        action: PkgAction,
    },
}

/// v1.1: Package management actions
#[derive(Subcommand, Debug)]
pub enum PkgAction {
    /// Install dependencies from astra.toml
    Install,

    /// Add a dependency to astra.toml
    Add {
        /// Package name
        name: String,

        /// Version requirement (e.g., "1.0", "^2.0")
        #[arg(long)]
        version: Option<String>,

        /// Git repository URL
        #[arg(long)]
        git: Option<String>,

        /// Local path
        #[arg(long)]
        path: Option<String>,
    },

    /// Remove a dependency from astra.toml
    Remove {
        /// Package name to remove
        name: String,
    },

    /// List installed packages
    List,
}

impl Cli {
    /// Run the CLI
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::parse();

        match cli.command {
            Command::Fmt { paths, check } => {
                fmt_cmd::run_fmt(&paths, check, cli.json)?;
            }
            Command::Check {
                paths,
                strict,
                no_cache,
                watch,
            } => {
                if watch {
                    check_cmd::run_watch_check(&paths, strict, no_cache, cli.json)?;
                } else {
                    check_cmd::run_check(&paths, strict, no_cache, cli.json)?;
                }
            }
            Command::Test {
                filter,
                seed,
                watch,
            } => {
                if watch {
                    test_cmd::run_watch_test(filter.as_deref(), seed, cli.json)?;
                } else {
                    test_cmd::run_test(filter.as_deref(), seed, cli.json)?;
                }
            }
            Command::Run { file, args } => {
                run_cmd::run_program(&file, &args)?;
            }
            Command::Repl => {
                repl_cmd::run_repl()?;
            }
            Command::Init { name, lib } => {
                init_cmd::run_init(name.as_deref(), lib)?;
            }
            Command::Doc {
                paths,
                output,
                format,
            } => {
                doc_cmd::run_doc(&paths, &output, &format)?;
            }
            Command::Fix {
                paths,
                only,
                dry_run,
            } => {
                explain_cmd::run_fix(&paths, only.as_deref(), dry_run, cli.json)?;
            }
            Command::Explain { code } => {
                explain_cmd::run_explain(&code)?;
            }
            Command::Lsp => {
                crate::lsp::run_server()?;
            }
            Command::Package { output, target } => {
                pkg_cmd::run_package(&output, &target)?;
            }
            Command::Pkg { action } => {
                pkg_cmd::run_pkg(action)?;
            }
        }

        Ok(())
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

/// Helper to escape a string for JSON output
fn json_escape(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{}\"", escaped)
}

/// Configure standard search paths for module resolution.
///
/// Adds the following search paths in order:
/// 1. The given base directory (usually the source file's parent)
/// 2. The current working directory
/// 3. The executable's directory (for finding stdlib relative to the binary)
///
/// B1: Configure search paths for the type checker
fn configure_checker_search_paths(
    checker: &mut crate::typechecker::TypeChecker,
    base_dir: Option<&std::path::Path>,
) {
    if let Some(base) = base_dir {
        checker.add_search_path(base.to_path_buf());
    }
    if let Ok(cwd) = std::env::current_dir() {
        checker.add_search_path(cwd.clone());
        let mut dir = cwd.as_path();
        loop {
            if dir.join("astra.toml").exists() {
                checker.add_search_path(dir.to_path_buf());
                break;
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => break,
            }
        }
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            checker.add_search_path(exe_dir.to_path_buf());
        }
    }
}

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
        assert_eq!(
            test_cmd::extract_call_int_arg(&expr, "seeded_rand"),
            Some(42)
        );
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
        assert_eq!(test_cmd::extract_call_int_arg(&expr, "seeded_rand"), None);
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
        assert_eq!(
            test_cmd::extract_method_int_arg(&expr, "Clock", "fixed"),
            Some(1000)
        );
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
        assert_eq!(
            test_cmd::extract_method_int_arg(&expr, "Clock", "fixed"),
            None
        );
    }

    #[test]
    fn test_build_test_capabilities_default() {
        let caps = test_cmd::build_test_capabilities(&None);
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
        let result = explain_cmd::get_error_explanation("E1001");
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("Type mismatch"));
    }

    #[test]
    fn test_explain_unknown_code() {
        let result = explain_cmd::get_error_explanation("E9999");
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
                explain_cmd::get_error_explanation(code).is_some(),
                "Missing explanation for {}",
                code
            );
        }
    }

    #[test]
    fn test_explain_warning_codes() {
        let result = explain_cmd::get_error_explanation("W0001");
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("Unused variable"));
    }

    #[test]
    fn test_generate_claude_md_app() {
        let md = init_cmd::generate_claude_md("my_app", false);
        assert!(md.contains("# my_app"));
        assert!(md.contains("astra check src/"));
        assert!(md.contains("astra test"));
        assert!(md.contains("astra fmt src/"));
        assert!(md.contains("astra fix src/"));
        assert!(md.contains("astra run src/main.astra"));
        assert!(md.contains("astra explain"));
        assert!(md.contains("effects"));
        assert!(md.contains("E0xxx"));
        assert!(md.contains("Option[T]"));
    }

    #[test]
    fn test_generate_claude_md_lib() {
        let md = init_cmd::generate_claude_md("my_lib", true);
        assert!(md.contains("# my_lib"));
        assert!(md.contains("astra check src/"));
        assert!(md.contains("astra test"));
        // Lib projects should not include a run command
        assert!(!md.contains("astra run src/main.astra"));
    }

    #[test]
    fn test_init_creates_claude_md() {
        let tmp = std::env::temp_dir().join("astra_test_init_claude_md");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Simulate what run_init does for the .claude directory
        let claude_dir = tmp.join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let md = init_cmd::generate_claude_md("test_project", false);
        std::fs::write(claude_dir.join("CLAUDE.md"), &md).unwrap();

        let path = tmp.join(".claude").join("CLAUDE.md");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# test_project"));
        assert!(content.contains("astra check"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
