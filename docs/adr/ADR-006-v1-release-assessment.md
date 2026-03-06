# ADR-006: Astra v1.0 Release Assessment

**Status**: Proposed
**Date**: 2026-03-05
**Context**: Comprehensive review of Astra's current capabilities, gaps, and readiness for a v1.0 release.

---

## Current State Summary

| Dimension              | Status        | Details |
|------------------------|---------------|---------|
| **Rust toolchain**     | ~24,200 LOC   | Parser (3.9k), Type checker (4.2k), Interpreter (8.3k), Formatter, CLI (1.8k), Diagnostics |
| **Test suite**         | 392 tests     | 388 unit tests + 4 golden test suites (23 `.astra` golden files), all passing |
| **Standard library**   | 13 modules    | core, prelude, option, result, error, list, collections, iter, string, regex, io, json, math |
| **Examples**           | 15 programs   | From hello-world to a multi-module task tracker project |
| **Documentation**      | 14 docs       | Spec, stdlib reference, 57 error codes, 5 ADRs, getting-started guide, examples cookbook |
| **CLI commands**       | 11 commands   | fmt, check, test, run, repl, init, doc, fix, explain, pkg, lsp |
| **Error codes**        | 57 codes      | E0xxx (syntax), E1xxx (type), E2xxx (effect), E3xxx (contract), E4xxx (runtime), W0xxx (warnings) |

---

## What Astra Can Do Today

### Language Features (Complete)
- **Type system**: Int, Float, Bool, Text, Unit, List, Tuple, Map, Set, Record, Enum, Option, Result, Function types
- **Generics**: Type parameters with trait bounds (`fn identity[T: Show](x: T) -> T`)
- **Pattern matching**: Literals, variants, records, tuples, wildcards, guards; exhaustiveness checking
- **Effect system**: Capability-based I/O (Console, Fs, Net, Clock, Rand, Env) with `effects()` declarations
- **Contracts**: Preconditions (`requires`) and postconditions (`ensures`) on functions
- **Type invariants**: `type Percentage = Int invariant self >= 0 and self <= 100`
- **Traits & impl blocks**: User-defined trait interfaces with method dispatch
- **Closures/lambdas**: First-class functions with captured environments
- **Pipe operator**: `x |> f` for functional composition
- **String interpolation**: `"Hello, ${name}!"` with escape sequences
- **Error propagation**: `?` and `?else` operators on Option/Result
- **Modules & imports**: Multi-file programs, selective imports, aliasing, re-exports
- **Mutable bindings**: `let mut` with compound assignment (`+=`, `-=`, etc.)
- **Loops**: `for..in`, `while`, `break`, `continue`
- **Tail-call optimization**: Self-recursive tail calls optimized
- **Async/await**: Async function declarations and await expressions (marked v1.1)

### Built-in Operations (50+ built-in functions)
- **Numeric**: `abs`, `min`, `max`, `pow`, `sqrt`, `floor`, `ceil`, `round`, `range`
- **String**: 15+ methods (split, replace, trim, contains, starts_with, regex methods, etc.)
- **List**: 25+ methods (map, filter, fold, flat_map, sort, zip, enumerate, etc.)
- **Map/Set**: Full CRUD plus set operations (union, intersection)
- **JSON**: `json_parse`, `json_stringify`
- **Regex**: `regex_match`, `regex_find_all`, `regex_replace`, `regex_split`, `regex_is_match`
- **I/O**: File read/write, HTTP GET/POST, HTTP server (`Net.serve`), env vars, CLI args

### Tooling (Comprehensive)
- **Formatter**: `astra fmt` — canonical code formatting (idempotent, no config disputes)
- **Type checker**: `astra check` — static analysis with lint warnings (W0xxx), `--watch` mode, incremental caching
- **Test runner**: `astra test` — test blocks with deterministic effect injection, `--filter`, `--seed`, `--watch`
- **Interpreter**: `astra run` — execute programs with real capabilities
- **REPL**: `astra repl` — interactive exploration
- **Project init**: `astra init` / `astra init --lib` — scaffold new projects
- **Doc generator**: `astra doc` — API documentation (markdown/html)
- **Auto-fixer**: `astra fix` — auto-apply diagnostic suggestions, `--dry-run`
- **Error explainer**: `astra explain E1001` — detailed explanations for all 57 error codes
- **Package manager**: `astra pkg` — install, add, remove, list (marked v1.1)
- **LSP server**: `astra lsp` — IDE integration
- **Diagnostics**: 57 error codes (E0xxx–W0xxx) with spans, suggestions, machine-actionable fixes, JSON output (`--json`)
- **Pre-commit hooks**: Enforces fmt, clippy, and test on every commit

### Testing Infrastructure
- **Capability mocking**: `using effects(Rand = Rand.seeded(42), Clock = Clock.fixed(1000))` for deterministic tests
- **Property testing**: `property` blocks with configurable iterations and seeds
- **Golden tests**: Snapshot-based testing for parser, typechecker, effects, and runtime
- **12 runtime test categories**: arithmetic, functions, control flow, loops, generics, destructuring, contracts, traits, effects, async, JSON, regex

---

## Gap Analysis: What Needs Attention for v1.0

### Critical (Must Resolve Before v1.0)

1. **Async/await completeness**
   Async/await exists syntactically and `Future` is defined, but it appears to be a stub (v1.1 marker). For v1.0, either:
   - Fully implement async with a runtime (event loop, concurrent futures)
   - Explicitly defer it: remove from default feature set, document as experimental/v1.1

2. **Performance story**
   Astra is a tree-walking interpreter. For a v1.0 release:
   - Document performance expectations clearly ("scripting-speed, not systems-speed")
   - Profile and benchmark common patterns
   - Optimize hot paths (Map is O(n) Vec-of-pairs — consider hash-based implementation)

3. **Parser error recovery**
   The parser appears to stop at the first error. For a language whose design goal is "machine-actionable diagnostics," multi-error recovery (reporting multiple errors per file) would significantly improve both developer and agent experience.

4. **Package manager maturity**
   `astra pkg` exists but is marked v1.1. For v1.0, at minimum:
   - `astra.toml` dependency declaration should work
   - Git-based or registry-based resolution should be functional
   - If not ready, clearly document it as experimental

### Important (Strongly Recommended for v1.0)

5. **Standard library gaps**
   - **Missing**: Date/time manipulation (only `Clock.now()` and `Clock.today()`, no parsing/formatting/arithmetic)
   - **Missing**: File path utilities (join, dirname, basename, extension)
   - **Missing**: Advanced string formatting (printf-style, number formatting, padding beyond pad_left/pad_right)
   - **Missing**: Sorting with custom comparators on all collections
   - **Weak**: Map/Set use Vec internally — O(n) lookup instead of O(1)

6. **Debugging support**
   - No `--debug` or `--trace` mode for step-through execution
   - Stack traces on runtime errors should show full call chain
   - Source maps or breakpoint support would help adoption

7. **Type system enhancements**
   - No type narrowing after `is_some()`/`is_ok()` checks (flow-sensitive typing)
   - No associated types on traits
   - No default method implementations in traits

8. **Documentation completeness**
   - Language specification (`docs/spec.md`) should cover all features exhaustively
   - Formal grammar (BNF/EBNF) needed for language lawyers and tool authors
   - Changelog format and migration guide for version transitions

### Nice to Have (Post-v1.0)

9. **FFI / Host language interop** — Call Rust/WASM functions from Astra
10. **Playground / web REPL** — Try Astra in the browser (WASM target)
11. **Benchmarking built-in** — `bench` blocks for performance testing
12. **Richer collection types** — Deque, PriorityQueue, SortedMap, proper HashMap
13. **Custom operators** — User-defined infix operators
14. **Compilation target** — Bytecode VM or WASM compilation for performance

---

## v1.0 Release Recommendation

### Verdict: **Close to v1.0, with a few blockers to resolve.**

Astra is in remarkably good shape:

**Strengths:**
- Complete, coherent language with types, effects, contracts, generics, and pattern matching
- 11 CLI commands covering the full development lifecycle (fmt, check, test, run, repl, init, doc, fix, explain, pkg, lsp)
- 57 error codes with detailed explanations and auto-fix suggestions
- Deterministic testing with capability mocking
- 392 passing tests across unit and golden test suites
- Well-organized codebase with ADRs documenting design decisions
- Standard library covering core functionality across 13 modules

**Blockers for v1.0:**
- Async and package management need to either work fully or be explicitly deferred
- Parser needs multi-error recovery for the "agent-native" promise
- Performance characteristics need documentation and basic optimization (Map/Set)

### Recommended Path to v1.0

#### Phase 1: Stabilization (current → v0.9)
- [ ] Decide async/await: ship fully or explicitly defer to v1.1
- [ ] Decide pkg: ship fully or explicitly defer to v1.1
- [ ] Add multi-error recovery to the parser
- [ ] Improve stack traces on runtime errors
- [ ] Fill stdlib gaps: date/time formatting, file paths
- [ ] Optimize Map/Set to use hash-based storage
- [ ] Write a formal grammar (BNF)
- [ ] Performance benchmarks and documentation

#### Phase 2: Polish (v0.9 → v1.0-rc)
- [ ] Complete the language specification
- [ ] Harden LSP for common IDE workflows
- [ ] Add at least 3 non-trivial example projects
- [ ] Publish a changelog and migration guide
- [ ] Create a "v1.0 stability guarantee" document

#### Phase 3: Release (v1.0)
- [ ] Freeze all public APIs and error codes
- [ ] Final test pass across all platforms
- [ ] Release announcement with documentation

---

## What Could Be Released Today

Astra is fully functional **right now** for:
- Educational use and language exploration
- Small-to-medium scripts and automation tools
- LLM/Agent code generation experiments
- Teaching effect systems, contracts, and capability-based security
- Prototyping with deterministic, testable I/O

A **v0.8.0** or **v0.9.0-beta** release would be appropriate today, signaling that the language is feature-complete and approaching stability, while setting expectations that some rough edges (async, pkg, performance) are still being polished.

---

## Decision

**Recommended**: Release as **v0.9.0-beta** now, targeting **v1.0** after completing Phases 1–2 above. The language is closer to v1.0 than initially assessed — the tooling surface (REPL, LSP, pkg, doc, fix, explain) is already in place; the remaining work is stabilization and polish rather than greenfield development.
