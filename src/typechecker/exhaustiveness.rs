//! Exhaustiveness checking for match expressions.
//!
//! Determines whether a set of match patterns covers all possible cases
//! for Option, Result, Bool, and user-defined enum types.

use super::*;
use std::collections::HashSet;

/// The kind of type being matched, used for exhaustiveness checking
#[derive(Debug, Clone)]
pub(super) enum MatchTypeKind {
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

/// Collect variable bindings from a pattern into a type environment
pub(super) fn collect_pattern_bindings(pattern: &Pattern, env: &mut TypeEnv) {
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
pub(super) fn is_catch_all(pattern: &Pattern) -> bool {
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

/// Methods on `TypeChecker` for exhaustiveness checking
impl TypeChecker {
    /// C2: Check exhaustiveness of a match expression
    pub(super) fn check_match_exhaustiveness(
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
    pub(super) fn infer_match_kind(
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
    pub(super) fn find_enum_containing_variant(
        &self,
        variant: &str,
        env: &TypeEnv,
    ) -> Option<EnumDef> {
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
}
