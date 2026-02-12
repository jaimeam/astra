# Astra Implementation Plan

> This document tracks the implementation status and coordinates parallel development across agents.
> **Last updated**: 2026-02-12

## Vision Alignment Check

**Astra's Core Value Proposition**: An LLM-native language with fast, deterministic feedback loops.

| Differentiator | Why It Matters for LLMs | Status |
|----------------|------------------------|--------|
| **Machine-readable diagnostics with fix suggestions** | LLMs can parse errors and apply fixes automatically | ✅ Codes + suggestions |
| **Explicit effects with enforcement** | LLMs see exactly what functions can do | ✅ Enforced in type checker |
| **Deterministic testing** | Tests never flake, LLMs trust results | ✅ `using effects()` works |
| **No null (Option/Result)** | Type system catches missing cases | ✅ Runtime works |
| **Exhaustive match checking** | Compiler catches forgotten cases | ✅ Implemented |
| **One canonical format** | No style choices to make | ✅ Formatter wired to CLI |
| **Contracts (requires/ensures)** | LLMs get pre/postcondition verification | ✅ Parsed + enforced at runtime |

---

## Priority Queue: LLM-Differentiating Features First

> **Rule**: Always prioritize features that make Astra better for LLMs than existing languages.

### 🔴 Critical Path (Enables the "LLM → check → fix → repeat" loop)

| # | Task | Impact | Status | Est. Time |
|---|------|--------|--------|-----------|
| **C1** | Option/Result runtime (Some/None/Ok/Err) | Unlocks null-free programming | ✅ Done | 2h |
| **C2** | Exhaustive match checking | Compiler catches missing cases | ✅ Done | 3h |
| **C3** | Error suggestions in diagnostics | LLMs can auto-apply fixes | ✅ Done | 4h |
| **C4** | Effect checking enforcement | Verify effects match declarations | ✅ Done | 4h |
| **C5** | Deterministic test effects (`using effects()`) | Inject mocked Clock/Rand | ✅ Done | 3h |

### 🟡 High Value (Improves LLM experience significantly)

| # | Task | Impact | Status | Est. Time |
|---|------|--------|--------|-----------|
| **H1** | `?` operator for Option/Result | Clean error propagation | ✅ Done | 1h |
| **H2** | `requires`/`ensures` parsing | Contract syntax | ✅ Done | 2h |
| **H3** | Contract runtime checks | Precondition/postcondition enforcement | ✅ Done | 2h |
| **H4** | Formatter wired to CLI | One canonical format via `astra fmt` | ✅ Done | 4h |
| **H5** | Type inference for let bindings | Less boilerplate | ⬜ Ready | 3h |

### 🟢 Nice to Have (General language features)

| # | Task | Impact | Status | Est. Time |
|---|------|--------|--------|-----------|
| **N1** | List literal syntax `[1, 2, 3]` | Convenience | ⬜ Ready | 2h |
| **N2** | `print` builtin (no newline) | Convenience | ⬜ Ready | 30m |
| **N3** | `len` and `to_text` builtins | Convenience | ⬜ Ready | 30m |
| **N4** | `if X then Y else Z` syntax | Alternative syntax | ⬜ Ready | 1h |

---

## Current Status Snapshot

### Completed ✅
- [x] Lexer with Logos
- [x] Recursive descent parser with AST
- [x] Diagnostics system with JSON output and stable error codes
- [x] Effect system data structures
- [x] Interpreter with full expression evaluation
- [x] CLI (run, check, test, fmt commands)
- [x] Test block parsing and execution
- [x] assert/assert_eq builtins
- [x] All 7 examples pass check and run
- [x] 73+ unit tests, 4 golden tests, 29+ Astra tests
- [x] Option/Result runtime (C1)
- [x] ? operator for Option/Result (H1)
- [x] Exhaustive match checking for Option/Result/Bool/enums (C2)
- [x] Error suggestions in diagnostics (C3)
- [x] Effect checking enforcement (C4)
- [x] Deterministic test effects with `using effects()` clause (C5)
- [x] Type checker wired into CLI check command
- [x] Function type resolution in type environment
- [x] Binary operator type inference (comparisons return Bool)
- [x] `requires`/`ensures` contract parsing (H2)
- [x] Contract runtime enforcement with E3001/E3002 errors (H3)
- [x] Formatter wired to `astra fmt` with check mode (H4)

### Not Started 🔴
- [ ] Type inference for let bindings (H5)

---

## Working Commands

```bash
cargo build                              # Build
cargo test                               # Run all tests (73+)
cargo run -- run examples/hello.astra    # Run a program
cargo run -- check examples/             # Check syntax + types + effects
cargo run -- test                        # Run test blocks (29+)
cargo run -- test "filter"               # Run matching tests
cargo run -- check --json file.astra     # JSON diagnostics with suggestions
cargo run -- fmt file.astra              # Format a file
cargo run -- fmt --check file.astra      # Check formatting without modifying
cargo run -- fmt .                       # Format all .astra files
```

---

## Session Log

| Date | Agent | Tasks Completed |
|------|-------|-----------------|
| 2026-01-26 | setup | Initial project structure, parser, interpreter |
| 2026-01-27 | claude | Test blocks, assert builtin, examples fixed, plan updated |
| 2026-01-27 | claude | C1: Option/Result runtime (Some/None/Ok/Err) |
| 2026-01-27 | claude | H1: ? operator for Option/Result |
| 2026-02-11 | claude | C2: Exhaustive match checking (Option/Result/Bool/enum) |
| 2026-02-11 | claude | C3: Error suggestions in diagnostics |
| 2026-02-11 | claude | C4: Effect checking enforcement |
| 2026-02-11 | claude | Type checker wired into CLI, function resolution, binary op fixes |
| 2026-02-11 | claude | C5: Deterministic test effects (using effects() clause) |
| 2026-02-12 | claude | H2: `requires`/`ensures` contract parsing |
| 2026-02-12 | claude | H3: Contract runtime checks (E3001 precondition, E3002 postcondition) |
| 2026-02-12 | claude | H4: Formatter wired to `astra fmt` CLI with check mode |

---

## For Next Agent

**Recommended task**: **H5 (Type inference for let bindings)**

This is the highest-impact remaining task because:
1. It reduces boilerplate - LLMs can write simpler code
2. `let x = 42` should infer `Int` without requiring `let x: Int = 42`
3. The type checker already has type inference infrastructure

**After H5, prioritize**:
- N1 (list literal syntax) - common data structure
- N2/N3 (print, len, to_text builtins) - basic I/O and introspection

**Avoid getting distracted by**:
- Advanced type system features (generics, traits)
- Performance optimizations
- Cross-module compilation

The goal is to make Astra demonstrably better for LLMs than Python/JS/Rust as quickly as possible.

---

## Appendix: File Locations

| Component | File | Tests |
|-----------|------|-------|
| Lexer | `src/parser/lexer.rs` | In-file |
| Parser | `src/parser/parser.rs` | In-file + golden |
| AST | `src/parser/ast.rs` | - |
| Type Checker | `src/typechecker/mod.rs` | In-file (16+) |
| Effects | `src/effects/mod.rs` | In-file |
| Interpreter | `src/interpreter/mod.rs` | In-file (25+) |
| CLI | `src/cli/mod.rs` | Integration |
| Diagnostics | `src/diagnostics/mod.rs` | In-file |
| Formatter | `src/formatter/mod.rs` | Via golden tests |
