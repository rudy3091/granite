use anyhow::Result;
use serde_yaml::Value;
use std::collections::HashMap;

/// Parse a markdown file's content into frontmatter and body.
/// Frontmatter is delimited by `---` lines at the start of the file.
pub fn parse(content: &str) -> (Option<HashMap<String, Value>>, &str) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content);
    }

    // Find the closing ---
    let after_first = &trimmed[3..];
    let after_first = after_first.strip_prefix('\n').unwrap_or(after_first);

    if let Some(end_pos) = after_first.find("\n---") {
        let yaml_str = &after_first[..end_pos];
        let body_start = end_pos + 4; // "\n---".len()
        let body = &after_first[body_start..];
        let body = body.strip_prefix('\n').unwrap_or(body);

        match serde_yaml::from_str::<HashMap<String, Value>>(yaml_str) {
            Ok(fm) => (Some(fm), body),
            Err(_) => (None, content),
        }
    } else {
        (None, content)
    }
}

/// Serialize frontmatter and body back into a markdown string
pub fn serialize(frontmatter: &HashMap<String, Value>, body: &str) -> String {
    let yaml = serde_yaml::to_string(frontmatter).unwrap_or_default();
    format!("---\n{}---\n\n{}", yaml, body)
}

/// Get the title from frontmatter, or None
pub fn get_title(fm: &HashMap<String, Value>) -> Option<String> {
    fm.get("title").and_then(|v| v.as_str()).map(String::from)
}

/// Get tags from frontmatter
pub fn get_tags(fm: &HashMap<String, Value>) -> Vec<String> {
    fm.get("tags")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Get aliases from frontmatter
pub fn get_aliases(fm: &HashMap<String, Value>) -> Vec<String> {
    fm.get("aliases")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Update the modified timestamp in frontmatter
pub fn set_modified(fm: &mut HashMap<String, Value>) {
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    fm.insert("modified".to_string(), Value::String(now));
}

/// Build initial frontmatter for a new note
pub fn new_frontmatter(title: &str) -> HashMap<String, Value> {
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let mut fm = HashMap::new();
    fm.insert("title".to_string(), Value::String(title.to_string()));
    fm.insert("created".to_string(), Value::String(now.clone()));
    fm.insert("modified".to_string(), Value::String(now));
    fm
}

/// Update frontmatter in an existing file content, returning the new content
pub fn update_modified_in_content(content: &str) -> Result<String> {
    let (fm, body) = parse(content);
    match fm {
        Some(mut fm) => {
            set_modified(&mut fm);
            Ok(serialize(&fm, body))
        }
        None => Ok(content.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_with_frontmatter() {
        let content = "---\ntitle: Test\ntags:\n  - rust\n---\n\n# Body\n";
        let (fm, body) = parse(content);
        assert!(fm.is_some());
        let fm = fm.unwrap();
        assert_eq!(get_title(&fm), Some("Test".to_string()));
        assert_eq!(get_tags(&fm), vec!["rust".to_string()]);
        assert!(body.contains("# Body"));
    }

    #[test]
    fn test_parse_without_frontmatter() {
        let content = "# Just a heading\n\nSome text.";
        let (fm, body) = parse(content);
        assert!(fm.is_none());
        assert_eq!(body, content);
    }
}
