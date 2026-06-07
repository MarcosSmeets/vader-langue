//! Module system: compiles a multi-file project (plus its dependencies) as one program.
//!
//! Two namespacing strategies:
//! - **Project folders** are flattened: `domain.User` -> `User` (the scaffolds keep
//!   intra-project names unique, so this is safe and ergonomic).
//! - **Dependencies are namespaced**: each dep's own symbols are renamed `dep__Symbol`
//!   (definitions + internal references), and the project's `dep.Symbol` references
//!   resolve to the same mangled name. So two dependencies — or a dep and the project —
//!   can define the same `User`/`greet` without colliding.
//!
//! Field access on variables (`uc.repo`) is never affected — only `package.Symbol`.
//! Known limitation: enum *variant* names are not namespaced across deps yet.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::ast::*;
use crate::{lexer, parser};

/// Loads all `.vd` files from a directory as a single normalized `Program`.
/// `include_tests` controls whether `*_test.vd` files are included (yes for `vader test`).
pub fn load(dir: &str, include_tests: bool) -> Result<Program, String> {
    let mut files = Vec::new();
    gather(Path::new(dir), include_tests, &mut files)?;
    if files.is_empty() {
        return Err(format!("no .vd files under `{}`", dir));
    }

    let mut items = Vec::new();
    let mut imports = Vec::new();

    // `vader.toml` dependencies: each dependency is its own NAMESPACE. We parse its files,
    // collect the symbols it defines, and rename them `dep__Symbol` (definitions + internal
    // references), so two dependencies (or a dep and the project) can share a name safely.
    let mut dep_packages: HashSet<String> = HashSet::new();
    if let Ok(toml) = std::fs::read_to_string(Path::new(dir).join("vader.toml")) {
        for d in crate::pkg::parse_deps(&toml) {
            let (dep_path, _commit) = crate::pkg::fetch(&d)?;
            let mut dfiles = Vec::new();
            gather(&dep_path, false, &mut dfiles)?;
            dfiles.sort();
            let mut ditems = Vec::new();
            for f in &dfiles {
                let src = std::fs::read_to_string(f).map_err(|e| format!("{}: {}", f.display(), e))?;
                let tokens = lexer::tokenize(&src).map_err(|e| format!("{}: {}", f.display(), e))?;
                let prog = parser::parse(tokens).map_err(|e| format!("{}: {}", f.display(), e))?;
                imports.extend(prog.imports);
                ditems.extend(prog.items);
            }
            let owned = collect_defined(&ditems);
            for it in &mut ditems {
                mangle_item(it, &d.name, &owned);
            }
            dep_packages.insert(d.name.clone());
            items.extend(ditems);
        }
    }

    files.sort();

    // intra-project qualifiers (folder names + the last import segment) are flattened away.
    let mut folder_packages: HashSet<String> = HashSet::new();
    for f in &files {
        if let Some(parent) = f.parent().and_then(|p| p.file_name()) {
            folder_packages.insert(parent.to_string_lossy().to_string());
        }
    }
    for f in &files {
        let src = std::fs::read_to_string(f).map_err(|e| format!("{}: {}", f.display(), e))?;
        let tokens = lexer::tokenize(&src).map_err(|e| format!("{}: {}", f.display(), e))?;
        let prog = parser::parse(tokens).map_err(|e| format!("{}: {}", f.display(), e))?;
        for imp in &prog.imports {
            if let Some(seg) = imp.rsplit('/').next() {
                folder_packages.insert(seg.to_string());
            }
        }
        imports.extend(prog.imports);
        items.extend(prog.items);
    }

    inject_stdlib(&imports, &mut items);

    let ns = Ns { folder: folder_packages, dep: dep_packages };
    let mut program = Program { imports, items };
    normalize(&mut program, &ns);
    Ok(program)
}

/// Names a set of items defines (top-level functions, structs, enums, interfaces).
fn collect_defined(items: &[Item]) -> HashSet<String> {
    let mut s = HashSet::new();
    for it in items {
        match it {
            Item::Function(f) if f.receiver.is_none() => { s.insert(f.name.clone()); }
            Item::Struct(st) => { s.insert(st.name.clone()); }
            Item::Enum(e) => { s.insert(e.name.clone()); }
            Item::Interface(i) => { s.insert(i.name.clone()); }
            _ => {}
        }
    }
    s
}

fn mangled(prefix: &str, n: &str) -> String { format!("{}__{}", prefix, n) }

/// Renames a dependency's own symbols (and references to them) to `prefix__Symbol`.
fn mangle_item(it: &mut Item, prefix: &str, owned: &HashSet<String>) {
    match it {
        Item::Function(f) => {
            if f.receiver.is_none() && owned.contains(&f.name) {
                f.name = mangled(prefix, &f.name);
            }
            if let Some(r) = &mut f.receiver { mangle_type(&mut r.ty, prefix, owned); }
            for p in &mut f.params { mangle_type(&mut p.ty, prefix, owned); }
            for t in &mut f.returns { mangle_type(t, prefix, owned); }
            mangle_block(&mut f.body, prefix, owned);
        }
        Item::Struct(s) => {
            if owned.contains(&s.name) { s.name = mangled(prefix, &s.name); }
            for fld in &mut s.fields { mangle_type(&mut fld.ty, prefix, owned); }
        }
        Item::Enum(e) => {
            if owned.contains(&e.name) { e.name = mangled(prefix, &e.name); }
            for v in &mut e.variants {
                for fld in &mut v.fields { mangle_type(&mut fld.ty, prefix, owned); }
            }
        }
        Item::Interface(i) => {
            if owned.contains(&i.name) { i.name = mangled(prefix, &i.name); }
            for m in &mut i.methods {
                for p in &mut m.params { mangle_type(&mut p.ty, prefix, owned); }
                for t in &mut m.returns { mangle_type(t, prefix, owned); }
            }
        }
        Item::Test(t) => mangle_block(&mut t.body, prefix, owned),
    }
}

fn mangle_type(t: &mut Type, prefix: &str, owned: &HashSet<String>) {
    match t {
        Type::Named(n) => { if owned.contains(n) { *n = mangled(prefix, n); } }
        Type::Slice(inner) => mangle_type(inner, prefix, owned),
        Type::Generic(name, args) => {
            if owned.contains(name) { *name = mangled(prefix, name); }
            for a in args { mangle_type(a, prefix, owned); }
        }
    }
}

fn mangle_block(b: &mut Block, prefix: &str, owned: &HashSet<String>) {
    for s in &mut b.stmts { mangle_stmt(s, prefix, owned); }
}

fn mangle_stmt(s: &mut Stmt, prefix: &str, owned: &HashSet<String>) {
    match s {
        Stmt::VarDecl { decls, values, .. } => {
            for d in decls { mangle_type(&mut d.ty, prefix, owned); }
            for v in values { mangle_expr(v, prefix, owned); }
        }
        Stmt::Assign { target, value } => {
            mangle_expr(target, prefix, owned);
            mangle_expr(value, prefix, owned);
        }
        Stmt::Return(vs) => { for v in vs { mangle_expr(v, prefix, owned); } }
        Stmt::If { cond, then_block, else_block } => {
            mangle_expr(cond, prefix, owned);
            mangle_block(then_block, prefix, owned);
            if let Some(eb) = else_block { mangle_block(eb, prefix, owned); }
        }
        Stmt::For { head, body } => {
            match head {
                ForHead::While(c) => mangle_expr(c, prefix, owned),
                ForHead::In { iter, .. } => mangle_expr(iter, prefix, owned),
                ForHead::Infinite => {}
            }
            mangle_block(body, prefix, owned);
        }
        Stmt::Spawn(c) => mangle_expr(c, prefix, owned),
        Stmt::Send { chan, value } => {
            mangle_expr(chan, prefix, owned);
            mangle_expr(value, prefix, owned);
        }
        Stmt::Assert(e) => mangle_expr(e, prefix, owned),
        Stmt::Expr(e) => mangle_expr(e, prefix, owned),
    }
}

fn mangle_expr(e: &mut Expr, prefix: &str, owned: &HashSet<String>) {
    match &mut e.kind {
        ExprKind::Ident(n) => { if owned.contains(n) { *n = mangled(prefix, n); } }
        ExprKind::Unary { expr, .. } => mangle_expr(expr, prefix, owned),
        ExprKind::Binary { left, right, .. } => {
            mangle_expr(left, prefix, owned);
            mangle_expr(right, prefix, owned);
        }
        ExprKind::Call { callee, args } => {
            mangle_expr(callee, prefix, owned);
            for a in args { mangle_expr(a, prefix, owned); }
        }
        ExprKind::Field { base, .. } => mangle_expr(base, prefix, owned),
        ExprKind::Index { base, index } => {
            mangle_expr(base, prefix, owned);
            mangle_expr(index, prefix, owned);
        }
        ExprKind::StructLit { name, fields } => {
            if owned.contains(name) { *name = mangled(prefix, name); }
            for (_, fe) in fields { mangle_expr(fe, prefix, owned); }
        }
        ExprKind::SliceLit(elems) => { for el in elems { mangle_expr(el, prefix, owned); } }
        ExprKind::Recv(inner) => mangle_expr(inner, prefix, owned),
        ExprKind::Match { scrutinee, arms } => {
            mangle_expr(scrutinee, prefix, owned);
            for arm in arms {
                if let Some(g) = &mut arm.guard { mangle_expr(g, prefix, owned); }
                match &mut arm.body {
                    MatchArmBody::Expr(ex) => mangle_expr(ex, prefix, owned),
                    MatchArmBody::Block(b) => mangle_block(b, prefix, owned),
                }
            }
        }
        _ => {}
    }
}

/// Minimal stdlib: injects the types of the `std/...` packages the project imports,
/// so the code transpiles. (v1: only `std/db` -> `Conn`.)
fn inject_stdlib(imports: &[String], items: &mut Vec<Item>) {
    let uses_db = imports.iter().any(|i| i.starts_with("std/db"));
    let has_conn = items
        .iter()
        .any(|it| matches!(it, Item::Struct(s) if s.name == "Conn"));
    if uses_db && !has_conn {
        items.push(Item::Struct(StructDef {
            visibility: Visibility::Public,
            name: "Conn".to_string(),
            type_params: Vec::new(),
            fields: Vec::new(),
        }));
    }
}

fn gather(dir: &Path, include_tests: bool, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(name.as_str(), "target" | ".git" | "node_modules") {
                continue;
            }
            gather(&path, include_tests, out)?;
        } else if name.ends_with(".vd") {
            if !include_tests && name.ends_with("_test.vd") {
                continue;
            }
            out.push(path);
        }
    }
    Ok(())
}

/// Namespace resolution: project folders are flattened (`domain.User` -> `User`),
/// dependencies are mangled (`greeter.Hello` -> `greeter__Hello`) so names never collide.
pub struct Ns {
    pub folder: HashSet<String>,
    pub dep: HashSet<String>,
}
impl Ns {
    pub fn folders(folder: HashSet<String>) -> Ns {
        Ns { folder, dep: HashSet::new() }
    }
    /// Resolves a possibly-qualified `pkg.rest` name.
    fn resolve(&self, name: &str) -> Option<String> {
        let (pkg, rest) = name.split_once('.')?;
        self.rewrite(pkg, rest)
    }
    fn rewrite(&self, pkg: &str, rest: &str) -> Option<String> {
        if self.dep.contains(pkg) {
            Some(format!("{}__{}", pkg, rest))
        } else if self.folder.contains(pkg) {
            Some(rest.to_string())
        } else {
            None
        }
    }
}

/// Resolves package qualifiers across the AST (strip folders, mangle deps).
pub fn normalize(program: &mut Program, ns: &Ns) {
    for item in &mut program.items {
        match item {
            Item::Function(f) => {
                if let Some(r) = &mut f.receiver {
                    normalize_type(&mut r.ty, ns);
                }
                for p in &mut f.params {
                    normalize_type(&mut p.ty, ns);
                }
                for t in &mut f.returns {
                    normalize_type(t, ns);
                }
                normalize_block(&mut f.body, ns);
            }
            Item::Struct(s) => {
                for fld in &mut s.fields {
                    normalize_type(&mut fld.ty, ns);
                }
            }
            Item::Interface(it) => {
                for m in &mut it.methods {
                    for p in &mut m.params {
                        normalize_type(&mut p.ty, ns);
                    }
                    for t in &mut m.returns {
                        normalize_type(t, ns);
                    }
                }
            }
            Item::Enum(e) => {
                for v in &mut e.variants {
                    for fld in &mut v.fields {
                        normalize_type(&mut fld.ty, ns);
                    }
                }
            }
            Item::Test(t) => normalize_block(&mut t.body, ns),
        }
    }
}

fn normalize_type(t: &mut Type, ns: &Ns) {
    match t {
        Type::Named(n) => {
            if let Some(s) = ns.resolve(n) {
                *n = s;
            }
        }
        Type::Slice(inner) => normalize_type(inner, ns),
        Type::Generic(name, args) => {
            if let Some(s) = ns.resolve(name) {
                *name = s;
            }
            for a in args {
                normalize_type(a, ns);
            }
        }
    }
}

fn normalize_block(b: &mut Block, ns: &Ns) {
    for s in &mut b.stmts {
        normalize_stmt(s, ns);
    }
}

fn normalize_stmt(s: &mut Stmt, ns: &Ns) {
    match s {
        Stmt::VarDecl { decls, values, .. } => {
            for d in decls {
                normalize_type(&mut d.ty, ns);
            }
            for v in values {
                normalize_expr(v, ns);
            }
        }
        Stmt::Assign { target, value } => {
            normalize_expr(target, ns);
            normalize_expr(value, ns);
        }
        Stmt::Return(vs) => {
            for v in vs {
                normalize_expr(v, ns);
            }
        }
        Stmt::If {
            cond,
            then_block,
            else_block,
        } => {
            normalize_expr(cond, ns);
            normalize_block(then_block, ns);
            if let Some(eb) = else_block {
                normalize_block(eb, ns);
            }
        }
        Stmt::For { head, body } => {
            match head {
                ForHead::While(c) => normalize_expr(c, ns),
                ForHead::In { iter, .. } => normalize_expr(iter, ns),
                ForHead::Infinite => {}
            }
            normalize_block(body, ns);
        }
        Stmt::Spawn(c) => normalize_expr(c, ns),
        Stmt::Send { chan, value } => {
            normalize_expr(chan, ns);
            normalize_expr(value, ns);
        }
        Stmt::Assert(e) => normalize_expr(e, ns),
        Stmt::Expr(e) => normalize_expr(e, ns),
    }
}

fn normalize_expr(e: &mut Expr, ns: &Ns) {
    // `package.symbol` (Field whose base is a package Ident) -> `symbol`
    let replacement = if let ExprKind::Field { base, field } = &e.kind {
        match &base.kind {
            ExprKind::Ident(p) => ns.rewrite(p, field),
            _ => None,
        }
    } else {
        None
    };
    if let Some(f) = replacement {
        e.kind = ExprKind::Ident(f);
        return;
    }

    match &mut e.kind {
        ExprKind::Unary { expr, .. } => normalize_expr(expr, ns),
        ExprKind::Binary { left, right, .. } => {
            normalize_expr(left, ns);
            normalize_expr(right, ns);
        }
        ExprKind::Call { callee, args } => {
            normalize_expr(callee, ns);
            for a in args {
                normalize_expr(a, ns);
            }
        }
        ExprKind::Field { base, .. } => normalize_expr(base, ns),
        ExprKind::Index { base, index } => {
            normalize_expr(base, ns);
            normalize_expr(index, ns);
        }
        ExprKind::StructLit { name, fields } => {
            if let Some(s) = ns.resolve(name) {
                *name = s;
            }
            for (_, fe) in fields {
                normalize_expr(fe, ns);
            }
        }
        ExprKind::SliceLit(elems) => {
            for el in elems {
                normalize_expr(el, ns);
            }
        }
        ExprKind::Recv(inner) => normalize_expr(inner, ns),
        ExprKind::Match { scrutinee, arms } => {
            normalize_expr(scrutinee, ns);
            for arm in arms {
                if let Some(g) = &mut arm.guard {
                    normalize_expr(g, ns);
                }
                match &mut arm.body {
                    MatchArmBody::Expr(ex) => normalize_expr(ex, ns),
                    MatchArmBody::Block(b) => normalize_block(b, ns),
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packages(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn strips_qualified_types_and_struct_lits() {
        let mut prog = parser::parse(
            lexer::tokenize(
                "fn f(): domain.User {\n    domain.User u = domain.User{ id: 1 }\n    return u\n}",
            )
            .unwrap(),
        )
        .unwrap();
        normalize(&mut prog, &Ns::folders(packages(&["domain"])));
        let dump = format!("{:?}", prog);
        assert!(!dump.contains("domain."), "qualifier left over: {}", dump);
        assert!(dump.contains("\"User\""));
    }

    #[test]
    fn rewrites_qualified_call_but_not_variable_field() {
        // `src.greet(x)` -> `greet(x)`, but `u.name` (variable) remains.
        let mut prog = parser::parse(
            lexer::tokenize("fn f(u User): string {\n    print(src.greet(u.name))\n    return u.name\n}")
                .unwrap(),
        )
        .unwrap();
        normalize(&mut prog, &Ns::folders(packages(&["src"])));
        let dump = format!("{:?}", prog);
        assert!(!dump.contains("\"src\""), "src should have been removed: {}", dump);
        // the field access on `u` (variable) must remain
        assert!(dump.contains("Field"));
    }

    #[test]
    fn dependency_symbols_are_namespaced() {
        // A dependency's own symbols are mangled `dep__Name`, and the project's
        // qualified references resolve to the same mangled name.
        let mut dep = parser::parse(
            lexer::tokenize("public fn Hello(): string {\n    return greet()\n}\nfn greet(): string {\n    return \"hi\"\n}")
                .unwrap(),
        )
        .unwrap();
        let owned = collect_defined(&dep.items);
        for it in &mut dep.items {
            mangle_item(it, "greeter", &owned);
        }
        let dump = format!("{:?}", dep.items);
        assert!(dump.contains("greeter__Hello"), "dep fn not mangled: {}", dump);
        assert!(dump.contains("greeter__greet"), "internal call not mangled: {}", dump);

        // project reference `greeter.Hello()` resolves to greeter__Hello
        let mut proj =
            parser::parse(lexer::tokenize("fn main() {\n    print(greeter.Hello())\n}").unwrap()).unwrap();
        let ns = Ns { folder: HashSet::new(), dep: packages(&["greeter"]) };
        normalize(&mut proj, &ns);
        let pd = format!("{:?}", proj);
        assert!(pd.contains("greeter__Hello"), "project ref not resolved: {}", pd);
    }
}
