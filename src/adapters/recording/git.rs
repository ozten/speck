//! Recording adapter for the `GitRepo` port.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::record_result;
use crate::cassette::recorder::CassetteRecorder;
use crate::ports::GitRepo;

/// Records git interactions while delegating to an inner implementation.
pub struct RecordingGitRepo {
    inner: Box<dyn GitRepo>,
    recorder: Arc<Mutex<CassetteRecorder>>,
}

impl RecordingGitRepo {
    /// Creates a new recording git repo wrapping the given implementation.
    pub fn new(inner: Box<dyn GitRepo>, recorder: Arc<Mutex<CassetteRecorder>>) -> Self {
        Self { inner, recorder }
    }
}

#[derive(Serialize)]
struct PathInput<'a> {
    path: &'a str,
}

impl GitRepo for RecordingGitRepo {
    fn current_commit(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.inner.current_commit();
        record_result(&self.recorder, "git", "current_commit", &(), &result);
        result
    }

    fn diff(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.inner.diff();
        record_result(&self.recorder, "git", "diff", &(), &result);
        result
    }

    fn list_files(
        &self,
        path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.inner.list_files(path);
        let input = PathInput { path: &path.display().to_string() };
        record_result(&self.recorder, "git", "list_files", &input, &result);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::live::git::LiveGitRepo;

    #[test]
    fn records_current_commit_interaction() {
        let dir = std::env::temp_dir().join("speck_rec_git_test");
        std::fs::create_dir_all(&dir).unwrap();
        let cassette_path = dir.join("git.cassette.yaml");

        let recorder = Arc::new(Mutex::new(CassetteRecorder::new(&cassette_path, "test", "abc")));

        // Scope the adapter so it's dropped before we try to unwrap
        {
            let git = RecordingGitRepo::new(Box::new(LiveGitRepo), Arc::clone(&recorder));
            let _ = git.current_commit();
        }

        let recorder = Arc::try_unwrap(recorder).unwrap().into_inner().unwrap();
        recorder.finish().unwrap();

        let content = std::fs::read_to_string(&cassette_path).unwrap();
        assert!(content.contains("git"));
        assert!(content.contains("current_commit"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
