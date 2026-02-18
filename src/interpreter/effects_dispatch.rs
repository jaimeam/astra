//! Effect capability dispatch for the Astra interpreter.
//!
//! Routes method calls on effect receivers (Console, Fs, Net, Clock, Rand, Env)
//! and user-defined effects to their capability implementations.

use super::error::RuntimeError;
use super::value::{format_value, Value};
use super::Interpreter;

impl Interpreter {
    /// Call a method on a receiver (for effects like Console.println)
    pub(crate) fn call_method(
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
            Value::Text(name) if name.starts_with("Clock") => {
                self.call_clock_method(method, args)
            }
            Value::Text(name) if name.starts_with("Rand") => self.call_rand_method(method, args),
            Value::Text(name) if name.starts_with("Env") => self.call_env_method(method, args),
            // Map/Set static constructors
            Value::Text(name) if name == "Map" => self.call_map_static_method(method, args),
            Value::Text(name) if name == "Set" => self.call_set_static_method(method, args),
            // For direct calls like Console.println()
            _ => {
                // Try to interpret receiver as effect name
                if let Value::Text(ref s) = receiver {
                    if s == "Console" {
                        return self.call_console_method(method, args);
                    }
                    // P6.2: User-defined effect dispatch
                    // If the receiver name matches a user-defined effect, look for a handler
                    if self.effect_defs.contains_key(s) {
                        return self.call_user_effect_method(s, method, args);
                    }
                }
                // Check if this is an Option/Result method
                self.call_value_method(receiver, method, args)
            }
        }
    }

    /// Call a Console effect method
    pub(crate) fn call_console_method(
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
    pub(crate) fn call_fs_method(
        &self,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
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
    pub(crate) fn call_net_method(
        &self,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
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
    pub(crate) fn call_clock_method(
        &self,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
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
    pub(crate) fn call_rand_method(
        &self,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
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
                let f = rand.float();
                Ok(Value::Float(f))
            }
            _ => Err(RuntimeError::unknown_method("Rand", method)),
        }
    }

    /// Call an Env effect method
    pub(crate) fn call_env_method(
        &self,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
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
                Ok(Value::List(args_vec))
            }
            _ => Err(RuntimeError::unknown_method("Env", method)),
        }
    }

    /// P6.2: Call a method on a user-defined effect.
    ///
    /// Looks up an effect handler in the environment as a record with method fields.
    /// For example, `effect Logger { fn log(msg: Text) -> Unit }` can be handled by
    /// providing a record value `{ log = fn(msg) { ... } }` bound as `__handler_Logger`.
    pub(crate) fn call_user_effect_method(
        &mut self,
        effect_name: &str,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        // Look for a handler bound in the environment
        let handler_name = format!("__handler_{}", effect_name);
        if let Some(handler) = self.env.lookup(&handler_name).cloned() {
            match handler {
                Value::Record(fields) => {
                    if let Some(func) = fields.get(method) {
                        return self.call_function(func.clone(), args);
                    }
                    Err(RuntimeError::new(
                        "E2003",
                        format!(
                            "Effect handler for '{}' does not implement operation '{}'",
                            effect_name, method
                        ),
                    ))
                }
                Value::Closure { .. } => {
                    // If the handler is a single closure, call it directly
                    self.call_function(handler, args)
                }
                _ => Err(RuntimeError::new(
                    "E2003",
                    format!(
                        "Effect handler for '{}' must be a record of functions, got {:?}",
                        effect_name, handler
                    ),
                )),
            }
        } else {
            // Validate that the operation exists in the effect definition
            if let Some(effect_def) = self.effect_defs.get(effect_name).cloned() {
                let valid_ops: Vec<&str> = effect_def
                    .operations
                    .iter()
                    .map(|o| o.name.as_str())
                    .collect();
                if !valid_ops.contains(&method) {
                    return Err(RuntimeError::unknown_method(effect_name, method));
                }
            }
            // No handler provided - return Unit (effect is unhandled)
            Ok(Value::Unit)
        }
    }
}
