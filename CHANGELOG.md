# Changelog

All notable changes to Astra will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-03-05

Astra v1.0 — the first stable release, ready for real projects.
See [docs/stability.md](docs/stability.md) for the stability guarantee.

### Language

- Full syntax: modules, functions, let bindings, if/else, match, for/while loops,
  break/continue/return
- Type system with inference, generics, traits, type aliases, and invariants
- Enums with associated data, Option[T], Result[T, E], and the `?` / `?else` operators
- Capability-based effects system (Console, Fs, Net, Clock, Rand, Env) with user-defined effects
- Pattern matching with exhaustiveness checking and guard clauses
- String interpolation (`"${expr}"`), multiline strings (`"""..."""`), escape sequences
- Range expressions (`0..10`, `0..=10`)
- Pipe operator (`value |> fn`)
- Tail call optimization for self-recursive functions
- Contracts (`requires`/`ensures`) and type invariants
- Mutable bindings (`let mut`) with compound assignment (`+=`, `-=`, `*=`, `/=`, `%=`)
- Closures and lambdas as first-class values

### Toolchain

- `astra run` — execute programs with real Fs/Net/Clock/Rand capabilities
- `astra check` — type check with JSON output, `--watch` mode, incremental caching
- `astra test` — deterministic test runner with mock injection, `--filter`, `--seed`, `--watch`
- `astra fmt` — canonical formatter (idempotent, deterministic)
- `astra fix` — auto-apply diagnostic suggestions with `--dry-run`
- `astra explain` — detailed error code explanations (57 codes)
- `astra repl` — interactive REPL
- `astra init` — project scaffolding with `--lib` support
- `astra doc` — generate API docs from `##` comments (markdown/html)
- `astra lsp` — LSP server with diagnostics, hover, completion, and code actions

### Standard Library (15 modules)

- `std.core` — identity, constant, Unit type alias
- `std.prelude` — auto-imported common utilities
- `std.option` — Option handling: is_some, is_none, unwrap_or, map
- `std.result` — Result handling: is_ok, is_err, unwrap_or, map, map_err
- `std.error` — Error utilities: wrap, from_text, or_else
- `std.list` — List utilities: is_empty, head, sort_by
- `std.collections` — Advanced collections: group_by, frequencies, chunks
- `std.iter` — Iterator functions: sum, product, all, any, reduce, flat_map
- `std.string` — String utilities: is_blank, pad_left, pad_right, chars
- `std.math` — Math functions: abs_val, min_val, max_val, clamp, is_even, is_odd
- `std.io` — I/O wrappers with effect declarations
- `std.json` — JSON parse and stringify
- `std.regex` — Regular expression matching, replace, split
- `std.datetime` — Date parsing, formatting, arithmetic, leap year detection
- `std.path` — File path manipulation: basename, dirname, extension, join, normalize

### Diagnostics (57 error codes)

- E0xxx: Syntax/parsing errors (11 codes)
- E1xxx: Type errors (16 codes)
- E2xxx: Effect errors (7 codes)
- E3xxx: Contract violations (5 codes)
- E4xxx: Runtime errors (8 codes)
- W0xxx: Warnings (8 codes)
- Multi-error recovery: parser reports multiple errors per file
- Stack traces with source locations (file:line:col)
- Machine-actionable suggestions with auto-fix support
- JSON output format for tool integration (`--json`)

### Performance

- Map/Set operations use O(log n) sorted binary search
- Tail-call optimization for self-recursive functions
- Incremental caching for `astra check`
- See [docs/performance.md](docs/performance.md) for full details

### Architecture Decisions

- ADR-001: Rust as implementation language
- ADR-002: Explicit effects over monads
- ADR-003: No null — Option[T] and Result[T, E] instead
- ADR-004: Built-in linting in `astra check`
- ADR-005: Interpreter-TypeChecker sync invariant
- ADR-006: v1.0 release assessment
- ADR-007: Async/await and package manager deferred to v1.1

### Not Included (Planned for v1.1)

- Async/await — syntax reserved, not yet functional
- Package manager — `astra pkg` command exists, resolution not implemented
- See [ADR-007](docs/adr/ADR-007-defer-async-pkg-to-v1.1.md) for rationale
