//! Parser for the Astra programming language
//!
//! This module provides:
//! - Lexer (tokenization)
//! - Parser (AST construction)
//! - AST definitions
//! - Span tracking

pub mod ast;
pub mod lexer;
#[allow(clippy::module_inception)]
pub mod parser;
pub mod span;

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
mod tests;
