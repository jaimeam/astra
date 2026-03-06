# ADR-007: Defer Async/Await and Package Manager to v1.1

**Status**: Accepted
**Date**: 2026-03-05

## Context

Astra v1.0 aims to deliver a stable, well-tested language. Two features currently exist in
partial form:

1. **Async/Await** — The parser recognizes `async fn` and `await` expressions, and the
   interpreter can create `Future` values, but there is no event loop, no concurrent
   execution, and `await` simply evaluates the future synchronously.

2. **Package Manager** (`astra pkg`) — The CLI command is defined, but dependency
   resolution, registry support, and lock-file generation are not implemented.

## Decision

Both features are **explicitly deferred to v1.1**. They will not be part of the v1.0
stability guarantee.

### Rationale

- **Half-implemented features erode trust.** A v1.0 label promises stability. Shipping
  async with no real concurrency, or a package manager with no registry, would confuse
  users and create upgrade friction when the real implementations land.

- **Both features have deep design implications.** Async interacts with the effect system
  (which effects can an async function declare?), error propagation (what happens when a
  future fails?), and testing (how do you deterministically test concurrent code?). The
  package manager needs dependency resolution, version constraints, and a distribution
  story. Rushing either would create backward-compatibility debt.

- **v1.0 is strong without them.** Astra's core — types, effects, contracts, pattern
  matching, modules, testing — is complete and well-tested. These are the features that
  define the language's identity.

## Consequences

- The `async` and `await` keywords remain reserved but produce a clear error:
  "Async/await is planned for v1.1. See docs/roadmap.md."
- `astra pkg` prints a message directing users to manual module management for now.
- The v1.1 roadmap will be published alongside the v1.0 release.
- No breaking changes to the existing syntax are planned — when async lands, it will
  use the existing `async fn` / `await` syntax.
