//! Abstract syntax tree for Vader.
//!
//! Phase 1: functions, methods, structs, interfaces, enums, generics, match,
//! imports, statements and expressions.

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub imports: Vec<String>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Function(Function),
    Struct(StructDef),
    Interface(InterfaceDef),
    Enum(EnumDef),
    Test(TestDef),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestDef {
    pub name: String,
    pub body: Block,
}

/// Visibility of a symbol. The default (no modifier) is `Private`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

/// A generic type parameter: `T` or `T Constraint`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    pub name: String,
    pub constraint: Option<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub visibility: Visibility,
    /// `Some` for methods: `fn (u User) greeting() ...`
    pub receiver: Option<Param>,
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    /// empty = no return; 1 = single; many = tuple `(int, error)`
    pub returns: Vec<Type>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    pub visibility: Visibility,
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<Param>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceDef {
    pub visibility: Visibility,
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub methods: Vec<MethodSig>,
}

/// A method signature in an interface (no body).
#[derive(Debug, Clone, PartialEq)]
pub struct MethodSig {
    pub name: String,
    pub params: Vec<Param>,
    pub returns: Vec<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    pub visibility: Visibility,
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    /// Optional fields that the variant carries: `Circle(radius float)`.
    pub fields: Vec<Param>,
}

/// A name with its type — used for parameters, fields and declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Named(String),               // int, string, User, T
    Generic(String, Vec<Type>),  // Foo[T, U], chan[int]
    Slice(Box<Type>),            // []T
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `[const] Type name {, Type name} = expr {, expr}`
    VarDecl {
        is_const: bool,
        decls: Vec<Param>,
        values: Vec<Expr>,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    Return(Vec<Expr>),
    If {
        cond: Expr,
        then_block: Block,
        else_block: Option<Block>,
    },
    For {
        head: ForHead,
        body: Block,
    },
    /// `spawn call(...)`
    Spawn(Expr),
    /// `chan <- value`
    Send {
        chan: Expr,
        value: Expr,
    },
    /// `assert <expr>` (inside `test` blocks)
    Assert(Expr),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForHead {
    Infinite,                          // for { }
    While(Expr),                       // for cond { }
    In { var: String, iter: Expr },    // for x in expr { }
}

/// An expression with the position (line:column) where it starts — used in errors.
/// The position is ignored in comparison (`PartialEq`), so `fmt`/round-trip
/// compare only the structure, not where the code sits in the file.
#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub line: usize,
    pub col: usize,
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Nil,
    Ident(String),
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Field {
        base: Box<Expr>,
        field: String,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    StructLit {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    SliceLit(Vec<Expr>),
    /// `<-ch` (receives from a channel)
    Recv(Box<Expr>),
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// One or more alternatives: `400, 404:`
    pub patterns: Vec<Pattern>,
    pub guard: Option<Expr>,
    pub body: MatchArmBody,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchArmBody {
    Expr(Expr),
    Block(Block),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard,                                        // _
    Literal(Expr),                                   // 200, "x", true, nil
    Binding(String),                                 // n  (or a nullary variant, resolved in the checker)
    Variant { name: String, bindings: Vec<String> }, // Circle(r), Rectangle(w, h)
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnOp {
    Neg, // -x
    Not, // !x
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Or,
    And,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Range,     // a..b
    RangeIncl, // a..=b
}
