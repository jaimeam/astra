//! Execution environment for the Astra interpreter.

use std::collections::HashMap;

use super::value::Value;

/// Execution environment using a scope stack for O(1) scope creation.
#[derive(Debug, Clone, Default)]
pub struct Environment {
    /// Stack of variable binding scopes (top = innermost scope)
    scopes: Vec<HashMap<String, Value>>,
}

impl Environment {
    /// Create a new environment with one empty scope
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    /// Push a new empty scope onto the stack (O(1))
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the top scope off the stack (O(1))
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Define a variable in the top scope
    pub fn define(&mut self, name: String, value: Value) {
        if let Some(top) = self.scopes.last_mut() {
            top.insert(name, value);
        }
    }

    /// Look up a variable, searching from top scope to bottom
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val);
            }
        }
        None
    }

    /// Update a mutable variable, searching from top scope to bottom
    pub fn update(&mut self, name: &str, value: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return true;
            }
        }
        false
    }

    /// Check if the environment has no user-defined bindings
    /// (used to detect empty closure environments for top-level functions)
    pub fn is_empty(&self) -> bool {
        self.scopes.iter().all(|s| s.is_empty())
    }

    /// Get the number of scopes
    pub fn scope_depth(&self) -> usize {
        self.scopes.len()
    }
}
