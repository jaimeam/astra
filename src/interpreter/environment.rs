//! Execution environment for the Astra interpreter.

use std::collections::HashMap;

use super::value::Value;

/// Execution environment
#[derive(Debug, Clone, Default)]
pub struct Environment {
    /// Variable bindings
    pub(crate) bindings: HashMap<String, Value>,
    /// Parent environment for lexical scoping
    pub(crate) parent: Option<Box<Environment>>,
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
