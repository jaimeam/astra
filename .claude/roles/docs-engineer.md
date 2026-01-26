# Role: Docs + Examples Engineer

## Responsibility
Build and maintain documentation, examples, and learning resources.

## Deliverables
- [ ] Getting started guide
- [ ] Language reference
- [ ] Idioms guide for agent-friendly code
- [ ] Example programs with explanations
- [ ] Error code documentation
- [ ] API documentation for stdlib

## Key Files
- `docs/getting-started.md` - Quick start guide
- `docs/spec.md` - Language specification
- `docs/idioms.md` - Best practices
- `docs/errors.md` - Error code reference
- `docs/formatting.md` - Formatting rules
- `docs/effects.md` - Effects system guide
- `docs/adr/` - Architecture Decision Records
- `examples/` - Example programs

## Documentation Standards

### Getting Started
1. Installation instructions
2. Hello World example
3. Basic syntax overview
4. Running tests
5. Next steps

### Error Documentation Format
```markdown
## E1001: Type mismatch

**Message**: Expected `{expected}`, found `{found}`

**Explanation**: This error occurs when...

**Example**:
\`\`\`astra
fn add(a: Int, b: Int) -> Int {
  a + "hello"  // Error: Expected Int, found Text
}
\`\`\`

**Fix**: Ensure the types match...
```

### ADR Format
```markdown
# ADR-001: Title

## Status
Accepted | Proposed | Deprecated

## Context
What is the issue we're addressing?

## Decision
What is the change we're making?

## Consequences
What are the positive and negative effects?
```

## Example Programs

### 1. Hello World (`examples/hello.astra`)
```astra
module hello

fn main() effects(Console) {
  Console.println("Hello, Astra!")
}
```

### 2. HTTP Client (`examples/http_client.astra`)
```astra
module http_client

fn main() effects(Net, Console) {
  match Net.get("https://api.example.com/data") {
    Ok(response) => Console.println(response.body)
    Err(e) => Console.println("Error: " + e.message)
  }
}

test "http client with mock" {
  using effects(Net = MockNet.returning({ status = 200, body = "test" }))
  // Test logic here
}
```

### 3. File Processing (`examples/file_processor.astra`)
```astra
module file_processor

fn process_file(path: Text) -> Result[Int, FsError]
  effects(Fs)
{
  let content = Fs.read(path)?
  let lines = content.split("\n")
  Ok(lines.length())
}
```

## Acceptance Criteria
- Documentation matches actual behavior
- All examples compile and run
- Examples are tested in CI
- Error codes are all documented
- No broken links

## Dependencies
- All other components (docs describe them)

## Testing Strategy
```bash
# Test all examples compile
cargo run -- check examples/

# Test all examples run
cargo run -- test examples/
```

## Common Pitfalls
- Documentation getting out of sync with code
- Missing examples for common patterns
- Unclear error messages in docs
- Not testing documentation examples
