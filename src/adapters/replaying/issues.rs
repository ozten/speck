//! Replaying adapter for the `IssueTracker` port.

use std::sync::{Arc, Mutex};

use super::{next_output, replay_result};
use crate::cassette::replayer::CassetteReplayer;
use crate::ports::{Issue, IssueTracker};

/// Serves recorded issue tracker results from a cassette.
pub struct ReplayingIssueTracker {
    replayer: Option<Arc<Mutex<CassetteReplayer>>>,
}

impl ReplayingIssueTracker {
    /// Create a replaying issue tracker backed by the given replayer.
    #[must_use]
    pub fn new(replayer: Arc<Mutex<CassetteReplayer>>) -> Self {
        Self { replayer: Some(replayer) }
    }

    /// Create a replaying issue tracker with no cassette. Panics when called.
    #[must_use]
    pub fn unconfigured() -> Self {
        Self { replayer: None }
    }
}

impl IssueTracker for ReplayingIssueTracker {
    fn create_issue(
        &self,
        _title: &str,
        _body: &str,
    ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
        let output = next_output(self.replayer.as_ref(), "issues", "create_issue");
        replay_result(output)
    }

    fn update_issue(
        &self,
        _id: &str,
        _title: Option<&str>,
        _body: Option<&str>,
        _status: Option<&str>,
    ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
        let output = next_output(self.replayer.as_ref(), "issues", "update_issue");
        replay_result(output)
    }

    fn list_issues(
        &self,
        _status: Option<&str>,
    ) -> Result<Vec<Issue>, Box<dyn std::error::Error + Send + Sync>> {
        let output = next_output(self.replayer.as_ref(), "issues", "list_issues");
        replay_result(output)
    }
}
