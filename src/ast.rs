#[derive(Clone, Debug)]
pub struct Program {
    /// Library names requested via `include <name>` at the top level.
    /// Resolved by the interpreter before `main` is called.
    pub includes: Vec<String>,
    pub globals: Vec<GlobalDecl>,
    pub functions: Vec<Function>,
}

#[derive(Clone, Debug)]
pub struct GlobalDecl {
    pub name: String,
    pub vector_size: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct LocalDecl {
    pub name: String,
    pub vector_size: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub locals: Vec<LocalDecl>,
    pub body: Stmt,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Compound(Vec<Stmt>),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    Switch(Expr, Box<Stmt>),
    Break,
    Return(Option<Expr>),
    Goto(String),
    Expr(Expr),
    Label(String, Box<Stmt>),
    Case(i64, Box<Stmt>),
    Default(Box<Stmt>),
}

#[derive(Clone, Debug)]
pub enum Expr {
    Constant(i64),
    CharConst(i64),
    StringLit(String),
    Var(String),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    AssignOp(BinaryOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Subscript(Box<Expr>, Box<Expr>),
    AddressOf(Box<Expr>),
    Indir(Box<Expr>),
    Increment(Box<Expr>, bool),
    Decrement(Box<Expr>, bool),
}

#[derive(Clone, Copy, Debug)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    LShift,
    RShift,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    BitAnd,
    BitXor,
    BitOr,
    And,
    Or,
}

#[derive(Clone, Copy, Debug)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
}
