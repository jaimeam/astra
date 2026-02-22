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
