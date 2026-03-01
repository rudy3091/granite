use anyhow::Result;
use std::path::Path;

use crate::index::Index;

pub fn run(vault_path: &Path, notes_for_tag: Option<&str>) -> Result<()> {
    let index = Index::build(vault_path)?;
    let tag_index = index.tag_index();

    if let Some(tag) = notes_for_tag {
        // List notes with a specific tag
        match tag_index.get(tag) {
            Some(paths) => {
                println!("Notes tagged #{}:", tag);
                for path in paths {
                    let entry = &index.notes[path];
                    println!("  {} ({})", entry.title(), path);
                }
                println!("\n{} note(s)", paths.len());
            }
            None => {
                println!("No notes found with tag #{}", tag);
            }
        }
    } else {
        // List all tags with counts
        if tag_index.is_empty() {
            println!("No tags found.");
            return Ok(());
        }

        let mut tags: Vec<_> = tag_index.iter().collect();
        tags.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));

        println!("Tags:");
        for (tag, paths) in &tags {
            println!("  #{} ({})", tag, paths.len());
        }
        println!("\n{} tag(s)", tags.len());
    }

    Ok(())
}
