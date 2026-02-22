//! Recording session managing per-port cassette recorders.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Utc;

use super::recorder::CassetteRecorder;

/// Manages per-port `CassetteRecorder` instances for a recording session.
///
/// Each port gets its own recorder writing to a separate cassette file.
/// All cassettes are stored in a timestamped directory.
pub struct RecordingSession {
    /// Recorder for LLM interactions.
    pub llm: Arc<Mutex<CassetteRecorder>>,
    /// Recorder for filesystem interactions.
    pub fs: Arc<Mutex<CassetteRecorder>>,
    /// Recorder for git interactions.
    pub git: Arc<Mutex<CassetteRecorder>>,
    /// Recorder for clock interactions.
    pub clock: Arc<Mutex<CassetteRecorder>>,
    /// Recorder for shell interactions.
    pub shell: Arc<Mutex<CassetteRecorder>>,
    /// Recorder for ID generator interactions.
    pub id_gen: Arc<Mutex<CassetteRecorder>>,
    /// Recorder for issue tracker interactions.
    pub issues: Arc<Mutex<CassetteRecorder>>,
    /// Output directory containing all cassette files.
    output_dir: PathBuf,
}

impl RecordingSession {
    /// Create a new recording session with a timestamped output directory.
    ///
    /// Creates directory at `.speck/cassettes/<timestamp>/` relative to cwd.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The cassette directory already exists
    /// - The directory cannot be created
    pub fn new() -> Result<Self, String> {
        let timestamp = Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string();
        let output_dir = PathBuf::from(".speck/cassettes").join(&timestamp);

        if output_dir.exists() {
            return Err(format!("Cassette directory already exists: {}", output_dir.display()));
        }

        std::fs::create_dir_all(&output_dir)
            .map_err(|e| format!("Failed to create cassette directory: {e}"))?;

        let commit = get_commit_hash();

        let make_recorder = |port: &str| -> Arc<Mutex<CassetteRecorder>> {
            let path = output_dir.join(format!("{port}.cassette.yaml"));
            let name = format!("{timestamp}-{port}");
            Arc::new(Mutex::new(CassetteRecorder::new(path, &name, &commit)))
        };

        Ok(Self {
            llm: make_recorder("llm"),
            fs: make_recorder("fs"),
            git: make_recorder("git"),
            clock: make_recorder("clock"),
            shell: make_recorder("shell"),
            id_gen: make_recorder("id_gen"),
            issues: make_recorder("issues"),
            output_dir,
        })
    }

    /// Finish all recorders and write cassette files to disk.
    ///
    /// Consumes the session and writes each port's cassette file.
    ///
    /// # Errors
    ///
    /// Returns an error if any cassette file cannot be written.
    pub fn finish(self) -> Result<PathBuf, String> {
        fn finish_one(arc: Arc<Mutex<CassetteRecorder>>, port: &str) -> Result<(), String> {
            let recorder = Arc::try_unwrap(arc)
                .map_err(|_| format!("Recording adapter for {port} still has references"))?
                .into_inner()
                .map_err(|e| format!("Recorder lock for {port} poisoned: {e}"))?;
            recorder.finish().map_err(|e| format!("Failed to write {port} cassette: {e}"))?;
            Ok(())
        }

        finish_one(self.llm, "llm")?;
        finish_one(self.fs, "fs")?;
        finish_one(self.git, "git")?;
        finish_one(self.clock, "clock")?;
        finish_one(self.shell, "shell")?;
        finish_one(self.id_gen, "id_gen")?;
        finish_one(self.issues, "issues")?;

        Ok(self.output_dir)
    }
}

/// Get the current git commit hash, or "unknown" with a warning if unavailable.
fn get_commit_hash() -> String {
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string());

    if let Some(h) = hash {
        h
    } else {
        eprintln!("Warning: Could not get git commit hash, using 'unknown'");
        "unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_creates_output_directory_and_finishes() {
        let session = RecordingSession::new();
        assert!(session.is_ok(), "RecordingSession::new() should succeed");

        let session = session.unwrap();
        let dir = session.output_dir.clone();
        assert!(dir.exists(), "Output directory should exist after new()");

        // Verify we can finish the session
        let result = session.finish();
        assert!(result.is_ok(), "finish() should succeed");

        // Cleanup - remove the entire .speck/cassettes directory to avoid
        // leaving test artifacts and race conditions between tests
        let cassettes_dir = PathBuf::from(".speck/cassettes");
        let _ = std::fs::remove_dir_all(&cassettes_dir);
    }

    #[test]
    fn get_commit_hash_returns_string() {
        let hash = get_commit_hash();
        // Should return either a valid hash or "unknown"
        assert!(!hash.is_empty());
    }
}
