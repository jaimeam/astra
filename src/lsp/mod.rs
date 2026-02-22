//! Language Server Protocol implementation for Astra.
//!
//! Provides IDE integration via the LSP protocol over stdio:
//! - Diagnostics (errors/warnings from type checker)
//! - Go-to-definition for functions and types
//! - Hover information (type info)
//! - Document symbols

use std::collections::HashMap;
use std::io::{self, BufRead, Read as IoRead, Write as IoWrite};

use serde_json::{json, Value};

use crate::diagnostics::{Severity, Span};
use crate::parser::ast::*;
use crate::parser::lexer::Lexer;
use crate::parser::parser::Parser;
use crate::parser::span::SourceFile;
use crate::typechecker::TypeChecker;

/// Run the LSP server on stdin/stdout
pub fn run_server() -> io::Result<()> {
    let mut server = LspServer::new();
    server.main_loop()
}

/// The LSP server state
struct LspServer {
    /// Open documents: URI -> source text
    documents: HashMap<String, String>,
    /// Parsed modules: URI -> Module
    modules: HashMap<String, Module>,
    /// Cached diagnostics per URI (for code actions)
    cached_diagnostics: HashMap<String, Vec<crate::diagnostics::Diagnostic>>,
    /// Whether the server has been initialized
    initialized: bool,
}

impl LspServer {
    fn new() -> Self {
        Self {
            documents: HashMap::new(),
            modules: HashMap::new(),
            cached_diagnostics: HashMap::new(),
            initialized: false,
        }
    }

    /// Main message loop: read JSON-RPC messages from stdin, dispatch, write responses to stdout
    fn main_loop(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut reader = stdin.lock();

        while let Ok(content_length) = read_content_length(&mut reader) {
            // Read the JSON body
            let mut body = vec![0u8; content_length];
            reader.read_exact(&mut body)?;

            let msg: Value = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Dispatch the message
            if let Some(response) = self.handle_message(&msg) {
                send_message(&response)?;
            }
        }

        Ok(())
    }

    /// Handle a single JSON-RPC message
    fn handle_message(&mut self, msg: &Value) -> Option<Value> {
        let method = msg.get("method")?.as_str()?;
        let id = msg.get("id");
        let params = msg.get("params").cloned().unwrap_or(json!({}));

        match method {
            "initialize" => {
                self.initialized = true;
                id.map(|id| {
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "capabilities": {
                                "textDocumentSync": {
                                    "openClose": true,
                                    "change": 1, // Full sync
                                    "save": { "includeText": true }
                                },
                                "hoverProvider": true,
                                "definitionProvider": true,
                                "documentSymbolProvider": true,
                                "completionProvider": {
                                    "triggerCharacters": [".", ":"]
                                },
                                "codeActionProvider": {
                                    "codeActionKinds": ["quickfix"]
                                }
                            },
                            "serverInfo": {
                                "name": "astra-lsp",
                                "version": "0.2.0"
                            }
                        }
                    })
                })
            }

            "initialized" => {
                // Client acknowledges initialization
                None
            }

            "shutdown" => id.map(|id| {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": null
                })
            }),

            "exit" => {
                std::process::exit(0);
            }

            "textDocument/didOpen" => {
                let uri = params["textDocument"]["uri"].as_str()?.to_string();
                let text = params["textDocument"]["text"].as_str()?.to_string();
                self.documents.insert(uri.clone(), text);
                self.publish_diagnostics(&uri);
                None
            }

            "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"].as_str()?.to_string();
                if let Some(changes) = params["contentChanges"].as_array() {
                    if let Some(change) = changes.first() {
                        if let Some(text) = change["text"].as_str() {
                            self.documents.insert(uri.clone(), text.to_string());
                            self.publish_diagnostics(&uri);
                        }
                    }
                }
                None
            }

            "textDocument/didSave" => {
                let uri = params["textDocument"]["uri"].as_str()?.to_string();
                if let Some(text) = params.get("text").and_then(|t| t.as_str()) {
                    self.documents.insert(uri.clone(), text.to_string());
                }
                self.publish_diagnostics(&uri);
                None
            }

            "textDocument/didClose" => {
                let uri = params["textDocument"]["uri"].as_str()?.to_string();
                self.documents.remove(&uri);
                self.modules.remove(&uri);
                self.cached_diagnostics.remove(&uri);
                // Clear diagnostics
                let notification = json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/publishDiagnostics",
                    "params": {
                        "uri": uri,
                        "diagnostics": []
                    }
                });
                let _ = send_message(&notification);
                None
            }

            "textDocument/hover" => {
                let result = self.handle_hover(&params);
                id.map(|id| {
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    })
                })
            }

            "textDocument/definition" => {
                let result = self.handle_definition(&params);
                id.map(|id| {
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    })
                })
            }

            "textDocument/documentSymbol" => {
                let result = self.handle_document_symbols(&params);
                id.map(|id| {
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    })
                })
            }

            "textDocument/completion" => {
                let result = self.handle_completion(&params);
                id.map(|id| {
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    })
                })
            }

            "textDocument/codeAction" => {
                let result = self.handle_code_action(&params);
                id.map(|id| {
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    })
                })
            }

            _ => {
                // Unknown method - return error for requests, ignore notifications
                id.map(|id| {
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32601,
                            "message": format!("Method not found: {}", method)
                        }
                    })
                })
            }
        }
    }

    /// Parse a document and run type checking, then publish diagnostics
    fn publish_diagnostics(&mut self, uri: &str) {
        let source = match self.documents.get(uri) {
            Some(s) => s.clone(),
            None => return,
        };

        let file_path = uri_to_path(uri);
        let source_file = SourceFile::new(file_path.into(), source.clone());
        let lexer = Lexer::new(&source_file);
        let mut parser = Parser::new(lexer, source_file.clone());

        let module = match parser.parse_module() {
            Ok(m) => m,
            Err(bag) => {
                // Cache parse error diagnostics for code actions
                self.cached_diagnostics
                    .insert(uri.to_string(), bag.diagnostics().to_vec());
                // Report parse errors
                let lsp_diags: Vec<Value> =
                    bag.diagnostics().iter().map(diagnostic_to_lsp).collect();
                let notification = json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/publishDiagnostics",
                    "params": {
                        "uri": uri,
                        "diagnostics": lsp_diags
                    }
                });
                let _ = send_message(&notification);
                return;
            }
        };

        // Store parsed module for other features
        self.modules.insert(uri.to_string(), module.clone());

        // Run type checker with search paths for cross-file resolution
        let mut checker = TypeChecker::new();
        // Add workspace root as search path
        if let Ok(cwd) = std::env::current_dir() {
            checker.add_search_path(cwd);
        }
        let type_diags = match checker.check_module(&module) {
            Ok(()) => checker.diagnostics().clone(),
            Err(bag) => bag,
        };

        // Cache diagnostics for code actions
        self.cached_diagnostics
            .insert(uri.to_string(), type_diags.diagnostics().to_vec());

        let lsp_diags: Vec<Value> = type_diags
            .diagnostics()
            .iter()
            .map(diagnostic_to_lsp)
            .collect();

        let notification = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": lsp_diags
            }
        });

        let _ = send_message(&notification);
    }

    /// Handle textDocument/hover
    fn handle_hover(&self, params: &Value) -> Value {
        let uri = match params["textDocument"]["uri"].as_str() {
            Some(u) => u,
            None => return Value::Null,
        };
        let line = params["position"]["line"].as_u64().unwrap_or(0) as usize;
        let col = params["position"]["character"].as_u64().unwrap_or(0) as usize;

        let module = match self.modules.get(uri) {
            Some(m) => m,
            None => return Value::Null,
        };

        // Find the item at the cursor position
        for item in &module.items {
            match item {
                Item::FnDef(def) => {
                    if span_contains(&def.span, line, col) {
                        let params_str: Vec<String> = def
                            .params
                            .iter()
                            .map(|p| format!("{}: {}", p.name, format_type_expr(&p.ty)))
                            .collect();
                        let ret_str = def
                            .return_type
                            .as_ref()
                            .map(|t| format!(" -> {}", format_type_expr(t)))
                            .unwrap_or_default();
                        let effects_str = if def.effects.is_empty() {
                            String::new()
                        } else {
                            format!(" effects({})", def.effects.join(", "))
                        };

                        let type_params_str = if def.type_params.is_empty() {
                            String::new()
                        } else {
                            format!("[{}]", def.type_params.join(", "))
                        };

                        let hover_text = format!(
                            "```astra\nfn {}{}({}){}{}\n```",
                            def.name,
                            type_params_str,
                            params_str.join(", "),
                            ret_str,
                            effects_str,
                        );
                        return json!({
                            "contents": {
                                "kind": "markdown",
                                "value": hover_text
                            }
                        });
                    }
                }
                Item::TypeDef(def) => {
                    if span_contains(&def.span, line, col) {
                        let hover_text = format!(
                            "```astra\ntype {} = {}\n```",
                            def.name,
                            format_type_expr(&def.value)
                        );
                        return json!({
                            "contents": {
                                "kind": "markdown",
                                "value": hover_text
                            }
                        });
                    }
                }
                Item::EnumDef(def) => {
                    if span_contains(&def.span, line, col) {
                        let variants: Vec<String> = def
                            .variants
                            .iter()
                            .map(|v| {
                                if v.fields.is_empty() {
                                    v.name.clone()
                                } else {
                                    let fields: Vec<String> = v
                                        .fields
                                        .iter()
                                        .map(|f| format!("{}: {}", f.name, format_type_expr(&f.ty)))
                                        .collect();
                                    format!("{}({})", v.name, fields.join(", "))
                                }
                            })
                            .collect();
                        let hover_text = format!(
                            "```astra\nenum {} {{\n  {}\n}}\n```",
                            def.name,
                            variants.join("\n  ")
                        );
                        return json!({
                            "contents": {
                                "kind": "markdown",
                                "value": hover_text
                            }
                        });
                    }
                }
                Item::TraitDef(def) => {
                    if span_contains(&def.span, line, col) {
                        let methods: Vec<String> = def
                            .methods
                            .iter()
                            .map(|m| {
                                let ps: Vec<String> = m
                                    .params
                                    .iter()
                                    .map(|p| format!("{}: {}", p.name, format_type_expr(&p.ty)))
                                    .collect();
                                let ret = m
                                    .return_type
                                    .as_ref()
                                    .map(|t| format!(" -> {}", format_type_expr(t)))
                                    .unwrap_or_default();
                                format!("fn {}({}){}", m.name, ps.join(", "), ret)
                            })
                            .collect();
                        let hover_text = format!(
                            "```astra\ntrait {} {{\n  {}\n}}\n```",
                            def.name,
                            methods.join("\n  ")
                        );
                        return json!({
                            "contents": {
                                "kind": "markdown",
                                "value": hover_text
                            }
                        });
                    }
                }
                _ => {}
            }
        }

        // P7: Cross-file hover for imported symbols
        let source = match self.documents.get(uri) {
            Some(s) => s,
            None => return Value::Null,
        };
        let ident = find_ident_at_position(source, line, col);
        if !ident.is_empty() {
            for item in &module.items {
                if let Item::Import(import) = item {
                    let is_imported = match &import.kind {
                        ImportKind::Items(names) => names.contains(&ident),
                        ImportKind::Module | ImportKind::Alias(_) => true,
                    };
                    if is_imported {
                        if let Some(target_path) = resolve_import_path(&import.path, uri) {
                            if let Ok(target_source) = std::fs::read_to_string(&target_path) {
                                let sf = SourceFile::new(target_path.clone(), target_source);
                                let lexer = Lexer::new(&sf);
                                let mut parser = Parser::new(lexer, sf.clone());
                                if let Ok(target_module) = parser.parse_module() {
                                    for target_item in &target_module.items {
                                        if let Item::FnDef(def) = target_item {
                                            if def.name == ident {
                                                let params_str: Vec<String> = def
                                                    .params
                                                    .iter()
                                                    .map(|p| {
                                                        format!(
                                                            "{}: {}",
                                                            p.name,
                                                            format_type_expr(&p.ty)
                                                        )
                                                    })
                                                    .collect();
                                                let ret_str = def
                                                    .return_type
                                                    .as_ref()
                                                    .map(|t| format!(" -> {}", format_type_expr(t)))
                                                    .unwrap_or_default();
                                                let effects_str = if def.effects.is_empty() {
                                                    String::new()
                                                } else {
                                                    format!(" effects({})", def.effects.join(", "))
                                                };
                                                let hover_text = format!(
                                                    "```astra\nfn {}({}){}{}\n```\n*from {}*",
                                                    def.name,
                                                    params_str.join(", "),
                                                    ret_str,
                                                    effects_str,
                                                    import.path.segments.join("."),
                                                );
                                                return json!({
                                                    "contents": {
                                                        "kind": "markdown",
                                                        "value": hover_text
                                                    }
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Value::Null
    }

    /// Handle textDocument/definition
    fn handle_definition(&self, params: &Value) -> Value {
        let uri = match params["textDocument"]["uri"].as_str() {
            Some(u) => u,
            None => return Value::Null,
        };
        let line = params["position"]["line"].as_u64().unwrap_or(0) as usize;
        let col = params["position"]["character"].as_u64().unwrap_or(0) as usize;

        let source = match self.documents.get(uri) {
            Some(s) => s,
            None => return Value::Null,
        };

        // Find the identifier at the cursor position
        let ident = find_ident_at_position(source, line, col);
        if ident.is_empty() {
            return Value::Null;
        }

        let module = match self.modules.get(uri) {
            Some(m) => m,
            None => return Value::Null,
        };

        // Search for definition in current module
        for item in &module.items {
            match item {
                Item::FnDef(def) if def.name == ident => {
                    return json!({
                        "uri": uri,
                        "range": span_to_range(&def.span)
                    });
                }
                Item::TypeDef(def) if def.name == ident => {
                    return json!({
                        "uri": uri,
                        "range": span_to_range(&def.span)
                    });
                }
                Item::EnumDef(def) if def.name == ident => {
                    return json!({
                        "uri": uri,
                        "range": span_to_range(&def.span)
                    });
                }
                Item::TraitDef(def) if def.name == ident => {
                    return json!({
                        "uri": uri,
                        "range": span_to_range(&def.span)
                    });
                }
                _ => {}
            }
        }

        // P7: Cross-file go-to-definition for imported symbols
        for item in &module.items {
            if let Item::Import(import) = item {
                let is_imported = match &import.kind {
                    ImportKind::Items(names) => names.contains(&ident),
                    ImportKind::Module | ImportKind::Alias(_) => true,
                };
                if is_imported {
                    // Resolve the module file
                    if let Some(target_path) = resolve_import_path(&import.path, uri) {
                        if let Ok(target_source) = std::fs::read_to_string(&target_path) {
                            let sf = SourceFile::new(target_path.clone(), target_source);
                            let lexer = Lexer::new(&sf);
                            let mut parser = Parser::new(lexer, sf.clone());
                            if let Ok(target_module) = parser.parse_module() {
                                for target_item in &target_module.items {
                                    match target_item {
                                        Item::FnDef(def) if def.name == ident => {
                                            let target_uri = path_to_uri(&target_path);
                                            return json!({
                                                "uri": target_uri,
                                                "range": span_to_range(&def.span)
                                            });
                                        }
                                        Item::TypeDef(def) if def.name == ident => {
                                            let target_uri = path_to_uri(&target_path);
                                            return json!({
                                                "uri": target_uri,
                                                "range": span_to_range(&def.span)
                                            });
                                        }
                                        Item::EnumDef(def) if def.name == ident => {
                                            let target_uri = path_to_uri(&target_path);
                                            return json!({
                                                "uri": target_uri,
                                                "range": span_to_range(&def.span)
                                            });
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Value::Null
    }

    /// Handle textDocument/documentSymbol
    fn handle_document_symbols(&self, params: &Value) -> Value {
        let uri = match params["textDocument"]["uri"].as_str() {
            Some(u) => u,
            None => return json!([]),
        };

        let module = match self.modules.get(uri) {
            Some(m) => m,
            None => return json!([]),
        };

        let mut symbols = Vec::new();

        for item in &module.items {
            match item {
                Item::FnDef(def) => {
                    symbols.push(json!({
                        "name": def.name,
                        "kind": 12, // Function
                        "range": span_to_range(&def.span),
                        "selectionRange": span_to_range(&def.span)
                    }));
                }
                Item::TypeDef(def) => {
                    symbols.push(json!({
                        "name": def.name,
                        "kind": 5, // Class (used for types)
                        "range": span_to_range(&def.span),
                        "selectionRange": span_to_range(&def.span)
                    }));
                }
                Item::EnumDef(def) => {
                    let children: Vec<Value> = def
                        .variants
                        .iter()
                        .map(|v| {
                            json!({
                                "name": v.name,
                                "kind": 22, // EnumMember
                                "range": span_to_range(&v.span),
                                "selectionRange": span_to_range(&v.span)
                            })
                        })
                        .collect();
                    symbols.push(json!({
                        "name": def.name,
                        "kind": 10, // Enum
                        "range": span_to_range(&def.span),
                        "selectionRange": span_to_range(&def.span),
                        "children": children
                    }));
                }
                Item::TraitDef(def) => {
                    let children: Vec<Value> = def
                        .methods
                        .iter()
                        .map(|m| {
                            json!({
                                "name": m.name,
                                "kind": 6, // Method
                                "range": span_to_range(&m.span),
                                "selectionRange": span_to_range(&m.span)
                            })
                        })
                        .collect();
                    symbols.push(json!({
                        "name": def.name,
                        "kind": 11, // Interface
                        "range": span_to_range(&def.span),
                        "selectionRange": span_to_range(&def.span),
                        "children": children
                    }));
                }
                Item::EffectDef(def) => {
                    symbols.push(json!({
                        "name": def.name,
                        "kind": 11, // Interface
                        "range": span_to_range(&def.span),
                        "selectionRange": span_to_range(&def.span)
                    }));
                }
                _ => {}
            }
        }

        json!(symbols)
    }

    /// Handle textDocument/completion
    fn handle_completion(&self, params: &Value) -> Value {
        let uri = match params["textDocument"]["uri"].as_str() {
            Some(u) => u,
            None => return json!([]),
        };

        let module = match self.modules.get(uri) {
            Some(m) => m,
            None => return json!([]),
        };

        let mut items = Vec::new();

        // Suggest functions, types, and enums from the current module
        for item in &module.items {
            match item {
                Item::FnDef(def) => {
                    let params_str: Vec<String> = def
                        .params
                        .iter()
                        .map(|p| format!("{}: {}", p.name, format_type_expr(&p.ty)))
                        .collect();
                    items.push(json!({
                        "label": def.name,
                        "kind": 3, // Function
                        "detail": format!("fn({})", params_str.join(", ")),
                        "insertText": def.name
                    }));
                }
                Item::TypeDef(def) => {
                    items.push(json!({
                        "label": def.name,
                        "kind": 22, // Struct
                        "detail": format!("type {}", def.name),
                        "insertText": def.name
                    }));
                }
                Item::EnumDef(def) => {
                    items.push(json!({
                        "label": def.name,
                        "kind": 13, // Enum
                        "detail": format!("enum {}", def.name),
                        "insertText": def.name
                    }));
                    for variant in &def.variants {
                        items.push(json!({
                            "label": variant.name,
                            "kind": 20, // EnumMember
                            "detail": format!("{}::{}", def.name, variant.name),
                            "insertText": variant.name
                        }));
                    }
                }
                _ => {}
            }
        }

        // Add keywords
        for keyword in &[
            "fn", "let", "mut", "if", "else", "match", "for", "while", "return", "break",
            "continue", "import", "from", "type", "enum", "trait", "impl", "effect", "test",
            "property", "true", "false", "Some", "None", "Ok", "Err",
        ] {
            items.push(json!({
                "label": keyword,
                "kind": 14, // Keyword
                "insertText": keyword
            }));
        }

        json!(items)
    }

    /// Handle textDocument/codeAction — convert diagnostic suggestions to quick fixes
    fn handle_code_action(&self, params: &Value) -> Value {
        let uri = match params["textDocument"]["uri"].as_str() {
            Some(u) => u,
            None => return json!([]),
        };

        let request_range = &params["range"];
        let req_start_line = request_range["start"]["line"].as_u64().unwrap_or(0) as usize;
        let req_end_line = request_range["end"]["line"].as_u64().unwrap_or(0) as usize;

        let diagnostics = match self.cached_diagnostics.get(uri) {
            Some(d) => d,
            None => return json!([]),
        };

        let mut actions = Vec::new();

        for diag in diagnostics {
            // Check if the diagnostic overlaps with the requested range
            let diag_start_line = diag.span.start_line.saturating_sub(1);
            let diag_end_line = diag.span.end_line.saturating_sub(1);
            if diag_end_line < req_start_line || diag_start_line > req_end_line {
                continue;
            }

            // Create a code action for each suggestion
            for suggestion in &diag.suggestions {
                if suggestion.edits.is_empty() {
                    continue;
                }

                let mut text_edits = Vec::new();
                for edit in &suggestion.edits {
                    text_edits.push(json!({
                        "range": span_to_range(&edit.span),
                        "newText": edit.replacement
                    }));
                }

                actions.push(json!({
                    "title": suggestion.title,
                    "kind": "quickfix",
                    "diagnostics": [diagnostic_to_lsp(diag)],
                    "edit": {
                        "changes": {
                            uri: text_edits
                        }
                    }
                }));
            }
        }

        json!(actions)
    }
}

/// Read the Content-Length header from the input stream
fn read_content_length(reader: &mut impl BufRead) -> io::Result<usize> {
    let mut header = String::new();
    loop {
        header.clear();
        let bytes_read = reader.read_line(&mut header)?;
        if bytes_read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF"));
        }
        let header = header.trim();
        if header.is_empty() {
            continue;
        }
        if let Some(len_str) = header.strip_prefix("Content-Length: ") {
            let len: usize = len_str.parse().map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("Invalid length: {}", e))
            })?;
            // Read the empty line after headers
            let mut empty = String::new();
            reader.read_line(&mut empty)?;
            return Ok(len);
        }
    }
}

/// Send a JSON-RPC message to stdout
fn send_message(msg: &Value) -> io::Result<()> {
    let body = serde_json::to_string(msg)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    write!(out, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    out.flush()
}

/// Convert a URI to a file path
fn uri_to_path(uri: &str) -> String {
    if let Some(path) = uri.strip_prefix("file://") {
        path.to_string()
    } else {
        uri.to_string()
    }
}

/// Convert an Astra diagnostic to LSP diagnostic JSON
fn diagnostic_to_lsp(diag: &crate::diagnostics::Diagnostic) -> Value {
    let severity = match diag.severity {
        Severity::Error => 1,
        Severity::Warning => 2,
        Severity::Info => 3,
        Severity::Hint => 4,
    };

    json!({
        "range": span_to_range(&diag.span),
        "severity": severity,
        "code": diag.code,
        "source": "astra",
        "message": diag.message,
    })
}

/// Convert an Astra span to an LSP range
fn span_to_range(span: &Span) -> Value {
    json!({
        "start": {
            "line": span.start_line.saturating_sub(1),
            "character": span.start_col.saturating_sub(1)
        },
        "end": {
            "line": span.end_line.saturating_sub(1),
            "character": span.end_col.saturating_sub(1)
        }
    })
}

/// Check if a span contains the given 0-indexed line and column
fn span_contains(span: &Span, line: usize, col: usize) -> bool {
    let line1 = line + 1; // Spans are 1-indexed
    let col1 = col + 1;
    if line1 < span.start_line || line1 > span.end_line {
        return false;
    }
    if line1 == span.start_line && col1 < span.start_col {
        return false;
    }
    if line1 == span.end_line && col1 > span.end_col {
        return false;
    }
    true
}

/// Find the identifier at a given position in the source text
fn find_ident_at_position(source: &str, line: usize, col: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    if line >= lines.len() {
        return String::new();
    }
    let line_text = lines[line];
    let chars: Vec<char> = line_text.chars().collect();
    if col >= chars.len() {
        return String::new();
    }

    // Expand from cursor position to find identifier boundaries
    let is_ident_char = |c: char| c.is_alphanumeric() || c == '_';

    if !is_ident_char(chars[col]) {
        return String::new();
    }

    let mut start = col;
    while start > 0 && is_ident_char(chars[start - 1]) {
        start -= 1;
    }

    let mut end = col;
    while end < chars.len() && is_ident_char(chars[end]) {
        end += 1;
    }

    chars[start..end].iter().collect()
}

/// Format a TypeExpr as a string for display
fn format_type_expr(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named { name, args, .. } => {
            if args.is_empty() {
                name.clone()
            } else {
                let args_str: Vec<String> = args.iter().map(format_type_expr).collect();
                format!("{}[{}]", name, args_str.join(", "))
            }
        }
        TypeExpr::Record { fields, .. } => {
            let fields_str: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", f.name, format_type_expr(&f.ty)))
                .collect();
            format!("{{ {} }}", fields_str.join(", "))
        }
        TypeExpr::Function {
            params,
            ret,
            effects,
            ..
        } => {
            let params_str: Vec<String> = params.iter().map(format_type_expr).collect();
            let effects_str = if effects.is_empty() {
                String::new()
            } else {
                format!(" effects({})", effects.join(", "))
            };
            format!(
                "({}) -> {}{}",
                params_str.join(", "),
                format_type_expr(ret),
                effects_str
            )
        }
        TypeExpr::Tuple { elements, .. } => {
            let elems_str: Vec<String> = elements.iter().map(format_type_expr).collect();
            format!("({})", elems_str.join(", "))
        }
    }
}

/// P7: Resolve an import path to an actual file path
fn resolve_import_path(module_path: &ModulePath, current_uri: &str) -> Option<std::path::PathBuf> {
    let segments = &module_path.segments;
    if segments.is_empty() {
        return None;
    }

    // Try relative to current file directory
    let current_file = uri_to_path(current_uri);
    let current_dir = std::path::Path::new(&current_file).parent()?;

    // Handle std.* modules - search in stdlib directory
    if segments.first().map(|s| s.as_str()) == Some("std") {
        // Look for stdlib next to executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let stdlib_dir = exe_dir.join("stdlib");
                if stdlib_dir.exists() {
                    // std.math -> stdlib/math.astra
                    if segments.len() >= 2 {
                        let filename = format!("{}.astra", segments[1]);
                        let path = stdlib_dir.join(&filename);
                        if path.exists() {
                            return Some(path);
                        }
                    }
                }
            }
        }
        // Also check cwd/stdlib
        if let Ok(cwd) = std::env::current_dir() {
            let stdlib_dir = cwd.join("stdlib");
            if segments.len() >= 2 {
                let filename = format!("{}.astra", segments[1]);
                let path = stdlib_dir.join(&filename);
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }

    // Try as relative path: module_name -> module_name.astra
    let filename = format!("{}.astra", segments.join("/"));
    let path = current_dir.join(&filename);
    if path.exists() {
        return Some(path);
    }

    // Try single-segment: module_name.astra in same directory
    if segments.len() == 1 {
        let filename = format!("{}.astra", segments[0]);
        let path = current_dir.join(&filename);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// P7: Convert a file path to an LSP URI
fn path_to_uri(path: &std::path::Path) -> String {
    format!("file://{}", path.display())
}
#[cfg(test)]
mod tests;
