use anyhow::{bail, Result};
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use rust_embed::RustEmbed;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;

use granite_core::index::Index;

/// The minimal HTML shell served at `/`. Kept in `granite-cli` (not built by
/// vite) so the binary embeds it directly; `web/index.html` mirrors its
/// structure for `pnpm dev`.
const INDEX_HTML: &str = include_str!("../../web-assets/index.html");

/// The `web/` frontend bundle (vite + pnpm), built by `build.rs` and embedded
/// at compile time — end users never need Node.js/pnpm at runtime.
#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct EditorAssets;

// ─── Shared server state ───────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    vault_path: Arc<PathBuf>,
    /// Rebuilt and swapped in whole after a save, so titles/tags/backlinks
    /// stay current without a more granular incremental-update mechanism.
    index: Arc<RwLock<Arc<Index>>>,
}

// ─── Public entry points ────────────────────────────────────────────────────

/// User-facing: stop the currently running daemon by sending SIGTERM.
pub fn kill(vault_path: &Path) -> Result<()> {
    let pid_file = vault_path.join(".granite").join("serve.pid");

    if !pid_file.exists() {
        bail!(
            "No running Granite server found (no PID file at {}).",
            pid_file.display()
        );
    }

    let pid_str = std::fs::read_to_string(&pid_file).unwrap_or_default();
    let pid: u32 = pid_str.trim().parse().unwrap_or(0);

    if pid == 0 {
        let _ = std::fs::remove_file(&pid_file);
        bail!("PID file is corrupt. Removed it.");
    }

    if !is_process_running(pid) {
        let _ = std::fs::remove_file(&pid_file);
        bail!(
            "Server process (PID {}) is not running. Removed stale PID file.",
            pid
        );
    }

    #[cfg(unix)]
    {
        let status = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status()?;
        if !status.success() {
            bail!("Failed to stop server process (PID {}).", pid);
        }
    }
    #[cfg(not(unix))]
    {
        bail!("granite serve kill is only supported on Unix systems.");
    }

    let _ = std::fs::remove_file(&pid_file);
    println!("Granite server (PID {}) stopped.", pid);
    Ok(())
}

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
        index: Arc::new(RwLock::new(Arc::new(index))),
    };

    let app = Router::new()
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route("/api/notes", get(handle_api_notes))
        .route(
            "/api/notes/*path",
            get(handle_api_note).put(handle_save_note),
        )
        .route("/web/main.js", get(handle_editor_js))
        .route("/web/style.css", get(handle_editor_css))
        .with_state(state);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ─── Route handlers ─────────────────────────────────────────────────────────

/// Snapshot the current index. Cheap: clones an `Arc`, not the index itself.
fn snapshot(state: &AppState) -> Arc<Index> {
    state.index.read().unwrap().clone()
}

/// Rebuild the index from disk and swap it in, so notes added or changed
/// outside this server (e.g. `granite new` in another terminal, or a file
/// dropped straight into `notes/`) are picked up. Falls back to the current
/// snapshot if the rebuild fails (e.g. transient I/O error).
fn refresh_index(state: &AppState) -> Arc<Index> {
    match Index::build(&state.vault_path) {
        Ok(new_index) => {
            let new_index = Arc::new(new_index);
            if let Ok(mut guard) = state.index.write() {
                *guard = new_index.clone();
            }
            new_index
        }
        Err(_) => snapshot(state),
    }
}

/// PUT /api/notes/*path — save edited note content, updating `modified`.
async fn handle_save_note(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
    body: String,
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

    let updated = match granite_core::frontmatter::update_modified_in_content(&body) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    if let Err(e) = std::fs::write(&safe_path, &updated) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    // Rebuild the index so titles/tags/backlinks reflect the edit on the
    // next request; a full rebuild is simplest given no incremental API.
    refresh_index(&state);

    Json(serde_json::json!({"ok": true})).into_response()
}

/// GET /web/main.js — the embedded frontend bundle (built by `build.rs`).
async fn handle_editor_js() -> impl IntoResponse {
    match EditorAssets::get("main.js") {
        Some(file) => (
            [(axum::http::header::CONTENT_TYPE, "application/javascript")],
            file.data.into_owned(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "main.js not found").into_response(),
    }
}

/// GET /web/style.css — the embedded frontend stylesheet (built by `build.rs`).
async fn handle_editor_css() -> impl IntoResponse {
    match EditorAssets::get("style.css") {
        Some(file) => (
            [(axum::http::header::CONTENT_TYPE, "text/css")],
            file.data.into_owned(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "main.css not found").into_response(),
    }
}

/// GET /api/notes — JSON list of all notes with metadata. Rebuilds the index
/// from disk on every call so notes added outside this server show up
/// without a restart.
async fn handle_api_notes(State(state): State<AppState>) -> Json<serde_json::Value> {
    let index = refresh_index(&state);
    let notes: Vec<_> = index
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

/// GET /api/notes/*path — JSON metadata + raw content for a single note.
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

    let (fm, _) = granite_core::frontmatter::parse(&content);
    let fm = fm.unwrap_or_default();
    let note_key = format!("notes/{}", path.trim_start_matches('/'));

    Json(serde_json::json!({
        "path": note_key,
        "title": granite_core::frontmatter::get_title(&fm),
        "tags": granite_core::frontmatter::get_tags(&fm),
        "content": content,
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;

    #[tokio::test]
    async fn api_notes_picks_up_files_added_after_startup() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("notes")).unwrap();
        std::fs::create_dir_all(dir.path().join(".granite")).unwrap();
        std::fs::write(dir.path().join("notes/first.md"), "# First\n").unwrap();

        let vault_path = Arc::new(dir.path().to_path_buf());
        let index = Index::build(&vault_path).unwrap();
        let state = AppState {
            vault_path: vault_path.clone(),
            index: Arc::new(RwLock::new(Arc::new(index))),
        };

        let before = handle_api_notes(State(state.clone())).await;
        assert_eq!(before.0.as_array().unwrap().len(), 1);

        // Simulate a note added by another process (e.g. `granite new`)
        // while the server keeps running.
        std::fs::write(dir.path().join("notes/second.md"), "# Second\n").unwrap();

        let after = handle_api_notes(State(state)).await;
        assert_eq!(after.0.as_array().unwrap().len(), 2);
    }
}
