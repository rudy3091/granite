use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Global configuration stored at ~/.config/granite/config.toml or ~/.granite/config.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalConfig {
    #[serde(default)]
    pub default_vault: Option<String>,
    #[serde(default)]
    pub vaults: Vec<VaultEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntry {
    pub path: String,
    pub name: String,
}

/// Per-vault configuration stored at .granite/config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    #[serde(default)]
    pub vault: VaultSection,
    #[serde(default)]
    pub defaults: DefaultsSection,
    #[serde(default)]
    pub sync: SyncSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultSection {
    #[serde(default = "default_vault_name")]
    pub name: String,
}

fn default_vault_name() -> String {
    "my-vault".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsSection {
    #[serde(default = "default_editor")]
    pub editor: String,
    #[serde(default = "default_template")]
    pub template: String,
    #[serde(default = "default_daily_format")]
    pub daily_format: String,
}

impl Default for DefaultsSection {
    fn default() -> Self {
        Self {
            editor: default_editor(),
            template: default_template(),
            daily_format: default_daily_format(),
        }
    }
}

fn default_editor() -> String {
    std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string())
}

fn default_template() -> String {
    "default".to_string()
}

fn default_daily_format() -> String {
    "%Y-%m-%d".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSection {
    #[serde(default)]
    pub auto_commit: bool,
    #[serde(default = "default_remote")]
    pub remote: String,
}

impl Default for SyncSection {
    fn default() -> Self {
        Self {
            auto_commit: false,
            remote: default_remote(),
        }
    }
}

fn default_remote() -> String {
    "origin".to_string()
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            vault: VaultSection::default(),
            defaults: DefaultsSection::default(),
            sync: SyncSection::default(),
        }
    }
}

impl VaultConfig {
    pub fn load(vault_path: &Path) -> Result<Self> {
        let config_path = vault_path.join(".granite").join("config.toml");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read {}", config_path.display()))?;
            toml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", config_path.display()))
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, vault_path: &Path) -> Result<()> {
        let config_path = vault_path.join(".granite").join("config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }
}

impl GlobalConfig {
    pub fn load() -> Result<Self> {
        for path in Self::config_paths() {
            if path.exists() {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                return toml::from_str(&content)
                    .with_context(|| format!("Failed to parse {}", path.display()));
            }
        }
        Ok(Self::default())
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::primary_config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(config_dir) = dirs_config() {
            paths.push(config_dir.join("granite").join("config.toml"));
        }
        if let Some(home) = dirs_home() {
            paths.push(home.join(".granite").join("config.toml"));
        }
        paths
    }

    pub fn primary_config_path() -> PathBuf {
        if let Some(config_dir) = dirs_config() {
            config_dir.join("granite").join("config.toml")
        } else if let Some(home) = dirs_home() {
            home.join(".granite").join("config.toml")
        } else {
            PathBuf::from(".granite/config.toml")
        }
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

fn dirs_config() -> Option<PathBuf> {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| dirs_home().map(|h| h.join(".config")))
}
