# Astra Implementation Plan — v0.2 Roadmap

> Status: Active | Last updated: 2026-02-17
>
> This document identifies the gaps between Astra's v0.1 implementation and
> a production-ready v0.2, then prioritizes features by the value they deliver
> to Astra's core mission: **fast, deterministic feedback loops for LLM agents**.
>
> ## Recently Completed
>
> - **P1.1 Real Fs/Net/Clock/Rand capabilities** — `astra run` now provides
>   all real capabilities (filesystem, HTTP, clock, randomness)
> - **P1.4 Float literals** — Already implemented in v0.1 (verified working)
> - **P1.5 `astra init`** — Project scaffolding command with `--lib` support
> - **P2.1 Richer diagnostic suggestions** — "Did you mean?" for E1002,
>   suggestions for W0001 (unused var), W0002 (unused import)
> - **P2.2 Import validation** — `E0011: Module not found` for invalid
>   `std.*` imports with list of available modules

---

## Current State Summary (v0.1)

| Component       | Status    | LOC   | Notes                            |
|----------------|-----------|-------|----------------------------------|
| Parser          | Complete  | 2,465 | Full grammar, error recovery     |
| Formatter       | Complete  | 1,071 | Canonical, idempotent            |
| Type Checker    | Complete  | 2,622 | Inference, exhaustiveness, lints |
| Effect System   | Complete  | 196   | 6 built-in effects, custom defs  |
| Interpreter     | Complete  | 6,488 | All core features, TCO           |
| Diagnostics     | Complete  | 671   | Stable codes, JSON output        |
| CLI             | Complete  | 1,037 | fmt/check/test/run/repl/lsp/pkg  |
| LSP             | Complete  | 917   | Diagnostics, hover, completion   |
| Testing         | Complete  | 366   | Deterministic, property-based    |
| Stdlib          | Partial   | 12 files | Declarations only; not loaded  |

**Tests: 291 unit + 4 golden = 295 total, all passing.**

---

## Gap Analysis

### Critical Gaps (block real-world usage)

1. **No real Fs/Net capabilities** — `Fs.read`, `Fs.write`, `Net.get`, `Net.post`
   are stubbed. Programs requiring file I/O or HTTP cannot run outside tests.

2. **Stdlib not loaded at runtime** — The 12 `.astra` stdlib files exist but the
   interpreter doesn't auto-load them. `import std.collections` is a no-op in
   practice.

3. **No `astra init` command** — No way to scaffold a new project with
   `astra.toml`, directory structure, and a starter `main.astra`.

4. **Float literals not in lexer** — The lexer handles `int_literal` but `1.5`
   is not tokenized as a float, despite `Float` being a built-in type.

5. **No `astra build` / compilation pipeline** — The `package` command copies
   source files but does no actual compilation or bundling.

### High-Value Gaps (significantly improve LLM experience)

6. **Incremental checking** — `astra check` re-parses everything from scratch.
   File-level caching with content hashing would make the edit-check loop much
   faster.

7. **Watch mode** — `astra check --watch` and `astra test --watch` for
   continuous feedback without re-running the CLI.

8. **Better diagnostic suggestions** — Many error codes lack `suggestions` with
   concrete edit locations. The JSON contract promises them, but most errors
   only include a message.

9. **Import validation** — Imports are parsed but not fully resolved/validated.
   `import std.nonexistent` produces no error.

10. **Trait constraint checking** — Generic constraints (`where T: Display`) are
    parsed but not enforced by the type checker.

### Medium-Value Gaps (improve usability)

11. **Tuple destructuring in let bindings** — `let (a, b) = get_pair()` is
    not supported despite tuples being a first-class type.

12. **Range expressions** — `1..10` syntax for ranges, useful in `for` loops
    and list comprehensions.

13. **String escape validation** — Invalid escape sequences like `"\q"` are not
    caught by the lexer.

14. **Multiline strings** — No support for triple-quoted or heredoc strings,
    making embedded text awkward.

15. **`astra doc`** — Generate API documentation from doc comments (`##`).

16. **Type aliases with generics** — `type StringList = List[Text]` works, but
    `type Pair[A, B] = { first: A, second: B }` may not resolve correctly at
    runtime.

### Low-Value / Future Gaps

17. **WASM target** — Listed as a target in `astra.toml` but completely
    unimplemented. Significant effort for limited near-term value.

18. **Async/await** — Parsed by the grammar but not interpreted. Requires
    event loop and runtime support.

19. **Module re-exports** — `public import` for re-exporting from library
    modules.

20. **Debugger / step execution** — Step-through debugging for the interpreter.

---

## Prioritized Feature Plan

Features are ordered by **value to LLM agent workflows** — the core design
goal of Astra. Each feature includes scope, rationale, and acceptance criteria.

### Phase 1: Make Real Programs Run (P1)

These features are required for Astra to be useful beyond toy examples.

#### P1.1: Real Filesystem Capability

**Rationale**: LLM agents need to read config, write output, and manipulate
files. Without real `Fs`, Astra programs are limited to pure computation.

**Scope**:
- Implement `RealFs` struct in `src/interpreter/capabilities.rs`
- Wire `Fs.read(path)`, `Fs.write(path, content)`, `Fs.exists(path)`,
  `Fs.delete(path)`, `Fs.list_dir(path)` to actual syscalls
- Gate behind `Fs` effect — programs must declare `effects(Fs)` to use
- Add `Fs.read_lines(path)` returning `List[Text]`
- Provide the real Fs capability in `astra run` (already has `RealConsole`
  and `RealEnv` patterns to follow)

**Acceptance criteria**:
- `astra run` can read and write files on disk
- Tests using `Fs = mock_fs` continue to use mocks
- New integration tests exercise real file operations

#### P1.2: Real Network Capability

**Rationale**: HTTP requests are essential for agent automation (API calls,
webhooks, fetching data).

**Scope**:
- Add `ureq` (or `minreq`) as a dependency for synchronous HTTP
- Implement `RealNet` with `Net.get(url) -> Result[Text, Text]` and
  `Net.post(url, body) -> Result[Text, Text]`
- Return structured response with status code, headers, body
- Gate behind `Net` effect
- Timeout support with configurable default (30s)

**Acceptance criteria**:
- `astra run` can make real HTTP GET/POST requests
- `Net` effect is enforced — pure functions cannot call `Net.get`
- Tests with `Net = mock_net` continue to use mocks

#### P1.3: Stdlib Auto-Loading

**Rationale**: The stdlib files exist but aren't loaded. `import std.math`
should give you `Math.abs`, `Math.sqrt`, etc. without manual path setup.

**Scope**:
- When `import std.*` is encountered, locate the corresponding `.astra`
  file in the stdlib directory
- Parse and load it into the interpreter's environment
- Cache loaded modules to avoid re-parsing
- Resolve stdlib relative to the binary (for installed Astra) and relative
  to the project root (for development)

**Acceptance criteria**:
- `import std.math` makes `Math.abs(x)` available
- `import std.collections` provides `List`, `Map`, `Set` utilities
- Module loading errors produce clear diagnostics

#### P1.4: Float Literal Support

**Rationale**: The language has `Float` as a built-in type but the lexer
doesn't tokenize `3.14`. This is a basic gap.

**Scope**:
- Add `FloatLit` token to the lexer (digits `.` digits, optional exponent)
- Add `FloatLit` AST node to the parser
- Ensure float arithmetic works end-to-end: `1.5 + 2.5 == 4.0`
- Update formatter to handle float literals

**Acceptance criteria**:
- `let pi = 3.14159` parses, type-checks as `Float`, evaluates correctly
- `1.0 / 3.0` produces a float result
- Golden tests updated for float literal syntax

#### P1.5: `astra init` Command

**Rationale**: Project scaffolding is the first thing a new user (or LLM
agent) needs to create a working project.

**Scope**:
- `astra init [name]` creates a directory with:
  - `astra.toml` with package metadata
  - `src/main.astra` with a hello world program
  - `.gitignore` for build artifacts
- `astra init --lib` creates a library project (no main, has `src/lib.astra`)

**Acceptance criteria**:
- `astra init my_project && cd my_project && astra run src/main.astra` works
- Generated `astra.toml` is valid and complete

---

### Phase 2: Improve the Feedback Loop (P2)

These features make the LLM-compiler feedback loop faster and richer.

#### P2.1: Richer Diagnostic Suggestions

**Rationale**: The JSON diagnostic contract promises `suggestions` with edit
locations, but most errors only have messages. LLMs need structured fix
suggestions to close the loop automatically.

**Scope**:
- Add suggestions to these error codes:
  - `E1001` (type mismatch): suggest type conversion function
  - `E1002` (unknown identifier): suggest similar names (edit distance)
  - `E1004` (non-exhaustive match): suggest missing arms (already partial)
  - `E2001` (effect not declared): suggest adding `effects(...)` clause
  - `W0001` (unused variable): suggest prefixing with `_`
  - `W0002` (unused import): suggest removing the import line
- Each suggestion must include file, line, column, and replacement text

**Acceptance criteria**:
- `astra check --json` output includes `suggestions` array with `edits`
- At least 6 error codes have actionable suggestions
- Suggestions are tested in golden tests

#### P2.2: Import Validation

**Rationale**: Silent failures on invalid imports waste LLM cycles. The agent
writes `import std.nonexistent` and gets no feedback.

**Scope**:
- Validate that imported module paths resolve to actual files
- Report `E0007: Module not found` with the attempted path and search paths
- Validate that filtered imports (`import std.math.{abs, sqrt}`) reference
  names that actually exist in the module

**Acceptance criteria**:
- `import std.nonexistent` produces `E0007` with clear message
- `import std.math.{nonexistent}` produces `E0008` naming the unknown symbol
- Suggestion includes available modules or symbols

#### P2.3: Incremental Checking (File-Level Cache)

**Rationale**: Re-parsing unchanged files wastes time. With content hashing,
only changed files need re-checking.

**Scope**:
- Hash file contents (SHA-256) and store alongside parse/check results
- On re-check, skip files whose hash hasn't changed
- Invalidate cache when imported modules change
- Store cache in `.astra-cache/` directory

**Acceptance criteria**:
- Second run of `astra check` on unchanged code is measurably faster
- Changing one file only re-checks that file and its dependents
- `astra check --no-cache` bypasses caching

#### P2.4: Watch Mode

**Rationale**: LLM agents benefit from continuous feedback. `--watch` lets
the compiler report errors immediately after file changes.

**Scope**:
- Add `notify` crate for filesystem watching
- `astra check --watch` re-runs check on `.astra` file changes
- `astra test --watch` re-runs tests on changes
- Debounce rapid changes (100ms)
- Clear terminal and show fresh output on each run

**Acceptance criteria**:
- Saving a file triggers re-check within 200ms
- Only changed files are re-parsed (builds on P2.3)
- Ctrl+C cleanly exits watch mode

---

### Phase 3: Language Completeness (P3)

These features fill in gaps in the language itself.

#### P3.1: Tuple Destructuring in Let Bindings

**Rationale**: Tuples are first-class but you can't unpack them in `let`
bindings, forcing awkward `.0` / `.1` access.

**Scope**:
- Extend `let` statement parsing to accept tuple patterns:
  `let (a, b, c) = get_triple()`
- Extend type checker to infer types for destructured bindings
- Extend interpreter to evaluate destructuring

**Acceptance criteria**:
- `let (x, y) = (1, 2)` works and binds `x = 1`, `y = 2`
- Type errors in destructuring produce clear messages
- Nested destructuring: `let (a, (b, c)) = (1, (2, 3))`

#### P3.2: Range Expressions

**Rationale**: `for i in range(0, 10)` works but `for i in 0..10` is more
natural and aligns with Rust syntax familiar to many LLMs.

**Scope**:
- Add `..` and `..=` operators to the lexer
- Parse `a..b` as `Range(a, b)` and `a..=b` as `RangeInclusive(a, b)`
- Support in `for` loops: `for i in 0..10 { ... }`
- Support `.contains(n)` method on ranges

**Acceptance criteria**:
- `for i in 0..5 { println(to_text(i)) }` prints 0 through 4
- `for i in 0..=5` prints 0 through 5
- Ranges work in match patterns: `match n { 0..10 => "small" }`

#### P3.3: Trait Constraint Enforcement

**Rationale**: Generic constraints are parsed but not checked. This means
invalid programs compile, which undermines verifiability.

**Scope**:
- Track trait implementations in the type checker's context
- When a generic function is called, verify the concrete type satisfies
  all `where` constraints
- Report `E1009: Trait constraint not satisfied` with details

**Acceptance criteria**:
- Calling `fn sort[T](list: List[T]) -> List[T] where T: Comparable`
  with a type that doesn't implement `Comparable` produces an error
- Error message names the missing trait and the concrete type

#### P3.4: Multiline Strings

**Rationale**: Embedding JSON, SQL, or templates in Astra requires awkward
concatenation without multiline strings.

**Scope**:
- Add `"""..."""` triple-quoted string syntax to the lexer
- Strip common leading whitespace (dedent)
- Support string interpolation inside multiline strings

**Acceptance criteria**:
- `let sql = """SELECT * FROM users WHERE id = ${id}"""`
  produces the expected string
- Formatter handles multiline strings without re-indenting content

---

### Phase 4: Ecosystem & Tooling (P4)

#### P4.1: `astra doc` Command

Generate HTML/Markdown documentation from doc comments (`##`).

#### P4.2: Package Registry Design

Design (not implement) a package registry protocol for sharing Astra
libraries. ADR required.

#### P4.3: LSP Enhancements

- Code actions from diagnostic suggestions (auto-fix)
- Rename symbol
- Find references
- Workspace symbol search

#### P4.4: Performance Profiling

`astra run --profile` that tracks function call counts and durations,
outputs a flame graph or summary table.

---

## Feature Value Assessment

| Feature | LLM Agent Value | User Value | Effort | Priority |
|---------|----------------|------------|--------|----------|
| P1.1 Real Fs | **Critical** | Critical | Medium | 1 |
| P1.2 Real Net | **Critical** | High | Medium | 2 |
| P1.3 Stdlib loading | **Critical** | Critical | Medium | 3 |
| P1.4 Float literals | **High** | High | Low | 4 |
| P1.5 `astra init` | **High** | High | Low | 5 |
| P2.1 Better suggestions | **Critical** | Medium | Medium | 6 |
| P2.2 Import validation | **High** | High | Low | 7 |
| P2.3 Incremental check | Medium | **High** | High | 8 |
| P2.4 Watch mode | Medium | **High** | Medium | 9 |
| P3.1 Tuple destructure | Medium | Medium | Low | 10 |
| P3.2 Range expressions | Medium | Medium | Low | 11 |
| P3.3 Trait enforcement | **High** | Medium | Medium | 12 |
| P3.4 Multiline strings | Medium | Medium | Low | 13 |
| P4.1 `astra doc` | Low | Medium | Medium | 14 |
| P4.2 Package registry | Low | Medium | High | 15 |
| P4.3 LSP enhancements | Medium | **High** | High | 16 |
| P4.4 Profiling | Low | Medium | Medium | 17 |

---

## Recommended Immediate Actions

Based on the analysis above, the highest-value work to do **right now** is:

1. **P1.4: Float literals** — Low effort, unblocks basic numeric programs
2. **P1.1: Real Fs capability** — Enables file I/O in `astra run`
3. **P1.5: `astra init`** — Enables project bootstrapping
4. **P2.2: Import validation** — Catches silent failures early
5. **P2.1: Richer suggestions** — Directly improves the LLM feedback loop

These five items deliver the highest ratio of value to effort and most
directly serve Astra's mission of being an LLM-native language with fast,
deterministic feedback loops.
