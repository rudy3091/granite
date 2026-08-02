# Granite — Functional Specification

> A terminal-first markdown knowledge base with git-synced persistence.

## 1. Overview

Granite is a local-first markdown management tool inspired by Obsidian, built for developers who live in the terminal. It provides a single Rust binary that handles note creation, linking, search, and synchronization — all backed by plain markdown files in a git repository.

### Design Priorities

1. **CLI workflow** — Fast, composable commands for daily note management from the terminal
2. **Web viewer & editor** — Local server for browsing, reading, and editing notes in a browser, accessible from mobile
3. **TUI mode** — Interactive terminal UI for power users (deferred)
4. **SDK / plugin foundation** — Core logic exposed as a reusable Rust library so external programs and future plugins can process note content (deferred, see §9 v0.4)

### Non-Goals

- Real-time collaboration
- Cloud-hosted service (sync via git remotes such as GitHub is supported and encouraged)

## 2. Core Concepts

### Vault

A vault is a directory containing markdown notes and a `.granite/` configuration directory. A vault is also a git repository. One user may have multiple vaults.

### Note

A note is a single markdown file (`.md`) with optional YAML frontmatter. Notes are the atomic unit of knowledge.

### Wiki-Link

Notes reference each other using `[[target]]` syntax. Links resolve by matching the target string against note filenames (without extension) or frontmatter `title` field.

- `[[my-note]]` — links to `my-note.md`
- `[[my-note|display text]]` — links to `my-note.md`, rendered as "display text"
- `[[subdir/my-note]]` — links to `subdir/my-note.md` relative to `notes/`

### Backlink

A backlink is the inverse of a wiki-link. If note A contains `[[B]]`, then B has a backlink from A. Backlinks are computed at index time, not stored in files.

### Tag

Tags are inline markers using `#tag-name` syntax. Tags can also be declared in frontmatter under `tags:`. Tags are used for categorization and filtering.

## 3. Architecture

### Single Binary

Granite ships as a single statically-linked Rust binary (`granite`). No runtime dependencies beyond `git` on the system PATH. The web editor's frontend (built with `vite` + `pnpm`) is compiled to static JS/CSS ahead of time and embedded into the binary at build time (e.g. via `rust-embed`) — end users never need Node.js, pnpm, or a browser build step installed.

Internally, the binary is split into a Cargo workspace: `granite-core` (library crate — vault, index, wiki-link, frontmatter, config, git logic; no CLI or output concerns) and `granite-cli` (binary crate — clap commands, stdout formatting, the `serve` web viewer). The CLI and web viewer both depend on `granite-core` the same way; a future plugin host or an external Rust program can depend on it too, without going through the CLI's stdout/JSON surface. This split is purely internal reorganization — the shipped binary name and all CLI behavior are unchanged.

### System Dependencies

- **git** — required, used via `std::process::Command` for all sync operations

### Key Crates

| Purpose | Crate | Notes |
|---|---|---|
| CLI parsing | `clap` (derive) | Subcommand-based interface |
| Fuzzy matching | `nucleo` | From helix-editor, fast fuzzy search |
| Markdown parsing | `pulldown-cmark` | GFM support (tables, strikethrough, task lists, autolinks), link extraction, HTML rendering |
| Frontmatter | `serde` + `serde_yaml` | YAML frontmatter parsing for index and metadata |
| Web server | `axum` | Local web viewer with file browsing |
| Web editor frontend | `vite` + `pnpm` (build-time only) | Markdown editor via `CodeMirror 6` + `@replit/codemirror-vim` — the same combination Obsidian uses for its live-preview editor with Vim keybindings; output bundled into the Rust binary at compile time, no Node.js needed at runtime |
| TUI framework | `ratatui` + `crossterm` | Deferred: interactive terminal UI |

### Context Resolution

Granite commands work from **any directory**. The "current context" determines which vault a command operates on. Granite resolves the active vault by checking these locations in order (later overrides earlier):

1. **`~/.config/granite/config.toml`** — global config; stores `default_vault` path and a list of registered vaults
2. **`~/.granite/config.toml`** — alternative global config location (same format)
3. **`$(pwd)/.granite/`** — if present, the current directory is itself a vault and takes highest priority

For example, if the user runs `granite new "my idea"` from `/Users/user/x/y/z` and the global config's `default_vault` points to `/Users/user/vaults/work`, the note is created under `/Users/user/vaults/work/notes/my-idea.md`. But if `/Users/user/x/y/z/.granite/` exists, that local vault is used instead.

**Global config format:**

```toml
default_vault = "/Users/user/vaults/work"

[[vaults]]
path = "/Users/user/vaults/work"
name = "work"

[[vaults]]
path = "/Users/user/vaults/personal"
name = "personal"
```

### In-Memory Index

On startup, granite scans the vault and builds an in-memory index. The index is the backbone of granite's seamless experience — every command benefits from pre-parsed frontmatter and link data without touching the filesystem again.

**Indexed data per note:**

- File path and last-modified timestamp
- All frontmatter fields as arbitrary key-value pairs (schemaless — no fixed schema)
- Forward links (all `[[wiki-links]]` found in body)
- Inline tags (all `#tag` occurrences in body)

**Derived data (computed from the above):**

- Backlinks: inverse mapping of forward links
- Tag index: tag → list of notes (from frontmatter `tags` + inline `#tags`)
- Title/alias lookup table: for fast link resolution and fuzzy search
- Frontmatter field index: any frontmatter key can be used to filter and sort notes

> **Future:** A query DSL will allow filtering and sorting notes by arbitrary frontmatter fields (e.g. `status:draft`, `priority:>3`).

The index is built by scanning all `.md` files under `notes/`. For CLI one-shot commands, the index is built, queried, and discarded. For the web viewer, the index stays resident and can refresh on file changes.

### Index Cache

To keep CLI commands fast, granite maintains an index cache at `.granite/index.json`. The cache stores the full parsed index with per-file timestamps. On startup, only files modified since the last cache write are re-parsed. This makes repeated CLI invocations near-instant even for large vaults.

## 4. Data Format

### Note File

Frontmatter is optional and has no mandatory format. Notes can have any YAML frontmatter fields, or none at all. The following are equivalent valid notes:

**With frontmatter (any fields the user chooses):**

```markdown
---
title: My Note Title
tags:
  - rust
  - programming
status: draft
priority: 1
---

# My Note Title

This is the note body. Link to [[another-note]] or [[folder/deep-note|a deep note]].

Use #inline-tags anywhere in the body.
```

**Without frontmatter (also valid):**

```markdown
# My Note Title

A note with no frontmatter. Granite infers the title from the filename
and timestamps from the filesystem.
```

### Frontmatter Fields

Frontmatter is **schemaless**. Any valid YAML key-value pairs are accepted, indexed, and queryable. Users can define their own fields freely (e.g. `status`, `priority`, `project`, `due`) — granite indexes them all without configuration.

A note without frontmatter is equally valid. Granite infers `title` from the filename and `created`/`modified` from filesystem timestamps when frontmatter is absent.

**Well-known fields:** The following fields are conventions that granite gives special behavior to. They are not required.

| Field | Type | Behavior |
|---|---|---|
| `title` | string | Used for link resolution, search ranking, and display. Defaults to filename. |
| `tags` | list[string] | Merged with inline `#tags` in the tag index. |
| `created` | datetime | Auto-set by `granite new`. Used for sorting. |
| `modified` | datetime | Auto-updated by `granite edit`. Powers "recently modified" views. |
| `aliases` | list[string] | Alternative names for wiki-link resolution. |
| `description` | string | Shown in search results and web viewer listings. |

**Auto-management:** `granite new` and `granite edit` automatically populate `created` and `modified`. Users never need to manage timestamps manually.

> **Future:** A DSL will allow querying and filtering notes by arbitrary frontmatter fields (e.g. `granite list --where "status = draft AND priority > 3"`).

### Link Resolution Order

1. Exact filename match (without `.md` extension)
2. Frontmatter `title` match (case-insensitive)
3. Frontmatter `aliases` match (case-insensitive)
4. Ambiguous — reported as warning

### File Naming

- Filenames use lowercase kebab-case: `my-note-title.md`
- Subdirectories are allowed under `notes/`
- No spaces in filenames (replaced with hyphens on creation)

## 5. Filesystem Layout

```
my-vault/                   # vault root (git repo root)
├── .granite/
│   ├── config.toml         # vault configuration
│   └── index.json          # cached index (gitignored)
├── notes/                  # all notes live here
│   ├── inbox/              # quick capture destination
│   ├── daily/              # daily notes (optional convention)
│   └── *.md                # notes, nested freely
├── templates/              # note templates (optional)
│   └── default.md
├── .gitignore
└── README.md
```

### config.toml

```toml
[vault]
name = "my-vault"

[defaults]
editor = "$EDITOR"          # fallback: vi
template = "default"        # default template for `granite new`
daily_format = "%Y-%m-%d"   # daily note filename format

[sync]
auto_commit = false         # auto-commit on save
remote = "origin"           # default git remote
```

## 6. CLI Interface

```
granite <command> [options]
```

### Commands

#### `granite init [path]`

Initialize a new vault at the given path (default: current directory).

- Creates `.granite/`, `notes/`, `templates/` directories
- Creates default `config.toml` and `.gitignore`
- Runs `git init` if not already a git repo

#### `granite new [title]`

Create a new note.

- If `title` is provided, creates `notes/<kebab-case-title>.md`
- If omitted, prompts interactively or uses timestamp
- Applies template from `templates/default.md` if it exists
- Populates frontmatter with `title`, `created` timestamp
- Opens the note in `$EDITOR`

**Flags:**
- `--no-edit` — create without opening editor
- `--template <name>` — use a specific template
- `--dir <subdir>` — create in `notes/<subdir>/`
- `--content <text>` — set the note body directly (skips template)

**Stdin support:** If stdin is not a terminal (i.e. data is piped), the piped content is used as the note body. This implies `--no-edit`.

```sh
# Pipe file contents as the note body
cat some_file | granite new --dir logs xxx_log

# Inline via flag
granite new --dir logs --content "$(cat some_file)" xxx_log
```

#### `granite edit <query>`

Open an existing note in `$EDITOR`.

- `query` is fuzzy-matched against note titles and filenames
- If multiple matches, presents an interactive picker (using nucleo)
- Updates `modified` timestamp in frontmatter on save

**Flags:**
- `--dir <subdir>` — limit search to notes under `notes/<subdir>/`; the `<subdir>` value is itself fuzzy-matched against available directories, and if multiple directories match an interactive picker is shown first

#### `granite view <query>`

Print a note's content to stdout.

- `query` is fuzzy-matched against note titles and filenames (same as `granite edit`)
- If multiple matches, presents an interactive picker
- Outputs the raw file content (including frontmatter) to stdout

**Flags:**
- `--no-frontmatter` — strip frontmatter, print only the body
- `--dir <subdir>` — limit search to notes under `notes/<subdir>/`; the `<subdir>` value is itself fuzzy-matched against available directories, and if multiple directories match an interactive picker is shown first

#### `granite list`

List all notes in the vault.

**Flags:**
- `--tag <tag>` — filter by tag
- `--sort <field>` — sort by `title`, `created`, `modified` (default: `modified`)
- `--tree` — show as directory tree
- `--paths` — print one absolute path per line, no decorators, no summary; safe for shell word-splitting and `$()` substitution
- `--no-summary` — suppress the trailing `N note(s)` count line; useful when piping to `wc -l` or `grep`
- `--format <fmt>` — output format: `plain` (default) or `json` (JSON array with `path`, `rel_path`, `title`, `tags`, `modified` fields per note)
- `--limit <N>` — output at most N notes after sorting; useful in `$()` contexts (e.g. `latest=$(granite list --paths --limit 1)`)
- `--dir <subdir>` — limit output to notes under `notes/<subdir>/`; the `<subdir>` value is itself fuzzy-matched against available directories, and if multiple directories match an interactive picker is shown first
- `--dir-only` — output subdirectory names (one per line) instead of notes; combine with `--dir` to explore subtrees; enables gradual vault navigation
- `--depth <N>` — limit traversal depth relative to vault root or `--dir` base (0 = immediate children only, 1 = one level down, etc.)

**Flag conflicts:** `--dir-only` is mutually exclusive with `--format json` and `--tag`. `--sort created` and `--sort modified` are not valid with `--dir-only` (directories have no timestamps).

**`--dir-only` with `--paths`:** When both flags are used together, directories are printed as absolute paths (one per line, no trailing slash), consistent with how `--paths` behaves for notes.

**Gradual exploration workflow:**
```sh
# Step 1: see top-level directory structure
granite list --dir-only

# Step 2: drill into a subtree
granite list --dir-only --dir projects

# Step 3: list notes inside
granite list --dir projects/2026
granite list --dir projects --depth 1   # only direct children of projects/
```

**Unix composability examples:**
```sh
# Pipe to fuzzy finder, open result in editor
granite list --paths | fzf | xargs $EDITOR

# Get the most recently modified note path
latest=$(granite list --paths --limit 1)

# Count notes with a tag
granite list --tag rust --paths | wc -l

# Process metadata with jq
granite list --format json | jq '.[] | .title'

# Open all todo-tagged notes
vim $(granite list --tag todo --paths)

# Scope a listing to immediate children only
granite list --dir projects --depth 1 --paths

# Count notes directly in inbox
granite list --dir inbox --depth 0 --no-summary --paths | wc -l
```

#### `granite search <pattern>`

Full-text search across all notes.

- v0.1: regex-based search over file contents (`grep`-style)
- Displays matching lines with context
- Results are interactive: select a result to open in `$EDITOR`

**Flags:**
- `--case-sensitive` — exact case matching (default: case-insensitive)

#### `granite links <note>`

Show link information for a note.

- Forward links: notes that `<note>` links to
- Backlinks: notes that link to `<note>`
- Orphans: notes with no incoming or outgoing links (with `--orphans` flag)

**Flags:**
- `--backlinks` — show only backlinks
- `--forward` — show only forward links
- `--orphans` — list all orphan notes

#### `granite tags`

List all tags and their note counts.

**Flags:**
- `--notes <tag>` — list notes with a given tag

#### `granite daily`

Create or open today's daily note.

- Filename based on `daily_format` config (e.g., `2026-03-01.md`)
- Created in `notes/daily/`

#### `granite sync [message]`

Git sync operations.

- Default (no subcommand): `git add notes/ && git commit -m "<message>" && git pull --rebase && git push`
- Commit message defaults to `"vault sync: <timestamp>"` if not provided

**Subcommands:**
- `granite sync status` — show `git status`
- `granite sync log` — show recent commits
- `granite sync pull` — pull from remote
- `granite sync push` — push to remote

#### `granite rename <old> <new>`

Rename a note and update all wiki-links across the vault that reference it.

#### `granite context`

Manage which vault granite operates on. Allows granite commands to work from any directory.

- `granite context` — show the currently active vault (resolved via context resolution priority)
- `granite context set <path>` — set the default vault in global config
- `granite context list` — list all registered vaults
- `granite context add <path>` — register a vault in global config
- `granite context remove <path>` — unregister a vault from global config

## 7. Web Viewer

```
granite serve [--port 3000]
```

Starts a local HTTP server exposing a single editor-focused page: a note list sidebar plus a live-preview Vim-keybound markdown editor. This is the primary way to edit notes outside the terminal, and is accessible from mobile devices on the same network.

### Design Principles

- **Editor-focused** — The whole UI is one page: pick a note from the sidebar, edit it, save. No separate rendered-markdown view, tag browser, or search page (deferred — see below).
- **Read & write** — Notes are edited via an Obsidian-style live-preview markdown editor with Vim keybindings, in addition to terminal editing. Both write to the same files under `notes/`.
- **Thin server** — `granite serve` is a JSON API (list/get/save notes) plus a static shell; all UI logic lives in the `web/` frontend.
- **Mobile-friendly** — Responsive layout that works on phone screens. Not a priority target, but usable.
- **Minimal frontend build** — A `vite` + `pnpm` project (`web/`) builds `main.js`, embedded into the binary via `rust-embed`. The HTML shell itself (`<div id="app">` + a script tag) is a small static file kept in `granite-cli` directly (not vite output), with a structurally identical copy in `web/` so `pnpm dev` renders the same shell with HMR.

### Routes

| Route | Description |
|---|---|
| `GET /` | The editor shell (static HTML, embedded in the binary) |
| `GET /web/main.js` | The frontend bundle (sidebar + editor logic, built by `vite`) |
| `GET /api/notes` | JSON: list of all notes with frontmatter metadata |
| `GET /api/notes/<path>` | JSON: single note metadata + raw content |
| `PUT /api/notes/<path>` | Save edited note content (raw markdown), updates `modified` frontmatter |

### Editor UI

The single page shows:

- A sidebar listing all notes (title, from `/api/notes`); clicking one loads its raw content into the editor
- A CodeMirror 6 + `@replit/codemirror-vim` editor pane (same combination Obsidian uses)
- A Save button and `:w` (Vim) both save via `PUT /api/notes/<path>`

Rendered markdown preview, wiki-link navigation, backlinks, tag browsing, and search are **not** part of the web UI yet — they were part of an earlier server-rendered prototype and were deliberately removed in favor of an editor-first minimum baseline. They may return as views built on top of the same JSON API in a later iteration; not currently scoped.

### Implementation

- **Server:** `axum`, serving a fixed set of JSON routes plus the static shell/bundle — no general-purpose static file server, no markdown rendering.
- **Frontend:** `vite` + `pnpm` project in `web/`, producing an ES module bundle (`main.js`) and its stylesheet (`style.css`), both embedded into the binary via `rust-embed`. `web/src/editor.ts` wraps CodeMirror 6 + `@replit/codemirror-vim`; `web/src/main.ts` is the app (fetches `/api/notes`, renders the sidebar, mounts the editor, saves via `PUT`) and imports `web/src/style.css`, the single source of the UI's CSS shared by both the vite dev shell and the embedded static shell. Written in TypeScript.
- **Index:** Reuses the same in-memory index as CLI commands; rebuilt and swapped in after each save, and on every `GET /api/notes` call, so the note list stays current even when notes are added or edited outside this server (e.g. `granite new` in another terminal, or a file dropped into `notes/`). The frontend polls `GET /api/notes` every 3 seconds to pick this up without a manual page refresh.

## 8. TUI Mode (Deferred)

> TUI mode is deferred. Not part of v0.1 or v0.2 scope.

```
granite tui
```

Planned as an interactive terminal UI built with `ratatui` + `crossterm` for power users who want to browse, search, and navigate notes without leaving the terminal.

### Planned Features

- File tree browser with vim-style navigation
- Rendered markdown preview pane
- Fuzzy search over note titles (nucleo-powered)
- Full-text content search
- Backlink panel
- Link following: navigate `[[wiki-links]]` inline
- Quick actions: create, edit, sync from within TUI

TUI will be specified in detail when development begins.

## 9. MVP Scope

### v0.1 — CLI Foundation

Core CLI commands and the indexing engine.

- [ ] `granite init` — vault scaffolding
- [ ] `granite new` — note creation with template and auto-frontmatter
- [ ] `granite edit` — fuzzy picker → `$EDITOR`
- [ ] `granite view` — print note content to stdout
- [ ] `granite list` — note listing with tag filter, sort, tree view
- [ ] `granite search` — regex-based full-text search
- [ ] `granite links` — forward links, backlinks, orphan detection
- [ ] `granite tags` — tag listing and per-tag note lookup
- [ ] `granite daily` — daily note creation/opening
- [ ] `granite sync` — git add/commit/pull/push via system git
- [ ] `granite rename` — rename with vault-wide link updating
- [ ] `granite context` — vault context management (set/list/add/remove)
- [ ] Context resolution (`~/.config/granite` → `~/.granite` → `$(pwd)/.granite`)
- [ ] In-memory index with JSON cache for fast repeated invocations
- [ ] Wiki-link parsing and resolution (filename → title → alias)
- [ ] YAML frontmatter auto-management (`created`, `modified`)

### v0.2 — Web Viewer & Editor

Web interface for editing notes, editor-first.

- [x] `granite serve` — local HTTP server (axum)
- [x] JSON API for note metadata (`GET /api/notes`, `GET /api/notes/<path>`)
- [x] Live-preview markdown editor with Vim keybindings (`CodeMirror 6` + `@replit/codemirror-vim`, same as Obsidian; built via `vite` + `pnpm`, bundled into the binary at compile time)
- [x] Save endpoint (`PUT /api/notes/<path>`) writing back to the note file and updating `modified`
- [ ] File explorer / rendered markdown note view / backlinks / tag browsing / search page — deferred; not currently scoped (removed from an earlier prototype in favor of an editor-first minimum baseline, see §7)
- [ ] Responsive CSS for mobile access

### v0.3 — TUI and Graph

Interactive terminal UI and link visualization.

- [ ] `granite tui` — ratatui-based interactive browser
- [ ] `granite graph` — link graph visualization (ASCII or web)
- [ ] Note templates system (multiple named templates)
- [ ] Broken link detection and reporting

### v0.4 — SDK & Plugin Foundation (deferred)

Expose granite's core logic as a reusable Rust library, laying the groundwork for a future plugin system without committing to a plugin API yet.

- [ ] Split the workspace into `granite-core` (library: vault, index, wiki-link, frontmatter, config, git) and `granite-cli` (binary: commands, output formatting, `serve`)
- [ ] `granite-core` published as a standalone crate with a documented public API
- [ ] No behavior change to the `granite` CLI binary or its output

### v0.5 — Full-text Search

Replace regex-based search with a proper full-text index once the above are done.

- [ ] Full-text search via tantivy index (replaces regex search), used by both `granite search` and the web viewer's search page

### Deferred

- Real-time collaboration
- Plugin system built on top of `granite-core` (API design, discovery/loading, sandboxing — not yet scoped)
- End-to-end encryption
- Cloud sync (beyond git remotes)
