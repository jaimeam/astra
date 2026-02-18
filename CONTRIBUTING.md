# Contributing to Astra

## Prerequisites

- [Rust](https://rustup.rs/) 1.70 or later
- Git

## Building

```bash
git clone https://github.com/jaimeam/astra.git
cd astra
cargo build
```

## Running Tests

```bash
# Run all tests (Rust unit tests + golden tests)
cargo test

# Run only Rust unit tests
cargo test --lib

# Run only golden file tests
cargo test --test golden

# Run Astra test blocks in example files
cargo run -- test examples/
```

## Project Structure

```
src/
├── main.rs          # CLI entrypoint
├── lib.rs           # Library root
├── parser/          # Lexer + Parser + AST
├── formatter/       # Canonical formatter
├── typechecker/     # Type system
├── effects/         # Effect system
├── interpreter/     # Runtime/VM
├── diagnostics/     # Error reporting
├── cli/             # Command-line interface
├── manifest/        # Project manifest (astra.toml)
└── testing/         # Test framework
```

## Development Workflow

1. Create a feature branch
2. Make your changes
3. Run `cargo fmt` to format Rust code
4. Run `cargo test` to verify all tests pass
5. Submit a pull request

## Code Style

### Rust Code

- Run `cargo fmt` before committing
- Run `cargo clippy` for linting
- Follow standard Rust naming conventions

### Astra Code

- Use the canonical formatter: `cargo run -- fmt <file>`
- 2 spaces for indentation, no tabs
- 100 character line limit
- See [Formatting Rules](docs/formatting.md) for complete details

## Adding Error Codes

All compiler diagnostics must have a stable error code:

- `E0xxx` — Syntax/parsing errors
- `E1xxx` — Type errors
- `E2xxx` — Effect errors
- `E3xxx` — Contract violations
- `E4xxx` — Runtime errors
- `W0xxx` — Warnings

When adding a new error code:
1. Register it in `docs/errors.md`
2. Use the next available number in the appropriate range
3. Include an example and fix in the documentation

## Adding Tests

### Rust Unit Tests

Add tests in the same file as the code being tested using `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // ...
    }
}
```

### Golden Tests

Golden tests compare compiler output against snapshot files. Add new `.astra` files to the appropriate `tests/` subdirectory:

- `tests/syntax/` — Parser output tests
- `tests/typecheck/` — Type checker tests
- `tests/effects/` — Effect system tests
- `tests/runtime/` — Interpreter tests

### Astra Test Blocks

Add `test` blocks directly in `.astra` files:

```astra
fn my_function(x: Int) -> Int { x + 1 }

test "my function adds one" {
  assert_eq(my_function(0), 1)
  assert_eq(my_function(41), 42)
}
```

## Architecture Decisions

Non-trivial design decisions are recorded as Architecture Decision Records (ADRs) in `docs/adr/`. When making a significant design choice:

1. Create a new file: `docs/adr/ADR-NNN-short-title.md`
2. Document the context, decision, and consequences
3. Reference the ADR in related code comments

## Documentation

- Language specification: `docs/spec.md`
- Error codes: `docs/errors.md`
- Effects system: `docs/effects.md`
- Testing guide: `docs/testing.md`
- Standard library: `docs/stdlib.md`
- Formatting rules: `docs/formatting.md`
- Getting started: `docs/getting-started.md`
- Design rationale: `docs/why-astra.md`

## License

By contributing, you agree that your contributions will be licensed under the same terms as the project (Apache 2.0 or MIT, at your option).
