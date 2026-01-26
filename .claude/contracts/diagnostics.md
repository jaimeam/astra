# Diagnostics Contract

> This document defines the stable interface for compiler diagnostics.
> All error messages, warnings, and suggestions must follow this format.

## Error Code Registry

Error codes are stable identifiers that never change meaning.

| Range | Category |
|-------|----------|
| E0xxx | Syntax/Parsing errors |
| E1xxx | Type errors |
| E2xxx | Effect errors |
| E3xxx | Contract violations |
| E4xxx | Runtime errors |
| W0xxx | Warnings |

### Syntax Errors (E0xxx)
- `E0001`: Unexpected token
- `E0002`: Unterminated string literal
- `E0003`: Invalid number literal
- `E0004`: Missing closing delimiter
- `E0005`: Invalid identifier
- `E0006`: Reserved keyword used as identifier
- `E0007`: Invalid escape sequence
- `E0008`: Unexpected end of file
- `E0009`: Invalid module declaration
- `E0010`: Duplicate module declaration

### Type Errors (E1xxx)
- `E1001`: Type mismatch
- `E1002`: Unknown identifier
- `E1003`: Missing type annotation on public API
- `E1004`: Non-exhaustive match
- `E1005`: Duplicate field in record
- `E1006`: Unknown field access
- `E1007`: Wrong number of arguments
- `E1008`: Cannot infer type
- `E1009`: Recursive type without indirection
- `E1010`: Invalid type application
- `E1011`: Duplicate type definition
- `E1012`: Unknown type
- `E1013`: Expected function type
- `E1014`: Expected record type
- `E1015`: Expected enum type

### Effect Errors (E2xxx)
- `E2001`: Effect not declared in function signature
- `E2002`: Unknown effect
- `E2003`: Capability not available in scope
- `E2004`: Cannot call effectful function from pure context
- `E2005`: Effect mismatch in function call
- `E2006`: Effect not mockable
- `E2007`: Invalid capability injection

### Contract Errors (E3xxx)
- `E3001`: Precondition violation (requires)
- `E3002`: Postcondition violation (ensures)
- `E3003`: Invariant violation
- `E3004`: Invalid contract expression
- `E3005`: Contract references unavailable binding

### Runtime Errors (E4xxx)
- `E4001`: Division by zero
- `E4002`: Index out of bounds
- `E4003`: Contract violation at runtime
- `E4004`: Resource limit exceeded
- `E4005`: Capability access denied
- `E4006`: Integer overflow
- `E4007`: Stack overflow
- `E4008`: Assertion failed

### Warnings (W0xxx)
- `W0001`: Unused variable
- `W0002`: Unused import
- `W0003`: Unreachable code
- `W0004`: Deprecated feature
- `W0005`: Wildcard match could be more specific
- `W0006`: Shadowed binding
- `W0007`: Redundant type annotation

## Diagnostic Structure

### Rust Types
```rust
struct Diagnostic {
    /// Stable error code
    code: ErrorCode,

    /// Severity level
    severity: Severity,

    /// Primary message (short, actionable)
    message: String,

    /// Source location
    span: Span,

    /// Additional context and explanations
    notes: Vec<Note>,

    /// Suggested fixes
    suggestions: Vec<Suggestion>,
}

enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

struct Note {
    message: String,
    span: Option<Span>,
}

struct Suggestion {
    title: String,
    edits: Vec<Edit>,
}

struct Edit {
    file: PathBuf,
    span: Span,
    replacement: String,
}
```

### JSON Format
```json
{
  "code": "E1001",
  "severity": "error",
  "message": "Type mismatch: expected `Int`, found `Text`",
  "span": {
    "file": "src/example.astra",
    "start": 100,
    "end": 110,
    "start_line": 5,
    "start_col": 10,
    "end_line": 5,
    "end_col": 20
  },
  "notes": [
    {
      "message": "Expected type comes from this annotation",
      "span": { "file": "src/example.astra", "start_line": 3, ... }
    }
  ],
  "suggestions": [
    {
      "title": "Convert to integer",
      "edits": [
        {
          "file": "src/example.astra",
          "span": { "start": 100, "end": 110, ... },
          "replacement": "text.parse_int()"
        }
      ]
    }
  ]
}
```

## Human-Readable Format

```
error[E1001]: Type mismatch: expected `Int`, found `Text`
  --> src/example.astra:5:10
   |
 3 | fn add(a: Int, b: Int) -> Int {
   |                           --- expected `Int` because of return type
 4 |   let result = a + b
 5 |   "hello"
   |   ^^^^^^^ expected `Int`, found `Text`
   |
help: convert to integer
   |
 5 |   "hello".parse_int()
   |          ++++++++++++
```

## Requirements

1. **Stability**: Error codes never change meaning
2. **Actionability**: Messages should tell users what to do
3. **Context**: Include relevant source locations
4. **Suggestions**: Provide fixes when possible
5. **Machine-readable**: JSON output for agent consumption
6. **Human-readable**: Pretty terminal output for developers

## Adding New Error Codes

1. Choose appropriate range (E0xxx, E1xxx, etc.)
2. Find next available number in range
3. Add to `docs/errors.md` with:
   - Code
   - Message template
   - Explanation
   - Example
   - Suggested fixes
4. Never reuse or change existing codes
