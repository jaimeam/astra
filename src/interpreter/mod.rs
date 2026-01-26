//! Interpreter for Astra programs
//!
//! Executes Astra code with capability-controlled effects.

use std::collections::HashMap;

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

/// Closure body - placeholder for now
#[derive(Debug, Clone)]
pub struct ClosureBody {
    // Will hold AST reference
    _placeholder: (),
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

    // Placeholder for evaluation methods
    // These will be implemented when the AST and type system are complete
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
