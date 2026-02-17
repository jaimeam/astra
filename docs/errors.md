# Astra Error Codes Reference

This document lists all error codes produced by the Astra compiler and runtime.

## Error Code Format

Error codes follow the pattern `E####` where:
- `E0xxx` - Syntax/Parsing errors
- `E1xxx` - Type errors
- `E2xxx` - Effect errors
- `E3xxx` - Contract violations
- `E4xxx` - Runtime errors
- `W0xxx` - Warnings

---

## Syntax Errors (E0xxx)

### E0001: Unexpected token

**Message**: `Unexpected token: expected {expected}, found {found}`

**Explanation**: The parser encountered a token that doesn't fit the expected grammar at this position.

**Example**:
```astra
fn add(a Int) -> Int {  # Error: expected ':', found 'Int'
  a
}
```

**Fix**: Add the missing punctuation or correct the syntax.

---

### E0002: Unterminated string literal

**Message**: `Unterminated string literal`

**Explanation**: A string literal was opened with `"` but never closed.

**Example**:
```astra
let s = "hello
```

**Fix**: Close the string with a matching `"`.

---

### E0003: Invalid number literal

**Message**: `Invalid number literal: {details}`

**Explanation**: A number literal is malformed.

**Example**:
```astra
let n = 123abc  # Error: Invalid number literal
```

**Fix**: Ensure numbers contain only digits.

---

### E0004: Missing closing delimiter

**Message**: `Missing closing {delimiter}`

**Explanation**: An opening bracket, brace, or parenthesis was not closed.

**Example**:
```astra
fn foo() {
  let x = (1 + 2
}
```

**Fix**: Add the matching closing delimiter.

---

### E0005: Invalid identifier

**Message**: `Invalid identifier: {name}`

**Explanation**: An identifier contains invalid characters or starts incorrectly.

---

### E0006: Reserved keyword used as identifier

**Message**: `'{keyword}' is a reserved keyword`

**Explanation**: A reserved keyword cannot be used as a variable or function name.

**Example**:
```astra
let match = 5  # Error: 'match' is a reserved keyword
```

**Fix**: Choose a different name.

---

### E0011: Module not found

**Message**: `Module not found: std.{name}`

**Explanation**: An import references a standard library module that does not exist.

**Example**:
```astra
import std.nonexistent  # Error: Module not found
```

**Fix**: Use a valid standard library module name. Available modules:
`std.collections`, `std.core`, `std.error`, `std.io`, `std.iter`,
`std.json`, `std.list`, `std.math`, `std.option`, `std.prelude`,
`std.result`, `std.string`.

---

## Type Errors (E1xxx)

### E1001: Type mismatch

**Message**: `Type mismatch: expected {expected}, found {found}`

**Explanation**: An expression has a different type than expected.

**Example**:
```astra
fn add(a: Int, b: Int) -> Int {
  a + "hello"  # Error: expected Int, found Text
}
```

**Fix**: Ensure the types match or add explicit conversion.

---

### E1002: Unknown identifier

**Message**: `Unknown identifier: {name}`

**Explanation**: A variable or function was used but not defined.

**Example**:
```astra
fn foo() -> Int {
  x + 1  # Error: Unknown identifier 'x'
}
```

**Fix**: Define the identifier or fix the spelling.

---

### E1003: Missing type annotation on public API

**Message**: `Public function '{name}' requires explicit return type annotation`

**Explanation**: Public functions must have explicit type annotations for parameters and return type.

**Example**:
```astra
public fn add(a, b) {  # Error: Missing type annotations
  a + b
}
```

**Fix**: Add type annotations:
```astra
public fn add(a: Int, b: Int) -> Int {
  a + b
}
```

---

### E1004: Non-exhaustive match

**Message**: `Non-exhaustive match: missing patterns {patterns}`

**Explanation**: A match expression doesn't cover all possible values.

**Example**:
```astra
enum Color = Red | Green | Blue

fn describe(c: Color) -> Text {
  match c {
    Red => "red"
    Green => "green"
    # Error: Missing pattern 'Blue'
  }
}
```

**Fix**: Add the missing patterns or use a wildcard `_`.

---

### E1005: Duplicate field in record

**Message**: `Duplicate field '{name}' in record`

**Example**:
```astra
let r = { x = 1, x = 2 }  # Error: Duplicate field 'x'
```

---

### E1006: Unknown field access

**Message**: `Unknown field '{name}' on type {type}`

**Example**:
```astra
type Point = { x: Int, y: Int }
let p = { x = 1, y = 2 }
p.z  # Error: Unknown field 'z'
```

---

### E1007: Wrong number of arguments

**Message**: `Wrong number of arguments: expected {expected}, found {found}`

---

### E1008: Cannot infer type

**Message**: `Cannot infer type for {item}; add a type annotation`

---

### E1016: Trait constraint not satisfied

**Message**: `Type '{type}' does not implement trait '{trait}' required by type parameter '{param}'`

**Explanation**: A generic function was called with a concrete type that doesn't satisfy
the declared trait bound on the type parameter.

**Example**:
```astra
trait Sortable {
  fn compare(self, other: Int) -> Int
}

fn sort_items[T: Sortable](items: List[T]) -> List[T] { items }

fn main() -> Unit {
  sort_items(["hello"])  # Error: Text does not implement Sortable
}
```

**Fix**: Either implement the trait for the type, or use a type that already implements it.

---

## Effect Errors (E2xxx)

### E2001: Effect not declared

**Message**: `Effect '{effect}' not declared in function signature`

**Explanation**: A function uses an effectful operation without declaring it.

**Example**:
```astra
fn fetch() -> Text {
  Net.get("http://example.com")  # Error: Effect 'Net' not declared
}
```

**Fix**: Add effects to the signature:
```astra
fn fetch() -> Text
  effects(Net)
{
  Net.get("http://example.com")
}
```

---

### E2002: Unknown effect

**Message**: `Unknown effect: {name}`

---

### E2003: Capability not available

**Message**: `Capability '{name}' not available in current scope`

---

### E2004: Effectful call from pure context

**Message**: `Cannot call effectful function '{name}' from pure context`

---

## Contract Errors (E3xxx)

### E3001: Precondition violation

**Message**: `Precondition violated: {condition}`

---

### E3002: Postcondition violation

**Message**: `Postcondition violated: {condition}`

---

### E3003: Invariant violation

**Message**: `Invariant violated: {condition}`

---

## Runtime Errors (E4xxx)

### E4001: Division by zero

**Message**: `Division by zero`

---

### E4002: Index out of bounds

**Message**: `Index {index} out of bounds for length {length}`

---

### E4003: Contract violation at runtime

**Message**: `Contract violation: {condition}`

---

### E4004: Resource limit exceeded

**Message**: `Resource limit exceeded: {resource}`

---

### E4005: Capability access denied

**Message**: `Capability access denied: {capability}`

---

### E4006: Integer overflow

**Message**: `Integer overflow in {operation}`

---

### E4007: Stack overflow

**Message**: `Stack overflow`

---

### E4008: Assertion failed

**Message**: `Assertion failed: {message}`

---

## Warnings (W0xxx)

Warnings indicate code that is valid but likely incorrect or suboptimal. By default, warnings are reported but do not prevent compilation. Use `astra check --strict` to treat all warnings as errors.

### W0001: Unused variable

**Message**: `Unused variable '{name}'`

**Explanation**: A variable was defined but never read. This usually indicates dead code or a missing reference.

**Example**:
```astra
fn main() -> Int {
  let unused = 42   # Warning: Unused variable 'unused'
  0
}
```

**Fix**: Remove the variable, use it, or prefix with `_` to suppress the warning:
```astra
fn main() -> Int {
  let _unused = 42  # No warning: underscore prefix suppresses W0001
  0
}
```

---

### W0002: Unused import

**Message**: `Unused import '{name}'`

**Explanation**: An imported module or item is never referenced in the file.

**Example**:
```astra
module example

import std.math     # Warning: Unused import 'math'

fn main() -> Int {
  42
}
```

**Fix**: Remove the import if it is no longer needed.

---

### W0003: Unreachable code

**Message**: `Unreachable code after return statement`

**Explanation**: Code appears after a `return` statement in the same block. It will never be executed.

**Example**:
```astra
fn main() -> Int {
  return 1
  let x = 2   # Warning: Unreachable code after return statement
  x
}
```

**Fix**: Remove the unreachable code or restructure the control flow.

---

### W0004: Deprecated feature

**Message**: `Deprecated: {feature}. Use {alternative} instead`

**Explanation**: A language feature or API has been deprecated. This warning is not yet emitted by the compiler but the code is reserved for future use.

---

### W0005: Wildcard match could be more specific

**Message**: `Wildcard pattern '_' on {type} type could hide unhandled variants`

**Explanation**: A `match` expression uses a wildcard `_` pattern when the matched type is fully known (e.g., `Option`, `Result`, `Bool`, or a user-defined enum). While valid, this can silently swallow new variants added later.

**Example**:
```astra
fn describe(x: Option[Int]) -> Text {
  match x {
    Some(n) => "got a number"
    _ => "nothing"            # Warning: Wildcard on Option type
  }
}
```

**Fix**: Match all variants explicitly:
```astra
fn describe(x: Option[Int]) -> Text {
  match x {
    Some(n) => "got a number"
    None => "nothing"         # No warning: all variants covered
  }
}
```

---

### W0006: Shadowed binding

**Message**: `Variable '{name}' shadows a previous binding in the same scope`

**Explanation**: A `let` binding reuses a name that was already defined in the same scope. This can cause confusion about which value is being referenced.

**Example**:
```astra
fn main() -> Int {
  let x = 1
  let x = 2   # Warning: Variable 'x' shadows a previous binding
  x
}
```

**Fix**: Use a different name for the second binding:
```astra
fn main() -> Int {
  let x = 1
  let y = 2
  y
}
```

---

### W0007: Redundant type annotation

**Message**: `Redundant type annotation on '{name}'`

**Explanation**: An explicit type annotation matches the type that would be inferred. This warning is not yet emitted by the compiler but the code is reserved for future use.

---

## Strictness Mode

Running `astra check --strict` treats all warnings as errors, causing the checker to exit with a non-zero status code if any warnings are present. This is recommended for CI pipelines and production codebases.

The strictness level can also be configured per-project in `astra.toml`:

```toml
[lint]
level = "deny"   # "warn" (default) or "deny" (treats warnings as errors)
```

---
