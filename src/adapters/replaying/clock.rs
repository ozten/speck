//! Replaying adapter for the Clock port.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

use super::next_output;
use crate::cassette::replayer::CassetteReplayer;
use crate::ports::Clock;

/// Serves recorded `now()` results from a cassette.
pub struct ReplayingClock {
    replayer: Option<Arc<Mutex<CassetteReplayer>>>,
}

impl ReplayingClock {
    /// Create a replaying clock backed by the given replayer.
    #[must_use]
    pub fn new(replayer: Arc<Mutex<CassetteReplayer>>) -> Self {
        Self { replayer: Some(replayer) }
    }

    /// Create a replaying clock with no cassette. Panics when called.
    #[must_use]
    pub fn unconfigured() -> Self {
        Self { replayer: None }
    }
}

impl Clock for ReplayingClock {
    fn now(&self) -> DateTime<Utc> {
        let output = next_output(self.replayer.as_ref(), "clock", "now");
        serde_json::from_value(output).expect("failed to deserialize clock output from cassette")
    }
}
