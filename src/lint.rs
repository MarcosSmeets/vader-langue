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
fn rank(arch: &str, seg: &str) -> Option<usize> {
    match arch {
        "clean" => match seg {
            "domain" => Some(0),
            "usecase" => Some(1),
            "adapter" => Some(2),
            "infra" => Some(3),
            _ => None,
        },
        "hexagonal" => match seg {
            "core" | "domain" | "port" | "service" => Some(0),
            "adapter" => Some(1),
            _ => None,
        },
        "mvc" => match seg {
            "model" => Some(0),
            "controller" | "view" => Some(1),
            _ => None,
        },
        _ => None,
    }
}

/// Rank of the outermost layer (where I/O is allowed).
fn max_rank(arch: &str) -> usize {
    match arch {
        "clean" => 3,
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
    import.starts_with("std/db") || import.starts_with("std/http") || import.contains("net/http")
}

/// Runs the architecture linter over a file, given its dependencies.
pub fn lint(arch: &str, file_path: &str, imports: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    let (flayer, frank) = match layer_of(arch, file_path) {
        Some(x) => x,
        None => return out, // file outside a known layer (e.g. cmd/, minimal)
    };

    for imp in imports {
        if frank < max_rank(arch) && is_io(imp) {
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
    fn domain_importing_infra_is_an_error() {
        let f = errors("clean", "loja/domain/user.vd", &["loja/infra/db"]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "R1");
        assert_eq!(f[0].severity, Severity::Error);
    }

    #[test]
    fn infra_importing_domain_is_fine() {
        let f = errors("clean", "loja/infra/db/repo.vd", &["loja/domain"]);
        assert!(f.is_empty());
    }

    #[test]
    fn usecase_importing_infra_is_an_error() {
        let f = errors("clean", "loja/usecase/create.vd", &["loja/infra/db"]);
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn domain_doing_io_is_an_error() {
        let f = errors("clean", "loja/domain/user.vd", &["std/db/postgres"]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "R3");
    }

    #[test]
    fn infra_doing_io_is_fine() {
        let f = errors("clean", "loja/infra/db/repo.vd", &["std/db/postgres"]);
        assert!(f.is_empty());
    }

    #[test]
    fn hexagonal_core_importing_adapter_is_an_error() {
        let f = errors("hexagonal", "app/core/service/s.vd", &["app/adapter/outbound/db"]);
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn minimal_has_no_rules() {
        let f = errors("minimal", "app/src/foo.vd", &["app/src/bar", "std/db"]);
        assert!(f.is_empty());
    }
}
