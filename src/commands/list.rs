use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use crate::index::{fuzzy_match, Index, NoteEntry};

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
    pub dir: Option<String>,
    pub dir_only: bool,
    pub depth: Option<usize>,
}

pub fn run(vault_path: &Path, opts: ListOptions) -> Result<()> {
    // Reject flag combinations where one flag would be silently ignored
    match opts.format {
        OutputFormat::Json if opts.paths => {
            bail!("--paths and --format json are mutually exclusive");
        }
        OutputFormat::Json if opts.tree => {
            bail!("--tree and --format json are mutually exclusive");
        }
        OutputFormat::Json if opts.no_summary => {
            bail!("--no-summary has no effect with --format json");
        }
        OutputFormat::Plain if opts.paths && opts.tree => {
            bail!("--paths and --tree are mutually exclusive");
        }
        OutputFormat::Plain if opts.paths && opts.no_summary => {
            bail!("--no-summary has no effect with --paths");
        }
        _ => {}
    }

    // --dir-only conflicts
    if opts.dir_only && opts.paths {
        bail!("--dir-only and --paths are mutually exclusive");
    }
    if opts.dir_only && matches!(opts.format, OutputFormat::Json) {
        bail!("--dir-only and --format json are mutually exclusive");
    }
    if opts.dir_only && opts.tag.is_some() {
        bail!("--dir-only and --tag are mutually exclusive");
    }

    let index = Index::build(vault_path)?;

    // Resolve --dir to a single directory prefix via fuzzy matching.
    // Use filesystem directories (not just index directories) so that empty dirs are valid.
    let dir_prefix: Option<String> = if let Some(ref dir_query) = opts.dir {
        let available_dirs = Index::filesystem_directories(vault_path);
        let dir_query_lower = dir_query.to_lowercase();

        // Prefer exact match first to avoid false multi-matches against child dirs
        // (e.g. "projects" should not also match "projects/2026").
        let exact: Vec<String> = available_dirs
            .iter()
            .filter(|d| d.to_lowercase() == dir_query_lower)
            .cloned()
            .collect();

        if !exact.is_empty() {
            Some(exact.into_iter().next().unwrap())
        } else {
            // Fall back to fuzzy/prefix matching (excludes child-of-match dirs to avoid noise)
            let matched: Vec<String> = available_dirs
                .into_iter()
                .filter(|d| {
                    let dl = d.to_lowercase();
                    dl.starts_with(&dir_query_lower)
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
        }
    } else {
        None
    };

    // --dir-only: list subdirectories instead of notes
    if opts.dir_only {
        let base_dir = dir_prefix.as_deref().unwrap_or("");
        let base_prefix = if base_dir.is_empty() {
            "notes/".to_string()
        } else {
            format!("notes/{}/", base_dir)
        };

        // Use filesystem directories so intermediate dirs (e.g. "projects" in "projects/2026/q1")
        // appear, not just leaf dirs that contain notes.
        let mut subdirs: Vec<String> = Index::filesystem_directories(vault_path)
            .into_iter()
            .filter_map(|d| {
                let full_prefix = format!("notes/{}/", d);
                // Must be under our base and not equal to it
                if !full_prefix.starts_with(&base_prefix) || full_prefix == base_prefix {
                    return None;
                }
                // Compute the relative path from the base
                let rel = d
                    .strip_prefix(base_dir)
                    .unwrap_or(&d)
                    .trim_start_matches('/')
                    .to_string();
                if rel.is_empty() {
                    return None;
                }
                // Depth = number of '/' separators in the relative path.
                // --depth N includes dirs at depth 0..=N (consistent with note depth semantics).
                let depth = rel.chars().filter(|&c| c == '/').count();
                if let Some(max) = opts.depth {
                    if depth > max {
                        return None;
                    }
                }
                Some(rel)
            })
            .collect();

        subdirs.sort();

        if let Some(n) = opts.limit {
            subdirs.truncate(n);
        }

        if subdirs.is_empty() {
            println!("No subdirectories found.");
            return Ok(());
        }

        if opts.tree {
            print_dir_tree(&subdirs);
        } else {
            subdirs.iter().for_each(|d| println!("  {}/", d));
        }

        if !opts.no_summary {
            println!("\n{} directory(s)", subdirs.len());
        }
        return Ok(());
    }

    let mut entries: Vec<_> = index.notes.iter().collect();

    // Filter by directory prefix (supports nested: --dir projects includes projects/2026/)
    if let Some(ref dir) = dir_prefix {
        let prefix = format!("notes/{}/", dir);
        entries.retain(|(path, _)| path.starts_with(&prefix));
    }

    // Filter by depth (relative to dir_prefix base or vault root)
    if let Some(max_depth) = opts.depth {
        let base = match &dir_prefix {
            Some(d) => format!("notes/{}/", d),
            None => "notes/".to_string(),
        };
        entries.retain(|(path, _)| {
            let rel = path.strip_prefix(&base).unwrap_or(path.as_str());
            // depth = number of '/' separators (filename has none at depth 0)
            let depth = rel.chars().filter(|&c| c == '/').count();
            depth <= max_depth
        });
    }

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
            OutputFormat::Plain => match &dir_prefix {
                Some(dir) => println!("No notes found in directory '{}'.", dir),
                None => println!("No notes found."),
            },
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

fn print_dir_tree(dirs: &[String]) {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for dir in dirs {
        let parts: Vec<&str> = dir.split('/').collect();
        let mut current = String::new();
        for (depth, part) in parts.iter().enumerate() {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(part);
            if seen.insert(current.clone()) {
                let indent = "  ".repeat(depth);
                println!("{}{}/", indent, part);
            }
        }
    }
}
