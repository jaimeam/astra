use super::*;
use crate::diagnostics::Span;
use crate::parser::ast::{Expr, NodeId};

fn dummy_span() -> Span {
    Span::new(std::path::PathBuf::from("test.astra"), 0, 0, 1, 1, 1, 1)
}

#[test]
fn test_extract_call_int_arg_match() {
    let expr = Expr::Call {
        id: NodeId::new(),
        span: dummy_span(),
        func: Box::new(Expr::Ident {
            id: NodeId::new(),
            span: dummy_span(),
            name: "seeded_rand".to_string(),
        }),
        args: vec![Expr::IntLit {
            id: NodeId::new(),
            span: dummy_span(),
            value: 42,
        }],
    };
    assert_eq!(extract_call_int_arg(&expr, "seeded_rand"), Some(42));
}

#[test]
fn test_extract_call_int_arg_wrong_name() {
    let expr = Expr::Call {
        id: NodeId::new(),
        span: dummy_span(),
        func: Box::new(Expr::Ident {
            id: NodeId::new(),
            span: dummy_span(),
            name: "other_fn".to_string(),
        }),
        args: vec![Expr::IntLit {
            id: NodeId::new(),
            span: dummy_span(),
            value: 42,
        }],
    };
    assert_eq!(extract_call_int_arg(&expr, "seeded_rand"), None);
}

#[test]
fn test_extract_method_int_arg_match() {
    let expr = Expr::MethodCall {
        id: NodeId::new(),
        span: dummy_span(),
        receiver: Box::new(Expr::Ident {
            id: NodeId::new(),
            span: dummy_span(),
            name: "Clock".to_string(),
        }),
        method: "fixed".to_string(),
        args: vec![Expr::IntLit {
            id: NodeId::new(),
            span: dummy_span(),
            value: 1000,
        }],
    };
    assert_eq!(extract_method_int_arg(&expr, "Clock", "fixed"), Some(1000));
}

#[test]
fn test_extract_method_int_arg_wrong_receiver() {
    let expr = Expr::MethodCall {
        id: NodeId::new(),
        span: dummy_span(),
        receiver: Box::new(Expr::Ident {
            id: NodeId::new(),
            span: dummy_span(),
            name: "Other".to_string(),
        }),
        method: "fixed".to_string(),
        args: vec![Expr::IntLit {
            id: NodeId::new(),
            span: dummy_span(),
            value: 1000,
        }],
    };
    assert_eq!(extract_method_int_arg(&expr, "Clock", "fixed"), None);
}

#[test]
fn test_build_test_capabilities_default() {
    let caps = build_test_capabilities(&None);
    assert!(caps.console.is_some());
    assert!(caps.rand.is_none());
    assert!(caps.clock.is_none());
}

#[test]
fn test_configure_search_paths() {
    let mut interpreter = Interpreter::new();
    configure_search_paths(&mut interpreter, None);
    assert!(!interpreter.search_paths.is_empty());
}

#[test]
fn test_explain_known_code() {
    let result = get_error_explanation("E1001");
    assert!(result.is_some());
    let text = result.unwrap();
    assert!(text.contains("Type mismatch"));
}

#[test]
fn test_explain_unknown_code() {
    let result = get_error_explanation("E9999");
    assert!(result.is_none());
}

#[test]
fn test_explain_all_error_codes() {
    // Verify all documented error codes have explanations
    let codes = [
        "E0001", "E0002", "E0003", "E0004", "E0005", "E0006", "E0007", "E0008", "E0009", "E0010",
        "E0011", "E1001", "E1002", "E1003", "E1004", "E1005", "E1006", "E1007", "E1008", "E1009",
        "E1010", "E1011", "E1012", "E1013", "E1014", "E1015", "E1016", "E2001", "E2002", "E2003",
        "E2004", "E2005", "E2006", "E2007", "E3001", "E3002", "E3003", "E3004", "E3005", "E4001",
        "E4002", "E4003", "E4004", "E4005", "E4006", "E4007", "E4008", "W0001", "W0002", "W0003",
        "W0004", "W0005", "W0006", "W0007", "W0008",
    ];
    for code in &codes {
        assert!(
            get_error_explanation(code).is_some(),
            "Missing explanation for {}",
            code
        );
    }
}

#[test]
fn test_explain_warning_codes() {
    let result = get_error_explanation("W0001");
    assert!(result.is_some());
    let text = result.unwrap();
    assert!(text.contains("Unused variable"));
}

#[test]
fn test_generate_claude_md_app() {
    let md = generate_claude_md("my_app", false);
    assert!(md.contains("# my_app"));
    assert!(md.contains("astra check src/"));
    assert!(md.contains("astra test"));
    assert!(md.contains("astra fmt src/"));
    assert!(md.contains("astra fix src/"));
    assert!(md.contains("astra run src/main.astra"));
    assert!(md.contains("astra explain"));
    assert!(md.contains("effects"));
    assert!(md.contains("E0xxx"));
    assert!(md.contains("Option[T]"));
}

#[test]
fn test_generate_claude_md_lib() {
    let md = generate_claude_md("my_lib", true);
    assert!(md.contains("# my_lib"));
    assert!(md.contains("astra check src/"));
    assert!(md.contains("astra test"));
    // Lib projects should not include a run command
    assert!(!md.contains("astra run src/main.astra"));
}

#[test]
fn test_init_creates_claude_md() {
    let tmp = std::env::temp_dir().join("astra_test_init_claude_md");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Simulate what run_init does for the .claude directory
    let claude_dir = tmp.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    let md = generate_claude_md("test_project", false);
    std::fs::write(claude_dir.join("CLAUDE.md"), &md).unwrap();

    let path = tmp.join(".claude").join("CLAUDE.md");
    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# test_project"));
    assert!(content.contains("astra check"));

    let _ = std::fs::remove_dir_all(&tmp);
}
