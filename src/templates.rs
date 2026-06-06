//! Project templates customized by the dev.
//!
//! A template is a folder in `~/.vader/templates/<name>/`. When creating a project,
//! the `__name__` placeholder is replaced by the project name — both in the file
//! contents and in the file/folder names. This way the dev keeps their own
//! patterns (structure, libs, organization) and reuses them with `vader new --template`.
//!
//! (Sharing templates via the package registry is future work.)

use std::path::{Path, PathBuf};

const PLACEHOLDER: &str = "__name__";
const SKIP: &[&str] = &[".git", "target", "node_modules"];

/// Replaces the placeholder with the project name.
pub fn apply_name(s: &str, name: &str) -> String {
    s.replace(PLACEHOLDER, name)
}

fn templates_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".vader").join("templates")
}

/// Lists the available custom templates.
pub fn list() -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(templates_dir()) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                names.push(e.file_name().to_string_lossy().to_string());
            }
        }
    }
    names.sort();
    names
}

/// Saves a folder as template `<name>`. Returns how many files were copied.
pub fn save(name: &str, src: &str) -> Result<usize, String> {
    let src = Path::new(src);
    if !src.is_dir() {
        return Err(format!("`{}` is not a folder", src.display()));
    }
    let dest = templates_dir().join(name);
    if dest.exists() {
        return Err(format!("template `{}` already exists", name));
    }
    let mut count = 0;
    copy_tree(src, &dest, &mut count)?;
    Ok(count)
}

fn copy_tree(src: &Path, dst: &Path, count: &mut usize) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if SKIP.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        let dest = dst.join(&name);
        if path.is_dir() {
            copy_tree(&path, &dest, count)?;
        } else {
            std::fs::copy(&path, &dest).map_err(|e| e.to_string())?;
            *count += 1;
        }
    }
    Ok(())
}

/// Creates a project from the template `<name>`, replacing `__name__` with the
/// project name. Fails if the template does not exist or the destination already exists.
pub fn create_from(name: &str, project: &str) -> Result<Vec<String>, String> {
    let tmpl = templates_dir().join(name);
    if !tmpl.is_dir() {
        return Err(format!(
            "template `{}` not found (see `vader template list`)",
            name
        ));
    }
    let root = Path::new(project);
    if root.exists() {
        return Err(format!("directory `{}` already exists", project));
    }
    let mut created = Vec::new();
    instantiate(&tmpl, root, project, &mut created)?;
    Ok(created)
}

fn instantiate(
    tmpl: &Path,
    dst: &Path,
    name: &str,
    created: &mut Vec<String>,
) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(tmpl).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let raw = entry.file_name().to_string_lossy().to_string();
        let dest = dst.join(apply_name(&raw, name));
        let path = entry.path();
        if path.is_dir() {
            instantiate(&path, &dest, name, created)?;
        } else {
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            // only substitute in text files; binaries are copied as-is.
            match String::from_utf8(bytes.clone()) {
                Ok(text) => std::fs::write(&dest, apply_name(&text, name)),
                Err(_) => std::fs::write(&dest, bytes),
            }
            .map_err(|e| e.to_string())?;
            created.push(dest.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_placeholder_everywhere() {
        assert_eq!(apply_name("import \"__name__/domain\"", "loja"), "import \"loja/domain\"");
        assert_eq!(apply_name("__name___test.vd", "loja"), "loja_test.vd");
        assert_eq!(apply_name("no placeholder", "loja"), "no placeholder");
    }
}
