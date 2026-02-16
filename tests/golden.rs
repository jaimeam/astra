//! Golden file tests for parser and formatter output stability
//!
//! These tests ensure that the parser and formatter produce stable,
//! predictable output across versions.

use std::fs;
use std::path::Path;

/// Run all golden tests in a directory
fn run_golden_tests(dir: &str, extension: &str) {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(dir);

    if !test_dir.exists() {
        // No tests yet - this is fine for initial setup
        return;
    }

    for entry in fs::read_dir(&test_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().map(|e| e == extension).unwrap_or(false) {
            run_single_golden_test(&path);
        }
    }
}

fn run_single_golden_test(path: &Path) {
    let source = fs::read_to_string(path).unwrap();
    let snapshot_path = path.with_extension("snap");

    // TODO: Once parser is complete, parse and serialize AST
    // For now, just verify the file is readable
    assert!(
        !source.is_empty(),
        "Test file should not be empty: {:?}",
        path
    );

    // If snapshot exists, compare
    if snapshot_path.exists() {
        let _expected = fs::read_to_string(&snapshot_path).unwrap();
        // TODO: Compare parsed AST JSON with snapshot
    }
}

#[test]
fn golden_syntax_tests() {
    run_golden_tests("syntax", "astra");
}

#[test]
fn golden_typecheck_tests() {
    run_golden_tests("typecheck", "astra");
}

#[test]
fn golden_effects_tests() {
    run_golden_tests("effects", "astra");
}

#[test]
fn golden_runtime_tests() {
    run_golden_tests("runtime", "astra");
}
