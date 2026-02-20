//! Replaying adapter for the `LlmClient` port.

use std::sync::Mutex;

use crate::cassette::replayer::CassetteReplayer;
use crate::ports::llm::{CompletionRequest, CompletionResponse, LlmClient, LlmFuture};

/// Replays recorded LLM completions from a cassette.
pub struct ReplayingLlmClient {
    replayer: Mutex<CassetteReplayer>,
}

impl ReplayingLlmClient {
    /// Creates a new replaying LLM client from a cassette replayer.
    #[must_use]
    pub fn new(replayer: CassetteReplayer) -> Self {
        Self { replayer: Mutex::new(replayer) }
    }
}

impl LlmClient for ReplayingLlmClient {
    fn complete(&self, _request: &CompletionRequest) -> LlmFuture<'_> {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("llm", "complete");
            interaction.output.clone()
        };
        let result: Result<CompletionResponse, Box<dyn std::error::Error + Send + Sync>> =
            if let Some(err) = output.get("err") {
                let msg = err.as_str().unwrap_or("unknown error").to_string();
                Err(msg.into())
            } else {
                let value = output.get("ok").unwrap_or(&output);
                serde_json::from_value(value.clone()).map_err(
                    |e| -> Box<dyn std::error::Error + Send + Sync> {
                        format!("llm::complete: failed to deserialize: {e}").into()
                    },
                )
            };
        Box::pin(std::future::ready(result))
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

    #[tokio::test]
    async fn replaying_llm_complete() {
        let replayer = make_replayer(vec![Interaction {
            seq: 0,
            port: "llm".into(),
            method: "complete".into(),
            input: json!({"prompt": "hello"}),
            output: json!({"ok": {"text": "world", "prompt_tokens": 5, "completion_tokens": 1}}),
        }]);
        let client = ReplayingLlmClient::new(replayer);
        let request =
            CompletionRequest { model: "test".into(), prompt: "hello".into(), max_tokens: 100 };
        let response = client.complete(&request).await.unwrap();
        assert_eq!(response.text, "world");
        assert_eq!(response.prompt_tokens, 5);
        assert_eq!(response.completion_tokens, 1);
    }

    #[tokio::test]
    async fn replaying_llm_complete_error() {
        let replayer = make_replayer(vec![Interaction {
            seq: 0,
            port: "llm".into(),
            method: "complete".into(),
            input: json!({}),
            output: json!({"err": "rate limited"}),
        }]);
        let client = ReplayingLlmClient::new(replayer);
        let request =
            CompletionRequest { model: "test".into(), prompt: "hello".into(), max_tokens: 100 };
        let result = client.complete(&request).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("rate limited"));
    }
}
