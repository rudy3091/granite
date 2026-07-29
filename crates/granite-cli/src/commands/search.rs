use anyhow::Result;
use regex::RegexBuilder;
use std::path::Path;
use walkdir::WalkDir;

pub struct SearchOptions {
    pub case_sensitive: bool,
}

pub fn run(vault_path: &Path, pattern: &str, opts: SearchOptions) -> Result<()> {
    let re = RegexBuilder::new(pattern)
        .case_insensitive(!opts.case_sensitive)
        .build()?;

    let notes_dir = vault_path.join("notes");
    if !notes_dir.exists() {
        println!("No notes directory found.");
        return Ok(());
    }

    let mut match_count = 0;
    let mut file_count = 0;

    for entry in WalkDir::new(&notes_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = path
            .strip_prefix(vault_path)
            .unwrap_or(path)
            .to_string_lossy();

        let mut file_has_match = false;
        for (line_num, line) in content.lines().enumerate() {
            if re.is_match(line) {
                if !file_has_match {
                    println!("\n{}:", rel_path);
                    file_has_match = true;
                    file_count += 1;
                }
                println!("  {}:{}", line_num + 1, line);
                match_count += 1;
            }
        }
    }

    if match_count == 0 {
        println!("No matches found for '{}'", pattern);
    } else {
        println!(
            "\n{} match(es) in {} file(s)",
            match_count, file_count
        );
    }

    Ok(())
}
