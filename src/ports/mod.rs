//! Port traits defining external boundaries.
//!
//! Each trait represents a boundary between the application core and an
//! external system (time, LLM, filesystem, git, shell, IDs, issues).
//! Implementations live in `src/adapters/`.

pub mod clock;
pub mod filesystem;
pub mod git;
pub mod id_gen;
pub mod issues;
pub mod llm;
pub mod shell;

pub use clock::Clock;
pub use filesystem::FileSystem;
pub use git::GitRepo;
pub use id_gen::IdGenerator;
pub use issues::{Issue, IssueTracker};
pub use llm::{CompletionFuture, CompletionRequest, CompletionResponse, LlmClient};
pub use shell::{ShellExecutor, ShellOutput};
