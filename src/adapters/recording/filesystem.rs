//! Recording adapter for the `FileSystem` port.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::{record_interaction, record_result};
use crate::cassette::recorder::CassetteRecorder;
use crate::ports::FileSystem;

/// Records filesystem interactions while delegating to an inner implementation.
pub struct RecordingFileSystem {
    inner: Box<dyn FileSystem>,
    recorder: Arc<Mutex<CassetteRecorder>>,
}

impl RecordingFileSystem {
    /// Creates a new recording filesystem wrapping the given implementation.
    pub fn new(inner: Box<dyn FileSystem>, recorder: Arc<Mutex<CassetteRecorder>>) -> Self {
        Self { inner, recorder }
    }
}

#[derive(Serialize)]
struct PathInput<'a> {
    path: &'a str,
}

#[derive(Serialize)]
struct WriteInput<'a> {
    path: &'a str,
    contents: &'a str,
}

impl FileSystem for RecordingFileSystem {
    fn read_to_string(
        &self,
        path: &Path,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.inner.read_to_string(path);
        let input = PathInput { path: &path.display().to_string() };
        record_result(&self.recorder, "fs", "read_to_string", &input, &result);
        result
    }

    fn write(
        &self,
        path: &Path,
        contents: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let result = self.inner.write(path, contents);
        let input = WriteInput { path: &path.display().to_string(), contents };
        record_result(&self.recorder, "fs", "write", &input, &result);
        result
    }

    fn exists(&self, path: &Path) -> bool {
        let result = self.inner.exists(path);
        let input = PathInput { path: &path.display().to_string() };
        record_interaction(&self.recorder, "fs", "exists", &input, &result);
        result
    }

    fn list_dir(
        &self,
        path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.inner.list_dir(path);
        let input = PathInput { path: &path.display().to_string() };
        record_result(&self.recorder, "fs", "list_dir", &input, &result);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::live::filesystem::LiveFileSystem;

    #[test]
    fn records_exists_interaction() {
        let dir = std::env::temp_dir().join("speck_rec_fs_test");
        std::fs::create_dir_all(&dir).unwrap();
        let cassette_path = dir.join("fs.cassette.yaml");

        let recorder = Arc::new(Mutex::new(CassetteRecorder::new(&cassette_path, "test", "abc")));

        // Scope the adapter so it's dropped before we try to unwrap
        {
            let fs = RecordingFileSystem::new(Box::new(LiveFileSystem), Arc::clone(&recorder));
            let _ = fs.exists(Path::new("/tmp"));
        }

        let recorder = Arc::try_unwrap(recorder).unwrap().into_inner().unwrap();
        recorder.finish().unwrap();

        let content = std::fs::read_to_string(&cassette_path).unwrap();
        assert!(content.contains("fs"));
        assert!(content.contains("exists"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
