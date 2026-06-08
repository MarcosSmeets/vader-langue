//! Architecture linter: enforces the project's architecture dependency rule.
//!
//! A file's layer comes from its path (`domain/`, `usecase/`, ...); an import's
//! layer comes from the imported path. The golden rule: **an inner layer cannot
//! import an outer layer**. A pure core (layer 0) also cannot import I/O.
//!
//! Details and ruleset in `docs/architecture-rules.md`.

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub severity: Severity,
    pub rule: &'static str,
    pub message: String,
}

/// Layer rank (lower = more inner). `None` if the segment is not a layer.
/// Folder names match the scaffolds (`vader new api --arch <clean|hexagonal|mvc|ddd>`).
fn rank(arch: &str, seg: &str) -> Option<usize> {
    match arch {
        "clean" => match seg {
            "domain" => Some(0),
            "application" => Some(1),
            "infrastructure" => Some(2),
            "interfaces" => Some(3),
            _ => None,
        },
        "hexagonal" => match seg {
            "core" => Some(0),
            "adapters" => Some(1),
            "infrastructure" => Some(2),
            _ => None,
        },
        "mvc" => match seg {
            "models" => Some(0),
            "repositories" => Some(1),
            "services" => Some(2),
            "controllers" | "routes" | "middleware" => Some(3),
            _ => None,
        },
        // DDD layers live inside each bounded context (contexts/<ctx>/<layer>/...).
        "ddd" => match seg {
            "domain" => Some(0),
            "application" => Some(1),
            "infrastructure" => Some(2),
            _ => None,
        },
        _ => None,
    }
}

/// Innermost rank at which doing I/O (std/db, std/http, ...) is allowed: the adapter
/// layers. Anything more inner than this must stay pure.
fn io_floor(arch: &str) -> usize {
    match arch {
        "clean" | "ddd" => 2,
        "hexagonal" | "mvc" => 1,
        _ => 0,
    }
}

fn layer_of(arch: &str, path: &str) -> Option<(String, usize)> {
    for seg in path.split(['/', '\\']) {
        if let Some(r) = rank(arch, seg) {
            return Some((seg.to_string(), r));
        }
    }
    None
}

fn is_io(import: &str) -> bool {
    import.starts_with("std/db")
        || import.starts_with("std/http")
        || import.starts_with("std/mongo")
        || import.contains("net/http")
}

/// Convention check: project `.vd` files must be snake_case (lowercase, digits, `_`),
/// starting with a letter. Catches `UserHandler.vd`, `user-handler.vd`, etc.
pub fn check_filename(path: &str) -> Option<Finding> {
    let p = std::path::Path::new(path);
    let stem = p.file_stem()?.to_str()?;
    let ok = stem
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase())
        && stem
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if ok {
        return None;
    }
    let shown = p.file_name().map(|s| s.to_string_lossy().to_string())?;
    Some(Finding {
        severity: Severity::Error,
        rule: "N1",
        message: format!(
            "file name `{}` is not snake_case (use lowercase letters, digits and `_`, e.g. `user_handler.vd`)",
            shown
        ),
    })
}

/// Runs the architecture linter over a file, given its dependencies.
pub fn lint(arch: &str, file_path: &str, imports: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    let (flayer, frank) = match layer_of(arch, file_path) {
        Some(x) => x,
        None => return out, // file outside a known layer (e.g. cmd/, minimal)
    };

    for imp in imports {
        if frank < io_floor(arch) && is_io(imp) {
            out.push(Finding {
                severity: Severity::Error,
                rule: "R3",
                message: format!(
                    "`{}` cannot import I/O `{}` directly (go through the port in the outer layer)",
                    flayer, imp
                ),
            });
            continue;
        }
        if let Some((ilayer, irank)) = layer_of(arch, imp) {
            if frank < irank {
                out.push(Finding {
                    severity: Severity::Error,
                    rule: "R1",
                    message: format!(
                        "`{}` (inner) cannot import `{}` (outer) — the dependency must point inward",
                        flayer, ilayer
                    ),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn errors(arch: &str, path: &str, imports: &[&str]) -> Vec<Finding> {
        let imps: Vec<String> = imports.iter().map(|s| s.to_string()).collect();
        lint(arch, path, &imps)
    }

    #[test]
    fn domain_importing_infrastructure_is_an_error() {
        let f = errors("clean", "loja/domain/user.vd", &["loja/infrastructure/db"]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "R1");
        assert_eq!(f[0].severity, Severity::Error);
    }

    #[test]
    fn infrastructure_importing_domain_is_fine() {
        let f = errors("clean", "loja/infrastructure/db/repo.vd", &["loja/domain"]);
        assert!(f.is_empty());
    }

    #[test]
    fn application_importing_infrastructure_is_an_error() {
        let f = errors("clean", "loja/application/create.vd", &["loja/infrastructure/db"]);
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn domain_doing_io_is_an_error() {
        let f = errors("clean", "loja/domain/user.vd", &["std/db/postgres"]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "R3");
    }

    #[test]
    fn infrastructure_doing_io_is_fine() {
        let f = errors("clean", "loja/infrastructure/db/repo.vd", &["std/db/postgres"]);
        assert!(f.is_empty());
    }

    #[test]
    fn hexagonal_core_importing_adapter_is_an_error() {
        let f = errors("hexagonal", "app/core/service/s.vd", &["app/adapters/outbound/db"]);
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn ddd_domain_importing_infrastructure_is_an_error() {
        let f = errors(
            "ddd",
            "app/contexts/users/domain/entity/user.vd",
            &["app/contexts/users/infrastructure/db"],
        );
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "R1");
    }

    #[test]
    fn mvc_model_importing_controller_is_an_error() {
        let f = errors("mvc", "app/models/user.vd", &["app/controllers/user_controller"]);
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn minimal_has_no_rules() {
        let f = errors("minimal", "app/src/foo.vd", &["app/src/bar", "std/db"]);
        assert!(f.is_empty());
    }

    #[test]
    fn snake_case_filenames_pass() {
        assert!(check_filename("app/domain/user_repository.vd").is_none());
        assert!(check_filename("app/cmd/main.vd").is_none());
    }

    #[test]
    fn non_snake_case_filenames_fail() {
        assert_eq!(check_filename("app/UserRepository.vd").unwrap().rule, "N1");
        assert_eq!(check_filename("app/user-repository.vd").unwrap().rule, "N1");
        assert_eq!(check_filename("app/2cool.vd").unwrap().rule, "N1");
    }
}
