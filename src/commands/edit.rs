use anyhow::{bail, Result};
use std::path::Path;

use crate::config::VaultConfig;
use crate::frontmatter;
use crate::index::Index;

pub fn run(vault_path: &Path, query: &str) -> Result<()> {
    let config = VaultConfig::load(vault_path)?;
    let index = Index::build(vault_path)?;

    let matches = index.fuzzy_search(query);

    if matches.is_empty() {
        bail!("No notes matching '{}'", query);
    }

    let (rel_path, _entry) = if matches.len() == 1 {
        matches.into_iter().next().unwrap()
    } else {
        // Show interactive picker
        println!("Multiple matches for '{}':", query);
        for (i, (path, entry)) in matches.iter().enumerate() {
            println!("  [{}] {} ({})", i + 1, entry.title(), path);
        }

        print!("Select [1-{}]: ", matches.len());
        use std::io::Write;
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let choice: usize = input.trim().parse().unwrap_or(0);

        if choice < 1 || choice > matches.len() {
            bail!("Invalid selection");
        }

        matches.into_iter().nth(choice - 1).unwrap()
    };

    let note_path = vault_path.join(&rel_path);

    // Open in editor
    let status = std::process::Command::new(&config.defaults.editor)
        .arg(&note_path)
        .status()?;

    if !status.success() {
        bail!("Editor exited with non-zero status");
    }

    // Update modified timestamp
    let content = std::fs::read_to_string(&note_path)?;
    let updated = frontmatter::update_modified_in_content(&content)?;
    if updated != content {
        std::fs::write(&note_path, updated)?;
    }

    Ok(())
}
