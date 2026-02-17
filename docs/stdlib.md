# Standard Library Reference

The Astra standard library provides core types, built-in functions, and utility modules. Built-in types and functions are always available without imports. Standard library modules can be imported with `import std.<module>`.

---

## Built-in Types

These types are built into the language and always available:

| Type | Description | Literal Syntax |
|------|-------------|----------------|
| `Int` | 64-bit signed integer | `0`, `42`, `-7` |
| `Float` | 64-bit floating point | `3.14`, `0.0`, `-1.5` |
| `Bool` | Boolean | `true`, `false` |
| `Text` | UTF-8 string | `"hello"`, `""`, `"line\n"` |
| `Unit` | Empty type (no value) | `()` (implicit) |
| `Option[T]` | Optional value | `Some(value)`, `None` |
| `Result[T, E]` | Success or error | `Ok(value)`, `Err(error)` |
| `List[T]` | Ordered collection | `[1, 2, 3]`, `[]` |
| `Tuple` | Fixed-size heterogeneous collection | `(1, "hello", true)` |
| `Map[K, V]` | Key-value collection | `Map.new()`, `Map.from([(k, v)])` |
| `Set[T]` | Unique value collection | `Set.new()`, `Set.from([1, 2, 3])` |

---

## Built-in Functions

These functions are always available without imports.

### I/O

| Function | Signature | Description |
|----------|-----------|-------------|
| `print(values...)` | `(...) -> Unit` | Print values to stdout without newline (requires Console) |
| `println(values...)` | `(...) -> Unit` | Print values to stdout with newline (requires Console) |

### Assertions (test-only)

| Function | Signature | Description |
|----------|-----------|-------------|
| `assert(cond)` | `(Bool) -> Unit` | Assert condition is true |
| `assert(cond, msg)` | `(Bool, Text) -> Unit` | Assert with custom error message |
| `assert_eq(a, b)` | `(T, T) -> Unit` | Assert two values are equal |

### Collections

| Function | Signature | Description |
|----------|-----------|-------------|
| `len(value)` | `(Text \| List \| Tuple \| Map \| Set) -> Int` | Returns length/size |
| `range(start, end)` | `(Int, Int) -> List[Int]` | Creates list of integers from start (inclusive) to end (exclusive) |

### Type Conversion

| Function | Signature | Description |
|----------|-----------|-------------|
| `to_text(value)` | `(T) -> Text` | Convert any value to its text representation |
| `to_int(value)` | `(Float \| Text \| Bool) -> Int` or `Option[Int]` | Convert to Int (Text returns Option) |
| `to_float(value)` | `(Int \| Text) -> Float` or `Option[Float]` | Convert to Float (Text returns Option) |

### Math

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs(x)` | `(Int \| Float) -> Int \| Float` | Absolute value |
| `min(a, b)` | `(Int, Int) -> Int` or `(Float, Float) -> Float` | Minimum of two values |
| `max(a, b)` | `(Int, Int) -> Int` or `(Float, Float) -> Float` | Maximum of two values |
| `pow(base, exp)` | `(Int \| Float, Int \| Float) -> Int \| Float` | Raise base to power |
| `sqrt(x)` | `(Float \| Int) -> Float` | Square root |
| `floor(x)` | `(Float) -> Float` | Round down to nearest integer |
| `ceil(x)` | `(Float) -> Float` | Round up to nearest integer |
| `round(x)` | `(Float) -> Float` | Round to nearest integer |

### Option/Result Constructors

| Function | Signature | Description |
|----------|-----------|-------------|
| `Some(value)` | `(T) -> Option[T]` | Wrap value in Some |
| `None` | `Option[T]` | Empty option value |
| `Ok(value)` | `(T) -> Result[T, E]` | Wrap value in Ok |
| `Err(error)` | `(E) -> Result[T, E]` | Wrap error in Err |

### Effect Convenience Wrappers

These provide direct access to common effect operations. They require the corresponding effect to be declared.

| Function | Signature | Effect | Description |
|----------|-----------|--------|-------------|
| `read_file(path)` | `(Text) -> Result[Text, Text]` | Fs | Read file contents |
| `write_file(path, content)` | `(Text, Text) -> Result[Unit, Text]` | Fs | Write file contents |
| `http_get(url)` | `(Text) -> Result[Text, Text]` | Net | HTTP GET request |
| `http_post(url, body)` | `(Text, Text) -> Result[Text, Text]` | Net | HTTP POST request |
| `random_int(min, max)` | `(Int, Int) -> Int` | Rand | Random integer in [min, max) |
| `random_bool()` | `() -> Bool` | Rand | Random boolean |
| `current_time_millis()` | `() -> Int` | Clock | Current time in milliseconds |
| `get_env(name)` | `(Text) -> Option[Text]` | Env | Get environment variable |

---

## Built-in Methods by Type

### Text Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `.len()` | `() -> Int` | Length in characters |
| `.to_upper()` | `() -> Text` | Uppercase version |
| `.to_lower()` | `() -> Text` | Lowercase version |
| `.trim()` | `() -> Text` | Remove leading/trailing whitespace |
| `.contains(needle)` | `(Text) -> Bool` | Check if contains substring |
| `.starts_with(prefix)` | `(Text) -> Bool` | Check if starts with prefix |
| `.ends_with(suffix)` | `(Text) -> Bool` | Check if ends with suffix |
| `.split(delimiter)` | `(Text) -> List[Text]` | Split by delimiter |
| `.replace(from, to)` | `(Text, Text) -> Text` | Replace all occurrences |
| `.chars()` | `() -> List[Text]` | List of individual characters |
| `.repeat(n)` | `(Int) -> Text` | Repeat string n times |
| `.index_of(needle)` | `(Text) -> Option[Int]` | Position of first occurrence, or None |
| `.substring(start, end)` | `(Int, Int) -> Text` | Substring from start (inclusive) to end (exclusive) |

```astra
let name = "Hello, World!"
name.len()                    # => 13
name.to_upper()               # => "HELLO, WORLD!"
name.to_lower()               # => "hello, world!"
name.contains("World")        # => true
name.starts_with("Hello")     # => true
name.split(", ")              # => ["Hello", "World!"]
name.replace("World", "Astra") # => "Hello, Astra!"
name.substring(0, 5)          # => "Hello"
name.index_of("World")        # => Some(7)
"abc".chars()                  # => ["a", "b", "c"]
"ha".repeat(3)                 # => "hahaha"
```

### List[T] Methods

#### Basic Operations

| Method | Signature | Description |
|--------|-----------|-------------|
| `.len()` | `() -> Int` | Number of elements |
| `.is_empty()` | `() -> Bool` | True if list has no elements |
| `.get(index)` | `(Int) -> Option[T]` | Element at index, or None |
| `.contains(value)` | `(T) -> Bool` | True if list contains value |
| `.head()` | `() -> Option[T]` | First element, or None |
| `.last()` | `() -> Option[T]` | Last element, or None |

#### Transformation

| Method | Signature | Description |
|--------|-----------|-------------|
| `.push(value)` | `(T) -> List[T]` | New list with value appended |
| `.concat(other)` | `(List[T]) -> List[T]` | New list with other appended |
| `.tail()` | `() -> List[T]` | All elements except first |
| `.reverse()` | `() -> List[T]` | Reversed list |
| `.sort()` | `() -> List[T]` | Sorted list |
| `.take(n)` | `(Int) -> List[T]` | First n elements |
| `.drop(n)` | `(Int) -> List[T]` | All except first n elements |
| `.slice(start, end)` | `(Int, Int) -> List[T]` | Sublist from start to end |
| `.enumerate()` | `() -> List[{index: Int, value: T}]` | List of records with index and value |
| `.zip(other)` | `(List[U]) -> List[{first: T, second: U}]` | Pair elements from two lists |
| `.join(separator)` | `(Text) -> Text` | Join elements as text with separator |

#### Higher-Order Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `.map(f)` | `((T) -> U) -> List[U]` | Apply function to each element |
| `.filter(f)` | `((T) -> Bool) -> List[T]` | Keep elements matching predicate |
| `.fold(init, f)` | `(U, (U, T) -> U) -> U` | Left fold with accumulator |
| `.each(f)` | `((T) -> Unit) -> Unit` | Execute function on each element |
| `.any(f)` | `((T) -> Bool) -> Bool` | True if any element matches |
| `.all(f)` | `((T) -> Bool) -> Bool` | True if all elements match |
| `.flat_map(f)` | `((T) -> List[U]) -> List[U]` | Map and flatten results |
| `.find(f)` | `((T) -> Bool) -> Option[T]` | First element matching predicate |

```astra
let nums = [3, 1, 4, 1, 5]
nums.len()                     # => 5
nums.head()                    # => Some(3)
nums.sort()                    # => [1, 1, 3, 4, 5]
nums.filter(fn(x) { x > 2 })  # => [3, 4, 5]
nums.map(fn(x) { x * 2 })     # => [6, 2, 8, 2, 10]
nums.fold(0, fn(acc, x) { acc + x })  # => 14
nums.any(fn(x) { x > 4 })     # => true
nums.find(fn(x) { x == 4 })   # => Some(4)
[1, 2, 3].zip([4, 5, 6])      # => [{first: 1, second: 4}, ...]
["a", "b", "c"].join(", ")    # => "a, b, c"
```

### Option[T] Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `.is_some()` | `() -> Bool` | True if contains a value |
| `.is_none()` | `() -> Bool` | True if empty |
| `.unwrap()` | `() -> T` | Returns value; **errors if None** |
| `.unwrap_or(default)` | `(T) -> T` | Returns value, or default if None |
| `.map(f)` | `((T) -> U) -> Option[U]` | Transform contained value |

```astra
let x = Some(42)
x.is_some()           # => true
x.unwrap()            # => 42
x.unwrap_or(0)        # => 42
x.map(fn(v) { v * 2 }) # => Some(84)

let y: Option[Int] = None
y.is_none()           # => true
y.unwrap_or(0)        # => 0
y.map(fn(v) { v * 2 }) # => None
```

### Result[T, E] Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `.is_ok()` | `() -> Bool` | True if Ok |
| `.is_err()` | `() -> Bool` | True if Err |
| `.unwrap()` | `() -> T` | Returns value; **errors if Err** |
| `.unwrap_or(default)` | `(T) -> T` | Returns value, or default if Err |
| `.map(f)` | `((T) -> U) -> Result[U, E]` | Transform success value |
| `.map_err(f)` | `((E) -> F) -> Result[T, F]` | Transform error value |

```astra
let ok = Ok(42)
ok.is_ok()                  # => true
ok.unwrap()                 # => 42
ok.map(fn(v) { v + 1 })    # => Ok(43)

let err = Err("failed")
err.is_err()                # => true
err.unwrap_or(0)            # => 0
err.map_err(fn(e) { "Error: " + e })  # => Err("Error: failed")
```

### Tuple Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `.len()` | `() -> Int` | Number of elements |
| `.to_list()` | `() -> List` | Convert to list |

Tuple fields are accessed by index:

```astra
let point = (10, 20, 30)
point.0     # => 10
point.1     # => 20
point.len() # => 3
```

### Map[K, V] Methods

#### Static Constructors

| Method | Signature | Description |
|--------|-----------|-------------|
| `Map.new()` | `() -> Map[K, V]` | Create empty map |
| `Map.from(pairs)` | `(List[(K, V)]) -> Map[K, V]` | Create from list of key-value tuples |

#### Instance Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `.len()` | `() -> Int` | Number of key-value pairs |
| `.is_empty()` | `() -> Bool` | True if map has no entries |
| `.get(key)` | `(K) -> V` | Get value by key (returns 0/empty for missing keys) |
| `.contains_key(key)` | `(K) -> Bool` | True if key exists |
| `.set(key, value)` | `(K, V) -> Map[K, V]` | New map with key-value added/updated |
| `.remove(key)` | `(K) -> Map[K, V]` | New map with key removed |
| `.keys()` | `() -> List[K]` | List of all keys |
| `.values()` | `() -> List[V]` | List of all values |
| `.entries()` | `() -> List[(K, V)]` | List of key-value tuples |

```astra
let m = Map.from([("a", 1), ("b", 2)])
m.get("a")          # => 1
m.contains_key("b") # => true
m.keys()            # => ["a", "b"]
m.set("c", 3)       # => Map with a=1, b=2, c=3
m.remove("a")       # => Map with b=2
m.len()             # => 2
```

### Set[T] Methods

#### Static Constructors

| Method | Signature | Description |
|--------|-----------|-------------|
| `Set.new()` | `() -> Set[T]` | Create empty set |
| `Set.from(items)` | `(List[T]) -> Set[T]` | Create from list (deduplicates) |

#### Instance Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `.len()` | `() -> Int` | Number of elements |
| `.is_empty()` | `() -> Bool` | True if set has no elements |
| `.contains(value)` | `(T) -> Bool` | True if value is in set |
| `.add(value)` | `(T) -> Set[T]` | New set with value added |
| `.remove(value)` | `(T) -> Set[T]` | New set with value removed |
| `.to_list()` | `() -> List[T]` | Convert to list |
| `.union(other)` | `(Set[T]) -> Set[T]` | Union of two sets |
| `.intersection(other)` | `(Set[T]) -> Set[T]` | Intersection of two sets |

```astra
let s = Set.from([1, 2, 3, 2, 1])
s.len()                # => 3 (duplicates removed)
s.contains(2)          # => true
s.add(4).len()         # => 4
s.union(Set.from([3, 4, 5]))         # => {1, 2, 3, 4, 5}
s.intersection(Set.from([2, 3, 4]))  # => {2, 3}
```

---

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

---

## Effects (Capabilities)

Effects represent I/O capabilities that functions must declare. See `docs/effects.md` for the full guide.

### Console

```astra
fn greet(name: Text) effects(Console) {
  Console.println("Hello, ${name}!")
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `Console.print(text)` | `(Text) -> Unit` | Print without newline |
| `Console.println(text)` | `(Text) -> Unit` | Print with newline |
| `Console.read_line()` | `() -> Option[Text]` | Read line from stdin |

### Fs (File System)

```astra
fn load(path: Text) -> Result[Text, Text] effects(Fs) {
  Fs.read(path)
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `Fs.read(path)` | `(Text) -> Result[Text, Text]` | Read file contents |
| `Fs.write(path, content)` | `(Text, Text) -> Result[Unit, Text]` | Write file contents |
| `Fs.exists(path)` | `(Text) -> Bool` | Check if file exists |

### Net (Network)

| Method | Signature | Description |
|--------|-----------|-------------|
| `Net.get(url)` | `(Text) -> Result[Text, Text]` | HTTP GET request |
| `Net.post(url, body)` | `(Text, Text) -> Result[Text, Text]` | HTTP POST request |

### Clock

| Method | Signature | Description |
|--------|-----------|-------------|
| `Clock.now()` | `() -> Int` | Current time in milliseconds |
| `Clock.sleep(millis)` | `(Int) -> Unit` | Sleep for milliseconds |

### Rand (Random)

| Method | Signature | Description |
|--------|-----------|-------------|
| `Rand.int(min, max)` | `(Int, Int) -> Int` | Random integer in [min, max) |
| `Rand.bool()` | `() -> Bool` | Random boolean |
| `Rand.float()` | `() -> Float` | Random float in [0.0, 1.0) |

### Env (Environment)

| Method | Signature | Description |
|--------|-----------|-------------|
| `Env.get(name)` | `(Text) -> Option[Text]` | Get environment variable |
| `Env.args()` | `() -> List[Text]` | Command-line arguments |

---

## Standard Library Modules

### std.core

Core type aliases and utility functions. Automatically imported via prelude.

```astra
import std.core
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `identity(x)` | `[T](T) -> T` | Returns its argument unchanged |
| `constant(x, _)` | `[T, U](T, U) -> T` | Returns first argument, ignores second |

### std.option

Utility functions for `Option[T]`. Automatically imported via prelude.

```astra
import std.option
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `is_some(opt)` | `(Option[T]) -> Bool` | True if contains a value |
| `is_none(opt)` | `(Option[T]) -> Bool` | True if empty |
| `unwrap_or(opt, default)` | `(Option[T], T) -> T` | Value or default |
| `map(opt, f)` | `(Option[T], (T) -> U) -> Option[U]` | Transform contained value |

### std.result

Utility functions for `Result[T, E]`. Automatically imported via prelude.

```astra
import std.result
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `is_ok(res)` | `(Result[T, E]) -> Bool` | True if Ok |
| `is_err(res)` | `(Result[T, E]) -> Bool` | True if Err |
| `unwrap_or(res, default)` | `(Result[T, E], T) -> T` | Value or default |
| `map(res, f)` | `(Result[T, E], (T) -> U) -> Result[U, E]` | Transform success value |
| `map_err(res, f)` | `(Result[T, E], (E) -> F) -> Result[T, F]` | Transform error value |

### std.list

Utility functions for `List[T]`.

```astra
import std.list
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `is_empty(list)` | `(List[T]) -> Bool` | True if list has no elements |
| `head(list)` | `(List[T]) -> Option[T]` | First element, or None |

### std.math

Math utility functions.

```astra
import std.math
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs_val(x)` | `(Int) -> Int` | Absolute value (wrapper around `abs`) |
| `min_val(a, b)` | `(Int, Int) -> Int` | Minimum (wrapper around `min`) |
| `max_val(a, b)` | `(Int, Int) -> Int` | Maximum (wrapper around `max`) |
| `clamp(x, low, high)` | `(Int, Int, Int) -> Int` | Clamp value to range [low, high] |
| `is_even(n)` | `(Int) -> Bool` | True if n is even |
| `is_odd(n)` | `(Int) -> Bool` | True if n is odd |

```astra
import std.math

clamp(15, 0, 10)   # => 10
clamp(-5, 0, 10)   # => 0
is_even(4)          # => true
is_odd(7)           # => true
```

### std.string

String utility functions.

```astra
import std.string
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `is_blank(s)` | `(Text) -> Bool` | True if string is empty or only whitespace |
| `pad_left(s, width, pad_char)` | `(Text, Int, Text) -> Text` | Left-pad to width |
| `pad_right(s, width, pad_char)` | `(Text, Int, Text) -> Text` | Right-pad to width |

```astra
import std.string

is_blank("  ")          # => true
is_blank("hello")       # => false
pad_left("42", 5, "0")  # => "00042"
pad_right("hi", 6, ".") # => "hi...."
```

### std.collections

Collection utility functions.

```astra
import std.collections
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `group_by_even(items)` | `(List[Int]) -> (List[Int], List[Int])` | Split into (evens, odds) |
| `frequencies(items)` | `(List[Int]) -> Map[Int, Int]` | Count occurrences of each value |
| `chunks(items, size)` | `(List[Int], Int) -> List[List[Int]]` | Split into chunks of given size |

```astra
import std.collections

group_by_even([1, 2, 3, 4, 5])  # => ([2, 4], [1, 3, 5])
frequencies([1, 2, 2, 3, 3, 3]) # => {1: 1, 2: 2, 3: 3}
chunks([1, 2, 3, 4, 5], 2)      # => [[1, 2], [3, 4], [5]]
```

### std.iter

Iterator-style utility functions for lists.

```astra
import std.iter
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `sum(list)` | `(List[Int]) -> Int` | Sum of all elements |
| `product(list)` | `(List[Int]) -> Int` | Product of all elements |
| `all(list)` | `(List[Bool]) -> Bool` | True if all elements are true |
| `any(list)` | `(List[Bool]) -> Bool` | True if any element is true |
| `count(list, pred)` | `(List[T], (T) -> Bool) -> Int` | Count elements matching predicate |
| `flat_map(list, f)` | `(List[T], (T) -> List[U]) -> List[U]` | Map and flatten |
| `reduce(list, f)` | `(List[T], (T, T) -> T) -> Option[T]` | Reduce to single value |

```astra
import std.iter

sum([1, 2, 3, 4, 5])          # => 15
product([1, 2, 3, 4])         # => 24
all([true, true, false])      # => false
count([1, 2, 3, 4], fn(x) { x > 2 })  # => 2
reduce([1, 2, 3], fn(a, b) { a + b })  # => Some(6)
```

### std.io

Convenience wrappers for I/O effects.

```astra
import std.io
```

| Function | Signature | Effects | Description |
|----------|-----------|---------|-------------|
| `print_line(text)` | `(Text) -> Unit` | Console | Print with newline |
| `print_text(text)` | `(Text) -> Unit` | Console | Print without newline |
| `read_line()` | `() -> Option[Text]` | Console | Read line from stdin |
| `read_file(path)` | `(Text) -> Text` | Fs | Read file contents |
| `write_file(path, content)` | `(Text, Text) -> Unit` | Fs | Write file contents |
| `file_exists(path)` | `(Text) -> Bool` | Fs | Check if file exists |

### std.json

Basic JSON utilities.

```astra
import std.json
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `stringify(value)` | `(Text) -> Text` | Wrap text in JSON quotes |
| `parse_int(s)` | `(Text) -> Result[Int, Text]` | Parse integer from string |
| `parse_bool(s)` | `(Text) -> Result[Bool, Text]` | Parse boolean from string |
| `escape(s)` | `(Text) -> Text` | Escape special characters for JSON |

```astra
import std.json

stringify("hello")     # => "\"hello\""
parse_int("42")        # => Ok(42)
parse_bool("true")     # => Ok(true)
escape("line\nnext")   # => "line\\nnext"
```

### std.error

Error handling utilities.

```astra
import std.error
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `wrap(msg, cause)` | `(Text, Text) -> Text` | Combine error messages: "msg: cause" |
| `from_text(msg)` | `(Text) -> Result[Unit, Text]` | Create Err from message |
| `ok_unit()` | `() -> Result[Unit, Text]` | Create Ok(()) |
| `map_error(result, prefix)` | `(Result[T, Text], Text) -> Result[T, Text]` | Add prefix to error message |
| `or_else(result, fallback)` | `(Result[T, Text], () -> Result[T, Text]) -> Result[T, Text]` | Try fallback on Err |

```astra
import std.error

wrap("read failed", "file not found")  # => "read failed: file not found"
map_error(Err("timeout"), "network")   # => Err("network: timeout")
or_else(Err("fail"), fn() { Ok(42) })  # => Ok(42)
```

---

## Prelude

The prelude (`std.prelude`) automatically imports commonly used modules:

```astra
import std.option
import std.result
import std.core
```

You do not need to explicitly import these — they are available in every Astra file.
