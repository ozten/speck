//! Live adapter for the `IssueTracker` port.
//!
//! Shells out to the `bd` CLI for issue management.

use crate::ports::{Issue, IssueTracker};
use std::process::Command;

/// Represents a bd CLI issue in JSON output.
#[derive(serde::Deserialize)]
struct BdIssue {
    id: String,
    title: String,
    #[serde(default)]
    description: Option<String>,
    status: String,
}

impl From<BdIssue> for Issue {
    fn from(bd: BdIssue) -> Self {
        Issue {
            id: bd.id,
            title: bd.title,
            body: bd.description.unwrap_or_default(),
            status: bd.status,
        }
    }
}

/// Live issue tracker that shells out to the `bd` CLI.
pub struct LiveIssueTracker;

impl IssueTracker for LiveIssueTracker {
    fn create_issue(
        &self,
        title: &str,
        body: &str,
    ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
        // Create the issue and capture the ID
        let create_output = Command::new("bd")
            .args(["create", title, "-d", body, "--silent"])
            .output()
            .map_err(|e| format!("Failed to run bd: {e}. Is bd installed?"))?;

        if !create_output.status.success() {
            let stderr = String::from_utf8_lossy(&create_output.stderr);
            return Err(format!("bd create failed: {stderr}").into());
        }

        let issue_id = String::from_utf8_lossy(&create_output.stdout).trim().to_string();
        if issue_id.is_empty() {
            return Err("bd create returned empty ID".into());
        }

        // Fetch the created issue details
        let show_output = Command::new("bd")
            .args(["show", &issue_id, "--json"])
            .output()
            .map_err(|e| format!("Failed to run bd show: {e}"))?;

        if !show_output.status.success() {
            let stderr = String::from_utf8_lossy(&show_output.stderr);
            return Err(format!("bd show failed: {stderr}").into());
        }

        let bd_issue: BdIssue = serde_json::from_slice(&show_output.stdout)
            .map_err(|e| format!("Failed to parse bd show JSON: {e}"))?;

        Ok(bd_issue.into())
    }

    fn update_issue(
        &self,
        id: &str,
        title: Option<&str>,
        body: Option<&str>,
        status: Option<&str>,
    ) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
        // Handle status=closed via bd close
        if status == Some("closed") {
            let close_output = Command::new("bd")
                .args(["close", id])
                .output()
                .map_err(|e| format!("Failed to run bd close: {e}"))?;

            if !close_output.status.success() {
                let stderr = String::from_utf8_lossy(&close_output.stderr);
                return Err(format!("bd close failed: {stderr}").into());
            }
        }

        // Handle title/body updates via bd update
        let mut update_args: Vec<String> = vec!["update".to_string(), id.to_string()];
        if let Some(t) = title {
            update_args.push("--title".to_string());
            update_args.push(t.to_string());
        }
        if let Some(b) = body {
            update_args.push("-d".to_string());
            update_args.push(b.to_string());
        }

        if update_args.len() > 2 {
            let update_output = Command::new("bd")
                .args(&update_args)
                .output()
                .map_err(|e| format!("Failed to run bd update: {e}"))?;

            if !update_output.status.success() {
                let stderr = String::from_utf8_lossy(&update_output.stderr);
                return Err(format!("bd update failed: {stderr}").into());
            }
        }

        // Fetch the updated issue
        let show_output = Command::new("bd")
            .args(["show", id, "--json"])
            .output()
            .map_err(|e| format!("Failed to run bd show: {e}"))?;

        if !show_output.status.success() {
            let stderr = String::from_utf8_lossy(&show_output.stderr);
            return Err(format!("bd show failed: {stderr}").into());
        }

        let bd_issue: BdIssue = serde_json::from_slice(&show_output.stdout)
            .map_err(|e| format!("Failed to parse bd show JSON: {e}"))?;

        Ok(bd_issue.into())
    }

    fn get_issue(&self, id: &str) -> Result<Issue, Box<dyn std::error::Error + Send + Sync>> {
        let show_output = Command::new("bd")
            .args(["show", id, "--json"])
            .output()
            .map_err(|e| format!("Failed to run bd show: {e}"))?;

        if !show_output.status.success() {
            let stderr = String::from_utf8_lossy(&show_output.stderr);
            return Err(format!("bd show failed: {stderr}").into());
        }

        let bd_issue: BdIssue = serde_json::from_slice(&show_output.stdout)
            .map_err(|e| format!("Failed to parse bd show JSON: {e}"))?;

        Ok(bd_issue.into())
    }

    fn list_issues(
        &self,
        status: Option<&str>,
    ) -> Result<Vec<Issue>, Box<dyn std::error::Error + Send + Sync>> {
        let mut args = vec!["list", "--json", "--limit", "0"];

        // If status is "all" or user wants all, add --all flag
        if status == Some("all") {
            args.push("--all");
        }

        let output = Command::new("bd")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to run bd: {e}. Is bd installed?"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("bd list failed: {stderr}").into());
        }

        let bd_issues: Vec<BdIssue> = serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("Failed to parse bd list JSON: {e}"))?;

        let mut issues: Vec<Issue> = bd_issues.into_iter().map(Issue::from).collect();

        // Filter by status if a specific status was requested (not "all")
        if let Some(s) = status {
            if s != "all" {
                issues.retain(|i| i.status == s);
            }
        }

        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bd_issue_converts_to_issue() {
        let bd = BdIssue {
            id: "speck-1".to_string(),
            title: "Test issue".to_string(),
            description: Some("A description".to_string()),
            status: "open".to_string(),
        };
        let issue: Issue = bd.into();
        assert_eq!(issue.id, "speck-1");
        assert_eq!(issue.title, "Test issue");
        assert_eq!(issue.body, "A description");
        assert_eq!(issue.status, "open");
    }

    #[test]
    fn bd_issue_with_no_description_defaults_to_empty() {
        let bd = BdIssue {
            id: "speck-2".to_string(),
            title: "No body".to_string(),
            description: None,
            status: "closed".to_string(),
        };
        let issue: Issue = bd.into();
        assert_eq!(issue.body, "");
    }
}
