use super::*;
use std::path::PathBuf;

#[test]
fn test_parse_empty_module() {
    let source = "module mymod\n";
    let result = parse_source(source, &PathBuf::from("test.astra"));
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_parse_simple_function() {
    let source = r#"module math

fn add(a: Int, b: Int) -> Int {
  a + b
}
"#;
    let result = parse_source(source, &PathBuf::from("test.astra"));
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_parse_test_with_using_effects() {
    let source = r#"module example

test "deterministic random" using effects(Rand = Rand.seeded(42)) {
  let x = Rand.int(1, 100)
  assert(x > 0)
}
"#;
    let result = parse_source(source, &PathBuf::from("test.astra"));
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
    let module = result.unwrap();
    if let Item::Test(test) = &module.items[0] {
        assert_eq!(test.name, "deterministic random");
        assert!(test.using.is_some());
        let using = test.using.as_ref().unwrap();
        assert_eq!(using.bindings.len(), 1);
        assert_eq!(using.bindings[0].effect, "Rand");
    } else {
        panic!("expected test block");
    }
}

#[test]
fn test_parse_test_with_multiple_effect_bindings() {
    let source = r#"module example

test "multi effects" using effects(Rand = Rand.seeded(42), Clock = Clock.fixed(1000)) {
  assert(true)
}
"#;
    let result = parse_source(source, &PathBuf::from("test.astra"));
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
    let module = result.unwrap();
    if let Item::Test(test) = &module.items[0] {
        let using = test.using.as_ref().unwrap();
        assert_eq!(using.bindings.len(), 2);
        assert_eq!(using.bindings[0].effect, "Rand");
        assert_eq!(using.bindings[1].effect, "Clock");
    } else {
        panic!("expected test block");
    }
}

#[test]
fn test_parse_requires_clause() {
    let source = r#"module example

fn divide(a: Int, b: Int) -> Int
  requires b != 0
{
  a / b
}
"#;
    let result = parse_source(source, &PathBuf::from("test.astra"));
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
    let module = result.unwrap();
    if let Item::FnDef(fndef) = &module.items[0] {
        assert_eq!(fndef.name, "divide");
        assert_eq!(fndef.requires.len(), 1);
        assert_eq!(fndef.ensures.len(), 0);
    } else {
        panic!("expected fn def");
    }
}

#[test]
fn test_parse_ensures_clause() {
    let source = r#"module example

fn abs(x: Int) -> Int
  ensures result >= 0
{
  if x < 0 { 0 - x } else { x }
}
"#;
    let result = parse_source(source, &PathBuf::from("test.astra"));
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
    let module = result.unwrap();
    if let Item::FnDef(fndef) = &module.items[0] {
        assert_eq!(fndef.name, "abs");
        assert_eq!(fndef.requires.len(), 0);
        assert_eq!(fndef.ensures.len(), 1);
    } else {
        panic!("expected fn def");
    }
}

#[test]
fn test_parse_requires_and_ensures() {
    let source = r#"module example

fn divide(a: Int, b: Int) -> Int
  requires b != 0
  ensures result >= 0
{
  a / b
}
"#;
    let result = parse_source(source, &PathBuf::from("test.astra"));
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
    let module = result.unwrap();
    if let Item::FnDef(fndef) = &module.items[0] {
        assert_eq!(fndef.name, "divide");
        assert_eq!(fndef.requires.len(), 1);
        assert_eq!(fndef.ensures.len(), 1);
    } else {
        panic!("expected fn def");
    }
}

#[test]
fn test_parse_multiple_requires() {
    let source = r#"module example

fn clamp(x: Int, lo: Int, hi: Int) -> Int
  requires lo <= hi
  requires x >= 0
{
  if x < lo { lo } else { if x > hi { hi } else { x } }
}
"#;
    let result = parse_source(source, &PathBuf::from("test.astra"));
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
    let module = result.unwrap();
    if let Item::FnDef(fndef) = &module.items[0] {
        assert_eq!(fndef.requires.len(), 2);
    } else {
        panic!("expected fn def");
    }
}

#[test]
fn test_parse_contracts_with_effects() {
    let source = r#"module example

fn safe_divide(a: Int, b: Int) -> Int effects(Console)
  requires b != 0
{
  Console.println("dividing")
  a / b
}
"#;
    let result = parse_source(source, &PathBuf::from("test.astra"));
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
    let module = result.unwrap();
    if let Item::FnDef(fndef) = &module.items[0] {
        assert_eq!(fndef.effects.len(), 1);
        assert_eq!(fndef.requires.len(), 1);
    } else {
        panic!("expected fn def");
    }
}

#[test]
fn test_parse_test_without_using() {
    let source = r#"module example

test "simple test" {
  assert(true)
}
"#;
    let result = parse_source(source, &PathBuf::from("test.astra"));
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
    let module = result.unwrap();
    if let Item::Test(test) = &module.items[0] {
        assert!(test.using.is_none());
    } else {
        panic!("expected test block");
    }
}
