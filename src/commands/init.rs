use anyhow::Result;

use crate::config::VaultConfig;
use crate::git;

pub fn run(path: Option<&str>) -> Result<()> {
    let vault_path = match path {
        Some(p) => std::path::PathBuf::from(p),
        None => std::env::current_dir()?,
    };

    // Create directory structure
    std::fs::create_dir_all(vault_path.join(".granite"))?;
    std::fs::create_dir_all(vault_path.join("notes").join("inbox"))?;
    std::fs::create_dir_all(vault_path.join("notes").join("daily"))?;
    std::fs::create_dir_all(vault_path.join("templates"))?;

    // Create vault config
    let vault_name = vault_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let config = VaultConfig {
        vault: crate::config::VaultSection { name: vault_name },
        ..VaultConfig::default()
    };
    config.save(&vault_path)?;

    // Create default template
    let default_template = vault_path.join("templates").join("default.md");
    if !default_template.exists() {
        std::fs::write(
            &default_template,
            "# {{ title }}\n\n",
        )?;
    }

    // Create .gitignore
    let gitignore = vault_path.join(".gitignore");
    if !gitignore.exists() {
        std::fs::write(&gitignore, ".granite/index.json\n")?;
    }

    // Create README
    let readme = vault_path.join("README.md");
    if !readme.exists() {
        let name = config.vault.name;
        std::fs::write(&readme, format!("# {}\n\nA Granite vault.\n", name))?;
    }

    // Initialize git repo
    git::init(&vault_path)?;

    println!("Initialized granite vault at {}", vault_path.display());
    Ok(())
}
