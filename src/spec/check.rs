//! Verification check types for task spec checks.

use serde::{Deserialize, Serialize};

/// A single verification check within a verification strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VerificationCheck {
    /// Run a test suite command and expect it to pass.
    TestSuite {
        /// The command to run.
        command: String,
        /// What to expect from the output.
        expected: String,
    },
    /// Run a SQL query and assert on the result.
    SqlAssertion {
        /// The SQL query to execute.
        query: String,
        /// Expected result description.
        expected: String,
    },
    /// Run a command and check its output.
    CommandOutput {
        /// The command to run.
        command: String,
        /// Expected output or assertion.
        expected: String,
    },
    /// Verify a migration can be rolled back.
    MigrationRollback {
        /// Description of the rollback check.
        description: String,
    },
    /// A custom check with a freeform description.
    Custom {
        /// Description of the custom check.
        description: String,
    },
}
