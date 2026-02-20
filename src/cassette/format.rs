//! Cassette data structures for recording and replaying interactions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single recorded interaction with an external port.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Interaction {
    /// Sequence number (assigned automatically by the recorder).
    pub seq: u64,
    /// Port name (e.g. "llm", "fs", "git").
    pub port: String,
    /// Method name invoked on the port.
    pub method: String,
    /// Input data sent to the port.
    pub input: serde_json::Value,
    /// Output data returned from the port.
    pub output: serde_json::Value,
}

/// A cassette containing a sequence of recorded interactions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cassette {
    /// Human-readable name for this cassette.
    pub name: String,
    /// When this cassette was recorded.
    pub recorded_at: DateTime<Utc>,
    /// Git commit hash at recording time.
    pub commit: String,
    /// Ordered list of interactions.
    pub interactions: Vec<Interaction>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_cassette() -> Cassette {
        Cassette {
            name: "test-cassette".into(),
            recorded_at: Utc::now(),
            commit: "abc123".into(),
            interactions: vec![
                Interaction {
                    seq: 0,
                    port: "llm".into(),
                    method: "complete".into(),
                    input: json!({"prompt": "hello"}),
                    output: json!({"text": "world"}),
                },
                Interaction {
                    seq: 1,
                    port: "fs".into(),
                    method: "read".into(),
                    input: json!({"path": "/tmp/test"}),
                    output: json!({"content": "data"}),
                },
            ],
        }
    }

    #[test]
    fn yaml_round_trip() {
        let cassette = sample_cassette();
        let yaml = serde_yaml::to_string(&cassette).expect("serialize");
        let deserialized: Cassette = serde_yaml::from_str(&yaml).expect("deserialize");
        assert_eq!(cassette, deserialized);
    }
}
