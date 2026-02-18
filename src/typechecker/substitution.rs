//! Hindley-Milner type inference with constraint-based unification.
//!
//! Contains the `Type` enum, `TypeVarId`, and `Substitution` (union-find)
//! used for type variable resolution and unification.

use std::collections::HashMap;

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
    /// Type parameter (generic, e.g., `T` in `fn id[T](x: T) -> T`)
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
