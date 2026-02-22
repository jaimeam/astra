use super::*;

#[test]
fn test_find_ident_at_position() {
    let source = "fn hello(x: Int) -> Text {\n  x\n}";
    assert_eq!(find_ident_at_position(source, 0, 3), "hello");
    assert_eq!(find_ident_at_position(source, 0, 9), "x");
    assert_eq!(find_ident_at_position(source, 0, 12), "Int");
}

#[test]
fn test_span_contains() {
    let span = Span::new(
        std::path::PathBuf::from("test.astra"),
        0,
        10,
        1,  // start_line
        5,  // start_col
        1,  // end_line
        15, // end_col
    );
    assert!(span_contains(&span, 0, 5)); // line 0, col 5 -> line 1, col 6
    assert!(span_contains(&span, 0, 10));
    assert!(!span_contains(&span, 0, 3)); // col 3 -> line 1, col 4, before span
    assert!(!span_contains(&span, 1, 5)); // line 1 -> line 2, past span
}

#[test]
fn test_uri_to_path() {
    assert_eq!(uri_to_path("file:///tmp/test.astra"), "/tmp/test.astra");
    assert_eq!(uri_to_path("/tmp/test.astra"), "/tmp/test.astra");
}

fn test_span() -> Span {
    Span::new(std::path::PathBuf::from("test.astra"), 0, 0, 1, 1, 1, 1)
}

#[test]
fn test_format_type_expr() {
    let ty = TypeExpr::Named {
        id: NodeId::new(),
        span: test_span(),
        name: "Int".to_string(),
        args: vec![],
    };
    assert_eq!(format_type_expr(&ty), "Int");

    let ty = TypeExpr::Named {
        id: NodeId::new(),
        span: test_span(),
        name: "List".to_string(),
        args: vec![TypeExpr::Named {
            id: NodeId::new(),
            span: test_span(),
            name: "Int".to_string(),
            args: vec![],
        }],
    };
    assert_eq!(format_type_expr(&ty), "List[Int]");
}

#[test]
fn test_diagnostic_to_lsp() {
    let diag = crate::diagnostics::Diagnostic {
        code: "E1001".to_string(),
        message: "Type mismatch".to_string(),
        severity: Severity::Error,
        span: test_span(),
        notes: vec![],
        suggestions: vec![],
    };
    let lsp = diagnostic_to_lsp(&diag);
    assert_eq!(lsp["severity"], 1);
    assert_eq!(lsp["code"], "E1001");
    assert_eq!(lsp["source"], "astra");
}

#[test]
fn test_span_to_range() {
    let span = Span::new(
        std::path::PathBuf::from("test.astra"),
        0,
        5,
        3,  // start_line
        10, // start_col
        3,  // end_line
        15, // end_col
    );
    let range = span_to_range(&span);
    assert_eq!(range["start"]["line"], 2);
    assert_eq!(range["start"]["character"], 9);
    assert_eq!(range["end"]["line"], 2);
    assert_eq!(range["end"]["character"], 14);
}

#[test]
fn test_code_action_from_suggestion() {
    use crate::diagnostics::{Edit, Suggestion};

    let mut server = LspServer::new();
    let uri = "file:///test.astra";

    // Cache a diagnostic with a suggestion
    let diag = crate::diagnostics::Diagnostic {
        code: "E1002".to_string(),
        message: "Unknown identifier 'prnt'".to_string(),
        severity: Severity::Error,
        span: Span::new(std::path::PathBuf::from("/test.astra"), 0, 4, 1, 1, 1, 4),
        notes: vec![],
        suggestions: vec![
            Suggestion::new("Did you mean 'print'?").with_edit(Edit::new(
                Span::new(std::path::PathBuf::from("/test.astra"), 0, 4, 1, 1, 1, 4),
                "print",
            )),
        ],
    };
    server
        .cached_diagnostics
        .insert(uri.to_string(), vec![diag]);

    // Request code actions for line 0
    let params = json!({
        "textDocument": { "uri": uri },
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 4 }
        },
        "context": { "diagnostics": [] }
    });

    let result = server.handle_code_action(&params);
    let actions = result.as_array().unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0]["title"], "Did you mean 'print'?");
    assert_eq!(actions[0]["kind"], "quickfix");

    // Verify the edit
    let changes = &actions[0]["edit"]["changes"][uri];
    let edits = changes.as_array().unwrap();
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["newText"], "print");
}

#[test]
fn test_code_action_no_diagnostics() {
    let server = LspServer::new();
    let params = json!({
        "textDocument": { "uri": "file:///empty.astra" },
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
        },
        "context": { "diagnostics": [] }
    });
    let result = server.handle_code_action(&params);
    assert_eq!(result, json!([]));
}
