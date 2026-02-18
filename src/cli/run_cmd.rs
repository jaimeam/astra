//! Handler for the `astra run` subcommand.

use std::path::PathBuf;

use crate::interpreter::{
    Capabilities, ClockCapability, ConsoleCapability, EnvCapability, FsCapability, Interpreter,
    NetCapability, RandCapability, Value,
};
use crate::parser::{Lexer, Parser as AstraParser, SourceFile};

use super::configure_search_paths;

pub(crate) fn run_program(
    file: &PathBuf,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    // Read the source file
    let source = std::fs::read_to_string(file)
        .map_err(|e| format!("Failed to read file {:?}: {}", file, e))?;

    // Parse the source
    let source_file = SourceFile::new(file.clone(), source.clone());
    let lexer = Lexer::new(&source_file);
    let mut parser = AstraParser::new(lexer, source_file.clone());
    let module = parser
        .parse_module()
        .map_err(|e| format!("Parse error:\n{}", e.format_text(&source)))?;

    // Set up capabilities -- provide all real capabilities for `astra run`
    let capabilities = Capabilities {
        console: Some(Box::new(RealConsole)),
        env: Some(Box::new(RealEnv::new(args.to_vec()))),
        fs: Some(Box::new(RealFs)),
        net: Some(Box::new(RealNet)),
        clock: Some(Box::new(RealClock)),
        rand: Some(Box::new(RealRand::new())),
    };

    // Create interpreter and run
    let mut interpreter = Interpreter::with_capabilities(capabilities);
    configure_search_paths(&mut interpreter, file.parent());
    match interpreter.eval_module(&module) {
        Ok(_) => Ok(()),
        Err(e) => {
            let trace = interpreter.format_stack_trace();
            let mut msg = format!("Runtime error: {}", e);
            if !trace.is_empty() {
                msg.push_str(&format!("\n{}", trace));
            }
            Err(msg.into())
        }
    }
}

/// Real console capability that prints to stdout
pub(super) struct RealConsole;

impl ConsoleCapability for RealConsole {
    fn print(&self, text: &str) {
        print!("{}", text);
    }

    fn println(&self, text: &str) {
        println!("{}", text);
    }

    fn read_line(&self) -> Option<String> {
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) => None, // EOF
            Ok(_) => Some(line.trim_end().to_string()),
            Err(_) => None,
        }
    }
}

/// Real environment capability
struct RealEnv {
    args: Vec<String>,
}

impl RealEnv {
    fn new(args: Vec<String>) -> Self {
        Self { args }
    }
}

impl EnvCapability for RealEnv {
    fn get(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn args(&self) -> Vec<String> {
        self.args.clone()
    }
}

/// Real filesystem capability that performs actual I/O
struct RealFs;

impl FsCapability for RealFs {
    fn read(&self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read '{}': {}", path, e))
    }

    fn write(&self, path: &str, content: &str) -> Result<(), String> {
        std::fs::write(path, content).map_err(|e| format!("Failed to write '{}': {}", path, e))
    }

    fn exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
}

/// Real network capability using ureq for HTTP
struct RealNet;

impl NetCapability for RealNet {
    fn get(&self, url: &str) -> Result<Value, String> {
        match ureq::get(url).call() {
            Ok(response) => {
                let body = response
                    .into_string()
                    .map_err(|e| format!("Failed to read response body: {}", e))?;
                Ok(Value::Text(body))
            }
            Err(e) => Err(format!("HTTP GET failed: {}", e)),
        }
    }

    fn post(&self, url: &str, body: &str) -> Result<Value, String> {
        match ureq::post(url).send_string(body) {
            Ok(response) => {
                let body = response
                    .into_string()
                    .map_err(|e| format!("Failed to read response body: {}", e))?;
                Ok(Value::Text(body))
            }
            Err(e) => Err(format!("HTTP POST failed: {}", e)),
        }
    }
}

/// Real clock capability using system time
struct RealClock;

impl ClockCapability for RealClock {
    fn now(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    fn sleep(&self, millis: u64) {
        std::thread::sleep(std::time::Duration::from_millis(millis));
    }
}

/// Real random capability using system randomness
struct RealRand {
    seed: std::cell::Cell<u64>,
}

impl RealRand {
    fn new() -> Self {
        // Seed from system time for non-deterministic randomness
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self {
            seed: std::cell::Cell::new(seed),
        }
    }

    fn next(&self) -> u64 {
        let mut x = self.seed.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.seed.set(x);
        x
    }
}

impl RandCapability for RealRand {
    fn int(&self, min: i64, max: i64) -> i64 {
        if max <= min {
            return min;
        }
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
