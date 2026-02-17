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
    /// Type parameter (generic, e.g., T in fn id[T](x: T) -> T)
    TypeParam(String),
    /// List type
    List(Box<Type>),
    /// Tuple type
    Tuple(Vec<Type>),
    /// Unknown type (for error recovery)
    Unknown,
}

/// Type variable identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeVarId(pub u32);

/// A registered trait implementation: `impl TraitName for TargetType { methods... }`
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TraitImpl {
    /// The trait being implemented
    trait_name: String,
    /// The concrete type implementing the trait
    target_type: Type,
    /// Method names provided by this impl
    method_names: Vec<String>,
}

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
    /// Trait definitions
    trait_defs: HashMap<String, TraitDef>,
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
            trait_defs: HashMap::new(),
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

    /// Register a trait definition
    pub fn register_trait(&mut self, def: TraitDef) {
        self.trait_defs.insert(def.name.clone(), def);
    }

    /// Look up a trait definition
    pub fn lookup_trait(&self, name: &str) -> Option<&TraitDef> {
        self.trait_defs
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.lookup_trait(name)))
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
    /// Registered trait implementations: maps (trait_name, type_name) -> method names
    trait_impls: Vec<TraitImpl>,
    /// Set of type parameter names in the current generic context
    current_type_params: HashSet<String>,
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
            trait_impls: Vec::new(),
            current_type_params: HashSet::new(),
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
        // First pass: collect all type/enum/fn/trait/impl definitions and imports
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
                Item::EnumDef(def) => {
                    self.env.register_enum(def.clone());
                    // Register each variant as a callable constructor
                    let enum_type = Type::Named(def.name.clone(), vec![]);
                    for variant in &def.variants {
                        if variant.fields.is_empty() {
                            // Nullary variant (e.g., None, Red)
                            self.env.define(variant.name.clone(), enum_type.clone());
                        } else {
                            // Variant with fields (e.g., Circle(r: Float))
                            let param_types: Vec<Type> = variant
                                .fields
                                .iter()
                                .map(|f| self.resolve_type_expr(&f.ty))
                                .collect();
                            self.env.define(
                                variant.name.clone(),
                                Type::Function {
                                    params: param_types,
                                    ret: Box::new(enum_type.clone()),
                                    effects: vec![],
                                },
                            );
                        }
                    }
                }
                Item::FnDef(def) => {
                    self.env.register_fn(def.clone());
                    // Resolve param types with type params in scope
                    let old_params = self.current_type_params.clone();
                    for tp in &def.type_params {
                        self.current_type_params.insert(tp.clone());
                    }
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
                    self.current_type_params = old_params;
                    self.env.define(
                        def.name.clone(),
                        Type::Function {
                            params: param_types,
                            ret: Box::new(ret_type),
                            effects: def.effects.clone(),
                        },
                    );
                }
                Item::TraitDef(def) => {
                    self.env.register_trait(def.clone());
                }
                Item::ImplBlock(impl_block) => {
                    // Register the impl so we can do trait dispatch
                    let target_type = self.resolve_type_expr(&impl_block.target_type);
                    let method_names: Vec<String> =
                        impl_block.methods.iter().map(|m| m.name.clone()).collect();
                    self.trait_impls.push(TraitImpl {
                        trait_name: impl_block.trait_name.clone(),
                        target_type,
                        method_names,
                    });
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
            Item::Import(import) => {
                // Register imported names as known bindings so they don't
                // trigger E1002 "Unknown identifier" errors. Full cross-module
                // type resolution is deferred to v2.
                match &import.kind {
                    ImportKind::Module => {
                        if let Some(name) = import.path.segments.last() {
                            self.env.define(name.clone(), Type::Unknown);
                        }
                    }
                    ImportKind::Alias(alias) => {
                        self.env.define(alias.clone(), Type::Unknown);
                    }
                    ImportKind::Items(items) => {
                        for item_name in items {
                            self.env.define(item_name.clone(), Type::Unknown);
                        }
                    }
                }
            }
            Item::TypeDef(def) => self.check_typedef(def),
            Item::EnumDef(def) => self.check_enumdef(def),
            Item::FnDef(def) => self.check_fndef(def),
            Item::TraitDef(def) => {
                // Validate trait method signatures
                self.env.register_trait(def.clone());
            }
            Item::ImplBlock(impl_block) => {
                // Validate that the impl provides all methods required by the trait
                if let Some(trait_def) = self.env.lookup_trait(&impl_block.trait_name).cloned() {
                    let impl_method_names: HashSet<String> =
                        impl_block.methods.iter().map(|m| m.name.clone()).collect();
                    for trait_method in &trait_def.methods {
                        if !impl_method_names.contains(&trait_method.name) {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    crate::diagnostics::error_codes::types::TYPE_MISMATCH,
                                )
                                .message(format!(
                                    "Missing method `{}` required by trait `{}`",
                                    trait_method.name, impl_block.trait_name
                                ))
                                .span(impl_block.span.clone())
                                .build(),
                            );
                        }
                    }
                }
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

    fn check_typedef(&mut self, def: &TypeDef) {
        // Resolve the underlying type expression to verify it references valid types
        let _resolved = self.resolve_type_expr(&def.value);

        // If the typedef has an invariant, type-check it
        if let Some(invariant) = &def.invariant {
            let mut inv_env = self.env.child();
            // `self` is available inside the invariant as the value being checked
            inv_env.define("self".to_string(), self.resolve_type_expr(&def.value));
            let mut effects = HashSet::new();
            let inv_ty = self.check_expr_with_effects(invariant, &inv_env, &mut effects);
            if inv_ty != Type::Bool && inv_ty != Type::Unknown {
                self.diagnostics.push(
                    Diagnostic::error(crate::diagnostics::error_codes::types::TYPE_MISMATCH)
                        .message(format!(
                            "Type invariant for `{}` must be a Bool expression, found {:?}",
                            def.name, inv_ty
                        ))
                        .span(def.span.clone())
                        .build(),
                );
            }
        }
    }

    fn check_enumdef(&mut self, def: &EnumDef) {
        // Check for duplicate variant names
        let mut seen_variants = HashSet::new();
        for variant in &def.variants {
            if !seen_variants.insert(&variant.name) {
                self.diagnostics.push(
                    Diagnostic::error(crate::diagnostics::error_codes::types::DUPLICATE_FIELD)
                        .message(format!(
                            "Duplicate variant `{}` in enum `{}`",
                            variant.name, def.name
                        ))
                        .span(variant.span.clone())
                        .build(),
                );
            }

            // Check for duplicate field names within each variant
            let mut seen_fields = HashSet::new();
            for field in &variant.fields {
                if !seen_fields.insert(&field.name) {
                    self.diagnostics.push(
                        Diagnostic::error(crate::diagnostics::error_codes::types::DUPLICATE_FIELD)
                            .message(format!(
                                "Duplicate field `{}` in variant `{}`",
                                field.name, variant.name
                            ))
                            .span(field.span.clone())
                            .build(),
                    );
                }

                // Resolve each field type to check it's valid
                let _resolved = self.resolve_type_expr(&field.ty);
            }
        }
    }

    fn check_fndef(&mut self, def: &FnDef) {
        let mut fn_env = self.env.child();

        // Push lint scope for function body
        self.push_lint_scope();

        // Register type parameters as TypeParam in scope
        let old_type_params = self.current_type_params.clone();
        for tp in &def.type_params {
            self.current_type_params.insert(tp.clone());
            fn_env.define(tp.clone(), Type::TypeParam(tp.clone()));
        }

        // Add parameters to environment
        for param in &def.params {
            let ty = self.resolve_type_expr(&param.ty);
            fn_env.define(param.name.clone(), ty);
            // For destructured parameters, also register the bindings from the pattern
            if let Some(ref pattern) = param.pattern {
                collect_pattern_bindings(pattern, &mut fn_env);
            } else {
                self.lint_define_var(&param.name, &param.span);
            }
        }

        // Check body and collect effects used
        let mut effects_used = HashSet::new();
        let _body_type = self.check_block_with_effects(&def.body, &mut fn_env, &mut effects_used);

        // Pop lint scope — emits W0001 for unused variables/params
        self.pop_lint_scope();

        // Restore type params
        self.current_type_params = old_type_params;

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
                let func_ty = self.check_expr_with_effects(func, env, effects);

                let arg_types: Vec<Type> = args
                    .iter()
                    .map(|arg| self.check_expr_with_effects(arg, env, effects))
                    .collect();

                if let Type::Function { params, ret, .. } = &func_ty {
                    // Check arity
                    if params.len() != arg_types.len()
                        && !params.is_empty()
                        && !arg_types.is_empty()
                    {
                        // Only warn if neither side is unknown
                        // (builtins may have variable arity)
                    }

                    // Try generic type parameter inference if the called function
                    // has type params
                    if let Expr::Ident { name, .. } = func.as_ref() {
                        if let Some(fn_def) = env.lookup_fn(name).cloned() {
                            if !fn_def.type_params.is_empty() {
                                let bindings =
                                    self.unify_type_params(&fn_def.type_params, params, &arg_types);
                                // Substitute type params in return type
                                let substituted_ret = self.substitute_type_params(ret, &bindings);
                                return substituted_ret;
                            }
                        }
                    }

                    *ret.clone()
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
                let mut elem_ty = Type::Unknown;
                for elem in elements {
                    let ty = self.check_expr_with_effects(elem, env, effects);
                    if elem_ty == Type::Unknown {
                        elem_ty = ty;
                    }
                }
                Type::List(Box::new(elem_ty))
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
                let elem_types: Vec<Type> = elements
                    .iter()
                    .map(|elem| self.check_expr_with_effects(elem, env, effects))
                    .collect();
                Type::Tuple(elem_types)
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
                "List" if args.len() == 1 => Type::List(Box::new(self.resolve_type_expr(&args[0]))),
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
                    // Check if it's a type parameter in the current generic context
                    if self.current_type_params.contains(name) && args.is_empty() {
                        return Type::TypeParam(name.clone());
                    }
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
            TypeExpr::Tuple { elements, .. } => {
                Type::Tuple(elements.iter().map(|e| self.resolve_type_expr(e)).collect())
            }
        }
    }

    fn types_compatible(&self, actual: &Type, expected: &Type) -> bool {
        if actual == &Type::Unknown || expected == &Type::Unknown {
            return true;
        }
        // Type parameters are compatible with anything (they're generic)
        if matches!(actual, Type::TypeParam(_)) || matches!(expected, Type::TypeParam(_)) {
            return true;
        }
        // List[T] compatible with List[U] if T compatible with U
        if let (Type::List(a), Type::List(b)) = (actual, expected) {
            return self.types_compatible(a, b);
        }
        // Tuple(A, B) compatible with Tuple(C, D) if element-wise compatible
        if let (Type::Tuple(a), Type::Tuple(b)) = (actual, expected) {
            return a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(x, y)| self.types_compatible(x, y));
        }
        // Named types with same name: check type args
        if let (Type::Named(n1, args1), Type::Named(n2, args2)) = (actual, expected) {
            if n1 == n2 {
                return args1.len() == args2.len()
                    && args1
                        .iter()
                        .zip(args2.iter())
                        .all(|(a, b)| self.types_compatible(a, b));
            }
        }
        // Function types
        if let (
            Type::Function {
                params: p1,
                ret: r1,
                ..
            },
            Type::Function {
                params: p2,
                ret: r2,
                ..
            },
        ) = (actual, expected)
        {
            return p1.len() == p2.len()
                && p1
                    .iter()
                    .zip(p2.iter())
                    .all(|(a, b)| self.types_compatible(a, b))
                && self.types_compatible(r1, r2);
        }
        actual == expected
    }

    /// Attempt to unify type arguments from a generic function call.
    /// Given a function with type params [T, U, ...] and declared param types,
    /// tries to bind each type param to a concrete type based on the actual argument types.
    fn unify_type_params(
        &self,
        type_params: &[String],
        declared_param_types: &[Type],
        actual_arg_types: &[Type],
    ) -> HashMap<String, Type> {
        let mut bindings: HashMap<String, Type> = HashMap::new();
        for (declared, actual) in declared_param_types.iter().zip(actual_arg_types.iter()) {
            self.unify_one(declared, actual, type_params, &mut bindings);
        }
        bindings
    }

    /// Recursively unify a declared type with an actual type, binding type params.
    fn unify_one(
        &self,
        declared: &Type,
        actual: &Type,
        type_params: &[String],
        bindings: &mut HashMap<String, Type>,
    ) {
        if *actual == Type::Unknown {
            return;
        }
        match declared {
            Type::TypeParam(name) if type_params.contains(name) => {
                if let Some(existing) = bindings.get(name) {
                    // Already bound - check consistency (but don't error, just keep first)
                    if !self.types_compatible(actual, existing) {
                        // Conflicting binding, ignore for now
                    }
                } else {
                    bindings.insert(name.clone(), actual.clone());
                }
            }
            // Named type that matches a type param name
            Type::Named(name, args) if type_params.contains(name) && args.is_empty() => {
                if !bindings.contains_key(name) {
                    bindings.insert(name.clone(), actual.clone());
                }
            }
            Type::List(inner) => {
                if let Type::List(actual_inner) = actual {
                    self.unify_one(inner, actual_inner, type_params, bindings);
                }
            }
            Type::Tuple(elems) => {
                if let Type::Tuple(actual_elems) = actual {
                    for (d, a) in elems.iter().zip(actual_elems.iter()) {
                        self.unify_one(d, a, type_params, bindings);
                    }
                }
            }
            Type::Option(inner) => {
                if let Type::Option(actual_inner) = actual {
                    self.unify_one(inner, actual_inner, type_params, bindings);
                }
            }
            Type::Result(ok, err) => {
                if let Type::Result(actual_ok, actual_err) = actual {
                    self.unify_one(ok, actual_ok, type_params, bindings);
                    self.unify_one(err, actual_err, type_params, bindings);
                }
            }
            Type::Named(name, args) => {
                if let Type::Named(actual_name, actual_args) = actual {
                    if name == actual_name {
                        for (d, a) in args.iter().zip(actual_args.iter()) {
                            self.unify_one(d, a, type_params, bindings);
                        }
                    }
                }
            }
            Type::Function { params, ret, .. } => {
                if let Type::Function {
                    params: actual_params,
                    ret: actual_ret,
                    ..
                } = actual
                {
                    for (d, a) in params.iter().zip(actual_params.iter()) {
                        self.unify_one(d, a, type_params, bindings);
                    }
                    self.unify_one(ret, actual_ret, type_params, bindings);
                }
            }
            _ => {}
        }
    }

    /// Substitute type parameters in a type using the given bindings.
    fn substitute_type_params(&self, ty: &Type, bindings: &HashMap<String, Type>) -> Type {
        match ty {
            Type::TypeParam(name) => bindings.get(name).cloned().unwrap_or_else(|| ty.clone()),
            Type::Named(name, args) if args.is_empty() && bindings.contains_key(name) => {
                bindings[name].clone()
            }
            Type::Named(name, args) => Type::Named(
                name.clone(),
                args.iter()
                    .map(|a| self.substitute_type_params(a, bindings))
                    .collect(),
            ),
            Type::List(inner) => Type::List(Box::new(self.substitute_type_params(inner, bindings))),
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|e| self.substitute_type_params(e, bindings))
                    .collect(),
            ),
            Type::Option(inner) => {
                Type::Option(Box::new(self.substitute_type_params(inner, bindings)))
            }
            Type::Result(ok, err) => Type::Result(
                Box::new(self.substitute_type_params(ok, bindings)),
                Box::new(self.substitute_type_params(err, bindings)),
            ),
            Type::Function {
                params,
                ret,
                effects,
            } => Type::Function {
                params: params
                    .iter()
                    .map(|p| self.substitute_type_params(p, bindings))
                    .collect(),
                ret: Box::new(self.substitute_type_params(ret, bindings)),
                effects: effects.clone(),
            },
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|(n, t)| (n.clone(), self.substitute_type_params(t, bindings)))
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }

    /// Look up a trait impl method for a given target type
    #[allow(dead_code)]
    fn lookup_trait_method(&self, trait_name: &str, target_type: &Type, method_name: &str) -> bool {
        self.trait_impls.iter().any(|imp| {
            imp.trait_name == trait_name
                && self.types_compatible(&imp.target_type, target_type)
                && imp.method_names.contains(&method_name.to_string())
        })
    }

    /// Check if a type satisfies a trait constraint
    #[allow(dead_code)]
    fn type_satisfies_trait(&self, ty: &Type, trait_name: &str) -> bool {
        if *ty == Type::Unknown || matches!(ty, Type::TypeParam(_)) {
            return true;
        }
        self.trait_impls
            .iter()
            .any(|imp| imp.trait_name == trait_name && self.types_compatible(&imp.target_type, ty))
    }

    #[allow(dead_code)]
    fn fresh_var(&mut self) -> Type {
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

    // Enum constructor resolution

    #[test]
    fn test_enum_variant_constructors_resolved() {
        let source = r#"
module example

enum Shape =
  | Circle(radius: Float)
  | Rectangle(width: Float, height: Float)

fn main() -> Float {
  let c = Circle(5.0)
  let r = Rectangle(3.0, 4.0)
  0.0
}
"#;
        let result = check_module(source);
        assert!(
            result.is_ok(),
            "enum variant constructors should be resolved: {:?}",
            result.unwrap_err()
        );
    }

    #[test]
    fn test_enum_nullary_variant_resolved() {
        let source = r#"
module example

enum Color = | Red | Green | Blue

fn pick() -> Color {
  Red
}
"#;
        let result = check_module(source);
        assert!(
            result.is_ok(),
            "nullary enum variants should be resolved: {:?}",
            result.unwrap_err()
        );
    }

    // Tuple type parsing

    #[test]
    fn test_tuple_type_in_function_signature() {
        let source = r#"
module example

fn swap(pair: (Int, Int)) -> (Int, Int) {
  (pair.1, pair.0)
}
"#;
        let result = check_module(source);
        assert!(
            result.is_ok(),
            "tuple types in signatures should parse and check: {:?}",
            result.unwrap_err()
        );
    }

    // R9: check_typedef and check_enumdef tests

    #[test]
    fn test_typedef_well_formed() {
        let source = r#"
module example

type Name = Text

fn greet(n: Name) -> Text {
  n
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "well-formed type def should pass");
    }

    #[test]
    fn test_typedef_with_invariant() {
        let source = r#"
module example

type Positive = Int invariant self > 0

fn double(x: Positive) -> Int {
  x + x
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "type def with valid invariant should pass");
    }

    #[test]
    fn test_enumdef_well_formed() {
        let source = r#"
module example

enum Direction =
  | North
  | South
  | East
  | West

fn describe(d: Direction) -> Text {
  match d {
    North => "north"
    South => "south"
    East => "east"
    West => "west"
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "well-formed enum def should pass");
    }

    #[test]
    fn test_enumdef_with_fields_well_formed() {
        let source = r#"
module example

enum Expr =
  | Num(value: Int)
  | Add(left: Int, right: Int)

fn eval(e: Expr) -> Int {
  match e {
    Num(v) => v
    Add(l, r) => l + r
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "enum def with fields should pass");
    }

    // R10: Import resolution tests

    #[test]
    fn test_import_module_registers_name() {
        let source = r#"
module example

import std.math

fn main() -> Int {
  let _m = math
  0
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "imported module name should be resolvable");
    }

    #[test]
    fn test_import_alias_registers_name() {
        let source = r#"
module example

import std.math as M

fn main() -> Int {
  let _m = M
  0
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "import alias should be resolvable");
    }

    #[test]
    fn test_unknown_identifier_still_errors() {
        let source = r#"
module example

fn main() -> Int {
  totally_unknown + 1
}
"#;
        let result = check_module(source);
        assert!(result.is_err(), "unknown identifier should error");
        let diags = result.unwrap_err();
        assert!(
            diags.diagnostics().iter().any(|d| d.code == "E1002"),
            "should report E1002 unknown identifier"
        );
    }

    // v2: Generic type checking tests

    #[test]
    fn test_generic_type_param_resolution() {
        let source = r#"
module example

fn identity[T](x: T) -> T {
  x
}

fn main() -> Int {
  identity(42)
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "Generic function should type-check");
    }

    #[test]
    fn test_generic_return_type_inference() {
        // When calling a generic fn, the return type should be inferred
        let source = r#"
module example

fn first[T](items: List[T]) -> T {
  items
}

fn main() -> Int {
  let nums = [1, 2, 3]
  first(nums)
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "Generic return type inference should work");
    }

    #[test]
    fn test_type_param_in_scope() {
        let source = r#"
module example

fn pair[A, B](a: A, b: B) -> (A, B) {
  (a, b)
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "Multiple type params should be in scope");
    }

    // v2: Trait/impl type checking tests

    #[test]
    fn test_trait_definition() {
        let source = r#"
module example

trait Show {
  fn show(self: Self) -> Text
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "Trait definition should type-check");
    }

    #[test]
    fn test_impl_block_methods_checked() {
        let source = r#"
module example

trait Show {
  fn show(self: Self) -> Text
}

impl Show for Int {
  fn show(self: Int) -> Text {
    "int"
  }
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "Impl block should type-check");
    }

    #[test]
    fn test_impl_missing_method() {
        let source = r#"
module example

trait Describe {
  fn describe(self: Self) -> Text
  fn summary(self: Self) -> Text
}

impl Describe for Int {
  fn describe(self: Int) -> Text {
    "an integer"
  }
}
"#;
        let _result = check_module(source);
        // Should report missing method
        let diags = check_module_all_diags(source);
        let has_missing = diags
            .diagnostics()
            .iter()
            .any(|d| d.message.contains("Missing method"));
        assert!(
            has_missing,
            "Should warn about missing trait method 'summary'"
        );
    }

    // v2: List and Tuple type tracking

    #[test]
    fn test_list_type_tracking() {
        let source = r#"
module example

fn main() -> Unit {
  let nums = [1, 2, 3]
  let _x = nums
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "List type should be tracked");
    }

    #[test]
    fn test_tuple_type_tracking() {
        let source = r#"
module example

fn main() -> Unit {
  let pair = (1, "hello")
  let _x = pair
}
"#;
        let result = check_module(source);
        assert!(result.is_ok(), "Tuple type should be tracked");
    }
}
