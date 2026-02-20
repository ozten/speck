//! Replaying adapter for the `GitRepo` port.

use std::path::Path;
use std::sync::Mutex;

use crate::cassette::replayer::CassetteReplayer;
use crate::ports::git::GitRepo;

/// Replays recorded git operations from a cassette.
pub struct ReplayingGitRepo {
    replayer: Mutex<CassetteReplayer>,
}

impl ReplayingGitRepo {
    /// Creates a new replaying git repo from a cassette replayer.
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

impl GitRepo for ReplayingGitRepo {
    fn current_commit(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("git", "current_commit");
            interaction.output.clone()
        };
        extract_result(&output, "git::current_commit")
    }

    fn diff(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("git", "diff");
            interaction.output.clone()
        };
        extract_result(&output, "git::diff")
    }

    fn list_files(
        &self,
        _path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("git", "list_files");
            interaction.output.clone()
        };
        extract_result(&output, "git::list_files")
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
    fn replaying_git_current_commit() {
        let replayer = make_replayer(vec![Interaction {
            seq: 0,
            port: "git".into(),
            method: "current_commit".into(),
            input: json!({}),
            output: json!({"ok": "abc123def"}),
        }]);
        let git = ReplayingGitRepo::new(replayer);
        assert_eq!(git.current_commit().unwrap(), "abc123def");
    }

    #[test]
    fn replaying_git_diff() {
        let replayer = make_replayer(vec![Interaction {
            seq: 0,
            port: "git".into(),
            method: "diff".into(),
            input: json!({}),
            output: json!({"ok": "--- a/file\n+++ b/file"}),
        }]);
        let git = ReplayingGitRepo::new(replayer);
        assert!(git.diff().unwrap().contains("--- a/file"));
    }
}
