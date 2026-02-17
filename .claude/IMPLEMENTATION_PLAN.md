# Astra Implementation Plan

> Tracks implementation status. For the complete v1.0 feature requirements, see **[V1_REQUIREMENTS.md](V1_REQUIREMENTS.md)**.
>
> **Last updated**: 2026-02-17

---

## Current State: v0.1 (Pre-release)

**What's built**: Full interpreter toolchain — parser, type checker, formatter, interpreter, effects system, CLI, LSP, 12 stdlib modules, 14 examples, 427 tests passing.

**What's missing for v1.0**: See [V1_REQUIREMENTS.md](V1_REQUIREMENTS.md) for the exhaustive list.

### Codebase Statistics

| Metric | Value |
|--------|-------|
| Rust source lines | ~19,800 |
| Rust unit tests | 320 |
| Golden tests | 4 |
| Astra integration tests | 103 |
| Example programs | 14 |
| Stdlib modules | 12 |
| Built-in functions/methods | 137+ |
| Error/warning codes | 55 |
| CLI commands | 11 |

### Component Status

| Component | Status | Key Files |
|-----------|--------|-----------|
| Lexer | Complete | `src/parser/lexer.rs` |
| Parser | Complete | `src/parser/parser.rs` |
| AST | Complete | `src/parser/ast.rs` |
| Type Checker | Working (single-file) | `src/typechecker/mod.rs` |
| Effects System | Complete | `src/effects/mod.rs` |
| Interpreter | Complete | `src/interpreter/mod.rs` + submodules |
| Formatter | Complete | `src/formatter/mod.rs` |
| Diagnostics | Complete | `src/diagnostics/mod.rs` |
| CLI | Complete | `src/cli/mod.rs` |
| LSP | Working (single-file) | `src/lsp/mod.rs` |
| Cache | Basic | `src/cache/mod.rs` |

---

## Completed Feature Phases (Historical)

All phases from the original plan (P1-P8) are complete. Key milestones:

- **Phase 1** (Core Language): range, break/continue, while, else-if, string interpolation, float, tuples, maps, type aliases, return, math builtins, negative literals
- **Phase 2** (Type System): Generic unification, traits/impls, type invariants, generic constraints, recursive types
- **Phase 3** (Stdlib): List/string methods, conversions, Set type, math/json/io/iter/error modules
- **Phase 4** (Modules): Named imports, circular detection, re-exports, stdlib resolution
- **Phase 5** (Testing): Property tests, stack traces, error recovery, custom asserts
- **Phase 6** (Advanced): Pipe operator, user-defined effects, mutable state, TCO, async/await syntax
- **Phase 7** (Tooling): REPL, package command, LSP server
- **Phase 8** (LLM Workflow): `astra fix`, `astra explain`, watch mode, unused function detection

---

## v1.0 Remaining Work

See [V1_REQUIREMENTS.md](V1_REQUIREMENTS.md) for full details. Summary:

### Tier 1: Blockers (5 items)
| ID | Task | Effort |
|----|------|--------|
| B1 | Cross-file type checking | Large |
| B2 | Remove/error on async/await | Small |
| B3 | Proper exit codes | Small |
| B4 | Type checker false positives audit | Medium |
| B5 | Runtime error source locations | Small |

### Tier 2: Expected Features (10 items)
| ID | Task | Effort |
|----|------|--------|
| E1 | Compound assignment operators (+=, -=) | Small |
| E2 | Index access for lists (list[i]) | Small |
| E3 | String concatenation clarity | Verify |
| E4 | Nested pattern matching | Verify |
| E5 | Closure variable capture | Verify |
| E6 | User-defined error types with Result | Verify |
| E7 | Map/Set literal syntax | Design decision |
| E8 | For loop destructuring | Small |
| E9 | Multi-line strings / escape sequences | Verify |
| E10 | Comparison operations for all types | Verify |

### Tier 3: Polish (11 items)
P1-P11 covering return type inference, error messages, REPL improvements, test output, doc comments, JSON output, LSP cross-file, and stdlib additions.

---

## Working Commands

```bash
cargo build                              # Build
cargo test                               # Run all tests (427)
cargo run -- run examples/hello.astra    # Run a program
cargo run -- check examples/             # Check all examples
cargo run -- check --watch .             # Watch mode
cargo run -- test                        # Run Astra test blocks (103)
cargo run -- test --watch                # Watch mode for tests
cargo run -- fmt .                       # Format all .astra files
cargo run -- fix .                       # Auto-apply diagnostic fixes
cargo run -- fix --dry-run .             # Preview fixes
cargo run -- explain E1001               # Explain an error code
cargo run -- repl                        # Interactive REPL
cargo run -- init my_project             # Scaffold new project
cargo run -- doc .                       # Generate API docs
cargo run -- lsp                         # Start LSP server
```

---

## File Locations

| Component | File | Tests |
|-----------|------|-------|
| Lexer | `src/parser/lexer.rs` | In-file |
| Parser | `src/parser/parser.rs` | In-file + golden |
| AST | `src/parser/ast.rs` | — |
| Type Checker | `src/typechecker/mod.rs` | In-file (30+) |
| Effects | `src/effects/mod.rs` | In-file |
| Interpreter | `src/interpreter/mod.rs` + submodules | In-file (150+) |
| CLI | `src/cli/mod.rs` | In-file (6) |
| Diagnostics | `src/diagnostics/mod.rs` | In-file (12+) |
| Formatter | `src/formatter/mod.rs` | In-file (21) + golden |
| LSP | `src/lsp/mod.rs` | — |
| Manifest | `src/manifest/mod.rs` | In-file |
| Testing | `src/testing/mod.rs` | In-file |

### Stdlib Modules (12)
| Module | Path |
|--------|------|
| `std.core` | `stdlib/core.astra` |
| `std.math` | `stdlib/math.astra` |
| `std.string` | `stdlib/string.astra` |
| `std.collections` | `stdlib/collections.astra` |
| `std.list` | `stdlib/list.astra` |
| `std.option` | `stdlib/option.astra` |
| `std.result` | `stdlib/result.astra` |
| `std.prelude` | `stdlib/prelude.astra` |
| `std.json` | `stdlib/json.astra` |
| `std.io` | `stdlib/io.astra` |
| `std.iter` | `stdlib/iter.astra` |
| `std.error` | `stdlib/error.astra` |

### Examples (14)
| Example | Features |
|---------|----------|
| `hello.astra` | Console effect |
| `option_handling.astra` | Option, pattern matching |
| `result_chaining.astra` | Result, ? operator |
| `effects_demo.astra` | Effect declarations |
| `deterministic_tests.astra` | Mocked Clock |
| `fibonacci.astra` | Recursion |
| `contracts.astra` | requires/ensures |
| `for_loops.astra` | For loops, mutable state |
| `generics.astra` | Generic functions, HOFs |
| `while_loops.astra` | While, break, return |
| `string_interp.astra` | String interpolation |
| `tuples_and_maps.astra` | Tuples, maps, sets |
| `tail_recursion.astra` | TCO |
| `traits_and_types.astra` | Traits, type aliases, invariants |
