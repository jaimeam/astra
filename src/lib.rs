//! Astra Programming Language
//!
//! Astra is an LLM/Agent-native programming language designed for verifiability
//! and deterministic feedback.

pub mod cli;
pub mod diagnostics;
pub mod effects;
pub mod formatter;
pub mod interpreter;
pub mod manifest;
pub mod parser;
pub mod testing;
pub mod typechecker;

/// Re-export commonly used types
pub mod prelude {
    pub use crate::diagnostics::{Diagnostic, Severity, Span};
    pub use crate::parser::ast::*;
}
