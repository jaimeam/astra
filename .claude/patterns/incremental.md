# Pattern: Incremental Compilation

## File-Level Caching

```rust
use std::collections::HashMap;
use std::path::PathBuf;

pub struct IncrementalCache {
    /// File content hash -> parsed AST
    parse_cache: HashMap<u64, Ast>,

    /// File content hash -> type check results
    typecheck_cache: HashMap<u64, TypedAst>,

    /// File modification times
    mtimes: HashMap<PathBuf, SystemTime>,
}

impl IncrementalCache {
    pub fn get_or_parse(&mut self, path: &Path, content: &str) -> &Ast {
        let hash = hash_content(content);

        self.parse_cache.entry(hash).or_insert_with(|| {
            parse(content)
        })
    }

    pub fn invalidate(&mut self, path: &Path) {
        // Remove cached data for changed file
        self.mtimes.remove(path);
        // Note: We use content hash, so cache entries auto-invalidate
    }

    pub fn is_fresh(&self, path: &Path) -> bool {
        if let (Some(&cached_mtime), Ok(current_mtime)) = (
            self.mtimes.get(path),
            std::fs::metadata(path).and_then(|m| m.modified())
        ) {
            cached_mtime == current_mtime
        } else {
            false
        }
    }
}
```

## Dependency Tracking

```rust
pub struct DependencyGraph {
    /// Module -> modules it imports
    imports: HashMap<ModuleId, HashSet<ModuleId>>,

    /// Module -> modules that import it
    dependents: HashMap<ModuleId, HashSet<ModuleId>>,
}

impl DependencyGraph {
    pub fn add_import(&mut self, from: ModuleId, to: ModuleId) {
        self.imports.entry(from).or_default().insert(to);
        self.dependents.entry(to).or_default().insert(from);
    }

    /// Get all modules that need rechecking when `module` changes
    pub fn affected_by(&self, module: ModuleId) -> HashSet<ModuleId> {
        let mut affected = HashSet::new();
        let mut queue = vec![module];

        while let Some(m) = queue.pop() {
            if affected.insert(m) {
                if let Some(deps) = self.dependents.get(&m) {
                    queue.extend(deps.iter().copied());
                }
            }
        }

        affected
    }
}
```

## Salsa-Style Queries (Future)

```rust
// Conceptual - for future implementation
#[salsa::query_group(CompilerDatabase)]
pub trait Compiler {
    #[salsa::input]
    fn source(&self, path: PathBuf) -> String;

    fn parse(&self, path: PathBuf) -> Ast;
    fn typecheck(&self, path: PathBuf) -> TypedAst;
    fn diagnostics(&self, path: PathBuf) -> Vec<Diagnostic>;
}

fn parse(db: &dyn Compiler, path: PathBuf) -> Ast {
    let source = db.source(path);
    parser::parse(&source)
}

fn typecheck(db: &dyn Compiler, path: PathBuf) -> TypedAst {
    let ast = db.parse(path);
    typechecker::check(&ast)
}
```

## Cache Persistence

```rust
impl IncrementalCache {
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let data = bincode::serialize(self).unwrap();
        std::fs::write(path, data)
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        let data = std::fs::read(path)?;
        Ok(bincode::deserialize(&data).unwrap_or_default())
    }
}

// Cache location: .astra/cache/
```

## Invalidation Strategy

1. **Content-based**: Hash file contents, not paths
2. **Dependency-aware**: Invalidate dependents when module changes
3. **Version-aware**: Invalidate all on compiler version change
4. **Graceful degradation**: Missing cache = full rebuild
