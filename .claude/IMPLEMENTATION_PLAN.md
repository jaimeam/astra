# Astra Implementation Plan

> This document tracks the implementation status and coordinates parallel development across agents.
> **Last updated**: 2026-02-16

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
| **H5** | Type inference for let bindings | Less boilerplate | ✅ Done | 3h |

### 🟢 Nice to Have (General language features)

| # | Task | Impact | Status | Est. Time |
|---|------|--------|--------|-----------|
| **N1** | List literal syntax `[1, 2, 3]` | Convenience | ✅ Done | 2h |
| **N2** | `print`/`println` builtins | Convenience I/O without effects | ✅ Done | 30m |
| **N3** | `len` and `to_text` builtins | Convenience | ✅ Done | 30m |
| **N4** | `if X then Y else Z` syntax | Alternative syntax | ✅ Done | 1h |

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
- [x] 92 unit tests, 4 golden tests, 33 Astra tests
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
- [x] Type inference for let bindings (H5)
- [x] List literal syntax `[1, 2, 3]` with methods (N1)
- [x] `print`/`println` builtins (N2)
- [x] `len` and `to_text` builtins (N3)
- [x] `if X then Y else Z` expression syntax (N4)

### Not Started 🔴
- [ ] Generic functions (`fn identity[T](x: T) -> T`)
- [ ] Lambda/closure expressions (`fn(x) { x + 1 }`)
- [ ] Multi-field variant patterns (`Rectangle(w, h)`)
- [ ] Record destructuring patterns (`{x, y}`)
- [ ] Function type syntax in expressions (`(Int) -> Int`)
- [ ] For loops / iterators
- [ ] Module system (imports across files)

---

## Working Commands

```bash
cargo build                              # Build
cargo test                               # Run all tests (92+)
cargo run -- run examples/hello.astra    # Run a program
cargo run -- check examples/             # Check syntax + types + effects
cargo run -- test                        # Run test blocks (33+)
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
| 2026-02-16 | claude | H5: Type inference for let bindings (verified working) |
| 2026-02-16 | claude | N1: List literal syntax with `get`, `contains`, `len` methods |
| 2026-02-16 | claude | N2: `print`/`println` builtins (no effect required) |
| 2026-02-16 | claude | N3: `len` and `to_text` builtins + `Text.len()` method |
| 2026-02-16 | claude | N4: `if X then Y else Z` inline expression syntax |

---

## For Next Agent

**Recommended tasks** (in priority order):

1. **Generic functions** (`fn identity[T](x: T) -> T`)
   - The parser already has type parameter parsing for types/enums
   - Needs: function type parameter parsing, type instantiation in checker
   - Impact: Enables stdlib functions like `map`, `filter`, `fold`

2. **Lambda/closure expressions** (`fn(x) { x + 1 }`)
   - Enables functional programming patterns (map/filter/fold)
   - The interpreter already supports closures
   - Needs: parser support for anonymous `fn` expressions

3. **Multi-field variant patterns** (`Rectangle(w, h)`)
   - Currently only single-field variants can be destructured
   - Needs: parser + pattern matcher updates

4. **For loops / iterators**
   - `for x in list { ... }` iteration
   - Essential for practical programming with lists

**Avoid getting distracted by**:
- Performance optimizations
- Cross-module compilation
- Advanced type system features (traits, type classes)

The goal is to make Astra demonstrably better for LLMs than Python/JS/Rust as quickly as possible.

---

## Appendix: File Locations

| Component | File | Tests |
|-----------|------|-------|
| Lexer | `src/parser/lexer.rs` | In-file |
| Parser | `src/parser/parser.rs` | In-file + golden |
| AST | `src/parser/ast.rs` | - |
| Type Checker | `src/typechecker/mod.rs` | In-file (17+) |
| Effects | `src/effects/mod.rs` | In-file |
| Interpreter | `src/interpreter/mod.rs` | In-file (44+) |
| CLI | `src/cli/mod.rs` | Integration |
| Diagnostics | `src/diagnostics/mod.rs` | In-file |
| Formatter | `src/formatter/mod.rs` | Via golden tests |
