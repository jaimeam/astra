//! Type manipulation operations for the type checker.
//!
//! Contains `resolve_type_expr`, `types_compatible`, `unify_type_params`,
//! `substitute_type_params`, `unify_one`, and related type operations.

use super::*;
use std::collections::HashMap;

/// Methods on `TypeChecker` for type resolution and compatibility
impl TypeChecker {
    pub(super) fn resolve_type_expr(&self, ty: &TypeExpr) -> Type {
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

    pub(super) fn types_compatible(&self, actual: &Type, expected: &Type) -> bool {
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
    pub(super) fn unify_type_params(
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
    pub(super) fn substitute_type_params(&self, ty: &Type, params: &HashMap<String, Type>) -> Type {
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
    pub(super) fn unify_one(
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
