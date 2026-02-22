//! Source file and span utilities

use crate::diagnostics::Span;
use std::path::PathBuf;

/// A source file with its content and line information
#[derive(Debug, Clone)]
pub struct SourceFile {
    path: PathBuf,
    content: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    /// Create a new source file
    pub fn new(path: PathBuf, content: String) -> Self {
        let line_starts = std::iter::once(0)
            .chain(content.match_indices('\n').map(|(i, _)| i + 1))
            .collect();

        Self {
            path,
            content,
            line_starts,
        }
    }

    /// Get the file path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Get the file content
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Create a span for a byte range
    pub fn span(&self, start: usize, end: usize) -> Span {
        let (start_line, start_col) = self.line_col(start);
        let (end_line, end_col) = self.line_col(end);

        Span {
            file: self.path.clone(),
            start,
            end,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Convert a byte offset to line and column (1-indexed)
    fn line_col(&self, offset: usize) -> (usize, usize) {
        let line = self
            .line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts.get(line).copied().unwrap_or(0);
        let col = offset - line_start + 1;
        (line + 1, col)
    }

    /// Get a line by number (1-indexed)
    pub fn get_line(&self, line: usize) -> Option<&str> {
        if line == 0 || line > self.line_starts.len() {
            return None;
        }

        let start = self.line_starts[line - 1];
        let end = self
            .line_starts
            .get(line)
            .map(|&e| e.saturating_sub(1))
            .unwrap_or(self.content.len());

        Some(&self.content[start..end])
    }
}
#[cfg(test)]
#[path = "span_tests.rs"]
mod tests;
