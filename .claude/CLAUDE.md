# Astra Project - Claude Agent Guide

> This is the primary reference document for Claude agents working on the Astra programming language.

## Project Overview

**Astra** is an LLM/Agent-native programming language designed for:
- Fast, deterministic feedback loops
- Machine-actionable error diagnostics
- Capability-controlled, sandboxable execution
- Maximum verifiability with minimal ambiguity

## Quick Reference

### Key Commands
```bash
# Development (Rust toolchain)
cargo build              # Build the compiler/toolchain
cargo test               # Run all tests
cargo fmt                # Auto-format Rust code
cargo clippy --all-targets --all-features -- -D warnings  # Lint check
cargo run -- fmt         # Run Astra formatter
cargo run -- check       # Run type checker
cargo run -- test        # Run Astra tests
cargo run -- run <file>  # Execute Astra program

# Testing
cargo test --lib         # Unit tests only
cargo test --test golden # Golden file tests

# Pre-commit setup (run once after clone)
git config core.hooksPath .githooks
```

### Project Structure
```
astra/
├── .claude/             # Agent documentation (you are here)
├── .githooks/           # Git hooks (fmt, clippy, test pre-commit)
├── src/                 # Rust source for toolchain
│   ├── main.rs          # CLI entrypoint
│   ├── lib.rs           # Library root
│   ├── parser/          # Lexer + Parser + AST
│   ├── formatter/       # Canonical formatter
│   ├── typechecker/     # Type system
│   ├── effects/         # Effect system
│   ├── interpreter/     # Runtime/VM
│   ├── diagnostics/     # Error reporting
│   └── cli/             # Command-line interface
├── stdlib/              # Astra standard library (.astra files)
├── tests/               # Test suites
│   ├── syntax/          # Parser golden tests
│   ├── typecheck/       # Type checker tests
│   ├── effects/         # Effect system tests
│   ├── runtime/         # Interpreter tests
│   └── golden/          # Golden file snapshots
├── docs/                # Documentation
│   ├── spec.md          # Language specification
│   ├── errors.md        # Error codes reference
│   ├── formatting.md    # Formatting rules
│   └── adr/             # Architecture Decision Records
├── examples/            # Example Astra programs
├── Cargo.toml           # Rust project manifest
├── astra.toml           # Astra project manifest (for self-hosting tests)
└── README.md
```

## Agent Work Style

- **Always work to full completion.** When given a task, do not stop partway through. Continue working until every part of the request is fully implemented, all tests pass, and changes are committed and pushed.
- **Do not ask for confirmation to continue.** If a task has multiple steps or sub-tasks, work through all of them without pausing to ask if you should keep going.
- **Fix issues as you encounter them.** If tests fail, clippy warns, or something breaks, fix it immediately and keep going.

## Core Design Principles

1. **Verifiability First**: Wrong code fails early with precise errors
2. **Unambiguous Semantics**: One obvious way to express things
3. **Local Reasoning**: No spooky action-at-a-distance
4. **Safe by Default**: Capability-based I/O, sandboxable
5. **Fast Feedback**: Quick incremental checking and testing

## Contracts & Interfaces

See `.claude/contracts/` for interface definitions that must remain stable:
- `ast.md`: AST node structure and serialization
- `diagnostics.md`: Error format and codes
- `effects.md`: Effect and capability interfaces

## Common Patterns

See `.claude/patterns/` for recommended implementation patterns.

## Before You Start

1. **Set up git hooks**: `git config core.hooksPath .githooks`
2. **Read the relevant contract** for your area of work
3. **Check existing ADRs** in `docs/adr/` for design decisions
4. **Run tests** before and after changes: `cargo test`
5. **Follow formatting**: `cargo fmt` for Rust, canonical format for Astra
6. **Document decisions**: Add ADRs for non-trivial choices

## Pre-commit Checks

A pre-commit hook (`.githooks/pre-commit`) runs automatically on every commit.
It enforces three checks that must all pass before a commit is accepted:

1. **`cargo fmt -- --check`** — Code must be formatted. Run `cargo fmt` to fix.
2. **`cargo clippy --all-targets --all-features -- -D warnings`** — No clippy warnings allowed.
3. **`cargo test`** — All tests must pass.

Activate the hooks after cloning:
```bash
git config core.hooksPath .githooks
```

**Do not bypass these hooks** with `--no-verify`. If a check fails, fix the
issue and try the commit again.

## Error Codes

All diagnostics must have stable error codes. Format: `E####`
- `E0xxx`: Syntax/parsing errors
- `E1xxx`: Type errors
- `E2xxx`: Effect errors
- `E3xxx`: Contract violations
- `E4xxx`: Runtime errors

See `docs/errors.md` for the complete registry.

## Testing Requirements

- All new features need tests
- Golden tests for parser/formatter output
- Property tests for core algorithms where applicable
- Tests must be deterministic (seeded randomness, fixed time)
