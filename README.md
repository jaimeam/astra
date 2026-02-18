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

**v1.0** - Astra is ready for first projects. The language, toolchain, and standard library support real multi-file projects with type checking, effect tracking, testing, and formatting.

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

## Known Limitations (v1.0)

These are intentional limitations of the v1.0 release. They are documented here so users are not surprised.

**KL1. No Full Hindley-Milner Type Inference** - Astra v1.0 uses practical type inference rather than full Hindley-Milner. Generic type parameters use basic unification at call sites. Add explicit type annotations if the checker cannot infer types in complex generic scenarios.

**KL2. Traits Are Runtime-Dispatched** - Trait method calls are resolved at runtime, not compile time. The type checker validates trait impl blocks but does not resolve trait methods on arbitrary expressions. Incorrect trait usage is caught at runtime.

**KL3. No Concurrency** - Astra v1.0 is single-threaded. There is no async/await, no threads, and no parallelism. `async` and `await` are reserved keywords that produce errors. Concurrency is planned for a future version.

**KL4. Interpreted Only** - All execution is via a tree-walking interpreter. Performance is adequate for small and medium programs but not suitable for compute-heavy workloads. For performance-critical code, consider calling out to external tools via effects.

**KL5. No Package Manager / Registry** - There is no way to install third-party packages. Projects use only the standard library and their own modules. Organize code as modules within your project. A package system is planned for a future version.

**KL6. No Debugger** - There is no step-through debugger. Use `println` for debugging output, `assert`/`assert_eq` for runtime checks, and `test` blocks for verifying behavior.

### Deferred to v1.1

The following features are explicitly out of scope for v1.0 and planned for future versions:

- **Full JSON object parsing** - The `std.json` module provides `stringify`, `parse_int`, `parse_bool`, and `escape`. Full JSON-to-value parsing (objects, arrays) is deferred to v1.1.
- **Regular expression support** - String operations include `contains`, `starts_with`, `split`, `index_of`, and `replace`. Pattern matching via regex is deferred to v1.1.
- **Full Hindley-Milner type inference** - Constraint-based type solving for complex generic scenarios.
- **True async/await** - Event loop, futures, and concurrent effects.
- **Package registry** - Publishing and installing third-party packages.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
