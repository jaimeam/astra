# ADR-006: Astra v1.0 Release Assessment

**Status**: Proposed
**Date**: 2026-03-05
**Context**: Comprehensive review of Astra's current capabilities, gaps, and readiness for a v1.0 release.

---

## Current State Summary

| Dimension              | Status        | Details |
|------------------------|---------------|---------|
| **Rust toolchain**     | ~24,200 LOC   | Parser (3.9k), Type checker (4.2k), Interpreter (8.3k), plus formatter, CLI, diagnostics |
| **Test suite**         | 392 tests     | 388 unit tests + 4 golden test suites (23 `.astra` golden files), all passing |
| **Standard library**   | 13 modules    | core, prelude, option, result, error, list, collections, iter, string, regex, io, json, math |
| **Examples**           | 15 programs   | From hello-world to a multi-module task tracker project |
| **Documentation**      | 14 docs       | Spec, stdlib reference, error codes, 5 ADRs, getting-started guide, examples cookbook |

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
- **String interpolation**: `"Hello, ${name}!"`
- **Error propagation**: `?` and `?else` operators on Option/Result
- **Modules & imports**: Multi-file programs, selective imports, aliasing, re-exports
- **Mutable bindings**: `let mut` with compound assignment (`+=`, `-=`, etc.)
- **Loops**: `for..in`, `while`, `break`, `continue`
- **Tail-call optimization**: Self-recursive tail calls optimized
- **Async/await**: Async function declarations and await expressions (marked v1.1)

### Built-in Operations (50+ built-in functions)
- **Numeric**: `abs`, `min`, `max`, `pow`, `sqrt`, `floor`, `ceil`, `round`, `range`
- **String**: 15+ methods (split, replace, trim, contains, starts_with, etc.)
- **List**: 25+ methods (map, filter, fold, flat_map, sort, zip, enumerate, etc.)
- **Map/Set**: Full CRUD plus set operations (union, intersection)
- **JSON**: `json_parse`, `json_stringify`
- **Regex**: `regex_match`, `regex_find_all`, `regex_replace`, `regex_split`, `regex_is_match`
- **I/O**: File read/write, HTTP GET/POST, HTTP server (`Net.serve`), env vars, CLI args

### Tooling
- **Formatter**: `astra fmt` — canonical code formatting
- **Type checker**: `astra check` — static analysis with lint warnings (W0xxx)
- **Test runner**: `astra test` — runs test blocks with deterministic effect injection
- **Interpreter**: `astra run` — execute programs
- **Diagnostics**: 40+ error codes (E0xxx–E4xxx) with spans, suggestions, and machine-actionable fixes
- **Pre-commit hooks**: Enforces fmt, clippy, and test on every commit

---

## Gap Analysis: What's Missing for v1.0

### Critical (Must Fix Before v1.0)

1. **No package manager / dependency system**
   Programs can import from `std.*` and local files, but there's no way to declare or fetch third-party dependencies. For a v1.0, at minimum a `astra.toml` manifest with dependency declaration and a resolution mechanism is needed.

2. **No compilation / performance story**
   Astra is purely interpreted (tree-walking interpreter). For a v1.0 release:
   - Performance benchmarks should be documented
   - A clear stance on "interpreted is intentional" vs "compilation planned" should be stated
   - At minimum, the interpreter should be profiled for pathological cases

3. **Limited error recovery in parser**
   The parser appears to stop at the first error. For a v1.0 language with "machine-actionable diagnostics" as a design goal, multi-error recovery (reporting multiple errors per file) would significantly improve the developer experience.

4. **No REPL**
   An interactive REPL is a standard expectation for a language with an interpreter. This is particularly important for an "agent-native" language where LLMs benefit from interactive exploration.

### Important (Strongly Recommended for v1.0)

5. **Standard library completeness**
   - **Missing**: Date/time manipulation (only `Clock.now()` and `Clock.today()`, no parsing/formatting)
   - **Missing**: File path utilities (join, dirname, basename, extension)
   - **Missing**: Advanced string formatting (printf-style, number formatting)
   - **Missing**: Sorting with custom comparators on all collections
   - **Missing**: HashMap/HashSet with proper hashing (current Map is a Vec of pairs — O(n) lookup)

6. **No concurrency primitives**
   Async/await exists syntactically but `Future` is a stub. For v1.0, either:
   - Ship async as a fully working feature, or
   - Remove it entirely and document it as post-v1.0

7. **No debugging support**
   - No `--debug` or `--trace` mode for step-through execution
   - No stack trace on runtime errors (just error code + message)
   - No source maps or breakpoint support

8. **Type system gaps**
   - No intersection or union types
   - No type narrowing after `is_some()`/`is_ok()` checks
   - No associated types on traits
   - No default method implementations in traits

9. **Documentation gaps**
   - Language specification (`docs/spec.md`) should be fleshed out to cover all features
   - No formal grammar (BNF/EBNF)
   - No migration guide or changelog format

### Nice to Have (Can Be Post-v1.0)

10. **FFI / Host language interop** — Call Rust/WASM functions from Astra
11. **LSP (Language Server Protocol)** — IDE support with autocomplete, go-to-definition
12. **Playground / web REPL** — Try Astra in the browser (WASM target)
13. **Property-based testing framework** — `property` blocks exist in syntax but implementation is unclear
14. **Benchmarking built-in** — `bench` blocks for performance testing
15. **Richer collection types** — Deque, PriorityQueue, SortedMap
16. **Custom operators** — User-defined infix operators

---

## v1.0 Release Recommendation

### Verdict: **Not yet ready for v1.0, but close.**

Astra has a remarkably solid foundation for a language at this stage:
- The core language is well-designed with clear principles
- The type system + effect system is coherent and useful
- Error diagnostics are excellent with stable error codes
- The standard library covers essential functionality
- Documentation quality is above average
- The test suite is comprehensive with 392 passing tests

**However**, a v1.0 label carries a stability promise. The critical gaps — particularly the lack of a package manager, the purely interpreted performance story, and incomplete async — mean that users adopting Astra v1.0 would hit friction quickly on real-world projects.

### Recommended Path to v1.0

#### Phase 1: Stabilization (v0.9)
- [ ] Decide async/await: ship it or defer it — no half-implemented features in v1.0
- [ ] Add multi-error recovery to the parser
- [ ] Add a REPL (`astra repl`)
- [ ] Improve stack traces on runtime errors
- [ ] Fill stdlib gaps: date/time formatting, file paths, number formatting
- [ ] Write a formal grammar (BNF)
- [ ] Performance audit: benchmark the interpreter, document expected performance

#### Phase 2: Ecosystem (v0.95)
- [ ] Design and implement `astra.toml` dependency declaration
- [ ] Build a basic package registry or git-based dependency resolution
- [ ] Add an LSP server (even minimal: diagnostics + go-to-definition)
- [ ] Property test implementation (the syntax already exists)

#### Phase 3: Release (v1.0)
- [ ] Freeze all public APIs and error codes
- [ ] Complete the language specification
- [ ] Publish a changelog and migration guide
- [ ] Create a "v1.0 stability guarantee" document
- [ ] Ship with at least 3 non-trivial example projects

---

## What Could Be Released Today

If the goal is to get something into users' hands quickly, **Astra v0.1.0** (or v0.5.0) would be appropriate today. The language is fully functional for:
- Educational use and language exploration
- Small-to-medium scripts and tools
- LLM/Agent code generation experiments
- Teaching effect systems and contracts

A pre-1.0 version sets correct expectations while still inviting early adopters and contributors.

---

## Decision

**Recommended**: Release as **v0.5.0** (beta) now, targeting **v1.0** after completing Phases 1–3 above.
