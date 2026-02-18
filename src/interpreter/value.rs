//! Runtime value types for the Astra interpreter.

use std::collections::HashMap;

use crate::parser::ast::*;

use super::environment::Environment;

/// Runtime value
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Value {
    /// Unit value
    Unit,
    /// Integer
    Int(i64),
    /// Float
    Float(f64),
    /// Boolean
    Bool(bool),
    /// Text string
    Text(String),
    /// Record value
    Record(HashMap<String, Value>),
    /// Enum variant
    Variant {
        name: String,
        data: Option<Box<Value>>,
    },
    /// Function closure
    Closure {
        name: Option<String>,
        params: Vec<String>,
        body: ClosureBody,
        env: Environment,
    },
    /// Option::Some
    Some(Box<Value>),
    /// Option::None
    None,
    /// Result::Ok
    Ok(Box<Value>),
    /// Result::Err
    Err(Box<Value>),
    /// List of values
    List(Vec<Value>),
    /// Tuple of values
    Tuple(Vec<Value>),
    /// Map of key-value pairs
    Map(Vec<(Value, Value)>),
    /// Set of unique values
    Set(Vec<Value>),
    /// Variant constructor for multi-field enums
    VariantConstructor {
        name: String,
        field_names: Vec<String>,
    },
    /// v1.1: Future value — represents an async computation that can be awaited.
    /// Contains the closure to execute and a flag indicating if it has been resolved.
    Future {
        /// The async function body (as a closure)
        func: Box<Value>,
        /// Arguments to pass when resolving
        args: Vec<Value>,
    },
}

/// Closure body containing the AST block and optional contracts
#[derive(Debug, Clone)]
pub struct ClosureBody {
    /// The block to execute when called
    pub block: Block,
    /// Preconditions (requires clauses)
    pub requires: Vec<Expr>,
    /// Postconditions (ensures clauses)
    pub ensures: Vec<Expr>,
}

/// Compare two values for equality
pub fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Unit, Value::Unit) => true,
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Text(a), Value::Text(b)) => a == b,
        (Value::None, Value::None) => true,
        (Value::Some(a), Value::Some(b)) => values_equal(a, b),
        (Value::Ok(a), Value::Ok(b)) => values_equal(a, b),
        (Value::Err(a), Value::Err(b)) => values_equal(a, b),
        (Value::Variant { name: n1, data: d1 }, Value::Variant { name: n2, data: d2 }) => {
            n1 == n2
                && match (d1, d2) {
                    (None, None) => true,
                    (Some(a), Some(b)) => values_equal(a, b),
                    _ => false,
                }
        }
        (Value::Record(r1), Value::Record(r2)) => {
            r1.len() == r2.len()
                && r1
                    .iter()
                    .all(|(k, v)| r2.get(k).is_some_and(|v2| values_equal(v, v2)))
        }
        (Value::List(a), Value::List(b))
        | (Value::Tuple(a), Value::Tuple(b))
        | (Value::Set(a), Value::Set(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| values_equal(x, y))
        }
        (Value::Map(a), Value::Map(b)) => {
            a.len() == b.len()
                && a.iter().all(|(k, v)| {
                    b.iter()
                        .any(|(k2, v2)| values_equal(k, k2) && values_equal(v, v2))
                })
        }
        // Closures, constructors, and futures are never equal
        (Value::Closure { .. }, Value::Closure { .. }) => false,
        (Value::VariantConstructor { .. }, Value::VariantConstructor { .. }) => false,
        (Value::Future { .. }, Value::Future { .. }) => false,
        _ => false,
    }
}

/// Compare two values for ordering (used by sort)
pub fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => a.cmp(b),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Text(a), Value::Text(b)) => a.cmp(b),
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        _ => std::cmp::Ordering::Equal,
    }
}

/// Format a value for display
pub fn format_value(value: &Value) -> String {
    match value {
        Value::Unit => "()".to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => {
            if f.fract() == 0.0 {
                format!("{:.1}", f)
            } else {
                f.to_string()
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Text(s) => s.clone(),
        Value::Record(fields) => {
            let field_strs: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_value(v)))
                .collect();
            format!("{{ {} }}", field_strs.join(", "))
        }
        Value::Variant { name, data } => {
            if let Some(d) = data {
                format!("{}({})", name, format_value(d))
            } else {
                name.clone()
            }
        }
        Value::Closure { .. } => "<closure>".to_string(),
        Value::VariantConstructor { name, .. } => format!("<constructor:{}>", name),
        Value::Future { .. } => "<future>".to_string(),
        Value::List(items) => {
            let item_strs: Vec<String> = items.iter().map(format_value).collect();
            format!("[{}]", item_strs.join(", "))
        }
        Value::Tuple(elements) => {
            let strs: Vec<String> = elements.iter().map(format_value).collect();
            format!("({})", strs.join(", "))
        }
        Value::Map(entries) => {
            let strs: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{}: {}", format_value(k), format_value(v)))
                .collect();
            format!("Map({{{}}})", strs.join(", "))
        }
        Value::Set(elements) => {
            let strs: Vec<String> = elements.iter().map(format_value).collect();
            format!("Set({{{}}})", strs.join(", "))
        }
        Value::Some(inner) => format!("Some({})", format_value(inner)),
        Value::None => "None".to_string(),
        Value::Ok(inner) => format!("Ok({})", format_value(inner)),
        Value::Err(inner) => format!("Err({})", format_value(inner)),
    }
}
