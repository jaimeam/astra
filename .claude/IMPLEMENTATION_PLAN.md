# Astra Implementation Plan

> This document tracks the implementation status and coordinates parallel development across agents.
> **Last updated**: 2026-02-11

## Vision Alignment Check

**Astra's Core Value Proposition**: An LLM-native language with fast, deterministic feedback loops.

| Differentiator | Why It Matters for LLMs | Status |
|----------------|------------------------|--------|
| **Machine-readable diagnostics with fix suggestions** | LLMs can parse errors and apply fixes automatically | ✅ Codes + suggestions |
| **Explicit effects with enforcement** | LLMs see exactly what functions can do | ✅ Enforced in type checker |
| **Deterministic testing** | Tests never flake, LLMs trust results | 🟡 Basic tests work |
| **No null (Option/Result)** | Type system catches missing cases | ✅ Runtime works |
| **Exhaustive match checking** | Compiler catches forgotten cases | ✅ Implemented |
| **One canonical format** | No style choices to make | 🔴 Placeholder only |

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
| **C5** | Deterministic test effects (`using effects()`) | Inject mocked Clock/Rand | ⬜ Ready | 3h |

### 🟡 High Value (Improves LLM experience significantly)

| # | Task | Impact | Status | Est. Time |
|---|------|--------|--------|-----------|
| **H1** | `?` operator for Option/Result | Clean error propagation | ✅ Done | 1h |
| **H2** | `requires`/`ensures` parsing | Contract syntax | ⬜ Ready | 2h |
| **H3** | Contract runtime checks | Precondition/postcondition enforcement | ⬜ Ready | 2h |
| **H4** | Basic formatter implementation | One canonical format | ⬜ Ready | 4h |
| **H5** | Type inference for let bindings | Less boilerplate | ⬜ Ready | 3h |

### 🟢 Nice to Have (General language features)

| # | Task | Impact | Status | Est. Time |
|---|------|--------|--------|-----------|
| **N1** | List literal syntax `[1, 2, 3]` | Convenience | ⬜ Ready | 2h |
| **N2** | `print` builtin (no newline) | Convenience | ⬜ Ready | 30m |
| **N3** | `len` and `to_text` builtins | Convenience | ⬜ Ready | 30m |
| **N4** | `if X then Y else Z` syntax | Alternative syntax | ⬜ Ready | 1h |

---

## Detailed Specifications: Critical Path

### C1: Option/Result Runtime (Some/None/Ok/Err)

**Why it matters**: Null-free programming is useless if the runtime doesn't support Option/Result.

**Current state**: Parser/typechecker know about Option, but interpreter doesn't recognize `Some`/`None`/`Ok`/`Err`.

**Implementation**:
```rust
// In interpreter/mod.rs, Expr::Ident handling:
"Some" => return Ok(Value::BuiltinConstructor("Some")),
"None" => return Ok(Value::None),
"Ok" => return Ok(Value::BuiltinConstructor("Ok")),
"Err" => return Ok(Value::BuiltinConstructor("Err")),

// In Expr::Call handling:
Value::BuiltinConstructor("Some") => {
    if args.len() != 1 {
        return Err(RuntimeError::arity_mismatch(1, args.len()));
    }
    Ok(Value::Some(Box::new(args.remove(0))))
}
```

**Test case**:
```astra
test "option construction" {
  let x = Some(42)
  let y = None
  match x {
    Some(n) => assert n == 42
    None => assert false
  }
}
```

**Files**: `src/interpreter/mod.rs`

---

### C2: Exhaustive Match Checking

**Why it matters**: This is THE killer feature. The compiler catches forgotten cases.

**Example error**:
```
error[E1004]: Non-exhaustive match: missing pattern `None`
  --> app.astra:15:3
   |
15 |   match user {
   |   ^^^^^
   |
   = suggestion: Add case `None => ???`
```

**Implementation**:
1. In typechecker, collect all enum variants for the matched type
2. Track which patterns are covered
3. Report missing patterns with suggestions

**Files**: `src/typechecker/mod.rs`, `src/diagnostics/mod.rs`

---

### C3: Error Suggestions in Diagnostics

**Why it matters**: LLMs can parse suggestions and apply them automatically.

**Current state**: Diagnostics have codes and messages, but no `suggestions` field.

**Target format**:
```json
{
  "code": "E1004",
  "message": "Non-exhaustive match: missing pattern `None`",
  "span": {"file": "app.astra", "line": 15, "col": 3},
  "suggestions": [{
    "title": "Add missing case",
    "edits": [
      {"line": 18, "col": 0, "insert": "    None => ???\n"}
    ]
  }]
}
```

**Implementation**:
1. Add `suggestions: Vec<Suggestion>` to `Diagnostic`
2. Add `Suggestion { title: String, edits: Vec<Edit> }`
3. Add `Edit { line: u32, col: u32, insert: Option<String>, delete: Option<Span> }`
4. Update error generators to include suggestions

**Files**: `src/diagnostics/mod.rs`, all error sites

---

### C4: Effect Checking Enforcement

**Why it matters**: Function signatures declare all capabilities—this must be enforced.

**Example error**:
```
error[E2001]: Effect `Console` used but not declared
  --> app.astra:5:3
   |
 5 |   Console.println("hello")
   |   ^^^^^^^^^^^^^^^
   |
   = function `greet` must declare `effects(Console)` or remove this call
```

**Implementation**:
1. During type checking, track which effects are used in function body
2. Compare against declared effects
3. Report mismatches with suggestions

**Files**: `src/effects/mod.rs`, `src/typechecker/mod.rs`

---

### C5: Deterministic Test Effects

**Why it matters**: Tests that involve randomness or time can be made deterministic.

**Syntax**:
```astra
test "random is reproducible" using effects(Rand = Rand.seeded(42)) {
  let x = Rand.int(1, 100)
  assert x == 67  # Always 67 with seed 42
}

test "time is fixed" using effects(Clock = Clock.fixed(1000)) {
  let now = Clock.now()
  assert now == 1000
}
```

**Implementation**:
1. Parse `using effects(...)` clause in test blocks
2. Create interpreter with injected capabilities
3. Provide `Rand.seeded(seed)` and `Clock.fixed(time)` constructors

**Files**: `src/parser/parser.rs`, `src/cli/mod.rs`, `src/interpreter/mod.rs`

---

## Current Status Snapshot

### Completed ✅
- [x] Lexer with Logos
- [x] Recursive descent parser with AST
- [x] Diagnostics system with JSON output and stable error codes
- [x] Effect system data structures
- [x] Interpreter with full expression evaluation
- [x] CLI (run, check, test commands)
- [x] Test block parsing and execution
- [x] assert/assert_eq builtins
- [x] All 7 examples pass check and run
- [x] 55+ unit tests, 4 golden tests
- [x] Option/Result runtime (C1)
- [x] ? operator for Option/Result (H1)
- [x] Exhaustive match checking for Option/Result/Bool/enums (C2)
- [x] Error suggestions in diagnostics (C3)
- [x] Effect checking enforcement (C4)
- [x] Type checker wired into CLI check command
- [x] Function type resolution in type environment
- [x] Binary operator type inference (comparisons return Bool)

### Not Started 🔴
- [ ] Deterministic test effects (C5)
- [ ] Contracts parsing (H2)
- [ ] Contract runtime checks (H3)
- [ ] Formatter implementation (H4)
- [ ] Type inference for let bindings (H5)

---

## Working Commands

```bash
cargo build                              # Build
cargo test                               # Run all tests (55+)
cargo run -- run examples/hello.astra    # Run a program
cargo run -- check examples/             # Check syntax + types + effects
cargo run -- test                        # Run test blocks
cargo run -- test "filter"               # Run matching tests
cargo run -- check --json file.astra     # JSON diagnostics with suggestions
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

---

## For Next Agent

**Recommended task**: **C5 (Deterministic test effects)**

This is the highest-impact remaining task because:
1. It completes the critical path for the "LLM → check → fix → repeat" loop
2. Tests with Clock/Rand can become fully deterministic
3. The infrastructure (SeededRand, FixedClock) already exists in the interpreter

**After C5, prioritize**:
- H4 (formatter) - one canonical format eliminates style decisions for LLMs
- H2/H3 (contracts) - preconditions/postconditions for stronger verification

**Avoid getting distracted by**:
- Nice-to-have syntax features (N1-N4)
- Performance optimizations
- Advanced type system features

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
| Interpreter | `src/interpreter/mod.rs` | In-file (11+) |
| CLI | `src/cli/mod.rs` | Integration |
| Diagnostics | `src/diagnostics/mod.rs` | In-file |
