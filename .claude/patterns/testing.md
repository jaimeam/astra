# Pattern: Testing in Astra Toolchain

## Unit Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specific_behavior() {
        // Arrange
        let input = "...";

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

## Golden File Tests

For parser and formatter output:

```rust
// tests/golden.rs

use std::path::Path;

fn run_golden_test(input_path: &Path, expected_path: &Path) {
    let input = std::fs::read_to_string(input_path).unwrap();
    let actual = process(&input);

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(expected_path, &actual).unwrap();
    } else {
        let expected = std::fs::read_to_string(expected_path).unwrap();
        assert_eq!(actual, expected, "Golden test failed for {:?}", input_path);
    }
}

#[test]
fn golden_parser() {
    for entry in glob::glob("tests/syntax/*.astra").unwrap() {
        let input_path = entry.unwrap();
        let expected_path = input_path.with_extension("ast.json");
        run_golden_test(&input_path, &expected_path);
    }
}
```

## Snapshot Testing for Diagnostics

```rust
#[test]
fn test_type_error_message() {
    let code = r#"
        fn add(a: Int) -> Int {
            a + "hello"
        }
    "#;

    let diagnostics = check(code);

    insta::assert_snapshot!(format_diagnostics(&diagnostics));
}
```

## Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn formatter_roundtrip(code in valid_astra_code()) {
        let formatted1 = format(&parse(&code));
        let formatted2 = format(&parse(&formatted1));
        assert_eq!(formatted1, formatted2);
    }

    #[test]
    fn lexer_span_coverage(code in any_astra_code()) {
        let tokens = tokenize(&code);
        let total_len: usize = tokens.iter().map(|t| t.span.len()).sum();
        // Tokens should cover all non-whitespace
        assert!(total_len <= code.len());
    }
}
```

## Test Helpers

```rust
// tests/helpers/mod.rs

pub fn parse_expr(code: &str) -> Expr {
    let full = format!("fn test() {{ {} }}", code);
    let ast = parse(&full).unwrap();
    // Extract expression from function body
    ast.items[0].body.stmts[0].as_expr().clone()
}

pub fn check_expr(code: &str) -> Result<Type, Diagnostic> {
    let expr = parse_expr(code);
    typecheck_expr(&expr, &Context::default())
}

pub fn assert_type_error(code: &str, expected_code: &str) {
    match check_expr(code) {
        Err(e) => assert_eq!(e.code, expected_code),
        Ok(t) => panic!("Expected error {}, got type {}", expected_code, t),
    }
}
```

## Integration Test Structure

```rust
// tests/integration/mod.rs

#[test]
fn test_full_pipeline() {
    let code = include_str!("fixtures/example.astra");

    // Parse
    let ast = parse(code).expect("Parse failed");

    // Type check
    let typed_ast = typecheck(&ast).expect("Type check failed");

    // Run
    let result = interpret(&typed_ast).expect("Runtime error");

    assert_eq!(result, Value::Int(42));
}
```

## Test Organization

```
tests/
├── syntax/           # Parser golden tests
│   ├── literals.astra
│   ├── literals.ast.json
│   ├── functions.astra
│   └── functions.ast.json
├── typecheck/        # Type checker tests
│   ├── basic.rs
│   ├── inference.rs
│   └── errors.rs
├── effects/          # Effect system tests
├── runtime/          # Interpreter tests
├── golden.rs         # Golden test runner
├── integration.rs    # Full pipeline tests
└── helpers/          # Shared test utilities
```
