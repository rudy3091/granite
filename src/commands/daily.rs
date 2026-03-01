use anyhow::Result;
use std::path::Path;

use crate::commands::new;
use crate::config::VaultConfig;

pub fn run(vault_path: &Path) -> Result<()> {
    let config = VaultConfig::load(vault_path)?;
    let today = chrono::Local::now().format(&config.defaults.daily_format).to_string();
    let daily_path = vault_path.join("notes").join("daily").join(format!("{}.md", today));

    if daily_path.exists() {
        // Open existing daily note
        println!("Opening daily note: {}", daily_path.display());
        let status = std::process::Command::new(&config.defaults.editor)
            .arg(&daily_path)
            .status()?;
        if !status.success() {
            anyhow::bail!("Editor exited with non-zero status");
        }
    } else {
        // Create new daily note
        new::run(
            vault_path,
            new::NewOptions {
                title: Some(today),
                no_edit: false,
                template: None,
                dir: Some("daily".to_string()),
                content: None,
            },
        )?;
    }

    Ok(())
}
