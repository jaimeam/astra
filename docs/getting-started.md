# Getting Started with Astra

Welcome to Astra, an LLM/Agent-native programming language designed for verifiability and deterministic feedback.

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/astra-lang/astra.git
cd astra

# Build the toolchain
cargo build --release

# Add to PATH (optional)
export PATH="$PATH:$(pwd)/target/release"
```

## Hello World

Create a file `hello.astra`:

```astra
module hello

fn main() effects(Console) {
  Console.println("Hello, Astra!")
}
```

Run it:

```bash
astra run hello.astra
```

## Project Structure

Create a new project:

```
my-project/
├── astra.toml        # Project manifest
├── src/
│   └── main.astra    # Main entry point
└── tests/
    └── main_test.astra
```

### astra.toml

```toml
[package]
name = "my-project"
version = "0.1.0"

[targets]
default = "interpreter"
```

### src/main.astra

```astra
module main

import std.option.{Some, None}

type Greeting = {
  message: Text,
  recipient: Text,
}

fn create_greeting(name: Text) -> Greeting {
  {
    message = "Hello",
    recipient = name,
  }
}

fn format_greeting(g: Greeting) -> Text {
  g.message + ", " + g.recipient + "!"
}

fn main() effects(Console) {
  let greeting = create_greeting("World")
  Console.println(format_greeting(greeting))
}
```

## Basic Concepts

### Variables

```astra
# Immutable (default)
let x = 42
let name = "Alice"

# Mutable
let mut counter = 0
counter = counter + 1
```

### Functions

```astra
# Simple function
fn add(a: Int, b: Int) -> Int {
  a + b
}

# Function with effects
fn read_file(path: Text) -> Result[Text, FsError]
  effects(Fs)
{
  Fs.read(path)
}
```

### Types

```astra
# Record type
type Point = { x: Int, y: Int }

# Enum type
enum Shape =
  | Circle(radius: Int)
  | Rectangle(width: Int, height: Int)
```

### Pattern Matching

```astra
fn area(shape: Shape) -> Int {
  match shape {
    Circle(r) => 3 * r * r  # Approximation
    Rectangle(w, h) => w * h
  }
}
```

### Option and Result

```astra
# Option for nullable values
fn find_user(id: Int) -> Option[User] {
  # Returns Some(user) or None
}

# Result for operations that can fail
fn divide(a: Int, b: Int) -> Result[Int, Text] {
  if b == 0 {
    Err("Division by zero")
  } else {
    Ok(a / b)
  }
}

# Using ? for early return
fn process(a: Int, b: Int) -> Result[Int, Text] {
  let quotient = divide(a, b)?  # Returns Err early if division fails
  Ok(quotient * 2)
}
```

## Effects System

Functions that interact with the outside world must declare their effects:

```astra
# Pure function (no effects)
fn pure_add(a: Int, b: Int) -> Int {
  a + b
}

# Function with network effect
fn fetch(url: Text) -> Result[Text, NetError]
  effects(Net)
{
  Net.get(url)
}

# Function with multiple effects
fn log_and_fetch(url: Text) -> Result[Text, NetError]
  effects(Console, Net)
{
  Console.println("Fetching: " + url)
  Net.get(url)
}
```

### Testing with Mock Effects

```astra
test "fetch with mock network" {
  using effects(Net = MockNet.returning({ status = 200, body = "OK" }))

  let result = fetch("http://example.com")
  assert result.is_ok()
  assert_eq(result.unwrap(), "OK")
}
```

## Writing Tests

Tests are built into the language:

```astra
# Unit test
test "addition works" {
  assert_eq(add(1, 2), 3)
}

# Property test
property "addition is commutative" {
  forall a: Int, b: Int {
    assert_eq(add(a, b), add(b, a))
  }
}
```

Run tests:

```bash
astra test
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `astra fmt` | Format code canonically |
| `astra check` | Check for errors without running |
| `astra test` | Run all tests |
| `astra run <file>` | Run a program |
| `astra package` | Create distributable artifact |

### Common Options

```bash
astra check --json          # Output errors as JSON
astra test --filter "math"  # Run tests matching "math"
astra run --seed 42         # Use specific random seed
```

## Next Steps

- Read the [Language Specification](spec.md)
- Explore [Error Codes](errors.md)
- Learn [Formatting Rules](formatting.md)
- Check out [Examples](../examples/)
