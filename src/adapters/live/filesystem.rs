//! Live filesystem adapter using `std::fs`.

use std::path::Path;

use crate::ports::filesystem::FileSystem;

/// Live filesystem adapter backed by real disk I/O.
pub struct LiveFileSystem;

impl FileSystem for LiveFileSystem {
    fn read_to_string(
        &self,
        path: &Path,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(std::fs::read_to_string(path)?)
    }

    fn write(
        &self,
        path: &Path,
        contents: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(std::fs::write(path, contents)?)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn list_dir(
        &self,
        path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                entries.push(name.to_string());
            }
        }
        entries.sort();
        Ok(entries)
    }
}
