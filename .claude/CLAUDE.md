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
cargo run -- fmt         # Run formatter
cargo run -- check       # Run type checker
cargo run -- test        # Run Astra tests
cargo run -- run <file>  # Execute Astra program

# Testing
cargo test --lib         # Unit tests only
cargo test --test golden # Golden file tests
```

### Project Structure
```
astra/
├── .claude/             # Agent documentation (you are here)
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

## Core Design Principles

1. **Verifiability First**: Wrong code fails early with precise errors
2. **Unambiguous Semantics**: One obvious way to express things
3. **Local Reasoning**: No spooky action-at-a-distance
4. **Safe by Default**: Capability-based I/O, sandboxable
5. **Fast Feedback**: Quick incremental checking and testing

## Agent Roles

See `.claude/roles/` for detailed role descriptions:
- **Language Architect**: Semantics + specification
- **Parser Engineer**: Grammar + AST + error recovery
- **Formatter Engineer**: Canonical formatting
- **Type System Engineer**: Type checker + inference
- **Effects Engineer**: Capability system
- **Runtime Engineer**: Interpreter/VM
- **CLI Engineer**: Toolchain commands
- **Stdlib Engineer**: Standard library
- **Testing Engineer**: Test framework + property testing
- **Docs Engineer**: Documentation + examples

## Contracts & Interfaces

See `.claude/contracts/` for interface definitions that must remain stable:
- `ast.md`: AST node structure and serialization
- `diagnostics.md`: Error format and codes
- `effects.md`: Effect and capability interfaces

## Common Patterns

See `.claude/patterns/` for recommended implementation patterns.

## Before You Start

1. **Read the relevant contract** for your area of work
2. **Check existing ADRs** in `docs/adr/` for design decisions
3. **Run tests** before and after changes: `cargo test`
4. **Follow formatting**: `cargo fmt` for Rust, canonical format for Astra
5. **Document decisions**: Add ADRs for non-trivial choices

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
