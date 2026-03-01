use anyhow::{bail, Result};
use std::path::Path;

use crate::config::VaultConfig;
use crate::frontmatter;
use crate::index::Index;

pub struct EditOptions {
    pub append: bool,
}

pub fn run(vault_path: &Path, query: &str, opts: EditOptions, stdin_content: Option<String>) -> Result<()> {
    if opts.append && stdin_content.is_none() {
        bail!("--append requires piped stdin content");
    }

    let config = VaultConfig::load(vault_path)?;
    let index = Index::build(vault_path)?;

    let matches = index.fuzzy_search(query);

    if matches.is_empty() {
        bail!("No notes matching '{}'", query);
    }

    let (rel_path, _entry) = if matches.len() == 1 {
        matches.into_iter().next().unwrap()
    } else if opts.append {
        bail!(
            "Ambiguous query '{}': {} matches. Use a more specific query.",
            query,
            matches.len()
        );
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

    if opts.append {
        let body_in = stdin_content.unwrap();
        let existing = std::fs::read_to_string(&note_path)?;
        let (fm_opt, old_body) = frontmatter::parse(&existing);
        let new_body = format!("{}\n{}", old_body.trim_end(), body_in);
        let new_content = match fm_opt {
            Some(mut fm) => {
                frontmatter::set_modified(&mut fm);
                frontmatter::serialize(&fm, &new_body)
            }
            None => new_body,
        };
        std::fs::write(&note_path, new_content)?;
        println!("Updated {}", note_path.display());
    } else {
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
    }

    Ok(())
}
