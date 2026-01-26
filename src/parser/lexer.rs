//! Lexer for the Astra programming language

use crate::diagnostics::{Diagnostic, Span};
use crate::parser::span::SourceFile;
use logos::Logos;

/// Token types for Astra
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n]+")]
pub enum TokenKind {
    // Keywords
    #[token("and")]
    And,
    #[token("as")]
    As,
    #[token("assert")]
    Assert,
    #[token("else")]
    Else,
    #[token("effects")]
    Effects,
    #[token("ensures")]
    Ensures,
    #[token("enum")]
    Enum,
    #[token("false")]
    False,
    #[token("fn")]
    Fn,
    #[token("for")]
    For,
    #[token("forall")]
    Forall,
    #[token("if")]
    If,
    #[token("import")]
    Import,
    #[token("in")]
    In,
    #[token("invariant")]
    Invariant,
    #[token("let")]
    Let,
    #[token("match")]
    Match,
    #[token("module")]
    Module,
    #[token("mut")]
    Mut,
    #[token("not")]
    Not,
    #[token("or")]
    Or,
    #[token("property")]
    Property,
    #[token("public")]
    Public,
    #[token("requires")]
    Requires,
    #[token("return")]
    Return,
    #[token("test")]
    Test,
    #[token("then")]
    Then,
    #[token("true")]
    True,
    #[token("type")]
    Type,
    #[token("using")]
    Using,

    // Literals
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    IntLit(i64),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    TextLit(String),

    // Identifiers
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    // Operators
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,

    #[token("==")]
    EqEq,
    #[token("!=")]
    BangEq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,

    #[token("?")]
    Question,
    #[token("?else")]
    QuestionElse,

    // Punctuation
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token("=")]
    Eq,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    #[token("|")]
    Pipe,
    #[token(".")]
    Dot,
    #[token("_")]
    Underscore,
    #[token("???")]
    Hole,

    // Comments
    #[regex(r"##[^\n]*", |lex| lex.slice().to_string())]
    DocComment(String),

    #[regex(r"#[^\n]*", |lex| lex.slice().to_string())]
    LineComment(String),

    // End of file
    Eof,
}

/// A token with its span
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Lexer for Astra source code
pub struct Lexer<'a> {
    source: &'a SourceFile,
    logos_lexer: logos::Lexer<'a, TokenKind>,
    peeked: Option<Token>,
    at_eof: bool,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given source file
    pub fn new(source: &'a SourceFile) -> Self {
        Self {
            source,
            logos_lexer: TokenKind::lexer(source.content()),
            peeked: None,
            at_eof: false,
        }
    }

    /// Get the next token
    pub fn next_token(&mut self) -> Result<Token, Diagnostic> {
        if let Some(token) = self.peeked.take() {
            return Ok(token);
        }

        if self.at_eof {
            return Ok(Token::new(
                TokenKind::Eof,
                self.source.span(self.source.content().len(), self.source.content().len()),
            ));
        }

        match self.logos_lexer.next() {
            Some(Ok(kind)) => {
                let span_range = self.logos_lexer.span();
                let span = self.source.span(span_range.start, span_range.end);
                Ok(Token::new(kind, span))
            }
            Some(Err(())) => {
                let span_range = self.logos_lexer.span();
                let span = self.source.span(span_range.start, span_range.end);
                Err(Diagnostic::error(crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN)
                    .message(format!("Unexpected character: {:?}", self.logos_lexer.slice()))
                    .span(span)
                    .build())
            }
            None => {
                self.at_eof = true;
                Ok(Token::new(
                    TokenKind::Eof,
                    self.source.span(self.source.content().len(), self.source.content().len()),
                ))
            }
        }
    }

    /// Peek at the next token without consuming it
    pub fn peek(&mut self) -> Result<&Token, Diagnostic> {
        if self.peeked.is_none() {
            self.peeked = Some(self.next_token()?);
        }
        Ok(self.peeked.as_ref().unwrap())
    }

    /// Check if we're at the end of the file
    pub fn is_eof(&mut self) -> bool {
        self.peek().map(|t| t.kind == TokenKind::Eof).unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn lex(source: &str) -> Vec<TokenKind> {
        let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
        let mut lexer = Lexer::new(&source_file);
        let mut tokens = Vec::new();

        loop {
            match lexer.next_token() {
                Ok(token) => {
                    if token.kind == TokenKind::Eof {
                        break;
                    }
                    tokens.push(token.kind);
                }
                Err(_) => break,
            }
        }

        tokens
    }

    #[test]
    fn test_keywords() {
        assert_eq!(lex("fn let if else match"), vec![
            TokenKind::Fn,
            TokenKind::Let,
            TokenKind::If,
            TokenKind::Else,
            TokenKind::Match,
        ]);
    }

    #[test]
    fn test_literals() {
        assert_eq!(lex("42 true false"), vec![
            TokenKind::IntLit(42),
            TokenKind::True,
            TokenKind::False,
        ]);
    }

    #[test]
    fn test_identifiers() {
        assert_eq!(lex("foo bar_baz _underscore"), vec![
            TokenKind::Ident("foo".to_string()),
            TokenKind::Ident("bar_baz".to_string()),
            TokenKind::Ident("_underscore".to_string()),
        ]);
    }

    #[test]
    fn test_operators() {
        assert_eq!(lex("+ - * / == != < > <= >="), vec![
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
        ]);
    }

    #[test]
    fn test_punctuation() {
        assert_eq!(lex("( ) { } [ ] , : = -> => |"), vec![
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
        ]);
    }
}
