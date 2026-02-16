# Effects System

The effects system is Astra's most distinctive feature. It makes side effects explicit, controllable, and testable.

## Why Effects?

In most languages, any function can secretly perform I/O:

```python
# Python - no way to know this reads files and makes HTTP calls
def process(path):
    data = open(path).read()        # Hidden file I/O
    requests.post("/api", data=data) # Hidden network I/O
    return len(data)
```

In Astra, capabilities must be declared in the function signature:

```astra
fn process(path: Text) -> Int
  effects(Fs, Net)
{
  let data = Fs.read(path)
  Net.post("/api", data)
  len(data)
}
```

An LLM (or human) reading `effects(Fs, Net)` immediately knows this function reads files and makes network calls. No surprises.

## Pure Functions

Functions without an `effects` clause are **pure** — they have no side effects and always return the same output for the same input:

```astra
fn add(a: Int, b: Int) -> Int {
  a + b
}

fn is_valid_email(s: Text) -> Bool {
  s.contains("@") and s.contains(".")
}
```

Pure functions:
- Can be called from anywhere (pure or effectful contexts)
- Are always deterministic
- Are trivially testable — no mocking needed

## Declaring Effects

Add an `effects(...)` clause after the parameter list (and before the return type or opening brace):

```astra
# Single effect
fn greet(name: Text) effects(Console) {
  Console.println("Hello, " + name + "!")
}

# Multiple effects
fn fetch_and_log(url: Text) -> Text
  effects(Net, Console)
{
  Console.println("Fetching: " + url)
  let response = Net.get(url)
  Console.println("Done")
  response
}
```

## Built-in Effects

Astra provides six built-in effects, each granting access to a specific capability:

### Console — Terminal I/O

```astra
fn greet(name: Text) effects(Console) {
  Console.print("Hello, ")    # Print without newline
  Console.println(name)        # Print with newline
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `Console.print(text)` | `(Text) -> Unit` | Print text without newline |
| `Console.println(text)` | `(Text) -> Unit` | Print text with newline |

### Fs — File System

```astra
fn copy_file(src: Text, dest: Text) effects(Fs) {
  let content = Fs.read(src)
  Fs.write(dest, content)
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `Fs.read(path)` | `(Text) -> Text` | Read file contents |
| `Fs.write(path, content)` | `(Text, Text) -> Unit` | Write content to file |

### Net — Network I/O

```astra
fn fetch_data(url: Text) -> Text effects(Net) {
  Net.get(url)
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `Net.get(url)` | `(Text) -> Text` | HTTP GET request |
| `Net.post(url, body)` | `(Text, Text) -> Text` | HTTP POST request |

### Clock — Time Access

```astra
fn timestamp() -> Int effects(Clock) {
  Clock.now()
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `Clock.now()` | `() -> Int` | Current time in milliseconds |
| `Clock.sleep(ms)` | `(Int) -> Unit` | Sleep for duration |

### Rand — Random Number Generation

```astra
fn roll_die() -> Int effects(Rand) {
  Rand.int(1, 6)
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `Rand.int(min, max)` | `(Int, Int) -> Int` | Random integer in range |
| `Rand.bool()` | `() -> Bool` | Random boolean |

### Env — Environment Variables

```astra
fn get_config() -> Text effects(Env) {
  Env.get("CONFIG_PATH")
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `Env.get(key)` | `(Text) -> Text` | Get environment variable |
| `Env.args()` | `() -> List[Text]` | Get command-line arguments |

## Effect Propagation

If function `A` calls function `B`, and `B` declares effects, then `A` must also declare those effects (or a superset):

```astra
fn inner() effects(Console) {
  Console.println("inner")
}

# Correct: caller declares Console because inner() uses it
fn outer() effects(Console) {
  inner()
}
```

Failing to declare a callee's effects produces error `E2001`:

```astra
fn broken() {
  inner()  # Error E2001: Effect 'Console' not declared in function signature
}
```

The fix is always to add the missing effect to your function's signature:

```astra
fn fixed() effects(Console) {
  inner()
}
```

Effects propagate up the entire call chain to `main()` or a test block.

## Effects with Contracts

Effects can be combined with `requires` and `ensures` clauses:

```astra
fn logged_divide(a: Int, b: Int) -> Int
  effects(Console)
  requires b != 0
  ensures result == a / b
{
  Console.println("Dividing...")
  a / b
}
```

The order is always: parameters → return type → effects → requires → ensures → body.

## Testing with Effects

The effects system enables **deterministic testing** — one of Astra's core design goals.

### Mocking Effects

Test blocks can inject mock capabilities using the `using effects(...)` clause:

```astra
test "fixed clock returns constant time"
  using effects(Clock = Clock.fixed(1700000000))
{
  let now = Clock.now()
  assert(now == 1700000000)
}

test "seeded rand is deterministic"
  using effects(Rand = Rand.seeded(42))
{
  let x = Rand.int(1, 100)
  let y = Rand.int(1, 100)
  assert_eq(x, 75)
  assert_eq(y, 72)
}
```

With `Clock.fixed(n)`, `Clock.now()` always returns `n` and `Clock.sleep()` is a no-op. With `Rand.seeded(n)`, random numbers follow a deterministic sequence.

### Multiple Mocked Effects

```astra
test "multiple deterministic effects"
  using effects(Clock = Clock.fixed(999), Rand = Rand.seeded(7))
{
  let time = Clock.now()
  let random = Rand.int(1, 10)
  assert_eq(time, 999)
  assert_eq(random, 8)
}
```

### Why This Matters

In other languages, tests involving time or randomness are inherently flaky:

```python
# Python - this test is non-deterministic
def test_expiry():
    token = create_token()
    time.sleep(1)
    assert token.is_expired()  # Flaky: depends on actual clock
```

In Astra, you control the clock:

```astra
test "token expires after timeout"
  using effects(Clock = Clock.fixed(1000))
{
  let token = create_token()
  # Clock is fixed - behavior is deterministic
  assert(token.is_expired(timeout = 500))
}
```

Same inputs, same outputs, every time. No flaky tests.

## Effect Errors

| Code | Description |
|------|-------------|
| `E2001` | Effect not declared in function signature |
| `E2002` | Unknown effect name |
| `E2003` | Capability not available in current scope |
| `E2004` | Effectful call from pure context |

See [Error Codes Reference](errors.md) for details on each error.

## Design Rationale

For the reasoning behind choosing explicit effects over monadic IO (Haskell-style) or no tracking at all, see [ADR-002: Effects Over Monads](adr/ADR-002-effects-over-monads.md).
