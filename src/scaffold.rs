//! Scaffolder for `vader new`: generates a project tree in one of the opinionated
//! architectures (clean/hexagonal/mvc/minimal), already in TDD format.
//!
//! Each architecture function returns a list of `(relative path, content)`.
//! `create` writes the files to disk under `<name>/`.

/// Default architecture for each project kind.
pub fn default_arch(kind: &str) -> &'static str {
    match kind {
        "api" | "worker" => "clean",
        _ => "minimal",
    }
}

/// Builds the list of files (path relative to the project root, content).
pub fn files_for(kind: &str, arch: &str, name: &str) -> Result<Vec<(String, String)>, String> {
    let mut files = vec![(
        "vader.toml".to_string(),
        format!(
            "[project]\nname         = \"{name}\"\nversion      = \"0.1.0\"\nkind         = \"{kind}\"\narchitecture = \"{arch}\"\n\n\
             [test]\n# blocks `git push` if coverage falls below the minimum\ncoverage_gate = true\nmin_coverage  = 80\n",
        ),
    )];
    let body = match arch {
        "clean" => clean(name),
        "hexagonal" => hexagonal(name),
        "mvc" => mvc(name),
        "minimal" => minimal(name),
        other => return Err(format!("unknown architecture `{}`", other)),
    };
    files.extend(body);

    // Executable projects are born Docker-ready (static binary -> minimal image).
    if kind != "lib" {
        files.push(("Dockerfile".to_string(), dockerfile(name)));
        files.push((".dockerignore".to_string(), ".git\n/target\n".to_string()));
    }
    Ok(files)
}

fn dockerfile(name: &str) -> String {
    format!(
        "# syntax=docker/dockerfile:1\n\n\
         # --- build (requires the Vader toolchain; swap for the official image once it exists) ---\n\
         FROM vader/toolchain:latest AS build\n\
         WORKDIR /src\n\
         COPY . .\n\
         RUN vader build .\n\n\
         # --- runtime: minimal image, the Vader binary is static ---\n\
         FROM scratch\n\
         COPY --from=build /src/{name}/{name} /app\n\
         ENTRYPOINT [\"/app\"]\n",
    )
}

/// Creates the project on disk under `<name>/`. Fails if the directory already exists.
pub fn create(kind: &str, arch: &str, name: &str) -> Result<Vec<String>, String> {
    write_project(name, files_for(kind, arch, name)?)
}

/// Writes a list of (relative path, content) under `<name>/`.
fn write_project(name: &str, files: Vec<(String, String)>) -> Result<Vec<String>, String> {
    let root = std::path::Path::new(name);
    if root.exists() {
        return Err(format!("directory `{}` already exists", name));
    }
    let mut created = Vec::new();
    for (rel, content) in files {
        let full = root.join(&rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&full, content).map_err(|e| e.to_string())?;
        created.push(full.to_string_lossy().replace('\\', "/"));
    }
    Ok(created)
}

/// Turnkey API project (arch `tdd`): native HTTP router with a default health-check
/// route, a CRUD example, and a DB connection whose DSN comes from the environment.
/// `db` is one of sqlite/postgres/mysql. Builds with `vader llvm .`.
pub fn create_api_tdd(name: &str, db: &str) -> Result<Vec<String>, String> {
    write_project(name, api_tdd_files(name, db))
}

/// CREATE TABLE statement for the chosen database engine.
fn db_create_sql(db: &str) -> &'static str {
    match db {
        "postgres" => "CREATE TABLE IF NOT EXISTS users (id SERIAL PRIMARY KEY, name TEXT)",
        "mysql" => {
            "CREATE TABLE IF NOT EXISTS users (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255))"
        }
        _ => "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT)",
    }
}

/// Example DSN for `.env`, per engine.
fn db_dsn_example(db: &str, name: &str) -> String {
    match db {
        "postgres" => format!("postgres://postgres:password@localhost:5432/{name}"),
        "mysql" => format!("mysql://root:password@localhost:3306/{name}"),
        _ => format!("./{name}.db"),
    }
}

fn api_tdd_files(name: &str, db: &str) -> Vec<(String, String)> {
    let create_sql = db_create_sql(db);
    let dsn = db_dsn_example(db, name);
    vec![
        f(
            "vader.toml",
            format!(
                "[project]\nname         = \"{name}\"\nversion      = \"0.1.0\"\nkind         = \"api\"\narchitecture = \"tdd\"\n\n\
                 [database]\nengine = \"{db}\"\n# the DSN is read from the DATABASE_URL environment variable\n\n\
                 [test]\ncoverage_gate = true\nmin_coverage  = 80\n",
            ),
        ),
        f(
            ".env.example",
            format!("# copy to .env and adjust; the app reads DATABASE_URL at startup\nDATABASE_URL={dsn}\n"),
        ),
        f(
            "cmd/main.vd",
            format!(
                "import \"std/http\"\nimport \"std/db\"\nimport \"std/env\"\n\n\
                 // Entry point: wire the routes and start the server.\n\
                 public fn main() {{\n    \
                     // The DB connection string comes from the environment — just set DATABASE_URL.\n    \
                     DB conn = db.open(env.read(\"DATABASE_URL\"))\n    \
                     db.must(conn, \"{create_sql}\")\n    \
                     db.close(conn)\n\n    \
                     Router r = newRouter()\n    \
                     r.get(\"/health\", health)      // default health-check route\n    \
                     r.get(\"/users\", listUsers)\n    \
                     r.post(\"/users\", createUser)\n\n    \
                     print(\"listening on http://localhost:8080\")\n    \
                     serve(8080, r)\n\
                 }}\n",
            ),
        ),
        f(
            "handlers/health.vd",
            "import \"std/http\"\n\n\
             // Default health-check route — handy for load balancers and uptime checks.\n\
             public fn health(s Server) {\n    \
                 http.respond(s, 200, \"application/json\", \"{\\\"status\\\":\\\"ok\\\"}\")\n\
             }\n"
                .to_string(),
        ),
        f(
            "handlers/users.vd",
            "import \"std/http\"\nimport \"std/db\"\nimport \"std/env\"\nimport \"std/json\"\n\n\
             public fn listUsers(s Server) {\n    \
                 DB conn = db.open(env.read(\"DATABASE_URL\"))\n    \
                 Rows rows = db.query(conn, \"SELECT id, name FROM users ORDER BY id\")\n    \
                 Json arr = json.array()\n    \
                 for db.next(rows) {\n        \
                     Json u = json.object()\n        \
                     json.set_int(u, \"id\", db.col_int(rows, 0))\n        \
                     json.set_str(u, \"name\", db.col_text(rows, 1))\n        \
                     json.add(arr, u)\n    \
                 }\n    \
                 http.respond(s, 200, \"application/json\", json.encode(arr))\n    \
                 db.close(conn)\n\
             }\n\n\
             public fn createUser(s Server) {\n    \
                 DB conn = db.open(env.read(\"DATABASE_URL\"))\n    \
                 Json body = json.parse(http.body(s))\n    \
                 string name = json.as_str(json.field(body, \"name\"))\n    \
                 // TODO: use parameterized queries in production (this concat is unsafe).\n    \
                 db.must(conn, \"INSERT INTO users (name) VALUES ('\" + name + \"')\")\n    \
                 http.respond(s, 201, \"application/json\", \"{\\\"status\\\":\\\"created\\\"}\")\n    \
                 db.close(conn)\n\
             }\n"
                .to_string(),
        ),
        f(
            "domain/user.vd",
            "// User entity.\npublic struct User {\n    id   int\n    name string\n}\n".to_string(),
        ),
        f(
            "domain/user_test.vd",
            "// auto-generated mirror test for the entity.\n\
             test \"user holds its fields\" {\n    \
                 User u = User{ id: 1, name: \"Ada\" }\n    \
                 assert u.id == 1\n    \
                 assert u.name == \"Ada\"\n\
             }\n"
                .to_string(),
        ),
        f("Dockerfile", dockerfile_native()),
        f(".dockerignore", ".git\n/target\n.env\n".to_string()),
    ]
}

/// Dockerfile for a natively-built API: `vader llvm --out` then a slim libc base
/// (the native binary embeds SQLite but links libc dynamically, so not `scratch`).
fn dockerfile_native() -> String {
    "# syntax=docker/dockerfile:1\n\n\
     # --- build: compile to a native binary (no run) ---\n\
     FROM vader/toolchain:latest AS build\n\
     WORKDIR /src\n\
     COPY . .\n\
     RUN vader llvm --out /out/server .\n\n\
     # --- runtime: slim image with libc ---\n\
     FROM debian:bookworm-slim\n\
     COPY --from=build /out/server /app/server\n\
     ENV DATABASE_URL=\"\"\n\
     EXPOSE 8080\n\
     ENTRYPOINT [\"/app/server\"]\n"
        .to_string()
}

fn f(path: &str, content: String) -> (String, String) {
    (path.to_string(), content)
}

fn clean(name: &str) -> Vec<(String, String)> {
    vec![
        f(
            "cmd/main.vd",
            format!(
                "import \"{name}/domain\"\nimport \"{name}/usecase\"\nimport \"{name}/infra/db\"\n\n\
                 // composition root: wires the concrete implementations to the ports.\n\
                 public fn main() {{\n    \
                     db.UserRepositoryPg repo = db.UserRepositoryPg{{}}\n    \
                     usecase.CreateUser createUser = usecase.CreateUser{{ repo: repo }}\n    \
                     domain.User user, error err = createUser.execute(\"Ada\")\n    \
                     if err != nil {{\n        print(\"error:\", err)\n        return\n    }}\n    \
                     print(\"created:\", user.name)\n\
                 }}\n",
            ),
        ),
        f(
            "domain/user.vd",
            "// domain/user.vd — pure entity (no infra dependency).\n\n\
             public struct User {\n    id   int\n    name string\n}\n"
                .to_string(),
        ),
        f(
            "domain/user_test.vd",
            "// auto-generated: mirror test for the entity.\n\n\
             test \"user holds its fields\" {\n    \
                 User u = User{ id: 1, name: \"Ada\" }\n    \
                 assert u.id == 1\n    \
                 assert u.name == \"Ada\"\n\
             }\n"
                .to_string(),
        ),
        f(
            "domain/user_repository.vd",
            "// persistence port — the domain only knows the abstraction.\n\n\
             public interface UserRepository {\n    \
                 fn save(user User): (User, error)\n    \
                 fn findById(id int): (User, error)\n\
             }\n"
                .to_string(),
        ),
        f(
            "usecase/create_user.vd",
            format!(
                "import \"{name}/domain\"\n\n\
                 public struct CreateUser {{\n    repo domain.UserRepository\n}}\n\n\
                 public fn (uc CreateUser) execute(name string): (domain.User, error) {{\n    \
                     if name == \"\" {{\n        return domain.User{{}}, error(\"name is required\")\n    }}\n    \
                     domain.User user = domain.User{{ name: name }}\n    \
                     return uc.repo.save(user)\n\
                 }}\n",
            ),
        ),
        f(
            "usecase/create_user_test.vd",
            "// auto-generated: use case test.\n\n\
             test \"execute rejects an empty name\" {\n    \
                 CreateUser uc = CreateUser{ repo: nil }\n    \
                 _, error err = uc.execute(\"\")\n    \
                 assert err != nil\n\
             }\n"
                .to_string(),
        ),
        f(
            "adapter/http/user_handler.vd",
            format!(
                "import \"{name}/domain\"\nimport \"{name}/usecase\"\n\n\
                 public struct UserHandler {{\n    createUser usecase.CreateUser\n}}\n\n\
                 public fn (h UserHandler) handleCreate(name string): (int, error) {{\n    \
                     domain.User user, error err = h.createUser.execute(name)\n    \
                     if err != nil {{\n        return 400, err\n    }}\n    \
                     print(\"created\", user.name)\n    \
                     return 201, nil\n\
                 }}\n",
            ),
        ),
        f(
            "infra/db/user_repository_pg.vd",
            format!(
                "import \"std/db\"\nimport \"{name}/domain\"\n\n\
                 // concrete implementation of the domain.UserRepository port (Postgres).\n\
                 public struct UserRepositoryPg {{\n    conn db.Conn\n}}\n\n\
                 public fn (r UserRepositoryPg) save(user domain.User): (domain.User, error) {{\n    \
                     // INSERT ... ; the row->entity mapping lives here, at the boundary.\n    \
                     return user, nil\n\
                 }}\n\n\
                 public fn (r UserRepositoryPg) findById(id int): (domain.User, error) {{\n    \
                     return domain.User{{}}, nil\n\
                 }}\n",
            ),
        ),
    ]
}

fn hexagonal(name: &str) -> Vec<(String, String)> {
    vec![
        f(
            "cmd/main.vd",
            format!(
                "import \"{name}/core/service\"\nimport \"{name}/adapter/outbound/db\"\n\n\
                 public fn main() {{\n    \
                     UserRepositoryPg repo = UserRepositoryPg{{}}\n    \
                     service.RegisterUser svc = service.RegisterUser{{ repo: repo }}\n    \
                     user, error err = svc.execute(\"Ada\")\n    \
                     if err != nil {{ print(\"error:\", err)  return }}\n    \
                     print(\"registered:\", user.name)\n\
                 }}\n",
            ),
        ),
        f(
            "core/domain/user.vd",
            "public struct User {\n    id   int\n    name string\n}\n".to_string(),
        ),
        f(
            "core/domain/user_test.vd",
            "test \"user holds its fields\" {\n    \
                 User u = User{ id: 1, name: \"Ada\" }\n    \
                 assert u.name == \"Ada\"\n\
             }\n"
                .to_string(),
        ),
        f(
            "core/port/inbound/register_user.vd",
            "// inbound port (driving) — what the world can ask of the core.\n\n\
             public interface RegisterUserPort {\n    fn execute(name string): (User, error)\n}\n"
                .to_string(),
        ),
        f(
            "core/port/outbound/user_repository.vd",
            "// outbound port (driven) — what the core needs from the world.\n\n\
             public interface UserRepository {\n    fn save(user User): (User, error)\n}\n"
                .to_string(),
        ),
        f(
            "core/service/register_user_service.vd",
            "// implements the inbound port, using only the outbound port.\n\n\
             public struct RegisterUser {\n    repo UserRepository\n}\n\n\
             public fn (s RegisterUser) execute(name string): (User, error) {\n    \
                 if name == \"\" { return User{}, error(\"name is required\") }\n    \
                 return s.repo.save(User{ name: name })\n\
             }\n"
                .to_string(),
        ),
        f(
            "adapter/inbound/http/user_handler.vd",
            "// driving adapter: translates HTTP into the inbound port.\n\n\
             public struct UserHandler {\n    register RegisterUserPort\n}\n"
                .to_string(),
        ),
        f(
            "adapter/outbound/db/user_repository_pg.vd",
            "// driven adapter: implements the outbound port.\n\n\
             public struct UserRepositoryPg {\n    // conn ...\n}\n\n\
             public fn (r UserRepositoryPg) save(user User): (User, error) {\n    return user, nil\n}\n"
                .to_string(),
        ),
    ]
}

fn mvc(name: &str) -> Vec<(String, String)> {
    let _ = name;
    vec![
        f(
            "cmd/main.vd",
            "public fn main() {\n    \
                 UserController c = UserController{}\n    \
                 c.create(\"Ada\")\n\
             }\n"
                .to_string(),
        ),
        f(
            "model/user.vd",
            "// model: data + business rule.\n\n\
             public struct User {\n    id   int\n    name string\n}\n\n\
             public fn (u User) isValid(): bool {\n    return u.name != \"\"\n}\n"
                .to_string(),
        ),
        f(
            "model/user_test.vd",
            "test \"empty user is invalid\" {\n    \
                 User u = User{ id: 1, name: \"\" }\n    \
                 assert u.isValid() == false\n\
             }\n"
                .to_string(),
        ),
        f(
            "view/user_view.vd",
            "// view: presentation / output serialization.\n\n\
             public fn renderUser(u User): string {\n    return \"User: \" + u.name\n}\n"
                .to_string(),
        ),
        f(
            "controller/user_controller.vd",
            "// controller: orchestrates request -> model -> view.\n\n\
             public struct UserController {}\n\n\
             public fn (c UserController) create(name string): string {\n    \
                 User u = User{ name: name }\n    \
                 if u.isValid() == false { return \"invalid\" }\n    \
                 return renderUser(u)\n\
             }\n"
                .to_string(),
        ),
        f(
            "controller/user_controller_test.vd",
            "test \"create renders a valid user\" {\n    \
                 UserController c = UserController{}\n    \
                 assert c.create(\"Ada\") == \"User: Ada\"\n\
             }\n"
                .to_string(),
        ),
    ]
}

fn minimal(name: &str) -> Vec<(String, String)> {
    vec![
        f(
            "cmd/main.vd",
            format!(
                "import \"{name}/src\"\n\n\
                 public fn main() {{\n    print(src.greet(\"World\"))\n}}\n",
            ),
        ),
        f(
            "src/greet.vd",
            "public fn greet(name string): string {\n    return \"Hello, \" + name\n}\n".to_string(),
        ),
        f(
            "src/greet_test.vd",
            "// auto-generated: mirror test for the function.\n\n\
             test \"greet builds a greeting\" {\n    assert greet(\"World\") == \"Hello, World\"\n}\n"
                .to_string(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(kind: &str, arch: &str, name: &str) -> Vec<String> {
        files_for(kind, arch, name)
            .unwrap()
            .into_iter()
            .map(|(p, _)| p)
            .collect()
    }

    #[test]
    fn clean_has_all_layers() {
        let p = paths("api", "clean", "demo");
        assert!(p.iter().any(|x| x.starts_with("domain/")));
        assert!(p.iter().any(|x| x.starts_with("usecase/")));
        assert!(p.iter().any(|x| x.starts_with("adapter/")));
        assert!(p.iter().any(|x| x.starts_with("infra/")));
    }

    #[test]
    fn every_arch_ships_a_test_file_and_toml() {
        for arch in ["clean", "hexagonal", "mvc", "minimal"] {
            let p = paths("api", arch, "demo");
            assert!(p.iter().any(|x| x == "vader.toml"), "{arch} missing toml");
            assert!(
                p.iter().any(|x| x.ends_with("_test.vd")),
                "{arch} missing a test (TDD by default)"
            );
            assert!(p.iter().any(|x| x.ends_with("main.vd")), "{arch} missing main");
        }
    }

    #[test]
    fn executable_kinds_get_a_dockerfile() {
        assert!(paths("api", "clean", "demo").iter().any(|p| p == "Dockerfile"));
        assert!(paths("cli", "minimal", "demo").iter().any(|p| p == "Dockerfile"));
        // lib is not executable -> no Dockerfile
        assert!(!paths("lib", "minimal", "demo").iter().any(|p| p == "Dockerfile"));
    }

    #[test]
    fn minimal_is_flat() {
        let p = paths("cli", "minimal", "demo");
        assert!(!p.iter().any(|x| x.starts_with("domain/")));
        assert!(p.iter().any(|x| x.starts_with("src/")));
    }

    #[test]
    fn toml_records_name_and_arch() {
        let files = files_for("api", "clean", "myapp").unwrap();
        let toml = &files.iter().find(|(p, _)| p == "vader.toml").unwrap().1;
        assert!(toml.contains("name         = \"myapp\""));
        assert!(toml.contains("architecture = \"clean\""));
    }

    #[test]
    fn unknown_arch_is_an_error() {
        assert!(files_for("api", "bogus", "x").is_err());
    }

    #[test]
    fn default_arch_per_kind() {
        assert_eq!(default_arch("api"), "clean");
        assert_eq!(default_arch("cli"), "minimal");
        assert_eq!(default_arch("lib"), "minimal");
    }
}
