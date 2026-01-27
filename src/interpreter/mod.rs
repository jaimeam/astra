//! Interpreter for Astra programs
//!
//! Executes Astra code with capability-controlled effects.

use std::collections::HashMap;
use std::fmt;

use crate::parser::ast::*;

/// Runtime error with error code and message
#[derive(Debug, Clone)]
pub struct RuntimeError {
    /// Error code (E4xxx series for runtime errors)
    pub code: &'static str,
    /// Human-readable error message
    pub message: String,
}

impl RuntimeError {
    /// Create a new runtime error
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Undefined variable error
    pub fn undefined_variable(name: &str) -> Self {
        Self::new("E4001", format!("undefined variable: {}", name))
    }

    /// Type mismatch error
    pub fn type_mismatch(expected: &str, got: &str) -> Self {
        Self::new("E4002", format!("type mismatch: expected {}, got {}", expected, got))
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
        Self::new("E4010", format!("expected {} arguments, got {}", expected, got))
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
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for RuntimeError {}

/// Runtime value
#[derive(Debug, Clone)]
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
}

/// Closure body containing the AST block
#[derive(Debug, Clone)]
pub struct ClosureBody {
    /// The block to execute when called
    pub block: Block,
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
            n1 == n2 && match (d1, d2) {
                (None, None) => true,
                (Some(a), Some(b)) => values_equal(a, b),
                _ => false,
            }
        }
        (Value::Record(r1), Value::Record(r2)) => {
            r1.len() == r2.len() && r1.iter().all(|(k, v)| {
                r2.get(k).map_or(false, |v2| values_equal(v, v2))
            })
        }
        // Closures are never equal
        (Value::Closure { .. }, Value::Closure { .. }) => false,
        _ => false,
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
pub struct Capabilities {
    pub net: Option<Box<dyn NetCapability>>,
    pub fs: Option<Box<dyn FsCapability>>,
    pub clock: Option<Box<dyn ClockCapability>>,
    pub rand: Option<Box<dyn RandCapability>>,
    pub console: Option<Box<dyn ConsoleCapability>>,
    pub env: Option<Box<dyn EnvCapability>>,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            net: None,
            fs: None,
            clock: None,
            rand: None,
            console: None,
            env: None,
        }
    }
}

/// Interpreter for Astra programs
pub struct Interpreter {
    /// Global environment
    pub env: Environment,
    /// Available capabilities
    pub capabilities: Capabilities,
}

impl Interpreter {
    /// Create a new interpreter
    pub fn new() -> Self {
        Self {
            env: Environment::new(),
            capabilities: Capabilities::default(),
        }
    }

    /// Create an interpreter with specific capabilities
    pub fn with_capabilities(capabilities: Capabilities) -> Self {
        Self {
            env: Environment::new(),
            capabilities,
        }
    }

    /// Load a module's definitions without running main
    pub fn load_module(&mut self, module: &Module) -> Result<(), RuntimeError> {
        // Collect all function definitions into the environment
        for item in &module.items {
            if let Item::FnDef(fn_def) = item {
                let params: Vec<String> = fn_def.params.iter().map(|p| p.name.clone()).collect();
                let closure = Value::Closure {
                    params,
                    body: ClosureBody {
                        block: fn_def.body.clone(),
                    },
                    env: Environment::new(), // Will use global env at call time
                };
                self.env.define(fn_def.name.clone(), closure);
            }
        }
        Ok(())
    }

    /// Evaluate a module
    pub fn eval_module(&mut self, module: &Module) -> Result<Value, RuntimeError> {
        // Load the module definitions
        self.load_module(module)?;

        // Look for and run main function if it exists
        if let Some(main_fn) = self.env.lookup("main").cloned() {
            if let Value::Closure { params, body, .. } = main_fn {
                if params.is_empty() {
                    // Execute in a child of the global environment
                    let call_env = self.env.child();
                    let old_env = std::mem::replace(&mut self.env, call_env);
                    let result = self.eval_block(&body.block);
                    self.env = old_env;
                    return result;
                }
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
                    _ => self.env
                        .lookup(name)
                        .cloned()
                        .ok_or_else(|| RuntimeError::undefined_variable(name))
                }
            }
            Expr::QualifiedIdent { module, name, .. } => {
                // For now, treat as effect call target
                Ok(Value::Text(format!("{}.{}", module, name)))
            }

            // Binary operations
            Expr::Binary { op, left, right, .. } => {
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
                                Value::Bool(false) => Err(RuntimeError::new(
                                    "E4020",
                                    "assertion failed",
                                )),
                                _ => Err(RuntimeError::type_mismatch("Bool", &format!("{:?}", cond))),
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
            Expr::MethodCall { receiver, method, args, .. } => {
                let recv = self.eval_expr(receiver)?;
                let mut arg_vals = Vec::new();
                for arg in args {
                    arg_vals.push(self.eval_expr(arg)?);
                }
                self.call_method(&recv, method, arg_vals)
            }

            // If expression
            Expr::If { cond, then_branch, else_branch, .. } => {
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
                    _ => Err(RuntimeError::type_mismatch("Bool", &format!("{:?}", cond_val))),
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
                    Value::None => Err(RuntimeError::unwrap_none()),
                    Value::Ok(inner) => Ok(*inner),
                    Value::Err(inner) => {
                        let msg = match *inner {
                            Value::Text(s) => s,
                            _ => "error".to_string(),
                        };
                        Err(RuntimeError::unwrap_err(&msg))
                    }
                    other => Ok(other), // Pass through non-Option/Result values
                }
            }

            // TryElse expression (unwrap or use default)
            Expr::TryElse { expr, else_expr, .. } => {
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
                    Value::Record(fields) => {
                        fields
                            .get(field)
                            .cloned()
                            .ok_or_else(|| RuntimeError::invalid_field_access(field))
                    }
                    _ => Err(RuntimeError::type_mismatch("Record", &format!("{:?}", val))),
                }
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
                // Return is handled by early exit in eval_block
                // For now, evaluate the value if present
                if let Some(val_expr) = value {
                    let _val = self.eval_expr(val_expr)?;
                }
                Ok(())
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
            // Handle return statements
            if let Stmt::Return { value, .. } = stmt {
                let result = if let Some(val_expr) = value {
                    self.eval_expr(val_expr)?
                } else {
                    Value::Unit
                };
                self.env = old_env;
                return Ok(result);
            }
            self.eval_stmt(stmt)?;
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
    fn eval_binary_op(&self, op: BinaryOp, left: &Value, right: &Value) -> Result<Value, RuntimeError> {
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
            (BinaryOp::Add, Value::Text(a), Value::Text(b)) => Ok(Value::Text(format!("{}{}", a, b))),

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
            Value::Closure { params, body, env } => {
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

                // Execute the body
                let old_env = std::mem::replace(&mut self.env, call_env);
                let result = self.eval_block(&body.block);
                self.env = old_env;
                result
            }
            _ => Err(RuntimeError::not_callable()),
        }
    }

    /// Call a method on a receiver (for effects like Console.println)
    fn call_method(&mut self, receiver: &Value, method: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        // Check if receiver is an effect identifier
        match receiver {
            Value::Text(name) if name == "Console.println" || name.starts_with("Console") => {
                self.call_console_method(method, args)
            }
            Value::Text(name) if name.starts_with("Fs") => {
                self.call_fs_method(method, args)
            }
            Value::Text(name) if name.starts_with("Net") => {
                self.call_net_method(method, args)
            }
            Value::Text(name) if name.starts_with("Clock") => {
                self.call_clock_method(method, args)
            }
            Value::Text(name) if name.starts_with("Rand") => {
                self.call_rand_method(method, args)
            }
            Value::Text(name) if name.starts_with("Env") => {
                self.call_env_method(method, args)
            }
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
    fn call_console_method(&mut self, method: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let console = self.capabilities.console.as_ref()
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
        let fs = self.capabilities.fs.as_ref()
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
                    if let (Some(Value::Text(path)), Some(Value::Text(content))) = (args.get(0), args.get(1)) {
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
        let net = self.capabilities.net.as_ref()
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
                    if let (Some(Value::Text(url)), Some(Value::Text(body))) = (args.get(0), args.get(1)) {
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
        let clock = self.capabilities.clock.as_ref()
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
        let rand = self.capabilities.rand.as_ref()
            .ok_or_else(|| RuntimeError::capability_not_available("Rand"))?;

        match method {
            "int" => {
                if args.len() == 2 {
                    if let (Some(Value::Int(min)), Some(Value::Int(max))) = (args.get(0), args.get(1)) {
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
        let env_cap = self.capabilities.env.as_ref()
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
    fn call_value_method(&self, receiver: &Value, method: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
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
            (Value::None, "unwrap_or") => {
                args.into_iter().next().ok_or_else(|| RuntimeError::arity_mismatch(1, 0))
            }
            (Value::Ok(inner), "unwrap_or") => Ok((**inner).clone()),
            (Value::Err(_), "unwrap_or") => {
                args.into_iter().next().ok_or_else(|| RuntimeError::arity_mismatch(1, 0))
            }

            _ => Err(RuntimeError::unknown_method(&format!("{:?}", receiver), method)),
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
        Pattern::Variant { name, data, .. } => {
            match (name.as_str(), value) {
                // Option::Some
                ("Some", Value::Some(inner)) => {
                    if let Some(data_pat) = data {
                        match_pattern(data_pat, inner)
                    } else {
                        Some(vec![])
                    }
                }
                // Option::None
                ("None", Value::None) => {
                    if data.is_none() {
                        Some(vec![])
                    } else {
                        None
                    }
                }
                // Result::Ok
                ("Ok", Value::Ok(inner)) => {
                    if let Some(data_pat) = data {
                        match_pattern(data_pat, inner)
                    } else {
                        Some(vec![])
                    }
                }
                // Result::Err
                ("Err", Value::Err(inner)) => {
                    if let Some(data_pat) = data {
                        match_pattern(data_pat, inner)
                    } else {
                        Some(vec![])
                    }
                }
                // Generic variant
                (pat_name, Value::Variant { name: var_name, data: var_data }) => {
                    if pat_name == var_name {
                        match (data, var_data) {
                            (None, None) => Some(vec![]),
                            (Some(data_pat), Some(var_val)) => match_pattern(data_pat, var_val),
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
        self.next() % 2 == 0
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
}
