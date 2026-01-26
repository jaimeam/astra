# Effects & Capabilities Contract

> This document defines the stable interface for the effect system and capabilities.

## Built-in Effects

| Effect | Description | Capability Module |
|--------|-------------|-------------------|
| `Net` | Network I/O | `capabilities.net` |
| `Fs` | Filesystem access | `capabilities.fs` |
| `Clock` | Time/date access | `capabilities.clock` |
| `Rand` | Random number generation | `capabilities.rand` |
| `Env` | Environment variables | `capabilities.env` |
| `Console` | Console I/O | `capabilities.console` |

## Effect Declaration Syntax

```astra
# Single effect
fn fetch(url: Text) -> Result[Text, Error]
  effects(Net)
{
  // ...
}

# Multiple effects
fn main()
  effects(Net, Console, Clock)
{
  // ...
}

# Pure function (no effects keyword)
fn add(a: Int, b: Int) -> Int {
  a + b
}
```

## Capability Interfaces

### Net Capability
```rust
trait NetCapability {
    fn get(&self, url: &str) -> Result<Response, NetError>;
    fn post(&self, url: &str, body: &str) -> Result<Response, NetError>;
    fn post_json<T: Serialize>(&self, url: &str, data: &T) -> Result<Response, NetError>;
}

struct Response {
    status: i32,
    body: String,
    headers: HashMap<String, String>,
}

enum NetError {
    ConnectionFailed,
    Timeout,
    InvalidUrl,
    StatusError(i32),
}
```

### Fs Capability
```rust
trait FsCapability {
    fn read(&self, path: &Path) -> Result<String, FsError>;
    fn write(&self, path: &Path, content: &str) -> Result<(), FsError>;
    fn exists(&self, path: &Path) -> bool;
    fn list_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FsError>;
    fn create_dir(&self, path: &Path) -> Result<(), FsError>;
    fn remove(&self, path: &Path) -> Result<(), FsError>;
}

enum FsError {
    NotFound,
    PermissionDenied,
    IoError(String),
}
```

### Clock Capability
```rust
trait ClockCapability {
    fn now(&self) -> Instant;
    fn sleep(&self, duration: Duration);
}

struct Instant {
    seconds: i64,
    nanos: u32,
}

struct Duration {
    seconds: u64,
    nanos: u32,
}
```

### Rand Capability
```rust
trait RandCapability {
    fn int(&self, min: i64, max: i64) -> i64;
    fn bool(&self) -> bool;
    fn float(&self) -> f64;  // 0.0 to 1.0
    fn bytes(&self, count: usize) -> Vec<u8>;
}
```

### Env Capability
```rust
trait EnvCapability {
    fn get(&self, name: &str) -> Option<String>;
    fn set(&self, name: &str, value: &str);
    fn args(&self) -> Vec<String>;
}
```

### Console Capability
```rust
trait ConsoleCapability {
    fn print(&self, text: &str);
    fn println(&self, text: &str);
    fn read_line(&self) -> Option<String>;
    fn eprint(&self, text: &str);
    fn eprintln(&self, text: &str);
}
```

## Effect Checking Rules

### Rule 1: Declaration Required
A function calling effectful operations must declare those effects.

```astra
# Error: E2001 - Net effect not declared
fn bad_fetch(url: Text) -> Text {
  Net.get(url).unwrap()  // Error!
}

# Correct
fn good_fetch(url: Text) -> Text
  effects(Net)
{
  Net.get(url).unwrap()
}
```

### Rule 2: Transitive Effects
Caller must declare all effects of callees.

```astra
fn helper() -> Int effects(Rand) {
  Rand.int(1, 100)
}

# Error: E2005 - Missing Rand effect
fn caller() -> Int {
  helper()  // Error!
}

# Correct
fn caller() -> Int effects(Rand) {
  helper()
}
```

### Rule 3: Effect Subsetting
A function can declare more effects than it uses (but linter warns).

```astra
# Warning: W0010 - Declared effect Net is unused
fn pure_but_declared(a: Int) -> Int
  effects(Net)
{
  a + 1
}
```

## Capability Injection for Tests

### Syntax
```astra
test "with mock network" {
  using effects(Net = MockNet.new())

  let result = fetch_data("http://example.com")
  assert result.is_ok()
}

test "with fixed time" {
  using effects(Clock = Clock.fixed(1234567890))

  let time = get_current_time()
  assert_eq(time.seconds, 1234567890)
}

test "with seeded random" {
  using effects(Rand = Rand.seeded(42))

  let a = Rand.int(1, 100)
  let b = Rand.int(1, 100)
  # Same seed always gives same sequence
  assert_eq(a, 67)
  assert_eq(b, 23)
}
```

### Mock Implementations

```rust
struct MockNet {
    responses: Vec<(String, Response)>,
    calls: RefCell<Vec<String>>,
}

impl MockNet {
    fn new() -> Self;
    fn returning(response: Response) -> Self;
    fn with_responses(responses: Vec<(String, Response)>) -> Self;
    fn calls(&self) -> Vec<String>;
}

struct FixedClock {
    instant: Instant,
}

impl FixedClock {
    fn fixed(seconds: i64) -> Self;
}

struct SeededRand {
    rng: StdRng,
}

impl SeededRand {
    fn seeded(seed: u64) -> Self;
}
```

## Type System Integration

Effects are part of function types:

```astra
# Type of fetch_data is:
# (Text) -> Result[Text, Error] effects(Net)

fn fetch_data(url: Text) -> Result[Text, Error]
  effects(Net)
{
  Net.get(url)
}

# Higher-order function with effect polymorphism (future feature)
fn map_with_effect[T, U, E](
  list: List[T],
  f: (T) -> U effects(E)
) -> List[U]
  effects(E)
{
  // ...
}
```

## Determinism Requirements

1. **Rand**: Always seeded in tests, seeded by default in main
2. **Clock**: Injectable, not global system time
3. **Fs/Net**: Mockable for tests
4. **Console**: Capturable for tests

## Future Considerations

- Effect polymorphism (parameterize over effects)
- Effect handlers (algebraic effects)
- Custom user-defined effects
- Effect inference within modules
