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
        path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let path_str = path.to_string_lossy();
        let output = Command::new("git")
            .args(["ls-files", &path_str])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git ls-files failed: {stderr}").into());
        }
        let files = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gets_current_commit() {
        let git = LiveGitRepo;
        let result = git.current_commit();

        assert!(result.is_ok());
        let hash = result.unwrap();
        assert_eq!(hash.len(), 40);
    }

    #[test]
    fn gets_diff() {
        let git = LiveGitRepo;
        let result = git.diff();

        assert!(result.is_ok());
    }

    #[test]
    fn lists_files() {
        let git = LiveGitRepo;
        let result = git.list_files(Path::new("src"));

        assert!(result.is_ok());
        let files = result.unwrap();
        assert!(!files.is_empty());
        assert!(files.iter().any(|f| f.contains("main.rs") || f.contains("lib.rs")));
    }
}
