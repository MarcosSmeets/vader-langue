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
    let files = files_for(kind, arch, name)?;
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
