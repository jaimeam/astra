# ADR-005: Interpreter-TypeChecker Built-in Sync Requirement

## Status

Accepted

## Context

In v1.1, several built-in functions (`json_parse`, `json_stringify`, regex
functions, effect-related I/O functions) were added to the interpreter's
runtime dispatch without corresponding entries in the type checker. This
created a class of bugs where:

1. **Silent type loss**: Functions like `json_parse` returned `Type::Unknown`
   from the type checker, which is compatible with everything. Code using
   these functions got no type checking at all — type errors only surfaced
   as runtime crashes.

2. **Phantom named types**: Using `Json` as a type annotation resolved to
   `Type::Named("Json", [])`, which didn't match any concrete type. This
   made it impossible to write properly typed function signatures involving
   JSON data.

3. **Inconsistent coverage**: Some builtins (like `len`, `to_text`) were
   registered in the type checker's known-names list, while others added
   in the same release cycle were not. No mechanism detected the drift.

### Scope of the Gap (at time of discovery)

**13 interpreter builtins** had no type checker entry. All have since been
fixed with proper `Type::Function` signatures:

| Function | Signature | Effects | Status |
|----------|-----------|---------|--------|
| `read_file` | `(Text) -> Text` | Fs | Fixed |
| `write_file` | `(Text, Text) -> Unit` | Fs | Fixed |
| `http_get` | `(Text) -> Text` | Net | Fixed |
| `http_post` | `(Text, Text) -> Text` | Net | Fixed |
| `random_int` | `(Int, Int) -> Int` | Rand | Fixed |
| `random_bool` | `() -> Bool` | Rand | Fixed |
| `current_time_millis` | `() -> Int` | Clock | Fixed |
| `get_env` | `(Text) -> Option[Text]` | Env | Fixed |
| `regex_match` | `(Text, Text) -> Option[Json]` | (pure) | Fixed |
| `regex_find_all` | `(Text, Text) -> List[Json]` | (pure) | Fixed |
| `regex_replace` | `(Text, Text, Text) -> Text` | (pure) | Fixed |
| `regex_split` | `(Text, Text) -> List[Text]` | (pure) | Fixed |
| `regex_is_match` | `(Text, Text) -> Bool` | (pure) | Fixed |

## Decision

**Every built-in function registered in the interpreter MUST have a
corresponding type signature in the type checker.** No built-in may exist
in only one layer.

### Rules

1. **Adding a built-in to the interpreter requires adding its type signature
   to the type checker in the same commit.** The two dispatch tables must
   stay in sync.

2. **Built-in functions should have proper `Type::Function` signatures, not
   `Type::Unknown`.** `Type::Unknown` is for error recovery, not for "we
   haven't typed this yet."

3. **New built-in types (like `Json`) need a `Type` enum variant** if they
   represent a distinct runtime concept that doesn't map to an existing
   variant. Using `Type::Named(name, [])` for a built-in type is a code
   smell — it won't match concrete types in compatibility checks.

4. **A test-time check verifies sync.** The
   `test_interpreter_typechecker_builtin_sync` test parses both source
   files and asserts every interpreter builtin has a type checker entry.

## Rationale

The interpreter and type checker are two views of the same language. If
they diverge, users get one of two bad outcomes:

- **Type checker rejects valid code**: The function works at runtime but
  `astra check` reports "unknown identifier." This is annoying but safe.

- **Type checker accepts invalid code**: The function returns
  `Type::Unknown`, which matches everything. Type errors slip through to
  runtime. This violates Astra's "verifiability first" principle.

The second case is worse because it's silent. The Json bug was this case:
`json_parse` returned `Type::Unknown`, so you could assign its result to
an `Int` variable and only discover the problem at runtime.

## Consequences

### Positive

- Type safety for all built-in functions, not just the ones added before
  v1.0
- `astra check` catches argument type errors for `json_parse`, regex
  functions, etc.
- `Json` type annotations work correctly in function signatures
- Clear rule for future contributors: touching interpreter dispatch means
  touching the type checker

### Negative

- More work per built-in (must update two files)
- Risk of over-typing effect-based builtins (their real signatures depend
  on capability injection)

### Implemented Safeguards

- `test_interpreter_typechecker_builtin_sync` in `src/typechecker/mod.rs`
  parses both source files with `include_str!`, extracts builtin names from
  the interpreter's Call dispatch and the type checker's Ident match, and
  fails the build if any interpreter builtin is missing from the type
  checker.
- All 13 builtins from the original gap have been typed with proper
  `Type::Function` signatures (including effect declarations).
- `Type::Json` was added as a first-class type variant for JSON values.

### Future Work

- Consider a declarative builtin registry (single source of truth for name,
  type signature, and runtime dispatch) to replace the two match blocks
