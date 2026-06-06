//! Tokens produced by the Vader lexer.

/// The kind of a lexical token. Literals carry their parsed value.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),

    // Keywords
    Fn,
    Return,
    If,
    Else,
    For,
    In,
    Struct,
    Interface,
    Enum,
    Match,
    Import,
    Const,
    Test,
    Assert,
    Spawn,
    Public,
    Private,
    True,
    False,
    Nil,

    // Punctuation
    LParen,    // (
    RParen,    // )
    LBrace,    // {
    RBrace,    // }
    LBracket,  // [
    RBracket,  // ]
    Comma,     // ,
    Colon,     // :
    Dot,       // .
    Arrow,     // <-

    // Operators
    Assign,    // =
    Eq,        // ==
    NotEq,     // !=
    Lt,        // <
    LtEq,      // <=
    Gt,        // >
    GtEq,      // >=
    Plus,      // +
    Minus,     // -
    Star,      // *
    Slash,     // /
    Percent,   // %
    And,       // &&
    Or,        // ||
    Not,       // !
    DotDot,    // ..  (range exclusivo)
    DotDotEq,  // ..= (range inclusivo)

    /// End of input.
    Eof,
}

impl TokenKind {
    /// Maps a word to its keyword kind, if it is a reserved keyword.
    /// Type names (`int`, `string`, ...) are intentionally NOT keywords —
    /// they are identifiers resolved later by the type checker.
    pub fn keyword(word: &str) -> Option<TokenKind> {
        let kw = match word {
            "fn" => TokenKind::Fn,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "struct" => TokenKind::Struct,
            "interface" => TokenKind::Interface,
            "enum" => TokenKind::Enum,
            "match" => TokenKind::Match,
            "import" => TokenKind::Import,
            "const" => TokenKind::Const,
            "test" => TokenKind::Test,
            "assert" => TokenKind::Assert,
            "spawn" => TokenKind::Spawn,
            "public" => TokenKind::Public,
            "private" => TokenKind::Private,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "nil" => TokenKind::Nil,
            _ => return None,
        };
        Some(kw)
    }
}

/// A token with its source position (1-based line and column).
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

impl Token {
    pub fn new(kind: TokenKind, line: usize, col: usize) -> Self {
        Token { kind, line, col }
    }
}
