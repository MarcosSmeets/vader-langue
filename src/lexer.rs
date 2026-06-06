//! The Vader lexer: turns source text into a stream of tokens.

use crate::token::{Token, TokenKind};

/// An error found while lexing, with its source position.
#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lex error at {}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for LexError {}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    /// Tokenizes the whole input, always ending with an `Eof` token.
    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if let Some(ch) = c {
            self.pos += 1;
            if ch == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        c
    }

    /// Skips whitespace, line comments (`//`) and block comments (`/* */`).
    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.advance();
                }
                Some('/') if self.peek_at(1) == Some('/') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                Some('/') if self.peek_at(1) == Some('*') => {
                    let (line, col) = (self.line, self.col);
                    self.advance(); // /
                    self.advance(); // *
                    loop {
                        match self.peek() {
                            Some('*') if self.peek_at(1) == Some('/') => {
                                self.advance();
                                self.advance();
                                break;
                            }
                            None => {
                                return Err(LexError {
                                    message: "unterminated block comment".to_string(),
                                    line,
                                    col,
                                });
                            }
                            _ => {
                                self.advance();
                            }
                        }
                    }
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_trivia()?;
        let (line, col) = (self.line, self.col);

        let c = match self.peek() {
            Some(c) => c,
            None => return Ok(Token::new(TokenKind::Eof, line, col)),
        };

        if c.is_ascii_digit() {
            return self.lex_number(line, col);
        }
        if c == '_' || c.is_alphabetic() {
            return Ok(self.lex_ident(line, col));
        }
        if c == '"' {
            return self.lex_string(line, col);
        }
        self.lex_symbol(line, col)
    }

    fn lex_ident(&mut self, line: usize, col: usize) -> Token {
        let mut word = String::new();
        while let Some(c) = self.peek() {
            if c == '_' || c.is_alphanumeric() {
                word.push(c);
                self.advance();
            } else {
                break;
            }
        }
        let kind = TokenKind::keyword(&word).unwrap_or(TokenKind::Ident(word));
        Token::new(kind, line, col)
    }

    fn lex_number(&mut self, line: usize, col: usize) -> Result<Token, LexError> {
        let mut text = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                text.push(c);
                self.advance();
            } else {
                break;
            }
        }
        // A '.' is a decimal point only when followed by another digit;
        // otherwise it belongs to a range operator (e.g. `0..10`).
        let is_float =
            self.peek() == Some('.') && self.peek_at(1).map_or(false, |c| c.is_ascii_digit());
        if is_float {
            text.push('.');
            self.advance(); // .
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    text.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
            let value: f64 = text.parse().map_err(|_| LexError {
                message: format!("invalid float literal `{}`", text),
                line,
                col,
            })?;
            return Ok(Token::new(TokenKind::Float(value), line, col));
        }
        let value: i64 = text.parse().map_err(|_| LexError {
            message: format!("invalid integer literal `{}`", text),
            line,
            col,
        })?;
        Ok(Token::new(TokenKind::Int(value), line, col))
    }

    fn lex_string(&mut self, line: usize, col: usize) -> Result<Token, LexError> {
        self.advance(); // opening quote
        let mut value = String::new();
        loop {
            match self.advance() {
                Some('"') => return Ok(Token::new(TokenKind::Str(value), line, col)),
                Some('\\') => {
                    let esc = self.advance().ok_or(LexError {
                        message: "unterminated string literal".to_string(),
                        line,
                        col,
                    })?;
                    match esc {
                        'n' => value.push('\n'),
                        't' => value.push('\t'),
                        'r' => value.push('\r'),
                        '\\' => value.push('\\'),
                        '"' => value.push('"'),
                        '0' => value.push('\0'),
                        other => {
                            return Err(LexError {
                                message: format!("unknown escape `\\{}`", other),
                                line,
                                col,
                            });
                        }
                    }
                }
                Some('\n') | None => {
                    return Err(LexError {
                        message: "unterminated string literal".to_string(),
                        line,
                        col,
                    });
                }
                Some(c) => value.push(c),
            }
        }
    }

    fn lex_symbol(&mut self, line: usize, col: usize) -> Result<Token, LexError> {
        let c = self.advance().unwrap();
        let kind = match c {
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            ':' => TokenKind::Colon,
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '.' => {
                if self.peek() == Some('.') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::DotDotEq
                    } else {
                        TokenKind::DotDot
                    }
                } else {
                    TokenKind::Dot
                }
            }
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::Eq
                } else {
                    TokenKind::Assign
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::NotEq
                } else {
                    TokenKind::Not
                }
            }
            '<' => match self.peek() {
                Some('-') => {
                    self.advance();
                    TokenKind::Arrow
                }
                Some('=') => {
                    self.advance();
                    TokenKind::LtEq
                }
                _ => TokenKind::Lt,
            },
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }
            '&' => {
                if self.peek() == Some('&') {
                    self.advance();
                    TokenKind::And
                } else {
                    return Err(LexError {
                        message: "unexpected `&` (did you mean `&&`?)".to_string(),
                        line,
                        col,
                    });
                }
            }
            '|' => {
                if self.peek() == Some('|') {
                    self.advance();
                    TokenKind::Or
                } else {
                    return Err(LexError {
                        message: "unexpected `|` (did you mean `||`?)".to_string(),
                        line,
                        col,
                    });
                }
            }
            other => {
                return Err(LexError {
                    message: format!("unexpected character `{}`", other),
                    line,
                    col,
                });
            }
        };
        Ok(Token::new(kind, line, col))
    }
}

/// Convenience: tokenize a source string in one call.
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(source).tokenize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        tokenize(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn empty_input_yields_eof() {
        assert_eq!(kinds(""), vec![Eof]);
    }

    #[test]
    fn skips_whitespace_and_comments() {
        let src = "  // a line comment\n /* a\n block */ \n";
        assert_eq!(kinds(src), vec![Eof]);
    }

    #[test]
    fn lexes_keywords_and_identifiers() {
        // `int` is an identifier, not a keyword.
        assert_eq!(
            kinds("fn main return int _bar baz1"),
            vec![
                Fn,
                Ident("main".into()),
                Return,
                Ident("int".into()),
                Ident("_bar".into()),
                Ident("baz1".into()),
                Eof,
            ]
        );
    }

    #[test]
    fn lexes_integers_and_floats() {
        assert_eq!(kinds("0 42 3.14"), vec![Int(0), Int(42), Float(3.14), Eof]);
    }

    #[test]
    fn range_is_not_a_float() {
        // `0..10` must lex as Int DotDot Int, never as a float.
        assert_eq!(kinds("0..10"), vec![Int(0), DotDot, Int(10), Eof]);
        assert_eq!(kinds("0..=10"), vec![Int(0), DotDotEq, Int(10), Eof]);
    }

    #[test]
    fn lexes_strings_with_escapes() {
        assert_eq!(
            kinds("\"hello\\n\\\"world\\\"\""),
            vec![Str("hello\n\"world\"".into()), Eof]
        );
    }

    #[test]
    fn lexes_operators() {
        assert_eq!(
            kinds("== != <= >= && || <- :"),
            vec![Eq, NotEq, LtEq, GtEq, And, Or, Arrow, Colon, Eof]
        );
        assert_eq!(kinds("+ - * / %"), vec![Plus, Minus, Star, Slash, Percent, Eof]);
    }

    #[test]
    fn lexes_a_function() {
        let src = "fn add(a, b int): int {\n    return a + b\n}";
        assert_eq!(
            kinds(src),
            vec![
                Fn,
                Ident("add".into()),
                LParen,
                Ident("a".into()),
                Comma,
                Ident("b".into()),
                Ident("int".into()),
                RParen,
                Colon,
                Ident("int".into()),
                LBrace,
                Return,
                Ident("a".into()),
                Plus,
                Ident("b".into()),
                RBrace,
                Eof,
            ]
        );
    }

    #[test]
    fn tracks_line_and_column() {
        let toks = tokenize("a\n  b").unwrap();
        assert_eq!((toks[0].line, toks[0].col), (1, 1)); // a
        assert_eq!((toks[1].line, toks[1].col), (2, 3)); // b
    }

    #[test]
    fn errors_on_unterminated_string() {
        assert!(tokenize("\"oops").is_err());
    }

    #[test]
    fn errors_on_unexpected_char() {
        let err = tokenize("@").unwrap_err();
        assert_eq!((err.line, err.col), (1, 1));
    }
}
