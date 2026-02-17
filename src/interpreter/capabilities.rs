//! Capability traits and mock implementations for the Astra effect system.

use super::value::Value;

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
