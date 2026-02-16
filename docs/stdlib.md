# Standard Library Reference

The Astra standard library provides core types and utility functions. It is automatically available via the prelude.

## Built-in Types

These types are built into the language and always available:

| Type | Description | Values |
|------|-------------|--------|
| `Int` | 64-bit signed integer | `0`, `42`, `-7` |
| `Bool` | Boolean | `true`, `false` |
| `Text` | UTF-8 string | `"hello"`, `""` |
| `Unit` | Empty type (no value) | Implicit |
| `Option[T]` | Optional value | `Some(value)`, `None` |
| `Result[T, E]` | Success or error | `Ok(value)`, `Err(error)` |
| `List[T]` | Ordered collection | `[1, 2, 3]`, `[]` |

## Built-in Functions

These functions are always available without imports:

| Function | Signature | Description |
|----------|-----------|-------------|
| `print(text)` | `(Text) -> Unit` | Print text without newline (requires Console) |
| `println(text)` | `(Text) -> Unit` | Print text with newline (requires Console) |
| `assert(cond)` | `(Bool) -> Unit` | Assert condition is true (test-only) |
| `assert_eq(a, b)` | `(T, T) -> Unit` | Assert two values are equal (test-only) |
| `len(collection)` | `(List[T]) -> Int` | Get length of a list |
| `to_text(value)` | `(T) -> Text` | Convert a value to its text representation |

## std.core

Core type aliases and utility functions.

**Module**: `std.core`

### Unit

```astra
type Unit = ()
```

Type alias for the empty tuple. Functions that return nothing implicitly return `Unit`.

### identity

```astra
fn identity[T](x: T) -> T
```

Returns its argument unchanged. Useful as a default transformation.

```astra
identity(42)      # => 42
identity("hello") # => "hello"
```

### const

```astra
fn const[T, U](x: T, _: U) -> T
```

Returns the first argument, ignoring the second. Useful for creating functions that always return a fixed value.

```astra
const(42, "ignored") # => 42
```

## std.option

Utility functions for working with `Option[T]` values.

**Module**: `std.option`

`Option[T]` is a built-in enum with two variants:
- `Some(value)` — a value is present
- `None` — no value

### is_some

```astra
fn is_some[T](opt: Option[T]) -> Bool
```

Returns `true` if the option contains a value.

```astra
is_some(Some(42))  # => true
is_some(None)      # => false
```

### is_none

```astra
fn is_none[T](opt: Option[T]) -> Bool
```

Returns `true` if the option is empty.

```astra
is_none(None)      # => true
is_none(Some(42))  # => false
```

### unwrap_or

```astra
fn unwrap_or[T](opt: Option[T], default: T) -> T
```

Returns the contained value, or `default` if the option is `None`.

```astra
unwrap_or(Some(42), 0)  # => 42
unwrap_or(None, 0)      # => 0
```

### map

```astra
fn map[T, U](opt: Option[T], f: (T) -> U) -> Option[U]
```

Transforms the contained value using `f`, or returns `None` if empty.

```astra
map(Some(5), fn(x) { x * 2 })  # => Some(10)
map(None, fn(x) { x * 2 })     # => None
```

## std.result

Utility functions for working with `Result[T, E]` values.

**Module**: `std.result`

`Result[T, E]` is a built-in enum with two variants:
- `Ok(value)` — operation succeeded
- `Err(error)` — operation failed

### is_ok

```astra
fn is_ok[T, E](res: Result[T, E]) -> Bool
```

Returns `true` if the result is `Ok`.

```astra
is_ok(Ok(42))           # => true
is_ok(Err("failed"))    # => false
```

### is_err

```astra
fn is_err[T, E](res: Result[T, E]) -> Bool
```

Returns `true` if the result is `Err`.

```astra
is_err(Err("failed"))   # => true
is_err(Ok(42))          # => false
```

### unwrap_or

```astra
fn unwrap_or[T, E](res: Result[T, E], default: T) -> T
```

Returns the success value, or `default` if the result is `Err`.

```astra
unwrap_or(Ok(42), 0)        # => 42
unwrap_or(Err("fail"), 0)   # => 0
```

### map

```astra
fn map[T, U, E](res: Result[T, E], f: (T) -> U) -> Result[U, E]
```

Transforms the success value using `f`, or passes through the error unchanged.

```astra
map(Ok(5), fn(x) { x * 2 })        # => Ok(10)
map(Err("fail"), fn(x) { x * 2 })  # => Err("fail")
```

### map_err

```astra
fn map_err[T, E, F](res: Result[T, E], f: (E) -> F) -> Result[T, F]
```

Transforms the error value using `f`, or passes through the success value unchanged.

```astra
map_err(Err("fail"), fn(e) { "Error: " + e })  # => Err("Error: fail")
map_err(Ok(42), fn(e) { "Error: " + e })        # => Ok(42)
```

## std.list

Utility functions for working with `List[T]` values.

**Module**: `std.list`

### is_empty

```astra
fn is_empty[T](list: List[T]) -> Bool
```

Returns `true` if the list has no elements.

```astra
is_empty([])       # => true
is_empty([1, 2])   # => false
```

### head

```astra
fn head[T](list: List[T]) -> Option[T]
```

Returns the first element of the list, or `None` if the list is empty.

```astra
head([1, 2, 3])   # => Some(1)
head([])           # => None
```

## Error Propagation Operators

These are built-in operators for working with `Option` and `Result`:

### The `?` operator

Propagates `None` or `Err` to the calling function's return value:

```astra
fn get_name(id: Int) -> Option[Text] {
  let user = find_user(id)?   # Returns None if find_user returns None
  Some(user.name)
}

fn read_config(path: Text) -> Result[Config, Text]
  effects(Fs)
{
  let content = Fs.read(path)?   # Returns Err if read fails
  parse_config(content)?
}
```

### The `?else` operator

Provides a fallback value when `?` would propagate an error:

```astra
fn get_name_or_default(id: Int) -> Text {
  let user = find_user(id) ?else { name = "Anonymous" }
  user.name
}
```

## Prelude

The prelude (`std.prelude`) automatically imports the core modules:

```astra
import std.option
import std.result
import std.core
```

You do not need to explicitly import these — they are available in every Astra file.
