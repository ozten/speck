//! Signal type classification for task specs.

use serde::{Deserialize, Serialize};

/// Classifies how observable a requirement's correctness signal is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    /// Directly testable via assertions on outputs.
    Clear,
    /// Requires human judgment or fuzzy matching.
    Fuzzy,
    /// Correctness depends on internal logic that must be exposed for testing.
    InternalLogic,
}
