use anyhow::{bail, Result};
use std::path::Path;

use crate::index::Index;
use crate::vault;
use crate::wikilink;

pub fn run(vault_path: &Path, old_query: &str, new_name: &str) -> Result<()> {
    let index = Index::build(vault_path)?;

    // Find the note to rename
    let matches = index.fuzzy_search(old_query);
    if matches.is_empty() {
        bail!("No notes matching '{}'", old_query);
    }
    let (old_rel_path, old_entry) = &matches[0];
    let old_stem = old_entry.stem();
    let old_abs_path = vault_path.join(old_rel_path);

    // Compute new path
    let new_filename = format!("{}.md", vault::to_kebab_case(new_name));
    let new_abs_path = old_abs_path.parent().unwrap().join(&new_filename);

    if new_abs_path.exists() {
        bail!("Target already exists: {}", new_abs_path.display());
    }

    // Rename the file
    std::fs::rename(&old_abs_path, &new_abs_path)?;
    let new_stem = vault::to_kebab_case(new_name);

    println!(
        "Renamed: {} → {}",
        old_abs_path.display(),
        new_abs_path.display()
    );

    // Update frontmatter title in renamed file
    let content = std::fs::read_to_string(&new_abs_path)?;
    let (fm, body) = crate::frontmatter::parse(&content);
    if let Some(mut fm) = fm {
        fm.insert(
            "title".to_string(),
            serde_yaml::Value::String(new_name.to_string()),
        );
        crate::frontmatter::set_modified(&mut fm);
        let new_content = crate::frontmatter::serialize(&fm, body);
        std::fs::write(&new_abs_path, new_content)?;
    }

    // Update all wiki-links in other notes that reference the old name
    let mut updated_count = 0;
    for (path, _entry) in &index.notes {
        if path == old_rel_path {
            continue;
        }
        let abs_path = vault_path.join(path);
        let content = std::fs::read_to_string(&abs_path)?;
        let new_content = wikilink::rename_links(&content, &old_stem, &new_stem);
        if new_content != content {
            std::fs::write(&abs_path, &new_content)?;
            updated_count += 1;
            println!("  Updated links in: {}", path);
        }
    }

    if updated_count > 0 {
        println!("Updated links in {} file(s)", updated_count);
    } else {
        println!("No other files contained links to [[{}]]", old_stem);
    }

    Ok(())
}
