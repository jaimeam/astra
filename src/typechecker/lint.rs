//! Lint checks for the type checker (W0001-W0007).
//!
//! Tracks variable definitions/usage, detects unused variables,
//! shadowed bindings, and manages lint scopes.

use super::*;

/// Tracks a variable definition for unused-variable lint (W0001)
#[derive(Debug, Clone)]
pub(super) struct VarBinding {
    pub name: String,
    pub span: Span,
    pub used: bool,
}

/// Tracks lint state within a scope
#[derive(Debug, Clone, Default)]
pub(super) struct LintScope {
    /// Variables defined in this scope, with usage tracking
    pub vars: Vec<VarBinding>,
    /// Set of variable names already defined in this scope (for shadowing detection)
    pub defined_names: HashSet<String>,
}

/// Methods on `TypeChecker` for lint scope management and warning generation
impl TypeChecker {
    /// Push a new lint scope
    pub(super) fn push_lint_scope(&mut self) {
        self.lint_scopes.push(LintScope::default());
    }

    /// Pop a lint scope and emit W0001 warnings for unused variables
    pub(super) fn pop_lint_scope(&mut self) {
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
    pub(super) fn lint_define_var(&mut self, name: &str, span: &Span) {
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
    pub(super) fn register_pattern_vars(&mut self, pattern: &Pattern, env: &mut TypeEnv, ty: Type) {
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
    pub(super) fn lint_use_var(&mut self, name: &str) {
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
}
