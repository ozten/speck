//! Replaying adapter for the `LlmClient` port.

use std::sync::{Arc, Mutex};

use super::{next_output, replay_result};
use crate::cassette::replayer::CassetteReplayer;
use crate::ports::{CompletionFuture, CompletionRequest, LlmClient};

/// Serves recorded LLM completions from a cassette.
pub struct ReplayingLlmClient {
    replayer: Option<Arc<Mutex<CassetteReplayer>>>,
}

impl ReplayingLlmClient {
    /// Create a replaying LLM client backed by the given replayer.
    #[must_use]
    pub fn new(replayer: Arc<Mutex<CassetteReplayer>>) -> Self {
        Self { replayer: Some(replayer) }
    }

    /// Create a replaying LLM client with no cassette. Panics when called.
    #[must_use]
    pub fn unconfigured() -> Self {
        Self { replayer: None }
    }
}

impl LlmClient for ReplayingLlmClient {
    fn complete(&self, _request: &CompletionRequest) -> CompletionFuture<'_> {
        let output = next_output(self.replayer.as_ref(), "llm", "complete");
        Box::pin(async move { replay_result(output) })
    }
}
