# Astra Formatting Rules

This document describes the canonical formatting rules for Astra code.

## Principles

1. **One canonical form** - No configuration options
2. **Readability** - Optimize for human scanning
3. **Diff-friendliness** - Small changes produce small diffs
4. **Consistency** - Same patterns formatted the same way

## Indentation

- Use **2 spaces** for indentation
- No tabs
- No trailing whitespace

```astra
fn example() -> Int {
  let x = 1
  if x > 0 {
    x + 1
  } else {
    0
  }
}
```

## Line Length

- Maximum line length: **100 characters**
- Break long lines at logical points

## Braces and Blocks

- Opening brace on same line
- Closing brace on its own line
- Single-expression blocks may be on one line if short

```astra
# Multi-line block
fn foo() -> Int {
  let x = compute_value()
  x + 1
}

# Short single expression (allowed on one line in simple cases)
fn identity(x: Int) -> Int { x }
```

## Functions

### Declaration

```astra
# Simple function
fn add(a: Int, b: Int) -> Int {
  a + b
}

# Function with effects
fn fetch(url: Text) -> Result[Text, Error]
  effects(Net)
{
  Net.get(url)
}

# Function with contracts
fn divide(a: Int, b: Int) -> Int
  requires b != 0
  ensures result * b == a
{
  a / b
}

# Complex function (all on separate lines)
public fn process_payment(
  customer: CustomerId,
  amount: Money,
  options: PaymentOptions,
) -> Result[Receipt, PaymentError]
  effects(Net, Clock)
  requires amount.cents > 0
  ensures result.is_ok() implies result.ok.amount == amount
{
  # body
}
```

### Calls

```astra
# Short call on one line
let result = add(1, 2)

# Long call with line breaks
let result = send_notification(
  recipient = user.email,
  subject = "Hello",
  body = format_message(template, data),
)
```

## Types

### Records

```astra
# Short record on one line
type Point = { x: Int, y: Int }

# Longer record on multiple lines
type User = {
  id: UserId,
  name: Text,
  email: Text,
  created_at: Timestamp,
}
```

### Enums

```astra
# Simple enum on one line
enum Color = Red | Green | Blue

# Enum with data on multiple lines
enum Result[T, E] =
  | Ok(value: T)
  | Err(error: E)
```

## Expressions

### Binary Operators

```astra
# Short expressions on one line
let sum = a + b
let is_valid = x > 0 and x < 100

# Long expressions broken at operators
let result =
  very_long_variable_name
  + another_long_name
  + third_term
```

### Match Expressions

```astra
# Simple match
match color {
  Red => "red"
  Green => "green"
  Blue => "blue"
}

# Match with longer arms
match result {
  Ok(value) => {
    process(value)
    value
  }
  Err(error) => {
    log_error(error)
    default_value
  }
}
```

### If Expressions

```astra
# Simple if on one line (only if very short)
let max = if a > b { a } else { b }

# Standard if formatting
if condition {
  do_something()
} else if other_condition {
  do_other_thing()
} else {
  fallback()
}
```

## Imports

```astra
# One import per line
import std.collections.List
import std.text.{format, join}
import myproject.utils as Utils

# Grouped by source (stdlib, external, internal)
# Alphabetized within groups
```

## Comments

```astra
# Single-line comment

## Documentation comment
## Describes the following item

fn documented_function() -> Int {
  # Inline comment explaining logic
  compute_value()
}
```

## Lists and Records

### Trailing Commas

Always use trailing commas in multi-line lists:

```astra
let items = [
  first_item,
  second_item,
  third_item,  # trailing comma
]

let config = {
  host = "localhost",
  port = 8080,
  debug = true,  # trailing comma
}
```

### Single-Line vs Multi-Line

```astra
# Short list on one line
let colors = [Red, Green, Blue]

# Long list on multiple lines
let months = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
]
```

## Tests

```astra
test "descriptive test name" {
  # Arrange
  let input = create_test_input()

  # Act
  let result = function_under_test(input)

  # Assert
  assert_eq(result, expected)
}

test "test with mock" {
  using effects(Net = MockNet.returning(test_response))

  let result = fetch_data()
  assert result.is_ok()
}
```

## Blank Lines

- One blank line between top-level items
- No blank line after opening brace
- No blank line before closing brace
- Use blank lines to separate logical groups within functions

```astra
type Point = { x: Int, y: Int }

fn distance(a: Point, b: Point) -> Float {
  let dx = b.x - a.x
  let dy = b.y - a.y

  sqrt(dx * dx + dy * dy)
}

fn midpoint(a: Point, b: Point) -> Point {
  {
    x = (a.x + b.x) / 2,
    y = (a.y + b.y) / 2,
  }
}
```
