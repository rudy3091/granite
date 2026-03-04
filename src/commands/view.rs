use anyhow::{bail, Result};
use std::io::Write;
use std::path::Path;

use crate::frontmatter;
use crate::index::{fuzzy_match, Index};

pub struct ViewOptions {
    pub no_frontmatter: bool,
    pub dir: Option<String>,
}

pub fn run(vault_path: &Path, query: &str, opts: ViewOptions) -> Result<()> {
    let index = Index::build(vault_path)?;

    // Resolve --dir to a single target directory via fuzzy matching
    let dir_prefix: Option<String> = if let Some(ref dir_query) = opts.dir {
        let available_dirs = index.directories();
        let dir_query_lower = dir_query.to_lowercase();

        let matched: Vec<String> = available_dirs
            .into_iter()
            .filter(|d| {
                let dl = d.to_lowercase();
                dl == dir_query_lower
                    || dl.starts_with(&dir_query_lower)
                    || dl.contains(&dir_query_lower)
                    || fuzzy_match(&dir_query_lower, &dl)
            })
            .collect();

        match matched.len() {
            0 => bail!("No directories matching '{}'", dir_query),
            1 => Some(matched.into_iter().next().unwrap()),
            _ => {
                println!("Multiple directories matching '{}':", dir_query);
                for (i, d) in matched.iter().enumerate() {
                    println!("  [{}] {}", i + 1, d);
                }
                print!("Select [1-{}]: ", matched.len());
                std::io::stdout().flush()?;
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                let choice: usize = input.trim().parse().unwrap_or(0);
                if choice < 1 || choice > matched.len() {
                    bail!("Invalid selection");
                }
                Some(matched.into_iter().nth(choice - 1).unwrap())
            }
        }
    } else {
        None
    };

    // Search notes, filtered to the selected directory when --dir is given
    let matches = {
        let all = index.fuzzy_search(query);
        if let Some(ref dir) = dir_prefix {
            let prefix = format!("notes/{}/", dir);
            all.into_iter()
                .filter(|(path, _)| path.starts_with(&prefix))
                .collect::<Vec<_>>()
        } else {
            all
        }
    };

    if matches.is_empty() {
        match &dir_prefix {
            Some(dir) => bail!("No notes matching '{}' in directory '{}'", query, dir),
            None => bail!("No notes matching '{}'", query),
        }
    }

    let (rel_path, _entry) = if matches.len() == 1 {
        matches.into_iter().next().unwrap()
    } else {
        println!("Multiple matches for '{}':", query);
        for (i, (path, entry)) in matches.iter().enumerate() {
            println!("  [{}] {} ({})", i + 1, entry.title(), path);
        }
        print!("Select [1-{}]: ", matches.len());
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

