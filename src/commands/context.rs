use anyhow::{bail, Result};
use std::path::PathBuf;

use crate::config::{GlobalConfig, VaultEntry};
use crate::vault;

pub enum ContextSubcommand {
    Show,
    Set { path: String },
    List,
    Add { path: String },
    Remove { path: String },
}

pub fn run(subcmd: ContextSubcommand) -> Result<()> {
    match subcmd {
        ContextSubcommand::Show => {
            match vault::resolve_vault() {
                Ok(path) => println!("Active vault: {}", path.display()),
                Err(e) => println!("No active vault: {}", e),
            }
        }

        ContextSubcommand::Set { path } => {
            let abs_path = to_absolute(&path)?;
            if !vault::is_vault(&abs_path) {
                bail!("{} is not a granite vault (missing .granite/)", abs_path.display());
            }
            let mut config = GlobalConfig::load()?;
            config.default_vault = Some(abs_path.to_string_lossy().to_string());
            config.save()?;
            println!("Default vault set to: {}", abs_path.display());
        }

        ContextSubcommand::List => {
            let config = GlobalConfig::load()?;
            if config.vaults.is_empty() {
                println!("No registered vaults.");
            } else {
                println!("Registered vaults:");
                for v in &config.vaults {
                    let marker = if config.default_vault.as_deref() == Some(&v.path) {
                        " (default)"
                    } else {
                        ""
                    };
                    println!("  {} — {}{}", v.name, v.path, marker);
                }
            }
        }

        ContextSubcommand::Add { path } => {
            let abs_path = to_absolute(&path)?;
            if !vault::is_vault(&abs_path) {
                bail!("{} is not a granite vault (missing .granite/)", abs_path.display());
            }
            let mut config = GlobalConfig::load()?;
            let path_str = abs_path.to_string_lossy().to_string();

            // Check for duplicates
            if config.vaults.iter().any(|v| v.path == path_str) {
                bail!("Vault already registered: {}", path_str);
            }

            let name = abs_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            config.vaults.push(VaultEntry {
                path: path_str.clone(),
                name,
            });

            // Set as default if it's the first vault
            if config.default_vault.is_none() {
                config.default_vault = Some(path_str.clone());
            }

            config.save()?;
            println!("Registered vault: {}", path_str);
        }

        ContextSubcommand::Remove { path } => {
            let abs_path = to_absolute(&path)?;
            let path_str = abs_path.to_string_lossy().to_string();
            let mut config = GlobalConfig::load()?;

            let before_len = config.vaults.len();
            config.vaults.retain(|v| v.path != path_str);

            if config.vaults.len() == before_len {
                bail!("Vault not found in registry: {}", path_str);
            }

            // Clear default if it was the removed vault
            if config.default_vault.as_deref() == Some(&path_str) {
                config.default_vault = config.vaults.first().map(|v| v.path.clone());
            }

            config.save()?;
            println!("Unregistered vault: {}", path_str);
        }
    }

    Ok(())
}

fn to_absolute(path: &str) -> Result<PathBuf> {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        Ok(p)
    } else {
        let cwd = std::env::current_dir()?;
        Ok(cwd.join(p).canonicalize().unwrap_or_else(|_| cwd.join(path)))
    }
}
