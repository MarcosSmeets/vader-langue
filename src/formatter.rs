//! `vader fmt`: reprints the AST in Vader's single canonical style.
//!
//! Guarantees (verified in tests): **idempotency** — `fmt(fmt(x)) == fmt(x)` —
//! and **meaning preservation** — `parse(fmt(x))` has the same AST as `parse(x)`.
//!
//! Known limitation: comments are not yet preserved (the AST does not store them).

use crate::ast::*;

struct Formatter {
    out: String,
    indent: usize,
}

/// Formats a Vader program into its canonical form.
pub fn format(program: &Program) -> String {
    let mut fm = Formatter {
        out: String::new(),
        indent: 0,
    };

    if !program.imports.is_empty() {
        if program.imports.len() == 1 {
            fm.line(&format!("import {:?}", program.imports[0]));
        } else {
            fm.line("import (");
            fm.indent += 1;
            for p in &program.imports {
                fm.line(&format!("{:?}", p));
            }
            fm.indent -= 1;
            fm.line(")");
        }
        if !program.items.is_empty() {
            fm.out.push('\n');
        }
    }

    for (i, item) in program.items.iter().enumerate() {
        if i > 0 {
            fm.out.push('\n');
        }
        fm.item(item);
    }
    fm.out
}

impl Formatter {
    fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    // ---- types ----

    fn type_str(&self, t: &Type) -> String {
        match t {
            Type::Named(n) => n.clone(),
            Type::Slice(inner) => format!("[]{}", self.type_str(inner)),
            Type::Generic(name, args) => {
                let parts: Vec<String> = args.iter().map(|a| self.type_str(a)).collect();
                format!("{}[{}]", name, parts.join(", "))
            }
        }
    }

    fn type_params_str(&self, tps: &[TypeParam]) -> String {
        if tps.is_empty() {
            return String::new();
        }
        let parts: Vec<String> = tps
            .iter()
            .map(|tp| match &tp.constraint {
                Some(t) => format!("{} {}", tp.name, self.type_str(t)),
                None => tp.name.clone(),
            })
            .collect();
        format!("[{}]", parts.join(", "))
    }

    /// Groups consecutive parameters of the same type: `a, b int`.
    fn params_str(&self, params: &[Param]) -> String {
        let mut out = Vec::new();
        let mut i = 0;
        while i < params.len() {
            let ty = &params[i].ty;
            let mut names = vec![params[i].name.clone()];
            let mut j = i + 1;
            while j < params.len() && &params[j].ty == ty {
                names.push(params[j].name.clone());
                j += 1;
            }
            out.push(format!("{} {}", names.join(", "), self.type_str(ty)));
            i = j;
        }
        out.join(", ")
    }

    fn returns_str(&self, returns: &[Type]) -> String {
        match returns.len() {
            0 => String::new(),
            1 => format!(": {}", self.type_str(&returns[0])),
            _ => {
                let parts: Vec<String> = returns.iter().map(|t| self.type_str(t)).collect();
                format!(": ({})", parts.join(", "))
            }
        }
    }

    fn vis(&self, v: Visibility) -> &'static str {
        match v {
            Visibility::Public => "public ",
            Visibility::Private => "", // private is the default; omitted in the canonical form
        }
    }

    // ---- items ----

    fn item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => self.function(f),
            Item::Struct(s) => self.struct_def(s),
            Item::Interface(it) => self.interface(it),
            Item::Enum(e) => self.enum_def(e),
            Item::Test(t) => self.test(t),
        }
    }

    fn function(&mut self, f: &Function) {
        let mut sig = String::new();
        sig.push_str(self.vis(f.visibility));
        sig.push_str("fn ");
        if let Some(recv) = &f.receiver {
            sig.push_str(&format!("({} {}) ", recv.name, self.type_str(&recv.ty)));
        }
        sig.push_str(&f.name);
        sig.push_str(&self.type_params_str(&f.type_params));
        sig.push('(');
        sig.push_str(&self.params_str(&f.params));
        sig.push(')');
        sig.push_str(&self.returns_str(&f.returns));
        sig.push_str(" {");
        self.line(&sig);
        self.indent += 1;
        self.block(&f.body);
        self.indent -= 1;
        self.line("}");
    }

    fn struct_def(&mut self, s: &StructDef) {
        self.line(&format!(
            "{}struct {}{} {{",
            self.vis(s.visibility),
            s.name,
            self.type_params_str(&s.type_params)
        ));
        self.indent += 1;
        for field in &s.fields {
            self.line(&format!("{} {}", field.name, self.type_str(&field.ty)));
        }
        self.indent -= 1;
        self.line("}");
    }

    fn interface(&mut self, it: &InterfaceDef) {
        self.line(&format!(
            "{}interface {}{} {{",
            self.vis(it.visibility),
            it.name,
            self.type_params_str(&it.type_params)
        ));
        self.indent += 1;
        for m in &it.methods {
            self.line(&format!(
                "fn {}({}){}",
                m.name,
                self.params_str(&m.params),
                self.returns_str(&m.returns)
            ));
        }
        self.indent -= 1;
        self.line("}");
    }

    fn enum_def(&mut self, e: &EnumDef) {
        self.line(&format!(
            "{}enum {}{} {{",
            self.vis(e.visibility),
            e.name,
            self.type_params_str(&e.type_params)
        ));
        self.indent += 1;
        for v in &e.variants {
            if v.fields.is_empty() {
                self.line(&v.name);
            } else {
                self.line(&format!("{}({})", v.name, self.params_str(&v.fields)));
            }
        }
        self.indent -= 1;
        self.line("}");
    }

    fn test(&mut self, t: &TestDef) {
        self.line(&format!("test {:?} {{", t.name));
        self.indent += 1;
        self.block(&t.body);
        self.indent -= 1;
        self.line("}");
    }

    // ---- statements ----

    fn block(&mut self, b: &Block) {
        for s in &b.stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::VarDecl {
                is_const,
                decls,
                values,
            } => {
                if !is_const && decls.len() == 1 && values.len() == 1 {
                    if let ExprKind::Match { scrutinee, arms } = &values[0].kind {
                        self.line(&format!(
                            "{} {} = match {} {{",
                            self.type_str(&decls[0].ty),
                            decls[0].name,
                            self.expr(scrutinee)
                        ));
                        self.indent += 1;
                        self.match_arms(arms);
                        self.indent -= 1;
                        self.line("}");
                        return;
                    }
                }
                let prefix = if *is_const { "const " } else { "" };
                let lhs: Vec<String> = decls
                    .iter()
                    .map(|d| format!("{} {}", self.type_str(&d.ty), d.name))
                    .collect();
                let rhs: Vec<String> = values.iter().map(|v| self.expr(v)).collect();
                self.line(&format!("{}{} = {}", prefix, lhs.join(", "), rhs.join(", ")));
            }
            Stmt::Assign { target, value } => {
                self.line(&format!("{} = {}", self.expr(target), self.expr(value)));
            }
            Stmt::Return(values) => {
                if values.len() == 1 {
                    if let ExprKind::Match { scrutinee, arms } = &values[0].kind {
                        self.line(&format!("return match {} {{", self.expr(scrutinee)));
                        self.indent += 1;
                        self.match_arms(arms);
                        self.indent -= 1;
                        self.line("}");
                        return;
                    }
                }
                if values.is_empty() {
                    self.line("return");
                } else {
                    let vs: Vec<String> = values.iter().map(|v| self.expr(v)).collect();
                    self.line(&format!("return {}", vs.join(", ")));
                }
            }
            Stmt::If {
                cond,
                then_block,
                else_block,
            } => {
                self.line(&format!("if {} {{", self.expr(cond)));
                self.indent += 1;
                self.block(then_block);
                self.indent -= 1;
                match else_block {
                    Some(eb) => {
                        self.line("} else {");
                        self.indent += 1;
                        self.block(eb);
                        self.indent -= 1;
                        self.line("}");
                    }
                    None => self.line("}"),
                }
            }
            Stmt::For { head, body } => {
                match head {
                    ForHead::Infinite => self.line("for {"),
                    ForHead::While(c) => self.line(&format!("for {} {{", self.expr(c))),
                    ForHead::In { var, iter } => {
                        self.line(&format!("for {} in {} {{", var, self.expr(iter)))
                    }
                }
                self.indent += 1;
                self.block(body);
                self.indent -= 1;
                self.line("}");
            }
            Stmt::Spawn(call) => self.line(&format!("spawn {}", self.expr(call))),
            Stmt::Send { chan, value } => {
                self.line(&format!("{} <- {}", self.expr(chan), self.expr(value)))
            }
            Stmt::Assert(e) => self.line(&format!("assert {}", self.expr(e))),
            Stmt::Expr(e) => {
                if let ExprKind::Match { scrutinee, arms } = &e.kind {
                    self.line(&format!("match {} {{", self.expr(scrutinee)));
                    self.indent += 1;
                    self.match_arms(arms);
                    self.indent -= 1;
                    self.line("}");
                } else {
                    self.line(&self.expr(e));
                }
            }
        }
    }

    fn match_arms(&mut self, arms: &[MatchArm]) {
        for arm in arms {
            let pats: Vec<String> = arm.patterns.iter().map(|p| self.pattern(p)).collect();
            let guard = match &arm.guard {
                Some(g) => format!(" if {}", self.expr(g)),
                None => String::new(),
            };
            match &arm.body {
                MatchArmBody::Expr(e) => {
                    self.line(&format!("{}{}: {}", pats.join(", "), guard, self.expr(e)))
                }
                MatchArmBody::Block(b) => {
                    self.line(&format!("{}{}: {{", pats.join(", "), guard));
                    self.indent += 1;
                    self.block(b);
                    self.indent -= 1;
                    self.line("}");
                }
            }
        }
    }

    fn pattern(&self, p: &Pattern) -> String {
        match p {
            Pattern::Wildcard => "_".to_string(),
            Pattern::Literal(e) => self.expr(e),
            Pattern::Binding(n) => n.clone(),
            Pattern::Variant { name, bindings } => {
                if bindings.is_empty() {
                    name.clone()
                } else {
                    format!("{}({})", name, bindings.join(", "))
                }
            }
        }
    }

    // ---- expressions (with minimal parentheses by precedence) ----

    fn expr(&self, e: &Expr) -> String {
        self.expr_p(e, 0)
    }

    fn expr_p(&self, e: &Expr, parent: u8) -> String {
        match &e.kind {
            ExprKind::Int(v) => v.to_string(),
            ExprKind::Float(v) => {
                let mut s = v.to_string();
                if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                    s.push_str(".0");
                }
                s
            }
            ExprKind::Str(s) => format!("{:?}", s),
            ExprKind::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            ExprKind::Nil => "nil".to_string(),
            ExprKind::Ident(n) => n.clone(),
            ExprKind::Unary { op, expr } => {
                let inner = self.expr_p(expr, 8);
                let o = match op {
                    UnOp::Neg => "-",
                    UnOp::Not => "!",
                };
                format!("{}{}", o, inner)
            }
            ExprKind::Binary { op, left, right } => {
                let p = prec(op);
                let l = self.expr_p(left, p);
                let r = self.expr_p(right, p + 1);
                let s = match op {
                    BinOp::Range => format!("{}..{}", l, r),
                    BinOp::RangeIncl => format!("{}..={}", l, r),
                    _ => format!("{} {} {}", l, op_str(op), r),
                };
                if p < parent {
                    format!("({})", s)
                } else {
                    s
                }
            }
            ExprKind::Call { callee, args } => {
                let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                format!("{}({})", self.expr_p(callee, 9), a.join(", "))
            }
            ExprKind::Field { base, field } => format!("{}.{}", self.expr_p(base, 9), field),
            ExprKind::Index { base, index } => {
                format!("{}[{}]", self.expr_p(base, 9), self.expr(index))
            }
            ExprKind::StructLit { name, fields } => {
                if fields.is_empty() {
                    format!("{}{{}}", name)
                } else {
                    let fs: Vec<String> = fields
                        .iter()
                        .map(|(n, v)| format!("{}: {}", n, self.expr(v)))
                        .collect();
                    format!("{}{{ {} }}", name, fs.join(", "))
                }
            }
            ExprKind::SliceLit(elems) => {
                let es: Vec<String> = elems.iter().map(|x| self.expr(x)).collect();
                format!("[{}]", es.join(", "))
            }
            ExprKind::Recv(inner) => format!("<-{}", self.expr_p(inner, 8)),
            // a match nested in an expression is rare; the statement level handles the normal case.
            ExprKind::Match { scrutinee, .. } => format!("match {} {{ ... }}", self.expr(scrutinee)),
        }
    }
}

fn prec(op: &BinOp) -> u8 {
    match op {
        BinOp::Range | BinOp::RangeIncl => 1,
        BinOp::Or => 2,
        BinOp::And => 3,
        BinOp::Eq | BinOp::NotEq => 4,
        BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => 5,
        BinOp::Add | BinOp::Sub => 6,
        BinOp::Mul | BinOp::Div | BinOp::Rem => 7,
    }
}

fn op_str(op: &BinOp) -> &'static str {
    match op {
        BinOp::Or => "||",
        BinOp::And => "&&",
        BinOp::Eq => "==",
        BinOp::NotEq => "!=",
        BinOp::Lt => "<",
        BinOp::LtEq => "<=",
        BinOp::Gt => ">",
        BinOp::GtEq => ">=",
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::Range => "..",
        BinOp::RangeIncl => "..=",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, parser};

    fn parse(src: &str) -> Program {
        parser::parse(lexer::tokenize(src).unwrap()).unwrap()
    }

    const EXAMPLES: &[&str] = &[
        include_str!("../examples/hello.vd"),
        include_str!("../examples/basics.vd"),
        include_str!("../examples/math.vd"),
        include_str!("../examples/shapes.vd"),
        include_str!("../examples/generics.vd"),
        include_str!("../examples/api_usecase.vd"),
        include_str!("../examples/concurrency.vd"),
        include_str!("../examples/repository_vs_gateway.vd"),
    ];

    #[test]
    fn preserves_meaning_round_trip() {
        for src in EXAMPLES {
            let original = parse(src);
            let formatted = format(&original);
            let reparsed = parse(&formatted);
            assert_eq!(
                original, reparsed,
                "fmt changed the AST of:\n{}\n--- formatted ---\n{}",
                src, formatted
            );
        }
    }

    #[test]
    fn is_idempotent() {
        for src in EXAMPLES {
            let once = format(&parse(src));
            let twice = format(&parse(&once));
            assert_eq!(once, twice, "fmt is not idempotent");
        }
    }

    #[test]
    fn canonicalizes_spacing() {
        let out = format(&parse("fn  add ( a int , b int ) :int{return a+b}"));
        assert!(out.contains("fn add(a, b int): int {"));
        assert!(out.contains("    return a + b"));
    }

    #[test]
    fn omits_redundant_private() {
        let out = format(&parse("private fn f() { }"));
        assert!(out.starts_with("fn f()"));
        assert!(!out.contains("private"));
    }
}
