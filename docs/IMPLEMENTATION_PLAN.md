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
> - **P2.3 Incremental checking** — File-level content hashing and caching
>   via `.astra-cache/`. Second runs of `astra check` skip unchanged files.
>   `--no-cache` flag bypasses caching.
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
> - **P4.1 `astra doc`** — Documentation generation from `##` doc comments.
>   Supports markdown and HTML output. Generates per-module docs + index.
> - **P4.3 LSP code actions** — Diagnostic suggestions are now wired into
>   `textDocument/codeAction` as quick fixes. IDEs can auto-apply "Did you
>   mean?" suggestions and other fixes with a single click.

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
| CLI             | Complete  | 1,600 | fmt/check/test/run/repl/lsp/pkg/init/doc |
| LSP             | Complete  | 1,000 | Diagnostics, hover, completion, code actions |
| Testing         | Complete  | 366   | Deterministic, property-based    |
| Cache           | Complete  | 170   | File-level incremental checking  |
| Stdlib          | Complete  | 12 files | All modules loadable            |

**Tests: 312 unit + 4 golden = 316 total, all passing.**

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
11. ~~Incremental checking~~ — ✅ File-level caching with `--no-cache` bypass
12. ~~`astra doc` command~~ — ✅ Markdown/HTML docs from `##` comments
13. ~~LSP code actions~~ — ✅ Quick fixes from diagnostic suggestions

### Remaining High-Value Gaps

1. **Watch mode** — `astra check --watch` and `astra test --watch` for
   continuous feedback without re-running the CLI. Requires `notify` crate.

### Remaining Medium-Value Gaps

2. **Type aliases with generics** — `type StringList = List[Text]` works, but
   `type Pair[A, B] = { first: A, second: B }` may not resolve correctly at
   runtime.

3. **LSP rename / find references** — Symbol rename and find-all-references
   would improve IDE experience significantly.

### Low-Value / Future Gaps

4. **WASM target** — Listed as a target in `astra.toml` but completely
   unimplemented. Significant effort for limited near-term value.

5. **Async/await** — Parsed by the grammar but not interpreted. Requires
   event loop and runtime support.

6. **Debugger / step execution** — Step-through debugging for the interpreter.

7. **Performance profiling** — `astra run --profile` for call timing analysis.

---

## Prioritized Feature Plan (Remaining)

### Phase 2: Improve the Feedback Loop (P2)

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
- Only changed files are re-parsed (builds on P2.3 incremental cache)
- Ctrl+C cleanly exits watch mode

---

### Phase 4: Ecosystem & Tooling (P4)

#### P4.2: Package Registry Design

Design (not implement) a package registry protocol for sharing Astra
libraries. ADR required.

#### P4.3: LSP Enhancements (continued)

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
| P2.3 Incremental check | Medium | **High** | High | ✅ Done |
| P2.4 Watch mode | Medium | **High** | Medium | 📋 Planned |
| P3.1 Tuple destructure | Medium | Medium | Low | ✅ Done (v0.1) |
| P3.2 Range expressions | Medium | Medium | Low | ✅ Done |
| P3.3 Trait enforcement | **High** | Medium | Medium | ✅ Done |
| P3.4 Multiline strings | Medium | Medium | Low | ✅ Done |
| String escape validation | **High** | High | Low | ✅ Done |
| P4.1 `astra doc` | Low | Medium | Medium | ✅ Done |
| P4.2 Package registry | Low | Medium | High | 📋 Planned |
| P4.3 LSP code actions | Medium | **High** | Medium | ✅ Done |
| P4.3 LSP rename/refs | Medium | **High** | High | 📋 Planned |
| P4.4 Profiling | Low | Medium | Medium | 📋 Planned |

---

## Recommended Next Actions

The highest-value remaining work is:

1. **P2.4: Watch mode** — Continuous feedback via `astra check --watch`
2. **P4.3: LSP rename/find references** — Improved IDE experience
3. **P4.2: Package registry design** — ADR for library sharing protocol
4. **P4.4: Performance profiling** — `astra run --profile`

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
