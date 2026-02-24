//! Plan scoring: specificity and verifiability feedback for spec documents.
//!
//! Evaluates a raw requirement document on two dimensions:
//! - **Specificity** (0-100): Are requirements unambiguous and detailed enough?
//! - **Verifiability** (0-100): Can concrete acceptance checks be derived?
//!
//! Returns an overall 0-100 score, per-dimension breakdowns, targeted questions
//! with enumerated options, and actionable recommendations.

use serde::{Deserialize, Serialize};

use crate::ports::llm::{CompletionRequest, LlmClient};

/// Score and issues for a single scoring dimension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubScore {
    /// 0-100 score for this dimension.
    pub score: u8,
    /// Specific issues found, each citing a section or quote from the doc.
    pub issues: Vec<String>,
}

/// A question the user should answer to improve the spec's score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoringQuestion {
    /// Human-readable question about the ambiguity or gap.
    pub description: String,
    /// Concrete options to choose from (2-4 items).
    pub options: Vec<String>,
    /// Index of the recommended option (0-indexed).
    pub recommended: Option<usize>,
}

/// Overall readiness verdict label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// Score >= 85: ready to sync.
    ReadyToSync,
    /// Score 60-84: refinement needed.
    NeedsRefinement,
    /// Score < 60: major revision required.
    MajorRevisionNeeded,
}

impl Verdict {
    /// Returns the human-readable label.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Verdict::ReadyToSync => "Ready to sync",
            Verdict::NeedsRefinement => "Needs refinement",
            Verdict::MajorRevisionNeeded => "Major revision needed",
        }
    }

    /// Derive verdict from a score.
    #[must_use]
    pub fn from_score(score: u8) -> Self {
        if score >= 85 {
            Verdict::ReadyToSync
        } else if score >= 60 {
            Verdict::NeedsRefinement
        } else {
            Verdict::MajorRevisionNeeded
        }
    }
}

/// Result of scoring a spec document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreResult {
    /// Overall 0-100 readiness score.
    pub overall_score: u8,
    /// Readiness verdict.
    pub verdict: Verdict,
    /// Specificity dimension score and issues.
    pub specificity: SubScore,
    /// Verifiability dimension score and issues.
    pub verifiability: SubScore,
    /// Questions with concrete options to resolve remaining gaps.
    pub questions: Vec<ScoringQuestion>,
    /// Actionable edits that would raise the score.
    pub recommendations: Vec<String>,
}

/// Score a spec document by evaluating specificity and verifiability via LLM.
///
/// # Errors
///
/// Returns an error if the LLM call fails or the response cannot be parsed.
pub async fn score_document(llm: &dyn LlmClient, doc_text: &str) -> Result<ScoreResult, String> {
    let prompt = build_scoring_prompt(doc_text);
    let request =
        CompletionRequest { model: "claude-sonnet-4-20250514".into(), prompt, max_tokens: 2048 };

    let response = llm.complete(&request).await.map_err(|e| format!("LLM scoring failed: {e}"))?;

    parse_scoring_response(&response.text)
}

/// Build the LLM prompt for scoring a spec document.
fn build_scoring_prompt(doc_text: &str) -> String {
    format!(
        "You are a spec quality evaluator. Score this requirements document on two dimensions.\n\n\
         ## Document\n\n\
         {doc_text}\n\n\
         ## Scoring Dimensions\n\n\
         **Specificity (0-100):** Are requirements unambiguous and detailed enough that an AI \
         agent could implement them without asking clarifying questions? Deduct points for: vague \
         language (\"good\", \"nice\", \"should work well\"), missing edge cases, unstated \
         constraints, multiple valid interpretations, missing data formats or type details.\n\n\
         **Verifiability (0-100):** Can concrete, executable acceptance checks be derived from \
         the requirements? Deduct points for: subjective quality criteria, no testable outcomes, \
         \"it should work\" without measurable criteria, missing success/failure conditions, no \
         observable outputs to assert against.\n\n\
         **Overall score** = round((specificity * 0.5) + (verifiability * 0.5)).\n\n\
         ## Instructions\n\n\
         Respond with JSON only (no markdown fences):\n\
         {{\n  \
           \"specificity_score\": 75,\n  \
           \"specificity_issues\": [\n    \
             \"Section 'Authentication': 'secure login' is vague — what algorithm, token TTL?\"\n  \
           ],\n  \
           \"verifiability_score\": 60,\n  \
           \"verifiability_issues\": [\n    \
             \"No testable output defined for the export feature\"\n  \
           ],\n  \
           \"questions\": [\n    \
             {{\n      \
               \"description\": \"What token format should auth use?\",\n      \
               \"options\": [\"JWT (recommended)\", \"Opaque session token\", \"API key\"],\n      \
               \"recommended\": 0\n    \
             }}\n  \
           ],\n  \
           \"recommendations\": [\n    \
             \"Add: 'Auth tokens use JWT, expire after 24h, and are validated via RS256.'\",\n    \
             \"Add acceptance criterion: 'POST /login returns 200 with {{\\\"token\\\": \\\"...\\\"}}'\"\n  \
           ]\n\
         }}\n\n\
         Rules:\n\
         - specificity_score and verifiability_score must be integers 0-100\n\
         - specificity_issues and verifiability_issues: cite specific sections or quote text from the doc\n\
         - questions: 2-4 options each, recommended is 0-indexed (or null)\n\
         - recommendations: concrete edits, not vague advice\n\
         - If the doc scores >= 85 overall, questions may be empty\n"
    )
}

/// Parse the LLM scoring response into a `ScoreResult`.
fn parse_scoring_response(response: &str) -> Result<ScoreResult, String> {
    #[derive(Deserialize)]
    struct LlmResponse {
        specificity_score: u8,
        #[serde(default)]
        specificity_issues: Vec<String>,
        verifiability_score: u8,
        #[serde(default)]
        verifiability_issues: Vec<String>,
        #[serde(default)]
        questions: Vec<QuestionResponse>,
        #[serde(default)]
        recommendations: Vec<String>,
    }

    #[derive(Deserialize)]
    struct QuestionResponse {
        description: String,
        #[serde(default)]
        options: Vec<String>,
        #[serde(default)]
        recommended: Option<usize>,
    }

    let parsed: LlmResponse = serde_json::from_str(super::extract_json(response))
        .map_err(|e| format!("failed to parse scoring response: {e}"))?;

    let overall_score = u8::try_from(u16::midpoint(
        u16::from(parsed.specificity_score),
        u16::from(parsed.verifiability_score),
    ))
    .unwrap_or(100);

    let verdict = Verdict::from_score(overall_score);

    Ok(ScoreResult {
        overall_score,
        verdict,
        specificity: SubScore {
            score: parsed.specificity_score,
            issues: parsed.specificity_issues,
        },
        verifiability: SubScore {
            score: parsed.verifiability_score,
            issues: parsed.verifiability_issues,
        },
        questions: parsed
            .questions
            .into_iter()
            .map(|q| ScoringQuestion {
                description: q.description,
                options: q.options,
                recommended: q.recommended,
            })
            .collect(),
        recommendations: parsed.recommendations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- parse_scoring_response tests ---

    #[test]
    fn parse_full_scoring_response() {
        let response = serde_json::to_string(&json!({
            "specificity_score": 70,
            "specificity_issues": ["Section 'Auth': 'secure' is vague"],
            "verifiability_score": 60,
            "verifiability_issues": ["No testable output defined"],
            "questions": [{
                "description": "What token format?",
                "options": ["JWT", "Opaque token"],
                "recommended": 0
            }],
            "recommendations": ["Add: 'Tokens expire after 24h'"]
        }))
        .unwrap();

        let result = parse_scoring_response(&response).unwrap();
        assert_eq!(result.overall_score, 65);
        assert_eq!(result.verdict, Verdict::NeedsRefinement);
        assert_eq!(result.specificity.score, 70);
        assert_eq!(result.specificity.issues.len(), 1);
        assert_eq!(result.verifiability.score, 60);
        assert_eq!(result.verifiability.issues.len(), 1);
        assert_eq!(result.questions.len(), 1);
        assert_eq!(result.questions[0].recommended, Some(0));
        assert_eq!(result.recommendations.len(), 1);
    }

    #[test]
    fn parse_high_score_ready_to_sync() {
        let response = serde_json::to_string(&json!({
            "specificity_score": 90,
            "specificity_issues": [],
            "verifiability_score": 88,
            "verifiability_issues": [],
            "questions": [],
            "recommendations": []
        }))
        .unwrap();

        let result = parse_scoring_response(&response).unwrap();
        assert_eq!(result.overall_score, 89);
        assert_eq!(result.verdict, Verdict::ReadyToSync);
        assert!(result.questions.is_empty());
        assert!(result.recommendations.is_empty());
    }

    #[test]
    fn parse_low_score_major_revision() {
        let response = serde_json::to_string(&json!({
            "specificity_score": 30,
            "specificity_issues": ["Requirement is too vague"],
            "verifiability_score": 40,
            "verifiability_issues": ["No acceptance criteria"],
            "questions": [{
                "description": "What is the expected output format?",
                "options": ["JSON", "CSV", "Plain text"],
                "recommended": null
            }],
            "recommendations": ["Define output format explicitly"]
        }))
        .unwrap();

        let result = parse_scoring_response(&response).unwrap();
        assert_eq!(result.overall_score, 35);
        assert_eq!(result.verdict, Verdict::MajorRevisionNeeded);
        assert_eq!(result.questions[0].recommended, None);
    }

    #[test]
    fn parse_rejects_invalid_json() {
        let result = parse_scoring_response("not json at all");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to parse scoring response"));
    }

    #[test]
    fn parse_minimal_response_uses_defaults() {
        let response = serde_json::to_string(&json!({
            "specificity_score": 50,
            "verifiability_score": 50
        }))
        .unwrap();

        let result = parse_scoring_response(&response).unwrap();
        assert_eq!(result.overall_score, 50);
        assert!(result.specificity.issues.is_empty());
        assert!(result.verifiability.issues.is_empty());
        assert!(result.questions.is_empty());
        assert!(result.recommendations.is_empty());
    }

    // --- Verdict tests ---

    #[test]
    fn verdict_from_score_thresholds() {
        assert_eq!(Verdict::from_score(85), Verdict::ReadyToSync);
        assert_eq!(Verdict::from_score(100), Verdict::ReadyToSync);
        assert_eq!(Verdict::from_score(84), Verdict::NeedsRefinement);
        assert_eq!(Verdict::from_score(60), Verdict::NeedsRefinement);
        assert_eq!(Verdict::from_score(59), Verdict::MajorRevisionNeeded);
        assert_eq!(Verdict::from_score(0), Verdict::MajorRevisionNeeded);
    }

    #[test]
    fn verdict_labels() {
        assert_eq!(Verdict::ReadyToSync.label(), "Ready to sync");
        assert_eq!(Verdict::NeedsRefinement.label(), "Needs refinement");
        assert_eq!(Verdict::MajorRevisionNeeded.label(), "Major revision needed");
    }

    // --- build_scoring_prompt tests ---

    #[test]
    fn prompt_includes_document_text() {
        let doc = "Add user authentication with JWT tokens.";
        let prompt = build_scoring_prompt(doc);
        assert!(prompt.contains(doc));
        assert!(prompt.contains("specificity_score"));
        assert!(prompt.contains("verifiability_score"));
        assert!(prompt.contains("questions"));
        assert!(prompt.contains("recommendations"));
    }

    // --- Integration test with cassette ---

    #[tokio::test]
    async fn score_document_via_cassette() {
        use crate::cassette::format::{Cassette, Interaction};
        use chrono::Utc;

        let dir = std::env::temp_dir().join("speck_score_test");
        std::fs::create_dir_all(&dir).unwrap();

        let score_response = serde_json::to_string(&json!({
            "specificity_score": 75,
            "specificity_issues": ["Missing edge case for empty input"],
            "verifiability_score": 80,
            "verifiability_issues": [],
            "questions": [{
                "description": "How should empty input be handled?",
                "options": ["Return error 400", "Return empty list"],
                "recommended": 0
            }],
            "recommendations": ["Add: 'Empty query returns HTTP 400 with error message'"]
        }))
        .unwrap();

        let cassette = Cassette {
            name: "score_test".into(),
            recorded_at: Utc::now(),
            commit: "abc".into(),
            interactions: vec![Interaction {
                seq: 0,
                port: "llm".into(),
                method: "complete".into(),
                input: json!({}),
                output: json!({
                    "ok": {
                        "text": score_response,
                        "prompt_tokens": 300,
                        "completion_tokens": 100
                    }
                }),
            }],
        };

        let yaml = serde_yaml::to_string(&cassette).unwrap();
        let cassette_path = dir.join("score_test.cassette.yaml");
        std::fs::write(&cassette_path, yaml).unwrap();

        use crate::context::ServiceContext;
        let ctx = ServiceContext::replaying(&cassette_path).unwrap();

        let result = score_document(ctx.llm.as_ref(), "Add search with query param").await.unwrap();

        assert_eq!(result.overall_score, 77);
        assert_eq!(result.verdict, Verdict::NeedsRefinement);
        assert_eq!(result.specificity.score, 75);
        assert_eq!(result.verifiability.score, 80);
        assert_eq!(result.questions.len(), 1);
        assert_eq!(result.recommendations.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
