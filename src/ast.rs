#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Import(ImportDecl),
    Var(VarDecl),
    Func(FuncDecl),
    Family(FamilyDecl),
    Stmt(Stmt),
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub module: String,
    pub alias: Option<String>,
    /// Identity of the loaded package, filled in by the package loader.
    pub key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub is_fixed: bool,
    pub temp: TempKind,
    pub name: String,
    pub ty: Option<String>,
    pub value: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempKind {
    Default,
    Hot,
    Cold,
}

#[derive(Debug, Clone)]
pub struct FuncDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub body: FuncBody,
    /// Decorators written above the function, without `@` (`Decorator.private`).
    pub decorators: Vec<String>,
    /// Who may call a family method, from its decorators.
    pub access: Access,
}

/// Method access set by `@Decorator.private` / `@Decorator.subclass`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Access {
    #[default]
    Public,
    /// Only methods of the declaring family.
    Private,
    /// Methods of the declaring family and of families inheriting from it.
    Subclass,
}

#[derive(Debug, Clone)]
pub enum FuncBody {
    Block(Vec<Stmt>),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct Param {
    pub ty: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct FamilyDecl {
    pub name: String,
    pub extends: Option<String>,
    pub methods: Vec<FuncDecl>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Var(VarDecl),
    Assign {
        target: Expr,
        value: Expr,
    },
    Ret(Expr),
    Expr(Expr),
    If {
        cond: Expr,
        then_block: Vec<Stmt>,
        elif_blocks: Vec<(Expr, Vec<Stmt>)>,
        else_block: Option<Vec<Stmt>>,
    },
    Repeat {
        times: Expr,
        body: Vec<Stmt>,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    ForEach {
        name: String,
        iterable: Expr,
        body: Vec<Stmt>,
    },
    For {
        init: Vec<Stmt>, cond: Expr, step: Vec<Stmt>, body: Vec<Stmt>,
    },
    Break,
    Continue,
    /// `run{...} handle(...){...} ... then{...}`
    Run {
        body: Vec<Stmt>,
        handlers: Vec<Handler>,
        then_block: Option<Vec<Stmt>>,
    },
}

/// One `handle(...)` block: the error family names it catches and, for the
/// `handle(Type e)` / `handle([T1, T2] e)` forms, the name bound to the error.
#[derive(Debug, Clone)]
pub struct Handler {
    pub types: Vec<String>,
    pub name: Option<String>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Expr {
    /// Numeric literal source text; resolution lowers it to Int/Long/Float.
    Number(String),
    Int(i64),
    Long(i64),
    Float(f64),
    /// Decoded string contents (escapes already applied).
    String(String),
    Bool(bool),
    Null,
    Ident(String),
    /// Runtime check/conversion against a type annotation.
    Typed { value: Box<Expr>, ty: String },
    /// Recursively freeze the value of a fixed binding.
    Freeze(Box<Expr>),
    Unary { op: String, value: Box<Expr> },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Member {
        object: Box<Expr>,
        field: String,
    },
    Binary {
        left: Box<Expr>,
        op: String,
        right: Box<Expr>,
    },
}
