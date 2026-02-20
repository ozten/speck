//! Replaying adapters that replay recorded interactions.

pub mod clock;
pub mod filesystem;
pub mod git;
pub mod id_gen;
pub mod issues;
pub mod llm;
pub mod shell;

pub use clock::ReplayingClock;
pub use filesystem::ReplayingFileSystem;
pub use git::ReplayingGitRepo;
pub use id_gen::ReplayingIdGenerator;
pub use issues::ReplayingIssueTracker;
pub use llm::ReplayingLlmClient;
pub use shell::ReplayingShellExecutor;
