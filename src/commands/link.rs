use anyhow::{bail, Result};
use std::path::Path;

use crate::frontmatter;
use crate::index::Index;
use crate::wikilink;

pub struct LinkOptions {
    pub content: Option<String>,
}

pub fn run(vault_path: &Path, target_query: &str, dest_query: &str, opts: LinkOptions) -> Result<()> {
    let index = Index::build(vault_path)?;

    // Resolve target — take top fuzzy match (consistent with rename.rs)
    let target_matches = index.fuzzy_search(target_query);
    if target_matches.is_empty() {
        bail!("No notes matching '{}'", target_query);
    }
    let (target_rel_path, _) = target_matches.into_iter().next().unwrap();

    // Resolve destination — must be unambiguous
    let dest_matches = index.fuzzy_search(dest_query);
    match dest_matches.len() {
        0 => bail!("No notes matching '{}'", dest_query),
        1 => {}
        n => bail!(
            "Ambiguous destination '{}': {} matches. Use a more specific query.",
            dest_query,
            n
        ),
    }
    let (_, dest_entry) = dest_matches.into_iter().next().unwrap();
    let dest_stem = dest_entry.stem();

    // Read target and detect duplicates
    let target_abs = vault_path.join(&target_rel_path);
    let existing = std::fs::read_to_string(&target_abs)?;
    let already_linked = wikilink::extract_links(&existing)
        .iter()
        .any(|l| l.target == dest_stem);
    if already_linked {
        eprintln!(
            "Warning: [[{}]] is already linked in {}. Skipping.",
            dest_stem, target_rel_path
        );
        return Ok(());
    }

    // Build text to append
    let append_text = opts.content.map_or_else(
        || format!("\n[[{}]]", dest_stem),
        |ctx| format!("\n{}\n[[{}]]", ctx, dest_stem),
    );

    // Parse frontmatter, update modified, rewrite
    let (fm_opt, body) = frontmatter::parse(&existing);
    let new_body = format!("{}{}", body.trim_end(), append_text);
    let new_content = match fm_opt {
        Some(mut fm) => {
            frontmatter::set_modified(&mut fm);
            frontmatter::serialize(&fm, &new_body)
        }
        None => new_body,
    };

    std::fs::write(&target_abs, new_content)?;
    println!("Linked [[{}]] into {}", dest_stem, target_rel_path);
    Ok(())
}
