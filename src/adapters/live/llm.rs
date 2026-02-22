//! Live adapter for the `LlmClient` port using the Anthropic messages API.

use std::env;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::ports::llm::{CompletionFuture, CompletionRequest, CompletionResponse, LlmClient};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Live LLM client that calls the Anthropic Claude API.
pub struct LiveLlmClient {
    client: Client,
}

impl LiveLlmClient {
    /// Creates a new live LLM client.
    #[must_use]
    pub fn new() -> Self {
        Self { client: Client::new() }
    }
}

impl Default for LiveLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Request body sent to the Anthropic messages API.
#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<AnthropicMessage<'a>>,
}

/// A single message in the Anthropic API request.
#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

/// Top-level response from the Anthropic messages API.
#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    usage: Usage,
}

/// A content block in the Anthropic response.
#[derive(Deserialize)]
struct ContentBlock {
    text: String,
}

/// Token usage reported by the Anthropic API.
#[derive(Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

/// Error response from the Anthropic API.
#[derive(Deserialize)]
struct AnthropicError {
    error: AnthropicErrorDetail,
}

/// Detail inside an Anthropic error response.
#[derive(Deserialize)]
struct AnthropicErrorDetail {
    message: String,
}

impl LlmClient for LiveLlmClient {
    fn complete(&self, request: &CompletionRequest) -> CompletionFuture<'_> {
        let model = request.model.clone();
        let prompt = request.prompt.clone();
        let max_tokens = request.max_tokens;

        Box::pin(async move {
            let api_key = env::var("ANTHROPIC_API_KEY").map_err(|_| {
                Box::<dyn std::error::Error + Send + Sync>::from(
                    "ANTHROPIC_API_KEY environment variable not set",
                )
            })?;

            let body = AnthropicRequest {
                model: &model,
                max_tokens,
                messages: vec![AnthropicMessage { role: "user", content: &prompt }],
            };

            let response = self
                .client
                .post(ANTHROPIC_API_URL)
                .header("x-api-key", &api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .json(&body)
                .send()
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    format!("Anthropic API request failed: {e}").into()
                })?;

            let status = response.status();
            let response_text =
                response.text().await.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    format!("Failed to read Anthropic API response: {e}").into()
                })?;

            if !status.is_success() {
                let msg = serde_json::from_str::<AnthropicError>(&response_text)
                    .map(|e| e.error.message)
                    .unwrap_or(response_text);
                return Err(format!("Anthropic API error ({}): {msg}", status.as_u16()).into());
            }

            let api_response: AnthropicResponse = serde_json::from_str(&response_text).map_err(
                |e| -> Box<dyn std::error::Error + Send + Sync> {
                    format!("Failed to parse Anthropic API response: {e}").into()
                },
            )?;

            let text = api_response.content.into_iter().map(|block| block.text).collect::<String>();

            Ok(CompletionResponse {
                text,
                prompt_tokens: api_response.usage.input_tokens,
                completion_tokens: api_response.usage.output_tokens,
            })
        })
    }
}
