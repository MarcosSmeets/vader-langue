//! Abstract syntax tree for Vader.
//!
//! Fase 1: funções, métodos, structs, interfaces, enums, genéricos, match,
//! imports, statements e expressões.

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

/// Visibilidade de um símbolo. O padrão (sem modificador) é `Private`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

/// Um parâmetro de tipo genérico: `T` ou `T Constraint`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    pub name: String,
    pub constraint: Option<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub visibility: Visibility,
    /// `Some` para métodos: `fn (u User) greeting() ...`
    pub receiver: Option<Param>,
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    /// vazio = sem retorno; 1 = simples; vários = tupla `(int, error)`
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

/// Assinatura de método numa interface (sem corpo).
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
    /// Campos opcionais que a variante carrega: `Circle(radius float)`.
    pub fields: Vec<Param>,
}

/// Um nome com seu tipo — serve para parâmetros, campos e declarações.
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
    /// `assert <expr>` (dentro de blocos `test`)
    Assert(Expr),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForHead {
    Infinite,                          // for { }
    While(Expr),                       // for cond { }
    In { var: String, iter: Expr },    // for x in expr { }
}

/// Uma expressão com a posição (linha:coluna) onde começa — usada nos erros.
/// A posição é ignorada na comparação (`PartialEq`), pra `fmt`/round-trip
/// compararem só a estrutura, não onde o código está no arquivo.
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
    /// `<-ch` (recebe de um canal)
    Recv(Box<Expr>),
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// Uma ou mais alternativas: `400, 404:`
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
    Binding(String),                                 // n  (ou variante nullary, resolvido no checker)
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
