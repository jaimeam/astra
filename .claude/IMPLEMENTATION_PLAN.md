# Astra Implementation Plan

> This document tracks the implementation status and coordinates parallel development across agents.

## Git Workflow

**Main Branch**: `claude/setup-astra-foundation-Ig1nr`

See [BRANCHING.md](BRANCHING.md) for full branching strategy.

### Agent Branch Assignments

| Agent | Suggested Branch Name | Task |
|-------|----------------------|------|
| typechecker-engineer | `claude/typechecker-inference-{id}` | Complete type inference |
| effects-engineer | `claude/effects-checking-{id}` | Effect system integration |
| parser-engineer | `claude/parser-contracts-{id}` | Add requires/ensures syntax |
| stdlib-engineer | `claude/stdlib-integration-{id}` | Stdlib runtime integration |
| docs-engineer | `claude/docs-getting-started-{id}` | Getting started guide |

---

## Current Status (v0.1)

### Completed
- [x] Project structure and Cargo.toml
- [x] Lexer with Logos
- [x] Recursive descent parser with AST
- [x] Basic formatter (canonical output)
- [x] Diagnostics system with JSON output and stable error codes
- [x] Effect system data structures
- [x] Basic type checker scaffolding
- [x] **Interpreter with full expression evaluation** (NEW)
- [x] **CLI wired up (run, check commands work)** (NEW)
- [x] **Standard library modules defined** (NEW)
- [x] CI workflow
- [x] **11 interpreter unit tests** (NEW)
- [x] **Parser supports expression statements** (NEW)

### In Progress
| Area | Status | Agent | Notes |
|------|--------|-------|-------|
| Interpreter evaluation | ✅ Complete | runtime-engineer | All expression types, recursion, effects |
| Type inference | 🟡 Partial | typechecker-engineer | Has basic env, needs full inference |
| Effect checking | 🟡 Partial | effects-engineer | Needs integration with type checker |
| Standard library | 🟡 Defined | stdlib-engineer | Modules exist, need runtime integration |
| Property testing | 🟡 Partial | testing-engineer | Framework exists, needs generators |

---

## Phase 1: Core Execution ✅ COMPLETE

### 1.1 Interpreter Evaluation ✅
**Owner**: runtime-engineer
**Status**: COMPLETE

Implemented:
- [x] `eval_expr()` for all expression types
- [x] `eval_stmt()` for all statement types
- [x] `eval_block()` with proper scoping
- [x] Capability invocations (Console.println, etc.)
- [x] Pattern matching evaluation
- [x] Option/Result types
- [x] Recursion support
- [x] 11 unit tests

### 1.2 Type Checker Completion
**Owner**: typechecker-engineer
**Priority**: HIGH
**Files**: `src/typechecker/mod.rs`

Tasks:
- [ ] Implement full type inference for expressions
- [ ] Add function signature checking
- [ ] Implement exhaustive match checking
- [ ] Add record type checking
- [ ] Generate machine-actionable type errors

### 1.3 Effect System Integration
**Owner**: effects-engineer
**Priority**: HIGH
**Files**: `src/effects/mod.rs`, `src/typechecker/mod.rs`

Tasks:
- [ ] Check effect declarations against usage in function bodies
- [ ] Verify effect propagation (callee effects subset of caller)
- [ ] Generate effect violation errors with suggestions

---

## Phase 2: Standard Library

### 2.1 Core Types 🟡 IN PROGRESS
**Owner**: stdlib-engineer
**Priority**: HIGH
**Files**: `stdlib/*.astra`

Modules defined (need runtime integration):
- [x] `core.astra` - Basic types and operations
- [x] `option.astra` - Option[T] with map, flatMap, unwrap_or, etc.
- [x] `result.astra` - Result[T, E] with map, flatMap, map_err, etc.
- [x] `list.astra` - List[T] with map, filter, fold, etc.
- [x] `prelude.astra` - Common imports
- [ ] `map` - Map[K, V] (not yet defined)
- [ ] `text` - Text operations (not yet defined)

### 2.2 Effect Implementations ✅ COMPLETE
**Owner**: runtime-engineer
**Files**: `src/interpreter/mod.rs`

Implemented in interpreter:
- [x] Console (println, print, read_line)
- [x] Fs (read, write, exists)
- [x] Net (get, post)
- [x] Clock (now, sleep)
- [x] Rand (int, bool, float)
- [x] Env (get, args)

---

## Phase 3: Tooling Polish

### 3.1 Better Error Messages
**Owner**: docs-engineer + all
**Priority**: MEDIUM

- [ ] Add suggestions to all error types
- [ ] Include "did you mean?" for typos
- [ ] Show relevant code context

### 3.2 Examples & Documentation 🟡 IN PROGRESS
**Owner**: docs-engineer
**Priority**: MEDIUM

- [x] hello.astra - Basic hello world
- [x] fibonacci.astra - Recursion example
- [x] contracts.astra - Type contracts (needs parser support)
- [x] effects_demo.astra - Effect usage (needs parser support)
- [x] option_handling.astra - Option patterns (needs parser support)
- [x] result_chaining.astra - Result chaining (needs parser support)
- [ ] Complete language specification
- [ ] Write getting-started guide

### 3.3 Property Testing
**Owner**: testing-engineer
**Priority**: MEDIUM

- [ ] Add generators for all base types
- [ ] Implement shrinking
- [ ] Add forall syntax support

---

## Agent Coordination Rules

### Ownership
- Each task has a single owning agent (see Owner column)
- Other agents may contribute but should coordinate via this document

### File Locks
When an agent starts work on a file, they should:
1. Check this document for conflicts
2. Update the "In Progress" table with their work
3. Commit frequently with clear messages

### Dependencies
```
Parser (done)
    ↓
Type Checker ←→ Effects Checker
    ↓              ↓
    └──────┬───────┘
           ↓
      Interpreter (done)
           ↓
        Stdlib (partial)
```

### Communication
- Major decisions: Update `docs/adr/` with ADR
- Interface changes: Update `.claude/contracts/`
- Bug fixes: Direct commit with test

---

## Quick Reference: What's Where

| Component | Main File | Tests | Status |
|-----------|-----------|-------|--------|
| Lexer | `src/parser/lexer.rs` | In-file | ✅ |
| Parser | `src/parser/parser.rs` | `src/parser/mod.rs` | ✅ |
| AST | `src/parser/ast.rs` | - | ✅ |
| Formatter | `src/formatter/mod.rs` | Golden tests | ✅ |
| Type Checker | `src/typechecker/mod.rs` | `tests/typecheck/` | 🟡 |
| Effects | `src/effects/mod.rs` | In-file + `tests/effects/` | 🟡 |
| Interpreter | `src/interpreter/mod.rs` | In-file (11 tests) | ✅ |
| CLI | `src/cli/mod.rs` | Integration | ✅ |
| Diagnostics | `src/diagnostics/mod.rs` | In-file | ✅ |

---

## Next Actions (Parallel)

1. **typechecker-engineer**: Complete type inference and checking
2. **effects-engineer**: Integrate effect checking with type system
3. **parser-engineer**: Add support for requires/ensures, assert, using syntax
4. **stdlib-engineer**: Add Map and Text modules, integrate with interpreter
5. **docs-engineer**: Write getting-started guide

## Working Commands

```bash
# Build
cargo build

# Run all tests (32 total)
cargo test

# Run a program
cargo run -- run examples/hello.astra

# Check syntax
cargo run -- check examples/

# Format (placeholder)
cargo run -- fmt examples/
```
