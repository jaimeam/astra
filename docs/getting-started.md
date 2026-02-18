# Getting Started with Astra

This guide teaches you the fundamentals of Astra, an LLM-native programming language designed for fast, deterministic feedback loops.

## Installation

Astra is currently built from source using Rust's Cargo toolchain.

### Prerequisites

- [Rust](https://rustup.rs/) (1.70 or later)
- Git

### Build and Install

```bash
# Clone the repository
git clone https://github.com/astra-lang/astra.git
cd astra

# Build the toolchain
cargo build --release

# Add to your PATH (add this to your shell profile to make it permanent)
export PATH="$PATH:$(pwd)/target/release"

# Verify installation
astra --help
```

The built binary is at `target/release/astra`. The `export PATH` line above makes the `astra` command available in your current shell. To make it permanent, add the export to your `~/.bashrc`, `~/.zshrc`, or equivalent.

> **Development shortcut**: If you're working on the Astra toolchain itself, you can skip the PATH setup and use `cargo run -- <command>` instead of `astra <command>`.

## Your First Program

Create a file named `hello.astra`:

```astra
module hello

fn main()
  effects(Console)
{
  Console.println("Hello, Astra!")
}
```

Run it:

```bash
astra run hello.astra
```

Output:

```
Hello, Astra!
```

### What This Code Does

1. **`module hello`** - Every Astra file starts with a module declaration. This defines the namespace.

2. **`fn main()`** - The entry point function. Unlike Python, functions are declared with `fn`.

3. **`effects(Console)`** - This declares that the function uses the `Console` capability. Astra requires you to explicitly declare all side effects. This is a key difference from Python/JavaScript where I/O is implicit.

4. **`Console.println(...)`** - Calls the `println` method on the `Console` capability to print text.

## Basic Syntax

### Modules

Every Astra file must start with a module declaration:

```astra
module my_module

# Rest of the code here
```

Comments start with `#` (like Python, unlike Rust's `//`).

### Let Bindings

Variables are declared with `let`. They are immutable by default:

```astra
module variables

fn main()
  effects(Console)
{
  # Immutable binding (default)
  let x = 42
  let name = "Alice"

  # Type annotations are optional when inferrable
  let count: Int = 10

  Console.println("Done")
}
```

**Key difference from Python**: Variables are immutable by default. You cannot reassign `x` after binding.

**Key difference from Rust**: Use `let mut` for mutable bindings. All bindings are immutable by default.

### Functions

Functions are defined with `fn`:

```astra
module functions

# Function with parameters and return type
fn add(a: Int, b: Int) -> Int {
  a + b
}

# The last expression is the return value (like Rust)
fn multiply(x: Int, y: Int) -> Int {
  x * y
}

fn main()
  effects(Console)
{
  let result = add(3, 4)
  Console.println("3 + 4 = 7")
}
```

**Key points**:
- Parameter types are required: `a: Int`
- Return type follows `->`: `-> Int`
- No `return` keyword needed for the final expression
- No semicolons at line ends (unlike Rust)

### Types

Astra has these built-in types:

| Type | Description | Example |
|------|-------------|---------|
| `Int` | 64-bit signed integer | `42`, `-7` |
| `Bool` | Boolean | `true`, `false` |
| `Text` | UTF-8 string | `"hello"` |
| `Unit` | Empty type | (implicit) |

#### Record Types

Define structured data with record types:

```astra
module records

# Define a record type
type Point = { x: Int, y: Int }

# Define a more complex type
type User = {
  name: Text,
  age: Int,
  active: Bool,
}

fn create_point(x: Int, y: Int) -> { x: Int, y: Int } {
  { x = x, y = y }
}

fn main()
  effects(Console)
{
  let p = { x = 10, y = 20 }
  Console.println("Point created")
}
```

**Key difference from Python**: Types are explicitly declared. There are no classes, only data records.

**Key difference from Rust**: Simpler syntax. No `struct` keyword, no lifetime annotations.

#### Enum Types

Define sum types (variants) with `enum`:

```astra
module enums

enum Color =
  | Red
  | Green
  | Blue

enum Shape =
  | Circle(radius: Int)
  | Rectangle(width: Int, height: Int)

fn describe_color(c: Color) -> Text {
  match c {
    Red => "red"
    Green => "green"
    Blue => "blue"
  }
}

fn main()
  effects(Console)
{
  Console.println("Colors defined")
}
```

### Option and Result

Astra has no `null`. Instead, use `Option[T]` for values that may be absent:

```astra
module options

fn find_user(id: Int) -> Option[{ name: Text, age: Int }] {
  if id == 1 {
    Some({ name = "Alice", age = 30 })
  } else {
    None
  }
}

fn main()
  effects(Console)
{
  let user = find_user(1)
  match user {
    Some(u) => Console.println("Found user")
    None => Console.println("User not found")
  }
}
```

For operations that can fail, use `Result[T, E]`:

```astra
module results

enum ParseError =
  | EmptyInput
  | InvalidFormat

fn parse_number(s: Text) -> Result[Int, ParseError] {
  if s == "42" {
    Ok(42)
  } else {
    Err(InvalidFormat)
  }
}

fn main()
  effects(Console)
{
  match parse_number("42") {
    Ok(n) => Console.println("Parsed successfully")
    Err(e) => Console.println("Parse failed")
  }
}
```

**Key difference from Python**: No `None` or exceptions. All potential failures are encoded in the type system.

**Key difference from Rust**: Similar to Rust's `Option` and `Result`, but simpler - no `.unwrap()` panics.

## Effects System

The effects system is Astra's most distinctive feature. It makes side effects explicit and controllable.

### Why Effects Matter

In Python or JavaScript, any function can secretly:
- Read/write files
- Make network requests
- Access the current time
- Generate random numbers

This makes code hard to test and reason about. In Astra, these capabilities must be declared.

### Declaring Effects

```astra
module effects_example

# Pure function - no effects keyword means no side effects
fn add(a: Int, b: Int) -> Int {
  a + b
}

# Function that prints - must declare Console effect
fn greet(name: Text)
  effects(Console)
{
  Console.println("Hello, " + name + "!")
}

fn main()
  effects(Console)
{
  let sum = add(2, 3)
  greet("World")
}
```

### Available Effects

| Effect | Capability | Description |
|--------|------------|-------------|
| `Console` | `Console.println(text)`, `Console.print(text)` | Terminal I/O |
| `Fs` | `Fs.read(path)`, `Fs.write(path, content)` | File system |
| `Net` | `Net.get(url)`, `Net.post(url, body)` | Network requests |
| `Clock` | `Clock.now()` | Current time |
| `Rand` | `Rand.int(min, max)`, `Rand.bool()` | Random numbers |
| `Env` | `Env.get(key)`, `Env.args()` | Environment variables |

### Multiple Effects

Functions can declare multiple effects:

```astra
module multi_effects

fn fetch_and_log(url: Text) -> Text
  effects(Net, Console)
{
  Console.println("Fetching: " + url)
  let response = Net.get(url)
  Console.println("Done")
  response
}

fn main()
  effects(Net, Console)
{
  let data = fetch_and_log("https://example.com")
  Console.println("Received data")
}
```

**Key insight for LLMs**: When you call a function with effects, your function must also declare those effects. Effects propagate up the call chain.

## Pattern Matching

Pattern matching is how you destructure and branch on data in Astra.

### Basic Matching

```astra
module matching

fn describe_number(n: Int) -> Text {
  match n {
    0 => "zero"
    1 => "one"
    _ => "many"
  }
}

fn main()
  effects(Console)
{
  Console.println(describe_number(0))
  Console.println(describe_number(1))
  Console.println(describe_number(42))
}
```

### Matching Enums

```astra
module enum_matching

enum Status =
  | Active
  | Inactive
  | Pending(reason: Text)

fn describe_status(s: Status) -> Text {
  match s {
    Active => "User is active"
    Inactive => "User is inactive"
    Pending(r) => "Pending: " + r
  }
}

fn main()
  effects(Console)
{
  let s = Pending(reason = "awaiting verification")
  Console.println(describe_status(s))
}
```

### Matching Option and Result

```astra
module option_matching

fn safe_divide(a: Int, b: Int) -> Option[Int] {
  if b == 0 {
    None
  } else {
    Some(a / b)
  }
}

fn main()
  effects(Console)
{
  match safe_divide(10, 2) {
    Some(result) => Console.println("Result: 5")
    None => Console.println("Cannot divide by zero")
  }
}
```

**Important**: Match expressions must be exhaustive. The compiler will error if you forget a case.

## Error Messages

Astra produces machine-readable error diagnostics designed for LLMs to parse and fix.

### Error Code Format

Errors follow the pattern `E####`:
- `E0xxx` - Syntax/parsing errors
- `E1xxx` - Type errors
- `E2xxx` - Effect errors
- `E3xxx` - Contract violations
- `E4xxx` - Runtime errors

### Example: Syntax Error

If you write invalid syntax:

```astra
module broken

fn add(a Int) -> Int {
  a
}
```

The compiler produces:

```
error[E0001]: Expected ':', found 'Int'
  --> broken.astra:3:9
   |
 3 | fn add(a Int) -> Int {
   |         ^^^
```

**Fix**: Add the colon between parameter name and type: `fn add(a: Int)`

### Example: Missing Match Case

```astra
module incomplete

enum Light = Red | Yellow | Green

fn describe(l: Light) -> Text {
  match l {
    Red => "stop"
    Green => "go"
  }
}
```

The compiler produces:

```
error[E1004]: Non-exhaustive match: missing pattern 'Yellow'
  --> incomplete.astra:6:3
   |
 6 |   match l {
   |   ^^^^^
```

**Fix**: Add the missing case: `Yellow => "caution"`

### The Feedback Loop

Astra is designed for this workflow:

```
Write code
    |
    v
Run: astra check file.astra
    |
    v
Parse error output (JSON available with --json)
    |
    v
Apply suggested fixes (or run: astra fix file.astra)
    |
    v
Repeat until: "0 errors"
    |
    v
Run: astra run file.astra
```

For LLMs: When you receive an error, look at:
1. The error code (e.g., `E1004`)
2. The location (file, line, column)
3. The message explaining what's wrong
4. Any suggested fixes

## CLI Commands

| Command | Description |
|---------|-------------|
| `astra run <file>` | Execute an Astra program |
| `astra check [files...]` | Type-check without running |
| `astra test [filter]` | Run tests deterministically |
| `astra fmt [files...]` | Format code canonically |
| `astra fix [files...]` | Auto-apply diagnostic suggestions |
| `astra explain <code>` | Explain an error code (e.g., `astra explain E1001`) |
| `astra repl` | Interactive REPL |
| `astra init <name>` | Scaffold a new project |
| `astra doc [files...]` | Generate API documentation |
| `astra lsp` | Start the LSP server |

### Useful Options

```bash
# Check with JSON output (for programmatic parsing)
astra check --json myfile.astra

# Check all files in a directory
astra check src/

# Watch mode — re-checks on file changes
astra check --watch .

# Auto-fix diagnostics (dry run first)
astra fix --dry-run .
astra fix .

# Run tests with watch mode
astra test --watch
```

## Complete Example: Fibonacci

Here's a complete working example demonstrating recursion and pattern matching:

```astra
module fibonacci

fn fib(n: Int) -> Int {
  match n {
    0 => 0
    1 => 1
    _ => fib(n - 1) + fib(n - 2)
  }
}

fn main()
  effects(Console)
{
  Console.println("Fibonacci sequence:")
  Console.println("fib(0) = 0")
  Console.println("fib(1) = 1")
  Console.println("fib(5) = 5")
  Console.println("fib(10) = 55")
}
```

Run it:

```bash
astra run fibonacci.astra
```

## Next Steps

- **[Astra by Example](examples.md)** - Cookbook of common patterns and idioms
- **[Effects System](effects.md)** - Deep dive into Astra's capability-based effects
- **[Testing Guide](testing.md)** - How to write deterministic tests
- **[Standard Library](stdlib.md)** - API reference for built-in types and functions
- **[Language Specification](spec.md)** - Complete syntax and semantics reference
- **[Error Codes Reference](errors.md)** - All error codes with explanations
- **[Formatting Rules](formatting.md)** - How the canonical formatter works
- **[Why Astra?](why-astra.md)** - Design philosophy and rationale

## Quick Reference

### Syntax Cheat Sheet

```astra
# Module declaration (required at top)
module my_module

# Comments
# This is a comment

# Let binding
let x = 42
let name: Text = "Alice"

# Function
fn add(a: Int, b: Int) -> Int {
  a + b
}

# Function with effects
fn greet(name: Text)
  effects(Console)
{
  Console.println("Hello, " + name)
}

# Record type
type Point = { x: Int, y: Int }

# Enum type
enum Result = Ok(value: Int) | Err(message: Text)

# If expression
if condition {
  value_if_true
} else {
  value_if_false
}

# Match expression
match value {
  Pattern1 => result1
  Pattern2 => result2
  _ => default_result
}

# Option
Some(value)
None

# Result
Ok(value)
Err(error)
```

### Key Differences from Python

| Python | Astra |
|--------|-------|
| `def func():` | `fn func() {` |
| `None` | `Option[T]` with `Some`/`None` |
| Exceptions | `Result[T, E]` with `Ok`/`Err` |
| Implicit I/O | Explicit `effects(...)` |
| Dynamic typing | Static typing |
| `# comment` | `# comment` (same!) |

### Key Differences from TypeScript

| TypeScript | Astra |
|------------|-------|
| `function func(): number { }` | `fn func() -> Int {` |
| `{ x: number, y: number }` | `{ x: Int, y: Int }` (similar structural types) |
| `null \| undefined` / optional chaining | `Option[T]` with exhaustive matching |
| `try/catch` with untyped errors | `Result[T, E]` with typed errors |
| No effect tracking | Explicit `effects(...)` |
| `// comment` | `# comment` |
| Semicolons optional | No semicolons |

### Key Differences from Go

| Go | Astra |
|----|-------|
| `func add(a, b int) int { }` | `fn add(a: Int, b: Int) -> Int {` |
| `struct Point { X int }` | `type Point = { x: Int }` |
| No enums with data | `enum Shape = Circle(r: Int) \| Rectangle(w: Int, h: Int)` |
| `if err != nil { return err }` | `?` operator or `?else` for fallback |
| `nil` for absent values | `Option[T]` with `Some`/`None` |
| No pattern matching | `match` with exhaustiveness checking |
| `gofmt` (canonical format) | Built-in formatter (same philosophy!) |
| `go test` (built-in runner) | `astra test` (built-in, inline test blocks) |

### Key Differences from Rust

| Rust | Astra |
|------|-------|
| `fn func() -> i32 { }` | `fn func() -> Int { }` |
| `struct Point { x: i32 }` | `type Point = { x: Int }` |
| `let mut x = 5;` | `let mut x = 5` |
| Semicolons required | No semicolons |
| `// comment` | `# comment` |
| Ownership/borrowing | Garbage collected |
| `pub fn` | `public fn` |

## Known Limitations

Astra v1.0 has a few intentional limitations to be aware of:

- **No full Hindley-Milner type inference** - Add explicit type annotations if the checker cannot infer types in complex generic scenarios
- **Traits are runtime-dispatched** - The type checker validates trait impls but does not resolve trait methods on expressions
- **No concurrency** - Single-threaded only; `async`/`await` are reserved keywords
- **Interpreted only** - Tree-walking interpreter; adequate for small/medium programs
- **No package manager** - Projects use the stdlib and their own modules only
- **No debugger** - Use `println`, `assert`, and `test` blocks for debugging

## Imports and Multi-File Projects

Astra supports splitting code across multiple files using the module system:

```astra
# file: math_utils.astra
module math_utils

public fn double(x: Int) -> Int {
  x * 2
}

public fn add(a: Int, b: Int) -> Int {
  a + b
}
```

```astra
# file: main.astra
module main

import math_utils.{double, add}

fn main()
  effects(Console)
{
  let result = double(5)
  Console.println("double(5) = ${result}")
}
```

Key points:
- Use `public fn` to export functions from a module
- Use `import module_name.{fn1, fn2}` to import specific functions
- The type checker validates cross-file function calls (argument types, counts, effects)
- Stdlib modules are imported with `import std.math.{clamp}`, etc.
