use anyhow::{bail, Result};
use std::path::PathBuf;

use crate::config::VaultConfig;
use crate::frontmatter;
use crate::vault;

pub struct NewOptions {
    pub title: Option<String>,
    pub no_edit: bool,
    pub template: Option<String>,
    pub dir: Option<String>,
}

pub fn run(vault_path: &std::path::Path, opts: NewOptions) -> Result<()> {
    let config = VaultConfig::load(vault_path)?;

    let title = match opts.title {
        Some(t) => t,
        None => {
            let now = chrono::Local::now();
            now.format("%Y%m%d%H%M%S").to_string()
        }
    };

    let filename = format!("{}.md", vault::to_kebab_case(&title));

    let notes_dir = match &opts.dir {
        Some(subdir) => vault_path.join("notes").join(subdir),
        None => vault_path.join("notes"),
    };
    std::fs::create_dir_all(&notes_dir)?;

    let note_path = notes_dir.join(&filename);
    if note_path.exists() {
        bail!("Note already exists: {}", note_path.display());
    }

    // Build content from template or default
    let template_name = opts
        .template
        .as_deref()
        .unwrap_or(&config.defaults.template);
    let template_path = vault_path
        .join("templates")
        .join(format!("{}.md", template_name));

    let body = if template_path.exists() {
        let tmpl = std::fs::read_to_string(&template_path)?;
        tmpl.replace("{{ title }}", &title)
    } else {
        format!("# {}\n\n", title)
    };

    let fm = frontmatter::new_frontmatter(&title);
    let content = frontmatter::serialize(&fm, &body);
    std::fs::write(&note_path, &content)?;

    println!("Created {}", note_path.display());

    if !opts.no_edit {
        open_editor(&config.defaults.editor, &note_path)?;
    }

    Ok(())
}

fn open_editor(editor: &str, path: &PathBuf) -> Result<()> {
    let status = std::process::Command::new(editor)
        .arg(path)
        .status()?;

    if !status.success() {
        bail!("Editor exited with non-zero status");
    }
    Ok(())
}
