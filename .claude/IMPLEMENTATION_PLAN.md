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
| **Generic functions** | Type-safe reusable code | ✅ `fn identity[T](x: T) -> T` |
| **For loops** | Practical iteration over collections | ✅ `for x in list { ... }` |
| **Multi-field variants** | Ergonomic enum destructuring | ✅ `Rectangle(w, h)` |
| **Module system** | Cross-file code organization | ✅ Basic imports work |

---

## Priority Queue: LLM-Differentiating Features First

> **Rule**: Always prioritize features that make Astra better for LLMs than existing languages.

### 🔴 Critical Path (Enables the "LLM → check → fix → repeat" loop)

| # | Task | Impact | Status |
|---|------|--------|--------|
| **C1** | Option/Result runtime (Some/None/Ok/Err) | Unlocks null-free programming | ✅ Done |
| **C2** | Exhaustive match checking | Compiler catches missing cases | ✅ Done |
| **C3** | Error suggestions in diagnostics | LLMs can auto-apply fixes | ✅ Done |
| **C4** | Effect checking enforcement | Verify effects match declarations | ✅ Done |
| **C5** | Deterministic test effects (`using effects()`) | Inject mocked Clock/Rand | ✅ Done |

### 🟡 High Value (Improves LLM experience significantly)

| # | Task | Impact | Status |
|---|------|--------|--------|
| **H1** | `?` operator for Option/Result | Clean error propagation | ✅ Done |
| **H2** | `requires`/`ensures` parsing | Contract syntax | ✅ Done |
| **H3** | Contract runtime checks | Precondition/postcondition enforcement | ✅ Done |
| **H4** | Formatter wired to CLI | One canonical format via `astra fmt` | ✅ Done |
| **H5** | Type inference for let bindings | Less boilerplate | ✅ Done |

### 🟢 Nice to Have (General language features)

| # | Task | Impact | Status |
|---|------|--------|--------|
| **N1** | List literal syntax `[1, 2, 3]` | Convenience | ✅ Done |
| **N2** | `print`/`println` builtins | Convenience I/O without effects | ✅ Done |
| **N3** | `len` and `to_text` builtins | Convenience | ✅ Done |
| **N4** | `if X then Y else Z` syntax | Alternative syntax | ✅ Done |

### 🔵 Language Completeness (Implemented in current session)

| # | Task | Impact | Status |
|---|------|--------|--------|
| **L1** | Lambda/closure expressions | Higher-order programming | ✅ Done |
| **L2** | Higher-order functions (map/filter/fold) | Functional programming patterns | ✅ Done |
| **L3** | Function type syntax `(Int) -> Int` | First-class function types | ✅ Done |
| **L4** | Record destructuring patterns | Ergonomic record access | ✅ Done |
| **L5** | Pattern guards (`n if n > 0`) | Expressive pattern matching | ✅ Done |
| **L6** | String methods (split, contains, etc.) | Text manipulation | ✅ Done |
| **L7** | Generic functions `fn id[T](x: T) -> T` | Type-safe reuse | ✅ Done |
| **L8** | For loops `for x in list { ... }` | Imperative iteration | ✅ Done |
| **L9** | Multi-field variant destructuring | Ergonomic enums | ✅ Done |
| **L10** | Basic module system (imports) | Cross-file organization | ✅ Done |
| **L11** | Assignment in blocks (`x = x + 1`) | Mutable state in loops | ✅ Done |

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
- [x] All 9 examples pass check and run
- [x] 148 unit tests, 4 golden tests, 42+ Astra tests
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
- [x] Lambda/closure expressions `fn(x) { x + 1 }` (L1)
- [x] Higher-order list methods: map, filter, fold, each, any, all, flat_map (L2)
- [x] Function type syntax `(Int) -> Int` in type expressions (L3)
- [x] Record destructuring in let and match patterns (L4)
- [x] Pattern guards `n if n > 0 => ...` (L5)
- [x] String methods: to_upper, to_lower, split, contains, etc. (L6)
- [x] Generic functions `fn identity[T](x: T) -> T` (L7)
- [x] For loops `for x in list { ... }` (L8)
- [x] Multi-field variant destructuring `Rectangle(w, h)` (L9)
- [x] Basic module system with file imports (L10)
- [x] Assignment statements in blocks `x = x + 1` (L11)

### Not Started 🔴
- [ ] Full generic type inference (currently type params are dynamic at runtime)
- [ ] Traits / type classes
- [ ] `for` with `range()` function
- [ ] `break` / `continue` in loops
- [ ] Mutable references / borrowing
- [ ] Named import filtering (`import foo.{bar, baz}` - parsed but not resolved)
- [ ] `invariant` blocks on types
- [ ] `property` test blocks (parsed but not fully executed)
- [ ] `package` command (`astra package`)
- [ ] WASM compilation target
- [ ] Incremental compilation
- [ ] LSP / language server protocol

---

## Working Commands

```bash
cargo build                              # Build
cargo test                               # Run all tests (152)
cargo run -- run examples/hello.astra    # Run a program
cargo run -- check examples/             # Check syntax + types + effects
cargo run -- test                        # Run test blocks (42+)
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
| 2026-02-16 | claude | L1: Lambda/closure expressions `fn(x) { x + 1 }` |
| 2026-02-16 | claude | L2: Higher-order list methods (map, filter, fold, each, any, all, flat_map) |
| 2026-02-16 | claude | L3: Function type syntax `(Int) -> Int` |
| 2026-02-16 | claude | L4: Record destructuring in let/match |
| 2026-02-16 | claude | L5: Pattern guards `n if n > 0 => ...` |
| 2026-02-16 | claude | L6: String methods (to_upper, to_lower, split, trim, etc.) |
| 2026-02-16 | claude | L7: Generic functions `fn identity[T](x: T) -> T` |
| 2026-02-16 | claude | L8: For loops `for x in list { ... }` |
| 2026-02-16 | claude | L9: Multi-field variant destructuring `Rectangle(w, h)` |
| 2026-02-16 | claude | L10: Basic module system (import resolution + loading) |
| 2026-02-16 | claude | L11: Assignment statements in blocks |

---

## For Next Agent

**Recommended tasks** (in priority order):

1. **`range()` builtin + for-range loops**
   - `for i in range(0, 10) { ... }`
   - Enables counting loops, very common pattern
   - Needs: `range` function returning `List[Int]` (or lazy iterator)

2. **`break` / `continue` in loops**
   - Essential control flow for for-loops
   - Needs: special `RuntimeError` variant for early loop exit (like `?` operator)

3. **Full generic type inference**
   - Currently generic type params are `Unknown` at check time
   - Needs: type unification, substitution during checking
   - Impact: Better error messages, type-safe generic collections

4. **Traits / type classes**
   - `trait Printable { fn to_text(self) -> Text }`
   - Enables ad-hoc polymorphism
   - Big feature, should be designed carefully first (see ADR pattern)

5. **Named import resolution**
   - `import std.math.{sqrt, abs}` - already parsed, needs runtime resolution
   - Currently imports load all definitions; need selective import

6. **Property-based testing execution**
   - `property` blocks are parsed but not fully executed
   - Needs: integration with `proptest` for random input generation

**Avoid getting distracted by**:
- Performance optimizations (premature at this stage)
- WASM compilation (requires backend rewrite)
- IDE integration (build a solid CLI first)

The goal is to make Astra demonstrably better for LLMs than Python/JS/Rust as quickly as possible.

---

## Test Coverage Summary

| Category | Count | Type |
|----------|-------|------|
| Unit tests (Rust) | 148 | `#[test]` in source |
| Golden tests | 4 | Snapshot comparisons |
| Astra tests | 42+ | `test` blocks in .astra |
| **Total** | **194+** | All passing ✅ |

---

## Appendix: File Locations

| Component | File | Tests |
|-----------|------|-------|
| Lexer | `src/parser/lexer.rs` | In-file |
| Parser | `src/parser/parser.rs` | In-file + golden |
| AST | `src/parser/ast.rs` | - |
| Type Checker | `src/typechecker/mod.rs` | In-file (17+) |
| Effects | `src/effects/mod.rs` | In-file |
| Interpreter | `src/interpreter/mod.rs` | In-file (90+) |
| CLI | `src/cli/mod.rs` | Integration |
| Diagnostics | `src/diagnostics/mod.rs` | In-file |
| Formatter | `src/formatter/mod.rs` | Via golden tests |
| Manifest | `src/manifest/mod.rs` | In-file |
| Testing | `src/testing/mod.rs` | In-file |

### Examples (9 working programs)
| Example | Features Demonstrated |
|---------|----------------------|
| `hello.astra` | Hello world, Console effect |
| `option_handling.astra` | Option type, pattern matching |
| `result_chaining.astra` | Result type, ? operator |
| `effects_demo.astra` | Effect declarations, capabilities |
| `deterministic_tests.astra` | Test blocks, mocked Clock |
| `fibonacci.astra` | Recursion, pattern matching |
| `contracts.astra` | requires/ensures contracts |
| `for_loops.astra` | For loops, mutable state |
| `generics.astra` | Generic functions, higher-order |
