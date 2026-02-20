//! Git repository port for version-control queries.

use std::path::Path;

/// Provides read access to a git repository.
///
/// Abstracting git access allows deterministic replay and testing
/// without requiring a real repository.
pub trait GitRepo: Send + Sync {
    /// Returns the hash of the current HEAD commit.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository has no commits or is invalid.
    fn current_commit(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;

    /// Returns the diff of the working tree against HEAD (or between two refs).
    ///
    /// # Errors
    ///
    /// Returns an error if the diff cannot be computed.
    fn diff(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;

    /// Lists all tracked files in the repository.
    ///
    /// # Errors
    ///
    /// Returns an error if the file list cannot be retrieved.
    fn list_files(
        &self,
        path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>>;
}
