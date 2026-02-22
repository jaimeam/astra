use super::*;

#[test]
fn test_line_col() {
    let source = SourceFile::new(
        PathBuf::from("test.astra"),
        "line1\nline2\nline3".to_string(),
    );

    assert_eq!(source.line_col(0), (1, 1)); // Start of line 1
    assert_eq!(source.line_col(5), (1, 6)); // End of line 1
    assert_eq!(source.line_col(6), (2, 1)); // Start of line 2
    assert_eq!(source.line_col(12), (3, 1)); // Start of line 3
}

#[test]
fn test_get_line() {
    let source = SourceFile::new(
        PathBuf::from("test.astra"),
        "line1\nline2\nline3".to_string(),
    );

    assert_eq!(source.get_line(1), Some("line1"));
    assert_eq!(source.get_line(2), Some("line2"));
    assert_eq!(source.get_line(3), Some("line3"));
    assert_eq!(source.get_line(4), None);
}

#[test]
fn test_span() {
    let source = SourceFile::new(PathBuf::from("test.astra"), "let x = 42".to_string());

    let span = source.span(4, 5);
    assert_eq!(span.start_line, 1);
    assert_eq!(span.start_col, 5);
    assert_eq!(span.end_line, 1);
    assert_eq!(span.end_col, 6);
}
