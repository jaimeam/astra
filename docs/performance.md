# Astra Performance Characteristics

## Architecture

Astra uses a **tree-walking interpreter** written in Rust. Source code is parsed into an
AST and evaluated directly — there is no bytecode compilation step.

This architecture prioritizes:
- **Fast startup** — no compilation phase means programs run immediately
- **Predictable behavior** — no JIT warmup, no optimization surprises
- **Simple debugging** — errors map directly to source locations
- **Small binary** — the entire toolchain is a single executable

## Performance Profile

### What Astra is Good At

| Workload | Performance | Why |
|----------|-------------|-----|
| Script-sized programs (< 1000 LOC) | Excellent | Startup dominates; interpreter overhead is negligible |
| I/O-bound programs | Excellent | Bottleneck is I/O, not interpretation |
| Development iteration | Excellent | No compilation wait; instant feedback |
| Test suites | Excellent | 150+ tests run in < 1 second |
| Pattern matching | Good | Direct AST dispatch, no indirection |
| String processing | Good | Backed by Rust's String implementation |

### What Astra is Not Optimized For

| Workload | Performance | Recommendation |
|----------|-------------|----------------|
| CPU-bound number crunching | Moderate | Use built-in functions (backed by Rust) for hot loops |
| Large data processing (> 1M records) | Moderate | Consider streaming/batching patterns |
| Long-running servers | Good for moderate load | Adequate for development and internal tools |
| Real-time systems | Not suitable | Use a compiled language |

### Complexity Guarantees

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Map get/set/remove | O(log n) | Sorted vector with binary search |
| Map.from / construction | O(n log n) | Sort on construction |
| Set contains/add/remove | O(log n) | Sorted vector with binary search |
| Set.from / construction | O(n log n) | Sort + dedup on construction |
| List append (push) | O(n) | Immutable — creates new list |
| List get (index) | O(1) | Direct indexing |
| List map/filter/fold | O(n) | Single pass |
| Pattern matching | O(patterns) | Linear in number of match arms |
| Module loading | O(1) amortized | Cached after first load |
| String concatenation | O(n + m) | Creates new string |

### Tail Call Optimization

Astra optimizes **self-recursive tail calls**. Functions whose last expression is a
call to themselves are optimized to use constant stack space:

```astra
fn sum_to(n: Int, acc: Int) -> Int {
  if n <= 0 { acc }
  else { sum_to(n - 1, acc + n) }  ## Optimized: no stack growth
}

sum_to(1000000, 0)  ## Works without stack overflow
```

Mutual recursion and non-tail calls are not optimized and will use stack space
proportional to call depth.

## Benchmarking Your Code

Use the `Clock` effect to measure execution time:

```astra
fn benchmark(label: Text, f: () -> Unit) -> Unit effects(Console, Clock) {
  let start = Clock.now()
  f()
  let elapsed = Clock.now() - start
  Console.println("${label}: ${to_text(elapsed)}ms")
}
```

## Design Philosophy

Astra is designed for **correctness and developer experience** over raw speed:

1. **Immutable by default** — slightly slower than mutation, but eliminates entire
   categories of bugs
2. **Effect tracking** — small overhead for capability checks, but enables sandboxing
   and deterministic testing
3. **Contracts checked at runtime** — requires/ensures add overhead, but catch bugs
   that types alone cannot
4. **Tree-walking interpreter** — simpler than a VM, easier to debug, adequate for
   the intended use cases

If you need C-level performance for a specific operation, the recommended approach
is to implement it as a built-in function in Rust (see the contributor guide).
