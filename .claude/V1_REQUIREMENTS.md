# Astra v1.0: Definitive Feature Requirements

> **Purpose**: This is the exhaustive, final list of everything needed before Astra v1.0 can
> be considered "ready for first repos." No more surprise follow-ups. Everything is here.
>
> **Date**: 2026-02-17
>
> **Scope**: Astra is a tree-walking interpreted language targeting LLM/agent workflows.
> v1.0 does NOT need a compiler backend, WASM target, or package registry. It needs to be
> a language where someone (human or LLM) can write real multi-file projects, run them,
> test them, and get clear feedback when something is wrong.

---

## Current State: Honest Assessment

**What works well** (~90% of day-to-day usage):
- Parser handles all syntax constructs
- Interpreter executes programs correctly
- Single-file type checking catches real errors
- Effect system enforces capability declarations
- Formatter produces canonical output
- Test runner executes test blocks with mock injection
- CLI tooling (run, check, test, fmt, fix, explain, repl, init, doc) all functional
- 427 tests passing, 14 examples running

**What's partially working** (works in demo, breaks in real projects):
- Cross-file imports work at runtime but type checker doesn't validate across modules
- Generics use basic unification, not full constraint solving
- Traits dispatch at runtime but type checker doesn't resolve trait methods on calls
- LSP works within a single file, no cross-file features
- Property tests run 100 iterations but have no shrinking on failure

**What's a stub or misleading** (looks implemented but isn't):
- `async`/`await` is pure syntax sugar that does nothing
- `astra package` just copies source files to a directory
- Incremental compilation cache stores diagnostics but doesn't cache parsed ASTs or type info

---

## Tier 1: BLOCKERS

These prevent anyone from using Astra for a real project. Each one will be hit within
the first hour of real usage.

### B1. Cross-File Type Checking

**Current state**: The type checker operates on a single file. When you `import std.math.{clamp}`, the type checker registers `clamp` as a known name (to suppress E1002 "unknown identifier") but does NOT know its parameter types, return type, or effect signature. Type errors in cross-module calls are only caught at runtime.

**What's needed**:
- When processing an import, parse the imported module and register the types of all imported symbols (functions, types, enums, traits)
- Validate argument types/counts for cross-module function calls at type-check time
- Validate that imported types are used correctly (field access, pattern matching)
- Validate effect propagation: if an imported function has `effects(Net)`, the caller must declare `effects(Net)`
- This applies to both `import std.*` (stdlib) and user-defined module imports

**Acceptance criteria**:
```astra
# file: math_utils.astra
module math_utils
public fn double(x: Int) -> Int { x * 2 }

# file: main.astra
import math_utils.{double}
fn main() {
  let result = double("hello")  # Should be E1001 at check time, not runtime
}
```
Running `astra check main.astra` must report `E1001: type mismatch` on the call to `double`.

**Scope**: This is the single largest piece of work. Touches typechecker, CLI (file resolution), and potentially parser (for multi-file passes).

---

### B2. Remove or Error on async/await

**Current state**: `await expr` is parsed and then evaluates `expr` synchronously. There is no async runtime, no futures, no concurrency. Users will write `await` expecting it to do something and get silently wrong behavior.

**What's needed** (choose one):
- **Option A (recommended)**: Remove `await` from the lexer/parser entirely. Remove `async` if it exists. Produce E0006 "reserved keyword" if someone writes `await`. This is honest — v1.0 is single-threaded.
- **Option B**: Keep the syntax but emit a W0004 "deprecated/experimental" warning: "async/await is not yet implemented and has no effect."

**Acceptance criteria**: Writing `await some_call()` either fails at parse/check time with a clear message, or produces a visible warning. It must NOT silently evaluate synchronously.

---

### B3. Proper Exit Codes

**Current state**: Need to verify that `astra run` returns exit code 0 on success and non-zero on:
- Runtime errors (division by zero, assertion failure, contract violation)
- Parse errors (invalid syntax)
- Type check errors (when running with checks)
- Uncaught errors (Result unwrap failure)

**What's needed**: `astra run`, `astra check`, `astra test`, and `astra fmt --check` must all return appropriate exit codes so they can be used in CI/CD pipelines and shell scripts.

**Acceptance criteria**:
```bash
astra run good_program.astra; echo $?    # 0
astra run bad_program.astra; echo $?     # 1
astra check bad_types.astra; echo $?     # 1
astra test failing_tests.astra; echo $?  # 1
astra fmt --check unformatted.astra; echo $?  # 1
```

---

### B4. Type Checker False Positives Audit

**Current state**: The type checker may produce false E1002 "unknown identifier" errors for valid code patterns that it doesn't understand. Specifically:
- Methods called on values returned from imported functions (the type of the return value is unknown)
- Chained method calls on collections (`list.filter(f).map(g)`)
- Variables bound in for loops over imported collections
- Enum variants from imported enums used in match patterns

**What's needed**: A systematic audit of what valid Astra code produces false type errors, followed by fixes. The type checker should never reject valid code — it's better to miss a bug than to block correct programs. If a check can't be performed reliably (e.g., because the type information isn't available), skip the check silently rather than report a false error.

**Acceptance criteria**: Every example program and every stdlib module passes `astra check --strict` with zero errors and zero warnings (other than intentional W0008 for demo unused functions).

---

### B5. Runtime Error Source Locations

**Current state**: Stack traces are attached to runtime errors, but need to verify they include file name and line numbers, not just function names.

**What's needed**: Every runtime error message must include:
- The file path where the error occurred
- The line number (and ideally column)
- The call stack showing how execution got there
- The error code (E4xxx)

**Acceptance criteria**:
```
Error E4001: Division by zero
  --> src/math.astra:15:7
  |
15|   a / b
  |       ^ division by zero

Call stack:
  main (src/main.astra:3)
  calculate (src/math.astra:15)
```

---

## Tier 2: EXPECTED FEATURES

Users will encounter these within the first few days of real usage. Not blocking for a
"hello world" or a single-file script, but blocking for any non-trivial project.

### E1. Compound Assignment Operators

**Current state**: To increment a variable you must write `x = x + 1`.

**What's needed**: Support `+=`, `-=`, `*=`, `/=`, `%=` as syntactic sugar.
- Parser: recognize these tokens
- Interpreter: desugar to `x = x OP expr`
- Formatter: format canonically
- Type checker: validate operand types

**Acceptance criteria**:
```astra
let mut count = 0
count += 1
count *= 2
```

---

### E2. Index Access for Lists

**Current state**: Need to verify whether `list[0]` or `list.get(0)` works. Lists have `head`, `tail`, and method-based access, but direct index syntax `list[i]` may not be supported.

**What's needed**: If not already present, add `list[index]` syntax that returns `Option[T]` (safe) or panics with E4002 (direct access). Decide on semantics:
- **Option A (recommended)**: `list[i]` returns the element directly and panics with E4002 if out of bounds (like most languages)
- **Option B**: `list.get(i)` returns `Option[T]` (safe), `list[i]` is syntax sugar for the direct access

**Acceptance criteria**:
```astra
let items = [10, 20, 30]
let first = items[0]       # 10
let second = items[1]      # 20
```

---

### E3. String Concatenation Clarity

**Current state**: Need to verify how strings are concatenated. Is it `+` operator, a `concat` method, or string interpolation only?

**What's needed**: Ensure there's a clear, obvious way to concatenate strings:
- `+` operator on Text values, OR
- `text.concat(other)` method, OR
- String interpolation `"${a}${b}"` as the primary method

Document whichever approach is canonical. If `+` doesn't work on strings, add it — every programmer expects this.

---

### E4. Nested Pattern Matching

**Current state**: Need to verify that nested patterns work:

```astra
match option_of_option {
  Some(Some(x)) => x
  Some(None) => 0
  None => 0
}
```

**What's needed**: If nested patterns don't work, implement them. Pattern matching is a core differentiator and must handle:
- Nested enum variants: `Some(Ok(value))`
- Nested record destructuring: `Some({name, age})`
- Mixed nesting: `Ok(Some([first, ..rest]))`

---

### E5. Closure Variable Capture

**Current state**: Need to verify that closures properly capture variables from their enclosing scope.

**What's needed**: Closures must capture by value (since Astra is immutable-by-default):
```astra
fn make_adder(n: Int) -> (Int) -> Int {
  fn(x) { x + n }  # n must be captured from enclosing scope
}

let add5 = make_adder(5)
assert_eq(add5(3), 8)
```

If closures don't capture outer variables, this is a fundamental bug that must be fixed.

---

### E6. User-Defined Error Types with Result

**Current state**: `Result[T, E]` is generic over the error type. Verify that users can define custom error enums and use them as the `E` parameter.

**What's needed**:
```astra
enum AppError =
  | NotFound(resource: Text)
  | Unauthorized
  | ParseFailed(reason: Text)

fn find_user(id: Int) -> Result[User, AppError] {
  if id < 0 {
    Err(NotFound("user"))
  } else {
    Ok(get_user(id))
  }
}
```

The `?` operator must propagate the correct error type. Match expressions on `Err(e)` must allow matching the inner error enum.

---

### E7. Map/Set Literal Syntax

**Current state**: Maps are created with `Map.new()` and built up with `.set()`. No literal syntax.

**What's needed**: Consider adding map literal syntax for convenience:
```astra
# Current (verbose)
let m = Map.new().set("a", 1).set("b", 2)

# Desired (if feasible within parser complexity budget)
let m = {"a": 1, "b": 2}
```

**Note**: This may conflict with record literal syntax `{field = value}`. If so, document the recommended pattern and defer literal syntax. The `Map.new().set().set()` chain is acceptable for v1.0 if literal syntax would be ambiguous.

---

### E8. For Loop Destructuring

**Current state**: Need to verify whether `for (key, value) in map.entries()` or `for {name, age} in users` works.

**What's needed**: For loops should support pattern destructuring in the binding position:
```astra
for (index, item) in list.enumerate() {
  println("${index}: ${item}")
}

for {name, age} in users {
  println("${name} is ${age}")
}
```

---

### E9. Multi-Line Strings

**Current state**: Need to verify whether multi-line string literals are supported (the plan mentions "multiline strings" in the parser).

**What's needed**: Support for multi-line strings, either:
- Triple-quoted strings `"""..."""`, OR
- Regular strings that span multiple lines with `\n` escapes

At minimum, `\n`, `\t`, `\\`, `\"` escape sequences must work in string literals.

---

### E10. Comparison Operations for All Types

**Current state**: Need to verify that `==` and `!=` work on all types (records, enums, lists, maps, tuples), not just primitives.

**What's needed**: Structural equality for all value types:
```astra
let a = {x = 1, y = 2}
let b = {x = 1, y = 2}
assert_eq(a == b, true)

let xs = [1, 2, 3]
let ys = [1, 2, 3]
assert_eq(xs == ys, true)

assert_eq(Some(42) == Some(42), true)
assert_eq(None == None, true)
```

---

## Tier 3: POLISH

These make the difference between "it works" and "it's pleasant to use." Important for
adoption but not blocking for validation.

### P1. Type Checker: Infer Return Types for Private Functions

**Current state**: Public functions require explicit return type annotations (E1003). Private functions should have their return types inferred.

**What's needed**: Verify that private function return type inference works correctly, especially for:
- Functions returning Option/Result
- Functions returning records or tuples
- Recursive functions (may need explicit annotation)

---

### P2. Better Error Messages for Common Mistakes

**Current state**: Error messages exist for all error codes, with suggestions.

**What's needed**: Audit error messages for the most common beginner mistakes and ensure they're helpful:
- Using `=` instead of `==` in conditions
- Missing `effects()` clause (suggest which effects are needed)
- Calling a function with wrong argument order (suggest the correct order if types match differently)
- Using an undefined variable that's similar to a defined one (Levenshtein suggestions — already implemented, verify it works)

---

### P3. REPL Improvements

**Current state**: REPL evaluates expressions and definitions. Single-file scope, no imports.

**What's needed**:
- Support `import` statements in REPL so users can load stdlib modules
- Show type information alongside values: `42 : Int` instead of just `42`
- Multi-line input (detect incomplete expressions and wait for more input)

---

### P4. Test Output Formatting

**Current state**: Test results are printed to stdout.

**What's needed**: Verify and improve test output to be:
- Clear pass/fail summary at the end: `103 passed, 0 failed`
- Failed test details with assertion context (expected vs actual values)
- JSON output mode for machine consumption (`astra test --json`)
- Proper filtering: `astra test "pattern"` should only run matching tests

---

### P5. Documentation Comments in Generated Docs

**Current state**: `astra doc` extracts `##` doc comments.

**What's needed**: Verify that:
- Doc comments on functions include parameter descriptions
- Doc comments on types describe fields
- Generated docs include function signatures with types
- Cross-references between types work (e.g., a function returning `Option[User]` links to both `Option` and `User`)

---

### P6. Consistent JSON Output Across All Commands

**Current state**: `astra check --json` and `astra fix --json` produce JSON output.

**What's needed**: Ensure all commands that produce structured output have a `--json` flag:
- `astra check --json` — diagnostics array
- `astra test --json` — test results array with pass/fail/error
- `astra fix --json` — applied fixes array
- `astra fmt --check --json` — files needing formatting

The JSON schema should be documented and stable.

---

### P7. LSP Cross-File Support

**Current state**: LSP works within a single file. Go-to-definition, hover, and completion only resolve symbols defined in the current file.

**What's needed**:
- Go-to-definition on imported symbols jumps to the source file
- Hover on imported functions shows their full signature
- Completion suggests symbols from imported modules
- Diagnostics work across the whole project (not just the open file)

---

### P8. Stdlib: sort_by with Custom Comparator

**Current state**: `list.sort()` sorts by natural order.

**What's needed**:
```astra
let users = [{name = "Zara", age = 25}, {name = "Alice", age = 30}]
let sorted = users.sort_by(fn(a, b) { a.name < b.name })
```

---

### P9. Stdlib: String to_chars and from_chars

**Current state**: String operations work on whole strings or substrings.

**What's needed**:
- `text.chars() -> List[Text]` — split into individual characters
- `Text.from_chars(chars: List[Text]) -> Text` — join characters back
- This enables character-level text processing

---

### P10. Stdlib: Basic JSON Object Parsing

**Current state**: `std.json` has `stringify`, `parse_int`, `parse_bool`, `escape`. No object/array parsing.

**What's needed**: At minimum, a way to parse JSON strings into Astra values:
```astra
# Parse JSON text into a Map/dynamic value
let data = json.parse('{"name": "Alice", "age": 30}')
# data is a Map[Text, ???] or a dedicated JsonValue enum
```

This is non-trivial because Astra is statically typed. Options:
- **Option A**: `JsonValue` enum with `JString(Text)`, `JNumber(Float)`, `JBool(Bool)`, `JNull`, `JArray(List[JsonValue])`, `JObject(Map[Text, JsonValue])`
- **Option B**: Typed deserializers: `json.parse_record[T](text: Text) -> Result[T, Text]` (more complex)
- **Option C**: Defer full JSON parsing to v1.1. The current `stringify`/`escape` functions are enough for JSON output. For JSON input, users can use string operations.

---

### P11. Stdlib: Regular Expression Support (or Pattern Matching on Strings)

**Current state**: String operations include `contains`, `starts_with`, `split`, `index_of`.

**What's needed**: Some form of pattern matching on strings:
```astra
# Option A: Regex
let pattern = Regex.new("[0-9]+")
let matches = pattern.find_all("abc 123 def 456")

# Option B: Glob/simple patterns
text.matches("*.txt")

# Option C: Defer to v1.1 — current string methods are sufficient for most cases
```

**Recommendation**: Defer to v1.1. The current string methods cover most use cases. Regex is a large feature with its own syntax and error handling.

---

## Tier 4: KNOWN LIMITATIONS (Accepted for v1.0, Document Clearly)

These are intentional limitations that should be **documented in a "Known Limitations" section
of the README/getting-started guide** so users aren't surprised.

### KL1. No Full Hindley-Milner Type Inference

Generic type parameters use basic unification at call sites. Complex generic scenarios may
require explicit type annotations. The type checker catches most real errors but won't
catch all type errors in generic code — those are caught at runtime instead.

**What to document**: "Astra v1.0 uses practical type inference rather than full Hindley-Milner.
Add explicit type annotations if the checker can't infer types."

### KL2. Traits Are Runtime-Dispatched

Trait method calls (`value.method()` where `method` comes from `impl Trait for Type`) are
resolved at runtime, not compile time. This means the type checker can't verify that a trait
method exists on a value at check time.

**What to document**: "Trait methods are dispatched dynamically. The type checker validates
trait impl blocks but doesn't resolve trait methods on arbitrary expressions."

### KL3. No Concurrency

Astra v1.0 is single-threaded. No async/await, no threads, no parallelism.

**What to document**: "Astra v1.0 is single-threaded. Concurrency is planned for a future version."

### KL4. Interpreted Only

All execution is via tree-walking interpreter. Performance is adequate for small/medium
programs but not suitable for compute-heavy workloads.

**What to document**: "Astra v1.0 is interpreted. For performance-critical code, consider
calling out to external tools via effects."

### KL5. No Package Manager / Registry

No way to install third-party packages. Projects use only the stdlib and their own code.

**What to document**: "Astra v1.0 has no package manager. Organize code as modules within
your project. A package system is planned for a future version."

### KL6. No Debugger

No step-through debugger. Debugging is done via `println`, `assert`, and test blocks.

**What to document**: "Use `println` for debugging output, `assert`/`assert_eq` for checks,
and `test` blocks for verifying behavior."

---

## v1.1+ Roadmap (Explicitly Deferred)

These are legitimate features that are OUT OF SCOPE for v1.0. They should not be worked
on until v1.0 is validated with real users.

### Future: Language Features
| Feature | Description | Why Deferred |
|---------|-------------|-------------|
| Full HM type inference | Constraint-based type solving | Current inference works for most cases; HM is a large engineering effort |
| True async/await | Event loop, futures, concurrent effects | Requires runtime redesign; v1.0 is single-threaded |
| Pattern matching: OR patterns | `Some(1) \| Some(2) => ...` | Nice syntax sugar; workaround: separate arms |
| Pattern matching: range patterns | `1..10 => ...` | Nice syntax sugar; workaround: guards |
| Default parameter values | `fn greet(name: Text = "world")` | Adds complexity to overload resolution |
| Named arguments | `greet(name = "Alice")` | May conflict with record literal syntax |
| Operator overloading | User-defined `+`, `-`, etc. | Complexity vs benefit tradeoff; can use named methods |
| Newtype / opaque types | `type UserId = Int` where UserId != Int | Currently type aliases are transparent; newtypes need coercion rules |
| Interface / structural typing | Types satisfy traits implicitly if they have the right methods | Requires full trait resolution; explicit impls are fine for v1.0 |

### Future: Tooling
| Feature | Description | Why Deferred |
|---------|-------------|-------------|
| WASM compilation target | Compile to WebAssembly | Requires a compiler backend; interpreter is sufficient for v1.0 |
| Package registry | Publish and install Astra packages | Needs protocol design, hosting, versioning; too early |
| Bytecode compiler | Compile to bytecode for faster execution | Interpreter performance is adequate for target use cases |
| Debugger (step-through) | `astra debug` with breakpoints | Large feature; println debugging is adequate for v1.0 |
| Performance profiler | `astra run --profile` | Useful but not essential for correctness validation |
| LSP: rename symbol | Rename across files | Requires cross-file symbol resolution (depends on B1) |
| LSP: find references | Find all usages of a symbol | Requires cross-file indexing |
| Incremental AST caching | Cache parsed ASTs for faster re-checking | Current cache stores diagnostics; full AST cache is optimization |

### Future: Standard Library
| Feature | Description | Why Deferred |
|---------|-------------|-------------|
| Full JSON parser | Parse JSON into typed Astra values | Complex feature (JsonValue enum, visitors) |
| Regular expressions | Regex matching and capture groups | Large feature with its own syntax |
| Date/time library | Date formatting, parsing, arithmetic | Complex domain; use Clock.now() for timestamps |
| HTTP client library | High-level HTTP client wrapper | ureq exists in Rust; Astra wrapper is convenience |
| File path utilities | Path joining, extension detection, normalization | Useful but can be done with string operations |
| Crypto / hashing | SHA256, HMAC, etc. | Security-critical; better to call external tools |
| CSV parsing | Read/write CSV files | Can be implemented in Astra once v1.0 is stable |

---

## Implementation Order Recommendation

For agents implementing these features, the recommended order is:

### Sprint 1: Foundation (B1-B5)
1. **B3**: Proper exit codes — small, verifiable, unblocks CI usage
2. **B5**: Runtime error source locations — verify/fix, likely mostly done
3. **B4**: Type checker false positives audit — systematic, prevents frustration
4. **B2**: Remove/error on async/await — small change, prevents confusion
5. **B1**: Cross-file type checking — largest item, do last in sprint

### Sprint 2: Language Completeness (E1-E10)
1. **E5**: Closure variable capture — verify, likely already works
2. **E6**: User-defined error types with Result — verify, likely already works
3. **E10**: Comparison operations for all types — verify structural equality
4. **E4**: Nested pattern matching — verify, may already work
5. **E3**: String concatenation — verify/document
6. **E9**: Multi-line strings — verify escape sequences
7. **E8**: For loop destructuring — likely small parser change
8. **E1**: Compound assignment operators — parser + interpreter
9. **E2**: Index access for lists — parser + interpreter
10. **E7**: Map/Set literal syntax — parser design decision needed

### Sprint 3: Polish (P1-P11)
- Prioritize P4 (test output) and P6 (JSON output) for LLM workflow
- Then P2 (error messages) and P7 (LSP cross-file)
- P8-P11 (stdlib additions) can be done incrementally

---

## Definition of Done: v1.0

Astra v1.0 is ready when ALL of the following are true:

1. **All Tier 1 (Blocker) items are complete** — B1 through B5
2. **All Tier 2 (Expected) items are complete or have a documented decision** — E1 through E10 (some may be deferred with rationale)
3. **All Tier 4 (Known Limitations) are documented** in README or getting-started guide
4. **All existing tests pass** (427+) with no regressions
5. **A "real project" example exists** — a non-trivial multi-file Astra project that demonstrates: imports, custom types, error handling, effects, tests, contracts
6. **An LLM can complete the check-fix-test loop** on the example project using only `astra check --json`, `astra fix`, and `astra test --json`
7. **Getting started guide is accurate** — a new user can follow `docs/getting-started.md` and have a working project

---

## Summary Statistics

| Category | Count | Status |
|----------|-------|--------|
| Tier 1: Blockers | 5 | Must complete |
| Tier 2: Expected | 10 | Must complete or decide |
| Tier 3: Polish | 11 | Should complete |
| Tier 4: Known Limitations | 6 | Must document |
| v1.1+ Deferred | 25+ | Explicitly out of scope |
