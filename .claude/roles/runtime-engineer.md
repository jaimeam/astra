# Role: Runtime/Interpreter/VM Engineer

## Responsibility
Build and maintain the interpreter/VM that executes Astra programs with controlled effects.

## Deliverables
- [ ] Tree-walking interpreter for v0.1
- [ ] Capability interface implementations
- [ ] Deterministic execution mode
- [ ] Resource limit hooks (timeouts, memory)
- [ ] Runtime error handling with spans

## Key Files
- `src/interpreter/mod.rs` - Interpreter entry point
- `src/interpreter/eval.rs` - Expression evaluation
- `src/interpreter/value.rs` - Runtime values
- `src/interpreter/env.rs` - Runtime environment
- `src/interpreter/capabilities/` - Built-in capability implementations

## Runtime Values
```rust
enum Value {
    Int(i64),
    Bool(bool),
    Text(String),
    Unit,
    Option(Option<Box<Value>>),
    Result(Result<Box<Value>, Box<Value>>),
    Record(HashMap<String, Value>),
    Enum { tag: String, data: Option<Box<Value>> },
    Function { params: Vec<String>, body: Expr, env: Env },
}
```

## Acceptance Criteria
- Runs all sample programs deterministically
- `Rand` is always seeded (reproducible)
- `Clock` is injectable (testable)
- Resource limits can be set (even if just placeholders)
- Runtime errors include source spans

## Determinism Requirements
- `Rand.int()` uses seeded PRNG
- `Clock.now()` comes from capability (not system time)
- Map/Set iteration order is defined
- No implicit system state access

## Interface Contract
Interpreter receives typed AST and capability set.
Must enforce that only declared capabilities are used.

## Dependencies
- Parser (AST)
- Type checker (typed AST)
- Effects checker (capability requirements)

## Testing Strategy
```bash
# Run interpreter tests
cargo test --lib interpreter

# Run integration tests
cargo test --test runtime
```

## Error Codes (E4xxx)
- `E4001`: Division by zero
- `E4002`: Index out of bounds
- `E4003`: Contract violation (requires/ensures)
- `E4004`: Resource limit exceeded
- `E4005`: Capability access denied

## Common Pitfalls
- Stack overflow on deep recursion
- Memory leaks in closures
- Forgetting to check capability permissions
- Non-deterministic behavior leaking through
