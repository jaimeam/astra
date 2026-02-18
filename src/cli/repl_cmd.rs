//! Handler for the `astra repl` subcommand.

use std::path::PathBuf;

use crate::interpreter::{format_value, Capabilities, Interpreter, Value};
use crate::parser::{Lexer, Parser as AstraParser, SourceFile};

use super::configure_search_paths;
use super::run_cmd::RealConsole;

pub(crate) fn run_repl() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{self, Write};

    println!("Astra REPL v1.0.0");
    println!("Type expressions to evaluate. Use :quit to exit, :help for help.");
    println!();

    // Keep track of definitions (accumulated source)
    let mut definitions = String::from("module repl\n");
    let mut def_count = 0u32;

    let make_interpreter = || {
        let mut interp = Interpreter::with_capabilities(Capabilities {
            console: Some(Box::new(RealConsole)),
            ..Default::default()
        });
        configure_search_paths(&mut interp, None);
        interp
    };

    // P3: Helper to describe value type for REPL display
    fn value_type_name(v: &Value) -> &'static str {
        match v {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Bool(_) => "Bool",
            Value::Text(_) => "Text",
            Value::List(_) => "List",
            Value::Tuple(_) => "Tuple",
            Value::Record(_) => "Record",
            Value::Map(_) => "Map",
            Value::Closure { .. } => "Function",
            Value::Unit => "Unit",
            Value::Some(_) | Value::None => "Option",
            Value::Ok(_) | Value::Err(_) => "Result",
            Value::Variant { .. } => "Enum",
            _ => "Value",
        }
    }

    loop {
        print!("astra> ");
        io::stdout().flush()?;

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // Handle REPL commands
        match input {
            ":quit" | ":q" | ":exit" => break,
            ":help" | ":h" => {
                println!("Commands:");
                println!("  :quit, :q    Exit the REPL");
                println!("  :help, :h    Show this help");
                println!("  :clear       Clear all definitions");
                println!();
                println!("Enter expressions, let bindings, fn/type/enum definitions, or import statements.");
                continue;
            }
            ":clear" => {
                definitions = String::from("module repl\n");
                def_count = 0;
                println!("Cleared.");
                continue;
            }
            _ => {}
        }

        // P3: Handle import statements directly as definitions
        if input.starts_with("import ") {
            let def_source = format!("{}\n{}", definitions, input);
            let source_file = SourceFile::new(PathBuf::from("repl.astra"), def_source.clone());
            let lexer = Lexer::new(&source_file);
            let mut parser = AstraParser::new(lexer, source_file.clone());

            match parser.parse_module() {
                Ok(_) => {
                    definitions = def_source;
                    def_count += 1;
                    println!("Imported.");
                }
                Err(e) => {
                    eprintln!("{}", e.format_text(&def_source));
                }
            }
            continue;
        }

        // Try to evaluate as expression first: wrap in a temp main function
        let expr_source = format!("{}\nfn __repl__() -> Unit {{ {} }}", definitions, input);

        let source_file = SourceFile::new(PathBuf::from("repl.astra"), expr_source.clone());
        let lexer = Lexer::new(&source_file);
        let mut parser = AstraParser::new(lexer, source_file.clone());

        match parser.parse_module() {
            Ok(module) => {
                let mut interp = make_interpreter();
                if let Err(e) = interp.load_module(&module) {
                    eprintln!("Error: {}", e);
                    continue;
                }
                if let Some(func) = interp.env.lookup("__repl__").cloned() {
                    match interp.call_function(func, vec![]) {
                        Ok(value) => {
                            if !matches!(value, Value::Unit) {
                                // P3: Show type alongside value
                                println!("{} : {}", format_value(&value), value_type_name(&value));
                            }
                        }
                        Err(e) if e.is_return => {
                            if let Some(val) = e.early_return {
                                println!("{} : {}", format_value(&val), value_type_name(&val));
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                        }
                    }
                }
            }
            Err(_) => {
                // Try as a top-level definition (fn, type, enum, let)
                let def_source = format!("{}\n{}", definitions, input);
                let source_file2 = SourceFile::new(PathBuf::from("repl.astra"), def_source.clone());
                let lexer2 = Lexer::new(&source_file2);
                let mut parser2 = AstraParser::new(lexer2, source_file2.clone());

                match parser2.parse_module() {
                    Ok(_) => {
                        definitions = def_source;
                        def_count += 1;
                        println!("Defined. ({} definitions)", def_count);
                    }
                    Err(e) => {
                        eprintln!("{}", e.format_text(&def_source));
                    }
                }
            }
        }
    }

    println!("Bye!");
    Ok(())
}
