//! Recording adapter for the `LlmClient` port.

use std::sync::{Arc, Mutex};

use super::record_result;
use crate::cassette::recorder::CassetteRecorder;
use crate::ports::{CompletionFuture, CompletionRequest, LlmClient};

/// Records LLM interactions while delegating to an inner implementation.
pub struct RecordingLlmClient {
    inner: Box<dyn LlmClient>,
    recorder: Arc<Mutex<CassetteRecorder>>,
}

impl RecordingLlmClient {
    /// Creates a new recording LLM client wrapping the given implementation.
    pub fn new(inner: Box<dyn LlmClient>, recorder: Arc<Mutex<CassetteRecorder>>) -> Self {
        Self { inner, recorder }
    }
}

impl LlmClient for RecordingLlmClient {
    fn complete(&self, request: &CompletionRequest) -> CompletionFuture<'_> {
        let request_clone = request.clone();
        let recorder = Arc::clone(&self.recorder);

        Box::pin(async move {
            // We need to call the inner implementation directly here
            // since we can't easily store the future
            let inner_ptr = &self.inner;
            let result = inner_ptr.complete(&request_clone).await;

            record_result(&recorder, "llm", "complete", &request_clone, &result);

            result
        })
    }
}

// Note: Testing the async LlmClient recording is more complex and would
// require a mock LlmClient. The pattern is the same as the sync adapters.
