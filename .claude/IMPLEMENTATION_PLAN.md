# Astra Implementation Plan

> This document tracks the implementation status and coordinates parallel development across agents.
> **Last updated**: 2026-01-27

## Git Workflow

**Main Branch**: `claude/setup-astra-foundation-Ig1nr`

See [BRANCHING.md](BRANCHING.md) for full branching strategy.

---

## Current Status (v0.1)

### Completed ✅
- [x] Project structure and Cargo.toml
- [x] Lexer with Logos
- [x] Recursive descent parser with AST
- [x] Basic formatter (canonical output)
- [x] Diagnostics system with JSON output and stable error codes
- [x] Effect system data structures
- [x] Basic type checker scaffolding
- [x] Interpreter with full expression evaluation
- [x] CLI wired up (run, check, test commands work)
- [x] Standard library modules defined
- [x] CI workflow
- [x] 32+ unit tests, 4 golden tests
- [x] Parser supports expression statements
- [x] **Test block parsing and execution** (NEW)
- [x] **assert/assert_eq builtins** (NEW)
- [x] **All 7 examples pass check and run** (NEW)

### Current Capabilities
```bash
cargo run -- run examples/hello.astra     # ✅ Works
cargo run -- check examples/*.astra       # ✅ Works
cargo run -- test                         # ✅ Works (runs test blocks)
cargo run -- fmt                          # 🟡 Placeholder
```

---

## Incremental Task Queue

> Pick the next unclaimed task from this list. Mark it [IN PROGRESS] with your session date.
> Complete one task fully (with tests) before starting another.

### Tier 1: Quick Wins (< 1 hour each)

| # | Task | Status | Files | Notes |
|---|------|--------|-------|-------|
| 1.1 | Add `print` builtin (no newline) | ⬜ Ready | interpreter | Like println but no \n |
| 1.2 | Add `len` builtin for Text | ⬜ Ready | interpreter | `len("hello")` → 5 |
| 1.3 | Add string `+` concatenation | ✅ Done | interpreter | Already works |
| 1.4 | Add `to_text` builtin for Int | ⬜ Ready | interpreter | `to_text(42)` → "42" |
| 1.5 | Add negation `-x` for Int | ⬜ Ready | interpreter | Unary minus |
| 1.6 | Support `else if` without braces | ⬜ Ready | parser | `if x {} else if y {}` |

### Tier 2: Small Features (1-2 hours each)

| # | Task | Status | Files | Notes |
|---|------|--------|-------|-------|
| 2.1 | Option builtins (Some/None) | ⬜ Ready | interpreter | Runtime support for Option type |
| 2.2 | Result builtins (Ok/Err) | ⬜ Ready | interpreter | Runtime support for Result type |
| 2.3 | `?` operator for Option | ⬜ Ready | interpreter | Early return on None |
| 2.4 | `?` operator for Result | ⬜ Ready | interpreter | Early return on Err |
| 2.5 | Property test execution | ⬜ Ready | cli, interpreter | Run `property` blocks |
| 2.6 | List literal syntax `[1, 2, 3]` | ⬜ Ready | parser, interpreter | Array creation |
| 2.7 | Basic List operations | ⬜ Ready | interpreter | len, get, push |

### Tier 3: Medium Features (2-4 hours each)

| # | Task | Status | Files | Notes |
|---|------|--------|-------|-------|
| 3.1 | Type inference for let bindings | ⬜ Ready | typechecker | Infer types from expressions |
| 3.2 | Function signature type checking | ⬜ Ready | typechecker | Validate arg/return types |
| 3.3 | Effect checking in functions | ⬜ Ready | effects, typechecker | Verify declared vs used |
| 3.4 | `requires` clause parsing | ⬜ Ready | parser | Precondition syntax |
| 3.5 | `ensures` clause parsing | ⬜ Ready | parser | Postcondition syntax |
| 3.6 | Contract runtime checks | ⬜ Ready | interpreter | Execute requires/ensures |
| 3.7 | `if X then Y else Z` syntax | ⬜ Ready | parser | Alternative to braces |
| 3.8 | Exhaustive match checking | ⬜ Ready | typechecker | Warn on non-exhaustive |

### Tier 4: Larger Features (4+ hours each)

| # | Task | Status | Files | Notes |
|---|------|--------|-------|-------|
| 4.1 | Full type inference algorithm | ⬜ Ready | typechecker | Hindley-Milner style |
| 4.2 | Generic type instantiation | ⬜ Ready | typechecker | `Option[Int]` etc |
| 4.3 | Module imports | ⬜ Ready | parser, interpreter | Cross-file imports |
| 4.4 | Map[K,V] type | ⬜ Ready | stdlib, interpreter | Hash map support |
| 4.5 | Property test generators | ⬜ Ready | testing | Int, Bool, Text generators |
| 4.6 | Property test shrinking | ⬜ Ready | testing | Minimize failing cases |

---

## Detailed Task Specifications

### Task 2.1: Option Builtins (Some/None)

**Goal**: Make `Some(x)` and `None` work at runtime

**Current state**: Parser handles Option types, but interpreter doesn't recognize `Some`/`None`

**Implementation**:
1. In `interpreter/mod.rs`, handle `Some` and `None` as special identifiers in `Expr::Ident`
2. Handle `Some(x)` as a call that wraps value in `Value::Some`
3. Add pattern matching for `Some(x)` and `None` patterns

**Test case**:
```astra
fn maybe_double(x: Int) -> Option[Int] {
  if x > 0 { Some(x * 2) } else { None }
}

test "option works" {
  match maybe_double(5) {
    Some(n) => assert n == 10
    None => assert false
  }
}
```

**Files**: `src/interpreter/mod.rs`
**Estimated time**: 1-2 hours

---

### Task 2.3: `?` Operator for Option

**Goal**: `value?` returns early with `None` if value is `None`

**Current state**: Parser has `Expr::Try`, interpreter doesn't handle it

**Implementation**:
1. In `eval_expr` for `Expr::Try`:
   - Evaluate inner expression
   - If `Value::None`, return `Value::None` from current function
   - If `Value::Some(x)`, unwrap to `x`
   - Otherwise, error

**Test case**:
```astra
fn get_doubled(opt: Option[Int]) -> Option[Int] {
  let x = opt?
  Some(x * 2)
}

test "? propagates None" {
  assert get_doubled(None) == None
  assert get_doubled(Some(5)) == Some(10)
}
```

**Files**: `src/interpreter/mod.rs`
**Estimated time**: 1 hour

---

### Task 3.4: `requires` Clause Parsing

**Goal**: Parse function preconditions

**Syntax**:
```astra
fn divide(a: Int, b: Int) -> Int
  requires b != 0
{
  a / b
}
```

**Current state**: Lexer has `Requires` token, AST has `requires: Vec<Expr>` in FnDef

**Implementation**:
1. In `parse_fn_def`, after return type and effects:
   - While `check(TokenKind::Requires)`:
     - Consume `requires`
     - Parse expression
     - Add to requires vec

**Test case**: Parse and verify AST contains requires clause

**Files**: `src/parser/parser.rs`
**Estimated time**: 1-2 hours

---

## Phase Roadmap

```
Phase 1: Core Language (Current)
├── Interpreter ✅
├── Test runner ✅
├── Option/Result builtins ⬜ (Next priority)
└── Basic type checking ⬜

Phase 2: Type Safety
├── Full type inference ⬜
├── Effect checking ⬜
├── Contract checking ⬜
└── Exhaustive matching ⬜

Phase 3: Standard Library
├── List operations ⬜
├── Map type ⬜
├── Text utilities ⬜
└── Module imports ⬜

Phase 4: Tooling
├── Property testing ⬜
├── Better error messages ⬜
├── IDE support (LSP) ⬜
└── Package system ⬜
```

---

## Quick Reference

### Working Commands
```bash
cargo build                              # Build
cargo test                               # Run all tests (32+)
cargo run -- run examples/hello.astra    # Run a program
cargo run -- check examples/             # Check syntax
cargo run -- test                        # Run test blocks
cargo run -- test "filter"               # Run matching tests
```

### File Locations
| Component | File | Tests |
|-----------|------|-------|
| Lexer | `src/parser/lexer.rs` | In-file |
| Parser | `src/parser/parser.rs` | In-file + golden |
| AST | `src/parser/ast.rs` | - |
| Type Checker | `src/typechecker/mod.rs` | In-file |
| Effects | `src/effects/mod.rs` | In-file |
| Interpreter | `src/interpreter/mod.rs` | In-file (11+) |
| CLI | `src/cli/mod.rs` | Integration |
| Diagnostics | `src/diagnostics/mod.rs` | In-file |

### Adding a New Builtin Function
1. Edit `src/interpreter/mod.rs`
2. Find `Expr::Call` handler (around line 357)
3. Add case in the builtin check:
```rust
"my_func" => {
    // implementation
}
```
4. Add test in the `#[cfg(test)]` section

### Adding New Syntax
1. Add token to `src/parser/lexer.rs` if needed
2. Add AST node to `src/parser/ast.rs` if needed
3. Add parsing in `src/parser/parser.rs`
4. Add golden test in `tests/syntax/`
5. Add evaluation in `src/interpreter/mod.rs`

---

## Session Log

| Date | Agent | Tasks Completed |
|------|-------|-----------------|
| 2026-01-26 | setup | Initial project structure, parser, interpreter |
| 2026-01-27 | claude | Test blocks, assert builtin, examples fixed |

---

## Notes for Next Agent

**Recommended next task**: Task 2.1 (Option builtins) - This unblocks many other features and the examples are already written to use Option.

**Things that work well**:
- The parser is solid and handles most syntax
- The interpreter evaluates expressions correctly
- Test runner works end-to-end

**Known limitations**:
- No module imports yet (single-file only)
- Type checker is scaffolding only
- Some examples simplified to avoid unimplemented features
