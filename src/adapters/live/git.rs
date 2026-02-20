//! Live git adapter using `git` CLI commands.

use std::path::Path;
use std::process::Command;

use crate::ports::git::GitRepo;

/// Live git adapter that shells out to the `git` CLI.
pub struct LiveGitRepo;

impl GitRepo for LiveGitRepo {
    fn current_commit(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let output = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git rev-parse HEAD failed: {stderr}").into());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn diff(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let output = Command::new("git").args(["diff", "HEAD"]).output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git diff HEAD failed: {stderr}").into());
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn list_files(
        &self,
        _path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let output = Command::new("git").args(["ls-files"]).output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git ls-files failed: {stderr}").into());
        }
        let files = String::from_utf8_lossy(&output.stdout).lines().map(String::from).collect();
        Ok(files)
    }
}
