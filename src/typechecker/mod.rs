//! Type checker for Astra
//!
//! Implements type checking, inference, exhaustiveness checking, effect enforcement,
//! and lint checks (W0001-W0007).

use crate::diagnostics::{Diagnostic, DiagnosticBag, Note, Span, Suggestion};
use crate::parser::ast::*;
use std::collections::{HashMap, HashSet};

/// Tracks a variable definition for unused-variable lint (W0001)
#[derive(Debug, Clone)]
struct VarBinding {
    name: String,
    span: Span,
    used: bool,
}

/// Tracks lint state within a scope
#[derive(Debug, Clone, Default)]
struct LintScope {
    /// Variables defined in this scope, with usage tracking
    vars: Vec<VarBinding>,
    /// Set of variable names already defined in this scope (for shadowing detection)
    defined_names: HashSet<String>,
}

/// Built-in types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// Unit type
    Unit,
    /// Integer type
    Int,
    /// Float type
    Float,
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

/// The kind of type being matched, used for exhaustiveness checking
#[derive(Debug, Clone)]
enum MatchTypeKind {
    /// Option type: must cover Some(_) and None
    Option,
    /// Result type: must cover Ok(_) and Err(_)
    Result,
    /// Bool type: must cover true and false
    Bool,
    /// User-defined enum: must cover all variants
    Enum {
        _name: String,
        variants: Vec<String>,
    },
    /// Unknown or unconstrained type (skip exhaustiveness)
    Other,
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
    /// Stack of lint scopes for tracking variable usage
    lint_scopes: Vec<LintScope>,
    /// Import names defined at module level, with usage tracking
    imports: Vec<(String, Span, bool)>,
}

impl TypeChecker {
    /// Create a new type checker
    pub fn new() -> Self {
        Self {
            env: TypeEnv::new(),
            diagnostics: DiagnosticBag::new(),
            next_var: 0,
            lint_scopes: Vec::new(),
            imports: Vec::new(),
        }
    }

    /// Push a new lint scope
    fn push_lint_scope(&mut self) {
        self.lint_scopes.push(LintScope::default());
    }

    /// Pop a lint scope and emit W0001 warnings for unused variables
    fn pop_lint_scope(&mut self) {
        if let Some(scope) = self.lint_scopes.pop() {
            for binding in &scope.vars {
                if !binding.used && !binding.name.starts_with('_') {
                    self.diagnostics.push(
                        Diagnostic::warning(
                            crate::diagnostics::error_codes::warnings::UNUSED_VARIABLE,
                        )
                        .message(format!("Unused variable `{}`", binding.name))
                        .span(binding.span.clone())
                        .note(Note::new(format!(
                            "prefix with `_` to suppress this warning: `_{}`",
                            binding.name
                        )))
                        .build(),
                    );
                }
            }
        }
    }

    /// Record a variable definition in the current lint scope.
    /// Also checks for shadowing (W0006).
    fn lint_define_var(&mut self, name: &str, span: &Span) {
        // W0006: Check for shadowing in current scope
        if let Some(scope) = self.lint_scopes.last() {
            if scope.defined_names.contains(name) && !name.starts_with('_') {
                self.diagnostics.push(
                    Diagnostic::warning(
                        crate::diagnostics::error_codes::warnings::SHADOWED_BINDING,
                    )
                    .message(format!(
                        "Variable `{}` shadows a previous binding in the same scope",
                        name
                    ))
                    .span(span.clone())
                    .build(),
                );
            }
        }

        if let Some(scope) = self.lint_scopes.last_mut() {
            scope.vars.push(VarBinding {
                name: name.to_string(),
                span: span.clone(),
                used: false,
            });
            scope.defined_names.insert(name.to_string());
        }
    }

    /// Mark a variable as used across all lint scopes (innermost first)
    fn lint_use_var(&mut self, name: &str) {
        for scope in self.lint_scopes.iter_mut().rev() {
            for binding in scope.vars.iter_mut().rev() {
                if binding.name == name {
                    binding.used = true;
                    return;
                }
            }
        }
        // Also mark imports as used
        for import in self.imports.iter_mut() {
            if import.0 == name {
                import.2 = true;
                return;
            }
        }
    }

    /// Check a module
    pub fn check_module(&mut self, module: &Module) -> Result<(), DiagnosticBag> {
        // First pass: collect all type/enum/fn definitions and imports
        for item in &module.items {
            match item {
                Item::Import(import) => {
                    // Track imports for W0002 (unused import)
                    let import_name = match &import.kind {
                        ImportKind::Module => {
                            import.path.segments.last().cloned().unwrap_or_default()
                        }
                        ImportKind::Alias(alias) => alias.clone(),
                        ImportKind::Items(items) => {
                            // Track each imported item
                            for item_name in items {
                                self.imports
                                    .push((item_name.clone(), import.span.clone(), false));
                            }
                            continue;
                        }
                    };
                    self.imports.push((import_name, import.span.clone(), false));
                }
                Item::TypeDef(def) => self.env.register_type(def.clone()),
                Item::EnumDef(def) => self.env.register_enum(def.clone()),
                Item::FnDef(def) => {
                    self.env.register_fn(def.clone());
                    // Also add function name as a binding so it can be looked up
                    let param_types: Vec<Type> = def
                        .params
                        .iter()
                        .map(|p| self.resolve_type_expr(&p.ty))
                        .collect();
                    let ret_type = def
                        .return_type
                        .as_ref()
                        .map(|t| self.resolve_type_expr(t))
                        .unwrap_or(Type::Unit);
                    self.env.define(
                        def.name.clone(),
                        Type::Function {
                            params: param_types,
                            ret: Box::new(ret_type),
                            effects: def.effects.clone(),
                        },
                    );
                }
                _ => {}
            }
        }

        // Second pass: type check all items
        for item in &module.items {
            self.check_item(item);
        }

        // W0002: Emit warnings for unused imports
        for (name, span, used) in &self.imports {
            if !*used {
                self.diagnostics.push(
                    Diagnostic::warning(crate::diagnostics::error_codes::warnings::UNUSED_IMPORT)
                        .message(format!("Unused import `{}`", name))
                        .span(span.clone())
                        .note(Note::new("remove this import if it is no longer needed"))
                        .build(),
                );
            }
        }

        if self.diagnostics.has_errors() {
            Err(self.diagnostics.clone())
        } else {
            Ok(())
        }
    }

    /// Get diagnostics (including non-error diagnostics like warnings)
    pub fn diagnostics(&self) -> &DiagnosticBag {
        &self.diagnostics
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
            Item::TraitDef(_) => {
                // Trait definitions define interfaces, checked structurally
            }
            Item::ImplBlock(impl_block) => {
                // Check each method in the impl block
                for method in &impl_block.methods {
                    self.check_fndef(method);
                }
            }
            Item::EffectDef(_) => {
                // Effect definitions are structurally valid if parsed
            }
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

        // Push lint scope for function body
        self.push_lint_scope();

        // Register type parameters as unknown types
        for tp in &def.type_params {
            fn_env.define(tp.clone(), Type::Unknown);
        }

        // Add parameters to environment
        for param in &def.params {
            let ty = self.resolve_type_expr(&param.ty);
            fn_env.define(param.name.clone(), ty);
            self.lint_define_var(&param.name, &param.span);
        }

        // Check body and collect effects used
        let mut effects_used = HashSet::new();
        let _body_type = self.check_block_with_effects(&def.body, &mut fn_env, &mut effects_used);

        // Pop lint scope — emits W0001 for unused variables/params
        self.pop_lint_scope();

        // C4: Effect enforcement - check that used effects are declared
        let declared_effects: HashSet<String> = def.effects.iter().cloned().collect();
        for used_effect in &effects_used {
            if !declared_effects.contains(used_effect) {
                self.diagnostics.push(
                    Diagnostic::error(
                        crate::diagnostics::error_codes::effects::EFFECT_NOT_DECLARED,
                    )
                    .message(format!(
                        "Effect `{}` used but not declared in function `{}`",
                        used_effect, def.name
                    ))
                    .span(def.span.clone())
                    .note(Note::new(format!(
                        "function `{}` must declare `effects({})` or remove this call",
                        def.name, used_effect
                    )))
                    .suggestion(Suggestion::new(format!(
                        "Add `effects({})` to the function signature",
                        effects_used.iter().cloned().collect::<Vec<_>>().join(", ")
                    )))
                    .build(),
                );
            }
        }
    }

    fn check_test(&mut self, test: &TestBlock) {
        let mut test_env = self.env.child();
        let mut effects_used = HashSet::new();
        self.push_lint_scope();
        self.check_block_with_effects(&test.body, &mut test_env, &mut effects_used);
        self.pop_lint_scope();
    }

    fn check_property(&mut self, prop: &PropertyBlock) {
        let mut prop_env = self.env.child();
        let mut effects_used = HashSet::new();
        self.push_lint_scope();
        self.check_block_with_effects(&prop.body, &mut prop_env, &mut effects_used);
        self.pop_lint_scope();
    }

    fn check_block_with_effects(
        &mut self,
        block: &Block,
        env: &mut TypeEnv,
        effects: &mut HashSet<String>,
    ) -> Type {
        let mut seen_return = false;
        for stmt in &block.stmts {
            // W0003: Unreachable code after return
            if seen_return {
                let stmt_span = match stmt {
                    Stmt::Let { span, .. }
                    | Stmt::LetPattern { span, .. }
                    | Stmt::Assign { span, .. }
                    | Stmt::Expr { span, .. }
                    | Stmt::Return { span, .. } => span.clone(),
                };
                self.diagnostics.push(
                    Diagnostic::warning(
                        crate::diagnostics::error_codes::warnings::UNREACHABLE_CODE,
                    )
                    .message("Unreachable code after return statement")
                    .span(stmt_span)
                    .note(Note::new("this code will never be executed"))
                    .build(),
                );
                break;
            }

            if matches!(stmt, Stmt::Return { .. }) {
                seen_return = true;
            }

            self.check_stmt_with_effects(stmt, env, effects);
        }

        if let Some(expr) = &block.expr {
            if seen_return {
                self.diagnostics.push(
                    Diagnostic::warning(
                        crate::diagnostics::error_codes::warnings::UNREACHABLE_CODE,
                    )
                    .message("Unreachable expression after return statement")
                    .span(expr.span().clone())
                    .note(Note::new("this expression will never be evaluated"))
                    .build(),
                );
            }
            self.check_expr_with_effects(expr, env, effects)
        } else {
            Type::Unit
        }
    }

    fn check_stmt_with_effects(
        &mut self,
        stmt: &Stmt,
        env: &mut TypeEnv,
        effects: &mut HashSet<String>,
    ) {
        match stmt {
            Stmt::Let {
                name,
                ty,
                value,
                span,
                ..
            } => {
                let value_type = self.check_expr_with_effects(value, env, effects);

                let declared_type = ty.as_ref().map(|t| self.resolve_type_expr(t));

                if let Some(declared) = &declared_type {
                    if !self.types_compatible(&value_type, declared) {
                        self.diagnostics.push(
                            Diagnostic::error(
                                crate::diagnostics::error_codes::types::TYPE_MISMATCH,
                            )
                            .message(format!(
                                "Expected type {:?}, found {:?}",
                                declared, value_type
                            ))
                            .suggestion(Suggestion::new(format!(
                                "Change the type annotation to match the value type `{:?}`",
                                value_type
                            )))
                            .build(),
                        );
                    }
                }

                env.define(name.clone(), declared_type.unwrap_or(value_type));

                // Track for lint (W0001 unused var, W0006 shadowed binding)
                self.lint_define_var(name, span);
            }
            Stmt::LetPattern { pattern, value, .. } => {
                self.check_expr_with_effects(value, env, effects);
                // Bind pattern variables into the environment
                collect_pattern_bindings(pattern, env);
            }
            Stmt::Assign { target, value, .. } => {
                let _target_type = self.check_expr_with_effects(target, env, effects);
                let _value_type = self.check_expr_with_effects(value, env, effects);
            }
            Stmt::Expr { expr, .. } => {
                self.check_expr_with_effects(expr, env, effects);
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    self.check_expr_with_effects(v, env, effects);
                }
            }
        }
    }

    fn check_expr_with_effects(
        &mut self,
        expr: &Expr,
        env: &TypeEnv,
        effects: &mut HashSet<String>,
    ) -> Type {
        match expr {
            Expr::IntLit { .. } => Type::Int,
            Expr::FloatLit { .. } => Type::Float,
            Expr::BoolLit { .. } => Type::Bool,
            Expr::TextLit { .. } => Type::Text,
            Expr::UnitLit { .. } => Type::Unit,
            Expr::Ident { name, span, .. } => {
                // Built-in constructors and effects are always available
                match name.as_str() {
                    "Some" | "None" | "Ok" | "Err" => Type::Unknown,
                    "Console" | "Fs" | "Net" | "Clock" | "Rand" | "Env" | "Map" | "Set" => {
                        Type::Unknown
                    }
                    "assert" | "assert_eq" | "print" | "println" | "len" | "to_text" | "range"
                    | "abs" | "min" | "max" | "pow" | "to_int" | "to_float" | "sqrt" | "floor"
                    | "ceil" | "round" => Type::Unknown,
                    _ => {
                        // Mark variable as used for W0001 lint
                        self.lint_use_var(name);

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
                }
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                let left_ty = self.check_expr_with_effects(left, env, effects);
                let right_ty = self.check_expr_with_effects(right, env, effects);

                match op {
                    // Comparison operators always return Bool
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge => Type::Bool,
                    // Logical operators return Bool
                    BinaryOp::And | BinaryOp::Or => Type::Bool,
                    // Arithmetic operators
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod => {
                        if left_ty == Type::Int && right_ty == Type::Int {
                            Type::Int
                        } else if left_ty == Type::Float || right_ty == Type::Float {
                            Type::Float
                        } else if left_ty == Type::Text
                            && right_ty == Type::Text
                            && *op == BinaryOp::Add
                        {
                            Type::Text
                        } else {
                            Type::Unknown
                        }
                    }
                    // Pipe operator returns the return type of the right-hand function
                    BinaryOp::Pipe => {
                        if let Type::Function { ret, .. } = right_ty {
                            *ret
                        } else {
                            Type::Unknown
                        }
                    }
                }
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_ty = self.check_expr_with_effects(cond, env, effects);
                if cond_ty != Type::Bool && cond_ty != Type::Unknown {
                    self.diagnostics.push(
                        Diagnostic::error(crate::diagnostics::error_codes::types::TYPE_MISMATCH)
                            .message("Condition must be Bool")
                            .build(),
                    );
                }

                let mut then_env = env.clone();
                let then_ty = self.check_block_with_effects(then_branch, &mut then_env, effects);

                if let Some(else_expr) = else_branch {
                    let else_ty = self.check_expr_with_effects(else_expr, env, effects);
                    if then_ty != else_ty && then_ty != Type::Unknown && else_ty != Type::Unknown {
                        self.diagnostics.push(
                            Diagnostic::error(
                                crate::diagnostics::error_codes::types::TYPE_MISMATCH,
                            )
                            .message("If branches have different types")
                            .build(),
                        );
                    }
                    then_ty
                } else {
                    Type::Unit
                }
            }
            Expr::Match {
                expr: scrutinee,
                arms,
                span,
                ..
            } => {
                let scrutinee_ty = self.check_expr_with_effects(scrutinee, env, effects);

                // Check each arm body with pattern bindings in scope
                let mut first_arm_ty = Type::Unit;
                for (i, arm) in arms.iter().enumerate() {
                    let mut arm_env = env.clone();
                    collect_pattern_bindings(&arm.pattern, &mut arm_env);
                    // Check guard expression if present
                    if let Some(guard) = &arm.guard {
                        self.check_expr_with_effects(guard, &arm_env, effects);
                    }
                    let arm_ty = self.check_expr_with_effects(&arm.body, &arm_env, effects);
                    if i == 0 {
                        first_arm_ty = arm_ty;
                    }
                }

                // C2: Exhaustiveness checking
                let patterns: Vec<&Pattern> = arms.iter().map(|a| &a.pattern).collect();
                self.check_match_exhaustiveness(&scrutinee_ty, &patterns, span, env);

                first_arm_ty
            }
            Expr::Call { func, args, .. } => {
                // C4: Track effect usage from method-like calls
                if let Expr::Ident { name, .. } = func.as_ref() {
                    match name.as_str() {
                        "assert" | "assert_eq" | "Some" | "Ok" | "Err" | "None" => {}
                        _ => {}
                    }
                }

                let func_ty = self.check_expr_with_effects(func, env, effects);

                for arg in args {
                    self.check_expr_with_effects(arg, env, effects);
                }

                if let Type::Function { ret, .. } = func_ty {
                    *ret
                } else {
                    Type::Unknown
                }
            }
            // C4: Track effect usage from qualified identifiers (e.g., Console.println)
            Expr::QualifiedIdent { module, .. } => {
                let known_effects = ["Console", "Fs", "Net", "Clock", "Rand", "Env"];
                if known_effects.contains(&module.as_str()) {
                    effects.insert(module.clone());
                }
                Type::Unknown
            }
            // C4: Track effect usage from method calls (e.g., Console.println())
            Expr::MethodCall {
                receiver,
                method: _,
                args,
                ..
            } => {
                // Check if receiver is an effect name
                if let Expr::Ident { name, .. } = receiver.as_ref() {
                    let known_effects = ["Console", "Fs", "Net", "Clock", "Rand", "Env"];
                    if known_effects.contains(&name.as_str()) {
                        effects.insert(name.clone());
                    }
                }
                self.check_expr_with_effects(receiver, env, effects);
                for arg in args {
                    self.check_expr_with_effects(arg, env, effects);
                }
                Type::Unknown
            }
            Expr::Record { fields, .. } => {
                let field_types: Vec<_> = fields
                    .iter()
                    .map(|(name, expr)| {
                        (
                            name.clone(),
                            self.check_expr_with_effects(expr, env, effects),
                        )
                    })
                    .collect();
                Type::Record(field_types)
            }
            Expr::FieldAccess { expr, field, .. } => {
                let expr_ty = self.check_expr_with_effects(expr, env, effects);
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
                self.check_block_with_effects(block, &mut block_env, effects)
            }
            Expr::Unary { expr, .. } => {
                self.check_expr_with_effects(expr, env, effects);
                Type::Unknown
            }
            Expr::Try { expr, .. } | Expr::TryElse { expr, .. } => {
                self.check_expr_with_effects(expr, env, effects);
                Type::Unknown
            }
            Expr::ListLit { elements, .. } => {
                for elem in elements {
                    self.check_expr_with_effects(elem, env, effects);
                }
                Type::Unknown // List type not fully tracked yet
            }
            Expr::Lambda {
                params,
                return_type,
                body,
                ..
            } => {
                let mut lambda_env = env.clone();
                self.push_lint_scope();
                for param in params {
                    let ty = param
                        .ty
                        .as_ref()
                        .map(|t| self.resolve_type_expr(t))
                        .unwrap_or(Type::Unknown);
                    lambda_env.define(param.name.clone(), ty);
                    self.lint_define_var(&param.name, &param.span);
                }
                let _body_ty = self.check_block_with_effects(body, &mut lambda_env, effects);
                self.pop_lint_scope();

                let param_types: Vec<Type> = params
                    .iter()
                    .map(|p| {
                        p.ty.as_ref()
                            .map(|t| self.resolve_type_expr(t))
                            .unwrap_or(Type::Unknown)
                    })
                    .collect();
                let ret_ty = return_type
                    .as_ref()
                    .map(|t| self.resolve_type_expr(t))
                    .unwrap_or(Type::Unknown);
                Type::Function {
                    params: param_types,
                    ret: Box::new(ret_ty),
                    effects: Vec::new(),
                }
            }
            Expr::ForIn {
                binding,
                iter,
                body,
                ..
            } => {
                self.check_expr_with_effects(iter, env, effects);
                let mut loop_env = env.clone();
                loop_env.define(binding.clone(), Type::Unknown);
                self.push_lint_scope();
                self.lint_define_var(binding, iter.span());
                self.check_block_with_effects(body, &mut loop_env, effects);
                self.pop_lint_scope();
                Type::Unit
            }
            Expr::While { cond, body, .. } => {
                self.check_expr_with_effects(cond, env, effects);
                let mut loop_env = env.clone();
                self.push_lint_scope();
                self.check_block_with_effects(body, &mut loop_env, effects);
                self.pop_lint_scope();
                Type::Unit
            }
            Expr::Break { .. } | Expr::Continue { .. } => Type::Unit,
            Expr::StringInterp { parts, .. } => {
                for part in parts {
                    if let StringPart::Expr(expr) = part {
                        self.check_expr_with_effects(expr, env, effects);
                    }
                }
                Type::Text
            }
            Expr::TupleLit { elements, .. } => {
                for elem in elements {
                    self.check_expr_with_effects(elem, env, effects);
                }
                Type::Unknown // Tuple type not fully tracked yet
            }
            Expr::MapLit { entries, .. } => {
                for (k, v) in entries {
                    self.check_expr_with_effects(k, env, effects);
                    self.check_expr_with_effects(v, env, effects);
                }
                Type::Unknown // Map type not fully tracked yet
            }
            Expr::Await { expr, .. } => {
                // P6.5: await just checks the inner expression
                self.check_expr_with_effects(expr, env, effects)
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
        }
    }

    /// C2: Check exhaustiveness of a match expression
    fn check_match_exhaustiveness(
        &mut self,
        scrutinee_ty: &Type,
        patterns: &[&Pattern],
        match_span: &Span,
        env: &TypeEnv,
    ) {
        // Determine the kind of type being matched
        let match_kind = self.infer_match_kind(scrutinee_ty, patterns, env);

        // Check for wildcard or catch-all patterns first
        #[allow(clippy::redundant_closure)]
        if patterns.iter().any(|p| is_catch_all(p)) {
            // W0005: Wildcard match — warn when the type is known and all
            // variants could be explicitly listed
            if let Some(wildcard_pat) = patterns
                .iter()
                .find(|p| matches!(p, Pattern::Wildcard { .. }))
            {
                let type_name = match &match_kind {
                    MatchTypeKind::Option => Some("Option"),
                    MatchTypeKind::Result => Some("Result"),
                    MatchTypeKind::Bool => Some("Bool"),
                    MatchTypeKind::Enum { .. } => Some("enum"),
                    MatchTypeKind::Other => None,
                };
                if let Some(name) = type_name {
                    let span = match wildcard_pat {
                        Pattern::Wildcard { span, .. } => span.clone(),
                        _ => match_span.clone(),
                    };
                    self.diagnostics.push(
                        Diagnostic::warning(
                            crate::diagnostics::error_codes::warnings::WILDCARD_MATCH,
                        )
                        .message(format!(
                            "Wildcard pattern `_` on {} type could hide unhandled variants",
                            name
                        ))
                        .span(span)
                        .note(Note::new(
                            "consider matching all variants explicitly to catch future additions",
                        ))
                        .build(),
                    );
                }
            }
            return; // Wildcard/identifier pattern covers everything
        }

        let missing = match match_kind {
            MatchTypeKind::Option => {
                let mut covered = HashSet::new();
                for pat in patterns {
                    if let Pattern::Variant { name, .. } = pat {
                        covered.insert(name.as_str());
                    }
                }
                let mut missing = Vec::new();
                if !covered.contains("Some") {
                    missing.push("Some(_)".to_string());
                }
                if !covered.contains("None") {
                    missing.push("None".to_string());
                }
                missing
            }
            MatchTypeKind::Result => {
                let mut covered = HashSet::new();
                for pat in patterns {
                    if let Pattern::Variant { name, .. } = pat {
                        covered.insert(name.as_str());
                    }
                }
                let mut missing = Vec::new();
                if !covered.contains("Ok") {
                    missing.push("Ok(_)".to_string());
                }
                if !covered.contains("Err") {
                    missing.push("Err(_)".to_string());
                }
                missing
            }
            MatchTypeKind::Bool => {
                let mut has_true = false;
                let mut has_false = false;
                for pat in patterns {
                    if let Pattern::BoolLit { value, .. } = pat {
                        if *value {
                            has_true = true;
                        } else {
                            has_false = true;
                        }
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
            MatchTypeKind::Enum { variants, .. } => {
                let covered: HashSet<&str> = patterns
                    .iter()
                    .filter_map(|p| match p {
                        Pattern::Variant { name, .. } => Some(name.as_str()),
                        _ => None,
                    })
                    .collect();
                variants
                    .iter()
                    .filter(|v| !covered.contains(v.as_str()))
                    .cloned()
                    .collect()
            }
            MatchTypeKind::Other => {
                // Can't check exhaustiveness for unknown types
                return;
            }
        };

        if !missing.is_empty() {
            let missing_display = missing.join(", ");
            let suggestion_text = missing
                .iter()
                .map(|m| format!("    {} => ???", m))
                .collect::<Vec<_>>()
                .join("\n");

            self.diagnostics.push(
                Diagnostic::error(crate::diagnostics::error_codes::types::NON_EXHAUSTIVE_MATCH)
                    .message(format!(
                        "Non-exhaustive match: missing pattern(s) `{}`",
                        missing_display
                    ))
                    .span(match_span.clone())
                    .suggestion(Suggestion::new(format!(
                        "Add missing case(s):\n{}",
                        suggestion_text
                    )))
                    .build(),
            );
        }
    }

    /// Infer what kind of type is being matched based on the scrutinee type and patterns
    fn infer_match_kind(
        &self,
        scrutinee_ty: &Type,
        patterns: &[&Pattern],
        env: &TypeEnv,
    ) -> MatchTypeKind {
        // First, try to determine from the scrutinee type
        match scrutinee_ty {
            Type::Option(_) => return MatchTypeKind::Option,
            Type::Result(_, _) => return MatchTypeKind::Result,
            Type::Bool => return MatchTypeKind::Bool,
            Type::Named(name, _) => {
                if let Some(enum_def) = env.lookup_enum(name) {
                    return MatchTypeKind::Enum {
                        _name: name.clone(),
                        variants: enum_def.variants.iter().map(|v| v.name.clone()).collect(),
                    };
                }
            }
            _ => {}
        }

        // If type is Unknown, infer from patterns
        let variant_names: Vec<&str> = patterns
            .iter()
            .filter_map(|p| match p {
                Pattern::Variant { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();

        // Check for Option patterns
        if variant_names.iter().any(|n| *n == "Some" || *n == "None") {
            return MatchTypeKind::Option;
        }

        // Check for Result patterns
        if variant_names.iter().any(|n| *n == "Ok" || *n == "Err") {
            return MatchTypeKind::Result;
        }

        // Check for Bool patterns
        if patterns
            .iter()
            .any(|p| matches!(p, Pattern::BoolLit { .. }))
        {
            return MatchTypeKind::Bool;
        }

        // Check for user-defined enum patterns
        if let Some(first_variant) = variant_names.first() {
            // Search enum definitions for a match
            if let Some(enum_def) = self.find_enum_containing_variant(first_variant, env) {
                return MatchTypeKind::Enum {
                    _name: enum_def.name.clone(),
                    variants: enum_def.variants.iter().map(|v| v.name.clone()).collect(),
                };
            }
        }

        MatchTypeKind::Other
    }

    /// Find an enum definition that contains the given variant name
    fn find_enum_containing_variant(&self, variant: &str, env: &TypeEnv) -> Option<EnumDef> {
        // Search in the current environment's enum defs
        for enum_def in env.enum_defs.values() {
            if enum_def.variants.iter().any(|v| v.name == variant) {
                return Some(enum_def.clone());
            }
        }
        // Search in parent
        if let Some(parent) = &env.parent {
            return self.find_enum_containing_variant(variant, parent);
        }
        None
    }

    fn resolve_type_expr(&self, ty: &TypeExpr) -> Type {
        match ty {
            TypeExpr::Named { name, args, .. } => match name.as_str() {
                "Int" => Type::Int,
                "Float" => Type::Float,
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
                "List" if args.len() == 1 => {
                    Type::Named("List".to_string(), vec![self.resolve_type_expr(&args[0])])
                }
                "Map" if args.len() == 2 => Type::Named(
                    "Map".to_string(),
                    vec![
                        self.resolve_type_expr(&args[0]),
                        self.resolve_type_expr(&args[1]),
                    ],
                ),
                "Set" if args.len() == 1 => {
                    Type::Named("Set".to_string(), vec![self.resolve_type_expr(&args[0])])
                }
                _ => {
                    // Check for type alias resolution (P1.9)
                    if let Some(type_def) = self.env.lookup_type(name).cloned() {
                        self.resolve_type_expr(&type_def.value)
                    } else {
                        Type::Named(
                            name.clone(),
                            args.iter().map(|a| self.resolve_type_expr(a)).collect(),
                        )
                    }
                }
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

/// Collect variable bindings from a pattern into a type environment
fn collect_pattern_bindings(pattern: &Pattern, env: &mut TypeEnv) {
    match pattern {
        Pattern::Ident { name, .. } => {
            env.define(name.clone(), Type::Unknown);
        }
        Pattern::Variant { fields, .. } => {
            for inner in fields {
                collect_pattern_bindings(inner, env);
            }
        }
        Pattern::Record { fields, .. } => {
            for (_, pat) in fields {
                collect_pattern_bindings(pat, env);
            }
        }
        Pattern::Tuple { elements, .. } => {
            for inner in elements {
                collect_pattern_bindings(inner, env);
            }
        }
        Pattern::Wildcard { .. }
        | Pattern::IntLit { .. }
        | Pattern::FloatLit { .. }
        | Pattern::BoolLit { .. }
        | Pattern::TextLit { .. } => {}
    }
}

/// Check if a pattern is a catch-all (wildcard or plain identifier binding)
fn is_catch_all(pattern: &Pattern) -> bool {
    matches!(pattern, Pattern::Wildcard { .. } | Pattern::Ident { .. })
}

/// Check exhaustiveness of pattern matching (public API)
pub fn check_exhaustiveness(
    scrutinee_type: &Type,
    patterns: &[Pattern],
) -> Result<(), Vec<String>> {
    // Check for catch-all
    if patterns.iter().any(is_catch_all) {
        return Ok(());
    }

    let missing = match scrutinee_type {
        Type::Option(_) => {
            let mut covered = HashSet::new();
            for pat in patterns {
                if let Pattern::Variant { name, .. } = pat {
                    covered.insert(name.as_str());
                }
            }
            let mut missing = Vec::new();
            if !covered.contains("Some") {
                missing.push("Some(_)".to_string());
            }
            if !covered.contains("None") {
                missing.push("None".to_string());
            }
            missing
        }
        Type::Result(_, _) => {
            let mut covered = HashSet::new();
            for pat in patterns {
                if let Pattern::Variant { name, .. } = pat {
                    covered.insert(name.as_str());
                }
            }
            let mut missing = Vec::new();
            if !covered.contains("Ok") {
                missing.push("Ok(_)".to_string());
            }
            if !covered.contains("Err") {
                missing.push("Err(_)".to_string());
            }
            missing
        }
        Type::Bool => {
            let mut has_true = false;
            let mut has_false = false;
            for pat in patterns {
                if let Pattern::BoolLit { value, .. } = pat {
                    if *value {
                        has_true = true;
                    } else {
                        has_false = true;
                    }
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
        _ => return Ok(()),
    };

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Lexer, Parser, SourceFile};
    use std::path::PathBuf;

    fn parse_module(source: &str) -> Module {
        let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
        let lexer = Lexer::new(&source_file);
        let mut parser = Parser::new(lexer, source_file.clone());
        parser.parse_module().expect("parse failed")
    }

    fn check_module(source: &str) -> Result<(), DiagnosticBag> {
        let module = parse_module(source);
        let mut checker = TypeChecker::new();
        checker.check_module(&module)
    }

    /// Check a module and return all diagnostics (errors + warnings),
    /// used for testing lint rules that produce warnings.
    fn check_module_all_diags(source: &str) -> DiagnosticBag {
        let module = parse_module(source);
        let mut checker = TypeChecker::new();
        let _ = checker.check_module(&module);
        checker.diagnostics().clone()
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

    // C2: Exhaustive match checking tests

    #[test]
    fn test_exhaustive_option_match() {
        let source = r#"
module example

fn main() -> Int {
  let x: Option[Int] = Some(42)
  match x {
    Some(n) => n
    None => 0
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "exhaustive Option match should pass");
    }

    #[test]
    fn test_non_exhaustive_option_missing_none() {
        let source = r#"
module example

fn main() -> Int {
  let x: Option[Int] = Some(42)
  match x {
    Some(n) => n
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_err(), "missing None should be an error");
        let diags = result.unwrap_err();
        let d = &diags.diagnostics()[0];
        assert_eq!(d.code, "E1004");
        assert!(
            d.message.contains("None"),
            "error should mention missing None"
        );
    }

    #[test]
    fn test_non_exhaustive_option_missing_some() {
        let source = r#"
module example

fn main() -> Int {
  let x: Option[Int] = Some(42)
  match x {
    None => 0
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_err(), "missing Some should be an error");
        let diags = result.unwrap_err();
        let d = &diags.diagnostics()[0];
        assert_eq!(d.code, "E1004");
        assert!(
            d.message.contains("Some"),
            "error should mention missing Some"
        );
    }

    #[test]
    fn test_exhaustive_result_match() {
        let source = r#"
module example

fn main() -> Int {
  let x: Result[Int, Text] = Ok(42)
  match x {
    Ok(n) => n
    Err(e) => 0
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "exhaustive Result match should pass");
    }

    #[test]
    fn test_non_exhaustive_result_missing_err() {
        let source = r#"
module example

fn main() -> Int {
  let x: Result[Int, Text] = Ok(42)
  match x {
    Ok(n) => n
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_err(), "missing Err should be an error");
        let diags = result.unwrap_err();
        let d = &diags.diagnostics()[0];
        assert_eq!(d.code, "E1004");
        assert!(
            d.message.contains("Err"),
            "error should mention missing Err"
        );
    }

    #[test]
    fn test_exhaustive_bool_match() {
        let source = r#"
module example

fn main() -> Int {
  match true {
    true => 1
    false => 0
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "exhaustive Bool match should pass");
    }

    #[test]
    fn test_non_exhaustive_bool_match() {
        let source = r#"
module example

fn main() -> Int {
  match true {
    true => 1
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_err(), "missing false should be an error");
        let diags = result.unwrap_err();
        let d = &diags.diagnostics()[0];
        assert_eq!(d.code, "E1004");
        assert!(d.message.contains("false"));
    }

    #[test]
    fn test_wildcard_covers_all() {
        let source = r#"
module example

fn main() -> Int {
  let x: Option[Int] = Some(42)
  match x {
    Some(n) => n
    _ => 0
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "wildcard should cover remaining patterns");
    }

    #[test]
    fn test_ident_covers_all() {
        let source = r#"
module example

fn main() -> Int {
  match 42 {
    x => x
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "identifier pattern should cover all");
    }

    #[test]
    fn test_non_exhaustive_has_suggestion() {
        let source = r#"
module example

fn main() -> Int {
  let x: Option[Int] = Some(42)
  match x {
    Some(n) => n
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_err());
        let diags = result.unwrap_err();
        let d = &diags.diagnostics()[0];
        assert!(
            !d.suggestions.is_empty(),
            "error should include a suggestion"
        );
        assert!(
            d.suggestions[0].title.contains("None"),
            "suggestion should mention the missing case"
        );
    }

    // C4: Effect enforcement tests

    #[test]
    fn test_effect_declared_correctly() {
        let source = r#"
module example

fn greet() effects(Console) {
  Console.println("hello")
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "declared effect should pass");
    }

    #[test]
    fn test_effect_not_declared() {
        let source = r#"
module example

fn greet() {
  Console.println("hello")
}
"#;
        let result = check_module(source);
        assert!(result.is_err(), "undeclared effect should be an error");
        let diags = result.unwrap_err();
        let d = &diags.diagnostics()[0];
        assert_eq!(d.code, "E2001");
        assert!(d.message.contains("Console"));
    }

    #[test]
    fn test_effect_enforcement_multiple_effects() {
        let source = r#"
module example

fn do_stuff() effects(Console, Fs) {
  Console.println("reading file")
  Fs.read("test.txt")
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "all effects declared should pass");
    }

    #[test]
    fn test_effect_enforcement_missing_one_effect() {
        let source = r#"
module example

fn do_stuff() effects(Console) {
  Console.println("reading file")
  Fs.read("test.txt")
}
"#;
        let result = check_module(source);
        assert!(result.is_err(), "missing Fs effect should be an error");
        let diags = result.unwrap_err();
        assert!(
            diags
                .diagnostics()
                .iter()
                .any(|d| d.code == "E2001" && d.message.contains("Fs")),
            "should report missing Fs effect"
        );
    }

    #[test]
    fn test_pure_function_no_effects() {
        let source = r#"
module example

fn add(a: Int, b: Int) -> Int {
  a + b
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "pure function should pass");
    }

    // H5: Type inference for let bindings
    #[test]
    fn test_let_without_type_annotation() {
        let source = r#"
module example

fn main() -> Int {
  let x = 42
  let y = x + 8
  y
}
"#;
        let result = check_module(source);
        assert!(
            result.is_ok(),
            "let without type annotation should pass type checking"
        );
    }

    // C3: Error suggestion tests

    #[test]
    fn test_effect_error_has_suggestion() {
        let source = r#"
module example

fn greet() {
  Console.println("hello")
}
"#;
        let result = check_module(source);
        assert!(result.is_err());
        let diags = result.unwrap_err();
        let d = &diags.diagnostics()[0];
        assert!(
            !d.suggestions.is_empty(),
            "effect error should include a suggestion"
        );
        assert!(
            d.suggestions[0].title.contains("effects"),
            "suggestion should mention adding effects declaration"
        );
    }

    // =========================================================================
    // Lint tests (W0001-W0007)
    // =========================================================================

    // W0001: Unused variable

    #[test]
    fn test_lint_unused_variable() {
        let source = r#"
module example

fn main() -> Int {
  let unused = 42
  0
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0001")
            .collect();
        assert!(!warnings.is_empty(), "should warn about unused variable");
        assert!(warnings[0].message.contains("unused"));
    }

    #[test]
    fn test_lint_used_variable_no_warning() {
        let source = r#"
module example

fn add(a: Int, b: Int) -> Int {
  a + b
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0001")
            .collect();
        assert!(
            warnings.is_empty(),
            "used variables should not generate warnings"
        );
    }

    #[test]
    fn test_lint_underscore_prefix_suppresses_unused() {
        let source = r#"
module example

fn main() -> Int {
  let _ignored = 42
  0
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0001")
            .collect();
        assert!(
            warnings.is_empty(),
            "underscore-prefixed variables should not warn"
        );
    }

    // W0003: Unreachable code

    #[test]
    fn test_lint_unreachable_code_after_return() {
        let source = r#"
module example

fn main() -> Int {
  return 1
  let x = 2
  x
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0003")
            .collect();
        assert!(
            !warnings.is_empty(),
            "should warn about unreachable code after return"
        );
    }

    #[test]
    fn test_lint_no_unreachable_code() {
        let source = r#"
module example

fn main() -> Int {
  let x = 2
  x
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0003")
            .collect();
        assert!(warnings.is_empty(), "no unreachable code warning expected");
    }

    // W0005: Wildcard match on known type

    #[test]
    fn test_lint_wildcard_match_on_option() {
        let source = r#"
module example

fn main() -> Int {
  let x: Option[Int] = Some(42)
  match x {
    Some(n) => n
    _ => 0
  }
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0005")
            .collect();
        assert!(
            !warnings.is_empty(),
            "should warn about wildcard on Option type"
        );
    }

    // W0006: Shadowed binding

    #[test]
    fn test_lint_shadowed_binding() {
        let source = r#"
module example

fn main() -> Int {
  let x = 1
  let x = 2
  x
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0006")
            .collect();
        assert!(!warnings.is_empty(), "should warn about shadowed binding");
        assert!(warnings[0].message.contains("shadows"));
    }

    #[test]
    fn test_lint_no_shadowing_different_names() {
        let source = r#"
module example

fn main() -> Int {
  let x = 1
  let y = 2
  x + y
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0006")
            .collect();
        assert!(
            warnings.is_empty(),
            "different names should not trigger shadowing warning"
        );
    }

    // W0002: Unused import

    #[test]
    fn test_lint_unused_import() {
        let source = r#"
module example

import std.math

fn main() -> Int {
  42
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0002")
            .collect();
        assert!(!warnings.is_empty(), "should warn about unused import");
        assert!(warnings[0].message.contains("math"));
    }

    // Integration: lint warnings don't block compilation

    #[test]
    fn test_lint_warnings_dont_cause_errors() {
        let source = r#"
module example

fn main() -> Int {
  let unused = 42
  let _ok = 1
  _ok
}
"#;
        let result = check_module(source);
        assert!(
            result.is_ok(),
            "lint warnings should not cause check_module to return Err"
        );
        let diags = check_module_all_diags(source);
        assert!(diags.has_warnings(), "should have warnings");
        assert!(!diags.has_errors(), "should not have errors");
    }
}
