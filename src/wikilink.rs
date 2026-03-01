use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub struct WikiLink {
    /// The target note (path or name, without .md)
    pub target: String,
    /// Optional display text
    pub display: Option<String>,
}

static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").unwrap());

/// Extract all wiki-links from markdown body text
pub fn extract_links(text: &str) -> Vec<WikiLink> {
    WIKILINK_RE
        .captures_iter(text)
        .map(|cap| WikiLink {
            target: cap[1].trim().to_string(),
            display: cap.get(2).map(|m| m.as_str().trim().to_string()),
        })
        .collect()
}

static INLINE_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[\s(])#([a-zA-Z][a-zA-Z0-9_-]*)").unwrap());

/// Extract all inline #tags from markdown body text
pub fn extract_inline_tags(text: &str) -> Vec<String> {
    INLINE_TAG_RE
        .captures_iter(text)
        .map(|cap| cap[1].to_string())
        .collect()
}

/// Replace all occurrences of a wiki-link target with a new target
pub fn rename_links(content: &str, old_name: &str, new_name: &str) -> String {
    let re = Regex::new(&format!(
        r"\[\[{old}(\|[^\]]+)?\]\]",
        old = regex::escape(old_name)
    ))
    .unwrap();
    re.replace_all(content, |caps: &regex::Captures| {
        if let Some(display) = caps.get(1) {
            format!("[[{new_name}{display}]]", display = display.as_str())
        } else {
            format!("[[{new_name}]]")
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_links() {
        let text = "See [[my-note]] and [[other|display text]] for details.";
        let links = extract_links(text);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "my-note");
        assert_eq!(links[0].display, None);
        assert_eq!(links[1].target, "other");
        assert_eq!(links[1].display, Some("display text".to_string()));
    }

    #[test]
    fn test_extract_inline_tags() {
        let text = "This is about #rust and #programming stuff.";
        let tags = extract_inline_tags(text);
        assert_eq!(tags, vec!["rust", "programming"]);
    }

    #[test]
    fn test_rename_links() {
        let content = "See [[old-note]] and [[old-note|display]].";
        let result = rename_links(content, "old-note", "new-note");
        assert_eq!(result, "See [[new-note]] and [[new-note|display]].");
    }
}
