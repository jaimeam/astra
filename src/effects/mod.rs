//! Effect system for Astra
//!
//! Tracks and enforces capability-based effects on functions.

use std::collections::HashSet;

/// Built-in effects that can be declared on functions
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Effect {
    /// Network I/O capability
    Net,
    /// Filesystem access capability
    Fs,
    /// Time/clock access capability
    Clock,
    /// Random number generation capability
    Rand,
    /// Environment variable access capability
    Env,
    /// Console I/O capability
    Console,
    /// Custom user-defined effect
    Custom(String),
}

impl Effect {
    /// Parse an effect from its string name
    pub fn from_name(name: &str) -> Option<Effect> {
        match name {
            "Net" => Some(Effect::Net),
            "Fs" => Some(Effect::Fs),
            "Clock" => Some(Effect::Clock),
            "Rand" => Some(Effect::Rand),
            "Env" => Some(Effect::Env),
            "Console" => Some(Effect::Console),
            _ => Some(Effect::Custom(name.to_string())),
        }
    }

    /// Get the name of this effect
    pub fn name(&self) -> &str {
        match self {
            Effect::Net => "Net",
            Effect::Fs => "Fs",
            Effect::Clock => "Clock",
            Effect::Rand => "Rand",
            Effect::Env => "Env",
            Effect::Console => "Console",
            Effect::Custom(name) => name,
        }
    }
}

/// A set of effects declared on a function
#[derive(Debug, Clone, Default)]
pub struct EffectSet {
    effects: HashSet<Effect>,
}

impl EffectSet {
    /// Create an empty effect set (pure function)
    pub fn new() -> Self {
        Self {
            effects: HashSet::new(),
        }
    }

    /// Create an effect set from a list of effect names
    pub fn from_names(names: &[String]) -> Self {
        let mut set = Self::new();
        for name in names {
            if let Some(effect) = Effect::from_name(name) {
                set.effects.insert(effect);
            }
        }
        set
    }

    /// Check if this set is pure (no effects)
    pub fn is_pure(&self) -> bool {
        self.effects.is_empty()
    }

    /// Check if this set contains a specific effect
    pub fn has(&self, effect: &Effect) -> bool {
        self.effects.contains(effect)
    }

    /// Add an effect to this set
    pub fn add(&mut self, effect: Effect) {
        self.effects.insert(effect);
    }

    /// Check if this set is a subset of another set
    pub fn is_subset_of(&self, other: &EffectSet) -> bool {
        self.effects.is_subset(&other.effects)
    }

    /// Get effects in this set that are not in another set
    pub fn difference<'a>(&'a self, other: &'a EffectSet) -> Vec<&'a Effect> {
        self.effects.difference(&other.effects).collect()
    }

    /// Merge another effect set into this one
    pub fn merge(&mut self, other: &EffectSet) {
        self.effects.extend(other.effects.iter().cloned());
    }

    /// Get an iterator over the effects
    pub fn iter(&self) -> impl Iterator<Item = &Effect> {
        self.effects.iter()
    }
}

/// Effect checker that validates effect declarations
pub struct EffectChecker {
    /// Stack of declared effect sets for nested contexts
    context_stack: Vec<EffectSet>,
}

impl EffectChecker {
    /// Create a new effect checker
    pub fn new() -> Self {
        Self {
            context_stack: vec![EffectSet::new()],
        }
    }

    /// Push a new effect context (e.g., entering a function)
    pub fn push_context(&mut self, effects: EffectSet) {
        self.context_stack.push(effects);
    }

    /// Pop the current effect context
    pub fn pop_context(&mut self) -> Option<EffectSet> {
        if self.context_stack.len() > 1 {
            self.context_stack.pop()
        } else {
            None
        }
    }

    /// Get the current effect context
    pub fn current_context(&self) -> &EffectSet {
        self.context_stack.last().unwrap()
    }

    /// Check if an effect is allowed in the current context
    pub fn is_allowed(&self, effect: &Effect) -> bool {
        self.current_context().has(effect)
    }

    /// Get missing effects (effects used but not declared)
    pub fn missing_effects<'a>(&'a self, used: &'a EffectSet) -> Vec<&'a Effect> {
        used.difference(self.current_context())
    }
}

impl Default for EffectChecker {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests;
