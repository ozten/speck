//! LLM client port for language-model completions.

use std::error::Error;
use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

/// Boxed future type alias used by [`LlmClient`] to keep the trait dyn-compatible.
pub type LlmFuture<'a> = Pin<
    Box<dyn Future<Output = Result<CompletionResponse, Box<dyn Error + Send + Sync>>> + Send + 'a>,
>;

/// A request to generate a completion from an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// The model identifier (e.g. `"claude-sonnet-4-20250514"`).
    pub model: String,
    /// The prompt or message sequence to send.
    pub prompt: String,
    /// Maximum number of tokens to generate.
    pub max_tokens: u32,
}

/// The response from an LLM completion call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// The generated text.
    pub text: String,
    /// Number of prompt tokens consumed.
    pub prompt_tokens: u32,
    /// Number of completion tokens generated.
    pub completion_tokens: u32,
}

/// Sends completion requests to a language model.
pub trait LlmClient: Send + Sync {
    /// Generates a completion for the given request.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails (network, auth, rate-limit, etc.).
    fn complete(&self, request: &CompletionRequest) -> LlmFuture<'_>;
}
