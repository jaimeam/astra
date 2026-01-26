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

### W0001: Unused variable

**Message**: `Unused variable: {name}`

**Fix**: Remove the variable or prefix with `_`.

---

### W0002: Unused import

**Message**: `Unused import: {module}`

---

### W0003: Unreachable code

**Message**: `Unreachable code after {statement}`

---

### W0004: Deprecated feature

**Message**: `Deprecated: {feature}. Use {alternative} instead`

---

### W0005: Wildcard match could be more specific

**Message**: `Wildcard match could be more specific`

---

### W0006: Shadowed binding

**Message**: `Variable '{name}' shadows previous binding`

---
