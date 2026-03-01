use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn granite() -> Command {
    cargo_bin_cmd!("granite")
}

fn init_vault() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    granite()
        .args(["init", dir.path().to_str().unwrap()])
        .assert()
        .success();
    dir
}

// ─── init ────────────────────────────────────────────────────────────────────

#[test]
fn test_init_creates_vault_structure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap();

    granite()
        .args(["init", path])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized granite vault"));

    assert!(dir.path().join(".granite").is_dir());
    assert!(dir.path().join("notes").is_dir());
    assert!(dir.path().join("notes/inbox").is_dir());
    assert!(dir.path().join("notes/daily").is_dir());
    assert!(dir.path().join("templates").is_dir());
    assert!(dir.path().join("templates/default.md").is_file());
    assert!(dir.path().join(".gitignore").is_file());
    assert!(dir.path().join("README.md").is_file());
}

#[test]
fn test_init_default_directory() {
    let dir = tempfile::tempdir().unwrap();
    granite()
        .current_dir(dir.path())
        .args(["init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized granite vault"));

    assert!(dir.path().join(".granite").is_dir());
}

// ─── new ─────────────────────────────────────────────────────────────────────

#[test]
fn test_new_creates_note() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "My Test Note", "--no-edit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    let note_path = dir.path().join("notes/my-test-note.md");
    assert!(note_path.exists(), "note file should be created");

    let content = fs::read_to_string(&note_path).unwrap();
    assert!(content.contains("My Test Note"));
}

#[test]
fn test_new_note_in_subdir() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Inbox Note", "--no-edit", "--dir", "inbox"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    assert!(dir.path().join("notes/inbox/inbox-note.md").exists());
}

#[test]
fn test_new_duplicate_note_fails() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Duplicate", "--no-edit"])
        .assert()
        .success();

    granite()
        .current_dir(dir.path())
        .args(["new", "Duplicate", "--no-edit"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Note already exists"));
}

// ─── list ────────────────────────────────────────────────────────────────────

#[test]
fn test_list_empty_vault() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No notes found."));
}

#[test]
fn test_list_shows_created_notes() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Alpha Note", "--no-edit"])
        .assert()
        .success();

    granite()
        .current_dir(dir.path())
        .args(["new", "Beta Note", "--no-edit"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("Alpha Note") || stdout.contains("alpha-note"));
    assert!(stdout.contains("Beta Note") || stdout.contains("beta-note"));
    assert!(stdout.contains("2 note(s)"));
}

#[test]
fn test_list_tree_flag() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Tree Note", "--no-edit", "--dir", "inbox"])
        .assert()
        .success();

    granite()
        .current_dir(dir.path())
        .args(["list", "--tree"])
        .assert()
        .success()
        .stdout(predicate::str::contains("inbox/"));
}

#[test]
fn test_list_filter_by_tag() {
    let dir = init_vault();

    // Write a note with a tag manually
    let note_path = dir.path().join("notes/tagged-note.md");
    fs::write(
        &note_path,
        "---\ntitle: Tagged Note\ntags: [rust]\n---\n\n# Tagged Note\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["list", "--tag", "rust"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Tagged Note"));
}

// ─── search ──────────────────────────────────────────────────────────────────

#[test]
fn test_search_finds_content() {
    let dir = init_vault();

    let note_path = dir.path().join("notes/searchable.md");
    fs::write(
        &note_path,
        "---\ntitle: Searchable\n---\n\n# Searchable\n\nThis note has a unique_keyword_xyz inside.\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["search", "unique_keyword_xyz"])
        .assert()
        .success()
        .stdout(predicate::str::contains("unique_keyword_xyz"))
        .stdout(predicate::str::contains("1 match(es)"));
}

#[test]
fn test_search_no_match() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Some Note", "--no-edit"])
        .assert()
        .success();

    granite()
        .current_dir(dir.path())
        .args(["search", "nonexistent_term_zzz"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No matches found"));
}

#[test]
fn test_search_case_insensitive_by_default() {
    let dir = init_vault();

    let note_path = dir.path().join("notes/case-note.md");
    fs::write(
        &note_path,
        "---\ntitle: Case Note\n---\n\n# Case Note\n\nHello World\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["search", "hello world"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello World"));
}

// ─── tags ────────────────────────────────────────────────────────────────────

#[test]
fn test_tags_empty_vault() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["tags"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No tags found."));
}

#[test]
fn test_tags_lists_frontmatter_tags() {
    let dir = init_vault();

    let note_path = dir.path().join("notes/rust-note.md");
    fs::write(
        &note_path,
        "---\ntitle: Rust Note\ntags: [rust, programming]\n---\n\n# Rust Note\n",
    )
    .unwrap();

    let output = granite()
        .current_dir(dir.path())
        .args(["tags"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("#rust"));
    assert!(stdout.contains("#programming"));
}

#[test]
fn test_tags_notes_for_tag() {
    let dir = init_vault();

    let note_path = dir.path().join("notes/rust-note.md");
    fs::write(
        &note_path,
        "---\ntitle: Rust Note\ntags: [rust]\n---\n\n# Rust Note\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["tags", "--notes", "rust"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Rust Note"));
}

// ─── links ───────────────────────────────────────────────────────────────────

#[test]
fn test_links_orphans_all_orphans() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Orphan One", "--no-edit"])
        .assert()
        .success();

    granite()
        .current_dir(dir.path())
        .args(["new", "Orphan Two", "--no-edit"])
        .assert()
        .success();

    granite()
        .current_dir(dir.path())
        .args(["links", "--orphans"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Orphan One").or(predicate::str::contains("orphan-one")));
}

#[test]
fn test_links_forward_and_backlinks() {
    let dir = init_vault();

    // Create two notes where "alpha" links to "beta"
    let alpha = dir.path().join("notes/alpha.md");
    fs::write(
        &alpha,
        "---\ntitle: Alpha\n---\n\n# Alpha\n\nSee [[beta]] for details.\n",
    )
    .unwrap();

    let beta = dir.path().join("notes/beta.md");
    fs::write(&beta, "---\ntitle: Beta\n---\n\n# Beta\n\nSome content.\n").unwrap();

    // Forward links from alpha
    granite()
        .current_dir(dir.path())
        .args(["links", "alpha", "--forward"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Beta").or(predicate::str::contains("beta")));

    // Backlinks to beta should include alpha
    granite()
        .current_dir(dir.path())
        .args(["links", "beta", "--backlinks"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Alpha").or(predicate::str::contains("alpha")));
}

// ─── serve kill ──────────────────────────────────────────────────────────────

#[test]
fn test_serve_kill_no_pid_file() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["serve", "kill"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No running Granite server found"));
}

#[test]
fn test_serve_kill_stale_pid_file() {
    let dir = init_vault();

    // Write a PID that is definitely not running
    let pid_file = dir.path().join(".granite/serve.pid");
    fs::write(&pid_file, "99999999").unwrap();

    granite()
        .current_dir(dir.path())
        .args(["serve", "kill"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not running").or(predicate::str::contains("stale")));

    // PID file should have been cleaned up
    assert!(!pid_file.exists(), "stale PID file should be removed");
}

// ─── rename ──────────────────────────────────────────────────────────────────

#[test]
fn test_rename_note() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Old Name", "--no-edit"])
        .assert()
        .success();

    granite()
        .current_dir(dir.path())
        .args(["rename", "Old Name", "New Name"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Renamed"));

    assert!(!dir.path().join("notes/old-name.md").exists());
    assert!(dir.path().join("notes/new-name.md").exists());
}

#[test]
fn test_rename_updates_wikilinks() {
    let dir = init_vault();

    // Create target note and a note that links to it
    let target = dir.path().join("notes/original.md");
    fs::write(
        &target,
        "---\ntitle: Original\n---\n\n# Original\n",
    )
    .unwrap();

    let linker = dir.path().join("notes/linker.md");
    fs::write(
        &linker,
        "---\ntitle: Linker\n---\n\n# Linker\n\nSee [[original]] for more.\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["rename", "original", "renamed"])
        .assert()
        .success();

    // The linker note should now contain [[renamed]]
    let updated = fs::read_to_string(dir.path().join("notes/linker.md")).unwrap();
    assert!(updated.contains("[[renamed]]"), "wikilinks should be updated");
    assert!(!updated.contains("[[original]]"), "old wikilinks should be gone");
}
