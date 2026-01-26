//! Abstract Syntax Tree definitions for Astra
//!
//! All AST nodes include:
//! - Unique node ID
//! - Source span
//! - Node-specific data

use crate::diagnostics::Span;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for AST nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl NodeId {
    /// Generate a new unique node ID
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

/// A complete Astra module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: NodeId,
    pub span: Span,
    pub name: ModulePath,
    pub items: Vec<Item>,
}

/// A module path (e.g., `foo.bar.baz`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulePath {
    pub id: NodeId,
    pub span: Span,
    pub segments: Vec<String>,
}

/// Top-level items in a module
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Item {
    Import(ImportDecl),
    TypeDef(TypeDef),
    EnumDef(EnumDef),
    FnDef(FnDef),
    Test(TestBlock),
    Property(PropertyBlock),
}

/// Import declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDecl {
    pub id: NodeId,
    pub span: Span,
    pub path: ModulePath,
    pub kind: ImportKind,
}

/// Kind of import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImportKind {
    /// `import foo.bar`
    Module,
    /// `import foo.bar as Baz`
    Alias(String),
    /// `import foo.bar.{A, B, C}`
    Items(Vec<String>),
}

/// Type definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDef {
    pub id: NodeId,
    pub span: Span,
    pub name: String,
    pub type_params: Vec<String>,
    pub value: TypeExpr,
    pub invariant: Option<Box<Expr>>,
}

/// Enum definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumDef {
    pub id: NodeId,
    pub span: Span,
    pub name: String,
    pub type_params: Vec<String>,
    pub variants: Vec<Variant>,
}

/// Enum variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    pub id: NodeId,
    pub span: Span,
    pub name: String,
    pub fields: Vec<Field>,
}

/// A field in a record or variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub id: NodeId,
    pub span: Span,
    pub name: String,
    pub ty: TypeExpr,
}

/// Function definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FnDef {
    pub id: NodeId,
    pub span: Span,
    pub visibility: Visibility,
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub effects: Vec<String>,
    pub requires: Vec<Expr>,
    pub ensures: Vec<Expr>,
    pub body: Block,
}

/// Visibility modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Private,
    Public,
}

/// Function parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Param {
    pub id: NodeId,
    pub span: Span,
    pub name: String,
    pub ty: TypeExpr,
}

/// Test block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestBlock {
    pub id: NodeId,
    pub span: Span,
    pub name: String,
    pub using: Option<UsingClause>,
    pub body: Block,
}

/// Property test block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyBlock {
    pub id: NodeId,
    pub span: Span,
    pub name: String,
    pub using: Option<UsingClause>,
    pub body: Block,
}

/// Using clause for capability injection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsingClause {
    pub id: NodeId,
    pub span: Span,
    pub bindings: Vec<EffectBinding>,
}

/// Effect binding in using clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectBinding {
    pub id: NodeId,
    pub span: Span,
    pub effect: String,
    pub value: Box<Expr>,
}

/// Type expression
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TypeExpr {
    /// Named type (e.g., `Int`, `Option[T]`)
    Named {
        id: NodeId,
        span: Span,
        name: String,
        args: Vec<TypeExpr>,
    },
    /// Record type (e.g., `{ x: Int, y: Int }`)
    Record {
        id: NodeId,
        span: Span,
        fields: Vec<Field>,
    },
    /// Function type (e.g., `(Int, Int) -> Int effects(Net)`)
    Function {
        id: NodeId,
        span: Span,
        params: Vec<TypeExpr>,
        ret: Box<TypeExpr>,
        effects: Vec<String>,
    },
}

/// A block of statements with optional trailing expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: NodeId,
    pub span: Span,
    pub stmts: Vec<Stmt>,
    pub expr: Option<Box<Expr>>,
}

/// Statement
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Stmt {
    /// Let binding
    Let {
        id: NodeId,
        span: Span,
        name: String,
        mutable: bool,
        ty: Option<TypeExpr>,
        value: Box<Expr>,
    },
    /// Assignment
    Assign {
        id: NodeId,
        span: Span,
        target: Box<Expr>,
        value: Box<Expr>,
    },
    /// Expression statement
    Expr {
        id: NodeId,
        span: Span,
        expr: Box<Expr>,
    },
    /// Return statement
    Return {
        id: NodeId,
        span: Span,
        value: Option<Box<Expr>>,
    },
}

/// Expression
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Expr {
    // Literals
    IntLit {
        id: NodeId,
        span: Span,
        value: i64,
    },
    BoolLit {
        id: NodeId,
        span: Span,
        value: bool,
    },
    TextLit {
        id: NodeId,
        span: Span,
        value: String,
    },
    UnitLit {
        id: NodeId,
        span: Span,
    },

    // Identifiers
    Ident {
        id: NodeId,
        span: Span,
        name: String,
    },
    QualifiedIdent {
        id: NodeId,
        span: Span,
        module: String,
        name: String,
    },

    // Compound
    Record {
        id: NodeId,
        span: Span,
        fields: Vec<(String, Box<Expr>)>,
    },
    FieldAccess {
        id: NodeId,
        span: Span,
        expr: Box<Expr>,
        field: String,
    },

    // Operations
    Binary {
        id: NodeId,
        span: Span,
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        id: NodeId,
        span: Span,
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Call {
        id: NodeId,
        span: Span,
        func: Box<Expr>,
        args: Vec<Expr>,
    },
    MethodCall {
        id: NodeId,
        span: Span,
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },

    // Control flow
    If {
        id: NodeId,
        span: Span,
        cond: Box<Expr>,
        then_branch: Box<Block>,
        else_branch: Option<Box<Expr>>,
    },
    Match {
        id: NodeId,
        span: Span,
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Block {
        id: NodeId,
        span: Span,
        block: Box<Block>,
    },

    // Error handling
    Try {
        id: NodeId,
        span: Span,
        expr: Box<Expr>,
    },
    TryElse {
        id: NodeId,
        span: Span,
        expr: Box<Expr>,
        else_expr: Box<Expr>,
    },

    // Special
    Hole {
        id: NodeId,
        span: Span,
    },
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Logical
    And,
    Or,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

/// Match arm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchArm {
    pub id: NodeId,
    pub span: Span,
    pub pattern: Pattern,
    pub body: Box<Expr>,
}

/// Pattern for matching
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Pattern {
    Wildcard {
        id: NodeId,
        span: Span,
    },
    Ident {
        id: NodeId,
        span: Span,
        name: String,
    },
    IntLit {
        id: NodeId,
        span: Span,
        value: i64,
    },
    BoolLit {
        id: NodeId,
        span: Span,
        value: bool,
    },
    TextLit {
        id: NodeId,
        span: Span,
        value: String,
    },
    Record {
        id: NodeId,
        span: Span,
        fields: Vec<(String, Pattern)>,
    },
    Variant {
        id: NodeId,
        span: Span,
        name: String,
        data: Option<Box<Pattern>>,
    },
}

/// Comment (preserved for formatter)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub span: Span,
    pub kind: CommentKind,
    pub text: String,
}

/// Kind of comment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommentKind {
    /// Regular line comment: `# ...`
    Line,
    /// Documentation comment: `## ...`
    Doc,
}
