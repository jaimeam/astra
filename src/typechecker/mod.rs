//! Type checker for Astra
//!
//! Implements type checking, inference, exhaustiveness checking, effect enforcement,
//! and lint checks (W0001-W0007).

use crate::diagnostics::{Diagnostic, DiagnosticBag, Edit, Note, Span, Suggestion};
use crate::parser::ast::*;
use std::collections::{HashMap, HashSet};
pub mod exhaustiveness;
pub mod lint;
pub mod substitution;
pub mod type_ops;

pub use exhaustiveness::check_exhaustiveness;
pub use substitution::{Substitution, Type, TypeVarId};

use exhaustiveness::collect_pattern_bindings;
use lint::LintScope;

/// Format a Type as a human-readable string for suggestions.
fn format_type(ty: &Type) -> String {
    match ty {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::Float => "Float".to_string(),
        Type::Bool => "Bool".to_string(),
        Type::Text => "Text".to_string(),
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
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
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

    // Generic type checking tests

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

    // Trait/impl type checking tests

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

    // List and Tuple type tracking

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

    #[test]
    fn test_suggestions_have_edits() {
        // W0001: unused variable suggestion should have an Edit
        let source = r#"
module example

fn main() -> Unit {
  let x = 42
}
"#;
        let diags = check_module_all_diags(source);
        let unused_diag = diags.diagnostics().iter().find(|d| d.code == "W0001");
        assert!(unused_diag.is_some(), "Should have W0001 for unused `x`");
        let unused = unused_diag.unwrap();
        assert!(
            !unused.suggestions.is_empty(),
            "W0001 should have a suggestion"
        );
        assert!(
            !unused.suggestions[0].edits.is_empty(),
            "W0001 suggestion should have an edit with replacement text"
        );
    }

    #[test]
    fn test_unknown_identifier_suggestion_has_edit() {
        // E1002: unknown identifier with similar name suggestion
        let source = r#"
module example

fn calculate(value: Int) -> Int {
  valu + 1
}
"#;
        let diags = check_module_all_diags(source);
        let error_diag = diags.diagnostics().iter().find(|d| d.code == "E1002");
        assert!(error_diag.is_some(), "Should have E1002 for `valu`");
        let error = error_diag.unwrap();
        assert!(
            !error.suggestions.is_empty(),
            "E1002 should have a did-you-mean suggestion"
        );
        assert!(
            !error.suggestions[0].edits.is_empty(),
            "E1002 suggestion should have an edit for replacement"
        );
    }

    #[test]
    fn test_trait_constraint_satisfied() {
        // Should pass: Int implements Show
        let source = r#"
module example

trait Show {
  fn to_text(self) -> Text
}

impl Show for Int {
  fn to_text(self) -> Text { "int" }
}

fn display[T: Show](value: T) -> Text {
  "ok"
}

fn main() -> Text {
  display(42)
}
"#;
        let diags = check_module_all_diags(source);
        let constraint_errors: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "E1016")
            .collect();
        assert!(
            constraint_errors.is_empty(),
            "Should have no E1016 errors when trait is implemented"
        );
    }

    #[test]
    fn test_trait_constraint_not_satisfied() {
        // Should fail: Text does not implement Sortable
        let source = r#"
module example

trait Sortable {
  fn compare(self, other: Int) -> Int
}

impl Sortable for Int {
  fn compare(self, other: Int) -> Int { 0 }
}

fn sort_items[T: Sortable](items: List[T]) -> List[T] {
  items
}

fn main() -> List[Text] {
  sort_items(["hello", "world"])
}
"#;
        let diags = check_module_all_diags(source);
        let constraint_errors: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "E1016")
            .collect();
        assert!(
            !constraint_errors.is_empty(),
            "Should have E1016 error: Text does not implement Sortable"
        );
        assert!(constraint_errors[0]
            .message
            .contains("does not implement trait"));
    }

    // W0008: Unused function tests

    #[test]
    fn test_lint_unused_private_function_warns() {
        let source = r#"
module example

fn unused_helper() -> Int {
  42
}

fn main() -> Int {
  0
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0008")
            .collect();
        assert!(
            !warnings.is_empty(),
            "should warn about unused private function"
        );
        assert!(warnings[0].message.contains("unused_helper"));
    }

    #[test]
    fn test_lint_used_function_no_warning() {
        let source = r#"
module example

fn helper() -> Int {
  42
}

fn main() -> Int {
  helper()
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0008")
            .collect();
        assert!(
            warnings.is_empty(),
            "should not warn about called function: {:?}",
            warnings
        );
    }

    #[test]
    fn test_lint_public_function_no_warning() {
        let source = r#"
module example

public fn api_endpoint() -> Int {
  42
}

fn main() -> Int {
  0
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0008")
            .collect();
        assert!(
            warnings.is_empty(),
            "should not warn about public functions"
        );
    }

    #[test]
    fn test_lint_underscore_function_no_warning() {
        let source = r#"
module example

fn _internal() -> Int {
  42
}

fn main() -> Int {
  0
}
"#;
        let diags = check_module_all_diags(source);
        let warnings: Vec<_> = diags
            .diagnostics()
            .iter()
            .filter(|d| d.code == "W0008")
            .collect();
        assert!(
            warnings.is_empty(),
            "should not warn about _-prefixed functions"
        );
    }
}
