# Astra

**Astra** is a programming language designed for LLMs and AI agents to write, verify, and maintain code.

> **Why not just use Python, TypeScript, Go, or Rust?** See [Why Astra?](docs/why-astra.md) for the full rationale.

## The Problem

When LLMs generate code, they enter a feedback loop: generate, check for errors, interpret diagnostics, fix, repeat. Existing languages work for this — agents write Rust, TypeScript, Go, and Python every day — but none were designed with this loop as the primary use case.

Three sources of friction slow every mainstream language:

1. **Side effects are invisible.** No mainstream language tracks I/O, network, or clock access in function signatures. Agents must read implementations to know what a function actually does.
2. **Diagnostics are human-first.** Even languages with structured error output (Rust's `--message-format=json`, TypeScript's stable codes) don't consistently bundle machine-actionable fix suggestions with exact edit locations.
3. **Test determinism is opt-in.** Flaky tests from time, randomness, or I/O are a discipline problem everywhere. The language doesn't prevent them.

## Astra's Solution

Astra is designed around three capabilities that no single mainstream language provides together:

- **Mandatory effect tracking** - function signatures declare all capabilities (`Net`, `Fs`, `Clock`); the compiler rejects undeclared effects
- **Agent-oriented diagnostics** - every error includes structured JSON with stable codes and suggested fixes with exact edit locations
- **Enforced test determinism** - effects must be mocked in tests; seeded randomness and fixed clocks are the default, not opt-in

Plus the building blocks you'd expect:

- **No null** - `Option[T]` with exhaustive matching; compiler catches missing cases
- **Typed error handling** - `Result[T, E]` with `?` and `?else` for concise propagation
- **Canonical formatting** - mandatory built-in formatter, no configuration
- **JSON / regex** - built into the standard library
- **15 stdlib modules** - from `std.math` to `std.datetime` and `std.path`

```
LLM generates code -> astra check -> JSON errors with fix suggestions -> LLM applies fixes -> repeat until passing
```

## Quick Start

```bash
# Build the toolchain
cargo build --release

# Add to PATH (or use 'cargo run --' instead of 'astra')
export PATH="$PATH:$(pwd)/target/release"

# Create a new project
astra init my_project && cd my_project

# Run, check, and test
astra run src/main.astra
astra check src/
astra test
```

`astra init` scaffolds everything you need including a `.claude/CLAUDE.md` that
makes the project immediately usable with [Claude Code](https://docs.anthropic.com/en/docs/claude-code)
and other AI agents. See [Getting Started](docs/getting-started.md) for the full tutorial.

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

### Planned for v1.1

- **Async/Await** — `async fn` and `await` syntax is reserved; full implementation coming in v1.1
- **Package Manager** — `astra pkg` command exists; dependency resolution coming in v1.1
- See [ADR-007](docs/adr/ADR-007-defer-async-pkg-to-v1.1.md) for rationale

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
| `astra pkg` | Package management (v1.1) |

## Documentation

- **[Why Astra?](docs/why-astra.md)** — The case for an LLM-native language
- [Getting Started](docs/getting-started.md) — Tutorial for your first Astra program
- [Astra by Example](docs/examples.md) — Cookbook of common patterns and idioms
- [Language Specification](docs/spec.md) — Complete syntax and semantics reference
- [Formal Grammar](docs/grammar.md) — EBNF grammar for the language
- [Effects System](docs/effects.md) — Guide to Astra's capability-based effects
- [Testing Guide](docs/testing.md) — How to write and run tests
- [Standard Library](docs/stdlib.md) — API reference for built-in types and functions
- [Error Codes Reference](docs/errors.md) — All error codes with examples and fixes
- [Formatting Rules](docs/formatting.md) — Canonical formatting specification
- [Performance](docs/performance.md) — Performance characteristics and guidance
- [Stability Guarantee](docs/stability.md) — v1.0 stability promises
- [Changelog](CHANGELOG.md) — Version history

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

## Known Limitations (v1.0)

- **Interpreted only** — Tree-walking interpreter. Adequate for scripts and tools, not for compute-heavy workloads. See [docs/performance.md](docs/performance.md).
- **Traits are runtime-dispatched** — The type checker validates trait impl blocks, but trait method calls are resolved at runtime.
- **No debugger** — Use `println`, `assert`/`assert_eq`, and `test` blocks for debugging.
- **Async/await not yet functional** — Syntax is reserved for v1.1.
- **Package manager not yet functional** — `astra pkg` exists but resolution is not implemented until v1.1.

See [docs/stability.md](docs/stability.md) for the v1.0 stability guarantee.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
