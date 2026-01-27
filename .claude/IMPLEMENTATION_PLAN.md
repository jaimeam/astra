# Astra Implementation Plan

> This document tracks the implementation status and coordinates parallel development across agents.

## Current Status (v0.1)

### Completed
- [x] Project structure and Cargo.toml
- [x] Lexer with Logos
- [x] Recursive descent parser with AST
- [x] Basic formatter (canonical output)
- [x] Diagnostics system with JSON output and stable error codes
- [x] Effect system data structures
- [x] Basic type checker scaffolding
- [x] Interpreter value types and capability interfaces
- [x] CLI skeleton (fmt, check, test, run commands)
- [x] CI workflow

### In Progress
| Area | Status | Agent | Notes |
|------|--------|-------|-------|
| Interpreter evaluation | 🔴 Not started | runtime-engineer | Need to implement `eval_expr`, `eval_stmt`, etc. |
| Type inference | 🟡 Partial | typechecker-engineer | Has basic env, needs full inference |
| Effect checking | 🟡 Partial | effects-engineer | Needs integration with type checker |
| Standard library | 🔴 Not started | stdlib-engineer | Need core types: Option, Result, List, Map |
| Property testing | 🟡 Partial | testing-engineer | Framework exists, needs generators |

---

## Phase 1: Core Execution (Current)

### 1.1 Interpreter Evaluation
**Owner**: runtime-engineer
**Priority**: HIGH
**Files**: `src/interpreter/mod.rs`

Tasks:
- [ ] Implement `eval_expr()` for all expression types
- [ ] Implement `eval_stmt()` for all statement types
- [ ] Implement `eval_block()` with proper scoping
- [ ] Wire up capability invocations (Console.println, etc.)
- [ ] Add pattern matching evaluation
- [ ] Handle Option/Result types correctly

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

### 2.1 Core Types
**Owner**: stdlib-engineer
**Priority**: HIGH
**Files**: `stdlib/core.astra`, `stdlib/option.astra`, `stdlib/result.astra`

Modules needed:
- [ ] `core` - Basic types and operations
- [ ] `option` - Option[T] with map, flatMap, unwrap_or, etc.
- [ ] `result` - Result[T, E] with map, flatMap, map_err, etc.
- [ ] `list` - List[T] with map, filter, fold, etc.
- [ ] `map` - Map[K, V]
- [ ] `text` - Text operations (length, split, join, etc.)

### 2.2 Effect Implementations
**Owner**: stdlib-engineer + runtime-engineer
**Priority**: MEDIUM
**Files**: `stdlib/effects/*.astra`, `src/interpreter/mod.rs`

- [ ] Console (println, print, read_line)
- [ ] Fs (read, write, exists, list)
- [ ] Net (get, post, post_json)
- [ ] Clock (now, sleep)
- [ ] Rand (int, bool, float, choice)
- [ ] Env (get, args)

---

## Phase 3: Tooling Polish

### 3.1 Better Error Messages
**Owner**: docs-engineer + all
**Priority**: MEDIUM

- [ ] Add suggestions to all error types
- [ ] Include "did you mean?" for typos
- [ ] Show relevant code context

### 3.2 Examples & Documentation
**Owner**: docs-engineer
**Priority**: MEDIUM

- [ ] Create 5+ example programs showcasing features
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
      Interpreter
           ↓
        Stdlib
```

### Communication
- Major decisions: Update `docs/adr/` with ADR
- Interface changes: Update `.claude/contracts/`
- Bug fixes: Direct commit with test

---

## Quick Reference: What's Where

| Component | Main File | Tests |
|-----------|-----------|-------|
| Lexer | `src/parser/lexer.rs` | In-file |
| Parser | `src/parser/parser.rs` | `src/parser/mod.rs` |
| AST | `src/parser/ast.rs` | - |
| Formatter | `src/formatter/mod.rs` | Golden tests |
| Type Checker | `src/typechecker/mod.rs` | `tests/typecheck/` |
| Effects | `src/effects/mod.rs` | In-file + `tests/effects/` |
| Interpreter | `src/interpreter/mod.rs` | `tests/runtime/` |
| CLI | `src/cli/mod.rs` | Integration |
| Diagnostics | `src/diagnostics/mod.rs` | In-file |

---

## Next Actions (Parallel)

1. **runtime-engineer**: Implement expression evaluation in interpreter
2. **typechecker-engineer**: Complete type inference
3. **stdlib-engineer**: Create core stdlib modules (Option, Result, List)
4. **testing-engineer**: Add more golden tests
5. **docs-engineer**: Create more example programs
