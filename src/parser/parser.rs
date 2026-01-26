//! Recursive descent parser for Astra

use crate::diagnostics::{Diagnostic, DiagnosticBag, Span};
use crate::parser::ast::*;
use crate::parser::lexer::{Lexer, Token, TokenKind};
use crate::parser::span::SourceFile;

/// Parser for Astra source code
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    source: SourceFile,
    errors: DiagnosticBag,
}

impl<'a> Parser<'a> {
    /// Create a new parser
    pub fn new(lexer: Lexer<'a>, source: SourceFile) -> Self {
        Self {
            lexer,
            source,
            errors: DiagnosticBag::new(),
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

    /// Parse a module path (e.g., `foo.bar.baz`)
    fn parse_module_path(&mut self) -> Result<ModulePath, Diagnostic> {
        let start_span = self.current_span();
        let mut segments = vec![self.expect_ident()?];

        while self.check(TokenKind::Dot) {
            self.advance()?;
            segments.push(self.expect_ident()?);
        }

        let end_span = self.current_span();
        Ok(ModulePath {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            segments,
        })
    }

    /// Parse a top-level item
    fn parse_item(&mut self) -> Result<Item, Diagnostic> {
        let token = self.peek()?;
        match &token.kind {
            TokenKind::Import => self.parse_import().map(Item::Import),
            TokenKind::Type => self.parse_type_def().map(Item::TypeDef),
            TokenKind::Enum => self.parse_enum_def().map(Item::EnumDef),
            TokenKind::Fn | TokenKind::Public => self.parse_fn_def().map(Item::FnDef),
            TokenKind::Test => self.parse_test().map(Item::Test),
            TokenKind::Property => self.parse_property().map(Item::Property),
            _ => Err(Diagnostic::error(crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN)
                .message(format!("Expected item, found {:?}", token.kind))
                .span(token.span.clone())
                .build()),
        }
    }

    /// Parse an import declaration
    fn parse_import(&mut self) -> Result<ImportDecl, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Import)?;

        let path = self.parse_module_path()?;

        let kind = if self.check(TokenKind::As) {
            self.advance()?;
            ImportKind::Alias(self.expect_ident()?)
        } else if self.check(TokenKind::Dot) {
            self.advance()?;
            self.expect(TokenKind::LBrace)?;
            let mut items = vec![self.expect_ident()?];
            while self.check(TokenKind::Comma) {
                self.advance()?;
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

    /// Parse a type definition
    fn parse_type_def(&mut self) -> Result<TypeDef, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Type)?;

        let name = self.expect_ident()?;
        let type_params = self.parse_optional_type_params()?;

        self.expect(TokenKind::Eq)?;
        let value = self.parse_type_expr()?;

        let invariant = if self.check_ident("invariant") {
            self.advance()?;
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

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

    /// Parse an enum definition
    fn parse_enum_def(&mut self) -> Result<EnumDef, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Enum)?;

        let name = self.expect_ident()?;
        let type_params = self.parse_optional_type_params()?;

        self.expect(TokenKind::Eq)?;

        // Optional leading pipe
        if self.check(TokenKind::Pipe) {
            self.advance()?;
        }

        let mut variants = vec![self.parse_variant()?];
        while self.check(TokenKind::Pipe) {
            self.advance()?;
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

    /// Parse an enum variant
    fn parse_variant(&mut self) -> Result<Variant, Diagnostic> {
        let start_span = self.current_span();
        let name = self.expect_ident()?;

        let fields = if self.check(TokenKind::LParen) {
            self.advance()?;
            let mut fields = Vec::new();
            if !self.check(TokenKind::RParen) {
                fields.push(self.parse_field()?);
                while self.check(TokenKind::Comma) {
                    self.advance()?;
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

    /// Parse a field
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

    /// Parse a function definition
    fn parse_fn_def(&mut self) -> Result<FnDef, Diagnostic> {
        let start_span = self.current_span();

        let visibility = if self.check(TokenKind::Public) {
            self.advance()?;
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
            self.advance()?;
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let effects = if self.check_ident("effects") {
            self.advance()?;
            self.expect(TokenKind::LParen)?;
            let mut effects = vec![self.expect_ident()?];
            while self.check(TokenKind::Comma) {
                self.advance()?;
                effects.push(self.expect_ident()?);
            }
            self.expect(TokenKind::RParen)?;
            effects
        } else {
            Vec::new()
        };

        let mut requires = Vec::new();
        let mut ensures = Vec::new();

        while self.check_ident("requires") || self.check_ident("ensures") {
            let token = self.advance()?;
            if let TokenKind::Ident(name) = &token.kind {
                let expr = self.parse_expr()?;
                if name == "requires" {
                    requires.push(expr);
                } else {
                    ensures.push(expr);
                }
            }
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

    /// Parse function parameters
    fn parse_params(&mut self) -> Result<Vec<Param>, Diagnostic> {
        let mut params = Vec::new();
        if !self.check(TokenKind::RParen) {
            params.push(self.parse_param()?);
            while self.check(TokenKind::Comma) {
                self.advance()?;
                if self.check(TokenKind::RParen) {
                    break;
                }
                params.push(self.parse_param()?);
            }
        }
        Ok(params)
    }

    /// Parse a single parameter
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

    /// Parse a test block
    fn parse_test(&mut self) -> Result<TestBlock, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Test)?;

        let name = self.expect_text()?;
        let using = self.parse_optional_using()?;
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

    /// Parse a property block
    fn parse_property(&mut self) -> Result<PropertyBlock, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Property)?;

        let name = self.expect_text()?;
        let using = self.parse_optional_using()?;
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

    /// Parse an optional using clause
    fn parse_optional_using(&mut self) -> Result<Option<UsingClause>, Diagnostic> {
        if !self.check(TokenKind::Using) {
            return Ok(None);
        }

        let start_span = self.current_span();
        self.advance()?;
        self.expect_ident_matching("effects")?;
        self.expect(TokenKind::LParen)?;

        let mut bindings = vec![self.parse_effect_binding()?];
        while self.check(TokenKind::Comma) {
            self.advance()?;
            if self.check(TokenKind::RParen) {
                break;
            }
            bindings.push(self.parse_effect_binding()?);
        }

        self.expect(TokenKind::RParen)?;

        let end_span = self.current_span();
        Ok(Some(UsingClause {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            bindings,
        }))
    }

    /// Parse an effect binding
    fn parse_effect_binding(&mut self) -> Result<EffectBinding, Diagnostic> {
        let start_span = self.current_span();
        let effect = self.expect_ident()?;
        self.expect(TokenKind::Eq)?;
        let value = Box::new(self.parse_expr()?);

        let end_span = self.current_span();
        Ok(EffectBinding {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            effect,
            value,
        })
    }

    /// Parse a type expression
    fn parse_type_expr(&mut self) -> Result<TypeExpr, Diagnostic> {
        let start_span = self.current_span();

        if self.check(TokenKind::LBrace) {
            // Record type
            self.advance()?;
            let mut fields = Vec::new();
            if !self.check(TokenKind::RBrace) {
                fields.push(self.parse_field()?);
                while self.check(TokenKind::Comma) {
                    self.advance()?;
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
        } else if self.check(TokenKind::LParen) {
            // Function type
            self.advance()?;
            let mut params = Vec::new();
            if !self.check(TokenKind::RParen) {
                params.push(self.parse_type_expr()?);
                while self.check(TokenKind::Comma) {
                    self.advance()?;
                    params.push(self.parse_type_expr()?);
                }
            }
            self.expect(TokenKind::RParen)?;
            self.expect(TokenKind::Arrow)?;
            let ret = Box::new(self.parse_type_expr()?);

            let effects = if self.check_ident("effects") {
                self.advance()?;
                self.expect(TokenKind::LParen)?;
                let mut effects = vec![self.expect_ident()?];
                while self.check(TokenKind::Comma) {
                    self.advance()?;
                    effects.push(self.expect_ident()?);
                }
                self.expect(TokenKind::RParen)?;
                effects
            } else {
                Vec::new()
            };

            let end_span = self.current_span();
            Ok(TypeExpr::Function {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                params,
                ret,
                effects,
            })
        } else {
            // Named type
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

    /// Parse optional type parameters
    fn parse_optional_type_params(&mut self) -> Result<Vec<String>, Diagnostic> {
        if !self.check(TokenKind::LBracket) {
            return Ok(Vec::new());
        }

        self.advance()?;
        let mut params = vec![self.expect_ident()?];
        while self.check(TokenKind::Comma) {
            self.advance()?;
            params.push(self.expect_ident()?);
        }
        self.expect(TokenKind::RBracket)?;

        Ok(params)
    }

    /// Parse optional type arguments
    fn parse_optional_type_args(&mut self) -> Result<Vec<TypeExpr>, Diagnostic> {
        if !self.check(TokenKind::LBracket) {
            return Ok(Vec::new());
        }

        self.advance()?;
        let mut args = vec![self.parse_type_expr()?];
        while self.check(TokenKind::Comma) {
            self.advance()?;
            args.push(self.parse_type_expr()?);
        }
        self.expect(TokenKind::RBracket)?;

        Ok(args)
    }

    /// Parse a block
    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::LBrace)?;

        let mut stmts = Vec::new();
        let mut expr = None;

        while !self.check(TokenKind::RBrace) && !self.is_eof() {
            // Try to parse a statement
            if let Some(stmt) = self.try_parse_stmt()? {
                stmts.push(stmt);
            } else {
                // Must be a trailing expression
                expr = Some(Box::new(self.parse_expr()?));
                break;
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

    /// Try to parse a statement (returns None if it looks like an expression)
    fn try_parse_stmt(&mut self) -> Result<Option<Stmt>, Diagnostic> {
        let start_span = self.current_span();

        if self.check(TokenKind::Let) {
            self.advance()?;
            let mutable = self.check(TokenKind::Mut);
            if mutable {
                self.advance()?;
            }

            let name = self.expect_ident()?;

            let ty = if self.check(TokenKind::Colon) {
                self.advance()?;
                Some(self.parse_type_expr()?)
            } else {
                None
            };

            self.expect(TokenKind::Eq)?;
            let value = Box::new(self.parse_expr()?);

            let end_span = self.current_span();
            return Ok(Some(Stmt::Let {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                name,
                mutable,
                ty,
                value,
            }));
        }

        if self.check(TokenKind::Return) {
            self.advance()?;
            let value = if self.check(TokenKind::RBrace) {
                None
            } else {
                Some(Box::new(self.parse_expr()?))
            };

            let end_span = self.current_span();
            return Ok(Some(Stmt::Return {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                value,
            }));
        }

        // Could be an expression statement or a trailing expression
        // We need to look ahead to determine
        Ok(None)
    }

    /// Parse an expression
    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_or_expr()
    }

    /// Parse an or expression
    fn parse_or_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_and_expr()?;

        while self.check(TokenKind::Or) {
            let start_span = self.get_span(&left);
            self.advance()?;
            let right = self.parse_and_expr()?;
            let end_span = self.get_span(&right);

            left = Expr::Binary {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse an and expression
    fn parse_and_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_comparison_expr()?;

        while self.check(TokenKind::And) {
            let start_span = self.get_span(&left);
            self.advance()?;
            let right = self.parse_comparison_expr()?;
            let end_span = self.get_span(&right);

            left = Expr::Binary {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse a comparison expression
    fn parse_comparison_expr(&mut self) -> Result<Expr, Diagnostic> {
        let left = self.parse_additive_expr()?;

        let op = match self.peek()?.kind {
            TokenKind::EqEq => Some(BinaryOp::Eq),
            TokenKind::BangEq => Some(BinaryOp::Ne),
            TokenKind::Lt => Some(BinaryOp::Lt),
            TokenKind::LtEq => Some(BinaryOp::Le),
            TokenKind::Gt => Some(BinaryOp::Gt),
            TokenKind::GtEq => Some(BinaryOp::Ge),
            _ => None,
        };

        if let Some(op) = op {
            let start_span = self.get_span(&left);
            self.advance()?;
            let right = self.parse_additive_expr()?;
            let end_span = self.get_span(&right);

            Ok(Expr::Binary {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                op,
                left: Box::new(left),
                right: Box::new(right),
            })
        } else {
            Ok(left)
        }
    }

    /// Parse an additive expression
    fn parse_additive_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_multiplicative_expr()?;

        loop {
            let op = match self.peek()?.kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };

            let start_span = self.get_span(&left);
            self.advance()?;
            let right = self.parse_multiplicative_expr()?;
            let end_span = self.get_span(&right);

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

    /// Parse a multiplicative expression
    fn parse_multiplicative_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_unary_expr()?;

        loop {
            let op = match self.peek()?.kind {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => break,
            };

            let start_span = self.get_span(&left);
            self.advance()?;
            let right = self.parse_unary_expr()?;
            let end_span = self.get_span(&right);

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

    /// Parse a unary expression
    fn parse_unary_expr(&mut self) -> Result<Expr, Diagnostic> {
        if self.check(TokenKind::Not) {
            let start_span = self.current_span();
            self.advance()?;
            let expr = self.parse_unary_expr()?;
            let end_span = self.get_span(&expr);

            return Ok(Expr::Unary {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                op: UnaryOp::Not,
                expr: Box::new(expr),
            });
        }

        if self.check(TokenKind::Minus) {
            let start_span = self.current_span();
            self.advance()?;
            let expr = self.parse_unary_expr()?;
            let end_span = self.get_span(&expr);

            return Ok(Expr::Unary {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                op: UnaryOp::Neg,
                expr: Box::new(expr),
            });
        }

        self.parse_postfix_expr()
    }

    /// Parse a postfix expression
    fn parse_postfix_expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary_expr()?;

        loop {
            if self.check(TokenKind::LParen) {
                // Function call
                let start_span = self.get_span(&expr);
                self.advance()?;

                let mut args = Vec::new();
                if !self.check(TokenKind::RParen) {
                    args.push(self.parse_expr()?);
                    while self.check(TokenKind::Comma) {
                        self.advance()?;
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
                // Field access or method call
                let start_span = self.get_span(&expr);
                self.advance()?;
                let name = self.expect_ident()?;

                if self.check(TokenKind::LParen) {
                    // Method call
                    self.advance()?;
                    let mut args = Vec::new();
                    if !self.check(TokenKind::RParen) {
                        args.push(self.parse_expr()?);
                        while self.check(TokenKind::Comma) {
                            self.advance()?;
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
                    // Field access
                    let end_span = self.current_span();
                    expr = Expr::FieldAccess {
                        id: NodeId::new(),
                        span: start_span.merge(&end_span),
                        expr: Box::new(expr),
                        field: name,
                    };
                }
            } else if self.check(TokenKind::Question) {
                // Try operator
                let start_span = self.get_span(&expr);
                self.advance()?;
                let end_span = self.current_span();

                expr = Expr::Try {
                    id: NodeId::new(),
                    span: start_span.merge(&end_span),
                    expr: Box::new(expr),
                };
            } else if self.check(TokenKind::QuestionElse) {
                // Try-else operator
                let start_span = self.get_span(&expr);
                self.advance()?;
                let else_expr = self.parse_expr()?;
                let end_span = self.get_span(&else_expr);

                expr = Expr::TryElse {
                    id: NodeId::new(),
                    span: start_span.merge(&end_span),
                    expr: Box::new(expr),
                    else_expr: Box::new(else_expr),
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    /// Parse a primary expression
    fn parse_primary_expr(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek()?.clone();

        match &token.kind {
            TokenKind::IntLit(n) => {
                let value = *n;
                self.advance()?;
                Ok(Expr::IntLit {
                    id: NodeId::new(),
                    span: token.span,
                    value,
                })
            }
            TokenKind::True => {
                self.advance()?;
                Ok(Expr::BoolLit {
                    id: NodeId::new(),
                    span: token.span,
                    value: true,
                })
            }
            TokenKind::False => {
                self.advance()?;
                Ok(Expr::BoolLit {
                    id: NodeId::new(),
                    span: token.span,
                    value: false,
                })
            }
            TokenKind::TextLit(s) => {
                let value = s.clone();
                self.advance()?;
                Ok(Expr::TextLit {
                    id: NodeId::new(),
                    span: token.span,
                    value,
                })
            }
            TokenKind::Ident(_) => self.parse_ident_or_record(),
            TokenKind::LBrace => self.parse_record_expr(),
            TokenKind::LParen => {
                self.advance()?;
                if self.check(TokenKind::RParen) {
                    self.advance()?;
                    return Ok(Expr::UnitLit {
                        id: NodeId::new(),
                        span: token.span,
                    });
                }
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::Hole => {
                self.advance()?;
                Ok(Expr::Hole {
                    id: NodeId::new(),
                    span: token.span,
                })
            }
            _ => Err(Diagnostic::error(crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN)
                .message(format!("Expected expression, found {:?}", token.kind))
                .span(token.span)
                .build()),
        }
    }

    /// Parse an identifier, qualified identifier, or constructor
    fn parse_ident_or_record(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        let name = self.expect_ident()?;

        if self.check(TokenKind::Dot) {
            self.advance()?;
            let field = self.expect_ident()?;
            let end_span = self.current_span();

            return Ok(Expr::QualifiedIdent {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                module: name,
                name: field,
            });
        }

        Ok(Expr::Ident {
            id: NodeId::new(),
            span: start_span,
            name,
        })
    }

    /// Parse a record expression
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
                self.advance()?;
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

    /// Parse an if expression
    fn parse_if_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::If)?;

        let cond = Box::new(self.parse_expr()?);
        let then_branch = Box::new(self.parse_block()?);

        let else_branch = if self.check(TokenKind::Else) {
            self.advance()?;
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

    /// Parse a match expression
    fn parse_match_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Match)?;

        let expr = Box::new(self.parse_expr()?);
        self.expect(TokenKind::LBrace)?;

        let mut arms = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_eof() {
            arms.push(self.parse_match_arm()?);
            // Optional comma between arms
            if self.check(TokenKind::Comma) {
                self.advance()?;
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

    /// Parse a match arm
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

    /// Parse a pattern
    fn parse_pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let token = self.peek()?.clone();

        match &token.kind {
            TokenKind::Underscore => {
                self.advance()?;
                Ok(Pattern::Wildcard {
                    id: NodeId::new(),
                    span: token.span,
                })
            }
            TokenKind::IntLit(n) => {
                let value = *n;
                self.advance()?;
                Ok(Pattern::IntLit {
                    id: NodeId::new(),
                    span: token.span,
                    value,
                })
            }
            TokenKind::True => {
                self.advance()?;
                Ok(Pattern::BoolLit {
                    id: NodeId::new(),
                    span: token.span,
                    value: true,
                })
            }
            TokenKind::False => {
                self.advance()?;
                Ok(Pattern::BoolLit {
                    id: NodeId::new(),
                    span: token.span,
                    value: false,
                })
            }
            TokenKind::TextLit(s) => {
                let value = s.clone();
                self.advance()?;
                Ok(Pattern::TextLit {
                    id: NodeId::new(),
                    span: token.span,
                    value,
                })
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance()?;

                // Check if it's a variant pattern
                if self.check(TokenKind::LParen) {
                    self.advance()?;
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
                } else if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    // Uppercase = variant without data
                    Ok(Pattern::Variant {
                        id: NodeId::new(),
                        span: token.span,
                        name,
                        data: None,
                    })
                } else {
                    // Lowercase = binding
                    Ok(Pattern::Ident {
                        id: NodeId::new(),
                        span: token.span,
                        name,
                    })
                }
            }
            TokenKind::LBrace => {
                self.advance()?;
                let mut fields = Vec::new();

                if !self.check(TokenKind::RBrace) {
                    let name = self.expect_ident()?;
                    let pattern = if self.check(TokenKind::Eq) {
                        self.advance()?;
                        self.parse_pattern()?
                    } else {
                        Pattern::Ident {
                            id: NodeId::new(),
                            span: self.current_span(),
                            name: name.clone(),
                        }
                    };
                    fields.push((name, pattern));

                    while self.check(TokenKind::Comma) {
                        self.advance()?;
                        if self.check(TokenKind::RBrace) {
                            break;
                        }
                        let name = self.expect_ident()?;
                        let pattern = if self.check(TokenKind::Eq) {
                            self.advance()?;
                            self.parse_pattern()?
                        } else {
                            Pattern::Ident {
                                id: NodeId::new(),
                                span: self.current_span(),
                                name: name.clone(),
                            }
                        };
                        fields.push((name, pattern));
                    }
                }

                self.expect(TokenKind::RBrace)?;
                let end_span = self.current_span();

                Ok(Pattern::Record {
                    id: NodeId::new(),
                    span: token.span.merge(&end_span),
                    fields,
                })
            }
            _ => Err(Diagnostic::error(crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN)
                .message(format!("Expected pattern, found {:?}", token.kind))
                .span(token.span)
                .build()),
        }
    }

    // Helper methods

    fn peek(&mut self) -> Result<&Token, Diagnostic> {
        self.lexer.peek()
    }

    fn advance(&mut self) -> Result<Token, Diagnostic> {
        self.lexer.next_token()
    }

    fn is_eof(&mut self) -> bool {
        self.lexer.is_eof()
    }

    fn current_span(&mut self) -> Span {
        self.peek()
            .map(|t| t.span.clone())
            .unwrap_or_else(|_| self.source.span(0, 0))
    }

    fn check(&mut self, kind: TokenKind) -> bool {
        self.peek()
            .map(|t| std::mem::discriminant(&t.kind) == std::mem::discriminant(&kind))
            .unwrap_or(false)
    }

    fn check_ident(&mut self, name: &str) -> bool {
        self.peek()
            .map(|t| matches!(&t.kind, TokenKind::Ident(n) if n == name))
            .unwrap_or(false)
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, Diagnostic> {
        let token = self.advance()?;
        if std::mem::discriminant(&token.kind) == std::mem::discriminant(&kind) {
            Ok(token)
        } else {
            Err(Diagnostic::error(crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN)
                .message(format!("Expected {:?}, found {:?}", kind, token.kind))
                .span(token.span)
                .build())
        }
    }

    fn expect_ident(&mut self) -> Result<String, Diagnostic> {
        let token = self.advance()?;
        match token.kind {
            TokenKind::Ident(name) => Ok(name),
            _ => Err(Diagnostic::error(crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN)
                .message(format!("Expected identifier, found {:?}", token.kind))
                .span(token.span)
                .build()),
        }
    }

    fn expect_ident_matching(&mut self, expected: &str) -> Result<(), Diagnostic> {
        let token = self.advance()?;
        match &token.kind {
            TokenKind::Ident(name) if name == expected => Ok(()),
            _ => Err(Diagnostic::error(crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN)
                .message(format!("Expected '{}', found {:?}", expected, token.kind))
                .span(token.span)
                .build()),
        }
    }

    fn expect_text(&mut self) -> Result<String, Diagnostic> {
        let token = self.advance()?;
        match token.kind {
            TokenKind::TextLit(s) => Ok(s),
            _ => Err(Diagnostic::error(crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN)
                .message(format!("Expected string literal, found {:?}", token.kind))
                .span(token.span)
                .build()),
        }
    }

    fn get_span(&self, expr: &Expr) -> Span {
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
            | Expr::Hole { span, .. } => span.clone(),
        }
    }

    fn recover_to_next_item(&mut self) {
        while !self.is_eof() {
            let token = self.peek();
            if let Ok(token) = token {
                match token.kind {
                    TokenKind::Import
                    | TokenKind::Type
                    | TokenKind::Enum
                    | TokenKind::Fn
                    | TokenKind::Public
                    | TokenKind::Test
                    | TokenKind::Property => return,
                    _ => {
                        let _ = self.advance();
                    }
                }
            } else {
                return;
            }
        }
    }
}
