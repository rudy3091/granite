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
    },

    /// Open a note in $EDITOR
    Edit {
        /// Fuzzy search query
        query: String,
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
        } => {
            let vault_path = vault::resolve_vault()?;
            commands::new::run(
                &vault_path,
                commands::new::NewOptions {
                    title,
                    no_edit,
                    template,
                    dir,
                },
            )?;
        }

        Commands::Edit { query } => {
            let vault_path = vault::resolve_vault()?;
            commands::edit::run(&vault_path, &query)?;
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
