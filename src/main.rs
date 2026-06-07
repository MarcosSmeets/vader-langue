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

/// Concurrency runtime (C), embedded in the binary and linked by clang when there are channels.
const RUNTIME_C: &str = include_str!("../runtime/vader_rt.c");
/// SQLite (amalgamation, public domain) + wrapper, embedded and linked when using `std/db`.
const SQLITE_C: &str = include_str!("../runtime/sqlite/sqlite3.c");
const SQLITE_H: &str = include_str!("../runtime/sqlite/sqlite3.h");
const VADER_DB_C: &str = include_str!("../runtime/vader_db.c");
/// Postgres driver (pure wire protocol: TCP + SCRAM auth + simple query).
const VADER_PG_C: &str = include_str!("../runtime/vader_pg.c");
/// MySQL/MariaDB driver (native protocol + mysql_native_password).
const VADER_MYSQL_C: &str = include_str!("../runtime/vader_mysql.c");
/// stdlib: HTTP (server + client) and JSON (parse/encode), linked on demand.
const VADER_HTTP_C: &str = include_str!("../runtime/vader_http.c");
const VADER_JSON_C: &str = include_str!("../runtime/vader_json.c");
/// HTTP router (newRouter + r.get/post + serve).
const VADER_ROUTER_C: &str = include_str!("../runtime/vader_router.c");
/// MongoDB client (BSON + OP_MSG): connect/insert/find/close.
const VADER_MONGO_C: &str = include_str!("../runtime/vader_mongo.c");
/// Shared SCRAM-SHA-256 crypto (used by the Mongo driver for auth).
const VADER_SCRAM_C: &str = include_str!("../runtime/vader_scram.c");
/// General-purpose stdlib: strings, math, time, fs, fmt.
const VADER_STR_C: &str = include_str!("../runtime/vader_str.c");
const VADER_MATH_C: &str = include_str!("../runtime/vader_math.c");
const VADER_TIME_C: &str = include_str!("../runtime/vader_time.c");
const VADER_FS_C: &str = include_str!("../runtime/vader_fs.c");
const VADER_FMT_C: &str = include_str!("../runtime/vader_fmt.c");
/// Arena/region allocator (long-lived service memory): bump-alloc per scope.
const VADER_MEM_C: &str = include_str!("../runtime/vader_mem.c");
/// std/os and std/env: access to the process environment.
const VADER_OS_C: &str = include_str!("../runtime/vader_os.c");

use vader::ast::Program;
use vader::{
    check, codegen, formatter, gen, lexer, lint, llvm, migrate, module, parser, pkg, scaffold,
    templates,
};

fn usage() {
    eprintln!("vader {} — compiler (Phase 1)", env!("CARGO_PKG_VERSION"));
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

    // Directory: compile the entire project via the module system.
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

/// Post-parse: type-check, architecture lint and (build/run) generate Go and compile.
fn finish(command: &str, path: &str, program: Program, is_dir: bool) -> ExitCode {
    if let Err(errors) = check::check(&program) {
        for e in &errors {
            eprintln!("type error at {}:{}: {}", e.line, e.col, e.message);
        }
        eprintln!("{} type error(s)", errors.len());
        return ExitCode::FAILURE;
    }

    // architecture: enforced automatically (only for a standalone file; a dir has no single layer)
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
        // in a project (dir), the binary goes INSIDE the folder so it doesn't collide with it.
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

/// `vader add <git-url|path>[@version] [name]` — adds a dependency (git/URL).
fn cmd_add(args: &[String]) -> ExitCode {
    let src = match args.get(2) {
        Some(s) => s,
        None => {
            eprintln!("usage: vader add <git-url|path>[@version] [name]");
            return ExitCode::FAILURE;
        }
    };
    // `add <name>` (no slash/scheme) resolves via the registry; otherwise it's a URL/path.
    let dep = if is_bare_name(src) {
        let reg = match resolve_registry(args) {
            Some(r) => r,
            None => {
                eprintln!(
                    "`{}` looks like a package name — pass --registry <dir|git-url>, set VADER_REGISTRY, or use the full URL/path",
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

    println!("fetching `{}` from {}...", name, url);
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

/// `vader remove <name>` — removes a dependency from `vader.toml`.
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
        eprintln!("`{}` is not in the dependencies", name);
        return ExitCode::FAILURE;
    }
    let _ = std::fs::write("vader.toml", pkg::write_deps(&toml, &deps));
    write_lock(&deps);
    println!("removed `{}`", name);
    ExitCode::SUCCESS
}

/// `vader publish [--registry <dir|git-url>]` — registers the current package in the index.
fn cmd_publish(args: &[String]) -> ExitCode {
    let registry = match resolve_registry(args) {
        Some(r) => r,
        None => {
            eprintln!("provide the registry: --registry <dir|git-url> or the VADER_REGISTRY env var");
            return ExitCode::FAILURE;
        }
    };
    let toml = std::fs::read_to_string("vader.toml").unwrap_or_default();
    let name = match toml_top_name(&toml) {
        Some(n) => n,
        None => {
            eprintln!("vader.toml has no `name = \"...\"` — run this at the project root");
            return ExitCode::FAILURE;
        }
    };
    let url = git_output(&["remote", "get-url", "origin"]).unwrap_or_default();
    if url.is_empty() {
        eprintln!("no `git remote origin` — the package needs a git repository");
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
                "(no tag)".to_string()
            } else {
                version
            };
            println!("published `{}` -> {} @ {} in registry {}", name, url, v, registry);
            println!("(if the registry is a git repo, commit+push index.json)");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Target registry: flag `--registry <reg>` or env `VADER_REGISTRY`.
fn resolve_registry(args: &[String]) -> Option<String> {
    if let Some(i) = args.iter().position(|a| a == "--registry") {
        return args.get(i + 1).cloned();
    }
    std::env::var("VADER_REGISTRY").ok()
}

/// Heuristic: a package name has no slash, scheme, or dot (≠ URL/path).
fn is_bare_name(s: &str) -> bool {
    !s.is_empty()
        && !s.contains('/')
        && !s.contains('\\')
        && !s.contains(':')
        && !s.contains('.')
}

/// Reads `name = "..."` from the top of `vader.toml` (before any section).
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

/// Runs `git <args>` and returns the stdout (trimmed), or None on failure/empty.
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

/// Regenerates `vader.lock` with the resolved commits of each dependency.
fn write_lock(deps: &[pkg::Dep]) {
    let mut out = String::from("# vader.lock — automatically generated; resolved commits\n");
    for d in deps {
        if let Ok((_p, commit)) = pkg::fetch(d) {
            out.push_str(&format!("{} = \"{}@{}\"\n", d.name, d.url, commit));
        }
    }
    let _ = std::fs::write("vader.lock", out);
}

/// `vader llvm <file.vd>` — Vader -> LLVM IR (text) -> clang -> native binary -> run.
fn cmd_llvm(args: &[String]) -> ExitCode {
    // The native backend links a POSIX C runtime (sockets/pthread); Windows isn't
    // supported natively yet. Be explicit instead of failing with a cryptic clang error.
    if cfg!(target_os = "windows") {
        eprintln!(
            "error: `vader llvm` (native backend) needs a POSIX toolchain (clang + the C runtime),\n\
             which isn't supported on native Windows yet. Options:\n\
             - run it inside WSL (recommended on Windows), or\n\
             - use the Go backend: `vader build` / `vader run` (works natively on Windows)."
        );
        return ExitCode::FAILURE;
    }
    // parse: first non-flag arg is the file/dir; `--out <path>` builds without running.
    let mut path: Option<&String> = None;
    let mut out: Option<&str> = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                out = args.get(i + 1).map(|s| s.as_str());
                i += 2;
            }
            a if a.starts_with("--") => i += 1,
            _ => {
                if path.is_none() {
                    path = Some(&args[i]);
                }
                i += 1;
            }
        }
    }
    let path = match path {
        Some(p) => p,
        None => {
            usage();
            return ExitCode::FAILURE;
        }
    };
    let tls = args.iter().any(|a| a == "--tls");
    // A directory builds the whole project natively (flattened by the module system).
    let result = if Path::new(path).is_dir() {
        match module::load(path, false) {
            Ok(program) => build_run_program(&program, false, tls, out),
            Err(e) => Err(e),
        }
    } else {
        match std::fs::read_to_string(path) {
            Ok(source) => build_run_source(&source, false, tls, out),
            Err(e) => Err(format!("cannot read `{}`: {}", path, e)),
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::FAILURE
        }
    }
}

/// Compiles a Vader source via LLVM + clang and runs the binary (Result, for reuse).
/// `quiet` suppresses the progress logs (used by `vader migrate`).
fn build_run_source(source: &str, quiet: bool, tls: bool, out: Option<&str>) -> Result<(), String> {
    let tokens = lexer::tokenize(source).map_err(|e| e.to_string())?;
    let mut program = parser::parse(tokens).map_err(|e| e.to_string())?;
    if !program.imports.is_empty() {
        let packages: std::collections::HashSet<String> = program
            .imports
            .iter()
            .filter_map(|i| i.rsplit('/').next().map(|s| s.to_string()))
            .collect();
        module::normalize(&mut program, &module::Ns::folders(packages));
    }
    build_run_program(&program, quiet, tls, out)
}

/// Compiles a runtime C source to a `.o`, cached by content hash (+ compile flags),
/// so DB/HTTP builds don't recompile the runtime every time. Returns the object path.
fn cached_obj(
    dir: &std::path::Path,
    name: &str,
    source: &str,
    extra: &[&str],
    quiet: bool,
) -> Result<std::path::PathBuf, String> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut h);
    extra.hash(&mut h);
    let obj = dir.join(format!("{}-{:016x}.o", name, h.finish()));
    if obj.exists() {
        return Ok(obj);
    }
    let src = dir.join(format!("{}.c", name));
    std::fs::write(&src, source).map_err(|e| format!("write {}: {}", name, e))?;
    if !quiet {
        println!("(compiling {} — cached after the first build)", name);
    }
    let mut c = Command::new("clang");
    c.arg("-c").arg("-O2");
    for a in extra {
        c.arg(a);
    }
    c.arg(&src).arg("-o").arg(&obj).current_dir(dir);
    match c.status() {
        Ok(s) if s.success() => Ok(obj),
        _ => Err(format!("clang failed to compile {}", name)),
    }
}

/// Type-checks, generates LLVM IR, compiles with clang and runs. Used by
/// `vader llvm <file|dir>` and `vader migrate`.
fn build_run_program(
    program: &Program,
    quiet: bool,
    tls: bool,
    out: Option<&str>,
) -> Result<(), String> {
    if let Err(errors) = check::check(program) {
        let msg = errors
            .iter()
            .map(|e| format!("{}:{}: {}", e.line, e.col, e.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("type error: {}", msg));
    }
    let ir = llvm::generate(program)?;

    let dir = std::env::temp_dir().join("vader_llvm");
    std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {}", e))?;
    let ll = dir.join("out.ll");
    let bin = match out {
        Some(p) => {
            let pb = std::path::PathBuf::from(p);
            if let Some(parent) = pb.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| format!("out dir: {}", e))?;
                }
            }
            pb
        }
        None => dir.join("out"),
    };
    std::fs::write(&ll, &ir).map_err(|e| format!("write IR: {}", e))?;
    if !quiet {
        println!("emitted LLVM IR: {}", ll.display());
    }

    let mut cmd = Command::new("clang");
    // -O2: the IR we emit is naive; let LLVM optimize it (competitive native code).
    cmd.arg("-O2").arg("-Wno-override-module").arg(&ll);
    if ir.contains("@vader_") {
        cmd.arg(cached_obj(&dir, "vader_rt", RUNTIME_C, &[], quiet)?);
        cmd.arg(cached_obj(&dir, "vader_mem", VADER_MEM_C, &[], quiet)?);
        cmd.arg(cached_obj(&dir, "vader_os", VADER_OS_C, &[], quiet)?);
        cmd.arg("-lpthread");
    }
    if ir.contains("@vader_db_") {
        let hdr = dir.join("sqlite3.h");
        let obj = dir.join("sqlite3.o");
        std::fs::write(&hdr, SQLITE_H).map_err(|e| format!("write sqlite3.h: {}", e))?;
        if !obj.exists() {
            let src = dir.join("sqlite3.c");
            std::fs::write(&src, SQLITE_C).map_err(|e| format!("write sqlite3.c: {}", e))?;
            if !quiet {
                println!("(compiling embedded SQLite — only the first time)");
            }
            let st = Command::new("clang")
                .arg("-c")
                .arg("-O1") // SQLite is a 9.5MB amalgamation; -O1 keeps the first build fast
                .arg(&src)
                .arg("-o")
                .arg(&obj)
                .current_dir(&dir)
                .status();
            match st {
                Ok(s) if s.success() => {}
                _ => return Err("clang failed to compile SQLite".into()),
            }
        }
        cmd.arg(&obj); // cached sqlite3.o
        cmd.arg(cached_obj(&dir, "vader_db", VADER_DB_C, &[], quiet)?);
        let tls_args: &[&str] = if tls { &["-DVADER_TLS"] } else { &[] };
        cmd.arg(cached_obj(&dir, "vader_pg", VADER_PG_C, tls_args, quiet)?);
        cmd.arg(cached_obj(&dir, "vader_mysql", VADER_MYSQL_C, tls_args, quiet)?);
        cmd.arg(cached_obj(&dir, "vader_scram", VADER_SCRAM_C, &[], quiet)?); // SHA-256 for caching_sha2
        if tls {
            cmd.arg("-lssl").arg("-lcrypto"); // TLS (Postgres) + RSA (MySQL caching_sha2 full auth)
        }
        cmd.arg("-lpthread").arg("-ldl").arg("-lm");
    }
    if ir.contains("@vader_http_") || ir.contains("@vader_router_") {
        cmd.arg(cached_obj(&dir, "vader_http", VADER_HTTP_C, &[], quiet)?);
        if ir.contains("@vader_router_") {
            cmd.arg(cached_obj(&dir, "vader_router", VADER_ROUTER_C, &[], quiet)?);
        }
    }
    if ir.contains("@vader_json_") || ir.contains("@vader_mongo_") {
        cmd.arg(cached_obj(&dir, "vader_json", VADER_JSON_C, &[], quiet)?);
    }
    if ir.contains("@vader_mongo_") {
        cmd.arg(cached_obj(&dir, "vader_mongo", VADER_MONGO_C, &[], quiet)?);
        cmd.arg(cached_obj(&dir, "vader_scram", VADER_SCRAM_C, &[], quiet)?);
    }
    if ir.contains("@vader_str_") {
        cmd.arg(cached_obj(&dir, "vader_str", VADER_STR_C, &[], quiet)?);
    }
    if ir.contains("@vader_math_") {
        cmd.arg(cached_obj(&dir, "vader_math", VADER_MATH_C, &[], quiet)?);
        cmd.arg("-lm");
    }
    if ir.contains("@vader_time_") {
        cmd.arg(cached_obj(&dir, "vader_time", VADER_TIME_C, &[], quiet)?);
    }
    if ir.contains("@vader_fs_") {
        cmd.arg(cached_obj(&dir, "vader_fs", VADER_FS_C, &[], quiet)?);
    }
    if ir.contains("@vader_fmt_") {
        cmd.arg(cached_obj(&dir, "vader_fmt", VADER_FMT_C, &[], quiet)?);
    }
    cmd.arg("-o").arg(&bin);
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(_) => return Err("clang failed to compile the IR".into()),
        Err(e) => return Err(format!("failed to invoke `clang`: {} (on PATH?)", e)),
    }
    // `--out`: build only, don't run (for Docker / producing a deployable binary).
    if out.is_some() {
        if !quiet {
            println!("built -> {}", bin.display());
        }
        return Ok(());
    }
    if !quiet {
        println!("compiled with clang -> {}\n--- running ---", bin.display());
    }
    match Command::new(&bin).status() {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("the program exited with code {}", s.code().unwrap_or(-1))),
        Err(e) => Err(format!("failed to run the binary: {}", e)),
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

/// Actually runs migrations: generates a small Vader program that opens the database and runs the
/// SQL via `db.must` (aborts if the SQL fails), compiles and runs it. Only marks as applied
/// if the process exits successfully.
fn migrate_run(args: &[String], up: bool) -> Result<(), String> {
    let dsn = resolve_migrate_dsn(args).ok_or(
        "provide the database: `vader migrate up --db <dsn>` or set [database] url in vader.toml",
    )?;
    let tls = args.iter().any(|a| a == "--tls");
    if up {
        let pend = migrate::pending();
        if pend.is_empty() {
            println!("nothing pending — all applied.");
            return Ok(());
        }
        for name in pend {
            println!("\u{25B6} applying {} ...", name);
            build_run_source(&migration_program(&dsn, &migrate::up_sql(&name)), true, tls, None)
                .map_err(|e| format!("failed at {}: {}", name, e))?;
            migrate::mark_applied(&name)?;
        }
        println!("ok — migrations applied to {}", dsn);
    } else {
        match migrate::last_applied() {
            None => println!("no migration applied."),
            Some(name) => {
                println!("\u{25C0} reverting {} ...", name);
                build_run_source(&migration_program(&dsn, &migrate::down_sql(&name)), true, tls, None)
                    .map_err(|e| format!("failed to revert {}: {}", name, e))?;
                migrate::unmark(&name)?;
                println!("ok — reverted {}", name);
            }
        }
    }
    Ok(())
}

/// Resolves the database DSN: flag `--db <dsn>` or `[database] url` in `vader.toml`.
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

/// Escapes a string into a Vader literal.
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

/// Generates the Vader program that applies a block of SQL to a database (via `db.must`).
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
                println!("(no templates — create one with `vader template save <name> <folder>`)");
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

/// `vader new <kind> <name> [--arch <arch>]` or `vader new --template <tmpl> <name>`
fn cmd_new(args: &[String]) -> ExitCode {
    // custom template mode
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
    let mut db_flag: Option<String> = None;
    let mut i = 4;
    while i < args.len() {
        if args[i] == "--arch" && i + 1 < args.len() {
            arch = args[i + 1].clone();
            i += 2;
        } else if args[i] == "--db" && i + 1 < args.len() {
            db_flag = Some(args[i + 1].clone());
            i += 2;
        } else {
            eprintln!("error: unexpected argument `{}`", args[i]);
            return ExitCode::FAILURE;
        }
    }

    // `tdd` is the turnkey API architecture: native HTTP router + DB-from-env + health-check.
    if arch == "tdd" {
        let db = match db_flag.or_else(prompt_database) {
            Some(d) => d,
            None => return ExitCode::FAILURE,
        };
        if !matches!(db.as_str(), "sqlite" | "postgres" | "mysql" | "mongo") {
            eprintln!("error: unknown database `{}` (sqlite|postgres|mysql|mongo)", db);
            return ExitCode::FAILURE;
        }
        return match scaffold::create_api_tdd(name, &db) {
            Ok(created) => {
                println!("created `{}` (api / tdd, {}):", name, db);
                for path in &created {
                    println!("  {}", path);
                }
                println!(
                    "\nnext:\n  cd {name}\n  cp .env.example .env    # set DATABASE_URL\n  vader llvm .            # build + run natively\n\nroutes: GET /health, GET /users, POST /users"
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {}", e);
                ExitCode::FAILURE
            }
        };
    }

    if !matches!(arch.as_str(), "clean" | "hexagonal" | "mvc" | "minimal") {
        eprintln!("error: unknown arch `{}` (clean|hexagonal|mvc|minimal|tdd)", arch);
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

/// Interactive prompt to choose the database for a turnkey API project.
fn prompt_database() -> Option<String> {
    use std::io::{IsTerminal, Write};
    // non-interactive (scripts/CI): default to sqlite instead of blocking on stdin.
    if !std::io::stdin().is_terminal() {
        return Some("sqlite".to_string());
    }
    println!("Choose a database for the API:");
    println!("  1) sqlite    (zero setup, embedded — recommended to start)");
    println!("  2) postgres");
    println!("  3) mysql");
    println!("  4) mongo     (document store)");
    print!("> ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return None;
    }
    match line.trim() {
        "1" | "sqlite" | "" => Some("sqlite".to_string()),
        "2" | "postgres" => Some("postgres".to_string()),
        "3" | "mysql" => Some("mysql".to_string()),
        "4" | "mongo" => Some("mongo".to_string()),
        other => {
            eprintln!("unknown choice `{}` (1/2/3/4)", other);
            None
        }
    }
}

/// Runs the architecture linter if there is `architecture` in vader.toml.
/// Returns `true` if there are no errors (warnings do not block).
fn lint_gate(file: &str, imports: &[String]) -> bool {
    let arch = match read_architecture() {
        Some(a) => a,
        None => return true, // no architecture configured => no rules
    };
    let mut ok = true;
    for f in lint::lint(&arch, file, imports) {
        let mark = match f.severity {
            lint::Severity::Error => {
                ok = false;
                "\u{1F534} error"
            }
            lint::Severity::Warning => "\u{1F7E1} warning",
        };
        eprintln!("{} [{}] {}", mark, f.rule, f.message);
    }
    ok
}

/// Reads `architecture = "..."` from `vader.toml` in the current directory.
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
            println!("no architecture rules (set `architecture` in vader.toml or use --arch)");
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
        println!("ok: `{}` respects the architecture `{}`", file, arch);
        return ExitCode::SUCCESS;
    }
    let mut has_error = false;
    for f in &findings {
        let (mark, is_err) = match f.severity {
            lint::Severity::Error => ("\u{1F534} error", true),
            lint::Severity::Warning => ("\u{1F7E1} warning", false),
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

/// Reads `[test]` from `vader.toml` in the current directory (very simple line parser).
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
        "#!/bin/sh\n# Vader coverage gate (generated by `vader test --install-hook`).\n\
         # Disable it in vader.toml: [test] coverage_gate = false\n\
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

    // directory => runs the tests of the entire project (includes `*_test.vd`)
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
                let mark = if path.ends_with("_test.vd") { " (mirror test)" } else { "" };
                println!("  created {}{}", path, mark);
            }
            println!("\nTDD by default: the test was born alongside it. 🟢");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}
