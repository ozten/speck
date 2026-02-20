//! Filesystem port for file I/O operations.

use std::path::Path;

/// Provides filesystem access for reading and writing files.
///
/// Abstracting the filesystem allows deterministic replay and testing
/// without touching the real disk.
pub trait FileSystem: Send + Sync {
    /// Reads the entire contents of a file as a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns an error if the file does not exist or is not valid UTF-8.
    fn read_to_string(
        &self,
        path: &Path,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;

    /// Writes the given contents to a file, creating or overwriting it.
    ///
    /// # Errors
    ///
    /// Returns an error if the write fails (permissions, disk full, etc.).
    fn write(
        &self,
        path: &Path,
        contents: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Returns `true` if the path exists on the filesystem.
    fn exists(&self, path: &Path) -> bool;

    /// Lists the entries in a directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is not a directory or cannot be read.
    fn list_dir(
        &self,
        path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>>;
}
