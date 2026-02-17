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
- [x] All 11 examples pass check and run
- [x] 177 unit tests, 4 golden tests, 48+ Astra tests
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
- [x] `range()` builtin for counting loops `for i in range(0, 10)` (P1.1)
- [x] `break` / `continue` in for and while loops (P1.2)
- [x] `while` loops with condition-based iteration (P1.3)
- [x] `else if` chains (P1.4 - was already in parser, verified working)
- [x] String interpolation `"Hello, ${name}!"` (P1.5)
- [x] `return` statement with proper early exit propagation (P1.10)
- [x] Math builtins: `abs`, `min`, `max`, `pow` (P1.11)
- [x] List methods: `tail`, `reverse`, `sort`, `take`, `drop`, `slice`, `enumerate`, `zip`, `find`, `join` (P3.1+P3.2)
- [x] String methods: `repeat`, `index_of`, `substring` (P3.3)

### 🟠 Phase 1: Core Language Gaps (Practical programming essentials)

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P1.1** | `range()` builtin + for-range loops | Counting loops `for i in range(0, 10)` | ✅ Done |
| **P1.2** | `break` / `continue` in loops | Essential loop control flow | ✅ Done |
| **P1.3** | `while` loops | Condition-based iteration | ✅ Done |
| **P1.4** | `else if` chains | `if {} else if {} else {}` | ✅ Done |
| **P1.5** | String interpolation | `"Hello, ${name}!"` | ✅ Done |
| **P1.6** | `Float` type | 64-bit floating point numbers | 🔴 Not started |
| **P1.7** | Tuple types | `(Int, Text)` with destructuring | 🔴 Not started |
| **P1.8** | `Map[K, V]` type | Hash map with literal syntax `{k: v}` | 🔴 Not started |
| **P1.9** | Type alias resolution | `type Name = Text` evaluated in checker + runtime | 🔴 Not started |
| **P1.10** | `return` statement | Early return from functions | ✅ Done |
| **P1.11** | Math builtins | `abs`, `min`, `max`, `pow`, `mod` | ✅ Done |
| **P1.12** | Negative number literals | `-42` parsed as literal not unary op | 🔴 Not started |

### 🟣 Phase 2: Type System Maturity

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P2.1** | Full generic type inference | Type unification + substitution | 🔴 Not started |
| **P2.2** | Traits / type classes | `trait Show { fn to_text(self) -> Text }` | 🔴 Not started |
| **P2.3** | Type invariants enforcement | `type Positive = Int invariant self > 0` | 🔴 Not started |
| **P2.4** | Generic type constraints | `fn sort[T: Ord](list: List[T])` | 🔴 Not started |
| **P2.5** | Recursive / self-referential types | Trees, linked lists | 🔴 Not started |

### 🔵 Phase 3: Standard Library Expansion

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P3.1** | List methods: `tail`, `last`, `reverse`, `sort` | Core list operations | ✅ Done |
| **P3.2** | List methods: `zip`, `enumerate`, `find`, `take`, `drop` | Advanced list operations | ✅ Done |
| **P3.3** | String methods: `join`, `repeat`, `index_of`, `substring` | Additional text manipulation | ✅ Done |
| **P3.4** | Conversion functions | `Int.parse("42")`, `Float.parse("3.14")` | 🔴 Not started |
| **P3.5** | `Set[T]` type | Unique collection with set operations | 🔴 Not started |
| **P3.6** | Math module | `std.math` with trig, sqrt, etc. for Float | 🔴 Not started |

### 🟤 Phase 4: Module System & Imports

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P4.1** | Named import resolution | `import foo.{bar, baz}` runtime filtering | 🔴 Not started |
| **P4.2** | Circular import detection | Proper error on cycles | 🔴 Not started |
| **P4.3** | Re-exports | `public import std.math` | 🔴 Not started |
| **P4.4** | Stdlib as importable modules | `import std.list`, `import std.math` | 🔴 Not started |

### ⚪ Phase 5: Testing & Diagnostics

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P5.1** | Property-based testing execution | `property` blocks with random input gen | 🔴 Not started |
| **P5.2** | Stack traces with source locations | Show call stack on runtime errors | 🔴 Not started |
| **P5.3** | Parser error recovery | Continue after errors for multi-error reporting | 🔴 Not started |
| **P5.4** | `expect` with custom error messages | `assert(x > 0, "x must be positive")` | 🔴 Not started |

### 🟡 Phase 6: Advanced Features

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P6.1** | Pipe operator `\|>` | `x \|> f \|> g` chaining | 🔴 Not started |
| **P6.2** | User-defined effects | Custom effect declarations beyond builtins | 🔴 Not started |
| **P6.3** | Mutable references / borrowing | Controlled mutation | 🔴 Not started |
| **P6.4** | Tail call optimization | Efficient recursion | 🔴 Not started |
| **P6.5** | Async/await with effects | `effects(Async)` for concurrent code | 🔴 Not started |

### ⬛ Phase 7: Tooling & Distribution

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P7.1** | REPL (`astra repl`) | Interactive development | 🔴 Not started |
| **P7.2** | `package` command | `astra package` for distribution | 🔴 Not started |
| **P7.3** | LSP / language server | IDE integration | 🔴 Not started |
| **P7.4** | WASM compilation target | Browser/edge deployment | 🔴 Not started |
| **P7.5** | Incremental compilation | Fast rebuilds | 🔴 Not started |

### Estimated Completion: ~65%
- **Done**: 44 features (C1-C5, H1-H5, N1-N4, L1-L11, P1.1-P1.5, P1.10-P1.11, P3.1-P3.3) + infrastructure
- **Remaining**: ~30 features across 7 phases
- **Phase 1** partially done (6/12) — Float, tuples, maps, type aliases remaining
- **Phases 2-3** make it production-worthy
- **Phases 4-7** are polish and ecosystem

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
| 2026-02-17 | claude | P1.1: `range()` builtin for counting loops |
| 2026-02-17 | claude | P1.2: `break` / `continue` in for and while loops |
| 2026-02-17 | claude | P1.3: `while` loops with break/continue support |
| 2026-02-17 | claude | P1.4: Verified `else if` chains working |
| 2026-02-17 | claude | P1.5: String interpolation `"${expr}"` |
| 2026-02-17 | claude | P1.10: `return` statement with proper propagation |
| 2026-02-17 | claude | P1.11: Math builtins (abs, min, max, pow) |
| 2026-02-17 | claude | P3.1+P3.2: List methods (tail, reverse, sort, take, drop, slice, enumerate, zip, find) |
| 2026-02-17 | claude | P3.3: String methods (repeat, index_of, substring) + List.join |
| 2026-02-17 | claude | Updated full implementation roadmap with ~40 remaining tasks |

---

## For Next Agent

**Priority**: Complete Phase 1 (core language gaps) first. These are the features
that any developer trying Astra would expect to "just work".

**Implementation order within Phase 1**:
1. P1.1 `range()` + P1.10 `return` + P1.11 math builtins (independent, parallelize)
2. P1.2 `break`/`continue` + P1.3 `while` loops (related loop features)
3. P1.4 `else if` + P1.5 string interpolation (parser changes)
4. P1.6 `Float` + P1.12 negative literals (type system additions)
5. P1.7 tuples + P1.8 `Map` (new collection types)
6. P1.9 type alias resolution (type checker)

**After Phase 1**: Phase 2 (type system) and Phase 3 (stdlib) can be parallelized.

**Avoid getting distracted by**:
- Performance optimizations (premature at this stage)
- WASM compilation (requires backend rewrite)
- IDE integration (build a solid CLI first)

The goal is to make Astra demonstrably better for LLMs than Python/JS/Rust as quickly as possible.

---

## Test Coverage Summary

| Category | Count | Type |
|----------|-------|------|
| Unit tests (Rust) | 177 | `#[test]` in source |
| Golden tests | 4 | Snapshot comparisons |
| Astra tests | 48+ | `test` blocks in .astra |
| **Total** | **229+** | All passing ✅ |

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

### Examples (11 working programs)
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
| `while_loops.astra` | While loops, break, return, fibonacci |
| `string_interp.astra` | String interpolation `"${expr}"` |
