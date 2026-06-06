//! Backend: transpila a AST da Vader para código-fonte Go.
//!
//! Incremento 1: funções, métodos, structs, statements, expressões, `print`/`error`.
//! Incremento 2: enum (-> interface + structs), `match` (-> switch) em posição de
//! statement, interfaces, genéricos (-> generics do Go) e canais.
//!
//! Constrói `package main` e só importa `fmt`/`errors` quando usados.

use std::collections::HashMap;

use crate::ast::*;

struct Gen {
    out: String,
    indent: usize,
    uses_fmt: bool,
    uses_errors: bool,
    /// nome da variante de enum -> nomes dos campos (em ordem)
    variants: HashMap<String, Vec<String>>,
    /// quando true, injeta `__cov("fn")` na entrada de cada função (modo `vader test`).
    coverage: bool,
}

/// Chave de cobertura de uma função (métodos viram `Tipo.metodo`).
fn fn_key(f: &Function) -> String {
    match &f.receiver {
        Some(r) => format!("{}.{}", type_base(&r.ty), f.name),
        None => f.name.clone(),
    }
}

fn type_base(t: &Type) -> String {
    match t {
        Type::Named(n) | Type::Generic(n, _) => n.clone(),
        Type::Slice(_) => "slice".to_string(),
    }
}

enum MatchMode {
    Return,
    Assign(String),
    Stmt,
}

/// Gera código Go a partir de um programa Vader já parseado (e idealmente checado).
pub fn generate(program: &Program) -> Result<String, String> {
    let mut variants = HashMap::new();
    for item in &program.items {
        if let Item::Enum(e) = item {
            for v in &e.variants {
                variants.insert(v.name.clone(), v.fields.iter().map(|f| f.name.clone()).collect());
            }
        }
    }

    let mut g = Gen {
        out: String::new(),
        indent: 0,
        uses_fmt: false,
        uses_errors: false,
        variants,
        coverage: false,
    };

    for item in &program.items {
        match item {
            Item::Function(f) => g.gen_function(f)?,
            Item::Struct(s) => g.gen_struct(s)?,
            Item::Interface(it) => g.gen_interface(it)?,
            Item::Enum(e) => g.gen_enum(e)?,
            // blocos `test` não entram no binário (são para `vader test`).
            Item::Test(_) => {}
        }
    }

    let mut header = String::from("package main\n\n");
    let mut imports = Vec::new();
    if g.uses_fmt {
        imports.push("\"fmt\"");
    }
    if g.uses_errors {
        imports.push("\"errors\"");
    }
    match imports.len() {
        0 => {}
        1 => header.push_str(&format!("import {}\n\n", imports[0])),
        _ => {
            header.push_str("import (\n");
            for i in &imports {
                header.push_str(&format!("\t{}\n", i));
            }
            header.push_str(")\n\n");
        }
    }

    Ok(format!("{}{}", header, g.out))
}

/// Gera um programa Go que roda os blocos `test`, reporta ✓/✗, imprime a cobertura
/// de funções e (se `gate`) sai com código != 0 quando a cobertura < `min_cov`.
pub fn generate_tests(program: &Program, gate: bool, min_cov: f64) -> Result<String, String> {
    let mut variants = HashMap::new();
    for item in &program.items {
        if let Item::Enum(e) = item {
            for v in &e.variants {
                variants.insert(v.name.clone(), v.fields.iter().map(|f| f.name.clone()).collect());
            }
        }
    }
    let mut g = Gen {
        out: String::new(),
        indent: 0,
        uses_fmt: false,
        uses_errors: false,
        variants,
        coverage: true,
    };

    // Declarações do usuário (pulando o `main` e os blocos `test`).
    let mut all_fns = Vec::new();
    for item in &program.items {
        match item {
            Item::Function(f) => {
                if f.receiver.is_none() && f.name == "main" {
                    continue;
                }
                all_fns.push(fn_key(f));
                g.gen_function(f)?;
            }
            Item::Struct(s) => g.gen_struct(s)?,
            Item::Interface(it) => g.gen_interface(it)?,
            Item::Enum(e) => g.gen_enum(e)?,
            Item::Test(_) => {}
        }
    }

    // Cada `test` vira uma func __test_N.
    let mut test_names = Vec::new();
    for item in &program.items {
        if let Item::Test(t) = item {
            g.line(&format!("func __test_{}() {{", test_names.len()));
            g.indent += 1;
            g.gen_block(&t.body)?;
            g.indent -= 1;
            g.line("}");
            g.line("");
            test_names.push(t.name.clone());
        }
    }

    // Runner main.
    let mut r = String::from("func main() {\n");
    r.push_str("    tests := []struct {\n        name string\n        fn   func()\n    }{\n");
    for (i, name) in test_names.iter().enumerate() {
        r.push_str(&format!("        {{{:?}, __test_{}}},\n", name, i));
    }
    r.push_str("    }\n");
    r.push_str("    passed := 0\n");
    r.push_str("    for _, t := range tests {\n");
    r.push_str("        if __run(t.fn) {\n            fmt.Println(\"  \\u2713\", t.name)\n            passed++\n");
    r.push_str("        } else {\n            fmt.Println(\"  \\u2717\", t.name)\n        }\n    }\n");
    r.push_str("    failed := len(tests) - passed\n");
    r.push_str("    fmt.Printf(\"\\n%d passed, %d failed\\n\", passed, failed)\n");
    r.push_str("    allFns := []string{");
    for (i, k) in all_fns.iter().enumerate() {
        if i > 0 {
            r.push_str(", ");
        }
        r.push_str(&format!("{:?}", k));
    }
    r.push_str("}\n");
    r.push_str("    covered := 0\n    for _, f := range allFns {\n        if __covered[f] {\n            covered++\n        }\n    }\n");
    r.push_str("    pct := 100.0\n    if len(allFns) > 0 {\n        pct = float64(covered) / float64(len(allFns)) * 100.0\n    }\n");
    r.push_str("    fmt.Printf(\"coverage: %.1f%% (%d/%d functions)\\n\", pct, covered, len(allFns))\n");
    r.push_str("    for _, f := range allFns {\n        if !__covered[f] {\n            fmt.Println(\"  uncovered:\", f)\n        }\n    }\n");
    r.push_str("    if failed > 0 {\n        os.Exit(1)\n    }\n");
    if gate {
        r.push_str(&format!(
            "    if pct < {min:.1} {{\n        fmt.Printf(\"\\u2717 coverage %.1f%% is below the minimum of {min:.1}%%\\n\", pct)\n        os.Exit(2)\n    }}\n",
            min = min_cov
        ));
    }
    r.push_str("}\n");

    let mut header = String::from("package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n");
    if g.uses_errors {
        header.push_str("\t\"errors\"\n");
    }
    header.push_str(")\n\n");
    header.push_str("var __covered = map[string]bool{}\n\n");
    header.push_str("func __cov(n string) { __covered[n] = true }\n\n");
    header.push_str("func __run(fn func()) (ok bool) {\n    defer func() {\n        if r := recover(); r != nil {\n            ok = false\n        }\n    }()\n    fn()\n    return true\n}\n\n");

    Ok(format!("{}{}{}", header, g.out, r))
}

impl Gen {
    fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn go_type(&self, t: &Type) -> Result<String, String> {
        match t {
            Type::Named(n) => Ok(match n.as_str() {
                "int" => "int",
                "float" => "float64",
                "bool" => "bool",
                "string" => "string",
                "error" => "error",
                other => other,
            }
            .to_string()),
            Type::Slice(inner) => Ok(format!("[]{}", self.go_type(inner)?)),
            Type::Generic(name, args) => {
                if name == "chan" && args.len() == 1 {
                    Ok(format!("chan {}", self.go_type(&args[0])?))
                } else {
                    let mut parts = Vec::new();
                    for a in args {
                        parts.push(self.go_type(a)?);
                    }
                    Ok(format!("{}[{}]", name, parts.join(", ")))
                }
            }
        }
    }

    /// Parâmetros de tipo Go: `[T any, U Constraint]`. Vazio se não houver.
    fn go_type_params(&self, tps: &[TypeParam]) -> Result<String, String> {
        if tps.is_empty() {
            return Ok(String::new());
        }
        let mut parts = Vec::new();
        for tp in tps {
            let constraint = match &tp.constraint {
                Some(t) => self.go_type(t)?,
                None => "any".to_string(),
            };
            parts.push(format!("{} {}", tp.name, constraint));
        }
        Ok(format!("[{}]", parts.join(", ")))
    }

    fn gen_struct(&mut self, s: &StructDef) -> Result<(), String> {
        let tp = self.go_type_params(&s.type_params)?;
        self.line(&format!("type {}{} struct {{", s.name, tp));
        self.indent += 1;
        for field in &s.fields {
            let ty = self.go_type(&field.ty)?;
            self.line(&format!("{} {}", field.name, ty));
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
        Ok(())
    }

    fn gen_interface(&mut self, it: &InterfaceDef) -> Result<(), String> {
        let tp = self.go_type_params(&it.type_params)?;
        self.line(&format!("type {}{} interface {{", it.name, tp));
        self.indent += 1;
        for m in &it.methods {
            let mut sig = String::new();
            sig.push_str(&m.name);
            sig.push('(');
            for (i, p) in m.params.iter().enumerate() {
                if i > 0 {
                    sig.push_str(", ");
                }
                sig.push_str(&format!("{} {}", p.name, self.go_type(&p.ty)?));
            }
            sig.push(')');
            self.append_returns(&mut sig, &m.returns)?;
            self.line(&sig);
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
        Ok(())
    }

    fn gen_enum(&mut self, e: &EnumDef) -> Result<(), String> {
        if !e.type_params.is_empty() {
            return Err("backend: enums genéricos ainda não suportados".into());
        }
        // Interface marcadora + uma struct por variante implementando-a.
        self.line(&format!("type {} interface {{ is{}() }}", e.name, e.name));
        for v in &e.variants {
            self.line(&format!("type {} struct {{", v.name));
            self.indent += 1;
            for f in &v.fields {
                let ty = self.go_type(&f.ty)?;
                self.line(&format!("{} {}", f.name, ty));
            }
            self.indent -= 1;
            self.line("}");
            self.line(&format!("func ({}) is{}() {{}}", v.name, e.name));
        }
        self.line("");
        Ok(())
    }

    fn append_returns(&self, sig: &mut String, returns: &[Type]) -> Result<(), String> {
        match returns.len() {
            0 => {}
            1 => {
                sig.push(' ');
                sig.push_str(&self.go_type(&returns[0])?);
            }
            _ => {
                sig.push_str(" (");
                for (i, r) in returns.iter().enumerate() {
                    if i > 0 {
                        sig.push_str(", ");
                    }
                    sig.push_str(&self.go_type(r)?);
                }
                sig.push(')');
            }
        }
        Ok(())
    }

    fn gen_function(&mut self, f: &Function) -> Result<(), String> {
        let mut sig = String::from("func ");
        if let Some(recv) = &f.receiver {
            sig.push_str(&format!("({} {}) ", recv.name, self.go_type(&recv.ty)?));
        }
        sig.push_str(&f.name);
        sig.push_str(&self.go_type_params(&f.type_params)?);
        sig.push('(');
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                sig.push_str(", ");
            }
            sig.push_str(&format!("{} {}", p.name, self.go_type(&p.ty)?));
        }
        sig.push(')');
        self.append_returns(&mut sig, &f.returns)?;
        sig.push_str(" {");
        self.line(&sig);

        self.indent += 1;
        if self.coverage && !(f.receiver.is_none() && f.name == "main") {
            self.line(&format!("__cov({:?})", fn_key(f)));
        }
        self.gen_block(&f.body)?;
        self.indent -= 1;
        self.line("}");
        self.line("");
        Ok(())
    }

    fn gen_block(&mut self, b: &Block) -> Result<(), String> {
        for st in &b.stmts {
            self.gen_stmt(st)?;
        }
        Ok(())
    }

    fn gen_stmt(&mut self, s: &Stmt) -> Result<(), String> {
        match s {
            Stmt::VarDecl {
                is_const,
                decls,
                values,
            } => {
                // `Type name = match ... { }` -> declara e atribui via switch.
                if !is_const && decls.len() == 1 && values.len() == 1 {
                    if let ExprKind::Match { scrutinee, arms } = &values[0].kind {
                        let ty = self.go_type(&decls[0].ty)?;
                        self.line(&format!("var {} {}", decls[0].name, ty));
                        self.gen_match(scrutinee, arms, MatchMode::Assign(decls[0].name.clone()))?;
                        self.line(&format!("_ = {}", decls[0].name));
                        return Ok(());
                    }
                }

                let names: Vec<String> = decls.iter().map(|d| d.name.clone()).collect();
                if *is_const {
                    let v = self.gen_expr(&values[0])?;
                    self.line(&format!("const {} = {}", names.join(", "), v));
                    return Ok(());
                }
                let rhs = if values.len() == names.len() {
                    let mut vs = Vec::new();
                    for v in values {
                        vs.push(self.gen_expr(v)?);
                    }
                    vs.join(", ")
                } else {
                    self.gen_expr(&values[0])?
                };
                self.line(&format!("var {} = {}", names.join(", "), rhs));
                for n in &names {
                    if n != "_" {
                        self.line(&format!("_ = {}", n));
                    }
                }
                Ok(())
            }
            Stmt::Assign { target, value } => {
                let t = self.gen_expr(target)?;
                let v = self.gen_expr(value)?;
                self.line(&format!("{} = {}", t, v));
                Ok(())
            }
            Stmt::Return(values) => {
                if values.len() == 1 {
                    if let ExprKind::Match { scrutinee, arms } = &values[0].kind {
                        return self.gen_match(scrutinee, arms, MatchMode::Return);
                    }
                }
                if values.is_empty() {
                    self.line("return");
                } else {
                    let mut vs = Vec::new();
                    for v in values {
                        vs.push(self.gen_expr(v)?);
                    }
                    self.line(&format!("return {}", vs.join(", ")));
                }
                Ok(())
            }
            Stmt::If {
                cond,
                then_block,
                else_block,
            } => {
                let c = self.gen_expr(cond)?;
                self.line(&format!("if {} {{", c));
                self.indent += 1;
                self.gen_block(then_block)?;
                self.indent -= 1;
                if let Some(eb) = else_block {
                    self.line("} else {");
                    self.indent += 1;
                    self.gen_block(eb)?;
                    self.indent -= 1;
                }
                self.line("}");
                Ok(())
            }
            Stmt::For { head, body } => self.gen_for(head, body),
            Stmt::Spawn(call) => {
                let c = self.gen_expr(call)?;
                self.line(&format!("go {}", c));
                Ok(())
            }
            Stmt::Send { chan, value } => {
                let c = self.gen_expr(chan)?;
                let v = self.gen_expr(value)?;
                self.line(&format!("{} <- {}", c, v));
                Ok(())
            }
            Stmt::Assert(e) => {
                let c = self.gen_expr(e)?;
                self.line(&format!("if !({}) {{ panic(\"assertion failed\") }}", c));
                Ok(())
            }
            Stmt::Expr(e) => {
                if let ExprKind::Match { scrutinee, arms } = &e.kind {
                    return self.gen_match(scrutinee, arms, MatchMode::Stmt);
                }
                let s = self.gen_expr(e)?;
                self.line(&s);
                Ok(())
            }
        }
    }

    fn gen_match(
        &mut self,
        scrut: &Expr,
        arms: &[MatchArm],
        mode: MatchMode,
    ) -> Result<(), String> {
        if arms.iter().any(|a| a.guard.is_some()) {
            return Err("backend: guardas em `match` ainda não suportadas".into());
        }
        let is_type_switch = arms
            .iter()
            .any(|a| a.patterns.iter().any(|p| matches!(p, Pattern::Variant { .. })));
        let scrut_s = self.gen_expr(scrut)?;

        if is_type_switch {
            self.line(&format!("switch __v := {}.(type) {{", scrut_s));
        } else {
            self.line(&format!("switch {} {{", scrut_s));
        }

        for arm in arms {
            let is_default = arm
                .patterns
                .iter()
                .any(|p| matches!(p, Pattern::Wildcard | Pattern::Binding(_)));
            if is_default {
                self.line("default:");
            } else if is_type_switch {
                let names: Vec<String> = arm
                    .patterns
                    .iter()
                    .filter_map(|p| match p {
                        Pattern::Variant { name, .. } => Some(name.clone()),
                        _ => None,
                    })
                    .collect();
                self.line(&format!("case {}:", names.join(", ")));
            } else {
                let mut lits = Vec::new();
                for p in &arm.patterns {
                    if let Pattern::Literal(e) = p {
                        lits.push(self.gen_expr(e)?);
                    }
                }
                self.line(&format!("case {}:", lits.join(", ")));
            }

            self.indent += 1;
            if is_type_switch {
                if let Some(Pattern::Variant { name, bindings }) = arm
                    .patterns
                    .iter()
                    .find(|p| matches!(p, Pattern::Variant { .. }))
                {
                    if let Some(fields) = self.variants.get(name).cloned() {
                        for (b, fld) in bindings.iter().zip(fields.iter()) {
                            self.line(&format!("{} := __v.{}", b, fld));
                            self.line(&format!("_ = {}", b));
                        }
                    }
                }
            }
            self.gen_match_body(&arm.body, &mode)?;
            self.indent -= 1;
        }

        self.line("}");
        if matches!(mode, MatchMode::Return) {
            self.line("panic(\"vader: unreachable match\")");
        }
        Ok(())
    }

    fn gen_match_body(&mut self, body: &MatchArmBody, mode: &MatchMode) -> Result<(), String> {
        match body {
            MatchArmBody::Expr(e) => {
                let s = self.gen_expr(e)?;
                match mode {
                    MatchMode::Return => self.line(&format!("return {}", s)),
                    MatchMode::Assign(name) => self.line(&format!("{} = {}", name, s)),
                    MatchMode::Stmt => self.line(&s),
                }
            }
            MatchArmBody::Block(b) => self.gen_block(b)?,
        }
        Ok(())
    }

    fn gen_for(&mut self, head: &ForHead, body: &Block) -> Result<(), String> {
        let mut suppress: Option<String> = None;
        match head {
            ForHead::Infinite => self.line("for {"),
            ForHead::While(c) => {
                let cs = self.gen_expr(c)?;
                self.line(&format!("for {} {{", cs));
            }
            ForHead::In { var, iter } => match &iter.kind {
                ExprKind::Binary {
                    op: BinOp::Range,
                    left,
                    right,
                } => {
                    let (l, r) = (self.gen_expr(left)?, self.gen_expr(right)?);
                    self.line(&format!("for {0} := {1}; {0} < {2}; {0}++ {{", var, l, r));
                }
                ExprKind::Binary {
                    op: BinOp::RangeIncl,
                    left,
                    right,
                } => {
                    let (l, r) = (self.gen_expr(left)?, self.gen_expr(right)?);
                    self.line(&format!("for {0} := {1}; {0} <= {2}; {0}++ {{", var, l, r));
                }
                _ => {
                    // range sobre canal (1 valor). Iteração de slice por elemento
                    // precisa de info de tipo e fica para um incremento futuro.
                    let it = self.gen_expr(iter)?;
                    self.line(&format!("for {} := range {} {{", var, it));
                    suppress = Some(var.clone());
                }
            },
        }
        self.indent += 1;
        if let Some(v) = suppress {
            self.line(&format!("_ = {}", v));
        }
        self.gen_block(body)?;
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    fn gen_expr(&mut self, e: &Expr) -> Result<String, String> {
        Ok(match &e.kind {
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
                let inner = self.gen_expr(expr)?;
                match op {
                    UnOp::Neg => format!("-({})", inner),
                    UnOp::Not => format!("!({})", inner),
                }
            }
            ExprKind::Binary { op, left, right } => {
                let o = match op {
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
                    BinOp::Range | BinOp::RangeIncl => {
                        return Err("backend: range fora de um `for` não é suportado".into())
                    }
                };
                format!("({} {} {})", self.gen_expr(left)?, o, self.gen_expr(right)?)
            }
            ExprKind::Call { callee, args } => self.gen_call(callee, args)?,
            ExprKind::Field { base, field } => format!("{}.{}", self.gen_expr(base)?, field),
            ExprKind::Index { base, index } => {
                format!("{}[{}]", self.gen_expr(base)?, self.gen_expr(index)?)
            }
            ExprKind::StructLit { name, fields } => {
                let mut parts = Vec::new();
                for (fname, fexpr) in fields {
                    parts.push(format!("{}: {}", fname, self.gen_expr(fexpr)?));
                }
                format!("{}{{{}}}", name, parts.join(", "))
            }
            ExprKind::SliceLit(elems) => {
                let mut parts = Vec::new();
                for el in elems {
                    parts.push(self.gen_expr(el)?);
                }
                format!("[]interface{{}}{{{}}}", parts.join(", "))
            }
            ExprKind::Recv(inner) => format!("<-{}", self.gen_expr(inner)?),
            ExprKind::Match { .. } => {
                return Err(
                    "backend: `match` só é suportado em posição de return/atribuição/statement"
                        .into(),
                )
            }
        })
    }

    fn gen_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<String, String> {
        // Construção de canal: chan[T](buffer) -> make(chan T, buffer)
        if let ExprKind::Index { base, index } = &callee.kind {
            if let ExprKind::Ident(b) = &base.kind {
                if b == "chan" {
                    let elem = self.gen_expr(index)?;
                    let mut bufs = Vec::new();
                    for a in args {
                        bufs.push(self.gen_expr(a)?);
                    }
                    let buf = if bufs.is_empty() {
                        String::new()
                    } else {
                        format!(", {}", bufs.join(", "))
                    };
                    return Ok(format!("make(chan {}{})", elem, buf));
                }
            }
        }

        let mut arg_strs = Vec::new();
        for a in args {
            arg_strs.push(self.gen_expr(a)?);
        }

        if let ExprKind::Ident(name) = &callee.kind {
            if name == "print" {
                self.uses_fmt = true;
                return Ok(format!("fmt.Println({})", arg_strs.join(", ")));
            }
            if name == "error" {
                self.uses_errors = true;
                return Ok(format!("errors.New({})", arg_strs.join(", ")));
            }
            // Construção de variante de enum: Circle(2.0) -> Circle{radius: 2.0}
            if let Some(field_names) = self.variants.get(name).cloned() {
                let mut parts = Vec::new();
                for (fname, astr) in field_names.iter().zip(arg_strs.iter()) {
                    parts.push(format!("{}: {}", fname, astr));
                }
                return Ok(format!("{}{{{}}}", name, parts.join(", ")));
            }
        }

        let c = self.gen_expr(callee)?;
        Ok(format!("{}({})", c, arg_strs.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, parser};

    fn gen(src: &str) -> String {
        let prog = parser::parse(lexer::tokenize(src).unwrap()).unwrap();
        generate(&prog).unwrap()
    }

    #[test]
    fn generates_hello() {
        let go = gen("fn main() { print(\"hi\") }");
        assert!(go.contains("package main"));
        assert!(go.contains("import \"fmt\""));
        assert!(go.contains("func main() {"));
        assert!(go.contains("fmt.Println(\"hi\")"));
    }

    #[test]
    fn generates_struct_and_method() {
        let go = gen("struct U { name string }\n fn (u U) hi(): string { return u.name }");
        assert!(go.contains("type U struct {"));
        assert!(go.contains("func (u U) hi() string {"));
    }

    #[test]
    fn generates_multi_return() {
        let go = gen("fn d(a, b int): (int, error) { return a, nil }");
        assert!(go.contains("func d(a int, b int) (int, error) {"));
    }

    #[test]
    fn maps_error_builtin() {
        let go = gen("fn f(): error { return error(\"boom\") }");
        assert!(go.contains("import \"errors\""));
        assert!(go.contains("errors.New(\"boom\")"));
    }

    #[test]
    fn range_for_becomes_c_style() {
        let go = gen("fn f() { for i in 0..3 { print(i) } }");
        assert!(go.contains("for i := 0; i < 3; i++ {"));
    }

    #[test]
    fn generates_generics() {
        let go = gen("struct Box[T] { value T }\n fn first[T](items []T): T { return items[0] }");
        assert!(go.contains("type Box[T any] struct {"));
        assert!(go.contains("func first[T any](items []T) T {"));
    }

    #[test]
    fn generates_interface() {
        let go = gen("interface Repo { fn save(x int): (int, error) }");
        assert!(go.contains("type Repo interface {"));
        assert!(go.contains("save(x int) (int, error)"));
    }

    #[test]
    fn generates_enum_and_match_as_switch() {
        let go = gen(
            "enum Shape { Circle(radius float)  Point }\n\
             fn area(s Shape): float { return match s { Circle(r): r  Point: 0.0 } }",
        );
        assert!(go.contains("type Shape interface { isShape() }"));
        assert!(go.contains("type Circle struct {"));
        assert!(go.contains("func (Circle) isShape() {}"));
        assert!(go.contains("switch __v := s.(type) {"));
        assert!(go.contains("case Circle:"));
        assert!(go.contains("r := __v.radius"));
    }

    #[test]
    fn generates_channel_make() {
        let go = gen("fn f() { chan[int] c = chan[int](8)  c <- 1 }");
        assert!(go.contains("make(chan int, 8)"));
        assert!(go.contains("c <- 1"));
    }

    #[test]
    fn generates_basics_example() {
        let src = include_str!("../examples/basics.vd");
        let prog = parser::parse(lexer::tokenize(src).unwrap()).unwrap();
        let go = generate(&prog).unwrap();
        assert!(go.contains("func main() {"));
        assert!(go.contains("errors.New(\"division by zero\")"));
    }

    #[test]
    fn generates_test_harness_with_coverage() {
        let prog = parser::parse(
            lexer::tokenize(
                "fn add(a, b int): int { return a + b }\n\
                 fn sub(a, b int): int { return a - b }\n\
                 test \"adds\" { assert add(2, 3) == 5 }",
            )
            .unwrap(),
        )
        .unwrap();
        let go = generate_tests(&prog, true, 80.0).unwrap();
        assert!(go.contains("func __test_0() {"));
        assert!(go.contains("__cov(\"add\")"));
        assert!(go.contains("allFns := []string{\"add\", \"sub\"}"));
        assert!(go.contains("coverage:"));
        assert!(go.contains("if pct < 80.0 {")); // gate
        assert!(go.contains("os.Exit(2)"));
    }

    #[test]
    fn generates_shapes_example() {
        let src = include_str!("../examples/shapes.vd");
        let prog = parser::parse(lexer::tokenize(src).unwrap()).unwrap();
        let go = generate(&prog).unwrap();
        assert!(go.contains("switch __v := s.(type) {"));
        assert!(go.contains("Circle{radius: 2.0}"));
    }
}
