//! The `vader` command-line compiler.
//!
//! Subcommands:
//!   vader lex   <file.vd>   print the token stream
//!   vader parse <file.vd>   print the AST
//!   vader check <file.vd>   type-check and report errors
//!   vader build <file.vd>   compile to a native binary (via Go)
//!   vader run   <file.vd>   compile and run

use std::path::Path;
use std::process::{Command, ExitCode};

/// Runtime de concorrência (C), embutido no binário e linkado pelo clang quando há canais.
const RUNTIME_C: &str = include_str!("../runtime/vader_rt.c");
/// SQLite (amalgamation, domínio público) + wrapper, embutidos e linkados quando usa `std/db`.
const SQLITE_C: &str = include_str!("../runtime/sqlite/sqlite3.c");
const SQLITE_H: &str = include_str!("../runtime/sqlite/sqlite3.h");
const VADER_DB_C: &str = include_str!("../runtime/vader_db.c");
/// Driver Postgres (wire protocol puro: TCP + auth SCRAM + simple query).
const VADER_PG_C: &str = include_str!("../runtime/vader_pg.c");
/// Driver MySQL/MariaDB (protocolo nativo + mysql_native_password).
const VADER_MYSQL_C: &str = include_str!("../runtime/vader_mysql.c");
/// stdlib: HTTP (servidor + cliente) e JSON (parse/encode), linkados sob demanda.
const VADER_HTTP_C: &str = include_str!("../runtime/vader_http.c");
const VADER_JSON_C: &str = include_str!("../runtime/vader_json.c");

use vader::ast::Program;
use vader::{
    check, codegen, formatter, gen, lexer, lint, llvm, migrate, module, parser, pkg, scaffold,
    templates,
};

fn usage() {
    eprintln!("vader {} — compiler (Fase 1)", env!("CARGO_PKG_VERSION"));
    eprintln!("usage:");
    eprintln!("  vader new <kind> <name> [--arch <arch>]   scaffold a project");
    eprintln!("       kind: api|worker|cli|lib   arch: clean|hexagonal|mvc|minimal");
    eprintln!("  vader new --template <tmpl> <name>   scaffold from a custom template");
    eprintln!("  vader template list                  list custom templates");
    eprintln!("  vader template save <tmpl> <dir>     save a folder as a template");
    eprintln!("  vader gen <thing> <Name>   generate an artifact + its test mirror");
    eprintln!("       thing: fn|struct|usecase|handler");
    eprintln!("  vader fmt [-w] <file.vd>   format (stdout, or -w to rewrite the file)");
    eprintln!("  vader lint <file.vd> [--arch <arch>]   check architecture rules");
    eprintln!("  vader migrate <new|gen|status|up|down> [name]   manage SQL migrations");
    eprintln!("  vader add <git-url|path>[@version] [name]   add a dependency (git/URL)");
    eprintln!("  vader add <name> [--registry <dir|git-url>]  add by name via a registry");
    eprintln!("  vader remove <name>        remove a dependency");
    eprintln!("  vader publish [--registry <dir|git-url>]   register this package in a registry");
    eprintln!("  vader test <file.vd>       run test blocks + coverage report");
    eprintln!("       --min-coverage <n>  override the gate threshold");
    eprintln!("       --no-gate           do not fail on low coverage");
    eprintln!("       --install-hook      write a git pre-push hook that runs this");
    eprintln!("  vader lex   <file.vd>   print the token stream");
    eprintln!("  vader parse <file.vd>   print the AST");
    eprintln!("  vader check <file.vd>   type-check and report errors");
    eprintln!("  vader build <file.vd>   compile to a native binary (via Go)");
    eprintln!("  vader run   <file.vd>   compile and run");
    eprintln!("  vader llvm  <file.vd>   compile via LLVM IR + clang (no Go) and run");
    eprintln!("  vader lsp              language server (stdio) — diagnostics for editors");
    eprintln!("  vader version          print the version");
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    if matches!(
        args.get(1).map(String::as_str),
        Some("version") | Some("--version") | Some("-V")
    ) {
        println!("vader {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    if args.get(1).map(String::as_str) == Some("new") {
        return cmd_new(&args);
    }
    if args.get(1).map(String::as_str) == Some("gen") {
        return cmd_gen(&args);
    }
    if args.get(1).map(String::as_str) == Some("template") {
        return cmd_template(&args);
    }
    if args.get(1).map(String::as_str) == Some("lint") {
        return cmd_lint(&args);
    }
    if args.get(1).map(String::as_str) == Some("migrate") {
        return cmd_migrate(&args);
    }
    if args.get(1).map(String::as_str) == Some("llvm") {
        return cmd_llvm(&args);
    }
    if args.get(1).map(String::as_str) == Some("lsp") {
        vader::lsp::run();
        return ExitCode::SUCCESS;
    }
    if args.get(1).map(String::as_str) == Some("add") {
        return cmd_add(&args);
    }
    if args.get(1).map(String::as_str) == Some("remove") {
        return cmd_remove(&args);
    }
    if args.get(1).map(String::as_str) == Some("publish") {
        return cmd_publish(&args);
    }
    if args.get(1).map(String::as_str) == Some("fmt") {
        return cmd_fmt(&args);
    }
    if args.get(1).map(String::as_str) == Some("test") {
        return cmd_test(&args);
    }

    let valid =
        args.len() >= 3 && matches!(args[1].as_str(), "lex" | "parse" | "check" | "build" | "run");
    if !valid {
        usage();
        return ExitCode::FAILURE;
    }

    let command = args[1].as_str();
    let path = &args[2];

    // Diretório: compila o projeto inteiro via sistema de módulos.
    if Path::new(path).is_dir() {
        if !matches!(command, "check" | "build" | "run") {
            eprintln!("`{}` works on a single file, not a directory", command);
            return ExitCode::FAILURE;
        }
        let program = match module::load(path, false) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{}", e);
                return ExitCode::FAILURE;
            }
        };
        return finish(command, path, program, true);
    }

    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{}`: {}", path, e);
            return ExitCode::FAILURE;
        }
    };

    let tokens = match lexer::tokenize(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        }
    };

    if command == "lex" {
        for t in &tokens {
            println!("{:>4}:{:<4} {:?}", t.line, t.col, t.kind);
        }
        return ExitCode::SUCCESS;
    }

    let program = match parser::parse(tokens) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        }
    };

    if command == "parse" {
        println!("{:#?}", program);
        return ExitCode::SUCCESS;
    }

    finish(command, path, program, false)
}

/// Pós-parse: type-check, lint de arquitetura e (build/run) gera Go e compila.
fn finish(command: &str, path: &str, program: Program, is_dir: bool) -> ExitCode {
    if let Err(errors) = check::check(&program) {
        for e in &errors {
            eprintln!("type error at {}:{}: {}", e.line, e.col, e.message);
        }
        eprintln!("{} type error(s)", errors.len());
        return ExitCode::FAILURE;
    }

    // arquitetura: fiscaliza automaticamente (só arquivo solto; dir não tem 1 camada)
    if !is_dir && !lint_gate(path, &program.imports) {
        eprintln!("architecture violation(s); aborting");
        return ExitCode::FAILURE;
    }

    if command == "check" {
        println!("ok: no type errors in `{}`", path);
        return ExitCode::SUCCESS;
    }

    let go_src = match codegen::generate(&program) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        }
    };

    let tmp = std::env::temp_dir().join("vader_build");
    if let Err(e) = std::fs::create_dir_all(&tmp) {
        eprintln!("error: cannot create temp dir: {}", e);
        return ExitCode::FAILURE;
    }
    let go_file = tmp.join("main.go");
    if let Err(e) = std::fs::write(&go_file, &go_src) {
        eprintln!("error: cannot write generated Go: {}", e);
        return ExitCode::FAILURE;
    }

    let result = if command == "run" {
        Command::new("go").arg("run").arg(&go_file).status()
    } else {
        let stem = Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "a.out".to_string());
        // num projeto (dir), o binário sai DENTRO da pasta pra não colidir com ela.
        let out = if is_dir {
            Path::new(path).join(&stem)
        } else {
            std::path::PathBuf::from(&stem)
        };
        let shown = out.display().to_string();
        Command::new("go")
            .arg("build")
            .arg("-o")
            .arg(&out)
            .arg(&go_file)
            .status()
            .map(|s| {
                if s.success() {
                    println!("built {}", shown);
                }
                s
            })
    };

    match result {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: failed to invoke `go`: {} (is Go on PATH?)", e);
            ExitCode::FAILURE
        }
    }
}

/// `vader add <git-url|path>[@version] [name]` — adiciona uma dependência (git/URL).
fn cmd_add(args: &[String]) -> ExitCode {
    let src = match args.get(2) {
        Some(s) => s,
        None => {
            eprintln!("usage: vader add <git-url|path>[@version] [name]");
            return ExitCode::FAILURE;
        }
    };
    // `add <nome>` (sem barra/scheme) resolve pelo registro; senão é URL/caminho.
    let dep = if is_bare_name(src) {
        let reg = match resolve_registry(args) {
            Some(r) => r,
            None => {
                eprintln!(
                    "`{}` parece um nome de pacote — passe --registry <dir|git-url>, defina VADER_REGISTRY, ou use a URL/caminho completo",
                    src
                );
                return ExitCode::FAILURE;
            }
        };
        match pkg::registry_lookup(&reg, src) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("error: {}", e);
                return ExitCode::FAILURE;
            }
        }
    } else {
        let (url, version) = pkg::split_source(src);
        let name = args
            .get(3)
            .filter(|a| !a.starts_with("--"))
            .cloned()
            .unwrap_or_else(|| pkg::derive_name(&url));
        pkg::Dep { name, url, version }
    };
    let name = dep.name.clone();
    let url = dep.url.clone();
    let version = dep.version.clone();

    println!("buscando `{}` de {}...", name, url);
    let commit = match pkg::fetch(&dep) {
        Ok((_dir, c)) => c,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::FAILURE;
        }
    };

    let toml = std::fs::read_to_string("vader.toml").unwrap_or_default();
    let mut deps = pkg::parse_deps(&toml);
    deps.retain(|d| d.name != name);
    deps.push(dep);
    if let Err(e) = std::fs::write("vader.toml", pkg::write_deps(&toml, &deps)) {
        eprintln!("error: cannot write vader.toml: {}", e);
        return ExitCode::FAILURE;
    }
    write_lock(&deps);
    let short = &commit[..commit.len().min(10)];
    let vshown = if version.is_empty() { "default" } else { &version };
    println!("added `{}` ({}@{}) -> {}", name, url, vshown, short);
    ExitCode::SUCCESS
}

/// `vader remove <name>` — remove uma dependência do `vader.toml`.
fn cmd_remove(args: &[String]) -> ExitCode {
    let name = match args.get(2) {
        Some(s) => s,
        None => {
            eprintln!("usage: vader remove <name>");
            return ExitCode::FAILURE;
        }
    };
    let toml = std::fs::read_to_string("vader.toml").unwrap_or_default();
    let mut deps = pkg::parse_deps(&toml);
    let before = deps.len();
    deps.retain(|d| &d.name != name);
    if deps.len() == before {
        eprintln!("`{}` não está nas dependências", name);
        return ExitCode::FAILURE;
    }
    let _ = std::fs::write("vader.toml", pkg::write_deps(&toml, &deps));
    write_lock(&deps);
    println!("removed `{}`", name);
    ExitCode::SUCCESS
}

/// `vader publish [--registry <dir|git-url>]` — registra o pacote atual no índice.
fn cmd_publish(args: &[String]) -> ExitCode {
    let registry = match resolve_registry(args) {
        Some(r) => r,
        None => {
            eprintln!("informe o registro: --registry <dir|git-url> ou a env VADER_REGISTRY");
            return ExitCode::FAILURE;
        }
    };
    let toml = std::fs::read_to_string("vader.toml").unwrap_or_default();
    let name = match toml_top_name(&toml) {
        Some(n) => n,
        None => {
            eprintln!("vader.toml sem `name = \"...\"` — rode na raiz do projeto");
            return ExitCode::FAILURE;
        }
    };
    let url = git_output(&["remote", "get-url", "origin"]).unwrap_or_default();
    if url.is_empty() {
        eprintln!("sem `git remote origin` — o pacote precisa de um repositório git");
        return ExitCode::FAILURE;
    }
    let version = git_output(&["describe", "--tags", "--abbrev=0"]).unwrap_or_default();
    let dep = pkg::Dep {
        name: name.clone(),
        url: url.clone(),
        version: version.clone(),
    };
    match pkg::registry_publish(&registry, &dep) {
        Ok(()) => {
            let v = if version.is_empty() {
                "(sem tag)".to_string()
            } else {
                version
            };
            println!("publicado `{}` -> {} @ {} no registro {}", name, url, v, registry);
            println!("(se o registro for um repo git, faça commit+push do index.json)");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Registro alvo: flag `--registry <reg>` ou env `VADER_REGISTRY`.
fn resolve_registry(args: &[String]) -> Option<String> {
    if let Some(i) = args.iter().position(|a| a == "--registry") {
        return args.get(i + 1).cloned();
    }
    std::env::var("VADER_REGISTRY").ok()
}

/// Heurística: um nome de pacote não tem barra, scheme nem ponto (≠ URL/caminho).
fn is_bare_name(s: &str) -> bool {
    !s.is_empty()
        && !s.contains('/')
        && !s.contains('\\')
        && !s.contains(':')
        && !s.contains('.')
}

/// Lê `name = "..."` do topo do `vader.toml` (antes de qualquer seção).
fn toml_top_name(toml: &str) -> Option<String> {
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            break;
        }
        if let Some((k, v)) = t.split_once('=') {
            if k.trim() == "name" {
                return Some(v.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// Roda `git <args>` e devolve o stdout (trim), ou None em falha/vazio.
fn git_output(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Regenera o `vader.lock` com os commits resolvidos de cada dependência.
fn write_lock(deps: &[pkg::Dep]) {
    let mut out = String::from("# vader.lock — gerado automaticamente; commits resolvidos\n");
    for d in deps {
        if let Ok((_p, commit)) = pkg::fetch(d) {
            out.push_str(&format!("{} = \"{}@{}\"\n", d.name, d.url, commit));
        }
    }
    let _ = std::fs::write("vader.lock", out);
}

/// `vader llvm <file.vd>` — Vader -> LLVM IR (texto) -> clang -> binário nativo -> roda.
fn cmd_llvm(args: &[String]) -> ExitCode {
    let path = match args.iter().skip(2).find(|a| !a.starts_with("--")) {
        Some(p) => p,
        None => {
            usage();
            return ExitCode::FAILURE;
        }
    };
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{}`: {}", path, e);
            return ExitCode::FAILURE;
        }
    };
    let tls = args.iter().any(|a| a == "--tls");
    match build_run_source(&source, false, tls) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::FAILURE
        }
    }
}

/// Compila uma fonte Vader via LLVM + clang e roda o binário (Result, p/ reuso).
/// `quiet` suprime os logs de progresso (usado pelo `vader migrate`).
fn build_run_source(source: &str, quiet: bool, tls: bool) -> Result<(), String> {
    let tokens = lexer::tokenize(source).map_err(|e| e.to_string())?;
    let mut program = parser::parse(tokens).map_err(|e| e.to_string())?;
    if !program.imports.is_empty() {
        let packages: std::collections::HashSet<String> = program
            .imports
            .iter()
            .filter_map(|i| i.rsplit('/').next().map(|s| s.to_string()))
            .collect();
        module::normalize(&mut program, &packages);
    }
    if let Err(errors) = check::check(&program) {
        let msg = errors
            .iter()
            .map(|e| format!("{}:{}: {}", e.line, e.col, e.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("type error: {}", msg));
    }
    let ir = llvm::generate(&program)?;

    let dir = std::env::temp_dir().join("vader_llvm");
    std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {}", e))?;
    let ll = dir.join("out.ll");
    let bin = dir.join("out");
    std::fs::write(&ll, &ir).map_err(|e| format!("write IR: {}", e))?;
    if !quiet {
        println!("emitted LLVM IR: {}", ll.display());
    }

    let mut cmd = Command::new("clang");
    cmd.arg("-Wno-override-module").arg(&ll);
    if ir.contains("@vader_") {
        let rt = dir.join("vader_rt.c");
        std::fs::write(&rt, RUNTIME_C).map_err(|e| format!("write runtime: {}", e))?;
        cmd.arg(&rt).arg("-lpthread");
        if !quiet {
            println!("(linkando runtime de concorrência)");
        }
    }
    if ir.contains("@vader_db_") {
        let hdr = dir.join("sqlite3.h");
        let obj = dir.join("sqlite3.o");
        std::fs::write(&hdr, SQLITE_H).map_err(|e| format!("write sqlite3.h: {}", e))?;
        if !obj.exists() {
            let src = dir.join("sqlite3.c");
            std::fs::write(&src, SQLITE_C).map_err(|e| format!("write sqlite3.c: {}", e))?;
            if !quiet {
                println!("(compilando SQLite embarcado — só na primeira vez)");
            }
            let st = Command::new("clang")
                .arg("-c")
                .arg("-O2")
                .arg(&src)
                .arg("-o")
                .arg(&obj)
                .current_dir(&dir)
                .status();
            match st {
                Ok(s) if s.success() => {}
                _ => return Err("clang falhou ao compilar o SQLite".into()),
            }
        }
        let db_c = dir.join("vader_db.c");
        let pg_c = dir.join("vader_pg.c");
        let my_c = dir.join("vader_mysql.c");
        std::fs::write(&db_c, VADER_DB_C).map_err(|e| format!("write vader_db.c: {}", e))?;
        std::fs::write(&pg_c, VADER_PG_C).map_err(|e| format!("write vader_pg.c: {}", e))?;
        std::fs::write(&my_c, VADER_MYSQL_C).map_err(|e| format!("write vader_mysql.c: {}", e))?;
        cmd.arg(&obj).arg(&db_c).arg(&pg_c).arg(&my_c);
        if tls {
            // TLS pro Postgres (cloud): habilita o caminho OpenSSL no driver.
            cmd.arg("-DVADER_TLS").arg("-lssl").arg("-lcrypto");
        }
        cmd.arg("-lpthread").arg("-ldl").arg("-lm");
        if !quiet {
            println!("(linkando SQLite + Postgres + MySQL{})", if tls { " + TLS" } else { "" });
        }
    }
    if ir.contains("@vader_http_") {
        let c = dir.join("vader_http.c");
        std::fs::write(&c, VADER_HTTP_C).map_err(|e| format!("write vader_http.c: {}", e))?;
        cmd.arg(&c);
        if !quiet {
            println!("(linkando std/http)");
        }
    }
    if ir.contains("@vader_json_") {
        let c = dir.join("vader_json.c");
        std::fs::write(&c, VADER_JSON_C).map_err(|e| format!("write vader_json.c: {}", e))?;
        cmd.arg(&c);
        if !quiet {
            println!("(linkando std/json)");
        }
    }
    cmd.arg("-o").arg(&bin);
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(_) => return Err("clang falhou ao compilar o IR".into()),
        Err(e) => return Err(format!("falha ao invocar `clang`: {} (no PATH?)", e)),
    }
    if !quiet {
        println!("compiled with clang -> {}\n--- running ---", bin.display());
    }
    match Command::new(&bin).status() {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("o programa saiu com código {}", s.code().unwrap_or(-1))),
        Err(e) => Err(format!("falha ao rodar o binário: {}", e)),
    }
}

/// `vader migrate <new|gen|status|up|down> [name]`
fn cmd_migrate(args: &[String]) -> ExitCode {
    let name = args.get(3);
    let result = match args.get(2).map(String::as_str) {
        Some("new") => match name {
            Some(n) => migrate::new_migration(n),
            None => {
                usage();
                return ExitCode::FAILURE;
            }
        },
        Some("gen") => match name {
            Some(n) => migrate::gen(n),
            None => {
                usage();
                return ExitCode::FAILURE;
            }
        },
        Some("status") => migrate::status(),
        Some("up") => migrate_run(args, true),
        Some("down") => migrate_run(args, false),
        _ => {
            usage();
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Executa migrations de verdade: gera um programinha Vader que abre o banco e roda o
/// SQL via `db.must` (aborta se o SQL falhar), compila e roda. Só marca como aplicada
/// se o processo sair com sucesso.
fn migrate_run(args: &[String], up: bool) -> Result<(), String> {
    let dsn = resolve_migrate_dsn(args).ok_or(
        "informe o banco: `vader migrate up --db <dsn>` ou defina [database] url no vader.toml",
    )?;
    let tls = args.iter().any(|a| a == "--tls");
    if up {
        let pend = migrate::pending();
        if pend.is_empty() {
            println!("nada pendente — tudo aplicado.");
            return Ok(());
        }
        for name in pend {
            println!("\u{25B6} aplicando {} ...", name);
            build_run_source(&migration_program(&dsn, &migrate::up_sql(&name)), true, tls)
                .map_err(|e| format!("falha em {}: {}", name, e))?;
            migrate::mark_applied(&name)?;
        }
        println!("ok — migrations aplicadas em {}", dsn);
    } else {
        match migrate::last_applied() {
            None => println!("nenhuma migration aplicada."),
            Some(name) => {
                println!("\u{25C0} revertendo {} ...", name);
                build_run_source(&migration_program(&dsn, &migrate::down_sql(&name)), true, tls)
                    .map_err(|e| format!("falha ao reverter {}: {}", name, e))?;
                migrate::unmark(&name)?;
                println!("ok — revertida {}", name);
            }
        }
    }
    Ok(())
}

/// Resolve o DSN do banco: flag `--db <dsn>` ou `[database] url` no `vader.toml`.
fn resolve_migrate_dsn(args: &[String]) -> Option<String> {
    if let Some(i) = args.iter().position(|a| a == "--db") {
        return args.get(i + 1).cloned();
    }
    let toml = std::fs::read_to_string("vader.toml").ok()?;
    let mut in_db = false;
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_db = t == "[database]";
            continue;
        }
        if in_db {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim() == "url" {
                    return Some(v.trim().trim_matches('"').to_string());
                }
            }
        }
    }
    None
}

/// Escapa uma string pra um literal Vader.
fn esc_vd(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o
}

/// Gera o programa Vader que aplica um bloco de SQL num banco (via `db.must`).
fn migration_program(dsn: &str, sql: &str) -> String {
    format!(
        "import \"std/db\"\npublic fn main() {{\n    DB __c = db.open(\"{}\")\n    db.must(__c, \"{}\")\n    db.close(__c)\n}}\n",
        esc_vd(dsn),
        esc_vd(sql)
    )
}

/// `vader template <list|save ...>`
fn cmd_template(args: &[String]) -> ExitCode {
    match args.get(2).map(String::as_str) {
        Some("list") => {
            let ts = templates::list();
            if ts.is_empty() {
                println!("(nenhum template — crie com `vader template save <nome> <pasta>`)");
            } else {
                for t in ts {
                    println!("  {}", t);
                }
            }
            ExitCode::SUCCESS
        }
        Some("save") => match (args.get(3), args.get(4)) {
            (Some(name), Some(dir)) => match templates::save(name, dir) {
                Ok(n) => {
                    println!("saved template `{}` ({} files)", name, n);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    ExitCode::FAILURE
                }
            },
            _ => {
                usage();
                ExitCode::FAILURE
            }
        },
        _ => {
            usage();
            ExitCode::FAILURE
        }
    }
}

/// `vader new <kind> <name> [--arch <arch>]` ou `vader new --template <tmpl> <name>`
fn cmd_new(args: &[String]) -> ExitCode {
    // modo template customizado
    if let Some(pos) = args.iter().position(|a| a == "--template") {
        let tmpl = match args.get(pos + 1) {
            Some(t) => t,
            None => {
                usage();
                return ExitCode::FAILURE;
            }
        };
        let name = args
            .iter()
            .skip(2)
            .find(|a| !a.starts_with("--") && *a != tmpl);
        let name = match name {
            Some(n) => n,
            None => {
                usage();
                return ExitCode::FAILURE;
            }
        };
        return match templates::create_from(tmpl, name) {
            Ok(created) => {
                println!("created `{}` from template `{}`:", name, tmpl);
                for path in &created {
                    println!("  {}", path);
                }
                println!("\nnext:\n  cd {}", name);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {}", e);
                ExitCode::FAILURE
            }
        };
    }

    if args.len() < 4 {
        usage();
        return ExitCode::FAILURE;
    }
    let kind = &args[2];
    let name = &args[3];

    if !matches!(kind.as_str(), "api" | "worker" | "cli" | "lib") {
        eprintln!("error: unknown kind `{}` (api|worker|cli|lib)", kind);
        return ExitCode::FAILURE;
    }

    let mut arch = scaffold::default_arch(kind).to_string();
    let mut i = 4;
    while i < args.len() {
        if args[i] == "--arch" && i + 1 < args.len() {
            arch = args[i + 1].clone();
            i += 2;
        } else {
            eprintln!("error: unexpected argument `{}`", args[i]);
            return ExitCode::FAILURE;
        }
    }
    if !matches!(arch.as_str(), "clean" | "hexagonal" | "mvc" | "minimal") {
        eprintln!("error: unknown arch `{}` (clean|hexagonal|mvc|minimal)", arch);
        return ExitCode::FAILURE;
    }

    match scaffold::create(kind, &arch, name) {
        Ok(created) => {
            println!("created `{}` ({} / {}):", name, kind, arch);
            for path in &created {
                println!("  {}", path);
            }
            println!("\nnext:\n  cd {}\n  vader run cmd/main.vd", name);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Roda o linter de arquitetura se houver `architecture` no vader.toml.
/// Retorna `true` se não houver erros (avisos não bloqueiam).
fn lint_gate(file: &str, imports: &[String]) -> bool {
    let arch = match read_architecture() {
        Some(a) => a,
        None => return true, // sem arquitetura configurada => sem regras
    };
    let mut ok = true;
    for f in lint::lint(&arch, file, imports) {
        let mark = match f.severity {
            lint::Severity::Error => {
                ok = false;
                "\u{1F534} erro"
            }
            lint::Severity::Warning => "\u{1F7E1} aviso",
        };
        eprintln!("{} [{}] {}", mark, f.rule, f.message);
    }
    ok
}

/// Lê `architecture = "..."` do `vader.toml` no diretório atual.
fn read_architecture() -> Option<String> {
    let s = std::fs::read_to_string("vader.toml").ok()?;
    for line in s.lines() {
        let l = line.trim();
        if l.starts_with("architecture") {
            if let Some(rhs) = l.split('=').nth(1) {
                return Some(rhs.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// `vader lint <file.vd> [--arch <arch>]`
fn cmd_lint(args: &[String]) -> ExitCode {
    let mut file: Option<&String> = None;
    let mut arch_override: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--arch" => {
                if i + 1 < args.len() {
                    arch_override = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            other => {
                if !other.starts_with("--") {
                    file = Some(&args[i]);
                }
            }
        }
        i += 1;
    }
    let file = match file {
        Some(f) => f,
        None => {
            usage();
            return ExitCode::FAILURE;
        }
    };

    let arch = match arch_override.or_else(read_architecture) {
        Some(a) => a,
        None => {
            println!("sem regras de arquitetura (defina `architecture` no vader.toml ou use --arch)");
            return ExitCode::SUCCESS;
        }
    };

    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{}`: {}", file, e);
            return ExitCode::FAILURE;
        }
    };
    let tokens = match lexer::tokenize(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        }
    };
    let program = match parser::parse(tokens) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        }
    };

    let findings = lint::lint(&arch, file, &program.imports);
    if findings.is_empty() {
        println!("ok: `{}` respeita a arquitetura `{}`", file, arch);
        return ExitCode::SUCCESS;
    }
    let mut has_error = false;
    for f in &findings {
        let (mark, is_err) = match f.severity {
            lint::Severity::Error => ("\u{1F534} erro", true),
            lint::Severity::Warning => ("\u{1F7E1} aviso", false),
        };
        has_error |= is_err;
        println!("{} [{}] {}", mark, f.rule, f.message);
    }
    if has_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

struct TestConfig {
    gate: bool,
    min: f64,
}

/// Lê `[test]` do `vader.toml` no diretório atual (parser de linha bem simples).
fn read_test_config() -> TestConfig {
    let mut cfg = TestConfig {
        gate: false,
        min: 0.0,
    };
    if let Ok(s) = std::fs::read_to_string("vader.toml") {
        for line in s.lines() {
            let l = line.trim();
            if l.starts_with("coverage_gate") {
                if l.contains("true") {
                    cfg.gate = true;
                } else if l.contains("false") {
                    cfg.gate = false;
                }
            } else if l.starts_with("min_coverage") {
                if let Some(rhs) = l.split('=').nth(1) {
                    if let Ok(n) = rhs.trim().parse::<f64>() {
                        cfg.min = n;
                    }
                }
            }
        }
    }
    cfg
}

fn install_pre_push_hook(file: &str) -> ExitCode {
    if !Path::new(".git").is_dir() {
        eprintln!("error: not a git repository (no .git/ here)");
        return ExitCode::FAILURE;
    }
    let hooks = Path::new(".git/hooks");
    if let Err(e) = std::fs::create_dir_all(hooks) {
        eprintln!("error: {}", e);
        return ExitCode::FAILURE;
    }
    let hook = hooks.join("pre-push");
    let content = format!(
        "#!/bin/sh\n# Vader coverage gate (gerado por `vader test --install-hook`).\n\
         # Desative em vader.toml: [test] coverage_gate = false\n\
         exec vader test {}\n",
        file
    );
    if let Err(e) = std::fs::write(&hook, content) {
        eprintln!("error: {}", e);
        return ExitCode::FAILURE;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755));
    }
    println!("installed .git/hooks/pre-push -> vader test {}", file);
    ExitCode::SUCCESS
}

/// `vader test [--no-gate] [--min-coverage N] [--install-hook] <file.vd>`
fn cmd_test(args: &[String]) -> ExitCode {
    let mut file: Option<String> = None;
    let mut no_gate = false;
    let mut install_hook = false;
    let mut min_override: Option<f64> = None;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--no-gate" => no_gate = true,
            "--install-hook" => install_hook = true,
            "--min-coverage" => {
                if i + 1 < args.len() {
                    min_override = args[i + 1].parse().ok();
                    i += 1;
                }
            }
            other => file = Some(other.to_string()),
        }
        i += 1;
    }

    let file = match file {
        Some(f) => f,
        None => {
            usage();
            return ExitCode::FAILURE;
        }
    };

    if install_hook {
        return install_pre_push_hook(&file);
    }

    let mut cfg = read_test_config();
    if no_gate {
        cfg.gate = false;
    }
    if let Some(m) = min_override {
        cfg.min = m;
        cfg.gate = true;
    }

    // diretório => roda os testes do projeto inteiro (inclui `*_test.vd`)
    let program = if Path::new(&file).is_dir() {
        match module::load(&file, true) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{}", e);
                return ExitCode::FAILURE;
            }
        }
    } else {
        let source = match std::fs::read_to_string(&file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read `{}`: {}", file, e);
                return ExitCode::FAILURE;
            }
        };
        let tokens = match lexer::tokenize(&source) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{}", e);
                return ExitCode::FAILURE;
            }
        };
        match parser::parse(tokens) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{}", e);
                return ExitCode::FAILURE;
            }
        }
    };
    if let Err(errors) = check::check(&program) {
        for e in &errors {
            eprintln!("type error at {}:{}: {}", e.line, e.col, e.message);
        }
        return ExitCode::FAILURE;
    }

    let go_src = match codegen::generate_tests(&program, cfg.gate, cfg.min) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        }
    };

    let dir = std::env::temp_dir().join("vader_test");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("error: cannot create temp dir: {}", e);
        return ExitCode::FAILURE;
    }
    let go_file = dir.join("main.go");
    if let Err(e) = std::fs::write(&go_file, &go_src) {
        eprintln!("error: cannot write generated Go: {}", e);
        return ExitCode::FAILURE;
    }

    match Command::new("go").arg("run").arg(&go_file).status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: failed to invoke `go`: {} (is Go on PATH?)", e);
            ExitCode::FAILURE
        }
    }
}

/// `vader fmt [-w] <file.vd>`
fn cmd_fmt(args: &[String]) -> ExitCode {
    let mut write = false;
    let mut file: Option<&String> = None;
    for a in &args[2..] {
        if a == "-w" {
            write = true;
        } else {
            file = Some(a);
        }
    }
    let path = match file {
        Some(p) => p,
        None => {
            usage();
            return ExitCode::FAILURE;
        }
    };

    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{}`: {}", path, e);
            return ExitCode::FAILURE;
        }
    };
    let tokens = match lexer::tokenize(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        }
    };
    let program = match parser::parse(tokens) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        }
    };

    let formatted = formatter::format(&program);
    if write {
        if let Err(e) = std::fs::write(path, &formatted) {
            eprintln!("error: cannot write `{}`: {}", path, e);
            return ExitCode::FAILURE;
        }
        println!("formatted {}", path);
    } else {
        print!("{}", formatted);
    }
    ExitCode::SUCCESS
}

/// `vader gen <thing> <Name>`
fn cmd_gen(args: &[String]) -> ExitCode {
    if args.len() < 4 {
        usage();
        return ExitCode::FAILURE;
    }
    let thing = &args[2];
    let name = &args[3];
    if !matches!(thing.as_str(), "fn" | "struct" | "usecase" | "handler") {
        eprintln!("error: unknown artifact `{}` (fn|struct|usecase|handler)", thing);
        return ExitCode::FAILURE;
    }

    match gen::create(thing, name) {
        Ok(created) => {
            for path in &created {
                let mark = if path.ends_with("_test.vd") { " (teste espelho)" } else { "" };
                println!("  created {}{}", path, mark);
            }
            println!("\nTDD por padrão: o teste nasceu junto. 🟢");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}
