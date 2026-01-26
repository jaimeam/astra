# Role: Parser Engineer

## Responsibility
Build and maintain the lexer, parser, and AST for the Astra language.

## Deliverables
- [ ] Grammar definition (EBNF) in `docs/grammar.ebnf`
- [ ] Lexer producing tokens with spans
- [ ] Parser producing AST with spans and node IDs
- [ ] AST definitions with stable JSON serialization
- [ ] Error recovery for partial/invalid code
- [ ] Golden tests in `tests/syntax/`

## Key Files
- `src/parser/lexer.rs` - Tokenization
- `src/parser/parser.rs` - Recursive descent parser
- `src/parser/ast.rs` - AST node definitions
- `src/parser/span.rs` - Source location tracking
- `src/parser/error.rs` - Parse error types

## Acceptance Criteria
- Parses all files in `tests/syntax/*.astra`
- Produces consistent spans for all nodes
- Error messages include source location and context
- AST serialization is deterministic

## Interface Contract
See `.claude/contracts/ast.md` for the AST structure requirements.

## Dependencies
- None (parser is foundational)

## Downstream Consumers
- Formatter (needs AST with spans)
- Type checker (needs typed AST)
- Interpreter (needs executable AST)

## Testing Strategy
```bash
# Run parser tests
cargo test --lib parser

# Update golden files (when intentionally changing output)
UPDATE_GOLDEN=1 cargo test --test golden
```

## Common Pitfalls
- Forgetting to track spans through all transformations
- Inconsistent handling of whitespace/comments
- Poor error messages for common mistakes
- Not preserving enough information for formatter round-trips
