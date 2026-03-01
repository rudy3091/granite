use anyhow::{bail, Result};
use std::path::Path;

use crate::frontmatter;
use crate::index::Index;

pub struct ViewOptions {
    pub no_frontmatter: bool,
}

pub fn run(vault_path: &Path, query: &str, opts: ViewOptions) -> Result<()> {
    let index = Index::build(vault_path)?;
    let matches = index.fuzzy_search(query);

    if matches.is_empty() {
        bail!("No notes matching '{}'", query);
    }

    let (rel_path, _entry) = if matches.len() == 1 {
        matches.into_iter().next().unwrap()
    } else {
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
    let content = std::fs::read_to_string(&note_path)?;

    if opts.no_frontmatter {
        let (_fm, body) = frontmatter::parse(&content);
        print!("{}", body);
    } else {
        print!("{}", content);
    }

    Ok(())
}
