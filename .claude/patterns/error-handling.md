# Pattern: Error Handling in Astra Toolchain

## Diagnostic Creation Pattern

Always create diagnostics with full context:

```rust
use crate::diagnostics::{Diagnostic, Severity, Span, Note, Suggestion, Edit};

fn type_mismatch(expected: &Type, found: &Type, span: Span, context_span: Option<Span>) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E1001")
        .message(format!("Type mismatch: expected `{}`, found `{}`", expected, found))
        .span(span);

    if let Some(ctx) = context_span {
        diagnostic = diagnostic.note(
            Note::new("Expected type comes from here").span(ctx)
        );
    }

    // Add suggestion if types are convertible
    if let Some(conversion) = find_conversion(found, expected) {
        diagnostic = diagnostic.suggestion(
            Suggestion::new("Convert type")
                .edit(Edit::new(span, conversion))
        );
    }

    diagnostic
}
```

## Result Propagation Pattern

Use `?` with context:

```rust
fn parse_file(path: &Path) -> Result<Ast, Diagnostic> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| Diagnostic::error("E0100")
            .message(format!("Could not read file: {}", e))
            .span(Span::file(path)))?;

    let tokens = lexer::tokenize(&content)
        .map_err(|e| e.with_file(path))?;

    let ast = parser::parse(&tokens)
        .map_err(|e| e.with_file(path))?;

    Ok(ast)
}
```

## Collecting Multiple Errors

Don't stop at first error when possible:

```rust
fn check_module(module: &Module) -> Result<(), Vec<Diagnostic>> {
    let mut errors = Vec::new();

    for item in &module.items {
        if let Err(e) = check_item(item) {
            errors.push(e);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
```

## Error Recovery in Parser

Continue parsing after errors:

```rust
fn parse_fn_def(&mut self) -> Result<FnDef, Diagnostic> {
    self.expect(Token::Fn)?;

    let name = match self.expect_ident() {
        Ok(name) => name,
        Err(e) => {
            self.errors.push(e);
            // Recover: use placeholder name
            "<error>".to_string()
        }
    };

    let params = self.parse_params().unwrap_or_else(|e| {
        self.errors.push(e);
        Vec::new()  // Recover: empty params
    });

    // Continue parsing body...
}
```
