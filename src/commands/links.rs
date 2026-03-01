use anyhow::{bail, Result};
use std::path::Path;

use crate::index::Index;

pub struct LinksOptions {
    pub backlinks_only: bool,
    pub forward_only: bool,
    pub orphans: bool,
}

pub fn run(vault_path: &Path, note_query: Option<&str>, opts: LinksOptions) -> Result<()> {
    let index = Index::build(vault_path)?;

    if opts.orphans {
        let orphans = index.orphans();
        if orphans.is_empty() {
            println!("No orphan notes found.");
        } else {
            println!("Orphan notes (no incoming or outgoing links):");
            for path in &orphans {
                let entry = &index.notes[path];
                println!("  {} ({})", entry.title(), path);
            }
            println!("\n{} orphan(s)", orphans.len());
        }
        return Ok(());
    }

    let query = note_query.unwrap_or("");
    if query.is_empty() {
        bail!("Please specify a note name, or use --orphans to list orphan notes");
    }

    // Find the note
    let matches = index.fuzzy_search(query);
    if matches.is_empty() {
        bail!("No notes matching '{}'", query);
    }
    let (rel_path, entry) = &matches[0];

    println!("Links for: {} ({})\n", entry.title(), rel_path);

    let all_backlinks = index.backlinks();

    if !opts.backlinks_only {
        println!("Forward links:");
        if entry.forward_links.is_empty() {
            println!("  (none)");
        } else {
            for target in &entry.forward_links {
                let resolved = index.resolve_link(target);
                match resolved {
                    Some(path) => {
                        let target_entry = &index.notes[&path];
                        println!("  → {} ({})", target_entry.title(), path);
                    }
                    None => {
                        println!("  → [[{}]] (unresolved)", target);
                    }
                }
            }
        }
        println!();
    }

    if !opts.forward_only {
        println!("Backlinks:");
        let bls = all_backlinks.get(rel_path.as_str());
        match bls {
            Some(sources) if !sources.is_empty() => {
                for source_path in sources {
                    let source_entry = &index.notes[source_path];
                    println!("  ← {} ({})", source_entry.title(), source_path);
                }
            }
            _ => {
                println!("  (none)");
            }
        }
    }

    Ok(())
}
