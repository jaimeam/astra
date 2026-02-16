//! Recursive descent parser for Astra
//!
//! This is a foundational implementation that will be expanded.
#![allow(clippy::result_large_err)]

use crate::diagnostics::{Diagnostic, DiagnosticBag, Span};
use crate::parser::ast::*;
use crate::parser::lexer::{Lexer, Token, TokenKind};
use crate::parser::span::SourceFile;

/// Parser for Astra source code
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    #[allow(dead_code)]
    source: SourceFile,
    errors: DiagnosticBag,
    peeked: Option<Token>,
}

impl<'a> Parser<'a> {
    /// Create a new parser
    pub fn new(lexer: Lexer<'a>, source: SourceFile) -> Self {
        Self {
            lexer,
            source,
            errors: DiagnosticBag::new(),
            peeked: None,
        }
    }

    /// Parse a complete module
    pub fn parse_module(&mut self) -> Result<Module, DiagnosticBag> {
        let start_span = self.current_span();

        // Parse module declaration
        self.expect(TokenKind::Module)?;
        let name = self.parse_module_path()?;

        // Parse items
        let mut items = Vec::new();
        while !self.is_eof() {
            match self.parse_item() {
                Ok(item) => items.push(item),
                Err(diag) => {
                    self.errors.push(diag);
                    self.recover_to_next_item();
                }
            }
        }

        if self.errors.has_errors() {
            return Err(self.errors.clone());
        }

        let end_span = self.current_span();
        Ok(Module {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            items,
        })
    }

    fn parse_module_path(&mut self) -> Result<ModulePath, Diagnostic> {
        let start_span = self.current_span();
        let first = self.expect_ident()?;
        let mut segments = vec![first];

        while self.check(TokenKind::Dot) {
            self.advance();
            segments.push(self.expect_ident()?);
        }

        let end_span = self.current_span();
        Ok(ModulePath {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            segments,
        })
    }

    fn parse_item(&mut self) -> Result<Item, Diagnostic> {
        let token = self.peek();
        match &token.kind {
            TokenKind::Import => self.parse_import().map(Item::Import),
            TokenKind::Type => self.parse_type_def().map(Item::TypeDef),
            TokenKind::Enum => self.parse_enum_def().map(Item::EnumDef),
            TokenKind::Fn | TokenKind::Public => self.parse_fn_def().map(Item::FnDef),
            TokenKind::Test => self.parse_test().map(Item::Test),
            TokenKind::Property => self.parse_property().map(Item::Property),
            _ => Err(self.error_unexpected("item")),
        }
    }

    fn parse_import(&mut self) -> Result<ImportDecl, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Import)?;
        let path = self.parse_module_path()?;

        let kind = if self.check(TokenKind::As) {
            self.advance();
            ImportKind::Alias(self.expect_ident()?)
        } else if self.check(TokenKind::Dot) {
            self.advance();
            self.expect(TokenKind::LBrace)?;
            let mut items = vec![self.expect_ident()?];
            while self.check(TokenKind::Comma) {
                self.advance();
                if self.check(TokenKind::RBrace) {
                    break;
                }
                items.push(self.expect_ident()?);
            }
            self.expect(TokenKind::RBrace)?;
            ImportKind::Items(items)
        } else {
            ImportKind::Module
        };

        let end_span = self.current_span();
        Ok(ImportDecl {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            path,
            kind,
        })
    }

    fn parse_type_def(&mut self) -> Result<TypeDef, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Type)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_optional_type_params()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_type_expr()?;
        let invariant = None; // Simplified for now

        let end_span = self.current_span();
        Ok(TypeDef {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            type_params,
            value,
            invariant,
        })
    }

    fn parse_enum_def(&mut self) -> Result<EnumDef, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Enum)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_optional_type_params()?;
        self.expect(TokenKind::Eq)?;

        if self.check(TokenKind::Pipe) {
            self.advance();
        }

        let mut variants = vec![self.parse_variant()?];
        while self.check(TokenKind::Pipe) {
            self.advance();
            variants.push(self.parse_variant()?);
        }

        let end_span = self.current_span();
        Ok(EnumDef {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            type_params,
            variants,
        })
    }

    fn parse_variant(&mut self) -> Result<Variant, Diagnostic> {
        let start_span = self.current_span();
        let name = self.expect_ident()?;

        let fields = if self.check(TokenKind::LParen) {
            self.advance();
            let mut fields = Vec::new();
            if !self.check(TokenKind::RParen) {
                fields.push(self.parse_field()?);
                while self.check(TokenKind::Comma) {
                    self.advance();
                    if self.check(TokenKind::RParen) {
                        break;
                    }
                    fields.push(self.parse_field()?);
                }
            }
            self.expect(TokenKind::RParen)?;
            fields
        } else {
            Vec::new()
        };

        let end_span = self.current_span();
        Ok(Variant {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            fields,
        })
    }

    fn parse_field(&mut self) -> Result<Field, Diagnostic> {
        let start_span = self.current_span();
        let name = self.expect_ident()?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type_expr()?;

        let end_span = self.current_span();
        Ok(Field {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            ty,
        })
    }

    fn parse_fn_def(&mut self) -> Result<FnDef, Diagnostic> {
        let start_span = self.current_span();

        let visibility = if self.check(TokenKind::Public) {
            self.advance();
            Visibility::Public
        } else {
            Visibility::Private
        };

        self.expect(TokenKind::Fn)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen)?;

        let return_type = if self.check(TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let effects = if self.check(TokenKind::Effects) {
            self.advance();
            self.expect(TokenKind::LParen)?;
            let mut effects = vec![self.expect_ident()?];
            while self.check(TokenKind::Comma) {
                self.advance();
                effects.push(self.expect_ident()?);
            }
            self.expect(TokenKind::RParen)?;
            effects
        } else {
            Vec::new()
        };

        // Parse optional requires clauses
        let mut requires = Vec::new();
        while self.check(TokenKind::Requires) {
            self.advance();
            requires.push(self.parse_expr()?);
        }

        // Parse optional ensures clauses
        let mut ensures = Vec::new();
        while self.check(TokenKind::Ensures) {
            self.advance();
            ensures.push(self.parse_expr()?);
        }

        let body = self.parse_block()?;

        let end_span = self.current_span();
        Ok(FnDef {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            visibility,
            name,
            params,
            return_type,
            effects,
            requires,
            ensures,
            body,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, Diagnostic> {
        let mut params = Vec::new();
        if !self.check(TokenKind::RParen) {
            params.push(self.parse_param()?);
            while self.check(TokenKind::Comma) {
                self.advance();
                if self.check(TokenKind::RParen) {
                    break;
                }
                params.push(self.parse_param()?);
            }
        }
        Ok(params)
    }

    fn parse_param(&mut self) -> Result<Param, Diagnostic> {
        let start_span = self.current_span();
        let name = self.expect_ident()?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type_expr()?;

        let end_span = self.current_span();
        Ok(Param {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            ty,
        })
    }

    fn parse_test(&mut self) -> Result<TestBlock, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Test)?;
        let name = self.expect_text()?;

        // Parse optional using clause
        let using = if self.check(TokenKind::Using) {
            Some(self.parse_using_clause()?)
        } else {
            None
        };

        let body = self.parse_block()?;

        let end_span = self.current_span();
        Ok(TestBlock {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            using,
            body,
        })
    }

    fn parse_property(&mut self) -> Result<PropertyBlock, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Property)?;
        let name = self.expect_text()?;

        // Parse optional using clause
        let using = if self.check(TokenKind::Using) {
            Some(self.parse_using_clause()?)
        } else {
            None
        };

        let body = self.parse_block()?;

        let end_span = self.current_span();
        Ok(PropertyBlock {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            using,
            body,
        })
    }

    /// Parse `using effects(Effect = Expr, ...)` clause
    fn parse_using_clause(&mut self) -> Result<UsingClause, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Using)?;
        self.expect(TokenKind::Effects)?;
        self.expect(TokenKind::LParen)?;

        let mut bindings = Vec::new();

        if !self.check(TokenKind::RParen) {
            bindings.push(self.parse_effect_binding()?);
            while self.check(TokenKind::Comma) {
                self.advance();
                if self.check(TokenKind::RParen) {
                    break;
                }
                bindings.push(self.parse_effect_binding()?);
            }
        }

        self.expect(TokenKind::RParen)?;

        let end_span = self.current_span();
        Ok(UsingClause {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            bindings,
        })
    }

    /// Parse `Effect = Expr` binding (e.g., `Rand = Rand.seeded(42)`)
    fn parse_effect_binding(&mut self) -> Result<EffectBinding, Diagnostic> {
        let start_span = self.current_span();
        let effect = self.expect_ident()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_expr()?;

        let end_span = self.current_span();
        Ok(EffectBinding {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            effect,
            value: Box::new(value),
        })
    }

    fn parse_type_expr(&mut self) -> Result<TypeExpr, Diagnostic> {
        let start_span = self.current_span();

        if self.check(TokenKind::LBrace) {
            self.advance();
            let mut fields = Vec::new();
            if !self.check(TokenKind::RBrace) {
                fields.push(self.parse_field()?);
                while self.check(TokenKind::Comma) {
                    self.advance();
                    if self.check(TokenKind::RBrace) {
                        break;
                    }
                    fields.push(self.parse_field()?);
                }
            }
            self.expect(TokenKind::RBrace)?;

            let end_span = self.current_span();
            Ok(TypeExpr::Record {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                fields,
            })
        } else {
            let name = self.expect_ident()?;
            let args = self.parse_optional_type_args()?;

            let end_span = self.current_span();
            Ok(TypeExpr::Named {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                name,
                args,
            })
        }
    }

    fn parse_optional_type_params(&mut self) -> Result<Vec<String>, Diagnostic> {
        if !self.check(TokenKind::LBracket) {
            return Ok(Vec::new());
        }
        self.advance();
        let mut params = vec![self.expect_ident()?];
        while self.check(TokenKind::Comma) {
            self.advance();
            params.push(self.expect_ident()?);
        }
        self.expect(TokenKind::RBracket)?;
        Ok(params)
    }

    fn parse_optional_type_args(&mut self) -> Result<Vec<TypeExpr>, Diagnostic> {
        if !self.check(TokenKind::LBracket) {
            return Ok(Vec::new());
        }
        self.advance();
        let mut args = vec![self.parse_type_expr()?];
        while self.check(TokenKind::Comma) {
            self.advance();
            args.push(self.parse_type_expr()?);
        }
        self.expect(TokenKind::RBracket)?;
        Ok(args)
    }

    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::LBrace)?;

        let mut stmts = Vec::new();
        let mut expr = None;

        while !self.check(TokenKind::RBrace) && !self.is_eof() {
            if self.check(TokenKind::Let) || self.check(TokenKind::Return) {
                stmts.push(self.parse_stmt()?);
            } else {
                // Parse an expression
                let e = self.parse_expr()?;

                // If we're at the end of the block, this is the final expression
                if self.check(TokenKind::RBrace) {
                    expr = Some(Box::new(e));
                    break;
                } else {
                    // Otherwise, this is an expression statement
                    let span = e.span().clone();
                    stmts.push(Stmt::Expr {
                        id: NodeId::new(),
                        span,
                        expr: Box::new(e),
                    });
                }
            }
        }

        self.expect(TokenKind::RBrace)?;

        let end_span = self.current_span();
        Ok(Block {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            stmts,
            expr,
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let start_span = self.current_span();

        if self.check(TokenKind::Let) {
            self.advance();
            let mutable = self.check(TokenKind::Mut);
            if mutable {
                self.advance();
            }
            let name = self.expect_ident()?;
            let ty = if self.check(TokenKind::Colon) {
                self.advance();
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            self.expect(TokenKind::Eq)?;
            let value = Box::new(self.parse_expr()?);

            let end_span = self.current_span();
            Ok(Stmt::Let {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                name,
                mutable,
                ty,
                value,
            })
        } else if self.check(TokenKind::Return) {
            self.advance();
            let value = if self.check(TokenKind::RBrace) {
                None
            } else {
                Some(Box::new(self.parse_expr()?))
            };

            let end_span = self.current_span();
            Ok(Stmt::Return {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                value,
            })
        } else {
            Err(self.error_unexpected("statement"))
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_binary_expr(0)
    }

    fn parse_binary_expr(&mut self, min_prec: u8) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_unary_expr()?;

        loop {
            let (op, prec) = match self.peek().kind {
                TokenKind::Or => (BinaryOp::Or, 1),
                TokenKind::And => (BinaryOp::And, 2),
                TokenKind::EqEq => (BinaryOp::Eq, 3),
                TokenKind::BangEq => (BinaryOp::Ne, 3),
                TokenKind::Lt => (BinaryOp::Lt, 4),
                TokenKind::LtEq => (BinaryOp::Le, 4),
                TokenKind::Gt => (BinaryOp::Gt, 4),
                TokenKind::GtEq => (BinaryOp::Ge, 4),
                TokenKind::Plus => (BinaryOp::Add, 5),
                TokenKind::Minus => (BinaryOp::Sub, 5),
                TokenKind::Star => (BinaryOp::Mul, 6),
                TokenKind::Slash => (BinaryOp::Div, 6),
                TokenKind::Percent => (BinaryOp::Mod, 6),
                _ => break,
            };

            if prec < min_prec {
                break;
            }

            let start_span = self.expr_span(&left);
            self.advance();
            let right = self.parse_binary_expr(prec + 1)?;
            let end_span = self.expr_span(&right);

            left = Expr::Binary {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_unary_expr(&mut self) -> Result<Expr, Diagnostic> {
        if self.check(TokenKind::Not) {
            let start_span = self.current_span();
            self.advance();
            let expr = self.parse_unary_expr()?;
            let end_span = self.expr_span(&expr);
            return Ok(Expr::Unary {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                op: UnaryOp::Not,
                expr: Box::new(expr),
            });
        }
        if self.check(TokenKind::Minus) {
            let start_span = self.current_span();
            self.advance();
            let expr = self.parse_unary_expr()?;
            let end_span = self.expr_span(&expr);
            return Ok(Expr::Unary {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                op: UnaryOp::Neg,
                expr: Box::new(expr),
            });
        }
        self.parse_postfix_expr()
    }

    fn parse_postfix_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary_expr()?;

        loop {
            if self.check(TokenKind::LParen) {
                let start_span = self.expr_span(&expr);
                self.advance();
                let mut args = Vec::new();
                if !self.check(TokenKind::RParen) {
                    args.push(self.parse_expr()?);
                    while self.check(TokenKind::Comma) {
                        self.advance();
                        if self.check(TokenKind::RParen) {
                            break;
                        }
                        args.push(self.parse_expr()?);
                    }
                }
                self.expect(TokenKind::RParen)?;
                let end_span = self.current_span();
                expr = Expr::Call {
                    id: NodeId::new(),
                    span: start_span.merge(&end_span),
                    func: Box::new(expr),
                    args,
                };
            } else if self.check(TokenKind::Dot) {
                let start_span = self.expr_span(&expr);
                self.advance();
                let name = self.expect_ident()?;

                if self.check(TokenKind::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !self.check(TokenKind::RParen) {
                        args.push(self.parse_expr()?);
                        while self.check(TokenKind::Comma) {
                            self.advance();
                            if self.check(TokenKind::RParen) {
                                break;
                            }
                            args.push(self.parse_expr()?);
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    let end_span = self.current_span();
                    expr = Expr::MethodCall {
                        id: NodeId::new(),
                        span: start_span.merge(&end_span),
                        receiver: Box::new(expr),
                        method: name,
                        args,
                    };
                } else {
                    let end_span = self.current_span();
                    expr = Expr::FieldAccess {
                        id: NodeId::new(),
                        span: start_span.merge(&end_span),
                        expr: Box::new(expr),
                        field: name,
                    };
                }
            } else if self.check(TokenKind::QuestionElse) {
                let start_span = self.expr_span(&expr);
                self.advance();
                let else_expr = self.parse_expr()?;
                let end_span = self.expr_span(&else_expr);
                expr = Expr::TryElse {
                    id: NodeId::new(),
                    span: start_span.merge(&end_span),
                    expr: Box::new(expr),
                    else_expr: Box::new(else_expr),
                };
            } else if self.check(TokenKind::Question) {
                let start_span = self.expr_span(&expr);
                self.advance();
                let end_span = self.current_span();
                expr = Expr::Try {
                    id: NodeId::new(),
                    span: start_span.merge(&end_span),
                    expr: Box::new(expr),
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary_expr(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();

        match &token.kind {
            TokenKind::IntLit(n) => {
                let value = *n;
                self.advance();
                Ok(Expr::IntLit {
                    id: NodeId::new(),
                    span: token.span,
                    value,
                })
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::BoolLit {
                    id: NodeId::new(),
                    span: token.span,
                    value: true,
                })
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::BoolLit {
                    id: NodeId::new(),
                    span: token.span,
                    value: false,
                })
            }
            TokenKind::TextLit(s) => {
                let value = s.clone();
                self.advance();
                Ok(Expr::TextLit {
                    id: NodeId::new(),
                    span: token.span,
                    value,
                })
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                Ok(Expr::Ident {
                    id: NodeId::new(),
                    span: token.span,
                    name,
                })
            }
            TokenKind::LBrace => self.parse_record_expr(),
            TokenKind::LParen => {
                self.advance();
                if self.check(TokenKind::RParen) {
                    self.advance();
                    return Ok(Expr::UnitLit {
                        id: NodeId::new(),
                        span: token.span,
                    });
                }
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }
            TokenKind::LBracket => self.parse_list_expr(),
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::Assert => self.parse_assert_expr(),
            TokenKind::Hole => {
                self.advance();
                Ok(Expr::Hole {
                    id: NodeId::new(),
                    span: token.span,
                })
            }
            _ => Err(self.error_unexpected("expression")),
        }
    }

    fn parse_record_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::LBrace)?;

        let mut fields = Vec::new();
        if !self.check(TokenKind::RBrace) {
            let name = self.expect_ident()?;
            self.expect(TokenKind::Eq)?;
            let value = self.parse_expr()?;
            fields.push((name, Box::new(value)));

            while self.check(TokenKind::Comma) {
                self.advance();
                if self.check(TokenKind::RBrace) {
                    break;
                }
                let name = self.expect_ident()?;
                self.expect(TokenKind::Eq)?;
                let value = self.parse_expr()?;
                fields.push((name, Box::new(value)));
            }
        }

        self.expect(TokenKind::RBrace)?;
        let end_span = self.current_span();

        Ok(Expr::Record {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            fields,
        })
    }

    fn parse_list_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::LBracket)?;

        let mut elements = Vec::new();
        if !self.check(TokenKind::RBracket) {
            elements.push(self.parse_expr()?);
            while self.check(TokenKind::Comma) {
                self.advance();
                if self.check(TokenKind::RBracket) {
                    break;
                }
                elements.push(self.parse_expr()?);
            }
        }

        self.expect(TokenKind::RBracket)?;
        let end_span = self.current_span();

        Ok(Expr::ListLit {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            elements,
        })
    }

    fn parse_if_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::If)?;
        let cond = Box::new(self.parse_expr()?);

        // Support both `if cond { ... }` and `if cond then expr else expr`
        if self.check(TokenKind::Then) {
            // N4: `if X then Y else Z` syntax
            self.advance();
            let then_expr = self.parse_expr()?;
            let then_span = then_expr.span().clone();
            let then_branch = Box::new(Block {
                id: NodeId::new(),
                span: then_span,
                stmts: Vec::new(),
                expr: Some(Box::new(then_expr)),
            });

            let else_branch = if self.check(TokenKind::Else) {
                self.advance();
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };

            let end_span = self.current_span();
            Ok(Expr::If {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                cond,
                then_branch,
                else_branch,
            })
        } else {
            let then_branch = Box::new(self.parse_block()?);

            let else_branch = if self.check(TokenKind::Else) {
                self.advance();
                if self.check(TokenKind::If) {
                    Some(Box::new(self.parse_if_expr()?))
                } else {
                    let block = self.parse_block()?;
                    Some(Box::new(Expr::Block {
                        id: NodeId::new(),
                        span: block.span.clone(),
                        block: Box::new(block),
                    }))
                }
            } else {
                None
            };

            let end_span = self.current_span();
            Ok(Expr::If {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                cond,
                then_branch,
                else_branch,
            })
        }
    }

    fn parse_assert_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Assert)?;
        let arg = self.parse_expr()?;
        let end_span = arg.span().clone();

        // Parse assert as a call to the builtin assert function
        Ok(Expr::Call {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            func: Box::new(Expr::Ident {
                id: NodeId::new(),
                span: start_span.clone(),
                name: "assert".to_string(),
            }),
            args: vec![arg],
        })
    }

    fn parse_match_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Match)?;
        let expr = Box::new(self.parse_expr()?);
        self.expect(TokenKind::LBrace)?;

        let mut arms = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_eof() {
            arms.push(self.parse_match_arm()?);
            if self.check(TokenKind::Comma) {
                self.advance();
            }
        }

        self.expect(TokenKind::RBrace)?;
        let end_span = self.current_span();

        Ok(Expr::Match {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            expr,
            arms,
        })
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm, Diagnostic> {
        let start_span = self.current_span();
        let pattern = self.parse_pattern()?;
        self.expect(TokenKind::FatArrow)?;
        let body = Box::new(self.parse_expr()?);

        let end_span = self.current_span();
        Ok(MatchArm {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            pattern,
            body,
        })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let token = self.peek().clone();

        match &token.kind {
            TokenKind::Underscore => {
                self.advance();
                Ok(Pattern::Wildcard {
                    id: NodeId::new(),
                    span: token.span,
                })
            }
            TokenKind::IntLit(n) => {
                let value = *n;
                self.advance();
                Ok(Pattern::IntLit {
                    id: NodeId::new(),
                    span: token.span,
                    value,
                })
            }
            TokenKind::True => {
                self.advance();
                Ok(Pattern::BoolLit {
                    id: NodeId::new(),
                    span: token.span,
                    value: true,
                })
            }
            TokenKind::False => {
                self.advance();
                Ok(Pattern::BoolLit {
                    id: NodeId::new(),
                    span: token.span,
                    value: false,
                })
            }
            TokenKind::TextLit(s) => {
                let value = s.clone();
                self.advance();
                Ok(Pattern::TextLit {
                    id: NodeId::new(),
                    span: token.span,
                    value,
                })
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();

                if self.check(TokenKind::LParen) {
                    self.advance();
                    let data = if self.check(TokenKind::RParen) {
                        None
                    } else {
                        Some(Box::new(self.parse_pattern()?))
                    };
                    self.expect(TokenKind::RParen)?;
                    let end_span = self.current_span();
                    Ok(Pattern::Variant {
                        id: NodeId::new(),
                        span: token.span.merge(&end_span),
                        name,
                        data,
                    })
                } else if name
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                {
                    Ok(Pattern::Variant {
                        id: NodeId::new(),
                        span: token.span,
                        name,
                        data: None,
                    })
                } else {
                    Ok(Pattern::Ident {
                        id: NodeId::new(),
                        span: token.span,
                        name,
                    })
                }
            }
            _ => Err(self.error_unexpected("pattern")),
        }
    }

    // Helper methods

    fn peek(&mut self) -> Token {
        if self.peeked.is_none() {
            self.peeked = Some(self.lexer.next_token());
        }
        self.peeked.clone().unwrap()
    }

    fn advance(&mut self) -> Token {
        if let Some(token) = self.peeked.take() {
            token
        } else {
            self.lexer.next_token()
        }
    }

    fn is_eof(&mut self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn current_span(&mut self) -> Span {
        self.peek().span.clone()
    }

    fn check(&mut self, kind: TokenKind) -> bool {
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(&kind)
    }

    #[allow(dead_code)]
    fn check_ident(&mut self, name: &str) -> bool {
        matches!(&self.peek().kind, TokenKind::Ident(n) if n == name)
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, Diagnostic> {
        let token = self.advance();
        if std::mem::discriminant(&token.kind) == std::mem::discriminant(&kind) {
            Ok(token)
        } else {
            Err(
                Diagnostic::error(crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN)
                    .message(format!("Expected {:?}, found {:?}", kind, token.kind))
                    .span(token.span)
                    .build(),
            )
        }
    }

    fn expect_ident(&mut self) -> Result<String, Diagnostic> {
        let token = self.advance();
        match token.kind {
            TokenKind::Ident(name) => Ok(name),
            _ => Err(
                Diagnostic::error(crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN)
                    .message(format!("Expected identifier, found {:?}", token.kind))
                    .span(token.span)
                    .build(),
            ),
        }
    }

    fn expect_text(&mut self) -> Result<String, Diagnostic> {
        let token = self.advance();
        match token.kind {
            TokenKind::TextLit(s) => Ok(s),
            _ => Err(
                Diagnostic::error(crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN)
                    .message(format!("Expected string literal, found {:?}", token.kind))
                    .span(token.span)
                    .build(),
            ),
        }
    }

    fn error_unexpected(&mut self, expected: &str) -> Diagnostic {
        let token = self.peek();
        Diagnostic::error(crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN)
            .message(format!("Expected {}, found {:?}", expected, token.kind))
            .span(token.span.clone())
            .build()
    }

    fn expr_span(&self, expr: &Expr) -> Span {
        match expr {
            Expr::IntLit { span, .. }
            | Expr::BoolLit { span, .. }
            | Expr::TextLit { span, .. }
            | Expr::UnitLit { span, .. }
            | Expr::Ident { span, .. }
            | Expr::QualifiedIdent { span, .. }
            | Expr::Record { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Call { span, .. }
            | Expr::MethodCall { span, .. }
            | Expr::If { span, .. }
            | Expr::Match { span, .. }
            | Expr::Block { span, .. }
            | Expr::Try { span, .. }
            | Expr::TryElse { span, .. }
            | Expr::ListLit { span, .. }
            | Expr::Hole { span, .. } => span.clone(),
        }
    }

    fn recover_to_next_item(&mut self) {
        while !self.is_eof() {
            match self.peek().kind {
                TokenKind::Import
                | TokenKind::Type
                | TokenKind::Enum
                | TokenKind::Fn
                | TokenKind::Public
                | TokenKind::Test
                | TokenKind::Property => return,
                _ => {
                    self.advance();
                }
            }
        }
    }
}
