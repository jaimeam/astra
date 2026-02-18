# Astra

**Astra** is a programming language designed for LLMs and AI agents to write, verify, and maintain code.

> **Why not just use Python, TypeScript, Go, or Rust?** See [Why Astra?](docs/why-astra.md) for the full rationale.

## The Problem

When LLMs generate code in existing languages, they face fundamental challenges:

| Language | Problems for LLMs |
|----------|-------------------|
| **Python/JS** | Runtime-only errors, non-deterministic tests, hidden side effects |
| **TypeScript** | Opt-in null safety, no effect tracking, non-deterministic tests |
| **Go** | No sum types, no pattern matching, verbose error handling, nil panics |
| **Rust** | Ownership complexity, human-oriented error messages |

**The result**: LLM generates code -> it fails -> error is ambiguous -> LLM guesses at fix -> cycle repeats.

## Astra's Solution

Astra provides **fast, deterministic feedback loops** designed for machine consumption:

- **Machine-readable diagnostics** with stable error codes and suggested fixes
- **Explicit effects** - function signatures declare all capabilities (Net, Fs, Clock, etc.)
- **Deterministic testing** - seeded randomness, mockable time, no flaky tests
- **One canonical format** - no style choices, the formatter decides everything
- **No null** - use `Option[T]` and exhaustive matching; compiler catches missing cases
- **Full JSON support** - parse and stringify JSON natively via `std.json`
- **Regular expressions** - pattern matching, replacement, and splitting via `std.regex`
- **Async/await** - declare `async` functions and `await` their results
- **Package management** - manage dependencies with `astra pkg`

```
LLM generates code -> astra check -> JSON errors with fix suggestions -> LLM applies fixes -> repeat until passing
```

## Quick Start

```bash
# Build the toolchain
cargo build --release

# Add to PATH (or use 'cargo run --' instead of 'astra')
export PATH="$PATH:$(pwd)/target/release"

# Run a program
astra run examples/hello.astra

# Check for errors
astra check examples/

# Run tests
astra test

# Format code
astra fmt examples/
```

See [Getting Started](docs/getting-started.md) for the full tutorial.

## Language Example

```astra
module payments

type Money = { currency: Currency, cents: Int }
  invariant cents >= 0

enum ChargeError =
  | InvalidAmount
  | NetworkFailure
  | Declined(reason: Text)

public fn charge(req: { customer: CustomerId, amount: Money })
  -> Result[{ id: ReceiptId, amount: Money }, ChargeError]
  effects(Net, Clock)
  requires req.amount.cents > 0
  ensures result.is_ok() implies result.ok.amount == req.amount
{
  let token = Net.env("PAYMENTS_TOKEN") ?else return Err(NetworkFailure)

  let res =
    Net.post_json("https://api.example/charge", req, headers = { "Auth": token })
      ?else return Err(NetworkFailure)

  match res.status {
    200 => Ok({ id = res.json.id, amount = req.amount })
    402 => Err(Declined(res.json.reason))
    _   => Err(NetworkFailure)
  }
}

test "rejects zero amount" {
  let req = { customer = "c1", amount = { currency = "EUR", cents = 0 } }
  assert charge(req).is_err()
}
```

## Core Principles

1. **Verifiability First** - Wrong code fails early with precise, machine-actionable errors
2. **Unambiguous Semantics** - One obvious way to express things; formatter is mandatory
3. **Local Reasoning** - Modules are explicit; no spooky action-at-a-distance
4. **Safe Execution by Default** - Capability-based I/O; sandbox-friendly
5. **Fast Incremental Iteration** - Check and test are fast, deterministic, and stable

## Key Features

### JSON Support

The `std.json` module provides complete JSON parsing and stringification. Use `json_parse(text)` to parse any JSON string into Astra values (objects become Records, arrays become Lists), and `json_stringify(value)` to convert any Astra value to JSON.

```astra
let data = json_parse("{\"name\": \"Astra\", \"version\": 1}")
assert_eq(data.name, "Astra")

let json = json_stringify({ items = [1, 2, 3], ok = true })
```

### Regular Expressions

The `std.regex` module and text methods support pattern matching. Use `regex_match`, `regex_find_all`, `regex_replace`, `regex_split`, and `regex_is_match` as builtins, or use text methods like `.matches()`, `.replace_pattern()`, `.split_pattern()`, and `.find_pattern()`.

```astra
import std.regex { is_match, find_all, replace }

let valid = is_match("^\\d{3}-\\d{4}$", "555-1234")
let cleaned = replace("\\s+", "  too   many   spaces  ", " ")
let words = "hello world".split_pattern("\\s+")
```

### Async/Await

Functions can be declared `async` and their results can be `await`ed. Calling an async function returns a `Future` value; `await` resolves it.

```astra
async fn fetch_data(url: Text) -> Text {
  Net.get(url)
}

fn main() effects(Net) {
  let data = await fetch_data("https://api.example.com/data")
  println(data)
}
```

### Package Management

The `astra pkg` commands manage dependencies declared in `astra.toml`. Supports path, git, and registry dependencies with lockfile generation.

```bash
astra pkg add mylib --version "1.0"
astra pkg add local-lib --path "../lib"
astra pkg add remote-lib --git "https://github.com/example/lib"
astra pkg install
astra pkg list
astra pkg remove mylib
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `astra run <file>` | Execute an Astra program |
| `astra check [files...]` | Parse + typecheck + lint |
| `astra test [filter]` | Run tests deterministically |
| `astra fmt [files...]` | Format files canonically |
| `astra fix [files...]` | Auto-apply diagnostic suggestions |
| `astra explain <code>` | Explain an error code |
| `astra repl` | Interactive REPL |
| `astra init <name>` | Scaffold a new project |
| `astra doc [files...]` | Generate API documentation |
| `astra lsp` | Start LSP server |
| `astra pkg install` | Install dependencies from astra.toml |
| `astra pkg add <name>` | Add a dependency |
| `astra pkg remove <name>` | Remove a dependency |
| `astra pkg list` | List installed packages |

## Documentation

- **[Why Astra?](docs/why-astra.md)** - The case for an LLM-native language
- [Getting Started](docs/getting-started.md) - Tutorial for your first Astra program
- [Astra by Example](docs/examples.md) - Cookbook of common patterns and idioms
- [Language Specification](docs/spec.md) - Complete syntax and semantics reference
- [Effects System](docs/effects.md) - Guide to Astra's capability-based effects
- [Testing Guide](docs/testing.md) - How to write and run tests
- [Standard Library](docs/stdlib.md) - API reference for built-in types and functions
- [Error Codes Reference](docs/errors.md) - All error codes with examples and fixes
- [Formatting Rules](docs/formatting.md) - Canonical formatting specification

## Project Structure

```
astra/
├── src/                 # Rust source for toolchain
│   ├── parser/          # Lexer + Parser + AST
│   ├── formatter/       # Canonical formatter
│   ├── typechecker/     # Type system
│   ├── effects/         # Effect system
│   ├── interpreter/     # Runtime/VM
│   └── cli/             # Command-line interface
├── stdlib/              # Astra standard library
├── tests/               # Test suites
├── docs/                # Documentation
└── examples/            # Example programs
```

## Known Limitations

- **Traits are runtime-dispatched** - Trait method calls are resolved at runtime, not compile time. The type checker validates trait impl blocks but does not resolve trait methods on arbitrary expressions. Incorrect trait usage is caught at runtime.

- **Interpreted only** - All execution is via a tree-walking interpreter. Performance is adequate for small and medium programs but not suitable for compute-heavy workloads. For performance-critical code, consider calling out to external tools via effects.

- **No debugger** - There is no step-through debugger. Use `println` for debugging output, `assert`/`assert_eq` for runtime checks, and `test` blocks for verifying behavior.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
