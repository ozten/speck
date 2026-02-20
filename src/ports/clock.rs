//! Clock port for obtaining the current time.

use chrono::{DateTime, Utc};

/// Provides the current time.
///
/// Abstracting time access allows deterministic replay by substituting
/// a fixed or recorded clock during tests and cassette playback.
pub trait Clock: Send + Sync {
    /// Returns the current UTC time.
    fn now(&self) -> DateTime<Utc>;
}
