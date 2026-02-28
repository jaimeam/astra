use super::*;
use crate::parser::{Lexer, Parser, SourceFile};
use std::path::PathBuf;

fn parse_module(source: &str) -> Module {
    let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
    let lexer = Lexer::new(&source_file);
    let mut parser = Parser::new(lexer, source_file.clone());
    parser.parse_module().expect("parse failed")
}

fn check_module(source: &str) -> Result<(), DiagnosticBag> {
    let module = parse_module(source);
    let mut checker = TypeChecker::new();
    checker.check_module(&module)
}

/// Check a module and return all diagnostics (errors + warnings),
/// used for testing lint rules that produce warnings.
fn check_module_all_diags(source: &str) -> DiagnosticBag {
    let module = parse_module(source);
    let mut checker = TypeChecker::new();
    let _ = checker.check_module(&module);
    checker.diagnostics().clone()
}

#[test]
fn test_type_env() {
    let mut env = TypeEnv::new();
    env.define("x".to_string(), Type::Int);

    assert_eq!(env.lookup("x"), Some(&Type::Int));
    assert_eq!(env.lookup("y"), None);
}

#[test]
fn test_child_env() {
    let mut parent = TypeEnv::new();
    parent.define("x".to_string(), Type::Int);

    let mut child = parent.child();
    child.define("y".to_string(), Type::Bool);

    assert_eq!(child.lookup("x"), Some(&Type::Int));
    assert_eq!(child.lookup("y"), Some(&Type::Bool));
}

// C2: Exhaustive match checking tests

#[test]
fn test_exhaustive_option_match() {
    let source = r#"
module example

fn main() -> Int {
  let x: Option[Int] = Some(42)
  match x {
Some(n) => n
None => 0
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "exhaustive Option match should pass");
}

#[test]
fn test_non_exhaustive_option_missing_none() {
    let source = r#"
module example

fn main() -> Int {
  let x: Option[Int] = Some(42)
  match x {
Some(n) => n
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_err(), "missing None should be an error");
    let diags = result.unwrap_err();
    let d = &diags.diagnostics()[0];
    assert_eq!(d.code, "E1004");
    assert!(
        d.message.contains("None"),
        "error should mention missing None"
    );
}

#[test]
fn test_non_exhaustive_option_missing_some() {
    let source = r#"
module example

fn main() -> Int {
  let x: Option[Int] = Some(42)
  match x {
None => 0
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_err(), "missing Some should be an error");
    let diags = result.unwrap_err();
    let d = &diags.diagnostics()[0];
    assert_eq!(d.code, "E1004");
    assert!(
        d.message.contains("Some"),
        "error should mention missing Some"
    );
}

#[test]
fn test_exhaustive_result_match() {
    let source = r#"
module example

fn main() -> Int {
  let x: Result[Int, Text] = Ok(42)
  match x {
Ok(n) => n
Err(e) => 0
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "exhaustive Result match should pass");
}

#[test]
fn test_non_exhaustive_result_missing_err() {
    let source = r#"
module example

fn main() -> Int {
  let x: Result[Int, Text] = Ok(42)
  match x {
Ok(n) => n
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_err(), "missing Err should be an error");
    let diags = result.unwrap_err();
    let d = &diags.diagnostics()[0];
    assert_eq!(d.code, "E1004");
    assert!(
        d.message.contains("Err"),
        "error should mention missing Err"
    );
}

#[test]
fn test_exhaustive_bool_match() {
    let source = r#"
module example

fn main() -> Int {
  match true {
true => 1
false => 0
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "exhaustive Bool match should pass");
}

#[test]
fn test_non_exhaustive_bool_match() {
    let source = r#"
module example

fn main() -> Int {
  match true {
true => 1
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_err(), "missing false should be an error");
    let diags = result.unwrap_err();
    let d = &diags.diagnostics()[0];
    assert_eq!(d.code, "E1004");
    assert!(d.message.contains("false"));
}

#[test]
fn test_wildcard_covers_all() {
    let source = r#"
module example

fn main() -> Int {
  let x: Option[Int] = Some(42)
  match x {
Some(n) => n
_ => 0
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "wildcard should cover remaining patterns");
}

#[test]
fn test_ident_covers_all() {
    let source = r#"
module example

fn main() -> Int {
  match 42 {
x => x
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "identifier pattern should cover all");
}

#[test]
fn test_non_exhaustive_has_suggestion() {
    let source = r#"
module example

fn main() -> Int {
  let x: Option[Int] = Some(42)
  match x {
Some(n) => n
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_err());
    let diags = result.unwrap_err();
    let d = &diags.diagnostics()[0];
    assert!(
        !d.suggestions.is_empty(),
        "error should include a suggestion"
    );
    assert!(
        d.suggestions[0].title.contains("None"),
        "suggestion should mention the missing case"
    );
}

// C4: Effect enforcement tests

#[test]
fn test_effect_declared_correctly() {
    let source = r#"
module example

fn greet() effects(Console) {
  Console.println("hello")
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "declared effect should pass");
}

#[test]
fn test_effect_not_declared() {
    let source = r#"
module example

fn greet() {
  Console.println("hello")
}
"#;
    let result = check_module(source);
    assert!(result.is_err(), "undeclared effect should be an error");
    let diags = result.unwrap_err();
    let d = &diags.diagnostics()[0];
    assert_eq!(d.code, "E2001");
    assert!(d.message.contains("Console"));
}

#[test]
fn test_effect_enforcement_multiple_effects() {
    let source = r#"
module example

fn do_stuff() effects(Console, Fs) {
  Console.println("reading file")
  Fs.read("test.txt")
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "all effects declared should pass");
}

#[test]
fn test_effect_enforcement_missing_one_effect() {
    let source = r#"
module example

fn do_stuff() effects(Console) {
  Console.println("reading file")
  Fs.read("test.txt")
}
"#;
    let result = check_module(source);
    assert!(result.is_err(), "missing Fs effect should be an error");
    let diags = result.unwrap_err();
    assert!(
        diags
            .diagnostics()
            .iter()
            .any(|d| d.code == "E2001" && d.message.contains("Fs")),
        "should report missing Fs effect"
    );
}

#[test]
fn test_pure_function_no_effects() {
    let source = r#"
module example

fn add(a: Int, b: Int) -> Int {
  a + b
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "pure function should pass");
}

// H5: Type inference for let bindings
#[test]
fn test_let_without_type_annotation() {
    let source = r#"
module example

fn main() -> Int {
  let x = 42
  let y = x + 8
  y
}
"#;
    let result = check_module(source);
    assert!(
        result.is_ok(),
        "let without type annotation should pass type checking"
    );
}

// C3: Error suggestion tests

#[test]
fn test_effect_error_has_suggestion() {
    let source = r#"
module example

fn greet() {
  Console.println("hello")
}
"#;
    let result = check_module(source);
    assert!(result.is_err());
    let diags = result.unwrap_err();
    let d = &diags.diagnostics()[0];
    assert!(
        !d.suggestions.is_empty(),
        "effect error should include a suggestion"
    );
    assert!(
        d.suggestions[0].title.contains("effects"),
        "suggestion should mention adding effects declaration"
    );
}

// =========================================================================
// Lint tests (W0001-W0007)
// =========================================================================

// W0001: Unused variable

#[test]
fn test_lint_unused_variable() {
    let source = r#"
module example

fn main() -> Int {
  let unused = 42
  0
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0001")
        .collect();
    assert!(!warnings.is_empty(), "should warn about unused variable");
    assert!(warnings[0].message.contains("unused"));
}

#[test]
fn test_lint_used_variable_no_warning() {
    let source = r#"
module example

fn add(a: Int, b: Int) -> Int {
  a + b
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0001")
        .collect();
    assert!(
        warnings.is_empty(),
        "used variables should not generate warnings"
    );
}

#[test]
fn test_lint_underscore_prefix_suppresses_unused() {
    let source = r#"
module example

fn main() -> Int {
  let _ignored = 42
  0
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0001")
        .collect();
    assert!(
        warnings.is_empty(),
        "underscore-prefixed variables should not warn"
    );
}

// W0003: Unreachable code

#[test]
fn test_lint_unreachable_code_after_return() {
    let source = r#"
module example

fn main() -> Int {
  return 1
  let x = 2
  x
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0003")
        .collect();
    assert!(
        !warnings.is_empty(),
        "should warn about unreachable code after return"
    );
}

#[test]
fn test_lint_no_unreachable_code() {
    let source = r#"
module example

fn main() -> Int {
  let x = 2
  x
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0003")
        .collect();
    assert!(warnings.is_empty(), "no unreachable code warning expected");
}

// W0005: Wildcard match on known type

#[test]
fn test_lint_wildcard_match_on_option() {
    let source = r#"
module example

fn main() -> Int {
  let x: Option[Int] = Some(42)
  match x {
Some(n) => n
_ => 0
  }
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0005")
        .collect();
    assert!(
        !warnings.is_empty(),
        "should warn about wildcard on Option type"
    );
}

// W0006: Shadowed binding

#[test]
fn test_lint_shadowed_binding() {
    let source = r#"
module example

fn main() -> Int {
  let x = 1
  let x = 2
  x
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0006")
        .collect();
    assert!(!warnings.is_empty(), "should warn about shadowed binding");
    assert!(warnings[0].message.contains("shadows"));
}

#[test]
fn test_lint_no_shadowing_different_names() {
    let source = r#"
module example

fn main() -> Int {
  let x = 1
  let y = 2
  x + y
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0006")
        .collect();
    assert!(
        warnings.is_empty(),
        "different names should not trigger shadowing warning"
    );
}

// W0002: Unused import

#[test]
fn test_lint_unused_import() {
    let source = r#"
module example

import std.math

fn main() -> Int {
  42
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0002")
        .collect();
    assert!(!warnings.is_empty(), "should warn about unused import");
    assert!(warnings[0].message.contains("math"));
}

// Integration: lint warnings don't block compilation

#[test]
fn test_lint_warnings_dont_cause_errors() {
    let source = r#"
module example

fn main() -> Int {
  let unused = 42
  let _ok = 1
  _ok
}
"#;
    let result = check_module(source);
    assert!(
        result.is_ok(),
        "lint warnings should not cause check_module to return Err"
    );
    let diags = check_module_all_diags(source);
    assert!(diags.has_warnings(), "should have warnings");
    assert!(!diags.has_errors(), "should not have errors");
}

// Enum constructor resolution

#[test]
fn test_enum_variant_constructors_resolved() {
    let source = r#"
module example

enum Shape =
  | Circle(radius: Float)
  | Rectangle(width: Float, height: Float)

fn main() -> Float {
  let c = Circle(5.0)
  let r = Rectangle(3.0, 4.0)
  0.0
}
"#;
    let result = check_module(source);
    assert!(
        result.is_ok(),
        "enum variant constructors should be resolved: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn test_enum_nullary_variant_resolved() {
    let source = r#"
module example

enum Color = | Red | Green | Blue

fn pick() -> Color {
  Red
}
"#;
    let result = check_module(source);
    assert!(
        result.is_ok(),
        "nullary enum variants should be resolved: {:?}",
        result.unwrap_err()
    );
}

// Tuple type parsing

#[test]
fn test_tuple_type_in_function_signature() {
    let source = r#"
module example

fn swap(pair: (Int, Int)) -> (Int, Int) {
  (pair.1, pair.0)
}
"#;
    let result = check_module(source);
    assert!(
        result.is_ok(),
        "tuple types in signatures should parse and check: {:?}",
        result.unwrap_err()
    );
}

// R9: check_typedef and check_enumdef tests

#[test]
fn test_typedef_well_formed() {
    let source = r#"
module example

type Name = Text

fn greet(n: Name) -> Text {
  n
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "well-formed type def should pass");
}

#[test]
fn test_typedef_with_invariant() {
    let source = r#"
module example

type Positive = Int invariant self > 0

fn double(x: Positive) -> Int {
  x + x
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "type def with valid invariant should pass");
}

#[test]
fn test_enumdef_well_formed() {
    let source = r#"
module example

enum Direction =
  | North
  | South
  | East
  | West

fn describe(d: Direction) -> Text {
  match d {
North => "north"
South => "south"
East => "east"
West => "west"
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "well-formed enum def should pass");
}

#[test]
fn test_enumdef_with_fields_well_formed() {
    let source = r#"
module example

enum Expr =
  | Num(value: Int)
  | Add(left: Int, right: Int)

fn eval(e: Expr) -> Int {
  match e {
Num(v) => v
Add(l, r) => l + r
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "enum def with fields should pass");
}

// R10: Import resolution tests

#[test]
fn test_import_module_registers_name() {
    let source = r#"
module example

import std.math

fn main() -> Int {
  let _m = math
  0
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "imported module name should be resolvable");
}

#[test]
fn test_import_alias_registers_name() {
    let source = r#"
module example

import std.math as M

fn main() -> Int {
  let _m = M
  0
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "import alias should be resolvable");
}

#[test]
fn test_unknown_identifier_still_errors() {
    let source = r#"
module example

fn main() -> Int {
  totally_unknown + 1
}
"#;
    let result = check_module(source);
    assert!(result.is_err(), "unknown identifier should error");
    let diags = result.unwrap_err();
    assert!(
        diags.diagnostics().iter().any(|d| d.code == "E1002"),
        "should report E1002 unknown identifier"
    );
}

// Generic type checking tests

#[test]
fn test_generic_type_param_resolution() {
    let source = r#"
module example

fn identity[T](x: T) -> T {
  x
}

fn main() -> Int {
  identity(42)
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "Generic function should type-check");
}

#[test]
fn test_generic_return_type_inference() {
    // When calling a generic fn, the return type should be inferred
    let source = r#"
module example

fn first[T](items: List[T]) -> T {
  items
}

fn main() -> Int {
  let nums = [1, 2, 3]
  first(nums)
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "Generic return type inference should work");
}

#[test]
fn test_type_param_in_scope() {
    let source = r#"
module example

fn pair[A, B](a: A, b: B) -> (A, B) {
  (a, b)
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "Multiple type params should be in scope");
}

// Trait/impl type checking tests

#[test]
fn test_trait_definition() {
    let source = r#"
module example

trait Show {
  fn show(self: Self) -> Text
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "Trait definition should type-check");
}

#[test]
fn test_impl_block_methods_checked() {
    let source = r#"
module example

trait Show {
  fn show(self: Self) -> Text
}

impl Show for Int {
  fn show(self: Int) -> Text {
"int"
  }
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "Impl block should type-check");
}

#[test]
fn test_impl_missing_method() {
    let source = r#"
module example

trait Describe {
  fn describe(self: Self) -> Text
  fn summary(self: Self) -> Text
}

impl Describe for Int {
  fn describe(self: Int) -> Text {
"an integer"
  }
}
"#;
    let _result = check_module(source);
    // Should report missing method
    let diags = check_module_all_diags(source);
    let has_missing = diags
        .diagnostics()
        .iter()
        .any(|d| d.message.contains("Missing method"));
    assert!(
        has_missing,
        "Should warn about missing trait method 'summary'"
    );
}

// List and Tuple type tracking

#[test]
fn test_list_type_tracking() {
    let source = r#"
module example

fn main() -> Unit {
  let nums = [1, 2, 3]
  let _x = nums
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "List type should be tracked");
}

#[test]
fn test_tuple_type_tracking() {
    let source = r#"
module example

fn main() -> Unit {
  let pair = (1, "hello")
  let _x = pair
}
"#;
    let result = check_module(source);
    assert!(result.is_ok(), "Tuple type should be tracked");
}

#[test]
fn test_suggestions_have_edits() {
    // W0001: unused variable suggestion should have an Edit
    let source = r#"
module example

fn main() -> Unit {
  let x = 42
}
"#;
    let diags = check_module_all_diags(source);
    let unused_diag = diags.diagnostics().iter().find(|d| d.code == "W0001");
    assert!(unused_diag.is_some(), "Should have W0001 for unused `x`");
    let unused = unused_diag.unwrap();
    assert!(
        !unused.suggestions.is_empty(),
        "W0001 should have a suggestion"
    );
    assert!(
        !unused.suggestions[0].edits.is_empty(),
        "W0001 suggestion should have an edit with replacement text"
    );
}

#[test]
fn test_unknown_identifier_suggestion_has_edit() {
    // E1002: unknown identifier with similar name suggestion
    let source = r#"
module example

fn calculate(value: Int) -> Int {
  valu + 1
}
"#;
    let diags = check_module_all_diags(source);
    let error_diag = diags.diagnostics().iter().find(|d| d.code == "E1002");
    assert!(error_diag.is_some(), "Should have E1002 for `valu`");
    let error = error_diag.unwrap();
    assert!(
        !error.suggestions.is_empty(),
        "E1002 should have a did-you-mean suggestion"
    );
    assert!(
        !error.suggestions[0].edits.is_empty(),
        "E1002 suggestion should have an edit for replacement"
    );
}

#[test]
fn test_trait_constraint_satisfied() {
    // Should pass: Int implements Show
    let source = r#"
module example

trait Show {
  fn to_text(self) -> Text
}

impl Show for Int {
  fn to_text(self) -> Text { "int" }
}

fn display[T: Show](value: T) -> Text {
  "ok"
}

fn main() -> Text {
  display(42)
}
"#;
    let diags = check_module_all_diags(source);
    let constraint_errors: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "E1016")
        .collect();
    assert!(
        constraint_errors.is_empty(),
        "Should have no E1016 errors when trait is implemented"
    );
}

#[test]
fn test_trait_constraint_not_satisfied() {
    // Should fail: Text does not implement Sortable
    let source = r#"
module example

trait Sortable {
  fn compare(self, other: Int) -> Int
}

impl Sortable for Int {
  fn compare(self, other: Int) -> Int { 0 }
}

fn sort_items[T: Sortable](items: List[T]) -> List[T] {
  items
}

fn main() -> List[Text] {
  sort_items(["hello", "world"])
}
"#;
    let diags = check_module_all_diags(source);
    let constraint_errors: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "E1016")
        .collect();
    assert!(
        !constraint_errors.is_empty(),
        "Should have E1016 error: Text does not implement Sortable"
    );
    assert!(constraint_errors[0]
        .message
        .contains("does not implement trait"));
}

// W0008: Unused function tests

#[test]
fn test_lint_unused_private_function_warns() {
    let source = r#"
module example

fn unused_helper() -> Int {
  42
}

fn main() -> Int {
  0
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0008")
        .collect();
    assert!(
        !warnings.is_empty(),
        "should warn about unused private function"
    );
    assert!(warnings[0].message.contains("unused_helper"));
}

#[test]
fn test_lint_used_function_no_warning() {
    let source = r#"
module example

fn helper() -> Int {
  42
}

fn main() -> Int {
  helper()
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0008")
        .collect();
    assert!(
        warnings.is_empty(),
        "should not warn about called function: {:?}",
        warnings
    );
}

#[test]
fn test_lint_public_function_no_warning() {
    let source = r#"
module example

public fn api_endpoint() -> Int {
  42
}

fn main() -> Int {
  0
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0008")
        .collect();
    assert!(
        warnings.is_empty(),
        "should not warn about public functions"
    );
}

#[test]
fn test_lint_underscore_function_no_warning() {
    let source = r#"
module example

fn _internal() -> Int {
  42
}

fn main() -> Int {
  0
}
"#;
    let diags = check_module_all_diags(source);
    let warnings: Vec<_> = diags
        .diagnostics()
        .iter()
        .filter(|d| d.code == "W0008")
        .collect();
    assert!(
        warnings.is_empty(),
        "should not warn about _-prefixed functions"
    );
}

// =========================================================================
// Json type and json_parse/json_stringify tests
// =========================================================================

#[test]
fn test_json_type_annotation() {
    let source = r#"
module example

fn process(data: Json) -> Json {
  data
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_json_parse_returns_json() {
    let source = r#"
module example

fn parse_data() -> Json {
  json_parse("{}")
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_json_stringify_returns_text() {
    let source = r#"
module example

fn to_string(data: Json) -> Text {
  json_stringify(data)
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_json_compatible_with_int() {
    let source = r#"
module example

fn use_json(data: Json) -> Int {
  data
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_json_compatible_with_text() {
    let source = r#"
module example

fn use_json(data: Json) -> Text {
  data
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_json_stringify_accepts_any_type() {
    let source = r#"
module example

fn stringify_int() -> Text {
  json_stringify(42)
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_json_parse_requires_text() {
    let source = r#"
module example

fn bad_parse() -> Json {
  json_parse(42)
}
"#;
    assert!(check_module(source).is_err());
}

#[test]
fn test_json_field_access() {
    let source = r#"
module example

fn get_field() -> Json {
  let obj = json_parse("{\"name\": \"Astra\"}")
  obj.name
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_json_index_access() {
    let source = r#"
module example

fn get_element() -> Json {
  let arr = json_parse("[1, 2, 3]")
  arr[0]
}
"#;
    assert!(check_module(source).is_ok());
}

// =========================================================================
// Regex builtin type signature tests
// =========================================================================

#[test]
fn test_regex_is_match_returns_bool() {
    let source = r#"
module example

fn check(pattern: Text, text: Text) -> Bool {
  regex_is_match(pattern, text)
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_regex_replace_returns_text() {
    let source = r#"
module example

fn clean(text: Text) -> Text {
  regex_replace("\\s+", text, " ")
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_regex_split_returns_list_text() {
    let source = r#"
module example

fn split_words(text: Text) -> List[Text] {
  regex_split("\\s+", text)
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_regex_find_all_returns_list() {
    let source = r#"
module example

fn find_all(text: Text) -> List[Json] {
  regex_find_all("[0-9]+", text)
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_regex_match_returns_option() {
    let source = r#"
module example

fn try_match(text: Text) -> Option[Json] {
  regex_match("[0-9]+", text)
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_regex_requires_text_args() {
    let source = r#"
module example

fn bad_regex() -> Bool {
  regex_is_match(42, true)
}
"#;
    assert!(check_module(source).is_err());
}

// =========================================================================
// Effect builtin type signature tests
// =========================================================================

#[test]
fn test_read_file_signature() {
    let source = r#"
module example

fn load(path: Text) -> Text effects(Fs) {
  read_file(path)
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_write_file_signature() {
    let source = r#"
module example

fn save(path: Text, data: Text) -> Unit effects(Fs) {
  write_file(path, data)
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_http_get_signature() {
    let source = r#"
module example

fn fetch(url: Text) -> Text effects(Net) {
  http_get(url)
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_http_post_signature() {
    let source = r#"
module example

fn send(url: Text, body: Text) -> Text effects(Net) {
  http_post(url, body)
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_random_int_signature() {
    let source = r#"
module example

fn roll_dice() -> Int effects(Rand) {
  random_int(1, 6)
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_random_bool_signature() {
    let source = r#"
module example

fn coin_flip() -> Bool effects(Rand) {
  random_bool()
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_current_time_millis_signature() {
    let source = r#"
module example

fn now() -> Int effects(Clock) {
  current_time_millis()
}
"#;
    assert!(check_module(source).is_ok());
}

#[test]
fn test_get_env_signature() {
    let source = r#"
module example

fn home_dir() -> Option[Text] effects(Env) {
  get_env("HOME")
}
"#;
    assert!(check_module(source).is_ok());
}

// =========================================================================
// Automated sync test: interpreter builtins must have typechecker entries
// =========================================================================

#[test]
fn test_interpreter_typechecker_builtin_sync() {
    // Extract builtin names from both source files by parsing the match arms.
    // This test ensures every function dispatched in the interpreter has a
    // corresponding entry in the type checker (ADR-005).
    let interp_src = include_str!("../interpreter/mod.rs");
    let tc_src = include_str!("mod.rs");

    let interp_builtins = extract_interpreter_builtins(interp_src);
    let tc_builtins = extract_typechecker_builtins(tc_src);

    let mut missing: Vec<&str> = Vec::new();
    for name in &interp_builtins {
        if !tc_builtins.contains(name) {
            missing.push(name);
        }
    }

    assert!(
        missing.is_empty(),
        "Interpreter builtins missing from type checker (ADR-005 violation):\n  {}\n\
         \nEvery builtin in the interpreter match dispatch must have a \
         corresponding entry in the type checker's Expr::Ident match.\n\
         See docs/adr/ADR-005-builtin-type-sync.md",
        missing.join(", ")
    );
}

/// Extract builtin function names from the interpreter's Call dispatch.
/// Looks for patterns like `"name" =>` inside the builtin match block
/// that starts after the `// Check for builtin functions first` comment.
fn extract_interpreter_builtins(src: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_call_dispatch = false;
    let mut found_marker = false;
    for line in src.lines() {
        let trimmed = line.trim();
        // Find the Call builtin dispatch (skip the Ident dispatch)
        if trimmed.contains("Check for builtin functions first") {
            found_marker = true;
            continue;
        }
        if found_marker && !in_call_dispatch && trimmed.contains("match name.as_str()") {
            in_call_dispatch = true;
            continue;
        }
        if !in_call_dispatch {
            continue;
        }
        // End of the match block
        if trimmed == "_ => {}" {
            break;
        }
        // Match lines like: "function_name" => {
        if let Some(start) = trimmed.find('"') {
            if let Some(end) = trimmed[start + 1..].find('"') {
                let name = &trimmed[start + 1..start + 1 + end];
                if trimmed[start + 1 + end + 1..]
                    .trim_start()
                    .starts_with("=>")
                {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}

/// Extract builtin identifiers from the type checker's Expr::Ident match.
/// Looks for quoted strings in the builtin recognition block.
fn extract_typechecker_builtins(src: &str) -> Vec<String> {
    let mut names = Vec::new();
    // Find the block starting with: // Built-in constructors and effects
    // and ending at the `_ =>` fallthrough
    let mut in_ident_match = false;
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.contains("Built-in constructors and effects") {
            in_ident_match = true;
            continue;
        }
        if !in_ident_match {
            continue;
        }
        // When we hit the _ => fallthrough, we're done
        if trimmed.starts_with("_ =>") || trimmed.starts_with("_ =") {
            break;
        }
        // Extract quoted names from lines like: "name" | "name2" => ...
        // or "name" => Type::Function { ... }
        let mut rest = trimmed;
        while let Some(start) = rest.find('"') {
            rest = &rest[start + 1..];
            if let Some(end) = rest.find('"') {
                let name = &rest[..end];
                // Filter out non-identifier strings and effect module names
                // Effect modules (Console, Fs, Net, Clock, Rand, Env) and
                // collection modules (Map, Set) are not callable builtins
                let effect_modules = ["Console", "Fs", "Net", "Clock", "Rand", "Env", "Map", "Set"];
                if !name.is_empty() && !name.contains(' ') && !effect_modules.contains(&name) {
                    names.push(name.to_string());
                }
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }
    }
    names
}

#[test]
fn test_to_int_text_returns_option_int() {
    // to_int(Text) returns Option[Int], comparing with Int must be a type error
    let result = check_module(
        r#"
module example
fn main() -> Bool {
  to_int("5") < 10
}
"#,
    );
    assert!(
        result.is_err(),
        "to_int(Text) < Int should be a type error (Option[Int] vs Int)"
    );
}

#[test]
fn test_to_float_text_returns_option_float() {
    // to_float(Text) returns Option[Float], comparing with Float must be a type error
    let result = check_module(
        r#"
module example
fn main() -> Bool {
  to_float("3.14") < 1.0
}
"#,
    );
    assert!(
        result.is_err(),
        "to_float(Text) < Float should be a type error (Option[Float] vs Float)"
    );
}

#[test]
fn test_to_int_from_int_returns_int() {
    // to_int(Int) returns Int, so comparing with Int is fine
    let result = check_module(
        r#"
module example
fn main() -> Bool {
  to_int(5) < 10
}
"#,
    );
    assert!(
        result.is_ok(),
        "to_int(Int) < Int should type-check: {:?}",
        result
    );
}

#[test]
fn test_to_int_from_float_returns_int() {
    // to_int(Float) returns Int
    let result = check_module(
        r#"
module example
fn main() -> Bool {
  to_int(3.14) < 10
}
"#,
    );
    assert!(
        result.is_ok(),
        "to_int(Float) < Int should type-check: {:?}",
        result
    );
}

#[test]
fn test_to_float_from_int_returns_float() {
    // to_float(Int) returns Float
    let result = check_module(
        r#"
module example
fn main() -> Bool {
  to_float(5) < 1.0
}
"#,
    );
    assert!(
        result.is_ok(),
        "to_float(Int) < Float should type-check: {:?}",
        result
    );
}

#[test]
fn test_comparison_type_mismatch_option_vs_int() {
    // Option[Int] cannot be compared with Int
    let result = check_module(
        r#"
module example
fn main() -> Bool {
  let x: Option[Int] = Some(5)
  x < 10
}
"#,
    );
    assert!(result.is_err(), "Option[Int] < Int should be a type error");
}
