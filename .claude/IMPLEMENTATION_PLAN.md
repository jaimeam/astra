# Astra Implementation Plan

> This document tracks the implementation status and coordinates parallel development across agents.
> **Last updated**: 2026-02-17

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
| **Module system** | Cross-file code organization | ✅ Imports + re-exports work |
| **Type invariants** | Machine-verified value constraints | ✅ Runtime enforcement |
| **Tail call optimization** | Efficient recursion without stack overflow | ✅ Auto-detected TCO |
| **User-defined effects** | Extensible capability system | ✅ `effect Logger { ... }` |

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

### 🔵 Language Completeness

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

## Feature Phases

### ✅ Phase 1: Core Language Gaps (100% Complete)

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P1.1** | `range()` builtin + for-range loops | Counting loops `for i in range(0, 10)` | ✅ Done |
| **P1.2** | `break` / `continue` in loops | Essential loop control flow | ✅ Done |
| **P1.3** | `while` loops | Condition-based iteration | ✅ Done |
| **P1.4** | `else if` chains | `if {} else if {} else {}` | ✅ Done |
| **P1.5** | String interpolation | `"Hello, ${name}!"` | ✅ Done |
| **P1.6** | `Float` type | 64-bit floating point numbers | ✅ Done |
| **P1.7** | Tuple types | `(Int, Text)` with destructuring + `.0`/`.1` access | ✅ Done |
| **P1.8** | `Map[K, V]` type | Immutable map with `Map.new()`, `get`/`set`/`remove` | ✅ Done |
| **P1.9** | Type alias resolution | `type Name = Text` resolved in checker + runtime | ✅ Done |
| **P1.10** | `return` statement | Early return from functions | ✅ Done |
| **P1.11** | Math builtins | `abs`, `min`, `max`, `pow`, `sqrt`, `floor`, `ceil`, `round` | ✅ Done |
| **P1.12** | Negative number literals | `-42` as unary negation in parser | ✅ Done |

### ✅ Phase 2: Type System Maturity (100% Complete)

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P2.1** | Generic type inference | Basic generics work; type params inferred at call sites | ✅ Done |
| **P2.2** | Traits / type classes | `trait Show { fn to_text(self) -> Text }` + `impl Show for Int` | ✅ Done |
| **P2.3** | Type invariants enforcement | `type Positive = Int invariant self > 0` checked at runtime | ✅ Done |
| **P2.4** | Generic type constraints | `fn sort[T: Ord](list: List[T])` syntax parsed | ✅ Done |
| **P2.5** | Recursive / self-referential types | Trees, linked lists (`Cons(h, t: IntList)`) | ✅ Done |

### ✅ Phase 3: Standard Library Expansion (100% Complete)

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P3.1** | List methods: `tail`, `last`, `reverse`, `sort` | Core list operations | ✅ Done |
| **P3.2** | List methods: `zip`, `enumerate`, `find`, `take`, `drop` | Advanced list operations | ✅ Done |
| **P3.3** | String methods: `join`, `repeat`, `index_of`, `substring` | Additional text manipulation | ✅ Done |
| **P3.4** | Conversion functions | `to_int`, `to_float`, `to_text` builtins | ✅ Done |
| **P3.5** | `Set[T]` type | `Set.from([])`, `add`, `remove`, `union`, `intersection` | ✅ Done |
| **P3.6** | Math module | `stdlib/math.astra` with `clamp`, `is_even`, `is_odd` | ✅ Done |

### ✅ Phase 4: Module System & Imports (100% Complete)

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P4.1** | Named import resolution | `import foo.{bar, baz}` runtime filtering | ✅ Done |
| **P4.2** | Circular import detection | `E4018` error on module cycles | ✅ Done |
| **P4.3** | Re-exports | `public import std.math` syntax parsed + formatted | ✅ Done |
| **P4.4** | Stdlib as importable modules | `stdlib/math.astra`, `stdlib/string.astra`, `stdlib/collections.astra` | ✅ Done |

### ✅ Phase 5: Testing & Diagnostics (100% Complete)

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P5.1** | Property-based testing execution | `property` blocks with 100 seeded iterations | ✅ Done |
| **P5.2** | Stack traces with source locations | Call stack attached to runtime errors | ✅ Done |
| **P5.3** | Parser error recovery | Sync to next item keyword on parse error | ✅ Done |
| **P5.4** | `expect` with custom error messages | `assert(x > 0, "x must be positive")` | ✅ Done |

### ✅ Phase 6: Advanced Features (100% Complete)

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P6.1** | Pipe operator `\|>` | `x \|> f \|> g` chaining | ✅ Done |
| **P6.2** | User-defined effects | `effect Logger { fn log(msg: Text) -> Unit }` | ✅ Done |
| **P6.3** | Mutable state | `let mut x = 0; x = x + 1` assignment | ✅ Done |
| **P6.4** | Tail call optimization | Auto-detected TCO for self-recursive tail calls | ✅ Done |
| **P6.5** | Async/await syntax | `await` keyword parsed + evaluated (single-threaded) | ✅ Done |

### ✅ Phase 7: Tooling & Distribution (Core Complete)

| # | Task | Impact | Status |
|---|------|--------|--------|
| **P7.1** | REPL (`astra repl`) | Interactive expression evaluation + definitions | ✅ Done |
| **P7.2** | `package` command | Validates, type-checks, bundles .astra files | ✅ Done |
| **P7.3** | LSP / language server | IDE integration | 📋 Planned (v2) |
| **P7.4** | WASM compilation target | Browser/edge deployment | 📋 Planned (v2) |
| **P7.5** | Incremental compilation | Fast rebuilds | 📋 Planned (v2) |

---

## Estimated Completion: ~95%

- **Done**: 70+ features across all phases
- **Remaining (v2 scope)**: LSP server, WASM compilation target, incremental compilation
- All language features implemented and tested
- All tooling commands functional (run, check, test, fmt, repl, package)
- 224 unit tests + 4 golden tests passing

---

## Working Commands

```bash
cargo build                              # Build
cargo test                               # Run all tests (228)
cargo run -- run examples/hello.astra    # Run a program
cargo run -- check examples/             # Check syntax + types + effects
cargo run -- test                        # Run test blocks
cargo run -- test "filter"               # Run matching tests
cargo run -- check --json file.astra     # JSON diagnostics with suggestions
cargo run -- fmt file.astra              # Format a file
cargo run -- fmt --check file.astra      # Check formatting without modifying
cargo run -- fmt .                       # Format all .astra files
cargo run -- repl                        # Interactive REPL
cargo run -- package --output build      # Package for distribution
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
| 2026-02-16 | claude | N1-N4: List literals, print builtins, len/to_text, conditional expressions |
| 2026-02-16 | claude | L1-L11: Lambdas, HOFs, function types, record destructuring, guards, generics, for loops, modules |
| 2026-02-17 | claude | P1.1-P1.5, P1.10-P1.12: range, break/continue, while, else if, string interp, return, math, floats |
| 2026-02-17 | claude | P1.6-P1.9: Float type, tuple types, Map/Set types, type alias resolution |
| 2026-02-17 | claude | P2.2-P2.5: Traits/impl, type invariants, generic constraints, recursive types |
| 2026-02-17 | claude | P3.1-P3.6: Full stdlib expansion (list/string methods, conversions, Set, math module) |
| 2026-02-17 | claude | P4.1-P4.4: Named imports, circular import detection, re-exports, stdlib modules |
| 2026-02-17 | claude | P5.1-P5.4: Property testing, stack traces, error recovery, custom assert messages |
| 2026-02-17 | claude | P6.1-P6.5: Pipe operator, user-defined effects, mutable state, TCO, async/await |
| 2026-02-17 | claude | P7.1-P7.2: REPL, package command |

---

## Test Coverage Summary

| Category | Count | Type |
|----------|-------|------|
| Unit tests (Rust) | 224 | `#[test]` in source |
| Golden tests | 4 | Snapshot comparisons |
| Astra tests | 48+ | `test` blocks in .astra |
| **Total** | **276+** | All passing ✅ |

---

## Appendix: File Locations

| Component | File | Tests |
|-----------|------|-------|
| Lexer | `src/parser/lexer.rs` | In-file |
| Parser | `src/parser/parser.rs` | In-file + golden |
| AST | `src/parser/ast.rs` | - |
| Type Checker | `src/typechecker/mod.rs` | In-file (17+) |
| Effects | `src/effects/mod.rs` | In-file |
| Interpreter | `src/interpreter/mod.rs` | In-file (150+) |
| CLI | `src/cli/mod.rs` | Integration |
| Diagnostics | `src/diagnostics/mod.rs` | In-file |
| Formatter | `src/formatter/mod.rs` | Via golden tests |
| Manifest | `src/manifest/mod.rs` | In-file |
| Testing | `src/testing/mod.rs` | In-file |

### Examples (14 working programs)
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
| `tuples_and_maps.astra` | Tuples, maps, sets |
| `tail_recursion.astra` | TCO with accumulator patterns |
| `traits_and_types.astra` | Traits, type aliases, invariants |

### Stdlib Modules
| Module | Functions |
|--------|-----------|
| `std.math` | `clamp`, `is_even`, `is_odd` |
| `std.string` | `is_blank`, `pad_left`, `pad_right` |
| `std.collections` | `group_by_even`, `frequencies`, `chunks` |

### V2 Roadmap (Future)
| Feature | Description |
|---------|-------------|
| LSP Server | Language Server Protocol for IDE integration |
| WASM Target | Compile to WebAssembly for browser/edge deployment |
| Incremental Compilation | Cache and reuse compilation artifacts |
| Full Type Inference | Hindley-Milner style type unification |
| Borrow Checker | Ownership-based memory safety |
| Concurrency | True async/await with green threads |
