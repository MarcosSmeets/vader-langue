//! `vader migrate`: versioned migrations (SQL files).
//!
//! v1 (honest): `new`/`gen` generate files, `status` lists, `up`/`down` track
//! locally and SHOW the SQL. **Execution against a real database** depends on the
//! drivers (`std/db`), which are still under construction.

use std::fs;

use crate::ast::*;
use crate::module;

const DIR: &str = "migrations";

fn ensure_dir() -> Result<(), String> {
    fs::create_dir_all(DIR).map_err(|e| e.to_string())
}

fn slug(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect()
}

fn next_seq() -> usize {
    let mut max = 0;
    if let Ok(entries) = fs::read_dir(DIR) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if let Some(num) = n.split('_').next() {
                if let Ok(v) = num.parse::<usize>() {
                    if v > max {
                        max = v;
                    }
                }
            }
        }
    }
    max + 1
}

pub fn new_migration(name: &str) -> Result<(), String> {
    ensure_dir()?;
    let base = format!("{:04}_{}", next_seq(), slug(name));
    let up = format!("{}/{}.up.sql", DIR, base);
    let down = format!("{}/{}.down.sql", DIR, base);
    fs::write(&up, "-- up SQL\n").map_err(|e| e.to_string())?;
    fs::write(&down, "-- down SQL\n").map_err(|e| e.to_string())?;
    println!("created {}\ncreated {}", up, down);
    Ok(())
}

fn sql_type(t: &Type) -> &'static str {
    match t {
        Type::Named(n) => match n.as_str() {
            "int" => "integer",
            "float" => "double precision",
            "bool" => "boolean",
            _ => "text",
        },
        _ => "text",
    }
}

/// Generates a migration from the project's entities (structs). Review first!
pub fn gen(name: &str) -> Result<(), String> {
    let program = module::load(".", false)?;
    ensure_dir()?;
    let mut up = String::new();
    let mut down = String::new();
    for item in &program.items {
        if let Item::Struct(s) = item {
            if s.fields.is_empty() {
                continue; // skip types without fields (e.g. stdlib's Conn)
            }
            let table = format!("{}s", slug(&s.name));
            up.push_str(&format!("create table {} (\n", table));
            let cols: Vec<String> = s
                .fields
                .iter()
                .map(|f| format!("    {} {}", f.name, sql_type(&f.ty)))
                .collect();
            up.push_str(&cols.join(",\n"));
            up.push_str("\n);\n\n");
            down.push_str(&format!("drop table {};\n", table));
        }
    }
    if up.is_empty() {
        return Err("no struct found to generate a migration".into());
    }
    let base = format!("{:04}_{}", next_seq(), slug(name));
    let upf = format!("{}/{}.up.sql", DIR, base);
    let downf = format!("{}/{}.down.sql", DIR, base);
    fs::write(&upf, &up).map_err(|e| e.to_string())?;
    fs::write(&downf, &down).map_err(|e| e.to_string())?;
    println!("generated from entities (review before applying):\n  {}\n  {}", upf, downf);
    Ok(())
}

fn migrations() -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(DIR) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if let Some(base) = n.strip_suffix(".up.sql") {
                names.push(base.to_string());
            }
        }
    }
    names.sort();
    names
}

fn applied() -> Vec<String> {
    fs::read_to_string(format!("{}/.applied", DIR))
        .map(|s| s.lines().map(|l| l.to_string()).collect())
        .unwrap_or_default()
}

fn set_applied(list: &[String]) -> Result<(), String> {
    fs::write(format!("{}/.applied", DIR), list.join("\n")).map_err(|e| e.to_string())
}

pub fn status() -> Result<(), String> {
    let all = migrations();
    if all.is_empty() {
        println!("(no migrations — create one with `vader migrate new <name>`)");
        return Ok(());
    }
    let app = applied();
    for m in &all {
        let mark = if app.contains(m) {
            "\u{2713} applied"
        } else {
            "\u{25CB} pending"
        };
        println!("  {} {}", mark, m);
    }
    Ok(())
}

/// Migrations not yet applied, in order.
pub fn pending() -> Vec<String> {
    let app = applied();
    migrations()
        .into_iter()
        .filter(|m| !app.contains(m))
        .collect()
}

/// Up SQL of a migration.
pub fn up_sql(name: &str) -> String {
    fs::read_to_string(format!("{}/{}.up.sql", DIR, name)).unwrap_or_default()
}

/// Down SQL of a migration.
pub fn down_sql(name: &str) -> String {
    fs::read_to_string(format!("{}/{}.down.sql", DIR, name)).unwrap_or_default()
}

/// Marks a migration as applied (local tracking in `migrations/.applied`).
pub fn mark_applied(name: &str) -> Result<(), String> {
    let mut app = applied();
    if !app.iter().any(|m| m == name) {
        app.push(name.to_string());
    }
    set_applied(&app)
}

/// Removes a migration from the applied tracking.
pub fn unmark(name: &str) -> Result<(), String> {
    let mut app = applied();
    app.retain(|m| m != name);
    set_applied(&app)
}

/// Last applied migration (target of `down`).
pub fn last_applied() -> Option<String> {
    applied().pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_and_sql_type() {
        assert_eq!(slug("Create Users!"), "create_users_");
        assert_eq!(sql_type(&Type::Named("int".into())), "integer");
        assert_eq!(sql_type(&Type::Named("string".into())), "text");
    }
}
