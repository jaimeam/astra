# ADR-002: Effects System over Monadic IO

## Status

Accepted

## Context

Astra needs to track and control side effects for:
- Sandboxable execution
- Deterministic testing
- Clear reasoning about what code does

Two main approaches exist in the functional programming world:
1. **Monadic IO** (Haskell-style): Effects threaded through types
2. **Effect Systems** (algebraic effects, capabilities): Effects declared in signatures

## Decision

**Use an explicit effect system with capability-based access.**

Functions declare effects in their signatures:
```astra
fn fetch(url: Text) -> Result[Text, Error]
  effects(Net)
{
  Net.get(url)
}
```

Capabilities are accessed through module namespaces (`Net`, `Fs`, etc.) and can be injected in tests.

## Rationale

### Why Effect System

1. **Readability**: Effects visible in signature without type-level complexity
2. **Agent-Friendly**: LLMs can easily see and generate effect declarations
3. **Testability**: Straightforward capability injection for mocking
4. **Gradual Adoption**: Can start pure and add effects as needed
5. **No Monad Tutorial**: Avoids cognitive overhead of understanding monads

### Why Not Monadic IO

1. **Complexity**: Monad transformers, do-notation learning curve
2. **Composition**: Combining different effects requires monad transformers or effect libraries
3. **Generation Difficulty**: LLMs struggle with complex type-level programming
4. **Verbosity**: Lifting, binding, and type signatures become noisy

### Why Not Implicit Effects

1. **Surprises**: Hidden effects violate principle of local reasoning
2. **Testing**: Hard to mock without explicit boundaries
3. **Security**: Can't enforce capability restrictions

## Consequences

### Positive

- Clear function signatures
- Easy capability injection for tests
- Natural security boundaries
- Simple mental model

### Negative

- Effect annotations add some verbosity
- Need to propagate effects through call chains
- Limited expressiveness compared to algebraic effects (for now)

### Future Considerations

- Effect polymorphism (generic over effects)
- Effect handlers (algebraic effects)
- Effect inference within modules

## Examples

### Declaring Effects
```astra
fn main() effects(Console, Net) {
  Console.println("Starting...")
  let data = Net.get("http://api.example.com/data")
  Console.println(data)
}
```

### Testing with Mocks
```astra
test "main with mocked network" {
  using effects(
    Console = TestConsole.new(),
    Net = MockNet.returning("test data")
  )

  main()
  assert TestConsole.output.contains("test data")
}
```

### Effect Checking
```astra
# Error E2001: Effect 'Net' not declared
fn bad() -> Text {
  Net.get("http://example.com")  # Compile error!
}
```
