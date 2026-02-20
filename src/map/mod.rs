//! Codebase map generation: structural snapshot of a target project.

pub mod diff;
pub mod generator;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Structural snapshot of a codebase tied to a specific commit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodebaseMap {
    /// Git commit hash this map was generated against.
    pub commit_hash: String,
    /// Timestamp when the map was generated.
    pub generated_at: DateTime<Utc>,
    /// Module summaries found in the project.
    pub modules: Vec<ModuleSummary>,
    /// Flat list of all files in the project tree.
    pub directory_tree: Vec<String>,
    /// Paths to test files / test infrastructure found.
    pub test_infrastructure: Vec<String>,
}

/// Summary of a single module boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleSummary {
    /// Path of the module relative to the project root.
    pub path: String,
    /// Public items (functions, structs, traits) found in the module.
    pub public_items: Vec<String>,
    /// Inferred dependencies (modules or crates referenced).
    pub dependencies: Vec<String>,
}
