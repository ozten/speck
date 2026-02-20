//! Verification strategy types for task specs.

use serde::{Deserialize, Serialize};

use super::check::VerificationCheck;

/// How to verify that a task's acceptance criteria are met.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum VerificationStrategy {
    /// Direct assertions via checks (tests, SQL, commands).
    DirectAssertion {
        /// The list of checks to run.
        checks: Vec<VerificationCheck>,
    },
    /// Refactor internal logic to expose a decision point for testing.
    RefactorToExpose {
        /// Description of the decision point to expose.
        decision_point: String,
        /// Required code structure after refactoring.
        required_structure: String,
        /// Test cases for the exposed function.
        cases: Vec<serde_yaml::Value>,
    },
    /// Assert on trace output from instrumented code.
    TraceAssertion {
        /// The trace point to instrument.
        trace_point: String,
        /// Path to the test input fixture.
        test_input: String,
        /// Expected trace entries.
        expected_trace: Vec<serde_yaml::Value>,
    },
}
