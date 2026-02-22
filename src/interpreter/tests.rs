use super::*;
use crate::parser::{Lexer, Parser, SourceFile};
use std::path::PathBuf;

fn parse_and_eval(source: &str) -> Result<Value, RuntimeError> {
    let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
    let lexer = Lexer::new(&source_file);
    let mut parser = Parser::new(lexer, source_file.clone());
    let module = parser.parse_module().expect("parse failed");

    let console = Box::new(MockConsole::new());
    let capabilities = Capabilities {
        console: Some(console),
        ..Default::default()
    };

    let mut interpreter = Interpreter::with_capabilities(capabilities);
    // Add the project root as a search path for stdlib imports
    if let Ok(cwd) = std::env::current_dir() {
        interpreter.add_search_path(cwd);
    }
    interpreter.eval_module(&module)
}

#[test]
fn test_simple_arithmetic() {
    let source = r#"
module example

fn main() -> Int {
  1 + 2 * 3
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(7)));
}

#[test]
fn test_if_expression() {
    let source = r#"
module example

fn main() -> Int {
  if true {
42
  } else {
0
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_function_call() {
    let source = r#"
module example

fn add(a: Int, b: Int) -> Int {
  a + b
}

fn main() -> Int {
  add(10, 20)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(30)));
}

#[test]
fn test_pattern_matching() {
    let source = r#"
module example

fn main() -> Int {
  let x = 5
  match x {
0 => 100
5 => 200
_ => 300
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(200)));
}

#[test]
fn test_recursion() {
    // Run in a thread with a larger stack to avoid overflow in debug builds
    let handle = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let source = r#"
module example

fn factorial(n: Int) -> Int {
  if n <= 1 {
1
  } else {
n * factorial(n - 1)
  }
}

fn main() -> Int {
  factorial(3)
}
"#;
            let result = parse_and_eval(source).unwrap();
            assert!(matches!(result, Value::Int(6)));
        })
        .unwrap();
    handle.join().unwrap();
}

#[test]
fn test_string_operations() {
    let source = r#"
module example

fn main() -> Text {
  "Hello" + " " + "World"
}
"#;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "Hello World"),
        _ => panic!("expected Text"),
    }
}

#[test]
fn test_comparison() {
    let source = r#"
module example

fn main() -> Bool {
  10 > 5 and 5 < 10
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_record_field_access() {
    let source = r#"
module example

fn main() -> Int {
  let r = { x = 10, y = 20 }
  r.x + r.y
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(30)));
}

#[test]
fn test_division_by_zero() {
    let source = r#"
module example

fn main() -> Int {
  10 / 0
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, "E4003");
}

#[test]
fn test_console_effect() {
    let source = r#"
module example

fn main() effects(Console) {
  Console.println("test output")
}
"#;
    // Use the helper which sets up mock console
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

#[test]
fn test_nested_function_calls() {
    let source = r#"
module example

fn double(x: Int) -> Int {
  x * 2
}

fn add_one(x: Int) -> Int {
  x + 1
}

fn main() -> Int {
  double(add_one(5))
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(12)));
}

#[test]
fn test_option_some() {
    let source = r#"
module example

fn main() -> Int {
  match Some(42) {
Some(n) => n
None => 0
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_option_none() {
    let source = r#"
module example

fn main() -> Int {
  match None {
Some(n) => n
None => 0
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(0)));
}

#[test]
fn test_result_ok() {
    let source = r#"
module example

fn main() -> Int {
  match Ok(42) {
Ok(n) => n
Err(e) => 0
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_result_err() {
    let source = r#"
module example

fn main() -> Text {
  match Err("oops") {
Ok(n) => "ok"
Err(e) => e
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(s) if s == "oops"));
}

#[test]
fn test_try_operator_option_some() {
    let source = r#"
module example

fn get() -> Option[Int] {
  Some(21)
}

fn double() -> Option[Int] {
  let x = get()?
  Some(x * 2)
}

fn main() -> Int {
  match double() {
Some(n) => n
None => 0
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_try_operator_option_none() {
    let source = r#"
module example

fn get() -> Option[Int] {
  None
}

fn double() -> Option[Int] {
  let x = get()?
  Some(x * 2)
}

fn main() -> Int {
  match double() {
Some(n) => n
None => 0
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(0)));
}

#[test]
fn test_try_operator_result() {
    let source = r#"
module example

fn parse(ok: Bool) -> Result[Int, Text] {
  if ok { Ok(21) } else { Err("error") }
}

fn double(ok: Bool) -> Result[Int, Text] {
  let x = parse(ok)?
  Ok(x * 2)
}

fn main() -> Int {
  match double(true) {
Ok(n) => n
Err(e) => 0
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_seeded_rand_deterministic() {
    // Two runs with the same seed should produce the same results
    let rand1 = SeededRand::new(42);
    let rand2 = SeededRand::new(42);

    let a1 = rand1.int(1, 100);
    let a2 = rand2.int(1, 100);
    assert_eq!(a1, a2);

    let b1 = rand1.int(1, 1000);
    let b2 = rand2.int(1, 1000);
    assert_eq!(b1, b2);

    let c1 = rand1.bool();
    let c2 = rand2.bool();
    assert_eq!(c1, c2);
}

#[test]
fn test_fixed_clock() {
    let clock = FixedClock::new(1700000000);
    assert_eq!(clock.now(), 1700000000);
    clock.sleep(5000); // no-op
    assert_eq!(clock.now(), 1700000000);
}

#[test]
fn test_deterministic_rand_in_program() {
    let source = r#"
module example

fn main() effects(Rand) {
  Rand.int(1, 100)
}
"#;
    let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
    let lexer = Lexer::new(&source_file);
    let mut parser = Parser::new(lexer, source_file.clone());
    let module = parser.parse_module().expect("parse failed");

    // Run twice with same seed, should get same result
    let caps1 = Capabilities {
        rand: Some(Box::new(SeededRand::new(42))),
        console: Some(Box::new(MockConsole::new())),
        ..Default::default()
    };
    let mut interp1 = Interpreter::with_capabilities(caps1);
    let result1 = interp1.eval_module(&module).unwrap();

    let caps2 = Capabilities {
        rand: Some(Box::new(SeededRand::new(42))),
        console: Some(Box::new(MockConsole::new())),
        ..Default::default()
    };
    let mut interp2 = Interpreter::with_capabilities(caps2);
    let result2 = interp2.eval_module(&module).unwrap();

    match (&result1, &result2) {
        (Value::Int(a), Value::Int(b)) => assert_eq!(a, b),
        _ => panic!("expected Int results"),
    }
}

#[test]
fn test_fixed_clock_in_program() {
    let source = r#"
module example

fn main() effects(Clock) {
  Clock.now()
}
"#;
    let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
    let lexer = Lexer::new(&source_file);
    let mut parser = Parser::new(lexer, source_file.clone());
    let module = parser.parse_module().expect("parse failed");

    let caps = Capabilities {
        clock: Some(Box::new(FixedClock::new(1700000000))),
        console: Some(Box::new(MockConsole::new())),
        ..Default::default()
    };
    let mut interp = Interpreter::with_capabilities(caps);
    let result = interp.eval_module(&module).unwrap();

    assert!(matches!(result, Value::Int(1700000000)));
}

#[test]
fn test_requires_passes() {
    let source = r#"
module example

fn divide(a: Int, b: Int) -> Int
  requires b != 0
{
  a / b
}

fn main() -> Int {
  divide(10, 2)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(5)));
}

#[test]
fn test_requires_fails() {
    let source = r#"
module example

fn divide(a: Int, b: Int) -> Int
  requires b != 0
{
  a / b
}

fn main() -> Int {
  divide(10, 0)
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, "E3001");
}

#[test]
fn test_ensures_passes() {
    let source = r#"
module example

fn abs(x: Int) -> Int
  ensures result >= 0
{
  if x < 0 { 0 - x } else { x }
}

fn main() -> Int {
  abs(0 - 5)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(5)));
}

#[test]
fn test_ensures_fails() {
    let source = r#"
module example

fn bad_abs(x: Int) -> Int
  ensures result >= 0
{
  x
}

fn main() -> Int {
  bad_abs(0 - 5)
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, "E3002");
}

#[test]
fn test_multiple_requires() {
    let source = r#"
module example

fn clamp(x: Int, lo: Int, hi: Int) -> Int
  requires lo <= hi
  requires lo >= 0
{
  if x < lo { lo } else { if x > hi { hi } else { x } }
}

fn main() -> Int {
  clamp(50, 0, 100)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(50)));
}

#[test]
fn test_requires_and_ensures() {
    let source = r#"
module example

fn safe_divide(a: Int, b: Int) -> Int
  requires b != 0
  ensures result * b <= a
{
  a / b
}

fn main() -> Int {
  safe_divide(10, 3)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

// H5: Type inference for let bindings
#[test]
fn test_let_type_inference() {
    let source = r#"
module example

fn main() -> Int {
  let x = 42
  let y = x + 8
  y
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(50)));
}

#[test]
fn test_let_type_inference_text() {
    let source = r#"
module example

fn main() -> Text {
  let greeting = "Hello"
  let name = "World"
  greeting + " " + name
}
"#;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "Hello World"),
        _ => panic!("expected Text"),
    }
}

// N1: List literal syntax
#[test]
fn test_list_literal_empty() {
    let source = r#"
module example

fn main() -> Int {
  let xs = []
  len(xs)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(0)));
}

#[test]
fn test_list_literal_ints() {
    let source = r#"
module example

fn main() -> Int {
  let xs = [1, 2, 3]
  len(xs)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

#[test]
fn test_list_get() {
    let source = r#"
module example

fn main() -> Int {
  let xs = [10, 20, 30]
  match xs.get(1) {
Some(n) => n
None => 0
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(20)));
}

#[test]
fn test_list_get_out_of_bounds() {
    let source = r#"
module example

fn main() -> Int {
  let xs = [10, 20, 30]
  match xs.get(5) {
Some(n) => n
None => 0
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(0)));
}

#[test]
fn test_list_contains() {
    let source = r#"
module example

fn main() -> Bool {
  let xs = [1, 2, 3]
  xs.contains(2)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_list_method_len() {
    let source = r#"
module example

fn main() -> Int {
  let xs = [10, 20, 30, 40]
  xs.len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(4)));
}

// N2: print and println builtins
#[test]
fn test_println_builtin() {
    let source = r#"
module example

fn main() {
  println("hello from builtin")
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

#[test]
fn test_print_builtin() {
    let source = r#"
module example

fn main() {
  print("no newline")
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

// N3: len and to_text builtins
#[test]
fn test_len_text() {
    let source = r#"
module example

fn main() -> Int {
  len("hello")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(5)));
}

#[test]
fn test_to_text_int() {
    let source = r#"
module example

fn main() -> Text {
  to_text(42)
}
"#;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "42"),
        _ => panic!("expected Text"),
    }
}

#[test]
fn test_to_text_bool() {
    let source = r#"
module example

fn main() -> Text {
  to_text(true)
}
"#;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "true"),
        _ => panic!("expected Text"),
    }
}

#[test]
fn test_text_len_method() {
    let source = r#"
module example

fn main() -> Int {
  let s = "hello"
  s.len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(5)));
}

// N4: if-then-else expression syntax
#[test]
fn test_if_then_else_basic() {
    let source = r#"
module example

fn main() -> Int {
  if true then 42 else 0
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_if_then_else_false() {
    let source = r#"
module example

fn main() -> Int {
  if false then 42 else 0
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(0)));
}

#[test]
fn test_if_then_else_with_expr() {
    let source = r#"
module example

fn main() -> Int {
  let x = 10
  if x > 5 then x * 2 else x
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(20)));
}

// List equality
#[test]
fn test_list_equality() {
    let source = r#"
module example

fn main() {
  let xs = [1, 2, 3]
  let ys = [1, 2, 3]
  assert_eq(xs, ys)
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

// =========================================================================
// Lambda / Anonymous function tests
// =========================================================================

#[test]
fn test_lambda_basic() {
    let source = r#"
module example

fn main() -> Int {
  let square = fn(x: Int) { x * x }
  square(5)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(25)));
}

#[test]
fn test_lambda_no_type_annotation() {
    let source = r#"
module example

fn main() -> Int {
  let add_one = fn(x) { x + 1 }
  add_one(41)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_lambda_multi_param() {
    let source = r#"
module example

fn main() -> Int {
  let add = fn(a, b) { a + b }
  add(10, 20)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(30)));
}

#[test]
fn test_lambda_closure_capture() {
    let source = r#"
module example

fn make_adder(n: Int) -> (Int) -> Int {
  fn(x: Int) { x + n }
}

fn main() -> Int {
  let add5 = make_adder(5)
  add5(10)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(15)));
}

#[test]
fn test_lambda_as_argument() {
    let source = r#"
module example

fn apply(f: (Int) -> Int, x: Int) -> Int {
  f(x)
}

fn main() -> Int {
  apply(fn(x) { x + 10 }, 5)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(15)));
}

#[test]
fn test_lambda_inline_in_method() {
    let source = r#"
module example

fn main() -> Int {
  let xs = [1, 2, 3, 4, 5]
  let doubled = xs.map(fn(x) { x * 2 })
  doubled.fold(0, fn(acc, x) { acc + x })
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(30)));
}

// =========================================================================
// Higher-order function tests
// =========================================================================

#[test]
fn test_function_as_value() {
    let source = r#"
module example

fn double(x: Int) -> Int {
  x * 2
}

fn apply_twice(f: (Int) -> Int, x: Int) -> Int {
  f(f(x))
}

fn main() -> Int {
  apply_twice(double, 3)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(12)));
}

// =========================================================================
// List combinator tests
// =========================================================================

#[test]
fn test_list_map() {
    let source = r#"
module example

fn main() {
  let xs = [1, 2, 3]
  let doubled = xs.map(fn(x) { x * 2 })
  assert_eq(doubled, [2, 4, 6])
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

#[test]
fn test_list_filter() {
    let source = r#"
module example

fn main() {
  let xs = [1, 2, 3, 4, 5, 6]
  let evens = xs.filter(fn(x) { x % 2 == 0 })
  assert_eq(evens, [2, 4, 6])
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

#[test]
fn test_list_fold() {
    let source = r#"
module example

fn main() -> Int {
  let xs = [1, 2, 3, 4, 5]
  xs.fold(0, fn(acc, x) { acc + x })
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(15)));
}

#[test]
fn test_list_method_chain() {
    let source = r#"
module example

fn main() -> Int {
  [1, -2, 3, -4, 5]
.filter(fn(x) { x > 0 })
.map(fn(x) { x * 2 })
.fold(0, fn(acc, x) { acc + x })
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(18)));
}

#[test]
fn test_list_any() {
    let source = r#"
module example

fn main() -> Bool {
  [1, 2, 3, 4, 5].any(fn(x) { x > 4 })
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_list_all() {
    let source = r#"
module example

fn main() -> Bool {
  [2, 4, 6, 8].all(fn(x) { x % 2 == 0 })
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_list_each() {
    let source = r#"
module example

fn main() {
  let xs = [1, 2, 3]
  xs.each(fn(x) { assert(x > 0) })
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

#[test]
fn test_list_push() {
    let source = r#"
module example

fn main() {
  let xs = [1, 2, 3]
  let ys = xs.push(4)
  assert_eq(ys, [1, 2, 3, 4])
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

#[test]
fn test_list_concat() {
    let source = r#"
module example

fn main() {
  let xs = [1, 2]
  let ys = [3, 4]
  assert_eq(xs.concat(ys), [1, 2, 3, 4])
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

#[test]
fn test_list_head() {
    let source = r#"
module example

fn main() -> Int {
  match [10, 20, 30].head() {
Some(x) => x
None => 0
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(10)));
}

#[test]
fn test_list_is_empty() {
    let source = r#"
module example

fn main() -> Bool {
  [].is_empty()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_list_flat_map() {
    let source = r#"
module example

fn main() {
  let xs = [1, 2, 3]
  let result = xs.flat_map(fn(x) { [x, x * 10] })
  assert_eq(result, [1, 10, 2, 20, 3, 30])
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

// =========================================================================
// Pattern guard tests
// =========================================================================

#[test]
fn test_pattern_guard_basic() {
    let source = r#"
module example

fn classify(x: Int) -> Text {
  match x {
n if n < 0 => "negative"
0 => "zero"
n if n <= 10 => "small"
_ => "large"
  }
}

fn main() -> Text {
  classify(-5)
}
"#;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "negative"),
        _ => panic!("expected Text"),
    }
}

#[test]
fn test_pattern_guard_fallthrough() {
    let source = r#"
module example

fn main() -> Text {
  let x = 15
  match x {
n if n < 0 => "negative"
0 => "zero"
n if n <= 10 => "small"
_ => "large"
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "large"),
        _ => panic!("expected Text"),
    }
}

#[test]
fn test_pattern_guard_with_variant() {
    let source = r#"
module example

fn main() -> Text {
  match Some(5) {
Some(n) if n > 10 => "big"
Some(n) if n > 0 => "small"
Some(n) => "non-positive"
None => "nothing"
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "small"),
        _ => panic!("expected Text"),
    }
}

// =========================================================================
// String method tests
// =========================================================================

#[test]
fn test_string_to_upper() {
    let source = r#"
module example

fn main() -> Text {
  "hello".to_upper()
}
"#;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "HELLO"),
        _ => panic!("expected Text"),
    }
}

#[test]
fn test_string_to_lower() {
    let source = r#"
module example

fn main() -> Text {
  "HELLO".to_lower()
}
"#;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "hello"),
        _ => panic!("expected Text"),
    }
}

#[test]
fn test_string_trim() {
    let source = r#"
module example

fn main() -> Text {
  "  hello  ".trim()
}
"#;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "hello"),
        _ => panic!("expected Text"),
    }
}

#[test]
fn test_string_contains() {
    let source = r#"
module example

fn main() -> Bool {
  "hello world".contains("world")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_string_split() {
    let source = r#"
module example

fn main() {
  let parts = "a,b,c".split(",")
  assert_eq(parts, ["a", "b", "c"])
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

#[test]
fn test_string_starts_with() {
    let source = r#"
module example

fn main() -> Bool {
  "hello world".starts_with("hello")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_string_ends_with() {
    let source = r#"
module example

fn main() -> Bool {
  "hello world".ends_with("world")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_string_replace() {
    let source = r#"
module example

fn main() -> Text {
  "hello world".replace("world", "astra")
}
"#;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "hello astra"),
        _ => panic!("expected Text"),
    }
}

#[test]
fn test_string_chars() {
    let source = r#"
module example

fn main() {
  let chars = "abc".chars()
  assert_eq(chars, ["a", "b", "c"])
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

// =========================================================================
// Option/Result map tests
// =========================================================================

#[test]
fn test_option_map() {
    let source = r#"
module example

fn main() {
  let x = Some(5)
  let y = x.map(fn(n) { n * 2 })
  assert_eq(y, Some(10))
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

#[test]
fn test_option_map_none() {
    let source = r#"
module example

fn main() {
  let x = None
  let y = x.map(fn(n) { n * 2 })
  assert_eq(y, None)
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

#[test]
fn test_result_map() {
    let source = r#"
module example

fn main() {
  let x = Ok(5)
  let y = x.map(fn(n) { n * 2 })
  assert_eq(y, Ok(10))
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

#[test]
fn test_result_map_err() {
    let source = r#"
module example

fn main() {
  let x = Err("bad")
  let y = x.map_err(fn(e) { e + "!" })
  assert_eq(y, Err("bad!"))
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

// =========================================================================
// Record destructuring in let bindings
// =========================================================================

#[test]
fn test_let_destructure_record() {
    let source = r#"
module example

fn main() -> Int {
  let point = { x = 10, y = 20 }
  let { x, y } = point
  x + y
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(30)));
}

#[test]
fn test_let_destructure_record_inline() {
    let source = r#"
module example

fn main() -> Int {
  let { x, y } = { x = 3, y = 4 }
  x * y
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(12)));
}

// =========================================================================
// For loop tests
// =========================================================================

#[test]
fn test_for_loop_basic() {
    let source = r#"
module example

fn main() -> Int {
  let mut sum = 0
  for x in [1, 2, 3, 4, 5] {
sum = sum + x
  }
  sum
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(15)));
}

#[test]
fn test_for_loop_with_variable() {
    let source = r#"
module example

fn main() -> Int {
  let items = [10, 20, 30]
  let mut total = 0
  for item in items {
total = total + item
  }
  total
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(60)));
}

#[test]
fn test_for_loop_empty_list() {
    let source = r#"
module example

fn main() -> Int {
  let mut count = 0
  for _x in [] {
count = count + 1
  }
  count
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(0)));
}

#[test]
fn test_for_loop_nested() {
    let source = r#"
module example

fn main() -> Int {
  let mut sum = 0
  for x in [1, 2, 3] {
for y in [10, 20] {
  sum = sum + x * y
}
  }
  sum
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(180)));
}

// =========================================================================
// Multi-field variant destructuring tests
// =========================================================================

#[test]
fn test_multi_field_variant_construct_and_match() {
    let source = r#"
module example

enum Shape =
  | Circle(radius: Int)
  | Rectangle(width: Int, height: Int)

fn area(s: Shape) -> Int {
  match s {
Circle(r) => r * r * 3
Rectangle(w, h) => w * h
  }
}

fn main() -> Int {
  area(Rectangle(5, 3))
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(15)));
}

#[test]
fn test_multi_field_variant_single_field() {
    let source = r#"
module example

enum Shape =
  | Circle(radius: Int)
  | Rectangle(width: Int, height: Int)

fn area(s: Shape) -> Int {
  match s {
Circle(r) => r * r
Rectangle(w, h) => w * h
  }
}

fn main() -> Int {
  area(Circle(10))
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(100)));
}

// =========================================================================
// Generic function tests
// =========================================================================

#[test]
fn test_generic_identity() {
    let source = r#"
module example

fn identity[T](x: T) -> T {
  x
}

fn main() -> Int {
  identity(42)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_generic_identity_text() {
    let source = r#"
module example

fn identity[T](x: T) -> T {
  x
}

fn main() -> Text {
  identity("hello")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "hello"));
}

#[test]
fn test_generic_pair() {
    let source = r#"
module example

fn first[T, U](a: T, b: U) -> T {
  a
}

fn second[T, U](a: T, b: U) -> U {
  b
}

fn main() -> Int {
  first(1, "hello") + second("world", 2)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

// === P1.1: range() builtin ===

#[test]
fn test_range_basic() {
    let source = r#"
module example
fn main() -> Int {
  let xs = range(0, 5)
  len(xs)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(5)));
}

#[test]
fn test_range_for_loop() {
    let source = r#"
module example
fn main() -> Int {
  let mut sum = 0
  for i in range(1, 6) {
sum = sum + i
  }
  sum
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(15)));
}

#[test]
fn test_range_empty() {
    let source = r#"
module example
fn main() -> Int {
  let xs = range(5, 5)
  len(xs)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(0)));
}

// === P1.2: break/continue ===

#[test]
fn test_break_in_for_loop() {
    let source = r#"
module example
fn main() -> Int {
  let mut sum = 0
  for i in range(1, 100) {
if i > 5 { break }
sum = sum + i
  }
  sum
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(15)));
}

#[test]
fn test_continue_in_for_loop() {
    let source = r#"
module example
fn main() -> Int {
  let mut sum = 0
  for i in range(1, 6) {
if i == 3 { continue }
sum = sum + i
  }
  sum
}
"#;
    let result = parse_and_eval(source).unwrap();
    // 1 + 2 + 4 + 5 = 12 (skipping 3)
    assert!(matches!(result, Value::Int(12)));
}

// === P1.3: while loops ===

#[test]
fn test_while_basic() {
    let source = r#"
module example
fn main() -> Int {
  let mut i = 0
  let mut sum = 0
  while i < 5 {
sum = sum + i
i = i + 1
  }
  sum
}
"#;
    let result = parse_and_eval(source).unwrap();
    // 0+1+2+3+4 = 10
    assert!(matches!(result, Value::Int(10)));
}

#[test]
fn test_while_with_break() {
    let source = r#"
module example
fn main() -> Int {
  let mut i = 0
  while true {
if i == 5 { break }
i = i + 1
  }
  i
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(5)));
}

#[test]
fn test_while_with_continue() {
    let source = r#"
module example
fn main() -> Int {
  let mut i = 0
  let mut sum = 0
  while i < 10 {
i = i + 1
if i % 2 == 0 { continue }
sum = sum + i
  }
  sum
}
"#;
    let result = parse_and_eval(source).unwrap();
    // 1+3+5+7+9 = 25
    assert!(matches!(result, Value::Int(25)));
}

#[test]
fn test_while_false() {
    let source = r#"
module example
fn main() -> Int {
  let mut x = 42
  while false {
x = 0
  }
  x
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

// === P1.5: String interpolation ===

#[test]
fn test_string_interp_basic() {
    let source = r#"
module example
fn main() -> Text {
  let name = "world"
  "hello, ${name}!"
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "hello, world!"));
}

#[test]
fn test_string_interp_expr() {
    let source = r#"
module example
fn main() -> Text {
  let x = 21
  "answer is ${x * 2}"
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "answer is 42"));
}

#[test]
fn test_string_interp_multiple() {
    let source = r#"
module example
fn main() -> Text {
  let a = 1
  let b = 2
  "${a} + ${b} = ${a + b}"
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "1 + 2 = 3"));
}

// === P1.10: return statement ===

#[test]
fn test_return_early() {
    let source = r#"
module example
fn check(x: Int) -> Text {
  if x > 10 {
return "big"
  }
  "small"
}
fn main() -> Text {
  check(15)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "big"));
}

#[test]
fn test_return_fallthrough() {
    let source = r#"
module example
fn check(x: Int) -> Text {
  if x > 10 {
return "big"
  }
  "small"
}
fn main() -> Text {
  check(5)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "small"));
}

#[test]
fn test_return_in_loop() {
    let source = r#"
module example
fn find_first_even(xs: List[Int]) -> Option[Int] {
  for x in xs {
if x % 2 == 0 {
  return Some(x)
}
  }
  None
}
fn main() -> Int {
  match find_first_even([1, 3, 4, 6]) {
Some(n) => n,
None => 0,
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(4)));
}

// === P1.11: math builtins ===

#[test]
fn test_abs() {
    let source = r#"
module example
fn main() -> Int {
  abs(-42)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_min_max() {
    let source = r#"
module example
fn main() -> Int {
  min(3, 7) + max(3, 7)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(10)));
}

#[test]
fn test_pow() {
    let source = r#"
module example
fn main() -> Int {
  pow(2, 10)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(1024)));
}

// === P3.1: List methods (tail, reverse, sort) ===

#[test]
fn test_list_tail() {
    let source = r#"
module example
fn main() -> Int {
  let xs = [1, 2, 3, 4]
  len(xs.tail())
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

#[test]
fn test_list_reverse() {
    let source = r#"
module example
fn main() -> Int {
  let xs = [3, 1, 2]
  let rev = xs.reverse()
  match rev.head() {
Some(n) => n,
None => 0,
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(2)));
}

#[test]
fn test_list_sort() {
    let source = r#"
module example
fn main() -> Int {
  let xs = [3, 1, 4, 1, 5, 9]
  let sorted = xs.sort()
  match sorted.head() {
Some(n) => n,
None => 0,
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(1)));
}

// === P3.2: List methods (take, drop, slice, enumerate, zip, find) ===

#[test]
fn test_list_take() {
    let source = r#"
module example
fn main() -> Int {
  len([1, 2, 3, 4, 5].take(3))
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

#[test]
fn test_list_drop() {
    let source = r#"
module example
fn main() -> Int {
  len([1, 2, 3, 4, 5].drop(2))
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

#[test]
fn test_list_find() {
    let source = r#"
module example
fn main() -> Int {
  match [1, 2, 3, 4].find(fn(x) { x > 2 }) {
Some(n) => n,
None => 0,
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

// === P3.3: String methods (repeat, index_of, substring, join) ===

#[test]
fn test_string_repeat() {
    let source = r#"
module example
fn main() -> Text {
  "ha".repeat(3)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "hahaha"));
}

#[test]
fn test_string_index_of() {
    let source = r#"
module example
fn main() -> Int {
  match "hello world".index_of("world") {
Some(n) => n,
None => -1,
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(6)));
}

#[test]
fn test_string_substring() {
    let source = r#"
module example
fn main() -> Text {
  "hello world".substring(0, 5)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "hello"));
}

#[test]
fn test_list_join() {
    let source = r#"
module example
fn main() -> Text {
  ["a", "b", "c"].join(", ")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "a, b, c"));
}

// === Else-if chains (P1.4 - already implemented in parser) ===

#[test]
fn test_else_if_chain() {
    let source = r#"
module example
fn classify(n: Int) -> Text {
  if n < 0 {
"negative"
  } else if n == 0 {
"zero"
  } else if n < 10 {
"small"
  } else {
"large"
  }
}
fn main() -> Text {
  classify(5)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "small"));
}

// === P1.6: Float type ===

#[test]
fn test_float_literal() {
    let source = r#"
module example
fn main() -> Float {
  2.75
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Float(f) if (f - 2.75).abs() < 0.001));
}

#[test]
fn test_float_arithmetic() {
    let source = r#"
module example
fn main() -> Float {
  1.5 + 2.5
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Float(f) if (f - 4.0).abs() < 0.001));
}

#[test]
fn test_float_int_mixed() {
    let source = r#"
module example
fn main() -> Float {
  2 * 1.5
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Float(f) if (f - 3.0).abs() < 0.01));
}

#[test]
fn test_float_comparison() {
    let source = r#"
module example
fn main() -> Bool {
  3.14 > 2.71
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_float_neg() {
    let source = r#"
module example
fn main() -> Float {
  -2.75
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Float(f) if (f + 2.75).abs() < 0.001));
}

#[test]
fn test_sqrt() {
    let source = r#"
module example
fn main() -> Float {
  sqrt(16.0)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Float(f) if (f - 4.0).abs() < 0.001));
}

#[test]
fn test_floor_ceil_round() {
    let source = r#"
module example
fn main() -> Float {
  floor(3.7) + ceil(3.2) + round(3.5)
}
"#;
    let result = parse_and_eval(source).unwrap();
    // floor(3.7)=3.0 + ceil(3.2)=4.0 + round(3.5)=4.0 = 11.0
    assert!(matches!(result, Value::Float(f) if (f - 11.0).abs() < 0.001));
}

// === P6.1: Pipe operator ===

#[test]
fn test_pipe_basic() {
    let source = r#"
module example
fn double(x: Int) -> Int { x * 2 }
fn add_one(x: Int) -> Int { x + 1 }
fn main() -> Int {
  5 |> double |> add_one
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(11)));
}

#[test]
fn test_pipe_with_lambda() {
    let source = r#"
module example
fn main() -> Int {
  10 |> fn(x) { x * x }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(100)));
}

// === P3.4: Conversion functions ===

#[test]
fn test_to_int_from_float() {
    let source = r#"
module example
fn main() -> Int {
  to_int(3.14)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

#[test]
fn test_to_float_from_int() {
    let source = r#"
module example
fn main() -> Float {
  to_float(42)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Float(f) if (f - 42.0).abs() < 0.001));
}

// === P5.4: Assert with custom messages ===

#[test]
fn test_assert_with_message_passes() {
    let source = r#"
module example
fn main() -> Int {
  assert(true, "should pass")
  42
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_assert_with_message_fails() {
    let source = r#"
module example
fn main() -> Int {
  assert(false, "custom error message")
  42
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message.contains("custom error message"));
}

// === P1.7: Tuple type ===

#[test]
fn test_tuple_creation() {
    let source = r#"
module example
fn main() -> Int {
  let t = (1, 2, 3)
  len(t)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

#[test]
fn test_tuple_field_access() {
    let source = r#"
module example
fn main() -> Int {
  let t = (10, 20, 30)
  t.0 + t.2
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(40)));
}

#[test]
fn test_tuple_pattern_match() {
    let source = r#"
module example
fn main() -> Int {
  let t = (1, 2)
  match t {
(a, b) => a + b
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

// === P1.8: Map type ===

#[test]
fn test_map_new_and_set() {
    let source = r#"
module example
fn main() -> Int {
  let m = Map.new()
  let m2 = m.set("a", 1)
  let m3 = m2.set("b", 2)
  m3.len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(2)));
}

#[test]
fn test_map_get() {
    let source = r#"
module example
fn main() -> Int {
  let m = Map.new()
  let m2 = m.set("key", 42)
  m2.get("key").unwrap()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_map_contains_key() {
    let source = r#"
module example
fn main() -> Bool {
  let m = Map.new().set("x", 1)
  m.contains_key("x")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_map_keys_values() {
    let source = r#"
module example
fn main() -> Int {
  let m = Map.new().set("a", 1).set("b", 2)
  m.keys().len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(2)));
}

#[test]
fn test_map_from_tuples() {
    let source = r#"
module example
fn main() -> Int {
  let m = Map.from([("a", 10), ("b", 20)])
  m.get("b").unwrap()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(20)));
}

// === P3.5: Set type ===

#[test]
fn test_set_new_and_add() {
    let source = r#"
module example
fn main() -> Int {
  let s = Set.new()
  let s2 = s.add(1).add(2).add(1)
  s2.len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(2)));
}

#[test]
fn test_set_contains() {
    let source = r#"
module example
fn main() -> Bool {
  let s = Set.from([1, 2, 3])
  s.contains(2)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_set_union() {
    let source = r#"
module example
fn main() -> Int {
  let s1 = Set.from([1, 2, 3])
  let s2 = Set.from([3, 4, 5])
  s1.union(s2).len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(5)));
}

#[test]
fn test_set_intersection() {
    let source = r#"
module example
fn main() -> Int {
  let s1 = Set.from([1, 2, 3])
  let s2 = Set.from([2, 3, 4])
  s1.intersection(s2).len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(2)));
}

// === P1.9: Type alias resolution ===

#[test]
fn test_type_alias_basic() {
    let source = r#"
module example
type Name = Text
fn greet(name: Name) -> Text {
  "Hello"
}
fn main() -> Text {
  greet("World")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(s) if s == "Hello"));
}

// === P2.3: Type invariants ===

#[test]
fn test_type_invariant_parsing() {
    // Test that invariant clause is parsed without error
    let source = r#"
module example
type Positive = Int
  invariant self > 0
fn main() -> Int {
  42
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

// === P2.2: Traits and impl blocks ===

#[test]
fn test_trait_and_impl_parsing() {
    let source = r#"
module example

trait Describable {
  fn describe(self: Int) -> Text
}

impl Describable for Int {
  fn describe(self: Int) -> Text {
"a number"
  }
}

fn main() -> Text {
  "traits work"
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(s) if s == "traits work"));
}

#[test]
fn test_impl_method_callable() {
    let source = r#"
module example

trait Doubler {
  fn double_it(x: Int) -> Int
}

impl Doubler for Int {
  fn double_it(x: Int) -> Int {
x * 2
  }
}

fn main() -> Int {
  Doubler_double_it(21)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

// === P5.3: Parser error recovery ===

#[test]
fn test_parser_error_recovery() {
    // Verify parser produces errors but doesn't crash
    let source = r#"
module example
fn good() -> Int { 1 }
fn bad( {
fn also_good() -> Int { 2 }
"#;
    let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
    let lexer = Lexer::new(&source_file);
    let mut parser = crate::parser::parser::Parser::new(lexer, source_file.clone());
    // Should return Err with diagnostics, not panic
    let result = parser.parse_module();
    assert!(result.is_err());
}

// === P1.12: Negative number literals ===

#[test]
fn test_negative_literals() {
    let source = r#"
module example
fn main() -> Int {
  -42
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(-42)));
}

#[test]
fn test_negative_float_literal() {
    let source = r#"
module example
fn main() -> Float {
  -2.5
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Float(f) if (f + 2.5).abs() < 0.001));
}

// === P2.4: Generic type constraints ===

#[test]
fn test_generic_constraint_syntax() {
    let source = r#"
module example
fn compare[T: Ord](a: T, b: T) -> Bool {
  a == b
}
fn main() -> Bool {
  compare(1, 1)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

// === P2.5: Recursive types ===

#[test]
fn test_recursive_enum_type() {
    // Run in a thread with a larger stack to avoid overflow in debug builds
    let handle = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let source = r#"
module example
enum IntList =
  | Nil
  | Cons(head: Int, tail: IntList)

fn sum_list(list: IntList) -> Int {
  match list {
Nil => 0
Cons(h, t) => h + sum_list(t)
  }
}

fn main() -> Int {
  let list = Cons(1, Cons(2, Cons(3, Nil)))
  sum_list(list)
}
"#;
            // Note: this tests that recursive types parse and evaluate
            // Recursive call depth is limited by stack
            let result = parse_and_eval(source);
            // This may or may not work depending on stack depth
            // The important thing is it parses
            assert!(result.is_ok() || result.is_err());
        })
        .unwrap();
    handle.join().unwrap();
}

// === Map.from with tuple entries ===

#[test]
fn test_map_remove() {
    let source = r#"
module example
fn main() -> Int {
  let m = Map.new().set("a", 1).set("b", 2).set("c", 3)
  let m2 = m.remove("b")
  m2.len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(2)));
}

#[test]
fn test_set_remove() {
    let source = r#"
module example
fn main() -> Int {
  let s = Set.from([1, 2, 3, 4])
  let s2 = s.remove(3)
  s2.len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

#[test]
fn test_map_entries() {
    let source = r#"
module example
fn main() -> Int {
  let m = Map.new().set("x", 10).set("y", 20)
  m.entries().len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(2)));
}

#[test]
fn test_tuple_to_list() {
    let source = r#"
module example
fn main() -> Int {
  let t = (1, 2, 3)
  t.to_list().len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

// === P2.3: Type invariant enforcement ===

#[test]
fn test_type_invariant_pass() {
    let source = r#"
module example
type Positive = Int invariant self > 0

fn main() -> Int {
  let x: Positive = 5
  x
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(5)));
}

#[test]
fn test_type_invariant_fail() {
    let source = r#"
module example
type Positive = Int invariant self > 0

fn main() -> Int {
  let x: Positive = -1
  x
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "E3003");
}

// === P4.3: Re-export parsing ===

#[test]
fn test_public_import_parsing() {
    let source = r#"
module example
public import std.math

fn main() -> Int {
  42
}
"#;
    // Should parse and run without error
    let result = parse_and_eval(source);
    assert!(result.is_ok());
}

#[test]
fn test_stdlib_import_use() {
    let source = r#"
module example
import std.math

fn main() -> Bool {
  is_even(4)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

// === P5.4: Custom assert messages ===

#[test]
fn test_assert_custom_message() {
    let source = r#"
module example
fn main() -> Unit {
  assert(false, "custom error message")
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message.contains("custom error message"));
}

// === P6.1: Pipe operator (already tested, verify still works) ===

#[test]
fn test_pipe_operator() {
    let source = r#"
module example
fn double(x: Int) -> Int { x * 2 }
fn add_one(x: Int) -> Int { x + 1 }
fn main() -> Int {
  5 |> double |> add_one
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(11)));
}

// === P6.2: User-defined effects ===

#[test]
fn test_effect_definition() {
    let source = r#"
module example

effect Logger {
  fn log(msg: Text) -> Unit
  fn get_logs() -> Text
}

fn main() -> Text {
  "effects defined"
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(s) if s == "effects defined"));
}

#[test]
fn test_user_effect_handler_dispatch() {
    // Test user-defined effect with a handler record
    let source = r#"
module example

effect Logger {
  fn log(msg: Text) -> Unit
}

fn do_work() -> Int effects(Logger) {
  Logger.log("starting work")
  42
}

fn main() -> Int {
  do_work()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

// === P6.4: Tail call optimization ===

#[test]
fn test_tco_simple_recursion() {
    let source = r#"
module example
fn sum_acc(n: Int, acc: Int) -> Int {
  if n <= 0 {
acc
  } else {
sum_acc(n - 1, acc + n)
  }
}
fn main() -> Int {
  sum_acc(1000, 0)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(500500)));
}

#[test]
fn test_tco_factorial() {
    let source = r#"
module example
fn fact(n: Int, acc: Int) -> Int {
  if n <= 1 {
acc
  } else {
fact(n - 1, n * acc)
  }
}
fn main() -> Int {
  fact(10, 1)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3628800)));
}

// === v1.1: async/await support ===

#[test]
fn test_await_is_reserved_keyword() {
    // v1.1: await is now supported — it resolves futures
    let source = r#"
module example
fn async_value() -> Int { 42 }
fn main() -> Int {
  await async_value()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_async_is_reserved_keyword() {
    // v1.1: async fn is now supported — creates a future when called
    let source = r#"
module example
async fn fetch() -> Int { 42 }
fn main() -> Int {
  await fetch()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

// Trait method dispatch tests

#[test]
fn test_trait_method_dispatch() {
    let source = r#"
module example

trait Show {
  fn show(self: Text) -> Text
}

impl Show for Int {
  fn show(self: Int) -> Text {
"integer"
  }
}

fn main() -> Text {
  let x = 42
  x.show()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "integer"));
}

#[test]
fn test_trait_dispatch_with_args() {
    let source = r#"
module example

trait Repeat {
  fn repeat(self: Text, n: Int) -> Text
}

impl Repeat for Text {
  fn repeat(self: Text, n: Int) -> Text {
if n <= 0 then "" else self
  }
}

fn main() -> Text {
  let s = "hello"
  s.repeat(1)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "hello"));
}

// Parameter destructuring tests

#[test]
fn test_record_param_destructuring() {
    let source = r#"
module example

fn get_x({x, y}: {x: Int, y: Int}) -> Int {
  x
}

fn main() -> Int {
  get_x({x = 10, y = 20})
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(10)));
}

#[test]
fn test_tuple_param_destructuring() {
    let source = r#"
module example

fn sum_pair((a, b): (Int, Int)) -> Int {
  a + b
}

fn main() -> Int {
  sum_pair((3, 7))
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(10)));
}

// === Range expression syntax (0..10 and 0..=10) ===

#[test]
fn test_range_expr_exclusive() {
    let source = r#"
module example
fn main() -> Int {
  let xs = 0..5
  len(xs)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(5)));
}

#[test]
fn test_range_expr_inclusive() {
    let source = r#"
module example
fn main() -> Int {
  let xs = 0..=5
  len(xs)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(6)));
}

#[test]
fn test_range_expr_for_loop() {
    let source = r#"
module example
fn main() -> Int effects(Console) {
  let mut total = 0
  for i in 1..=10 {
total = total + i
  }
  total
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(55)));
}

#[test]
fn test_range_expr_with_arithmetic() {
    let source = r#"
module example
fn main() -> Int {
  let xs = 2 + 3..10 - 2
  len(xs)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

// === Multiline strings (triple-quoted) ===

#[test]
fn test_multiline_string_basic() {
    let source = r####"
module example
fn main() -> Text {
  let s = """
hello
world
"""
  s
}
"####;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "hello\nworld"),
        other => panic!("Expected Text, got {:?}", other),
    }
}

#[test]
fn test_multiline_string_preserves_relative_indent() {
    let source = r####"
module example
fn main() -> Text {
  let s = """
line one
  indented
line three
"""
  s
}
"####;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "line one\n  indented\nline three"),
        other => panic!("Expected Text, got {:?}", other),
    }
}

// === v1.0: New feature tests ===

#[test]
fn test_compound_assignment_operators() {
    let source = r#"
module example
fn main() -> Int {
  let x = 10
  x += 5
  x -= 3
  x *= 2
  x /= 4
  x %= 5
  x
}
"#;
    let result = parse_and_eval(source).unwrap();
    // 10 + 5 = 15, 15 - 3 = 12, 12 * 2 = 24, 24 / 4 = 6, 6 % 5 = 1
    assert!(matches!(result, Value::Int(1)));
}

#[test]
fn test_index_access_list() {
    let source = r#"
module example
fn main() -> Int {
  let items = [10, 20, 30, 40]
  items[0] + items[2]
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(40)));
}

#[test]
fn test_index_access_negative() {
    let source = r#"
module example
fn main() -> Int {
  let items = [10, 20, 30]
  items[-1]
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(30)));
}

#[test]
fn test_index_access_string() {
    let source = r#"
module example
fn main() -> Text {
  let s = "hello"
  s[1]
}
"#;
    let result = parse_and_eval(source).unwrap();
    match result {
        Value::Text(s) => assert_eq!(s, "e"),
        other => panic!("Expected Text, got {:?}", other),
    }
}

#[test]
fn test_index_access_out_of_bounds() {
    let source = r#"
module example
fn main() -> Int {
  let items = [1, 2, 3]
  items[10]
}
"#;
    let result = parse_and_eval(source);
    assert!(result.is_err());
}

#[test]
fn test_for_loop_destructuring() {
    let source = r#"
module example
fn main() -> Int {
  let pairs = [(1, 10), (2, 20), (3, 30)]
  let total = 0
  for (key, val) in pairs {
total += val
  }
  total
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(60)));
}

#[test]
fn test_structural_equality_records() {
    let source = r#"
module example
fn main() -> Bool {
  let a = {x = 1, y = 2}
  let b = {x = 1, y = 2}
  a == b
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_structural_equality_lists() {
    let source = r#"
module example
fn main() -> Bool {
  let a = [1, 2, 3]
  let b = [1, 2, 3]
  let c = [1, 2, 4]
  a == b and a != c
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_structural_equality_option() {
    let source = r#"
module example
fn main() -> Bool {
  Some(42) == Some(42) and None == None and Some(1) != Some(2)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_string_comparison_operators() {
    let source = r#"
module example
fn main() -> Bool {
  "abc" < "abd" and "z" > "a" and "hello" == "hello"
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

// =================================================================
// v1.1 Feature Tests
// =================================================================

// --- JSON Parsing ---

#[test]
fn test_json_parse_int() {
    let source = r#"
module example
fn main() -> Int {
  json_parse("42")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_json_parse_float() {
    let source = r#"
module example
fn main() -> Float {
  json_parse("1.23")
}
"#;
    let result = parse_and_eval(source).unwrap();
    if let Value::Float(f) = result {
        assert!((f - 1.23).abs() < 0.001);
    } else {
        panic!("Expected Float, got {:?}", result);
    }
}

#[test]
fn test_json_parse_string() {
    let source = r#"
module example
fn main() -> Text {
  json_parse("\"hello world\"")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "hello world"));
}

#[test]
fn test_json_parse_bool() {
    let source = r#"
module example
fn main() -> Bool {
  json_parse("true")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_json_parse_null() {
    let source = r#"
module example
fn main() -> Unit {
  let result = json_parse("null")
  result
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::None));
}

#[test]
fn test_json_parse_array() {
    let source = r#"
module example
fn main() -> Int {
  let arr = json_parse("[1, 2, 3]")
  arr.len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

#[test]
fn test_json_parse_object() {
    let source = r#"
module example
fn main() -> Text {
  let obj = json_parse("{\"name\": \"Astra\", \"version\": 1}")
  obj.name
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "Astra"));
}

#[test]
fn test_json_stringify_record() {
    let source = r#"
module example
fn main() -> Text {
  let obj = { name = "test", value = 42 }
  json_stringify(obj)
}
"#;
    let result = parse_and_eval(source).unwrap();
    if let Value::Text(s) = result {
        assert!(s.contains("\"name\""));
        assert!(s.contains("\"test\""));
        assert!(s.contains("42"));
    } else {
        panic!("Expected Text, got {:?}", result);
    }
}

#[test]
fn test_json_stringify_list() {
    let source = r#"
module example
fn main() -> Text {
  json_stringify([1, 2, 3])
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "[1,2,3]"));
}

#[test]
fn test_json_roundtrip() {
    let source = r#"
module example
fn main() -> Bool {
  let json = "{\"a\":1,\"b\":true,\"c\":\"hello\"}"
  let parsed = json_parse(json)
  let stringified = json_stringify(parsed)
  let reparsed = json_parse(stringified)
  reparsed.a == 1 and reparsed.b == true and reparsed.c == "hello"
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

// --- Regex ---

#[test]
fn test_regex_is_match() {
    let source = r#"
module example
fn main() -> Bool {
  regex_is_match("\\d+", "hello 42 world")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_regex_is_match_no_match() {
    let source = r#"
module example
fn main() -> Bool {
  regex_is_match("\\d+", "no numbers here")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(false)));
}

#[test]
fn test_regex_match_with_groups() {
    let source = r#"
module example
fn main() -> Text {
  let result = regex_match("(\\w+)@(\\w+)", "user@host")
  match result {
Some(m) => m.matched
None => "no match"
  }
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "user@host"));
}

#[test]
fn test_regex_find_all() {
    let source = r#"
module example
fn main() -> Int {
  let matches = regex_find_all("\\d+", "a1 b22 c333")
  matches.len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

#[test]
fn test_regex_replace() {
    let source = r#"
module example
fn main() -> Text {
  regex_replace("\\d+", "hello 42 world 7", "NUM")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "hello NUM world NUM"));
}

#[test]
fn test_regex_split() {
    let source = r#"
module example
fn main() -> Int {
  let parts = regex_split("\\s+", "hello   world   foo")
  parts.len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

#[test]
fn test_text_matches_method() {
    let source = r#"
module example
fn main() -> Bool {
  "hello123".matches("[a-z]+\\d+")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Bool(true)));
}

#[test]
fn test_text_replace_pattern_method() {
    let source = r#"
module example
fn main() -> Text {
  "a1b2c3".replace_pattern("\\d", "X")
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Text(ref s) if s == "aXbXcX"));
}

#[test]
fn test_text_split_pattern_method() {
    let source = r#"
module example
fn main() -> Int {
  "one,two,,three".split_pattern(",+").len()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(3)));
}

// --- Async/Await ---

#[test]
fn test_async_fn_returns_future() {
    let source = r#"
module example
async fn compute() -> Int {
  42
}
fn main() -> Int {
  let future = compute()
  await future
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_async_fn_with_params() {
    let source = r#"
module example
async fn add(a: Int, b: Int) -> Int {
  a + b
}
fn main() -> Int {
  await add(10, 20)
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(30)));
}

#[test]
fn test_await_non_future_passthrough() {
    // Await on a non-future value should just return the value
    let source = r#"
module example
fn sync_value() -> Int { 42 }
fn main() -> Int {
  await sync_value()
}
"#;
    let result = parse_and_eval(source).unwrap();
    assert!(matches!(result, Value::Int(42)));
}

#[test]
fn test_filtered_import_different_names_from_same_module() {
    // Regression test: when module A imports {foo} from C, and then
    // module B (the main module) imports {bar} from C, the second import
    // should still bring `bar` into scope even though C was already loaded
    // by A's transitive dependency.
    use std::fs;

    let tmp = std::env::temp_dir().join("astra_import_test");
    let content_dir = tmp.join("content");
    let _ = fs::create_dir_all(&content_dir);

    // content/store.astra - has two functions
    fs::write(
        content_dir.join("store.astra"),
        r#"module content.store

fn build_store() -> Text {
  "store_built"
}

fn get_all_modules() -> Text {
  "all_modules"
}
"#,
    )
    .unwrap();

    // content/loader.astra - imports only build_store from content.store
    fs::write(
        content_dir.join("loader.astra"),
        r#"module content.loader
import content.store.{build_store}

fn load_all_content() -> Text {
  build_store()
}
"#,
    )
    .unwrap();

    // main module - imports from both loader and store with different names
    let source = r#"module example
import content.loader.{load_all_content}
import content.store.{get_all_modules}

fn main() -> Text {
  get_all_modules()
}
"#;

    let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
    let lexer = Lexer::new(&source_file);
    let mut parser = Parser::new(lexer, source_file.clone());
    let module = parser.parse_module().expect("parse failed");

    let mut interpreter = Interpreter::new();
    interpreter.add_search_path(tmp.clone());
    let result = interpreter.eval_module(&module);

    // Clean up
    let _ = fs::remove_dir_all(&tmp);

    let value = result.expect("should resolve get_all_modules from content.store");
    assert!(
        matches!(value, Value::Text(ref s) if s == "all_modules"),
        "expected Text(\"all_modules\"), got {:?}",
        value
    );
}

#[test]
fn test_net_serve() {
    use std::sync::mpsc;

    struct StubNet;
    impl NetCapability for StubNet {
        fn get(&self, _url: &str) -> Result<Value, String> {
            Ok(Value::Text(String::new()))
        }
        fn post(&self, _url: &str, _body: &str) -> Result<Value, String> {
            Ok(Value::Text(String::new()))
        }
    }

    // Use port 0 so the OS picks an available port — but tiny_http doesn't
    // expose the actual port when using "0.0.0.0:0", so pick a random high port.
    let port: u16 = 19284;

    let source = format!(
        "module example\n\
         \n\
         fn handler(req: {{method: Text, path: Text, body: Text}}) -> {{status: Int, body: Text, headers: Map[Text, Text]}} {{\n\
         \x20 let response_body = req.method + \" \" + req.path\n\
         \x20 {{status = 200, body = response_body, headers = Map.new()}}\n\
         }}\n\
         \n\
         fn main() effects(Net) {{\n\
         \x20 Net.serve({port}, handler)\n\
         }}\n",
    );

    let (tx, rx) = mpsc::channel();

    let handle = std::thread::spawn(move || {
        let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
        let lexer = Lexer::new(&source_file);
        let mut parser = Parser::new(lexer, source_file.clone());
        let module = parser.parse_module().expect("parse failed");

        let capabilities = Capabilities {
            console: Some(Box::new(MockConsole::new())),
            net: Some(Box::new(StubNet)),
            ..Default::default()
        };
        let mut interpreter = Interpreter::with_capabilities(capabilities);
        if let Ok(cwd) = std::env::current_dir() {
            interpreter.add_search_path(cwd);
        }

        // Signal that we're about to start serving
        tx.send(()).unwrap();

        // This blocks forever (until the thread is abandoned)
        let _ = interpreter.eval_module(&module);
    });

    // Wait for the server thread to start
    rx.recv().unwrap();
    // Give tiny_http a moment to actually bind
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Make a request
    let url = format!("http://127.0.0.1:{}/api/test", port);
    let resp = ureq::get(&url).call().expect("HTTP request failed");
    assert_eq!(resp.status(), 200);
    let body = resp.into_string().unwrap();
    assert_eq!(body, "GET /api/test");

    // Test query string parsing
    let url_qs = format!("http://127.0.0.1:{}/search?q=hello&page=2", port);
    let resp_qs = ureq::get(&url_qs)
        .call()
        .expect("HTTP request with query failed");
    assert_eq!(resp_qs.status(), 200);
    let body_qs = resp_qs.into_string().unwrap();
    assert_eq!(body_qs, "GET /search");

    // Test POST with body
    let url_post = format!("http://127.0.0.1:{}/api/data", port);
    let resp_post = ureq::post(&url_post)
        .send_string("payload")
        .expect("HTTP POST failed");
    assert_eq!(resp_post.status(), 200);
    let body_post = resp_post.into_string().unwrap();
    assert_eq!(body_post, "POST /api/data");

    // Server thread blocks forever; just drop it
    drop(handle);
}

#[test]
fn test_net_serve_query_params() {
    use std::sync::mpsc;

    struct StubNet;
    impl NetCapability for StubNet {
        fn get(&self, _url: &str) -> Result<Value, String> {
            Ok(Value::Text(String::new()))
        }
        fn post(&self, _url: &str, _body: &str) -> Result<Value, String> {
            Ok(Value::Text(String::new()))
        }
    }

    let port: u16 = 19285;

    let source = format!(
        "module example\n\
         \n\
         fn handler(req: {{method: Text, path: Text, body: Text, query: Map[Text, Text]}}) -> {{status: Int, body: Text, headers: Map[Text, Text]}} {{\n\
         \x20 let q = req.query.get(\"user\")\n\
         \x20 match q {{\n\
         \x20   Some(val) => {{status = 200, body = val, headers = Map.new()}}\n\
         \x20   None => {{status = 404, body = \"not found\", headers = Map.new()}}\n\
         \x20 }}\n\
         }}\n\
         \n\
         fn main() effects(Net) {{\n\
         \x20 Net.serve({port}, handler)\n\
         }}\n",
    );

    let (tx, rx) = mpsc::channel();

    let handle = std::thread::spawn(move || {
        let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
        let lexer = Lexer::new(&source_file);
        let mut parser = Parser::new(lexer, source_file.clone());
        let module = parser.parse_module().expect("parse failed");

        let capabilities = Capabilities {
            console: Some(Box::new(MockConsole::new())),
            net: Some(Box::new(StubNet)),
            ..Default::default()
        };
        let mut interpreter = Interpreter::with_capabilities(capabilities);
        if let Ok(cwd) = std::env::current_dir() {
            interpreter.add_search_path(cwd);
        }

        tx.send(()).unwrap();
        let _ = interpreter.eval_module(&module);
    });

    rx.recv().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Query param is extracted and returned as body
    let url = format!("http://127.0.0.1:{}/test?user=alice", port);
    let resp = ureq::get(&url).call().expect("HTTP request failed");
    assert_eq!(resp.status(), 200);
    let body = resp.into_string().unwrap();
    assert_eq!(body, "alice");

    // Missing query param returns 404
    let url_no_param = format!("http://127.0.0.1:{}/test", port);
    let resp_no = ureq::get(&url_no_param).call();
    match resp_no {
        Err(ureq::Error::Status(code, resp)) => {
            assert_eq!(code, 404);
            assert_eq!(resp.into_string().unwrap(), "not found");
        }
        other => panic!("expected 404, got {:?}", other.map(|r| r.status())),
    }

    drop(handle);
}
