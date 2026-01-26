# Pattern: AST Traversal

## Visitor Pattern

```rust
pub trait Visitor {
    fn visit_module(&mut self, module: &Module) {
        walk_module(self, module);
    }

    fn visit_item(&mut self, item: &Item) {
        walk_item(self, item);
    }

    fn visit_fn_def(&mut self, fn_def: &FnDef) {
        walk_fn_def(self, fn_def);
    }

    fn visit_expr(&mut self, expr: &Expr) {
        walk_expr(self, expr);
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_pattern(&mut self, pattern: &Pattern) {
        walk_pattern(self, pattern);
    }

    fn visit_type(&mut self, ty: &TypeExpr) {
        walk_type(self, ty);
    }
}

pub fn walk_module<V: Visitor + ?Sized>(v: &mut V, module: &Module) {
    for item in &module.items {
        v.visit_item(item);
    }
}

pub fn walk_expr<V: Visitor + ?Sized>(v: &mut V, expr: &Expr) {
    match &expr.kind {
        ExprKind::Binary { left, right, .. } => {
            v.visit_expr(left);
            v.visit_expr(right);
        }
        ExprKind::Call { func, args } => {
            v.visit_expr(func);
            for arg in args {
                v.visit_expr(arg);
            }
        }
        ExprKind::If { cond, then_branch, else_branch } => {
            v.visit_expr(cond);
            v.visit_expr(then_branch);
            if let Some(else_br) = else_branch {
                v.visit_expr(else_br);
            }
        }
        // ... other cases
        _ => {}
    }
}
```

## Mutable Visitor (Transformer)

```rust
pub trait MutVisitor {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        walk_expr_mut(self, expr);
    }

    fn visit_stmt_mut(&mut self, stmt: &mut Stmt) {
        walk_stmt_mut(self, stmt);
    }
}

pub fn walk_expr_mut<V: MutVisitor + ?Sized>(v: &mut V, expr: &mut Expr) {
    match &mut expr.kind {
        ExprKind::Binary { left, right, .. } => {
            v.visit_expr_mut(left);
            v.visit_expr_mut(right);
        }
        // ... other cases
        _ => {}
    }
}
```

## Fold Pattern (Transforming AST)

```rust
pub trait Folder {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        fold_expr(self, expr)
    }

    fn fold_stmt(&mut self, stmt: Stmt) -> Stmt {
        fold_stmt(self, stmt)
    }
}

pub fn fold_expr<F: Folder + ?Sized>(f: &mut F, expr: Expr) -> Expr {
    let kind = match expr.kind {
        ExprKind::Binary { op, left, right } => ExprKind::Binary {
            op,
            left: Box::new(f.fold_expr(*left)),
            right: Box::new(f.fold_expr(*right)),
        },
        // ... other cases
        other => other,
    };
    Expr { kind, ..expr }
}

// Example: Constant folding
struct ConstantFolder;

impl Folder for ConstantFolder {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        let expr = fold_expr(self, expr);  // Fold children first

        match &expr.kind {
            ExprKind::Binary { op: BinOp::Add, left, right } => {
                if let (ExprKind::IntLit(a), ExprKind::IntLit(b)) =
                    (&left.kind, &right.kind)
                {
                    return Expr {
                        kind: ExprKind::IntLit(a + b),
                        span: expr.span,
                        id: expr.id,
                    };
                }
            }
            _ => {}
        }
        expr
    }
}
```

## Query Pattern (Finding Nodes)

```rust
pub fn find_all<P>(ast: &Ast, predicate: P) -> Vec<&Expr>
where
    P: Fn(&Expr) -> bool,
{
    struct Finder<'a, P> {
        predicate: P,
        results: Vec<&'a Expr>,
    }

    impl<'a, P: Fn(&Expr) -> bool> Visitor for Finder<'a, P> {
        fn visit_expr(&mut self, expr: &Expr) {
            if (self.predicate)(expr) {
                self.results.push(expr);
            }
            walk_expr(self, expr);
        }
    }

    let mut finder = Finder {
        predicate,
        results: Vec::new(),
    };
    finder.visit_module(&ast.module);
    finder.results
}

// Usage
let holes = find_all(&ast, |e| matches!(e.kind, ExprKind::Hole));
```

## Span Preservation

Always preserve spans when transforming:

```rust
impl Folder for MyTransform {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        let span = expr.span.clone();  // Preserve original span
        let id = expr.id;

        let new_kind = match expr.kind {
            // Transform...
        };

        Expr { kind: new_kind, span, id }
    }
}
```
