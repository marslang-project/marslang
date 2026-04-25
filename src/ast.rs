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
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(String),
    String(String),
    Bool(bool),
    Null,
    Ident(String),
    Raw(String),
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
