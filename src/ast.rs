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
    /// Source line of the `takepkg`, so the loader can say where a bad import is.
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub is_fixed: bool,
    pub temp: TempKind,
    pub name: String,
    pub ty: Option<String>,
    pub value: Expr,
    /// Source line, for a declaration at the top level of a file, where it is
    /// an item rather than a statement; 0 inside a block, where the statement
    /// carries the line.
    pub line: usize,
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
    /// Decorators written above the function.
    pub decorators: Vec<Decorator>,
    /// Who may call a family method, from its decorators.
    pub access: Access,
    /// Documentation from `@Decorator.docstring(...)`, with indentation removed.
    pub doc: Option<String>,
    /// Source lines of the header and of the last line of the body.
    pub line: usize,
    pub end_line: usize,
}

/// `@Alias.name` or `@Alias.name(argument)` above a func or family.
#[derive(Debug, Clone)]
pub struct Decorator {
    /// Without `@`: `Decorator.private`.
    pub name: String,
    pub arg: Option<Expr>,
}

/// Method access set by `@Decorator.private` / `@Decorator.subclass`. On a
/// function declared outside a family, `Private` means the package does not
/// export it; there is nothing to restrict at runtime.
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
    pub decorators: Vec<Decorator>,
    /// `@Decorator.private`: the package keeps this family to itself.
    pub private: bool,
    /// Documentation from `@Decorator.docstring(...)`, with indentation removed.
    pub doc: Option<String>,
    /// Source lines of the header and of the closing brace.
    pub line: usize,
    pub end_line: usize,
}

/// A statement and the source line it starts on, which runtime errors report.
#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
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
    /// A `func` declared inside a block: a closure bound to `name`, a local.
    Func {
        name: String,
        decl: std::sync::Arc<FuncDecl>,
    },
    /// `run{...} handle(...){...} ... then{...}`
    Run {
        body: Vec<Stmt>,
        handlers: Vec<Handler>,
        then_block: Option<Vec<Stmt>>,
    },
    /// `match (subject){ pattern => statement; ... }`: the first arm that
    /// matches runs; when none does, nothing happens.
    Match {
        subject: Expr,
        arms: Vec<MatchArm<Vec<Stmt>>>,
    },
}

/// One arm of a `match`: its pattern, an optional `if (guard)`, and its body,
/// which is statements in the statement form and an expression in the
/// expression form.
#[derive(Debug, Clone)]
pub struct MatchArm<B> {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: B,
    /// Source line of the arm, for errors; 0 when the whole match was written
    /// inside one expression, where the enclosing statement's line holds.
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    /// `1`, `"quit"`, `2, 3`, `null`: equal (`==`) to one of the literals.
    Literals(Vec<Expr>),
    /// `_`: anything.
    Wildcard,
    /// `int n`, `Circle c`, `[int, float] x`, `string`: a value of one of the
    /// types, bound to `name` for the arm when one is given.
    Type { types: Vec<String>, name: Option<String> },
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
    /// Numeric literal source text; resolution lowers it to Int/Long/Big/Float.
    Number(String),
    Int(i64),
    Long(i64),
    /// A whole-number literal beyond longint.
    Big(num_bigint::BigInt),
    Float(f64),
    /// Decoded string contents (escapes already applied).
    String(String),
    /// `f"Hello {name}"`: its text pieces, as `String`s, and the expressions
    /// between braces, in order; each becomes text as `string(value)` does.
    Template(Vec<Expr>),
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
    /// `object[index]`: one character of a string, or one element of an array.
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    /// `func(int x) => x + 1`: an anonymous function, closing over the
    /// variables around it.
    Lambda(std::sync::Arc<FuncDecl>),
    /// `match (subject){ pattern => value; ... }`: the value of the first arm
    /// that matches; a `RangeError` when none does.
    Match {
        subject: Box<Expr>,
        arms: Vec<MatchArm<Expr>>,
    },
    Binary {
        left: Box<Expr>,
        op: String,
        right: Box<Expr>,
    },
}
