# Astra Implementation Plan — v0.2 Roadmap

> Status: Active | Last updated: 2026-02-17
>
> This document identifies the gaps between Astra's v0.1 implementation and
> a production-ready v0.2, then prioritizes features by the value they deliver
> to Astra's core mission: **fast, deterministic feedback loops for LLM agents**.
>
> ## Recently Completed (v0.2 session)
>
> - **P1.1 Real Fs/Net/Clock/Rand capabilities** — `astra run` now provides
>   all real capabilities (filesystem, HTTP, clock, randomness)
> - **P1.4 Float literals** — Already implemented in v0.1 (verified working)
> - **P1.5 `astra init`** — Project scaffolding command with `--lib` support
> - **P2.1 Richer diagnostic suggestions** — "Did you mean?" for E1002,
>   suggestions for W0001 (unused var), W0002 (unused import), all with
>   concrete `Edit` objects containing span and replacement text
> - **P2.2 Import validation** — `E0011: Module not found` for invalid
>   `std.*` imports with list of available modules
> - **P3.2 Range expressions** — `0..10` (exclusive) and `0..=10` (inclusive)
>   syntax with `..` and `..=` operators, full lexer/parser/typechecker/
>   interpreter/formatter support
> - **P3.3 Trait constraint enforcement** — `fn sort[T: Ord](items: List[T])`
>   bounds are now checked at call sites; `E1016` reports when a concrete type
>   doesn't implement the required trait
> - **P3.4 Multiline strings** — `"""..."""` triple-quoted strings with
>   automatic dedent and string interpolation support
> - **String escape validation** — Invalid escape sequences (`\q`, `\a`, etc.)
>   now report `E0007` with a clear error message listing valid escapes

---

## Current State Summary (v0.2)

| Component       | Status    | LOC   | Notes                            |
|----------------|-----------|-------|----------------------------------|
| Parser          | Complete  | 2,700 | Full grammar, error recovery, range/multiline |
| Formatter       | Complete  | 1,100 | Canonical, idempotent, range support |
| Type Checker    | Complete  | 2,950 | Inference, exhaustiveness, lints, trait constraints |
| Effect System   | Complete  | 196   | 6 built-in effects, custom defs  |
| Interpreter     | Complete  | 6,700 | All core features, TCO, ranges   |
| Diagnostics     | Complete  | 750   | Stable codes, JSON output, edit suggestions |
| CLI             | Complete  | 1,255 | fmt/check/test/run/repl/lsp/pkg/init |
| LSP             | Complete  | 917   | Diagnostics, hover, completion   |
| Testing         | Complete  | 366   | Deterministic, property-based    |
| Stdlib          | Complete  | 12 files | All modules loadable            |

**Tests: 305 unit + 4 golden = 309 total, all passing.**

---

## Gap Analysis (Updated)

### Resolved Gaps (no longer blockers)

1. ~~No real Fs/Net capabilities~~ — ✅ RealFs and RealNet implemented
2. ~~Stdlib not loaded at runtime~~ — ✅ `import std.*` resolves and loads
3. ~~No `astra init` command~~ — ✅ Scaffolding with `--lib` support
4. ~~Float literals not in lexer~~ — ✅ `3.14` parses and evaluates
5. ~~Better diagnostic suggestions~~ — ✅ Edit objects with span data
6. ~~Import validation~~ — ✅ `E0011` for invalid std.* imports
7. ~~Trait constraint checking~~ — ✅ `E1016` for unsatisfied bounds
8. ~~Range expressions~~ — ✅ `0..10` and `0..=10` syntax
9. ~~String escape validation~~ — ✅ `E0007` for invalid escapes
10. ~~Multiline strings~~ — ✅ `"""..."""` with dedent

### Remaining High-Value Gaps

6. **Incremental checking** — `astra check` re-parses everything from scratch.
   File-level caching with content hashing would make the edit-check loop much
   faster.

7. **Watch mode** — `astra check --watch` and `astra test --watch` for
   continuous feedback without re-running the CLI.

### Remaining Medium-Value Gaps

15. **`astra doc`** — Generate API documentation from doc comments (`##`).

16. **Type aliases with generics** — `type StringList = List[Text]` works, but
    `type Pair[A, B] = { first: A, second: B }` may not resolve correctly at
    runtime.

### Low-Value / Future Gaps

17. **WASM target** — Listed as a target in `astra.toml` but completely
    unimplemented. Significant effort for limited near-term value.

18. **Async/await** — Parsed by the grammar but not interpreted. Requires
    event loop and runtime support.

20. **Debugger / step execution** — Step-through debugging for the interpreter.

---

## Prioritized Feature Plan (Remaining)

### Phase 2: Improve the Feedback Loop (P2)

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

## Feature Value Assessment (Updated)

| Feature | LLM Agent Value | User Value | Effort | Status |
|---------|----------------|------------|--------|--------|
| P1.1 Real Fs | **Critical** | Critical | Medium | ✅ Done |
| P1.2 Real Net | **Critical** | High | Medium | ✅ Done |
| P1.3 Stdlib loading | **Critical** | Critical | Medium | ✅ Done |
| P1.4 Float literals | **High** | High | Low | ✅ Done |
| P1.5 `astra init` | **High** | High | Low | ✅ Done |
| P2.1 Better suggestions | **Critical** | Medium | Medium | ✅ Done |
| P2.2 Import validation | **High** | High | Low | ✅ Done |
| P2.3 Incremental check | Medium | **High** | High | 📋 Planned |
| P2.4 Watch mode | Medium | **High** | Medium | 📋 Planned |
| P3.1 Tuple destructure | Medium | Medium | Low | ✅ Done (v0.1) |
| P3.2 Range expressions | Medium | Medium | Low | ✅ Done |
| P3.3 Trait enforcement | **High** | Medium | Medium | ✅ Done |
| P3.4 Multiline strings | Medium | Medium | Low | ✅ Done |
| String escape validation | **High** | High | Low | ✅ Done |
| P4.1 `astra doc` | Low | Medium | Medium | 📋 Planned |
| P4.2 Package registry | Low | Medium | High | 📋 Planned |
| P4.3 LSP enhancements | Medium | **High** | High | 📋 Planned |
| P4.4 Profiling | Low | Medium | Medium | 📋 Planned |

---

## Recommended Next Actions

The highest-value remaining work is:

1. **P2.3: Incremental checking** — File-level caching for faster feedback loops
2. **P2.4: Watch mode** — Continuous feedback via `astra check --watch`
3. **P4.3: LSP code actions** — Auto-fix from diagnostic suggestions
4. **P4.1: `astra doc`** — Documentation generation from doc comments

These items improve developer experience and the LLM agent feedback loop,
which is Astra's core differentiator.

## Error Code Registry (Updated)

| Range | Count | Description |
|-------|-------|-------------|
| E0xxx | 11 | Syntax/parsing errors |
| E1xxx | 16 | Type errors (including E1016 trait constraint) |
| E2xxx | 7 | Effect errors |
| E3xxx | 5 | Contract violations |
| E4xxx | 8 | Runtime errors |
| W0xxx | 7 | Warnings |
| **Total** | **54** | All with stable codes |
