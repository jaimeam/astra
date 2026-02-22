//! Lexer for the Astra programming language

use crate::diagnostics::Span;
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
    #[token("while")]
    While,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,

    #[token("trait")]
    Trait,
    #[token("impl")]
    Impl,
    #[token("effect")]
    Effect,
    #[token("await")]
    Await,
    #[token("async")]
    Async,

    // Literals
    #[regex(r"[0-9]+\.[0-9]+", priority = 3, callback = |lex| lex.slice().parse::<f64>().ok())]
    FloatLit(f64),

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    IntLit(i64),

    #[token(r#"""""#, priority = 4, callback = multiline_string_callback)]
    MultilineTextLit(String),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    TextLit(String),

    // Identifiers (note: single underscore is handled as Underscore token, not Ident)
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", priority = 1, callback = |lex| lex.slice().to_string())]
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

    // E1: Compound assignment operators
    #[token("+=")]
    PlusEq,
    #[token("-=")]
    MinusEq,
    #[token("*=")]
    StarEq,
    #[token("/=")]
    SlashEq,
    #[token("%=")]
    PercentEq,

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
    #[token("|>")]
    PipeArrow,
    #[token("|")]
    Pipe,
    #[token("..=")]
    DotDotEq,
    #[token("..")]
    DotDot,
    #[token(".")]
    Dot,
    #[token("_", priority = 3)]
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

/// Callback for lexing multiline triple-quoted strings.
/// Scans forward from after the opening `"""` to find the closing `"""`.
fn multiline_string_callback(lex: &mut logos::Lexer<TokenKind>) -> Option<String> {
    let remainder = lex.remainder();
    // Find the closing """
    if let Some(end) = remainder.find("\"\"\"") {
        let content = &remainder[..end];
        lex.bump(end + 3); // consume content + closing """
        Some(content.to_string())
    } else {
        None // unterminated — logos will report an error token
    }
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
    peeked2: Option<Token>,
    at_eof: bool,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given source file
    pub fn new(source: &'a SourceFile) -> Self {
        Self {
            source,
            logos_lexer: TokenKind::lexer(source.content()),
            peeked: None,
            peeked2: None,
            at_eof: false,
        }
    }

    /// Get the next token (never fails - returns Error token on invalid input)
    pub fn next_token(&mut self) -> Token {
        if let Some(token) = self.peeked.take() {
            self.peeked = self.peeked2.take();
            return token;
        }

        self.read_token()
    }

    fn read_token(&mut self) -> Token {
        if self.at_eof {
            return Token::new(
                TokenKind::Eof,
                self.source
                    .span(self.source.content().len(), self.source.content().len()),
            );
        }

        loop {
            match self.logos_lexer.next() {
                Some(Ok(kind)) => {
                    // Skip comments
                    match &kind {
                        TokenKind::LineComment(_) | TokenKind::DocComment(_) => continue,
                        _ => {}
                    }
                    let span_range = self.logos_lexer.span();
                    let span = self.source.span(span_range.start, span_range.end);
                    return Token::new(kind, span);
                }
                Some(Err(())) => {
                    // On error, skip the character and continue
                    let span_range = self.logos_lexer.span();
                    let span = self.source.span(span_range.start, span_range.end);
                    // Return an error token - parser will handle it
                    return Token::new(TokenKind::Eof, span);
                }
                None => {
                    self.at_eof = true;
                    return Token::new(
                        TokenKind::Eof,
                        self.source
                            .span(self.source.content().len(), self.source.content().len()),
                    );
                }
            }
        }
    }

    /// Peek at the next token without consuming it
    pub fn peek(&mut self) -> &Token {
        if self.peeked.is_none() {
            self.peeked = Some(self.read_token());
        }
        self.peeked.as_ref().unwrap()
    }

    /// Peek two tokens ahead
    pub fn peek_ahead(&mut self) -> Token {
        if self.peeked.is_none() {
            self.peeked = Some(self.read_token());
        }
        if self.peeked2.is_none() {
            self.peeked2 = Some(self.read_token());
        }
        self.peeked2.clone().unwrap()
    }

    /// Check if we're at the end of the file
    pub fn is_eof(&mut self) -> bool {
        self.peek().kind == TokenKind::Eof
    }
}
#[cfg(test)]
#[path = "lexer_tests.rs"]
mod tests;
