//! Interpreter for Astra programs
//!
//! Executes Astra code with capability-controlled effects.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use crate::parser::ast::*;

/// Runtime error with error code and message
#[derive(Debug, Clone)]
pub struct RuntimeError {
    /// Error code (E4xxx series for runtime errors)
    pub code: &'static str,
    /// Human-readable error message
    pub message: String,
    /// Early return value (for ? operator propagation)
    pub early_return: Option<Box<Value>>,
    /// Loop break signal
    pub is_break: bool,
    /// Loop continue signal
    pub is_continue: bool,
    /// Function return signal
    pub is_return: bool,
}

impl RuntimeError {
    /// Create a new runtime error
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            early_return: None,
            is_break: false,
            is_continue: false,
            is_return: false,
        }
    }

    /// Create an early return (for ? operator)
    pub fn early_return(value: Value) -> Self {
        Self {
            code: "EARLY_RETURN",
            message: String::new(),
            early_return: Some(Box::new(value)),
            is_break: false,
            is_continue: false,
            is_return: false,
        }
    }

    /// Create a break signal
    pub fn loop_break() -> Self {
        Self {
            code: "BREAK",
            message: String::new(),
            early_return: None,
            is_break: true,
            is_continue: false,
            is_return: false,
        }
    }

    /// Create a continue signal
    pub fn loop_continue() -> Self {
        Self {
            code: "CONTINUE",
            message: String::new(),
            early_return: None,
            is_break: false,
            is_continue: true,
            is_return: false,
        }
    }

    /// Create a function return signal
    pub fn function_return(value: Value) -> Self {
        Self {
            code: "RETURN",
            message: String::new(),
            early_return: Some(Box::new(value)),
            is_break: false,
            is_continue: false,
            is_return: true,
        }
    }

    /// Check if this is an early return
    pub fn is_early_return(&self) -> bool {
        self.early_return.is_some() && !self.is_return
    }

    /// Check if this is a control flow signal (break/continue/return)
    pub fn is_control_flow(&self) -> bool {
        self.is_break || self.is_continue || self.is_return || self.early_return.is_some()
    }

    /// Get the early return value
    pub fn get_early_return(self) -> Option<Value> {
        self.early_return.map(|v| *v)
    }

    /// Undefined variable error
    pub fn undefined_variable(name: &str) -> Self {
        Self::new("E4001", format!("undefined variable: {}", name))
    }

    /// Type mismatch error
    pub fn type_mismatch(expected: &str, got: &str) -> Self {
        Self::new(
            "E4002",
            format!("type mismatch: expected {}, got {}", expected, got),
        )
    }

    /// Division by zero error
    pub fn division_by_zero() -> Self {
        Self::new("E4003", "division by zero")
    }

    /// Capability not available error
    pub fn capability_not_available(cap: &str) -> Self {
        Self::new("E4004", format!("capability not available: {}", cap))
    }

    /// Unknown function error
    pub fn unknown_function(name: &str) -> Self {
        Self::new("E4005", format!("unknown function: {}", name))
    }

    /// Unknown method error
    pub fn unknown_method(receiver: &str, method: &str) -> Self {
        Self::new("E4006", format!("unknown method: {}.{}", receiver, method))
    }

    /// Pattern match failure error
    pub fn match_failure() -> Self {
        Self::new("E4007", "no pattern matched")
    }

    /// Invalid field access error
    pub fn invalid_field_access(field: &str) -> Self {
        Self::new("E4008", format!("invalid field access: {}", field))
    }

    /// Not callable error
    pub fn not_callable() -> Self {
        Self::new("E4009", "value is not callable")
    }

    /// Arity mismatch error
    pub fn arity_mismatch(expected: usize, got: usize) -> Self {
        Self::new(
            "E4010",
            format!("expected {} arguments, got {}", expected, got),
        )
    }

    /// Unwrap None error
    pub fn unwrap_none() -> Self {
        Self::new("E4011", "tried to unwrap None")
    }

    /// Unwrap Err error
    pub fn unwrap_err(msg: &str) -> Self {
        Self::new("E4012", format!("tried to unwrap Err: {}", msg))
    }

    /// Hole encountered error
    pub fn hole_encountered() -> Self {
        Self::new("E4013", "encountered incomplete code (hole)")
    }

    /// Precondition violation error
    pub fn precondition_violated(fn_name: &str) -> Self {
        Self::new(
            "E3001",
            format!("precondition violated in function `{}`", fn_name),
        )
    }

    /// Postcondition violation error
    pub fn postcondition_violated(fn_name: &str) -> Self {
        Self::new(
            "E3002",
            format!("postcondition violated in function `{}`", fn_name),
        )
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for RuntimeError {}

/// Runtime value
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Value {
    /// Unit value
    Unit,
    /// Integer
    Int(i64),
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
    /// Variant constructor for multi-field enums
    VariantConstructor {
        name: String,
        field_names: Vec<String>,
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
fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Unit, Value::Unit) => true,
        (Value::Int(a), Value::Int(b)) => a == b,
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
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| values_equal(x, y))
        }
        // Closures and constructors are never equal
        (Value::Closure { .. }, Value::Closure { .. }) => false,
        (Value::VariantConstructor { .. }, Value::VariantConstructor { .. }) => false,
        _ => false,
    }
}

/// Compare two values for ordering (used by sort)
fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => a.cmp(b),
        (Value::Text(a), Value::Text(b)) => a.cmp(b),
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        _ => std::cmp::Ordering::Equal,
    }
}

/// Execution environment
#[derive(Debug, Clone, Default)]
pub struct Environment {
    /// Variable bindings
    bindings: HashMap<String, Value>,
    /// Parent environment for lexical scoping
    parent: Option<Box<Environment>>,
}

impl Environment {
    /// Create a new empty environment
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            parent: None,
        }
    }

    /// Create a child environment
    pub fn child(&self) -> Self {
        Self {
            bindings: HashMap::new(),
            parent: Some(Box::new(self.clone())),
        }
    }

    /// Define a variable
    pub fn define(&mut self, name: String, value: Value) {
        self.bindings.insert(name, value);
    }

    /// Look up a variable
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        self.bindings
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.lookup(name)))
    }

    /// Update a mutable variable
    pub fn update(&mut self, name: &str, value: Value) -> bool {
        if self.bindings.contains_key(name) {
            self.bindings.insert(name.to_string(), value);
            true
        } else if let Some(parent) = &mut self.parent {
            parent.update(name, value)
        } else {
            false
        }
    }
}

/// Capability interface for Net effect
pub trait NetCapability {
    fn get(&self, url: &str) -> Result<Value, String>;
    fn post(&self, url: &str, body: &str) -> Result<Value, String>;
}

/// Capability interface for Fs effect
pub trait FsCapability {
    fn read(&self, path: &str) -> Result<String, String>;
    fn write(&self, path: &str, content: &str) -> Result<(), String>;
    fn exists(&self, path: &str) -> bool;
}

/// Capability interface for Clock effect
pub trait ClockCapability {
    fn now(&self) -> i64;
    fn sleep(&self, millis: u64);
}

/// Capability interface for Rand effect
pub trait RandCapability {
    fn int(&self, min: i64, max: i64) -> i64;
    fn bool(&self) -> bool;
    fn float(&self) -> f64;
}

/// Capability interface for Console effect
pub trait ConsoleCapability {
    fn print(&self, text: &str);
    fn println(&self, text: &str);
    fn read_line(&self) -> Option<String>;
}

/// Capability interface for Env effect
pub trait EnvCapability {
    fn get(&self, name: &str) -> Option<String>;
    fn args(&self) -> Vec<String>;
}

/// Runtime capabilities
#[derive(Default)]
pub struct Capabilities {
    pub net: Option<Box<dyn NetCapability>>,
    pub fs: Option<Box<dyn FsCapability>>,
    pub clock: Option<Box<dyn ClockCapability>>,
    pub rand: Option<Box<dyn RandCapability>>,
    pub console: Option<Box<dyn ConsoleCapability>>,
    pub env: Option<Box<dyn EnvCapability>>,
}

/// Interpreter for Astra programs
pub struct Interpreter {
    /// Global environment
    pub env: Environment,
    /// Available capabilities
    pub capabilities: Capabilities,
    /// Search paths for module resolution
    pub search_paths: Vec<PathBuf>,
    /// Already-loaded modules (to prevent circular imports)
    loaded_modules: std::collections::HashSet<String>,
}

impl Interpreter {
    /// Create a new interpreter
    pub fn new() -> Self {
        Self {
            env: Environment::new(),
            capabilities: Capabilities::default(),
            search_paths: Vec::new(),
            loaded_modules: std::collections::HashSet::new(),
        }
    }

    /// Create an interpreter with specific capabilities
    pub fn with_capabilities(capabilities: Capabilities) -> Self {
        Self {
            env: Environment::new(),
            capabilities,
            search_paths: Vec::new(),
            loaded_modules: std::collections::HashSet::new(),
        }
    }

    /// Load a module's definitions without running main
    /// Resolve a module path to a file path
    fn resolve_module_path(&self, segments: &[String]) -> Option<PathBuf> {
        let relative = segments.join("/") + ".astra";
        for search_path in &self.search_paths {
            let candidate = search_path.join(&relative);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        // Also check relative to cwd
        let candidate = PathBuf::from(&relative);
        if candidate.exists() {
            return Some(candidate);
        }
        None
    }

    /// Load an imported module by path segments
    fn load_import(&mut self, segments: &[String]) -> Result<(), RuntimeError> {
        let module_key = segments.join(".");
        if self.loaded_modules.contains(&module_key) {
            return Ok(()); // Already loaded
        }
        self.loaded_modules.insert(module_key);

        let file_path = match self.resolve_module_path(segments) {
            Some(p) => p,
            None => return Ok(()), // Silently skip unresolvable imports for now
        };

        let source = std::fs::read_to_string(&file_path).map_err(|e| {
            RuntimeError::new("E4016", format!("Failed to read module file: {}", e))
        })?;

        let source_file = crate::parser::span::SourceFile::new(file_path.clone(), source.clone());
        let lexer = crate::parser::lexer::Lexer::new(&source_file);
        let mut parser = crate::parser::parser::Parser::new(lexer, source_file.clone());
        let imported_module = parser.parse_module().map_err(|_| {
            RuntimeError::new(
                "E4017",
                format!("Failed to parse module: {}", file_path.display()),
            )
        })?;

        self.load_module(&imported_module)?;
        Ok(())
    }

    pub fn load_module(&mut self, module: &Module) -> Result<(), RuntimeError> {
        // Process imports first
        for item in &module.items {
            if let Item::Import(import) = item {
                self.load_import(&import.path.segments)?;
            }
        }

        // Collect all function and enum definitions into the environment
        for item in &module.items {
            match item {
                Item::FnDef(fn_def) => {
                    let params: Vec<String> =
                        fn_def.params.iter().map(|p| p.name.clone()).collect();
                    let closure = Value::Closure {
                        name: Some(fn_def.name.clone()),
                        params,
                        body: ClosureBody {
                            block: fn_def.body.clone(),
                            requires: fn_def.requires.clone(),
                            ensures: fn_def.ensures.clone(),
                        },
                        env: Environment::new(), // Will use global env at call time
                    };
                    self.env.define(fn_def.name.clone(), closure);
                }
                Item::EnumDef(enum_def) => {
                    // Register variant constructors
                    for variant in &enum_def.variants {
                        if !variant.fields.is_empty() {
                            let field_names: Vec<String> =
                                variant.fields.iter().map(|f| f.name.clone()).collect();
                            self.env.define(
                                variant.name.clone(),
                                Value::VariantConstructor {
                                    name: variant.name.clone(),
                                    field_names,
                                },
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Add a search path for module resolution
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    /// Evaluate a module
    pub fn eval_module(&mut self, module: &Module) -> Result<Value, RuntimeError> {
        // Load the module definitions
        self.load_module(module)?;

        // Look for and run main function if it exists
        if let Some(Value::Closure { params, body, .. }) = self.env.lookup("main").cloned() {
            if params.is_empty() {
                // Execute in a child of the global environment
                let call_env = self.env.child();
                let old_env = std::mem::replace(&mut self.env, call_env);
                let result = self.eval_block(&body.block);
                self.env = old_env;
                // Handle early returns from ? operator
                return match result {
                    Err(e) if e.is_early_return() => Ok(e.get_early_return().unwrap()),
                    other => other,
                };
            }
        }

        Ok(Value::Unit)
    }

    /// Evaluate an expression
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            // Literals
            Expr::IntLit { value, .. } => Ok(Value::Int(*value)),
            Expr::BoolLit { value, .. } => Ok(Value::Bool(*value)),
            Expr::TextLit { value, .. } => Ok(Value::Text(value.clone())),
            Expr::UnitLit { .. } => Ok(Value::Unit),

            // Identifiers
            Expr::Ident { name, .. } => {
                // Check for effect names first
                match name.as_str() {
                    "Console" | "Fs" | "Net" | "Clock" | "Rand" | "Env" => {
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
                    _ => self
                        .env
                        .lookup(name)
                        .cloned()
                        .ok_or_else(|| RuntimeError::undefined_variable(name)),
                }
            }
            Expr::QualifiedIdent { module, name, .. } => {
                // For now, treat as effect call target
                Ok(Value::Text(format!("{}.{}", module, name)))
            }

            // Binary operations
            Expr::Binary {
                op, left, right, ..
            } => {
                let left_val = self.eval_expr(left)?;
                let right_val = self.eval_expr(right)?;
                self.eval_binary_op(*op, &left_val, &right_val)
            }

            // Unary operations
            Expr::Unary { op, expr, .. } => {
                let val = self.eval_expr(expr)?;
                self.eval_unary_op(*op, &val)
            }

            // Function calls
            Expr::Call { func, args, .. } => {
                // Check for builtin functions first
                if let Expr::Ident { name, .. } = func.as_ref() {
                    match name.as_str() {
                        "assert" => {
                            if args.len() != 1 {
                                return Err(RuntimeError::arity_mismatch(1, args.len()));
                            }
                            let cond = self.eval_expr(&args[0])?;
                            return match cond {
                                Value::Bool(true) => Ok(Value::Unit),
                                Value::Bool(false) => {
                                    Err(RuntimeError::new("E4020", "assertion failed"))
                                }
                                _ => {
                                    Err(RuntimeError::type_mismatch("Bool", &format!("{:?}", cond)))
                                }
                            };
                        }
                        "assert_eq" => {
                            if args.len() != 2 {
                                return Err(RuntimeError::arity_mismatch(2, args.len()));
                            }
                            let left = self.eval_expr(&args[0])?;
                            let right = self.eval_expr(&args[1])?;
                            return if values_equal(&left, &right) {
                                Ok(Value::Unit)
                            } else {
                                Err(RuntimeError::new(
                                    "E4021",
                                    format!("assertion failed: {:?} != {:?}", left, right),
                                ))
                            };
                        }
                        // Option/Result constructors
                        "Some" => {
                            if args.len() != 1 {
                                return Err(RuntimeError::arity_mismatch(1, args.len()));
                            }
                            let val = self.eval_expr(&args[0])?;
                            return Ok(Value::Some(Box::new(val)));
                        }
                        "Ok" => {
                            if args.len() != 1 {
                                return Err(RuntimeError::arity_mismatch(1, args.len()));
                            }
                            let val = self.eval_expr(&args[0])?;
                            return Ok(Value::Ok(Box::new(val)));
                        }
                        "Err" => {
                            if args.len() != 1 {
                                return Err(RuntimeError::arity_mismatch(1, args.len()));
                            }
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
                            if args.len() != 1 {
                                return Err(RuntimeError::arity_mismatch(1, args.len()));
                            }
                            let val = self.eval_expr(&args[0])?;
                            return match val {
                                Value::Text(s) => Ok(Value::Int(s.len() as i64)),
                                Value::List(l) => Ok(Value::Int(l.len() as i64)),
                                _ => Err(RuntimeError::type_mismatch(
                                    "Text or List",
                                    &format!("{:?}", val),
                                )),
                            };
                        }
                        // N3: to_text builtin
                        "to_text" => {
                            if args.len() != 1 {
                                return Err(RuntimeError::arity_mismatch(1, args.len()));
                            }
                            let val = self.eval_expr(&args[0])?;
                            return Ok(Value::Text(format_value(&val)));
                        }
                        // P1.1: range builtin
                        "range" => {
                            if args.len() != 2 {
                                return Err(RuntimeError::arity_mismatch(2, args.len()));
                            }
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
                            if args.len() != 1 {
                                return Err(RuntimeError::arity_mismatch(1, args.len()));
                            }
                            let val = self.eval_expr(&args[0])?;
                            return match val {
                                Value::Int(n) => Ok(Value::Int(n.abs())),
                                _ => Err(RuntimeError::type_mismatch("Int", &format!("{:?}", val))),
                            };
                        }
                        "min" => {
                            if args.len() != 2 {
                                return Err(RuntimeError::arity_mismatch(2, args.len()));
                            }
                            let a = self.eval_expr(&args[0])?;
                            let b = self.eval_expr(&args[1])?;
                            return match (a, b) {
                                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.min(b))),
                                _ => Err(RuntimeError::type_mismatch("(Int, Int)", "other")),
                            };
                        }
                        "max" => {
                            if args.len() != 2 {
                                return Err(RuntimeError::arity_mismatch(2, args.len()));
                            }
                            let a = self.eval_expr(&args[0])?;
                            let b = self.eval_expr(&args[1])?;
                            return match (a, b) {
                                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.max(b))),
                                _ => Err(RuntimeError::type_mismatch("(Int, Int)", "other")),
                            };
                        }
                        "pow" => {
                            if args.len() != 2 {
                                return Err(RuntimeError::arity_mismatch(2, args.len()));
                            }
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
                                _ => Err(RuntimeError::type_mismatch("(Int, Int)", "other")),
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
                self.call_function(func_val, arg_vals)
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
                        // Create new environment with pattern bindings
                        let mut match_env = self.env.child();
                        for (name, value) in bindings {
                            match_env.define(name, value);
                        }

                        // Check guard if present
                        if let Some(guard) = &arm.guard {
                            let old_env = std::mem::replace(&mut self.env, match_env.clone());
                            let guard_val = self.eval_expr(guard)?;
                            self.env = old_env;
                            match guard_val {
                                Value::Bool(true) => {}
                                Value::Bool(false) => continue,
                                _ => {
                                    return Err(RuntimeError::type_mismatch(
                                        "Bool",
                                        &format!("{:?}", guard_val),
                                    ))
                                }
                            }
                        }

                        let old_env = std::mem::replace(&mut self.env, match_env);
                        let result = self.eval_expr(&arm.body);
                        self.env = old_env;
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
                    _ => Err(RuntimeError::type_mismatch("Record", &format!("{:?}", val))),
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
                iter,
                body,
                ..
            } => {
                let iter_val = self.eval_expr(iter)?;
                match iter_val {
                    Value::List(items) => {
                        'for_loop: for item in &items {
                            self.env.define(binding.clone(), item.clone());
                            for stmt in &body.stmts {
                                match self.eval_stmt(stmt) {
                                    Ok(()) => {}
                                    Err(e) if e.is_break => break 'for_loop,
                                    Err(e) if e.is_continue => continue 'for_loop,
                                    Err(e) if e.is_return => {
                                        self.env.bindings.remove(binding);
                                        return Err(e);
                                    }
                                    Err(e) => {
                                        self.env.bindings.remove(binding);
                                        return Err(e);
                                    }
                                }
                            }
                            if let Some(expr) = &body.expr {
                                match self.eval_expr(expr) {
                                    Ok(_) => {}
                                    Err(e) if e.is_break => break 'for_loop,
                                    Err(e) if e.is_continue => continue 'for_loop,
                                    Err(e) => {
                                        self.env.bindings.remove(binding);
                                        return Err(e);
                                    }
                                }
                            }
                        }
                        self.env.bindings.remove(binding);
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

            // Hole (incomplete code)
            Expr::Hole { .. } => Err(RuntimeError::hole_encountered()),
        }
    }

    /// Evaluate a statement
    pub fn eval_stmt(&mut self, stmt: &Stmt) -> Result<(), RuntimeError> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let val = self.eval_expr(value)?;
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
        let old_env = self.env.clone();
        self.env = self.env.child();

        // Execute all statements
        for stmt in &block.stmts {
            match self.eval_stmt(stmt) {
                Ok(()) => {}
                Err(e) if e.is_return => {
                    self.env = old_env;
                    return Err(e);
                }
                Err(e) => {
                    self.env = old_env;
                    return Err(e);
                }
            }
        }

        // Evaluate trailing expression or return Unit
        let result = if let Some(expr) = &block.expr {
            self.eval_expr(expr)?
        } else {
            Value::Unit
        };

        self.env = old_env;
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

            // Logical operations
            (BinaryOp::And, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a && *b)),
            (BinaryOp::Or, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a || *b)),

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
            (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            _ => Err(RuntimeError::type_mismatch(
                &format!("valid type for {:?}", op),
                &format!("{:?}", val),
            )),
        }
    }

    /// Call a function value with arguments
    fn call_function(&mut self, func: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
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

                // Determine parent environment:
                // - For module-level functions (empty captured env), use current env
                // - For closures (non-empty captured env), use captured env
                let parent_env = if env.bindings.is_empty() && env.parent.is_none() {
                    &self.env
                } else {
                    &env
                };

                // Create new environment with appropriate parent
                let mut call_env = parent_env.child();
                for (param, arg) in params.iter().zip(args) {
                    call_env.define(param.clone(), arg);
                }

                let fn_name = name.as_deref().unwrap_or("<anonymous>");

                // Check preconditions (requires clauses)
                if !body.requires.is_empty() {
                    let old_env = std::mem::replace(&mut self.env, call_env.clone());
                    for req_expr in &body.requires {
                        let cond = self.eval_expr(req_expr)?;
                        match cond {
                            Value::Bool(true) => {}
                            Value::Bool(false) => {
                                self.env = old_env;
                                return Err(RuntimeError::precondition_violated(fn_name));
                            }
                            _ => {
                                self.env = old_env;
                                return Err(RuntimeError::type_mismatch(
                                    "Bool",
                                    &format!("{:?}", cond),
                                ));
                            }
                        }
                    }
                    self.env = old_env;
                }

                // Save call_env for postcondition checking (has param bindings)
                let saved_call_env = if !body.ensures.is_empty() {
                    Some(call_env.clone())
                } else {
                    None
                };

                // Execute the body
                let old_env = std::mem::replace(&mut self.env, call_env);
                let result = self.eval_block(&body.block);
                self.env = old_env;

                // Handle early returns from ? operator and return statements
                let result = match result {
                    Err(e) if e.is_return => Ok(e.get_early_return().unwrap_or(Value::Unit)),
                    Err(e) if e.is_early_return() => Ok(e.get_early_return().unwrap()),
                    other => other,
                }?;

                // Check postconditions (ensures clauses)
                if !body.ensures.is_empty() {
                    // Use the call env (which has param bindings) and add `result`
                    let mut ensures_env = saved_call_env.unwrap();
                    ensures_env.define("result".to_string(), result.clone());
                    let old_env = std::mem::replace(&mut self.env, ensures_env);
                    for ens_expr in &body.ensures {
                        let cond = self.eval_expr(ens_expr)?;
                        match cond {
                            Value::Bool(true) => {}
                            Value::Bool(false) => {
                                self.env = old_env;
                                return Err(RuntimeError::postcondition_violated(fn_name));
                            }
                            _ => {
                                self.env = old_env;
                                return Err(RuntimeError::type_mismatch(
                                    "Bool",
                                    &format!("{:?}", cond),
                                ));
                            }
                        }
                    }
                    self.env = old_env;
                }

                Ok(result)
            }
            _ => Err(RuntimeError::not_callable()),
        }
    }

    /// Call a method on a receiver (for effects like Console.println)
    fn call_method(
        &mut self,
        receiver: &Value,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        // Check if receiver is an effect identifier
        match receiver {
            Value::Text(name) if name == "Console.println" || name.starts_with("Console") => {
                self.call_console_method(method, args)
            }
            Value::Text(name) if name.starts_with("Fs") => self.call_fs_method(method, args),
            Value::Text(name) if name.starts_with("Net") => self.call_net_method(method, args),
            Value::Text(name) if name.starts_with("Clock") => self.call_clock_method(method, args),
            Value::Text(name) if name.starts_with("Rand") => self.call_rand_method(method, args),
            Value::Text(name) if name.starts_with("Env") => self.call_env_method(method, args),
            // For direct calls like Console.println()
            _ => {
                // Try to interpret receiver as effect name
                if let Value::Text(ref s) = receiver {
                    if s == "Console" {
                        return self.call_console_method(method, args);
                    }
                }
                // Check if this is an Option/Result method
                self.call_value_method(receiver, method, args)
            }
        }
    }

    /// Call a Console effect method
    fn call_console_method(
        &mut self,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let console = self
            .capabilities
            .console
            .as_ref()
            .ok_or_else(|| RuntimeError::capability_not_available("Console"))?;

        match method {
            "print" => {
                if let Some(Value::Text(text)) = args.first() {
                    console.print(text);
                    Ok(Value::Unit)
                } else if let Some(val) = args.first() {
                    console.print(&format_value(val));
                    Ok(Value::Unit)
                } else {
                    Err(RuntimeError::arity_mismatch(1, args.len()))
                }
            }
            "println" => {
                if let Some(Value::Text(text)) = args.first() {
                    console.println(text);
                    Ok(Value::Unit)
                } else if let Some(val) = args.first() {
                    console.println(&format_value(val));
                    Ok(Value::Unit)
                } else {
                    console.println("");
                    Ok(Value::Unit)
                }
            }
            "read_line" => {
                let line = console.read_line();
                match line {
                    Some(s) => Ok(Value::Some(Box::new(Value::Text(s)))),
                    None => Ok(Value::None),
                }
            }
            _ => Err(RuntimeError::unknown_method("Console", method)),
        }
    }

    /// Call a Fs effect method
    fn call_fs_method(&self, method: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let fs = self
            .capabilities
            .fs
            .as_ref()
            .ok_or_else(|| RuntimeError::capability_not_available("Fs"))?;

        match method {
            "read" => {
                if let Some(Value::Text(path)) = args.first() {
                    match fs.read(path) {
                        Ok(content) => Ok(Value::Ok(Box::new(Value::Text(content)))),
                        Err(e) => Ok(Value::Err(Box::new(Value::Text(e)))),
                    }
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            "write" => {
                if args.len() == 2 {
                    if let (Some(Value::Text(path)), Some(Value::Text(content))) =
                        (args.first(), args.get(1))
                    {
                        match fs.write(path, content) {
                            Ok(()) => Ok(Value::Ok(Box::new(Value::Unit))),
                            Err(e) => Ok(Value::Err(Box::new(Value::Text(e)))),
                        }
                    } else {
                        Err(RuntimeError::type_mismatch("(Text, Text)", "other"))
                    }
                } else {
                    Err(RuntimeError::arity_mismatch(2, args.len()))
                }
            }
            "exists" => {
                if let Some(Value::Text(path)) = args.first() {
                    Ok(Value::Bool(fs.exists(path)))
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            _ => Err(RuntimeError::unknown_method("Fs", method)),
        }
    }

    /// Call a Net effect method
    fn call_net_method(&self, method: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let net = self
            .capabilities
            .net
            .as_ref()
            .ok_or_else(|| RuntimeError::capability_not_available("Net"))?;

        match method {
            "get" => {
                if let Some(Value::Text(url)) = args.first() {
                    match net.get(url) {
                        Ok(val) => Ok(Value::Ok(Box::new(val))),
                        Err(e) => Ok(Value::Err(Box::new(Value::Text(e)))),
                    }
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            "post" => {
                if args.len() == 2 {
                    if let (Some(Value::Text(url)), Some(Value::Text(body))) =
                        (args.first(), args.get(1))
                    {
                        match net.post(url, body) {
                            Ok(val) => Ok(Value::Ok(Box::new(val))),
                            Err(e) => Ok(Value::Err(Box::new(Value::Text(e)))),
                        }
                    } else {
                        Err(RuntimeError::type_mismatch("(Text, Text)", "other"))
                    }
                } else {
                    Err(RuntimeError::arity_mismatch(2, args.len()))
                }
            }
            _ => Err(RuntimeError::unknown_method("Net", method)),
        }
    }

    /// Call a Clock effect method
    fn call_clock_method(&self, method: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let clock = self
            .capabilities
            .clock
            .as_ref()
            .ok_or_else(|| RuntimeError::capability_not_available("Clock"))?;

        match method {
            "now" => Ok(Value::Int(clock.now())),
            "sleep" => {
                if let Some(Value::Int(millis)) = args.first() {
                    clock.sleep(*millis as u64);
                    Ok(Value::Unit)
                } else {
                    Err(RuntimeError::type_mismatch("Int", "other"))
                }
            }
            _ => Err(RuntimeError::unknown_method("Clock", method)),
        }
    }

    /// Call a Rand effect method
    fn call_rand_method(&self, method: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let rand = self
            .capabilities
            .rand
            .as_ref()
            .ok_or_else(|| RuntimeError::capability_not_available("Rand"))?;

        match method {
            "int" => {
                if args.len() == 2 {
                    if let (Some(Value::Int(min)), Some(Value::Int(max))) =
                        (args.first(), args.get(1))
                    {
                        Ok(Value::Int(rand.int(*min, *max)))
                    } else {
                        Err(RuntimeError::type_mismatch("(Int, Int)", "other"))
                    }
                } else {
                    Err(RuntimeError::arity_mismatch(2, args.len()))
                }
            }
            "bool" => Ok(Value::Bool(rand.bool())),
            "float" => {
                // Return as int for now since we don't have floats
                let f = rand.float();
                Ok(Value::Int((f * 1000000.0) as i64))
            }
            _ => Err(RuntimeError::unknown_method("Rand", method)),
        }
    }

    /// Call an Env effect method
    fn call_env_method(&self, method: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let env_cap = self
            .capabilities
            .env
            .as_ref()
            .ok_or_else(|| RuntimeError::capability_not_available("Env"))?;

        match method {
            "get" => {
                if let Some(Value::Text(name)) = args.first() {
                    match env_cap.get(name) {
                        Some(val) => Ok(Value::Some(Box::new(Value::Text(val)))),
                        None => Ok(Value::None),
                    }
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            "args" => {
                let args_vec: Vec<Value> = env_cap.args().into_iter().map(Value::Text).collect();
                // Return as a record with numbered fields for now
                let mut record = HashMap::new();
                for (i, arg) in args_vec.into_iter().enumerate() {
                    record.insert(i.to_string(), arg);
                }
                Ok(Value::Record(record))
            }
            _ => Err(RuntimeError::unknown_method("Env", method)),
        }
    }

    /// Call a method on a value (for Option/Result operations)
    /// Try to handle higher-order methods that need &mut self for call_function.
    /// Returns Some(result) if handled, None if not a higher-order method.
    fn try_ho_method(
        &mut self,
        receiver: &Value,
        method: &str,
        args: Vec<Value>,
    ) -> Option<Result<Value, RuntimeError>> {
        match (receiver, method) {
            (Value::List(items), "map") => {
                let items = items.clone();
                Some(self.ho_list_map(&items, args))
            }
            (Value::List(items), "filter") => {
                let items = items.clone();
                Some(self.ho_list_filter(&items, args))
            }
            (Value::List(items), "fold") => {
                let items = items.clone();
                Some(self.ho_list_fold(&items, args))
            }
            (Value::List(items), "each") => {
                let items = items.clone();
                Some(self.ho_list_each(&items, args))
            }
            (Value::List(items), "any") => {
                let items = items.clone();
                Some(self.ho_list_any(&items, args))
            }
            (Value::List(items), "all") => {
                let items = items.clone();
                Some(self.ho_list_all(&items, args))
            }
            (Value::List(items), "flat_map") => {
                let items = items.clone();
                Some(self.ho_list_flat_map(&items, args))
            }
            (Value::List(items), "find") => {
                let items = items.clone();
                Some(self.ho_list_find(&items, args))
            }
            (Value::Some(inner), "map") => {
                let inner = (**inner).clone();
                Some(self.ho_option_map(inner, args))
            }
            (Value::None, "map") => Some(Ok(Value::None)),
            (Value::Ok(inner), "map") => {
                let inner = (**inner).clone();
                Some(self.ho_result_map(inner, args))
            }
            (Value::Err(_), "map") => Some(Ok(receiver.clone())),
            (Value::Ok(_), "map_err") => Some(Ok(receiver.clone())),
            (Value::Err(inner), "map_err") => {
                let inner = (**inner).clone();
                Some(self.ho_result_map_err(inner, args))
            }
            _ => None,
        }
    }

    fn ho_list_map(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        let mut result = Vec::new();
        for item in items {
            result.push(self.call_function(func.clone(), vec![item.clone()])?);
        }
        Ok(Value::List(result))
    }

    fn ho_list_filter(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        let mut result = Vec::new();
        for item in items {
            match self.call_function(func.clone(), vec![item.clone()])? {
                Value::Bool(true) => result.push(item.clone()),
                Value::Bool(false) => {}
                other => return Err(RuntimeError::type_mismatch("Bool", &format!("{:?}", other))),
            }
        }
        Ok(Value::List(result))
    }

    fn ho_list_fold(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        if args.len() != 2 {
            return Err(RuntimeError::arity_mismatch(2, args.len()));
        }
        let mut args_iter = args.into_iter();
        let mut acc = args_iter.next().unwrap();
        let func = args_iter.next().unwrap();
        for item in items {
            acc = self.call_function(func.clone(), vec![acc, item.clone()])?;
        }
        Ok(acc)
    }

    fn ho_list_each(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        for item in items {
            self.call_function(func.clone(), vec![item.clone()])?;
        }
        Ok(Value::Unit)
    }

    fn ho_list_any(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        for item in items {
            if let Value::Bool(true) = self.call_function(func.clone(), vec![item.clone()])? {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    }

    fn ho_list_all(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        for item in items {
            if let Value::Bool(false) = self.call_function(func.clone(), vec![item.clone()])? {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    }

    fn ho_list_flat_map(
        &mut self,
        items: &[Value],
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        let mut result = Vec::new();
        for item in items {
            match self.call_function(func.clone(), vec![item.clone()])? {
                Value::List(inner) => result.extend(inner),
                other => return Err(RuntimeError::type_mismatch("List", &format!("{:?}", other))),
            }
        }
        Ok(Value::List(result))
    }

    fn ho_list_find(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        for item in items {
            if let Value::Bool(true) = self.call_function(func.clone(), vec![item.clone()])? {
                return Ok(Value::Some(Box::new(item.clone())));
            }
        }
        Ok(Value::None)
    }

    fn ho_option_map(&mut self, inner: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        let result = self.call_function(func, vec![inner])?;
        Ok(Value::Some(Box::new(result)))
    }

    fn ho_result_map(&mut self, inner: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        let result = self.call_function(func, vec![inner])?;
        Ok(Value::Ok(Box::new(result)))
    }

    fn ho_result_map_err(&mut self, inner: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        let result = self.call_function(func, vec![inner])?;
        Ok(Value::Err(Box::new(result)))
    }

    fn call_value_method(
        &mut self,
        receiver: &Value,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        // Handle higher-order methods that need &mut self for call_function
        // These are extracted before the immutable match to avoid borrow conflicts
        if let Some(result) = self.try_ho_method(receiver, method, args.clone()) {
            return result;
        }

        match (receiver, method) {
            // Option methods
            (Value::Some(inner), "unwrap") => Ok((**inner).clone()),
            (Value::None, "unwrap") => Err(RuntimeError::unwrap_none()),
            (Value::Some(_), "is_some") => Ok(Value::Bool(true)),
            (Value::None, "is_some") => Ok(Value::Bool(false)),
            (Value::Some(_), "is_none") => Ok(Value::Bool(false)),
            (Value::None, "is_none") => Ok(Value::Bool(true)),

            // Result methods
            (Value::Ok(inner), "unwrap") => Ok((**inner).clone()),
            (Value::Err(e), "unwrap") => {
                let msg = match &**e {
                    Value::Text(s) => s.clone(),
                    _ => "error".to_string(),
                };
                Err(RuntimeError::unwrap_err(&msg))
            }
            (Value::Ok(_), "is_ok") => Ok(Value::Bool(true)),
            (Value::Err(_), "is_ok") => Ok(Value::Bool(false)),
            (Value::Ok(_), "is_err") => Ok(Value::Bool(false)),
            (Value::Err(_), "is_err") => Ok(Value::Bool(true)),

            // unwrap_or for Option/Result
            (Value::Some(inner), "unwrap_or") => Ok((**inner).clone()),
            (Value::None, "unwrap_or") => args
                .into_iter()
                .next()
                .ok_or_else(|| RuntimeError::arity_mismatch(1, 0)),
            (Value::Ok(inner), "unwrap_or") => Ok((**inner).clone()),
            (Value::Err(_), "unwrap_or") => args
                .into_iter()
                .next()
                .ok_or_else(|| RuntimeError::arity_mismatch(1, 0)),

            // List methods
            (Value::List(items), "len") => Ok(Value::Int(items.len() as i64)),
            (Value::List(items), "get") => {
                if let Some(Value::Int(idx)) = args.first() {
                    let idx = *idx as usize;
                    if idx < items.len() {
                        Ok(Value::Some(Box::new(items[idx].clone())))
                    } else {
                        Ok(Value::None)
                    }
                } else {
                    Err(RuntimeError::type_mismatch("Int", "other"))
                }
            }
            (Value::List(items), "contains") => {
                if let Some(needle) = args.first() {
                    Ok(Value::Bool(items.iter().any(|v| values_equal(v, needle))))
                } else {
                    Err(RuntimeError::arity_mismatch(1, 0))
                }
            }
            (Value::List(items), "is_empty") => Ok(Value::Bool(items.is_empty())),
            (Value::List(items), "head") => {
                if let Some(first) = items.first() {
                    Ok(Value::Some(Box::new(first.clone())))
                } else {
                    Ok(Value::None)
                }
            }
            (Value::List(items), "last") => {
                if let Some(last) = items.last() {
                    Ok(Value::Some(Box::new(last.clone())))
                } else {
                    Ok(Value::None)
                }
            }
            (Value::List(items), "push") => {
                if let Some(val) = args.into_iter().next() {
                    let mut new_items = items.clone();
                    new_items.push(val);
                    Ok(Value::List(new_items))
                } else {
                    Err(RuntimeError::arity_mismatch(1, 0))
                }
            }
            (Value::List(items), "concat") => {
                if let Some(Value::List(other)) = args.first() {
                    let mut new_items = items.clone();
                    new_items.extend(other.clone());
                    Ok(Value::List(new_items))
                } else {
                    Err(RuntimeError::type_mismatch("List", "other"))
                }
            }
            // P3.1: tail, reverse, sort
            (Value::List(items), "tail") => {
                if items.is_empty() {
                    Ok(Value::List(vec![]))
                } else {
                    Ok(Value::List(items[1..].to_vec()))
                }
            }
            (Value::List(items), "reverse") => {
                let mut rev = items.clone();
                rev.reverse();
                Ok(Value::List(rev))
            }
            (Value::List(items), "sort") => {
                let mut sorted = items.clone();
                sorted.sort_by(compare_values);
                Ok(Value::List(sorted))
            }
            // P3.2: take, drop, slice, enumerate, zip
            (Value::List(items), "take") => {
                if let Some(Value::Int(n)) = args.first() {
                    let n = (*n).max(0) as usize;
                    Ok(Value::List(items.iter().take(n).cloned().collect()))
                } else {
                    Err(RuntimeError::type_mismatch("Int", "other"))
                }
            }
            (Value::List(items), "drop") => {
                if let Some(Value::Int(n)) = args.first() {
                    let n = (*n).max(0) as usize;
                    Ok(Value::List(items.iter().skip(n).cloned().collect()))
                } else {
                    Err(RuntimeError::type_mismatch("Int", "other"))
                }
            }
            (Value::List(items), "slice") => {
                if args.len() == 2 {
                    if let (Some(Value::Int(start)), Some(Value::Int(end))) =
                        (args.first(), args.get(1))
                    {
                        let start = (*start).max(0) as usize;
                        let end = (*end).max(0) as usize;
                        let end = end.min(items.len());
                        if start <= end {
                            Ok(Value::List(items[start..end].to_vec()))
                        } else {
                            Ok(Value::List(vec![]))
                        }
                    } else {
                        Err(RuntimeError::type_mismatch("(Int, Int)", "other"))
                    }
                } else {
                    Err(RuntimeError::arity_mismatch(2, args.len()))
                }
            }
            (Value::List(items), "enumerate") => {
                let pairs: Vec<Value> = items
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let mut fields = HashMap::new();
                        fields.insert("index".to_string(), Value::Int(i as i64));
                        fields.insert("value".to_string(), v.clone());
                        Value::Record(fields)
                    })
                    .collect();
                Ok(Value::List(pairs))
            }
            (Value::List(items), "zip") => {
                if let Some(Value::List(other)) = args.first() {
                    let pairs: Vec<Value> = items
                        .iter()
                        .zip(other.iter())
                        .map(|(a, b)| {
                            let mut fields = HashMap::new();
                            fields.insert("first".to_string(), a.clone());
                            fields.insert("second".to_string(), b.clone());
                            Value::Record(fields)
                        })
                        .collect();
                    Ok(Value::List(pairs))
                } else {
                    Err(RuntimeError::type_mismatch("List", "other"))
                }
            }

            // Text methods
            (Value::Text(s), "len") => Ok(Value::Int(s.len() as i64)),
            (Value::Text(s), "to_upper") => Ok(Value::Text(s.to_uppercase())),
            (Value::Text(s), "to_lower") => Ok(Value::Text(s.to_lowercase())),
            (Value::Text(s), "trim") => Ok(Value::Text(s.trim().to_string())),
            (Value::Text(s), "contains") => {
                if let Some(Value::Text(needle)) = args.first() {
                    Ok(Value::Bool(s.contains(needle.as_str())))
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "starts_with") => {
                if let Some(Value::Text(prefix)) = args.first() {
                    Ok(Value::Bool(s.starts_with(prefix.as_str())))
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "ends_with") => {
                if let Some(Value::Text(suffix)) = args.first() {
                    Ok(Value::Bool(s.ends_with(suffix.as_str())))
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "split") => {
                if let Some(Value::Text(delimiter)) = args.first() {
                    let parts: Vec<Value> = s
                        .split(delimiter.as_str())
                        .map(|p| Value::Text(p.to_string()))
                        .collect();
                    Ok(Value::List(parts))
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "replace") => {
                if args.len() == 2 {
                    if let (Some(Value::Text(from)), Some(Value::Text(to))) =
                        (args.first(), args.get(1))
                    {
                        Ok(Value::Text(s.replace(from.as_str(), to.as_str())))
                    } else {
                        Err(RuntimeError::type_mismatch("(Text, Text)", "other"))
                    }
                } else {
                    Err(RuntimeError::arity_mismatch(2, args.len()))
                }
            }
            (Value::Text(s), "chars") => {
                let chars: Vec<Value> = s.chars().map(|c| Value::Text(c.to_string())).collect();
                Ok(Value::List(chars))
            }
            // P3.3: join, repeat, index_of, substring
            (Value::Text(s), "repeat") => {
                if let Some(Value::Int(n)) = args.first() {
                    Ok(Value::Text(s.repeat((*n).max(0) as usize)))
                } else {
                    Err(RuntimeError::type_mismatch("Int", "other"))
                }
            }
            (Value::Text(s), "index_of") => {
                if let Some(Value::Text(needle)) = args.first() {
                    match s.find(needle.as_str()) {
                        Some(pos) => Ok(Value::Some(Box::new(Value::Int(pos as i64)))),
                        None => Ok(Value::None),
                    }
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "substring") => {
                if args.len() == 2 {
                    if let (Some(Value::Int(start)), Some(Value::Int(end))) =
                        (args.first(), args.get(1))
                    {
                        let start = (*start).max(0) as usize;
                        let end = (*end).max(0) as usize;
                        let end = end.min(s.len());
                        if start <= end && start <= s.len() {
                            Ok(Value::Text(s[start..end].to_string()))
                        } else {
                            Ok(Value::Text(String::new()))
                        }
                    } else {
                        Err(RuntimeError::type_mismatch("(Int, Int)", "other"))
                    }
                } else {
                    Err(RuntimeError::arity_mismatch(2, args.len()))
                }
            }
            // List join method (on List[Text])
            (Value::List(items), "join") => {
                if let Some(Value::Text(sep)) = args.first() {
                    let strs: Vec<String> = items.iter().map(format_value).collect();
                    Ok(Value::Text(strs.join(sep.as_str())))
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }

            _ => Err(RuntimeError::unknown_method(
                &format!("{:?}", receiver),
                method,
            )),
        }
    }
}

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
    }
}

/// Format a value for display
fn format_value(value: &Value) -> String {
    match value {
        Value::Unit => "()".to_string(),
        Value::Int(n) => n.to_string(),
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
        Value::List(items) => {
            let item_strs: Vec<String> = items.iter().map(format_value).collect();
            format!("[{}]", item_strs.join(", "))
        }
        Value::Some(inner) => format!("Some({})", format_value(inner)),
        Value::None => "None".to_string(),
        Value::Ok(inner) => format!("Ok({})", format_value(inner)),
        Value::Err(inner) => format!("Err({})", format_value(inner)),
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock console capability for testing
pub struct MockConsole {
    output: std::cell::RefCell<Vec<String>>,
}

impl MockConsole {
    pub fn new() -> Self {
        Self {
            output: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn output(&self) -> Vec<String> {
        self.output.borrow().clone()
    }
}

impl Default for MockConsole {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleCapability for MockConsole {
    fn print(&self, text: &str) {
        self.output.borrow_mut().push(text.to_string());
    }

    fn println(&self, text: &str) {
        self.output.borrow_mut().push(format!("{}\n", text));
    }

    fn read_line(&self) -> Option<String> {
        None
    }
}

/// Seeded random capability for deterministic testing
pub struct SeededRand {
    seed: std::cell::Cell<u64>,
}

impl SeededRand {
    pub fn new(seed: u64) -> Self {
        Self {
            seed: std::cell::Cell::new(seed),
        }
    }

    fn next(&self) -> u64 {
        // Simple xorshift64
        let mut x = self.seed.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.seed.set(x);
        x
    }
}

impl RandCapability for SeededRand {
    fn int(&self, min: i64, max: i64) -> i64 {
        let range = (max - min + 1) as u64;
        let r = self.next() % range;
        min + r as i64
    }

    fn bool(&self) -> bool {
        self.next().is_multiple_of(2)
    }

    fn float(&self) -> f64 {
        (self.next() as f64) / (u64::MAX as f64)
    }
}

/// Fixed clock capability for deterministic testing
pub struct FixedClock {
    time: i64,
}

impl FixedClock {
    pub fn new(time: i64) -> Self {
        Self { time }
    }
}

impl ClockCapability for FixedClock {
    fn now(&self) -> i64 {
        self.time
    }

    fn sleep(&self, _millis: u64) {
        // No-op for fixed clock
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
  factorial(5)
}
"#;
        let result = parse_and_eval(source).unwrap();
        assert!(matches!(result, Value::Int(120)));
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
}
