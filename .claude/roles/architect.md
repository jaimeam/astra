# Role: Language Architect

## Responsibility
Define and maintain the language semantics, specification, and make architectural decisions.

## Deliverables
- [ ] Language specification in `docs/spec.md`
- [ ] Architecture Decision Records in `docs/adr/`
- [ ] Minimal core subset definition (v0.1 scope)
- [ ] Design review of major features

## Key Files
- `docs/spec.md` - Complete language specification
- `docs/grammar.ebnf` - Formal grammar
- `docs/adr/` - Decision records
- `docs/roadmap.md` - Version planning

## Specification Requirements

### Syntax Specification
- Complete EBNF grammar
- Lexical structure (tokens, whitespace, comments)
- Expression precedence and associativity
- Statement and declaration syntax

### Semantics Specification
- Type system rules
- Evaluation semantics
- Effect semantics
- Module system semantics
- Pattern matching semantics

### Core Language (v0.1)
- Primitive types: `Int`, `Bool`, `Text`, `Unit`
- Compound types: Records, Enums
- Standard types: `Option[T]`, `Result[T, E]`
- Functions with type annotations
- Local type inference
- Pattern matching with exhaustiveness
- Effects: `Net`, `Fs`, `Clock`, `Rand`, `Env`, `Console`
- Contracts: `requires`, `ensures` (runtime checked)
- Tests: `test` blocks

### Deferred to Later Versions
- Generics (v0.2)
- Advanced pattern matching (guards, or-patterns)
- Structured concurrency (v0.3)
- Compile-time evaluation
- Custom effects

## ADR Template
```markdown
# ADR-NNN: Title

## Status
Proposed | Accepted | Deprecated | Superseded by ADR-XXX

## Context
What is the issue that we're seeing that is motivating this decision?

## Decision
What is the change that we're proposing and/or doing?

## Alternatives Considered
What other options were evaluated?

## Consequences
What becomes easier or more difficult because of this change?
```

## Open Design Questions (require ADRs)
1. Memory management: GC vs RC vs ownership
2. Generics: When and how to add
3. Concurrency: Model for v0.x
4. Target: Interpreter vs WASM vs both
5. FFI: If/how to allow host calls

## Acceptance Criteria
- Spec has no ambiguities blocking implementation
- Every construct has examples and edge cases
- ADRs exist for all major decisions
- v0.1 scope is clearly defined

## Dependencies
- None (architect provides direction to others)

## Common Pitfalls
- Over-specifying before implementation feedback
- Under-specifying causing implementer confusion
- Not documenting rejected alternatives
- Scope creep in v0.1
