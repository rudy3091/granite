# Granite — Functional Specification

> A terminal-first markdown knowledge base with git-synced persistence.

## 1. Overview

Granite is a local-first markdown management tool inspired by Obsidian, built for developers who live in the terminal. It provides a single Rust binary that handles note creation, linking, search, and synchronization — all backed by plain markdown files in a git repository.

### Design Priorities

1. **CLI workflow** — Fast, composable commands for daily note management from the terminal
2. **Web viewer** — Local server for browsing and reading notes in a browser, accessible from mobile
3. **TUI mode** — Interactive terminal UI for power users (deferred)

### Non-Goals

- WYSIWYG or rich-text editing
- Real-time collaboration
- Cloud-hosted service (sync via git remotes such as GitHub is supported and encouraged)
- Plugin/extension system

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

Granite ships as a single statically-linked Rust binary (`granite`). No runtime dependencies beyond `git` on the system PATH.

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

Starts a local HTTP server that provides a read-only file explorer and markdown viewer. This is the primary way to browse and read notes outside the terminal, and is accessible from mobile devices on the same network.

### Design Principles

- **Read-only** — No editing through the web interface. Editing is done in the terminal.
- **File explorer first** — The landing page is a navigable directory tree of `notes/`, not a dashboard.
- **Rendered markdown** — Notes are rendered as HTML with wiki-links converted to clickable links.
- **Mobile-friendly** — Responsive layout that works on phone screens. Not a priority target, but usable.
- **Zero JS frameworks** — Plain HTML/CSS with minimal vanilla JS. Fast to load, no build step for frontend.

### Routes

| Route | Description |
|---|---|
| `GET /` | File explorer: directory tree of `notes/` |
| `GET /notes/<path>` | Rendered markdown view of a note |
| `GET /tags` | List of all tags with note counts |
| `GET /tags/<tag>` | List of notes with a given tag |
| `GET /search?q=<query>` | Search results page |
| `GET /api/notes` | JSON: list of all notes with frontmatter metadata |
| `GET /api/notes/<path>` | JSON: single note metadata + rendered HTML |

### File Explorer View

The landing page shows the vault's `notes/` directory as a navigable tree:

- Folders are expandable/collapsible
- Each note shows its title (from frontmatter or filename) and tags
- Sorted by last modified by default
- Click a note to open its rendered view

### Note View

Individual note pages show:

- Rendered markdown content (headings, lists, code blocks, images)
- Wiki-links rendered as clickable internal links
- Frontmatter displayed as subtle metadata (title, tags, dates)
- Backlinks section at the bottom: list of notes linking to this note
- Tag links that navigate to `/tags/<tag>`

### Implementation

- **Server:** `axum` with `tower-http` for static assets
- **Rendering:** `pulldown-cmark` for GitHub Flavored Markdown → HTML conversion (tables, strikethrough, task lists, autolinks)
- **Templates:** Simple HTML templates (handlebars or inline), no SPA
- **Styling:** Single CSS file, responsive, minimal
- **Index:** Reuses the same in-memory index as CLI commands; stays resident while server runs

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

### v0.2 — Web Viewer

Read-only web interface for browsing and reading notes.

- [ ] `granite serve` — local HTTP server (axum)
- [ ] File explorer landing page (directory tree of notes)
- [ ] Rendered markdown note view with clickable wiki-links
- [ ] Backlinks displayed on each note page
- [ ] Tag browsing pages
- [ ] Search page
- [ ] JSON API for note metadata
- [ ] Responsive CSS for mobile access

### v0.3 — Search and TUI

Advanced search and interactive terminal UI.

- [ ] Full-text search via tantivy index (replaces regex search)
- [ ] `granite tui` — ratatui-based interactive browser
- [ ] `granite graph` — link graph visualization (ASCII or web)
- [ ] Note templates system (multiple named templates)
- [ ] Broken link detection and reporting

### Deferred

- Real-time collaboration
- Plugin/extension system
- End-to-end encryption
- Cloud sync (beyond git remotes)
- Web-based editing
