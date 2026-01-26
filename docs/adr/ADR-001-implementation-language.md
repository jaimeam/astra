# ADR-001: Implementation Language

## Status

Accepted

## Context

We need to choose a language for implementing the Astra compiler and toolchain. The implementation language affects:

- Development velocity
- Runtime performance
- Ecosystem and library availability
- Target platform support
- Tooling quality

Astra's goals of verifiability, fast feedback loops, and deterministic behavior inform this choice.

## Decision

**Implement the Astra toolchain in Rust.**

## Rationale

### Why Rust

1. **Memory Safety**: Prevents entire classes of bugs without runtime overhead
2. **Performance**: Fast compilation and execution, important for toolchain responsiveness
3. **Ecosystem**: Excellent crates for parsers (logos, nom), CLIs (clap), serialization (serde)
4. **WASM Support**: Easy path to compile Astra programs to WASM
5. **Tooling**: cargo, clippy, and rustfmt provide excellent developer experience
6. **Type System**: Expressive type system helps model language constructs accurately
7. **Error Handling**: Result type aligns with Astra's own error handling philosophy

### Alternatives Considered

**OCaml**
- Pro: Excellent for compiler front-ends, pattern matching, ADTs
- Con: Smaller ecosystem, packaging complexity, less familiar to contributors
- Con: Harder to build robust CLI tooling

**TypeScript**
- Pro: Fast prototyping, familiar to many developers
- Con: Runtime type unsoundness contradicts verifiability goals
- Con: Performance concerns for incremental compilation
- Con: Node.js dependency adds complexity

**Zig**
- Pro: Low-level control, fast compilation
- Con: Less mature ecosystem
- Con: Fewer libraries for parsing and CLI

## Consequences

### Positive

- Toolchain will be fast and memory-efficient
- Single binary distribution (no runtime dependencies)
- Strong guarantees about toolchain correctness
- Easy cross-compilation to multiple platforms
- Clear path to WASM compilation target

### Negative

- Steeper learning curve for some contributors
- Longer initial development time vs scripting languages
- Compile times for the toolchain itself

### Neutral

- Need to learn Rust idioms for compiler development
- Will use cargo for project management

## Implementation Notes

- Use `logos` for lexer (fast, macro-based)
- Use `clap` for CLI argument parsing
- Use `serde` + `serde_json` for serialization
- Use `miette` for beautiful error reporting
- Use `insta` for snapshot testing
