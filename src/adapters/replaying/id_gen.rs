//! Replaying adapter for the `IdGenerator` port.

use std::sync::{Arc, Mutex};

use super::next_output;
use crate::cassette::replayer::CassetteReplayer;
use crate::ports::IdGenerator;

/// Serves recorded IDs from a cassette.
pub struct ReplayingIdGenerator {
    replayer: Option<Arc<Mutex<CassetteReplayer>>>,
}

impl ReplayingIdGenerator {
    /// Create a replaying ID generator backed by the given replayer.
    #[must_use]
    pub fn new(replayer: Arc<Mutex<CassetteReplayer>>) -> Self {
        Self { replayer: Some(replayer) }
    }

    /// Create a replaying ID generator with no cassette. Panics when called.
    #[must_use]
    pub fn unconfigured() -> Self {
        Self { replayer: None }
    }
}

impl IdGenerator for ReplayingIdGenerator {
    fn generate_id(&self) -> String {
        let output = next_output(self.replayer.as_ref(), "id_gen", "generate_id");
        serde_json::from_value(output).expect("failed to deserialize id_gen output from cassette")
    }
}
