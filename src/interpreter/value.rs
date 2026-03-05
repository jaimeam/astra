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

/// Assign an integer tag to each value variant for cross-type ordering.
fn type_tag(v: &Value) -> u8 {
    match v {
        Value::Unit => 0,
        Value::Bool(_) => 1,
        Value::Int(_) => 2,
        Value::Float(_) => 3,
        Value::Text(_) => 4,
        Value::None => 5,
        Value::Some(_) => 6,
        Value::Ok(_) => 7,
        Value::Err(_) => 8,
        Value::Tuple(_) => 9,
        Value::List(_) => 10,
        Value::Record(_) => 11,
        Value::Variant { .. } => 12,
        Value::Map(_) => 13,
        Value::Set(_) => 14,
        Value::Closure { .. } => 15,
        Value::VariantConstructor { .. } => 16,
        Value::Future { .. } => 17,
    }
}

/// Total ordering for values, used to keep Map keys and Set elements sorted
/// for O(log n) binary-search lookups.
pub fn compare_values_total(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let ta = type_tag(a);
    let tb = type_tag(b);
    if ta != tb {
        return ta.cmp(&tb);
    }
    match (a, b) {
        (Value::Unit, Value::Unit) => Ordering::Equal,
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Text(x), Value::Text(y)) => x.cmp(y),
        (Value::None, Value::None) => Ordering::Equal,
        (Value::Some(x), Value::Some(y)) | (Value::Ok(x), Value::Ok(y)) | (Value::Err(x), Value::Err(y)) => {
            compare_values_total(x, y)
        }
        (Value::Tuple(xs), Value::Tuple(ys)) | (Value::List(xs), Value::List(ys)) | (Value::Set(xs), Value::Set(ys)) => {
            for (x, y) in xs.iter().zip(ys.iter()) {
                let c = compare_values_total(x, y);
                if c != Ordering::Equal {
                    return c;
                }
            }
            xs.len().cmp(&ys.len())
        }
        (Value::Variant { name: n1, data: d1 }, Value::Variant { name: n2, data: d2 }) => {
            let nc = n1.cmp(n2);
            if nc != Ordering::Equal {
                return nc;
            }
            match (d1, d2) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (Some(a), Some(b)) => compare_values_total(a, b),
            }
        }
        _ => Ordering::Equal,
    }
}

// ---------------------------------------------------------------------------
// Sorted-vector helpers for O(log n) Map/Set operations
// ---------------------------------------------------------------------------

/// Find the index of `key` in a sorted map vec, or where it would be inserted.
pub fn map_search(entries: &[(Value, Value)], key: &Value) -> Result<usize, usize> {
    entries.binary_search_by(|(k, _)| compare_values_total(k, key))
}

/// Look up a key in a sorted map vec. O(log n).
pub fn map_get<'a>(entries: &'a [(Value, Value)], key: &Value) -> Option<&'a Value> {
    map_search(entries, key).ok().map(|i| &entries[i].1)
}

/// Insert or update a key in a sorted map vec, returning a new vec.
pub fn map_set(entries: &[(Value, Value)], key: Value, value: Value) -> Vec<(Value, Value)> {
    let mut new = entries.to_vec();
    match map_search(&new, &key) {
        Ok(i) => new[i].1 = value,
        Err(i) => new.insert(i, (key, value)),
    }
    new
}

/// Remove a key from a sorted map vec, returning a new vec.
pub fn map_remove(entries: &[(Value, Value)], key: &Value) -> Vec<(Value, Value)> {
    let mut new = entries.to_vec();
    if let Ok(i) = map_search(&new, key) {
        new.remove(i);
    }
    new
}

/// Find the index of `val` in a sorted set vec, or where it would be inserted.
pub fn set_search(elements: &[Value], val: &Value) -> Result<usize, usize> {
    elements.binary_search_by(|e| compare_values_total(e, val))
}

/// Check membership in a sorted set. O(log n).
pub fn set_contains(elements: &[Value], val: &Value) -> bool {
    set_search(elements, val).is_ok()
}

/// Insert a value into a sorted set vec (no duplicates), returning a new vec.
pub fn set_add(elements: &[Value], val: Value) -> Vec<Value> {
    let mut new = elements.to_vec();
    if let Err(i) = set_search(&new, &val) {
        new.insert(i, val);
    }
    new
}

/// Remove a value from a sorted set vec, returning a new vec.
pub fn set_remove(elements: &[Value], val: &Value) -> Vec<Value> {
    let mut new = elements.to_vec();
    if let Ok(i) = set_search(&new, val) {
        new.remove(i);
    }
    new
}

/// Build a sorted map from an unsorted vec of pairs.
pub fn sorted_map_from(mut entries: Vec<(Value, Value)>) -> Vec<(Value, Value)> {
    entries.sort_by(|(a, _), (b, _)| compare_values_total(a, b));
    entries.dedup_by(|(a, _), (b, _)| compare_values_total(a, b) == std::cmp::Ordering::Equal);
    entries
}

/// Build a sorted set from an unsorted vec.
pub fn sorted_set_from(mut elements: Vec<Value>) -> Vec<Value> {
    elements.sort_by(compare_values_total);
    elements.dedup_by(|a, b| compare_values_total(a, b) == std::cmp::Ordering::Equal);
    elements
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
