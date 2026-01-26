# Role: Tooling/CLI + Manifest Engineer

## Responsibility
Build and maintain the CLI toolchain and project manifest handling.

## Deliverables
- [ ] CLI with commands: `fmt`, `check`, `test`, `run`, `package`
- [ ] Manifest parsing (`astra.toml`)
- [ ] Lockfile handling (`astra.lock`)
- [ ] JSON diagnostic output
- [ ] Incremental caching strategy

## Key Files
- `src/main.rs` - CLI entry point
- `src/cli/mod.rs` - Command implementations
- `src/cli/fmt.rs` - Format command
- `src/cli/check.rs` - Check command
- `src/cli/test.rs` - Test command
- `src/cli/run.rs` - Run command
- `src/manifest/mod.rs` - Manifest parsing
- `src/manifest/lock.rs` - Lockfile handling

## CLI Commands
```bash
astra fmt [files...]     # Format files (all if none specified)
astra check [files...]   # Parse + typecheck + lint
astra test [filter]      # Run tests, optional name filter
astra run <target>       # Run main entrypoint
astra package            # Create distributable artifact
```

## Command Flags
```bash
--json          # Output diagnostics as JSON
--quiet         # Minimal output
--verbose       # Detailed output
--watch         # Watch for changes (future)
```

## Manifest Format (astra.toml)
```toml
[package]
name = "my-project"
version = "0.1.0"

[targets]
default = "wasm"

[dependencies]
other-lib = "1.0.0"

[features]
default = ["std"]
```

## Acceptance Criteria
- `astra check` is fast (< 1s for small projects)
- Diagnostics available in both human and JSON format
- Exit codes: 0 = success, 1 = errors, 2 = usage error
- Manifest errors have clear messages

## Interface Contract
See `.claude/contracts/diagnostics.md` for JSON output format.

## Dependencies
- All other components (CLI orchestrates them)

## Testing Strategy
```bash
# Run CLI tests
cargo test --lib cli

# Integration tests
cargo test --test cli_integration
```

## Common Pitfalls
- Inconsistent exit codes
- Missing error handling for file I/O
- Not respecting --json flag everywhere
- Slow startup time
