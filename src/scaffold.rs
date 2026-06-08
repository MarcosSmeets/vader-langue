//! Scaffolder for `vader new`: generates a project tree in one of the opinionated
//! architectures (clean/hexagonal/mvc/minimal), already in TDD format.
//!
//! Each architecture function returns a list of `(relative path, content)`.
//! `create` writes the files to disk under `<name>/`.

/// Default architecture for each project kind.
pub fn default_arch(kind: &str) -> &'static str {
    match kind {
        "api" => "tdd", // turnkey REST API: native router + DB-from-env + health-check
        "worker" => "clean",
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

fn dockerfile(_name: &str) -> String {
    // The native binary links libc dynamically (so not `scratch`); `--out` fixes the path.
    "# syntax=docker/dockerfile:1\n\n\
     # --- build: compile to a native binary (no run) ---\n\
     FROM vader/toolchain:latest AS build\n\
     WORKDIR /src\n\
     COPY . .\n\
     RUN vader build --out /out/app .\n\n\
     # --- runtime: slim image with libc ---\n\
     FROM debian:bookworm-slim\n\
     COPY --from=build /out/app /app\n\
     ENTRYPOINT [\"/app\"]\n"
        .to_string()
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
/// `db` is one of sqlite/postgres/mysql/mongo. Runs with `vader run`, or `docker compose up`.
pub fn create_api_tdd(name: &str, db: &str) -> Result<Vec<String>, String> {
    write_project(name, api_tdd_files(name, db))
}

/// Turnkey API for a chosen architecture: a User vertical slice (HTTP + DB + router)
/// laid out per `arch`. `tdd` is the flat layout; clean/hexagonal/mvc/ddd are layered
/// (the dependency rule is enforced by `vader build`/`run`). SQL databases only.
pub fn create_api(name: &str, db: &str, arch: &str) -> Result<Vec<String>, String> {
    if arch == "tdd" {
        return write_project(name, api_tdd_files(name, db));
    }
    write_project(name, api_layered_files(name, db, arch))
}

// ---- shared vertical-slice source (same code; only the folder layout changes per arch) ----

fn entity_vd() -> String {
    "// User entity — pure, no outward dependencies.\n\
     public struct User {\n    id   int\n    name string\n}\n"
        .to_string()
}

fn repo_interface_vd() -> String {
    "import \"std/json\"\n\n\
     // Repository contract: declared in the inner layer, implemented in the outer one.\n\
     public interface UserRepository {\n    \
         fn save(name string)\n    \
         fn all(): Json\n\
     }\n"
        .to_string()
}

fn usecase_vd() -> String {
    "// Business operations: depend on the repository abstraction, never a concrete DB.\n\
     public fn create_user(repo UserRepository, name string) {\n    \
         repo.save(name)\n\
     }\n\
     public fn list_users(repo UserRepository): Json {\n    \
         return repo.all()\n\
     }\n"
        .to_string()
}

fn repo_impl_vd(db: &str) -> String {
    let create = db_create_sql(db);
    format!(
        "import \"std/db\"\nimport \"std/env\"\nimport \"std/json\"\n\n\
         // Concrete repository — the only place that touches the database.\n\
         public struct PgUserRepository {{ tag string }}\n\n\
         public fn (r PgUserRepository) save(name string) {{\n    \
             DB conn = db.open(env.read(\"DATABASE_URL\"))\n    \
             Stmt st = db.prepare(conn, \"INSERT INTO users (name) VALUES (?)\")\n    \
             db.bind_str(st, name)\n    \
             db.run(st)\n    \
             db.close(conn)\n\
         }}\n\n\
         public fn (r PgUserRepository) all(): Json {{\n    \
             DB conn = db.open(env.read(\"DATABASE_URL\"))\n    \
             Rows rows = db.query(conn, \"SELECT id, name FROM users ORDER BY id\")\n    \
             Json arr = json.array()\n    \
             for db.next(rows) {{\n        \
                 Json u = json.object()\n        \
                 json.set_int(u, \"id\", db.col_int(rows, 0))\n        \
                 json.set_str(u, \"name\", db.col_text(rows, 1))\n        \
                 json.add(arr, u)\n    \
             }}\n    \
             db.close(conn)\n    \
             return arr\n\
         }}\n\n\
         // Schema setup, run once at startup.\n\
         public fn migrate() {{\n    \
             DB conn = db.open(env.read(\"DATABASE_URL\"))\n    \
             db.must(conn, \"{create}\")\n    \
             db.close(conn)\n\
         }}\n",
    )
}

fn handler_vd() -> String {
    "import \"std/http\"\nimport \"std/json\"\n\n\
     // Default health-check route — handy for load balancers and uptime checks.\n\
     public fn health(s Server) {\n    \
         http.json(s, 200, \"{\\\"status\\\":\\\"ok\\\"}\")\n\
     }\n\
     public fn listUsers(s Server) {\n    \
         PgUserRepository repo = PgUserRepository{ tag: \"pg\" }\n    \
         http.json(s, 200, json.encode(list_users(repo)))\n\
     }\n\
     public fn createUser(s Server) {\n    \
         Json body = json.parse(http.body(s))\n    \
         PgUserRepository repo = PgUserRepository{ tag: \"pg\" }\n    \
         create_user(repo, json.as_str(json.field(body, \"name\")))\n    \
         http.json(s, 201, \"{\\\"status\\\":\\\"created\\\"}\")\n\
     }\n"
        .to_string()
}

fn router_vd() -> String {
    "import \"std/http\"\n\n\
     // All HTTP routes in one place.\n\
     public fn router(): Router {\n    \
         Router r = newRouter()\n    \
         r.get(\"/health\", health)\n    \
         r.get(\"/users\", listUsers)\n    \
         r.post(\"/users\", createUser)\n    \
         return r\n\
     }\n"
        .to_string()
}

fn api_main_vd() -> String {
    "import \"std/http\"\n\n\
     // Entry point: ensure the schema, then start the server.\n\
     public fn main() {\n    \
         migrate()\n    \
         print(\"listening on http://localhost:8080\")\n    \
         serve(8080, router())\n\
     }\n"
        .to_string()
}

fn entity_test_vd() -> String {
    "// Tests live in test/ — segregated from the code they exercise.\n\
     test \"user holds its fields\" {\n    \
         User u = User{ id: 1, name: \"Ada\" }\n    \
         assert u.id == 1\n    \
         assert u.name == \"Ada\"\n\
     }\n"
        .to_string()
}

/// Maps the vertical-slice files onto each architecture's folder layout. The folder of
/// a file is its layer (the linter enforces the dependency direction between layers).
fn arch_layout(arch: &str, db: &str) -> Vec<(&'static str, String)> {
    let entity = entity_vd();
    let iface = repo_interface_vd();
    let usecase = usecase_vd();
    let repo = repo_impl_vd(db);
    let handler = handler_vd();
    let router = router_vd();
    let main = api_main_vd();
    match arch {
        "clean" => vec![
            ("domain/user.vd", entity),
            ("domain/user_repository.vd", iface),
            ("application/create_user.vd", usecase),
            ("infrastructure/user_repository_pg.vd", repo),
            ("interfaces/user_handler.vd", handler),
            ("interfaces/router.vd", router),
            ("cmd/main.vd", main),
        ],
        "hexagonal" => vec![
            ("core/domain/user.vd", entity),
            ("core/ports/user_repository.vd", iface),
            ("core/services/user_service.vd", usecase),
            ("adapters/outbound/user_repository_pg.vd", repo),
            ("adapters/inbound/user_handler.vd", handler),
            ("adapters/inbound/router.vd", router),
            ("cmd/main.vd", main),
        ],
        "mvc" => vec![
            ("models/user.vd", entity),
            ("repositories/user_repository.vd", iface),
            ("repositories/user_repository_pg.vd", repo),
            ("services/user_service.vd", usecase),
            ("controllers/user_controller.vd", handler),
            ("routes/routes.vd", router),
            ("cmd/main.vd", main),
        ],
        // DDD: organised by bounded context (here, `users`).
        _ => vec![
            ("contexts/users/domain/user.vd", entity),
            ("contexts/users/domain/user_repository.vd", iface),
            ("contexts/users/application/create_user.vd", usecase),
            ("contexts/users/infrastructure/user_repository_pg.vd", repo),
            ("contexts/users/infrastructure/http/user_handler.vd", handler),
            ("contexts/users/infrastructure/http/router.vd", router),
            ("cmd/main.vd", main),
        ],
    }
}

fn api_layered_files(name: &str, db: &str, arch: &str) -> Vec<(String, String)> {
    let dsn = db_dsn_example(db, name);
    let mut files: Vec<(String, String)> = vec![
        f(
            "vader.toml",
            format!(
                "[project]\nname         = \"{name}\"\nversion      = \"0.1.0\"\nkind         = \"api\"\narchitecture = \"{arch}\"\n\n\
                 [database]\nengine = \"{db}\"\n# the DSN is read from the DATABASE_URL environment variable\n\n\
                 [test]\n# flip to true once you have tests to enforce a coverage floor on every run.\ncoverage_gate = false\nmin_coverage  = 80\n",
            ),
        ),
        f(
            ".env.example",
            format!("# copy to .env and adjust; the app reads DATABASE_URL at startup\nDATABASE_URL={dsn}\n"),
        ),
        f("test/user_test.vd", entity_test_vd()),
        f("Dockerfile", dockerfile_native()),
        f(".dockerignore", ".git\n/target\n.env\n".to_string()),
        f("docker-compose.yml", docker_compose(db, name)),
    ];
    for (path, content) in arch_layout(arch, db) {
        files.push(f(path, content));
    }
    files
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
        "mongo" => format!("mongodb://localhost:27017/{name}"),
        _ => format!("./{name}.db"),
    }
}

fn api_tdd_files(name: &str, db: &str) -> Vec<(String, String)> {
    let dsn = db_dsn_example(db, name);
    let is_mongo = db == "mongo";

    // cmd/main.vd: a thin composition root. Route wiring lives in routes/, schema
    // setup in infra/ — `main` only bootstraps and starts the server.
    let main_vd = if is_mongo {
        "import \"std/http\"\n\n\
         // Entry point: wire nothing here — routes live in routes/, handlers in handlers/.\n\
         public fn main() {\n    \
             print(\"listening on http://localhost:8080\")\n    \
             serve(8080, router())\n\
         }\n"
            .to_string()
    } else {
        "import \"std/http\"\n\n\
         // Entry point: ensure the schema (infra/), then start the server (routes/).\n\
         public fn main() {\n    \
             migrate()                          // create tables if missing (DATABASE_URL)\n    \
             print(\"listening on http://localhost:8080\")\n    \
             serve(8080, router())\n\
         }\n"
            .to_string()
    };

    // routes/routes.vd: every route in one place — the wiring is out of `main`.
    let routes_vd = "import \"std/http\"\n\n\
         // All HTTP routes in one place. `main` just calls `router()`.\n\
         public fn router(): Router {\n    \
             Router r = newRouter()\n    \
             r.get(\"/health\", health)          // default health-check route\n    \
             r.get(\"/users\", listUsers)\n    \
             r.post(\"/users\", createUser)\n    \
             return r\n\
         }\n"
        .to_string();

    // handlers respond with JSON by default — `http.json(s, status, body)`, no content type.
    let users_vd = if is_mongo {
        "import \"std/http\"\nimport \"std/mongo\"\nimport \"std/env\"\nimport \"std/json\"\n\n\
         public fn listUsers(s Server) {\n    \
             Mongo m = mongo.connect(env.read(\"DATABASE_URL\"))\n    \
             Json users = mongo.find(m, \"users\", json.object())\n    \
             http.json(s, 200, json.encode(users))\n    \
             mongo.close(m)\n\
         }\n\n\
         public fn createUser(s Server) {\n    \
             Mongo m = mongo.connect(env.read(\"DATABASE_URL\"))\n    \
             Json body = json.parse(http.body(s))\n    \
             error e = mongo.insert(m, \"users\", body)\n    \
             if e != nil {\n        \
                 http.json(s, 500, \"{\\\"error\\\":\\\"insert failed\\\"}\")\n    \
             } else {\n        \
                 http.json(s, 201, \"{\\\"status\\\":\\\"created\\\"}\")\n    \
             }\n    \
             mongo.close(m)\n\
         }\n"
            .to_string()
    } else {
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
             http.json(s, 200, json.encode(arr))\n    \
             db.close(conn)\n\
         }\n\n\
         public fn createUser(s Server) {\n    \
             DB conn = db.open(env.read(\"DATABASE_URL\"))\n    \
             Json body = json.parse(http.body(s))\n    \
             string name = json.as_str(json.field(body, \"name\"))\n    \
             // parameterized query — `?` placeholders, no string concatenation.\n    \
             Stmt st = db.prepare(conn, \"INSERT INTO users (name) VALUES (?)\")\n    \
             db.bind_str(st, name)\n    \
             db.run(st)\n    \
             http.json(s, 201, \"{\\\"status\\\":\\\"created\\\"}\")\n    \
             db.close(conn)\n\
         }\n"
            .to_string()
    };

    let mut files = vec![
        f(
            "vader.toml",
            format!(
                "[project]\nname         = \"{name}\"\nversion      = \"0.1.0\"\nkind         = \"api\"\narchitecture = \"tdd\"\n\n\
                 [database]\nengine = \"{db}\"\n# the DSN is read from the DATABASE_URL environment variable\n\n\
                 [test]\n# flip to true once you have tests to enforce a coverage floor on every run.\ncoverage_gate = false\nmin_coverage  = 80\n",
            ),
        ),
        f(
            ".env.example",
            format!("# copy to .env and adjust; the app reads DATABASE_URL at startup\nDATABASE_URL={dsn}\n"),
        ),
        f("cmd/main.vd", main_vd),
        f("routes/routes.vd", routes_vd),
        f(
            "handlers/health.vd",
            "import \"std/http\"\n\n\
             // Default health-check route — handy for load balancers and uptime checks.\n\
             public fn health(s Server) {\n    \
                 http.json(s, 200, \"{\\\"status\\\":\\\"ok\\\"}\")\n\
             }\n"
                .to_string(),
        ),
        f("handlers/users.vd", users_vd),
        f(
            "domain/user.vd",
            "// User entity.\npublic struct User {\n    id   int\n    name string\n}\n".to_string(),
        ),
        f(
            "test/user_test.vd",
            "// Tests live in test/ — segregated from the code they exercise.\n\
             test \"user holds its fields\" {\n    \
                 User u = User{ id: 1, name: \"Ada\" }\n    \
                 assert u.id == 1\n    \
                 assert u.name == \"Ada\"\n\
             }\n"
                .to_string(),
        ),
        f("Dockerfile", dockerfile_native()),
        f(".dockerignore", ".git\n/target\n.env\n".to_string()),
        f("docker-compose.yml", docker_compose(db, name)),
    ];

    // SQL engines get an infra layer that owns schema setup; Mongo is schemaless.
    if !is_mongo {
        let create_sql = db_create_sql(db);
        files.push(f(
            "infra/db.vd",
            format!(
                "import \"std/db\"\nimport \"std/env\"\n\n\
                 // Schema setup, run once at startup. The DSN comes from DATABASE_URL.\n\
                 public fn migrate() {{\n    \
                     DB conn = db.open(env.read(\"DATABASE_URL\"))\n    \
                     db.must(conn, \"{create_sql}\")\n    \
                     db.close(conn)\n\
                 }}\n",
            ),
        ));
    }

    files
}

/// A turnkey `docker-compose.yml`: builds the app from the Dockerfile and, for a
/// server-backed engine, brings up the database too — credentials and the in-network
/// `DATABASE_URL` are pre-wired so `docker compose up` just works. SQLite is embedded,
/// so it only needs the app (with a volume to persist the file).
fn docker_compose(db: &str, name: &str) -> String {
    match db {
        "postgres" => format!(
            "services:\n  \
               app:\n    \
                 build: .\n    \
                 ports:\n      - \"8080:8080\"\n    \
                 environment:\n      \
                   DATABASE_URL: postgres://postgres:password@db:5432/{name}\n    \
                 depends_on:\n      db:\n        condition: service_healthy\n  \
               db:\n    \
                 image: postgres:16\n    \
                 environment:\n      \
                   POSTGRES_PASSWORD: password\n      POSTGRES_DB: {name}\n    \
                 ports:\n      - \"5432:5432\"\n    \
                 healthcheck:\n      \
                   test: [\"CMD-SHELL\", \"pg_isready -U postgres\"]\n      \
                   interval: 3s\n      timeout: 3s\n      retries: 10\n    \
                 volumes:\n      - pgdata:/var/lib/postgresql/data\n\
             volumes:\n  pgdata:\n",
        ),
        "mysql" => format!(
            "services:\n  \
               app:\n    \
                 build: .\n    \
                 ports:\n      - \"8080:8080\"\n    \
                 environment:\n      \
                   DATABASE_URL: mysql://root:password@db:3306/{name}\n    \
                 depends_on:\n      db:\n        condition: service_healthy\n  \
               db:\n    \
                 image: mysql:8.0\n    \
                 # native_password keeps auth simple (no TLS/RSA needed for caching_sha2).\n    \
                 command: [\"--default-authentication-plugin=mysql_native_password\"]\n    \
                 environment:\n      \
                   MYSQL_ROOT_PASSWORD: password\n      MYSQL_DATABASE: {name}\n    \
                 ports:\n      - \"3306:3306\"\n    \
                 healthcheck:\n      \
                   test: [\"CMD\", \"mysqladmin\", \"ping\", \"-h\", \"localhost\", \"-ppassword\"]\n      \
                   interval: 3s\n      timeout: 3s\n      retries: 10\n    \
                 volumes:\n      - mysqldata:/var/lib/mysql\n\
             volumes:\n  mysqldata:\n",
        ),
        "mongo" => format!(
            "services:\n  \
               app:\n    \
                 build: .\n    \
                 ports:\n      - \"8080:8080\"\n    \
                 environment:\n      \
                   DATABASE_URL: mongodb://db:27017/{name}\n    \
                 depends_on:\n      - db\n  \
               db:\n    \
                 image: mongo:7\n    \
                 ports:\n      - \"27017:27017\"\n    \
                 volumes:\n      - mongodata:/data/db\n\
             volumes:\n  mongodata:\n",
        ),
        // SQLite: embedded — no DB service, just persist the file on a volume.
        _ => format!(
            "services:\n  \
               app:\n    \
                 build: .\n    \
                 ports:\n      - \"8080:8080\"\n    \
                 environment:\n      \
                   DATABASE_URL: /data/{name}.db\n    \
                 volumes:\n      - sqlitedata:/data\n\
             volumes:\n  sqlitedata:\n",
        ),
    }
}

/// Dockerfile for a natively-built API: `vader build --out` then a slim libc base
/// (the native binary embeds SQLite but links libc dynamically, so not `scratch`).
fn dockerfile_native() -> String {
    "# syntax=docker/dockerfile:1\n\n\
     # --- build: compile to a native binary (no run) ---\n\
     FROM vader/toolchain:latest AS build\n\
     WORKDIR /src\n\
     COPY . .\n\
     RUN vader build --out /out/server .\n\n\
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
        assert_eq!(default_arch("api"), "tdd");
        assert_eq!(default_arch("cli"), "minimal");
        assert_eq!(default_arch("lib"), "minimal");
    }

    #[test]
    fn layered_api_places_files_per_architecture() {
        let has = |files: &[(String, String)], p: &str| files.iter().any(|(path, _)| path == p);
        for (arch, entity) in [
            ("clean", "domain/user.vd"),
            ("hexagonal", "core/domain/user.vd"),
            ("mvc", "models/user.vd"),
            ("ddd", "contexts/users/domain/user.vd"),
        ] {
            let files = api_layered_files("shop", "sqlite", arch);
            assert!(has(&files, entity), "{arch}: missing {entity}");
            assert!(has(&files, "cmd/main.vd"), "{arch}: missing cmd/main.vd");
            assert!(has(&files, "docker-compose.yml"), "{arch}: missing compose");
            // the architecture is recorded so `vader build` can enforce its layer rules.
            assert!(
                files
                    .iter()
                    .any(|(p, c)| p == "vader.toml" && c.contains(&format!("\"{arch}\""))),
                "{arch}: vader.toml missing architecture"
            );
        }
    }
}
