//! Type checker (Phase 1, increment 1).
//!
//! Builds the symbol table (functions, methods, structs) and validates function
//! bodies. It is **conservative**: anything it does not yet model (generics,
//! channels, external types) becomes `Unknown` and produces no error — avoiding
//! false positives.
//!
//! Errors carry the position (line:column) of the expression involved.

use std::collections::{HashMap, HashSet};

use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub struct TypeError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

/// Resolved type used internally by the checker.
#[derive(Debug, Clone, PartialEq)]
enum Ty {
    Int,
    Float,
    Bool,
    String,
    Error,
    Nil,
    Void,
    Struct(String),
    Enum(String),
    Slice(Box<Ty>),
    Chan(Box<Ty>),
    /// Could not be determined (generic, external type). Suppresses errors.
    Unknown,
}

#[derive(Clone)]
struct FnSig {
    params: Vec<Ty>,
    returns: Vec<Ty>,
}

pub struct Checker {
    functions: HashMap<String, FnSig>,
    methods: HashMap<(String, String), FnSig>,
    structs: HashMap<String, Vec<(String, Ty)>>,
    enums: HashSet<String>,
    interfaces: HashSet<String>,
    /// type parameter names (generics) — polymorphic, don't error on resolve
    type_params: HashSet<String>,
    /// opaque stdlib handles (DB/Rows/Server/Json/Conn) — resolve without error
    opaque: HashSet<String>,
    /// variant name -> (field types, enum name)
    variant_ctors: HashMap<String, (Vec<Ty>, String)>,
    scopes: Vec<HashMap<String, Ty>>,
    current_returns: Vec<Ty>,
    errors: Vec<TypeError>,
    cur_line: usize,
    cur_col: usize,
}

/// Type-check a program. `Ok(())` if clean, else the list of errors.
pub fn check(program: &Program) -> Result<(), Vec<TypeError>> {
    let mut c = Checker {
        functions: HashMap::new(),
        methods: HashMap::new(),
        structs: HashMap::new(),
        enums: HashSet::new(),
        interfaces: HashSet::new(),
        type_params: HashSet::new(),
        opaque: ["DB", "Rows", "Server", "Json", "Conn", "Arena", "Router", "Stmt"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        variant_ctors: HashMap::new(),
        scopes: Vec::new(),
        current_returns: Vec::new(),
        errors: Vec::new(),
        cur_line: 0,
        cur_col: 0,
    };
    c.run(program);
    if c.errors.is_empty() {
        Ok(())
    } else {
        // remove identical errors (e.g. unknown type seen in the sig and the body)
        let mut seen = HashSet::new();
        c.errors
            .retain(|e| seen.insert((e.line, e.col, e.message.clone())));
        Err(c.errors)
    }
}

impl Checker {
    fn run(&mut self, program: &Program) {
        // Pass 0: interface names + union of ALL the program's type params
        // (so `resolve` can distinguish "unknown type" from "generic parameter").
        for item in &program.items {
            let tps = match item {
                Item::Function(f) => &f.type_params,
                Item::Struct(s) => &s.type_params,
                Item::Interface(i) => &i.type_params,
                Item::Enum(e) => &e.type_params,
                _ => continue,
            };
            for tp in tps {
                self.type_params.insert(tp.name.clone());
            }
            if let Item::Interface(i) = item {
                self.interfaces.insert(i.name.clone());
            }
        }

        // Pass 1a: register struct names (so cross-references resolve).
        for item in &program.items {
            if let Item::Struct(s) = item {
                if self.structs.contains_key(&s.name) {
                    self.error(format!("duplicate struct `{}`", s.name));
                }
                self.structs.insert(s.name.clone(), Vec::new());
            }
        }
        // Pass 1b: fill struct fields.
        for item in &program.items {
            if let Item::Struct(s) = item {
                let mut fields = Vec::new();
                for f in &s.fields {
                    let ty = self.resolve(&f.ty);
                    fields.push((f.name.clone(), ty));
                }
                self.structs.insert(s.name.clone(), fields);
            }
        }
        // Pass 1c: register enums and their variant constructors.
        for item in &program.items {
            if let Item::Enum(e) = item {
                self.enums.insert(e.name.clone());
            }
        }
        for item in &program.items {
            if let Item::Enum(e) = item {
                for v in &e.variants {
                    let mut ptys = Vec::new();
                    for f in &v.fields {
                        ptys.push(self.resolve(&f.ty));
                    }
                    self.variant_ctors
                        .insert(v.name.clone(), (ptys, e.name.clone()));
                }
            }
        }
        // Pass 2: register function / method signatures.
        for item in &program.items {
            if let Item::Function(f) = item {
                let mut params = Vec::new();
                for p in &f.params {
                    params.push(self.resolve(&p.ty));
                }
                let mut returns = Vec::new();
                for t in &f.returns {
                    returns.push(self.resolve(t));
                }
                let sig = FnSig { params, returns };
                match &f.receiver {
                    Some(recv) => {
                        if let Ty::Struct(s) = self.resolve(&recv.ty) {
                            self.methods.insert((s, f.name.clone()), sig);
                        }
                    }
                    None => {
                        if self.functions.contains_key(&f.name) {
                            self.error(format!("duplicate function `{}`", f.name));
                        }
                        self.functions.insert(f.name.clone(), sig);
                    }
                }
            }
        }
        // std/db: registers the driver's intrinsic functions when the project imports it.
        // DB/Rows are opaque handles (resolve to Unknown, which is lenient).
        if program.imports.iter().any(|i| i.starts_with("std/db")) {
            use Ty::*;
            let sigs: [(&str, Vec<Ty>, Vec<Ty>); 15] = [
                ("open", vec![String], vec![Unknown]),
                ("exec", vec![Unknown, String], vec![Error]),
                ("must", vec![Unknown, String], vec![]),
                ("query", vec![Unknown, String], vec![Unknown]),
                ("next", vec![Unknown], vec![Bool]),
                ("col_int", vec![Unknown, Int], vec![Int]),
                ("col_text", vec![Unknown, Int], vec![String]),
                ("col_float", vec![Unknown, Int], vec![Float]),
                ("close", vec![Unknown], vec![]),
                ("prepare", vec![Unknown, String], vec![Unknown]),
                ("bind_str", vec![Unknown, String], vec![]),
                ("bind_int", vec![Unknown, Int], vec![]),
                ("bind_float", vec![Unknown, Float], vec![]),
                ("run", vec![Unknown], vec![Error]),
                ("query_stmt", vec![Unknown], vec![Unknown]),
            ];
            for (name, params, returns) in sigs {
                self.functions
                    .entry(name.to_string())
                    .or_insert(FnSig { params, returns });
            }
        }

        // std/http: server (Server -> Unknown) + client.
        if program.imports.iter().any(|i| i.starts_with("std/http")) {
            use Ty::*;
            let sigs: [(&str, Vec<Ty>, Vec<Ty>); 11] = [
                ("listen", vec![Int], vec![Unknown]),
                ("accept", vec![Unknown], vec![Bool]),
                ("method", vec![Unknown], vec![String]),
                ("path", vec![Unknown], vec![String]),
                ("body", vec![Unknown], vec![String]),
                ("header", vec![Unknown, String], vec![String]),
                ("respond", vec![Unknown, Int, String, String], vec![]),
                ("get", vec![String], vec![String]),
                ("post", vec![String, String, String], vec![String]),
                ("newRouter", vec![], vec![Unknown]),
                ("serve", vec![Int, Unknown], vec![]),
            ];
            for (name, params, returns) in sigs {
                self.functions
                    .entry(name.to_string())
                    .or_insert(FnSig { params, returns });
            }
        }

        // std/json: parse + accessors + builder + encode (Json -> Unknown).
        if program.imports.iter().any(|i| i.starts_with("std/json")) {
            use Ty::*;
            let sigs: [(&str, Vec<Ty>, Vec<Ty>); 19] = [
                ("parse", vec![String], vec![Unknown]),
                ("field", vec![Unknown, String], vec![Unknown]),
                ("elem", vec![Unknown, Int], vec![Unknown]),
                ("as_str", vec![Unknown], vec![String]),
                ("as_int", vec![Unknown], vec![Int]),
                ("as_float", vec![Unknown], vec![Float]),
                ("as_bool", vec![Unknown], vec![Bool]),
                ("count", vec![Unknown], vec![Int]),
                ("object", vec![], vec![Unknown]),
                ("array", vec![], vec![Unknown]),
                ("set", vec![Unknown, String, Unknown], vec![Unknown]),
                ("set_str", vec![Unknown, String, String], vec![Unknown]),
                ("set_int", vec![Unknown, String, Int], vec![Unknown]),
                ("set_float", vec![Unknown, String, Float], vec![Unknown]),
                ("set_bool", vec![Unknown, String, Bool], vec![Unknown]),
                ("add", vec![Unknown, Unknown], vec![Unknown]),
                ("add_str", vec![Unknown, String], vec![Unknown]),
                ("add_int", vec![Unknown, Int], vec![Unknown]),
                ("encode", vec![Unknown], vec![String]),
            ];
            for (name, params, returns) in sigs {
                self.functions
                    .entry(name.to_string())
                    .or_insert(FnSig { params, returns });
            }
        }

        // std/env: reading environment variables.
        if program.imports.iter().any(|i| i.starts_with("std/env")) {
            self.functions.entry("read".to_string()).or_insert(FnSig {
                params: vec![Ty::String],
                returns: vec![Ty::String],
            });
        }

        // std/mem: arena/region (Arena -> opaque Unknown).
        if program.imports.iter().any(|i| i.starts_with("std/mem")) {
            use Ty::*;
            let sigs: [(&str, Vec<Ty>, Vec<Ty>); 2] = [
                ("scope", vec![], vec![Unknown]),
                ("release", vec![Unknown], vec![]),
            ];
            for (name, params, returns) in sigs {
                self.functions
                    .entry(name.to_string())
                    .or_insert(FnSig { params, returns });
            }
        }

        // Pass 3: check bodies.
        for item in &program.items {
            match item {
                Item::Function(f) => self.check_function(f),
                Item::Test(t) => {
                    self.current_returns = Vec::new();
                    self.check_block(&t.body);
                }
                _ => {}
            }
        }
    }

    fn resolve(&mut self, t: &Type) -> Ty {
        match t {
            Type::Named(n) => match n.as_str() {
                "int" => Ty::Int,
                "float" => Ty::Float,
                "bool" => Ty::Bool,
                "string" => Ty::String,
                "error" => Ty::Error,
                _ => {
                    if self.structs.contains_key(n) {
                        Ty::Struct(n.clone())
                    } else if self.enums.contains(n) {
                        Ty::Enum(n.clone())
                    } else if self.interfaces.contains(n)
                        || self.opaque.contains(n)
                        || self.type_params.contains(n)
                    {
                        Ty::Unknown // interface / opaque handle / generic param: polymorphic
                    } else {
                        self.error(format!("unknown type `{}`", n));
                        Ty::Unknown
                    }
                }
            },
            Type::Slice(inner) => Ty::Slice(Box::new(self.resolve(inner))),
            Type::Generic(name, args) if name == "chan" && args.len() == 1 => {
                Ty::Chan(Box::new(self.resolve(&args[0])))
            }
            Type::Generic(..) => Ty::Unknown,
        }
    }

    /// Converts an expression that represents a type (e.g. `int` in `chan[int]`).
    fn type_from_expr(&mut self, e: &Expr) -> Ty {
        match &e.kind {
            ExprKind::Ident(n) => self.resolve(&Type::Named(n.clone())),
            _ => Ty::Unknown,
        }
    }

    // ---- scopes ----

    fn declare(&mut self, name: &str, t: Ty) {
        self.scopes.last_mut().unwrap().insert(name.to_string(), t);
    }

    fn lookup(&self, name: &str) -> Option<Ty> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) {
                return Some(t.clone());
            }
        }
        None
    }

    fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(TypeError {
            message: msg.into(),
            line: self.cur_line,
            col: self.cur_col,
        });
    }

    // ---- type relations ----

    fn assignable(&self, from: &Ty, to: &Ty) -> bool {
        if matches!(from, Ty::Unknown) || matches!(to, Ty::Unknown) {
            return true;
        }
        if from == to {
            return true;
        }
        if matches!(from, Ty::Nil) {
            return matches!(
                to,
                Ty::Error | Ty::Struct(_) | Ty::Enum(_) | Ty::Slice(_) | Ty::Chan(_)
            );
        }
        // slices/channels compare by element (lets generic `[]T` pass)
        if let (Ty::Slice(a), Ty::Slice(b)) = (from, to) {
            return self.assignable(a, b);
        }
        if let (Ty::Chan(a), Ty::Chan(b)) = (from, to) {
            return self.assignable(a, b);
        }
        false
    }

    fn comparable(&self, a: &Ty, b: &Ty) -> bool {
        if matches!(a, Ty::Unknown) || matches!(b, Ty::Unknown) {
            return true;
        }
        if a == b {
            return true;
        }
        let nilable =
            |t: &Ty| matches!(t, Ty::Error | Ty::Struct(_) | Ty::Enum(_) | Ty::Slice(_) | Ty::Nil);
        (matches!(a, Ty::Nil) && nilable(b)) || (matches!(b, Ty::Nil) && nilable(a))
    }

    fn numeric_match(&self, a: &Ty, b: &Ty) -> bool {
        matches!(a, Ty::Unknown) || matches!(b, Ty::Unknown) || (a == b && matches!(a, Ty::Int | Ty::Float))
    }

    fn ty_name(&self, t: &Ty) -> String {
        match t {
            Ty::Int => "int".into(),
            Ty::Float => "float".into(),
            Ty::Bool => "bool".into(),
            Ty::String => "string".into(),
            Ty::Error => "error".into(),
            Ty::Nil => "nil".into(),
            Ty::Void => "void".into(),
            Ty::Struct(s) => s.clone(),
            Ty::Enum(s) => s.clone(),
            Ty::Slice(t) => format!("[]{}", self.ty_name(t)),
            Ty::Chan(t) => format!("chan[{}]", self.ty_name(t)),
            Ty::Unknown => "?".into(),
        }
    }

    // ---- functions / statements ----

    fn check_function(&mut self, f: &Function) {
        self.scopes.push(HashMap::new());
        if let Some(recv) = &f.receiver {
            let t = self.resolve(&recv.ty);
            self.declare(&recv.name, t);
        }
        for p in &f.params {
            let t = self.resolve(&p.ty);
            self.declare(&p.name, t);
        }
        let mut crets = Vec::new();
        for t in &f.returns {
            crets.push(self.resolve(t));
        }
        self.current_returns = crets;
        self.check_block(&f.body);
        self.scopes.pop();
    }

    fn check_block(&mut self, b: &Block) {
        self.scopes.push(HashMap::new());
        for s in &b.stmts {
            self.check_stmt(s);
        }
        self.scopes.pop();
    }

    fn check_stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::VarDecl { decls, values, .. } => self.check_var_decl(decls, values),
            Stmt::Assign { target, value } => {
                if !matches!(
                    target.kind,
                    ExprKind::Ident(_) | ExprKind::Field { .. } | ExprKind::Index { .. }
                ) {
                    self.error("invalid assignment target");
                }
                let tt = self.infer(target);
                let vt = self.infer(value);
                if !self.assignable(&vt, &tt) {
                    self.error(format!(
                        "cannot assign `{}` to `{}`",
                        self.ty_name(&vt),
                        self.ty_name(&tt)
                    ));
                }
            }
            Stmt::Return(values) => self.check_return(values),
            Stmt::If {
                cond,
                then_block,
                else_block,
            } => {
                let ct = self.infer(cond);
                if !matches!(ct, Ty::Bool | Ty::Unknown) {
                    self.error(format!(
                        "if condition must be bool, found `{}`",
                        self.ty_name(&ct)
                    ));
                }
                self.check_block(then_block);
                if let Some(eb) = else_block {
                    self.check_block(eb);
                }
            }
            Stmt::For { head, body } => {
                self.scopes.push(HashMap::new());
                match head {
                    ForHead::Infinite => {}
                    ForHead::While(c) => {
                        let ct = self.infer(c);
                        if !matches!(ct, Ty::Bool | Ty::Unknown) {
                            self.error(format!(
                                "for condition must be bool, found `{}`",
                                self.ty_name(&ct)
                            ));
                        }
                    }
                    ForHead::In { var, iter } => {
                        let it = self.infer(iter);
                        let vt = match &iter.kind {
                            ExprKind::Binary {
                                op: BinOp::Range | BinOp::RangeIncl,
                                ..
                            } => Ty::Int,
                            _ => match it {
                                Ty::Slice(inner) => *inner,
                                Ty::Chan(e) => *e,
                                _ => Ty::Unknown,
                            },
                        };
                        self.declare(var, vt);
                    }
                }
                for st in &body.stmts {
                    self.check_stmt(st);
                }
                self.scopes.pop();
            }
            Stmt::Spawn(call) => {
                self.infer(call);
            }
            Stmt::Send { chan, value } => {
                let ct = self.infer(chan);
                let vt = self.infer(value);
                if let Ty::Chan(elem) = ct {
                    if !self.assignable(&vt, &elem) {
                        self.error(format!(
                            "cannot send `{}` on a `chan[{}]`",
                            self.ty_name(&vt),
                            self.ty_name(&elem)
                        ));
                    }
                }
            }
            Stmt::Assert(e) => {
                let t = self.infer(e);
                if !matches!(t, Ty::Bool | Ty::Unknown) {
                    self.error(format!("assert requires bool, found `{}`", self.ty_name(&t)));
                }
            }
            Stmt::Expr(e) => {
                self.infer(e);
            }
        }
    }

    fn check_var_decl(&mut self, decls: &[Param], values: &[Expr]) {
        let decl_tys: Vec<Ty> = decls.iter().map(|d| self.resolve(&d.ty)).collect();

        if decls.len() == values.len() {
            for (i, v) in values.iter().enumerate() {
                let vt = self.infer(v);
                if !self.assignable(&vt, &decl_tys[i]) {
                    self.error(format!(
                        "`{}`: expected `{}`, found `{}`",
                        decls[i].name,
                        self.ty_name(&decl_tys[i]),
                        self.ty_name(&vt)
                    ));
                }
            }
        } else if values.len() == 1 && decls.len() > 1 {
            if let Some(rets) = self.call_return_types(&values[0]) {
                self.infer(&values[0]); // surface arg errors
                if rets.len() == decls.len() {
                    for (i, rt) in rets.iter().enumerate() {
                        if !self.assignable(rt, &decl_tys[i]) {
                            self.error(format!(
                                "`{}`: expected `{}`, found `{}`",
                                decls[i].name,
                                self.ty_name(&decl_tys[i]),
                                self.ty_name(rt)
                            ));
                        }
                    }
                } else {
                    self.error(format!(
                        "declared {} variables but the call returns {}",
                        decls.len(),
                        rets.len()
                    ));
                }
            } else {
                self.infer(&values[0]);
            }
        } else {
            self.error(format!(
                "declared {} variable(s) but provided {} value(s)",
                decls.len(),
                values.len()
            ));
            for v in values {
                self.infer(v);
            }
        }

        for (i, d) in decls.iter().enumerate() {
            self.declare(&d.name, decl_tys[i].clone());
        }
    }

    fn check_return(&mut self, values: &[Expr]) {
        let rets = self.current_returns.clone();
        if values.len() == rets.len() {
            for (i, v) in values.iter().enumerate() {
                let vt = self.infer(v);
                if !self.assignable(&vt, &rets[i]) {
                    self.error(format!(
                        "return value {}: expected `{}`, found `{}`",
                        i + 1,
                        self.ty_name(&rets[i]),
                        self.ty_name(&vt)
                    ));
                }
            }
        } else if values.len() == 1 && rets.len() > 1 {
            if let Some(callrets) = self.call_return_types(&values[0]) {
                self.infer(&values[0]);
                if callrets.len() != rets.len() {
                    self.error(format!(
                        "function returns {} values but the call provides {}",
                        rets.len(),
                        callrets.len()
                    ));
                } else {
                    for (i, rt) in callrets.iter().enumerate() {
                        if !self.assignable(rt, &rets[i]) {
                            self.error(format!(
                                "return value {}: expected `{}`, found `{}`",
                                i + 1,
                                self.ty_name(&rets[i]),
                                self.ty_name(rt)
                            ));
                        }
                    }
                }
            } else {
                self.infer(&values[0]);
            }
        } else if !(values.is_empty() && rets.is_empty()) {
            self.error(format!(
                "function returns {} value(s) but {} were provided",
                rets.len(),
                values.len()
            ));
            for v in values {
                self.infer(v);
            }
        }
    }

    fn call_return_types(&self, expr: &Expr) -> Option<Vec<Ty>> {
        if let ExprKind::Call { callee, .. } = &expr.kind {
            if let ExprKind::Ident(name) = &callee.kind {
                if let Some(sig) = self.functions.get(name) {
                    return Some(sig.returns.clone());
                }
            }
        }
        None
    }

    // ---- expressions ----

    fn infer(&mut self, e: &Expr) -> Ty {
        self.cur_line = e.line;
        self.cur_col = e.col;
        match &e.kind {
            ExprKind::Int(_) => Ty::Int,
            ExprKind::Float(_) => Ty::Float,
            ExprKind::Str(_) => Ty::String,
            ExprKind::Bool(_) => Ty::Bool,
            ExprKind::Nil => Ty::Nil,
            ExprKind::Ident(name) => {
                if let Some(t) = self.lookup(name) {
                    t
                } else if self.functions.contains_key(name) {
                    Ty::Unknown
                } else {
                    self.error(format!("undeclared variable `{}`", name));
                    Ty::Unknown
                }
            }
            ExprKind::Unary { op, expr } => {
                let t = self.infer(expr);
                match op {
                    UnOp::Neg => {
                        if !matches!(t, Ty::Int | Ty::Float | Ty::Unknown) {
                            self.error(format!("cannot negate `{}`", self.ty_name(&t)));
                            return Ty::Unknown;
                        }
                        t
                    }
                    UnOp::Not => {
                        if !matches!(t, Ty::Bool | Ty::Unknown) {
                            self.error(format!("`!` requires bool, found `{}`", self.ty_name(&t)));
                        }
                        Ty::Bool
                    }
                }
            }
            ExprKind::Binary { op, left, right } => self.infer_binary(op, left, right),
            ExprKind::Call { callee, args } => self.infer_call(callee, args),
            ExprKind::Field { base, field } => {
                let b = self.infer(base);
                self.infer_field(&b, field)
            }
            ExprKind::Index { base, index } => {
                self.infer(index);
                let b = self.infer(base);
                match b {
                    Ty::Slice(t) => *t,
                    Ty::Unknown => Ty::Unknown,
                    other => {
                        self.error(format!("cannot index `{}`", self.ty_name(&other)));
                        Ty::Unknown
                    }
                }
            }
            ExprKind::StructLit { name, fields } => self.infer_struct_lit(name, fields),
            ExprKind::SliceLit(elems) => {
                let mut et = Ty::Unknown;
                for el in elems {
                    let t = self.infer(el);
                    if matches!(et, Ty::Unknown) {
                        et = t;
                    }
                }
                Ty::Slice(Box::new(et))
            }
            ExprKind::Recv(inner) => match self.infer(inner) {
                Ty::Chan(e) => *e,
                _ => Ty::Unknown,
            },
            ExprKind::Match { scrutinee, arms } => {
                self.infer(scrutinee);
                let mut result = Ty::Unknown;
                for arm in arms {
                    self.scopes.push(HashMap::new());
                    for p in &arm.patterns {
                        self.declare_pattern_bindings(p);
                    }
                    if let Some(g) = &arm.guard {
                        let gt = self.infer(g);
                        if !matches!(gt, Ty::Bool | Ty::Unknown) {
                            self.error("match guard must be bool");
                        }
                    }
                    let bt = match &arm.body {
                        MatchArmBody::Expr(ex) => self.infer(ex),
                        MatchArmBody::Block(b) => {
                            for st in &b.stmts {
                                self.check_stmt(st);
                            }
                            Ty::Void
                        }
                    };
                    self.scopes.pop();
                    if matches!(result, Ty::Unknown) {
                        result = bt;
                    }
                }
                result
            }
        }
    }

    fn declare_pattern_bindings(&mut self, p: &Pattern) {
        match p {
            Pattern::Variant { bindings, .. } => {
                for b in bindings {
                    self.declare(b, Ty::Unknown);
                }
            }
            Pattern::Binding(n) => self.declare(n, Ty::Unknown),
            _ => {}
        }
    }

    fn infer_binary(&mut self, op: &BinOp, l: &Expr, r: &Expr) -> Ty {
        let lt = self.infer(l);
        let rt = self.infer(r);
        use BinOp::*;
        match op {
            Range | RangeIncl => {
                if !matches!(lt, Ty::Int | Ty::Unknown) || !matches!(rt, Ty::Int | Ty::Unknown) {
                    self.error("range bounds must be int");
                }
                Ty::Unknown
            }
            And | Or => {
                if !matches!(lt, Ty::Bool | Ty::Unknown) || !matches!(rt, Ty::Bool | Ty::Unknown) {
                    self.error("logical operator requires bool operands");
                }
                Ty::Bool
            }
            Eq | NotEq => {
                if !self.comparable(&lt, &rt) {
                    self.error(format!(
                        "cannot compare `{}` and `{}`",
                        self.ty_name(&lt),
                        self.ty_name(&rt)
                    ));
                }
                Ty::Bool
            }
            Lt | LtEq | Gt | GtEq => {
                if !self.numeric_match(&lt, &rt) {
                    self.error(format!(
                        "cannot order `{}` and `{}`",
                        self.ty_name(&lt),
                        self.ty_name(&rt)
                    ));
                }
                Ty::Bool
            }
            Add => {
                if matches!(lt, Ty::Unknown) || matches!(rt, Ty::Unknown) {
                    Ty::Unknown
                } else if lt == Ty::String && rt == Ty::String {
                    Ty::String
                } else if lt == Ty::Int && rt == Ty::Int {
                    Ty::Int
                } else if lt == Ty::Float && rt == Ty::Float {
                    Ty::Float
                } else {
                    self.error(format!(
                        "cannot add `{}` and `{}`",
                        self.ty_name(&lt),
                        self.ty_name(&rt)
                    ));
                    Ty::Unknown
                }
            }
            Sub | Mul | Div | Rem => {
                if matches!(lt, Ty::Unknown) || matches!(rt, Ty::Unknown) {
                    Ty::Unknown
                } else if lt == Ty::Int && rt == Ty::Int {
                    Ty::Int
                } else if lt == Ty::Float && rt == Ty::Float {
                    Ty::Float
                } else {
                    self.error(format!(
                        "arithmetic requires matching numeric types, found `{}` and `{}`",
                        self.ty_name(&lt),
                        self.ty_name(&rt)
                    ));
                    Ty::Unknown
                }
            }
        }
    }

    fn infer_call(&mut self, callee: &Expr, args: &[Expr]) -> Ty {
        // channel creation: chan[T](buffer)
        let chan_elem = if let ExprKind::Index { base, index } = &callee.kind {
            match &base.kind {
                ExprKind::Ident(n) if n == "chan" => Some(self.type_from_expr(index)),
                _ => None,
            }
        } else {
            None
        };
        if let Some(elem) = chan_elem {
            for a in args {
                self.infer(a);
            }
            return Ty::Chan(Box::new(elem));
        }

        let arg_tys: Vec<Ty> = args.iter().map(|a| self.infer(a)).collect();

        if let ExprKind::Ident(name) = &callee.kind {
            match name.as_str() {
                "print" | "close" => return Ty::Void,
                "len" => return Ty::Int,
                "newmap" => return Ty::Unknown,
                "append" => return Ty::Unknown,
                "error" => return Ty::Error,
                _ => {}
            }
            if let Some((ptys, enum_name)) = self.variant_ctors.get(name).cloned() {
                if ptys.len() != args.len() {
                    self.error(format!(
                        "variant `{}` expects {} field(s), got {}",
                        name,
                        ptys.len(),
                        args.len()
                    ));
                } else {
                    for (i, (at, pt)) in arg_tys.iter().zip(ptys.iter()).enumerate() {
                        if !self.assignable(at, pt) {
                            self.error(format!(
                                "field {} of `{}`: expected `{}`, found `{}`",
                                i + 1,
                                name,
                                self.ty_name(pt),
                                self.ty_name(at)
                            ));
                        }
                    }
                }
                return Ty::Enum(enum_name);
            }
            if let Some(sig) = self.functions.get(name).cloned() {
                if sig.params.len() != args.len() {
                    self.error(format!(
                        "function `{}` expects {} argument(s), got {}",
                        name,
                        sig.params.len(),
                        args.len()
                    ));
                } else {
                    for (i, (at, pt)) in arg_tys.iter().zip(sig.params.iter()).enumerate() {
                        if !self.assignable(at, pt) {
                            self.error(format!(
                                "argument {} of `{}`: expected `{}`, found `{}`",
                                i + 1,
                                name,
                                self.ty_name(pt),
                                self.ty_name(at)
                            ));
                        }
                    }
                }
                return match sig.returns.len() {
                    0 => Ty::Void,
                    1 => sig.returns[0].clone(),
                    _ => Ty::Unknown, // multiple: handled in var-decl/return
                };
            }
            self.error(format!("undeclared function `{}`", name));
            return Ty::Unknown;
        }

        // method or complex callee: infer leniently
        self.infer(callee);
        Ty::Unknown
    }

    fn infer_field(&mut self, base: &Ty, field: &str) -> Ty {
        match base {
            Ty::Struct(s) => {
                if let Some(fields) = self.structs.get(s) {
                    if let Some((_, t)) = fields.iter().find(|(n, _)| n == field) {
                        return t.clone();
                    }
                }
                if self.methods.contains_key(&(s.clone(), field.to_string())) {
                    return Ty::Unknown; // method access; will be called
                }
                self.error(format!("struct `{}` has no field `{}`", s, field));
                Ty::Unknown
            }
            Ty::Unknown => Ty::Unknown,
            other => {
                self.error(format!(
                    "cannot access field `{}` on `{}`",
                    field,
                    self.ty_name(other)
                ));
                Ty::Unknown
            }
        }
    }

    fn infer_struct_lit(&mut self, name: &str, fields: &[(String, Expr)]) -> Ty {
        if let Some(def) = self.structs.get(name).cloned() {
            for (fname, fexpr) in fields {
                let ft = self.infer(fexpr);
                if let Some((_, expected)) = def.iter().find(|(n, _)| n == fname) {
                    if !self.assignable(&ft, expected) {
                        self.error(format!(
                            "field `{}` of `{}`: expected `{}`, found `{}`",
                            fname,
                            name,
                            self.ty_name(expected),
                            self.ty_name(&ft)
                        ));
                    }
                } else {
                    self.error(format!("struct `{}` has no field `{}`", name, fname));
                }
            }
            Ty::Struct(name.to_string())
        } else {
            for (_, fexpr) in fields {
                self.infer(fexpr);
            }
            Ty::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, parser};

    fn errors(src: &str) -> Vec<TypeError> {
        let prog = parser::parse(lexer::tokenize(src).unwrap()).unwrap();
        match check(&prog) {
            Ok(()) => Vec::new(),
            Err(e) => e,
        }
    }

    fn ok(src: &str) {
        let errs = errors(src);
        assert!(errs.is_empty(), "expected no type errors, got: {:?}", errs);
    }

    fn fails(src: &str) {
        assert!(!errors(src).is_empty(), "expected a type error but found none");
    }

    #[test]
    fn clean_program_ok() {
        ok("fn add(a, b int): int { return a + b }");
    }

    #[test]
    fn rejects_unknown_type() {
        fails("fn f() { Foo x = nil }"); // unknown local type
        fails("fn g(x Bar) { }"); // unknown type in signature
    }

    #[test]
    fn generic_and_interface_types_ok() {
        ok("fn id[T](x T): T { return x }"); // type param is polymorphic
        ok("interface Shape { fn area(): float }\nfn f(s Shape) { }"); // interface resolves
    }

    #[test]
    fn stdlib_opaque_types_ok() {
        ok("fn f(d DB, s Server, j Json) { }"); // opaque stdlib handles
    }

    #[test]
    fn undeclared_variable() {
        fails("fn f() { int x = y }");
    }

    #[test]
    fn type_mismatch_in_decl() {
        fails("fn f() { int x = \"text\" }");
    }

    #[test]
    fn string_concat_ok() {
        ok("fn f(): string { string a = \"x\" + \"y\"  return a }");
    }

    #[test]
    fn add_int_and_string_fails() {
        fails("fn f() { int x = 1 + \"y\" }");
    }

    #[test]
    fn if_condition_must_be_bool() {
        fails("fn f() { if 1 { } }");
    }

    #[test]
    fn if_condition_bool_ok() {
        ok("fn f(x int) { if x > 0 { } }");
    }

    #[test]
    fn call_arity_mismatch() {
        fails("fn g(a int): int { return a }\n fn f() { int x = g(1, 2) }");
    }

    #[test]
    fn call_arg_type_mismatch() {
        fails("fn g(a int): int { return a }\n fn f() { int x = g(\"s\") }");
    }

    #[test]
    fn multi_return_decl_ok() {
        ok("fn d(a, b int): (int, error) { return a, nil }\n fn f() { int r, error e = d(1, 2) }");
    }

    #[test]
    fn return_arity_mismatch() {
        fails("fn f(): int { return }");
    }

    #[test]
    fn field_access_ok() {
        ok("struct U { name string }\n fn f(u U): string { return u.name }");
    }

    #[test]
    fn field_access_unknown_fails() {
        fails("struct U { name string }\n fn f(u U): string { return u.nope }");
    }

    #[test]
    fn errors_carry_line_and_column() {
        let prog = parser::parse(
            lexer::tokenize("fn f() {\n    int x = \"text\"\n}").unwrap(),
        )
        .unwrap();
        let errs = check(&prog).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line, 2); // the wrong value is on line 2
    }

    #[test]
    fn duplicate_function_is_an_error() {
        fails("fn f() {}\n fn f() {}");
    }

    #[test]
    fn duplicate_struct_is_an_error() {
        fails("struct S { x int }\n struct S { y int }");
    }

    #[test]
    fn channels_typecheck() {
        let src = include_str!("../examples/concurrency.vd");
        let prog = parser::parse(lexer::tokenize(src).unwrap()).unwrap();
        assert!(
            check(&prog).is_ok(),
            "concurrency.vd should type-check: {:?}",
            check(&prog).err()
        );
    }

    #[test]
    fn checks_basics_example() {
        let src = include_str!("../examples/basics.vd");
        let prog = parser::parse(lexer::tokenize(src).unwrap()).unwrap();
        assert!(
            check(&prog).is_ok(),
            "basics.vd should type-check, got: {:?}",
            check(&prog).err()
        );
    }
}
