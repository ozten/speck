//! Replaying adapter for the `FileSystem` port.

use std::path::Path;
use std::sync::Mutex;

use crate::cassette::replayer::CassetteReplayer;
use crate::ports::filesystem::FileSystem;

/// Replays recorded filesystem operations from a cassette.
pub struct ReplayingFileSystem {
    replayer: Mutex<CassetteReplayer>,
}

impl ReplayingFileSystem {
    /// Creates a new replaying filesystem from a cassette replayer.
    #[must_use]
    pub fn new(replayer: CassetteReplayer) -> Self {
        Self { replayer: Mutex::new(replayer) }
    }
}

/// Extracts a Result from a cassette output JSON value.
///
/// Expects `{"ok": <value>}` or `{"err": "message"}`.
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

impl FileSystem for ReplayingFileSystem {
    fn read_to_string(
        &self,
        _path: &Path,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("fs", "read_to_string");
            interaction.output.clone()
        };
        extract_result(&output, "fs::read_to_string")
    }

    fn write(
        &self,
        _path: &Path,
        _contents: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("fs", "write");
            interaction.output.clone()
        };
        if let Some(err) = output.get("err") {
            let msg = err.as_str().unwrap_or("unknown error").to_string();
            return Err(msg.into());
        }
        Ok(())
    }

    fn exists(&self, _path: &Path) -> bool {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("fs", "exists");
            interaction.output.clone()
        };
        output.as_bool().expect("fs::exists: expected boolean output")
    }

    fn list_dir(
        &self,
        _path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("fs", "list_dir");
            interaction.output.clone()
        };
        extract_result(&output, "fs::list_dir")
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
    fn replaying_fs_read_to_string() {
        let replayer = make_replayer(vec![Interaction {
            seq: 0,
            port: "fs".into(),
            method: "read_to_string".into(),
            input: json!({"path": "/tmp/test.txt"}),
            output: json!({"ok": "file contents"}),
        }]);
        let fs = ReplayingFileSystem::new(replayer);
        let result = fs.read_to_string(Path::new("/tmp/test.txt")).unwrap();
        assert_eq!(result, "file contents");
    }

    #[test]
    fn replaying_fs_read_to_string_error() {
        let replayer = make_replayer(vec![Interaction {
            seq: 0,
            port: "fs".into(),
            method: "read_to_string".into(),
            input: json!({"path": "/missing"}),
            output: json!({"err": "file not found"}),
        }]);
        let fs = ReplayingFileSystem::new(replayer);
        let result = fs.read_to_string(Path::new("/missing"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("file not found"));
    }

    #[test]
    fn replaying_fs_exists() {
        let replayer = make_replayer(vec![Interaction {
            seq: 0,
            port: "fs".into(),
            method: "exists".into(),
            input: json!({"path": "/tmp/test.txt"}),
            output: json!(true),
        }]);
        let fs = ReplayingFileSystem::new(replayer);
        assert!(fs.exists(Path::new("/tmp/test.txt")));
    }
}
