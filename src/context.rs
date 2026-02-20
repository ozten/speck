//! Service context bundling all port trait objects.

use std::path::Path;

use crate::adapters::replaying::{
    ReplayingClock, ReplayingFileSystem, ReplayingGitRepo, ReplayingIdGenerator,
    ReplayingIssueTracker, ReplayingLlmClient, ReplayingShellExecutor,
};
use crate::cassette::config::CassetteConfig;
use crate::cassette::recorder::CassetteRecorder;
use crate::ports::clock::Clock;
use crate::ports::filesystem::FileSystem;
use crate::ports::git::GitRepo;
use crate::ports::id_gen::IdGenerator;
use crate::ports::issues::IssueTracker;
use crate::ports::llm::LlmClient;
use crate::ports::shell::ShellExecutor;

/// Bundles all port trait objects into a single context.
///
/// Each field provides access to one external boundary. Constructors
/// wire up different adapter implementations (live, replaying, recording).
pub struct ServiceContext {
    /// Clock for obtaining the current time.
    pub clock: Box<dyn Clock>,
    /// Filesystem for file I/O.
    pub fs: Box<dyn FileSystem>,
    /// Git repository for version-control queries.
    pub git: Box<dyn GitRepo>,
    /// Shell executor for running commands.
    pub shell: Box<dyn ShellExecutor>,
    /// ID generator for unique identifiers.
    pub id_gen: Box<dyn IdGenerator>,
    /// LLM client for language-model completions.
    pub llm: Box<dyn LlmClient>,
    /// Issue tracker for managing work items.
    pub issues: Box<dyn IssueTracker>,
    /// Optional cassette recorder; written to disk on drop.
    recorder: Option<CassetteRecorder>,
}

impl ServiceContext {
    /// Creates a live context with real adapters for filesystem, shell, clock, and git.
    ///
    /// Remaining ports (`id_gen`, llm, issues) use panicking stubs.
    #[must_use]
    pub fn live() -> Self {
        use crate::adapters::live::clock::LiveClock;
        use crate::adapters::live::filesystem::LiveFileSystem;
        use crate::adapters::live::git::LiveGitRepo;
        use crate::adapters::live::shell::LiveShellExecutor;

        Self {
            clock: Box::new(LiveClock),
            fs: Box::new(LiveFileSystem),
            git: Box::new(LiveGitRepo),
            shell: Box::new(LiveShellExecutor),
            id_gen: Box::new(PanickingIdGenerator),
            llm: Box::new(PanickingLlmClient),
            issues: Box::new(PanickingIssueTracker),
            recorder: None,
        }
    }

    /// Creates a recording context that writes a cassette file on drop.
    ///
    /// Uses live adapters for actual work. The cassette is written to `path`
    /// when this context is dropped. This is the developer-only mechanism
    /// for capturing cassettes via the `SPECK_RECORD` env var.
    #[must_use]
    pub fn recording(path: &Path) -> Self {
        use crate::adapters::live::clock::LiveClock;
        use crate::adapters::live::filesystem::LiveFileSystem;
        use crate::adapters::live::git::LiveGitRepo;
        use crate::adapters::live::shell::LiveShellExecutor;

        Self {
            clock: Box::new(LiveClock),
            fs: Box::new(LiveFileSystem),
            git: Box::new(LiveGitRepo),
            shell: Box::new(LiveShellExecutor),
            id_gen: Box::new(PanickingIdGenerator),
            llm: Box::new(PanickingLlmClient),
            issues: Box::new(PanickingIssueTracker),
            recorder: Some(CassetteRecorder::new(path, "speck-session", "unknown")),
        }
    }

    /// Creates a replaying context from a monolithic cassette file.
    ///
    /// All ports are served by a single cassette — each port/method pair
    /// is dispatched to the right interaction stream automatically.
    ///
    /// # Errors
    ///
    /// Returns an error if the cassette file cannot be read or parsed.
    pub fn replaying(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read cassette file {}: {e}", path.display()))?;
        let cassette: crate::cassette::format::Cassette = serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse cassette file {}: {e}", path.display()))?;

        // Each port gets its own replayer from the same cassette so that
        // per-port cursors are independent.
        Ok(Self {
            clock: Box::new(ReplayingClock::new(crate::cassette::replayer::CassetteReplayer::new(
                &cassette,
            ))),
            fs: Box::new(ReplayingFileSystem::new(
                crate::cassette::replayer::CassetteReplayer::new(&cassette),
            )),
            git: Box::new(ReplayingGitRepo::new(crate::cassette::replayer::CassetteReplayer::new(
                &cassette,
            ))),
            shell: Box::new(ReplayingShellExecutor::new(
                crate::cassette::replayer::CassetteReplayer::new(&cassette),
            )),
            id_gen: Box::new(ReplayingIdGenerator::new(
                crate::cassette::replayer::CassetteReplayer::new(&cassette),
            )),
            llm: Box::new(ReplayingLlmClient::new(
                crate::cassette::replayer::CassetteReplayer::new(&cassette),
            )),
            issues: Box::new(ReplayingIssueTracker::new(
                crate::cassette::replayer::CassetteReplayer::new(&cassette),
            )),
            recorder: None,
        })
    }

    /// Creates a replaying context from per-port cassette files.
    ///
    /// Each port can have its own cassette file. Ports without a configured
    /// cassette file will use a panicking adapter that fails with a clear
    /// message when called.
    ///
    /// # Errors
    ///
    /// Returns an error if any configured cassette file cannot be read or parsed.
    pub fn replaying_from(config: &CassetteConfig) -> Result<Self, String> {
        let replayers = config.load_all()?;

        Ok(Self {
            clock: match replayers.clock {
                Some(r) => Box::new(ReplayingClock::new(r)),
                None => Box::new(PanickingClock),
            },
            fs: match replayers.fs {
                Some(r) => Box::new(ReplayingFileSystem::new(r)),
                None => Box::new(PanickingFileSystem),
            },
            git: match replayers.git {
                Some(r) => Box::new(ReplayingGitRepo::new(r)),
                None => Box::new(PanickingGitRepo),
            },
            shell: match replayers.shell {
                Some(r) => Box::new(ReplayingShellExecutor::new(r)),
                None => Box::new(PanickingShellExecutor),
            },
            id_gen: match replayers.id_gen {
                Some(r) => Box::new(ReplayingIdGenerator::new(r)),
                None => Box::new(PanickingIdGenerator),
            },
            llm: match replayers.llm {
                Some(r) => Box::new(ReplayingLlmClient::new(r)),
                None => Box::new(PanickingLlmClient),
            },
            issues: match replayers.issues {
                Some(r) => Box::new(ReplayingIssueTracker::new(r)),
                None => Box::new(PanickingIssueTracker),
            },
            recorder: None,
        })
    }
}

impl Drop for ServiceContext {
    fn drop(&mut self) {
        if let Some(recorder) = self.recorder.take() {
            if let Err(e) = recorder.finish() {
                eprintln!("Warning: failed to write cassette: {e}");
            }
        }
    }
}

// --- Panicking adapters for unspecified ports ---

struct PanickingClock;
impl Clock for PanickingClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        panic!("Clock port not configured in CassetteConfig — no cassette loaded for clock");
    }
}

struct PanickingFileSystem;
impl FileSystem for PanickingFileSystem {
    fn read_to_string(
        &self,
        _path: &Path,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        panic!("FileSystem port not configured in CassetteConfig — no cassette loaded for fs");
    }
    fn write(
        &self,
        _path: &Path,
        _contents: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        panic!("FileSystem port not configured in CassetteConfig — no cassette loaded for fs");
    }
    fn exists(&self, _path: &Path) -> bool {
        panic!("FileSystem port not configured in CassetteConfig — no cassette loaded for fs");
    }
    fn list_dir(
        &self,
        _path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        panic!("FileSystem port not configured in CassetteConfig — no cassette loaded for fs");
    }
}

struct PanickingGitRepo;
impl GitRepo for PanickingGitRepo {
    fn current_commit(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        panic!("GitRepo port not configured in CassetteConfig — no cassette loaded for git");
    }
    fn diff(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        panic!("GitRepo port not configured in CassetteConfig — no cassette loaded for git");
    }
    fn list_files(
        &self,
        _path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        panic!("GitRepo port not configured in CassetteConfig — no cassette loaded for git");
    }
}

struct PanickingShellExecutor;
impl ShellExecutor for PanickingShellExecutor {
    fn run(
        &self,
        _command: &str,
    ) -> Result<crate::ports::shell::ShellOutput, Box<dyn std::error::Error + Send + Sync>> {
        panic!(
            "ShellExecutor port not configured in CassetteConfig — no cassette loaded for shell"
        );
    }
}

struct PanickingIdGenerator;
impl IdGenerator for PanickingIdGenerator {
    fn generate_id(&self) -> String {
        panic!("IdGenerator port not configured in CassetteConfig — no cassette loaded for id_gen");
    }
}

struct PanickingLlmClient;
impl LlmClient for PanickingLlmClient {
    fn complete(
        &self,
        _request: &crate::ports::llm::CompletionRequest,
    ) -> crate::ports::llm::LlmFuture<'_> {
        panic!("LlmClient port not configured in CassetteConfig — no cassette loaded for llm");
    }
}

struct PanickingIssueTracker;
impl IssueTracker for PanickingIssueTracker {
    fn create_issue(
        &self,
        _title: &str,
        _body: &str,
    ) -> Result<crate::ports::issues::Issue, Box<dyn std::error::Error + Send + Sync>> {
        panic!(
            "IssueTracker port not configured in CassetteConfig — no cassette loaded for issues"
        );
    }
    fn update_issue(
        &self,
        _id: &str,
        _title: Option<&str>,
        _body: Option<&str>,
        _status: Option<&str>,
    ) -> Result<crate::ports::issues::Issue, Box<dyn std::error::Error + Send + Sync>> {
        panic!(
            "IssueTracker port not configured in CassetteConfig — no cassette loaded for issues"
        );
    }
    fn list_issues(
        &self,
        _status: Option<&str>,
    ) -> Result<Vec<crate::ports::issues::Issue>, Box<dyn std::error::Error + Send + Sync>> {
        panic!(
            "IssueTracker port not configured in CassetteConfig — no cassette loaded for issues"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
    use chrono::Utc;
    use serde_json::json;

    fn write_cassette(path: &Path, interactions: Vec<Interaction>) {
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
    fn replaying_context_from_monolithic_cassette() {
        let dir = std::env::temp_dir().join("speck_ctx_test_mono");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("full.cassette.yaml");

        write_cassette(
            &path,
            vec![
                Interaction {
                    seq: 0,
                    port: "clock".into(),
                    method: "now".into(),
                    input: json!({}),
                    output: json!("2024-06-15T10:30:00Z"),
                },
                Interaction {
                    seq: 1,
                    port: "id_gen".into(),
                    method: "generate_id".into(),
                    input: json!({}),
                    output: json!("uuid-001"),
                },
            ],
        );

        let ctx = ServiceContext::replaying(&path).unwrap();
        let time = ctx.clock.now();
        assert_eq!(time.to_rfc3339(), "2024-06-15T10:30:00+00:00");
        let id = ctx.id_gen.generate_id();
        assert_eq!(id, "uuid-001");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replaying_from_per_port_cassettes() {
        let dir = std::env::temp_dir().join("speck_ctx_test_ports");
        std::fs::create_dir_all(&dir).unwrap();

        let clock_path = dir.join("clock.cassette.yaml");
        write_cassette(
            &clock_path,
            vec![Interaction {
                seq: 0,
                port: "clock".into(),
                method: "now".into(),
                input: json!({}),
                output: json!("2024-01-01T00:00:00Z"),
            }],
        );

        let config = CassetteConfig { clock: Some(clock_path), ..CassetteConfig::default() };
        let ctx = ServiceContext::replaying_from(&config).unwrap();
        let time = ctx.clock.now();
        assert_eq!(time.to_rfc3339(), "2024-01-01T00:00:00+00:00");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[should_panic(expected = "not configured in CassetteConfig")]
    fn unspecified_port_panics_with_clear_message() {
        let config = CassetteConfig::panic_on_unspecified();
        let ctx = ServiceContext::replaying_from(&config).unwrap();
        let _ = ctx.clock.now();
    }
}
