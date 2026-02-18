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
    peeked2: Option<Token>,
}

impl<'a> Parser<'a> {
    /// Create a new parser
    pub fn new(lexer: Lexer<'a>, source: SourceFile) -> Self {
        Self {
            lexer,
            source,
            errors: DiagnosticBag::new(),
            peeked: None,
            peeked2: None,
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
            // Look ahead: only consume the dot if followed by an identifier
            // (not a `{` for destructuring imports like `import foo.{a, b}`)
            let next = self.peek2();
            if matches!(next.kind, TokenKind::Ident(_)) {
                self.advance(); // consume the dot
                segments.push(self.expect_ident()?);
            } else {
                break;
            }
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
            TokenKind::Import => self.parse_import_item().map(Item::Import),
            TokenKind::Type => self.parse_type_def().map(Item::TypeDef),
            TokenKind::Enum => self.parse_enum_def().map(Item::EnumDef),
            TokenKind::Public => {
                // public can precede fn or import
                // Parser already has Public in peeked, so lexer.peek() is the next token
                let ahead = self.lexer.peek();
                match ahead.kind {
                    TokenKind::Import => self.parse_import_item().map(Item::Import),
                    _ => self.parse_fn_def().map(Item::FnDef),
                }
            }
            TokenKind::Fn => self.parse_fn_def().map(Item::FnDef),
            TokenKind::Trait => self.parse_trait_def().map(Item::TraitDef),
            TokenKind::Impl => self.parse_impl_block().map(Item::ImplBlock),
            TokenKind::Effect => self.parse_effect_def().map(Item::EffectDef),
            TokenKind::Test => self.parse_test().map(Item::Test),
            TokenKind::Property => self.parse_property().map(Item::Property),
            _ => Err(self.error_unexpected("item")),
        }
    }

    fn parse_import_item(&mut self) -> Result<ImportDecl, Diagnostic> {
        let start_span = self.current_span();

        // P4.3: Check for public import (re-export)
        let public = if self.check(TokenKind::Public) {
            self.advance();
            true
        } else {
            false
        };

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
            public,
        })
    }

    fn parse_type_def(&mut self) -> Result<TypeDef, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Type)?;
        let name = self.expect_ident()?;
        let (type_params, _bounds) = self.parse_optional_type_params()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_type_expr()?;

        // Parse optional invariant clause
        let invariant = if self.check(TokenKind::Invariant) {
            self.advance();
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

    fn parse_enum_def(&mut self) -> Result<EnumDef, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Enum)?;
        let name = self.expect_ident()?;
        let (type_params, _bounds) = self.parse_optional_type_params()?;
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

    fn parse_trait_def(&mut self) -> Result<TraitDef, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Trait)?;
        let name = self.expect_ident()?;
        let (type_params, _bounds) = self.parse_optional_type_params()?;
        self.expect(TokenKind::LBrace)?;
        let methods = self.parse_fn_signatures()?;
        self.expect(TokenKind::RBrace)?;
        let end_span = self.current_span();

        Ok(TraitDef {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            type_params,
            methods,
        })
    }

    fn parse_impl_block(&mut self) -> Result<ImplBlock, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Impl)?;
        let trait_name = self.expect_ident()?;

        // Expect "for"
        self.expect(TokenKind::For)?;
        let target_type = self.parse_type_expr()?;
        self.expect(TokenKind::LBrace)?;

        let mut methods = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_eof() {
            methods.push(self.parse_fn_def()?);
        }
        self.expect(TokenKind::RBrace)?;
        let end_span = self.current_span();

        Ok(ImplBlock {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            trait_name,
            target_type,
            methods,
        })
    }

    fn parse_effect_def(&mut self) -> Result<EffectDecl, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Effect)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let operations = self.parse_fn_signatures()?;
        self.expect(TokenKind::RBrace)?;
        let end_span = self.current_span();

        Ok(EffectDecl {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            operations,
        })
    }

    /// Parse a block of function signatures (used by trait and effect definitions).
    /// Parses `fn name(params) -> RetType` entries until `}`.
    fn parse_fn_signatures(&mut self) -> Result<Vec<TraitMethod>, Diagnostic> {
        let mut methods = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_eof() {
            self.expect(TokenKind::Fn)?;
            let method_start = self.current_span();
            let method_name = self.expect_ident()?;
            self.expect(TokenKind::LParen)?;
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
            self.expect(TokenKind::RParen)?;

            let return_type = if self.check(TokenKind::Arrow) {
                self.advance();
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            let method_end = self.current_span();

            methods.push(TraitMethod {
                id: NodeId::new(),
                span: method_start.merge(&method_end),
                name: method_name,
                params,
                return_type,
            });
        }
        Ok(methods)
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
        let (type_params, type_param_bounds) = self.parse_optional_type_params()?;
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
            type_params,
            type_param_bounds,
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

        // Check for destructuring patterns: `{x, y}: {x: Int, y: Int}` or `(a, b): (Int, Text)`
        if self.check(TokenKind::LBrace) || self.check(TokenKind::LParen) {
            let pattern = self.parse_pattern()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type_expr()?;
            let end_span = self.current_span();
            // Use a generated name for the parameter
            let name = format!("__destructured_{}", NodeId::new().0);
            return Ok(Param {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                name,
                ty,
                pattern: Some(pattern),
            });
        }

        let name = self.expect_ident()?;

        // Check for `self` parameter (no type annotation)
        if name == "self" && !self.check(TokenKind::Colon) {
            let end_span = self.current_span();
            return Ok(Param {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                name,
                ty: TypeExpr::Named {
                    id: NodeId::new(),
                    span: start_span.merge(&end_span),
                    name: "Self".to_string(),
                    args: vec![],
                },
                pattern: None,
            });
        }

        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type_expr()?;

        let end_span = self.current_span();
        Ok(Param {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            ty,
            pattern: None,
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
        } else if self.check(TokenKind::LParen) {
            // Could be:
            //   () -> T           function type with no params
            //   ()                unit type
            //   (T1, T2) -> T    function type
            //   (T1, T2)         tuple type
            //   (T)              parenthesized type
            self.advance();

            let mut types = Vec::new();
            if !self.check(TokenKind::RParen) {
                types.push(self.parse_type_expr()?);
                while self.check(TokenKind::Comma) {
                    self.advance();
                    if self.check(TokenKind::RParen) {
                        break;
                    }
                    types.push(self.parse_type_expr()?);
                }
            }
            self.expect(TokenKind::RParen)?;

            if self.check(TokenKind::Arrow) {
                // Function type: (params) -> RetType effects(...)
                self.advance();
                let ret = Box::new(self.parse_type_expr()?);

                // Optional effects
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

                let end_span = self.current_span();
                Ok(TypeExpr::Function {
                    id: NodeId::new(),
                    span: start_span.merge(&end_span),
                    params: types,
                    ret,
                    effects,
                })
            } else if types.is_empty() {
                // () = Unit type
                let end_span = self.current_span();
                Ok(TypeExpr::Named {
                    id: NodeId::new(),
                    span: start_span.merge(&end_span),
                    name: "Unit".to_string(),
                    args: Vec::new(),
                })
            } else if types.len() == 1 {
                // (T) = parenthesized type, just unwrap
                Ok(types.into_iter().next().unwrap())
            } else {
                // (T1, T2, ...) = tuple type
                let end_span = self.current_span();
                Ok(TypeExpr::Tuple {
                    id: NodeId::new(),
                    span: start_span.merge(&end_span),
                    elements: types,
                })
            }
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

    #[allow(clippy::type_complexity)]
    fn parse_optional_type_params(
        &mut self,
    ) -> Result<(Vec<String>, Vec<(String, String)>), Diagnostic> {
        if !self.check(TokenKind::LBracket) {
            return Ok((Vec::new(), Vec::new()));
        }
        self.advance();
        let mut params = Vec::new();
        let mut bounds = Vec::new();
        let name = self.expect_ident()?;
        // P2.4: Parse optional trait bound (T: Bound)
        if self.check(TokenKind::Colon) {
            self.advance();
            let bound = self.expect_ident()?;
            bounds.push((name.clone(), bound));
        }
        params.push(name);
        while self.check(TokenKind::Comma) {
            self.advance();
            let name = self.expect_ident()?;
            if self.check(TokenKind::Colon) {
                self.advance();
                let bound = self.expect_ident()?;
                bounds.push((name.clone(), bound));
            }
            params.push(name);
        }
        self.expect(TokenKind::RBracket)?;
        Ok((params, bounds))
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
        let (stmts, expr) = self.parse_block_stmts()?;
        self.expect(TokenKind::RBrace)?;

        let end_span = self.current_span();
        Ok(Block {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            stmts,
            expr,
        })
    }

    /// Parse the interior statements and optional trailing expression of a block.
    /// Assumes the opening `{` has already been consumed.
    fn parse_block_stmts(&mut self) -> Result<(Vec<Stmt>, Option<Box<Expr>>), Diagnostic> {
        let mut stmts = Vec::new();
        let mut expr = None;

        while !self.check(TokenKind::RBrace) && !self.is_eof() {
            if self.check(TokenKind::Let) || self.check(TokenKind::Return) {
                stmts.push(self.parse_stmt()?);
            } else if self.check(TokenKind::Fn) && matches!(self.peek2().kind, TokenKind::Ident(_))
            {
                // Local named function definition: `fn name(...) { ... }`
                stmts.push(self.parse_local_fn_stmt()?);
            } else {
                // Parse an expression
                let e = self.parse_expr()?;

                // Check for assignment: `expr = value`
                if self.check(TokenKind::Eq) {
                    self.advance();
                    let value = self.parse_expr()?;
                    let span = e.span().merge(value.span());
                    stmts.push(Stmt::Assign {
                        id: NodeId::new(),
                        span,
                        target: Box::new(e),
                        value: Box::new(value),
                    });
                }
                // E1: Compound assignment operators (+=, -=, *=, /=, %=)
                else if let Some(op) = self.check_compound_assign() {
                    self.advance();
                    let rhs = self.parse_expr()?;
                    let span = e.span().merge(rhs.span());
                    // Desugar `x += expr` into `x = x + expr`
                    let desugared = Expr::Binary {
                        id: NodeId::new(),
                        span: span.clone(),
                        op,
                        left: Box::new(e.clone()),
                        right: Box::new(rhs),
                    };
                    stmts.push(Stmt::Assign {
                        id: NodeId::new(),
                        span,
                        target: Box::new(e),
                        value: Box::new(desugared),
                    });
                } else if self.check(TokenKind::RBrace) {
                    // If we're at the end of the block, this is the final expression
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

        Ok((stmts, expr))
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let start_span = self.current_span();

        if self.check(TokenKind::Let) {
            self.advance();

            // Check for destructuring pattern: `let { ... } = expr` or `let (...) = expr`
            if self.check(TokenKind::LBrace) || self.check(TokenKind::LParen) {
                let pattern = self.parse_pattern()?;
                let ty = if self.check(TokenKind::Colon) {
                    self.advance();
                    Some(self.parse_type_expr()?)
                } else {
                    None
                };
                self.expect(TokenKind::Eq)?;
                let value = Box::new(self.parse_expr()?);
                let end_span = self.current_span();
                return Ok(Stmt::LetPattern {
                    id: NodeId::new(),
                    span: start_span.merge(&end_span),
                    pattern,
                    ty,
                    value,
                });
            }

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
            // Range expressions: `..` and `..=` at precedence 1 (between pipe and or)
            if min_prec <= 1 {
                let inclusive = match self.peek().kind {
                    TokenKind::DotDotEq => Some(true),
                    TokenKind::DotDot => Some(false),
                    _ => None,
                };
                if let Some(inclusive) = inclusive {
                    let start_span = self.expr_span(&left);
                    self.advance();
                    let right = self.parse_binary_expr(2)?;
                    let end_span = self.expr_span(&right);
                    left = Expr::Range {
                        id: NodeId::new(),
                        span: start_span.merge(&end_span),
                        start: Box::new(left),
                        end: Box::new(right),
                        inclusive,
                    };
                    continue;
                }
            }

            let (op, prec) = match self.peek().kind {
                TokenKind::PipeArrow => (BinaryOp::Pipe, 0),
                TokenKind::Or => (BinaryOp::Or, 2),
                TokenKind::And => (BinaryOp::And, 3),
                TokenKind::EqEq => (BinaryOp::Eq, 4),
                TokenKind::BangEq => (BinaryOp::Ne, 4),
                TokenKind::Lt => (BinaryOp::Lt, 5),
                TokenKind::LtEq => (BinaryOp::Le, 5),
                TokenKind::Gt => (BinaryOp::Gt, 5),
                TokenKind::GtEq => (BinaryOp::Ge, 5),
                TokenKind::Plus => (BinaryOp::Add, 6),
                TokenKind::Minus => (BinaryOp::Sub, 6),
                TokenKind::Star => (BinaryOp::Mul, 7),
                TokenKind::Slash => (BinaryOp::Div, 7),
                TokenKind::Percent => (BinaryOp::Mod, 7),
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
        // B2: async/await are reserved keywords — not implemented in v1.0
        if self.check(TokenKind::Await) {
            let span = self.current_span();
            return Err(Diagnostic::error("E0006")
                .message("`await` is a reserved keyword. Astra v1.0 is single-threaded and does not support async/await. Remove the `await` keyword — the expression will be evaluated synchronously.")
                .span(span)
                .build());
        }
        if self.check(TokenKind::Async) {
            let span = self.current_span();
            return Err(Diagnostic::error("E0006")
                .message("`async` is a reserved keyword. Astra v1.0 is single-threaded and does not support async/await.")
                .span(span)
                .build());
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
                // Allow both identifiers and int literals (for tuple indexing: t.0, t.1)
                let name = if let TokenKind::IntLit(n) = &self.peek().kind {
                    let s = n.to_string();
                    self.advance();
                    s
                } else {
                    self.expect_ident()?
                };

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
            }
            // E2: Index access: expr[index]
            else if self.check(TokenKind::LBracket) {
                let start_span = self.expr_span(&expr);
                self.advance();
                let index = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;
                let end_span = self.current_span();
                expr = Expr::IndexAccess {
                    id: NodeId::new(),
                    span: start_span.merge(&end_span),
                    expr: Box::new(expr),
                    index: Box::new(index),
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
            TokenKind::FloatLit(n) => {
                let value = *n;
                self.advance();
                Ok(Expr::FloatLit {
                    id: NodeId::new(),
                    span: token.span,
                    value,
                })
            }
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
                // Check for string interpolation: "${...}"
                if value.contains("${") {
                    self.parse_string_interp(&value, &token.span)
                } else {
                    match unescape_string(&value) {
                        Ok(unescaped) => Ok(Expr::TextLit {
                            id: NodeId::new(),
                            span: token.span,
                            value: unescaped,
                        }),
                        Err((msg, _)) => Err(Diagnostic::error(
                            crate::diagnostics::error_codes::syntax::INVALID_ESCAPE,
                        )
                        .message(msg)
                        .span(token.span)
                        .build()),
                    }
                }
            }
            TokenKind::MultilineTextLit(s) => {
                let raw = s.clone();
                self.advance();
                let dedented = dedent_multiline_string(&raw);
                // Check for string interpolation: "${...}"
                if dedented.contains("${") {
                    self.parse_string_interp(&dedented, &token.span)
                } else {
                    match unescape_string(&dedented) {
                        Ok(unescaped) => Ok(Expr::TextLit {
                            id: NodeId::new(),
                            span: token.span,
                            value: unescaped,
                        }),
                        Err((msg, _)) => Err(Diagnostic::error(
                            crate::diagnostics::error_codes::syntax::INVALID_ESCAPE,
                        )
                        .message(msg)
                        .span(token.span)
                        .build()),
                    }
                }
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
            TokenKind::LBrace => self.parse_brace_expr(),
            TokenKind::LParen => {
                self.advance();
                if self.check(TokenKind::RParen) {
                    self.advance();
                    return Ok(Expr::UnitLit {
                        id: NodeId::new(),
                        span: token.span,
                    });
                }
                let first = self.parse_expr()?;
                // Check for tuple: (a, b, ...)
                if self.check(TokenKind::Comma) {
                    let mut elements = vec![first];
                    while self.check(TokenKind::Comma) {
                        self.advance();
                        if self.check(TokenKind::RParen) {
                            break;
                        }
                        elements.push(self.parse_expr()?);
                    }
                    self.expect(TokenKind::RParen)?;
                    let end_span = self.current_span();
                    return Ok(Expr::TupleLit {
                        id: NodeId::new(),
                        span: token.span.merge(&end_span),
                        elements,
                    });
                }
                self.expect(TokenKind::RParen)?;
                Ok(first)
            }
            TokenKind::LBracket => self.parse_list_expr(),
            TokenKind::Fn => self.parse_lambda_expr(),
            TokenKind::For => self.parse_for_expr(),
            TokenKind::While => self.parse_while_expr(),
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::Assert => self.parse_assert_expr(),
            TokenKind::Break => {
                self.advance();
                Ok(Expr::Break {
                    id: NodeId::new(),
                    span: token.span,
                })
            }
            TokenKind::Continue => {
                self.advance();
                Ok(Expr::Continue {
                    id: NodeId::new(),
                    span: token.span,
                })
            }
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

    /// Parse a `{...}` expression, disambiguating between record literals and block expressions.
    ///
    /// Record: `{ name = expr, ... }`
    /// Block: `{ stmt; stmt; expr }` (contains let, return, fn, or expressions)
    fn parse_brace_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::LBrace)?;

        // Empty braces: `{}` → empty record
        if self.check(TokenKind::RBrace) {
            self.advance();
            let end_span = self.current_span();
            return Ok(Expr::Record {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                fields: vec![],
            });
        }

        // If next token starts a statement, it's definitely a block
        if matches!(self.peek().kind, TokenKind::Let | TokenKind::Return) {
            return self.parse_block_body(start_span);
        }

        // If next token is `fn` followed by an identifier, it's a local function def → block
        if self.check(TokenKind::Fn) && matches!(self.peek2().kind, TokenKind::Ident(_)) {
            return self.parse_block_body(start_span);
        }

        // If it's a keyword that can only start an expression (not a record field), it's a block
        if matches!(
            self.peek().kind,
            TokenKind::For
                | TokenKind::While
                | TokenKind::If
                | TokenKind::Match
                | TokenKind::Assert
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Fn
                | TokenKind::True
                | TokenKind::False
                | TokenKind::IntLit(_)
                | TokenKind::FloatLit(_)
                | TokenKind::TextLit(_)
                | TokenKind::MultilineTextLit(_)
                | TokenKind::LBracket
                | TokenKind::LParen
                | TokenKind::Minus
                | TokenKind::Not
        ) {
            return self.parse_block_body(start_span);
        }

        // If it's an identifier, we need to check what follows
        if matches!(self.peek().kind, TokenKind::Ident(_)) {
            // Peek at the token after the identifier
            let after_ident = &self.peek2().kind;
            if matches!(after_ident, TokenKind::Eq) {
                // `{ name = expr` → could be record literal OR block with assignment
                // Parse first field, then check for comma to disambiguate
                let ident_token = self.advance();
                let ident_name = match &ident_token.kind {
                    TokenKind::Ident(n) => n.clone(),
                    _ => unreachable!(),
                };
                let ident_span = ident_token.span.clone();
                self.advance(); // consume `=`
                let value = self.parse_expr()?;

                if self.check(TokenKind::Comma) {
                    // Comma found → record literal. Parse remaining fields.
                    let mut fields = vec![(ident_name, Box::new(value))];
                    while self.check(TokenKind::Comma) {
                        self.advance();
                        if self.check(TokenKind::RBrace) {
                            break; // trailing comma
                        }
                        let field_name = self.expect_ident()?;
                        self.expect(TokenKind::Eq)?;
                        let field_value = self.parse_expr()?;
                        fields.push((field_name, Box::new(field_value)));
                    }
                    self.expect(TokenKind::RBrace)?;
                    let end_span = self.current_span();
                    return Ok(Expr::Record {
                        id: NodeId::new(),
                        span: start_span.merge(&end_span),
                        fields,
                    });
                } else {
                    // No comma → block with assignment statement
                    let target = Expr::Ident {
                        id: NodeId::new(),
                        span: ident_span,
                        name: ident_name,
                    };
                    let assign_span = target.span().merge(value.span());
                    let assign_stmt = Stmt::Assign {
                        id: NodeId::new(),
                        span: assign_span,
                        target: Box::new(target),
                        value: Box::new(value),
                    };

                    // Continue parsing remaining block statements
                    let mut stmts = vec![assign_stmt];
                    let mut trailing_expr = None;
                    while !self.check(TokenKind::RBrace) && !self.is_eof() {
                        if self.check(TokenKind::Let) || self.check(TokenKind::Return) {
                            stmts.push(self.parse_stmt()?);
                        } else if self.check(TokenKind::Fn)
                            && matches!(self.peek2().kind, TokenKind::Ident(_))
                        {
                            stmts.push(self.parse_local_fn_stmt()?);
                        } else {
                            let e = self.parse_expr()?;
                            if self.check(TokenKind::Eq) {
                                self.advance();
                                let val = self.parse_expr()?;
                                let span = e.span().merge(val.span());
                                stmts.push(Stmt::Assign {
                                    id: NodeId::new(),
                                    span,
                                    target: Box::new(e),
                                    value: Box::new(val),
                                });
                            } else if let Some(op) = self.check_compound_assign() {
                                self.advance();
                                let rhs = self.parse_expr()?;
                                let span = e.span().merge(rhs.span());
                                let desugared = Expr::Binary {
                                    id: NodeId::new(),
                                    span: span.clone(),
                                    op,
                                    left: Box::new(e.clone()),
                                    right: Box::new(rhs),
                                };
                                stmts.push(Stmt::Assign {
                                    id: NodeId::new(),
                                    span,
                                    target: Box::new(e),
                                    value: Box::new(desugared),
                                });
                            } else if self.check(TokenKind::RBrace) {
                                trailing_expr = Some(Box::new(e));
                                break;
                            } else {
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
                    return Ok(Expr::Block {
                        id: NodeId::new(),
                        span: start_span.merge(&end_span),
                        block: Box::new(Block {
                            id: NodeId::new(),
                            span: start_span.merge(&end_span),
                            stmts,
                            expr: trailing_expr,
                        }),
                    });
                }
            }
            // Otherwise it's a block (e.g., `{ x + 1 }` or `{ x }`)
            return self.parse_block_body(start_span);
        }

        // Default: treat as block expression
        self.parse_block_body(start_span)
    }

    /// Parse the body of a block expression after `{` has already been consumed.
    fn parse_block_body(&mut self, start_span: Span) -> Result<Expr, Diagnostic> {
        let (stmts, expr) = self.parse_block_stmts()?;
        self.expect(TokenKind::RBrace)?;
        let end_span = self.current_span();

        Ok(Expr::Block {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            block: Box::new(Block {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                stmts,
                expr,
            }),
        })
    }

    /// Parse a local named function definition as a let statement.
    /// `fn name(params) -> RetType { body }` becomes `let name = fn(params) -> RetType { body }`
    fn parse_local_fn_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Fn)?;
        let name = self.expect_ident()?;

        // Parse type parameters if present (e.g., `fn id[T](x: T)`)
        let (_type_params, _type_param_bounds) = self.parse_optional_type_params()?;

        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        if !self.check(TokenKind::RParen) {
            params.push(self.parse_lambda_param()?);
            while self.check(TokenKind::Comma) {
                self.advance();
                if self.check(TokenKind::RParen) {
                    break;
                }
                params.push(self.parse_lambda_param()?);
            }
        }
        self.expect(TokenKind::RParen)?;

        let return_type = if self.check(TokenKind::Arrow) {
            self.advance();
            Some(Box::new(self.parse_type_expr()?))
        } else {
            None
        };

        let body = Box::new(self.parse_block()?);
        let end_span = self.current_span();

        Ok(Stmt::Let {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            mutable: false,
            ty: None,
            value: Box::new(Expr::Lambda {
                id: NodeId::new(),
                span: start_span.merge(&end_span),
                params,
                return_type,
                body,
            }),
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

    fn parse_lambda_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::Fn)?;
        self.expect(TokenKind::LParen)?;

        // Parse lambda parameters (name or name: Type)
        let mut params = Vec::new();
        if !self.check(TokenKind::RParen) {
            params.push(self.parse_lambda_param()?);
            while self.check(TokenKind::Comma) {
                self.advance();
                if self.check(TokenKind::RParen) {
                    break;
                }
                params.push(self.parse_lambda_param()?);
            }
        }
        self.expect(TokenKind::RParen)?;

        // Optional return type
        let return_type = if self.check(TokenKind::Arrow) {
            self.advance();
            Some(Box::new(self.parse_type_expr()?))
        } else {
            None
        };

        let body = Box::new(self.parse_block()?);
        let end_span = self.current_span();

        Ok(Expr::Lambda {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            params,
            return_type,
            body,
        })
    }

    fn parse_lambda_param(&mut self) -> Result<LambdaParam, Diagnostic> {
        let start_span = self.current_span();
        let name = self.expect_ident()?;
        let ty = if self.check(TokenKind::Colon) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        let end_span = self.current_span();
        Ok(LambdaParam {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            name,
            ty,
        })
    }

    fn parse_for_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::For)?;

        // E8: Support destructuring patterns in for loops
        // `for x in ...`, `for (a, b) in ...`, `for {x, y} in ...`
        let (binding, pattern) = if self.check(TokenKind::LParen) || self.check(TokenKind::LBrace) {
            let pat = self.parse_pattern()?;
            ("_for_pattern".to_string(), Some(pat))
        } else {
            (self.expect_ident()?, None)
        };

        self.expect(TokenKind::In)?;
        let iter = Box::new(self.parse_expr()?);
        let body = Box::new(self.parse_block()?);
        let end_span = self.current_span();

        Ok(Expr::ForIn {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            binding,
            pattern,
            iter,
            body,
        })
    }

    fn parse_while_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::While)?;
        let cond = Box::new(self.parse_expr()?);

        // P2: Detect `=` in conditions and suggest `==`
        if self.check(TokenKind::Eq) {
            let span = self.current_span();
            return Err(Diagnostic::error("E0001")
                .message("Found `=` (assignment) in condition — did you mean `==` (equality)?")
                .span(span)
                .suggestion(crate::diagnostics::Suggestion::new(
                    "Use `==` for comparison".to_string(),
                ))
                .build());
        }

        let body = Box::new(self.parse_block()?);
        let end_span = self.current_span();

        Ok(Expr::While {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            cond,
            body,
        })
    }

    fn parse_if_expr(&mut self) -> Result<Expr, Diagnostic> {
        let start_span = self.current_span();
        self.expect(TokenKind::If)?;
        let cond = Box::new(self.parse_expr()?);

        // P2: Detect `=` in conditions and suggest `==`
        if self.check(TokenKind::Eq) {
            let span = self.current_span();
            return Err(Diagnostic::error("E0001")
                .message("Found `=` (assignment) in condition — did you mean `==` (equality)?")
                .span(span)
                .suggestion(crate::diagnostics::Suggestion::new(
                    "Use `==` for comparison".to_string(),
                ))
                .build());
        }

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

        // If followed by '(', parse as assert(cond) or assert(cond, msg)
        let args = if self.check(TokenKind::LParen) {
            self.advance();
            let mut args = vec![self.parse_expr()?];
            if self.check(TokenKind::Comma) {
                self.advance();
                args.push(self.parse_expr()?);
            }
            self.expect(TokenKind::RParen)?;
            args
        } else {
            vec![self.parse_expr()?]
        };

        let end_span = self.current_span();

        // Parse assert as a call to the builtin assert function
        Ok(Expr::Call {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            func: Box::new(Expr::Ident {
                id: NodeId::new(),
                span: start_span.clone(),
                name: "assert".to_string(),
            }),
            args,
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

        // Optional guard: `pattern if guard_expr => body`
        let guard = if self.check(TokenKind::If) {
            self.advance();
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        self.expect(TokenKind::FatArrow)?;
        let body = Box::new(self.parse_expr()?);

        let end_span = self.current_span();
        Ok(MatchArm {
            id: NodeId::new(),
            span: start_span.merge(&end_span),
            pattern,
            guard,
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
                    let mut fields = Vec::new();
                    if !self.check(TokenKind::RParen) {
                        fields.push(self.parse_pattern()?);
                        while self.check(TokenKind::Comma) {
                            self.advance();
                            if self.check(TokenKind::RParen) {
                                break;
                            }
                            fields.push(self.parse_pattern()?);
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    let end_span = self.current_span();
                    Ok(Pattern::Variant {
                        id: NodeId::new(),
                        span: token.span.merge(&end_span),
                        name,
                        fields,
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
                        fields: Vec::new(),
                    })
                } else {
                    Ok(Pattern::Ident {
                        id: NodeId::new(),
                        span: token.span,
                        name,
                    })
                }
            }
            TokenKind::LBrace => {
                // Record pattern: { x, y } or { x = pat, y = pat }
                self.advance();
                let mut fields = Vec::new();
                if !self.check(TokenKind::RBrace) {
                    loop {
                        let field_name = self.expect_ident()?;
                        let field_pat = if self.check(TokenKind::Eq) {
                            self.advance();
                            self.parse_pattern()?
                        } else {
                            // Shorthand: `{ x }` means `{ x = x }`
                            Pattern::Ident {
                                id: NodeId::new(),
                                span: self.current_span(),
                                name: field_name.clone(),
                            }
                        };
                        fields.push((field_name, field_pat));
                        if !self.check(TokenKind::Comma) {
                            break;
                        }
                        self.advance();
                        if self.check(TokenKind::RBrace) {
                            break;
                        }
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
            TokenKind::FloatLit(n) => {
                let value = *n;
                self.advance();
                Ok(Pattern::FloatLit {
                    id: NodeId::new(),
                    span: token.span,
                    value,
                })
            }
            TokenKind::LParen => {
                // Tuple pattern: (pat1, pat2, ...)
                self.advance();
                let mut elements = Vec::new();
                if !self.check(TokenKind::RParen) {
                    elements.push(self.parse_pattern()?);
                    while self.check(TokenKind::Comma) {
                        self.advance();
                        if self.check(TokenKind::RParen) {
                            break;
                        }
                        elements.push(self.parse_pattern()?);
                    }
                }
                self.expect(TokenKind::RParen)?;
                let end_span = self.current_span();
                Ok(Pattern::Tuple {
                    id: NodeId::new(),
                    span: token.span.merge(&end_span),
                    elements,
                })
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

    /// Peek at the second upcoming token (two-token lookahead)
    fn peek2(&mut self) -> Token {
        // Ensure first peeked token is buffered
        let _ = self.peek();
        if self.peeked2.is_none() {
            self.peeked2 = Some(self.lexer.next_token());
        }
        self.peeked2.clone().unwrap()
    }

    fn advance(&mut self) -> Token {
        if let Some(token) = self.peeked.take() {
            // Move peeked2 into peeked if it exists
            if self.peeked2.is_some() {
                self.peeked = self.peeked2.take();
            }
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

    /// E1: Check for compound assignment operators (+=, -=, *=, /=, %=).
    /// Returns the corresponding binary operator if found.
    fn check_compound_assign(&mut self) -> Option<BinaryOp> {
        match self.peek().kind {
            TokenKind::PlusEq => Some(BinaryOp::Add),
            TokenKind::MinusEq => Some(BinaryOp::Sub),
            TokenKind::StarEq => Some(BinaryOp::Mul),
            TokenKind::SlashEq => Some(BinaryOp::Div),
            TokenKind::PercentEq => Some(BinaryOp::Mod),
            _ => None,
        }
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

    fn parse_string_interp(&mut self, raw: &str, span: &Span) -> Result<Expr, Diagnostic> {
        let mut parts = Vec::new();
        let mut remaining = raw;

        while let Some(dollar_pos) = remaining.find("${") {
            // Add literal part before ${
            if dollar_pos > 0 {
                match unescape_string(&remaining[..dollar_pos]) {
                    Ok(unescaped) => parts.push(StringPart::Literal(unescaped)),
                    Err((msg, _)) => {
                        return Err(Diagnostic::error(
                            crate::diagnostics::error_codes::syntax::INVALID_ESCAPE,
                        )
                        .message(msg)
                        .span(span.clone())
                        .build());
                    }
                }
            }

            // Find the matching closing brace
            let expr_start = dollar_pos + 2;
            let mut brace_depth = 1;
            let mut end_pos = expr_start;
            let chars: Vec<char> = remaining[expr_start..].chars().collect();
            for ch in &chars {
                match ch {
                    '{' => brace_depth += 1,
                    '}' => {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                end_pos += ch.len_utf8();
            }

            if brace_depth != 0 {
                return Err(Diagnostic::error(
                    crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN,
                )
                .message("Unclosed string interpolation: missing '}'")
                .span(span.clone())
                .build());
            }

            let expr_str = &remaining[expr_start..end_pos];
            // Parse the expression string
            let source_file = crate::parser::span::SourceFile::new(
                std::path::PathBuf::from("<interp>"),
                format!("module __interp\nfn __x() -> Int {{ {} }}", expr_str),
            );
            let lexer = Lexer::new(&source_file);
            let mut parser = Parser::new(lexer, source_file.clone());
            // Parse module, extract the expression from the function body
            match parser.parse_module() {
                Ok(module) => {
                    if let Some(Item::FnDef(f)) = module.items.first() {
                        if let Some(expr) = &f.body.expr {
                            parts.push(StringPart::Expr(expr.clone()));
                        } else if let Some(Stmt::Expr { expr, .. }) = f.body.stmts.first() {
                            parts.push(StringPart::Expr(expr.clone()));
                        }
                    }
                }
                Err(_) => {
                    return Err(Diagnostic::error(
                        crate::diagnostics::error_codes::syntax::UNEXPECTED_TOKEN,
                    )
                    .message(format!(
                        "Invalid expression in string interpolation: {}",
                        expr_str
                    ))
                    .span(span.clone())
                    .build());
                }
            }

            remaining = &remaining[end_pos + 1..]; // skip past '}'
        }

        // Add remaining literal
        if !remaining.is_empty() {
            match unescape_string(remaining) {
                Ok(unescaped) => parts.push(StringPart::Literal(unescaped)),
                Err((msg, _)) => {
                    return Err(Diagnostic::error(
                        crate::diagnostics::error_codes::syntax::INVALID_ESCAPE,
                    )
                    .message(msg)
                    .span(span.clone())
                    .build());
                }
            }
        }

        Ok(Expr::StringInterp {
            id: NodeId::new(),
            span: span.clone(),
            parts,
        })
    }

    fn expr_span(&self, expr: &Expr) -> Span {
        match expr {
            Expr::IntLit { span, .. }
            | Expr::FloatLit { span, .. }
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
            | Expr::TupleLit { span, .. }
            | Expr::MapLit { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::ForIn { span, .. }
            | Expr::While { span, .. }
            | Expr::Break { span, .. }
            | Expr::Continue { span, .. }
            | Expr::StringInterp { span, .. }
            | Expr::Range { span, .. }
            | Expr::IndexAccess { span, .. }
            | Expr::Await { span, .. }
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
                | TokenKind::Effect
                | TokenKind::Public
                | TokenKind::Trait
                | TokenKind::Impl
                | TokenKind::Test
                | TokenKind::Property => return,
                _ => {
                    self.advance();
                }
            }
        }
    }
}

/// Process escape sequences in a string literal.
/// Returns Err with a description if an invalid escape sequence is found.
fn unescape_string(s: &str) -> Result<String, (String, char)> {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('0') => result.push('\0'),
                Some('$') => result.push('$'),
                Some(other) => {
                    return Err((
                        format!(
                            "Invalid escape sequence `\\{}`. Valid escapes: \\n, \\r, \\t, \\\\, \\\", \\0, \\$",
                            other
                        ),
                        other,
                    ));
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

/// Dedent a multiline string by stripping common leading whitespace.
/// Also strips the first line if it's empty (immediately after `"""`) and
/// the last line if it's only whitespace (immediately before closing `"""`).
fn dedent_multiline_string(s: &str) -> String {
    let mut lines: Vec<&str> = s.split('\n').collect();

    // Strip leading empty line (the newline right after opening """)
    if !lines.is_empty() && lines[0].trim().is_empty() {
        lines.remove(0);
    }

    // Strip trailing whitespace-only line (the line before closing """)
    if !lines.is_empty() && lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }

    // Find minimum indentation across non-empty lines
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    // Strip common indentation
    lines
        .iter()
        .map(|l| {
            if l.len() >= min_indent {
                &l[min_indent..]
            } else {
                l.trim_start()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
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
}
