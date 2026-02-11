//! Parser for the Astra programming language
//!
//! This module provides:
//! - Lexer (tokenization)
//! - Parser (AST construction)
//! - AST definitions
//! - Span tracking

pub mod ast;
pub mod lexer;
pub mod parser;
mod span;

pub use ast::*;
pub use lexer::Lexer;
pub use parser::Parser;
pub use span::SourceFile;

use crate::diagnostics::{Diagnostic, DiagnosticBag};
use std::path::Path;

/// Parse a source file into an AST
pub fn parse_file(path: &Path) -> Result<Module, DiagnosticBag> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        let mut bag = DiagnosticBag::new();
        bag.push(
            Diagnostic::error("E0100")
                .message(format!("Could not read file: {}", e))
                .span(crate::diagnostics::Span::file(path))
                .build(),
        );
        bag
    })?;

    parse_source(&content, path)
}

/// Parse source code into an AST
pub fn parse_source(source: &str, path: &Path) -> Result<Module, DiagnosticBag> {
    let source_file = SourceFile::new(path.to_path_buf(), source.to_string());
    let lexer = Lexer::new(&source_file);
    let mut parser = Parser::new(lexer, source_file.clone());
    parser.parse_module()
}

#[cfg(test)]
mod tests {
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
}
