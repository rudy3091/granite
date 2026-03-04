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

// ─── list --paths ─────────────────────────────────────────────────────────────

#[test]
fn test_list_paths_prints_absolute_paths() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Path Note", "--no-edit"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--paths"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    // Each non-empty line must be an absolute path ending in .md
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(!lines.is_empty(), "should have at least one path");
    for line in &lines {
        assert!(line.ends_with(".md"), "each line should end with .md, got: {}", line);
        assert!(std::path::Path::new(line).is_absolute(), "path should be absolute: {}", line);
    }
    // No summary line
    assert!(!stdout.contains("note(s)"), "summary line should not appear with --paths");
}

#[test]
fn test_list_paths_empty_vault_produces_no_output() {
    let dir = init_vault();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--paths"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.trim().is_empty(), "empty vault --paths should produce no output");
}

#[test]
fn test_list_paths_respects_tag_filter() {
    let dir = init_vault();

    fs::write(
        dir.path().join("notes/rust-note.md"),
        "---\ntitle: Rust Note\ntags: [rust]\n---\n\n# Rust Note\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("notes/other-note.md"),
        "---\ntitle: Other Note\n---\n\n# Other Note\n",
    )
    .unwrap();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--paths", "--tag", "rust"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("rust-note.md"), "rust-tagged note path should appear");
    assert!(!stdout.contains("other-note.md"), "untagged note should not appear");
}

// ─── list --no-summary ────────────────────────────────────────────────────────

#[test]
fn test_list_no_summary_suppresses_count_line() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Summary Note", "--no-edit"])
        .assert()
        .success();

    granite()
        .current_dir(dir.path())
        .args(["list", "--no-summary"])
        .assert()
        .success()
        .stdout(predicate::str::contains("note(s)").not());
}

#[test]
fn test_list_default_still_shows_summary() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Backward Compat Note", "--no-edit"])
        .assert()
        .success();

    granite()
        .current_dir(dir.path())
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("note(s)"));
}

// ─── list --format json ───────────────────────────────────────────────────────

#[test]
fn test_list_format_json_is_valid_json_array() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Json Note", "--no-edit"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("output should be valid JSON");
    assert!(parsed.is_array(), "output should be a JSON array");
}

#[test]
fn test_list_format_json_contains_expected_fields() {
    let dir = init_vault();

    fs::write(
        dir.path().join("notes/field-note.md"),
        "---\ntitle: Field Note\ntags: [alpha]\n---\n\n# Field Note\n",
    )
    .unwrap();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    let note = parsed.iter().find(|v| v["title"] == "Field Note").expect("Field Note should be in output");
    assert!(note.get("path").is_some(), "should have 'path' field");
    assert!(note.get("rel_path").is_some(), "should have 'rel_path' field");
    assert!(note.get("title").is_some(), "should have 'title' field");
    assert!(note.get("tags").is_some(), "should have 'tags' field");
    assert!(note.get("modified").is_some(), "should have 'modified' field");
    assert_eq!(note["tags"][0], "alpha");
}

#[test]
fn test_list_format_json_empty_vault_returns_empty_array() {
    let dir = init_vault();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap().trim().to_string();
    assert_eq!(stdout, "[]", "empty vault should return empty JSON array");
}

#[test]
fn test_list_format_unknown_exits_with_error() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["list", "--format", "toml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown format"));
}

// ─── list --limit ─────────────────────────────────────────────────────────────

#[test]
fn test_list_limit_returns_at_most_n_notes() {
    let dir = init_vault();

    for i in 1..=5 {
        granite()
            .current_dir(dir.path())
            .args(["new", &format!("Note {}", i), "--no-edit"])
            .assert()
            .success();
    }

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--paths", "--limit", "3"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let line_count = stdout.lines().count();
    assert_eq!(line_count, 3, "should return exactly 3 paths");
}

#[test]
fn test_list_limit_respects_sort_order() {
    let dir = init_vault();

    fs::write(
        dir.path().join("notes/aaa-note.md"),
        "---\ntitle: AAA Note\n---\n\n# AAA Note\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("notes/zzz-note.md"),
        "---\ntitle: ZZZ Note\n---\n\n# ZZZ Note\n",
    )
    .unwrap();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--paths", "--sort", "title", "--limit", "1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("aaa-note.md"), "alphabetically first note should be returned");
    assert!(!stdout.contains("zzz-note.md"), "second note should not appear");
}

#[test]
fn test_list_limit_larger_than_count_is_safe() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Only Note One", "--no-edit"])
        .assert()
        .success();
    granite()
        .current_dir(dir.path())
        .args(["new", "Only Note Two", "--no-edit"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--paths", "--limit", "100"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert_eq!(stdout.lines().count(), 2, "should return all 2 notes when limit exceeds count");
}

// ─── list flag conflicts ──────────────────────────────────────────────────────

#[test]
fn test_list_paths_and_tree_conflict() {
    let dir = init_vault();
    granite()
        .current_dir(dir.path())
        .args(["list", "--paths", "--tree"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn test_list_format_json_and_paths_conflict() {
    let dir = init_vault();
    granite()
        .current_dir(dir.path())
        .args(["list", "--format", "json", "--paths"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn test_list_format_json_and_tree_conflict() {
    let dir = init_vault();
    granite()
        .current_dir(dir.path())
        .args(["list", "--format", "json", "--tree"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn test_list_format_json_and_no_summary_conflict() {
    let dir = init_vault();
    granite()
        .current_dir(dir.path())
        .args(["list", "--format", "json", "--no-summary"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--no-summary has no effect"));
}

#[test]
fn test_list_paths_and_no_summary_conflict() {
    let dir = init_vault();
    granite()
        .current_dir(dir.path())
        .args(["list", "--paths", "--no-summary"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--no-summary has no effect"));
}

// ─── list --dir ───────────────────────────────────────────────────────────────

#[test]
fn test_list_dir_filters_to_subdirectory() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Inbox Note", "--no-edit", "--dir", "inbox"])
        .assert()
        .success();
    granite()
        .current_dir(dir.path())
        .args(["new", "Projects Note", "--no-edit"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--dir", "inbox"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("Inbox Note"), "inbox note should appear");
    assert!(!stdout.contains("Projects Note"), "root note should not appear");
}

#[test]
fn test_list_dir_includes_nested_subdirectories() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Deep Note", "--no-edit", "--dir", "projects/2026"])
        .assert()
        .success();

    granite()
        .current_dir(dir.path())
        .args(["list", "--dir", "projects"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Deep Note"));
}

#[test]
fn test_list_dir_no_match_returns_error() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["list", "--dir", "nonexistent_zzz"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No directories matching"));
}

#[test]
fn test_list_dir_fuzzy_matches_directory_name() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Proj Note", "--no-edit", "--dir", "projects"])
        .assert()
        .success();

    // "proj" is a prefix of "projects" — should fuzzy-match
    granite()
        .current_dir(dir.path())
        .args(["list", "--dir", "proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Proj Note"));
}

#[test]
fn test_list_dir_with_paths_flag() {
    let dir = init_vault();

    // "inbox a" → kebab-case → inbox-a.md
    granite()
        .current_dir(dir.path())
        .args(["new", "inbox a", "--no-edit", "--dir", "inbox"])
        .assert()
        .success();
    granite()
        .current_dir(dir.path())
        .args(["new", "Root Note", "--no-edit"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--dir", "inbox", "--paths"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("inbox-a.md"), "inbox note path should appear");
    assert!(!stdout.contains("root-note.md"), "root note path should not appear");
    for line in stdout.lines().filter(|l| !l.is_empty()) {
        assert!(
            std::path::Path::new(line).is_absolute(),
            "path should be absolute: {}",
            line
        );
    }
}

#[test]
fn test_list_dir_with_format_json() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Json Dir Note", "--no-edit", "--dir", "inbox"])
        .assert()
        .success();
    granite()
        .current_dir(dir.path())
        .args(["new", "Other", "--no-edit"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--dir", "inbox", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed.len(), 1, "only the inbox note should appear");
    assert_eq!(parsed[0]["title"], "Json Dir Note");
}

#[test]
fn test_list_dir_and_tag_filters_compose() {
    let dir = init_vault();

    // Use inline #rust tag in body so the index picks it up
    granite()
        .current_dir(dir.path())
        .args(["new", "Tagged Inbox", "--no-edit", "--dir", "inbox", "--content", "About #rust"])
        .assert()
        .success();
    granite()
        .current_dir(dir.path())
        .args(["new", "Untagged Inbox", "--no-edit", "--dir", "inbox"])
        .assert()
        .success();
    granite()
        .current_dir(dir.path())
        .args(["new", "Tagged Root", "--no-edit", "--content", "About #rust"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--dir", "inbox", "--tag", "rust"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("Tagged Inbox"), "tagged inbox note should appear");
    assert!(!stdout.contains("Untagged Inbox"), "untagged inbox note should not appear");
    assert!(!stdout.contains("Tagged Root"), "tagged root note should not appear");
}

#[test]
fn test_list_dir_empty_result_shows_dir_message() {
    let dir = init_vault();

    // inbox exists (created by init_vault) but has no notes
    granite()
        .current_dir(dir.path())
        .args(["list", "--dir", "inbox"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No notes found in directory 'inbox'"));
}

// ─── list --dir-only ─────────────────────────────────────────────────────────

#[test]
fn test_list_dir_only_shows_directories() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Proj Note", "--no-edit", "--dir", "projects"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--dir-only"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("projects"), "projects dir should appear");
    assert!(!stdout.contains("Proj Note"), "note title should not appear");
    assert!(!stdout.contains("proj-note"), "note filename should not appear");
}

#[test]
fn test_list_dir_only_one_dir_per_line() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "P", "--no-edit", "--dir", "projects"])
        .assert()
        .success();
    granite()
        .current_dir(dir.path())
        .args(["new", "A", "--no-edit", "--dir", "archive"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--dir-only", "--no-summary"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let dir_lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(dir_lines.len() >= 2, "at least two dir lines expected");
    for line in &dir_lines {
        assert!(
            !line.contains("projects") || !line.contains("archive"),
            "dirs should be on separate lines, got: {}",
            line
        );
    }
}

#[test]
fn test_list_dir_only_with_dir_shows_subdirs() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Note 2026", "--no-edit", "--dir", "projects/2026"])
        .assert()
        .success();
    granite()
        .current_dir(dir.path())
        .args(["new", "Old", "--no-edit", "--dir", "projects/archive"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--dir-only", "--dir", "projects"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("2026"), "2026 subdir should appear");
    assert!(stdout.contains("archive"), "archive subdir should appear");
    assert!(!stdout.contains("Note 2026"), "note titles should not appear");
}

#[test]
fn test_list_dir_only_no_subdirectories() {
    let dir = init_vault();

    // inbox (created by init_vault) has no subdirectories
    granite()
        .current_dir(dir.path())
        .args(["list", "--dir-only", "--dir", "inbox"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No subdirectories found"));
}

#[test]
fn test_list_dir_only_with_no_summary() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "P", "--no-edit", "--dir", "projects"])
        .assert()
        .success();

    granite()
        .current_dir(dir.path())
        .args(["list", "--dir-only", "--no-summary"])
        .assert()
        .success()
        .stdout(predicate::str::contains("director").not());
}

#[test]
fn test_list_dir_only_with_limit() {
    let dir = init_vault();

    for name in &["aaa", "bbb", "ccc"] {
        granite()
            .current_dir(dir.path())
            .args(["new", &format!("Note {}", name), "--no-edit", "--dir", name])
            .assert()
            .success();
    }

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--dir-only", "--limit", "2", "--no-summary"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 2, "limit should restrict to 2 dirs, got: {:?}", lines);
}

#[test]
fn test_list_dir_only_conflict_with_paths() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["list", "--dir-only", "--paths"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn test_list_dir_only_conflict_with_format_json() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["list", "--dir-only", "--format", "json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn test_list_dir_only_conflict_with_tag() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["list", "--dir-only", "--tag", "rust"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn test_list_dir_only_with_sort_is_ignored() {
    // --sort has no effect on --dir-only (dirs are always listed alphabetically);
    // the flag is accepted without error rather than conflicting.
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "P", "--no-edit", "--dir", "projects"])
        .assert()
        .success();

    granite()
        .current_dir(dir.path())
        .args(["list", "--dir-only", "--sort", "created"])
        .assert()
        .success();
}

// ─── list --depth ─────────────────────────────────────────────────────────────

#[test]
fn test_list_depth_zero_shows_only_root_notes() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Root Note", "--no-edit"])
        .assert()
        .success();
    granite()
        .current_dir(dir.path())
        .args(["new", "Inbox Note", "--no-edit", "--dir", "inbox"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--depth", "0"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("Root Note"), "root note should appear at depth 0");
    assert!(!stdout.contains("Inbox Note"), "inbox note should not appear at depth 0");
}

#[test]
fn test_list_depth_one_includes_one_level_subdirs() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Root Note", "--no-edit"])
        .assert()
        .success();
    granite()
        .current_dir(dir.path())
        .args(["new", "Inbox Note", "--no-edit", "--dir", "inbox"])
        .assert()
        .success();
    granite()
        .current_dir(dir.path())
        .args(["new", "Deep Note", "--no-edit", "--dir", "inbox/deep"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--depth", "1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("Root Note"), "root note should appear");
    assert!(stdout.contains("Inbox Note"), "depth-1 note should appear");
    assert!(!stdout.contains("Deep Note"), "depth-2 note should not appear at --depth 1");
}

#[test]
fn test_list_depth_with_dir_is_relative() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Projects Top", "--no-edit", "--dir", "projects"])
        .assert()
        .success();
    granite()
        .current_dir(dir.path())
        .args(["new", "Projects Nested", "--no-edit", "--dir", "projects/2026"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--dir", "projects", "--depth", "0"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("Projects Top"), "direct child note should appear");
    assert!(
        !stdout.contains("Projects Nested"),
        "nested note should not appear at depth 0 relative to dir"
    );
}

#[test]
fn test_list_depth_with_dir_only() {
    let dir = init_vault();

    granite()
        .current_dir(dir.path())
        .args(["new", "Q1 Note", "--no-edit", "--dir", "projects/2026/q1"])
        .assert()
        .success();

    let output = granite()
        .current_dir(dir.path())
        .args(["list", "--dir-only", "--depth", "1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("projects"), "top-level projects dir should appear");
    assert!(!stdout.contains("q1"), "depth-3 dir should not appear at --depth 1");
}
