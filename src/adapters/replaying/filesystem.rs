//! Replaying adapter for the `FileSystem` port.

use std::path::Path;
use std::sync::{Arc, Mutex};

use super::{next_output, replay_result};
use crate::cassette::replayer::CassetteReplayer;
use crate::ports::FileSystem;

/// Serves recorded filesystem results from a cassette.
pub struct ReplayingFileSystem {
    replayer: Option<Arc<Mutex<CassetteReplayer>>>,
}

impl ReplayingFileSystem {
    /// Create a replaying filesystem backed by the given replayer.
    #[must_use]
    pub fn new(replayer: Arc<Mutex<CassetteReplayer>>) -> Self {
        Self { replayer: Some(replayer) }
    }

    /// Create a replaying filesystem with no cassette. Panics when called.
    #[must_use]
    pub fn unconfigured() -> Self {
        Self { replayer: None }
    }
}

impl FileSystem for ReplayingFileSystem {
    fn read_to_string(
        &self,
        _path: &Path,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let output = next_output(self.replayer.as_ref(), "fs", "read_to_string");
        replay_result(output)
    }

    fn write(
        &self,
        _path: &Path,
        _contents: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let output = next_output(self.replayer.as_ref(), "fs", "write");
        replay_result(output)
    }

    fn exists(&self, _path: &Path) -> bool {
        let output = next_output(self.replayer.as_ref(), "fs", "exists");
        serde_json::from_value(output)
            .expect("failed to deserialize fs exists output from cassette")
    }

    fn list_dir(
        &self,
        _path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let output = next_output(self.replayer.as_ref(), "fs", "list_dir");
        replay_result(output)
    }
}
