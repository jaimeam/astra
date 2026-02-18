# {{PROJECT_NAME}}

This project uses the [Astra](https://github.com/jaimeam/astra) programming language.

## Commands

```bash
astra check src/              # Type-check all files (must pass with 0 errors)
astra check --json src/       # Same, with machine-readable JSON output
astra test                    # Run all tests (must pass)
astra fix src/                # Auto-apply suggested fixes
astra fmt src/                # Format all code canonically
{{RUN_HINT}}astra explain E1001           # Explain any error code
```

## Workflow

1. Write or edit `.astra` files in `src/`
2. Run `astra check src/` — fix any errors before moving on
3. Run `astra test` — fix any failures
4. Run `astra fmt src/` — format before committing

Use `astra check --json src/` for structured error output. Each diagnostic
includes an error code, file location, message, and suggested fix.

## Language Quick Reference

- Every file starts with `module <name>`
- Functions: `fn name(param: Type) -> ReturnType { body }`
- Side effects must be declared: `fn name() effects(Console, Fs) { ... }`
- No null — use `Option[T]` with `Some(value)` / `None`
- Errors use `Result[T, E]` with `Ok(value)` / `Err(error)`
- Export with `public fn`, import with `import module.{name}`
- Tests are inline `test "name" { ... }` blocks
- Comments use `#`, no semicolons

## Effects

Functions that perform I/O must declare their effects:

| Effect    | Description         | Example                              |
|-----------|---------------------|--------------------------------------|
| `Console` | Terminal I/O        | `println("hello")`                   |
| `Fs`      | File system         | `Fs.read("path")`                    |
| `Net`     | Network requests    | `Net.get("https://...")`             |
| `Clock`   | Current time        | `Clock.now()`                        |
| `Rand`    | Random numbers      | `Rand.int(1, 100)`                   |
| `Env`     | Environment vars    | `Env.get("KEY")`                     |

Pure functions (no `effects` clause) have no side effects and are safe to
call from anywhere.

## Error Codes

- `E0xxx` — Syntax / parsing errors
- `E1xxx` — Type errors
- `E2xxx` — Effect errors
- `E3xxx` — Contract violations
- `E4xxx` — Runtime errors

Run `astra explain <code>` for a detailed explanation of any error.

## Docs

- [Getting Started](https://github.com/jaimeam/astra/blob/main/docs/getting-started.md)
- [Language Spec](https://github.com/jaimeam/astra/blob/main/docs/spec.md)
- [Standard Library](https://github.com/jaimeam/astra/blob/main/docs/stdlib.md)
- [Effects System](https://github.com/jaimeam/astra/blob/main/docs/effects.md)
- [Error Codes](https://github.com/jaimeam/astra/blob/main/docs/errors.md)
