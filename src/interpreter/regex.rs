//! Regular expression support for Astra values.

use std::collections::HashMap;

use super::error::RuntimeError;
use super::value::Value;

/// Check if a regex pattern matches a string, returns Option with match info
pub(super) fn regex_match(pattern: &str, text: &str) -> Result<Value, RuntimeError> {
    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new("E4016", format!("Invalid regex pattern: {}", e)))?;

    match re.captures(text) {
        Some(caps) => {
            let full_match = caps.get(0).map_or("", |m| m.as_str());
            let mut fields = HashMap::new();
            fields.insert("matched".to_string(), Value::Text(full_match.to_string()));
            fields.insert(
                "start".to_string(),
                Value::Int(caps.get(0).map_or(0, |m| m.start()) as i64),
            );
            fields.insert(
                "end".to_string(),
                Value::Int(caps.get(0).map_or(0, |m| m.end()) as i64),
            );

            // Collect capture groups
            let groups: Vec<Value> = caps
                .iter()
                .skip(1) // skip full match
                .map(|m| match m {
                    Some(m) => Value::Some(Box::new(Value::Text(m.as_str().to_string()))),
                    None => Value::None,
                })
                .collect();
            fields.insert("groups".to_string(), Value::List(groups));

            Ok(Value::Some(Box::new(Value::Record(fields))))
        }
        None => Ok(Value::None),
    }
}

/// Find all matches of a regex pattern in a string
pub(super) fn regex_find_all(pattern: &str, text: &str) -> Result<Value, RuntimeError> {
    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new("E4016", format!("Invalid regex pattern: {}", e)))?;

    let matches: Vec<Value> = re
        .find_iter(text)
        .map(|m| {
            let mut fields = HashMap::new();
            fields.insert("matched".to_string(), Value::Text(m.as_str().to_string()));
            fields.insert("start".to_string(), Value::Int(m.start() as i64));
            fields.insert("end".to_string(), Value::Int(m.end() as i64));
            Value::Record(fields)
        })
        .collect();

    Ok(Value::List(matches))
}

/// Replace all matches of a regex pattern in a string
pub(super) fn regex_replace(
    pattern: &str,
    text: &str,
    replacement: &str,
) -> Result<Value, RuntimeError> {
    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new("E4016", format!("Invalid regex pattern: {}", e)))?;

    Ok(Value::Text(re.replace_all(text, replacement).to_string()))
}

/// Split a string by a regex pattern
pub(super) fn regex_split(pattern: &str, text: &str) -> Result<Value, RuntimeError> {
    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new("E4016", format!("Invalid regex pattern: {}", e)))?;

    let parts: Vec<Value> = re.split(text).map(|s| Value::Text(s.to_string())).collect();

    Ok(Value::List(parts))
}

/// Check if a regex pattern matches a string (returns Bool)
pub(super) fn regex_is_match(pattern: &str, text: &str) -> Result<Value, RuntimeError> {
    let re = regex::Regex::new(pattern)
        .map_err(|e| RuntimeError::new("E4016", format!("Invalid regex pattern: {}", e)))?;

    Ok(Value::Bool(re.is_match(text)))
}
