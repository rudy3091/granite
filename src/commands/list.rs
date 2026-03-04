use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;

use crate::index::{Index, NoteEntry};

pub enum OutputFormat {
    Plain,
    Json,
}

pub struct ListOptions {
    pub tag: Option<String>,
    pub sort: String,
    pub tree: bool,
    pub paths: bool,
    pub format: OutputFormat,
    pub no_summary: bool,
    pub limit: Option<usize>,
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

    // Apply limit after sort
    if let Some(n) = opts.limit {
        entries.truncate(n);
    }

    if entries.is_empty() {
        match opts.format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Plain if opts.paths => {}
            OutputFormat::Plain => println!("No notes found."),
        }
        return Ok(());
    }

    match opts.format {
        OutputFormat::Json => print_json(vault_path, &entries)?,
        OutputFormat::Plain if opts.paths => print_paths(vault_path, &entries),
        OutputFormat::Plain if opts.tree => {
            print_tree(&entries);
            if !opts.no_summary {
                println!("\n{} note(s)", entries.len());
            }
        }
        OutputFormat::Plain => {
            print_plain(&entries);
            if !opts.no_summary {
                println!("\n{} note(s)", entries.len());
            }
        }
    }

    Ok(())
}

fn print_paths(vault_path: &Path, entries: &[(&String, &NoteEntry)]) {
    entries.iter().for_each(|(rel, _)| {
        println!("{}", vault_path.join(rel.as_str()).display());
    });
}

fn print_plain(entries: &[(&String, &NoteEntry)]) {
    entries.iter().for_each(|(path, entry)| {
        let tags = entry.all_tags();
        let tag_str = if tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", tags.join(", "))
        };
        println!("  {} ({}){}", entry.title(), path, tag_str);
    });
}

fn print_json(vault_path: &Path, entries: &[(&String, &NoteEntry)]) -> Result<()> {
    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|(rel, entry)| {
            serde_json::json!({
                "path": vault_path.join(rel.as_str()).to_string_lossy(),
                "rel_path": rel,
                "title": entry.title(),
                "tags": entry.all_tags(),
                "modified": entry.modified_ts,
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&items)?);
    Ok(())
}

fn print_tree(entries: &[(&String, &NoteEntry)]) {
    // Group by directory
    let mut tree: BTreeMap<String, Vec<(&String, &NoteEntry)>> = BTreeMap::new();

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
            let connector = if i == notes.len() - 1 { "└──" } else { "├──" };
            println!("  {} {}", connector, entry.title());
        }
    }
}
