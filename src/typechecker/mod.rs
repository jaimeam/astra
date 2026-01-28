//! Type checker for Astra
//!
//! Implements type checking, inference, and exhaustiveness checking.

use crate::diagnostics::{Diagnostic, DiagnosticBag, Edit, Span, Suggestion};
use crate::parser::ast::*;
use std::collections::HashMap;

/// Built-in types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// Unit type
    Unit,
    /// Integer type
    Int,
    /// Boolean type
    Bool,
    /// Text/string type
    Text,
    /// Function type
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
        effects: Vec<String>,
    },
    /// Record type
    Record(Vec<(String, Type)>),
    /// Named type (user-defined or generic)
    Named(String, Vec<Type>),
    /// Option type
    Option(Box<Type>),
    /// Result type
    Result(Box<Type>, Box<Type>),
    /// Type variable (for inference)
    Var(TypeVarId),
    /// Unknown type (for error recovery)
    Unknown,
}

/// Type variable identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeVarId(pub u32);

/// Type environment for tracking bindings
#[derive(Debug, Clone, Default)]
pub struct TypeEnv {
    /// Variable types
    bindings: HashMap<String, Type>,
    /// Type definitions
    type_defs: HashMap<String, TypeDef>,
    /// Enum definitions
    enum_defs: HashMap<String, EnumDef>,
    /// Function definitions
    fn_defs: HashMap<String, FnDef>,
    /// Parent environment
    parent: Option<Box<TypeEnv>>,
}

impl TypeEnv {
    /// Create a new empty environment
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a child environment
    pub fn child(&self) -> Self {
        Self {
            bindings: HashMap::new(),
            type_defs: HashMap::new(),
            enum_defs: HashMap::new(),
            fn_defs: HashMap::new(),
            parent: Some(Box::new(self.clone())),
        }
    }

    /// Define a variable's type
    pub fn define(&mut self, name: String, ty: Type) {
        self.bindings.insert(name, ty);
    }

    /// Look up a variable's type
    pub fn lookup(&self, name: &str) -> Option<&Type> {
        self.bindings
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.lookup(name)))
    }

    /// Register a type definition
    pub fn register_type(&mut self, def: TypeDef) {
        self.type_defs.insert(def.name.clone(), def);
    }

    /// Register an enum definition
    pub fn register_enum(&mut self, def: EnumDef) {
        self.enum_defs.insert(def.name.clone(), def);
    }

    /// Register a function definition
    pub fn register_fn(&mut self, def: FnDef) {
        self.fn_defs.insert(def.name.clone(), def);
    }

    /// Look up a type definition
    pub fn lookup_type(&self, name: &str) -> Option<&TypeDef> {
        self.type_defs
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.lookup_type(name)))
    }

    /// Look up an enum definition
    pub fn lookup_enum(&self, name: &str) -> Option<&EnumDef> {
        self.enum_defs
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.lookup_enum(name)))
    }

    /// Look up a function definition
    pub fn lookup_fn(&self, name: &str) -> Option<&FnDef> {
        self.fn_defs
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.lookup_fn(name)))
    }
}

/// Type checker
pub struct TypeChecker {
    /// Current environment
    env: TypeEnv,
    /// Diagnostics collected during checking
    diagnostics: DiagnosticBag,
    /// Next type variable ID (used in fresh_type_var)
    #[allow(dead_code)]
    next_var: u32,
}

impl TypeChecker {
    /// Create a new type checker
    pub fn new() -> Self {
        Self {
            env: TypeEnv::new(),
            diagnostics: DiagnosticBag::new(),
            next_var: 0,
        }
    }

    /// Check a module
    pub fn check_module(&mut self, module: &Module) -> Result<(), DiagnosticBag> {
        // First pass: collect all type/enum/fn definitions
        for item in &module.items {
            match item {
                Item::TypeDef(def) => self.env.register_type(def.clone()),
                Item::EnumDef(def) => self.env.register_enum(def.clone()),
                Item::FnDef(def) => self.env.register_fn(def.clone()),
                _ => {}
            }
        }

        // Second pass: type check all items
        for item in &module.items {
            self.check_item(item);
        }

        if self.diagnostics.has_errors() {
            Err(self.diagnostics.clone())
        } else {
            Ok(())
        }
    }

    /// Check a single item
    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Import(_) => {
                // TODO: resolve imports
            }
            Item::TypeDef(def) => self.check_typedef(def),
            Item::EnumDef(def) => self.check_enumdef(def),
            Item::FnDef(def) => self.check_fndef(def),
            Item::Test(test) => self.check_test(test),
            Item::Property(prop) => self.check_property(prop),
        }
    }

    fn check_typedef(&mut self, _def: &TypeDef) {
        // TODO: check type definition is well-formed
    }

    fn check_enumdef(&mut self, _def: &EnumDef) {
        // TODO: check enum definition is well-formed
    }

    fn check_fndef(&mut self, def: &FnDef) {
        let mut fn_env = self.env.child();

        // Add parameters to environment
        for param in &def.params {
            let ty = self.resolve_type_expr(&param.ty);
            fn_env.define(param.name.clone(), ty);
        }

        // Check body
        let _body_type = self.check_block(&def.body, &mut fn_env);

        // TODO: check return type matches
    }

    fn check_test(&mut self, test: &TestBlock) {
        let mut test_env = self.env.child();
        self.check_block(&test.body, &mut test_env);
    }

    fn check_property(&mut self, prop: &PropertyBlock) {
        let mut prop_env = self.env.child();
        self.check_block(&prop.body, &mut prop_env);
    }

    fn check_block(&mut self, block: &Block, env: &mut TypeEnv) -> Type {
        for stmt in &block.stmts {
            self.check_stmt(stmt, env);
        }

        if let Some(expr) = &block.expr {
            self.check_expr(expr, env)
        } else {
            Type::Unit
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, env: &mut TypeEnv) {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                let value_type = self.check_expr(value, env);

                let declared_type = ty.as_ref().map(|t| self.resolve_type_expr(t));

                if let Some(declared) = &declared_type {
                    if !self.types_compatible(&value_type, declared) {
                        self.diagnostics.push(
                            Diagnostic::error(crate::diagnostics::error_codes::types::TYPE_MISMATCH)
                                .message(format!(
                                    "Expected type {:?}, found {:?}",
                                    declared, value_type
                                ))
                                .build(),
                        );
                    }
                }

                env.define(name.clone(), declared_type.unwrap_or(value_type));
            }
            Stmt::Assign { target, value, .. } => {
                let _target_type = self.check_expr(target, env);
                let _value_type = self.check_expr(value, env);
                // TODO: check types match
            }
            Stmt::Expr { expr, .. } => {
                self.check_expr(expr, env);
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    self.check_expr(v, env);
                }
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr, env: &TypeEnv) -> Type {
        match expr {
            Expr::IntLit { .. } => Type::Int,
            Expr::BoolLit { .. } => Type::Bool,
            Expr::TextLit { .. } => Type::Text,
            Expr::UnitLit { .. } => Type::Unit,
            Expr::Ident { name, span, .. } => {
                if let Some(ty) = env.lookup(name) {
                    ty.clone()
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            crate::diagnostics::error_codes::types::UNKNOWN_IDENTIFIER,
                        )
                        .message(format!("Unknown identifier: {}", name))
                        .span(span.clone())
                        .build(),
                    );
                    Type::Unknown
                }
            }
            Expr::Binary { left, right, .. } => {
                let left_ty = self.check_expr(left, env);
                let right_ty = self.check_expr(right, env);

                // Simplified: assume binary ops return Int for Int operands
                if left_ty == Type::Int && right_ty == Type::Int {
                    Type::Int
                } else if left_ty == Type::Bool && right_ty == Type::Bool {
                    Type::Bool
                } else {
                    Type::Unknown
                }
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_ty = self.check_expr(cond, env);
                if cond_ty != Type::Bool && cond_ty != Type::Unknown {
                    self.diagnostics.push(
                        Diagnostic::error(crate::diagnostics::error_codes::types::TYPE_MISMATCH)
                            .message("Condition must be Bool")
                            .build(),
                    );
                }

                let mut then_env = env.clone();
                let then_ty = self.check_block(then_branch, &mut then_env);

                if let Some(else_expr) = else_branch {
                    let else_ty = self.check_expr(else_expr, env);
                    if then_ty != else_ty && then_ty != Type::Unknown && else_ty != Type::Unknown {
                        self.diagnostics.push(
                            Diagnostic::error(crate::diagnostics::error_codes::types::TYPE_MISMATCH)
                                .message("If branches have different types")
                                .build(),
                        );
                    }
                    then_ty
                } else {
                    Type::Unit
                }
            }
            Expr::Match { expr, arms, span, .. } => {
                let scrutinee_ty = self.check_expr(expr, env);

                // Collect patterns for exhaustiveness checking
                let patterns: Vec<&Pattern> = arms.iter().map(|arm| &arm.pattern).collect();

                // Check exhaustiveness
                self.check_match_exhaustiveness(&scrutinee_ty, &patterns, span);

                if arms.is_empty() {
                    Type::Unit
                } else {
                    // Type check all arm bodies (not just the first)
                    let mut result_ty = Type::Unknown;
                    for arm in arms {
                        let arm_ty = self.check_expr(&arm.body, env);
                        if result_ty == Type::Unknown {
                            result_ty = arm_ty;
                        }
                        // TODO: check all arms have compatible types
                    }
                    result_ty
                }
            }
            Expr::Call { func, args, .. } => {
                let func_ty = self.check_expr(func, env);

                for arg in args {
                    self.check_expr(arg, env);
                }

                if let Type::Function { ret, .. } = func_ty {
                    *ret
                } else {
                    Type::Unknown
                }
            }
            Expr::Record { fields, .. } => {
                let field_types: Vec<_> = fields
                    .iter()
                    .map(|(name, expr)| (name.clone(), self.check_expr(expr, env)))
                    .collect();
                Type::Record(field_types)
            }
            Expr::FieldAccess { expr, field, .. } => {
                let expr_ty = self.check_expr(expr, env);
                if let Type::Record(fields) = expr_ty {
                    fields
                        .iter()
                        .find(|(n, _)| n == field)
                        .map(|(_, t)| t.clone())
                        .unwrap_or(Type::Unknown)
                } else {
                    Type::Unknown
                }
            }
            Expr::Block { block, .. } => {
                let mut block_env = env.clone();
                self.check_block(block, &mut block_env)
            }
            Expr::Hole { span, .. } => {
                self.diagnostics.push(
                    Diagnostic::info("H0001")
                        .message("Typed hole - type unknown")
                        .span(span.clone())
                        .build(),
                );
                Type::Unknown
            }
            _ => Type::Unknown,
        }
    }

    fn resolve_type_expr(&self, ty: &TypeExpr) -> Type {
        match ty {
            TypeExpr::Named { name, args, .. } => match name.as_str() {
                "Int" => Type::Int,
                "Bool" => Type::Bool,
                "Text" => Type::Text,
                "Unit" => Type::Unit,
                "Option" if args.len() == 1 => {
                    Type::Option(Box::new(self.resolve_type_expr(&args[0])))
                }
                "Result" if args.len() == 2 => Type::Result(
                    Box::new(self.resolve_type_expr(&args[0])),
                    Box::new(self.resolve_type_expr(&args[1])),
                ),
                _ => Type::Named(
                    name.clone(),
                    args.iter().map(|a| self.resolve_type_expr(a)).collect(),
                ),
            },
            TypeExpr::Record { fields, .. } => Type::Record(
                fields
                    .iter()
                    .map(|f| (f.name.clone(), self.resolve_type_expr(&f.ty)))
                    .collect(),
            ),
            TypeExpr::Function {
                params,
                ret,
                effects,
                ..
            } => Type::Function {
                params: params.iter().map(|p| self.resolve_type_expr(p)).collect(),
                ret: Box::new(self.resolve_type_expr(ret)),
                effects: effects.clone(),
            },
        }
    }

    fn types_compatible(&self, actual: &Type, expected: &Type) -> bool {
        if actual == &Type::Unknown || expected == &Type::Unknown {
            return true;
        }
        actual == expected
    }

    /// Check if a match expression is exhaustive
    fn check_match_exhaustiveness(
        &mut self,
        scrutinee_ty: &Type,
        patterns: &[&Pattern],
        match_span: &Span,
    ) {
        // Check if any pattern is a wildcard or catch-all identifier
        for pattern in patterns {
            if self.pattern_is_catch_all(pattern) {
                return; // Wildcard/identifier covers everything
            }
        }

        // Collect missing variants based on the scrutinee type
        let missing = self.find_missing_patterns(scrutinee_ty, patterns);

        if !missing.is_empty() {
            let missing_str = missing.join(", ");
            let suggestion_text = missing
                .iter()
                .map(|m| format!("    {} => ???", m))
                .collect::<Vec<_>>()
                .join("\n");

            self.diagnostics.push(
                Diagnostic::error(crate::diagnostics::error_codes::types::NON_EXHAUSTIVE_MATCH)
                    .message(format!(
                        "Non-exhaustive match: missing pattern{}{}",
                        if missing.len() == 1 { " " } else { "s " },
                        missing_str
                    ))
                    .span(match_span.clone())
                    .suggestion(
                        Suggestion::new("Add missing case(s)")
                            .with_edit(Edit::new(match_span.clone(), suggestion_text)),
                    )
                    .build(),
            );
        }
    }

    /// Check if a pattern is a catch-all (wildcard or simple identifier binding)
    fn pattern_is_catch_all(&self, pattern: &Pattern) -> bool {
        match pattern {
            Pattern::Wildcard { .. } => true,
            // Simple identifier binding catches everything
            Pattern::Ident { .. } => true,
            _ => false,
        }
    }

    /// Find missing patterns for a given type
    fn find_missing_patterns(&self, ty: &Type, patterns: &[&Pattern]) -> Vec<String> {
        match ty {
            Type::Option(_) => self.find_missing_option_patterns(patterns),
            Type::Result(_, _) => self.find_missing_result_patterns(patterns),
            Type::Bool => self.find_missing_bool_patterns(patterns),
            _ => vec![], // Unknown type or we don't track exhaustiveness for it
        }
    }

    /// Find missing Option patterns (Some/None)
    fn find_missing_option_patterns(&self, patterns: &[&Pattern]) -> Vec<String> {
        let mut has_some = false;
        let mut has_none = false;

        for pattern in patterns {
            match pattern {
                Pattern::Variant { name, .. } => {
                    match name.as_str() {
                        "Some" => has_some = true,
                        "None" => has_none = true,
                        _ => {}
                    }
                }
                Pattern::Wildcard { .. } | Pattern::Ident { .. } => {
                    // Catch-all pattern covers both
                    has_some = true;
                    has_none = true;
                }
                _ => {}
            }
        }

        let mut missing = Vec::new();
        if !has_some {
            missing.push("Some(_)".to_string());
        }
        if !has_none {
            missing.push("None".to_string());
        }
        missing
    }

    /// Find missing Result patterns (Ok/Err)
    fn find_missing_result_patterns(&self, patterns: &[&Pattern]) -> Vec<String> {
        let mut has_ok = false;
        let mut has_err = false;

        for pattern in patterns {
            match pattern {
                Pattern::Variant { name, .. } => {
                    match name.as_str() {
                        "Ok" => has_ok = true,
                        "Err" => has_err = true,
                        _ => {}
                    }
                }
                Pattern::Wildcard { .. } | Pattern::Ident { .. } => {
                    // Catch-all pattern covers both
                    has_ok = true;
                    has_err = true;
                }
                _ => {}
            }
        }

        let mut missing = Vec::new();
        if !has_ok {
            missing.push("Ok(_)".to_string());
        }
        if !has_err {
            missing.push("Err(_)".to_string());
        }
        missing
    }

    /// Find missing Bool patterns (true/false)
    fn find_missing_bool_patterns(&self, patterns: &[&Pattern]) -> Vec<String> {
        let mut has_true = false;
        let mut has_false = false;

        for pattern in patterns {
            match pattern {
                Pattern::BoolLit { value: true, .. } => has_true = true,
                Pattern::BoolLit { value: false, .. } => has_false = true,
                Pattern::Wildcard { .. } | Pattern::Ident { .. } => {
                    has_true = true;
                    has_false = true;
                }
                _ => {}
            }
        }

        let mut missing = Vec::new();
        if !has_true {
            missing.push("true".to_string());
        }
        if !has_false {
            missing.push("false".to_string());
        }
        missing
    }

    fn _fresh_var(&mut self) -> Type {
        let id = TypeVarId(self.next_var);
        self.next_var += 1;
        Type::Var(id)
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Check exhaustiveness of pattern matching
pub fn check_exhaustiveness(
    _scrutinee_type: &Type,
    _patterns: &[Pattern],
) -> Result<(), Vec<String>> {
    // TODO: implement exhaustiveness checking
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dummy_span() -> Span {
        Span::new(PathBuf::from("test.astra"), 0, 0, 1, 1, 1, 1)
    }

    #[test]
    fn test_type_env() {
        let mut env = TypeEnv::new();
        env.define("x".to_string(), Type::Int);

        assert_eq!(env.lookup("x"), Some(&Type::Int));
        assert_eq!(env.lookup("y"), None);
    }

    #[test]
    fn test_child_env() {
        let mut parent = TypeEnv::new();
        parent.define("x".to_string(), Type::Int);

        let mut child = parent.child();
        child.define("y".to_string(), Type::Bool);

        assert_eq!(child.lookup("x"), Some(&Type::Int));
        assert_eq!(child.lookup("y"), Some(&Type::Bool));
    }

    #[test]
    fn test_option_exhaustive_both_cases() {
        let checker = TypeChecker::new();
        let some_pat = Pattern::Variant {
            id: NodeId::new(),
            span: dummy_span(),
            name: "Some".to_string(),
            data: Some(Box::new(Pattern::Ident {
                id: NodeId::new(),
                span: dummy_span(),
                name: "x".to_string(),
            })),
        };
        let none_pat = Pattern::Variant {
            id: NodeId::new(),
            span: dummy_span(),
            name: "None".to_string(),
            data: None,
        };
        let patterns: Vec<&Pattern> = vec![&some_pat, &none_pat];
        let missing = checker.find_missing_option_patterns(&patterns);
        assert!(missing.is_empty(), "Expected no missing patterns");
    }

    #[test]
    fn test_option_missing_none() {
        let checker = TypeChecker::new();
        let some_pat = Pattern::Variant {
            id: NodeId::new(),
            span: dummy_span(),
            name: "Some".to_string(),
            data: Some(Box::new(Pattern::Ident {
                id: NodeId::new(),
                span: dummy_span(),
                name: "x".to_string(),
            })),
        };
        let patterns: Vec<&Pattern> = vec![&some_pat];
        let missing = checker.find_missing_option_patterns(&patterns);
        assert_eq!(missing, vec!["None"]);
    }

    #[test]
    fn test_option_missing_some() {
        let checker = TypeChecker::new();
        let none_pat = Pattern::Variant {
            id: NodeId::new(),
            span: dummy_span(),
            name: "None".to_string(),
            data: None,
        };
        let patterns: Vec<&Pattern> = vec![&none_pat];
        let missing = checker.find_missing_option_patterns(&patterns);
        assert_eq!(missing, vec!["Some(_)"]);
    }

    #[test]
    fn test_option_wildcard_covers_all() {
        let checker = TypeChecker::new();
        let wildcard = Pattern::Wildcard {
            id: NodeId::new(),
            span: dummy_span(),
        };
        let patterns: Vec<&Pattern> = vec![&wildcard];
        let missing = checker.find_missing_option_patterns(&patterns);
        assert!(missing.is_empty(), "Wildcard should cover all cases");
    }

    #[test]
    fn test_result_exhaustive() {
        let checker = TypeChecker::new();
        let ok_pat = Pattern::Variant {
            id: NodeId::new(),
            span: dummy_span(),
            name: "Ok".to_string(),
            data: Some(Box::new(Pattern::Ident {
                id: NodeId::new(),
                span: dummy_span(),
                name: "x".to_string(),
            })),
        };
        let err_pat = Pattern::Variant {
            id: NodeId::new(),
            span: dummy_span(),
            name: "Err".to_string(),
            data: Some(Box::new(Pattern::Ident {
                id: NodeId::new(),
                span: dummy_span(),
                name: "e".to_string(),
            })),
        };
        let patterns: Vec<&Pattern> = vec![&ok_pat, &err_pat];
        let missing = checker.find_missing_result_patterns(&patterns);
        assert!(missing.is_empty(), "Expected no missing patterns");
    }

    #[test]
    fn test_result_missing_err() {
        let checker = TypeChecker::new();
        let ok_pat = Pattern::Variant {
            id: NodeId::new(),
            span: dummy_span(),
            name: "Ok".to_string(),
            data: Some(Box::new(Pattern::Ident {
                id: NodeId::new(),
                span: dummy_span(),
                name: "x".to_string(),
            })),
        };
        let patterns: Vec<&Pattern> = vec![&ok_pat];
        let missing = checker.find_missing_result_patterns(&patterns);
        assert_eq!(missing, vec!["Err(_)"]);
    }

    #[test]
    fn test_bool_exhaustive() {
        let checker = TypeChecker::new();
        let true_pat = Pattern::BoolLit {
            id: NodeId::new(),
            span: dummy_span(),
            value: true,
        };
        let false_pat = Pattern::BoolLit {
            id: NodeId::new(),
            span: dummy_span(),
            value: false,
        };
        let patterns: Vec<&Pattern> = vec![&true_pat, &false_pat];
        let missing = checker.find_missing_bool_patterns(&patterns);
        assert!(missing.is_empty(), "Expected no missing patterns");
    }

    #[test]
    fn test_bool_missing_false() {
        let checker = TypeChecker::new();
        let true_pat = Pattern::BoolLit {
            id: NodeId::new(),
            span: dummy_span(),
            value: true,
        };
        let patterns: Vec<&Pattern> = vec![&true_pat];
        let missing = checker.find_missing_bool_patterns(&patterns);
        assert_eq!(missing, vec!["false"]);
    }

    #[test]
    fn test_ident_pattern_catches_all() {
        let checker = TypeChecker::new();
        let ident_pat = Pattern::Ident {
            id: NodeId::new(),
            span: dummy_span(),
            name: "x".to_string(),
        };
        let patterns: Vec<&Pattern> = vec![&ident_pat];

        // For Option
        let missing = checker.find_missing_option_patterns(&patterns);
        assert!(missing.is_empty(), "Ident should cover all Option cases");

        // For Result
        let missing = checker.find_missing_result_patterns(&patterns);
        assert!(missing.is_empty(), "Ident should cover all Result cases");

        // For Bool
        let missing = checker.find_missing_bool_patterns(&patterns);
        assert!(missing.is_empty(), "Ident should cover all Bool cases");
    }
}
