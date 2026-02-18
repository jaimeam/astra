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

    /// Create a child environment (clones self as parent)
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

    /// Propagate mutations from a child scope back to this (parent) scope.
    /// After a block executes in a child env, any updates to variables that
    /// exist in the parent need to be copied back since the child was working
    /// on a cloned copy of the parent.
    pub fn propagate_from_child(&mut self, child: &Environment) {
        if let Some(child_parent) = &child.parent {
            // The child's parent is a snapshot of `self` from before the block.
            // Any differences in bindings represent mutations that occurred inside the block.
            for (name, value) in &child_parent.bindings {
                if self.bindings.contains_key(name) {
                    self.bindings.insert(name.clone(), value.clone());
                }
            }
            // Recursively propagate to grandparent
            if let Some(grandparent) = &child_parent.parent {
                if let Some(our_parent) = &mut self.parent {
                    our_parent.propagate_from_child_parent(grandparent);
                }
            }
        }
    }

    /// Helper: propagate from a parent snapshot (not a child with parent).
    fn propagate_from_child_parent(&mut self, snapshot: &Environment) {
        for (name, value) in &snapshot.bindings {
            if self.bindings.contains_key(name) {
                self.bindings.insert(name.clone(), value.clone());
            }
        }
        if let Some(snapshot_parent) = &snapshot.parent {
            if let Some(our_parent) = &mut self.parent {
                our_parent.propagate_from_child_parent(snapshot_parent);
            }
        }
    }
}
