//! Recursive-descent parser: tokens -> AST.
//!
//! Fase 1 / incremento 1. Expressões usam precedence climbing. Literais de struct
//! são desabilitados no cabeçalho de `if`/`for` (mesma solução do Go) para evitar
//! ambiguidade com o `{` do bloco.

use crate::ast::*;
use crate::token::{Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at {}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ParseError {}

type PResult<T> = Result<T, ParseError>;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    allow_struct_lit: bool,
}

/// Parse a token stream into a `Program`.
pub fn parse(tokens: Vec<Token>) -> PResult<Program> {
    Parser {
        tokens,
        pos: 0,
        allow_struct_lit: true,
    }
    .parse_program()
}

impl Parser {
    // ---- cursor helpers ----

    fn kind(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek2(&self) -> &TokenKind {
        match self.tokens.get(self.pos + 1) {
            Some(t) => &t.kind,
            // tokens always end with Eof (the last element).
            None => &self.tokens[self.tokens.len() - 1].kind,
        }
    }

    fn at(&self, k: &TokenKind) -> bool {
        self.kind() == k
    }

    fn cur_pos(&self) -> (usize, usize) {
        let t = &self.tokens[self.pos];
        (t.line, t.col)
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if !matches!(t.kind, TokenKind::Eof) {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, k: &TokenKind) -> bool {
        if self.at(k) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn error<T>(&self, msg: impl Into<String>) -> PResult<T> {
        let t = &self.tokens[self.pos];
        Err(ParseError {
            message: msg.into(),
            line: t.line,
            col: t.col,
        })
    }

    fn expect(&mut self, k: &TokenKind) -> PResult<()> {
        if self.eat(k) {
            Ok(())
        } else {
            self.error(format!("expected {:?}, found {:?}", k, self.kind()))
        }
    }

    fn expect_ident(&mut self) -> PResult<String> {
        match self.kind().clone() {
            TokenKind::Ident(s) => {
                self.advance();
                Ok(s)
            }
            other => self.error(format!("expected identifier, found {:?}", other)),
        }
    }

    // ---- items ----

    fn parse_program(&mut self) -> PResult<Program> {
        let mut imports = Vec::new();
        let mut items = Vec::new();
        while !self.at(&TokenKind::Eof) {
            if self.at(&TokenKind::Import) {
                imports.extend(self.parse_import()?);
            } else {
                items.push(self.parse_item()?);
            }
        }
        Ok(Program { imports, items })
    }

    fn parse_import(&mut self) -> PResult<Vec<String>> {
        self.expect(&TokenKind::Import)?;
        if self.eat(&TokenKind::LParen) {
            let mut paths = Vec::new();
            while !self.at(&TokenKind::RParen) && !self.at(&TokenKind::Eof) {
                paths.push(self.expect_string()?);
            }
            self.expect(&TokenKind::RParen)?;
            Ok(paths)
        } else {
            Ok(vec![self.expect_string()?])
        }
    }

    fn expect_string(&mut self) -> PResult<String> {
        match self.kind().clone() {
            TokenKind::Str(s) => {
                self.advance();
                Ok(s)
            }
            other => self.error(format!("expected string literal, found {:?}", other)),
        }
    }

    fn parse_item(&mut self) -> PResult<Item> {
        if self.at(&TokenKind::Test) {
            return Ok(Item::Test(self.parse_test()?));
        }
        let visibility = self.parse_visibility();
        if self.at(&TokenKind::Fn) {
            return Ok(Item::Function(self.parse_function(visibility)?));
        }
        if self.at(&TokenKind::Struct) {
            return Ok(Item::Struct(self.parse_struct(visibility)?));
        }
        if self.at(&TokenKind::Interface) {
            return Ok(Item::Interface(self.parse_interface(visibility)?));
        }
        if self.at(&TokenKind::Enum) {
            return Ok(Item::Enum(self.parse_enum(visibility)?));
        }
        self.error(format!(
            "expected a declaration (fn/struct/interface/enum) at top level, found {:?}",
            self.kind()
        ))
    }

    /// `[ ident [constraint] {, ident [constraint]} ]` — vazio se não houver `[`.
    fn parse_type_params(&mut self) -> PResult<Vec<TypeParam>> {
        let mut params = Vec::new();
        if self.eat(&TokenKind::LBracket) {
            loop {
                let name = self.expect_ident()?;
                let constraint = if self.at(&TokenKind::Comma) || self.at(&TokenKind::RBracket) {
                    None
                } else {
                    Some(self.parse_type()?)
                };
                params.push(TypeParam { name, constraint });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::RBracket)?;
        }
        Ok(params)
    }

    fn parse_test(&mut self) -> PResult<TestDef> {
        self.expect(&TokenKind::Test)?;
        let name = self.expect_string()?;
        let body = self.parse_block()?;
        Ok(TestDef { name, body })
    }

    /// Optional `public`/`private` modifier. Defaults to `Private`.
    fn parse_visibility(&mut self) -> Visibility {
        if self.eat(&TokenKind::Public) {
            Visibility::Public
        } else if self.eat(&TokenKind::Private) {
            Visibility::Private
        } else {
            Visibility::Private
        }
    }

    fn parse_function(&mut self, visibility: Visibility) -> PResult<Function> {
        self.expect(&TokenKind::Fn)?;

        // Optional receiver for methods: `fn (u User) name(...)`.
        let mut receiver = None;
        if self.at(&TokenKind::LParen) {
            self.advance(); // (
            let name = self.expect_ident()?;
            let ty = self.parse_type()?;
            self.expect(&TokenKind::RParen)?;
            receiver = Some(Param { name, ty });
        }

        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;

        let mut returns = Vec::new();
        if self.eat(&TokenKind::Colon) {
            returns = self.parse_return_types()?;
        }

        let body = self.parse_block()?;
        Ok(Function {
            visibility,
            receiver,
            name,
            type_params,
            params,
            returns,
            body,
        })
    }

    /// Parses params with grouped types: `a, b int` -> a:int, b:int.
    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        // Collect each comma-separated entry, deferring the type when absent.
        let mut raw: Vec<(String, Option<Type>)> = Vec::new();
        while !self.at(&TokenKind::RParen) {
            let name = self.expect_ident()?;
            let ty = if self.at(&TokenKind::Comma) || self.at(&TokenKind::RParen) {
                None
            } else {
                Some(self.parse_type()?)
            };
            raw.push((name, ty));
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        // Resolve grouping: each typeless name takes the next explicit type.
        let mut params = Vec::new();
        let mut i = 0;
        while i < raw.len() {
            if raw[i].1.is_some() {
                params.push(Param {
                    name: raw[i].0.clone(),
                    ty: raw[i].1.clone().unwrap(),
                });
                i += 1;
            } else {
                let mut j = i;
                while j < raw.len() && raw[j].1.is_none() {
                    j += 1;
                }
                if j >= raw.len() {
                    return self.error("parameter group is missing a type");
                }
                let ty = raw[j].1.clone().unwrap();
                for entry in raw.iter().take(j + 1).skip(i) {
                    params.push(Param {
                        name: entry.0.clone(),
                        ty: ty.clone(),
                    });
                }
                i = j + 1;
            }
        }
        Ok(params)
    }

    fn parse_return_types(&mut self) -> PResult<Vec<Type>> {
        if self.eat(&TokenKind::LParen) {
            let mut types = vec![self.parse_type()?];
            while self.eat(&TokenKind::Comma) {
                types.push(self.parse_type()?);
            }
            self.expect(&TokenKind::RParen)?;
            Ok(types)
        } else {
            Ok(vec![self.parse_type()?])
        }
    }

    fn parse_struct(&mut self, visibility: Visibility) -> PResult<StructDef> {
        self.expect(&TokenKind::Struct)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(&TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            let fname = self.expect_ident()?;
            let ty = self.parse_type()?;
            fields.push(Param { name: fname, ty });
            self.eat(&TokenKind::Comma); // separador opcional
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(StructDef {
            visibility,
            name,
            type_params,
            fields,
        })
    }

    fn parse_interface(&mut self, visibility: Visibility) -> PResult<InterfaceDef> {
        self.expect(&TokenKind::Interface)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(&TokenKind::LBrace)?;
        let mut methods = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            self.expect(&TokenKind::Fn)?;
            let mname = self.expect_ident()?;
            self.expect(&TokenKind::LParen)?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen)?;
            let mut returns = Vec::new();
            if self.eat(&TokenKind::Colon) {
                returns = self.parse_return_types()?;
            }
            methods.push(MethodSig {
                name: mname,
                params,
                returns,
            });
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(InterfaceDef {
            visibility,
            name,
            type_params,
            methods,
        })
    }

    fn parse_enum(&mut self, visibility: Visibility) -> PResult<EnumDef> {
        self.expect(&TokenKind::Enum)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(&TokenKind::LBrace)?;
        let mut variants = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            let vname = self.expect_ident()?;
            let fields = if self.eat(&TokenKind::LParen) {
                let p = self.parse_params()?;
                self.expect(&TokenKind::RParen)?;
                p
            } else {
                Vec::new()
            };
            variants.push(EnumVariant {
                name: vname,
                fields,
            });
            self.eat(&TokenKind::Comma); // separador opcional
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(EnumDef {
            visibility,
            name,
            type_params,
            variants,
        })
    }

    fn parse_type(&mut self) -> PResult<Type> {
        if self.eat(&TokenKind::LBracket) {
            self.expect(&TokenKind::RBracket)?;
            let inner = self.parse_type()?;
            return Ok(Type::Slice(Box::new(inner)));
        }
        let mut name = self.expect_ident()?;
        // tipo qualificado: pkg.Type
        if self.at(&TokenKind::Dot) && matches!(self.peek2(), TokenKind::Ident(_)) {
            self.advance(); // .
            let n2 = self.expect_ident()?;
            name = format!("{}.{}", name, n2);
        }
        if self.eat(&TokenKind::LBracket) {
            let mut args = vec![self.parse_type()?];
            while self.eat(&TokenKind::Comma) {
                args.push(self.parse_type()?);
            }
            self.expect(&TokenKind::RBracket)?;
            // map[K]V — o tipo do valor vem DEPOIS do `]`
            if name == "map" {
                args.push(self.parse_type()?);
            }
            return Ok(Type::Generic(name, args));
        }
        Ok(Type::Named(name))
    }

    // ---- statements ----

    fn parse_block(&mut self) -> PResult<Block> {
        self.expect(&TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(Block { stmts })
    }

    fn parse_stmt(&mut self) -> PResult<Stmt> {
        if self.at(&TokenKind::Const) {
            return self.parse_var_decl(true);
        }
        if self.at(&TokenKind::Return) {
            return self.parse_return();
        }
        if self.at(&TokenKind::If) {
            return self.parse_if();
        }
        if self.at(&TokenKind::For) {
            return self.parse_for();
        }
        if self.at(&TokenKind::Spawn) {
            self.advance();
            let call = self.parse_expr()?;
            return Ok(Stmt::Spawn(call));
        }
        if self.at(&TokenKind::Assert) {
            self.advance();
            let e = self.parse_expr()?;
            return Ok(Stmt::Assert(e));
        }
        // `Type name ...` -> declaração. Detecta tipos compostos (chan[int], []T)
        // tentando parsear um tipo seguido de identificador (com backtracking).
        if self.looks_like_var_decl() {
            return self.parse_var_decl(false);
        }

        let expr = self.parse_expr()?;
        if self.eat(&TokenKind::Assign) {
            let value = self.parse_expr()?;
            return Ok(Stmt::Assign {
                target: expr,
                value,
            });
        }
        if self.eat(&TokenKind::Arrow) {
            // envio em canal: `chan <- value`
            let value = self.parse_expr()?;
            return Ok(Stmt::Send { chan: expr, value });
        }
        Ok(Stmt::Expr(expr))
    }

    /// Look-ahead com backtracking: o statement começa com `Type name`?
    fn looks_like_var_decl(&mut self) -> bool {
        // `_, ...` (descarte iniciando um retorno múltiplo)
        if matches!(self.kind(), TokenKind::Ident(u) if u == "_")
            && matches!(self.peek2(), TokenKind::Comma)
        {
            return true;
        }
        if !matches!(self.kind(), TokenKind::Ident(_)) && !self.at(&TokenKind::LBracket) {
            return false;
        }
        let save = self.pos;
        let ok = self.parse_type().is_ok() && matches!(self.kind(), TokenKind::Ident(_));
        self.pos = save;
        ok
    }

    fn parse_var_decl(&mut self, is_const: bool) -> PResult<Stmt> {
        if is_const {
            self.expect(&TokenKind::Const)?;
        }
        let mut decls = Vec::new();
        loop {
            // descarte: `_` (sem tipo) num retorno múltiplo
            let is_discard = matches!(self.kind(), TokenKind::Ident(u) if u == "_")
                && matches!(self.peek2(), TokenKind::Comma | TokenKind::Assign);
            if is_discard {
                self.advance();
                decls.push(Param {
                    name: "_".to_string(),
                    ty: Type::Named("_".to_string()),
                });
            } else {
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                decls.push(Param { name, ty });
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::Assign)?;
        let values = self.parse_expr_list()?;
        Ok(Stmt::VarDecl {
            is_const,
            decls,
            values,
        })
    }

    fn parse_return(&mut self) -> PResult<Stmt> {
        self.expect(&TokenKind::Return)?;
        if self.at(&TokenKind::RBrace) || self.at(&TokenKind::Eof) {
            Ok(Stmt::Return(Vec::new()))
        } else {
            Ok(Stmt::Return(self.parse_expr_list()?))
        }
    }

    fn parse_if(&mut self) -> PResult<Stmt> {
        self.expect(&TokenKind::If)?;
        let cond = self.parse_expr_no_struct()?;
        let then_block = self.parse_block()?;
        let else_block = if self.eat(&TokenKind::Else) {
            if self.at(&TokenKind::If) {
                // `else if` -> bloco com um único if aninhado.
                let nested = self.parse_if()?;
                Some(Block { stmts: vec![nested] })
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        Ok(Stmt::If {
            cond,
            then_block,
            else_block,
        })
    }

    fn parse_for(&mut self) -> PResult<Stmt> {
        self.expect(&TokenKind::For)?;

        if self.at(&TokenKind::LBrace) {
            let body = self.parse_block()?;
            return Ok(Stmt::For {
                head: ForHead::Infinite,
                body,
            });
        }

        let is_in =
            matches!(self.kind(), TokenKind::Ident(_)) && matches!(self.peek2(), TokenKind::In);
        if is_in {
            let var = self.expect_ident()?;
            self.expect(&TokenKind::In)?;
            let iter = self.parse_expr_no_struct()?;
            let body = self.parse_block()?;
            return Ok(Stmt::For {
                head: ForHead::In { var, iter },
                body,
            });
        }

        let cond = self.parse_expr_no_struct()?;
        let body = self.parse_block()?;
        Ok(Stmt::For {
            head: ForHead::While(cond),
            body,
        })
    }

    // ---- expressions ----

    fn parse_expr_list(&mut self) -> PResult<Vec<Expr>> {
        let mut list = vec![self.parse_expr()?];
        while self.eat(&TokenKind::Comma) {
            list.push(self.parse_expr()?);
        }
        Ok(list)
    }

    fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_binary(1)
    }

    fn parse_expr_no_struct(&mut self) -> PResult<Expr> {
        let saved = self.allow_struct_lit;
        self.allow_struct_lit = false;
        let result = self.parse_expr();
        self.allow_struct_lit = saved;
        result
    }

    fn parse_binary(&mut self, min_bp: u8) -> PResult<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let (op, bp) = match self.kind() {
                TokenKind::DotDot => (BinOp::Range, 1),
                TokenKind::DotDotEq => (BinOp::RangeIncl, 1),
                TokenKind::Or => (BinOp::Or, 2),
                TokenKind::And => (BinOp::And, 3),
                TokenKind::Eq => (BinOp::Eq, 4),
                TokenKind::NotEq => (BinOp::NotEq, 4),
                TokenKind::Lt => (BinOp::Lt, 5),
                TokenKind::LtEq => (BinOp::LtEq, 5),
                TokenKind::Gt => (BinOp::Gt, 5),
                TokenKind::GtEq => (BinOp::GtEq, 5),
                TokenKind::Plus => (BinOp::Add, 6),
                TokenKind::Minus => (BinOp::Sub, 6),
                TokenKind::Star => (BinOp::Mul, 7),
                TokenKind::Slash => (BinOp::Div, 7),
                TokenKind::Percent => (BinOp::Rem, 7),
                _ => break,
            };
            if bp < min_bp {
                break;
            }
            self.advance();
            let right = self.parse_binary(bp + 1)?;
            let (line, col) = (left.line, left.col);
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                line,
                col,
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        let (line, col) = self.cur_pos();
        if self.at(&TokenKind::Minus) {
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr {
                kind: ExprKind::Unary {
                    op: UnOp::Neg,
                    expr: Box::new(e),
                },
                line,
                col,
            });
        }
        if self.at(&TokenKind::Not) {
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr {
                kind: ExprKind::Unary {
                    op: UnOp::Not,
                    expr: Box::new(e),
                },
                line,
                col,
            });
        }
        if self.at(&TokenKind::Arrow) {
            self.advance();
            let e = self.parse_unary()?;
            return Ok(Expr {
                kind: ExprKind::Recv(Box::new(e)),
                line,
                col,
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut e = self.parse_primary()?;
        loop {
            // ASI-lite: um postfix (call/índice/campo) só continua na MESMA linha;
            // numa linha nova ele é o começo de um novo statement.
            if self.pos > 0 && self.tokens[self.pos].line > self.tokens[self.pos - 1].line {
                break;
            }
            let (line, col) = (e.line, e.col);
            if self.at(&TokenKind::LParen) {
                self.advance();
                let args = self.parse_args()?;
                self.expect(&TokenKind::RParen)?;
                e = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(e),
                        args,
                    },
                    line,
                    col,
                };
            } else if self.at(&TokenKind::Dot) {
                self.advance();
                let field = self.expect_ident()?;
                // struct literal qualificado: pkg.Type{ ... }
                let pkg = match &e.kind {
                    ExprKind::Ident(p) => Some(p.clone()),
                    _ => None,
                };
                if self.allow_struct_lit && self.at(&TokenKind::LBrace) && pkg.is_some() {
                    let kind = self.parse_struct_lit(format!("{}.{}", pkg.unwrap(), field))?;
                    e = Expr { kind, line, col };
                } else {
                    e = Expr {
                        kind: ExprKind::Field {
                            base: Box::new(e),
                            field,
                        },
                        line,
                        col,
                    };
                }
            } else if self.at(&TokenKind::LBracket) {
                self.advance();
                let index = self.parse_expr()?;
                self.expect(&TokenKind::RBracket)?;
                e = Expr {
                    kind: ExprKind::Index {
                        base: Box::new(e),
                        index: Box::new(index),
                    },
                    line,
                    col,
                };
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_args(&mut self) -> PResult<Vec<Expr>> {
        let saved = self.allow_struct_lit;
        self.allow_struct_lit = true;
        let mut args = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                args.push(self.parse_expr()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.allow_struct_lit = saved;
        Ok(args)
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let (line, col) = self.cur_pos();
        let kind = match self.kind().clone() {
            TokenKind::Int(v) => {
                self.advance();
                ExprKind::Int(v)
            }
            TokenKind::Float(v) => {
                self.advance();
                ExprKind::Float(v)
            }
            TokenKind::Str(s) => {
                self.advance();
                ExprKind::Str(s)
            }
            TokenKind::True => {
                self.advance();
                ExprKind::Bool(true)
            }
            TokenKind::False => {
                self.advance();
                ExprKind::Bool(false)
            }
            TokenKind::Nil => {
                self.advance();
                ExprKind::Nil
            }
            TokenKind::Ident(name) => {
                self.advance();
                if self.allow_struct_lit && self.at(&TokenKind::LBrace) {
                    self.parse_struct_lit(name)?
                } else {
                    ExprKind::Ident(name)
                }
            }
            TokenKind::LParen => {
                self.advance();
                let saved = self.allow_struct_lit;
                self.allow_struct_lit = true;
                let e = self.parse_expr()?;
                self.allow_struct_lit = saved;
                self.expect(&TokenKind::RParen)?;
                return Ok(e);
            }
            TokenKind::LBracket => {
                self.advance();
                let saved = self.allow_struct_lit;
                self.allow_struct_lit = true;
                let mut elems = Vec::new();
                while !self.at(&TokenKind::RBracket) && !self.at(&TokenKind::Eof) {
                    elems.push(self.parse_expr()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.allow_struct_lit = saved;
                self.expect(&TokenKind::RBracket)?;
                ExprKind::SliceLit(elems)
            }
            TokenKind::Match => self.parse_match()?,
            other => return self.error(format!("unexpected token in expression: {:?}", other)),
        };
        Ok(Expr { kind, line, col })
    }

    fn parse_match(&mut self) -> PResult<ExprKind> {
        self.expect(&TokenKind::Match)?;
        let scrutinee = self.parse_expr_no_struct()?;
        self.expect(&TokenKind::LBrace)?;
        let mut arms = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            let patterns = self.parse_pattern_list()?;
            let guard = if self.eat(&TokenKind::If) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(&TokenKind::Colon)?;
            let body = if self.at(&TokenKind::LBrace) {
                MatchArmBody::Block(self.parse_block()?)
            } else {
                MatchArmBody::Expr(self.parse_expr()?)
            };
            arms.push(MatchArm {
                patterns,
                guard,
                body,
            });
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(ExprKind::Match {
            scrutinee: Box::new(scrutinee),
            arms,
        })
    }

    fn parse_pattern_list(&mut self) -> PResult<Vec<Pattern>> {
        let mut pats = vec![self.parse_pattern()?];
        while self.eat(&TokenKind::Comma) {
            pats.push(self.parse_pattern()?);
        }
        Ok(pats)
    }

    fn parse_pattern(&mut self) -> PResult<Pattern> {
        let (line, col) = self.cur_pos();
        let lit = |k: ExprKind| Pattern::Literal(Expr { kind: k, line, col });
        match self.kind().clone() {
            TokenKind::Int(v) => {
                self.advance();
                Ok(lit(ExprKind::Int(v)))
            }
            TokenKind::Float(v) => {
                self.advance();
                Ok(lit(ExprKind::Float(v)))
            }
            TokenKind::Str(s) => {
                self.advance();
                Ok(lit(ExprKind::Str(s)))
            }
            TokenKind::True => {
                self.advance();
                Ok(lit(ExprKind::Bool(true)))
            }
            TokenKind::False => {
                self.advance();
                Ok(lit(ExprKind::Bool(false)))
            }
            TokenKind::Nil => {
                self.advance();
                Ok(lit(ExprKind::Nil))
            }
            TokenKind::Ident(name) => {
                self.advance();
                if name == "_" {
                    Ok(Pattern::Wildcard)
                } else if self.eat(&TokenKind::LParen) {
                    let mut bindings = Vec::new();
                    if !self.at(&TokenKind::RParen) {
                        loop {
                            bindings.push(self.expect_ident()?);
                            if !self.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    Ok(Pattern::Variant { name, bindings })
                } else {
                    Ok(Pattern::Binding(name))
                }
            }
            other => self.error(format!("unexpected token in pattern: {:?}", other)),
        }
    }

    fn parse_struct_lit(&mut self, name: String) -> PResult<ExprKind> {
        self.expect(&TokenKind::LBrace)?;
        let saved = self.allow_struct_lit;
        self.allow_struct_lit = true;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            let fname = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let val = self.parse_expr()?;
            fields.push((fname, val));
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.allow_struct_lit = saved;
        self.expect(&TokenKind::RBrace)?;
        Ok(ExprKind::StructLit { name, fields })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn parse_ok(src: &str) -> Program {
        parse(tokenize(src).unwrap()).unwrap()
    }

    #[test]
    fn parses_a_simple_function() {
        let prog = parse_ok("fn add(a, b int): int { return a + b }");
        assert_eq!(prog.items.len(), 1);
        let f = match &prog.items[0] {
            Item::Function(f) => f,
            _ => panic!("expected function"),
        };
        assert_eq!(f.name, "add");
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[0].ty, Type::Named("int".into())); // grouped type
        assert_eq!(f.params[1].ty, Type::Named("int".into()));
        assert_eq!(f.returns, vec![Type::Named("int".into())]);
    }

    #[test]
    fn parses_tuple_return() {
        let prog = parse_ok("fn d(a, b int): (int, error) { return a, b }");
        let f = match &prog.items[0] {
            Item::Function(f) => f,
            _ => panic!(),
        };
        assert_eq!(
            f.returns,
            vec![Type::Named("int".into()), Type::Named("error".into())]
        );
    }

    #[test]
    fn parses_var_decls() {
        let prog = parse_ok("fn f() { int x = 1\n const int y = 2\n int a, error b = g() }");
        let f = match &prog.items[0] {
            Item::Function(f) => f,
            _ => panic!(),
        };
        match &f.body.stmts[0] {
            Stmt::VarDecl { is_const, decls, .. } => {
                assert!(!is_const);
                assert_eq!(decls[0].name, "x");
            }
            _ => panic!("expected var decl"),
        }
        match &f.body.stmts[1] {
            Stmt::VarDecl { is_const, .. } => assert!(is_const),
            _ => panic!("expected const decl"),
        }
        match &f.body.stmts[2] {
            Stmt::VarDecl { decls, .. } => assert_eq!(decls.len(), 2),
            _ => panic!("expected multi decl"),
        }
    }

    #[test]
    fn respects_operator_precedence() {
        // 1 + 2 * 3  ==>  Add(1, Mul(2, 3))
        let prog = parse_ok("fn f(): int { return 1 + 2 * 3 }");
        let f = match &prog.items[0] {
            Item::Function(f) => f,
            _ => panic!(),
        };
        let ret = match &f.body.stmts[0] {
            Stmt::Return(v) => &v[0],
            _ => panic!(),
        };
        match &ret.kind {
            ExprKind::Binary { op: BinOp::Add, left, right } => {
                assert!(matches!(left.kind, ExprKind::Int(1)));
                assert!(matches!(right.kind, ExprKind::Binary { op: BinOp::Mul, .. }));
            }
            _ => panic!("expected Add at the top"),
        }
    }

    #[test]
    fn parses_if_else() {
        let prog = parse_ok("fn f() { if x > 0 { return } else { return } }");
        let f = match &prog.items[0] {
            Item::Function(f) => f,
            _ => panic!(),
        };
        assert!(matches!(f.body.stmts[0], Stmt::If { else_block: Some(_), .. }));
    }

    #[test]
    fn parses_method_struct_lit_and_field_access() {
        let prog = parse_ok(
            "struct User { id int  name string }\n\
             fn (u User) greeting(): string { return u.name }\n\
             fn main() { User u = User{ id: 1, name: \"x\" } }",
        );
        assert_eq!(prog.items.len(), 3);
        // método tem receiver
        if let Item::Function(f) = &prog.items[1] {
            assert!(f.receiver.is_some());
            assert_eq!(f.receiver.as_ref().unwrap().ty, Type::Named("User".into()));
        } else {
            panic!("expected method");
        }
    }

    #[test]
    fn struct_lit_disabled_in_if_condition() {
        // `if x { ... }` deve tratar x como condição, não como `x{...}`.
        let prog = parse_ok("fn f() { if x { return } }");
        let f = match &prog.items[0] {
            Item::Function(f) => f,
            _ => panic!(),
        };
        match &f.body.stmts[0] {
            Stmt::If { cond, .. } => assert!(matches!(&cond.kind, ExprKind::Ident(n) if n == "x")),
            _ => panic!("expected if"),
        }
    }

    #[test]
    fn parses_basics_example() {
        // Integração: o exemplo real precisa parsear inteiro.
        let src = include_str!("../examples/basics.vd");
        let prog = parse(tokenize(src).unwrap()).unwrap();
        // demo, divide, User, greeting, main
        assert_eq!(prog.items.len(), 5);
    }

    #[test]
    fn parses_math_example() {
        let src = include_str!("../examples/math.vd");
        let prog = parse(tokenize(src).unwrap()).unwrap();
        assert_eq!(prog.items.len(), 2); // add, divide
    }

    #[test]
    fn parses_test_block() {
        let prog = parse_ok("test \"adds\" { int x = 1\n assert x == 1 }");
        assert!(matches!(prog.items[0], Item::Test(_)));
        if let Item::Test(t) = &prog.items[0] {
            assert_eq!(t.name, "adds");
            assert!(matches!(t.body.stmts[1], Stmt::Assert(_)));
        }
    }

    #[test]
    fn parses_import() {
        let prog = parse_ok("import (\n \"std/fmt\"\n \"myapp/x\"\n )\n fn main() {}");
        assert_eq!(prog.imports, vec!["std/fmt".to_string(), "myapp/x".to_string()]);
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parses_enum_and_match() {
        let src = include_str!("../examples/shapes.vd");
        let prog = parse(tokenize(src).unwrap()).unwrap();
        assert_eq!(prog.items.len(), 3); // enum Shape, fn area, fn main
    }

    #[test]
    fn parses_match_structure() {
        let prog = parse_ok("fn f(s int): int { return match s { 1, 2: 10  _: 0 } }");
        let f = match &prog.items[0] {
            Item::Function(f) => f,
            _ => panic!(),
        };
        let ret = match &f.body.stmts[0] {
            Stmt::Return(v) => &v[0],
            _ => panic!(),
        };
        match &ret.kind {
            ExprKind::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
                assert_eq!(arms[0].patterns.len(), 2); // 1, 2
            }
            _ => panic!("expected match"),
        }
    }

    #[test]
    fn parses_generics_example() {
        let src = include_str!("../examples/generics.vd");
        let prog = parse(tokenize(src).unwrap()).unwrap();
        assert_eq!(prog.items.len(), 3); // Box, Repository, first
    }

    #[test]
    fn parses_spawn_and_send() {
        let prog = parse_ok("fn f() { spawn work(c)  c <- 1 }");
        let f = match &prog.items[0] {
            Item::Function(f) => f,
            _ => panic!(),
        };
        assert!(matches!(f.body.stmts[0], Stmt::Spawn(_)));
        assert!(matches!(f.body.stmts[1], Stmt::Send { .. }));
    }

    #[test]
    fn parses_remaining_examples() {
        // Os outros exemplos reais precisam parsear inteiros.
        parse(tokenize(include_str!("../examples/api_usecase.vd")).unwrap()).unwrap();
        parse(tokenize(include_str!("../examples/concurrency.vd")).unwrap()).unwrap();
        parse(tokenize(include_str!("../examples/repository_vs_gateway.vd")).unwrap()).unwrap();
    }

    #[test]
    fn parses_visibility() {
        let prog = parse_ok("public fn a() {}\n fn b() {}\n private struct S { x int }");
        let vis_a = match &prog.items[0] {
            Item::Function(f) => f.visibility,
            _ => panic!(),
        };
        let vis_b = match &prog.items[1] {
            Item::Function(f) => f.visibility,
            _ => panic!(),
        };
        let vis_s = match &prog.items[2] {
            Item::Struct(s) => s.visibility,
            _ => panic!(),
        };
        assert_eq!(vis_a, Visibility::Public);
        assert_eq!(vis_b, Visibility::Private); // padrão
        assert_eq!(vis_s, Visibility::Private);
    }
}
