//! Module loading and resolution for the Astra interpreter.

use std::path::{Path, PathBuf};

use crate::parser::ast::*;

use super::environment::Environment;
use super::error::RuntimeError;
use super::value::{ClosureBody, Value};
use super::Interpreter;

impl Interpreter {
    /// Resolve a module path to a file path
    pub(super) fn resolve_module_path(&self, segments: &[String]) -> Option<PathBuf> {
        let relative = segments.join("/") + ".astra";

        // P4.4: Map `std.*` imports to `stdlib/*` directory
        let stdlib_relative =
            if segments.first().map(|s| s.as_str()) == Some("std") && segments.len() > 1 {
                let mut stdlib_segments = vec!["stdlib".to_string()];
                stdlib_segments.extend(segments[1..].iter().cloned());
                Some(stdlib_segments.join("/") + ".astra")
            } else {
                None
            };

        for search_path in &self.search_paths {
            let candidate = search_path.join(&relative);
            if candidate.exists() {
                return Some(candidate);
            }
            // Also try the stdlib mapping
            if let Some(ref stdlib_rel) = stdlib_relative {
                let candidate = search_path.join(stdlib_rel);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
        // Also check relative to cwd
        let candidate = PathBuf::from(&relative);
        if candidate.exists() {
            return Some(candidate);
        }
        if let Some(ref stdlib_rel) = stdlib_relative {
            let candidate = PathBuf::from(stdlib_rel);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        None
    }

    /// Parse a module file from disk into an AST Module.
    /// Shared helper used by both `load_import` and `load_import_filtered`.
    fn parse_module_file(&self, file_path: &Path) -> Result<Module, RuntimeError> {
        let source = std::fs::read_to_string(file_path).map_err(|e| {
            RuntimeError::new("E4016", format!("Failed to read module file: {}", e))
        })?;

        let source_file =
            crate::parser::span::SourceFile::new(file_path.to_path_buf(), source.clone());
        let lexer = crate::parser::lexer::Lexer::new(&source_file);
        let mut parser = crate::parser::parser::Parser::new(lexer, source_file.clone());
        parser.parse_module().map_err(|_| {
            RuntimeError::new(
                "E4017",
                format!("Failed to parse module: {}", file_path.display()),
            )
        })
    }

    /// Load an imported module by path segments
    pub(super) fn load_import(&mut self, segments: &[String]) -> Result<(), RuntimeError> {
        let module_key = segments.join(".");
        if self.loaded_modules.contains(&module_key) {
            return Ok(()); // Already loaded
        }
        // P4.2: Circular import detection
        if self.loading_modules.contains(&module_key) {
            return Err(RuntimeError::new(
                "E4018",
                format!("Circular import detected: {}", module_key),
            ));
        }
        self.loading_modules.insert(module_key.clone());

        let file_path = match self.resolve_module_path(segments) {
            Some(p) => p,
            None => return Ok(()), // Silently skip unresolvable imports for now
        };

        let imported_module = self.parse_module_file(&file_path)?;

        self.load_module(&imported_module)?;
        self.loading_modules.remove(&module_key);
        self.loaded_modules.insert(module_key);
        Ok(())
    }

    pub fn load_module(&mut self, module: &Module) -> Result<(), RuntimeError> {
        self.load_module_with_filter(module, None)
    }

    /// Load a module, optionally filtering which names to import
    pub(super) fn load_module_with_filter(
        &mut self,
        module: &Module,
        filter: Option<&[String]>,
    ) -> Result<(), RuntimeError> {
        // Process imports first (P4.1: named import resolution)
        for item in &module.items {
            if let Item::Import(import) = item {
                match &import.kind {
                    ImportKind::Module => {
                        self.load_import(&import.path.segments)?;
                    }
                    ImportKind::Items(names) => {
                        self.load_import_filtered(&import.path.segments, names)?;
                    }
                    ImportKind::Alias(_alias) => {
                        // For now, load the full module (alias support later)
                        self.load_import(&import.path.segments)?;
                    }
                }
            }
        }

        // Collect all function and enum definitions into the environment
        for item in &module.items {
            match item {
                Item::FnDef(fn_def) => {
                    // Apply filter if specified
                    if let Some(names) = filter {
                        if !names.contains(&fn_def.name) {
                            continue;
                        }
                    }
                    let params: Vec<String> =
                        fn_def.params.iter().map(|p| p.name.clone()).collect();

                    // Build body block, prepending destructuring for pattern params
                    let mut body_block = fn_def.body.clone();
                    let mut destructure_stmts = Vec::new();
                    for param in &fn_def.params {
                        if let Some(ref pattern) = param.pattern {
                            destructure_stmts.push(Stmt::LetPattern {
                                id: NodeId::new(),
                                span: param.span.clone(),
                                pattern: pattern.clone(),
                                ty: None,
                                value: Box::new(Expr::Ident {
                                    id: NodeId::new(),
                                    span: param.span.clone(),
                                    name: param.name.clone(),
                                }),
                            });
                        }
                    }
                    if !destructure_stmts.is_empty() {
                        destructure_stmts.extend(body_block.stmts);
                        body_block.stmts = destructure_stmts;
                    }

                    let closure = Value::Closure {
                        name: Some(fn_def.name.clone()),
                        params,
                        body: ClosureBody {
                            block: body_block,
                            requires: fn_def.requires.clone(),
                            ensures: fn_def.ensures.clone(),
                        },
                        env: Environment::new(), // Will use global env at call time
                    };
                    // v1.1: Track async functions
                    if fn_def.is_async {
                        self.async_fns.insert(fn_def.name.clone());
                    }
                    self.env.define(fn_def.name.clone(), closure);
                }
                Item::EnumDef(enum_def) => {
                    // Register variant constructors and nullary variant values
                    for variant in &enum_def.variants {
                        if let Some(names) = filter {
                            if !names.contains(&variant.name) {
                                continue;
                            }
                        }
                        if variant.fields.is_empty() {
                            // Nullary variant: register as a value (e.g., `Red`, `None`)
                            self.env.define(
                                variant.name.clone(),
                                Value::Variant {
                                    name: variant.name.clone(),
                                    data: None,
                                },
                            );
                        } else {
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
                Item::ImplBlock(impl_block) => {
                    // Extract the target type name from the TypeExpr
                    let target_type_name = match &impl_block.target_type {
                        TypeExpr::Named { name, .. } => name.clone(),
                        _ => "Unknown".to_string(),
                    };

                    let mut methods_map = std::collections::HashMap::new();
                    for method in &impl_block.methods {
                        let params: Vec<String> =
                            method.params.iter().map(|p| p.name.clone()).collect();
                        let closure = Value::Closure {
                            name: Some(method.name.clone()),
                            params,
                            body: ClosureBody {
                                block: method.body.clone(),
                                requires: method.requires.clone(),
                                ensures: method.ensures.clone(),
                            },
                            env: Environment::new(),
                        };
                        // Register with qualified name for backward compat
                        let qualified_name = format!("{}_{}", impl_block.trait_name, method.name);
                        self.env.define(qualified_name, closure.clone());
                        methods_map.insert(method.name.clone(), closure);
                    }

                    // Register in the trait impl registry for dispatch
                    self.trait_impls.push(super::RuntimeTraitImpl {
                        target_type_name,
                        methods: methods_map,
                    });
                }
                Item::TypeDef(type_def) => {
                    // P2.3: Store type definitions for invariant enforcement
                    self.type_defs
                        .insert(type_def.name.clone(), type_def.clone());
                }
                Item::EffectDef(effect_def) => {
                    // P6.2: Store user-defined effect declarations
                    self.effect_defs
                        .insert(effect_def.name.clone(), effect_def.clone());
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// P2.3: Check type invariant for a value against a named type
    pub(super) fn check_type_invariant(
        &mut self,
        type_name: &str,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        if let Some(type_def) = self.type_defs.get(type_name).cloned() {
            if let Some(invariant) = &type_def.invariant {
                let invariant = invariant.clone();
                // Evaluate invariant with `self` bound to the value
                self.env.push_scope();
                self.env.define("self".to_string(), value.clone());
                let result = self.eval_expr(&invariant);
                self.env.pop_scope();

                match result {
                    Ok(Value::Bool(true)) => Ok(()),
                    Ok(Value::Bool(false)) => Err(RuntimeError::new(
                        "E3003",
                        format!(
                            "Type invariant violated for {}: {} does not satisfy invariant",
                            type_name,
                            super::value::format_value(value)
                        ),
                    )),
                    Ok(_) => Err(RuntimeError::new(
                        "E3003",
                        "Type invariant must evaluate to Bool",
                    )),
                    Err(e) => Err(e),
                }
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    /// Load an imported module, only bringing specific names into scope
    pub(super) fn load_import_filtered(
        &mut self,
        segments: &[String],
        names: &[String],
    ) -> Result<(), RuntimeError> {
        let module_key = segments.join(".");
        if self.loaded_modules.contains(&module_key) {
            // Module already executed — just import the requested names from cached env
            if let Some(module_env) = self.loaded_module_envs.get(&module_key).cloned() {
                for name in names {
                    if let Some(value) = module_env.lookup(name).cloned() {
                        let imported_value = rebind_closure_env(value, &module_env);
                        self.env.define(name.clone(), imported_value);
                    }
                }
            }
            return Ok(());
        }
        self.loaded_modules.insert(module_key.clone());

        let file_path = match self.resolve_module_path(segments) {
            Some(p) => p,
            None => return Ok(()),
        };

        let imported_module = self.parse_module_file(&file_path)?;

        // Load ALL module definitions into a temporary child environment so that
        // internal dependencies between module functions are preserved.
        let saved_env = std::mem::replace(&mut self.env, Environment::new());
        self.load_module(&imported_module)?;
        let module_env = self.env.clone();
        self.env = saved_env;

        // Import only the requested names, capturing the module env in closures
        // so they can access module-internal functions at call time.
        for name in names {
            if let Some(value) = module_env.lookup(name).cloned() {
                let imported_value = rebind_closure_env(value, &module_env);
                self.env.define(name.clone(), imported_value);
            }
        }
        // Cache the module env so subsequent filtered imports can pull names from it
        self.loaded_module_envs.insert(module_key, module_env);
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
                self.env.push_scope();
                let result = self.eval_block(&body.block);
                self.env.pop_scope();
                // Handle early returns from ? operator
                return match result {
                    Err(e) if e.is_early_return() => Ok(e.get_early_return().unwrap()),
                    other => other,
                };
            }
        }

        Ok(Value::Unit)
    }
}

/// Re-bind a closure's environment to a given module environment.
/// Non-closure values are passed through unchanged.
fn rebind_closure_env(value: Value, env: &Environment) -> Value {
    match value {
        Value::Closure {
            name: fn_name,
            params,
            body,
            ..
        } => Value::Closure {
            name: fn_name,
            params,
            body,
            env: env.clone(),
        },
        other => other,
    }
}
