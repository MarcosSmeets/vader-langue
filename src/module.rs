//! Module system v1: compiles a multi-file project as a single program.
//!
//! Strategy: joins all the project's `.vd` files and **normalizes qualified names**
//! (`domain.User` -> `User`, `usecase.CreateUser{...}` -> `CreateUser{...}`),
//! treating the project as a flat namespace. The recognized qualifiers are the
//! folder names + the last segment of the imports. Field access on variables
//! (`uc.repo`) is NOT affected — only `package.Symbol`.
//!
//! Prerequisite: unique type/function names in the project (which the scaffolds guarantee).

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
    // `vader.toml` dependencies: fetches them (git clone into the cache) and injects the `.vd` files
    let mut dep_packages: Vec<String> = Vec::new();
    if let Ok(toml) = std::fs::read_to_string(Path::new(dir).join("vader.toml")) {
        for d in crate::pkg::parse_deps(&toml) {
            let (dep_path, _commit) = crate::pkg::fetch(&d)?;
            dep_packages.push(d.name.clone());
            gather(&dep_path, false, &mut files)?;
        }
    }

    files.sort();

    let mut packages: HashSet<String> = HashSet::new();
    for p in dep_packages {
        packages.insert(p); // dep name = package for `import`/normalization
    }
    for f in &files {
        if let Some(parent) = f.parent().and_then(|p| p.file_name()) {
            packages.insert(parent.to_string_lossy().to_string());
        }
    }

    let mut items = Vec::new();
    let mut imports = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).map_err(|e| format!("{}: {}", f.display(), e))?;
        let tokens = lexer::tokenize(&src).map_err(|e| format!("{}: {}", f.display(), e))?;
        let prog = parser::parse(tokens).map_err(|e| format!("{}: {}", f.display(), e))?;
        for imp in &prog.imports {
            if let Some(seg) = imp.rsplit('/').next() {
                packages.insert(seg.to_string());
            }
        }
        imports.extend(prog.imports);
        items.extend(prog.items);
    }

    inject_stdlib(&imports, &mut items);

    let mut program = Program { imports, items };
    normalize(&mut program, &packages);
    Ok(program)
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

fn strip(name: &str, packages: &HashSet<String>) -> Option<String> {
    let (pkg, rest) = name.split_once('.')?;
    if packages.contains(pkg) {
        Some(rest.to_string())
    } else {
        None
    }
}

/// Removes the package qualifiers from the entire AST.
pub fn normalize(program: &mut Program, packages: &HashSet<String>) {
    for item in &mut program.items {
        match item {
            Item::Function(f) => {
                if let Some(r) = &mut f.receiver {
                    normalize_type(&mut r.ty, packages);
                }
                for p in &mut f.params {
                    normalize_type(&mut p.ty, packages);
                }
                for t in &mut f.returns {
                    normalize_type(t, packages);
                }
                normalize_block(&mut f.body, packages);
            }
            Item::Struct(s) => {
                for fld in &mut s.fields {
                    normalize_type(&mut fld.ty, packages);
                }
            }
            Item::Interface(it) => {
                for m in &mut it.methods {
                    for p in &mut m.params {
                        normalize_type(&mut p.ty, packages);
                    }
                    for t in &mut m.returns {
                        normalize_type(t, packages);
                    }
                }
            }
            Item::Enum(e) => {
                for v in &mut e.variants {
                    for fld in &mut v.fields {
                        normalize_type(&mut fld.ty, packages);
                    }
                }
            }
            Item::Test(t) => normalize_block(&mut t.body, packages),
        }
    }
}

fn normalize_type(t: &mut Type, packages: &HashSet<String>) {
    match t {
        Type::Named(n) => {
            if let Some(s) = strip(n, packages) {
                *n = s;
            }
        }
        Type::Slice(inner) => normalize_type(inner, packages),
        Type::Generic(name, args) => {
            if let Some(s) = strip(name, packages) {
                *name = s;
            }
            for a in args {
                normalize_type(a, packages);
            }
        }
    }
}

fn normalize_block(b: &mut Block, packages: &HashSet<String>) {
    for s in &mut b.stmts {
        normalize_stmt(s, packages);
    }
}

fn normalize_stmt(s: &mut Stmt, packages: &HashSet<String>) {
    match s {
        Stmt::VarDecl { decls, values, .. } => {
            for d in decls {
                normalize_type(&mut d.ty, packages);
            }
            for v in values {
                normalize_expr(v, packages);
            }
        }
        Stmt::Assign { target, value } => {
            normalize_expr(target, packages);
            normalize_expr(value, packages);
        }
        Stmt::Return(vs) => {
            for v in vs {
                normalize_expr(v, packages);
            }
        }
        Stmt::If {
            cond,
            then_block,
            else_block,
        } => {
            normalize_expr(cond, packages);
            normalize_block(then_block, packages);
            if let Some(eb) = else_block {
                normalize_block(eb, packages);
            }
        }
        Stmt::For { head, body } => {
            match head {
                ForHead::While(c) => normalize_expr(c, packages),
                ForHead::In { iter, .. } => normalize_expr(iter, packages),
                ForHead::Infinite => {}
            }
            normalize_block(body, packages);
        }
        Stmt::Spawn(c) => normalize_expr(c, packages),
        Stmt::Send { chan, value } => {
            normalize_expr(chan, packages);
            normalize_expr(value, packages);
        }
        Stmt::Assert(e) => normalize_expr(e, packages),
        Stmt::Expr(e) => normalize_expr(e, packages),
    }
}

fn normalize_expr(e: &mut Expr, packages: &HashSet<String>) {
    // `package.symbol` (Field whose base is a package Ident) -> `symbol`
    let replacement = if let ExprKind::Field { base, field } = &e.kind {
        match &base.kind {
            ExprKind::Ident(p) if packages.contains(p) => Some(field.clone()),
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
        ExprKind::Unary { expr, .. } => normalize_expr(expr, packages),
        ExprKind::Binary { left, right, .. } => {
            normalize_expr(left, packages);
            normalize_expr(right, packages);
        }
        ExprKind::Call { callee, args } => {
            normalize_expr(callee, packages);
            for a in args {
                normalize_expr(a, packages);
            }
        }
        ExprKind::Field { base, .. } => normalize_expr(base, packages),
        ExprKind::Index { base, index } => {
            normalize_expr(base, packages);
            normalize_expr(index, packages);
        }
        ExprKind::StructLit { name, fields } => {
            if let Some(s) = strip(name, packages) {
                *name = s;
            }
            for (_, fe) in fields {
                normalize_expr(fe, packages);
            }
        }
        ExprKind::SliceLit(elems) => {
            for el in elems {
                normalize_expr(el, packages);
            }
        }
        ExprKind::Recv(inner) => normalize_expr(inner, packages),
        ExprKind::Match { scrutinee, arms } => {
            normalize_expr(scrutinee, packages);
            for arm in arms {
                if let Some(g) = &mut arm.guard {
                    normalize_expr(g, packages);
                }
                match &mut arm.body {
                    MatchArmBody::Expr(ex) => normalize_expr(ex, packages),
                    MatchArmBody::Block(b) => normalize_block(b, packages),
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
        normalize(&mut prog, &packages(&["domain"]));
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
        normalize(&mut prog, &packages(&["src"]));
        let dump = format!("{:?}", prog);
        assert!(!dump.contains("\"src\""), "src should have been removed: {}", dump);
        // the field access on `u` (variable) must remain
        assert!(dump.contains("Field"));
    }
}
