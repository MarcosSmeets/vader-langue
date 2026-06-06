//! `vader migrate`: migrations versionadas (arquivos SQL).
//!
//! v1 (honesto): `new`/`gen` geram arquivos, `status` lista, `up`/`down` rastreiam
//! localmente e MOSTRAM o SQL. A **execução contra um banco real** depende dos
//! drivers (`std/db`), que ainda estão em construção.

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
    fs::write(&up, "-- SQL de subida\n").map_err(|e| e.to_string())?;
    fs::write(&down, "-- SQL de reversão\n").map_err(|e| e.to_string())?;
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

/// Gera uma migration a partir das entidades (structs) do projeto. Revise antes!
pub fn gen(name: &str) -> Result<(), String> {
    let program = module::load(".", false)?;
    ensure_dir()?;
    let mut up = String::new();
    let mut down = String::new();
    for item in &program.items {
        if let Item::Struct(s) = item {
            if s.fields.is_empty() {
                continue; // pula tipos sem campos (ex.: Conn da stdlib)
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
        return Err("nenhum struct encontrado para gerar migration".into());
    }
    let base = format!("{:04}_{}", next_seq(), slug(name));
    let upf = format!("{}/{}.up.sql", DIR, base);
    let downf = format!("{}/{}.down.sql", DIR, base);
    fs::write(&upf, &up).map_err(|e| e.to_string())?;
    fs::write(&downf, &down).map_err(|e| e.to_string())?;
    println!("gerado das entidades (revise antes de aplicar):\n  {}\n  {}", upf, downf);
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
        println!("(nenhuma migration — crie com `vader migrate new <nome>`)");
        return Ok(());
    }
    let app = applied();
    for m in &all {
        let mark = if app.contains(m) {
            "\u{2713} aplicada"
        } else {
            "\u{25CB} pendente"
        };
        println!("  {} {}", mark, m);
    }
    Ok(())
}

pub fn up() -> Result<(), String> {
    let all = migrations();
    let mut app = applied();
    let mut ran = 0;
    for m in &all {
        if !app.contains(m) {
            let sql = fs::read_to_string(format!("{}/{}.up.sql", DIR, m)).unwrap_or_default();
            println!("\u{25B6} {} (up):\n{}", m, sql.trim_end());
            app.push(m.clone());
            ran += 1;
        }
    }
    set_applied(&app)?;
    println!("\n{} migration(s) marcada(s) como aplicada(s) [rastreamento local].", ran);
    println!("nota: a execução no banco real depende dos drivers (std/db), em construção.");
    Ok(())
}

pub fn down() -> Result<(), String> {
    let mut app = applied();
    match app.pop() {
        Some(last) => {
            let sql = fs::read_to_string(format!("{}/{}.down.sql", DIR, last)).unwrap_or_default();
            println!("\u{25C0} {} (down):\n{}", last, sql.trim_end());
            set_applied(&app)?;
            println!("\nrevertida [local]. Execução real depende dos drivers (std/db).");
        }
        None => println!("nenhuma migration aplicada."),
    }
    Ok(())
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
