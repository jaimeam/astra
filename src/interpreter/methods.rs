//! Method dispatch for the Astra interpreter.
//!
//! Contains all method call handling: effect methods, value methods,
//! higher-order methods, and trait dispatch.

use std::collections::HashMap;

use super::error::{check_arity, RuntimeError};
use super::regex::{regex_find_all, regex_is_match, regex_match, regex_replace, regex_split};
use super::value::{compare_values, format_value, values_equal, Value};
use super::Interpreter;

impl Interpreter {
    /// Call a method on a receiver (for effects like Console.println)
    pub(super) fn call_method(
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
            Value::Text(name) if name.starts_with("Clock") => self.call_clock_method(method, args),
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
    pub(super) fn call_console_method(
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
    pub(super) fn call_fs_method(
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
    pub(super) fn call_net_method(
        &mut self,
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
            "serve" => {
                if args.len() != 2 {
                    return Err(RuntimeError::arity_mismatch(2, args.len()));
                }
                let port = match &args[0] {
                    Value::Int(p) => *p as u16,
                    _ => return Err(RuntimeError::type_mismatch("Int", "other")),
                };
                let handler = args[1].clone();

                let addr = format!("0.0.0.0:{}", port);
                let server = tiny_http::Server::http(&addr).map_err(|e| {
                    RuntimeError::new("E4020", format!("Failed to bind to {}: {}", addr, e))
                })?;

                eprintln!("Astra server listening on http://0.0.0.0:{}", port);

                for mut request in server.incoming_requests() {
                    let method_str = Value::Text(request.method().to_string().to_uppercase());
                    let full_url = request.url().to_string();

                    // Split path and query string
                    let (path_str, query_map) = if let Some(qmark) = full_url.find('?') {
                        let path = &full_url[..qmark];
                        let qs = &full_url[qmark + 1..];
                        let pairs: Vec<(Value, Value)> = qs
                            .split('&')
                            .filter(|s: &&str| !s.is_empty())
                            .map(|pair: &str| {
                                let mut parts = pair.splitn(2, '=');
                                let key = super::url_decode(parts.next().unwrap_or(""));
                                let val = super::url_decode(parts.next().unwrap_or(""));
                                (Value::Text(key), Value::Text(val))
                            })
                            .collect();
                        (path.to_string(), Value::Map(pairs))
                    } else {
                        (full_url.clone(), Value::Map(Vec::new()))
                    };

                    // Read body
                    let mut body_buf = String::new();
                    let _ = request.as_reader().read_to_string(&mut body_buf);

                    let req_record = Value::Record({
                        let mut m = std::collections::HashMap::new();
                        m.insert("method".to_string(), method_str);
                        m.insert("path".to_string(), Value::Text(path_str));
                        m.insert("body".to_string(), Value::Text(body_buf));
                        m.insert("query".to_string(), query_map);
                        m
                    });

                    let response_val = self.call_function(handler.clone(), vec![req_record]);

                    match response_val {
                        Ok(Value::Record(fields)) => {
                            let status_code = match fields.get("status") {
                                Some(Value::Int(s)) => *s as u16,
                                _ => 200u16,
                            };
                            let body = match fields.get("body") {
                                Some(Value::Text(b)) => b.clone(),
                                _ => String::new(),
                            };

                            let mut response = tiny_http::Response::from_string(body)
                                .with_status_code(tiny_http::StatusCode(status_code));

                            if let Some(Value::Map(headers)) = fields.get("headers") {
                                for (k, v) in headers {
                                    if let (Value::Text(hk), Value::Text(hv)) = (k, v) {
                                        if let Ok(header) = tiny_http::Header::from_bytes(
                                            hk.as_bytes(),
                                            hv.as_bytes(),
                                        ) {
                                            response.add_header(header);
                                        }
                                    }
                                }
                            }

                            if let Err(e) = request.respond(response) {
                                eprintln!("Failed to send response: {}", e);
                            }
                        }
                        Ok(other) => {
                            eprintln!("Handler returned non-record value: {:?}", other);
                            let response = tiny_http::Response::from_string(
                                "Internal Server Error: handler did not return a Response record",
                            )
                            .with_status_code(tiny_http::StatusCode(500));
                            if let Err(e) = request.respond(response) {
                                eprintln!("Failed to send error response: {}", e);
                            }
                        }
                        Err(e) => {
                            eprintln!("Handler error: {}", e);
                            let response = tiny_http::Response::from_string(format!(
                                "Internal Server Error: {}",
                                e
                            ))
                            .with_status_code(tiny_http::StatusCode(500));
                            if let Err(e) = request.respond(response) {
                                eprintln!("Failed to send error response: {}", e);
                            }
                        }
                    }
                }

                Ok(Value::Unit)
            }
            _ => Err(RuntimeError::unknown_method("Net", method)),
        }
    }

    /// Call a Clock effect method
    pub(super) fn call_clock_method(
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
            "today" => Ok(Value::Text(clock.today())),
            _ => Err(RuntimeError::unknown_method("Clock", method)),
        }
    }

    /// Call a Rand effect method
    pub(super) fn call_rand_method(
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
    pub(super) fn call_env_method(
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
    pub(super) fn call_user_effect_method(
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
            // This allows effect declarations to be used without requiring handlers
            // when running in contexts that don't need the effect to actually do anything.
            Ok(Value::Unit)
        }
    }

    /// Map static constructor methods (Map.new(), Map.from(...))
    fn call_map_static_method(
        &mut self,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        match method {
            "new" => Ok(Value::Map(Vec::new())),
            "from" => {
                if let Some(Value::List(pairs)) = args.into_iter().next() {
                    let mut entries = Vec::new();
                    for pair in pairs {
                        match pair {
                            Value::Tuple(ref elems) if elems.len() == 2 => {
                                entries.push((elems[0].clone(), elems[1].clone()));
                            }
                            _ => {
                                return Err(RuntimeError::type_mismatch(
                                    "List of (key, value) tuples",
                                    &format!("{:?}", pair),
                                ));
                            }
                        }
                    }
                    Ok(Value::Map(entries))
                } else {
                    Err(RuntimeError::type_mismatch("List", "other"))
                }
            }
            _ => Err(RuntimeError::unknown_method("Map", method)),
        }
    }

    /// Set static constructor methods (Set.new(), Set.from(...))
    fn call_set_static_method(
        &mut self,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        match method {
            "new" => Ok(Value::Set(Vec::new())),
            "from" => {
                if let Some(Value::List(items)) = args.into_iter().next() {
                    let mut unique = Vec::new();
                    for item in items {
                        if !unique.iter().any(|e| values_equal(e, &item)) {
                            unique.push(item);
                        }
                    }
                    Ok(Value::Set(unique))
                } else {
                    Err(RuntimeError::type_mismatch("List", "other"))
                }
            }
            _ => Err(RuntimeError::unknown_method("Set", method)),
        }
    }

    /// Call a method on a value (for Option/Result operations)
    /// Try to handle higher-order methods that need &mut self for call_function.
    /// Returns Some(result) if handled, None if not a higher-order method.
    fn try_ho_method(
        &mut self,
        receiver: &Value,
        method: &str,
        args: Vec<Value>,
    ) -> Option<Result<Value, RuntimeError>> {
        match (receiver, method) {
            (Value::List(items), "map") => {
                let items = items.clone();
                Some(self.ho_list_map(&items, args))
            }
            (Value::List(items), "filter") => {
                let items = items.clone();
                Some(self.ho_list_filter(&items, args))
            }
            (Value::List(items), "fold") => {
                let items = items.clone();
                Some(self.ho_list_fold(&items, args))
            }
            (Value::List(items), "each") => {
                let items = items.clone();
                Some(self.ho_list_each(&items, args))
            }
            (Value::List(items), "any") => {
                let items = items.clone();
                Some(self.ho_list_any(&items, args))
            }
            (Value::List(items), "all") => {
                let items = items.clone();
                Some(self.ho_list_all(&items, args))
            }
            (Value::List(items), "flat_map") => {
                let items = items.clone();
                Some(self.ho_list_flat_map(&items, args))
            }
            (Value::List(items), "find") => {
                let items = items.clone();
                Some(self.ho_list_find(&items, args))
            }
            (Value::Some(inner), "map") => {
                let inner = (**inner).clone();
                Some(self.ho_option_map(inner, args))
            }
            (Value::None, "map") => Some(Ok(Value::None)),
            (Value::Ok(inner), "map") => {
                let inner = (**inner).clone();
                Some(self.ho_result_map(inner, args))
            }
            (Value::Err(_), "map") => Some(Ok(receiver.clone())),
            (Value::Ok(_), "map_err") => Some(Ok(receiver.clone())),
            (Value::Err(inner), "map_err") => {
                let inner = (**inner).clone();
                Some(self.ho_result_map_err(inner, args))
            }
            _ => None,
        }
    }

    fn ho_list_map(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        let mut result = Vec::new();
        for item in items {
            result.push(self.call_function(func.clone(), vec![item.clone()])?);
        }
        Ok(Value::List(result))
    }

    fn ho_list_filter(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        let mut result = Vec::new();
        for item in items {
            match self.call_function(func.clone(), vec![item.clone()])? {
                Value::Bool(true) => result.push(item.clone()),
                Value::Bool(false) => {}
                other => return Err(RuntimeError::type_mismatch("Bool", &format!("{:?}", other))),
            }
        }
        Ok(Value::List(result))
    }

    fn ho_list_fold(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        check_arity(&args, 2)?;
        let mut args_iter = args.into_iter();
        let mut acc = args_iter.next().unwrap();
        let func = args_iter.next().unwrap();
        for item in items {
            acc = self.call_function(func.clone(), vec![acc, item.clone()])?;
        }
        Ok(acc)
    }

    fn ho_list_each(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        for item in items {
            self.call_function(func.clone(), vec![item.clone()])?;
        }
        Ok(Value::Unit)
    }

    fn ho_list_any(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        for item in items {
            if let Value::Bool(true) = self.call_function(func.clone(), vec![item.clone()])? {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    }

    fn ho_list_all(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        for item in items {
            if let Value::Bool(false) = self.call_function(func.clone(), vec![item.clone()])? {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    }

    fn ho_list_flat_map(
        &mut self,
        items: &[Value],
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        let mut result = Vec::new();
        for item in items {
            match self.call_function(func.clone(), vec![item.clone()])? {
                Value::List(inner) => result.extend(inner),
                other => return Err(RuntimeError::type_mismatch("List", &format!("{:?}", other))),
            }
        }
        Ok(Value::List(result))
    }

    fn ho_list_find(&mut self, items: &[Value], args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        for item in items {
            if let Value::Bool(true) = self.call_function(func.clone(), vec![item.clone()])? {
                return Ok(Value::Some(Box::new(item.clone())));
            }
        }
        Ok(Value::None)
    }

    fn ho_option_map(&mut self, inner: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        let result = self.call_function(func, vec![inner])?;
        Ok(Value::Some(Box::new(result)))
    }

    fn ho_result_map(&mut self, inner: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        let result = self.call_function(func, vec![inner])?;
        Ok(Value::Ok(Box::new(result)))
    }

    fn ho_result_map_err(&mut self, inner: Value, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = args
            .into_iter()
            .next()
            .ok_or_else(|| RuntimeError::arity_mismatch(1, 0))?;
        let result = self.call_function(func, vec![inner])?;
        Ok(Value::Err(Box::new(result)))
    }

    pub(super) fn call_value_method(
        &mut self,
        receiver: &Value,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        // Handle higher-order methods that need &mut self for call_function
        // These are extracted before the immutable match to avoid borrow conflicts
        if let Some(result) = self.try_ho_method(receiver, method, args.clone()) {
            return result;
        }

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
            (Value::None, "unwrap_or") => args
                .into_iter()
                .next()
                .ok_or_else(|| RuntimeError::arity_mismatch(1, 0)),
            (Value::Ok(inner), "unwrap_or") => Ok((**inner).clone()),
            (Value::Err(_), "unwrap_or") => args
                .into_iter()
                .next()
                .ok_or_else(|| RuntimeError::arity_mismatch(1, 0)),

            // List methods
            (Value::List(items), "len") => Ok(Value::Int(items.len() as i64)),
            (Value::List(items), "get") => {
                if let Some(Value::Int(idx)) = args.first() {
                    let idx = *idx as usize;
                    if idx < items.len() {
                        Ok(Value::Some(Box::new(items[idx].clone())))
                    } else {
                        Ok(Value::None)
                    }
                } else {
                    Err(RuntimeError::type_mismatch("Int", "other"))
                }
            }
            (Value::List(items), "contains") => {
                if let Some(needle) = args.first() {
                    Ok(Value::Bool(items.iter().any(|v| values_equal(v, needle))))
                } else {
                    Err(RuntimeError::arity_mismatch(1, 0))
                }
            }
            (Value::List(items), "is_empty") => Ok(Value::Bool(items.is_empty())),
            (Value::List(items), "head") => {
                if let Some(first) = items.first() {
                    Ok(Value::Some(Box::new(first.clone())))
                } else {
                    Ok(Value::None)
                }
            }
            (Value::List(items), "last") => {
                if let Some(last) = items.last() {
                    Ok(Value::Some(Box::new(last.clone())))
                } else {
                    Ok(Value::None)
                }
            }
            (Value::List(items), "push") => {
                if let Some(val) = args.into_iter().next() {
                    let mut new_items = items.clone();
                    new_items.push(val);
                    Ok(Value::List(new_items))
                } else {
                    Err(RuntimeError::arity_mismatch(1, 0))
                }
            }
            (Value::List(items), "set") => {
                if args.len() == 2 {
                    if let Value::Int(idx) = &args[0] {
                        let idx = *idx as usize;
                        if idx < items.len() {
                            let mut new_items = items.clone();
                            new_items[idx] = args[1].clone();
                            Ok(Value::List(new_items))
                        } else {
                            Err(RuntimeError::new(
                                "E4010",
                                format!(
                                    "Index {} out of bounds for list of length {}",
                                    idx,
                                    items.len()
                                ),
                            ))
                        }
                    } else {
                        Err(RuntimeError::type_mismatch(
                            "Int",
                            &format!("{:?}", args[0]),
                        ))
                    }
                } else {
                    Err(RuntimeError::arity_mismatch(2, args.len()))
                }
            }
            (Value::List(items), "concat") => {
                if let Some(Value::List(other)) = args.first() {
                    let mut new_items = items.clone();
                    new_items.extend(other.clone());
                    Ok(Value::List(new_items))
                } else {
                    Err(RuntimeError::type_mismatch("List", "other"))
                }
            }
            // P3.1: tail, reverse, sort
            (Value::List(items), "tail") => {
                if items.is_empty() {
                    Ok(Value::List(vec![]))
                } else {
                    Ok(Value::List(items[1..].to_vec()))
                }
            }
            (Value::List(items), "reverse") => {
                let mut rev = items.clone();
                rev.reverse();
                Ok(Value::List(rev))
            }
            (Value::List(items), "sort") => {
                let mut sorted = items.clone();
                sorted.sort_by(compare_values);
                Ok(Value::List(sorted))
            }
            // P3.2: take, drop, slice, enumerate, zip
            (Value::List(items), "take") => {
                if let Some(Value::Int(n)) = args.first() {
                    let n = (*n).max(0) as usize;
                    Ok(Value::List(items.iter().take(n).cloned().collect()))
                } else {
                    Err(RuntimeError::type_mismatch("Int", "other"))
                }
            }
            (Value::List(items), "drop") => {
                if let Some(Value::Int(n)) = args.first() {
                    let n = (*n).max(0) as usize;
                    Ok(Value::List(items.iter().skip(n).cloned().collect()))
                } else {
                    Err(RuntimeError::type_mismatch("Int", "other"))
                }
            }
            (Value::List(items), "slice") => {
                if args.len() == 2 {
                    if let (Some(Value::Int(start)), Some(Value::Int(end))) =
                        (args.first(), args.get(1))
                    {
                        let start = (*start).max(0) as usize;
                        let end = (*end).max(0) as usize;
                        let end = end.min(items.len());
                        if start <= end {
                            Ok(Value::List(items[start..end].to_vec()))
                        } else {
                            Ok(Value::List(vec![]))
                        }
                    } else {
                        Err(RuntimeError::type_mismatch("(Int, Int)", "other"))
                    }
                } else {
                    Err(RuntimeError::arity_mismatch(2, args.len()))
                }
            }
            (Value::List(items), "enumerate") => {
                let pairs: Vec<Value> = items
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let mut fields = HashMap::new();
                        fields.insert("index".to_string(), Value::Int(i as i64));
                        fields.insert("value".to_string(), v.clone());
                        Value::Record(fields)
                    })
                    .collect();
                Ok(Value::List(pairs))
            }
            (Value::List(items), "zip") => {
                if let Some(Value::List(other)) = args.first() {
                    let pairs: Vec<Value> = items
                        .iter()
                        .zip(other.iter())
                        .map(|(a, b)| {
                            let mut fields = HashMap::new();
                            fields.insert("first".to_string(), a.clone());
                            fields.insert("second".to_string(), b.clone());
                            Value::Record(fields)
                        })
                        .collect();
                    Ok(Value::List(pairs))
                } else {
                    Err(RuntimeError::type_mismatch("List", "other"))
                }
            }

            // Text methods
            (Value::Text(s), "len") => Ok(Value::Int(s.len() as i64)),
            (Value::Text(s), "to_upper") => Ok(Value::Text(s.to_uppercase())),
            (Value::Text(s), "to_lower") => Ok(Value::Text(s.to_lowercase())),
            (Value::Text(s), "trim") => Ok(Value::Text(s.trim().to_string())),
            (Value::Text(s), "contains") => {
                if let Some(Value::Text(needle)) = args.first() {
                    Ok(Value::Bool(s.contains(needle.as_str())))
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "starts_with") => {
                if let Some(Value::Text(prefix)) = args.first() {
                    Ok(Value::Bool(s.starts_with(prefix.as_str())))
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "ends_with") => {
                if let Some(Value::Text(suffix)) = args.first() {
                    Ok(Value::Bool(s.ends_with(suffix.as_str())))
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "split") => {
                if let Some(Value::Text(delimiter)) = args.first() {
                    let parts: Vec<Value> = s
                        .split(delimiter.as_str())
                        .map(|p| Value::Text(p.to_string()))
                        .collect();
                    Ok(Value::List(parts))
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "replace") => {
                if args.len() == 2 {
                    if let (Some(Value::Text(from)), Some(Value::Text(to))) =
                        (args.first(), args.get(1))
                    {
                        Ok(Value::Text(s.replace(from.as_str(), to.as_str())))
                    } else {
                        Err(RuntimeError::type_mismatch("(Text, Text)", "other"))
                    }
                } else {
                    Err(RuntimeError::arity_mismatch(2, args.len()))
                }
            }
            (Value::Text(s), "chars") => {
                let chars: Vec<Value> = s.chars().map(|c| Value::Text(c.to_string())).collect();
                Ok(Value::List(chars))
            }
            // P3.3: join, repeat, index_of, substring
            (Value::Text(s), "repeat") => {
                if let Some(Value::Int(n)) = args.first() {
                    Ok(Value::Text(s.repeat((*n).max(0) as usize)))
                } else {
                    Err(RuntimeError::type_mismatch("Int", "other"))
                }
            }
            (Value::Text(s), "index_of") => {
                if let Some(Value::Text(needle)) = args.first() {
                    match s.find(needle.as_str()) {
                        Some(pos) => Ok(Value::Some(Box::new(Value::Int(pos as i64)))),
                        None => Ok(Value::None),
                    }
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "slice") | (Value::Text(s), "substring") => {
                if args.len() == 2 {
                    if let (Some(Value::Int(start)), Some(Value::Int(end))) =
                        (args.first(), args.get(1))
                    {
                        let start = (*start).max(0) as usize;
                        let end = (*end).max(0) as usize;
                        let end = end.min(s.len());
                        if start <= end && start <= s.len() {
                            Ok(Value::Text(s[start..end].to_string()))
                        } else {
                            Ok(Value::Text(String::new()))
                        }
                    } else {
                        Err(RuntimeError::type_mismatch("(Int, Int)", "other"))
                    }
                } else {
                    Err(RuntimeError::arity_mismatch(2, args.len()))
                }
            }
            // v1.1: Regex text methods
            (Value::Text(s), "matches") => {
                if let Some(Value::Text(pattern)) = args.first() {
                    regex_is_match(pattern, s)
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "find_pattern") => {
                if let Some(Value::Text(pattern)) = args.first() {
                    regex_match(pattern, s)
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "find_all_pattern") => {
                if let Some(Value::Text(pattern)) = args.first() {
                    regex_find_all(pattern, s)
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }
            (Value::Text(s), "replace_pattern") => {
                if args.len() == 2 {
                    if let (Some(Value::Text(pattern)), Some(Value::Text(replacement))) =
                        (args.first(), args.get(1))
                    {
                        regex_replace(pattern, s, replacement)
                    } else {
                        Err(RuntimeError::type_mismatch("(Text, Text)", "other"))
                    }
                } else {
                    Err(RuntimeError::arity_mismatch(2, args.len()))
                }
            }
            (Value::Text(s), "split_pattern") => {
                if let Some(Value::Text(pattern)) = args.first() {
                    regex_split(pattern, s)
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }

            // List join method (on List[Text])
            (Value::List(items), "join") => {
                if let Some(Value::Text(sep)) = args.first() {
                    let strs: Vec<String> = items.iter().map(format_value).collect();
                    Ok(Value::Text(strs.join(sep.as_str())))
                } else {
                    Err(RuntimeError::type_mismatch("Text", "other"))
                }
            }

            // Tuple methods
            (Value::Tuple(elements), "len") => Ok(Value::Int(elements.len() as i64)),
            (Value::Tuple(elements), "to_list") => Ok(Value::List(elements.clone())),

            // Map instance methods
            (Value::Map(entries), "len") => Ok(Value::Int(entries.len() as i64)),
            (Value::Map(entries), "is_empty") => Ok(Value::Bool(entries.is_empty())),
            (Value::Map(entries), "get") => {
                if let Some(key) = args.first() {
                    for (k, v) in entries {
                        if values_equal(k, key) {
                            return Ok(Value::Some(Box::new(v.clone())));
                        }
                    }
                    Ok(Value::None)
                } else {
                    Err(RuntimeError::arity_mismatch(1, 0))
                }
            }
            (Value::Map(entries), "contains_key") => {
                if let Some(key) = args.first() {
                    Ok(Value::Bool(
                        entries.iter().any(|(k, _)| values_equal(k, key)),
                    ))
                } else {
                    Err(RuntimeError::arity_mismatch(1, 0))
                }
            }
            (Value::Map(entries), "keys") => {
                let keys: Vec<Value> = entries.iter().map(|(k, _)| k.clone()).collect();
                Ok(Value::List(keys))
            }
            (Value::Map(entries), "values") => {
                let vals: Vec<Value> = entries.iter().map(|(_, v)| v.clone()).collect();
                Ok(Value::List(vals))
            }
            (Value::Map(entries), "entries") => {
                let pairs: Vec<Value> = entries
                    .iter()
                    .map(|(k, v)| Value::Tuple(vec![k.clone(), v.clone()]))
                    .collect();
                Ok(Value::List(pairs))
            }
            (Value::Map(entries), "set") => {
                if args.len() == 2 {
                    let key = args[0].clone();
                    let val = args[1].clone();
                    let mut new_entries: Vec<(Value, Value)> = entries
                        .iter()
                        .filter(|(k, _)| !values_equal(k, &key))
                        .cloned()
                        .collect();
                    new_entries.push((key, val));
                    Ok(Value::Map(new_entries))
                } else {
                    Err(RuntimeError::arity_mismatch(2, args.len()))
                }
            }
            (Value::Map(entries), "remove") => {
                if let Some(key) = args.first() {
                    let new_entries: Vec<(Value, Value)> = entries
                        .iter()
                        .filter(|(k, _)| !values_equal(k, key))
                        .cloned()
                        .collect();
                    Ok(Value::Map(new_entries))
                } else {
                    Err(RuntimeError::arity_mismatch(1, 0))
                }
            }

            // Set instance methods
            (Value::Set(elements), "len") => Ok(Value::Int(elements.len() as i64)),
            (Value::Set(elements), "is_empty") => Ok(Value::Bool(elements.is_empty())),
            (Value::Set(elements), "contains") => {
                if let Some(val) = args.first() {
                    Ok(Value::Bool(elements.iter().any(|e| values_equal(e, val))))
                } else {
                    Err(RuntimeError::arity_mismatch(1, 0))
                }
            }
            (Value::Set(elements), "add") => {
                if let Some(val) = args.into_iter().next() {
                    let mut new_elements = elements.clone();
                    if !new_elements.iter().any(|e| values_equal(e, &val)) {
                        new_elements.push(val);
                    }
                    Ok(Value::Set(new_elements))
                } else {
                    Err(RuntimeError::arity_mismatch(1, 0))
                }
            }
            (Value::Set(elements), "remove") => {
                if let Some(val) = args.first() {
                    let new_elements: Vec<Value> = elements
                        .iter()
                        .filter(|e| !values_equal(e, val))
                        .cloned()
                        .collect();
                    Ok(Value::Set(new_elements))
                } else {
                    Err(RuntimeError::arity_mismatch(1, 0))
                }
            }
            (Value::Set(elements), "to_list") => Ok(Value::List(elements.clone())),
            (Value::Set(elements), "union") => {
                if let Some(Value::Set(other)) = args.first() {
                    let mut result = elements.clone();
                    for item in other {
                        if !result.iter().any(|e| values_equal(e, item)) {
                            result.push(item.clone());
                        }
                    }
                    Ok(Value::Set(result))
                } else {
                    Err(RuntimeError::type_mismatch("Set", "other"))
                }
            }
            (Value::Set(elements), "intersection") => {
                if let Some(Value::Set(other)) = args.first() {
                    let result: Vec<Value> = elements
                        .iter()
                        .filter(|e| other.iter().any(|o| values_equal(e, o)))
                        .cloned()
                        .collect();
                    Ok(Value::Set(result))
                } else {
                    Err(RuntimeError::type_mismatch("Set", "other"))
                }
            }

            _ => {
                // Try trait method dispatch before failing
                if let Some(result) = self.try_trait_dispatch(receiver, method, args) {
                    return result;
                }
                Err(RuntimeError::unknown_method(
                    &format!("{:?}", receiver),
                    method,
                ))
            }
        }
    }

    /// Get the runtime type name of a value (for trait dispatch)
    fn value_type_name(value: &Value) -> &'static str {
        match value {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Bool(_) => "Bool",
            Value::Text(_) => "Text",
            Value::Unit => "Unit",
            Value::List(_) => "List",
            Value::Tuple(_) => "Tuple",
            Value::Map(_) => "Map",
            Value::Set(_) => "Set",
            Value::Some(_) | Value::None => "Option",
            Value::Ok(_) | Value::Err(_) => "Result",
            Value::Record(_) => "Record",
            Value::Variant { .. } | Value::VariantConstructor { .. } => "Variant",
            Value::Closure { .. } => "Closure",
            Value::Future { .. } => "Future",
        }
    }

    /// Try to dispatch a method call through registered trait implementations.
    /// Returns Some(Result) if a matching impl was found, None otherwise.
    fn try_trait_dispatch(
        &mut self,
        receiver: &Value,
        method: &str,
        args: Vec<Value>,
    ) -> Option<Result<Value, RuntimeError>> {
        let type_name = Self::value_type_name(receiver);

        // Also try variant name for enum types
        let variant_type = if let Value::Variant { name, .. } = receiver {
            Some(name.as_str())
        } else {
            None
        };

        // Search trait impls for a matching method
        for imp in &self.trait_impls.clone() {
            let matches = imp.target_type_name == type_name
                || variant_type.is_some_and(|v| imp.target_type_name == v);

            if matches {
                if let Some(closure) = imp.methods.get(method) {
                    let closure = closure.clone();
                    // Call the method with `self` as first argument
                    let mut call_args = vec![receiver.clone()];
                    call_args.extend(args);
                    return Some(self.call_function(closure, call_args));
                }
            }
        }

        None
    }
}
