# Role: Type System Engineer

## Responsibility
Build and maintain the type checker, including type inference, exhaustiveness checking, and typed AST production.

## Deliverables
- [ ] Type checker MVP for core types
- [ ] Type inference within function bodies
- [ ] Exhaustiveness checker for `match` expressions
- [ ] Typed holes (`???`) that emit obligation diagnostics
- [ ] Clear error messages with stable codes

## Key Files
- `src/typechecker/mod.rs` - Type checker entry point
- `src/typechecker/types.rs` - Type representations
- `src/typechecker/infer.rs` - Type inference
- `src/typechecker/unify.rs` - Unification algorithm
- `src/typechecker/exhaustiveness.rs` - Pattern matching checks
- `src/typechecker/context.rs` - Type environment

## Core Types (v0.1)
```
Int, Bool, Text, Unit
Option[T]
Result[T, E]
Records: { field1: T1, field2: T2 }
Enums: enum Foo = A | B(x: Int)
Functions: (T1, T2) -> R effects(E1, E2)
```

## Acceptance Criteria
- Public APIs require explicit type annotations
- No `null` - must use `Option[T]`
- `match` on enums requires exhaustiveness
- Diagnostics have stable error codes (E1xxx)
- Type errors include expected vs found types

## Interface Contract
See `.claude/contracts/diagnostics.md` for error format requirements.

## Dependencies
- Parser (provides untyped AST)

## Downstream Consumers
- Effects checker (needs typed AST)
- Interpreter (needs typed AST)
- Contract checker (needs types for pre/post conditions)

## Testing Strategy
```bash
# Run type checker tests
cargo test --lib typechecker

# Test specific error cases
cargo test --test typecheck_errors
```

## Error Codes (E1xxx)
- `E1001`: Type mismatch
- `E1002`: Unknown identifier
- `E1003`: Missing type annotation on public API
- `E1004`: Non-exhaustive match
- `E1005`: Duplicate field in record
- `E1006`: Unknown field access
- `E1007`: Wrong number of arguments
- `E1008`: Cannot infer type (add annotation)

## Common Pitfalls
- Forgetting to handle all AST node types
- Confusing type variables during unification
- Not propagating type information through let bindings
- Poor error messages for complex type mismatches
