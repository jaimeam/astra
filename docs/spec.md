# Astra Language Specification (v0.1 Draft)

> This document defines the syntax and semantics of the Astra programming language.

## 1. Lexical Structure

### 1.1 Character Set

Astra source files are UTF-8 encoded text.

### 1.2 Whitespace and Comments

```
whitespace := ' ' | '\t' | '\n' | '\r'

line_comment := '#' <any character except newline>* '\n'
doc_comment := '##' <any character except newline>* '\n'
```

### 1.3 Identifiers

```
identifier := (letter | '_') (letter | digit | '_')*
letter := 'a'..'z' | 'A'..'Z'
digit := '0'..'9'
```

Reserved keywords cannot be used as identifiers:
```
and, as, assert, bool, else, effects, ensures, enum, false,
fn, for, if, import, in, Int, invariant, let, match, module,
mut, not, Option, or, property, public, requires, Result,
return, test, Text, then, true, type, Unit, using, where
```

### 1.4 Literals

```
int_literal := digit+
bool_literal := 'true' | 'false'
text_literal := '"' <string_char>* '"'
string_char := <any char except '"' or '\\'> | escape_sequence
escape_sequence := '\\' ('n' | 'r' | 't' | '\\' | '"')
```

### 1.5 Operators and Punctuation

```
operators := '+' | '-' | '*' | '/' | '%' | '==' | '!=' | '<' | '>' | '<=' | '>='
           | 'and' | 'or' | 'not' | '?' | '?else'

punctuation := '(' | ')' | '{' | '}' | '[' | ']' | ',' | ':' | '=' | '->' | '=>' | '|'
```

## 2. Grammar

### 2.1 Modules

```
module := 'module' module_path item*
module_path := identifier ('.' identifier)*

item := import_decl | type_def | enum_def | fn_def | test_block | property_block
```

### 2.2 Imports

```
import_decl := 'import' module_path ('as' identifier)?
             | 'import' module_path '.{' identifier (',' identifier)* '}'
```

### 2.3 Type Definitions

```
type_def := 'type' identifier type_params? '=' type_expr invariant_clause?
type_params := '[' identifier (',' identifier)* ']'
invariant_clause := 'invariant' expr

type_expr := named_type | record_type | function_type
named_type := identifier type_args?
type_args := '[' type_expr (',' type_expr)* ']'
record_type := '{' field_def (',' field_def)* '}'
field_def := identifier ':' type_expr
function_type := '(' type_expr (',' type_expr)* ')' '->' type_expr effects_clause?
```

### 2.4 Enum Definitions

```
enum_def := 'enum' identifier type_params? '=' variant ('|' variant)*
variant := identifier variant_data?
variant_data := '(' field_def (',' field_def)* ')'
```

### 2.5 Function Definitions

```
fn_def := visibility? 'fn' identifier '(' params? ')' return_type? effects_clause? contract_clauses? block

visibility := 'public'
params := param (',' param)*
param := identifier ':' type_expr
return_type := '->' type_expr
effects_clause := 'effects' '(' identifier (',' identifier)* ')'
contract_clauses := requires_clause* ensures_clause*
requires_clause := 'requires' expr
ensures_clause := 'ensures' expr
```

### 2.6 Statements

```
stmt := let_stmt | assign_stmt | return_stmt | expr_stmt

let_stmt := 'let' 'mut'? identifier type_annotation? '=' expr
type_annotation := ':' type_expr
assign_stmt := expr '=' expr
return_stmt := 'return' expr?
expr_stmt := expr
```

### 2.7 Expressions

```
expr := or_expr

or_expr := and_expr ('or' and_expr)*
and_expr := cmp_expr ('and' cmp_expr)*
cmp_expr := add_expr (cmp_op add_expr)?
cmp_op := '==' | '!=' | '<' | '>' | '<=' | '>='
add_expr := mul_expr (('+' | '-') mul_expr)*
mul_expr := unary_expr (('*' | '/' | '%') unary_expr)*
unary_expr := 'not' unary_expr | postfix_expr
postfix_expr := primary_expr (call_args | field_access | method_call | try_expr)*
call_args := '(' (expr (',' expr)*)? ')'
field_access := '.' identifier
method_call := '.' identifier call_args
try_expr := '?' | '?else' expr

primary_expr := int_literal | bool_literal | text_literal
              | identifier | qualified_name
              | record_expr | enum_expr
              | if_expr | match_expr | block
              | '(' expr ')'

qualified_name := identifier '.' identifier
record_expr := '{' field_init (',' field_init)* '}'
field_init := identifier '=' expr
enum_expr := identifier ('(' expr ')')?

if_expr := 'if' expr block ('else' (if_expr | block))?
match_expr := 'match' expr '{' match_arm (',' match_arm)* '}'
match_arm := pattern '=>' expr
block := '{' stmt* expr? '}'
```

### 2.8 Patterns

```
pattern := '_' | identifier | literal_pattern | record_pattern | enum_pattern
literal_pattern := int_literal | bool_literal | text_literal
record_pattern := '{' field_pattern (',' field_pattern)* '}'
field_pattern := identifier ('=' pattern)?
enum_pattern := identifier ('(' pattern ')')?
```

### 2.9 Tests and Properties

```
test_block := 'test' text_literal using_clause? block
property_block := 'property' text_literal using_clause? block

using_clause := 'using' 'effects' '(' effect_binding (',' effect_binding)* ')'
effect_binding := identifier '=' expr
```

## 3. Type System

### 3.1 Built-in Types

| Type | Description |
|------|-------------|
| `Int` | 64-bit signed integer |
| `Bool` | Boolean (true/false) |
| `Text` | UTF-8 string |
| `Unit` | Unit type (empty tuple) |
| `Option[T]` | Optional value |
| `Result[T, E]` | Success or error |

### 3.2 Type Inference

- Type inference is performed within function bodies
- Public function signatures require explicit type annotations
- Local variables can omit type annotations when inferrable

### 3.3 Type Checking Rules

1. All expressions have a type
2. Function arguments must match parameter types exactly
3. Match expressions must be exhaustive over enum variants
4. The `?` operator requires `Option` or `Result` type
5. Effects must be declared in function signature

## 4. Effects System

### 4.1 Built-in Effects

| Effect | Capability Module | Description |
|--------|-------------------|-------------|
| `Net` | `capabilities.net` | Network I/O |
| `Fs` | `capabilities.fs` | Filesystem |
| `Clock` | `capabilities.clock` | Time access |
| `Rand` | `capabilities.rand` | Randomness |
| `Env` | `capabilities.env` | Environment |
| `Console` | `capabilities.console` | Console I/O |

### 4.2 Effect Rules

1. Functions are pure by default
2. Effectful operations require declared effects
3. Callers must declare all effects of callees
4. Effects can be injected in tests

## 5. Contracts

### 5.1 Preconditions

```astra
fn divide(a: Int, b: Int) -> Int
  requires b != 0
{
  a / b
}
```

### 5.2 Postconditions

```astra
fn abs(n: Int) -> Int
  ensures result >= 0
{
  if n < 0 { -n } else { n }
}
```

### 5.3 Type Invariants

```astra
type PositiveInt = Int
  invariant self > 0
```

## 6. Standard Library

See `stdlib/` for the standard library implementation.

### 6.1 Core Types

- `Option[T]`: `Some(T)` | `None`
- `Result[T, E]`: `Ok(T)` | `Err(E)`

### 6.2 Core Functions

- `assert(condition: Bool)` - Assert condition is true
- `assert_eq(a: T, b: T)` - Assert values are equal

## 7. Evaluation Semantics

### 7.1 Evaluation Order

- Expressions are evaluated left-to-right
- Function arguments are evaluated before the call
- Short-circuit evaluation for `and` and `or`

### 7.2 Pattern Matching

- Patterns are matched top-to-bottom
- First matching arm is executed
- Non-exhaustive matches are compile errors

### 7.3 Error Propagation

- `?` on `None` returns `None` from function
- `?` on `Err(e)` returns `Err(e)` from function
- `?else expr` provides fallback on failure

## 8. Diagnostics and Linting

### 8.1 Diagnostic Model

All compiler output uses structured diagnostics with:
- Stable error code (`E####` for errors, `W####` for warnings)
- Severity level: `error`, `warning`, `info`, `hint`
- Source span with file, line, and column
- Optional notes and suggested fixes
- JSON output via `--json` flag

### 8.2 Built-in Lint Checks

The type checker emits warnings for common issues. Warnings do not prevent compilation unless `--strict` mode is enabled.

| Code | Description |
|------|-------------|
| W0001 | Unused variable (suppress with `_` prefix) |
| W0002 | Unused import |
| W0003 | Unreachable code after `return` |
| W0004 | Deprecated feature (reserved) |
| W0005 | Wildcard pattern on known exhaustive type |
| W0006 | Shadowed binding in same scope |
| W0007 | Redundant type annotation (reserved) |

### 8.3 Strict Mode

`astra check --strict` treats all warnings as errors. The check exits with a non-zero status if any warnings are present. This is intended for CI and production use.

Strictness can also be configured in `astra.toml`:
```toml
[lint]
level = "deny"
```
