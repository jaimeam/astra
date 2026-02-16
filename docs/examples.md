# Astra by Example

A cookbook of common patterns and idioms in Astra. Each example is self-contained and can be run with `cargo run -- run <file>`.

## Hello World

```astra
module hello

fn main() effects(Console) {
  Console.println("Hello, Astra!")
}
```

Every Astra file starts with `module`. The `main` function is the entry point. `effects(Console)` declares that this function performs console I/O.

## Variables and Types

```astra
module basics

fn main() effects(Console) {
  # Let bindings are immutable
  let name = "Alice"
  let age = 30
  let active = true

  # Type annotations are optional when inferrable
  let score: Int = 100

  Console.println("Name: " + name)
}
```

## Functions

```astra
module functions

# Parameters require type annotations
# Return type follows ->
fn add(a: Int, b: Int) -> Int {
  a + b
}

# Last expression is the return value
fn max(a: Int, b: Int) -> Int {
  if a > b { a } else { b }
}

# Functions without -> return Unit implicitly
fn greet(name: Text) effects(Console) {
  Console.println("Hello, " + name)
}
```

## Record Types

```astra
module records

type Point = { x: Int, y: Int }

type User = {
  name: Text,
  age: Int,
  active: Bool,
}

fn create_user(name: Text, age: Int) -> User {
  { name = name, age = age, active = true }
}

fn describe_user(u: User) -> Text {
  u.name + " (age " + to_text(u.age) + ")"
}

test "create and describe user" {
  let u = create_user("Alice", 30)
  assert_eq(u.name, "Alice")
  assert_eq(u.age, 30)
  assert(u.active)
}
```

## Enums (Sum Types)

```astra
module enums

enum Color =
  | Red
  | Green
  | Blue

enum Shape =
  | Circle(radius: Int)
  | Rectangle(width: Int, height: Int)

fn area(s: Shape) -> Int {
  match s {
    Circle(r) => 3 * r * r
    Rectangle(w, h) => w * h
  }
}

test "area calculations" {
  assert_eq(area(Circle(radius = 5)), 75)
  assert_eq(area(Rectangle(width = 4, height = 3)), 12)
}
```

## Pattern Matching

Pattern matching is how you branch on data in Astra. The compiler ensures all cases are covered.

```astra
module matching

enum TrafficLight = Red | Yellow | Green

fn action(light: TrafficLight) -> Text {
  match light {
    Red => "stop"
    Yellow => "caution"
    Green => "go"
  }
}

# Match on integers
fn describe(n: Int) -> Text {
  match n {
    0 => "zero"
    1 => "one"
    _ => "many"
  }
}

# Match on Option
fn display(opt: Option[Int]) -> Text {
  match opt {
    Some(n) => "value: " + to_text(n)
    None => "empty"
  }
}

# Match on Result
fn show_result(r: Result[Int, Text]) -> Text {
  match r {
    Ok(n) => "success: " + to_text(n)
    Err(e) => "error: " + e
  }
}
```

## Option — Handling Missing Values

Astra has no `null`. Use `Option[T]` for values that may be absent:

```astra
module options

fn find_user(id: Int) -> Option[{ name: Text, age: Int }] {
  if id == 1 {
    Some({ name = "Alice", age = 30 })
  } else {
    None
  }
}

# Use match to handle both cases
fn greet_user(id: Int) -> Text {
  match find_user(id) {
    Some(user) => "Hello, " + user.name
    None => "User not found"
  }
}

# Use ? to propagate None
fn get_user_name(id: Int) -> Option[Text] {
  let user = find_user(id)?
  Some(user.name)
}

# Use ?else for a default
fn get_name_or_default(id: Int) -> Text {
  let user = find_user(id) ?else { name = "Guest", age = 0 }
  user.name
}

test "option handling" {
  assert_eq(greet_user(1), "Hello, Alice")
  assert_eq(greet_user(99), "User not found")
}
```

## Result — Handling Errors

Use `Result[T, E]` for operations that can fail:

```astra
module results

enum ParseError =
  | EmptyInput
  | InvalidFormat(detail: Text)

fn parse_age(input: Text) -> Result[Int, ParseError] {
  if input == "" {
    Err(EmptyInput)
  } else {
    if input == "30" {
      Ok(30)
    } else {
      Err(InvalidFormat(detail = "expected a number"))
    }
  }
}

# Chain operations with ?
fn validate_user_age(input: Text) -> Result[Text, ParseError] {
  let age = parse_age(input)?
  if age >= 0 and age <= 150 {
    Ok("valid age: " + to_text(age))
  } else {
    Err(InvalidFormat(detail = "age out of range"))
  }
}

test "result chaining" {
  assert(validate_user_age("30").is_ok())
  assert(validate_user_age("").is_err())
}
```

## Effects — Declaring Side Effects

Functions that perform I/O must declare their effects:

```astra
module effects_example

# Pure function — no effects
fn double(x: Int) -> Int {
  x * 2
}

# Prints to console — must declare Console
fn print_doubled(x: Int) effects(Console) {
  Console.println(to_text(double(x)))
}

# Reads a file — must declare Fs
fn read_config(path: Text) -> Text effects(Fs) {
  Fs.read(path)
}

# Multiple effects
fn fetch_and_print(url: Text) effects(Net, Console) {
  let body = Net.get(url)
  Console.println(body)
}
```

## Deterministic Testing

Mock effects to make tests reproducible:

```astra
module random_example

fn roll_dice() -> Int effects(Rand) {
  Rand.int(1, 6)
}

fn generate_id() -> Int effects(Rand, Clock) {
  let time = Clock.now()
  let random = Rand.int(0, 999)
  time * 1000 + random
}

test "deterministic random"
  using effects(Rand = Rand.seeded(42))
{
  let roll = roll_dice()
  # Same seed always produces same result
  assert(roll >= 1 and roll <= 6)
}

test "deterministic id generation"
  using effects(Rand = Rand.seeded(1), Clock = Clock.fixed(100))
{
  let id = generate_id()
  # With fixed clock and seeded rand, id is always the same
  assert(id > 0)
}
```

## Contracts — Preconditions and Postconditions

Use `requires` and `ensures` to enforce function contracts:

```astra
module contracts_example

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

fn clamp(x: Int, lo: Int, hi: Int) -> Int
  requires lo <= hi
  ensures result >= lo
  ensures result <= hi
{
  if x < lo { lo }
  else { if x > hi { hi } else { x } }
}

test "contracts enforce correctness" {
  assert_eq(divide(10, 3), 3)
  assert_eq(abs(-5), 5)
  assert_eq(clamp(50, 0, 100), 50)
}
```

## Recursion

```astra
module recursion

fn factorial(n: Int) -> Int {
  match n {
    0 => 1
    _ => n * factorial(n - 1)
  }
}

# Tail-recursive fibonacci with accumulator
fn fib(n: Int) -> Int {
  fib_helper(n, 0, 1)
}

fn fib_helper(n: Int, a: Int, b: Int) -> Int {
  match n {
    0 => a
    _ => fib_helper(n - 1, b, a + b)
  }
}

test "factorial" {
  assert_eq(factorial(0), 1)
  assert_eq(factorial(5), 120)
}

test "fibonacci" {
  assert_eq(fib(0), 0)
  assert_eq(fib(1), 1)
  assert_eq(fib(10), 55)
}
```

## If Expressions

`if` is an expression in Astra — it returns a value:

```astra
module conditionals

fn classify(n: Int) -> Text {
  if n > 0 {
    "positive"
  } else if n < 0 {
    "negative"
  } else {
    "zero"
  }
}

fn min(a: Int, b: Int) -> Int {
  if a < b { a } else { b }
}

test "classify numbers" {
  assert_eq(classify(5), "positive")
  assert_eq(classify(-3), "negative")
  assert_eq(classify(0), "zero")
}
```

## Lists

```astra
module lists

fn sum(items: List[Int]) -> Int {
  if is_empty(items) {
    0
  } else {
    match head(items) {
      Some(first) => first + sum(tail(items))
      None => 0
    }
  }
}

test "list operations" {
  let numbers = [1, 2, 3, 4, 5]
  assert_eq(len(numbers), 5)
  assert_eq(head(numbers), Some(1))
  assert(not is_empty(numbers))
  assert(is_empty([]))
}
```

## Putting It Together — A Complete Program

```astra
module todo_app

type Todo = {
  title: Text,
  done: Bool,
}

fn create_todo(title: Text) -> Todo {
  { title = title, done = false }
}

fn complete(todo: Todo) -> Todo {
  { title = todo.title, done = true }
}

fn format_todo(todo: Todo) -> Text {
  let status = if todo.done { "[x]" } else { "[ ]" }
  status + " " + todo.title
}

fn count_done(todos: List[Todo]) -> Int {
  # Count completed items
  todos.filter(fn(t) { t.done }).length()
}

test "create a todo" {
  let todo = create_todo("Buy groceries")
  assert_eq(todo.title, "Buy groceries")
  assert(not todo.done)
}

test "complete a todo" {
  let todo = create_todo("Buy groceries")
  let done = complete(todo)
  assert(done.done)
  assert_eq(done.title, "Buy groceries")
}

test "format todo" {
  let todo = create_todo("Buy groceries")
  assert_eq(format_todo(todo), "[ ] Buy groceries")
  assert_eq(format_todo(complete(todo)), "[x] Buy groceries")
}

fn main() effects(Console) {
  let todo = create_todo("Learn Astra")
  Console.println(format_todo(todo))
  Console.println(format_todo(complete(todo)))
}
```
