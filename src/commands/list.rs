use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;

use crate::index::Index;

pub struct ListOptions {
    pub tag: Option<String>,
    pub sort: String,
    pub tree: bool,
}

pub fn run(vault_path: &Path, opts: ListOptions) -> Result<()> {
    let index = Index::build(vault_path)?;

    let mut entries: Vec<_> = index.notes.iter().collect();

    // Filter by tag
    if let Some(ref tag) = opts.tag {
        entries.retain(|(_, entry)| entry.all_tags().iter().any(|t| t == tag));
    }

    // Sort
    match opts.sort.as_str() {
        "title" => entries.sort_by(|a, b| a.1.title().cmp(&b.1.title())),
        "created" => entries.sort_by(|a, b| {
            let a_created = a.1.frontmatter.get("created").and_then(|v| v.as_str());
            let b_created = b.1.frontmatter.get("created").and_then(|v| v.as_str());
            b_created.cmp(&a_created)
        }),
        _ => {
            // Default: sort by modified timestamp (newest first)
            entries.sort_by(|a, b| b.1.modified_ts.cmp(&a.1.modified_ts));
        }
    }

    if entries.is_empty() {
        println!("No notes found.");
        return Ok(());
    }

    if opts.tree {
        print_tree(&entries);
    } else {
        for (path, entry) in &entries {
            let tags = entry.all_tags();
            let tag_str = if tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", tags.join(", "))
            };
            println!("  {} ({}){}", entry.title(), path, tag_str);
        }
    }

    println!("\n{} note(s)", entries.len());
    Ok(())
}

fn print_tree(entries: &[(&String, &crate::index::NoteEntry)]) {
    // Group by directory
    let mut tree: BTreeMap<String, Vec<(&String, &crate::index::NoteEntry)>> = BTreeMap::new();

    for (path, entry) in entries {
        let dir = Path::new(path.as_str())
            .parent()
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .to_string();
        tree.entry(dir).or_default().push((path, entry));
    }

    for (dir, notes) in &tree {
        let display_dir = if dir.is_empty() { "." } else { dir };
        println!("{}/", display_dir);
        for (i, (_path, entry)) in notes.iter().enumerate() {
            let connector = if i == notes.len() - 1 {
                "└──"
            } else {
                "├──"
            };
            println!("  {} {}", connector, entry.title());
        }
    }
}
