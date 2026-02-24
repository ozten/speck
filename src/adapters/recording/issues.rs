//! Recording adapter for the `IssueTracker` port.

use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::record_result;
use crate::cassette::recorder::CassetteRecorder;
use crate::ports::{Issue, IssueTracker};

/// Records issue tracker interactions while delegating to an inner implementation.
pub struct RecordingIssueTracker {
    inner: Box<dyn IssueTracker>,
    recorder: Arc<Mutex<CassetteRecorder>>,
}

impl RecordingIssueTracker {
    /// Creates a new recording issue tracker wrapping the given implementation.
    pub fn new(inner: Box<dyn IssueTracker>, recorder: Arc<Mutex<CassetteRecorder>>) -> Self {
        Self { inner, recorder }
    }
}

#[derive(Serialize)]
struct CreateIssueInput<'a> {
    title: &'a str,
    body: &'a str,
}

#[derive(Serialize)]
struct UpdateIssueInput<'a> {
    id: &'a str,
    title: Option<&'a str>,
    body: Option<&'a str>,
    status: Option<&'a str>,
}

#[derive(Serialize)]
struct ListIssuesInput<'a> {
    status: Option<&'a str>,
}

#[derive(Serialize)]
struct GetIssueInput<'a> {
    id: &'a str,
}

impl IssueTracker for RecordingIssueTracker {
    fn create_issue(
        &self,
        title: &str,
        body: &str,
    ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.inner.create_issue(title, body);
        let input = CreateIssueInput { title, body };
        record_result(&self.recorder, "issues", "create_issue", &input, &result);
        result
    }

    fn update_issue(
        &self,
        id: &str,
        title: Option<&str>,
        body: Option<&str>,
        status: Option<&str>,
    ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.inner.update_issue(id, title, body, status);
        let input = UpdateIssueInput { id, title, body, status };
        record_result(&self.recorder, "issues", "update_issue", &input, &result);
        result
    }

    fn list_issues(
        &self,
        status: Option<&str>,
    ) -> Result<Vec<Issue>, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.inner.list_issues(status);
        let input = ListIssuesInput { status };
        record_result(&self.recorder, "issues", "list_issues", &input, &result);
        result
    }

    fn get_issue(&self, id: &str) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.inner.get_issue(id);
        let input = GetIssueInput { id };
        record_result(&self.recorder, "issues", "get_issue", &input, &result);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeIssueTracker;

    impl IssueTracker for FakeIssueTracker {
        fn create_issue(
            &self,
            title: &str,
            body: &str,
        ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
            Ok(Issue {
                id: "fake-1".into(),
                title: title.into(),
                body: body.into(),
                status: "open".into(),
            })
        }

        fn update_issue(
            &self,
            id: &str,
            title: Option<&str>,
            _body: Option<&str>,
            _status: Option<&str>,
        ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
            Ok(Issue {
                id: id.into(),
                title: title.unwrap_or("updated").into(),
                body: String::new(),
                status: "open".into(),
            })
        }

        fn list_issues(
            &self,
            _status: Option<&str>,
        ) -> Result<Vec<Issue>, Box<dyn std::error::Error + Send + Sync>> {
            Ok(vec![])
        }

        fn get_issue(&self, id: &str) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
            Ok(Issue {
                id: id.into(),
                title: "Fake issue".into(),
                body: String::new(),
                status: "open".into(),
            })
        }
    }

    #[test]
    fn records_create_issue_interaction() {
        let dir = std::env::temp_dir().join("speck_rec_issues_test");
        std::fs::create_dir_all(&dir).unwrap();
        let cassette_path = dir.join("issues.cassette.yaml");

        let recorder = Arc::new(Mutex::new(CassetteRecorder::new(&cassette_path, "test", "abc")));

        // Scope the adapter so it's dropped before we try to unwrap
        {
            let tracker =
                RecordingIssueTracker::new(Box::new(FakeIssueTracker), Arc::clone(&recorder));
            let _ = tracker.create_issue("Test Issue", "Test body");
        }

        let recorder = Arc::try_unwrap(recorder).unwrap().into_inner().unwrap();
        recorder.finish().unwrap();

        let content = std::fs::read_to_string(&cassette_path).unwrap();
        assert!(content.contains("issues"));
        assert!(content.contains("create_issue"));
        assert!(content.contains("Test Issue"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
