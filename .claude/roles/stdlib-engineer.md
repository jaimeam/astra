# Role: Standard Library Engineer

## Responsibility
Build and maintain the Astra standard library with core types and capability modules.

## Deliverables
- [ ] Core types: `Option`, `Result`
- [ ] Collections with deterministic semantics
- [ ] Text utilities
- [ ] Testing helpers: `assert`, `assert_eq`
- [ ] Capability modules: `Net`, `Fs`, `Clock`, `Rand`, `Env`, `Console`

## Key Files
- `stdlib/core.astra` - Core types and functions
- `stdlib/option.astra` - Option type and methods
- `stdlib/result.astra` - Result type and methods
- `stdlib/collections/list.astra` - List type
- `stdlib/collections/map.astra` - Map type (ordered)
- `stdlib/collections/set.astra` - Set type (ordered)
- `stdlib/text.astra` - Text utilities
- `stdlib/testing.astra` - Test assertions
- `stdlib/capabilities/net.astra` - Network capability
- `stdlib/capabilities/fs.astra` - Filesystem capability
- `stdlib/capabilities/clock.astra` - Time capability
- `stdlib/capabilities/rand.astra` - Random capability

## Core Types

### Option[T]
```astra
enum Option[T] =
  | Some(value: T)
  | None

fn map[T, U](self: Option[T], f: (T) -> U) -> Option[U]
fn and_then[T, U](self: Option[T], f: (T) -> Option[U]) -> Option[U]
fn unwrap_or[T](self: Option[T], default: T) -> T
fn is_some[T](self: Option[T]) -> Bool
fn is_none[T](self: Option[T]) -> Bool
```

### Result[T, E]
```astra
enum Result[T, E] =
  | Ok(value: T)
  | Err(error: E)

fn map[T, U, E](self: Result[T, E], f: (T) -> U) -> Result[U, E]
fn map_err[T, E, F](self: Result[T, E], f: (E) -> F) -> Result[T, F]
fn and_then[T, U, E](self: Result[T, E], f: (T) -> Result[U, E]) -> Result[U, E]
fn unwrap_or[T, E](self: Result[T, E], default: T) -> T
fn is_ok[T, E](self: Result[T, E]) -> Bool
fn is_err[T, E](self: Result[T, E]) -> Bool
```

## Capability Interfaces

### Net
```astra
module capabilities.net

type Response = { status: Int, body: Text, headers: Map[Text, Text] }

fn get(url: Text) -> Result[Response, NetError] effects(Net)
fn post(url: Text, body: Text) -> Result[Response, NetError] effects(Net)
fn post_json[T](url: Text, data: T) -> Result[Response, NetError] effects(Net)
```

### Clock
```astra
module capabilities.clock

type Instant = { seconds: Int, nanos: Int }

fn now() -> Instant effects(Clock)
fn sleep(duration: Duration) effects(Clock)
```

### Rand
```astra
module capabilities.rand

fn int(min: Int, max: Int) -> Int effects(Rand)
fn bool() -> Bool effects(Rand)
fn choose[T](list: List[T]) -> Option[T] effects(Rand)
fn shuffle[T](list: List[T]) -> List[T] effects(Rand)
```

## Acceptance Criteria
- No global side effects in stdlib
- All capability access requires declared effects
- Deterministic iteration order for collections
- Comprehensive docstrings on public APIs

## Testing Strategy
```bash
# Run stdlib tests
cargo test --test stdlib
```

## Common Pitfalls
- Non-deterministic collection iteration
- Missing edge case handling
- Inconsistent naming conventions
- Capability leakage through closures
