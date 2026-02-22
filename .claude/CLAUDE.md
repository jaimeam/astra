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

## Critical Invariant: Interpreter-TypeChecker Sync

**Every built-in function in the interpreter MUST have a type signature in the type checker.**

The interpreter dispatches built-in functions in `src/interpreter/mod.rs` (the big
`match name.as_str()` block). The type checker recognizes built-in names in
`src/typechecker/mod.rs` (the `Expr::Ident` match). These two lists MUST stay in sync.

When adding a new built-in:
1. Add the runtime implementation in `src/interpreter/mod.rs`
2. Add the type signature in `src/typechecker/mod.rs` — use `Type::Function { params, ret, effects }`
   with concrete types, NOT `Type::Unknown`
3. If the built-in introduces a new type concept (like `Json`), add a `Type` enum variant —
   do NOT use `Type::Named("Foo", [])` for built-in types (it won't match concrete types)
4. Add tests covering both the runtime behavior AND the type checking

See `docs/adr/ADR-005-builtin-type-sync.md` for the full rationale (born from a bug where
`json_parse`/`json_stringify` had runtime implementations but no type checker entries).

## File Size & Organization

**Keep source files under ~500 lines of implementation code.** When a file grows beyond
this, split logically related code into submodules (see `src/interpreter/` for an example
of extracting `methods.rs`, `modules.rs`, `json.rs`, `regex.rs`, `pattern.rs`).

Guidelines:
- **Tests go in separate files**, not inline `#[cfg(test)] mod tests` blocks.
  - For directory modules (`foo/mod.rs`): create `foo/tests.rs`
  - For file modules (`foo.rs`): use `#[cfg(test)] #[path = "foo_tests.rs"] mod tests;`
    and create the sibling `foo_tests.rs`
- **Split `impl` blocks across files** when a struct has many methods — Rust allows
  multiple `impl` blocks for the same type across files in the same crate module.
- **Extract standalone functions** (those that don't take `&self`) into topic-specific
  submodules when they form a coherent group (e.g., JSON, regex, pattern matching).
- Use `pub(super)` or `pub(crate)` visibility for extracted code that shouldn't be
  part of the public API.

## Testing Requirements

- All new features need tests
- Golden tests for parser/formatter output
- Property tests for core algorithms where applicable
- Tests must be deterministic (seeded randomness, fixed time)
- **Tests live in separate files** (see "File Size & Organization" above)
