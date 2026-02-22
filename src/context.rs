//! Service context that bundles all port trait objects.

use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::adapters::live::clock::LiveClock;
use crate::adapters::live::filesystem::LiveFileSystem;
use crate::adapters::live::git::LiveGitRepo;
use crate::adapters::live::id_gen::LiveIdGenerator;
use crate::adapters::live::issues::LiveIssueTracker;
use crate::adapters::live::llm::LiveLlmClient;
use crate::adapters::live::shell::LiveShellExecutor;
use crate::adapters::recording::clock::RecordingClock;
use crate::adapters::recording::filesystem::RecordingFileSystem;
use crate::adapters::recording::git::RecordingGitRepo;
use crate::adapters::recording::id_gen::RecordingIdGenerator;
use crate::adapters::recording::issues::RecordingIssueTracker;
use crate::adapters::recording::llm::RecordingLlmClient;
use crate::adapters::recording::shell::RecordingShellExecutor;
use crate::adapters::replaying::clock::ReplayingClock;
use crate::adapters::replaying::filesystem::ReplayingFileSystem;
use crate::adapters::replaying::git::ReplayingGitRepo;
use crate::adapters::replaying::id_gen::ReplayingIdGenerator;
use crate::adapters::replaying::issues::ReplayingIssueTracker;
use crate::adapters::replaying::llm::ReplayingLlmClient;
use crate::adapters::replaying::shell::ReplayingShellExecutor;
use crate::cassette::config::CassetteConfig;
use crate::cassette::session::RecordingSession;
use crate::ports::{
    Clock, FileSystem, GitRepo, IdGenerator, IssueTracker, LlmClient, ShellExecutor,
};

/// Bundles all port trait objects into a single context.
pub struct ServiceContext {
    /// Clock port for obtaining the current time.
    pub clock: Box<dyn Clock>,
    /// Filesystem port for file I/O operations.
    pub fs: Box<dyn FileSystem>,
    /// Git repository port for version-control queries.
    pub git: Box<dyn GitRepo>,
    /// Shell executor port for running external commands.
    pub shell: Box<dyn ShellExecutor>,
    /// ID generator port for producing unique identifiers.
    pub id_gen: Box<dyn IdGenerator>,
    /// LLM client port for language-model completions.
    pub llm: Box<dyn LlmClient>,
    /// Issue tracker port for managing work items.
    pub issues: Box<dyn IssueTracker>,
}

impl ServiceContext {
    /// Create a live context with real adapters for all ports.
    #[must_use]
    pub fn live() -> Self {
        Self {
            clock: Box::new(LiveClock),
            fs: Box::new(LiveFileSystem),
            git: Box::new(LiveGitRepo),
            shell: Box::new(LiveShellExecutor),
            id_gen: Box::new(LiveIdGenerator::new()),
            llm: Box::new(LiveLlmClient::new()),
            issues: Box::new(LiveIssueTracker),
        }
    }

    /// Create a recording context that wraps live adapters with recorders.
    ///
    /// All interactions are recorded to per-port cassette files in a
    /// timestamped directory under `.speck/cassettes/`.
    ///
    /// Returns both the context and the recording session. The session must
    /// be finished after the context is dropped to write the cassette files.
    ///
    /// # Errors
    ///
    /// Returns an error if the recording session cannot be initialized.
    pub fn recording() -> Result<(Self, RecordingSession), String> {
        let session = RecordingSession::new()?;

        let ctx = Self {
            clock: Box::new(RecordingClock::new(Box::new(LiveClock), Arc::clone(&session.clock))),
            fs: Box::new(RecordingFileSystem::new(
                Box::new(LiveFileSystem),
                Arc::clone(&session.fs),
            )),
            git: Box::new(RecordingGitRepo::new(Box::new(LiveGitRepo), Arc::clone(&session.git))),
            shell: Box::new(RecordingShellExecutor::new(
                Box::new(LiveShellExecutor),
                Arc::clone(&session.shell),
            )),
            id_gen: Box::new(RecordingIdGenerator::new(
                Box::new(LiveIdGenerator::new()),
                Arc::clone(&session.id_gen),
            )),
            llm: Box::new(RecordingLlmClient::new(
                Box::new(LiveLlmClient::new()),
                Arc::clone(&session.llm),
            )),
            issues: Box::new(RecordingIssueTracker::new(
                Box::new(LiveIssueTracker),
                Arc::clone(&session.issues),
            )),
        };

        Ok((ctx, session))
    }

    /// Creates a replaying context from a monolithic cassette file.
    ///
    /// All ports share the same cassette replayer, serving interactions
    /// in the order they were recorded.
    ///
    /// # Errors
    ///
    /// Returns an error if the cassette file cannot be read or parsed.
    pub fn replaying(path: &Path) -> Result<Self, String> {
        let replayer = Arc::new(Mutex::new(CassetteConfig::load_monolithic(path)?));
        Ok(Self {
            clock: Box::new(ReplayingClock::new(Arc::clone(&replayer))),
            fs: Box::new(ReplayingFileSystem::new(Arc::clone(&replayer))),
            git: Box::new(ReplayingGitRepo::new(Arc::clone(&replayer))),
            shell: Box::new(ReplayingShellExecutor::new(Arc::clone(&replayer))),
            id_gen: Box::new(ReplayingIdGenerator::new(Arc::clone(&replayer))),
            llm: Box::new(ReplayingLlmClient::new(Arc::clone(&replayer))),
            issues: Box::new(ReplayingIssueTracker::new(replayer)),
        })
    }

    /// Create a replaying context from per-port cassette configuration.
    ///
    /// Each port gets its own cassette replayer. Ports without a configured
    /// cassette will panic with a clear message when called.
    ///
    /// # Errors
    ///
    /// Returns an error if any configured cassette file cannot be read or parsed.
    pub fn replaying_from(config: &CassetteConfig) -> Result<Self, String> {
        let replayers = config.load_all()?;

        let wrap = |r| Option::map(r, |r| Arc::new(Mutex::new(r)));

        let clock: Box<dyn Clock> = match wrap(replayers.clock) {
            Some(r) => Box::new(ReplayingClock::new(r)),
            None => Box::new(ReplayingClock::unconfigured()),
        };
        let fs: Box<dyn FileSystem> = match wrap(replayers.fs) {
            Some(r) => Box::new(ReplayingFileSystem::new(r)),
            None => Box::new(ReplayingFileSystem::unconfigured()),
        };
        let git: Box<dyn GitRepo> = match wrap(replayers.git) {
            Some(r) => Box::new(ReplayingGitRepo::new(r)),
            None => Box::new(ReplayingGitRepo::unconfigured()),
        };
        let shell: Box<dyn ShellExecutor> = match wrap(replayers.shell) {
            Some(r) => Box::new(ReplayingShellExecutor::new(r)),
            None => Box::new(ReplayingShellExecutor::unconfigured()),
        };
        let id_gen: Box<dyn IdGenerator> = match wrap(replayers.id_gen) {
            Some(r) => Box::new(ReplayingIdGenerator::new(r)),
            None => Box::new(ReplayingIdGenerator::unconfigured()),
        };
        let llm: Box<dyn LlmClient> = match wrap(replayers.llm) {
            Some(r) => Box::new(ReplayingLlmClient::new(r)),
            None => Box::new(ReplayingLlmClient::unconfigured()),
        };
        let issues: Box<dyn IssueTracker> = match wrap(replayers.issues) {
            Some(r) => Box::new(ReplayingIssueTracker::new(r)),
            None => Box::new(ReplayingIssueTracker::unconfigured()),
        };

        Ok(Self { clock, fs, git, shell, id_gen, llm, issues })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
    use chrono::Utc;
    use serde_json::json;

    fn write_cassette_file(path: &Path, interactions: Vec<Interaction>) {
        let cassette = Cassette {
            name: "test".into(),
            recorded_at: Utc::now(),
            commit: "abc".into(),
            interactions,
        };
        let yaml = serde_yaml::to_string(&cassette).unwrap();
        std::fs::write(path, yaml).unwrap();
    }

    #[test]
    fn replaying_context_serves_recorded_data() {
        let dir = std::env::temp_dir().join("speck_ctx_replaying");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.cassette.yaml");

        write_cassette_file(
            &path,
            vec![
                Interaction {
                    seq: 0,
                    port: "clock".into(),
                    method: "now".into(),
                    input: json!(null),
                    output: json!("2024-01-15T12:00:00Z"),
                },
                Interaction {
                    seq: 1,
                    port: "id_gen".into(),
                    method: "generate_id".into(),
                    input: json!(null),
                    output: json!("test-id-42"),
                },
            ],
        );

        let ctx = ServiceContext::replaying(&path).unwrap();
        let now = ctx.clock.now();
        assert_eq!(now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true), "2024-01-15T12:00:00Z");

        let id = ctx.id_gen.generate_id();
        assert_eq!(id, "test-id-42");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replaying_from_with_per_port_cassettes() {
        let dir = std::env::temp_dir().join("speck_ctx_replaying_from");
        std::fs::create_dir_all(&dir).unwrap();

        let clock_path = dir.join("clock.cassette.yaml");
        write_cassette_file(
            &clock_path,
            vec![Interaction {
                seq: 0,
                port: "clock".into(),
                method: "now".into(),
                input: json!(null),
                output: json!("2024-06-01T08:30:00Z"),
            }],
        );

        let config = CassetteConfig { clock: Some(clock_path), ..CassetteConfig::default() };

        let ctx = ServiceContext::replaying_from(&config).unwrap();
        let now = ctx.clock.now();
        assert_eq!(now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true), "2024-06-01T08:30:00Z");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[should_panic(expected = "no cassette configured for port")]
    fn replaying_from_panics_on_unconfigured_port() {
        let config = CassetteConfig::default();
        let ctx = ServiceContext::replaying_from(&config).unwrap();
        let _ = ctx.clock.now();
    }
}
