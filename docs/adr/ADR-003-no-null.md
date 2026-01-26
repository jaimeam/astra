# ADR-003: No Null - Option and Result Types

## Status

Accepted

## Context

Null references have been called the "billion dollar mistake" by Tony Hoare. They cause:
- NullPointerExceptions at runtime
- Defensive null checking everywhere
- Unclear APIs (does this function return null?)
- Difficulty for LLMs to reason about nullability

Astra needs a clear approach to optional values and error handling.

## Decision

**Astra has no null. Use `Option[T]` for optional values and `Result[T, E]` for operations that can fail.**

```astra
# Optional value
fn find_user(id: Int) -> Option[User]

# Fallible operation
fn parse_int(s: Text) -> Result[Int, ParseError]
```

## Rationale

### Why No Null

1. **Type Safety**: Compiler enforces handling of missing values
2. **Explicitness**: Function signatures clearly indicate optionality
3. **No Surprises**: No unexpected null pointer errors
4. **Agent-Friendly**: Clear patterns for LLMs to generate

### Why Option[T]

1. **Explicit Optionality**: `Option[User]` vs `User` clearly different
2. **Pattern Matching**: Forces handling of `None` case
3. **Chainable**: `map`, `and_then`, etc. for clean transformations

### Why Result[T, E]

1. **Typed Errors**: Know exactly what can go wrong
2. **No Exceptions**: Control flow is explicit
3. **Composition**: `?` operator for clean propagation

## Consequences

### Positive

- No null pointer exceptions
- Clear, self-documenting APIs
- Exhaustive pattern matching catches missing cases
- Easy to test error paths

### Negative

- Slightly more verbose than nullable types
- Need to explicitly handle or propagate errors
- Learning curve for those used to null

## Usage Patterns

### Option[T]
```astra
# Creating options
let some_value: Option[Int] = Some(42)
let no_value: Option[Int] = None

# Pattern matching
match find_user(id) {
  Some(user) => greet(user)
  None => "User not found"
}

# Chaining
let name = find_user(id)
  .map(fn(u) { u.name })
  .unwrap_or("Anonymous")

# Early return with ?else
fn get_username(id: Int) -> Text {
  let user = find_user(id) ?else return "Unknown"
  user.name
}
```

### Result[T, E]
```astra
# Creating results
let success: Result[Int, Text] = Ok(42)
let failure: Result[Int, Text] = Err("Something went wrong")

# Pattern matching
match parse_int("123") {
  Ok(n) => n * 2
  Err(e) => {
    log_error(e)
    0
  }
}

# Propagation with ?
fn process(s: Text) -> Result[Int, ParseError] {
  let n = parse_int(s)?  # Returns Err early if parsing fails
  Ok(n * 2)
}

# Converting between Option and Result
let opt = result.ok()  # Result -> Option (discards error)
let res = option.ok_or("missing")  # Option -> Result
```

## Error Handling Philosophy

1. **Use Option** when absence is normal and expected
2. **Use Result** when failure needs explanation
3. **Propagate with ?** for clean error handling
4. **Match exhaustively** when you need to handle all cases
5. **Never panic** in library code (return Result instead)
