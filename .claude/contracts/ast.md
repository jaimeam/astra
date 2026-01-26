# AST Contract

> This document defines the stable interface for the Abstract Syntax Tree.
> All agents working with the AST must follow these conventions.

## Node Structure

Every AST node MUST include:

```rust
struct AstNode {
    /// Unique identifier for this node (stable across parses of same source)
    id: NodeId,

    /// Source location span
    span: Span,

    /// Node-specific data
    kind: NodeKind,
}

struct NodeId(u64);

struct Span {
    /// Source file path
    file: PathBuf,

    /// Start position (0-indexed byte offset)
    start: usize,

    /// End position (0-indexed byte offset, exclusive)
    end: usize,

    /// Start line (1-indexed)
    start_line: usize,

    /// Start column (1-indexed, UTF-8 characters)
    start_col: usize,

    /// End line (1-indexed)
    end_line: usize,

    /// End column (1-indexed, UTF-8 characters)
    end_col: usize,
}
```

## Node Kinds

### Module Level
```rust
enum ModuleItem {
    Import(ImportNode),
    TypeDef(TypeDefNode),
    EnumDef(EnumDefNode),
    FnDef(FnDefNode),
    Test(TestNode),
    Property(PropertyNode),
}
```

### Types
```rust
enum TypeExpr {
    Named { name: String, args: Vec<TypeExpr> },
    Record { fields: Vec<(String, TypeExpr)> },
    Function { params: Vec<TypeExpr>, ret: Box<TypeExpr>, effects: Vec<String> },
    Option { inner: Box<TypeExpr> },
    Result { ok: Box<TypeExpr>, err: Box<TypeExpr> },
}
```

### Expressions
```rust
enum Expr {
    // Literals
    IntLit(i64),
    BoolLit(bool),
    TextLit(String),
    UnitLit,

    // Identifiers
    Ident(String),
    QualifiedIdent { module: String, name: String },

    // Compound
    Record { fields: Vec<(String, Expr)> },
    FieldAccess { expr: Box<Expr>, field: String },
    EnumVariant { name: String, data: Option<Box<Expr>> },

    // Operations
    BinaryOp { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    UnaryOp { op: UnOp, expr: Box<Expr> },
    Call { func: Box<Expr>, args: Vec<Expr> },
    MethodCall { receiver: Box<Expr>, method: String, args: Vec<Expr> },

    // Control flow
    If { cond: Box<Expr>, then_branch: Box<Expr>, else_branch: Option<Box<Expr>> },
    Match { expr: Box<Expr>, arms: Vec<MatchArm> },
    Block(Vec<Stmt>),

    // Error handling
    Try { expr: Box<Expr>, else_branch: Box<Expr> },

    // Special
    Hole,  // ??? placeholder
}
```

### Statements
```rust
enum Stmt {
    Let { name: String, mutable: bool, type_ann: Option<TypeExpr>, value: Expr },
    Assign { target: Expr, value: Expr },
    Expr(Expr),
    Return(Option<Expr>),
}
```

### Patterns
```rust
enum Pattern {
    Wildcard,
    Ident(String),
    IntLit(i64),
    BoolLit(bool),
    TextLit(String),
    Record { fields: Vec<(String, Pattern)> },
    EnumVariant { name: String, data: Option<Box<Pattern>> },
}
```

## JSON Serialization

AST nodes serialize to JSON for golden tests. Format:

```json
{
  "id": 42,
  "span": {
    "file": "src/example.astra",
    "start": 100,
    "end": 150,
    "start_line": 5,
    "start_col": 1,
    "end_line": 5,
    "end_col": 51
  },
  "kind": {
    "type": "FnDef",
    "name": "add",
    "params": [...],
    "return_type": {...},
    "effects": ["Net"],
    "requires": [...],
    "ensures": [...],
    "body": {...}
  }
}
```

## Stability Requirements

1. **Node IDs**: Must be deterministic. Same source → same IDs.
2. **Spans**: Must accurately reflect source positions.
3. **Serialization**: Format must not change within a major version.
4. **Round-trip**: `serialize(parse(code))` must be stable.

## Traversal

Provide visitor pattern for AST traversal:

```rust
trait AstVisitor {
    fn visit_module(&mut self, node: &ModuleNode);
    fn visit_fn_def(&mut self, node: &FnDefNode);
    fn visit_expr(&mut self, node: &Expr);
    fn visit_stmt(&mut self, node: &Stmt);
    fn visit_pattern(&mut self, node: &Pattern);
    fn visit_type(&mut self, node: &TypeExpr);
}
```

## Comments

Comments are preserved in the AST for formatter:

```rust
struct Comment {
    kind: CommentKind,
    text: String,
    span: Span,
}

enum CommentKind {
    Line,   // # comment
    Doc,    // ## doc comment
}
```

Comments are associated with the nearest following AST node.
