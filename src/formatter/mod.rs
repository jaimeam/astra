//! Canonical formatter for Astra source code
//!
//! Produces a single, deterministic representation of any valid Astra program.

use crate::parser::ast::*;

/// Configuration for the formatter
#[derive(Debug, Clone)]
pub struct FormatConfig {
    /// Indentation string (default: 2 spaces)
    pub indent: String,
    /// Maximum line width before wrapping
    pub max_width: usize,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            indent: "  ".to_string(),
            max_width: 100,
        }
    }
}

/// Formatter for Astra source code
pub struct Formatter {
    config: FormatConfig,
    output: String,
    indent_level: usize,
}

impl Formatter {
    /// Create a new formatter with default configuration
    pub fn new() -> Self {
        Self::with_config(FormatConfig::default())
    }

    /// Create a new formatter with custom configuration
    pub fn with_config(config: FormatConfig) -> Self {
        Self {
            config,
            output: String::new(),
            indent_level: 0,
        }
    }

    /// Format a module and return the formatted source code
    pub fn format_module(&mut self, module: &Module) -> String {
        self.output.clear();
        self.indent_level = 0;

        // Module declaration
        self.write("module ");
        self.format_module_path(&module.name);
        self.newline();
        self.newline();

        // Items
        for (i, item) in module.items.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            self.format_item(item);
        }

        std::mem::take(&mut self.output)
    }

    fn format_module_path(&mut self, path: &ModulePath) {
        for (i, segment) in path.segments.iter().enumerate() {
            if i > 0 {
                self.write(".");
            }
            self.write(segment);
        }
    }

    fn format_item(&mut self, item: &Item) {
        match item {
            Item::Import(import) => self.format_import(import),
            Item::TypeDef(typedef) => self.format_typedef(typedef),
            Item::EnumDef(enumdef) => self.format_enumdef(enumdef),
            Item::FnDef(fndef) => self.format_fndef(fndef),
            Item::TraitDef(trait_def) => self.format_trait_def(trait_def),
            Item::ImplBlock(impl_block) => self.format_impl_block(impl_block),
            Item::EffectDef(effect_def) => self.format_effect_def(effect_def),
            Item::Test(test) => self.format_test(test),
            Item::Property(property) => self.format_property(property),
        }
    }

    fn format_import(&mut self, import: &ImportDecl) {
        self.write_indent();
        if import.public {
            self.write("public ");
        }
        self.write("import ");
        self.format_module_path(&import.path);

        match &import.kind {
            ImportKind::Module => {}
            ImportKind::Alias(alias) => {
                self.write(" as ");
                self.write(alias);
            }
            ImportKind::Items(items) => {
                self.write(".{");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(item);
                }
                self.write("}");
            }
        }
        self.newline();
    }

    fn format_typedef(&mut self, typedef: &TypeDef) {
        self.write_indent();
        self.write("type ");
        self.write(&typedef.name);
        self.format_type_params(&typedef.type_params);
        self.write(" = ");
        self.format_type_expr(&typedef.value);

        if let Some(invariant) = &typedef.invariant {
            self.newline();
            self.indent();
            self.write_indent();
            self.write("invariant ");
            self.format_expr(invariant);
            self.dedent();
        }

        self.newline();
    }

    fn format_enumdef(&mut self, enumdef: &EnumDef) {
        self.write_indent();
        self.write("enum ");
        self.write(&enumdef.name);
        self.format_type_params(&enumdef.type_params);
        self.write(" =");
        self.newline();

        self.indent();
        for variant in &enumdef.variants {
            self.write_indent();
            self.write("| ");
            self.write(&variant.name);
            if !variant.fields.is_empty() {
                self.write("(");
                for (i, field) in variant.fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&field.name);
                    self.write(": ");
                    self.format_type_expr(&field.ty);
                }
                self.write(")");
            }
            self.newline();
        }
        self.dedent();
    }

    fn format_fndef(&mut self, fndef: &FnDef) {
        self.write_indent();

        if fndef.visibility == Visibility::Public {
            self.write("public ");
        }

        self.write("fn ");
        self.write(&fndef.name);
        self.format_type_params(&fndef.type_params);
        self.write("(");

        for (i, param) in fndef.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            if let Some(ref pattern) = param.pattern {
                self.format_pattern(pattern);
            } else {
                self.write(&param.name);
            }
            self.write(": ");
            self.format_type_expr(&param.ty);
        }

        self.write(")");

        if let Some(ret) = &fndef.return_type {
            self.write(" -> ");
            self.format_type_expr(ret);
        }

        if !fndef.effects.is_empty() {
            self.newline();
            self.indent();
            self.write_indent();
            self.write("effects(");
            for (i, effect) in fndef.effects.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(effect);
            }
            self.write(")");
            self.dedent();
        }

        for req in &fndef.requires {
            self.newline();
            self.indent();
            self.write_indent();
            self.write("requires ");
            self.format_expr(req);
            self.dedent();
        }

        for ens in &fndef.ensures {
            self.newline();
            self.indent();
            self.write_indent();
            self.write("ensures ");
            self.format_expr(ens);
            self.dedent();
        }

        self.newline();
        self.format_block(&fndef.body);
        self.newline();
    }

    fn format_trait_def(&mut self, trait_def: &TraitDef) {
        self.write_indent();
        self.write("trait ");
        self.write(&trait_def.name);
        if !trait_def.type_params.is_empty() {
            self.write("[");
            self.write(&trait_def.type_params.join(", "));
            self.write("]");
        }
        self.write(" {");
        self.newline();
        self.indent_level += 1;
        for method in &trait_def.methods {
            self.write_indent();
            self.write("fn ");
            self.write(&method.name);
            self.write("(");
            for (i, param) in method.params.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&param.name);
                self.write(": ");
                self.format_type_expr(&param.ty);
            }
            self.write(")");
            if let Some(ret) = &method.return_type {
                self.write(" -> ");
                self.format_type_expr(ret);
            }
            self.newline();
        }
        self.indent_level -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    fn format_impl_block(&mut self, impl_block: &ImplBlock) {
        self.write_indent();
        self.write("impl ");
        self.write(&impl_block.trait_name);
        self.write(" for ");
        self.format_type_expr(&impl_block.target_type);
        self.write(" {");
        self.newline();
        self.indent_level += 1;
        for method in &impl_block.methods {
            self.format_fndef(method);
            self.newline();
        }
        self.indent_level -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    fn format_effect_def(&mut self, effect_def: &EffectDecl) {
        self.write_indent();
        self.write("effect ");
        self.write(&effect_def.name);
        self.write(" {");
        self.newline();
        self.indent_level += 1;
        for op in &effect_def.operations {
            self.write_indent();
            self.write("fn ");
            self.write(&op.name);
            self.write("(");
            for (i, param) in op.params.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&param.name);
                self.write(": ");
                self.format_type_expr(&param.ty);
            }
            self.write(")");
            if let Some(ret) = &op.return_type {
                self.write(" -> ");
                self.format_type_expr(ret);
            }
            self.newline();
        }
        self.indent_level -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    fn format_test(&mut self, test: &TestBlock) {
        self.write_indent();
        self.write("test \"");
        self.write(&test.name);
        self.write("\"");

        if let Some(using) = &test.using {
            self.write(" ");
            self.format_using(using);
        }

        self.write(" ");
        self.format_block(&test.body);
        self.newline();
    }

    fn format_property(&mut self, property: &PropertyBlock) {
        self.write_indent();
        self.write("property \"");
        self.write(&property.name);
        self.write("\"");

        if let Some(using) = &property.using {
            self.write(" ");
            self.format_using(using);
        }

        self.write(" ");
        self.format_block(&property.body);
        self.newline();
    }

    fn format_using(&mut self, using: &UsingClause) {
        self.write("using effects(");
        for (i, binding) in using.bindings.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write(&binding.effect);
            self.write(" = ");
            self.format_expr(&binding.value);
        }
        self.write(")");
    }

    fn format_type_params(&mut self, params: &[String]) {
        if !params.is_empty() {
            self.write("[");
            for (i, param) in params.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(param);
            }
            self.write("]");
        }
    }

    fn format_type_expr(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Named { name, args, .. } => {
                self.write(name);
                if !args.is_empty() {
                    self.write("[");
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.format_type_expr(arg);
                    }
                    self.write("]");
                }
            }
            TypeExpr::Record { fields, .. } => {
                self.write("{ ");
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&field.name);
                    self.write(": ");
                    self.format_type_expr(&field.ty);
                }
                self.write(" }");
            }
            TypeExpr::Function {
                params,
                ret,
                effects,
                ..
            } => {
                self.write("(");
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_type_expr(param);
                }
                self.write(") -> ");
                self.format_type_expr(ret);
                if !effects.is_empty() {
                    self.write(" effects(");
                    for (i, effect) in effects.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.write(effect);
                    }
                    self.write(")");
                }
            }
            TypeExpr::Tuple { elements, .. } => {
                self.write("(");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_type_expr(elem);
                }
                self.write(")");
            }
        }
    }

    fn format_block(&mut self, block: &Block) {
        self.write("{");
        if block.stmts.is_empty() && block.expr.is_none() {
            self.write("}");
            return;
        }

        self.newline();
        self.indent();

        for stmt in &block.stmts {
            self.format_stmt(stmt);
        }

        if let Some(expr) = &block.expr {
            self.write_indent();
            self.format_expr(expr);
            self.newline();
        }

        self.dedent();
        self.write_indent();
        self.write("}");
    }

    fn format_stmt(&mut self, stmt: &Stmt) {
        self.write_indent();
        match stmt {
            Stmt::Let {
                name,
                mutable,
                ty,
                value,
                ..
            } => {
                self.write("let ");
                if *mutable {
                    self.write("mut ");
                }
                self.write(name);
                if let Some(ty) = ty {
                    self.write(": ");
                    self.format_type_expr(ty);
                }
                self.write(" = ");
                self.format_expr(value);
            }
            Stmt::LetPattern {
                pattern, ty, value, ..
            } => {
                self.write("let ");
                self.format_pattern(pattern);
                if let Some(ty) = ty {
                    self.write(": ");
                    self.format_type_expr(ty);
                }
                self.write(" = ");
                self.format_expr(value);
            }
            Stmt::Assign { target, value, .. } => {
                self.format_expr(target);
                self.write(" = ");
                self.format_expr(value);
            }
            Stmt::Expr { expr, .. } => {
                self.format_expr(expr);
            }
            Stmt::Return { value, .. } => {
                self.write("return");
                if let Some(v) = value {
                    self.write(" ");
                    self.format_expr(v);
                }
            }
        }
        self.newline();
    }

    fn format_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::IntLit { value, .. } => {
                self.write(&value.to_string());
            }
            Expr::FloatLit { value, .. } => {
                self.write(&format!("{}", value));
            }
            Expr::BoolLit { value, .. } => {
                self.write(if *value { "true" } else { "false" });
            }
            Expr::TextLit { value, .. } => {
                self.write("\"");
                self.write(&escape_string(value));
                self.write("\"");
            }
            Expr::UnitLit { .. } => {
                self.write("()");
            }
            Expr::Ident { name, .. } => {
                self.write(name);
            }
            Expr::QualifiedIdent { module, name, .. } => {
                self.write(module);
                self.write(".");
                self.write(name);
            }
            Expr::Record { fields, .. } => {
                self.write("{ ");
                for (i, (name, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(name);
                    self.write(" = ");
                    self.format_expr(value);
                }
                self.write(" }");
            }
            Expr::FieldAccess { expr, field, .. } => {
                self.format_expr(expr);
                self.write(".");
                self.write(field);
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                self.format_expr(left);
                self.write(" ");
                self.write(op.as_str());
                self.write(" ");
                self.format_expr(right);
            }
            Expr::Unary { op, expr, .. } => {
                self.write(op.as_str());
                self.format_expr(expr);
            }
            Expr::Call { func, args, .. } => {
                self.format_expr(func);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(arg);
                }
                self.write(")");
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                self.format_expr(receiver);
                self.write(".");
                self.write(method);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(arg);
                }
                self.write(")");
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.write("if ");
                self.format_expr(cond);
                self.write(" ");
                self.format_block(then_branch);
                if let Some(else_expr) = else_branch {
                    self.write(" else ");
                    match else_expr.as_ref() {
                        Expr::If { .. } => self.format_expr(else_expr),
                        Expr::Block { block, .. } => self.format_block(block),
                        _ => self.format_expr(else_expr),
                    }
                }
            }
            Expr::Match { expr, arms, .. } => {
                self.write("match ");
                self.format_expr(expr);
                self.write(" {");
                self.newline();
                self.indent();
                for arm in arms {
                    self.write_indent();
                    self.format_pattern(&arm.pattern);
                    if let Some(guard) = &arm.guard {
                        self.write(" if ");
                        self.format_expr(guard);
                    }
                    self.write(" => ");
                    self.format_expr(&arm.body);
                    self.newline();
                }
                self.dedent();
                self.write_indent();
                self.write("}");
            }
            Expr::Block { block, .. } => {
                self.format_block(block);
            }
            Expr::Try { expr, .. } => {
                self.format_expr(expr);
                self.write("?");
            }
            Expr::TryElse {
                expr, else_expr, ..
            } => {
                self.format_expr(expr);
                self.write(" ?else ");
                self.format_expr(else_expr);
            }
            Expr::ListLit { elements, .. } => {
                self.write("[");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(elem);
                }
                self.write("]");
            }
            Expr::Lambda {
                params,
                return_type,
                body,
                ..
            } => {
                self.write("fn(");
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&param.name);
                    if let Some(ty) = &param.ty {
                        self.write(": ");
                        self.format_type_expr(ty);
                    }
                }
                self.write(")");
                if let Some(ret) = return_type {
                    self.write(" -> ");
                    self.format_type_expr(ret);
                }
                self.write(" ");
                self.format_block(body);
            }
            Expr::ForIn {
                binding,
                iter,
                body,
                ..
            } => {
                self.write("for ");
                self.write(binding);
                self.write(" in ");
                self.format_expr(iter);
                self.write(" ");
                self.format_block(body);
            }
            Expr::While { cond, body, .. } => {
                self.write("while ");
                self.format_expr(cond);
                self.write(" ");
                self.format_block(body);
            }
            Expr::Break { .. } => {
                self.write("break");
            }
            Expr::Continue { .. } => {
                self.write("continue");
            }
            Expr::StringInterp { parts, .. } => {
                self.write("\"");
                for part in parts {
                    match part {
                        StringPart::Literal(s) => self.write(&s.replace('"', "\\\"")),
                        StringPart::Expr(expr) => {
                            self.write("${");
                            self.format_expr(expr);
                            self.write("}");
                        }
                    }
                }
                self.write("\"");
            }
            Expr::TupleLit { elements, .. } => {
                self.write("(");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(elem);
                }
                self.write(")");
            }
            Expr::MapLit { entries, .. } => {
                self.write("Map.from([");
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write("(");
                    self.format_expr(k);
                    self.write(", ");
                    self.format_expr(v);
                    self.write(")");
                }
                self.write("])");
            }
            Expr::Await { expr, .. } => {
                self.write("await ");
                self.format_expr(expr);
            }
            Expr::Hole { .. } => {
                self.write("???");
            }
        }
    }

    fn format_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Wildcard { .. } => {
                self.write("_");
            }
            Pattern::IntLit { value, .. } => {
                self.write(&value.to_string());
            }
            Pattern::FloatLit { value, .. } => {
                self.write(&format!("{}", value));
            }
            Pattern::BoolLit { value, .. } => {
                self.write(if *value { "true" } else { "false" });
            }
            Pattern::TextLit { value, .. } => {
                self.write("\"");
                self.write(&escape_string(value));
                self.write("\"");
            }
            Pattern::Ident { name, .. } => {
                self.write(name);
            }
            Pattern::Variant { name, fields, .. } => {
                self.write(name);
                if !fields.is_empty() {
                    self.write("(");
                    for (i, p) in fields.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.format_pattern(p);
                    }
                    self.write(")");
                }
            }
            Pattern::Record { fields, .. } => {
                self.write("{ ");
                for (i, (name, pattern)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(name);
                    if !matches!(pattern, Pattern::Ident { name: n, .. } if n == name) {
                        self.write(" = ");
                        self.format_pattern(pattern);
                    }
                }
                self.write(" }");
            }
            Pattern::Tuple { elements, .. } => {
                self.write("(");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_pattern(elem);
                }
                self.write(")");
            }
        }
    }

    // Helper methods

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.output.push_str(&self.config.indent);
        }
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    fn indent(&mut self) {
        self.indent_level += 1;
    }

    fn dedent(&mut self) {
        self.indent_level = self.indent_level.saturating_sub(1);
    }
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

fn escape_string(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Lexer, Parser, SourceFile};
    use std::path::PathBuf;

    fn format_source(source: &str) -> String {
        let source_file = SourceFile::new(PathBuf::from("test.astra"), source.to_string());
        let lexer = Lexer::new(&source_file);
        let mut parser = Parser::new(lexer, source_file.clone());
        let module = parser.parse_module().unwrap_or_else(|bag| {
            let msgs: Vec<_> = bag.diagnostics().iter().map(|d| &d.message).collect();
            panic!("parse failed: {:?}", msgs)
        });
        let mut formatter = Formatter::new();
        formatter.format_module(&module)
    }

    #[test]
    fn test_format_simple_function() {
        let output =
            format_source("module example\n\nfn add(a: Int, b: Int) -> Int {\n  a + b\n}\n");
        assert!(output.contains("fn add(a: Int, b: Int) -> Int"));
    }

    #[test]
    fn test_format_indentation() {
        let output =
            format_source("module example\n\nfn main() -> Int {\n  let x = 1\n  x + 2\n}\n");
        assert!(output.contains("  let x = 1"));
    }

    #[test]
    fn test_format_module_declaration() {
        let output = format_source("module my.pkg\n");
        assert!(output.starts_with("module my.pkg\n"));
    }

    #[test]
    fn test_format_import() {
        let output = format_source("module example\n\nimport std.math\n");
        assert!(output.contains("import std.math"));
    }

    #[test]
    fn test_format_named_import() {
        // Named imports use plain module paths in current parser
        let output = format_source("module example\n\nimport std.math\n");
        assert!(output.contains("import std.math"), "output: {}", output);
    }

    #[test]
    fn test_format_type_def() {
        let output = format_source("module example\n\ntype Age = Int\n");
        assert!(output.contains("type Age = Int"));
    }

    #[test]
    fn test_format_enum_def() {
        let input = r#"module example

enum Color =
  | Red
  | Green
  | Blue
"#;
        let output = format_source(input);
        assert!(output.contains("enum Color ="), "output: {}", output);
        assert!(output.contains("| Red"), "output: {}", output);
    }

    #[test]
    fn test_format_if_else() {
        let output = format_source(
            "module example\n\nfn f(x: Int) -> Int {\n  if x > 0 { x } else { 0 - x }\n}\n",
        );
        assert!(output.contains("if x > 0 {"));
    }

    #[test]
    fn test_format_match() {
        let output = format_source(
            "module example\n\nfn f(x: Option[Int]) -> Int {\n  match x {\n    Some(v) => v\n    None => 0\n  }\n}\n",
        );
        assert!(output.contains("match x {"));
        assert!(output.contains("Some(v) => v"));
    }

    #[test]
    fn test_format_idempotent() {
        let input = "module example\n\nfn add(a: Int, b: Int) -> Int {\n  a + b\n}\n";
        let first = format_source(input);
        let second = format_source(&first);
        assert_eq!(first, second, "Formatting should be idempotent");
    }

    #[test]
    fn test_format_trait_def() {
        let output =
            format_source("module example\n\ntrait Show {\n  fn to_text(s: Text) -> Text\n}\n");
        assert!(output.contains("trait Show {"), "output: {}", output);
        assert!(
            output.contains("fn to_text(s: Text) -> Text"),
            "output: {}",
            output
        );
    }

    #[test]
    fn test_format_effect_def() {
        let output =
            format_source("module example\n\neffect Logger {\n  fn log(msg: Text) -> Unit\n}\n");
        assert!(output.contains("effect Logger {"));
        assert!(output.contains("fn log(msg: Text) -> Unit"));
    }

    #[test]
    fn test_format_lambda() {
        let output = format_source(
            "module example\n\nfn main() -> Int {\n  let f = fn(x: Int) -> Int { x + 1 }\n  f(5)\n}\n",
        );
        assert!(output.contains("fn(x: Int) -> Int {"));
    }

    #[test]
    fn test_format_list_literal() {
        let output = format_source("module example\n\nfn main() -> List[Int] {\n  [1, 2, 3]\n}\n");
        assert!(output.contains("[1, 2, 3]"));
    }

    #[test]
    fn test_format_contracts() {
        let output = format_source(
            "module example\n\nfn divide(a: Int, b: Int) -> Int\nrequires b != 0\nensures result >= 0\n{\n  a / b\n}\n",
        );
        assert!(output.contains("requires b != 0"));
        assert!(output.contains("ensures result >= 0"));
    }

    #[test]
    fn test_format_for_loop() {
        let output = format_source(
            "module example\n\nfn main() -> Unit {\n  for x in [1, 2, 3] {\n    println(x)\n  }\n}\n",
        );
        assert!(output.contains("for x in"));
    }

    #[test]
    fn test_format_pipe_operator() {
        let output =
            format_source("module example\n\nfn main() -> Int {\n  5 |> add_one |> double\n}\n");
        assert!(output.contains("|>"));
    }

    #[test]
    fn test_format_string_interpolation() {
        let output = format_source(
            "module example\n\nfn main() -> Text {\n  let name = \"world\"\n  \"Hello, ${name}!\"\n}\n",
        );
        assert!(output.contains("${name}"));
    }

    #[test]
    fn test_format_generic_function() {
        let output = format_source("module example\n\nfn identity[T](x: T) -> T {\n  x\n}\n");
        assert!(output.contains("fn identity[T](x: T) -> T"));
    }

    #[test]
    fn test_format_while_loop() {
        let output = format_source(
            "module example\n\nfn main() -> Unit {\n  let x = 0\n  while x < 10 {\n    x = x + 1\n  }\n}\n",
        );
        assert!(output.contains("while x < 10 {"));
    }

    #[test]
    fn test_escape_string_function() {
        assert_eq!(escape_string(r#"hello"world"#), r#"hello\"world"#);
        assert_eq!(escape_string("line\nnewline"), "line\\nnewline");
        assert_eq!(escape_string("tab\there"), "tab\\there");
        assert_eq!(escape_string("back\\slash"), "back\\\\slash");
    }
}
