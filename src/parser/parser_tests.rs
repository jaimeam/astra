use super::*;

#[test]
fn test_unescape_valid_sequences() {
    assert_eq!(unescape_string(r"hello\nworld").unwrap(), "hello\nworld");
    assert_eq!(unescape_string(r"tab\there").unwrap(), "tab\there");
    assert_eq!(unescape_string(r"cr\rhere").unwrap(), "cr\rhere");
    assert_eq!(
        unescape_string(r"escaped\\slash").unwrap(),
        "escaped\\slash"
    );
    assert_eq!(
        unescape_string(r#"escaped\"quote"#).unwrap(),
        "escaped\"quote"
    );
    assert_eq!(unescape_string(r"null\0byte").unwrap(), "null\0byte");
    assert_eq!(unescape_string(r"dollar\$sign").unwrap(), "dollar$sign");
    assert_eq!(unescape_string("no escapes").unwrap(), "no escapes");
}

#[test]
fn test_unescape_invalid_sequences() {
    assert!(unescape_string(r"\q").is_err());
    assert!(unescape_string(r"\a").is_err());
    assert!(unescape_string(r"\x").is_err());
    assert!(unescape_string(r"hello\qworld").is_err());

    let err = unescape_string(r"\q").unwrap_err();
    assert!(err.0.contains("Invalid escape sequence"));
    assert_eq!(err.1, 'q');
}

#[test]
fn test_dedent_multiline_string() {
    // Basic dedent
    let input = "\n    hello\n    world\n    ";
    assert_eq!(dedent_multiline_string(input), "hello\nworld");

    // Mixed indentation: dedent to minimum
    let input2 = "\n    hello\n      world\n    ";
    assert_eq!(dedent_multiline_string(input2), "hello\n  world");

    // No indentation
    let input3 = "\nhello\nworld\n";
    assert_eq!(dedent_multiline_string(input3), "hello\nworld");

    // Single line
    let input4 = "\n    hello\n    ";
    assert_eq!(dedent_multiline_string(input4), "hello");
}
