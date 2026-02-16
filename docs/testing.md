# Testing in Astra

Testing is a first-class language feature in Astra. Tests are written inline with your code using the `test` keyword — no external test framework, no macros, no separate files required.

## Writing Tests

### Basic Tests

Use the `test` keyword followed by a descriptive string and a block:

```astra
module math

fn add(a: Int, b: Int) -> Int {
  a + b
}

test "add returns the sum of two numbers" {
  assert_eq(add(2, 3), 5)
}

test "add handles zero" {
  assert_eq(add(0, 0), 0)
  assert_eq(add(5, 0), 5)
  assert_eq(add(0, 5), 5)
}

test "add handles negative numbers" {
  assert_eq(add(-1, 1), 0)
  assert_eq(add(-3, -4), -7)
}
```

Tests live in the same file as the code they test, right next to the functions they exercise. This makes it easy to keep tests in sync with the code.

### Assertions

Astra provides two built-in assertion functions:

| Function | Description |
|----------|-------------|
| `assert(condition)` | Asserts that the condition is `true`. Fails with `E4008` if false. |
| `assert_eq(left, right)` | Asserts that two values are equal. Fails with `E4008` showing both values. |

```astra
test "assertions" {
  assert(2 + 2 == 4)
  assert(true)

  assert_eq(10 / 2, 5)
  assert_eq("hello", "hello")
}
```

### Running Tests

```bash
# Run all tests in the project
cargo run -- test

# Run tests in a specific file
cargo run -- test examples/fibonacci.astra

# Run tests matching a filter (by test name)
cargo run -- test "add"
```

## Testing Pure Functions

Pure functions (no effects) are the simplest to test — no setup, no mocking:

```astra
module string_utils

fn is_palindrome(s: Text) -> Bool {
  s == reverse(s)
}

fn clamp(x: Int, lo: Int, hi: Int) -> Int {
  if x < lo {
    lo
  } else {
    if x > hi { hi } else { x }
  }
}

test "palindrome detection" {
  assert(is_palindrome("racecar"))
  assert(is_palindrome("abba"))
  assert(not is_palindrome("hello"))
}

test "clamp constrains values to range" {
  assert_eq(clamp(50, 0, 100), 50)
  assert_eq(clamp(-10, 0, 100), 0)
  assert_eq(clamp(200, 0, 100), 100)
}
```

## Testing with Effects

When testing functions that use effects, you inject mock capabilities using the `using effects(...)` clause. This makes tests deterministic.

### Mocking the Clock

```astra
test "fixed clock returns constant time"
  using effects(Clock = Clock.fixed(1700000000))
{
  let now = Clock.now()
  assert(now == 1700000000)
}

test "sleep is a no-op with fixed clock"
  using effects(Clock = Clock.fixed(5000))
{
  Clock.sleep(1000)
  let now = Clock.now()
  assert(now == 5000)  # Time didn't advance
}
```

### Mocking Randomness

```astra
test "seeded rand produces deterministic sequence"
  using effects(Rand = Rand.seeded(42))
{
  let x = Rand.int(1, 100)
  let y = Rand.int(1, 100)
  assert_eq(x, 75)  # Always 75 with seed 42
  assert_eq(y, 72)  # Always 72 next
}
```

### Multiple Mocked Effects

```astra
test "multiple effects in one test"
  using effects(Clock = Clock.fixed(999), Rand = Rand.seeded(7))
{
  let time = Clock.now()
  let random = Rand.int(1, 10)
  assert_eq(time, 999)
  assert_eq(random, 8)
}
```

## Testing with Contracts

Functions with `requires` and `ensures` clauses have their contracts checked at runtime:

```astra
fn divide(a: Int, b: Int) -> Int
  requires b != 0
{
  a / b
}

fn abs(x: Int) -> Int
  ensures result >= 0
{
  if x < 0 { 0 - x } else { x }
}

test "divide works with valid inputs" {
  assert_eq(divide(10, 2), 5)
  assert_eq(divide(7, 3), 2)
}

test "abs returns non-negative values" {
  assert_eq(abs(5), 5)
  assert_eq(abs(0 - 5), 5)
  assert_eq(abs(0), 0)
}
```

Calling `divide(10, 0)` would produce a runtime error `E3001: Precondition violated: b != 0`.

## Testing Enums and Pattern Matching

```astra
module user

enum Status =
  | Active
  | Inactive
  | Pending(reason: Text)

fn describe(s: Status) -> Text {
  match s {
    Active => "active"
    Inactive => "inactive"
    Pending(r) => "pending: " + r
  }
}

test "describe all status variants" {
  assert_eq(describe(Active), "active")
  assert_eq(describe(Inactive), "inactive")
  assert_eq(describe(Pending(reason = "review")), "pending: review")
}
```

## Testing Option and Result

```astra
module lookup

fn find_user(id: Int) -> Option[Text] {
  if id == 1 {
    Some("Alice")
  } else {
    None
  }
}

fn parse_int(s: Text) -> Result[Int, Text] {
  if s == "42" {
    Ok(42)
  } else {
    Err("not a number: " + s)
  }
}

test "find_user returns Some for known id" {
  match find_user(1) {
    Some(name) => assert_eq(name, "Alice")
    None => assert(false)
  }
}

test "find_user returns None for unknown id" {
  assert(find_user(999).is_none())
}

test "parse_int succeeds on valid input" {
  assert(parse_int("42").is_ok())
}

test "parse_int fails on invalid input" {
  assert(parse_int("abc").is_err())
}
```

## Test Organization

Tests should be placed directly after the functions they test:

```astra
module calculator

fn add(a: Int, b: Int) -> Int { a + b }

test "add" {
  assert_eq(add(1, 2), 3)
}

fn multiply(a: Int, b: Int) -> Int { a * b }

test "multiply" {
  assert_eq(multiply(3, 4), 12)
}
```

This keeps tests close to the code they verify, making it easy to see at a glance whether a function is tested and what behaviors are covered.

## Property Tests

Astra supports property-based testing with the `property` keyword:

```astra
property "addition is commutative" {
  let a = Rand.int(-100, 100)
  let b = Rand.int(-100, 100)
  assert_eq(add(a, b), add(b, a))
}
```

Property tests use seeded randomness internally, so they are deterministic and reproducible.

## Key Differences from Other Languages

| Feature | Python (pytest) | Rust | Astra |
|---------|----------------|------|-------|
| Test syntax | `def test_foo():` | `#[test] fn test_foo()` | `test "foo" { }` |
| Assertions | `assert x == y` | `assert_eq!(x, y)` | `assert_eq(x, y)` |
| Test location | Separate files | Same file, `#[cfg(test)]` module | Same file, inline |
| Mocking I/O | External libraries | External libraries | Built-in `using effects()` |
| Determinism | Not guaranteed | Not guaranteed | Guaranteed by design |
| Test runner | External (`pytest`) | Built-in (`cargo test`) | Built-in (`astra test`) |
