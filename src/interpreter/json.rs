//! JSON parsing and stringifying for Astra values.

use std::collections::HashMap;

use super::error::RuntimeError;
use super::value::{format_value, Value};

/// Parse a JSON string into an Astra Value
pub fn json_parse_value(input: &str) -> Result<Value, RuntimeError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::new("E4016", "Empty JSON input"));
    }
    let (val, rest) = json_parse_inner(trimmed)?;
    let rest = rest.trim();
    if !rest.is_empty() {
        return Err(RuntimeError::new(
            "E4016",
            format!("Unexpected trailing content in JSON: {:?}", rest),
        ));
    }
    Ok(val)
}

/// Recursive JSON parser — returns (Value, remaining_input)
fn json_parse_inner(input: &str) -> Result<(Value, &str), RuntimeError> {
    let input = input.trim_start();
    if input.is_empty() {
        return Err(RuntimeError::new("E4016", "Unexpected end of JSON"));
    }

    match input.as_bytes()[0] {
        b'"' => json_parse_string(input),
        b'{' => json_parse_object(input),
        b'[' => json_parse_array(input),
        b't' | b'f' => json_parse_bool(input),
        b'n' => json_parse_null(input),
        b'-' | b'0'..=b'9' => json_parse_number(input),
        ch => Err(RuntimeError::new(
            "E4016",
            format!("Unexpected character in JSON: '{}'", ch as char),
        )),
    }
}

fn json_parse_string(input: &str) -> Result<(Value, &str), RuntimeError> {
    debug_assert!(input.starts_with('"'));
    let rest = &input[1..];
    let mut result = String::new();
    let mut chars = rest.char_indices();

    while let Some((i, ch)) = chars.next() {
        match ch {
            '"' => return Ok((Value::Text(result), &rest[i + 1..])),
            '\\' => {
                if let Some((_, esc)) = chars.next() {
                    match esc {
                        '"' => result.push('"'),
                        '\\' => result.push('\\'),
                        '/' => result.push('/'),
                        'n' => result.push('\n'),
                        'r' => result.push('\r'),
                        't' => result.push('\t'),
                        'b' => result.push('\u{0008}'),
                        'f' => result.push('\u{000C}'),
                        'u' => {
                            let mut hex = String::new();
                            for _ in 0..4 {
                                if let Some((_, h)) = chars.next() {
                                    hex.push(h);
                                }
                            }
                            if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                if let Some(c) = char::from_u32(code) {
                                    result.push(c);
                                }
                            }
                        }
                        _ => {
                            result.push('\\');
                            result.push(esc);
                        }
                    }
                }
            }
            _ => result.push(ch),
        }
    }
    Err(RuntimeError::new("E4016", "Unterminated JSON string"))
}

fn json_parse_object(input: &str) -> Result<(Value, &str), RuntimeError> {
    debug_assert!(input.starts_with('{'));
    let mut rest = input[1..].trim_start();
    let mut fields = HashMap::new();

    if let Some(stripped) = rest.strip_prefix('}') {
        return Ok((Value::Record(fields), stripped));
    }

    loop {
        // Parse key (must be a string)
        if !rest.starts_with('"') {
            return Err(RuntimeError::new(
                "E4016",
                "Expected string key in JSON object",
            ));
        }
        let (key_val, after_key) = json_parse_string(rest)?;
        let key = match key_val {
            Value::Text(s) => s,
            _ => unreachable!(),
        };
        rest = after_key.trim_start();

        // Expect colon
        if !rest.starts_with(':') {
            return Err(RuntimeError::new(
                "E4016",
                "Expected ':' after key in JSON object",
            ));
        }
        rest = rest[1..].trim_start();

        // Parse value
        let (val, after_val) = json_parse_inner(rest)?;
        fields.insert(key, val);
        rest = after_val.trim_start();

        if rest.starts_with(',') {
            rest = rest[1..].trim_start();
        } else if rest.starts_with('}') {
            rest = &rest[1..];
            break;
        } else {
            return Err(RuntimeError::new(
                "E4016",
                "Expected ',' or '}' in JSON object",
            ));
        }
    }

    Ok((Value::Record(fields), rest))
}

fn json_parse_array(input: &str) -> Result<(Value, &str), RuntimeError> {
    debug_assert!(input.starts_with('['));
    let mut rest = input[1..].trim_start();
    let mut elements = Vec::new();

    if let Some(stripped) = rest.strip_prefix(']') {
        return Ok((Value::List(elements), stripped));
    }

    loop {
        let (val, after_val) = json_parse_inner(rest)?;
        elements.push(val);
        rest = after_val.trim_start();

        if rest.starts_with(',') {
            rest = rest[1..].trim_start();
        } else if rest.starts_with(']') {
            rest = &rest[1..];
            break;
        } else {
            return Err(RuntimeError::new(
                "E4016",
                "Expected ',' or ']' in JSON array",
            ));
        }
    }

    Ok((Value::List(elements), rest))
}

fn json_parse_bool(input: &str) -> Result<(Value, &str), RuntimeError> {
    if let Some(rest) = input.strip_prefix("true") {
        Ok((Value::Bool(true), rest))
    } else if let Some(rest) = input.strip_prefix("false") {
        Ok((Value::Bool(false), rest))
    } else {
        Err(RuntimeError::new(
            "E4016",
            format!(
                "Invalid JSON value starting with '{}'",
                &input[..1.min(input.len())]
            ),
        ))
    }
}

fn json_parse_null(input: &str) -> Result<(Value, &str), RuntimeError> {
    if let Some(rest) = input.strip_prefix("null") {
        Ok((Value::None, rest))
    } else {
        Err(RuntimeError::new("E4016", "Invalid JSON null"))
    }
}

fn json_parse_number(input: &str) -> Result<(Value, &str), RuntimeError> {
    let mut end = 0;
    let bytes = input.as_bytes();
    let len = bytes.len();

    // Optional minus
    if end < len && bytes[end] == b'-' {
        end += 1;
    }
    // Digits
    while end < len && bytes[end].is_ascii_digit() {
        end += 1;
    }
    // Fractional part
    let mut is_float = false;
    if end < len && bytes[end] == b'.' {
        is_float = true;
        end += 1;
        while end < len && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    // Exponent
    if end < len && (bytes[end] == b'e' || bytes[end] == b'E') {
        is_float = true;
        end += 1;
        if end < len && (bytes[end] == b'+' || bytes[end] == b'-') {
            end += 1;
        }
        while end < len && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }

    let num_str = &input[..end];
    let rest = &input[end..];

    if is_float {
        match num_str.parse::<f64>() {
            Ok(f) => Ok((Value::Float(f), rest)),
            Err(_) => Err(RuntimeError::new(
                "E4016",
                format!("Invalid JSON number: {}", num_str),
            )),
        }
    } else {
        match num_str.parse::<i64>() {
            Ok(n) => Ok((Value::Int(n), rest)),
            Err(_) => {
                // Try as float if it doesn't fit in i64
                match num_str.parse::<f64>() {
                    Ok(f) => Ok((Value::Float(f), rest)),
                    Err(_) => Err(RuntimeError::new(
                        "E4016",
                        format!("Invalid JSON number: {}", num_str),
                    )),
                }
            }
        }
    }
}

/// Stringify an Astra value to a JSON string
pub fn json_stringify_value(value: &Value) -> String {
    match value {
        Value::Unit => "null".to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => {
            if f.is_infinite() || f.is_nan() {
                "null".to_string()
            } else {
                let s = f.to_string();
                // Ensure float representation
                if s.contains('.') || s.contains('e') || s.contains('E') {
                    s
                } else {
                    format!("{}.0", s)
                }
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Text(s) => {
            let escaped = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            format!("\"{}\"", escaped)
        }
        Value::None => "null".to_string(),
        Value::Some(inner) => json_stringify_value(inner),
        Value::Ok(inner) => json_stringify_value(inner),
        Value::Err(inner) => {
            // Represent as {"error": ...}
            format!("{{\"error\":{}}}", json_stringify_value(inner))
        }
        Value::Record(fields) => {
            let mut sorted_keys: Vec<&String> = fields.keys().collect();
            sorted_keys.sort();
            let parts: Vec<String> = sorted_keys
                .iter()
                .map(|k| {
                    format!(
                        "\"{}\":{}",
                        k,
                        json_stringify_value(fields.get(*k).unwrap())
                    )
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(json_stringify_value).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Tuple(elements) => {
            let parts: Vec<String> = elements.iter().map(json_stringify_value).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Map(entries) => {
            let parts: Vec<String> = entries
                .iter()
                .map(|(k, v)| {
                    let key_str = match k {
                        Value::Text(s) => s.clone(),
                        _ => format_value(k),
                    };
                    format!(
                        "\"{}\":{}",
                        key_str.replace('\\', "\\\\").replace('"', "\\\""),
                        json_stringify_value(v)
                    )
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Set(elements) => {
            let parts: Vec<String> = elements.iter().map(json_stringify_value).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Variant { name, data } => {
            if let Some(d) = data {
                format!(
                    "{{\"variant\":\"{}\",\"data\":{}}}",
                    name,
                    json_stringify_value(d)
                )
            } else {
                format!("\"{}\"", name)
            }
        }
        Value::Closure { .. } | Value::VariantConstructor { .. } | Value::Future { .. } => {
            "null".to_string()
        }
    }
}
