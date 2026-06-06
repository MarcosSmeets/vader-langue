//! Backend LLVM (Fase 3): transpila a AST para **LLVM IR em texto** (`.ll`),
//! compilado pelo `clang` para binário nativo — sem Go.
//!
//! Cobertura (subset sequencial): funções, métodos, multi-retorno; tipos int/bool/
//! float/string/error/struct; aritmética/comparação/lógicos; if/for; recursão;
//! strings (literal/concat/comparação) e `print` com vários argumentos.
//!
//! Ainda NÃO suportado (precisa de runtime ou é o próximo passo): canais/goroutines,
//! maps, slices, interfaces (vtables), genéricos (monomorfização), enum/match.
//! Memória de strings/structs vaza (sem GC) — alinhado com a visão sem-GC.
//!
//! Locais usam alloca/load/store; tudo nomeado (`%tN`, `bbN`).

use std::collections::{HashMap, HashSet};

use crate::ast::*;

struct Gen {
    out: String,
    globals: String,
    tmp: usize,
    label: usize,
    str_count: usize,
    terminated: bool,
    cur_ret: String,
    vars: HashMap<String, (String, String)>, // nome -> (alloca, lltype)
    structs: HashMap<String, Vec<(String, String)>>, // struct -> [(campo, lltype)]
    enums: HashMap<String, ()>,                       // nomes de enums
    variants: HashMap<String, (String, usize, Vec<(String, String)>)>, // variante -> (enum, tag, campos)
    funcs: HashMap<String, (Vec<String>, String)>,           // função -> (params, ret)
    methods: HashMap<(String, String), (Vec<String>, String)>, // (struct, método) -> (params, ret)
    /// interface -> [(método, tipos dos params, ret)]
    interfaces: HashMap<String, Vec<(String, Vec<String>, String)>>,
    /// funções genéricas (templates), por nome
    generics: HashMap<String, Function>,
    /// substituição de type params ativa durante a geração de uma instância
    type_subst: HashMap<String, String>,
    /// instâncias já geradas (nomes mangled)
    mono_done: HashSet<String>,
    /// instâncias a gerar: (nome mangled, template, substituição)
    pending: Vec<(String, Function, HashMap<String, String>)>,
    /// variáveis de canal -> tipo LLVM do elemento (canal em si é i8* opaco)
    chan_elems: HashMap<String, String>,
    /// variáveis de map -> (chave é string?, tipo LLVM do valor)
    map_types: HashMap<String, (bool, String)>,
    /// thunks de goroutine a gerar: (nome, função alvo, tipos dos args)
    pending_thunks: Vec<(String, String, Vec<String>)>,
    thunk_count: usize,
    /// projeto importa `std/db`? (habilita o despacho das intrínsecas do driver)
    has_db: bool,
    has_http: bool,
    has_json: bool,
    needs: Needs,
}

#[derive(Default)]
struct Needs {
    printf: bool,
    puts: bool,
    strcat: bool,
    strcmp: bool,
    malloc: bool,
    chan: bool,
    map: bool,
    db: bool,
    http: bool,
    json: bool,
}

enum MatchMode {
    Return,
    Assign(String, String), // (alloca, lltype)
    Stmt,
}

/// Gera LLVM IR (texto) a partir de um programa Vader já checado.
pub fn generate(program: &Program) -> Result<String, String> {
    let mut g = Gen {
        out: String::new(),
        globals: String::new(),
        tmp: 0,
        label: 0,
        str_count: 0,
        terminated: false,
        cur_ret: "void".to_string(),
        vars: HashMap::new(),
        structs: HashMap::new(),
        enums: HashMap::new(),
        variants: HashMap::new(),
        funcs: HashMap::new(),
        methods: HashMap::new(),
        interfaces: HashMap::new(),
        generics: HashMap::new(),
        type_subst: HashMap::new(),
        mono_done: HashSet::new(),
        pending: Vec::new(),
        chan_elems: HashMap::new(),
        map_types: HashMap::new(),
        pending_thunks: Vec::new(),
        thunk_count: 0,
        has_db: program.imports.iter().any(|i| i.starts_with("std/db")),
        has_http: program.imports.iter().any(|i| i.starts_with("std/http")),
        has_json: program.imports.iter().any(|i| i.starts_with("std/json")),
        needs: Needs::default(),
    };

    // pré-passo 1: structs (nomes primeiro, depois campos, p/ referências cruzadas)
    for item in &program.items {
        if let Item::Struct(s) = item {
            g.structs.insert(s.name.clone(), Vec::new());
        }
    }
    for item in &program.items {
        if let Item::Struct(s) = item {
            let mut fields = Vec::new();
            for f in &s.fields {
                fields.push((f.name.clone(), g.ty_of(&f.ty)?));
            }
            g.structs.insert(s.name.clone(), fields);
        }
    }

    // enums: registra nomes e variantes (representação tagged-union {i32, i8*})
    for item in &program.items {
        if let Item::Enum(e) = item {
            g.enums.insert(e.name.clone(), ());
            for (tag, v) in e.variants.iter().enumerate() {
                let mut fields = Vec::new();
                for f in &v.fields {
                    fields.push((f.name.clone(), g.ty_of(&f.ty)?));
                }
                g.variants
                    .insert(v.name.clone(), (e.name.clone(), tag, fields));
            }
        }
    }

    // nomes de interfaces (pro ty_of reconhecer já em pré-passo 2)
    for item in &program.items {
        if let Item::Interface(it) = item {
            g.interfaces.insert(it.name.clone(), Vec::new());
        }
    }

    // pré-passo 2: assinaturas de funções, métodos e interfaces (com tipos de params)
    for item in &program.items {
        match item {
            Item::Function(f) => {
                if !f.type_params.is_empty() {
                    // função genérica (template); monomorfizada sob demanda.
                    if f.receiver.is_none() {
                        g.generics.insert(f.name.clone(), f.clone());
                    }
                } else {
                    let ret = g.ret_type(f)?;
                    let mut params = Vec::new();
                    for p in &f.params {
                        params.push(g.ty_of(&p.ty)?);
                    }
                    match &f.receiver {
                        Some(r) => {
                            if let Type::Named(sname) = &r.ty {
                                g.methods
                                    .insert((sname.clone(), f.name.clone()), (params, ret));
                            }
                        }
                        None => {
                            g.funcs.insert(f.name.clone(), (params, ret));
                        }
                    }
                }
            }
            Item::Interface(it) => {
                let mut ms = Vec::new();
                for m in &it.methods {
                    let mut params = Vec::new();
                    for p in &m.params {
                        params.push(g.ty_of(&p.ty)?);
                    }
                    let ret = match m.returns.len() {
                        0 => "void".to_string(),
                        1 => g.ty_of(&m.returns[0])?,
                        _ => {
                            let mut ts = Vec::new();
                            for t in &m.returns {
                                ts.push(g.ty_of(t)?);
                            }
                            format!("{{ {} }}", ts.join(", "))
                        }
                    };
                    ms.push((m.name.clone(), params, ret));
                }
                g.interfaces.insert(it.name.clone(), ms);
            }
            _ => {}
        }
    }

    // tipos LLVM dos structs
    let mut typedefs = String::new();
    for item in &program.items {
        if let Item::Struct(s) = item {
            let tys: Vec<String> = g.structs[&s.name].iter().map(|(_, t)| t.clone()).collect();
            typedefs.push_str(&format!("%{} = type {{ {} }}\n", s.name, tys.join(", ")));
        }
    }
    for item in &program.items {
        if let Item::Enum(e) = item {
            typedefs.push_str(&format!("%{} = type {{ i32, i8* }}\n", e.name));
        }
    }
    for item in &program.items {
        if let Item::Interface(it) = item {
            typedefs.push_str(&format!("%{} = type {{ i8*, i8* }}\n", it.name));
            let fnptrs: Vec<String> = g.interfaces[&it.name]
                .iter()
                .map(|(_, params, ret)| {
                    let mut ptypes = vec!["i8*".to_string()];
                    ptypes.extend(params.clone());
                    format!("{} ({})*", ret, ptypes.join(", "))
                })
                .collect();
            typedefs.push_str(&format!(
                "%vtable.{} = type {{ {} }}\n",
                it.name,
                fnptrs.join(", ")
            ));
        }
    }

    for item in &program.items {
        if let Item::Function(f) = item {
            if f.type_params.is_empty() {
                g.gen_function(f, None)?;
            }
        }
    }
    g.gen_impls(); // shims + vtables das implementações de interface
    // monomorfização: gera as instâncias genéricas agendadas (pode agendar mais)
    while let Some((mangled, gfn, subst)) = g.pending.pop() {
        g.type_subst = subst;
        g.gen_function(&gfn, Some(&mangled))?;
        g.type_subst = HashMap::new();
    }
    // trampolins das goroutines (spawn)
    for (thunk, fname, argtypes) in g.pending_thunks.clone() {
        g.gen_thunk(&thunk, &fname, &argtypes);
    }

    let mut result = String::from("; gerado pela Vader (backend LLVM IR)\n\n");
    result.push_str(&typedefs);
    if !typedefs.is_empty() {
        result.push('\n');
    }
    result.push_str(&g.globals);
    if g.needs.printf {
        result.push_str("declare i32 @printf(i8*, ...)\n");
    }
    if g.needs.puts {
        result.push_str("declare i32 @puts(i8*)\n");
    }
    if g.needs.strcmp {
        result.push_str("declare i32 @strcmp(i8*, i8*)\n");
    }
    if g.needs.malloc || g.needs.strcat {
        result.push_str("declare i8* @malloc(i64)\n");
    }
    if g.needs.strcat {
        result.push_str(STRCAT_IR);
    }
    if g.needs.chan {
        result.push_str(
            "declare i8* @vader_chan_make(i64, i64)\n\
             declare void @vader_chan_send(i8*, i8*)\n\
             declare i32 @vader_chan_recv(i8*, i8*)\n\
             declare void @vader_chan_close(i8*)\n\
             declare void @vader_go(i8* (i8*)*, i8*)\n",
        );
    }
    if g.needs.db {
        result.push_str(
            "declare i8* @vader_db_open(i8*)\n\
             declare i8* @vader_db_exec(i8*, i8*)\n\
             declare void @vader_db_must(i8*, i8*)\n\
             declare i8* @vader_db_query(i8*, i8*)\n\
             declare i32 @vader_db_next(i8*)\n\
             declare i64 @vader_db_col_int(i8*, i32)\n\
             declare double @vader_db_col_float(i8*, i32)\n\
             declare i8* @vader_db_col_text(i8*, i32)\n\
             declare void @vader_db_close(i8*)\n",
        );
    }
    if g.needs.http {
        result.push_str(
            "declare i8* @vader_http_listen(i64)\n\
             declare i32 @vader_http_accept(i8*)\n\
             declare i8* @vader_http_method(i8*)\n\
             declare i8* @vader_http_path(i8*)\n\
             declare i8* @vader_http_body(i8*)\n\
             declare i8* @vader_http_header(i8*, i8*)\n\
             declare void @vader_http_respond(i8*, i64, i8*, i8*)\n\
             declare i8* @vader_http_get(i8*)\n\
             declare i8* @vader_http_post(i8*, i8*, i8*)\n",
        );
    }
    if g.needs.json {
        result.push_str(
            "declare i8* @vader_json_parse(i8*)\n\
             declare i8* @vader_json_field(i8*, i8*)\n\
             declare i8* @vader_json_elem(i8*, i32)\n\
             declare i8* @vader_json_as_str(i8*)\n\
             declare i64 @vader_json_as_int(i8*)\n\
             declare double @vader_json_as_float(i8*)\n\
             declare i32 @vader_json_as_bool(i8*)\n\
             declare i64 @vader_json_count(i8*)\n\
             declare i8* @vader_json_object()\n\
             declare i8* @vader_json_array()\n\
             declare i8* @vader_json_set(i8*, i8*, i8*)\n\
             declare i8* @vader_json_set_str(i8*, i8*, i8*)\n\
             declare i8* @vader_json_set_int(i8*, i8*, i64)\n\
             declare i8* @vader_json_set_float(i8*, i8*, double)\n\
             declare i8* @vader_json_set_bool(i8*, i8*, i32)\n\
             declare i8* @vader_json_add(i8*, i8*)\n\
             declare i8* @vader_json_add_str(i8*, i8*)\n\
             declare i8* @vader_json_add_int(i8*, i64)\n\
             declare i8* @vader_json_encode(i8*)\n",
        );
    }
    if g.needs.map {
        result.push_str(
            "declare i8* @vader_map_make(i64, i32)\n\
             declare void @vader_map_set_int(i8*, i64, i8*)\n\
             declare i32 @vader_map_get_int(i8*, i64, i8*)\n\
             declare void @vader_map_set_str(i8*, i8*, i8*)\n\
             declare i32 @vader_map_get_str(i8*, i8*, i8*)\n\
             declare i64 @vader_map_len(i8*)\n",
        );
    }
    result.push('\n');
    result.push_str(&g.out);
    Ok(result)
}

/// Helper de runtime: concatena duas strings C (vaza memória; sem-GC).
const STRCAT_IR: &str = "declare i64 @strlen(i8*)
declare i8* @strcpy(i8*, i8*)
declare i8* @strcat(i8*, i8*)
define i8* @vader_strcat(i8* %a, i8* %b) {
  %la = call i64 @strlen(i8* %a)
  %lb = call i64 @strlen(i8* %b)
  %s = add i64 %la, %lb
  %sz = add i64 %s, 1
  %buf = call i8* @malloc(i64 %sz)
  %c1 = call i8* @strcpy(i8* %buf, i8* %a)
  %c2 = call i8* @strcat(i8* %buf, i8* %b)
  ret i8* %buf
}
";

impl Gen {
    fn ty_of(&self, t: &Type) -> Result<String, String> {
        match t {
            Type::Named(n) if self.type_subst.contains_key(n) => Ok(self.type_subst[n].clone()),
            Type::Named(n) => Ok(match n.as_str() {
                "int" => "i64".to_string(),
                "bool" => "i1".to_string(),
                "float" => "double".to_string(),
                "string" | "error" => "i8*".to_string(),
                "DB" | "Rows" | "Server" | "Json" => "i8*".to_string(), // handles opacos da stdlib
                other => {
                    if self.structs.contains_key(other)
                        || self.enums.contains_key(other)
                        || self.interfaces.contains_key(other)
                    {
                        format!("%{}", other)
                    } else {
                        return Err(format!("backend LLVM: tipo `{}` não suportado", other));
                    }
                }
            }),
            Type::Slice(inner) => Ok(format!("{{ {}*, i64 }}", self.ty_of(inner)?)),
            Type::Generic(name, _) if name == "chan" || name == "map" => Ok("i8*".to_string()),
            _ => Err("backend LLVM: tipo genérico não suportado".into()),
        }
    }

    /// Extrai o tipo do elemento de um slice `{ T*, i64 }` -> `T`.
    fn slice_elem(bt: &str) -> String {
        bt.trim_start_matches('{')
            .trim()
            .split('*')
            .next()
            .unwrap_or("i8")
            .trim()
            .to_string()
    }

    fn ret_type(&self, f: &Function) -> Result<String, String> {
        if f.receiver.is_none() && f.name == "main" {
            return Ok("i32".to_string());
        }
        match f.returns.len() {
            0 => Ok("void".to_string()),
            1 => self.ty_of(&f.returns[0]),
            _ => {
                let mut tys = Vec::new();
                for t in &f.returns {
                    tys.push(self.ty_of(t)?);
                }
                Ok(format!("{{ {} }}", tys.join(", ")))
            }
        }
    }

    fn fresh(&mut self) -> String {
        self.tmp += 1;
        format!("%t{}", self.tmp)
    }
    fn fresh_label(&mut self, base: &str) -> String {
        self.label += 1;
        format!("{}{}", base, self.label)
    }
    fn emit(&mut self, line: &str) {
        self.out.push_str("  ");
        self.out.push_str(line);
        self.out.push('\n');
    }
    fn label(&mut self, l: &str) {
        self.out.push_str(l);
        self.out.push_str(":\n");
        self.terminated = false;
    }
    fn br(&mut self, l: &str) {
        if !self.terminated {
            self.emit(&format!("br label %{}", l));
            self.terminated = true;
        }
    }
    fn ret(&mut self, s: &str) {
        self.emit(&format!("ret {}", s));
        self.terminated = true;
    }
    fn to_i1(&mut self, v: String, t: &str) -> String {
        if t == "i1" {
            v
        } else {
            let r = self.fresh();
            self.emit(&format!("{} = icmp ne i64 {}, 0", r, v));
            r
        }
    }

    fn string_ptr(&mut self, s: &str) -> String {
        let name = format!("@.str{}", self.str_count);
        self.str_count += 1;
        let mut esc = String::new();
        let mut len = 0;
        for b in s.bytes() {
            len += 1;
            if b == b'"' || b == b'\\' || !(0x20..=0x7e).contains(&b) {
                esc.push_str(&format!("\\{:02X}", b));
            } else {
                esc.push(b as char);
            }
        }
        esc.push_str("\\00");
        len += 1;
        self.globals.push_str(&format!(
            "{} = private constant [{} x i8] c\"{}\"\n",
            name, len, esc
        ));
        let p = self.fresh();
        self.emit(&format!(
            "{} = getelementptr inbounds [{} x i8], [{} x i8]* {}, i64 0, i64 0",
            p, len, len, name
        ));
        p
    }

    fn gen_function(&mut self, f: &Function, name_override: Option<&str>) -> Result<(), String> {
        self.tmp = 0;
        self.label = 0;
        self.vars.clear();
        self.chan_elems.clear();
        self.map_types.clear();
        self.terminated = false;
        let ret = self.ret_type(f)?;
        self.cur_ret = ret.clone();

        // nome (mangled p/ método) + params (receiver primeiro)
        let (name, mut all_params) = match &f.receiver {
            Some(r) => {
                let sty = self.ty_of(&r.ty)?;
                (
                    format!("\"{}.{}\"", type_base(&r.ty), f.name),
                    vec![(r.name.clone(), sty)],
                )
            }
            None => (
                name_override
                    .map(|s| format!("\"{}\"", s))
                    .unwrap_or_else(|| f.name.clone()),
                Vec::new(),
            ),
        };
        for p in &f.params {
            all_params.push((p.name.clone(), self.ty_of(&p.ty)?));
        }
        let sig: Vec<String> = all_params
            .iter()
            .map(|(n, t)| format!("{} %{}", t, n))
            .collect();
        self.out
            .push_str(&format!("define {} @{}({}) {{\n", ret, name, sig.join(", ")));
        self.out.push_str("entry:\n");

        for (pn, pt) in &all_params {
            let addr = self.fresh();
            self.emit(&format!("{} = alloca {}", addr, pt));
            self.emit(&format!("store {} %{}, {}* {}", pt, pn, pt, addr));
            self.vars.insert(pn.clone(), (addr, pt.clone()));
        }
        // registra tipos de elemento dos params que são canais
        for p in &f.params {
            if let Type::Generic(n, args) = &p.ty {
                if n == "chan" && args.len() == 1 {
                    let e = self.ty_of(&args[0])?;
                    self.chan_elems.insert(p.name.clone(), e);
                }
            }
        }

        for s in &f.body.stmts {
            self.gen_stmt(s)?;
        }

        if !self.terminated {
            let r = self.cur_ret.clone();
            match r.as_str() {
                "i32" => self.ret("i32 0"),
                "void" => self.ret("void"),
                "double" => self.ret("double 0.0"),
                "i8*" => self.ret("i8* null"),
                _ if r.starts_with('{') || r.starts_with('%') => {
                    self.ret(&format!("{} zeroinitializer", r))
                }
                other => self.ret(&format!("{} 0", other)),
            }
        }
        self.out.push_str("}\n\n");
        Ok(())
    }

    fn gen_stmt(&mut self, s: &Stmt) -> Result<(), String> {
        if self.terminated {
            return Ok(());
        }
        match s {
            Stmt::VarDecl { decls, values, .. } => {
                if decls.len() == 1 && values.len() == 1 {
                    if let ExprKind::Match { scrutinee, arms } = &values[0].kind {
                        let dty = self.ty_of(&decls[0].ty)?;
                        let addr = self.fresh();
                        self.emit(&format!("{} = alloca {}", addr, dty));
                        self.vars
                            .insert(decls[0].name.clone(), (addr.clone(), dty.clone()));
                        return self.gen_match(scrutinee, arms, MatchMode::Assign(addr, dty));
                    }
                }
                self.gen_var_decl(decls, values)
            }
            Stmt::Assign { target, value } => match &target.kind {
                ExprKind::Ident(n) => {
                    let (addr, ty) = self
                        .vars
                        .get(n)
                        .cloned()
                        .ok_or(format!("backend LLVM: variável `{}` desconhecida", n))?;
                    let (v, _) = self.gen_expr(value)?;
                    self.emit(&format!("store {} {}, {}* {}", ty, v, ty, addr));
                    Ok(())
                }
                ExprKind::Index { base, index } => {
                    // map set: m[k] = v
                    if let ExprKind::Ident(bn) = &base.kind {
                        if let Some((keyisstr, valty)) = self.map_types.get(bn).cloned() {
                            return self.gen_map_set(base, index, value, keyisstr, &valty);
                        }
                    }
                    // slice store: s[i] = v
                    let (bv, bt) = self.gen_expr(base)?;
                    let elemty = Self::slice_elem(&bt);
                    let ptr = self.fresh();
                    self.emit(&format!("{} = extractvalue {} {}, 0", ptr, bt, bv));
                    let (iv, _) = self.gen_expr(index)?;
                    let ep = self.fresh();
                    self.emit(&format!(
                        "{} = getelementptr {}, {}* {}, i64 {}",
                        ep, elemty, elemty, ptr, iv
                    ));
                    let (vv, _) = self.gen_expr(value)?;
                    self.emit(&format!("store {} {}, {}* {}", elemty, vv, elemty, ep));
                    Ok(())
                }
                _ => Err("backend LLVM: alvo de atribuição não suportado".into()),
            },
            Stmt::Return(values) => {
                if values.len() == 1 {
                    if let ExprKind::Match { scrutinee, arms } = &values[0].kind {
                        return self.gen_match(scrutinee, arms, MatchMode::Return);
                    }
                }
                self.gen_return(values)
            }
            Stmt::If {
                cond,
                then_block,
                else_block,
            } => {
                let (c, ct) = self.gen_expr(cond)?;
                let ci1 = self.to_i1(c, &ct);
                let then_l = self.fresh_label("then");
                let end_l = self.fresh_label("ifend");
                let else_l = if else_block.is_some() {
                    self.fresh_label("else")
                } else {
                    end_l.clone()
                };
                self.emit(&format!("br i1 {}, label %{}, label %{}", ci1, then_l, else_l));
                self.terminated = true;
                self.label(&then_l);
                for st in &then_block.stmts {
                    self.gen_stmt(st)?;
                }
                self.br(&end_l);
                if let Some(eb) = else_block {
                    self.label(&else_l);
                    for st in &eb.stmts {
                        self.gen_stmt(st)?;
                    }
                    self.br(&end_l);
                }
                self.label(&end_l);
                Ok(())
            }
            Stmt::For { head, body } => self.gen_for(head, body),
            Stmt::Expr(e) => {
                if let ExprKind::Match { scrutinee, arms } = &e.kind {
                    return self.gen_match(scrutinee, arms, MatchMode::Stmt);
                }
                self.gen_expr(e)?;
                Ok(())
            }
            Stmt::Send { chan, value } => {
                let (cv, _) = self.gen_expr(chan)?;
                let (vv, vt) = self.gen_expr(value)?;
                let tmp = self.fresh();
                self.emit(&format!("{} = alloca {}", tmp, vt));
                self.emit(&format!("store {} {}, {}* {}", vt, vv, vt, tmp));
                let raw = self.fresh();
                self.emit(&format!("{} = bitcast {}* {} to i8*", raw, vt, tmp));
                self.needs.chan = true;
                self.emit(&format!("call void @vader_chan_send(i8* {}, i8* {})", cv, raw));
                Ok(())
            }
            Stmt::Spawn(call) => self.gen_spawn(call),
            Stmt::Assert(_) => Err("backend LLVM: `assert` fora de teste não suportado".into()),
        }
    }

    fn gen_var_decl(&mut self, decls: &[Param], values: &[Expr]) -> Result<(), String> {
        // multi-retorno: `int r, error e = call()`
        if decls.len() > 1 && values.len() == 1 {
            let (agg, aggty) = self.gen_expr(&values[0])?;
            for (i, d) in decls.iter().enumerate() {
                let dty = self.ty_of(&d.ty)?;
                let ex = self.fresh();
                self.emit(&format!("{} = extractvalue {} {}, {}", ex, aggty, agg, i));
                if d.name != "_" {
                    let addr = self.fresh();
                    self.emit(&format!("{} = alloca {}", addr, dty));
                    self.emit(&format!("store {} {}, {}* {}", dty, ex, dty, addr));
                    self.vars.insert(d.name.clone(), (addr, dty));
                }
            }
            return Ok(());
        }
        if decls.len() != 1 || values.len() != 1 {
            return Err("backend LLVM: declaração múltipla só a partir de chamada".into());
        }
        // map[K]V m = newmap()  -> cria o map (tipo K/V vem da declaração)
        if let Type::Generic(n, args) = &decls[0].ty {
            if n == "map" && args.len() == 2 {
                let keyisstr = matches!(&args[0], Type::Named(k) if k == "string");
                let valty = self.ty_of(&args[1])?;
                let szp = self.fresh();
                self.emit(&format!("{} = getelementptr {}, {}* null, i32 1", szp, valty, valty));
                let szi = self.fresh();
                self.emit(&format!("{} = ptrtoint {}* {} to i64", szi, valty, szp));
                self.needs.map = true;
                let m = self.fresh();
                self.emit(&format!(
                    "{} = call i8* @vader_map_make(i64 {}, i32 {})",
                    m,
                    szi,
                    if keyisstr { 1 } else { 0 }
                ));
                let addr = self.fresh();
                self.emit(&format!("{} = alloca i8*", addr));
                self.emit(&format!("store i8* {}, i8** {}", m, addr));
                self.vars
                    .insert(decls[0].name.clone(), (addr, "i8*".to_string()));
                self.map_types
                    .insert(decls[0].name.clone(), (keyisstr, valty));
                return Ok(());
            }
        }
        let dty = self.ty_of(&decls[0].ty)?;
        let (v, vt) = self.gen_expr(&values[0])?;
        if decls[0].name == "_" {
            return Ok(());
        }
        let v = self.coerce(v, &vt, &dty);
        let addr = self.fresh();
        self.emit(&format!("{} = alloca {}", addr, dty));
        self.emit(&format!("store {} {}, {}* {}", dty, v, dty, addr));
        self.vars.insert(decls[0].name.clone(), (addr, dty));
        if let Type::Generic(n, args) = &decls[0].ty {
            if n == "chan" && args.len() == 1 {
                let e = self.ty_of(&args[0])?;
                self.chan_elems.insert(decls[0].name.clone(), e);
            }
        }
        Ok(())
    }

    fn gen_return(&mut self, values: &[Expr]) -> Result<(), String> {
        let rt = self.cur_ret.clone();
        if values.is_empty() {
            match rt.as_str() {
                "i32" => self.ret("i32 0"),
                _ => self.ret("void"),
            }
            return Ok(());
        }
        if values.len() == 1 {
            let (v, vt) = self.gen_expr(&values[0])?;
            let v = self.coerce(v, &vt, &rt);
            self.ret(&format!("{} {}", rt, v));
            return Ok(());
        }
        // multi-retorno: monta o agregado
        let mut cur = "undef".to_string();
        let mut vals = Vec::new();
        for v in values {
            vals.push(self.gen_expr(v)?);
        }
        for (i, (v, t)) in vals.iter().enumerate() {
            let next = self.fresh();
            self.emit(&format!(
                "{} = insertvalue {} {}, {} {}, {}",
                next, rt, cur, t, v, i
            ));
            cur = next;
        }
        self.ret(&format!("{} {}", rt, cur));
        Ok(())
    }

    fn gen_for(&mut self, head: &ForHead, body: &Block) -> Result<(), String> {
        match head {
            ForHead::In { var, iter } => {
                if let ExprKind::Binary {
                    op: o @ (BinOp::Range | BinOp::RangeIncl),
                    left,
                    right,
                } = &iter.kind
                {
                    let (start, _) = self.gen_expr(left)?;
                    let addr = self.fresh();
                    self.emit(&format!("{} = alloca i64", addr));
                    self.emit(&format!("store i64 {}, i64* {}", start, addr));
                    self.vars.insert(var.clone(), (addr.clone(), "i64".to_string()));
                    let cond_l = self.fresh_label("loopcond");
                    let body_l = self.fresh_label("loopbody");
                    let end_l = self.fresh_label("loopend");
                    let cmp = if matches!(o, BinOp::RangeIncl) { "sle" } else { "slt" };
                    self.br(&cond_l);
                    self.label(&cond_l);
                    let (bound, _) = self.gen_expr(right)?;
                    let iv = self.fresh();
                    self.emit(&format!("{} = load i64, i64* {}", iv, addr));
                    let c = self.fresh();
                    self.emit(&format!("{} = icmp {} i64 {}, {}", c, cmp, iv, bound));
                    self.emit(&format!("br i1 {}, label %{}, label %{}", c, body_l, end_l));
                    self.terminated = true;
                    self.label(&body_l);
                    for st in &body.stmts {
                        self.gen_stmt(st)?;
                    }
                    if !self.terminated {
                        let cur = self.fresh();
                        self.emit(&format!("{} = load i64, i64* {}", cur, addr));
                        let nx = self.fresh();
                        self.emit(&format!("{} = add i64 {}, 1", nx, cur));
                        self.emit(&format!("store i64 {}, i64* {}", nx, addr));
                    }
                    self.br(&cond_l);
                    self.label(&end_l);
                    return Ok(());
                }
                // iteração de canal: for x in ch  (recebe até o canal fechar)
                if let ExprKind::Ident(cn) = &iter.kind {
                    if let Some(elem) = self.chan_elems.get(cn).cloned() {
                        let (cv, _) = self.gen_expr(iter)?;
                        let xaddr = self.fresh();
                        self.emit(&format!("{} = alloca {}", xaddr, elem));
                        self.vars.insert(var.clone(), (xaddr.clone(), elem.clone()));
                        let cond_l = self.fresh_label("chancond");
                        let body_l = self.fresh_label("chanbody");
                        let end_l = self.fresh_label("chanend");
                        self.br(&cond_l);
                        self.label(&cond_l);
                        let raw = self.fresh();
                        self.emit(&format!("{} = bitcast {}* {} to i8*", raw, elem, xaddr));
                        self.needs.chan = true;
                        let ok = self.fresh();
                        self.emit(&format!(
                            "{} = call i32 @vader_chan_recv(i8* {}, i8* {})",
                            ok, cv, raw
                        ));
                        let cmp = self.fresh();
                        self.emit(&format!("{} = icmp ne i32 {}, 0", cmp, ok));
                        self.emit(&format!(
                            "br i1 {}, label %{}, label %{}",
                            cmp, body_l, end_l
                        ));
                        self.terminated = true;
                        self.label(&body_l);
                        for st in &body.stmts {
                            self.gen_stmt(st)?;
                        }
                        self.br(&cond_l);
                        self.label(&end_l);
                        return Ok(());
                    }
                }
                // iteração de slice: for x in xs
                let (sv, st) = self.gen_expr(iter)?;
                if !st.starts_with('{') {
                    return Err("backend LLVM: for-in só sobre range ou slice".into());
                }
                let elemty = Self::slice_elem(&st);
                let ptr = self.fresh();
                self.emit(&format!("{} = extractvalue {} {}, 0", ptr, st, sv));
                let len = self.fresh();
                self.emit(&format!("{} = extractvalue {} {}, 1", len, st, sv));
                let iaddr = self.fresh();
                self.emit(&format!("{} = alloca i64", iaddr));
                self.emit(&format!("store i64 0, i64* {}", iaddr));
                let xaddr = self.fresh();
                self.emit(&format!("{} = alloca {}", xaddr, elemty));
                self.vars.insert(var.clone(), (xaddr.clone(), elemty.clone()));
                let cond_l = self.fresh_label("loopcond");
                let body_l = self.fresh_label("loopbody");
                let end_l = self.fresh_label("loopend");
                self.br(&cond_l);
                self.label(&cond_l);
                let iv = self.fresh();
                self.emit(&format!("{} = load i64, i64* {}", iv, iaddr));
                let c = self.fresh();
                self.emit(&format!("{} = icmp slt i64 {}, {}", c, iv, len));
                self.emit(&format!("br i1 {}, label %{}, label %{}", c, body_l, end_l));
                self.terminated = true;
                self.label(&body_l);
                let iv2 = self.fresh();
                self.emit(&format!("{} = load i64, i64* {}", iv2, iaddr));
                let ep = self.fresh();
                self.emit(&format!(
                    "{} = getelementptr {}, {}* {}, i64 {}",
                    ep, elemty, elemty, ptr, iv2
                ));
                let ev = self.fresh();
                self.emit(&format!("{} = load {}, {}* {}", ev, elemty, elemty, ep));
                self.emit(&format!("store {} {}, {}* {}", elemty, ev, elemty, xaddr));
                for st in &body.stmts {
                    self.gen_stmt(st)?;
                }
                if !self.terminated {
                    let cur = self.fresh();
                    self.emit(&format!("{} = load i64, i64* {}", cur, iaddr));
                    let nx = self.fresh();
                    self.emit(&format!("{} = add i64 {}, 1", nx, cur));
                    self.emit(&format!("store i64 {}, i64* {}", nx, iaddr));
                }
                self.br(&cond_l);
                self.label(&end_l);
                Ok(())
            }
            ForHead::While(cond) => {
                let cond_l = self.fresh_label("whilecond");
                let body_l = self.fresh_label("whilebody");
                let end_l = self.fresh_label("whileend");
                self.br(&cond_l);
                self.label(&cond_l);
                let (c, ct) = self.gen_expr(cond)?;
                let ci1 = self.to_i1(c, &ct);
                self.emit(&format!("br i1 {}, label %{}, label %{}", ci1, body_l, end_l));
                self.terminated = true;
                self.label(&body_l);
                for st in &body.stmts {
                    self.gen_stmt(st)?;
                }
                self.br(&cond_l);
                self.label(&end_l);
                Ok(())
            }
            ForHead::Infinite => {
                let body_l = self.fresh_label("loopinf");
                self.br(&body_l);
                self.label(&body_l);
                for st in &body.stmts {
                    self.gen_stmt(st)?;
                }
                self.br(&body_l);
                Ok(())
            }
        }
    }

    fn gen_map_get(
        &mut self,
        base: &Expr,
        index: &Expr,
        keyisstr: bool,
        valty: &str,
    ) -> Result<(String, String), String> {
        let (mv, _) = self.gen_expr(base)?;
        let out = self.fresh();
        self.emit(&format!("{} = alloca {}", out, valty));
        let raw = self.fresh();
        self.emit(&format!("{} = bitcast {}* {} to i8*", raw, valty, out));
        self.needs.map = true;
        let (kv, _) = self.gen_expr(index)?;
        let ok = self.fresh();
        if keyisstr {
            self.emit(&format!(
                "{} = call i32 @vader_map_get_str(i8* {}, i8* {}, i8* {})",
                ok, mv, kv, raw
            ));
        } else {
            self.emit(&format!(
                "{} = call i32 @vader_map_get_int(i8* {}, i64 {}, i8* {})",
                ok, mv, kv, raw
            ));
        }
        let val = self.fresh();
        self.emit(&format!("{} = load {}, {}* {}", val, valty, valty, out));
        Ok((val, valty.to_string()))
    }

    fn gen_map_set(
        &mut self,
        base: &Expr,
        index: &Expr,
        value: &Expr,
        keyisstr: bool,
        valty: &str,
    ) -> Result<(), String> {
        let (mv, _) = self.gen_expr(base)?;
        let (kv, _) = self.gen_expr(index)?;
        let (vv, _) = self.gen_expr(value)?;
        let tmp = self.fresh();
        self.emit(&format!("{} = alloca {}", tmp, valty));
        self.emit(&format!("store {} {}, {}* {}", valty, vv, valty, tmp));
        let raw = self.fresh();
        self.emit(&format!("{} = bitcast {}* {} to i8*", raw, valty, tmp));
        self.needs.map = true;
        if keyisstr {
            self.emit(&format!(
                "call void @vader_map_set_str(i8* {}, i8* {}, i8* {})",
                mv, kv, raw
            ));
        } else {
            self.emit(&format!(
                "call void @vader_map_set_int(i8* {}, i64 {}, i8* {})",
                mv, kv, raw
            ));
        }
        Ok(())
    }

    fn gen_spawn(&mut self, call: &Expr) -> Result<(), String> {
        let (fname, cargs) = match &call.kind {
            ExprKind::Call { callee, args } => match &callee.kind {
                ExprKind::Ident(n) => (n.clone(), args),
                _ => return Err("backend LLVM: spawn só de função simples".into()),
            },
            _ => return Err("backend LLVM: spawn precisa de uma chamada".into()),
        };
        let mut argvals = Vec::new();
        for a in cargs {
            argvals.push(self.gen_expr(a)?);
        }
        let argtypes: Vec<String> = argvals.iter().map(|(_, t)| t.clone()).collect();
        let structty = format!("{{ {} }}", argtypes.join(", "));
        // empacota os args num struct no heap
        let szp = self.fresh();
        self.emit(&format!("{} = getelementptr {}, {}* null, i32 1", szp, structty, structty));
        let szi = self.fresh();
        self.emit(&format!("{} = ptrtoint {}* {} to i64", szi, structty, szp));
        self.needs.malloc = true;
        let raw = self.fresh();
        self.emit(&format!("{} = call i8* @malloc(i64 {})", raw, szi));
        let pp = self.fresh();
        self.emit(&format!("{} = bitcast i8* {} to {}*", pp, raw, structty));
        for (i, (v, t)) in argvals.iter().enumerate() {
            let fp = self.fresh();
            self.emit(&format!(
                "{} = getelementptr {}, {}* {}, i32 0, i32 {}",
                fp, structty, structty, pp, i
            ));
            self.emit(&format!("store {} {}, {}* {}", t, v, t, fp));
        }
        let thunk = format!("spawn$thunk${}", self.thunk_count);
        self.thunk_count += 1;
        self.pending_thunks.push((thunk.clone(), fname, argtypes));
        self.needs.chan = true; // vader_go é declarado junto do runtime
        self.emit(&format!(
            "call void @vader_go(i8* (i8*)* @\"{}\", i8* {})",
            thunk, raw
        ));
        Ok(())
    }

    /// Gera o trampolim de uma goroutine: desempacota os args e chama a função.
    fn gen_thunk(&mut self, thunk: &str, fname: &str, argtypes: &[String]) {
        let structty = format!("{{ {} }}", argtypes.join(", "));
        self.out
            .push_str(&format!("define i8* @\"{}\"(i8* %arg) {{\nentry:\n", thunk));
        self.out
            .push_str(&format!("  %p = bitcast i8* %arg to {}*\n", structty));
        let mut callargs = Vec::new();
        for (i, t) in argtypes.iter().enumerate() {
            self.out.push_str(&format!(
                "  %a{}p = getelementptr {}, {}* %p, i32 0, i32 {}\n",
                i, structty, structty, i
            ));
            self.out
                .push_str(&format!("  %a{} = load {}, {}* %a{}p\n", i, t, t, i));
            callargs.push(format!("{} %a{}", t, i));
        }
        let ret = self
            .funcs
            .get(fname)
            .map(|(_, r)| r.clone())
            .unwrap_or_else(|| "void".to_string());
        if ret == "void" {
            self.out
                .push_str(&format!("  call void @{}({})\n", fname, callargs.join(", ")));
        } else {
            self.out.push_str(&format!(
                "  %r = call {} @{}({})\n",
                ret,
                fname,
                callargs.join(", ")
            ));
        }
        self.out.push_str("  ret i8* null\n}\n\n");
    }

    fn gen_expr(&mut self, e: &Expr) -> Result<(String, String), String> {
        match &e.kind {
            ExprKind::Int(v) => Ok((v.to_string(), "i64".to_string())),
            ExprKind::Float(v) => Ok((format!("0x{:016X}", v.to_bits()), "double".to_string())),
            ExprKind::Bool(b) => Ok(((if *b { "true" } else { "false" }).to_string(), "i1".to_string())),
            ExprKind::Nil => Ok(("null".to_string(), "i8*".to_string())),
            ExprKind::Str(s) => {
                let p = self.string_ptr(s);
                Ok((p, "i8*".to_string()))
            }
            ExprKind::Ident(n) => {
                let (addr, ty) = self
                    .vars
                    .get(n)
                    .cloned()
                    .ok_or(format!("backend LLVM: variável `{}` desconhecida", n))?;
                let t = self.fresh();
                self.emit(&format!("{} = load {}, {}* {}", t, ty, ty, addr));
                Ok((t, ty))
            }
            ExprKind::Unary { op, expr } => {
                let (v, t) = self.gen_expr(expr)?;
                match op {
                    UnOp::Neg => {
                        let r = self.fresh();
                        if t == "double" {
                            self.emit(&format!("{} = fneg double {}", r, v));
                        } else {
                            self.emit(&format!("{} = sub i64 0, {}", r, v));
                        }
                        Ok((r, t))
                    }
                    UnOp::Not => {
                        let r = self.fresh();
                        self.emit(&format!("{} = xor i1 {}, true", r, v));
                        Ok((r, "i1".to_string()))
                    }
                }
            }
            ExprKind::Binary { op, left, right } => self.gen_binary(op, left, right),
            ExprKind::Call { callee, args } => self.gen_call(callee, args),
            ExprKind::Field { base, field } => {
                let (bv, bt) = self.gen_expr(base)?;
                let sname = bt.trim_start_matches('%').to_string();
                let (idx, fty) = self
                    .struct_field(&sname, field)
                    .ok_or(format!("backend LLVM: campo `{}` em `{}`", field, bt))?;
                let r = self.fresh();
                self.emit(&format!("{} = extractvalue {} {}, {}", r, bt, bv, idx));
                Ok((r, fty))
            }
            ExprKind::StructLit { name, fields } => self.gen_struct_lit(name, fields),
            ExprKind::Index { base, index } => {
                // lookup de map: m[k]
                if let ExprKind::Ident(bn) = &base.kind {
                    if let Some((keyisstr, valty)) = self.map_types.get(bn).cloned() {
                        return self.gen_map_get(base, index, keyisstr, &valty);
                    }
                }
                // índice de slice: s[i]
                let (bv, bt) = self.gen_expr(base)?;
                let (iv, _) = self.gen_expr(index)?;
                let elemty = Self::slice_elem(&bt);
                let ptr = self.fresh();
                self.emit(&format!("{} = extractvalue {} {}, 0", ptr, bt, bv));
                let ep = self.fresh();
                self.emit(&format!(
                    "{} = getelementptr {}, {}* {}, i64 {}",
                    ep, elemty, elemty, ptr, iv
                ));
                let lv = self.fresh();
                self.emit(&format!("{} = load {}, {}* {}", lv, elemty, elemty, ep));
                Ok((lv, elemty))
            }
            ExprKind::SliceLit(elems) => self.gen_slice_lit(elems),
            ExprKind::Recv(inner) => {
                let (cv, _) = self.gen_expr(inner)?;
                let elem = self.recv_elem(inner)?;
                let out = self.fresh();
                self.emit(&format!("{} = alloca {}", out, elem));
                let raw = self.fresh();
                self.emit(&format!("{} = bitcast {}* {} to i8*", raw, elem, out));
                self.needs.chan = true;
                let ok = self.fresh();
                self.emit(&format!(
                    "{} = call i32 @vader_chan_recv(i8* {}, i8* {})",
                    ok, cv, raw
                ));
                let _ = ok;
                let val = self.fresh();
                self.emit(&format!("{} = load {}, {}* {}", val, elem, elem, out));
                Ok((val, elem))
            }
            _ => Err("backend LLVM: construção não suportada no subset".into()),
        }
    }

    fn recv_elem(&self, e: &Expr) -> Result<String, String> {
        if let ExprKind::Ident(n) = &e.kind {
            if let Some(el) = self.chan_elems.get(n) {
                return Ok(el.clone());
            }
        }
        Err("backend LLVM: não consegui inferir o tipo do elemento no recv".into())
    }

    fn gen_slice_lit(&mut self, elems: &[Expr]) -> Result<(String, String), String> {
        if elems.is_empty() {
            return Err("backend LLVM: slice literal vazio precisa de tipo (subset)".into());
        }
        let mut vals = Vec::new();
        for el in elems {
            vals.push(self.gen_expr(el)?);
        }
        let elemty = vals[0].1.clone();
        let n = elems.len();
        let slicety = format!("{{ {}*, i64 }}", elemty);
        // sizeof(n elementos) via getelementptr null
        let szp = self.fresh();
        self.emit(&format!(
            "{} = getelementptr {}, {}* null, i64 {}",
            szp, elemty, elemty, n
        ));
        let szi = self.fresh();
        self.emit(&format!("{} = ptrtoint {}* {} to i64", szi, elemty, szp));
        self.needs.malloc = true;
        let raw = self.fresh();
        self.emit(&format!("{} = call i8* @malloc(i64 {})", raw, szi));
        let arr = self.fresh();
        self.emit(&format!("{} = bitcast i8* {} to {}*", arr, raw, elemty));
        for (i, (v, t)) in vals.iter().enumerate() {
            let ep = self.fresh();
            self.emit(&format!(
                "{} = getelementptr {}, {}* {}, i64 {}",
                ep, elemty, elemty, arr, i
            ));
            self.emit(&format!("store {} {}, {}* {}", t, v, t, ep));
        }
        let s0 = self.fresh();
        self.emit(&format!(
            "{} = insertvalue {} undef, {}* {}, 0",
            s0, slicety, elemty, arr
        ));
        let s1 = self.fresh();
        self.emit(&format!("{} = insertvalue {} {}, i64 {}, 1", s1, slicety, s0, n));
        Ok((s1, slicety))
    }

    fn struct_field(&self, sname: &str, field: &str) -> Option<(usize, String)> {
        let fields = self.structs.get(sname)?;
        fields
            .iter()
            .position(|(n, _)| n == field)
            .map(|i| (i, fields[i].1.clone()))
    }

    fn gen_struct_lit(
        &mut self,
        name: &str,
        fields: &[(String, Expr)],
    ) -> Result<(String, String), String> {
        let lltype = format!("%{}", name);
        let def = self
            .structs
            .get(name)
            .cloned()
            .ok_or(format!("backend LLVM: struct `{}` desconhecido", name))?;
        let mut cur = "undef".to_string();
        for (fname, fexpr) in fields {
            let idx = def
                .iter()
                .position(|(n, _)| n == fname)
                .ok_or(format!("backend LLVM: campo `{}` em `{}`", fname, name))?;
            let (v, t) = self.gen_expr(fexpr)?;
            let fty = def[idx].1.clone();
            let v = self.coerce(v, &t, &fty);
            let next = self.fresh();
            self.emit(&format!(
                "{} = insertvalue {} {}, {} {}, {}",
                next, lltype, cur, fty, v, idx
            ));
            cur = next;
        }
        Ok((cur, lltype))
    }

    fn gen_binary(
        &mut self,
        op: &BinOp,
        left: &Expr,
        right: &Expr,
    ) -> Result<(String, String), String> {
        if matches!(op, BinOp::Range | BinOp::RangeIncl) {
            return Err("backend LLVM: range só dentro de `for`".into());
        }
        let (l, lt) = self.gen_expr(left)?;
        let (r, rt) = self.gen_expr(right)?;
        use BinOp::*;
        // concatenação / comparação de strings
        if lt == "i8*" && matches!(op, Add | Eq | NotEq) {
            return self.gen_string_op(op, l, r);
        }
        let is_f = lt == "double" || rt == "double";
        match op {
            Add | Sub | Mul | Div | Rem => {
                let instr = match (op, is_f) {
                    (Add, false) => "add",
                    (Sub, false) => "sub",
                    (Mul, false) => "mul",
                    (Div, false) => "sdiv",
                    (Rem, false) => "srem",
                    (Add, true) => "fadd",
                    (Sub, true) => "fsub",
                    (Mul, true) => "fmul",
                    (Div, true) => "fdiv",
                    (Rem, true) => "frem",
                    _ => unreachable!(),
                };
                let ty = if is_f { "double" } else { "i64" };
                let t = self.fresh();
                self.emit(&format!("{} = {} {} {}, {}", t, instr, ty, l, r));
                Ok((t, ty.to_string()))
            }
            Eq | NotEq | Lt | LtEq | Gt | GtEq => {
                let t = self.fresh();
                if is_f {
                    let c = match op {
                        Eq => "oeq",
                        NotEq => "one",
                        Lt => "olt",
                        LtEq => "ole",
                        Gt => "ogt",
                        GtEq => "oge",
                        _ => unreachable!(),
                    };
                    self.emit(&format!("{} = fcmp {} double {}, {}", t, c, l, r));
                } else {
                    let opty = if lt == "i1" && rt == "i1" { "i1" } else { "i64" };
                    let c = match op {
                        Eq => "eq",
                        NotEq => "ne",
                        Lt => "slt",
                        LtEq => "sle",
                        Gt => "sgt",
                        GtEq => "sge",
                        _ => unreachable!(),
                    };
                    self.emit(&format!("{} = icmp {} {} {}, {}", t, c, opty, l, r));
                }
                Ok((t, "i1".to_string()))
            }
            And | Or => {
                let instr = if matches!(op, And) { "and" } else { "or" };
                let t = self.fresh();
                self.emit(&format!("{} = {} i1 {}, {}", t, instr, l, r));
                Ok((t, "i1".to_string()))
            }
            Range | RangeIncl => unreachable!(),
        }
    }

    fn gen_string_op(
        &mut self,
        op: &BinOp,
        l: String,
        r: String,
    ) -> Result<(String, String), String> {
        match op {
            BinOp::Add => {
                self.needs.strcat = true;
                let t = self.fresh();
                self.emit(&format!("{} = call i8* @vader_strcat(i8* {}, i8* {})", t, l, r));
                Ok((t, "i8*".to_string()))
            }
            BinOp::Eq | BinOp::NotEq => {
                // null (nil) compara como ponteiro; senão usa strcmp
                let t = self.fresh();
                if l == "null" || r == "null" {
                    let c = if matches!(op, BinOp::Eq) { "eq" } else { "ne" };
                    self.emit(&format!("{} = icmp {} i8* {}, {}", t, c, l, r));
                } else {
                    self.needs.strcmp = true;
                    let cmp = self.fresh();
                    self.emit(&format!("{} = call i32 @strcmp(i8* {}, i8* {})", cmp, l, r));
                    let c = if matches!(op, BinOp::Eq) { "eq" } else { "ne" };
                    self.emit(&format!("{} = icmp {} i32 {}, 0", t, c, cmp));
                }
                Ok((t, "i1".to_string()))
            }
            _ => unreachable!(),
        }
    }

    /// Intrínsecas do driver `std/db` (SQLite). Retorna `Some` se tratou o nome.
    fn gen_db_call(
        &mut self,
        name: &str,
        args: &[Expr],
    ) -> Result<Option<(String, String)>, String> {
        let (en, ret, ptys): (&str, &str, &[&str]) = match name {
            "open" => ("vader_db_open", "i8*", &["i8*"]),
            "exec" => ("vader_db_exec", "i8*", &["i8*", "i8*"]),
            "must" => ("vader_db_must", "void", &["i8*", "i8*"]),
            "query" => ("vader_db_query", "i8*", &["i8*", "i8*"]),
            "next" => ("vader_db_next", "i1", &["i8*"]),
            "col_int" => ("vader_db_col_int", "i64", &["i8*", "i32"]),
            "col_float" => ("vader_db_col_float", "double", &["i8*", "i32"]),
            "col_text" => ("vader_db_col_text", "i8*", &["i8*", "i32"]),
            "close" => ("vader_db_close", "void", &["i8*"]),
            _ => return Ok(None),
        };
        self.needs.db = true;
        Ok(Some(self.emit_extern(en, ret, ptys, args)?))
    }

    /// Intrínsecas de `std/http` (servidor + cliente).
    fn gen_http_call(
        &mut self,
        name: &str,
        args: &[Expr],
    ) -> Result<Option<(String, String)>, String> {
        let (en, ret, ptys): (&str, &str, &[&str]) = match name {
            "listen" => ("vader_http_listen", "i8*", &["i64"]),
            "accept" => ("vader_http_accept", "i1", &["i8*"]),
            "method" => ("vader_http_method", "i8*", &["i8*"]),
            "path" => ("vader_http_path", "i8*", &["i8*"]),
            "body" => ("vader_http_body", "i8*", &["i8*"]),
            "header" => ("vader_http_header", "i8*", &["i8*", "i8*"]),
            "respond" => ("vader_http_respond", "void", &["i8*", "i64", "i8*", "i8*"]),
            "get" => ("vader_http_get", "i8*", &["i8*"]),
            "post" => ("vader_http_post", "i8*", &["i8*", "i8*", "i8*"]),
            _ => return Ok(None),
        };
        self.needs.http = true;
        Ok(Some(self.emit_extern(en, ret, ptys, args)?))
    }

    /// Intrínsecas de `std/json` (parse + acessores + builder + encode).
    fn gen_json_call(
        &mut self,
        name: &str,
        args: &[Expr],
    ) -> Result<Option<(String, String)>, String> {
        let (en, ret, ptys): (&str, &str, &[&str]) = match name {
            "parse" => ("vader_json_parse", "i8*", &["i8*"]),
            "field" => ("vader_json_field", "i8*", &["i8*", "i8*"]),
            "elem" => ("vader_json_elem", "i8*", &["i8*", "i32"]),
            "as_str" => ("vader_json_as_str", "i8*", &["i8*"]),
            "as_int" => ("vader_json_as_int", "i64", &["i8*"]),
            "as_float" => ("vader_json_as_float", "double", &["i8*"]),
            "as_bool" => ("vader_json_as_bool", "i1", &["i8*"]),
            "count" => ("vader_json_count", "i64", &["i8*"]),
            "object" => ("vader_json_object", "i8*", &[]),
            "array" => ("vader_json_array", "i8*", &[]),
            "set" => ("vader_json_set", "i8*", &["i8*", "i8*", "i8*"]),
            "set_str" => ("vader_json_set_str", "i8*", &["i8*", "i8*", "i8*"]),
            "set_int" => ("vader_json_set_int", "i8*", &["i8*", "i8*", "i64"]),
            "set_float" => ("vader_json_set_float", "i8*", &["i8*", "i8*", "double"]),
            "set_bool" => ("vader_json_set_bool", "i8*", &["i8*", "i8*", "i32"]),
            "add" => ("vader_json_add", "i8*", &["i8*", "i8*"]),
            "add_str" => ("vader_json_add_str", "i8*", &["i8*", "i8*"]),
            "add_int" => ("vader_json_add_int", "i8*", &["i8*", "i64"]),
            "encode" => ("vader_json_encode", "i8*", &["i8*"]),
            _ => return Ok(None),
        };
        self.needs.json = true;
        Ok(Some(self.emit_extern(en, ret, ptys, args)?))
    }

    /// Emite uma chamada a uma função externa (runtime C). Coage args (i64->i32,
    /// i1->i32) e converte o retorno (i1<-i32 via icmp; void; senão direto).
    fn emit_extern(
        &mut self,
        extern_name: &str,
        ret: &str,
        ptys: &[&str],
        args: &[Expr],
    ) -> Result<(String, String), String> {
        let mut argstrs = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let (v, t) = self.gen_expr(a)?;
            let pty = ptys.get(i).copied().unwrap_or("i8*");
            let v = if t == "i64" && pty == "i32" {
                let r = self.fresh();
                self.emit(&format!("{} = trunc i64 {} to i32", r, v));
                r
            } else if t == "i1" && pty == "i32" {
                let r = self.fresh();
                self.emit(&format!("{} = zext i1 {} to i32", r, v));
                r
            } else {
                v
            };
            argstrs.push(format!("{} {}", pty, v));
        }
        let joined = argstrs.join(", ");
        if ret == "i1" {
            let r = self.fresh();
            self.emit(&format!("{} = call i32 @{}({})", r, extern_name, joined));
            let b = self.fresh();
            self.emit(&format!("{} = icmp ne i32 {}, 0", b, r));
            return Ok((b, "i1".to_string()));
        }
        if ret == "void" {
            self.emit(&format!("call void @{}({})", extern_name, joined));
            return Ok(("0".to_string(), "void".to_string()));
        }
        let r = self.fresh();
        self.emit(&format!("{} = call {} @{}({})", r, ret, extern_name, joined));
        Ok((r, ret.to_string()))
    }

    fn gen_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<(String, String), String> {
        // criação de canal: chan[T](buffer) -> vader_chan_make(sizeof(T), buffer)
        if let ExprKind::Index { base, index } = &callee.kind {
            if let ExprKind::Ident(b) = &base.kind {
                if b == "chan" {
                    let elem = match &index.kind {
                        ExprKind::Ident(n) => self.ty_of(&Type::Named(n.clone()))?,
                        _ => return Err("backend LLVM: tipo do canal inválido".into()),
                    };
                    let szp = self.fresh();
                    self.emit(&format!("{} = getelementptr {}, {}* null, i32 1", szp, elem, elem));
                    let szi = self.fresh();
                    self.emit(&format!("{} = ptrtoint {}* {} to i64", szi, elem, szp));
                    let buf = if args.is_empty() {
                        "1".to_string()
                    } else {
                        self.gen_expr(&args[0])?.0
                    };
                    self.needs.chan = true;
                    let r = self.fresh();
                    self.emit(&format!(
                        "{} = call i8* @vader_chan_make(i64 {}, i64 {})",
                        r, szi, buf
                    ));
                    return Ok((r, "i8*".to_string()));
                }
            }
        }
        if let ExprKind::Field { base, field } = &callee.kind {
            let (bv, bt) = self.gen_expr(base)?;
            let tyname = bt.trim_start_matches('%').to_string();
            // despacho dinâmico de interface (vtable)
            if let Some(methods) = self.interfaces.get(&tyname).cloned() {
                let idx = methods
                    .iter()
                    .position(|(m, _, _)| m == field)
                    .ok_or(format!("backend LLVM: método `{}` na interface `{}`", field, tyname))?;
                let (_, params, ret) = methods[idx].clone();
                let mut ptypes = vec!["i8*".to_string()];
                ptypes.extend(params.clone());
                let fnptrty = format!("{} ({})*", ret, ptypes.join(", "));
                let vt = self.fresh();
                self.emit(&format!("{} = extractvalue {} {}, 1", vt, bt, bv));
                let vtp = self.fresh();
                self.emit(&format!("{} = bitcast i8* {} to %vtable.{}*", vtp, vt, tyname));
                let mpp = self.fresh();
                self.emit(&format!(
                    "{} = getelementptr %vtable.{}, %vtable.{}* {}, i32 0, i32 {}",
                    mpp, tyname, tyname, vtp, idx
                ));
                let mp = self.fresh();
                self.emit(&format!("{} = load {}, {}* {}", mp, fnptrty, fnptrty, mpp));
                let data = self.fresh();
                self.emit(&format!("{} = extractvalue {} {}, 0", data, bt, bv));
                let mut argstrs = vec![format!("i8* {}", data)];
                for (a, pty) in args.iter().zip(params.iter()) {
                    let (v, t) = self.gen_expr(a)?;
                    let v = self.coerce(v, &t, pty);
                    argstrs.push(format!("{} {}", pty, v));
                }
                return self.emit_call(&ret, &mp, &argstrs);
            }
            // método de struct
            if let Some((params, ret)) =
                self.methods.get(&(tyname.clone(), field.clone())).cloned()
            {
                let mut argstrs = vec![format!("{} {}", bt, bv)];
                for (a, pty) in args.iter().zip(params.iter()) {
                    let (v, t) = self.gen_expr(a)?;
                    let v = self.coerce(v, &t, pty);
                    argstrs.push(format!("{} {}", pty, v));
                }
                return self.emit_call(&ret, &format!("@\"{}.{}\"", tyname, field), &argstrs);
            }
            return Err(format!("backend LLVM: método `{}.{}` desconhecido", tyname, field));
        }

        let name = match &callee.kind {
            ExprKind::Ident(n) => n.clone(),
            _ => return Err("backend LLVM: chamada complexa não suportada".into()),
        };
        // intrínsecas da stdlib (têm prioridade — `close` aqui é do banco, não de canal)
        if self.has_db {
            if let Some(r) = self.gen_db_call(&name, args)? {
                return Ok(r);
            }
        }
        if self.has_http {
            if let Some(r) = self.gen_http_call(&name, args)? {
                return Ok(r);
            }
        }
        if self.has_json {
            if let Some(r) = self.gen_json_call(&name, args)? {
                return Ok(r);
            }
        }
        if name == "print" {
            return self.gen_print(args);
        }
        if name == "error" && args.len() == 1 {
            let (v, _) = self.gen_expr(&args[0])?;
            return Ok((v, "i8*".to_string()));
        }
        if name == "len" && args.len() == 1 {
            if let ExprKind::Ident(an) = &args[0].kind {
                if self.map_types.contains_key(an) {
                    let (mv, _) = self.gen_expr(&args[0])?;
                    self.needs.map = true;
                    let r = self.fresh();
                    self.emit(&format!("{} = call i64 @vader_map_len(i8* {})", r, mv));
                    return Ok((r, "i64".to_string()));
                }
            }
            let (v, t) = self.gen_expr(&args[0])?;
            let r = self.fresh();
            self.emit(&format!("{} = extractvalue {} {}, 1", r, t, v));
            return Ok((r, "i64".to_string()));
        }
        if name == "close" && args.len() == 1 {
            let (v, _) = self.gen_expr(&args[0])?;
            self.needs.chan = true;
            self.emit(&format!("call void @vader_chan_close(i8* {})", v));
            return Ok(("0".to_string(), "void".to_string()));
        }
        if let Some((ename, tag, fields)) = self.variants.get(&name).cloned() {
            return self.gen_variant_ctor(&ename, tag, &fields, args);
        }
        // chamada de função genérica -> monomorfiza
        if let Some(gfn) = self.generics.get(&name).cloned() {
            return self.gen_generic_call(&gfn, args);
        }
        let (params, ret) = self
            .funcs
            .get(&name)
            .cloned()
            .ok_or(format!("backend LLVM: função `{}` desconhecida", name))?;
        let mut argstrs = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let (v, t) = self.gen_expr(a)?;
            let pty = params.get(i).cloned().unwrap_or_else(|| t.clone());
            let v = self.coerce(v, &t, &pty);
            argstrs.push(format!("{} {}", pty, v));
        }
        self.emit_call(&ret, &format!("@{}", name), &argstrs)
    }

    fn gen_generic_call(
        &mut self,
        gfn: &Function,
        args: &[Expr],
    ) -> Result<(String, String), String> {
        let tparams: HashSet<String> = gfn.type_params.iter().map(|tp| tp.name.clone()).collect();
        let mut argvals = Vec::new();
        for a in args {
            argvals.push(self.gen_expr(a)?);
        }
        // infere a substituição T -> lltype a partir dos argumentos
        let mut subst = HashMap::new();
        for (i, p) in gfn.params.iter().enumerate() {
            if let Some((_, at)) = argvals.get(i) {
                unify(&tparams, &p.ty, at, &mut subst);
            }
        }
        let mut parts = Vec::new();
        for tp in &gfn.type_params {
            let s = subst.get(&tp.name).cloned().unwrap_or_else(|| "i8".to_string());
            parts.push(sanitize(&s));
        }
        let mangled = format!("{}${}", gfn.name, parts.join("$"));
        let mut ptys = Vec::new();
        for p in &gfn.params {
            ptys.push(self.apply_subst_ty(&p.ty, &subst)?);
        }
        let ret = match gfn.returns.len() {
            0 => "void".to_string(),
            1 => self.apply_subst_ty(&gfn.returns[0], &subst)?,
            _ => {
                let mut ts = Vec::new();
                for t in &gfn.returns {
                    ts.push(self.apply_subst_ty(t, &subst)?);
                }
                format!("{{ {} }}", ts.join(", "))
            }
        };
        if !self.mono_done.contains(&mangled) {
            self.mono_done.insert(mangled.clone());
            self.pending.push((mangled.clone(), gfn.clone(), subst.clone()));
        }
        let mut argstrs = Vec::new();
        for (i, (v, t)) in argvals.iter().enumerate() {
            let pty = ptys.get(i).cloned().unwrap_or_else(|| t.clone());
            let v2 = self.coerce(v.clone(), t, &pty);
            argstrs.push(format!("{} {}", pty, v2));
        }
        self.emit_call(&ret, &format!("@\"{}\"", mangled), &argstrs)
    }

    fn apply_subst_ty(&self, t: &Type, subst: &HashMap<String, String>) -> Result<String, String> {
        match t {
            Type::Named(n) if subst.contains_key(n) => Ok(subst[n].clone()),
            Type::Slice(inner) => Ok(format!("{{ {}*, i64 }}", self.apply_subst_ty(inner, subst)?)),
            _ => self.ty_of(t),
        }
    }

    fn emit_call(
        &mut self,
        ret: &str,
        callee: &str,
        args: &[String],
    ) -> Result<(String, String), String> {
        if ret == "void" {
            self.emit(&format!("call void {}({})", callee, args.join(", ")));
            Ok(("0".to_string(), "void".to_string()))
        } else {
            let t = self.fresh();
            self.emit(&format!(
                "{} = call {} {}({})",
                t,
                ret,
                callee,
                args.join(", ")
            ));
            Ok((t, ret.to_string()))
        }
    }

    fn gen_variant_ctor(
        &mut self,
        ename: &str,
        tag: usize,
        fields: &[(String, String)],
        args: &[Expr],
    ) -> Result<(String, String), String> {
        let enumty = format!("%{}", ename);
        let data = if fields.is_empty() {
            "null".to_string()
        } else {
            let fieldtys: Vec<String> = fields.iter().map(|(_, t)| t.clone()).collect();
            let vstruct = format!("{{ {} }}", fieldtys.join(", "));
            let szp = self.fresh();
            self.emit(&format!("{} = getelementptr {}, {}* null, i32 1", szp, vstruct, vstruct));
            let szi = self.fresh();
            self.emit(&format!("{} = ptrtoint {}* {} to i64", szi, vstruct, szp));
            self.needs.malloc = true;
            let raw = self.fresh();
            self.emit(&format!("{} = call i8* @malloc(i64 {})", raw, szi));
            let pp = self.fresh();
            self.emit(&format!("{} = bitcast i8* {} to {}*", pp, raw, vstruct));
            for (i, a) in args.iter().enumerate() {
                let (v, t) = self.gen_expr(a)?;
                let fp = self.fresh();
                self.emit(&format!(
                    "{} = getelementptr {}, {}* {}, i32 0, i32 {}",
                    fp, vstruct, vstruct, pp, i
                ));
                self.emit(&format!("store {} {}, {}* {}", t, v, t, fp));
            }
            raw
        };
        let e0 = self.fresh();
        self.emit(&format!("{} = insertvalue {} undef, i32 {}, 0", e0, enumty, tag));
        let e1 = self.fresh();
        self.emit(&format!("{} = insertvalue {} {}, i8* {}, 1", e1, enumty, e0, data));
        Ok((e1, enumty))
    }

    fn gen_match(
        &mut self,
        scrut: &Expr,
        arms: &[MatchArm],
        mode: MatchMode,
    ) -> Result<(), String> {
        if arms.iter().any(|a| a.guard.is_some()) {
            return Err("backend LLVM: guardas em match não suportadas".into());
        }
        let (sv, st) = self.gen_expr(scrut)?;
        if !st.starts_with('%') || !self.enums.contains_key(st.trim_start_matches('%')) {
            return Err("backend LLVM: match só sobre enum no subset".into());
        }
        let tag = self.fresh();
        self.emit(&format!("{} = extractvalue {} {}, 0", tag, st, sv));
        let data = self.fresh();
        self.emit(&format!("{} = extractvalue {} {}, 1", data, st, sv));
        let end_l = self.fresh_label("matchend");

        let mut cases: Vec<(usize, String, usize)> = Vec::new();
        let mut default_arm: Option<(String, usize)> = None;
        for (ai, arm) in arms.iter().enumerate() {
            let vname = match &arm.patterns[0] {
                Pattern::Variant { name, .. } => Some(name.clone()),
                Pattern::Binding(n) if self.variants.contains_key(n) => Some(n.clone()),
                _ => None,
            };
            match vname.and_then(|n| self.variants.get(&n).map(|(_, t, _)| *t)) {
                Some(t) => {
                    let lbl = self.fresh_label("case");
                    cases.push((t, lbl, ai));
                }
                None => {
                    let lbl = self.fresh_label("default");
                    default_arm = Some((lbl, ai));
                }
            }
        }
        let default_l = match &default_arm {
            Some((l, _)) => l.clone(),
            None => self.fresh_label("default"),
        };
        let arms_str: Vec<String> = cases
            .iter()
            .map(|(tv, lbl, _)| format!("i32 {}, label %{}", tv, lbl))
            .collect();
        self.emit(&format!(
            "switch i32 {}, label %{} [ {} ]",
            tag,
            default_l,
            arms_str.join(" ")
        ));
        self.terminated = true;

        for (_tv, lbl, ai) in cases.clone() {
            self.label(&lbl);
            if let Pattern::Variant { name, bindings } = &arms[ai].patterns[0] {
                if let Some((_e, _t, fields)) = self.variants.get(name).cloned() {
                    if !fields.is_empty() {
                        let fieldtys: Vec<String> =
                            fields.iter().map(|(_, t)| t.clone()).collect();
                        let vstruct = format!("{{ {} }}", fieldtys.join(", "));
                        let pp = self.fresh();
                        self.emit(&format!("{} = bitcast i8* {} to {}*", pp, data, vstruct));
                        for (i, b) in bindings.iter().enumerate() {
                            let fty = fields[i].1.clone();
                            let fp = self.fresh();
                            self.emit(&format!(
                                "{} = getelementptr {}, {}* {}, i32 0, i32 {}",
                                fp, vstruct, vstruct, pp, i
                            ));
                            let lv = self.fresh();
                            self.emit(&format!("{} = load {}, {}* {}", lv, fty, fty, fp));
                            let addr = self.fresh();
                            self.emit(&format!("{} = alloca {}", addr, fty));
                            self.emit(&format!("store {} {}, {}* {}", fty, lv, fty, addr));
                            self.vars.insert(b.clone(), (addr, fty));
                        }
                    }
                }
            }
            self.gen_match_body(&arms[ai].body, &mode, &end_l)?;
        }

        self.label(&default_l);
        if let Some((_, ai)) = default_arm {
            self.gen_match_body(&arms[ai].body, &mode, &end_l)?;
        } else {
            self.emit("unreachable");
            self.terminated = true;
        }

        self.label(&end_l);
        if matches!(mode, MatchMode::Return) {
            self.emit("unreachable");
            self.terminated = true;
        }
        Ok(())
    }

    fn gen_match_body(
        &mut self,
        body: &MatchArmBody,
        mode: &MatchMode,
        end_l: &str,
    ) -> Result<(), String> {
        match body {
            MatchArmBody::Expr(e) => {
                let (v, _) = self.gen_expr(e)?;
                match mode {
                    MatchMode::Return => {
                        let rt = self.cur_ret.clone();
                        self.ret(&format!("{} {}", rt, v));
                    }
                    MatchMode::Assign(addr, ty) => {
                        self.emit(&format!("store {} {}, {}* {}", ty, v, ty, addr));
                        self.br(end_l);
                    }
                    MatchMode::Stmt => self.br(end_l),
                }
            }
            MatchArmBody::Block(b) => {
                for s in &b.stmts {
                    self.gen_stmt(s)?;
                }
                self.br(end_l);
            }
        }
        Ok(())
    }

    /// Gera shims + vtables: para cada struct que implementa cada interface.
    fn gen_impls(&mut self) {
        let ifaces: Vec<(String, Vec<(String, Vec<String>, String)>)> = self
            .interfaces
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let structs: Vec<String> = self.structs.keys().cloned().collect();
        for (iname, methods) in &ifaces {
            if methods.is_empty() {
                continue;
            }
            for sname in &structs {
                let implements = methods
                    .iter()
                    .all(|(m, _, _)| self.methods.contains_key(&(sname.clone(), m.clone())));
                if !implements {
                    continue;
                }
                let mut vt_entries = Vec::new();
                for (m, params, ret) in methods {
                    let shim = format!("@\"{}.{}$shim.{}\"", sname, m, iname);
                    let mut sig = vec!["i8* %self".to_string()];
                    for (i, p) in params.iter().enumerate() {
                        sig.push(format!("{} %a{}", p, i));
                    }
                    self.out
                        .push_str(&format!("define {} {}({}) {{\nentry:\n", ret, shim, sig.join(", ")));
                    self.out
                        .push_str(&format!("  %p = bitcast i8* %self to %{}*\n", sname));
                    self.out
                        .push_str(&format!("  %v = load %{}, %{}* %p\n", sname, sname));
                    let mut callargs = vec![format!("%{} %v", sname)];
                    for (i, p) in params.iter().enumerate() {
                        callargs.push(format!("{} %a{}", p, i));
                    }
                    if ret == "void" {
                        self.out.push_str(&format!(
                            "  call void @\"{}.{}\"({})\n  ret void\n",
                            sname,
                            m,
                            callargs.join(", ")
                        ));
                    } else {
                        self.out.push_str(&format!(
                            "  %r = call {} @\"{}.{}\"({})\n  ret {} %r\n",
                            ret,
                            sname,
                            m,
                            callargs.join(", "),
                            ret
                        ));
                    }
                    self.out.push_str("}\n\n");
                    let mut ptypes = vec!["i8*".to_string()];
                    ptypes.extend(params.clone());
                    vt_entries.push(format!("{} ({})* {}", ret, ptypes.join(", "), shim));
                }
                self.globals.push_str(&format!(
                    "@\"vtable.{}.{}\" = constant %vtable.{} {{ {} }}\n",
                    sname,
                    iname,
                    iname,
                    vt_entries.join(", ")
                ));
            }
        }
    }

    /// Converte (faz "box") um struct numa interface (aloca no heap + monta fat pointer).
    fn box_interface(&mut self, v: String, structty: &str, ifacety: &str) -> String {
        let sname = structty.trim_start_matches('%');
        let iname = ifacety.trim_start_matches('%');
        let szp = self.fresh();
        self.emit(&format!("{} = getelementptr {}, {}* null, i32 1", szp, structty, structty));
        let szi = self.fresh();
        self.emit(&format!("{} = ptrtoint {}* {} to i64", szi, structty, szp));
        self.needs.malloc = true;
        let raw = self.fresh();
        self.emit(&format!("{} = call i8* @malloc(i64 {})", raw, szi));
        let sp = self.fresh();
        self.emit(&format!("{} = bitcast i8* {} to {}*", sp, raw, structty));
        self.emit(&format!("store {} {}, {}* {}", structty, v, structty, sp));
        let i0 = self.fresh();
        self.emit(&format!("{} = insertvalue {} undef, i8* {}, 0", i0, ifacety, raw));
        let i1 = self.fresh();
        self.emit(&format!(
            "{} = insertvalue {} {}, i8* bitcast (%vtable.{}* @\"vtable.{}.{}\" to i8*), 1",
            i1, ifacety, i0, iname, sname, iname
        ));
        i1
    }

    fn coerce(&mut self, v: String, from: &str, to: &str) -> String {
        if from == to {
            return v;
        }
        if to.starts_with('%')
            && self.interfaces.contains_key(to.trim_start_matches('%'))
            && from.starts_with('%')
            && self.structs.contains_key(from.trim_start_matches('%'))
        {
            return self.box_interface(v, from, to);
        }
        v
    }

    fn gen_print(&mut self, args: &[Expr]) -> Result<(String, String), String> {
        if args.is_empty() {
            self.needs.puts = true;
            let p = self.string_ptr("");
            let r = self.fresh();
            self.emit(&format!("{} = call i32 @puts(i8* {})", r, p));
            return Ok(("0".to_string(), "void".to_string()));
        }
        // monta um printf com formato baseado nos tipos
        let mut specs = Vec::new();
        let mut argvals = Vec::new();
        for a in args {
            let (v, t) = self.gen_expr(a)?;
            match t.as_str() {
                "i8*" => {
                    specs.push("%s");
                    argvals.push(format!("i8* {}", v));
                }
                "double" => {
                    specs.push("%g");
                    argvals.push(format!("double {}", v));
                }
                "i1" => {
                    let z = self.fresh();
                    self.emit(&format!("{} = zext i1 {} to i64", z, v));
                    specs.push("%ld");
                    argvals.push(format!("i64 {}", z));
                }
                "i64" => {
                    specs.push("%ld");
                    argvals.push(format!("i64 {}", v));
                }
                other => {
                    return Err(format!("backend LLVM: `print` não suporta `{}`", other));
                }
            }
        }
        self.needs.printf = true;
        let fmt = format!("{}\n", specs.join(" "));
        let fp = self.string_ptr(&fmt);
        let r = self.fresh();
        let mut call_args = vec![format!("i8* {}", fp)];
        call_args.extend(argvals);
        self.emit(&format!(
            "{} = call i32 (i8*, ...) @printf({})",
            r,
            call_args.join(", ")
        ));
        Ok(("0".to_string(), "void".to_string()))
    }
}

fn type_base(t: &Type) -> String {
    match t {
        Type::Named(n) => n.clone(),
        Type::Generic(n, _) => n.clone(),
        Type::Slice(_) => "slice".to_string(),
    }
}

/// Infere a substituição de type params casando o tipo do parâmetro (AST) com o
/// tipo LLVM concreto do argumento. Ex.: `[]T` vs `{ i64*, i64 }` -> T = i64.
fn unify(tparams: &HashSet<String>, t: &Type, lltype: &str, subst: &mut HashMap<String, String>) {
    match t {
        Type::Named(n) if tparams.contains(n) => {
            subst.entry(n.clone()).or_insert_with(|| lltype.to_string());
        }
        Type::Slice(inner) => {
            let elem = Gen::slice_elem(lltype);
            unify(tparams, inner, &elem, subst);
        }
        _ => {}
    }
}

/// Deixa um tipo LLVM seguro como pedaço de nome de função (`{ i64*, i64 }` -> `_i64__i64_`).
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, parser};

    fn ir(src: &str) -> String {
        let prog = parser::parse(lexer::tokenize(src).unwrap()).unwrap();
        generate(&prog).unwrap()
    }

    #[test]
    fn emits_function_and_arithmetic() {
        let out = ir("fn add(a, b int): int { return a + b }");
        assert!(out.contains("define i64 @add(i64 %a, i64 %b)"));
        assert!(out.contains("add i64"));
    }

    #[test]
    fn emits_struct_and_method() {
        let out =
            ir("struct U { name string }\n fn (u U) hi(): string { return u.name }");
        assert!(out.contains("%U = type { i8* }"));
        assert!(out.contains("@\"U.hi\""));
        assert!(out.contains("extractvalue"));
    }

    #[test]
    fn emits_multi_return() {
        let out = ir("fn d(a, b int): (int, error) { return a, nil }");
        assert!(out.contains("define { i64, i8* } @d"));
        assert!(out.contains("insertvalue"));
    }

    #[test]
    fn emits_sqlite_driver() {
        use crate::module;
        use std::collections::HashSet;
        let src = "import \"std/db\"\n\
                   public fn main() {\n\
                   DB c = db.open(\"x.db\")\n\
                   db.exec(c, \"CREATE TABLE t (id INTEGER)\")\n\
                   Rows r = db.query(c, \"SELECT id FROM t\")\n\
                   for db.next(r) {\n\
                   int id = db.col_int(r, 0)\n\
                   print(id)\n\
                   }\n\
                   db.close(c)\n\
                   }";
        let mut prog = parser::parse(lexer::tokenize(src).unwrap()).unwrap();
        let pkgs: HashSet<String> = ["db".to_string()].into_iter().collect();
        module::normalize(&mut prog, &pkgs);
        let out = generate(&prog).unwrap();
        assert!(out.contains("declare i8* @vader_db_open(i8*)"));
        assert!(out.contains("call i8* @vader_db_open"));
        assert!(out.contains("call i8* @vader_db_query"));
        assert!(out.contains("call i64 @vader_db_col_int"));
        assert!(out.contains("call void @vader_db_close"));
    }

    #[test]
    fn emits_http_and_json() {
        use crate::module;
        use std::collections::HashSet;
        let src = "import \"std/http\"\n\
                   import \"std/json\"\n\
                   public fn main() {\n\
                   Server s = http.listen(8080)\n\
                   for http.accept(s) {\n\
                   Json o = json.object()\n\
                   json.set_str(o, \"p\", http.path(s))\n\
                   http.respond(s, 200, \"application/json\", json.encode(o))\n\
                   }\n\
                   }";
        let mut prog = parser::parse(lexer::tokenize(src).unwrap()).unwrap();
        let pkgs: HashSet<String> = ["http".to_string(), "json".to_string()].into_iter().collect();
        module::normalize(&mut prog, &pkgs);
        let out = generate(&prog).unwrap();
        assert!(out.contains("call i8* @vader_http_listen"));
        assert!(out.contains("@vader_http_respond"));
        assert!(out.contains("call i8* @vader_json_object"));
        assert!(out.contains("@vader_json_encode"));
        assert!(out.contains("declare i8* @vader_http_get"));
    }

    #[test]
    fn compiles_basics_and_shapes_to_ir() {
        // exemplos reais (structs/métodos/multi-retorno e enum/match) geram IR sem erro.
        for src in [
            include_str!("../examples/basics.vd"),
            include_str!("../examples/shapes.vd"),
        ] {
            let prog = parser::parse(lexer::tokenize(src).unwrap()).unwrap();
            generate(&prog).unwrap_or_else(|e| panic!("falhou: {}", e));
        }
    }

    #[test]
    fn emits_maps() {
        let out = ir("fn main() { map[string]int m = newmap()\n m[\"a\"] = 1\n print(m[\"a\"])\n print(len(m)) }");
        assert!(out.contains("@vader_map_make"));
        assert!(out.contains("@vader_map_set_str"));
        assert!(out.contains("@vader_map_get_str"));
        assert!(out.contains("@vader_map_len"));
    }

    #[test]
    fn emits_channels_and_goroutines() {
        let out = ir(
            "fn w(c chan[int]) { for x in c { print(x) } }\n\
             fn main() { chan[int] c = chan[int](4)\n spawn w(c)\n c <- 1\n close(c) }",
        );
        assert!(out.contains("@vader_chan_make"));
        assert!(out.contains("@vader_chan_send"));
        assert!(out.contains("@vader_chan_close"));
        assert!(out.contains("@vader_go"));
        assert!(out.contains("spawn$thunk$0"));
    }

    #[test]
    fn emits_generic_monomorphization() {
        let out = ir("fn id[T](x T): T { return x }\n fn main() { print(id(42)) }");
        assert!(out.contains("@\"id$i64\""));
        assert!(out.contains("define i64 @\"id$i64\"(i64 %x)"));
    }

    #[test]
    fn emits_interface_dispatch() {
        let out = ir("interface I { fn f(): int }\n struct S { x int }\n fn (s S) f(): int { return s.x }\n fn g(i I): int { return i.f() }\n fn main() { S s = S{ x: 1 }\n I a = s\n print(g(a)) }");
        assert!(out.contains("%vtable.I = type"));
        assert!(out.contains("$shim.I"));
        assert!(out.contains("vtable.S.I"));
    }

    #[test]
    fn emits_slices() {
        let out = ir("fn s(xs []int): int { return xs[0] }\n fn m(): int { []int a = [1, 2]\n return len(a) }");
        assert!(out.contains("getelementptr"));
        assert!(out.contains("@malloc"));
        assert!(out.contains("extractvalue"));
    }

    #[test]
    fn emits_float_and_string_concat() {
        let out = ir("fn f(): float { return 1.5 + 2.5 }\n fn g(): string { return \"a\" + \"b\" }");
        assert!(out.contains("fadd double"));
        assert!(out.contains("@vader_strcat"));
    }
}
