use anyhow::{bail, Result};
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use pulldown_cmark::{html as cm_html, Options, Parser};
use regex::RegexBuilder;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::net::TcpListener;
use walkdir::WalkDir;

use crate::index::Index;

// ─── Shared server state ───────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    vault_path: Arc<PathBuf>,
    index: Arc<Index>,
}

// ─── Public entry points ────────────────────────────────────────────────────

/// User-facing: validate port, check for running server, spawn background daemon.
pub fn run(vault_path: &Path, port: u16) -> Result<()> {
    let pid_file = vault_path.join(".granite").join("serve.pid");

    // Check for a server that is already running.
    if pid_file.exists() {
        let pid_str = std::fs::read_to_string(&pid_file).unwrap_or_default();
        let pid: u32 = pid_str.trim().parse().unwrap_or(0);
        if pid > 0 && is_process_running(pid) {
            bail!(
                "Granite server is already running (PID {}).\nTo stop it: kill {}",
                pid,
                pid
            );
        }
        // Stale PID file — the process is gone.
        let _ = std::fs::remove_file(&pid_file);
    }

    // Probe the port before spawning. This gives a clear error immediately if
    // the port is occupied by something else (not our server).
    match std::net::TcpListener::bind(format!("127.0.0.1:{}", port)) {
        Ok(_) => {} // Listener is dropped, releasing the port for the daemon.
        Err(ref e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            bail!(
                "Port {} is already in use. Choose a different port with --port <PORT>.",
                port
            );
        }
        Err(e) => return Err(e.into()),
    }

    // Spawn the daemon: re-execute this binary with the hidden `serve-fg`
    // subcommand. The child runs the async axum server and outlives this process.
    let exe = std::env::current_exe()?;
    let child = std::process::Command::new(&exe)
        .args([
            "serve-fg",
            vault_path.to_str().unwrap_or("."),
            &port.to_string(),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    // Record the PID so the user can stop the server.
    std::fs::write(&pid_file, child.id().to_string())?;

    // Detach: drop the Child handle without waiting.  The child is reparented
    // to init/systemd on Unix when this process exits.
    std::mem::forget(child);

    println!("Granite server started at http://localhost:{}", port);
    println!("Serving vault: {}", vault_path.display());
    println!("Stop with:     kill $(cat {})", pid_file.display());

    Ok(())
}

/// Internal: async server that runs inside the spawned daemon process.
pub async fn run_daemon(vault_path: PathBuf, port: u16, index: Index) -> Result<()> {
    let state = AppState {
        vault_path: Arc::new(vault_path),
        index: Arc::new(index),
    };

    let app = Router::new()
        .route("/", get(handle_index))
        .route("/notes/*path", get(handle_note))
        .route("/tags", get(handle_tags))
        .route("/tags/:tag", get(handle_tag))
        .route("/search", get(handle_search))
        .route("/api/notes", get(handle_api_notes))
        .route("/api/notes/*path", get(handle_api_note))
        .with_state(state);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ─── Route handlers ─────────────────────────────────────────────────────────

/// GET / — file explorer: list of all notes sorted by modification time.
async fn handle_index(State(state): State<AppState>) -> Html<String> {
    let vault_name = state
        .vault_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Vault".to_string());

    let mut entries: Vec<_> = state.index.notes.values().collect();
    entries.sort_by(|a, b| b.modified_ts.cmp(&a.modified_ts));

    let mut items = String::new();
    for entry in &entries {
        let rel = entry.rel_path.strip_prefix("notes/").unwrap_or(&entry.rel_path);
        let tags_html: String = entry
            .all_tags()
            .iter()
            .map(|t| format!(r#"<a class="tag" href="/tags/{}">{}</a>"#, he(t), he(t)))
            .collect::<Vec<_>>()
            .join(" ");
        items.push_str(&format!(
            r#"<div class="note-entry"><a href="/notes/{}">{}</a><span class="tags"> {}</span></div>"#,
            he(rel),
            he(&entry.title()),
            tags_html
        ));
    }

    if items.is_empty() {
        items =
            "<p>No notes found. Create one with <code>granite new</code>.</p>".to_string();
    }

    Html(page(
        &vault_name,
        &format!(
            r#"<h1>{}</h1><p class="meta">{} note(s)</p><div class="note-list">{}</div>"#,
            he(&vault_name),
            entries.len(),
            items
        ),
    ))
}

/// GET /notes/*path — rendered markdown view of a single note.
async fn handle_note(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> impl IntoResponse {
    let safe_path = match safe_notes_path(&state.vault_path, &path) {
        Some(p) => p,
        None => {
            return (
                StatusCode::FORBIDDEN,
                Html(error_page("Access denied: invalid path")),
            )
                .into_response()
        }
    };

    let content = match std::fs::read_to_string(&safe_path) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Html(error_page("Note not found")),
            )
                .into_response()
        }
    };

    let (fm, body) = crate::frontmatter::parse(&content);
    let fm = fm.unwrap_or_default();

    let title = crate::frontmatter::get_title(&fm).unwrap_or_else(|| {
        safe_path
            .file_stem()
            .map(|s| s.to_string_lossy().replace('-', " "))
            .unwrap_or_else(|| "Untitled".to_string())
    });

    let tags = crate::frontmatter::get_tags(&fm);
    let tags_html: String = tags
        .iter()
        .map(|t| format!(r#"<a class="tag" href="/tags/{}">{}</a>"#, he(t), he(t)))
        .collect::<Vec<_>>()
        .join(" ");

    // Pre-process wiki-links, then render markdown → HTML.
    let processed = preprocess_wikilinks(body, &state.index);
    let rendered = render_markdown(&processed);

    // Compute backlinks from the in-memory index.
    let note_key = format!("notes/{}", path.trim_start_matches('/'));
    let backlinks_map = state.index.backlinks();
    let backlinks = backlinks_map.get(&note_key).cloned().unwrap_or_default();

    let backlinks_html = if backlinks.is_empty() {
        String::new()
    } else {
        let links: String = backlinks
            .iter()
            .map(|p| {
                let rel = p.strip_prefix("notes/").unwrap_or(p);
                let bl_title = state
                    .index
                    .notes
                    .get(p)
                    .map(|e| e.title())
                    .unwrap_or_else(|| rel.to_string());
                format!(
                    r#"<div class="note-entry"><a href="/notes/{}">{}</a></div>"#,
                    he(rel),
                    he(&bl_title)
                )
            })
            .collect();
        format!(
            r#"<div class="backlinks"><h2>Backlinks</h2>{}</div>"#,
            links
        )
    };

    let body_html = format!(
        r#"<h1>{}</h1>
<div class="frontmatter-meta">{}</div>
<div class="note-content">{}</div>
{}"#,
        he(&title),
        tags_html,
        rendered,
        backlinks_html
    );

    Html(page(&title, &body_html)).into_response()
}

/// GET /tags — list of all tags with note counts.
async fn handle_tags(State(state): State<AppState>) -> Html<String> {
    let tag_index = state.index.tag_index();
    let mut tags: Vec<_> = tag_index.iter().collect();
    tags.sort_by_key(|(t, _)| t.to_lowercase());

    let mut items = String::new();
    for (tag, notes) in &tags {
        items.push_str(&format!(
            r#"<div class="note-entry"><a href="/tags/{}">{}</a> <span class="meta">({} note{})</span></div>"#,
            he(tag),
            he(tag),
            notes.len(),
            if notes.len() == 1 { "" } else { "s" }
        ));
    }

    if items.is_empty() {
        items = "<p>No tags found.</p>".to_string();
    }

    Html(page(
        "Tags",
        &format!("<h1>Tags</h1><div class=\"note-list\">{}</div>", items),
    ))
}

/// GET /tags/:tag — notes that carry a given tag.
async fn handle_tag(
    State(state): State<AppState>,
    AxumPath(tag): AxumPath<String>,
) -> impl IntoResponse {
    let tag_index = state.index.tag_index();
    let notes = match tag_index.get(&tag) {
        Some(n) => n.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Html(error_page(&format!("Tag #{} not found", he(&tag)))),
            )
                .into_response()
        }
    };

    let items: String = notes
        .iter()
        .map(|p| {
            let rel = p.strip_prefix("notes/").unwrap_or(p);
            let title = state
                .index
                .notes
                .get(p)
                .map(|e| e.title())
                .unwrap_or_else(|| rel.to_string());
            format!(
                r#"<div class="note-entry"><a href="/notes/{}">{}</a></div>"#,
                he(rel),
                he(&title)
            )
        })
        .collect();

    Html(page(
        &format!("#{}", tag),
        &format!(
            "<h1>#{}</h1><div class=\"note-list\">{}</div>",
            he(&tag),
            items
        ),
    ))
    .into_response()
}

#[derive(serde::Deserialize)]
struct SearchParams {
    q: Option<String>,
}

/// GET /search?q=<query> — full-text search results page.
async fn handle_search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Html<String> {
    let query = params.q.as_deref().unwrap_or("");

    let search_form = format!(
        r#"<form class="search-form" method="get" action="/search">
  <input type="text" name="q" value="{}" placeholder="Search notes..." autofocus>
  <button type="submit">Search</button>
</form>"#,
        he(query)
    );

    if query.is_empty() {
        return Html(page(
            "Search",
            &format!("<h1>Search</h1>{}", search_form),
        ));
    }

    let results = search_notes(&state.vault_path, query);
    let count = results.len();

    let results_html: String = results
        .iter()
        .map(|(rel_path, lines)| {
            let display = rel_path.strip_prefix("notes/").unwrap_or(rel_path);
            let title = state
                .index
                .notes
                .get(rel_path.as_str())
                .map(|e| e.title())
                .unwrap_or_else(|| display.to_string());
            let lines_html: String = lines
                .iter()
                .map(|(no, line)| {
                    format!(
                        r#"<div class="result-line"><span class="meta">:{}</span> {}</div>"#,
                        no,
                        he(line)
                    )
                })
                .collect();
            format!(
                r#"<div class="search-result">
  <div class="result-file"><a href="/notes/{}">{}</a></div>
  {}
</div>"#,
                he(display),
                he(&title),
                lines_html
            )
        })
        .collect();

    let body = if results_html.is_empty() {
        format!(
            "<h1>Search</h1>{}<p>No matches found for <strong>{}</strong>.</p>",
            search_form,
            he(query)
        )
    } else {
        format!(
            "<h1>Search</h1>{}<p class=\"meta\">{} file(s) matched</p>{}",
            search_form,
            count,
            results_html
        )
    };

    Html(page("Search", &body))
}

/// GET /api/notes — JSON list of all notes with metadata.
async fn handle_api_notes(State(state): State<AppState>) -> Json<serde_json::Value> {
    let notes: Vec<_> = state
        .index
        .notes
        .values()
        .map(|e| {
            serde_json::json!({
                "path": e.rel_path,
                "title": e.title(),
                "tags": e.all_tags(),
                "modified_ts": e.modified_ts,
            })
        })
        .collect();
    Json(serde_json::json!(notes))
}

/// GET /api/notes/*path — JSON metadata + rendered HTML for a single note.
async fn handle_api_note(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> impl IntoResponse {
    let safe_path = match safe_notes_path(&state.vault_path, &path) {
        Some(p) => p,
        None => {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error": "Access denied"})),
            )
                .into_response()
        }
    };

    let content = match std::fs::read_to_string(&safe_path) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Note not found"})),
            )
                .into_response()
        }
    };

    let (fm, body) = crate::frontmatter::parse(&content);
    let fm = fm.unwrap_or_default();

    let note_key = format!("notes/{}", path.trim_start_matches('/'));
    let backlinks_map = state.index.backlinks();
    let backlinks = backlinks_map.get(&note_key).cloned().unwrap_or_default();

    let processed = preprocess_wikilinks(body, &state.index);
    let html = render_markdown(&processed);

    Json(serde_json::json!({
        "path": note_key,
        "title": crate::frontmatter::get_title(&fm),
        "tags": crate::frontmatter::get_tags(&fm),
        "html": html,
        "backlinks": backlinks,
    }))
    .into_response()
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Resolve a user-supplied path to a canonical file path inside `vault/notes/`,
/// rejecting any attempt to escape via `..`, root prefixes, or null bytes.
fn safe_notes_path(vault_path: &Path, raw: &str) -> Option<PathBuf> {
    if raw.contains('\0') {
        return None;
    }

    let rel = Path::new(raw.trim_start_matches('/'));

    // Reject any component that could escape the notes directory.
    for component in rel.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
            _ => {}
        }
    }

    let notes_dir = vault_path.join("notes");
    let full = notes_dir.join(rel);

    // Canonicalize resolves symlinks; the prefix check catches any remaining
    // edge cases (e.g. symlinks pointing outside the vault).
    let canonical = full.canonicalize().ok()?;
    let canonical_notes = notes_dir.canonicalize().ok()?;

    if canonical.starts_with(&canonical_notes) {
        Some(canonical)
    } else {
        None
    }
}

/// Check whether a process is still running by sending signal 0 (Unix only).
fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// Replace `[[target]]` / `[[target|display]]` with standard markdown links
/// so that pulldown-cmark renders them as clickable `<a>` tags.
fn preprocess_wikilinks(content: &str, index: &Index) -> String {
    use std::sync::LazyLock;
    use regex::Regex;

    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\[\[([^\]\|]+?)(?:\|([^\]]+?))?\]\]").unwrap());

    RE.replace_all(content, |caps: &regex::Captures| {
        let target = caps[1].trim();
        let display = caps.get(2).map(|m| m.as_str().trim()).unwrap_or(target);

        let href = if let Some(resolved) = index.resolve_link(target) {
            let p = resolved.strip_prefix("notes/").unwrap_or(&resolved);
            format!("/notes/{}", p)
        } else {
            format!("/notes/{}.md", crate::vault::to_kebab_case(target))
        };

        format!("[{}]({})", display, href)
    })
    .into_owned()
}

/// Render a markdown string to HTML using pulldown-cmark (GFM extensions).
fn render_markdown(content: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(content, opts);
    let mut html_out = String::new();
    cm_html::push_html(&mut html_out, parser);
    html_out
}

/// Search for lines matching `query` (case-insensitive regex) across all notes.
/// Returns a list of `(rel_path, [(line_number, line_text)])`.
fn search_notes(vault_path: &Path, query: &str) -> Vec<(String, Vec<(usize, String)>)> {
    let re = match RegexBuilder::new(query)
        .case_insensitive(true)
        .build()
    {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let notes_dir = vault_path.join("notes");
    let mut results = Vec::new();

    for entry in WalkDir::new(&notes_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let rel = path
            .strip_prefix(vault_path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let matched: Vec<(usize, String)> = content
            .lines()
            .enumerate()
            .filter_map(|(i, line)| {
                if re.is_match(line) {
                    Some((i + 1, line.to_string()))
                } else {
                    None
                }
            })
            .collect();

        if !matched.is_empty() {
            results.push((rel, matched));
        }
    }

    results
}

/// Minimal HTML escaping for user-supplied strings inserted into HTML.
fn he(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Wrap `content` in a full HTML page with navigation and shared CSS.
fn page(title: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — Granite</title>
<style>
*{{box-sizing:border-box;margin:0;padding:0}}
body{{font-family:system-ui,-apple-system,sans-serif;max-width:900px;margin:0 auto;padding:1rem 1.5rem;color:#1a1a1a;line-height:1.6}}
nav{{padding:.75rem 0;border-bottom:1px solid #e5e7eb;margin-bottom:2rem;display:flex;gap:1.5rem;align-items:center}}
nav a{{color:#374151;font-weight:500;text-decoration:none}}
nav a:first-child{{font-weight:700;font-size:1.05em}}
a{{color:#2563eb;text-decoration:none}}
a:hover{{text-decoration:underline}}
.tag{{background:#eff6ff;color:#1d4ed8;border:1px solid #bfdbfe;border-radius:12px;padding:2px 8px;font-size:.8em;white-space:nowrap;text-decoration:none}}
.tag:hover{{background:#dbeafe}}
.meta{{color:#6b7280;font-size:.875em}}
.note-list{{margin-top:1rem}}
.note-entry{{padding:.6rem 0;border-bottom:1px solid #f3f4f6;display:flex;align-items:baseline;gap:.5rem;flex-wrap:wrap}}
.note-entry a{{font-weight:500}}
.frontmatter-meta{{margin:.75rem 0 1.5rem;display:flex;gap:.5rem;flex-wrap:wrap}}
.note-content p{{margin:.75rem 0}}
.note-content h1{{font-size:1.75rem;margin:1.5rem 0 .5rem}}
.note-content h2{{font-size:1.375rem;margin:1.5rem 0 .5rem;padding-bottom:.25rem;border-bottom:1px solid #e5e7eb}}
.note-content h3{{font-size:1.125rem;margin:1.25rem 0 .4rem}}
.note-content ul,.note-content ol{{padding-left:1.75rem;margin:.75rem 0}}
.note-content li{{margin:.25rem 0}}
.note-content blockquote{{border-left:4px solid #d1d5db;padding-left:1rem;color:#6b7280;margin:1rem 0}}
.note-content pre{{background:#f9fafb;border:1px solid #e5e7eb;padding:1rem;overflow-x:auto;border-radius:6px;margin:1rem 0}}
.note-content code{{background:#f3f4f6;padding:2px 5px;border-radius:4px;font-size:.875em;font-family:Menlo,Consolas,monospace}}
.note-content pre code{{background:none;padding:0}}
.note-content table{{border-collapse:collapse;width:100%;margin:1rem 0}}
.note-content th,.note-content td{{border:1px solid #e5e7eb;padding:.5rem .75rem;text-align:left}}
.note-content th{{background:#f9fafb;font-weight:600}}
h1{{font-size:1.75rem;margin-bottom:.5rem}}
.backlinks{{margin-top:3rem;padding-top:1rem;border-top:2px solid #e5e7eb}}
.backlinks h2{{margin-bottom:.75rem}}
.search-form{{display:flex;gap:.5rem;margin-bottom:1.5rem}}
.search-form input{{flex:1;padding:.5rem .75rem;border:1px solid #d1d5db;border-radius:6px;font-size:1rem}}
.search-form button{{padding:.5rem 1rem;background:#2563eb;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:1rem}}
.search-form button:hover{{background:#1d4ed8}}
.search-result{{padding:.75rem 0;border-bottom:1px solid #f3f4f6}}
.result-file{{font-weight:600;margin-bottom:.25rem}}
.result-line{{font-family:Menlo,Consolas,monospace;font-size:.8em;background:#fefce8;padding:2px 6px;border-radius:3px;margin:.2rem 0}}
@media(max-width:640px){{body{{padding:.75rem}}h1{{font-size:1.375rem}}}}
</style>
</head>
<body>
<nav>
  <a href="/">&#128210; Granite</a>
  <a href="/tags">Tags</a>
  <a href="/search">Search</a>
</nav>
{content}
</body>
</html>"#,
        title = he(title),
        content = content,
    )
}

fn error_page(msg: &str) -> String {
    page(
        "Error",
        &format!(
            r#"<h1>Error</h1><p>{}</p><p><a href="/">Back to home</a></p>"#,
            he(msg)
        ),
    )
}
