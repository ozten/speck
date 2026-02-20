//! Task spec types for the specification format.
//!
//! Defines the Rust types that mirror the YAML task spec schema.
//! These are serialized/deserialized by the store and consumed by validate.

mod check;
mod signal;
mod task_spec;
mod verification;

pub use check::VerificationCheck;
pub use signal::SignalType;
pub use task_spec::{TaskContext, TaskSpec};
pub use verification::VerificationStrategy;
