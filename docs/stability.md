# Astra v1.0 Stability Guarantee

## What This Means

Starting with v1.0, Astra makes the following stability promises:

### Language Stability

1. **No breaking syntax changes.** All programs that compile and run under v1.0 will
   continue to compile and run under any v1.x release.

2. **Error codes are stable.** Error codes (E0xxx through E4xxx and W0xxx) will not be
   removed or change their meaning. New error codes may be added.

3. **Standard library is stable.** Public functions and types in `std.*` modules will
   not be removed or change their signatures. New functions and modules may be added.

4. **Effect names are stable.** The built-in effects (Console, Fs, Net, Clock, Rand, Env)
   and their methods will not be removed or change their signatures. New effects and
   methods may be added.

### Tooling Stability

5. **CLI interface is stable.** The commands `fmt`, `check`, `test`, `run`, `init`, `doc`,
   `fix`, and `explain` will not change their behavior in breaking ways. New commands and
   flags may be added.

6. **JSON output is stable.** The `--json` flag produces structured output whose schema
   will not change in breaking ways. New fields may be added.

7. **Formatter output is stable.** The canonical format produced by `astra fmt` will not
   change within a minor version series (v1.0.x). It may change between minor versions
   (v1.1, v1.2) with clear documentation.

### What Is NOT Guaranteed

- **Performance characteristics** may change (generally for the better).
- **Error message wording** may change (error codes remain stable).
- **Internal APIs** (anything not `public`) may change.
- **Compiler/interpreter internals** may change.
- **Features marked v1.1** (async/await, package manager) are experimental and not
  covered by this guarantee.

## Versioning Scheme

Astra follows [Semantic Versioning](https://semver.org/):

- **MAJOR** (1.x.x → 2.0.0): Breaking changes to the language or standard library.
  These will be rare and well-documented with migration guides.
- **MINOR** (1.0.x → 1.1.0): New features, new standard library modules, new error
  codes. Backward compatible.
- **PATCH** (1.0.0 → 1.0.1): Bug fixes, performance improvements, documentation
  updates. Backward compatible.

## Deprecation Policy

When a feature needs to be replaced:

1. The old feature is **deprecated** with a W0xxx warning for at least one minor version.
2. The `astra fix` command provides automatic migration where possible.
3. The old feature is removed only in the next **major** version.
4. A migration guide is published with every major version.

## Reporting Stability Issues

If you believe a release violates this stability guarantee, please report it as a bug.
Unintentional breakage will be treated as high-priority and patched promptly.
