//! Replaying adapter for the Clock port.

use std::sync::Mutex;

use chrono::{DateTime, Utc};

use crate::cassette::replayer::CassetteReplayer;
use crate::ports::clock::Clock;

/// Replays recorded clock values from a cassette.
pub struct ReplayingClock {
    replayer: Mutex<CassetteReplayer>,
}

impl ReplayingClock {
    /// Creates a new replaying clock from a cassette replayer.
    #[must_use]
    pub fn new(replayer: CassetteReplayer) -> Self {
        Self { replayer: Mutex::new(replayer) }
    }
}

impl Clock for ReplayingClock {
    fn now(&self) -> DateTime<Utc> {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("clock", "now");
            interaction.output.clone()
        };
        serde_json::from_value(output).expect("clock::now: failed to deserialize DateTime<Utc>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
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
    fn replaying_clock_serves_recorded_time() {
        let ts = "2024-06-15T10:30:00Z";
        let replayer = make_replayer(vec![Interaction {
            seq: 0,
            port: "clock".into(),
            method: "now".into(),
            input: json!({}),
            output: json!(ts),
        }]);
        let clock = ReplayingClock::new(replayer);
        let result = clock.now();
        assert_eq!(result.to_rfc3339(), "2024-06-15T10:30:00+00:00");
    }

    #[test]
    fn replaying_clock_serves_multiple_times() {
        let replayer = make_replayer(vec![
            Interaction {
                seq: 0,
                port: "clock".into(),
                method: "now".into(),
                input: json!({}),
                output: json!("2024-01-01T00:00:00Z"),
            },
            Interaction {
                seq: 1,
                port: "clock".into(),
                method: "now".into(),
                input: json!({}),
                output: json!("2024-01-01T00:01:00Z"),
            },
        ]);
        let clock = ReplayingClock::new(replayer);
        let t1 = clock.now();
        let t2 = clock.now();
        assert!(t2 > t1);
    }
}
