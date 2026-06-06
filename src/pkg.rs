//! Package manager (v1): dependencies via **git/URL**, no hosted registry.
//!
//! Each dependency declared in the `[dependencies]` of `vader.toml` is resolved with
//! `git clone` into a local cache (`~/.vader/pkg`) and its `.vd` files enter the project via
//! the module system. A central hosted registry is a future layer on top of this.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A dependency: `name` is the package used in `import`, `url` is the git source
/// (http(s)/ssh URL or local path), `version` is a tag/branch (empty = default).
#[derive(Clone, Debug, PartialEq)]
pub struct Dep {
    pub name: String,
    pub url: String,
    pub version: String,
}

/// Root of the package cache: `~/.vader/pkg`.
pub fn cache_root() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".vader").join("pkg")
}

/// Cache directory of a dep (`~/.vader/pkg/<name>@<version|default>`).
pub fn dep_dir(d: &Dep) -> PathBuf {
    let v = if d.version.is_empty() {
        "default"
    } else {
        &d.version
    };
    cache_root().join(format!("{}@{}", d.name, v))
}

/// Ensures the dep is in the cache (runs `git clone` if missing). Returns (path, resolved commit).
pub fn fetch(d: &Dep) -> Result<(PathBuf, String), String> {
    let dir = dep_dir(d);
    if !dir.join(".git").exists() {
        std::fs::create_dir_all(cache_root()).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&dir); // clean up partial clone
        let mut cmd = Command::new("git");
        cmd.arg("clone").arg("--depth").arg("1");
        if !d.version.is_empty() {
            cmd.arg("--branch").arg(&d.version);
        }
        cmd.arg(&d.url).arg(&dir);
        let st = cmd
            .status()
            .map_err(|e| format!("failed to invoke git (is it installed?): {}", e))?;
        if !st.success() {
            return Err(format!(
                "git clone failed for `{}` ({})",
                d.name, d.url
            ));
        }
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(&dir)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|e| e.to_string())?;
    let commit = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok((dir, commit))
}

/// Derives the package name from the URL (last segment, without `.git`).
pub fn derive_name(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("dep")
        .trim_end_matches(".git")
        .to_string()
}

/// Splits `url[@version]` into a (url, version), respecting ssh URLs `git@host:...`.
pub fn split_source(src: &str) -> (String, String) {
    match src.rsplit_once('@') {
        // only a version if what comes after the @ doesn't look like part of a URL
        Some((u, v)) if !v.contains('/') && !v.contains(':') && !u.is_empty() => {
            (u.to_string(), v.to_string())
        }
        _ => (src.to_string(), String::new()),
    }
}

/// Reads the `[dependencies]` from a `vader.toml` content (simple line parser).
pub fn parse_deps(toml: &str) -> Vec<Dep> {
    let mut deps = Vec::new();
    let mut in_section = false;
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_section = t == "[dependencies]";
            continue;
        }
        if !in_section || t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some((name, rest)) = t.split_once('=') {
            let name = name.trim().to_string();
            let val = rest.trim().trim_matches('"').to_string();
            let (url, version) = split_source(&val);
            if !name.is_empty() && !url.is_empty() {
                deps.push(Dep { name, url, version });
            }
        }
    }
    deps
}

/// Rewrites the `vader.toml` content with the given `[dependencies]` section (at the end).
pub fn write_deps(toml: &str, deps: &[Dep]) -> String {
    let mut out = String::new();
    let mut skip = false;
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            skip = t == "[dependencies]";
        }
        if !skip {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !deps.is_empty() {
        while out.ends_with("\n\n") {
            out.pop();
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("\n[dependencies]\n");
        for d in deps {
            if d.version.is_empty() {
                out.push_str(&format!("{} = \"{}\"\n", d.name, d.url));
            } else {
                out.push_str(&format!("{} = \"{}@{}\"\n", d.name, d.url, d.version));
            }
        }
    }
    out
}

// ===================== package registry ==============================
// A registry is an `index.json` (map name -> {url, version}) in a local directory
// or in a git repo. No dedicated server: it can be a GitHub repo (tap style).

fn registry_is_remote(registry: &str) -> bool {
    registry.contains("://") || (registry.contains('@') && registry.contains(':'))
}

/// Resolves the path of the registry's `index.json` (clones if it's a git repo).
pub fn registry_index(registry: &str) -> Result<PathBuf, String> {
    if registry_is_remote(registry) {
        let dir = cache_root()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("registry");
        if dir.join(".git").exists() {
            let _ = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .arg("pull")
                .arg("--quiet")
                .status();
        } else {
            if let Some(p) = dir.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let _ = std::fs::remove_dir_all(&dir);
            let st = Command::new("git")
                .arg("clone")
                .arg("--depth")
                .arg("1")
                .arg(registry)
                .arg(&dir)
                .status()
                .map_err(|e| format!("git: {}", e))?;
            if !st.success() {
                return Err("failed to clone the registry".into());
            }
        }
        Ok(dir.join("index.json"))
    } else {
        Ok(Path::new(registry).join("index.json"))
    }
}

/// Looks up a package by name in the registry.
pub fn registry_lookup(registry: &str, name: &str) -> Result<Dep, String> {
    let idx = registry_index(registry)?;
    let content = std::fs::read_to_string(&idx)
        .map_err(|e| format!("could not read {}: {}", idx.display(), e))?;
    let json = crate::json::parse(&content).ok_or("invalid index.json")?;
    let entry = json
        .get(name)
        .ok_or(format!("package `{}` is not in the registry", name))?;
    let url = entry
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or("entry without `url`")?
        .to_string();
    let version = entry
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(Dep {
        name: name.to_string(),
        url,
        version,
    })
}

/// Adds/updates a package in the registry's `index.json` (writes locally).
pub fn registry_publish(registry: &str, dep: &Dep) -> Result<(), String> {
    use crate::json::Json;
    let idx = registry_index(registry)?;
    let mut entries: Vec<(String, Json)> = match std::fs::read_to_string(&idx)
        .ok()
        .and_then(|c| crate::json::parse(&c))
    {
        Some(Json::Obj(o)) => o,
        _ => Vec::new(),
    };
    let entry = Json::Obj(vec![
        ("url".to_string(), Json::Str(dep.url.clone())),
        ("version".to_string(), Json::Str(dep.version.clone())),
    ]);
    entries.retain(|(k, _)| k != &dep.name);
    entries.push((dep.name.clone(), entry));
    if let Some(p) = idx.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    std::fs::write(&idx, Json::Obj(entries).to_string()).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dependencies_section() {
        let toml = "name = \"app\"\n\n[dependencies]\ngreeter = \"/tmp/greeter\"\nhttpx = \"https://github.com/u/httpx@v1.2.0\"\n";
        let deps = parse_deps(toml);
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0], Dep { name: "greeter".into(), url: "/tmp/greeter".into(), version: "".into() });
        assert_eq!(deps[1].version, "v1.2.0");
        assert_eq!(deps[1].url, "https://github.com/u/httpx");
    }

    #[test]
    fn ssh_url_not_treated_as_version() {
        let (url, version) = split_source("git@github.com:u/repo.git");
        assert_eq!(url, "git@github.com:u/repo.git");
        assert_eq!(version, "");
    }

    #[test]
    fn derives_names() {
        assert_eq!(derive_name("/tmp/greeter"), "greeter");
        assert_eq!(derive_name("https://github.com/u/greeter.git"), "greeter");
        assert_eq!(derive_name("git@github.com:u/greeter.git"), "greeter");
    }

    #[test]
    fn registry_publish_then_lookup() {
        let dir = std::env::temp_dir().join("vader_reg_unit_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let reg = dir.to_str().unwrap();
        let dep = Dep {
            name: "foo".into(),
            url: "https://example/foo".into(),
            version: "v1.2.3".into(),
        };
        registry_publish(reg, &dep).unwrap();
        assert_eq!(registry_lookup(reg, "foo").unwrap(), dep);
        assert!(registry_lookup(reg, "missing").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn upsert_roundtrip() {
        let toml = "name = \"app\"\n";
        let deps = vec![Dep { name: "greeter".into(), url: "/tmp/greeter".into(), version: "".into() }];
        let out = write_deps(toml, &deps);
        assert!(out.contains("[dependencies]"));
        assert_eq!(parse_deps(&out), deps);
    }
}
