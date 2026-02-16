# ADR-004: Built-in Linting over External Tools

## Status

Accepted

## Context

Most programming languages rely on external linting tools for static analysis beyond type checking:
- Python: ruff, pylint, flake8 (linting) + mypy, pyright (type checking)
- JavaScript: ESLint (linting) + TypeScript (type checking)
- Go: golangci-lint (aggregates many external linters)

This creates fragmentation: projects must configure, version, and coordinate multiple tools. CI pipelines become complex. Error formats differ between tools, making machine consumption harder.

Astra needs to decide whether lint checks (unused variables, unreachable code, shadowed bindings, etc.) should be:
1. **Built into the compiler** as part of `astra check`
2. **External tools** following the Python/JS model

## Decision

**Build lint checks directly into the Astra type checker and expose them through `astra check`.**

Lint rules produce warnings (W0xxx codes) using the same diagnostic infrastructure as type errors (E1xxx) and effect errors (E2xxx). A `--strict` flag treats warnings as errors.

## Rationale

### Why Built-in

1. **Single tool, single pass**: `astra check` runs parsing, type checking, effect checking, and linting in one command. No tool coordination needed.
2. **Consistent diagnostics**: Warnings use the same structured format as errors — same spans, same JSON output, same suggestion system. Agents and IDEs consume one format.
3. **Zero configuration for correctness**: Lint rules that catch real bugs (unused variables, unreachable code) are always active. No `.eslintrc` or `ruff.toml` to configure just to get basic correctness checking.
4. **Agent-friendly**: LLMs working on Astra code get lint feedback in the same `astra check` loop they use for type errors. No separate tool invocation.
5. **Verifiability principle**: Astra's core design principle is "wrong code fails early with precise errors." Lint warnings are a natural extension of this.

### Why Not External Tools

1. **Fragmentation tax**: External tools add configuration files, version pinning, and CI complexity that contradicts Astra's simplicity goals.
2. **Format mismatch**: External tools produce their own error formats, requiring adapters for machine consumption.
3. **Discovery problem**: New users must know which tools to install. Built-in means it works out of the box.
4. **Rust precedent**: Rust's `clippy` started external but is now tightly integrated. Astra benefits from learning this lesson early.

### What Astra Does NOT Need from External Linters

- **Style linting**: The canonical formatter (`astra fmt`) already handles all style concerns. No need for style-related lint rules.
- **Bolted-on type checking**: Types are native to Astra. No need for a mypy equivalent.

## Consequences

### Positive

- `astra check` is the single command for all static analysis
- `--strict` mode provides CI-ready enforcement with no extra tools
- Warning codes (W0xxx) are stable and documented alongside error codes
- Machine-readable JSON output includes both errors and warnings

### Negative

- Lint rules add complexity to the type checker
- New lint rules require compiler changes (not just plugin updates)
- Risk of the compiler becoming a monolith

### Mitigations

- Lint logic is isolated in `LintScope` / lint helper methods, not interleaved with type checking logic
- Per-rule configuration in `astra.toml` allows suppressing rules that don't apply to a project
- `_` prefix convention suppresses W0001 without annotations

## Implemented Rules

| Code | Rule | Severity |
|------|------|----------|
| W0001 | Unused variable | Warning |
| W0002 | Unused import | Warning |
| W0003 | Unreachable code after return | Warning |
| W0004 | Deprecated feature | Reserved |
| W0005 | Wildcard match on known exhaustive type | Warning |
| W0006 | Shadowed binding in same scope | Warning |
| W0007 | Redundant type annotation | Reserved |
