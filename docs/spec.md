# Astra Language Specification (v1.0)

> This document defines the syntax and semantics of the Astra programming language.
> For the formal grammar, see [grammar.md](grammar.md).

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

Reserved keywords:
```
and, as, assert, async, await, break, continue, effect, else,
effects, ensures, enum, false, fn, for, forall, if, impl, import,
in, invariant, let, match, module, mut, not, or, property, public,
requires, return, test, then, trait, true, type, using, while
```

### 1.4 Literals

```
int_literal    := digit+
float_literal  := digit+ '.' digit+
bool_literal   := 'true' | 'false'
text_literal   := '"' string_char* '"'
multiline_text := '"""' <any>* '"""'
string_char    := <any char except '"' or '\\'> | escape_sequence
escape_sequence := '\\' ('n' | 'r' | 't' | '\\' | '"' | '0' | '$')
unit_literal   := '(' ')'
```

### 1.5 Operators and Punctuation

```
operators   := '+' | '-' | '*' | '/' | '%'
             | '==' | '!=' | '<' | '>' | '<=' | '>='
             | 'and' | 'or' | 'not'
             | '?' | '?else' | '|>'
             | '+=' | '-=' | '*=' | '/=' | '%='

punctuation := '(' | ')' | '{' | '}' | '[' | ']'
             | ',' | ':' | '=' | '->' | '=>' | '|' | '.'
             | '..' | '..='
```

## 2. Module System

### 2.1 Module Declaration

Every Astra source file begins with a module declaration:

```astra
module examples.mymodule
```

### 2.2 Imports

```astra
import foo.bar                     ## Whole module
import foo.bar as Alias            ## Aliased import
import foo.bar.{A, B, C}          ## Selective import
public import foo.bar              ## Re-export
```

Standard library modules are imported via `std.*`:
```astra
import std.datetime
import std.path.{basename, extension}
```

## 3. Type System

### 3.1 Built-in Types

| Type | Description | Default value |
|------|-------------|---------------|
| `Int` | 64-bit signed integer | `0` |
| `Float` | 64-bit floating point | `0.0` |
| `Bool` | Boolean | `false` |
| `Text` | UTF-8 string | `""` |
| `Unit` | Unit type (empty tuple) | `()` |

### 3.2 Compound Types

| Type | Description | Literal syntax |
|------|-------------|----------------|
| `List[T]` | Ordered collection | `[1, 2, 3]` |
| `Tuple` | Fixed-size mixed collection | `(1, "hello", true)` |
| `Map[K, V]` | Key-value pairs (sorted) | `Map.from([(k, v)])` |
| `Set[T]` | Unique values (sorted) | `Set.from([1, 2, 3])` |
| `Record` | Named fields | `{ name = "Alice", age = 30 }` |
| `Option[T]` | Optional value: `Some(T)` or `None` | `Some(42)`, `None` |
| `Result[T, E]` | Success or error: `Ok(T)` or `Err(E)` | `Ok(42)`, `Err("fail")` |
| `(T) -> U` | Function type | `fn(x: Int) -> Int { x + 1 }` |

### 3.3 User-Defined Types

#### Type Aliases
```astra
type UserId = Int
type Percentage = Int
  invariant self >= 0 and self <= 100
```

#### Enums
```astra
enum Shape =
  | Circle(radius: Float)
  | Rectangle(width: Float, height: Float)
  | Point
```

#### Traits
```astra
trait Describable {
  fn describe(self: Text) -> Text
}

impl Describable for Int {
  fn describe(self: Text) -> Text { "an integer" }
}
```

### 3.4 Generics

Functions and types support type parameters:

```astra
fn identity[T](x: T) -> T { x }
fn apply[T, U](x: T, f: (T) -> U) -> U { f(x) }
fn show[T: Show](x: T) -> Text { x.describe() }  ## With trait bound
```

### 3.5 Type Inference

- Public function signatures require explicit type annotations
- Local variables can omit type annotations when inferrable
- Lambda parameters can omit types: `fn(x) { x + 1 }`
- Constraint-based unification resolves generic type parameters

## 4. Expressions

### 4.1 Operator Precedence (lowest to highest)

| Precedence | Operators | Associativity |
|-----------|-----------|---------------|
| 1 | `\|>` (pipe) | Left |
| 2 | `or` | Left |
| 3 | `and` | Left |
| 4 | `==`, `!=`, `<`, `>`, `<=`, `>=` | None |
| 5 | `+`, `-` | Left |
| 6 | `*`, `/`, `%` | Left |
| 7 | `not`, `-` (negation) | Prefix |
| 8 | `.` (field/method), `[i]` (index), `?`, `?else` | Postfix |

### 4.2 Control Flow

```astra
## If expression (always produces a value)
let x = if cond { a } else { b }

## Match expression with exhaustiveness checking
match shape {
  Circle(r) => 3.14 * r * r
  Rectangle(w, h) => w * h
  Point => 0.0
}

## Match with guard clauses
match n {
  x if x > 0 => "positive"
  x if x < 0 => "negative"
  _ => "zero"
}

## For-in loop
for item in list { process(item) }
for (key, value) in pairs { println("${key}: ${value}") }

## While loop
while condition { body }

## Break, continue, return
break
continue
return value
```

### 4.3 String Interpolation

```astra
let name = "world"
let greeting = "Hello, ${name}!"
let computed = "sum = ${a + b}"
```

Escape sequences: `\n`, `\r`, `\t`, `\\`, `\"`, `\0`, `\$`

### 4.4 Range Expressions

```astra
let exclusive = 0..10    ## [0, 1, 2, ..., 9]
let inclusive = 0..=10   ## [0, 1, 2, ..., 10]
```

### 4.5 Index Access

```astra
list[0]        ## List indexing (supports negative: list[-1])
map[key]       ## Map lookup (error if key not found)
text[i]        ## Character indexing
tuple.0        ## Tuple field access
```

### 4.6 Pipe Operator

```astra
data |> transform |> validate |> save
## Equivalent to: save(validate(transform(data)))
```

### 4.7 Error Propagation

```astra
let value = maybe_none()?         ## Propagates None to caller
let value = maybe_err()?          ## Propagates Err to caller
let value = risky()?else default  ## Use default on failure
```

### 4.8 Lambdas

```astra
fn(x: Int) -> Int { x * 2 }
fn(x) { x * 2 }                   ## Type inference
list.map(fn(x) { x + 1 })
```

### 4.9 Hole Expression

```astra
let x = ???   ## Placeholder; type-checks but errors at runtime (E4013)
```

## 5. Statements

```astra
let x = 42                         ## Immutable binding
let mut y = 0                      ## Mutable binding
let { name, age } = person         ## Destructuring
y = y + 1                          ## Assignment (mutable only)
y += 1                             ## Compound assignment (+=, -=, *=, /=, %=)
return value                       ## Early return
```

## 6. Function Definitions

```astra
public fn divide(a: Int, b: Int) -> Int
  effects(Console)
  requires b != 0
  ensures result >= 0
{
  Console.println("dividing")
  a / b
}
```

Components:
- `public` — visibility modifier (default is private)
- Type parameters: `fn identity[T](x: T) -> T`
- `effects(...)` — declares required capabilities
- `requires` — precondition (checked before execution)
- `ensures` — postcondition (`result` refers to return value)

## 7. Effects System

### 7.1 Built-in Effects

| Effect | Methods |
|--------|---------|
| `Console` | `print(text)`, `println(text)`, `read_line()` |
| `Fs` | `read(path)`, `write(path, content)`, `exists(path)` |
| `Net` | `get(url)`, `post(url, body)`, `serve(port, handler)` |
| `Clock` | `now()`, `today()`, `sleep(millis)` |
| `Rand` | `int(min, max)`, `bool()`, `float()` |
| `Env` | `get(name)`, `args()` |

### 7.2 Effect Rules

1. **Pure by default** — functions without `effects(...)` cannot use I/O
2. **Declaration required** — effectful operations require declared effects
3. **Transitive propagation** — callers must declare all effects of callees
4. **Testable** — effects can be mocked in test blocks with `using effects(...)`

### 7.3 User-Defined Effects

```astra
effect Logger {
  fn log(msg: Text) -> Unit
}
```

### 7.4 Deterministic Testing

```astra
test "seeded random" using effects(Rand = Rand.seeded(42)) {
  let x = Rand.int(1, 100)
  assert(x > 0)
}

test "fixed clock" using effects(Clock = Clock.fixed(1000)) {
  assert_eq(Clock.now(), 1000)
}
```

## 8. Contracts

### 8.1 Preconditions

```astra
fn divide(a: Int, b: Int) -> Int
  requires b != 0
{
  a / b
}
```

Violated preconditions produce error E3001.

### 8.2 Postconditions

```astra
fn abs(n: Int) -> Int
  ensures result >= 0
{
  if n < 0 { 0 - n } else { n }
}
```

Violated postconditions produce error E3002.

### 8.3 Type Invariants

```astra
type PositiveInt = Int
  invariant self > 0
```

Violated invariants produce error E3003.

## 9. Pattern Matching

### 9.1 Pattern Types

| Pattern | Example | Binds |
|---------|---------|-------|
| Wildcard | `_` | Nothing |
| Identifier | `x` | Value to `x` |
| Literal | `42`, `true`, `"hello"` | Nothing |
| Record | `{ name, age }` | Fields to variables |
| Variant | `Some(x)`, `None` | Inner value |
| Tuple | `(a, b, c)` | Elements to variables |
| Guard | `x if x > 0` | Value to `x` if guard passes |

### 9.2 Exhaustiveness

Match expressions on known types (Option, Result, Bool, user enums) must
cover all variants. Missing cases produce error E1004.

### 9.3 Destructuring

Patterns work in `let` bindings and `for` loops:

```astra
let { name, age } = person
let (x, y) = point
for (key, value) in map.entries() { ... }
```

## 10. Testing

```astra
test "addition" {
  assert_eq(1 + 1, 2)
}

property "list reverse is involutory"
  using effects(Rand = Rand.seeded(42))
{
  let xs = [Rand.int(0, 100), Rand.int(0, 100), Rand.int(0, 100)]
  assert_eq(xs.reverse().reverse(), xs)
}
```

## 11. Diagnostics and Linting

### 11.1 Error Code Categories

| Range | Category | Example |
|-------|----------|---------|
| E0xxx | Syntax/parsing | E0001: Unexpected token |
| E1xxx | Type errors | E1001: Type mismatch |
| E2xxx | Effect errors | E2001: Effect not declared |
| E3xxx | Contract violations | E3001: Precondition violated |
| E4xxx | Runtime errors | E4003: Division by zero |
| W0xxx | Warnings | W0001: Unused variable |

### 11.2 Built-in Lint Checks

| Code | Description |
|------|-------------|
| W0001 | Unused variable (suppress with `_` prefix) |
| W0002 | Unused import |
| W0003 | Unreachable code after `return` |
| W0005 | Wildcard pattern on known exhaustive type |
| W0006 | Shadowed binding in same scope |
| W0008 | Unused private function |

### 11.3 Strict Mode

`astra check --strict` treats all warnings as errors.

## 12. Standard Library

See [stdlib.md](stdlib.md) for the complete API reference.

15 modules: `std.core`, `std.prelude`, `std.option`, `std.result`, `std.error`,
`std.list`, `std.collections`, `std.iter`, `std.string`, `std.math`, `std.io`,
`std.json`, `std.regex`, `std.datetime`, `std.path`.

## 13. Future Work (v1.1)

The following features are syntactically reserved but not included in v1.0:

- **Async/await** — `async fn` and `await` expressions
- **Package manager** — `astra pkg` for dependency management

See [ADR-007](adr/ADR-007-defer-async-pkg-to-v1.1.md) for rationale.
