# Role: Effects & Capabilities Engineer

## Responsibility
Build and maintain the effect system that tracks and enforces capability-based I/O.

## Deliverables
- [ ] Effect declarations in function signatures
- [ ] Capability resolution and checking
- [ ] Test injection mechanism for capabilities
- [ ] Enforcement: no I/O without declared effects
- [ ] Effect inference within function bodies

## Key Files
- `src/effects/mod.rs` - Effect system entry point
- `src/effects/types.rs` - Effect type definitions
- `src/effects/checker.rs` - Effect checking
- `src/effects/capabilities.rs` - Capability definitions
- `src/effects/injection.rs` - Test capability injection

## Built-in Effects (v0.1)
```astra
Net     # Network I/O
Fs      # Filesystem access
Clock   # Time/date access
Rand    # Random number generation
Env     # Environment variables
Console # Console I/O
```

## Syntax
```astra
# Declare effects in function signature
fn fetch_data(url: Text) -> Result[Text, Error]
  effects(Net)
{
  Net.get(url)
}

# Pure function (no effects)
fn add(a: Int, b: Int) -> Int {
  a + b
}

# Test with injected capabilities
test "fetch with mock" {
  using effects(Net = MockNet.new())

  let result = fetch_data("http://example.com")
  assert result.is_ok()
}
```

## Acceptance Criteria
- Calling `Net.get()` in a pure function is compile error (E2001)
- Effects are checked transitively (callee effects ⊆ caller effects)
- Tests can replace capabilities with mocks
- Effect declarations are part of the function type

## Interface Contract
See `.claude/contracts/effects.md` for capability interface requirements.

## Dependencies
- Parser (effect syntax in AST)
- Type checker (effect types integrate with function types)

## Testing Strategy
```bash
# Run effect system tests
cargo test --lib effects

# Test capability injection
cargo test --test effect_injection
```

## Error Codes (E2xxx)
- `E2001`: Effect not declared in function signature
- `E2002`: Unknown effect
- `E2003`: Capability not available in scope
- `E2004`: Cannot call effectful function from pure context
- `E2005`: Effect mismatch in function call

## Common Pitfalls
- Forgetting transitive effect checking
- Not tracking effects through closures
- Complex interaction with generic functions
- Capability injection scoping issues
