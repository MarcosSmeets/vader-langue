//! `vader gen <thing> <Name>`: generates an artifact (function, struct, usecase, handler)
//! **always alongside its mirror test** — TDD by default is the rule, not an option.
//!
//! `gen_files` is pure (returns `(path, content)`); `create` writes to disk.

/// Converts PascalCase/camelCase to snake_case (for file names).
pub fn to_snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// List of files `(path relative to cwd, content)` for the artifact.
/// Always includes the implementation file **and** the mirror `_test.vd`.
pub fn gen_files(thing: &str, name: &str) -> Result<Vec<(String, String)>, String> {
    let snake = to_snake(name);
    match thing {
        "fn" => Ok(vec![
            (
                format!("{snake}.vd"),
                format!(
                    "public fn {name}() {{\n    // TODO: implement\n}}\n",
                ),
            ),
            (
                format!("{snake}_test.vd"),
                format!(
                    "// auto-generated alongside the function (TDD by default).\n\n\
                     test \"{name} works\" {{\n    \
                         // TODO: arrange / act / assert\n    \
                         assert true\n\
                     }}\n",
                ),
            ),
        ]),
        "struct" => Ok(vec![
            (
                format!("{snake}.vd"),
                format!("public struct {name} {{\n    id int\n}}\n"),
            ),
            (
                format!("{snake}_test.vd"),
                format!(
                    "// auto-generated alongside the struct (TDD by default).\n\n\
                     test \"{name} can be built\" {{\n    \
                         {name} value = {name}{{ id: 1 }}\n    \
                         assert value.id == 1\n\
                     }}\n",
                ),
            ),
        ]),
        "usecase" => Ok(vec![
            (
                format!("usecase/{snake}.vd"),
                format!(
                    "public struct {name} {{\n    \
                         // TODO: inject the required ports (Repository/Gateway)\n\
                     }}\n\n\
                     public fn (uc {name}) execute(): (bool, error) {{\n    \
                         // TODO: implement\n    \
                         return true, nil\n\
                     }}\n",
                ),
            ),
            (
                format!("usecase/{snake}_test.vd"),
                format!(
                    "// auto-generated alongside the use case (TDD by default).\n\n\
                     test \"{name}.execute runs\" {{\n    \
                         {name} uc = {name}{{}}\n    \
                         bool ok, error err = uc.execute()\n    \
                         assert err == nil\n    \
                         assert ok == true\n\
                     }}\n",
                ),
            ),
        ]),
        "handler" => Ok(vec![
            (
                format!("adapter/http/{snake}.vd"),
                format!(
                    "public struct {name} {{\n}}\n\n\
                     public fn (h {name}) handle(): (int, error) {{\n    \
                         // TODO: call the use case\n    \
                         return 200, nil\n\
                     }}\n",
                ),
            ),
            (
                format!("adapter/http/{snake}_test.vd"),
                format!(
                    "// auto-generated alongside the handler (TDD by default).\n\n\
                     test \"{name}.handle returns 200\" {{\n    \
                         {name} h = {name}{{}}\n    \
                         int status, error err = h.handle()\n    \
                         assert err == nil\n    \
                         assert status == 200\n\
                     }}\n",
                ),
            ),
        ]),
        other => Err(format!(
            "unknown artifact `{}` (fn|struct|usecase|handler)",
            other
        )),
    }
}

/// Writes the files relative to the current directory. Fails if any already exists.
pub fn create(thing: &str, name: &str) -> Result<Vec<String>, String> {
    let files = gen_files(thing, name)?;
    for (rel, _) in &files {
        if std::path::Path::new(rel).exists() {
            return Err(format!("file `{}` already exists", rel));
        }
    }
    let mut created = Vec::new();
    for (rel, content) in files {
        let path = std::path::Path::new(&rel);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        std::fs::write(path, content).map_err(|e| e.to_string())?;
        created.push(rel);
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(thing: &str, name: &str) -> Vec<String> {
        gen_files(thing, name)
            .unwrap()
            .into_iter()
            .map(|(p, _)| p)
            .collect()
    }

    #[test]
    fn snake_case_conversion() {
        assert_eq!(to_snake("CreateOrder"), "create_order");
        assert_eq!(to_snake("somar"), "somar");
    }

    #[test]
    fn fn_creates_mirror_test() {
        let p = paths("fn", "somar");
        assert!(p.contains(&"somar.vd".to_string()));
        assert!(p.contains(&"somar_test.vd".to_string()));
    }

    #[test]
    fn usecase_goes_to_usecase_dir() {
        let p = paths("usecase", "CreateOrder");
        assert!(p.contains(&"usecase/create_order.vd".to_string()));
        assert!(p.contains(&"usecase/create_order_test.vd".to_string()));
    }

    #[test]
    fn every_artifact_ships_a_test() {
        for t in ["fn", "struct", "usecase", "handler"] {
            let p = paths(t, "Thing");
            assert!(
                p.iter().any(|x| x.ends_with("_test.vd")),
                "{t} without mirror test"
            );
        }
    }

    #[test]
    fn unknown_artifact_is_an_error() {
        assert!(gen_files("widget", "X").is_err());
    }

    #[test]
    fn generated_code_parses() {
        // what gen produces must be valid (parseable) Vader.
        for t in ["fn", "struct", "usecase", "handler"] {
            for (path, content) in gen_files(t, "Thing").unwrap() {
                let toks = crate::lexer::tokenize(&content).unwrap();
                crate::parser::parse(toks)
                    .unwrap_or_else(|e| panic!("{} does not parse: {}", path, e));
            }
        }
    }
}
