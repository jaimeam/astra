//! Type checker for Astra
//!
//! Implements type checking, inference, exhaustiveness checking, effect enforcement,
//! and lint checks (W0001-W0007).

use crate::diagnostics::{Diagnostic, DiagnosticBag, Edit, Note, Span, Suggestion};
use crate::parser::ast::*;
use std::collections::{HashMap, HashSet};

/// Format a Type as a human-readable string for suggestions.
fn format_type(ty: &Type) -> String {
    match ty {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::Float => "Float".to_string(),
        Type::Bool => "Bool".to_string(),
        Type::Text => "Text".to_string(),
        Type::Json => "Json".to_string(),
        Type::Option(inner) => format!("Option[{}]", format_type(inner)),
        Type::Result(ok, err) => format!("Result[{}, {}]", format_type(ok), format_type(err)),
        Type::List(inner) => format!("List[{}]", format_type(inner)),
        Type::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(format_type).collect();
            format!("({})", parts.join(", "))
        }
        Type::Record(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_type(v)))
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
        Type::Function {
            params,
            ret,
            effects,
        } => {
            let param_str: Vec<String> = params.iter().map(format_type).collect();
            let fx = if effects.is_empty() {
                String::new()
            } else {
                format!(" effects({})", effects.join(", "))
            };
            format!("({}) -> {}{}", param_str.join(", "), format_type(ret), fx)
        }
        Type::Named(name, args) => {
            if args.is_empty() {
                name.clone()
            } else {
                let arg_str: Vec<String> = args.iter().map(format_type).collect();
                format!("{}[{}]", name, arg_str.join(", "))
            }
        }
        Type::TypeParam(name) => name.clone(),
        Type::Var(id) => format!("?{}", id.0),
        Type::Unknown => "Unknown".to_string(),
    }
}

/// Get the span of a TypeExpr.
fn type_expr_span(ty: &TypeExpr) -> Span {
    match ty {
        TypeExpr::Named { span, .. }
        | TypeExpr::Record { span, .. }
        | TypeExpr::Function { span, .. }
        | TypeExpr::Tuple { span, .. } => span.clone(),
    }
}

/// Extract a simple name from a TypeExpr (for trait impl tracking).
fn type_expr_to_name(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named { name, .. } => name.clone(),
        TypeExpr::Tuple { .. } => "Tuple".to_string(),
        TypeExpr::Record { .. } => "Record".to_string(),
        TypeExpr::Function { .. } => "Function".to_string(),
    }
}

/// Compute Levenshtein edit distance between two strings.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();

    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate().take(m + 1) {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate().take(n + 1) {
        *val = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}

/// Find the most similar name in the environment to the given unknown name.
/// Returns `Some(name)` if a name within edit distance 3 exists, `None` otherwise.
fn find_similar_name(name: &str, env: &TypeEnv) -> Option<String> {
    let mut best: Option<(String, usize)> = None;
    let max_distance = 3.min(name.len().div_ceil(2));

    for candidate in env.bindings.keys() {
        if candidate == name {
            continue;
        }
        let dist = edit_distance(name, candidate);
        if dist <= max_distance && best.as_ref().is_none_or(|(_, d)| dist < *d) {
            best = Some((candidate.clone(), dist));
        }
    }

    // Also check parent environments
    if let Some(ref parent) = env.parent {
        if let Some(parent_suggestion) = find_similar_name(name, parent) {
            let parent_dist = edit_distance(name, &parent_suggestion);
            if best.as_ref().is_none_or(|(_, d)| parent_dist < *d) {
                return Some(parent_suggestion);
            }
        }
    }

    best.map(|(name, _)| name)
}

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
    /// Type parameter (generic, e.g., `T` in `fn id\[T\](x: T) -> T`)
    TypeParam(String),
    /// List type
    List(Box<Type>),
    /// Tuple type
    Tuple(Vec<Type>),
    /// JSON dynamic type (represents any JSON-compatible value)
    Json,
    /// Unknown type (for error recovery)
    Unknown,
}

/// Type variable identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeVarId(pub u32);

// =============================================================================
// v1.1: Hindley-Milner type inference with constraint-based unification
// =============================================================================

/// Substitution map for type variable resolution (union-find).
/// Maps type variable IDs to their resolved types.
#[derive(Debug, Clone, Default)]
pub struct Substitution {
    /// Resolved type variable bindings
    bindings: HashMap<TypeVarId, Type>,
    /// Counter for generating fresh type variables
    next_var: u32,
}

impl Substitution {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            next_var: 0,
        }
    }

    /// Generate a fresh type variable
    pub fn fresh_var(&mut self) -> Type {
        let id = TypeVarId(self.next_var);
        self.next_var += 1;
        Type::Var(id)
    }

    /// Look up the current binding for a type variable, following chains
    pub fn resolve(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(id) => {
                if let Some(bound) = self.bindings.get(id) {
                    self.resolve(bound)
                } else {
                    ty.clone()
                }
            }
            _ => ty.clone(),
        }
    }

    /// Apply substitution to a type, resolving all type variables
    pub fn apply(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(id) => {
                if let Some(bound) = self.bindings.get(id) {
                    self.apply(bound)
                } else {
                    ty.clone()
                }
            }
            Type::Option(inner) => Type::Option(Box::new(self.apply(inner))),
            Type::Result(ok, err) => {
                Type::Result(Box::new(self.apply(ok)), Box::new(self.apply(err)))
            }
            Type::List(inner) => Type::List(Box::new(self.apply(inner))),
            Type::Tuple(elems) => Type::Tuple(elems.iter().map(|e| self.apply(e)).collect()),
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|(k, v)| (k.clone(), self.apply(v)))
                    .collect(),
            ),
            Type::Function {
                params,
                ret,
                effects,
            } => Type::Function {
                params: params.iter().map(|p| self.apply(p)).collect(),
                ret: Box::new(self.apply(ret)),
                effects: effects.clone(),
            },
            Type::Named(name, args) => {
                Type::Named(name.clone(), args.iter().map(|a| self.apply(a)).collect())
            }
            _ => ty.clone(),
        }
    }

    /// Unify two types, updating the substitution.
    /// Returns true on success, false if the types are incompatible.
    pub fn unify(&mut self, a: &Type, b: &Type) -> bool {
        let a = self.resolve(a);
        let b = self.resolve(b);

        // Unknown matches anything (error recovery)
        if a == Type::Unknown || b == Type::Unknown {
            return true;
        }

        // TypeParam matches anything (generic parameter)
        if matches!(&a, Type::TypeParam(_)) || matches!(&b, Type::TypeParam(_)) {
            return true;
        }

        // Json is compatible with any JSON-representable type
        if a == Type::Json || b == Type::Json {
            return true;
        }

        // Same type — trivially unifies
        if a == b {
            return true;
        }

        // Bind type variables
        if let Type::Var(id) = &a {
            if !self.occurs_in(*id, &b) {
                self.bindings.insert(*id, b);
                return true;
            }
            return false; // occurs check failure
        }
        if let Type::Var(id) = &b {
            if !self.occurs_in(*id, &a) {
                self.bindings.insert(*id, a);
                return true;
            }
            return false;
        }

        // Structural unification
        match (&a, &b) {
            (Type::Option(a_inner), Type::Option(b_inner)) => self.unify(a_inner, b_inner),
            (Type::Result(a_ok, a_err), Type::Result(b_ok, b_err)) => {
                self.unify(a_ok, b_ok) && self.unify(a_err, b_err)
            }
            (Type::List(a_inner), Type::List(b_inner)) => self.unify(a_inner, b_inner),
            (Type::Tuple(a_elems), Type::Tuple(b_elems)) => {
                a_elems.len() == b_elems.len()
                    && a_elems
                        .iter()
                        .zip(b_elems.iter())
                        .all(|(x, y)| self.unify(x, y))
            }
            (Type::Record(a_fields), Type::Record(b_fields)) => {
                // For records, check that all fields in a exist in b and vice versa
                if a_fields.len() != b_fields.len() {
                    return false;
                }
                for (name, a_ty) in a_fields {
                    if let Some((_, b_ty)) = b_fields.iter().find(|(n, _)| n == name) {
                        if !self.unify(a_ty, b_ty) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                true
            }
            (
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
            ) => {
                p1.len() == p2.len()
                    && p1.iter().zip(p2.iter()).all(|(a, b)| self.unify(a, b))
                    && self.unify(r1, r2)
            }
            (Type::Named(n1, args1), Type::Named(n2, args2)) => {
                n1 == n2
                    && args1.len() == args2.len()
                    && args1
                        .iter()
                        .zip(args2.iter())
                        .all(|(a, b)| self.unify(a, b))
            }
            _ => false,
        }
    }

    /// Occurs check: does type variable `id` appear in `ty`?
    fn occurs_in(&self, id: TypeVarId, ty: &Type) -> bool {
        let ty = self.resolve(ty);
        match &ty {
            Type::Var(other_id) => id == *other_id,
            Type::Option(inner) | Type::List(inner) => self.occurs_in(id, inner),
            Type::Result(ok, err) => self.occurs_in(id, ok) || self.occurs_in(id, err),
            Type::Tuple(elems) => elems.iter().any(|e| self.occurs_in(id, e)),
            Type::Record(fields) => fields.iter().any(|(_, t)| self.occurs_in(id, t)),
            Type::Function { params, ret, .. } => {
                params.iter().any(|p| self.occurs_in(id, p)) || self.occurs_in(id, ret)
            }
            Type::Named(_, args) => args.iter().any(|a| self.occurs_in(id, a)),
            _ => false,
        }
    }

    /// Instantiate a type scheme by replacing TypeParams with fresh type variables.
    /// This is the "inst" operation in HM: when using a polymorphic value,
    /// each type parameter gets a fresh type variable.
    pub fn instantiate(&mut self, ty: &Type, param_map: &mut HashMap<String, Type>) -> Type {
        match ty {
            Type::TypeParam(name) => {
                if let Some(existing) = param_map.get(name) {
                    existing.clone()
                } else {
                    let fresh = self.fresh_var();
                    param_map.insert(name.clone(), fresh.clone());
                    fresh
                }
            }
            Type::Option(inner) => Type::Option(Box::new(self.instantiate(inner, param_map))),
            Type::Result(ok, err) => Type::Result(
                Box::new(self.instantiate(ok, param_map)),
                Box::new(self.instantiate(err, param_map)),
            ),
            Type::List(inner) => Type::List(Box::new(self.instantiate(inner, param_map))),
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|e| self.instantiate(e, param_map))
                    .collect(),
            ),
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|(k, v)| (k.clone(), self.instantiate(v, param_map)))
                    .collect(),
            ),
            Type::Function {
                params,
                ret,
                effects,
            } => Type::Function {
                params: params
                    .iter()
                    .map(|p| self.instantiate(p, param_map))
                    .collect(),
                ret: Box::new(self.instantiate(ret, param_map)),
                effects: effects.clone(),
            },
            Type::Named(name, args) => Type::Named(
                name.clone(),
                args.iter()
                    .map(|a| self.instantiate(a, param_map))
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }
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

/// Known standard library module names (must correspond to stdlib/*.astra files)
const KNOWN_STDLIB_MODULES: &[&str] = &[
    "collections",
    "core",
    "error",
    "io",
    "iter",
    "json",
    "list",
    "math",
    "option",
    "prelude",
    "regex",
    "result",
    "string",
];

/// Type checker
pub struct TypeChecker {
    /// Current environment
    env: TypeEnv,
    /// Diagnostics collected during checking
    diagnostics: DiagnosticBag,
    /// Stack of lint scopes for tracking variable usage
    lint_scopes: Vec<LintScope>,
    /// Import names defined at module level, with usage tracking
    imports: Vec<(String, Span, bool)>,
    /// Set of type parameter names in the current generic context
    current_type_params: HashSet<String>,
    /// Trait implementations: maps (trait_name, type_name) -> true
    trait_impls: HashSet<(String, String)>,
    /// Defined private function names and their spans (for unused function detection)
    defined_fns: Vec<(String, Span, Visibility)>,
    /// Functions that have been referenced/called
    called_fns: HashSet<String>,
    /// B1: Search paths for resolving imports
    search_paths: Vec<std::path::PathBuf>,
    /// B1: Already-resolved modules to prevent infinite recursion
    resolved_modules: HashSet<String>,
    /// v1.1: Substitution for HM type inference unification
    subst: Substitution,
}

impl TypeChecker {
    /// Create a new type checker
    pub fn new() -> Self {
        Self {
            env: TypeEnv::new(),
            diagnostics: DiagnosticBag::new(),
            lint_scopes: Vec::new(),
            imports: Vec::new(),
            current_type_params: HashSet::new(),
            trait_impls: HashSet::new(),
            defined_fns: Vec::new(),
            called_fns: HashSet::new(),
            search_paths: Vec::new(),
            resolved_modules: HashSet::new(),
            subst: Substitution::new(),
        }
    }

    /// B1: Add a search path for module resolution
    pub fn add_search_path(&mut self, path: std::path::PathBuf) {
        self.search_paths.push(path);
    }

    /// B1: Resolve import path segments to a filesystem path
    fn resolve_module_path(&self, segments: &[String]) -> Option<std::path::PathBuf> {
        let relative = segments.join("/") + ".astra";

        // Map `std.*` imports to `stdlib/*`
        let stdlib_relative =
            if segments.first().map(|s| s.as_str()) == Some("std") && segments.len() > 1 {
                let mut stdlib_segments = vec!["stdlib".to_string()];
                stdlib_segments.extend(segments[1..].iter().cloned());
                Some(stdlib_segments.join("/") + ".astra")
            } else {
                None
            };

        for search_path in &self.search_paths {
            let candidate = search_path.join(&relative);
            if candidate.exists() {
                return Some(candidate);
            }
            if let Some(ref stdlib_rel) = stdlib_relative {
                let candidate = search_path.join(stdlib_rel);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
        // Check relative to cwd
        let candidate = std::path::PathBuf::from(&relative);
        if candidate.exists() {
            return Some(candidate);
        }
        if let Some(ref stdlib_rel) = stdlib_relative {
            let candidate = std::path::PathBuf::from(stdlib_rel);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        None
    }

    /// B1: Resolve an import — parse the imported module and extract type information
    fn resolve_import_types(&mut self, import: &ImportDecl) {
        let segments = &import.path.segments;
        let module_key = segments.join(".");

        // Prevent circular resolution
        if self.resolved_modules.contains(&module_key) {
            return;
        }
        self.resolved_modules.insert(module_key);

        let file_path = match self.resolve_module_path(segments) {
            Some(p) => p,
            None => return, // Can't resolve — will use Unknown types
        };

        let source = match std::fs::read_to_string(&file_path) {
            Ok(s) => s,
            Err(_) => return,
        };

        let source_file = crate::parser::span::SourceFile::new(file_path.clone(), source.clone());
        let lexer = crate::parser::lexer::Lexer::new(&source_file);
        let mut parser = crate::parser::parser::Parser::new(lexer, source_file.clone());
        let module = match parser.parse_module() {
            Ok(m) => m,
            Err(_) => return,
        };

        // Extract type information from the module's items
        let filter = match &import.kind {
            ImportKind::Items(names) => Some(names.clone()),
            _ => None,
        };

        for item in &module.items {
            match item {
                Item::FnDef(fn_def) => {
                    // Only register if it matches the import filter
                    if let Some(ref names) = filter {
                        if !names.contains(&fn_def.name) {
                            continue;
                        }
                    }
                    // Build function type from signature
                    let param_types: Vec<Type> = fn_def
                        .params
                        .iter()
                        .map(|p| self.resolve_type_expr(&p.ty))
                        .collect();
                    let ret_type = fn_def
                        .return_type
                        .as_ref()
                        .map(|t| self.resolve_type_expr(t))
                        .unwrap_or(Type::Unit);
                    let effects: Vec<String> =
                        fn_def.effects.iter().map(|e| e.to_string()).collect();
                    let fn_type = Type::Function {
                        params: param_types,
                        ret: Box::new(ret_type),
                        effects,
                    };
                    self.env.define(fn_def.name.clone(), fn_type);
                }
                Item::TypeDef(def) => {
                    if let Some(ref names) = filter {
                        if !names.contains(&def.name) {
                            continue;
                        }
                    }
                    self.env.register_type(def.clone());
                }
                Item::EnumDef(def) => {
                    if let Some(ref names) = filter {
                        if !names.contains(&def.name) {
                            // Check if any variant name matches
                            let has_variant = def.variants.iter().any(|v| names.contains(&v.name));
                            if !has_variant {
                                continue;
                            }
                        }
                    }
                    self.env.register_enum(def.clone());
                    let enum_type = Type::Named(def.name.clone(), vec![]);
                    for variant in &def.variants {
                        if let Some(ref names) = filter {
                            if !names.contains(&variant.name) && !names.contains(&def.name) {
                                continue;
                            }
                        }
                        if variant.fields.is_empty() {
                            self.env.define(variant.name.clone(), enum_type.clone());
                        } else {
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
                Item::TraitDef(def) => {
                    if let Some(ref names) = filter {
                        if !names.contains(&def.name) {
                            continue;
                        }
                    }
                    self.env
                        .define(def.name.clone(), Type::Named(def.name.clone(), vec![]));
                }
                _ => {}
            }
        }
    }

    /// Push a new lint scope
    /// Validate that an import path resolves to a known module.
    /// For `std.*` imports, checks against the known stdlib module list.
    fn validate_import_path(&mut self, import: &ImportDecl) {
        let segments = &import.path.segments;
        if segments.len() >= 2 && segments[0] == "std" {
            let module_name = &segments[1];
            if !KNOWN_STDLIB_MODULES.contains(&module_name.as_str()) {
                let available: Vec<&str> = KNOWN_STDLIB_MODULES
                    .iter()
                    .copied()
                    .filter(|m| {
                        // Suggest modules with similar names (simple prefix match)
                        m.starts_with(&module_name[..1.min(module_name.len())])
                    })
                    .collect();
                let mut diag =
                    Diagnostic::error(crate::diagnostics::error_codes::syntax::MODULE_NOT_FOUND)
                        .message(format!("Module not found: `std.{}`", module_name))
                        .span(import.span.clone());

                if !available.is_empty() {
                    let suggestions_text = available
                        .iter()
                        .map(|m| format!("std.{}", m))
                        .collect::<Vec<_>>()
                        .join(", ");
                    diag = diag.note(Note::new(format!(
                        "available std modules: {}",
                        suggestions_text
                    )));
                } else {
                    let all_modules = KNOWN_STDLIB_MODULES
                        .iter()
                        .map(|m| format!("std.{}", m))
                        .collect::<Vec<_>>()
                        .join(", ");
                    diag = diag.note(Note::new(format!("available std modules: {}", all_modules)));
                }

                self.diagnostics.push(diag.build());
            }
        }
    }

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
                        .suggestion(
                            Suggestion::new(format!("Rename to `_{}`", binding.name)).with_edit(
                                Edit::new(binding.span.clone(), format!("_{}", binding.name)),
                            ),
                        )
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

    /// E8: Register all variables from a pattern into the environment
    fn register_pattern_vars(&mut self, pattern: &Pattern, env: &mut TypeEnv, ty: Type) {
        match pattern {
            Pattern::Ident { name, span, .. } => {
                env.define(name.clone(), ty);
                self.lint_define_var(name, span);
            }
            Pattern::Tuple { elements, .. } => {
                for elem in elements {
                    self.register_pattern_vars(elem, env, Type::Unknown);
                }
            }
            Pattern::Record { fields, .. } => {
                for (name, pat) in fields {
                    let _ = name; // field name
                    self.register_pattern_vars(pat, env, Type::Unknown);
                }
            }
            Pattern::Variant { fields, .. } => {
                for field in fields {
                    self.register_pattern_vars(field, env, Type::Unknown);
                }
            }
            Pattern::Wildcard { .. }
            | Pattern::IntLit { .. }
            | Pattern::FloatLit { .. }
            | Pattern::BoolLit { .. }
            | Pattern::TextLit { .. } => {}
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
                    // Validate that the import path resolves to a known module
                    self.validate_import_path(import);

                    // Track imports for W0002 (unused import)
                    // Skip public re-exports — they exist for external consumers
                    if import.public {
                        continue;
                    }
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
                    // Track for unused function lint
                    if def.name != "main" {
                        self.defined_fns
                            .push((def.name.clone(), def.span.clone(), def.visibility));
                    }
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
                    // Register that this type implements this trait
                    let type_name = type_expr_to_name(&impl_block.target_type);
                    self.trait_impls
                        .insert((impl_block.trait_name.clone(), type_name));
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
                        .suggestion(
                            Suggestion::new("Remove this import")
                                .with_edit(Edit::new(span.clone(), "")),
                        )
                        .build(),
                );
            }
        }

        // W0008: Emit warnings for unused private functions
        for (name, span, visibility) in &self.defined_fns {
            // Only warn about private functions — public ones may be used externally
            if matches!(visibility, Visibility::Private)
                && !self.called_fns.contains(name)
                && !name.starts_with('_')
            {
                self.diagnostics.push(
                    Diagnostic::warning(crate::diagnostics::error_codes::warnings::UNUSED_FUNCTION)
                        .message(format!("Function `{}` is defined but never used", name))
                        .span(span.clone())
                        .note(Note::new(
                            "if this is intentional, prefix the name with `_` or make it `public`",
                        ))
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
                // B1: Resolve imported module and register types of all
                // imported symbols for cross-file type checking.
                self.resolve_import_types(import);

                // Fallback: register any unresolved names as Unknown so they
                // don't trigger E1002 "Unknown identifier" errors.
                match &import.kind {
                    ImportKind::Module => {
                        if let Some(name) = import.path.segments.last() {
                            if self.env.lookup(name).is_none() {
                                self.env.define(name.clone(), Type::Unknown);
                            }
                        }
                    }
                    ImportKind::Alias(alias) => {
                        if self.env.lookup(alias).is_none() {
                            self.env.define(alias.clone(), Type::Unknown);
                        }
                    }
                    ImportKind::Items(items) => {
                        for item_name in items {
                            if self.env.lookup(item_name).is_none() {
                                self.env.define(item_name.clone(), Type::Unknown);
                            }
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
        let body_type = self.check_block_with_effects(&def.body, &mut fn_env, &mut effects_used);

        // P1: Infer return type for private functions without explicit annotation
        if def.return_type.is_none() && def.visibility == crate::parser::ast::Visibility::Private {
            // Update the registered function type with the inferred body type
            let param_types: Vec<Type> = def
                .params
                .iter()
                .map(|p| self.resolve_type_expr(&p.ty))
                .collect();
            self.env.define(
                def.name.clone(),
                Type::Function {
                    params: param_types,
                    ret: Box::new(body_type),
                    effects: def.effects.clone(),
                },
            );
        }

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
                        let type_display = format_type(&value_type);
                        let mut suggestion = Suggestion::new(format!(
                            "Change the type annotation to `{}`",
                            type_display
                        ));
                        // If we have the type expression span, add a concrete edit
                        if let Some(type_expr) = ty {
                            let type_span = type_expr_span(type_expr);
                            suggestion = suggestion.with_edit(Edit::new(type_span, &type_display));
                        }
                        self.diagnostics.push(
                            Diagnostic::error(
                                crate::diagnostics::error_codes::types::TYPE_MISMATCH,
                            )
                            .message(format!(
                                "Expected type {:?}, found {:?}",
                                declared, value_type
                            ))
                            .suggestion(suggestion)
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
                    // v1.1: JSON builtins
                    "json_parse" => Type::Function {
                        params: vec![Type::Text],
                        ret: Box::new(Type::Json),
                        effects: vec![],
                    },
                    "json_stringify" => Type::Function {
                        params: vec![Type::Json],
                        ret: Box::new(Type::Text),
                        effects: vec![],
                    },
                    // v1.1: Regex builtins
                    "regex_match" => Type::Function {
                        params: vec![Type::Text, Type::Text],
                        ret: Box::new(Type::Option(Box::new(Type::Json))),
                        effects: vec![],
                    },
                    "regex_find_all" => Type::Function {
                        params: vec![Type::Text, Type::Text],
                        ret: Box::new(Type::List(Box::new(Type::Json))),
                        effects: vec![],
                    },
                    "regex_replace" => Type::Function {
                        params: vec![Type::Text, Type::Text, Type::Text],
                        ret: Box::new(Type::Text),
                        effects: vec![],
                    },
                    "regex_split" => Type::Function {
                        params: vec![Type::Text, Type::Text],
                        ret: Box::new(Type::List(Box::new(Type::Text))),
                        effects: vec![],
                    },
                    "regex_is_match" => Type::Function {
                        params: vec![Type::Text, Type::Text],
                        ret: Box::new(Type::Bool),
                        effects: vec![],
                    },
                    // v1.1: Effect convenience builtins
                    "read_file" => Type::Function {
                        params: vec![Type::Text],
                        ret: Box::new(Type::Text),
                        effects: vec!["Fs".to_string()],
                    },
                    "write_file" => Type::Function {
                        params: vec![Type::Text, Type::Text],
                        ret: Box::new(Type::Unit),
                        effects: vec!["Fs".to_string()],
                    },
                    "http_get" => Type::Function {
                        params: vec![Type::Text],
                        ret: Box::new(Type::Text),
                        effects: vec!["Net".to_string()],
                    },
                    "http_post" => Type::Function {
                        params: vec![Type::Text, Type::Text],
                        ret: Box::new(Type::Text),
                        effects: vec!["Net".to_string()],
                    },
                    "random_int" => Type::Function {
                        params: vec![Type::Int, Type::Int],
                        ret: Box::new(Type::Int),
                        effects: vec!["Rand".to_string()],
                    },
                    "random_bool" => Type::Function {
                        params: vec![],
                        ret: Box::new(Type::Bool),
                        effects: vec!["Rand".to_string()],
                    },
                    "current_time_millis" => Type::Function {
                        params: vec![],
                        ret: Box::new(Type::Int),
                        effects: vec!["Clock".to_string()],
                    },
                    "get_env" => Type::Function {
                        params: vec![Type::Text],
                        ret: Box::new(Type::Option(Box::new(Type::Text))),
                        effects: vec!["Env".to_string()],
                    },
                    _ => {
                        // Mark variable as used for W0001 lint
                        self.lint_use_var(name);
                        // Track function references for unused function lint
                        self.called_fns.insert(name.clone());

                        if let Some(ty) = env.lookup(name) {
                            ty.clone()
                        } else {
                            let mut diag = Diagnostic::error(
                                crate::diagnostics::error_codes::types::UNKNOWN_IDENTIFIER,
                            )
                            .message(format!("Unknown identifier: {}", name))
                            .span(span.clone());

                            // Suggest similar names from the environment
                            if let Some(similar) = find_similar_name(name, env) {
                                diag = diag
                                    .note(Note::new(format!("did you mean `{}`?", similar)))
                                    .suggestion(
                                        Suggestion::new(format!("Replace with `{}`", similar))
                                            .with_edit(Edit::new(span.clone(), &similar)),
                                    );
                            }

                            self.diagnostics.push(diag.build());
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
                    | BinaryOp::Ge => {
                        if left_ty != Type::Unknown
                            && right_ty != Type::Unknown
                            && !self.types_compatible(&left_ty, &right_ty)
                        {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    crate::diagnostics::error_codes::types::TYPE_MISMATCH,
                                )
                                .message(format!(
                                    "Cannot compare `{}` with `{}`",
                                    format_type(&left_ty),
                                    format_type(&right_ty)
                                ))
                                .build(),
                            );
                        }
                        Type::Bool
                    }
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

                // Handle overloaded built-in functions whose return type
                // depends on the argument type (to_int, to_float).
                if let Expr::Ident { name, .. } = func.as_ref() {
                    match name.as_str() {
                        "to_int" => {
                            if arg_types.len() == 1 {
                                return match &arg_types[0] {
                                    Type::Text => Type::Option(Box::new(Type::Int)),
                                    Type::Int | Type::Float | Type::Bool => Type::Int,
                                    Type::Unknown => Type::Unknown,
                                    other => {
                                        self.diagnostics.push(
                                            Diagnostic::error(
                                                crate::diagnostics::error_codes::types::TYPE_MISMATCH,
                                            )
                                            .message(format!(
                                                "Type mismatch in argument 1: expected `Int`, `Float`, `Text`, or `Bool`, found `{}`",
                                                format_type(other)
                                            ))
                                            .span(args[0].span().clone())
                                            .build(),
                                        );
                                        Type::Unknown
                                    }
                                };
                            }
                        }
                        "to_float" => {
                            if arg_types.len() == 1 {
                                return match &arg_types[0] {
                                    Type::Text => Type::Option(Box::new(Type::Float)),
                                    Type::Int | Type::Float => Type::Float,
                                    Type::Unknown => Type::Unknown,
                                    other => {
                                        self.diagnostics.push(
                                            Diagnostic::error(
                                                crate::diagnostics::error_codes::types::TYPE_MISMATCH,
                                            )
                                            .message(format!(
                                                "Type mismatch in argument 1: expected `Int`, `Float`, or `Text`, found `{}`",
                                                format_type(other)
                                            ))
                                            .span(args[0].span().clone())
                                            .build(),
                                        );
                                        Type::Unknown
                                    }
                                };
                            }
                        }
                        _ => {}
                    }
                }

                if let Type::Function {
                    params,
                    ret,
                    effects: fn_effects,
                } = &func_ty
                {
                    // Check arity
                    if params.len() != arg_types.len()
                        && !params.is_empty()
                        && !arg_types.is_empty()
                    {
                        self.diagnostics.push(
                            Diagnostic::error(
                                crate::diagnostics::error_codes::types::WRONG_ARGUMENT_COUNT,
                            )
                            .message(format!(
                                "Expected {} argument(s) but found {}",
                                params.len(),
                                arg_types.len()
                            ))
                            .span(func.span().clone())
                            .build(),
                        );
                    }

                    // B1: Check argument types against parameter types
                    for (i, (param_ty, arg_ty)) in params.iter().zip(arg_types.iter()).enumerate() {
                        if !self.types_compatible(param_ty, arg_ty) {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    crate::diagnostics::error_codes::types::TYPE_MISMATCH,
                                )
                                .message(format!(
                                    "Type mismatch in argument {}: expected `{}`, found `{}`",
                                    i + 1,
                                    format_type(param_ty),
                                    format_type(arg_ty)
                                ))
                                .span(if i < args.len() {
                                    args[i].span().clone()
                                } else {
                                    func.span().clone()
                                })
                                .build(),
                            );
                        }
                    }

                    // B1: Propagate effects from called function to caller
                    // This allows the existing effect enforcement (in check_fndef)
                    // to verify the caller declares the necessary effects.
                    for fn_effect in fn_effects {
                        effects.insert(fn_effect.clone());
                    }

                    // Try generic type parameter inference if the called function
                    // has type params
                    if let Expr::Ident { name, span, .. } = func.as_ref() {
                        if let Some(fn_def) = env.lookup_fn(name).cloned() {
                            if !fn_def.type_params.is_empty() {
                                let bindings =
                                    self.unify_type_params(&fn_def.type_params, params, &arg_types);

                                // Check trait bounds on type parameters
                                for (param_name, bound_name) in &fn_def.type_param_bounds {
                                    if let Some(concrete_ty) = bindings.get(param_name) {
                                        let concrete_name = format_type(concrete_ty);
                                        if !self
                                            .trait_impls
                                            .contains(&(bound_name.clone(), concrete_name.clone()))
                                        {
                                            self.diagnostics.push(
                                                Diagnostic::error(crate::diagnostics::error_codes::types::TRAIT_CONSTRAINT_NOT_SATISFIED)
                                                    .message(format!(
                                                        "Type `{}` does not implement trait `{}` required by type parameter `{}`",
                                                        concrete_name, bound_name, param_name
                                                    ))
                                                    .span(span.clone())
                                                    .note(Note::new(format!(
                                                        "add `impl {} for {} {{ ... }}` to satisfy this constraint",
                                                        bound_name, concrete_name
                                                    )))
                                                    .build(),
                                            );
                                        }
                                    }
                                }

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
                } else if expr_ty == Type::Json {
                    // Field access on Json returns Json (dynamic access)
                    Type::Json
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
                pattern,
                iter,
                body,
                ..
            } => {
                self.check_expr_with_effects(iter, env, effects);
                let mut loop_env = env.clone();
                self.push_lint_scope();
                // E8: If there's a destructuring pattern, register pattern vars
                if let Some(pat) = pattern {
                    self.register_pattern_vars(pat, &mut loop_env, Type::Unknown);
                } else {
                    loop_env.define(binding.clone(), Type::Unknown);
                    self.lint_define_var(binding, iter.span());
                }
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
            Expr::Range {
                start, end, span, ..
            } => {
                let start_ty = self.check_expr_with_effects(start, env, effects);
                let end_ty = self.check_expr_with_effects(end, env, effects);
                // Both bounds must be Int
                if !self.types_compatible(&start_ty, &Type::Int) {
                    self.diagnostics.push(
                        Diagnostic::error(crate::diagnostics::error_codes::types::TYPE_MISMATCH)
                            .message(format!("Range start must be Int, found {:?}", start_ty))
                            .span(span.clone())
                            .build(),
                    );
                }
                if !self.types_compatible(&end_ty, &Type::Int) {
                    self.diagnostics.push(
                        Diagnostic::error(crate::diagnostics::error_codes::types::TYPE_MISMATCH)
                            .message(format!("Range end must be Int, found {:?}", end_ty))
                            .span(span.clone())
                            .build(),
                    );
                }
                Type::List(Box::new(Type::Int))
            }
            // E2: Index access — check the inner expression and index
            Expr::IndexAccess { expr, index, .. } => {
                let collection_type = self.check_expr_with_effects(expr, env, effects);
                let _index_type = self.check_expr_with_effects(index, env, effects);
                // Return the element type if we can determine it
                match collection_type {
                    Type::List(inner) => *inner,
                    Type::Text => Type::Text,
                    Type::Json => Type::Json,
                    Type::Tuple(elements) => {
                        // If the index is a literal int, we can be precise
                        if let Expr::IntLit { value, .. } = index.as_ref() {
                            let idx = *value as usize;
                            if idx < elements.len() {
                                elements[idx].clone()
                            } else {
                                Type::Unknown
                            }
                        } else {
                            Type::Unknown
                        }
                    }
                    _ => Type::Unknown,
                }
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
                "Json" => Type::Json,
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
        // v1.1: Use the substitution to resolve type variables before comparing
        let actual = self.subst.apply(actual);
        let expected = self.subst.apply(expected);

        if actual == Type::Unknown || expected == Type::Unknown {
            return true;
        }
        // Type parameters are compatible with anything (they're generic)
        if matches!(&actual, Type::TypeParam(_)) || matches!(&expected, Type::TypeParam(_)) {
            return true;
        }
        // Type variables are compatible (will be resolved during unification)
        if matches!(&actual, Type::Var(_)) || matches!(&expected, Type::Var(_)) {
            return true;
        }
        // Json is compatible with any JSON-representable type
        if actual == Type::Json || expected == Type::Json {
            return true;
        }
        // List[T] compatible with List[U] if T compatible with U
        if let (Type::List(a), Type::List(b)) = (&actual, &expected) {
            return self.types_compatible(a, b);
        }
        // Tuple(A, B) compatible with Tuple(C, D) if element-wise compatible
        if let (Type::Tuple(a), Type::Tuple(b)) = (&actual, &expected) {
            return a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(x, y)| self.types_compatible(x, y));
        }
        // Option types
        if let (Type::Option(a), Type::Option(b)) = (&actual, &expected) {
            return self.types_compatible(a, b);
        }
        // Result types
        if let (Type::Result(a_ok, a_err), Type::Result(b_ok, b_err)) = (&actual, &expected) {
            return self.types_compatible(a_ok, b_ok) && self.types_compatible(a_err, b_err);
        }
        // Named types with same name: check type args
        if let (Type::Named(n1, args1), Type::Named(n2, args2)) = (&actual, &expected) {
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
        ) = (&actual, &expected)
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
    /// v1.1: Uses the Substitution-based unification for more accurate inference.
    fn unify_type_params(
        &mut self,
        type_params: &[String],
        declared_param_types: &[Type],
        actual_arg_types: &[Type],
    ) -> HashMap<String, Type> {
        // Create fresh type variables for each type parameter
        let mut param_vars: HashMap<String, Type> = HashMap::new();
        for tp in type_params {
            let fresh = self.subst.fresh_var();
            param_vars.insert(tp.clone(), fresh);
        }

        // Substitute type params with fresh vars in declared types
        for (declared, actual) in declared_param_types.iter().zip(actual_arg_types.iter()) {
            let instantiated = self.substitute_type_params(declared, &param_vars);
            // Unify the instantiated declared type with the actual type
            self.subst.unify(&instantiated, actual);
        }

        // Extract the resolved bindings
        let mut bindings: HashMap<String, Type> = HashMap::new();
        for (name, var) in &param_vars {
            let resolved = self.subst.apply(var);
            if !matches!(resolved, Type::Var(_)) {
                bindings.insert(name.clone(), resolved);
            }
        }

        // Fallback: also try the old unification for any params not resolved
        for (declared, actual) in declared_param_types.iter().zip(actual_arg_types.iter()) {
            self.unify_one(declared, actual, type_params, &mut bindings);
        }

        bindings
    }

    /// Substitute TypeParam references with provided types.
    fn substitute_type_params(&self, ty: &Type, params: &HashMap<String, Type>) -> Type {
        match ty {
            Type::TypeParam(name) => params.get(name).cloned().unwrap_or_else(|| ty.clone()),
            Type::Named(name, args) if params.contains_key(name) && args.is_empty() => {
                params[name].clone()
            }
            Type::Option(inner) => {
                Type::Option(Box::new(self.substitute_type_params(inner, params)))
            }
            Type::Result(ok, err) => Type::Result(
                Box::new(self.substitute_type_params(ok, params)),
                Box::new(self.substitute_type_params(err, params)),
            ),
            Type::List(inner) => Type::List(Box::new(self.substitute_type_params(inner, params))),
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|e| self.substitute_type_params(e, params))
                    .collect(),
            ),
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|(k, v)| (k.clone(), self.substitute_type_params(v, params)))
                    .collect(),
            ),
            Type::Function {
                params: p,
                ret,
                effects,
            } => Type::Function {
                params: p
                    .iter()
                    .map(|t| self.substitute_type_params(t, params))
                    .collect(),
                ret: Box::new(self.substitute_type_params(ret, params)),
                effects: effects.clone(),
            },
            Type::Named(name, args) => Type::Named(
                name.clone(),
                args.iter()
                    .map(|a| self.substitute_type_params(a, params))
                    .collect(),
            ),
            _ => ty.clone(),
        }
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
mod tests;
