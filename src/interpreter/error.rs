//! Runtime error types for the Astra interpreter.

use std::fmt;

use super::value::Value;

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
    /// B5: Source location where the error occurred
    pub span: Option<crate::diagnostics::Span>,
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
            span: None,
        }
    }

    /// Create a runtime error with source location
    pub fn with_span(mut self, span: crate::diagnostics::Span) -> Self {
        self.span = Some(span);
        self
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
            span: None,
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
            span: None,
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
            span: None,
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
            span: None,
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
        if let Some(span) = &self.span {
            write!(
                f,
                "[{}] {}\n  --> {}:{}:{}",
                self.code,
                self.message,
                span.file.display(),
                span.start_line,
                span.start_col
            )
        } else {
            write!(f, "[{}] {}", self.code, self.message)
        }
    }
}

impl std::error::Error for RuntimeError {}

/// Check that `args` has exactly `expected` elements, returning an arity error if not.
pub fn check_arity<T>(args: &[T], expected: usize) -> Result<(), RuntimeError> {
    if args.len() != expected {
        Err(RuntimeError::arity_mismatch(expected, args.len()))
    } else {
        Ok(())
    }
}
