//! Live adapter for the `IssueTracker` port.

use crate::ports::{Issue, IssueTracker};

/// Live issue tracker adapter.
///
/// Currently a stub that returns errors. A real implementation would
/// integrate with GitHub, Jira, Linear, or another issue tracking system.
pub struct LiveIssueTracker;

impl IssueTracker for LiveIssueTracker {
    fn create_issue(
        &self,
        _title: &str,
        _body: &str,
    ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
        Err("Live issue tracking not yet implemented".into())
    }

    fn update_issue(
        &self,
        _id: &str,
        _title: Option<&str>,
        _body: Option<&str>,
        _status: Option<&str>,
    ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
        Err("Live issue tracking not yet implemented".into())
    }

    fn list_issues(
        &self,
        _status: Option<&str>,
    ) -> Result<Vec<Issue>, Box<dyn std::error::Error + Send + Sync>> {
        Err("Live issue tracking not yet implemented".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_issue_returns_not_implemented() {
        let tracker = LiveIssueTracker;
        let result = tracker.create_issue("Test", "Body");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not yet implemented"));
    }
}
