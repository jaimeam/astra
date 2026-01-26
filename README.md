# Astra

**Astra** is a programming language designed for LLMs and AI agents to write, verify, and maintain code.

> **Why not just use Python, JavaScript, or Rust?** See [Why Astra?](docs/why-astra.md) for the full rationale.

## The Problem

When LLMs generate code in existing languages, they face fundamental challenges:

| Language | Problems for LLMs |
|----------|-------------------|
| **Python/JS** | Runtime-only errors, non-deterministic tests, hidden side effects |
| **Rust** | Ownership complexity, human-oriented error messages |

**The result**: LLM generates code → it fails → error is ambiguous → LLM guesses at fix → cycle repeats.

## Astra's Solution

Astra provides **fast, deterministic feedback loops** designed for machine consumption:

- **Machine-readable diagnostics** with stable error codes and suggested fixes
- **Explicit effects** - function signatures declare all capabilities (Net, Fs, Clock, etc.)
- **Deterministic testing** - seeded randomness, mockable time, no flaky tests
- **One canonical format** - no style choices, the formatter decides everything
- **No null** - use `Option[T]` and exhaustive matching; compiler catches missing cases

```
LLM generates code → astra check → JSON errors with fix suggestions → LLM applies fixes → repeat until passing
```

## Status

🚧 **Early Development** - Astra is currently in the initial development phase (v0.1).

## Quick Start

```bash
# Build the toolchain
cargo build

# Format Astra code
cargo run -- fmt examples/

# Check for errors
cargo run -- check examples/

# Run tests
cargo run -- test examples/

# Run a program
cargo run -- run examples/hello.astra
```

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

## CLI Commands

| Command | Description |
|---------|-------------|
| `astra fmt [files...]` | Format files canonically |
| `astra check [files...]` | Parse + typecheck + lint |
| `astra test [filter]` | Run tests deterministically |
| `astra run <target>` | Run main entrypoint |
| `astra package` | Create distributable artifact |

## Documentation

- **[Why Astra?](docs/why-astra.md)** - The case for an LLM-native language
- [Getting Started](docs/getting-started.md)
- [Language Specification](docs/spec.md)
- [Error Codes Reference](docs/errors.md)
- [Formatting Rules](docs/formatting.md)
- [Effects System](docs/effects.md)

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

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
