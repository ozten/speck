//! Issue tracker port for managing work items.

use serde::{Deserialize, Serialize};

/// Represents an issue in the tracker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    /// The unique identifier for this issue.
    pub id: String,
    /// The issue title.
    pub title: String,
    /// The issue body / description.
    pub body: String,
    /// The current status (e.g. "open", "closed").
    pub status: String,
}

/// Manages issues in an external tracker.
///
/// Abstracting issue tracking allows deterministic replay and testing
/// without touching a real issue tracker API.
pub trait IssueTracker: Send + Sync {
    /// Creates a new issue and returns it with its assigned ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the issue cannot be created.
    fn create_issue(
        &self,
        title: &str,
        body: &str,
    ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>>;

    /// Updates an existing issue's title, body, or status.
    ///
    /// # Errors
    ///
    /// Returns an error if the issue cannot be found or updated.
    fn update_issue(
        &self,
        id: &str,
        title: Option<&str>,
        body: Option<&str>,
        status: Option<&str>,
    ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>>;

    /// Lists issues, optionally filtered by status.
    ///
    /// # Errors
    ///
    /// Returns an error if the issues cannot be listed.
    fn list_issues(
        &self,
        status: Option<&str>,
    ) -> Result<Vec<Issue>, Box<dyn std::error::Error + Send + Sync>>;
}
