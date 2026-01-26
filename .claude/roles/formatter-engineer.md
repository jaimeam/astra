# Role: Formatter Engineer

## Responsibility
Build and maintain the canonical formatter that produces deterministic, normalized Astra code.

## Deliverables
- [ ] Formatter that round-trips: `format(parse(format(code))) == format(code)`
- [ ] Formatting rules documentation in `docs/formatting.md`
- [ ] Golden tests for formatter output
- [ ] Integration with `astra fmt` CLI command

## Key Files
- `src/formatter/mod.rs` - Formatter entry point
- `src/formatter/pretty.rs` - Pretty printing logic
- `src/formatter/rules.rs` - Formatting rules
- `docs/formatting.md` - Human-readable rules

## Acceptance Criteria
- Formatting is idempotent: formatting twice gives same result
- Formatting is deterministic: same input always gives same output
- Minimal diffs: small code changes produce small format changes
- Preserves comments in appropriate locations

## Key Principles
1. **One canonical form**: No configuration options that change output
2. **Readability**: Optimize for human scanning
3. **Diff-friendliness**: Trailing commas, one item per line for long lists
4. **Consistent indentation**: 2 spaces, no tabs

## Interface Contract
Formatter receives AST from parser, must preserve:
- All semantic content
- Comments (associated with nearest node)
- Enough structure for round-trip stability

## Dependencies
- Parser (provides AST)

## Testing Strategy
```bash
# Run formatter tests
cargo test --lib formatter

# Test round-trip stability
cargo test --test formatter_roundtrip
```

## Common Pitfalls
- Comment association ambiguity
- Long line breaking heuristics
- Inconsistent spacing around operators
- Not handling all AST node types
