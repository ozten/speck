//! Core task spec type.

use serde::{Deserialize, Serialize};

use super::signal::SignalType;
use super::verification::VerificationStrategy;

/// Context about the codebase area a task touches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskContext {
    /// Abstract module references (e.g., "`MetricsService`").
    #[serde(default)]
    pub modules: Vec<String>,
    /// Patterns to follow (e.g., "Follow existing migration conventions").
    #[serde(default)]
    pub patterns: Option<String>,
    /// Task IDs this task depends on.
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// A fully-specified task produced by `spec plan` and consumed by `spec validate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Unique task identifier (e.g., "IMPACT-42").
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Parent requirement reference.
    #[serde(default)]
    pub requirement: Option<String>,
    /// Codebase context for this task.
    #[serde(default)]
    pub context: Option<TaskContext>,
    /// What must be true when the task is complete.
    pub acceptance_criteria: Vec<String>,
    /// How observable the correctness signal is.
    pub signal_type: SignalType,
    /// How to verify the acceptance criteria.
    pub verification: VerificationStrategy,
}
