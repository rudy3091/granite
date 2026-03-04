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


#[test]
fn test_new_with_content_flag() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Content Note", "--no-edit", "--content", "Hello from content flag."])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    let content = fs::read_to_string(dir.path().join("notes/content-note.md")).unwrap();
    assert!(content.contains("Hello from content flag."), "note body should contain provided content");
    assert!(!content.contains("# Content Note\n\n"), "template heading should not be used");
}

#[test]
fn test_new_with_stdin_content() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Stdin Note", "--no-edit"])
        .write_stdin("Hello from stdin.")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"));

    let content = fs::read_to_string(dir.path().join("notes/stdin-note.md")).unwrap();
    assert!(content.contains("Hello from stdin."), "note body should contain stdin content");
}

#[test]
fn test_new_stdin_implies_no_edit() {
    // When stdin is piped, the note is created without opening an editor
    // (no-edit is implied). We verify this by checking the note is created
    // successfully when piping — if it tried to open $EDITOR in a non-interactive
    // context it would fail.
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Piped Note"])
        .write_stdin("Piped body content.")
        .env("EDITOR", "false") // "false" binary exits non-zero — would fail if invoked
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("notes/piped-note.md")).unwrap();
    assert!(content.contains("Piped body content."));
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

// ─── view ─────────────────────────────────────────────────────────────────────

#[test]
fn test_view_prints_content() {
    let dir = init_vault();

    let note_path = dir.path().join("notes/view-test.md");
    fs::write(
        &note_path,
        "---\ntitle: View Test\n---\n\n# View Test\n\nHello from view.\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["view", "view-test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("View Test"))
        .stdout(predicate::str::contains("Hello from view."));
}

#[test]
fn test_view_no_frontmatter() {
    let dir = init_vault();

    let note_path = dir.path().join("notes/view-fm.md");
    fs::write(
        &note_path,
        "---\ntitle: FM Note\n---\n\n# FM Note\n\nBody only.\n",
    )
    .unwrap();

    let output = granite()
        .current_dir(dir.path())
        .args(["view", "view-fm", "--no-frontmatter"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("Body only."), "body should be present");
    assert!(!stdout.contains("title:"), "frontmatter should be stripped");
}

#[test]
fn test_view_no_match() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["view", "nonexistent_note_zzz"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No notes matching"));
}

#[test]
fn test_view_fuzzy_match() {
    let dir = init_vault();

    let note_path = dir.path().join("notes/fuzzy-view-note.md");
    fs::write(
        &note_path,
        "---\ntitle: Fuzzy View Note\n---\n\n# Fuzzy View Note\n\nFuzzy content.\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["view", "fuzzy-view"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Fuzzy content."));
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

// ─── edit --append ────────────────────────────────────────────────────────────

#[test]
fn test_edit_append_via_stdin() {
    let dir = init_vault();

    // Create a note with a known body
    let note_path = dir.path().join("notes/append-target.md");
    fs::write(
        &note_path,
        "---\ntitle: Append Target\nmodified: 2020-01-01T00:00:00\n---\n\n# Append Target\n\nOriginal body.\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["edit", "--append", "append-target"])
        .write_stdin("appended line\n")
        .assert()
        .success();

    let content = fs::read_to_string(&note_path).unwrap();
    assert!(content.contains("Original body."), "original body must be preserved");
    assert!(content.contains("appended line"), "new text must be appended");
    // modified timestamp should be updated (not still 2020-01-01)
    assert!(!content.contains("2020-01-01T00:00:00"), "modified should be updated");
}

#[test]
fn test_edit_append_without_stdin_fails() {
    let dir = init_vault();

    let note_path = dir.path().join("notes/no-stdin-note.md");
    fs::write(&note_path, "---\ntitle: No Stdin\n---\n\n# No Stdin\n").unwrap();

    // No write_stdin → stdin is not piped → should fail with clear error
    granite()
        .current_dir(dir.path())
        .args(["edit", "--append", "no-stdin-note"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--append requires piped stdin"));
}

#[test]
fn test_edit_append_ambiguous_query_fails() {
    let dir = init_vault();

    // Two notes that share a common prefix so "log" matches both
    let note1 = dir.path().join("notes/log-alpha.md");
    fs::write(&note1, "---\ntitle: Log Alpha\n---\n\n# Log Alpha\n").unwrap();

    let note2 = dir.path().join("notes/log-beta.md");
    fs::write(&note2, "---\ntitle: Log Beta\n---\n\n# Log Beta\n").unwrap();

    granite()
        .current_dir(dir.path())
        .args(["edit", "--append", "log"])
        .write_stdin("some content\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Ambiguous"));
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

// ─── link ─────────────────────────────────────────────────────────────────────

#[test]
fn test_link_appends_wikilink() {
    let dir = init_vault();

    fs::write(
        dir.path().join("notes/note-a.md"),
        "---\ntitle: Note A\n---\n\n# Note A\n\nSome content.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("notes/note-b.md"),
        "---\ntitle: Note B\n---\n\n# Note B\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["link", "note-a", "note-b"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Linked [[note-b]] into"));

    let content = fs::read_to_string(dir.path().join("notes/note-a.md")).unwrap();
    assert!(content.contains("[[note-b]]"), "target should contain the wiki-link");
}

#[test]
fn test_link_with_content_flag() {
    let dir = init_vault();

    fs::write(
        dir.path().join("notes/note-a.md"),
        "---\ntitle: Note A\n---\n\n# Note A\n\nSome content.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("notes/note-b.md"),
        "---\ntitle: Note B\n---\n\n# Note B\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["link", "note-a", "note-b", "--content", "See also"])
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("notes/note-a.md")).unwrap();
    assert!(content.contains("See also"), "content text should be present");
    assert!(content.contains("[[note-b]]"), "wiki-link should be present");
    // content text should appear before the link
    let content_pos = content.find("See also").unwrap();
    let link_pos = content.find("[[note-b]]").unwrap();
    assert!(content_pos < link_pos, "content should precede the link");
}

#[test]
fn test_link_updates_modified_timestamp() {
    let dir = init_vault();

    fs::write(
        dir.path().join("notes/note-a.md"),
        "---\ntitle: Note A\nmodified: 2020-01-01T00:00:00\n---\n\n# Note A\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("notes/note-b.md"),
        "---\ntitle: Note B\n---\n\n# Note B\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["link", "note-a", "note-b"])
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("notes/note-a.md")).unwrap();
    assert!(
        !content.contains("2020-01-01T00:00:00"),
        "modified timestamp should be updated"
    );
}

#[test]
fn test_link_warns_on_duplicate() {
    let dir = init_vault();

    // note-a already links to note-b
    fs::write(
        dir.path().join("notes/note-a.md"),
        "---\ntitle: Note A\n---\n\n# Note A\n\nSee [[note-b]] for more.\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("notes/note-b.md"),
        "---\ntitle: Note B\n---\n\n# Note B\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["link", "note-a", "note-b"])
        .assert()
        .success()
        .stderr(predicate::str::contains("already linked"));

    // exactly one occurrence of [[note-b]] — no duplicate added
    let content = fs::read_to_string(dir.path().join("notes/note-a.md")).unwrap();
    assert_eq!(
        content.matches("[[note-b]]").count(),
        1,
        "should not duplicate the link"
    );
}

#[test]
fn test_link_target_not_found_fails() {
    let dir = init_vault();

    fs::write(
        dir.path().join("notes/note-b.md"),
        "---\ntitle: Note B\n---\n\n# Note B\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["link", "nonexistent_zzz", "note-b"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No notes matching"));
}

#[test]
fn test_link_destination_not_found_fails() {
    let dir = init_vault();

    fs::write(
        dir.path().join("notes/note-a.md"),
        "---\ntitle: Note A\n---\n\n# Note A\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["link", "note-a", "nonexistent_zzz"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No notes matching"));
}

#[test]
fn test_link_ambiguous_destination_fails() {
    let dir = init_vault();

    fs::write(
        dir.path().join("notes/note-a.md"),
        "---\ntitle: Note A\n---\n\n# Note A\n",
    )
    .unwrap();
    // Two notes sharing a common prefix so "log" matches both
    fs::write(
        dir.path().join("notes/log-alpha.md"),
        "---\ntitle: Log Alpha\n---\n\n# Log Alpha\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("notes/log-beta.md"),
        "---\ntitle: Log Beta\n---\n\n# Log Beta\n",
    )
    .unwrap();

    granite()
        .current_dir(dir.path())
        .args(["link", "note-a", "log"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Ambiguous"));
}
