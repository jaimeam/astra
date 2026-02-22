//! Pattern matching for Astra values.

use super::value::Value;
use crate::parser::ast::Pattern;

/// Match a pattern against a value, returning bindings if successful
pub fn match_pattern(pattern: &Pattern, value: &Value) -> Option<Vec<(String, Value)>> {
    match pattern {
        // Wildcard matches anything
        Pattern::Wildcard { .. } => Some(vec![]),

        // Identifier binds the value
        Pattern::Ident { name, .. } => Some(vec![(name.clone(), value.clone())]),

        // Literal patterns
        Pattern::IntLit { value: pat_val, .. } => {
            if let Value::Int(v) = value {
                if v == pat_val {
                    return Some(vec![]);
                }
            }
            None
        }
        Pattern::FloatLit { value: pat_val, .. } => {
            if let Value::Float(v) = value {
                if v == pat_val {
                    return Some(vec![]);
                }
            }
            None
        }
        Pattern::BoolLit { value: pat_val, .. } => {
            if let Value::Bool(v) = value {
                if v == pat_val {
                    return Some(vec![]);
                }
            }
            None
        }
        Pattern::TextLit { value: pat_val, .. } => {
            if let Value::Text(v) = value {
                if v == pat_val {
                    return Some(vec![]);
                }
            }
            None
        }

        // Record pattern
        Pattern::Record { fields, .. } => {
            if let Value::Record(val_fields) = value {
                let mut bindings = Vec::new();
                for (name, pat) in fields {
                    if let Some(field_val) = val_fields.get(name) {
                        if let Some(sub_bindings) = match_pattern(pat, field_val) {
                            bindings.extend(sub_bindings);
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                Some(bindings)
            } else {
                None
            }
        }

        // Variant pattern (for enums, Some/None, Ok/Err)
        Pattern::Variant { name, fields, .. } => {
            match (name.as_str(), value) {
                // Option::Some
                ("Some", Value::Some(inner)) => {
                    if fields.len() == 1 {
                        match_pattern(&fields[0], inner)
                    } else if fields.is_empty() {
                        Some(vec![])
                    } else {
                        None
                    }
                }
                // Option::None
                ("None", Value::None) => {
                    if fields.is_empty() {
                        Some(vec![])
                    } else {
                        None
                    }
                }
                // Result::Ok
                ("Ok", Value::Ok(inner)) => {
                    if fields.len() == 1 {
                        match_pattern(&fields[0], inner)
                    } else if fields.is_empty() {
                        Some(vec![])
                    } else {
                        None
                    }
                }
                // Result::Err
                ("Err", Value::Err(inner)) => {
                    if fields.len() == 1 {
                        match_pattern(&fields[0], inner)
                    } else if fields.is_empty() {
                        Some(vec![])
                    } else {
                        None
                    }
                }
                // Generic variant
                (
                    pat_name,
                    Value::Variant {
                        name: var_name,
                        data: var_data,
                    },
                ) => {
                    if pat_name == var_name {
                        match (fields.as_slice(), var_data) {
                            // No fields pattern, no data
                            ([], None) => Some(vec![]),
                            // Single field pattern, single data value
                            ([pat], Some(var_val)) => match_pattern(pat, var_val),
                            // Multi-field pattern, data is a Record
                            (pats, Some(var_val)) if pats.len() > 1 => {
                                // Multi-field variant data is stored as a Record
                                if let Value::Record(ref field_map) = **var_val {
                                    let mut bindings = Vec::new();
                                    // Positional matching: match patterns to fields by order
                                    let mut sorted_keys: Vec<_> = field_map.keys().collect();
                                    sorted_keys.sort();
                                    if pats.len() != sorted_keys.len() {
                                        return None;
                                    }
                                    for (pat, key) in pats.iter().zip(sorted_keys.iter()) {
                                        if let Some(val) = field_map.get(*key) {
                                            if let Some(sub) = match_pattern(pat, val) {
                                                bindings.extend(sub);
                                            } else {
                                                return None;
                                            }
                                        } else {
                                            return None;
                                        }
                                    }
                                    Some(bindings)
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // Tuple pattern
        Pattern::Tuple { elements, .. } => {
            if let Value::Tuple(values) = value {
                if elements.len() != values.len() {
                    return None;
                }
                let mut bindings = Vec::new();
                for (pat, val) in elements.iter().zip(values.iter()) {
                    if let Some(sub) = match_pattern(pat, val) {
                        bindings.extend(sub);
                    } else {
                        return None;
                    }
                }
                Some(bindings)
            } else {
                None
            }
        }
    }
}
