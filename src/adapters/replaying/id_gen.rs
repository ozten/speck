//! Replaying adapter for the `IdGenerator` port.

use std::sync::Mutex;

use crate::cassette::replayer::CassetteReplayer;
use crate::ports::id_gen::IdGenerator;

/// Replays recorded IDs from a cassette.
pub struct ReplayingIdGenerator {
    replayer: Mutex<CassetteReplayer>,
}

impl ReplayingIdGenerator {
    /// Creates a new replaying ID generator from a cassette replayer.
    #[must_use]
    pub fn new(replayer: CassetteReplayer) -> Self {
        Self { replayer: Mutex::new(replayer) }
    }
}

impl IdGenerator for ReplayingIdGenerator {
    fn generate_id(&self) -> String {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("id_gen", "generate_id");
            interaction.output.clone()
        };
        output.as_str().expect("id_gen::generate_id: expected string output").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
    use chrono::Utc;
    use serde_json::json;

    fn make_replayer(interactions: Vec<Interaction>) -> CassetteReplayer {
        let cassette = Cassette {
            name: "test".into(),
            recorded_at: Utc::now(),
            commit: "abc".into(),
            interactions,
        };
        CassetteReplayer::new(&cassette)
    }

    #[test]
    fn replaying_id_generator() {
        let replayer = make_replayer(vec![
            Interaction {
                seq: 0,
                port: "id_gen".into(),
                method: "generate_id".into(),
                input: json!({}),
                output: json!("uuid-001"),
            },
            Interaction {
                seq: 1,
                port: "id_gen".into(),
                method: "generate_id".into(),
                input: json!({}),
                output: json!("uuid-002"),
            },
        ]);
        let gen = ReplayingIdGenerator::new(replayer);
        assert_eq!(gen.generate_id(), "uuid-001");
        assert_eq!(gen.generate_id(), "uuid-002");
    }
}
