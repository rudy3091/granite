use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

use crate::config::GlobalConfig;

/// Resolve the active vault path using the context resolution priority:
/// 1. Current directory has .granite/ → use current directory
/// 2. Global config default_vault
/// 3. Error if nothing found
pub fn resolve_vault() -> Result<PathBuf> {
    // Check current directory first
    let cwd = std::env::current_dir()?;
    if cwd.join(".granite").is_dir() {
        return Ok(cwd);
    }

    // Walk up parent directories
    let mut dir = cwd.as_path();
    while let Some(parent) = dir.parent() {
        if parent.join(".granite").is_dir() {
            return Ok(parent.to_path_buf());
        }
        dir = parent;
    }

    // Check global config
    let global = GlobalConfig::load()?;
    if let Some(default_vault) = &global.default_vault {
        let path = PathBuf::from(default_vault);
        if path.join(".granite").is_dir() {
            return Ok(path);
        }
    }

    bail!(
        "No vault found. Run `granite init` to create one, or `granite context set <path>` to set a default vault."
    )
}

/// Ensure a path is a valid vault (has .granite/ directory)
pub fn is_vault(path: &Path) -> bool {
    path.join(".granite").is_dir()
}

/// Get the notes directory for a vault
pub fn notes_dir(vault_path: &Path) -> PathBuf {
    vault_path.join("notes")
}

/// Get the templates directory for a vault
pub fn templates_dir(vault_path: &Path) -> PathBuf {
    vault_path.join("templates")
}

/// Convert a title string to kebab-case filename
pub fn to_kebab_case(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("My Note Title"), "my-note-title");
        assert_eq!(to_kebab_case("hello world"), "hello-world");
        assert_eq!(to_kebab_case("  spaces  "), "spaces");
        assert_eq!(to_kebab_case("already-kebab"), "already-kebab");
        assert_eq!(to_kebab_case("CamelCase"), "camelcase");
    }
}
