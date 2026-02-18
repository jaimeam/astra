//! Handler for the `astra doc` subcommand.

use std::path::PathBuf;

use crate::parser::{Lexer, Parser as AstraParser, SourceFile};

use super::walkdir;

pub(crate) fn run_doc(
    paths: &[PathBuf],
    output: &PathBuf,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Collect all .astra files
    let mut astra_files = Vec::new();
    for path in paths {
        if path.is_file() && path.extension().is_some_and(|ext| ext == "astra") {
            astra_files.push(path.clone());
        } else if path.is_dir() {
            for entry in walkdir(path)? {
                if entry.extension().is_some_and(|ext| ext == "astra") {
                    astra_files.push(entry);
                }
            }
        }
    }

    if astra_files.is_empty() {
        println!("No .astra files found");
        return Ok(());
    }

    std::fs::create_dir_all(output)?;

    let mut module_docs = Vec::new();

    for file_path in &astra_files {
        let source = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {:?}: {}", file_path, e))?;

        let source_file = SourceFile::new(file_path.clone(), source.clone());
        let lexer = Lexer::new(&source_file);
        let mut parser = AstraParser::new(lexer, source_file.clone());

        let module = match parser.parse_module() {
            Ok(m) => m,
            Err(_) => continue, // Skip files with parse errors
        };

        let doc = generate_module_doc(&module, &source, file_path);
        if !doc.is_empty() {
            let module_name = module.name.segments.join(".");
            let ext = if format == "html" { "html" } else { "md" };
            let out_file = output.join(format!("{}.{}", module_name, ext));

            let content = if format == "html" {
                markdown_to_html(&doc)
            } else {
                doc.clone()
            };

            std::fs::write(&out_file, &content)?;
            module_docs.push((module_name, out_file));
        }
    }

    // Generate index
    let ext = if format == "html" { "html" } else { "md" };
    let index_path = output.join(format!("index.{}", ext));
    let mut index = String::new();
    index.push_str("# API Documentation\n\n");
    for (name, path) in &module_docs {
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        index.push_str(&format!("- [{}]({})\n", name, filename));
    }

    let index_content = if format == "html" {
        markdown_to_html(&index)
    } else {
        index
    };
    std::fs::write(&index_path, index_content)?;

    println!(
        "Generated documentation for {} module(s) in {:?}",
        module_docs.len(),
        output
    );
    Ok(())
}

/// Generate documentation for a single module.
fn generate_module_doc(
    module: &crate::parser::ast::Module,
    source: &str,
    file_path: &std::path::Path,
) -> String {
    use crate::parser::ast::*;

    let mut doc = String::new();
    let module_name = module.name.segments.join(".");
    doc.push_str(&format!("# Module `{}`\n\n", module_name));
    doc.push_str(&format!(
        "Source: `{}`\n\n",
        file_path.file_name().unwrap_or_default().to_string_lossy()
    ));

    // Extract module-level doc comments (## lines before any items)
    let module_doc = extract_doc_comment(source, 1);
    if !module_doc.is_empty() {
        doc.push_str(&module_doc);
        doc.push_str("\n\n");
    }

    // Collect items by category
    let mut functions = Vec::new();
    let mut types = Vec::new();
    let mut enums = Vec::new();
    let mut traits = Vec::new();
    let mut effects = Vec::new();

    for item in &module.items {
        match item {
            Item::FnDef(def) => functions.push(def),
            Item::TypeDef(def) => types.push(def),
            Item::EnumDef(def) => enums.push(def),
            Item::TraitDef(def) => traits.push(def),
            Item::EffectDef(def) => effects.push(def),
            _ => {}
        }
    }

    // Document types
    if !types.is_empty() {
        doc.push_str("## Types\n\n");
        for def in &types {
            let doc_comment = extract_doc_comment(source, def.span.start_line);
            doc.push_str(&format!("### `type {}`\n\n", def.name));
            doc.push_str(&format!(
                "```astra\ntype {} = {}\n```\n\n",
                def.name,
                format_type_expr_for_doc(&def.value)
            ));
            if !doc_comment.is_empty() {
                doc.push_str(&doc_comment);
                doc.push_str("\n\n");
            }
        }
    }

    // Document enums
    if !enums.is_empty() {
        doc.push_str("## Enums\n\n");
        for def in &enums {
            let doc_comment = extract_doc_comment(source, def.span.start_line);
            doc.push_str(&format!("### `enum {}`\n\n", def.name));
            doc.push_str("```astra\nenum ");
            doc.push_str(&def.name);
            doc.push_str(" {\n");
            for v in &def.variants {
                if v.fields.is_empty() {
                    doc.push_str(&format!("  {}\n", v.name));
                } else {
                    let fields: Vec<String> = v
                        .fields
                        .iter()
                        .map(|f| format!("{}: {}", f.name, format_type_expr_for_doc(&f.ty)))
                        .collect();
                    doc.push_str(&format!("  {}({})\n", v.name, fields.join(", ")));
                }
            }
            doc.push_str("}\n```\n\n");
            if !doc_comment.is_empty() {
                doc.push_str(&doc_comment);
                doc.push_str("\n\n");
            }
        }
    }

    // Document traits
    if !traits.is_empty() {
        doc.push_str("## Traits\n\n");
        for def in &traits {
            let doc_comment = extract_doc_comment(source, def.span.start_line);
            doc.push_str(&format!("### `trait {}`\n\n", def.name));
            doc.push_str("```astra\ntrait ");
            doc.push_str(&def.name);
            doc.push_str(" {\n");
            for m in &def.methods {
                let params: Vec<String> = m
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, format_type_expr_for_doc(&p.ty)))
                    .collect();
                let ret = m
                    .return_type
                    .as_ref()
                    .map(|t| format!(" -> {}", format_type_expr_for_doc(t)))
                    .unwrap_or_default();
                doc.push_str(&format!("  fn {}({}){}\n", m.name, params.join(", "), ret));
            }
            doc.push_str("}\n```\n\n");
            if !doc_comment.is_empty() {
                doc.push_str(&doc_comment);
                doc.push_str("\n\n");
            }
        }
    }

    // Document effects
    if !effects.is_empty() {
        doc.push_str("## Effects\n\n");
        for def in &effects {
            let doc_comment = extract_doc_comment(source, def.span.start_line);
            doc.push_str(&format!("### `effect {}`\n\n", def.name));
            if !doc_comment.is_empty() {
                doc.push_str(&doc_comment);
                doc.push_str("\n\n");
            }
        }
    }

    // Document functions
    let public_fns: Vec<_> = functions
        .iter()
        .filter(|f| matches!(f.visibility, Visibility::Public))
        .collect();
    let private_fns: Vec<_> = functions
        .iter()
        .filter(|f| matches!(f.visibility, Visibility::Private))
        .collect();

    if !public_fns.is_empty() {
        doc.push_str("## Public Functions\n\n");
        for def in &public_fns {
            doc.push_str(&format_fn_doc(def, source));
        }
    }

    if !private_fns.is_empty() {
        doc.push_str("## Functions\n\n");
        for def in &private_fns {
            doc.push_str(&format_fn_doc(def, source));
        }
    }

    doc
}

/// Format documentation for a single function.
fn format_fn_doc(def: &crate::parser::ast::FnDef, source: &str) -> String {
    let mut doc = String::new();
    let doc_comment = extract_doc_comment(source, def.span.start_line);

    let type_params_str = if def.type_params.is_empty() {
        String::new()
    } else {
        format!("[{}]", def.type_params.join(", "))
    };

    let params_str: Vec<String> = def
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, format_type_expr_for_doc(&p.ty)))
        .collect();

    let ret_str = def
        .return_type
        .as_ref()
        .map(|t| format!(" -> {}", format_type_expr_for_doc(t)))
        .unwrap_or_default();

    let effects_str = if def.effects.is_empty() {
        String::new()
    } else {
        format!(" effects({})", def.effects.join(", "))
    };

    doc.push_str(&format!("### `{}`\n\n", def.name));
    doc.push_str(&format!(
        "```astra\nfn {}{}({}){}{}\n```\n\n",
        def.name,
        type_params_str,
        params_str.join(", "),
        ret_str,
        effects_str
    ));
    if !doc_comment.is_empty() {
        doc.push_str(&doc_comment);
        doc.push_str("\n\n");
    }
    doc
}

/// Extract doc comments (`##` lines) immediately before the given line.
fn extract_doc_comment(source: &str, item_line: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    if item_line == 0 || item_line > lines.len() {
        return String::new();
    }

    let mut doc_lines = Vec::new();
    let mut line_idx = item_line.saturating_sub(2); // 0-indexed, line before the item

    // Walk backwards collecting ## doc comment lines
    loop {
        if line_idx >= lines.len() {
            break;
        }
        let line = lines[line_idx].trim();
        if let Some(comment) = line.strip_prefix("##") {
            doc_lines.push(comment.trim().to_string());
        } else {
            break;
        }
        if line_idx == 0 {
            break;
        }
        line_idx -= 1;
    }

    doc_lines.reverse();
    doc_lines.join("\n")
}

/// Format a TypeExpr for documentation output.
fn format_type_expr_for_doc(ty: &crate::parser::ast::TypeExpr) -> String {
    use crate::parser::ast::TypeExpr;
    match ty {
        TypeExpr::Named { name, args, .. } => {
            if args.is_empty() {
                name.clone()
            } else {
                let args_str: Vec<String> = args.iter().map(format_type_expr_for_doc).collect();
                format!("{}[{}]", name, args_str.join(", "))
            }
        }
        TypeExpr::Record { fields, .. } => {
            let fields_str: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", f.name, format_type_expr_for_doc(&f.ty)))
                .collect();
            format!("{{ {} }}", fields_str.join(", "))
        }
        TypeExpr::Function {
            params,
            ret,
            effects,
            ..
        } => {
            let params_str: Vec<String> = params.iter().map(format_type_expr_for_doc).collect();
            let effects_str = if effects.is_empty() {
                String::new()
            } else {
                format!(" effects({})", effects.join(", "))
            };
            format!(
                "({}) -> {}{}",
                params_str.join(", "),
                format_type_expr_for_doc(ret),
                effects_str
            )
        }
        TypeExpr::Tuple { elements, .. } => {
            let elems_str: Vec<String> = elements.iter().map(format_type_expr_for_doc).collect();
            format!("({})", elems_str.join(", "))
        }
    }
}

/// Basic markdown-to-HTML conversion for the `--format html` option.
fn markdown_to_html(md: &str) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\">\n");
    html.push_str("<title>Astra API Docs</title>\n");
    html.push_str("<style>body{font-family:sans-serif;max-width:800px;margin:0 auto;padding:20px}");
    html.push_str("pre{background:#f4f4f4;padding:12px;overflow-x:auto}");
    html.push_str("code{background:#f4f4f4;padding:2px 4px}</style>\n");
    html.push_str("</head><body>\n");

    let mut in_code_block = false;
    for line in md.lines() {
        if line.starts_with("```") {
            if in_code_block {
                html.push_str("</code></pre>\n");
                in_code_block = false;
            } else {
                html.push_str("<pre><code>");
                in_code_block = true;
            }
        } else if in_code_block {
            html.push_str(&line.replace('<', "&lt;").replace('>', "&gt;"));
            html.push('\n');
        } else if let Some(heading) = line.strip_prefix("### ") {
            html.push_str(&format!("<h3>{}</h3>\n", heading));
        } else if let Some(heading) = line.strip_prefix("## ") {
            html.push_str(&format!("<h2>{}</h2>\n", heading));
        } else if let Some(heading) = line.strip_prefix("# ") {
            html.push_str(&format!("<h1>{}</h1>\n", heading));
        } else if let Some(item) = line.strip_prefix("- ") {
            html.push_str(&format!("<li>{}</li>\n", item));
        } else if line.is_empty() {
            html.push_str("<br>\n");
        } else {
            html.push_str(&format!("<p>{}</p>\n", line));
        }
    }

    html.push_str("</body></html>\n");
    html
}
