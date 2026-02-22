use super::*;
use std::path::PathBuf;

fn lex(source: &str) -> Vec<TokenKind> {
    let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
    let mut lexer = Lexer::new(&source_file);
    let mut tokens = Vec::new();

    loop {
        let token = lexer.next_token();
        if token.kind == TokenKind::Eof {
            break;
        }
        tokens.push(token.kind);
    }

    tokens
}

#[test]
fn test_keywords() {
    assert_eq!(
        lex("fn let if else match"),
        vec![
            TokenKind::Fn,
            TokenKind::Let,
            TokenKind::If,
            TokenKind::Else,
            TokenKind::Match,
        ]
    );
}

#[test]
fn test_literals() {
    assert_eq!(
        lex("42 true false"),
        vec![TokenKind::IntLit(42), TokenKind::True, TokenKind::False,]
    );
}

#[test]
fn test_identifiers() {
    assert_eq!(
        lex("foo bar_baz _underscore"),
        vec![
            TokenKind::Ident("foo".to_string()),
            TokenKind::Ident("bar_baz".to_string()),
            TokenKind::Ident("_underscore".to_string()),
        ]
    );
}

#[test]
fn test_operators() {
    assert_eq!(
        lex("+ - * / == != < > <= >="),
        vec![
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::EqEq,
            TokenKind::BangEq,
            TokenKind::Lt,
            TokenKind::Gt,
            TokenKind::LtEq,
            TokenKind::GtEq,
        ]
    );
}

#[test]
fn test_punctuation() {
    assert_eq!(
        lex("( ) { } [ ] , : = -> => |"),
        vec![
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::Comma,
            TokenKind::Colon,
            TokenKind::Eq,
            TokenKind::Arrow,
            TokenKind::FatArrow,
            TokenKind::Pipe,
        ]
    );
}

#[test]
fn test_range_tokens() {
    assert_eq!(
        lex("0..10"),
        vec![
            TokenKind::IntLit(0),
            TokenKind::DotDot,
            TokenKind::IntLit(10)
        ]
    );
    assert_eq!(
        lex("0..=10"),
        vec![
            TokenKind::IntLit(0),
            TokenKind::DotDotEq,
            TokenKind::IntLit(10)
        ]
    );
    // Dot should still work for field access
    assert_eq!(
        lex("x.y"),
        vec![
            TokenKind::Ident("x".to_string()),
            TokenKind::Dot,
            TokenKind::Ident("y".to_string()),
        ]
    );
}
