use crate::error::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Void,
    Bool,
    Char,
    Int,
    Short,
    Long,
    Float,
    Double,
    UnsignedInt,
    UnsignedChar,
    UnsignedShort,
    UnsignedLong,
    Pointer(Box<Type>),
    Array(Box<Type>, usize),
    Struct(String),
    Union(String),
    Enum(String),
    Nullptr,
    Auto,
    TypeofExpression(Box<Expr>),
    TypeofType(Box<Type>),
    Atomic(Box<Type>),
}

impl Type {
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::Bool
                | Type::Char
                | Type::Int
                | Type::Short
                | Type::Long
                | Type::UnsignedInt
                | Type::UnsignedChar
                | Type::UnsignedShort
                | Type::UnsignedLong
        )
    }

    pub fn is_floating(&self) -> bool {
        matches!(self, Type::Float | Type::Double)
    }

    pub fn is_numeric(&self) -> bool {
        self.is_integer() || self.is_floating()
    }

    pub fn is_pointer(&self) -> bool {
        matches!(self, Type::Pointer(_) | Type::Nullptr)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Shl,
    Shr,
    BitAnd,
    BitOr,
    BitXor,
    LogicalAnd,
    LogicalOr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,     // -x
    Not,     // !x
    BitNot,  // ~x
    Deref,   // *x
    AddrOf,  // &x
    PreInc,  // ++x
    PreDec,  // --x
    PostInc, // x++
    PostDec, // x--
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Char(char),
    String(String),
    Nullptr,
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub node: ExprNode,
    pub span: Span,
    pub ty: Option<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprNode {
    Literal(Literal),
    Identifier(String),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Cast(Type, Box<Expr>),
    Member(Box<Expr>, String, bool), // expr.member (false) or expr->member (true)
    SizeofExpr(Box<Expr>),
    SizeofType(Type),
    AlignofExpr(Box<Expr>),
    AlignofType(Type),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub node: StmtNode,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtNode {
    Compound(Vec<Stmt>),
    Expr(Expr),
    Decl(Type, String, Option<Expr>), // type, name, optional initializer
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    For(Option<Box<Stmt>>, Option<Expr>, Option<Expr>, Box<Stmt>),
    Switch(Expr, Box<Stmt>),
    Case(Expr, Box<Stmt>),
    Default(Box<Stmt>),
    Break,
    Continue,
    Return(Option<Expr>),
    Unsafe(Box<Stmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<Field>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub is_variadic: bool,
    pub body: Option<Stmt>, // None for prototype declarations
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GlobalDecl {
    Function(FunctionDecl),
    Struct(StructDecl),
    GlobalVar(Type, String, Option<Expr>, Span),
}

#[derive(Debug, Clone)]
pub struct Program {
    pub decls: Vec<GlobalDecl>,
}
