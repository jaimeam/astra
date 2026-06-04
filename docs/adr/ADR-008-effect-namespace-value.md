# ADR-008: Effect Namespaces Are a Distinct Runtime Value

## Status

Accepted

## Context

Effect/capability namespaces (`Console`, `Fs`, `Net`, `Clock`, `Rand`, `Env`,
the `Map`/`Set` static constructors, and user-defined effects) were represented
at runtime as `Value::Text(name)` — the exact same variant used for ordinary
strings. Method dispatch in `call_method` then decided whether a receiver was an
effect by **inspecting the string's content**:

```rust
Value::Text(name) if name.starts_with("Net") => self.call_net_method(...),
Value::Text(name) if name.starts_with("Fs")  => self.call_fs_method(...),
Value::Text(name) if name.starts_with("Console") => self.call_console_method(...),
```

This conflated a value's *type* with its *content*. Any string literal whose
content happened to begin with a capitalized effect name was misrouted to that
effect's handler:

| Input        | Old result                              |
|--------------|-----------------------------------------|
| `"Netback"`  | `[E4004] capability not available: Net` |
| `"Network"`  | `[E4004] capability not available: Net` |
| `"Net"`      | `[E4004] capability not available: Net` |
| `"Fsfoo"`    | `[E4004] capability not available: Fs`  |
| `"Console"`  | `[E4006] unknown method: Console.to_lower` |
| `"Coke"`     | OK (no effect prefix)                   |
| `"NETBACK"`  | OK (case-sensitive prefix missed)       |

`astra check` reported no errors because the type checker correctly typed the
receiver as `Text`; the bug lived entirely in the interpreter's value-based
dispatch.

## Decision

Effect/namespace references get their own runtime variant, `Value::Effect(String)`,
distinct from `Value::Text`.

- Identifiers that name a built-in effect/namespace, and user-defined effect
  names, evaluate to `Value::Effect(name)` (see `eval_expr` for `Expr::Ident`).
- `call_method` dispatches to an effect handler **only** when the receiver is a
  `Value::Effect`, matching the exact name. Every other receiver — including all
  `Value::Text` — resolves by type through `call_value_method`.

Method dispatch now keys off the receiver's type, never the string a value
happens to hold. A `Text` value can spell anything (`"Net"`, `"Console"`,
`"Netback"`) and still resolves its Text methods.

## Consequences

- String methods (`to_lower`, `to_upper`, `trim`, `contains`, `starts_with`,
  `len`, `split`, `replace`, …) work on any string content, regardless of
  whether it resembles an effect name.
- Effect/capability resolution applies only to genuine identifier expressions in
  source, never to string values — matching the language's local-reasoning and
  unambiguous-semantics principles.
- Adding a `Value` variant requires arms in the exhaustive matches over `Value`
  (`type_tag`, `format_value`, `json_stringify_value`, `value_type_name`). These
  are covered.
- Regression coverage lives in `src/interpreter/tests.rs`
  (`test_string_value_never_dispatches_to_effect`,
  `test_all_string_methods_on_effect_prefixed_literal`,
  `test_effect_namespace_still_dispatches`).
