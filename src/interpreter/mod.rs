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
mod tests;
