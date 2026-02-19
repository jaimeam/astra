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

**13 interpreter builtins** had no type checker entry:

| Function | Expected Signature | Required Effects |
|----------|-------------------|-----------------|
| `read_file` | `(Text) -> Text` | Fs |
| `write_file` | `(Text, Text) -> Unit` | Fs |
| `http_get` | `(Text) -> Text` | Net |
| `http_post` | `(Text, Text) -> Text` | Net |
| `random_int` | `(Int, Int) -> Int` | Rand |
| `random_bool` | `() -> Bool` | Rand |
| `current_time_millis` | `() -> Int` | Clock |
| `get_env` | `(Text) -> Option[Text]` | Env |
| `regex_match` | `(Text, Text) -> List[Text]` | (pure) |
| `regex_find_all` | `(Text, Text) -> List[Text]` | (pure) |
| `regex_replace` | `(Text, Text, Text) -> Text` | (pure) |
| `regex_split` | `(Text, Text) -> List[Text]` | (pure) |
| `regex_is_match` | `(Text, Text) -> Bool` | (pure) |

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

4. **A compile-time or test-time check should verify sync.** (See
   consequences below.)

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

### Future Work

- Add a `#[test]` that extracts both builtin name lists and asserts they
  match
- Consider a declarative builtin registry (single source of truth for name,
  type signature, and runtime dispatch)
- Type the remaining 13 builtins identified in this ADR
