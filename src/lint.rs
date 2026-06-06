//! Linter de arquitetura: fiscaliza a regra de dependência da arquitetura do projeto.
//!
//! A camada de um arquivo vem do seu caminho (`domain/`, `usecase/`, ...); a camada
//! de um import vem do caminho importado. A regra de ouro: **camada interna não pode
//! importar camada externa**. Núcleo puro (camada 0) também não pode importar I/O.
//!
//! Detalhes e ruleset em `docs/architecture-rules.md`.

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

/// Rank da camada (menor = mais interno). `None` se o segmento não é uma camada.
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

/// Rank da camada mais externa (onde I/O é permitido).
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

/// Roda o linter de arquitetura sobre um arquivo, dadas suas dependências.
pub fn lint(arch: &str, file_path: &str, imports: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    let (flayer, frank) = match layer_of(arch, file_path) {
        Some(x) => x,
        None => return out, // arquivo fora de uma camada conhecida (ex.: cmd/, minimal)
    };

    for imp in imports {
        if frank < max_rank(arch) && is_io(imp) {
            out.push(Finding {
                severity: Severity::Error,
                rule: "R3",
                message: format!(
                    "`{}` não pode importar I/O `{}` diretamente (vai pela porta na camada externa)",
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
                        "`{}` (interna) não pode importar `{}` (externa) — a dependência aponta para dentro",
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
