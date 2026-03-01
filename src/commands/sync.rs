use anyhow::Result;
use std::path::Path;

use crate::config::VaultConfig;
use crate::git;

pub enum SyncSubcommand {
    Default { message: Option<String> },
    Status,
    Log,
    Pull,
    Push,
}

pub fn run(vault_path: &Path, subcmd: SyncSubcommand) -> Result<()> {
    let config = VaultConfig::load(vault_path)?;
    let remote = &config.sync.remote;

    match subcmd {
        SyncSubcommand::Default { message } => {
            let msg = message.unwrap_or_else(|| {
                let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                format!("vault sync: {}", ts)
            });

            println!("Adding notes...");
            git::add(vault_path, &["notes/"])?;

            println!("Committing: {}", msg);
            match git::commit(vault_path, &msg) {
                Ok(_) => {}
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("nothing to commit") {
                        println!("Nothing to commit.");
                    } else {
                        return Err(e);
                    }
                }
            }

            println!("Pulling from {}...", remote);
            match git::pull(vault_path, remote) {
                Ok(output) => {
                    if !output.trim().is_empty() {
                        println!("{}", output.trim());
                    }
                }
                Err(e) => {
                    eprintln!("Pull failed (no remote?): {}", e);
                }
            }

            println!("Pushing to {}...", remote);
            match git::push(vault_path, remote) {
                Ok(output) => {
                    if !output.trim().is_empty() {
                        println!("{}", output.trim());
                    }
                }
                Err(e) => {
                    eprintln!("Push failed (no remote?): {}", e);
                }
            }

            println!("Sync complete.");
        }

        SyncSubcommand::Status => {
            let output = git::status(vault_path)?;
            if output.trim().is_empty() {
                println!("Nothing to commit, working tree clean.");
            } else {
                println!("{}", output);
            }
        }

        SyncSubcommand::Log => {
            let output = git::log(vault_path, 10)?;
            println!("{}", output);
        }

        SyncSubcommand::Pull => {
            println!("Pulling from {}...", remote);
            let output = git::pull(vault_path, remote)?;
            println!("{}", output.trim());
        }

        SyncSubcommand::Push => {
            println!("Pushing to {}...", remote);
            let output = git::push(vault_path, remote)?;
            println!("{}", output.trim());
        }
    }

    Ok(())
}
