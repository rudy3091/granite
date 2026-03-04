use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;
use walkdir::WalkDir;

use crate::frontmatter;
use crate::wikilink;

/// A single note's indexed data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteEntry {
    /// Relative path from vault root (e.g. "notes/my-note.md")
    pub rel_path: String,
    /// Last modified timestamp (seconds since epoch)
    pub modified_ts: u64,
    /// Parsed frontmatter fields
    pub frontmatter: HashMap<String, Value>,
    /// Forward wiki-links found in body
    pub forward_links: Vec<String>,
    /// Inline tags found in body
    pub inline_tags: Vec<String>,
}

impl NoteEntry {
    /// Get the note's title: from frontmatter, or derived from filename
    pub fn title(&self) -> String {
        frontmatter::get_title(&self.frontmatter)
            .unwrap_or_else(|| self.stem().replace('-', " "))
    }

    /// Get the filename stem (without extension and directory)
    pub fn stem(&self) -> String {
        Path::new(&self.rel_path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }

    /// Get all tags (frontmatter + inline)
    pub fn all_tags(&self) -> Vec<String> {
        let mut tags = frontmatter::get_tags(&self.frontmatter);
        for t in &self.inline_tags {
            if !tags.contains(t) {
                tags.push(t.clone());
            }
        }
        tags
    }

    /// Get aliases from frontmatter
    pub fn aliases(&self) -> Vec<String> {
        frontmatter::get_aliases(&self.frontmatter)
    }
}

/// The in-memory index of all notes in a vault
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Index {
    pub notes: HashMap<String, NoteEntry>,
}

impl Index {
    /// Build the index by scanning the vault's notes/ directory.
    /// If a cache exists and entries are up-to-date, reuse them.
    pub fn build(vault_path: &Path) -> Result<Self> {
        let cache = Self::load_cache(vault_path);
        let notes_dir = vault_path.join("notes");
        let mut notes = HashMap::new();

        if !notes_dir.exists() {
            return Ok(Self { notes });
        }

        for entry in WalkDir::new(&notes_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }

            let rel_path = path
                .strip_prefix(vault_path)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            let modified_ts = path
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            // Check cache: if timestamp matches, reuse cached entry
            if let Some(ref cached) = cache {
                if let Some(cached_entry) = cached.notes.get(&rel_path) {
                    if cached_entry.modified_ts == modified_ts {
                        notes.insert(rel_path, cached_entry.clone());
                        continue;
                    }
                }
            }

            // Parse the file
            match Self::parse_note(path, &rel_path, modified_ts) {
                Ok(entry) => {
                    notes.insert(rel_path, entry);
                }
                Err(e) => {
                    eprintln!("Warning: failed to parse {}: {}", rel_path, e);
                }
            }
        }

        let index = Self { notes };
        index.save_cache(vault_path)?;
        Ok(index)
    }

    fn parse_note(path: &Path, rel_path: &str, modified_ts: u64) -> Result<NoteEntry> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", rel_path))?;

        let (fm, body) = frontmatter::parse(&content);
        let frontmatter = fm.unwrap_or_default();
        let forward_links = wikilink::extract_links(body)
            .into_iter()
            .map(|l| l.target)
            .collect();
        let inline_tags = wikilink::extract_inline_tags(body);

        Ok(NoteEntry {
            rel_path: rel_path.to_string(),
            modified_ts,
            frontmatter,
            forward_links,
            inline_tags,
        })
    }

    /// Compute backlinks: for each note, which other notes link to it
    pub fn backlinks(&self) -> HashMap<String, Vec<String>> {
        let mut bl: HashMap<String, Vec<String>> = HashMap::new();
        for (source_path, entry) in &self.notes {
            for target in &entry.forward_links {
                if let Some(target_path) = self.resolve_link(target) {
                    bl.entry(target_path)
                        .or_default()
                        .push(source_path.clone());
                }
            }
        }
        bl
    }

    /// Build a tag index: tag → list of note paths
    pub fn tag_index(&self) -> HashMap<String, Vec<String>> {
        let mut ti: HashMap<String, Vec<String>> = HashMap::new();
        for (path, entry) in &self.notes {
            for tag in entry.all_tags() {
                ti.entry(tag).or_default().push(path.clone());
            }
        }
        ti
    }

    /// Resolve a wiki-link target to a note's rel_path.
    /// Resolution order: exact filename → title → aliases
    pub fn resolve_link(&self, target: &str) -> Option<String> {
        let target_lower = target.to_lowercase();

        // 1. Exact filename match (with or without notes/ prefix)
        for (path, _entry) in &self.notes {
            let stem = Path::new(path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if stem == target_lower {
                return Some(path.clone());
            }
            // Also try matching with subdirectory path
            let rel_no_ext = path.strip_suffix(".md").unwrap_or(path);
            let rel_no_prefix = rel_no_ext.strip_prefix("notes/").unwrap_or(rel_no_ext);
            if rel_no_prefix.to_lowercase() == target_lower {
                return Some(path.clone());
            }
        }

        // 2. Frontmatter title match (case-insensitive)
        for (path, entry) in &self.notes {
            if let Some(title) = frontmatter::get_title(&entry.frontmatter) {
                if title.to_lowercase() == target_lower {
                    return Some(path.clone());
                }
            }
        }

        // 3. Aliases match (case-insensitive)
        for (path, entry) in &self.notes {
            for alias in entry.aliases() {
                if alias.to_lowercase() == target_lower {
                    return Some(path.clone());
                }
            }
        }

        None
    }

    /// Find notes matching a fuzzy query against titles and filenames
    pub fn fuzzy_search(&self, query: &str) -> Vec<(String, NoteEntry)> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<(String, NoteEntry, i64)> = Vec::new();

        for (path, entry) in &self.notes {
            let title = entry.title().to_lowercase();
            let stem = entry.stem().to_lowercase();

            let score = if title == query_lower || stem == query_lower {
                100 // Exact match
            } else if title.starts_with(&query_lower) || stem.starts_with(&query_lower) {
                80 // Prefix match
            } else if title.contains(&query_lower) || stem.contains(&query_lower) {
                60 // Contains match
            } else {
                // Simple fuzzy: check if all query chars appear in order
                if fuzzy_match(&query_lower, &title) || fuzzy_match(&query_lower, &stem) {
                    40
                } else {
                    continue;
                }
            };

            results.push((path.clone(), entry.clone(), score));
        }

        results.sort_by(|a, b| b.2.cmp(&a.2));
        results.into_iter().map(|(p, e, _)| (p, e)).collect()
    }

    /// Load the index cache from .granite/index.json
    fn load_cache(vault_path: &Path) -> Option<Self> {
        let cache_path = vault_path.join(".granite").join("index.json");
        if !cache_path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&cache_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save the index cache to .granite/index.json
    fn save_cache(&self, vault_path: &Path) -> Result<()> {
        let cache_path = vault_path.join(".granite").join("index.json");
        let content = serde_json::to_string(self)?;
        std::fs::write(&cache_path, content)?;
        Ok(())
    }

    /// Return all unique subdirectories under notes/ (relative to notes/).
    /// E.g. a note at "notes/inbox/foo.md" contributes "inbox".
    /// Notes directly under notes/ contribute nothing (root level).
    pub fn directories(&self) -> Vec<String> {
        let mut dirs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for path in self.notes.keys() {
            if let Some(parent) = Path::new(path).parent() {
                let rel = parent.strip_prefix("notes").unwrap_or(parent);
                let s = rel
                    .to_string_lossy()
                    .trim_start_matches('/')
                    .to_string();
                if !s.is_empty() {
                    dirs.insert(s);
                }
            }
        }
        dirs.into_iter().collect()
    }

    /// Return all subdirectories that physically exist under notes/, regardless of whether
    /// they contain any indexed notes. Useful for --dir resolution when directories may be empty.
    pub fn filesystem_directories(vault_path: &Path) -> Vec<String> {
        let notes_dir = vault_path.join("notes");
        let mut dirs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        if !notes_dir.exists() {
            return vec![];
        }
        for entry in WalkDir::new(&notes_dir)
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() {
                if let Ok(rel) = entry.path().strip_prefix(&notes_dir) {
                    let s = rel.to_string_lossy().to_string();
                    if !s.is_empty() {
                        dirs.insert(s);
                    }
                }
            }
        }
        dirs.into_iter().collect()
    }

    /// Find orphan notes (no incoming or outgoing links)
    pub fn orphans(&self) -> Vec<String> {
        let bl = self.backlinks();
        self.notes
            .iter()
            .filter(|(path, entry)| {
                entry.forward_links.is_empty() && !bl.contains_key(path.as_str())
            })
            .map(|(path, _)| path.clone())
            .collect()
    }
}

/// Simple fuzzy matching: check if all chars of needle appear in order in haystack
pub fn fuzzy_match(needle: &str, haystack: &str) -> bool {
    let mut hay_chars = haystack.chars();
    for nc in needle.chars() {
        loop {
            match hay_chars.next() {
                Some(hc) if hc == nc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match() {
        assert!(fuzzy_match("mn", "my-note"));
        assert!(fuzzy_match("note", "my-note"));
        assert!(!fuzzy_match("xyz", "my-note"));
    }

    fn make_index(paths: &[&str]) -> Index {
        let notes = paths
            .iter()
            .map(|&p| {
                (
                    p.to_string(),
                    NoteEntry {
                        rel_path: p.to_string(),
                        modified_ts: 0,
                        frontmatter: HashMap::new(),
                        forward_links: vec![],
                        inline_tags: vec![],
                    },
                )
            })
            .collect();
        Index { notes }
    }

    #[test]
    fn test_directories_collects_unique_subdirs() {
        let index = make_index(&[
            "notes/inbox/foo.md",
            "notes/inbox/bar.md",
            "notes/daily/2026-01-01.md",
            "notes/top-level.md",
        ]);
        let mut dirs = index.directories();
        dirs.sort();
        assert_eq!(dirs, vec!["daily", "inbox"]);
    }

    #[test]
    fn test_directories_nested() {
        let index = make_index(&[
            "notes/projects/2026/my-note.md",
            "notes/projects/2026/other.md",
            "notes/inbox/task.md",
        ]);
        let mut dirs = index.directories();
        dirs.sort();
        assert_eq!(dirs, vec!["inbox", "projects/2026"]);
    }

    #[test]
    fn test_directories_empty_when_all_root() {
        let index = make_index(&["notes/foo.md", "notes/bar.md"]);
        assert!(index.directories().is_empty());
    }
}
