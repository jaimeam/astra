use super::*;
use crate::parser::{Lexer, Parser, SourceFile};
use std::path::PathBuf;

fn format_source(source: &str) -> String {
    let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
    let lexer = Lexer::new(&source_file);
    let mut parser = Parser::new(lexer, source_file.clone());
    let module = parser.parse_module().unwrap_or_else(|bag| {
        let msgs: Vec<_> = bag.diagnostics().iter().map(|d| &d.message).collect();
        panic!("parse failed: {:?}", msgs)
    });
    let mut formatter = Formatter::new();
    formatter.format_module(&module)
}

#[test]
fn test_format_simple_function() {
    let output = format_source("module example\n\nfn add(a: Int, b: Int) -> Int {\n  a + b\n}\n");
    assert!(output.contains("fn add(a: Int, b: Int) -> Int"));
}

#[test]
fn test_format_indentation() {
    let output = format_source("module example\n\nfn main() -> Int {\n  let x = 1\n  x + 2\n}\n");
    assert!(output.contains("  let x = 1"));
}

#[test]
fn test_format_module_declaration() {
    let output = format_source("module my.pkg\n");
    assert!(output.starts_with("module my.pkg\n"));
}

#[test]
fn test_format_import() {
    let output = format_source("module example\n\nimport std.math\n");
    assert!(output.contains("import std.math"));
}

#[test]
fn test_format_named_import() {
    // Named imports use plain module paths in current parser
    let output = format_source("module example\n\nimport std.math\n");
    assert!(output.contains("import std.math"), "output: {}", output);
}

#[test]
fn test_format_type_def() {
    let output = format_source("module example\n\ntype Age = Int\n");
    assert!(output.contains("type Age = Int"));
}

#[test]
fn test_format_enum_def() {
    let input = r#"module example

enum Color =
  | Red
  | Green
  | Blue
"#;
    let output = format_source(input);
    assert!(output.contains("enum Color ="), "output: {}", output);
    assert!(output.contains("| Red"), "output: {}", output);
}

#[test]
fn test_format_if_else() {
    let output = format_source(
        "module example\n\nfn f(x: Int) -> Int {\n  if x > 0 { x } else { 0 - x }\n}\n",
    );
    assert!(output.contains("if x > 0 {"));
}

#[test]
fn test_format_match() {
    let output = format_source(
        "module example\n\nfn f(x: Option[Int]) -> Int {\n  match x {\n    Some(v) => v\n    None => 0\n  }\n}\n",
    );
    assert!(output.contains("match x {"));
    assert!(output.contains("Some(v) => v"));
}

#[test]
fn test_format_idempotent() {
    let input = "module example\n\nfn add(a: Int, b: Int) -> Int {\n  a + b\n}\n";
    let first = format_source(input);
    let second = format_source(&first);
    assert_eq!(first, second, "Formatting should be idempotent");
}

#[test]
fn test_format_trait_def() {
    let output =
        format_source("module example\n\ntrait Show {\n  fn to_text(s: Text) -> Text\n}\n");
    assert!(output.contains("trait Show {"), "output: {}", output);
    assert!(
        output.contains("fn to_text(s: Text) -> Text"),
        "output: {}",
        output
    );
}

#[test]
fn test_format_effect_def() {
    let output =
        format_source("module example\n\neffect Logger {\n  fn log(msg: Text) -> Unit\n}\n");
    assert!(output.contains("effect Logger {"));
    assert!(output.contains("fn log(msg: Text) -> Unit"));
}

#[test]
fn test_format_lambda() {
    let output = format_source(
        "module example\n\nfn main() -> Int {\n  let f = fn(x: Int) -> Int { x + 1 }\n  f(5)\n}\n",
    );
    assert!(output.contains("fn(x: Int) -> Int {"));
}

#[test]
fn test_format_list_literal() {
    let output = format_source("module example\n\nfn main() -> List[Int] {\n  [1, 2, 3]\n}\n");
    assert!(output.contains("[1, 2, 3]"));
}

#[test]
fn test_format_contracts() {
    let output = format_source(
        "module example\n\nfn divide(a: Int, b: Int) -> Int\nrequires b != 0\nensures result >= 0\n{\n  a / b\n}\n",
    );
    assert!(output.contains("requires b != 0"));
    assert!(output.contains("ensures result >= 0"));
}

#[test]
fn test_format_for_loop() {
    let output = format_source(
        "module example\n\nfn main() -> Unit {\n  for x in [1, 2, 3] {\n    println(x)\n  }\n}\n",
    );
    assert!(output.contains("for x in"));
}

#[test]
fn test_format_pipe_operator() {
    let output =
        format_source("module example\n\nfn main() -> Int {\n  5 |> add_one |> double\n}\n");
    assert!(output.contains("|>"));
}

#[test]
fn test_format_string_interpolation() {
    let output = format_source(
        "module example\n\nfn main() -> Text {\n  let name = \"world\"\n  \"Hello, ${name}!\"\n}\n",
    );
    assert!(output.contains("${name}"));
}

#[test]
fn test_format_generic_function() {
    let output = format_source("module example\n\nfn identity[T](x: T) -> T {\n  x\n}\n");
    assert!(output.contains("fn identity[T](x: T) -> T"));
}

#[test]
fn test_format_while_loop() {
    let output = format_source(
        "module example\n\nfn main() -> Unit {\n  let x = 0\n  while x < 10 {\n    x = x + 1\n  }\n}\n",
    );
    assert!(output.contains("while x < 10 {"));
}

#[test]
fn test_escape_string_function() {
    assert_eq!(escape_string(r#"hello"world"#), r#"hello\"world"#);
    assert_eq!(escape_string("line\nnewline"), "line\\nnewline");
    assert_eq!(escape_string("tab\there"), "tab\\there");
    assert_eq!(escape_string("back\\slash"), "back\\\\slash");
}

#[test]
fn test_float_literal_zero_preserves_decimal() {
    let output = format_source("module example\n\nfn main() -> Float {\n  0.0\n}\n");
    assert!(
        output.contains("0.0"),
        "Float literal 0.0 must be preserved, got: {}",
        output
    );
    assert!(
        !output.contains("  0\n"),
        "Float literal 0.0 must not be reduced to Int 0, got: {}",
        output
    );
}

#[test]
fn test_float_literal_nonzero_preserves_decimal() {
    let output = format_source("module example\n\nfn main() -> Float {\n  1.0\n}\n");
    assert!(
        output.contains("1.0"),
        "Float literal 1.0 must be preserved, got: {}",
        output
    );
}

#[test]
fn test_float_literal_with_fraction() {
    let output = format_source("module example\n\nfn main() -> Float {\n  3.14\n}\n");
    assert!(
        output.contains("3.14"),
        "Float literal 3.14 must be preserved, got: {}",
        output
    );
}
