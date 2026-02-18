# Changelog

## v1.0.0

Astra v1.0 — the first release ready for real projects.

### Language

- Full syntax: modules, functions, let bindings, if/else, match, for/while loops, break/continue/return
- Type system with inference, generics, traits, type aliases, and invariants
- Enums with associated data, Option[T], Result[T, E], and the `?` operator
- Capability-based effects system (Console, Fs, Net, Clock, Rand, Env) with user-defined effects
- Pattern matching with exhaustiveness checking
- String interpolation (`"${expr}"`), multiline strings (`"""..."""`), escape sequences
- Range expressions (`0..10`, `0..=10`)
- Pipe operator (`value |> fn`)
- Tail call optimization
- Contracts (`requires`/`ensures`)
- Mutable bindings (`let mut`)

### Toolchain

- `astra run` — execute programs with real Fs/Net/Clock/Rand capabilities
- `astra check` — type check with JSON output, `--watch` mode, incremental caching
- `astra test` — deterministic test runner with mock injection and property tests
- `astra fmt` — canonical formatter
- `astra fix` — auto-apply diagnostic suggestions
- `astra explain` — detailed error code explanations (55 codes)
- `astra repl` — interactive REPL
- `astra init` — project scaffolding with `--lib` support
- `astra doc` — generate API docs from `##` comments
- `astra lsp` — LSP server with diagnostics, hover, completion, and code actions

### Standard Library (12 modules)

`std.core`, `std.math`, `std.string`, `std.collections`, `std.list`, `std.option`, `std.result`, `std.prelude`, `std.json`, `std.io`, `std.iter`, `std.error`

### Known Limitations

See [README.md](README.md#known-limitations-v10) for the full list. Key items:
- No full Hindley-Milner type inference
- Traits are runtime-dispatched
- Single-threaded (no concurrency)
- Interpreted only (tree-walking)
- No package manager
- No debugger
