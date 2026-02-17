//! Diagnostic reporting for the Astra compiler
//!
//! This module provides structured error reporting with stable error codes,
//! source spans, and machine-readable JSON output.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod error_codes;
pub use error_codes::*;

/// A source location span
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Source file path
    pub file: PathBuf,

    /// Start byte offset (0-indexed)
    pub start: usize,

    /// End byte offset (0-indexed, exclusive)
    pub end: usize,

    /// Start line (1-indexed)
    pub start_line: usize,

    /// Start column (1-indexed)
    pub start_col: usize,

    /// End line (1-indexed)
    pub end_line: usize,

    /// End column (1-indexed)
    pub end_col: usize,
}

impl Span {
    /// Create a new span
    pub fn new(
        file: PathBuf,
        start: usize,
        end: usize,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Self {
        Self {
            file,
            start,
            end,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Create a span for an entire file
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self {
            file: path.into(),
            start: 0,
            end: 0,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 1,
        }
    }

    /// Merge two spans into one that covers both
    pub fn merge(&self, other: &Span) -> Span {
        Span {
            file: self.file.clone(),
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            start_line: self.start_line.min(other.start_line),
            start_col: if self.start_line <= other.start_line {
                self.start_col
            } else {
                other.start_col
            },
            end_line: self.end_line.max(other.end_line),
            end_col: if self.end_line >= other.end_line {
                self.end_col
            } else {
                other.end_col
            },
        }
    }
}

/// Severity level for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

/// An additional note attached to a diagnostic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// Note message
    pub message: String,

    /// Optional span for the note
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

impl Note {
    /// Create a new note with a message
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }

    /// Attach a span to this note
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}

/// A suggested code fix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    /// Title describing the suggestion
    pub title: String,

    /// Edits to apply
    pub edits: Vec<Edit>,
}

impl Suggestion {
    /// Create a new suggestion
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            edits: Vec::new(),
        }
    }

    /// Add an edit to this suggestion
    pub fn with_edit(mut self, edit: Edit) -> Self {
        self.edits.push(edit);
        self
    }
}

/// A code edit (replacement)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edit {
    /// File to edit
    pub file: PathBuf,

    /// Span to replace
    pub span: Span,

    /// Replacement text
    pub replacement: String,
}

impl Edit {
    /// Create a new edit
    pub fn new(span: Span, replacement: impl Into<String>) -> Self {
        Self {
            file: span.file.clone(),
            span,
            replacement: replacement.into(),
        }
    }
}

/// A compiler diagnostic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable error code (e.g., "E1001")
    pub code: String,

    /// Severity level
    pub severity: Severity,

    /// Primary message
    pub message: String,

    /// Primary source span
    pub span: Span,

    /// Additional notes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,

    /// Suggested fixes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    /// Create a new error diagnostic
    pub fn error(code: impl Into<String>) -> DiagnosticBuilder {
        DiagnosticBuilder {
            code: code.into(),
            severity: Severity::Error,
            message: String::new(),
            span: None,
            notes: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    /// Create a new warning diagnostic
    pub fn warning(code: impl Into<String>) -> DiagnosticBuilder {
        DiagnosticBuilder {
            code: code.into(),
            severity: Severity::Warning,
            message: String::new(),
            span: None,
            notes: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    /// Create a new info diagnostic
    pub fn info(code: impl Into<String>) -> DiagnosticBuilder {
        DiagnosticBuilder {
            code: code.into(),
            severity: Severity::Info,
            message: String::new(),
            span: None,
            notes: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    /// Check if this is an error
    pub fn is_error(&self) -> bool {
        matches!(self.severity, Severity::Error)
    }

    /// Format as JSON
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Format as human-readable string
    pub fn to_human_readable(&self, source: &str) -> String {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
            Severity::Hint => "hint",
        };

        let mut output = format!(
            "{}[{}]: {}\n  --> {}:{}:{}\n",
            severity,
            self.code,
            self.message,
            self.span.file.display(),
            self.span.start_line,
            self.span.start_col
        );

        // Show source context
        let lines: Vec<&str> = source.lines().collect();
        if self.span.start_line > 0 && self.span.start_line <= lines.len() {
            let line = lines[self.span.start_line - 1];
            output.push_str(&format!(
                "   |\n{:>3} | {}\n   |",
                self.span.start_line, line
            ));

            // Underline the error
            let underline_start = self.span.start_col.saturating_sub(1);
            let underline_len = if self.span.end_line == self.span.start_line {
                self.span.end_col.saturating_sub(self.span.start_col).max(1)
            } else {
                line.len().saturating_sub(underline_start)
            };

            output.push_str(&format!(
                " {}{}\n",
                " ".repeat(underline_start),
                "^".repeat(underline_len)
            ));
        }

        // Add notes
        for note in &self.notes {
            output.push_str(&format!("   = note: {}\n", note.message));
        }

        // Add suggestions
        for suggestion in &self.suggestions {
            output.push_str(&format!("   = help: {}\n", suggestion.title));
        }

        output
    }
}

/// Builder for constructing diagnostics
pub struct DiagnosticBuilder {
    code: String,
    severity: Severity,
    message: String,
    span: Option<Span>,
    notes: Vec<Note>,
    suggestions: Vec<Suggestion>,
}

impl DiagnosticBuilder {
    /// Set the message
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Set the primary span
    pub fn span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Add a note
    pub fn note(mut self, note: Note) -> Self {
        self.notes.push(note);
        self
    }

    /// Add a suggestion
    pub fn suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Build the diagnostic
    pub fn build(self) -> Diagnostic {
        Diagnostic {
            code: self.code,
            severity: self.severity,
            message: self.message,
            span: self.span.unwrap_or_else(|| Span::file("")),
            notes: self.notes,
            suggestions: self.suggestions,
        }
    }
}

/// A collection of diagnostics
#[derive(Debug, Default, Clone)]
pub struct DiagnosticBag {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBag {
    /// Create a new empty bag
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a diagnostic
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_error())
    }

    /// Check if there are any warnings
    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| matches!(d.severity, Severity::Warning))
    }

    /// Count warnings
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| matches!(d.severity, Severity::Warning))
            .count()
    }

    /// Count errors
    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_error()).count()
    }

    /// Get all diagnostics
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Take all diagnostics
    pub fn take(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    /// Merge another bag into this one
    pub fn merge(&mut self, other: DiagnosticBag) {
        self.diagnostics.extend(other.diagnostics);
    }

    /// Get the number of diagnostics
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Check if the bag is empty
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Format all diagnostics as JSON
    pub fn to_json(&self) -> String {
        let json_array: Vec<String> = self.diagnostics.iter().map(|d| d.to_json()).collect();
        format!("[{}]", json_array.join(","))
    }

    /// Format all diagnostics as human-readable text
    pub fn format_text(&self, source: &str) -> String {
        self.diagnostics
            .iter()
            .map(|d| d.to_human_readable(source))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl From<Diagnostic> for DiagnosticBag {
    fn from(diagnostic: Diagnostic) -> Self {
        let mut bag = DiagnosticBag::new();
        bag.push(diagnostic);
        bag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_json() {
        let diag = Diagnostic::error("E1001")
            .message("Type mismatch")
            .span(Span::new(PathBuf::from("test.astra"), 10, 20, 1, 10, 1, 20))
            .build();

        let json = diag.to_json();
        assert!(json.contains("E1001"));
        assert!(json.contains("Type mismatch"));
    }

    #[test]
    fn test_span_merge() {
        let span1 = Span::new(PathBuf::from("test.astra"), 10, 20, 1, 10, 1, 20);
        let span2 = Span::new(PathBuf::from("test.astra"), 15, 30, 1, 15, 2, 5);

        let merged = span1.merge(&span2);
        assert_eq!(merged.start, 10);
        assert_eq!(merged.end, 30);
    }

    #[test]
    fn test_diagnostic_warning() {
        let diag = Diagnostic::warning("W0001")
            .message("Unused variable")
            .build();
        assert!(!diag.is_error());
        assert_eq!(diag.severity, Severity::Warning);
    }

    #[test]
    fn test_diagnostic_with_suggestion() {
        let diag = Diagnostic::error("E1001")
            .message("Type mismatch")
            .span(Span::new(PathBuf::from("test.astra"), 0, 5, 1, 1, 1, 5))
            .suggestion(
                Suggestion::new("Replace with correct type").with_edit(Edit::new(
                    Span::new(PathBuf::from("test.astra"), 0, 5, 1, 1, 1, 5),
                    "Int",
                )),
            )
            .build();

        let json = diag.to_json();
        assert!(json.contains("Replace with correct type"));
        assert!(json.contains("Int"));
    }

    #[test]
    fn test_diagnostic_human_readable() {
        let diag = Diagnostic::error("E1001")
            .message("Type mismatch: expected Int, got Text")
            .span(Span::new(PathBuf::from("test.astra"), 0, 3, 1, 1, 1, 3))
            .build();

        let source = "foo";
        let output = diag.to_human_readable(source);
        assert!(output.contains("error[E1001]"));
        assert!(output.contains("Type mismatch"));
    }

    #[test]
    fn test_diagnostic_bag_operations() {
        let mut bag = DiagnosticBag::new();
        assert!(bag.is_empty());
        assert_eq!(bag.len(), 0);

        bag.push(Diagnostic::error("E0001").message("error").build());
        bag.push(Diagnostic::warning("W0001").message("warning").build());

        assert!(!bag.is_empty());
        assert_eq!(bag.len(), 2);
        assert!(bag.has_errors());
        assert!(bag.has_warnings());
        assert_eq!(bag.error_count(), 1);
        assert_eq!(bag.warning_count(), 1);
    }

    #[test]
    fn test_diagnostic_bag_merge() {
        let mut bag1 = DiagnosticBag::new();
        bag1.push(Diagnostic::error("E0001").message("err1").build());

        let mut bag2 = DiagnosticBag::new();
        bag2.push(Diagnostic::warning("W0001").message("warn1").build());

        bag1.merge(bag2);
        assert_eq!(bag1.len(), 2);
        assert_eq!(bag1.error_count(), 1);
        assert_eq!(bag1.warning_count(), 1);
    }

    #[test]
    fn test_diagnostic_bag_json() {
        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::error("E0001").message("test error").build());

        let json = bag.to_json();
        assert!(json.contains("E0001"));
        assert!(json.contains("test error"));
    }

    #[test]
    fn test_diagnostic_bag_format_text() {
        let mut bag = DiagnosticBag::new();
        bag.push(
            Diagnostic::error("E0001")
                .message("syntax error")
                .span(Span::new(PathBuf::from("test.astra"), 0, 3, 1, 1, 1, 3))
                .build(),
        );

        let text = bag.format_text("foo");
        assert!(text.contains("syntax error"));
    }

    #[test]
    fn test_diagnostic_info_severity() {
        let diag = Diagnostic::info("I0001").message("Info message").build();
        assert_eq!(diag.severity, Severity::Info);
        assert!(!diag.is_error());
    }

    #[test]
    fn test_span_file_constructor() {
        let span = Span::file(PathBuf::from("test.astra"));
        assert_eq!(span.file, PathBuf::from("test.astra"));
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 0);
    }

    #[test]
    fn test_diagnostic_note() {
        let diag = Diagnostic::error("E1001")
            .message("Error")
            .note(Note::new("Additional context"))
            .build();

        let json = diag.to_json();
        assert!(json.contains("Additional context"));
    }
}
