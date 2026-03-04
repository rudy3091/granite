mod commands;
mod config;
mod frontmatter;
mod git;
mod index;
mod vault;
mod wikilink;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "granite", version, about = "A terminal-first markdown knowledge base")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new vault
    Init {
        /// Path to create the vault (default: current directory)
        path: Option<String>,
    },

    /// Create a new note
    New {
        /// Note title
        title: Option<String>,

        /// Create without opening editor
        #[arg(long)]
        no_edit: bool,

        /// Template to use
        #[arg(long)]
        template: Option<String>,

        /// Subdirectory under notes/
        #[arg(long)]
        dir: Option<String>,

        /// Set note body directly (skips template)
        #[arg(long)]
        content: Option<String>,
    },

    /// Open a note in $EDITOR
    Edit {
        /// Fuzzy search query
        query: String,

        /// Append piped stdin to the note (skips editor)
        #[arg(long)]
        append: bool,
    },

    /// Print a note's content to stdout
    View {
        /// Fuzzy search query
        query: String,

        /// Strip frontmatter, print only the body
        #[arg(long)]
        no_frontmatter: bool,

        /// Limit search to notes under notes/<subdir>/ (fuzzy-matched against available dirs)
        #[arg(long)]
        dir: Option<String>,
    },

    /// List all notes
    List {
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,

        /// Sort field: title, created, modified (default: modified)
        #[arg(long, default_value = "modified")]
        sort: String,

        /// Show as directory tree
        #[arg(long)]
        tree: bool,
    },

    /// Full-text search across notes
    Search {
        /// Regex pattern to search for
        pattern: String,

        /// Exact case matching
        #[arg(long)]
        case_sensitive: bool,
    },

    /// Show link information for a note
    Links {
        /// Note to inspect
        note: Option<String>,

        /// Show only backlinks
        #[arg(long)]
        backlinks: bool,

        /// Show only forward links
        #[arg(long)]
        forward: bool,

        /// List all orphan notes
        #[arg(long)]
        orphans: bool,
    },

    /// List all tags
    Tags {
        /// List notes with a given tag
        #[arg(long)]
        notes: Option<String>,
    },

    /// Create or open today's daily note
    Daily,

    /// Git sync operations
    Sync {
        #[command(subcommand)]
        subcommand: Option<SyncCommands>,

        /// Commit message (for default sync)
        #[arg(long, short)]
        message: Option<String>,
    },

    /// Rename a note and update all wiki-links
    Rename {
        /// Current note name/query
        old: String,
        /// New note name
        new: String,
    },

    /// Manage vault context
    Context {
        #[command(subcommand)]
        subcommand: Option<ContextCommands>,
    },

    /// Start a local web server to browse notes in a browser
    Serve {
        #[command(subcommand)]
        subcommand: Option<ServeCommands>,

        /// Port to listen on
        #[arg(long, default_value_t = 3000)]
        port: u16,
    },

    /// Internal: run the web server in the foreground (invoked by `granite serve`)
    #[command(hide = true)]
    ServeFg {
        vault_path: String,
        port: u16,
    },
}

#[derive(Subcommand)]
enum ServeCommands {
    /// Stop the currently running server daemon
    Kill,
}

#[derive(Subcommand)]
enum SyncCommands {
    /// Show git status
    Status,
    /// Show recent commits
    Log,
    /// Pull from remote
    Pull,
    /// Push to remote
    Push,
}

#[derive(Subcommand)]
enum ContextCommands {
    /// Set the default vault
    Set {
        /// Path to the vault
        path: String,
    },
    /// List all registered vaults
    List,
    /// Register a vault
    Add {
        /// Path to the vault
        path: String,
    },
    /// Unregister a vault
    Remove {
        /// Path to the vault
        path: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path } => {
            commands::init::run(path.as_deref())?;
        }

        Commands::New {
            title,
            no_edit,
            template,
            dir,
            content,
        } => {
            let vault_path = vault::resolve_vault()?;
            // Read from stdin when piped; stdin content implies no-edit
            let (resolved_content, resolved_no_edit) = if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                (Some(content.unwrap_or(buf)), true)
            } else {
                (content, no_edit)
            };
            commands::new::run(
                &vault_path,
                commands::new::NewOptions {
                    title,
                    no_edit: resolved_no_edit,
                    template,
                    dir,
                    content: resolved_content,
                },
            )?;
        }

        Commands::Edit { query, append } => {
            let vault_path = vault::resolve_vault()?;
            let stdin_content = if append && !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                if buf.is_empty() { None } else { Some(buf) }
            } else {
                None
            };
            commands::edit::run(
                &vault_path,
                &query,
                commands::edit::EditOptions { append },
                stdin_content,
            )?;
        }

        Commands::View {
            query,
            no_frontmatter,
            dir,
        } => {
            let vault_path = vault::resolve_vault()?;
            commands::view::run(&vault_path, &query, commands::view::ViewOptions { no_frontmatter, dir })?;
        }

        Commands::List { tag, sort, tree } => {
            let vault_path = vault::resolve_vault()?;
            commands::list::run(
                &vault_path,
                commands::list::ListOptions { tag, sort, tree },
            )?;
        }

        Commands::Search {
            pattern,
            case_sensitive,
        } => {
            let vault_path = vault::resolve_vault()?;
            commands::search::run(
                &vault_path,
                &pattern,
                commands::search::SearchOptions { case_sensitive },
            )?;
        }

        Commands::Links {
            note,
            backlinks,
            forward,
            orphans,
        } => {
            let vault_path = vault::resolve_vault()?;
            commands::links::run(
                &vault_path,
                note.as_deref(),
                commands::links::LinksOptions {
                    backlinks_only: backlinks,
                    forward_only: forward,
                    orphans,
                },
            )?;
        }

        Commands::Tags { notes } => {
            let vault_path = vault::resolve_vault()?;
            commands::tags::run(&vault_path, notes.as_deref())?;
        }

        Commands::Daily => {
            let vault_path = vault::resolve_vault()?;
            commands::daily::run(&vault_path)?;
        }

        Commands::Sync {
            subcommand,
            message,
        } => {
            let vault_path = vault::resolve_vault()?;
            let subcmd = match subcommand {
                None => commands::sync::SyncSubcommand::Default { message },
                Some(SyncCommands::Status) => commands::sync::SyncSubcommand::Status,
                Some(SyncCommands::Log) => commands::sync::SyncSubcommand::Log,
                Some(SyncCommands::Pull) => commands::sync::SyncSubcommand::Pull,
                Some(SyncCommands::Push) => commands::sync::SyncSubcommand::Push,
            };
            commands::sync::run(&vault_path, subcmd)?;
        }

        Commands::Rename { old, new } => {
            let vault_path = vault::resolve_vault()?;
            commands::rename::run(&vault_path, &old, &new)?;
        }

        Commands::Serve { subcommand, port } => {
            let vault_path = vault::resolve_vault()?;
            match subcommand {
                None => commands::serve::run(&vault_path, port)?,
                Some(ServeCommands::Kill) => commands::serve::kill(&vault_path)?,
            }
        }

        Commands::ServeFg { vault_path, port } => {
            let vp = std::path::PathBuf::from(&vault_path);
            let index = index::Index::build(&vp)?;
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(commands::serve::run_daemon(vp, port, index))?;
        }

        Commands::Context { subcommand } => {
            let subcmd = match subcommand {
                None => commands::context::ContextSubcommand::Show,
                Some(ContextCommands::Set { path }) => {
                    commands::context::ContextSubcommand::Set { path }
                }
                Some(ContextCommands::List) => commands::context::ContextSubcommand::List,
                Some(ContextCommands::Add { path }) => {
                    commands::context::ContextSubcommand::Add { path }
                }
                Some(ContextCommands::Remove { path }) => {
                    commands::context::ContextSubcommand::Remove { path }
                }
            };
            commands::context::run(subcmd)?;
        }
    }

    Ok(())
}
