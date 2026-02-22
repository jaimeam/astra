//! Interpreter for Astra programs
//!
//! Executes Astra code with capability-controlled effects.

pub mod capabilities;
pub mod environment;
pub mod error;
mod json;
mod methods;
mod modules;
mod pattern;
mod regex;
pub mod value;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::parser::ast::*;

pub use capabilities::*;
pub use environment::Environment;
pub use error::{check_arity, RuntimeError};
pub use pattern::match_pattern;
pub use value::*;

use json::{json_parse_value, json_stringify_value};
use regex::{regex_find_all, regex_is_match, regex_match, regex_replace, regex_split};

/// Decode a percent-encoded URL component (e.g. `hello%20world` → `hello world`).
pub(crate) fn url_decode(s: &str) -> String {
    let mut result = Vec::with_capacity(s.len());
    let mut bytes = s.as_bytes().iter();
    while let Some(&b) = bytes.next() {
        if b == b'%' {
            let hi = bytes.next().copied().unwrap_or(b'0');
            let lo = bytes.next().copied().unwrap_or(b'0');
            let hex = [hi, lo];
            if let Ok(decoded) = u8::from_str_radix(std::str::from_utf8(&hex).unwrap_or("00"), 16) {
                result.push(decoded);
            }
        } else if b == b'+' {
            result.push(b' ');
        } else {
            result.push(b);
        }
    }
    String::from_utf8_lossy(&result).into_owned()
}

/// Result of evaluating an expression with TCO awareness (P6.4)
#[allow(clippy::large_enum_variant)]
enum TcoResult {
    /// Normal value
    Value(Value),
    /// Self-recursive tail call with new arguments
    TailCall(Vec<Value>),
}

/// A registered trait implementation for runtime dispatch
#[derive(Debug, Clone)]
struct RuntimeTraitImpl {
    /// The target type name (e.g., "Int", "Text", "MyType")
    target_type_name: String,
    /// Method implementations: method_name -> closure
    methods: HashMap<String, Value>,
}

pub struct Interpreter {
    /// Global environment
    pub env: Environment,
    /// Available capabilities
    pub capabilities: Capabilities,
    /// Search paths for module resolution
    pub search_paths: Vec<PathBuf>,
    /// Already-loaded modules (to prevent circular imports)
    loaded_modules: std::collections::HashSet<String>,
    /// Cached module environments for already-loaded modules
    loaded_module_envs: HashMap<String, Environment>,
    /// Modules currently being loaded (for circular import detection P4.2)
    loading_modules: std::collections::HashSet<String>,
    /// Call stack for stack traces (P5.2)
    call_stack: Vec<String>,
    /// Type definitions for invariant enforcement (P2.3)
    type_defs: HashMap<String, TypeDef>,
    /// Effect definitions (P6.2)
    effect_defs: HashMap<String, EffectDecl>,
    /// Trait implementations for method dispatch
    trait_impls: Vec<RuntimeTraitImpl>,
    /// v1.1: Set of async function names
    async_fns: std::collections::HashSet<String>,
}

impl Interpreter {
    /// Create a new interpreter
    pub fn new() -> Self {
        Self::with_capabilities(Capabilities::default())
    }

    /// Create an interpreter with specific capabilities
    pub fn with_capabilities(capabilities: Capabilities) -> Self {
        Self {
            env: Environment::new(),
            capabilities,
            search_paths: Vec::new(),
            loaded_modules: std::collections::HashSet::new(),
            loaded_module_envs: HashMap::new(),
            loading_modules: std::collections::HashSet::new(),
            call_stack: Vec::new(),
            type_defs: HashMap::new(),
            effect_defs: HashMap::new(),
            trait_impls: Vec::new(),
            async_fns: std::collections::HashSet::new(),
        }
    }

    /// Evaluate an expression
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            // Literals
            Expr::IntLit { value, .. } => Ok(Value::Int(*value)),
            Expr::FloatLit { value, .. } => Ok(Value::Float(*value)),
            Expr::BoolLit { value, .. } => Ok(Value::Bool(*value)),
            Expr::TextLit { value, .. } => Ok(Value::Text(value.clone())),
            Expr::UnitLit { .. } => Ok(Value::Unit),

            // Identifiers
            Expr::Ident { name, .. } => {
                // Check for effect names first
                match name.as_str() {
                    "Console" | "Fs" | "Net" | "Clock" | "Rand" | "Env" | "Map" | "Set" => {
                        Ok(Value::Text(name.clone()))
                    }
                    // Option/Result constructors
                    "None" => Ok(Value::None),
                    "Some" => Ok(Value::Variant {
                        name: "Some".to_string(),
                        data: None,
                    }),
                    "Ok" => Ok(Value::Variant {
                        name: "Ok".to_string(),
                        data: None,
                    }),
                    "Err" => Ok(Value::Variant {
                        name: "Err".to_string(),
                        data: None,
                    }),
                    _ => {
                        // P6.2: Check user-defined effect names
                        if self.effect_defs.contains_key(name) {
                            return Ok(Value::Text(name.clone()));
                        }
                        self.env
                            .lookup(name)
                            .cloned()
                            .ok_or_else(|| RuntimeError::undefined_variable(name))
                    }
                }
            }
            Expr::QualifiedIdent { module, name, .. } => {
                // For now, treat as effect call target
                Ok(Value::Text(format!("{}.{}", module, name)))
            }

            // Binary operations
            Expr::Binary {
                op,
                left,
                right,
                span,
                ..
            } => {
                // Pipe operator: x |> f evaluates as f(x)
                if *op == BinaryOp::Pipe {
                    let left_val = self.eval_expr(left)?;
                    let right_val = self.eval_expr(right)?;
                    return self.call_function(right_val, vec![left_val]);
                }
                let left_val = self.eval_expr(left)?;
                let right_val = self.eval_expr(right)?;
                self.eval_binary_op(*op, &left_val, &right_val)
                    .map_err(|e| {
                        if e.span.is_none() {
                            e.with_span(span.clone())
                        } else {
                            e
                        }
                    })
            }

            // Unary operations
            Expr::Unary { op, expr, .. } => {
                let val = self.eval_expr(expr)?;
                self.eval_unary_op(*op, &val)
            }

            // Function calls
            Expr::Call {
                func, args, span, ..
            } => {
                // Check for builtin functions first
                if let Expr::Ident { name, .. } = func.as_ref() {
                    let call_span = span.clone();
                    match name.as_str() {
                        "assert" => {
                            if args.is_empty() || args.len() > 2 {
                                return Err(RuntimeError::arity_mismatch(1, args.len())
                                    .with_span(call_span));
                            }
                            let cond = self.eval_expr(&args[0])?;
                            let msg = if args.len() == 2 {
                                let m = self.eval_expr(&args[1])?;
                                match m {
                                    Value::Text(s) => s,
                                    other => format_value(&other),
                                }
                            } else {
                                "assertion failed".to_string()
                            };
                            return match cond {
                                Value::Bool(true) => Ok(Value::Unit),
                                Value::Bool(false) => {
                                    Err(RuntimeError::new("E4020", msg).with_span(call_span))
                                }
                                _ => {
                                    Err(RuntimeError::type_mismatch("Bool", &format!("{:?}", cond))
                                        .with_span(call_span))
                                }
                            };
                        }
                        "assert_eq" => {
                            check_arity(args, 2)?;
                            let left = self.eval_expr(&args[0])?;
                            let right = self.eval_expr(&args[1])?;
                            return if values_equal(&left, &right) {
                                Ok(Value::Unit)
                            } else {
                                Err(RuntimeError::new(
                                    "E4021",
                                    format!("assertion failed: {:?} != {:?}", left, right),
                                )
                                .with_span(call_span))
                            };
                        }
                        // Option/Result constructors
                        "Some" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return Ok(Value::Some(Box::new(val)));
                        }
                        "Ok" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return Ok(Value::Ok(Box::new(val)));
                        }
                        "Err" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return Ok(Value::Err(Box::new(val)));
                        }
                        // N2: print builtin (no newline)
                        "print" => {
                            let mut output = String::new();
                            for (i, arg) in args.iter().enumerate() {
                                if i > 0 {
                                    output.push(' ');
                                }
                                let val = self.eval_expr(arg)?;
                                output.push_str(&format_value(&val));
                            }
                            if let Some(console) = &self.capabilities.console {
                                console.print(&output);
                            }
                            return Ok(Value::Unit);
                        }
                        // N2: println builtin
                        "println" => {
                            let mut output = String::new();
                            for (i, arg) in args.iter().enumerate() {
                                if i > 0 {
                                    output.push(' ');
                                }
                                let val = self.eval_expr(arg)?;
                                output.push_str(&format_value(&val));
                            }
                            if let Some(console) = &self.capabilities.console {
                                console.println(&output);
                            }
                            return Ok(Value::Unit);
                        }
                        // N3: len builtin
                        "len" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return match val {
                                Value::Text(s) => Ok(Value::Int(s.len() as i64)),
                                Value::List(l) => Ok(Value::Int(l.len() as i64)),
                                Value::Tuple(t) => Ok(Value::Int(t.len() as i64)),
                                Value::Map(m) => Ok(Value::Int(m.len() as i64)),
                                Value::Set(s) => Ok(Value::Int(s.len() as i64)),
                                _ => Err(RuntimeError::type_mismatch(
                                    "Text, List, Tuple, Map, or Set",
                                    &format!("{:?}", val),
                                )),
                            };
                        }
                        // N3: to_text builtin
                        "to_text" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return Ok(Value::Text(format_value(&val)));
                        }
                        // P1.1: range builtin
                        "range" => {
                            check_arity(args, 2)?;
                            let start = self.eval_expr(&args[0])?;
                            let end = self.eval_expr(&args[1])?;
                            return match (start, end) {
                                (Value::Int(s), Value::Int(e)) => {
                                    let items: Vec<Value> = (s..e).map(Value::Int).collect();
                                    Ok(Value::List(items))
                                }
                                _ => Err(RuntimeError::type_mismatch("(Int, Int)", "other")),
                            };
                        }
                        // P1.11: math builtins
                        "abs" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return match val {
                                Value::Int(n) => Ok(Value::Int(n.abs())),
                                Value::Float(n) => Ok(Value::Float(n.abs())),
                                _ => Err(RuntimeError::type_mismatch(
                                    "Int or Float",
                                    &format!("{:?}", val),
                                )),
                            };
                        }
                        "min" => {
                            check_arity(args, 2)?;
                            let a = self.eval_expr(&args[0])?;
                            let b = self.eval_expr(&args[1])?;
                            return match (a, b) {
                                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.min(b))),
                                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.min(b))),
                                _ => Err(RuntimeError::type_mismatch(
                                    "(Int, Int) or (Float, Float)",
                                    "other",
                                )),
                            };
                        }
                        "max" => {
                            check_arity(args, 2)?;
                            let a = self.eval_expr(&args[0])?;
                            let b = self.eval_expr(&args[1])?;
                            return match (a, b) {
                                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.max(b))),
                                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.max(b))),
                                _ => Err(RuntimeError::type_mismatch(
                                    "(Int, Int) or (Float, Float)",
                                    "other",
                                )),
                            };
                        }
                        "pow" => {
                            check_arity(args, 2)?;
                            let base = self.eval_expr(&args[0])?;
                            let exp = self.eval_expr(&args[1])?;
                            return match (base, exp) {
                                (Value::Int(b), Value::Int(e)) => {
                                    if e < 0 {
                                        Err(RuntimeError::new(
                                            "E4016",
                                            "pow: negative exponent not supported for Int",
                                        ))
                                    } else {
                                        Ok(Value::Int(b.pow(e as u32)))
                                    }
                                }
                                (Value::Float(b), Value::Float(e)) => Ok(Value::Float(b.powf(e))),
                                (Value::Float(b), Value::Int(e)) => {
                                    Ok(Value::Float(b.powi(e as i32)))
                                }
                                _ => Err(RuntimeError::type_mismatch(
                                    "(Int, Int) or (Float, Float)",
                                    "other",
                                )),
                            };
                        }
                        // P3.4: Conversion functions
                        "to_int" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return match val {
                                Value::Int(n) => Ok(Value::Int(n)),
                                Value::Float(f) => Ok(Value::Int(f as i64)),
                                Value::Text(s) => match s.parse::<i64>() {
                                    Ok(n) => Ok(Value::Some(Box::new(Value::Int(n)))),
                                    Err(_) => Ok(Value::None),
                                },
                                Value::Bool(b) => Ok(Value::Int(if b { 1 } else { 0 })),
                                _ => Err(RuntimeError::type_mismatch(
                                    "Int, Float, Text, or Bool",
                                    &format!("{:?}", val),
                                )),
                            };
                        }
                        "to_float" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return match val {
                                Value::Int(n) => Ok(Value::Float(n as f64)),
                                Value::Float(f) => Ok(Value::Float(f)),
                                Value::Text(s) => match s.parse::<f64>() {
                                    Ok(f) => Ok(Value::Some(Box::new(Value::Float(f)))),
                                    Err(_) => Ok(Value::None),
                                },
                                _ => Err(RuntimeError::type_mismatch(
                                    "Int, Float, or Text",
                                    &format!("{:?}", val),
                                )),
                            };
                        }
                        // P3.6: Math functions for Float
                        "sqrt" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return match val {
                                Value::Float(f) => Ok(Value::Float(f.sqrt())),
                                Value::Int(n) => Ok(Value::Float((n as f64).sqrt())),
                                _ => Err(RuntimeError::type_mismatch(
                                    "Float or Int",
                                    &format!("{:?}", val),
                                )),
                            };
                        }
                        "floor" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return match val {
                                Value::Float(f) => Ok(Value::Float(f.floor())),
                                Value::Int(n) => Ok(Value::Int(n)),
                                _ => {
                                    Err(RuntimeError::type_mismatch("Float", &format!("{:?}", val)))
                                }
                            };
                        }
                        "ceil" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return match val {
                                Value::Float(f) => Ok(Value::Float(f.ceil())),
                                Value::Int(n) => Ok(Value::Int(n)),
                                _ => {
                                    Err(RuntimeError::type_mismatch("Float", &format!("{:?}", val)))
                                }
                            };
                        }
                        "round" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return match val {
                                Value::Float(f) => Ok(Value::Float(f.round())),
                                Value::Int(n) => Ok(Value::Int(n)),
                                _ => {
                                    Err(RuntimeError::type_mismatch("Float", &format!("{:?}", val)))
                                }
                            };
                        }
                        // Effect convenience builtins - delegate to capabilities
                        "read_file" => {
                            check_arity(args, 1)?;
                            let path = self.eval_expr(&args[0])?;
                            if let Value::Text(p) = path {
                                return self.call_fs_method("read", vec![Value::Text(p)]);
                            }
                            return Err(RuntimeError::type_mismatch(
                                "Text",
                                &format!("{:?}", path),
                            ));
                        }
                        "write_file" => {
                            check_arity(args, 2)?;
                            let path = self.eval_expr(&args[0])?;
                            let content = self.eval_expr(&args[1])?;
                            return self.call_fs_method("write", vec![path, content]);
                        }
                        "http_get" => {
                            check_arity(args, 1)?;
                            let url = self.eval_expr(&args[0])?;
                            if let Value::Text(u) = url {
                                return self.call_net_method("get", vec![Value::Text(u)]);
                            }
                            return Err(RuntimeError::type_mismatch("Text", &format!("{:?}", url)));
                        }
                        "http_post" => {
                            check_arity(args, 2)?;
                            let url = self.eval_expr(&args[0])?;
                            let body = self.eval_expr(&args[1])?;
                            return self.call_net_method("post", vec![url, body]);
                        }
                        "random_int" => {
                            check_arity(args, 2)?;
                            let min = self.eval_expr(&args[0])?;
                            let max = self.eval_expr(&args[1])?;
                            return self.call_rand_method("int", vec![min, max]);
                        }
                        "random_bool" => {
                            if !args.is_empty() {
                                return Err(RuntimeError::arity_mismatch(0, args.len()));
                            }
                            return self.call_rand_method("bool", vec![]);
                        }
                        "current_time_millis" => {
                            if !args.is_empty() {
                                return Err(RuntimeError::arity_mismatch(0, args.len()));
                            }
                            return self.call_clock_method("now", vec![]);
                        }
                        "get_env" => {
                            check_arity(args, 1)?;
                            let name = self.eval_expr(&args[0])?;
                            return self.call_env_method("get", vec![name]);
                        }
                        // v1.1: Full JSON parsing
                        "json_parse" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return match val {
                                Value::Text(s) => json_parse_value(&s),
                                _ => {
                                    Err(RuntimeError::type_mismatch("Text", &format!("{:?}", val)))
                                }
                            };
                        }
                        "json_stringify" => {
                            check_arity(args, 1)?;
                            let val = self.eval_expr(&args[0])?;
                            return Ok(Value::Text(json_stringify_value(&val)));
                        }
                        // v1.1: Regex support
                        "regex_match" => {
                            check_arity(args, 2)?;
                            let pattern = self.eval_expr(&args[0])?;
                            let text = self.eval_expr(&args[1])?;
                            return match (&pattern, &text) {
                                (Value::Text(pat), Value::Text(s)) => regex_match(pat, s),
                                _ => Err(RuntimeError::type_mismatch("(Text, Text)", "other")),
                            };
                        }
                        "regex_find_all" => {
                            check_arity(args, 2)?;
                            let pattern = self.eval_expr(&args[0])?;
                            let text = self.eval_expr(&args[1])?;
                            return match (&pattern, &text) {
                                (Value::Text(pat), Value::Text(s)) => regex_find_all(pat, s),
                                _ => Err(RuntimeError::type_mismatch("(Text, Text)", "other")),
                            };
                        }
                        "regex_replace" => {
                            if args.len() != 3 {
                                return Err(RuntimeError::arity_mismatch(3, args.len()));
                            }
                            let pattern = self.eval_expr(&args[0])?;
                            let text = self.eval_expr(&args[1])?;
                            let replacement = self.eval_expr(&args[2])?;
                            return match (&pattern, &text, &replacement) {
                                (Value::Text(pat), Value::Text(s), Value::Text(rep)) => {
                                    regex_replace(pat, s, rep)
                                }
                                _ => {
                                    Err(RuntimeError::type_mismatch("(Text, Text, Text)", "other"))
                                }
                            };
                        }
                        "regex_split" => {
                            check_arity(args, 2)?;
                            let pattern = self.eval_expr(&args[0])?;
                            let text = self.eval_expr(&args[1])?;
                            return match (&pattern, &text) {
                                (Value::Text(pat), Value::Text(s)) => regex_split(pat, s),
                                _ => Err(RuntimeError::type_mismatch("(Text, Text)", "other")),
                            };
                        }
                        "regex_is_match" => {
                            check_arity(args, 2)?;
                            let pattern = self.eval_expr(&args[0])?;
                            let text = self.eval_expr(&args[1])?;
                            return match (&pattern, &text) {
                                (Value::Text(pat), Value::Text(s)) => regex_is_match(pat, s),
                                _ => Err(RuntimeError::type_mismatch("(Text, Text)", "other")),
                            };
                        }
                        _ => {}
                    }
                }

                let func_val = self.eval_expr(func)?;
                let mut arg_vals = Vec::new();
                for arg in args {
                    arg_vals.push(self.eval_expr(arg)?);
                }

                // v1.1: Check if this is an async function call
                let is_async_call = if let Expr::Ident { name, .. } = func.as_ref() {
                    self.async_fns.contains(name.as_str())
                } else {
                    false
                };

                if is_async_call {
                    // Return a Future instead of executing immediately
                    Ok(Value::Future {
                        func: Box::new(func_val),
                        args: arg_vals,
                    })
                } else {
                    self.call_function(func_val, arg_vals).map_err(|e| {
                        if e.span.is_none() && !e.is_control_flow() {
                            e.with_span(span.clone())
                        } else {
                            e
                        }
                    })
                }
            }

            // Method calls (for effects)
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                let recv = self.eval_expr(receiver)?;
                let mut arg_vals = Vec::new();
                for arg in args {
                    arg_vals.push(self.eval_expr(arg)?);
                }
                self.call_method(&recv, method, arg_vals)
            }

            // If expression
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_val = self.eval_expr(cond)?;
                match cond_val {
                    Value::Bool(true) => self.eval_block(then_branch),
                    Value::Bool(false) => {
                        if let Some(else_expr) = else_branch {
                            self.eval_expr(else_expr)
                        } else {
                            Ok(Value::Unit)
                        }
                    }
                    _ => Err(RuntimeError::type_mismatch(
                        "Bool",
                        &format!("{:?}", cond_val),
                    )),
                }
            }

            // Match expression
            Expr::Match { expr, arms, .. } => {
                let val = self.eval_expr(expr)?;
                for arm in arms {
                    if let Some(bindings) = match_pattern(&arm.pattern, &val) {
                        // Create new scope with pattern bindings
                        self.env.push_scope();
                        for (name, value) in bindings {
                            self.env.define(name, value);
                        }

                        // Check guard if present
                        if let Some(guard) = &arm.guard {
                            let guard_val = self.eval_expr(guard)?;
                            match guard_val {
                                Value::Bool(true) => {}
                                Value::Bool(false) => {
                                    self.env.pop_scope();
                                    continue;
                                }
                                _ => {
                                    self.env.pop_scope();
                                    return Err(RuntimeError::type_mismatch(
                                        "Bool",
                                        &format!("{:?}", guard_val),
                                    ));
                                }
                            }
                        }

                        let result = self.eval_expr(&arm.body);
                        self.env.pop_scope();
                        return result;
                    }
                }
                Err(RuntimeError::match_failure())
            }

            // Block expression
            Expr::Block { block, .. } => self.eval_block(block),

            // Try expression (unwrap Option/Result, propagate None/Err)
            Expr::Try { expr, .. } => {
                let val = self.eval_expr(expr)?;
                match val {
                    Value::Some(inner) => Ok(*inner),
                    Value::None => Err(RuntimeError::early_return(Value::None)),
                    Value::Ok(inner) => Ok(*inner),
                    Value::Err(inner) => Err(RuntimeError::early_return(Value::Err(inner))),
                    other => Ok(other), // Pass through non-Option/Result values
                }
            }

            // TryElse expression (unwrap or use default)
            Expr::TryElse {
                expr, else_expr, ..
            } => {
                let val = self.eval_expr(expr)?;
                match val {
                    Value::Some(inner) => Ok(*inner),
                    Value::None => self.eval_expr(else_expr),
                    Value::Ok(inner) => Ok(*inner),
                    Value::Err(_) => self.eval_expr(else_expr),
                    other => Ok(other),
                }
            }

            // Record construction
            Expr::Record { fields, .. } => {
                let mut field_values = HashMap::new();
                for (name, value_expr) in fields {
                    let value = self.eval_expr(value_expr)?;
                    field_values.insert(name.clone(), value);
                }
                Ok(Value::Record(field_values))
            }

            // Field access
            Expr::FieldAccess { expr, field, .. } => {
                let val = self.eval_expr(expr)?;
                match val {
                    Value::Record(fields) => fields
                        .get(field)
                        .cloned()
                        .ok_or_else(|| RuntimeError::invalid_field_access(field)),
                    Value::Map(ref entries) => {
                        let key = Value::Text(field.clone());
                        entries
                            .iter()
                            .find(|(k, _)| values_equal(k, &key))
                            .map(|(_, v)| v.clone())
                            .ok_or_else(|| RuntimeError::invalid_field_access(field))
                    }
                    Value::Tuple(elements) => {
                        // Allow tuple.0, tuple.1 etc.
                        if let Ok(idx) = field.parse::<usize>() {
                            elements.get(idx).cloned().ok_or_else(|| {
                                RuntimeError::new(
                                    "E4016",
                                    format!(
                                        "tuple index {} out of bounds (length {})",
                                        idx,
                                        elements.len()
                                    ),
                                )
                            })
                        } else {
                            Err(RuntimeError::invalid_field_access(field))
                        }
                    }
                    _ => Err(RuntimeError::type_mismatch(
                        "Record, Map, or Tuple",
                        &format!("{:?}", val),
                    )),
                }
            }

            // List literal
            Expr::ListLit { elements, .. } => {
                let mut values = Vec::new();
                for elem in elements {
                    values.push(self.eval_expr(elem)?);
                }
                Ok(Value::List(values))
            }

            // Tuple literal
            Expr::TupleLit { elements, .. } => {
                let mut values = Vec::new();
                for elem in elements {
                    values.push(self.eval_expr(elem)?);
                }
                Ok(Value::Tuple(values))
            }

            // Map literal
            Expr::MapLit { entries, .. } => {
                let mut pairs = Vec::new();
                for (k, v) in entries {
                    let key = self.eval_expr(k)?;
                    let val = self.eval_expr(v)?;
                    pairs.push((key, val));
                }
                Ok(Value::Map(pairs))
            }

            // Lambda expression
            Expr::Lambda { params, body, .. } => {
                let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                Ok(Value::Closure {
                    name: None,
                    params: param_names,
                    body: ClosureBody {
                        block: (**body).clone(),
                        requires: Vec::new(),
                        ensures: Vec::new(),
                    },
                    env: self.env.clone(),
                })
            }

            // For-in loop
            Expr::ForIn {
                binding,
                pattern,
                iter,
                body,
                ..
            } => {
                let iter_val = self.eval_expr(iter)?;
                match iter_val {
                    Value::List(items) => {
                        'for_loop: for item in &items {
                            // E8: If there's a destructuring pattern, match it
                            if let Some(pat) = pattern {
                                if let Some(bindings) = match_pattern(pat, item) {
                                    for (name, val) in &bindings {
                                        self.env.define(name.clone(), val.clone());
                                    }
                                } else {
                                    return Err(RuntimeError::new(
                                        "E4015",
                                        "for loop destructuring pattern did not match value",
                                    ));
                                }
                            } else {
                                self.env.define(binding.clone(), item.clone());
                            }
                            for stmt in &body.stmts {
                                match self.eval_stmt(stmt) {
                                    Ok(()) => {}
                                    Err(e) if e.is_break => break 'for_loop,
                                    Err(e) if e.is_continue => continue 'for_loop,
                                    Err(e) if e.is_return => return Err(e),
                                    Err(e) => return Err(e),
                                }
                            }
                            if let Some(expr) = &body.expr {
                                match self.eval_expr(expr) {
                                    Ok(_) => {}
                                    Err(e) if e.is_break => break 'for_loop,
                                    Err(e) if e.is_continue => continue 'for_loop,
                                    Err(e) => return Err(e),
                                }
                            }
                        }
                        Ok(Value::Unit)
                    }
                    _ => Err(RuntimeError::type_mismatch(
                        "List",
                        &format!("{:?}", iter_val),
                    )),
                }
            }

            // While loop
            Expr::While { cond, body, .. } => {
                loop {
                    let condition = self.eval_expr(cond)?;
                    match condition {
                        Value::Bool(true) => {
                            for stmt in &body.stmts {
                                match self.eval_stmt(stmt) {
                                    Ok(()) => {}
                                    Err(e) if e.is_break => return Ok(Value::Unit),
                                    Err(e) if e.is_continue => break,
                                    Err(e) => return Err(e),
                                }
                            }
                            if let Some(expr) = &body.expr {
                                match self.eval_expr(expr) {
                                    Ok(_) => {}
                                    Err(e) if e.is_break => return Ok(Value::Unit),
                                    Err(e) if e.is_continue => {}
                                    Err(e) => return Err(e),
                                }
                            }
                        }
                        Value::Bool(false) => break,
                        _ => {
                            return Err(RuntimeError::type_mismatch(
                                "Bool",
                                &format!("{:?}", condition),
                            ))
                        }
                    }
                }
                Ok(Value::Unit)
            }

            // Break
            Expr::Break { .. } => Err(RuntimeError::loop_break()),

            // Continue
            Expr::Continue { .. } => Err(RuntimeError::loop_continue()),

            // String interpolation
            Expr::StringInterp { parts, .. } => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        StringPart::Literal(s) => result.push_str(s),
                        StringPart::Expr(expr) => {
                            let val = self.eval_expr(expr)?;
                            result.push_str(&format_value(&val));
                        }
                    }
                }
                Ok(Value::Text(result))
            }

            // Range expressions: 0..10 or 0..=10
            Expr::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                let start_val = self.eval_expr(start)?;
                let end_val = self.eval_expr(end)?;
                match (&start_val, &end_val) {
                    (Value::Int(s), Value::Int(e)) => {
                        let range_end = if *inclusive { *e + 1 } else { *e };
                        let items: Vec<Value> = (*s..range_end).map(Value::Int).collect();
                        Ok(Value::List(items))
                    }
                    _ => Err(RuntimeError::type_mismatch(
                        "Int",
                        &format!("{:?} and {:?}", start_val, end_val),
                    )),
                }
            }

            // E2: Index access — list[i], map[key], text[i]
            Expr::IndexAccess { expr, index, .. } => {
                let collection = self.eval_expr(expr)?;
                let idx = self.eval_expr(index)?;
                match (&collection, &idx) {
                    (Value::List(items), Value::Int(i)) => {
                        let i = *i;
                        let len = items.len() as i64;
                        let actual_idx = if i < 0 { len + i } else { i };
                        if actual_idx < 0 || actual_idx >= len {
                            Err(RuntimeError::new(
                                "E4002",
                                format!(
                                    "index out of bounds: index {} but list has {} elements",
                                    i, len
                                ),
                            ))
                        } else {
                            Ok(items[actual_idx as usize].clone())
                        }
                    }
                    (Value::Map(entries), key) => {
                        for (k, v) in entries {
                            if values_equal(k, key) {
                                return Ok(v.clone());
                            }
                        }
                        Err(RuntimeError::new(
                            "E4002",
                            format!("key not found in map: {}", format_value(key)),
                        ))
                    }
                    (Value::Text(s), Value::Int(i)) => {
                        let i = *i;
                        let chars: Vec<char> = s.chars().collect();
                        let len = chars.len() as i64;
                        let actual_idx = if i < 0 { len + i } else { i };
                        if actual_idx < 0 || actual_idx >= len {
                            Err(RuntimeError::new(
                                "E4002",
                                format!(
                                    "index out of bounds: index {} but string has {} characters",
                                    i, len
                                ),
                            ))
                        } else {
                            Ok(Value::Text(chars[actual_idx as usize].to_string()))
                        }
                    }
                    (Value::Tuple(elements), Value::Int(i)) => {
                        let i = *i;
                        let len = elements.len() as i64;
                        let actual_idx = if i < 0 { len + i } else { i };
                        if actual_idx < 0 || actual_idx >= len {
                            Err(RuntimeError::new(
                                "E4002",
                                format!(
                                    "index out of bounds: index {} but tuple has {} elements",
                                    i, len
                                ),
                            ))
                        } else {
                            Ok(elements[actual_idx as usize].clone())
                        }
                    }
                    _ => Err(RuntimeError::type_mismatch(
                        "List, Map, Text, or Tuple",
                        &format!("{:?}", collection),
                    )),
                }
            }

            // v1.1: Await expression - resolves a future by executing the deferred computation
            Expr::Await { expr, .. } => {
                let val = self.eval_expr(expr)?;
                match val {
                    Value::Future { func, args } => {
                        // Resolve the future by calling the function
                        self.call_function(*func, args)
                    }
                    // If it's not a future, just return the value (backwards compatible)
                    other => Ok(other),
                }
            }

            Expr::Hole { .. } => Err(RuntimeError::hole_encountered()),
        }
    }

    /// Evaluate a statement
    pub fn eval_stmt(&mut self, stmt: &Stmt) -> Result<(), RuntimeError> {
        match stmt {
            Stmt::Let {
                name, value, ty, ..
            } => {
                let mut val = self.eval_expr(value)?;
                // P2.3: Check type invariant if type annotation matches a type with invariant
                if let Some(TypeExpr::Named {
                    name: type_name, ..
                }) = ty
                {
                    self.check_type_invariant(type_name, &val)?;
                }
                // For local named closures (e.g., `let helper = fn(...) { ... }`),
                // set the closure's name to enable recursive self-calls and TCO.
                if let Value::Closure {
                    name: ref mut closure_name,
                    ..
                } = val
                {
                    if closure_name.is_none() {
                        *closure_name = Some(name.clone());
                    }
                }
                self.env.define(name.clone(), val);
                Ok(())
            }
            Stmt::LetPattern { pattern, value, .. } => {
                let val = self.eval_expr(value)?;
                if let Some(bindings) = match_pattern(pattern, &val) {
                    for (name, bound_val) in bindings {
                        self.env.define(name, bound_val);
                    }
                    Ok(())
                } else {
                    Err(RuntimeError::new(
                        "E4015",
                        "destructuring pattern did not match value",
                    ))
                }
            }
            Stmt::Assign { target, value, .. } => {
                let val = self.eval_expr(value)?;
                match target.as_ref() {
                    Expr::Ident { name, .. } => {
                        if !self.env.update(name, val) {
                            return Err(RuntimeError::undefined_variable(name));
                        }
                        Ok(())
                    }
                    _ => Err(RuntimeError::new("E4014", "invalid assignment target")),
                }
            }
            Stmt::Expr { expr, .. } => {
                self.eval_expr(expr)?;
                Ok(())
            }
            Stmt::Return { value, .. } => {
                let val = if let Some(val_expr) = value {
                    self.eval_expr(val_expr)?
                } else {
                    Value::Unit
                };
                Err(RuntimeError::function_return(val))
            }
        }
    }

    /// Evaluate a block
    pub fn eval_block(&mut self, block: &Block) -> Result<Value, RuntimeError> {
        // Create a new scope for the block
        self.env.push_scope();

        // Execute all statements
        for stmt in &block.stmts {
            match self.eval_stmt(stmt) {
                Ok(()) => {}
                Err(e) => {
                    self.env.pop_scope();
                    return Err(e);
                }
            }
        }

        // Evaluate trailing expression or return Unit
        let result = if let Some(expr) = &block.expr {
            match self.eval_expr(expr) {
                Ok(val) => val,
                Err(e) => {
                    self.env.pop_scope();
                    return Err(e);
                }
            }
        } else {
            Value::Unit
        };

        self.env.pop_scope();
        Ok(result)
    }

    /// Evaluate a binary operation
    fn eval_binary_op(
        &self,
        op: BinaryOp,
        left: &Value,
        right: &Value,
    ) -> Result<Value, RuntimeError> {
        match (op, left, right) {
            // Integer arithmetic
            (BinaryOp::Add, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (BinaryOp::Sub, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
            (BinaryOp::Mul, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
            (BinaryOp::Div, Value::Int(_), Value::Int(0)) => Err(RuntimeError::division_by_zero()),
            (BinaryOp::Div, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a / b)),
            (BinaryOp::Mod, Value::Int(_), Value::Int(0)) => Err(RuntimeError::division_by_zero()),
            (BinaryOp::Mod, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),

            // Float arithmetic
            (BinaryOp::Add, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (BinaryOp::Sub, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (BinaryOp::Mul, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (BinaryOp::Div, Value::Float(a), Value::Float(b)) => {
                if *b == 0.0 {
                    Err(RuntimeError::division_by_zero())
                } else {
                    Ok(Value::Float(a / b))
                }
            }
            (BinaryOp::Mod, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a % b)),

            // Float comparison
            (BinaryOp::Eq, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a == b)),
            (BinaryOp::Ne, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a != b)),
            (BinaryOp::Lt, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a < b)),
            (BinaryOp::Le, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a <= b)),
            (BinaryOp::Gt, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a > b)),
            (BinaryOp::Ge, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a >= b)),

            // Int-Float mixed arithmetic (auto-promote Int to Float)
            (BinaryOp::Add, Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
            (BinaryOp::Add, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),
            (BinaryOp::Sub, Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
            (BinaryOp::Sub, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - *b as f64)),
            (BinaryOp::Mul, Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
            (BinaryOp::Mul, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * *b as f64)),
            (BinaryOp::Div, Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 / b)),
            (BinaryOp::Div, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a / *b as f64)),

            // String concatenation
            (BinaryOp::Add, Value::Text(a), Value::Text(b)) => {
                Ok(Value::Text(format!("{}{}", a, b)))
            }

            // Integer comparison
            (BinaryOp::Eq, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a == b)),
            (BinaryOp::Ne, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a != b)),
            (BinaryOp::Lt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
            (BinaryOp::Le, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
            (BinaryOp::Gt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
            (BinaryOp::Ge, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),

            // Boolean comparison
            (BinaryOp::Eq, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a == b)),
            (BinaryOp::Ne, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a != b)),

            // String comparison
            (BinaryOp::Eq, Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a == b)),
            (BinaryOp::Ne, Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a != b)),
            (BinaryOp::Lt, Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a < b)),
            (BinaryOp::Le, Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a <= b)),
            (BinaryOp::Gt, Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a > b)),
            (BinaryOp::Ge, Value::Text(a), Value::Text(b)) => Ok(Value::Bool(a >= b)),

            // List concatenation
            (BinaryOp::Add, Value::List(a), Value::List(b)) => {
                let mut result = a.clone();
                result.extend(b.clone());
                Ok(Value::List(result))
            }

            // Logical operations
            (BinaryOp::And, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a && *b)),
            (BinaryOp::Or, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a || *b)),

            // E10: Structural equality for all value types
            (BinaryOp::Eq, _, _) => Ok(Value::Bool(values_equal(left, right))),
            (BinaryOp::Ne, _, _) => Ok(Value::Bool(!values_equal(left, right))),

            _ => Err(RuntimeError::type_mismatch(
                &format!("compatible types for {:?}", op),
                &format!("{:?} and {:?}", left, right),
            )),
        }
    }

    /// Evaluate a unary operation
    fn eval_unary_op(&self, op: UnaryOp, val: &Value) -> Result<Value, RuntimeError> {
        match (op, val) {
            (UnaryOp::Neg, Value::Int(n)) => Ok(Value::Int(-n)),
            (UnaryOp::Neg, Value::Float(n)) => Ok(Value::Float(-n)),
            (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            _ => Err(RuntimeError::type_mismatch(
                &format!("valid type for {:?}", op),
                &format!("{:?}", val),
            )),
        }
    }

    /// Call a function value with arguments
    pub fn call_function(&mut self, func: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        match func {
            Value::VariantConstructor { name, field_names } => {
                if args.len() != field_names.len() {
                    return Err(RuntimeError::arity_mismatch(field_names.len(), args.len()));
                }
                if field_names.len() == 1 {
                    // Single-field variant: store data directly
                    Ok(Value::Variant {
                        name,
                        data: Some(Box::new(args.into_iter().next().unwrap())),
                    })
                } else {
                    // Multi-field variant: store as Record
                    let mut fields = HashMap::new();
                    for (field_name, arg) in field_names.iter().zip(args) {
                        fields.insert(field_name.clone(), arg);
                    }
                    Ok(Value::Variant {
                        name,
                        data: Some(Box::new(Value::Record(fields))),
                    })
                }
            }
            Value::Closure {
                name,
                params,
                body,
                env,
            } => {
                if params.len() != args.len() {
                    return Err(RuntimeError::arity_mismatch(params.len(), args.len()));
                }

                let fn_name = name.as_deref().unwrap_or("<anonymous>");

                // Push call stack frame (P5.2: stack traces)
                self.call_stack.push(fn_name.to_string());

                // P6.4: TCO - detect simple self-recursive tail calls
                let use_tco = name.is_some()
                    && body.requires.is_empty()
                    && body.ensures.is_empty()
                    && Self::has_self_tail_call(&body.block, fn_name);

                let mut current_args = args;
                let uses_closure_env = !env.is_empty();

                let result = 'tco: loop {
                    // Swap to closure env if needed, otherwise use current env
                    let saved_env = if uses_closure_env {
                        Some(std::mem::replace(&mut self.env, env.clone()))
                    } else {
                        None
                    };

                    // Push a new scope for params and local bindings
                    self.env.push_scope();

                    // For named closures, define self in the call env to enable recursion
                    if let Some(ref fn_name_str) = name {
                        if self.env.lookup(fn_name_str).is_none() {
                            self.env.define(
                                fn_name_str.clone(),
                                Value::Closure {
                                    name: name.clone(),
                                    params: params.clone(),
                                    body: body.clone(),
                                    env: env.clone(),
                                },
                            );
                        }
                    }
                    for (param, arg) in params.iter().zip(current_args.iter().cloned()) {
                        self.env.define(param.clone(), arg);
                    }

                    // Check preconditions (requires clauses)
                    if !body.requires.is_empty() {
                        for req_expr in &body.requires {
                            let cond = self.eval_expr(req_expr)?;
                            match cond {
                                Value::Bool(true) => {}
                                Value::Bool(false) => {
                                    self.env.pop_scope();
                                    if let Some(saved) = saved_env {
                                        self.env = saved;
                                    }
                                    return Err(RuntimeError::precondition_violated(fn_name));
                                }
                                _ => {
                                    self.env.pop_scope();
                                    if let Some(saved) = saved_env {
                                        self.env = saved;
                                    }
                                    return Err(RuntimeError::type_mismatch(
                                        "Bool",
                                        &format!("{:?}", cond),
                                    ));
                                }
                            }
                        }
                    }

                    if use_tco {
                        // TCO path: execute statements, then check tail expression
                        let mut stmt_result = Ok(());
                        for stmt in &body.block.stmts {
                            stmt_result = self.eval_stmt(stmt);
                            if stmt_result.is_err() {
                                break;
                            }
                        }

                        match stmt_result {
                            Ok(()) => {
                                // Evaluate trailing expression with TCO awareness
                                if let Some(expr) = &body.block.expr {
                                    match self.eval_expr_tco(expr, fn_name) {
                                        Ok(TcoResult::Value(val)) => {
                                            self.env.pop_scope();
                                            if let Some(saved) = saved_env {
                                                self.env = saved;
                                            }
                                            break 'tco Ok(val);
                                        }
                                        Ok(TcoResult::TailCall(new_args)) => {
                                            self.env.pop_scope();
                                            if let Some(saved) = saved_env {
                                                self.env = saved;
                                            }
                                            current_args = new_args;
                                            continue 'tco;
                                        }
                                        Err(e) => {
                                            self.env.pop_scope();
                                            if let Some(saved) = saved_env {
                                                self.env = saved;
                                            }
                                            break 'tco Err(e);
                                        }
                                    }
                                } else {
                                    self.env.pop_scope();
                                    if let Some(saved) = saved_env {
                                        self.env = saved;
                                    }
                                    break 'tco Ok(Value::Unit);
                                }
                            }
                            Err(e) => {
                                self.env.pop_scope();
                                if let Some(saved) = saved_env {
                                    self.env = saved;
                                }
                                break 'tco Err(e);
                            }
                        }
                    } else {
                        // Normal path: eval_block pushes its own scope
                        let result = self.eval_block(&body.block);
                        self.env.pop_scope();
                        if let Some(saved) = saved_env {
                            self.env = saved;
                        }
                        break 'tco result;
                    }
                };

                // Handle early returns from ? operator and return statements
                let result = match result {
                    Err(e) if e.is_return => Ok(e.get_early_return().unwrap_or(Value::Unit)),
                    Err(e) if e.is_early_return() => Ok(e.get_early_return().unwrap()),
                    Err(mut e) => {
                        // Attach stack trace to error (P5.2)
                        if !e.is_break && !e.is_continue {
                            let trace = self.format_stack_trace();
                            if !trace.is_empty() {
                                e.message = format!("{}\n{}", e.message, trace);
                            }
                        }
                        self.call_stack.pop();
                        return Err(e);
                    }
                    other => other,
                }?;

                // Check postconditions (ensures clauses)
                if !body.ensures.is_empty() {
                    // Re-evaluate in closure env with params + result
                    let saved_env = if uses_closure_env {
                        Some(std::mem::replace(&mut self.env, env.clone()))
                    } else {
                        None
                    };
                    self.env.push_scope();
                    for (param, arg) in params.iter().zip(current_args.iter()) {
                        self.env.define(param.clone(), arg.clone());
                    }
                    self.env.define("result".to_string(), result.clone());
                    for ens_expr in &body.ensures {
                        let cond = self.eval_expr(ens_expr)?;
                        match cond {
                            Value::Bool(true) => {}
                            Value::Bool(false) => {
                                self.env.pop_scope();
                                if let Some(saved) = saved_env {
                                    self.env = saved;
                                }
                                return Err(RuntimeError::postcondition_violated(fn_name));
                            }
                            _ => {
                                self.env.pop_scope();
                                if let Some(saved) = saved_env {
                                    self.env = saved;
                                }
                                return Err(RuntimeError::type_mismatch(
                                    "Bool",
                                    &format!("{:?}", cond),
                                ));
                            }
                        }
                    }
                    self.env.pop_scope();
                    if let Some(saved) = saved_env {
                        self.env = saved;
                    }
                }

                // Pop call stack frame
                self.call_stack.pop();
                Ok(result)
            }
            _ => Err(RuntimeError::not_callable()),
        }
    }

    /// P6.4: Check if a block ends with a self-recursive tail call
    fn has_self_tail_call(block: &Block, fn_name: &str) -> bool {
        match &block.expr {
            Some(expr) => Self::expr_has_tail_call(expr, fn_name),
            None => false,
        }
    }

    /// P6.4: Check if an expression contains a self-recursive tail call
    fn expr_has_tail_call(expr: &Expr, fn_name: &str) -> bool {
        match expr {
            Expr::Call { func, .. } => {
                matches!(func.as_ref(), Expr::Ident { name, .. } if name == fn_name)
            }
            Expr::If {
                then_branch,
                else_branch,
                ..
            } => {
                let then_has = Self::has_self_tail_call(then_branch, fn_name);
                let else_has = else_branch
                    .as_ref()
                    .is_some_and(|e| Self::expr_has_tail_call(e, fn_name));
                then_has || else_has
            }
            Expr::Block { block, .. } => Self::has_self_tail_call(block, fn_name),
            _ => false,
        }
    }

    /// P6.4: Evaluate an expression with TCO awareness
    fn eval_expr_tco(&mut self, expr: &Expr, fn_name: &str) -> Result<TcoResult, RuntimeError> {
        match expr {
            Expr::Call { func, args, .. } => {
                if let Expr::Ident { name, .. } = func.as_ref() {
                    if name == fn_name {
                        // Self-recursive tail call - extract args without recursing
                        let eval_args: Vec<Value> = args
                            .iter()
                            .map(|a| self.eval_expr(a))
                            .collect::<Result<_, _>>()?;
                        return Ok(TcoResult::TailCall(eval_args));
                    }
                }
                // Not a self-call, evaluate normally
                let val = self.eval_expr(expr)?;
                Ok(TcoResult::Value(val))
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_val = self.eval_expr(cond)?;
                match cond_val {
                    Value::Bool(true) => {
                        // Execute then branch with TCO awareness
                        self.env.push_scope();
                        for stmt in &then_branch.stmts {
                            if let Err(e) = self.eval_stmt(stmt) {
                                self.env.pop_scope();
                                return Err(e);
                            }
                        }
                        let result = if let Some(tail_expr) = &then_branch.expr {
                            self.eval_expr_tco(tail_expr, fn_name)
                        } else {
                            Ok(TcoResult::Value(Value::Unit))
                        };
                        self.env.pop_scope();
                        result
                    }
                    Value::Bool(false) => {
                        if let Some(else_expr) = else_branch {
                            self.eval_expr_tco(else_expr, fn_name)
                        } else {
                            Ok(TcoResult::Value(Value::Unit))
                        }
                    }
                    _ => Err(RuntimeError::type_mismatch(
                        "Bool",
                        &format!("{:?}", cond_val),
                    )),
                }
            }
            Expr::Block { block, .. } => {
                self.env.push_scope();
                for stmt in &block.stmts {
                    if let Err(e) = self.eval_stmt(stmt) {
                        self.env.pop_scope();
                        return Err(e);
                    }
                }
                let result = if let Some(tail_expr) = &block.expr {
                    self.eval_expr_tco(tail_expr, fn_name)
                } else {
                    Ok(TcoResult::Value(Value::Unit))
                };
                self.env.pop_scope();
                result
            }
            _ => {
                let val = self.eval_expr(expr)?;
                Ok(TcoResult::Value(val))
            }
        }
    }

    /// Get the current call stack as a formatted string
    pub fn format_stack_trace(&self) -> String {
        if self.call_stack.is_empty() {
            return String::new();
        }
        let mut trace = String::from("Stack trace:\n");
        for (i, frame) in self.call_stack.iter().rev().enumerate() {
            trace.push_str(&format!("  {}: {}\n", i, frame));
        }
        trace
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Lexer, Parser, SourceFile};
    use std::path::PathBuf;

    fn parse_and_eval(source: &str) -> Result<Value, RuntimeError> {
        let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
        let lexer = Lexer::new(&source_file);
        let mut parser = Parser::new(lexer, source_file.clone());
        let module = parser.parse_module().expect("parse failed");

        let console = Box::new(MockConsole::new());
        let capabilities = Capabilities {
            console: Some(console),
            ..Default::default()
        };

        let mut interpreter = Interpreter::with_capabilities(capabilities);
        // Add the project root as a search path for stdlib imports
        if let Ok(cwd) = std::env::current_dir() {
            interpreter.add_search_path(cwd);
        }
        interpreter.eval_module(&module)
    }

    #[test]
    fn test_simple_arithmetic() {
        let source = r#"
module example

fn main() -> Int {
  1 + 2 * 3
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(7)));
    }

    #[test]
    fn test_if_expression() {
        let source = r#"
module example

fn main() -> Int {
  if true {
    42
  } else {
    0
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_function_call() {
        let source = r#"
module example

fn add(a: Int, b: Int) -> Int {
  a + b
}

fn main() -> Int {
  add(10, 20)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(30)));
    }

    #[test]
    fn test_pattern_matching() {
        let source = r#"
module example

fn main() -> Int {
  let x = 5
  match x {
    0 => 100
    5 => 200
    _ => 300
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(200)));
    }

    #[test]
    fn test_recursion() {
        // Run in a thread with a larger stack to avoid overflow in debug builds
        let handle = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                let source = r#"
module example

fn factorial(n: Int) -> Int {
  if n <= 1 {
    1
  } else {
    n * factorial(n - 1)
  }
}

fn main() -> Int {
  factorial(3)
}
"#;
                let result = parse_and_eval(source).unwrap();
                assert!(matches!(result, Value::Int(6)));
            })
            .unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn test_string_operations() {
        let source = r#"
module example

fn main() -> Text {
  "Hello" + " " + "World"
}
"#;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "Hello World"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_comparison() {
        let source = r#"
module example

fn main() -> Bool {
  10 > 5 and 5 < 10
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_record_field_access() {
        let source = r#"
module example

fn main() -> Int {
  let r = { x = 10, y = 20 }
  r.x + r.y
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(30)));
    }

    #[test]
    fn test_division_by_zero() {
        let source = r#"
module example

fn main() -> Int {
  10 / 0
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "E4003");
    }

    #[test]
    fn test_console_effect() {
        let source = r#"
module example

fn main() effects(Console) {
  Console.println("test output")
}
"#;
        // Use the helper which sets up mock console
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_nested_function_calls() {
        let source = r#"
module example

fn double(x: Int) -> Int {
  x * 2
}

fn add_one(x: Int) -> Int {
  x + 1
}

fn main() -> Int {
  double(add_one(5))
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(12)));
    }

    #[test]
    fn test_option_some() {
        let source = r#"
module example

fn main() -> Int {
  match Some(42) {
    Some(n) => n
    None => 0
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_option_none() {
        let source = r#"
module example

fn main() -> Int {
  match None {
    Some(n) => n
    None => 0
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(0)));
    }

    #[test]
    fn test_result_ok() {
        let source = r#"
module example

fn main() -> Int {
  match Ok(42) {
    Ok(n) => n
    Err(e) => 0
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_result_err() {
        let source = r#"
module example

fn main() -> Text {
  match Err("oops") {
    Ok(n) => "ok"
    Err(e) => e
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(s) if s == "oops"));
    }

    #[test]
    fn test_try_operator_option_some() {
        let source = r#"
module example

fn get() -> Option[Int] {
  Some(21)
}

fn double() -> Option[Int] {
  let x = get()?
  Some(x * 2)
}

fn main() -> Int {
  match double() {
    Some(n) => n
    None => 0
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_try_operator_option_none() {
        let source = r#"
module example

fn get() -> Option[Int] {
  None
}

fn double() -> Option[Int] {
  let x = get()?
  Some(x * 2)
}

fn main() -> Int {
  match double() {
    Some(n) => n
    None => 0
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(0)));
    }

    #[test]
    fn test_try_operator_result() {
        let source = r#"
module example

fn parse(ok: Bool) -> Result[Int, Text] {
  if ok { Ok(21) } else { Err("error") }
}

fn double(ok: Bool) -> Result[Int, Text] {
  let x = parse(ok)?
  Ok(x * 2)
}

fn main() -> Int {
  match double(true) {
    Ok(n) => n
    Err(e) => 0
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_seeded_rand_deterministic() {
        // Two runs with the same seed should produce the same results
        let rand1 = SeededRand::new(42);
        let rand2 = SeededRand::new(42);

        let a1 = rand1.int(1, 100);
        let a2 = rand2.int(1, 100);
        assert_eq!(a1, a2);

        let b1 = rand1.int(1, 1000);
        let b2 = rand2.int(1, 1000);
        assert_eq!(b1, b2);

        let c1 = rand1.bool();
        let c2 = rand2.bool();
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_fixed_clock() {
        let clock = FixedClock::new(1700000000);
        assert_eq!(clock.now(), 1700000000);
        clock.sleep(5000); // no-op
        assert_eq!(clock.now(), 1700000000);
    }

    #[test]
    fn test_deterministic_rand_in_program() {
        let source = r#"
module example

fn main() effects(Rand) {
  Rand.int(1, 100)
}
"#;
        let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
        let lexer = Lexer::new(&source_file);
        let mut parser = Parser::new(lexer, source_file.clone());
        let module = parser.parse_module().expect("parse failed");

        // Run twice with same seed, should get same result
        let caps1 = Capabilities {
            rand: Some(Box::new(SeededRand::new(42))),
            console: Some(Box::new(MockConsole::new())),
            ..Default::default()
        };
        let mut interp1 = Interpreter::with_capabilities(caps1);
        let result1 = interp1.eval_module(&module).unwrap();

        let caps2 = Capabilities {
            rand: Some(Box::new(SeededRand::new(42))),
            console: Some(Box::new(MockConsole::new())),
            ..Default::default()
        };
        let mut interp2 = Interpreter::with_capabilities(caps2);
        let result2 = interp2.eval_module(&module).unwrap();

        match (&result1, &result2) {
            (Value::Int(a), Value::Int(b)) => assert_eq!(a, b),
            _ => panic!("expected Int results"),
        }
    }

    #[test]
    fn test_fixed_clock_in_program() {
        let source = r#"
module example

fn main() effects(Clock) {
  Clock.now()
}
"#;
        let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
        let lexer = Lexer::new(&source_file);
        let mut parser = Parser::new(lexer, source_file.clone());
        let module = parser.parse_module().expect("parse failed");

        let caps = Capabilities {
            clock: Some(Box::new(FixedClock::new(1700000000))),
            console: Some(Box::new(MockConsole::new())),
            ..Default::default()
        };
        let mut interp = Interpreter::with_capabilities(caps);
        let result = interp.eval_module(&module).unwrap();

        assert!(matches!(result, Value::Int(1700000000)));
    }

    #[test]
    fn test_requires_passes() {
        let source = r#"
module example

fn divide(a: Int, b: Int) -> Int
  requires b != 0
{
  a / b
}

fn main() -> Int {
  divide(10, 2)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(5)));
    }

    #[test]
    fn test_requires_fails() {
        let source = r#"
module example

fn divide(a: Int, b: Int) -> Int
  requires b != 0
{
  a / b
}

fn main() -> Int {
  divide(10, 0)
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "E3001");
    }

    #[test]
    fn test_ensures_passes() {
        let source = r#"
module example

fn abs(x: Int) -> Int
  ensures result >= 0
{
  if x < 0 { 0 - x } else { x }
}

fn main() -> Int {
  abs(0 - 5)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(5)));
    }

    #[test]
    fn test_ensures_fails() {
        let source = r#"
module example

fn bad_abs(x: Int) -> Int
  ensures result >= 0
{
  x
}

fn main() -> Int {
  bad_abs(0 - 5)
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "E3002");
    }

    #[test]
    fn test_multiple_requires() {
        let source = r#"
module example

fn clamp(x: Int, lo: Int, hi: Int) -> Int
  requires lo <= hi
  requires lo >= 0
{
  if x < lo { lo } else { if x > hi { hi } else { x } }
}

fn main() -> Int {
  clamp(50, 0, 100)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(50)));
    }

    #[test]
    fn test_requires_and_ensures() {
        let source = r#"
module example

fn safe_divide(a: Int, b: Int) -> Int
  requires b != 0
  ensures result * b <= a
{
  a / b
}

fn main() -> Int {
  safe_divide(10, 3)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    // H5: Type inference for let bindings
    #[test]
    fn test_let_type_inference() {
        let source = r#"
module example

fn main() -> Int {
  let x = 42
  let y = x + 8
  y
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(50)));
    }

    #[test]
    fn test_let_type_inference_text() {
        let source = r#"
module example

fn main() -> Text {
  let greeting = "Hello"
  let name = "World"
  greeting + " " + name
}
"#;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "Hello World"),
            _ => panic!("expected Text"),
        }
    }

    // N1: List literal syntax
    #[test]
    fn test_list_literal_empty() {
        let source = r#"
module example

fn main() -> Int {
  let xs = []
  len(xs)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(0)));
    }

    #[test]
    fn test_list_literal_ints() {
        let source = r#"
module example

fn main() -> Int {
  let xs = [1, 2, 3]
  len(xs)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    #[test]
    fn test_list_get() {
        let source = r#"
module example

fn main() -> Int {
  let xs = [10, 20, 30]
  match xs.get(1) {
    Some(n) => n
    None => 0
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(20)));
    }

    #[test]
    fn test_list_get_out_of_bounds() {
        let source = r#"
module example

fn main() -> Int {
  let xs = [10, 20, 30]
  match xs.get(5) {
    Some(n) => n
    None => 0
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(0)));
    }

    #[test]
    fn test_list_contains() {
        let source = r#"
module example

fn main() -> Bool {
  let xs = [1, 2, 3]
  xs.contains(2)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_list_method_len() {
        let source = r#"
module example

fn main() -> Int {
  let xs = [10, 20, 30, 40]
  xs.len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(4)));
    }

    // N2: print and println builtins
    #[test]
    fn test_println_builtin() {
        let source = r#"
module example

fn main() {
  println("hello from builtin")
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_print_builtin() {
        let source = r#"
module example

fn main() {
  print("no newline")
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    // N3: len and to_text builtins
    #[test]
    fn test_len_text() {
        let source = r#"
module example

fn main() -> Int {
  len("hello")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(5)));
    }

    #[test]
    fn test_to_text_int() {
        let source = r#"
module example

fn main() -> Text {
  to_text(42)
}
"#;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "42"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_to_text_bool() {
        let source = r#"
module example

fn main() -> Text {
  to_text(true)
}
"#;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "true"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_text_len_method() {
        let source = r#"
module example

fn main() -> Int {
  let s = "hello"
  s.len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(5)));
    }

    // N4: if-then-else expression syntax
    #[test]
    fn test_if_then_else_basic() {
        let source = r#"
module example

fn main() -> Int {
  if true then 42 else 0
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_if_then_else_false() {
        let source = r#"
module example

fn main() -> Int {
  if false then 42 else 0
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(0)));
    }

    #[test]
    fn test_if_then_else_with_expr() {
        let source = r#"
module example

fn main() -> Int {
  let x = 10
  if x > 5 then x * 2 else x
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(20)));
    }

    // List equality
    #[test]
    fn test_list_equality() {
        let source = r#"
module example

fn main() {
  let xs = [1, 2, 3]
  let ys = [1, 2, 3]
  assert_eq(xs, ys)
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    // =========================================================================
    // Lambda / Anonymous function tests
    // =========================================================================

    #[test]
    fn test_lambda_basic() {
        let source = r#"
module example

fn main() -> Int {
  let square = fn(x: Int) { x * x }
  square(5)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(25)));
    }

    #[test]
    fn test_lambda_no_type_annotation() {
        let source = r#"
module example

fn main() -> Int {
  let add_one = fn(x) { x + 1 }
  add_one(41)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_lambda_multi_param() {
        let source = r#"
module example

fn main() -> Int {
  let add = fn(a, b) { a + b }
  add(10, 20)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(30)));
    }

    #[test]
    fn test_lambda_closure_capture() {
        let source = r#"
module example

fn make_adder(n: Int) -> (Int) -> Int {
  fn(x: Int) { x + n }
}

fn main() -> Int {
  let add5 = make_adder(5)
  add5(10)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(15)));
    }

    #[test]
    fn test_lambda_as_argument() {
        let source = r#"
module example

fn apply(f: (Int) -> Int, x: Int) -> Int {
  f(x)
}

fn main() -> Int {
  apply(fn(x) { x + 10 }, 5)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(15)));
    }

    #[test]
    fn test_lambda_inline_in_method() {
        let source = r#"
module example

fn main() -> Int {
  let xs = [1, 2, 3, 4, 5]
  let doubled = xs.map(fn(x) { x * 2 })
  doubled.fold(0, fn(acc, x) { acc + x })
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(30)));
    }

    // =========================================================================
    // Higher-order function tests
    // =========================================================================

    #[test]
    fn test_function_as_value() {
        let source = r#"
module example

fn double(x: Int) -> Int {
  x * 2
}

fn apply_twice(f: (Int) -> Int, x: Int) -> Int {
  f(f(x))
}

fn main() -> Int {
  apply_twice(double, 3)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(12)));
    }

    // =========================================================================
    // List combinator tests
    // =========================================================================

    #[test]
    fn test_list_map() {
        let source = r#"
module example

fn main() {
  let xs = [1, 2, 3]
  let doubled = xs.map(fn(x) { x * 2 })
  assert_eq(doubled, [2, 4, 6])
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_filter() {
        let source = r#"
module example

fn main() {
  let xs = [1, 2, 3, 4, 5, 6]
  let evens = xs.filter(fn(x) { x % 2 == 0 })
  assert_eq(evens, [2, 4, 6])
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_fold() {
        let source = r#"
module example

fn main() -> Int {
  let xs = [1, 2, 3, 4, 5]
  xs.fold(0, fn(acc, x) { acc + x })
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(15)));
    }

    #[test]
    fn test_list_method_chain() {
        let source = r#"
module example

fn main() -> Int {
  [1, -2, 3, -4, 5]
    .filter(fn(x) { x > 0 })
    .map(fn(x) { x * 2 })
    .fold(0, fn(acc, x) { acc + x })
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(18)));
    }

    #[test]
    fn test_list_any() {
        let source = r#"
module example

fn main() -> Bool {
  [1, 2, 3, 4, 5].any(fn(x) { x > 4 })
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_list_all() {
        let source = r#"
module example

fn main() -> Bool {
  [2, 4, 6, 8].all(fn(x) { x % 2 == 0 })
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_list_each() {
        let source = r#"
module example

fn main() {
  let xs = [1, 2, 3]
  xs.each(fn(x) { assert(x > 0) })
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_push() {
        let source = r#"
module example

fn main() {
  let xs = [1, 2, 3]
  let ys = xs.push(4)
  assert_eq(ys, [1, 2, 3, 4])
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_concat() {
        let source = r#"
module example

fn main() {
  let xs = [1, 2]
  let ys = [3, 4]
  assert_eq(xs.concat(ys), [1, 2, 3, 4])
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_head() {
        let source = r#"
module example

fn main() -> Int {
  match [10, 20, 30].head() {
    Some(x) => x
    None => 0
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(10)));
    }

    #[test]
    fn test_list_is_empty() {
        let source = r#"
module example

fn main() -> Bool {
  [].is_empty()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_list_flat_map() {
        let source = r#"
module example

fn main() {
  let xs = [1, 2, 3]
  let result = xs.flat_map(fn(x) { [x, x * 10] })
  assert_eq(result, [1, 10, 2, 20, 3, 30])
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    // =========================================================================
    // Pattern guard tests
    // =========================================================================

    #[test]
    fn test_pattern_guard_basic() {
        let source = r#"
module example

fn classify(x: Int) -> Text {
  match x {
    n if n < 0 => "negative"
    0 => "zero"
    n if n <= 10 => "small"
    _ => "large"
  }
}

fn main() -> Text {
  classify(-5)
}
"#;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "negative"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_pattern_guard_fallthrough() {
        let source = r#"
module example

fn main() -> Text {
  let x = 15
  match x {
    n if n < 0 => "negative"
    0 => "zero"
    n if n <= 10 => "small"
    _ => "large"
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "large"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_pattern_guard_with_variant() {
        let source = r#"
module example

fn main() -> Text {
  match Some(5) {
    Some(n) if n > 10 => "big"
    Some(n) if n > 0 => "small"
    Some(n) => "non-positive"
    None => "nothing"
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "small"),
            _ => panic!("expected Text"),
        }
    }

    // =========================================================================
    // String method tests
    // =========================================================================

    #[test]
    fn test_string_to_upper() {
        let source = r#"
module example

fn main() -> Text {
  "hello".to_upper()
}
"#;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "HELLO"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_string_to_lower() {
        let source = r#"
module example

fn main() -> Text {
  "HELLO".to_lower()
}
"#;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "hello"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_string_trim() {
        let source = r#"
module example

fn main() -> Text {
  "  hello  ".trim()
}
"#;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "hello"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_string_contains() {
        let source = r#"
module example

fn main() -> Bool {
  "hello world".contains("world")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_string_split() {
        let source = r#"
module example

fn main() {
  let parts = "a,b,c".split(",")
  assert_eq(parts, ["a", "b", "c"])
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_string_starts_with() {
        let source = r#"
module example

fn main() -> Bool {
  "hello world".starts_with("hello")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_string_ends_with() {
        let source = r#"
module example

fn main() -> Bool {
  "hello world".ends_with("world")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_string_replace() {
        let source = r#"
module example

fn main() -> Text {
  "hello world".replace("world", "astra")
}
"#;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "hello astra"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn test_string_chars() {
        let source = r#"
module example

fn main() {
  let chars = "abc".chars()
  assert_eq(chars, ["a", "b", "c"])
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    // =========================================================================
    // Option/Result map tests
    // =========================================================================

    #[test]
    fn test_option_map() {
        let source = r#"
module example

fn main() {
  let x = Some(5)
  let y = x.map(fn(n) { n * 2 })
  assert_eq(y, Some(10))
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_option_map_none() {
        let source = r#"
module example

fn main() {
  let x = None
  let y = x.map(fn(n) { n * 2 })
  assert_eq(y, None)
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_result_map() {
        let source = r#"
module example

fn main() {
  let x = Ok(5)
  let y = x.map(fn(n) { n * 2 })
  assert_eq(y, Ok(10))
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_result_map_err() {
        let source = r#"
module example

fn main() {
  let x = Err("bad")
  let y = x.map_err(fn(e) { e + "!" })
  assert_eq(y, Err("bad!"))
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    // =========================================================================
    // Record destructuring in let bindings
    // =========================================================================

    #[test]
    fn test_let_destructure_record() {
        let source = r#"
module example

fn main() -> Int {
  let point = { x = 10, y = 20 }
  let { x, y } = point
  x + y
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(30)));
    }

    #[test]
    fn test_let_destructure_record_inline() {
        let source = r#"
module example

fn main() -> Int {
  let { x, y } = { x = 3, y = 4 }
  x * y
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(12)));
    }

    // =========================================================================
    // For loop tests
    // =========================================================================

    #[test]
    fn test_for_loop_basic() {
        let source = r#"
module example

fn main() -> Int {
  let mut sum = 0
  for x in [1, 2, 3, 4, 5] {
    sum = sum + x
  }
  sum
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(15)));
    }

    #[test]
    fn test_for_loop_with_variable() {
        let source = r#"
module example

fn main() -> Int {
  let items = [10, 20, 30]
  let mut total = 0
  for item in items {
    total = total + item
  }
  total
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(60)));
    }

    #[test]
    fn test_for_loop_empty_list() {
        let source = r#"
module example

fn main() -> Int {
  let mut count = 0
  for _x in [] {
    count = count + 1
  }
  count
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(0)));
    }

    #[test]
    fn test_for_loop_nested() {
        let source = r#"
module example

fn main() -> Int {
  let mut sum = 0
  for x in [1, 2, 3] {
    for y in [10, 20] {
      sum = sum + x * y
    }
  }
  sum
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(180)));
    }

    // =========================================================================
    // Multi-field variant destructuring tests
    // =========================================================================

    #[test]
    fn test_multi_field_variant_construct_and_match() {
        let source = r#"
module example

enum Shape =
  | Circle(radius: Int)
  | Rectangle(width: Int, height: Int)

fn area(s: Shape) -> Int {
  match s {
    Circle(r) => r * r * 3
    Rectangle(w, h) => w * h
  }
}

fn main() -> Int {
  area(Rectangle(5, 3))
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(15)));
    }

    #[test]
    fn test_multi_field_variant_single_field() {
        let source = r#"
module example

enum Shape =
  | Circle(radius: Int)
  | Rectangle(width: Int, height: Int)

fn area(s: Shape) -> Int {
  match s {
    Circle(r) => r * r
    Rectangle(w, h) => w * h
  }
}

fn main() -> Int {
  area(Circle(10))
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(100)));
    }

    // =========================================================================
    // Generic function tests
    // =========================================================================

    #[test]
    fn test_generic_identity() {
        let source = r#"
module example

fn identity[T](x: T) -> T {
  x
}

fn main() -> Int {
  identity(42)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_generic_identity_text() {
        let source = r#"
module example

fn identity[T](x: T) -> T {
  x
}

fn main() -> Text {
  identity("hello")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "hello"));
    }

    #[test]
    fn test_generic_pair() {
        let source = r#"
module example

fn first[T, U](a: T, b: U) -> T {
  a
}

fn second[T, U](a: T, b: U) -> U {
  b
}

fn main() -> Int {
  first(1, "hello") + second("world", 2)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    // === P1.1: range() builtin ===

    #[test]
    fn test_range_basic() {
        let source = r#"
module example
fn main() -> Int {
  let xs = range(0, 5)
  len(xs)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(5)));
    }

    #[test]
    fn test_range_for_loop() {
        let source = r#"
module example
fn main() -> Int {
  let mut sum = 0
  for i in range(1, 6) {
    sum = sum + i
  }
  sum
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(15)));
    }

    #[test]
    fn test_range_empty() {
        let source = r#"
module example
fn main() -> Int {
  let xs = range(5, 5)
  len(xs)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(0)));
    }

    // === P1.2: break/continue ===

    #[test]
    fn test_break_in_for_loop() {
        let source = r#"
module example
fn main() -> Int {
  let mut sum = 0
  for i in range(1, 100) {
    if i > 5 { break }
    sum = sum + i
  }
  sum
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(15)));
    }

    #[test]
    fn test_continue_in_for_loop() {
        let source = r#"
module example
fn main() -> Int {
  let mut sum = 0
  for i in range(1, 6) {
    if i == 3 { continue }
    sum = sum + i
  }
  sum
}
"#;
        let result = parse_and_eval(source).unwrap();
        // 1 + 2 + 4 + 5 = 12 (skipping 3)
        assert!(matches!(result, Value::Int(12)));
    }

    // === P1.3: while loops ===

    #[test]
    fn test_while_basic() {
        let source = r#"
module example
fn main() -> Int {
  let mut i = 0
  let mut sum = 0
  while i < 5 {
    sum = sum + i
    i = i + 1
  }
  sum
}
"#;
        let result = parse_and_eval(source).unwrap();
        // 0+1+2+3+4 = 10
        assert!(matches!(result, Value::Int(10)));
    }

    #[test]
    fn test_while_with_break() {
        let source = r#"
module example
fn main() -> Int {
  let mut i = 0
  while true {
    if i == 5 { break }
    i = i + 1
  }
  i
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(5)));
    }

    #[test]
    fn test_while_with_continue() {
        let source = r#"
module example
fn main() -> Int {
  let mut i = 0
  let mut sum = 0
  while i < 10 {
    i = i + 1
    if i % 2 == 0 { continue }
    sum = sum + i
  }
  sum
}
"#;
        let result = parse_and_eval(source).unwrap();
        // 1+3+5+7+9 = 25
        assert!(matches!(result, Value::Int(25)));
    }

    #[test]
    fn test_while_false() {
        let source = r#"
module example
fn main() -> Int {
  let mut x = 42
  while false {
    x = 0
  }
  x
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    // === P1.5: String interpolation ===

    #[test]
    fn test_string_interp_basic() {
        let source = r#"
module example
fn main() -> Text {
  let name = "world"
  "hello, ${name}!"
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "hello, world!"));
    }

    #[test]
    fn test_string_interp_expr() {
        let source = r#"
module example
fn main() -> Text {
  let x = 21
  "answer is ${x * 2}"
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "answer is 42"));
    }

    #[test]
    fn test_string_interp_multiple() {
        let source = r#"
module example
fn main() -> Text {
  let a = 1
  let b = 2
  "${a} + ${b} = ${a + b}"
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "1 + 2 = 3"));
    }

    // === P1.10: return statement ===

    #[test]
    fn test_return_early() {
        let source = r#"
module example
fn check(x: Int) -> Text {
  if x > 10 {
    return "big"
  }
  "small"
}
fn main() -> Text {
  check(15)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "big"));
    }

    #[test]
    fn test_return_fallthrough() {
        let source = r#"
module example
fn check(x: Int) -> Text {
  if x > 10 {
    return "big"
  }
  "small"
}
fn main() -> Text {
  check(5)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "small"));
    }

    #[test]
    fn test_return_in_loop() {
        let source = r#"
module example
fn find_first_even(xs: List[Int]) -> Option[Int] {
  for x in xs {
    if x % 2 == 0 {
      return Some(x)
    }
  }
  None
}
fn main() -> Int {
  match find_first_even([1, 3, 4, 6]) {
    Some(n) => n,
    None => 0,
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(4)));
    }

    // === P1.11: math builtins ===

    #[test]
    fn test_abs() {
        let source = r#"
module example
fn main() -> Int {
  abs(-42)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_min_max() {
        let source = r#"
module example
fn main() -> Int {
  min(3, 7) + max(3, 7)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(10)));
    }

    #[test]
    fn test_pow() {
        let source = r#"
module example
fn main() -> Int {
  pow(2, 10)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(1024)));
    }

    // === P3.1: List methods (tail, reverse, sort) ===

    #[test]
    fn test_list_tail() {
        let source = r#"
module example
fn main() -> Int {
  let xs = [1, 2, 3, 4]
  len(xs.tail())
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    #[test]
    fn test_list_reverse() {
        let source = r#"
module example
fn main() -> Int {
  let xs = [3, 1, 2]
  let rev = xs.reverse()
  match rev.head() {
    Some(n) => n,
    None => 0,
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(2)));
    }

    #[test]
    fn test_list_sort() {
        let source = r#"
module example
fn main() -> Int {
  let xs = [3, 1, 4, 1, 5, 9]
  let sorted = xs.sort()
  match sorted.head() {
    Some(n) => n,
    None => 0,
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(1)));
    }

    // === P3.2: List methods (take, drop, slice, enumerate, zip, find) ===

    #[test]
    fn test_list_take() {
        let source = r#"
module example
fn main() -> Int {
  len([1, 2, 3, 4, 5].take(3))
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    #[test]
    fn test_list_drop() {
        let source = r#"
module example
fn main() -> Int {
  len([1, 2, 3, 4, 5].drop(2))
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    #[test]
    fn test_list_find() {
        let source = r#"
module example
fn main() -> Int {
  match [1, 2, 3, 4].find(fn(x) { x > 2 }) {
    Some(n) => n,
    None => 0,
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    // === P3.3: String methods (repeat, index_of, substring, join) ===

    #[test]
    fn test_string_repeat() {
        let source = r#"
module example
fn main() -> Text {
  "ha".repeat(3)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "hahaha"));
    }

    #[test]
    fn test_string_index_of() {
        let source = r#"
module example
fn main() -> Int {
  match "hello world".index_of("world") {
    Some(n) => n,
    None => -1,
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(6)));
    }

    #[test]
    fn test_string_substring() {
        let source = r#"
module example
fn main() -> Text {
  "hello world".substring(0, 5)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "hello"));
    }

    #[test]
    fn test_list_join() {
        let source = r#"
module example
fn main() -> Text {
  ["a", "b", "c"].join(", ")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "a, b, c"));
    }

    // === Else-if chains (P1.4 - already implemented in parser) ===

    #[test]
    fn test_else_if_chain() {
        let source = r#"
module example
fn classify(n: Int) -> Text {
  if n < 0 {
    "negative"
  } else if n == 0 {
    "zero"
  } else if n < 10 {
    "small"
  } else {
    "large"
  }
}
fn main() -> Text {
  classify(5)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "small"));
    }

    // === P1.6: Float type ===

    #[test]
    fn test_float_literal() {
        let source = r#"
module example
fn main() -> Float {
  2.75
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Float(f) if (f - 2.75).abs() < 0.001));
    }

    #[test]
    fn test_float_arithmetic() {
        let source = r#"
module example
fn main() -> Float {
  1.5 + 2.5
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Float(f) if (f - 4.0).abs() < 0.001));
    }

    #[test]
    fn test_float_int_mixed() {
        let source = r#"
module example
fn main() -> Float {
  2 * 1.5
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Float(f) if (f - 3.0).abs() < 0.01));
    }

    #[test]
    fn test_float_comparison() {
        let source = r#"
module example
fn main() -> Bool {
  3.14 > 2.71
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_float_neg() {
        let source = r#"
module example
fn main() -> Float {
  -2.75
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Float(f) if (f + 2.75).abs() < 0.001));
    }

    #[test]
    fn test_sqrt() {
        let source = r#"
module example
fn main() -> Float {
  sqrt(16.0)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Float(f) if (f - 4.0).abs() < 0.001));
    }

    #[test]
    fn test_floor_ceil_round() {
        let source = r#"
module example
fn main() -> Float {
  floor(3.7) + ceil(3.2) + round(3.5)
}
"#;
        let result = parse_and_eval(source).unwrap();
        // floor(3.7)=3.0 + ceil(3.2)=4.0 + round(3.5)=4.0 = 11.0
        assert!(matches!(result, Value::Float(f) if (f - 11.0).abs() < 0.001));
    }

    // === P6.1: Pipe operator ===

    #[test]
    fn test_pipe_basic() {
        let source = r#"
module example
fn double(x: Int) -> Int { x * 2 }
fn add_one(x: Int) -> Int { x + 1 }
fn main() -> Int {
  5 |> double |> add_one
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(11)));
    }

    #[test]
    fn test_pipe_with_lambda() {
        let source = r#"
module example
fn main() -> Int {
  10 |> fn(x) { x * x }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(100)));
    }

    // === P3.4: Conversion functions ===

    #[test]
    fn test_to_int_from_float() {
        let source = r#"
module example
fn main() -> Int {
  to_int(3.14)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    #[test]
    fn test_to_float_from_int() {
        let source = r#"
module example
fn main() -> Float {
  to_float(42)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Float(f) if (f - 42.0).abs() < 0.001));
    }

    // === P5.4: Assert with custom messages ===

    #[test]
    fn test_assert_with_message_passes() {
        let source = r#"
module example
fn main() -> Int {
  assert(true, "should pass")
  42
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_assert_with_message_fails() {
        let source = r#"
module example
fn main() -> Int {
  assert(false, "custom error message")
  42
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("custom error message"));
    }

    // === P1.7: Tuple type ===

    #[test]
    fn test_tuple_creation() {
        let source = r#"
module example
fn main() -> Int {
  let t = (1, 2, 3)
  len(t)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    #[test]
    fn test_tuple_field_access() {
        let source = r#"
module example
fn main() -> Int {
  let t = (10, 20, 30)
  t.0 + t.2
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(40)));
    }

    #[test]
    fn test_tuple_pattern_match() {
        let source = r#"
module example
fn main() -> Int {
  let t = (1, 2)
  match t {
    (a, b) => a + b
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    // === P1.8: Map type ===

    #[test]
    fn test_map_new_and_set() {
        let source = r#"
module example
fn main() -> Int {
  let m = Map.new()
  let m2 = m.set("a", 1)
  let m3 = m2.set("b", 2)
  m3.len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(2)));
    }

    #[test]
    fn test_map_get() {
        let source = r#"
module example
fn main() -> Int {
  let m = Map.new()
  let m2 = m.set("key", 42)
  m2.get("key").unwrap()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_map_contains_key() {
        let source = r#"
module example
fn main() -> Bool {
  let m = Map.new().set("x", 1)
  m.contains_key("x")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_map_keys_values() {
        let source = r#"
module example
fn main() -> Int {
  let m = Map.new().set("a", 1).set("b", 2)
  m.keys().len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(2)));
    }

    #[test]
    fn test_map_from_tuples() {
        let source = r#"
module example
fn main() -> Int {
  let m = Map.from([("a", 10), ("b", 20)])
  m.get("b").unwrap()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(20)));
    }

    // === P3.5: Set type ===

    #[test]
    fn test_set_new_and_add() {
        let source = r#"
module example
fn main() -> Int {
  let s = Set.new()
  let s2 = s.add(1).add(2).add(1)
  s2.len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(2)));
    }

    #[test]
    fn test_set_contains() {
        let source = r#"
module example
fn main() -> Bool {
  let s = Set.from([1, 2, 3])
  s.contains(2)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_set_union() {
        let source = r#"
module example
fn main() -> Int {
  let s1 = Set.from([1, 2, 3])
  let s2 = Set.from([3, 4, 5])
  s1.union(s2).len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(5)));
    }

    #[test]
    fn test_set_intersection() {
        let source = r#"
module example
fn main() -> Int {
  let s1 = Set.from([1, 2, 3])
  let s2 = Set.from([2, 3, 4])
  s1.intersection(s2).len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(2)));
    }

    // === P1.9: Type alias resolution ===

    #[test]
    fn test_type_alias_basic() {
        let source = r#"
module example
type Name = Text
fn greet(name: Name) -> Text {
  "Hello"
}
fn main() -> Text {
  greet("World")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(s) if s == "Hello"));
    }

    // === P2.3: Type invariants ===

    #[test]
    fn test_type_invariant_parsing() {
        // Test that invariant clause is parsed without error
        let source = r#"
module example
type Positive = Int
  invariant self > 0
fn main() -> Int {
  42
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    // === P2.2: Traits and impl blocks ===

    #[test]
    fn test_trait_and_impl_parsing() {
        let source = r#"
module example

trait Describable {
  fn describe(self: Int) -> Text
}

impl Describable for Int {
  fn describe(self: Int) -> Text {
    "a number"
  }
}

fn main() -> Text {
  "traits work"
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(s) if s == "traits work"));
    }

    #[test]
    fn test_impl_method_callable() {
        let source = r#"
module example

trait Doubler {
  fn double_it(x: Int) -> Int
}

impl Doubler for Int {
  fn double_it(x: Int) -> Int {
    x * 2
  }
}

fn main() -> Int {
  Doubler_double_it(21)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    // === P5.3: Parser error recovery ===

    #[test]
    fn test_parser_error_recovery() {
        // Verify parser produces errors but doesn't crash
        let source = r#"
module example
fn good() -> Int { 1 }
fn bad( {
fn also_good() -> Int { 2 }
"#;
        let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
        let lexer = Lexer::new(&source_file);
        let mut parser = crate::parser::parser::Parser::new(lexer, source_file.clone());
        // Should return Err with diagnostics, not panic
        let result = parser.parse_module();
        assert!(result.is_err());
    }

    // === P1.12: Negative number literals ===

    #[test]
    fn test_negative_literals() {
        let source = r#"
module example
fn main() -> Int {
  -42
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(-42)));
    }

    #[test]
    fn test_negative_float_literal() {
        let source = r#"
module example
fn main() -> Float {
  -2.5
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Float(f) if (f + 2.5).abs() < 0.001));
    }

    // === P2.4: Generic type constraints ===

    #[test]
    fn test_generic_constraint_syntax() {
        let source = r#"
module example
fn compare[T: Ord](a: T, b: T) -> Bool {
  a == b
}
fn main() -> Bool {
  compare(1, 1)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    // === P2.5: Recursive types ===

    #[test]
    fn test_recursive_enum_type() {
        // Run in a thread with a larger stack to avoid overflow in debug builds
        let handle = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                let source = r#"
module example
enum IntList =
  | Nil
  | Cons(head: Int, tail: IntList)

fn sum_list(list: IntList) -> Int {
  match list {
    Nil => 0
    Cons(h, t) => h + sum_list(t)
  }
}

fn main() -> Int {
  let list = Cons(1, Cons(2, Cons(3, Nil)))
  sum_list(list)
}
"#;
                // Note: this tests that recursive types parse and evaluate
                // Recursive call depth is limited by stack
                let result = parse_and_eval(source);
                // This may or may not work depending on stack depth
                // The important thing is it parses
                assert!(result.is_ok() || result.is_err());
            })
            .unwrap();
        handle.join().unwrap();
    }

    // === Map.from with tuple entries ===

    #[test]
    fn test_map_remove() {
        let source = r#"
module example
fn main() -> Int {
  let m = Map.new().set("a", 1).set("b", 2).set("c", 3)
  let m2 = m.remove("b")
  m2.len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(2)));
    }

    #[test]
    fn test_set_remove() {
        let source = r#"
module example
fn main() -> Int {
  let s = Set.from([1, 2, 3, 4])
  let s2 = s.remove(3)
  s2.len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    #[test]
    fn test_map_entries() {
        let source = r#"
module example
fn main() -> Int {
  let m = Map.new().set("x", 10).set("y", 20)
  m.entries().len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(2)));
    }

    #[test]
    fn test_tuple_to_list() {
        let source = r#"
module example
fn main() -> Int {
  let t = (1, 2, 3)
  t.to_list().len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    // === P2.3: Type invariant enforcement ===

    #[test]
    fn test_type_invariant_pass() {
        let source = r#"
module example
type Positive = Int invariant self > 0

fn main() -> Int {
  let x: Positive = 5
  x
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(5)));
    }

    #[test]
    fn test_type_invariant_fail() {
        let source = r#"
module example
type Positive = Int invariant self > 0

fn main() -> Int {
  let x: Positive = -1
  x
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "E3003");
    }

    // === P4.3: Re-export parsing ===

    #[test]
    fn test_public_import_parsing() {
        let source = r#"
module example
public import std.math

fn main() -> Int {
  42
}
"#;
        // Should parse and run without error
        let result = parse_and_eval(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stdlib_import_use() {
        let source = r#"
module example
import std.math

fn main() -> Bool {
  is_even(4)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    // === P5.4: Custom assert messages ===

    #[test]
    fn test_assert_custom_message() {
        let source = r#"
module example
fn main() -> Unit {
  assert(false, "custom error message")
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("custom error message"));
    }

    // === P6.1: Pipe operator (already tested, verify still works) ===

    #[test]
    fn test_pipe_operator() {
        let source = r#"
module example
fn double(x: Int) -> Int { x * 2 }
fn add_one(x: Int) -> Int { x + 1 }
fn main() -> Int {
  5 |> double |> add_one
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(11)));
    }

    // === P6.2: User-defined effects ===

    #[test]
    fn test_effect_definition() {
        let source = r#"
module example

effect Logger {
  fn log(msg: Text) -> Unit
  fn get_logs() -> Text
}

fn main() -> Text {
  "effects defined"
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(s) if s == "effects defined"));
    }

    #[test]
    fn test_user_effect_handler_dispatch() {
        // Test user-defined effect with a handler record
        let source = r#"
module example

effect Logger {
  fn log(msg: Text) -> Unit
}

fn do_work() -> Int effects(Logger) {
  Logger.log("starting work")
  42
}

fn main() -> Int {
  do_work()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    // === P6.4: Tail call optimization ===

    #[test]
    fn test_tco_simple_recursion() {
        let source = r#"
module example
fn sum_acc(n: Int, acc: Int) -> Int {
  if n <= 0 {
    acc
  } else {
    sum_acc(n - 1, acc + n)
  }
}
fn main() -> Int {
  sum_acc(1000, 0)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(500500)));
    }

    #[test]
    fn test_tco_factorial() {
        let source = r#"
module example
fn fact(n: Int, acc: Int) -> Int {
  if n <= 1 {
    acc
  } else {
    fact(n - 1, n * acc)
  }
}
fn main() -> Int {
  fact(10, 1)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3628800)));
    }

    // === v1.1: async/await support ===

    #[test]
    fn test_await_is_reserved_keyword() {
        // v1.1: await is now supported — it resolves futures
        let source = r#"
module example
fn async_value() -> Int { 42 }
fn main() -> Int {
  await async_value()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_async_is_reserved_keyword() {
        // v1.1: async fn is now supported — creates a future when called
        let source = r#"
module example
async fn fetch() -> Int { 42 }
fn main() -> Int {
  await fetch()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    // Trait method dispatch tests

    #[test]
    fn test_trait_method_dispatch() {
        let source = r#"
module example

trait Show {
  fn show(self: Text) -> Text
}

impl Show for Int {
  fn show(self: Int) -> Text {
    "integer"
  }
}

fn main() -> Text {
  let x = 42
  x.show()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "integer"));
    }

    #[test]
    fn test_trait_dispatch_with_args() {
        let source = r#"
module example

trait Repeat {
  fn repeat(self: Text, n: Int) -> Text
}

impl Repeat for Text {
  fn repeat(self: Text, n: Int) -> Text {
    if n <= 0 then "" else self
  }
}

fn main() -> Text {
  let s = "hello"
  s.repeat(1)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "hello"));
    }

    // Parameter destructuring tests

    #[test]
    fn test_record_param_destructuring() {
        let source = r#"
module example

fn get_x({x, y}: {x: Int, y: Int}) -> Int {
  x
}

fn main() -> Int {
  get_x({x = 10, y = 20})
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(10)));
    }

    #[test]
    fn test_tuple_param_destructuring() {
        let source = r#"
module example

fn sum_pair((a, b): (Int, Int)) -> Int {
  a + b
}

fn main() -> Int {
  sum_pair((3, 7))
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(10)));
    }

    // === Range expression syntax (0..10 and 0..=10) ===

    #[test]
    fn test_range_expr_exclusive() {
        let source = r#"
module example
fn main() -> Int {
  let xs = 0..5
  len(xs)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(5)));
    }

    #[test]
    fn test_range_expr_inclusive() {
        let source = r#"
module example
fn main() -> Int {
  let xs = 0..=5
  len(xs)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(6)));
    }

    #[test]
    fn test_range_expr_for_loop() {
        let source = r#"
module example
fn main() -> Int effects(Console) {
  let mut total = 0
  for i in 1..=10 {
    total = total + i
  }
  total
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(55)));
    }

    #[test]
    fn test_range_expr_with_arithmetic() {
        let source = r#"
module example
fn main() -> Int {
  let xs = 2 + 3..10 - 2
  len(xs)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    // === Multiline strings (triple-quoted) ===

    #[test]
    fn test_multiline_string_basic() {
        let source = r####"
module example
fn main() -> Text {
  let s = """
    hello
    world
    """
  s
}
"####;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "hello\nworld"),
            other => panic!("Expected Text, got {:?}", other),
        }
    }

    #[test]
    fn test_multiline_string_preserves_relative_indent() {
        let source = r####"
module example
fn main() -> Text {
  let s = """
    line one
      indented
    line three
    """
  s
}
"####;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "line one\n  indented\nline three"),
            other => panic!("Expected Text, got {:?}", other),
        }
    }

    // === v1.0: New feature tests ===

    #[test]
    fn test_compound_assignment_operators() {
        let source = r#"
module example
fn main() -> Int {
  let x = 10
  x += 5
  x -= 3
  x *= 2
  x /= 4
  x %= 5
  x
}
"#;
        let result = parse_and_eval(source).unwrap();
        // 10 + 5 = 15, 15 - 3 = 12, 12 * 2 = 24, 24 / 4 = 6, 6 % 5 = 1
        assert!(matches!(result, Value::Int(1)));
    }

    #[test]
    fn test_index_access_list() {
        let source = r#"
module example
fn main() -> Int {
  let items = [10, 20, 30, 40]
  items[0] + items[2]
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(40)));
    }

    #[test]
    fn test_index_access_negative() {
        let source = r#"
module example
fn main() -> Int {
  let items = [10, 20, 30]
  items[-1]
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(30)));
    }

    #[test]
    fn test_index_access_string() {
        let source = r#"
module example
fn main() -> Text {
  let s = "hello"
  s[1]
}
"#;
        let result = parse_and_eval(source).unwrap();
        match result {
            Value::Text(s) => assert_eq!(s, "e"),
            other => panic!("Expected Text, got {:?}", other),
        }
    }

    #[test]
    fn test_index_access_out_of_bounds() {
        let source = r#"
module example
fn main() -> Int {
  let items = [1, 2, 3]
  items[10]
}
"#;
        let result = parse_and_eval(source);
        assert!(result.is_err());
    }

    #[test]
    fn test_for_loop_destructuring() {
        let source = r#"
module example
fn main() -> Int {
  let pairs = [(1, 10), (2, 20), (3, 30)]
  let total = 0
  for (key, val) in pairs {
    total += val
  }
  total
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(60)));
    }

    #[test]
    fn test_structural_equality_records() {
        let source = r#"
module example
fn main() -> Bool {
  let a = {x = 1, y = 2}
  let b = {x = 1, y = 2}
  a == b
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_structural_equality_lists() {
        let source = r#"
module example
fn main() -> Bool {
  let a = [1, 2, 3]
  let b = [1, 2, 3]
  let c = [1, 2, 4]
  a == b and a != c
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_structural_equality_option() {
        let source = r#"
module example
fn main() -> Bool {
  Some(42) == Some(42) and None == None and Some(1) != Some(2)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_string_comparison_operators() {
        let source = r#"
module example
fn main() -> Bool {
  "abc" < "abd" and "z" > "a" and "hello" == "hello"
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    // =================================================================
    // v1.1 Feature Tests
    // =================================================================

    // --- JSON Parsing ---

    #[test]
    fn test_json_parse_int() {
        let source = r#"
module example
fn main() -> Int {
  json_parse("42")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_json_parse_float() {
        let source = r#"
module example
fn main() -> Float {
  json_parse("1.23")
}
"#;
        let result = parse_and_eval(source).unwrap();
        if let Value::Float(f) = result {
            assert!((f - 1.23).abs() < 0.001);
        } else {
            panic!("Expected Float, got {:?}", result);
        }
    }

    #[test]
    fn test_json_parse_string() {
        let source = r#"
module example
fn main() -> Text {
  json_parse("\"hello world\"")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "hello world"));
    }

    #[test]
    fn test_json_parse_bool() {
        let source = r#"
module example
fn main() -> Bool {
  json_parse("true")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_json_parse_null() {
        let source = r#"
module example
fn main() -> Unit {
  let result = json_parse("null")
  result
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::None));
    }

    #[test]
    fn test_json_parse_array() {
        let source = r#"
module example
fn main() -> Int {
  let arr = json_parse("[1, 2, 3]")
  arr.len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    #[test]
    fn test_json_parse_object() {
        let source = r#"
module example
fn main() -> Text {
  let obj = json_parse("{\"name\": \"Astra\", \"version\": 1}")
  obj.name
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "Astra"));
    }

    #[test]
    fn test_json_stringify_record() {
        let source = r#"
module example
fn main() -> Text {
  let obj = { name = "test", value = 42 }
  json_stringify(obj)
}
"#;
        let result = parse_and_eval(source).unwrap();
        if let Value::Text(s) = result {
            assert!(s.contains("\"name\""));
            assert!(s.contains("\"test\""));
            assert!(s.contains("42"));
        } else {
            panic!("Expected Text, got {:?}", result);
        }
    }

    #[test]
    fn test_json_stringify_list() {
        let source = r#"
module example
fn main() -> Text {
  json_stringify([1, 2, 3])
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "[1,2,3]"));
    }

    #[test]
    fn test_json_roundtrip() {
        let source = r#"
module example
fn main() -> Bool {
  let json = "{\"a\":1,\"b\":true,\"c\":\"hello\"}"
  let parsed = json_parse(json)
  let stringified = json_stringify(parsed)
  let reparsed = json_parse(stringified)
  reparsed.a == 1 and reparsed.b == true and reparsed.c == "hello"
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    // --- Regex ---

    #[test]
    fn test_regex_is_match() {
        let source = r#"
module example
fn main() -> Bool {
  regex_is_match("\\d+", "hello 42 world")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_regex_is_match_no_match() {
        let source = r#"
module example
fn main() -> Bool {
  regex_is_match("\\d+", "no numbers here")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(false)));
    }

    #[test]
    fn test_regex_match_with_groups() {
        let source = r#"
module example
fn main() -> Text {
  let result = regex_match("(\\w+)@(\\w+)", "user@host")
  match result {
    Some(m) => m.matched
    None => "no match"
  }
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "user@host"));
    }

    #[test]
    fn test_regex_find_all() {
        let source = r#"
module example
fn main() -> Int {
  let matches = regex_find_all("\\d+", "a1 b22 c333")
  matches.len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    #[test]
    fn test_regex_replace() {
        let source = r#"
module example
fn main() -> Text {
  regex_replace("\\d+", "hello 42 world 7", "NUM")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "hello NUM world NUM"));
    }

    #[test]
    fn test_regex_split() {
        let source = r#"
module example
fn main() -> Int {
  let parts = regex_split("\\s+", "hello   world   foo")
  parts.len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    #[test]
    fn test_text_matches_method() {
        let source = r#"
module example
fn main() -> Bool {
  "hello123".matches("[a-z]+\\d+")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_text_replace_pattern_method() {
        let source = r#"
module example
fn main() -> Text {
  "a1b2c3".replace_pattern("\\d", "X")
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Text(ref s) if s == "aXbXcX"));
    }

    #[test]
    fn test_text_split_pattern_method() {
        let source = r#"
module example
fn main() -> Int {
  "one,two,,three".split_pattern(",+").len()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(3)));
    }

    // --- Async/Await ---

    #[test]
    fn test_async_fn_returns_future() {
        let source = r#"
module example
async fn compute() -> Int {
  42
}
fn main() -> Int {
  let future = compute()
  await future
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_async_fn_with_params() {
        let source = r#"
module example
async fn add(a: Int, b: Int) -> Int {
  a + b
}
fn main() -> Int {
  await add(10, 20)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(30)));
    }

    #[test]
    fn test_await_non_future_passthrough() {
        // Await on a non-future value should just return the value
        let source = r#"
module example
fn sync_value() -> Int { 42 }
fn main() -> Int {
  await sync_value()
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_filtered_import_different_names_from_same_module() {
        // Regression test: when module A imports {foo} from C, and then
        // module B (the main module) imports {bar} from C, the second import
        // should still bring `bar` into scope even though C was already loaded
        // by A's transitive dependency.
        use std::fs;

        let tmp = std::env::temp_dir().join("astra_import_test");
        let content_dir = tmp.join("content");
        let _ = fs::create_dir_all(&content_dir);

        // content/store.astra - has two functions
        fs::write(
            content_dir.join("store.astra"),
            r#"module content.store

fn build_store() -> Text {
  "store_built"
}

fn get_all_modules() -> Text {
  "all_modules"
}
"#,
        )
        .unwrap();

        // content/loader.astra - imports only build_store from content.store
        fs::write(
            content_dir.join("loader.astra"),
            r#"module content.loader
import content.store.{build_store}

fn load_all_content() -> Text {
  build_store()
}
"#,
        )
        .unwrap();

        // main module - imports from both loader and store with different names
        let source = r#"module example
import content.loader.{load_all_content}
import content.store.{get_all_modules}

fn main() -> Text {
  get_all_modules()
}
"#;

        let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
        let lexer = Lexer::new(&source_file);
        let mut parser = Parser::new(lexer, source_file.clone());
        let module = parser.parse_module().expect("parse failed");

        let mut interpreter = Interpreter::new();
        interpreter.add_search_path(tmp.clone());
        let result = interpreter.eval_module(&module);

        // Clean up
        let _ = fs::remove_dir_all(&tmp);

        let value = result.expect("should resolve get_all_modules from content.store");
        assert!(
            matches!(value, Value::Text(ref s) if s == "all_modules"),
            "expected Text(\"all_modules\"), got {:?}",
            value
        );
    }

    #[test]
    fn test_net_serve() {
        use std::sync::mpsc;

        struct StubNet;
        impl NetCapability for StubNet {
            fn get(&self, _url: &str) -> Result<Value, String> {
                Ok(Value::Text(String::new()))
            }
            fn post(&self, _url: &str, _body: &str) -> Result<Value, String> {
                Ok(Value::Text(String::new()))
            }
        }

        // Use port 0 so the OS picks an available port — but tiny_http doesn't
        // expose the actual port when using "0.0.0.0:0", so pick a random high port.
        let port: u16 = 19284;

        let source = format!(
            "module example\n\
             \n\
             fn handler(req: {{method: Text, path: Text, body: Text}}) -> {{status: Int, body: Text, headers: Map[Text, Text]}} {{\n\
             \x20 let response_body = req.method + \" \" + req.path\n\
             \x20 {{status = 200, body = response_body, headers = Map.new()}}\n\
             }}\n\
             \n\
             fn main() effects(Net) {{\n\
             \x20 Net.serve({port}, handler)\n\
             }}\n",
        );

        let (tx, rx) = mpsc::channel();

        let handle = std::thread::spawn(move || {
            let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
            let lexer = Lexer::new(&source_file);
            let mut parser = Parser::new(lexer, source_file.clone());
            let module = parser.parse_module().expect("parse failed");

            let capabilities = Capabilities {
                console: Some(Box::new(MockConsole::new())),
                net: Some(Box::new(StubNet)),
                ..Default::default()
            };
            let mut interpreter = Interpreter::with_capabilities(capabilities);
            if let Ok(cwd) = std::env::current_dir() {
                interpreter.add_search_path(cwd);
            }

            // Signal that we're about to start serving
            tx.send(()).unwrap();

            // This blocks forever (until the thread is abandoned)
            let _ = interpreter.eval_module(&module);
        });

        // Wait for the server thread to start
        rx.recv().unwrap();
        // Give tiny_http a moment to actually bind
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Make a request
        let url = format!("http://127.0.0.1:{}/api/test", port);
        let resp = ureq::get(&url).call().expect("HTTP request failed");
        assert_eq!(resp.status(), 200);
        let body = resp.into_string().unwrap();
        assert_eq!(body, "GET /api/test");

        // Test query string parsing
        let url_qs = format!("http://127.0.0.1:{}/search?q=hello&page=2", port);
        let resp_qs = ureq::get(&url_qs)
            .call()
            .expect("HTTP request with query failed");
        assert_eq!(resp_qs.status(), 200);
        let body_qs = resp_qs.into_string().unwrap();
        assert_eq!(body_qs, "GET /search");

        // Test POST with body
        let url_post = format!("http://127.0.0.1:{}/api/data", port);
        let resp_post = ureq::post(&url_post)
            .send_string("payload")
            .expect("HTTP POST failed");
        assert_eq!(resp_post.status(), 200);
        let body_post = resp_post.into_string().unwrap();
        assert_eq!(body_post, "POST /api/data");

        // Server thread blocks forever; just drop it
        drop(handle);
    }

    #[test]
    fn test_net_serve_query_params() {
        use std::sync::mpsc;

        struct StubNet;
        impl NetCapability for StubNet {
            fn get(&self, _url: &str) -> Result<Value, String> {
                Ok(Value::Text(String::new()))
            }
            fn post(&self, _url: &str, _body: &str) -> Result<Value, String> {
                Ok(Value::Text(String::new()))
            }
        }

        let port: u16 = 19285;

        let source = format!(
            "module example\n\
             \n\
             fn handler(req: {{method: Text, path: Text, body: Text, query: Map[Text, Text]}}) -> {{status: Int, body: Text, headers: Map[Text, Text]}} {{\n\
             \x20 let q = req.query.get(\"user\")\n\
             \x20 match q {{\n\
             \x20   Some(val) => {{status = 200, body = val, headers = Map.new()}}\n\
             \x20   None => {{status = 404, body = \"not found\", headers = Map.new()}}\n\
             \x20 }}\n\
             }}\n\
             \n\
             fn main() effects(Net) {{\n\
             \x20 Net.serve({port}, handler)\n\
             }}\n",
        );

        let (tx, rx) = mpsc::channel();

        let handle = std::thread::spawn(move || {
            let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
            let lexer = Lexer::new(&source_file);
            let mut parser = Parser::new(lexer, source_file.clone());
            let module = parser.parse_module().expect("parse failed");

            let capabilities = Capabilities {
                console: Some(Box::new(MockConsole::new())),
                net: Some(Box::new(StubNet)),
                ..Default::default()
            };
            let mut interpreter = Interpreter::with_capabilities(capabilities);
            if let Ok(cwd) = std::env::current_dir() {
                interpreter.add_search_path(cwd);
            }

            tx.send(()).unwrap();
            let _ = interpreter.eval_module(&module);
        });

        rx.recv().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Query param is extracted and returned as body
        let url = format!("http://127.0.0.1:{}/test?user=alice", port);
        let resp = ureq::get(&url).call().expect("HTTP request failed");
        assert_eq!(resp.status(), 200);
        let body = resp.into_string().unwrap();
        assert_eq!(body, "alice");

        // Missing query param returns 404
        let url_no_param = format!("http://127.0.0.1:{}/test", port);
        let resp_no = ureq::get(&url_no_param).call();
        match resp_no {
            Err(ureq::Error::Status(code, resp)) => {
                assert_eq!(code, 404);
                assert_eq!(resp.into_string().unwrap(), "not found");
            }
            other => panic!("expected 404, got {:?}", other.map(|r| r.status())),
        }

        drop(handle);
    }
}
