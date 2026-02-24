//! Single-pass spec analysis for `speck plan`.
//!
//! Analyzes task specs via LLM to identify unresolved questions,
//! verification gaps, and ambiguous requirements. Outputs structured
//! feedback with enumerated options and recommendations.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::context::ServiceContext;
use crate::ports::llm::CompletionRequest;
use crate::spec::TaskSpec;

/// A question the planner needs answered before specs are fully resolved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PushbackQuestion {
    /// The task spec ID this question relates to.
    pub task_id: String,
    /// Human-readable description of the problem.
    pub description: String,
    /// Proposed options (labeled a, b, c, ...).
    pub options: Vec<String>,
    /// Index of the recommended option (0-indexed), if any.
    pub recommended: Option<usize>,
}

/// Result of a single-pass spec analysis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalysisResult {
    /// Summary of findings across all specs.
    pub summary: String,
    /// Questions requiring user input (empty when all specs are resolved).
    pub questions: Vec<PushbackQuestion>,
}

/// A single requirement item extracted from a multi-item PRD.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrdItem {
    /// Human-readable title for this item.
    pub title: String,
    /// The requirement text for this item.
    pub requirement: String,
    /// Indices (0-based) of other items this one depends on.
    #[serde(default)]
    pub depends_on: Vec<usize>,
}

/// Result of decomposing a PRD into individual items.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecompositionResult {
    /// The individual items extracted from the PRD.
    pub items: Vec<PrdItem>,
}

/// Decomposes a PRD document into individual requirement items via LLM.
///
/// If the PRD contains multiple distinct features or subsections, each is
/// extracted as a separate `PrdItem` with its own title and requirement text.
/// A single-item PRD produces a single-element result.
///
/// # Errors
///
/// Returns an error if the LLM call fails or the response cannot be parsed.
pub async fn decompose_prd(
    ctx: &ServiceContext,
    requirement_text: &str,
) -> Result<DecompositionResult, String> {
    let prompt = build_decomposition_prompt(requirement_text);
    let request =
        CompletionRequest { model: "claude-sonnet-4-20250514".into(), prompt, max_tokens: 4096 };

    let response =
        ctx.llm.complete(&request).await.map_err(|e| format!("LLM decomposition failed: {e}"))?;

    parse_decomposition_response(&response.text)
}

/// Builds the LLM prompt for PRD decomposition.
fn build_decomposition_prompt(requirement_text: &str) -> String {
    format!(
        r#"Analyze the following PRD/spec document and decompose it into individual work items.

## Document

{requirement_text}

## Instructions

Identify distinct features, changes, or work items described in this document. Each numbered section, feature subsection, or independent change should become a separate item.

Respond with JSON (no markdown fences):
{{
  "items": [
    {{
      "title": "Short descriptive title for this item",
      "requirement": "The full requirement text for this specific item",
      "depends_on": [0]
    }}
  ]
}}

Rules:
- Each item should be a self-contained work unit with its own requirement text.
- "depends_on" is an array of 0-based indices of other items this one depends on. Use empty array if no dependencies.
- If the document describes only a single feature/change, return exactly one item.
- Preserve the document's own ordering and structure.
- Do NOT split a single coherent feature into artificial sub-parts. Only split when the document itself delineates separate items.
- Keep the requirement text detailed enough to implement independently."#
    )
}

/// Parses the LLM decomposition response into a `DecompositionResult`.
fn parse_decomposition_response(response: &str) -> Result<DecompositionResult, String> {
    let parsed: DecompositionResult = serde_json::from_str(super::extract_json(response))
        .map_err(|e| format!("failed to parse LLM decomposition response: {e}"))?;

    if parsed.items.is_empty() {
        return Err("decomposition produced zero items".into());
    }

    Ok(parsed)
}

/// Analyzes task specs via LLM in a single pass, returning structured feedback.
///
/// Identifies specs lacking proper verification strategies or with ambiguous
/// requirements, and proposes concrete options with recommendations.
///
/// # Errors
///
/// Returns an error if the LLM call fails or the response cannot be parsed.
pub async fn analyze_specs(
    ctx: &ServiceContext,
    specs: &[TaskSpec],
) -> Result<AnalysisResult, String> {
    let prompt = build_analysis_prompt(specs);
    let request =
        CompletionRequest { model: "claude-sonnet-4-20250514".into(), prompt, max_tokens: 4096 };

    let response =
        ctx.llm.complete(&request).await.map_err(|e| format!("LLM analysis failed: {e}"))?;

    parse_analysis_response(&response.text)
}

/// Builds the LLM prompt for analyzing current specs.
fn build_analysis_prompt(specs: &[TaskSpec]) -> String {
    let mut prompt = String::new();
    prompt.push_str(
        "Analyze these task specs and identify any that lack proper verification strategies \
         or have ambiguous requirements.\n\n",
    );

    prompt.push_str("## Task Specs\n\n");
    for spec in specs {
        let _ = writeln!(prompt, "### {} — {}", spec.id, spec.title);
        if let Some(req) = &spec.requirement {
            let _ = writeln!(prompt, "Requirement: {req}");
        }
        let _ = writeln!(prompt, "Signal type: {:?}", spec.signal_type);
        let _ = writeln!(prompt, "Acceptance criteria:");
        for ac in &spec.acceptance_criteria {
            let _ = writeln!(prompt, "  - {ac}");
        }
        let _ = writeln!(prompt, "Verification: {:?}\n", spec.verification);
    }

    prompt.push_str(
        "## Instructions\n\n\
         Respond with JSON (no markdown fences):\n\
         {\n  \
           \"summary\": \"Brief overview of findings\",\n  \
           \"questions\": [\n    \
             {\n      \
               \"task_id\": \"TASK-ID\",\n      \
               \"description\": \"What's unclear or unverifiable\",\n      \
               \"options\": [\"option a description\", \"option b description\"],\n      \
               \"recommended\": 0\n    \
             }\n  \
           ]\n\
         }\n\n\
         - If all specs have clear verification strategies, return an empty questions array.\n\
         - Each question should offer 2-3 concrete options.\n\
         - Focus on verification strategy gaps and ambiguous acceptance criteria.\n\
         - Set \"recommended\" to the 0-indexed option you recommend (or null if no preference).\n",
    );

    prompt
}

/// Parses the LLM analysis response into an `AnalysisResult`.
fn parse_analysis_response(response: &str) -> Result<AnalysisResult, String> {
    #[derive(Deserialize)]
    struct LlmResponse {
        summary: String,
        #[serde(default)]
        questions: Vec<QuestionResponse>,
    }

    #[derive(Deserialize)]
    struct QuestionResponse {
        task_id: String,
        description: String,
        #[serde(default)]
        options: Vec<String>,
        #[serde(default)]
        recommended: Option<usize>,
    }

    let parsed: LlmResponse = serde_json::from_str(super::extract_json(response))
        .map_err(|e| format!("failed to parse LLM analysis response: {e}"))?;

    let questions = parsed
        .questions
        .into_iter()
        .map(|q| PushbackQuestion {
            task_id: q.task_id,
            description: q.description,
            options: q.options,
            recommended: q.recommended,
        })
        .collect();

    Ok(AnalysisResult { summary: parsed.summary, questions })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
    use crate::context::ServiceContext;
    use crate::spec::{SignalType, VerificationCheck, VerificationStrategy};
    use chrono::Utc;
    use serde_json::json;
    use std::path::Path;

    /// Helper to write a cassette file and return its path.
    fn write_cassette(
        dir: &Path,
        name: &str,
        interactions: Vec<Interaction>,
    ) -> std::path::PathBuf {
        let cassette = Cassette {
            name: name.into(),
            recorded_at: Utc::now(),
            commit: "abc".into(),
            interactions,
        };
        let yaml = serde_yaml::to_string(&cassette).unwrap();
        let path = dir.join(format!("{name}.cassette.yaml"));
        std::fs::write(&path, yaml).unwrap();
        path
    }

    fn sample_spec(id: &str, title: &str, has_verification: bool) -> TaskSpec {
        let verification = if has_verification {
            VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::TestSuite {
                    command: "cargo test".into(),
                    expected: "all pass".into(),
                }],
            }
        } else {
            VerificationStrategy::DirectAssertion { checks: vec![] }
        };

        TaskSpec {
            id: id.into(),
            title: title.into(),
            requirement: Some("req-1".into()),
            context: None,
            acceptance_criteria: vec!["it works".into()],
            signal_type: SignalType::Clear,
            verification,
            affected_globs: None,
        }
    }

    // --- parse_analysis_response tests ---

    #[test]
    fn parse_analysis_with_questions() {
        let response = serde_json::to_string(&json!({
            "summary": "Task 1 has no verification strategy",
            "questions": [{
                "task_id": "TASK-1",
                "description": "No component test infrastructure exists",
                "options": [
                    "Add foundational task for component tests",
                    "Use structural assertions only"
                ],
                "recommended": 0
            }]
        }))
        .unwrap();

        let result = parse_analysis_response(&response).unwrap();
        assert_eq!(result.summary, "Task 1 has no verification strategy");
        assert_eq!(result.questions.len(), 1);
        assert_eq!(result.questions[0].task_id, "TASK-1");
        assert_eq!(result.questions[0].options.len(), 2);
        assert_eq!(result.questions[0].recommended, Some(0));
    }

    #[test]
    fn parse_analysis_all_resolved() {
        let response = serde_json::to_string(&json!({
            "summary": "All specs have verification strategies",
            "questions": []
        }))
        .unwrap();

        let result = parse_analysis_response(&response).unwrap();
        assert!(result.questions.is_empty());
    }

    #[test]
    fn parse_analysis_rejects_invalid_json() {
        let result = parse_analysis_response("not json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to parse"));
    }

    #[test]
    fn parse_analysis_without_recommended() {
        let response = serde_json::to_string(&json!({
            "summary": "Needs work",
            "questions": [{
                "task_id": "TASK-1",
                "description": "Missing tests",
                "options": ["Add tests", "Skip"]
            }]
        }))
        .unwrap();

        let result = parse_analysis_response(&response).unwrap();
        assert_eq!(result.questions[0].recommended, None);
    }

    // --- build_analysis_prompt tests ---

    #[test]
    fn analysis_prompt_includes_spec_details() {
        let specs = vec![sample_spec("TASK-1", "Build UI", false)];
        let prompt = build_analysis_prompt(&specs);
        assert!(prompt.contains("TASK-1"));
        assert!(prompt.contains("Build UI"));
        assert!(prompt.contains("Clear"));
        assert!(prompt.contains("it works"));
        assert!(prompt.contains("recommended"));
    }

    // --- parse_decomposition_response tests ---

    #[test]
    fn parse_decomposition_multiple_items() {
        let response = serde_json::to_string(&json!({
            "items": [
                {
                    "title": "Add user registration",
                    "requirement": "Users can sign up with email and password",
                    "depends_on": []
                },
                {
                    "title": "Add login flow",
                    "requirement": "Users can log in with their credentials",
                    "depends_on": [0]
                },
                {
                    "title": "Add profile page",
                    "requirement": "Users can view and edit their profile",
                    "depends_on": [0, 1]
                }
            ]
        }))
        .unwrap();

        let result = parse_decomposition_response(&response).unwrap();
        assert_eq!(result.items.len(), 3);
        assert_eq!(result.items[0].title, "Add user registration");
        assert!(result.items[0].depends_on.is_empty());
        assert_eq!(result.items[1].title, "Add login flow");
        assert_eq!(result.items[1].depends_on, vec![0]);
        assert_eq!(result.items[2].depends_on, vec![0, 1]);
    }

    #[test]
    fn parse_decomposition_single_item() {
        let response = serde_json::to_string(&json!({
            "items": [{
                "title": "Add CSV export",
                "requirement": "Export data as CSV",
                "depends_on": []
            }]
        }))
        .unwrap();

        let result = parse_decomposition_response(&response).unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Add CSV export");
    }

    #[test]
    fn parse_decomposition_rejects_empty_items() {
        let response = serde_json::to_string(&json!({ "items": [] })).unwrap();
        let result = parse_decomposition_response(&response);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("zero items"));
    }

    #[test]
    fn parse_decomposition_rejects_invalid_json() {
        let result = parse_decomposition_response("not json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to parse"));
    }

    #[test]
    fn decomposition_prompt_includes_document() {
        let prompt = build_decomposition_prompt("# My PRD\n\n## Feature 1\nDo stuff.");
        assert!(prompt.contains("# My PRD"));
        assert!(prompt.contains("## Feature 1"));
        assert!(prompt.contains("depends_on"));
    }

    // --- analyze_specs integration test ---

    #[tokio::test]
    async fn analyze_specs_returns_structured_feedback() {
        let dir = std::env::temp_dir().join("speck_analysis_test_feedback");
        std::fs::create_dir_all(&dir).unwrap();

        let analysis_response = serde_json::to_string(&json!({
            "summary": "Task 1 needs verification strategy",
            "questions": [{
                "task_id": "TASK-1",
                "description": "No test infrastructure",
                "options": ["Add tests", "Use structural assertions"],
                "recommended": 0
            }]
        }))
        .unwrap();

        let interactions = vec![Interaction {
            seq: 0,
            port: "llm".into(),
            method: "complete".into(),
            input: json!({}),
            output: json!({
                "ok": {
                    "text": analysis_response,
                    "prompt_tokens": 200,
                    "completion_tokens": 50
                }
            }),
        }];

        let cassette_path = write_cassette(&dir, "analysis_feedback", interactions);
        let ctx = ServiceContext::replaying(&cassette_path).unwrap();

        let specs = vec![sample_spec("TASK-1", "Build UI", false)];
        let result = analyze_specs(&ctx, &specs).await.unwrap();

        assert_eq!(result.summary, "Task 1 needs verification strategy");
        assert_eq!(result.questions.len(), 1);
        assert_eq!(result.questions[0].recommended, Some(0));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
