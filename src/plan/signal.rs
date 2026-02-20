//! Signal type classification engine for the pushback rule.
//!
//! Given a requirement description and codebase context, classifies the
//! verification signal as clear, fuzzy-but-constrainable, or internal-logic.
//! For each type, proposes the appropriate verification strategy.

use serde::{Deserialize, Serialize};

use crate::ports::llm::{CompletionRequest, LlmClient};

/// The type of verification signal produced by a requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalType {
    /// Output is directly observable and assertable.
    Clear,
    /// Observable but with soft correctness criteria; can be decomposed
    /// into clear structural sub-assertions.
    FuzzyButConstrainable,
    /// Correctness depends on internal logic at a specific code point.
    InternalLogic,
}

/// A single clear sub-assertion decomposed from a fuzzy requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubAssertion {
    /// Human-readable description of what to check.
    pub description: String,
    /// The concrete check to perform.
    pub check: String,
}

/// The verification strategy proposed for a requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStrategy {
    /// Direct assertion with specific checks (for clear signals).
    DirectAssertion {
        /// The concrete checks to run.
        checks: Vec<String>,
    },
    /// Decomposition into clear structural sub-assertions (for fuzzy signals).
    StructuralDecomposition {
        /// The sub-assertions that together verify the fuzzy requirement.
        sub_assertions: Vec<SubAssertion>,
    },
    /// Refactor code to make the decision point testable (for internal logic).
    RefactorToExpose {
        /// Description of what to refactor and how.
        description: String,
    },
    /// Instrument code path and assert on trace output (for internal logic).
    TraceAssertion {
        /// Description of the trace point and expected output.
        description: String,
    },
}

/// Result of classifying a requirement's signal type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassificationResult {
    /// Successfully classified with a proposed verification strategy.
    Classified {
        /// The signal type.
        signal_type: SignalType,
        /// The proposed verification strategy.
        strategy: VerificationStrategy,
    },
    /// Requirement is under-specified; pushback needed before it enters the system.
    PushbackRequired {
        /// Why the requirement cannot be classified.
        reason: String,
    },
}

/// Classify a requirement and propose a verification strategy.
///
/// Uses the LLM to analyze the requirement text and codebase context,
/// then classifies the signal type and proposes appropriate verification.
///
/// # Errors
///
/// Returns an error if the LLM call fails or the response cannot be parsed.
pub async fn classify(
    llm: &dyn LlmClient,
    requirement: &str,
    codebase_context: &str,
) -> Result<ClassificationResult, Box<dyn std::error::Error + Send + Sync>> {
    let prompt = build_classification_prompt(requirement, codebase_context);
    let request = CompletionRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        prompt,
        max_tokens: 1024,
    };

    let response = llm.complete(&request).await?;
    parse_classification_response(&response.text)
}

fn build_classification_prompt(requirement: &str, codebase_context: &str) -> String {
    format!(
        r#"Analyze the following requirement and classify its verification signal type.

Requirement: {requirement}

Codebase context:
{codebase_context}

Classify the signal as one of:
1. "clear" - Output is directly observable and assertable
2. "fuzzy" - Observable but soft criteria, can be decomposed into clear sub-assertions
3. "internal" - Depends on internal logic at a specific code point
4. "pushback" - Requirement is under-specified, needs clarification

Respond with a JSON object:
- For "clear": {{"type": "clear", "checks": ["check1", "check2"]}}
- For "fuzzy": {{"type": "fuzzy", "sub_assertions": [{{"description": "...", "check": "..."}}, ...]}}
- For "internal": {{"type": "internal", "approach": "refactor"|"trace", "description": "..."}}
- For "pushback": {{"type": "pushback", "reason": "..."}}

Respond ONLY with the JSON object, no other text."#
    )
}

fn parse_classification_response(
    text: &str,
) -> Result<ClassificationResult, Box<dyn std::error::Error + Send + Sync>> {
    let value: serde_json::Value = serde_json::from_str(text.trim())?;

    let signal_type = value
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or("missing 'type' field in classification response")?;

    match signal_type {
        "clear" => {
            let checks = value
                .get("checks")
                .and_then(|c| c.as_array())
                .ok_or("missing 'checks' field for clear signal")?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            Ok(ClassificationResult::Classified {
                signal_type: SignalType::Clear,
                strategy: VerificationStrategy::DirectAssertion { checks },
            })
        }
        "fuzzy" => {
            let sub_assertions = value
                .get("sub_assertions")
                .and_then(|s| s.as_array())
                .ok_or("missing 'sub_assertions' field for fuzzy signal")?
                .iter()
                .map(|v| {
                    let description =
                        v.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
                    let check = v.get("check").and_then(|c| c.as_str()).unwrap_or("").to_string();
                    SubAssertion { description, check }
                })
                .collect();
            Ok(ClassificationResult::Classified {
                signal_type: SignalType::FuzzyButConstrainable,
                strategy: VerificationStrategy::StructuralDecomposition { sub_assertions },
            })
        }
        "internal" => {
            let approach = value.get("approach").and_then(|a| a.as_str()).unwrap_or("refactor");
            let description =
                value.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
            let strategy = if approach == "trace" {
                VerificationStrategy::TraceAssertion { description }
            } else {
                VerificationStrategy::RefactorToExpose { description }
            };
            Ok(ClassificationResult::Classified {
                signal_type: SignalType::InternalLogic,
                strategy,
            })
        }
        "pushback" => {
            let reason = value
                .get("reason")
                .and_then(|r| r.as_str())
                .unwrap_or("Requirement is under-specified")
                .to_string();
            Ok(ClassificationResult::PushbackRequired { reason })
        }
        other => Err(format!("unknown signal type: {other}").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::adapters::replaying::llm::ReplayingLlmClient;
    use crate::cassette::format::{Cassette, Interaction};
    use crate::cassette::replayer::CassetteReplayer;
    use chrono::Utc;
    use serde_json::json;

    fn llm_from_response(response_text: &str) -> ReplayingLlmClient {
        let cassette = Cassette {
            name: "signal-test".into(),
            recorded_at: Utc::now(),
            commit: "test".into(),
            interactions: vec![Interaction {
                seq: 0,
                port: "llm".into(),
                method: "complete".into(),
                input: json!({}),
                output: json!({"Ok": {
                    "text": response_text,
                    "prompt_tokens": 100,
                    "completion_tokens": 50,
                }}),
            }],
        };
        ReplayingLlmClient::new(Arc::new(Mutex::new(CassetteReplayer::new(&cassette))))
    }

    fn llm_from_error(error_msg: &str) -> ReplayingLlmClient {
        let cassette = Cassette {
            name: "signal-error-test".into(),
            recorded_at: Utc::now(),
            commit: "test".into(),
            interactions: vec![Interaction {
                seq: 0,
                port: "llm".into(),
                method: "complete".into(),
                input: json!({}),
                output: json!({"Err": error_msg}),
            }],
        };
        ReplayingLlmClient::new(Arc::new(Mutex::new(CassetteReplayer::new(&cassette))))
    }

    #[tokio::test]
    async fn classifies_clear_signal() {
        let llm = llm_from_response(
            r#"{"type": "clear", "checks": ["CLI --help lists new subcommand", "Exit code is 0"]}"#,
        );

        let result = classify(
            &llm,
            "The CLI --help should list the new subcommand",
            "src/cli.rs defines subcommands",
        )
        .await
        .unwrap();

        assert_eq!(
            result,
            ClassificationResult::Classified {
                signal_type: SignalType::Clear,
                strategy: VerificationStrategy::DirectAssertion {
                    checks: vec!["CLI --help lists new subcommand".into(), "Exit code is 0".into(),],
                },
            }
        );
    }

    #[tokio::test]
    async fn classifies_fuzzy_signal() {
        let llm = llm_from_response(
            r#"{"type": "fuzzy", "sub_assertions": [{"description": "Events are in date order", "check": "assert timestamps are monotonically increasing"}, {"description": "Each event renders title", "check": "assert each event node contains a title element"}]}"#,
        );

        let result = classify(
            &llm,
            "The timeline shows events in chronological order with reasonable spacing",
            "src/components/timeline.rs",
        )
        .await
        .unwrap();

        match &result {
            ClassificationResult::Classified {
                signal_type,
                strategy: VerificationStrategy::StructuralDecomposition { sub_assertions },
            } => {
                assert_eq!(*signal_type, SignalType::FuzzyButConstrainable);
                assert_eq!(sub_assertions.len(), 2);
                assert_eq!(sub_assertions[0].description, "Events are in date order");
                assert_eq!(
                    sub_assertions[1].check,
                    "assert each event node contains a title element"
                );
            }
            other => panic!("expected fuzzy classification, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn classifies_internal_logic_refactor() {
        let llm = llm_from_response(
            r#"{"type": "internal", "approach": "refactor", "description": "Extract the branching logic at line 42 into a named function should_use_fast_path()"}"#,
        );

        let result = classify(
            &llm,
            "At point Z in the pipeline, branch to Y instead of C",
            "src/pipeline.rs contains the processing logic",
        )
        .await
        .unwrap();

        assert_eq!(
            result,
            ClassificationResult::Classified {
                signal_type: SignalType::InternalLogic,
                strategy: VerificationStrategy::RefactorToExpose {
                    description:
                        "Extract the branching logic at line 42 into a named function should_use_fast_path()"
                            .into(),
                },
            }
        );
    }

    #[tokio::test]
    async fn classifies_internal_logic_trace() {
        let llm = llm_from_response(
            r#"{"type": "internal", "approach": "trace", "description": "Add tracing span at the decision point and assert trace contains expected branch"}"#,
        );

        let result = classify(
            &llm,
            "The cache eviction should prefer LRU entries",
            "src/cache.rs has the eviction logic inline",
        )
        .await
        .unwrap();

        assert_eq!(
            result,
            ClassificationResult::Classified {
                signal_type: SignalType::InternalLogic,
                strategy: VerificationStrategy::TraceAssertion {
                    description:
                        "Add tracing span at the decision point and assert trace contains expected branch"
                            .into(),
                },
            }
        );
    }

    #[tokio::test]
    async fn classifies_pushback_required() {
        let llm = llm_from_response(
            r#"{"type": "pushback", "reason": "The requirement does not specify what 'reasonable spacing' means — need pixel values or relative constraints"}"#,
        );

        let result = classify(&llm, "Make it look good", "src/ui.rs").await.unwrap();

        assert_eq!(
            result,
            ClassificationResult::PushbackRequired {
                reason: "The requirement does not specify what 'reasonable spacing' means — need pixel values or relative constraints".into(),
            }
        );
    }

    #[tokio::test]
    async fn returns_error_on_llm_failure() {
        let llm = llm_from_error("rate limited");

        let result = classify(&llm, "some requirement", "some context").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("rate limited"));
    }

    #[tokio::test]
    async fn returns_error_on_invalid_json() {
        let llm = llm_from_response("this is not json");

        let result = classify(&llm, "some requirement", "some context").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn returns_error_on_unknown_signal_type() {
        let llm = llm_from_response(r#"{"type": "unknown_type"}"#);

        let result = classify(&llm, "some requirement", "some context").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown signal type"));
    }

    #[test]
    fn parse_clear_signal() {
        let json = r#"{"type": "clear", "checks": ["check1", "check2"]}"#;
        let result = parse_classification_response(json).unwrap();
        assert_eq!(
            result,
            ClassificationResult::Classified {
                signal_type: SignalType::Clear,
                strategy: VerificationStrategy::DirectAssertion {
                    checks: vec!["check1".into(), "check2".into()],
                },
            }
        );
    }

    #[test]
    fn parse_pushback_signal() {
        let json = r#"{"type": "pushback", "reason": "under-specified"}"#;
        let result = parse_classification_response(json).unwrap();
        assert_eq!(
            result,
            ClassificationResult::PushbackRequired { reason: "under-specified".into() }
        );
    }
}
