# Role: Testing/Property Testing Engineer

## Responsibility
Build and maintain the test framework, including unit tests, property tests, and test runner.

## Deliverables
- [ ] Unit test harness with `test` blocks
- [ ] Property testing with `property` blocks
- [ ] Generator combinators for property tests
- [ ] Shrinking for counterexample minimization
- [ ] JSON test results output
- [ ] Deterministic test execution

## Key Files
- `src/testing/mod.rs` - Test framework entry point
- `src/testing/runner.rs` - Test execution
- `src/testing/harness.rs` - Test harness
- `src/testing/property.rs` - Property testing
- `src/testing/generators.rs` - Value generators
- `src/testing/shrink.rs` - Shrinking logic
- `src/testing/results.rs` - Result types and JSON output

## Test Syntax
```astra
# Unit test
test "addition works" {
  assert_eq(1 + 1, 2)
}

# Property test
property "reverse twice is identity" {
  forall list: List[Int] {
    assert_eq(list.reverse().reverse(), list)
  }
}

# Test with capability injection
test "clock-dependent code" {
  using effects(Clock = Clock.fixed(1234567890))

  let result = get_formatted_time()
  assert_eq(result, "2009-02-13 23:31:30")
}
```

## JSON Results Format
```json
{
  "summary": {
    "total": 42,
    "passed": 40,
    "failed": 2,
    "skipped": 0,
    "duration_ms": 1234
  },
  "tests": [
    {
      "name": "addition works",
      "file": "src/math.astra",
      "line": 10,
      "status": "passed",
      "duration_ms": 5
    },
    {
      "name": "reverse twice is identity",
      "file": "src/list.astra",
      "line": 25,
      "status": "failed",
      "duration_ms": 150,
      "failure": {
        "message": "Assertion failed",
        "counterexample": { "list": "[1, 2, 3]" },
        "shrunk_from": { "list": "[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]" }
      }
    }
  ]
}
```

## Acceptance Criteria
- Tests are deterministic (same results every run)
- Property tests use seeded PRNG
- Failing property tests produce minimal counterexamples
- JSON output matches schema
- Tests can inject fake capabilities

## Determinism Requirements
- `Rand` in tests uses fixed seed by default
- `Clock` in tests uses fixed time by default
- Test execution order is deterministic
- Parallel test execution (if any) is reproducible

## Interface Contract
Test runner receives AST with test blocks and produces results.
Must respect capability injection syntax.

## Dependencies
- Parser (test block syntax)
- Type checker (test type checking)
- Interpreter (test execution)
- Effects system (capability injection)

## Testing Strategy
```bash
# Run test framework tests
cargo test --lib testing

# Meta-test: run Astra tests
cargo run -- test tests/astra/
```

## Common Pitfalls
- Non-deterministic test ordering
- Inefficient shrinking
- Poor generator distribution
- Not isolating tests from each other
