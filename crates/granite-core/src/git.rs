use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// Run a git command in the given directory and return stdout
fn run_git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .context("Failed to execute git")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", args.join(" "), stderr.trim())
    }
}

/// Initialize a git repository if not already one
pub fn init(dir: &Path) -> Result<()> {
    if dir.join(".git").exists() {
        return Ok(());
    }
    run_git(dir, &["init"])?;
    Ok(())
}

/// Git add files
pub fn add(dir: &Path, paths: &[&str]) -> Result<()> {
    let mut args = vec!["add"];
    args.extend_from_slice(paths);
    run_git(dir, &args)?;
    Ok(())
}

/// Git commit with message
pub fn commit(dir: &Path, message: &str) -> Result<()> {
    run_git(dir, &["commit", "-m", message])?;
    Ok(())
}

/// Git pull --rebase
pub fn pull(dir: &Path, remote: &str) -> Result<String> {
    run_git(dir, &["pull", "--rebase", remote])
}

/// Git push
pub fn push(dir: &Path, remote: &str) -> Result<String> {
    run_git(dir, &["push", remote])
}

/// Git status
pub fn status(dir: &Path) -> Result<String> {
    run_git(dir, &["status", "--short"])
}

/// Git log (recent commits)
pub fn log(dir: &Path, count: usize) -> Result<String> {
    let n = format!("-{}", count);
    run_git(dir, &["log", "--oneline", &n])
}
