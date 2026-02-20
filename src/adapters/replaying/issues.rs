//! Replaying adapter for the `IssueTracker` port.

use std::sync::Mutex;

use crate::cassette::replayer::CassetteReplayer;
use crate::ports::issues::{Issue, IssueTracker};

/// Replays recorded issue tracker operations from a cassette.
pub struct ReplayingIssueTracker {
    replayer: Mutex<CassetteReplayer>,
}

impl ReplayingIssueTracker {
    /// Creates a new replaying issue tracker from a cassette replayer.
    #[must_use]
    pub fn new(replayer: CassetteReplayer) -> Self {
        Self { replayer: Mutex::new(replayer) }
    }
}

/// Extracts a Result from a cassette output JSON value.
fn extract_result<T: serde::de::DeserializeOwned>(
    output: &serde_json::Value,
    context: &str,
) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(err) = output.get("err") {
        let msg = err.as_str().unwrap_or("unknown error").to_string();
        return Err(msg.into());
    }
    let value = output.get("ok").unwrap_or(output);
    serde_json::from_value(value.clone())
        .map_err(|e| format!("{context}: failed to deserialize: {e}").into())
}

impl IssueTracker for ReplayingIssueTracker {
    fn create_issue(
        &self,
        _title: &str,
        _body: &str,
    ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("issues", "create_issue");
            interaction.output.clone()
        };
        extract_result(&output, "issues::create_issue")
    }

    fn update_issue(
        &self,
        _id: &str,
        _title: Option<&str>,
        _body: Option<&str>,
        _status: Option<&str>,
    ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("issues", "update_issue");
            interaction.output.clone()
        };
        extract_result(&output, "issues::update_issue")
    }

    fn list_issues(
        &self,
        _status: Option<&str>,
    ) -> Result<Vec<Issue>, Box<dyn std::error::Error + Send + Sync>> {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("issues", "list_issues");
            interaction.output.clone()
        };
        extract_result(&output, "issues::list_issues")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
    use chrono::Utc;
    use serde_json::json;

    fn make_replayer(interactions: Vec<Interaction>) -> CassetteReplayer {
        let cassette = Cassette {
            name: "test".into(),
            recorded_at: Utc::now(),
            commit: "abc".into(),
            interactions,
        };
        CassetteReplayer::new(&cassette)
    }

    #[test]
    fn replaying_issue_tracker_create() {
        let replayer = make_replayer(vec![Interaction {
            seq: 0,
            port: "issues".into(),
            method: "create_issue".into(),
            input: json!({"title": "Bug", "body": "Broken"}),
            output: json!({"ok": {"id": "ISS-1", "title": "Bug", "body": "Broken", "status": "open"}}),
        }]);
        let tracker = ReplayingIssueTracker::new(replayer);
        let issue = tracker.create_issue("Bug", "Broken").unwrap();
        assert_eq!(issue.id, "ISS-1");
        assert_eq!(issue.status, "open");
    }

    #[test]
    fn replaying_issue_tracker_list() {
        let replayer = make_replayer(vec![Interaction {
            seq: 0,
            port: "issues".into(),
            method: "list_issues".into(),
            input: json!({"status": "open"}),
            output: json!({"ok": [
                {"id": "ISS-1", "title": "Bug", "body": "Broken", "status": "open"},
                {"id": "ISS-2", "title": "Feature", "body": "New thing", "status": "open"}
            ]}),
        }]);
        let tracker = ReplayingIssueTracker::new(replayer);
        let issues = tracker.list_issues(Some("open")).unwrap();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].id, "ISS-1");
    }
}
