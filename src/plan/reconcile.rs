//! Pass 2.5: Reconciliation pass for the two-pass planning system.
//!
//! After all deep dives, reviews all proposed task specs together to identify
//! duplicated efforts, shared abstractions that should be extracted, and
//! dependency ordering issues. This is cheap — it reads task specs, not code.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::context::ServiceContext;
use crate::ports::llm::{CompletionRequest, CompletionResponse};
use crate::spec::TaskSpec;

/// A suggestion to merge two or more tasks that duplicate effort.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MergeSuggestion {
    /// IDs of the tasks that overlap.
    pub task_ids: Vec<String>,
    /// Why these tasks are considered duplicates.
    pub reason: String,
    /// Proposed merged title.
    pub merged_title: String,
}

/// A suggestion to extract a shared abstraction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractionSuggestion {
    /// IDs of the tasks that would benefit from this extraction.
    pub task_ids: Vec<String>,
    /// Description of the shared abstraction.
    pub abstraction: String,
    /// Suggested new foundational task title.
    pub suggested_task_title: String,
}

/// A suggested reordering to fix dependency issues.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReorderSuggestion {
    /// The task that should come earlier.
    pub task_id: String,
    /// The task it should precede.
    pub should_precede: String,
    /// Why this reordering is needed.
    pub reason: String,
}

/// Result of the reconciliation pass.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReconciliationResult {
    /// Tasks that duplicate effort and could be merged.
    pub suggested_merges: Vec<MergeSuggestion>,
    /// Shared abstractions that should be extracted into new foundational tasks.
    pub suggested_extractions: Vec<ExtractionSuggestion>,
    /// Dependency reorderings to fix circular deps or ordering issues.
    pub suggested_reorders: Vec<ReorderSuggestion>,
    /// Circular dependency chains detected (each entry is a cycle of task IDs).
    pub circular_dependencies: Vec<Vec<String>>,
}

/// Performs the reconciliation pass (Pass 2.5 of two-pass planning).
///
/// Reviews all task specs together after deep dives to identify:
/// - Duplicated efforts (same files modified, similar acceptance criteria)
/// - Shared abstractions that multiple tasks touch similarly
/// - Dependency ordering issues (circular deps, foundational tasks not first)
///
/// # Errors
///
/// Returns an error if LLM analysis fails or the response cannot be parsed.
pub async fn reconcile(
    ctx: &ServiceContext,
    task_specs: &[TaskSpec],
) -> Result<ReconciliationResult, String> {
    // First, detect circular dependencies locally (no LLM needed).
    let circular = detect_circular_dependencies(task_specs);

    // Then ask the LLM to analyze for duplicates, shared abstractions, and ordering.
    let prompt = build_reconciliation_prompt(task_specs, &circular);
    let request =
        CompletionRequest { model: "claude-sonnet-4-20250514".into(), prompt, max_tokens: 4096 };

    let response: CompletionResponse =
        ctx.llm.complete(&request).await.map_err(|e| format!("LLM reconciliation failed: {e}"))?;

    parse_reconciliation_response(&response.text, circular)
}

/// Detects circular dependencies among task specs using their context.dependencies.
fn detect_circular_dependencies(specs: &[TaskSpec]) -> Vec<Vec<String>> {
    // Build adjacency map from task dependencies.
    let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
    let spec_ids: HashSet<&str> = specs.iter().map(|s| s.id.as_str()).collect();

    for spec in specs {
        let deps: Vec<&str> = spec
            .context
            .as_ref()
            .map(|c| {
                c.dependencies.iter().map(String::as_str).filter(|d| spec_ids.contains(d)).collect()
            })
            .unwrap_or_default();
        graph.insert(spec.id.as_str(), deps);
    }

    // Find cycles using DFS with coloring.
    let mut cycles: Vec<Vec<String>> = Vec::new();
    let mut visited: HashSet<&str> = HashSet::new();
    let mut on_stack: HashSet<&str> = HashSet::new();
    let mut stack: Vec<&str> = Vec::new();

    for id in &spec_ids {
        if !visited.contains(id) {
            dfs_find_cycles(id, &graph, &mut visited, &mut on_stack, &mut stack, &mut cycles);
        }
    }

    cycles
}

/// DFS helper for cycle detection.
fn dfs_find_cycles<'a>(
    node: &'a str,
    graph: &HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    on_stack: &mut HashSet<&'a str>,
    stack: &mut Vec<&'a str>,
    cycles: &mut Vec<Vec<String>>,
) {
    visited.insert(node);
    on_stack.insert(node);
    stack.push(node);

    if let Some(deps) = graph.get(node) {
        for &dep in deps {
            if !visited.contains(dep) {
                dfs_find_cycles(dep, graph, visited, on_stack, stack, cycles);
            } else if on_stack.contains(dep) {
                // Found a cycle — extract it from the stack.
                let cycle_start = stack.iter().position(|&n| n == dep).unwrap();
                let cycle: Vec<String> =
                    stack[cycle_start..].iter().map(|s| (*s).to_string()).collect();
                cycles.push(cycle);
            }
        }
    }

    stack.pop();
    on_stack.remove(node);
}

/// Builds the LLM prompt for reconciliation analysis.
fn build_reconciliation_prompt(specs: &[TaskSpec], circular: &[Vec<String>]) -> String {
    let mut prompt = String::new();

    prompt.push_str(
        "Review these task specs together and identify issues.\n\n\
         ## Task Specs\n\n",
    );

    for spec in specs {
        let _ = writeln!(prompt, "### {} — {}", spec.id, spec.title);
        if let Some(req) = &spec.requirement {
            let _ = writeln!(prompt, "Requirement: {req}");
        }
        if let Some(ctx) = &spec.context {
            if !ctx.modules.is_empty() {
                let _ = writeln!(prompt, "Modules: {}", ctx.modules.join(", "));
            }
            if !ctx.dependencies.is_empty() {
                let _ = writeln!(prompt, "Dependencies: {}", ctx.dependencies.join(", "));
            }
        }
        let _ = writeln!(prompt, "Acceptance criteria:");
        for ac in &spec.acceptance_criteria {
            let _ = writeln!(prompt, "  - {ac}");
        }
        prompt.push('\n');
    }

    if !circular.is_empty() {
        prompt.push_str("## Detected Circular Dependencies\n\n");
        for cycle in circular {
            let _ = writeln!(prompt, "- Cycle: {}", cycle.join(" -> "));
        }
        prompt.push('\n');
    }

    prompt.push_str(
        "## Instructions\n\n\
         Respond with JSON (no markdown fences):\n\
         {\n  \
           \"merges\": [\n    \
             {\"task_ids\": [\"ID1\", \"ID2\"], \"reason\": \"why\", \"merged_title\": \"title\"}\n  \
           ],\n  \
           \"extractions\": [\n    \
             {\"task_ids\": [\"ID1\", \"ID2\"], \"abstraction\": \"what\", \"suggested_task_title\": \"title\"}\n  \
           ],\n  \
           \"reorders\": [\n    \
             {\"task_id\": \"ID\", \"should_precede\": \"ID2\", \"reason\": \"why\"}\n  \
           ]\n\
         }\n\n\
         - merges: Tasks with overlapping work (same files, similar criteria) that should be combined.\n\
         - extractions: Shared abstractions multiple tasks need that should become foundational tasks.\n\
         - reorders: Tasks that should come earlier because others depend on their output.\n\
         - Return empty arrays if no issues are found.\n",
    );

    prompt
}

/// Parses the LLM reconciliation response into a `ReconciliationResult`.
fn parse_reconciliation_response(
    response: &str,
    circular: Vec<Vec<String>>,
) -> Result<ReconciliationResult, String> {
    #[derive(Deserialize)]
    struct LlmResponse {
        #[serde(default)]
        merges: Vec<MergeSuggestion>,
        #[serde(default)]
        extractions: Vec<ExtractionSuggestion>,
        #[serde(default)]
        reorders: Vec<ReorderSuggestion>,
    }

    let parsed: LlmResponse = serde_json::from_str(response)
        .map_err(|e| format!("failed to parse LLM reconciliation response: {e}"))?;

    Ok(ReconciliationResult {
        suggested_merges: parsed.merges,
        suggested_extractions: parsed.extractions,
        suggested_reorders: parsed.reorders,
        circular_dependencies: circular,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
    use crate::context::ServiceContext;
    use crate::spec::{SignalType, TaskContext, VerificationCheck, VerificationStrategy};
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

    fn sample_spec(id: &str, title: &str, modules: &[&str], deps: &[&str]) -> TaskSpec {
        TaskSpec {
            id: id.into(),
            title: title.into(),
            requirement: Some("req-1".into()),
            context: Some(TaskContext {
                modules: modules.iter().map(|s| (*s).to_string()).collect(),
                patterns: None,
                dependencies: deps.iter().map(|s| (*s).to_string()).collect(),
            }),
            acceptance_criteria: vec!["it works".into()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::TestSuite {
                    command: "cargo test".into(),
                    expected: "all pass".into(),
                }],
            },
        }
    }

    // --- detect_circular_dependencies tests ---

    #[test]
    fn no_circular_deps() {
        let specs = vec![
            sample_spec("T1", "Task 1", &["mod_a"], &[]),
            sample_spec("T2", "Task 2", &["mod_b"], &["T1"]),
            sample_spec("T3", "Task 3", &["mod_c"], &["T2"]),
        ];
        let cycles = detect_circular_dependencies(&specs);
        assert!(cycles.is_empty());
    }

    #[test]
    fn detects_simple_cycle() {
        let specs = vec![
            sample_spec("T1", "Task 1", &["mod_a"], &["T2"]),
            sample_spec("T2", "Task 2", &["mod_b"], &["T1"]),
        ];
        let cycles = detect_circular_dependencies(&specs);
        assert!(!cycles.is_empty());
        // The cycle should contain both T1 and T2.
        let flat: HashSet<String> = cycles.into_iter().flatten().collect();
        assert!(flat.contains("T1"));
        assert!(flat.contains("T2"));
    }

    #[test]
    fn detects_three_node_cycle() {
        let specs = vec![
            sample_spec("T1", "Task 1", &[], &["T2"]),
            sample_spec("T2", "Task 2", &[], &["T3"]),
            sample_spec("T3", "Task 3", &[], &["T1"]),
        ];
        let cycles = detect_circular_dependencies(&specs);
        assert!(!cycles.is_empty());
        let flat: HashSet<String> = cycles.into_iter().flatten().collect();
        assert!(flat.contains("T1"));
        assert!(flat.contains("T2"));
        assert!(flat.contains("T3"));
    }

    #[test]
    fn ignores_external_dependencies() {
        // T1 depends on "EXTERNAL" which is not in the spec list.
        let specs = vec![
            sample_spec("T1", "Task 1", &[], &["EXTERNAL"]),
            sample_spec("T2", "Task 2", &[], &["T1"]),
        ];
        let cycles = detect_circular_dependencies(&specs);
        assert!(cycles.is_empty());
    }

    // --- build_reconciliation_prompt tests ---

    #[test]
    fn prompt_includes_specs_and_cycles() {
        let specs = vec![
            sample_spec("T1", "Auth module", &["auth"], &["T2"]),
            sample_spec("T2", "Login UI", &["ui"], &["T1"]),
        ];
        let circular = vec![vec!["T1".into(), "T2".into()]];

        let prompt = build_reconciliation_prompt(&specs, &circular);
        assert!(prompt.contains("T1 — Auth module"));
        assert!(prompt.contains("T2 — Login UI"));
        assert!(prompt.contains("Modules: auth"));
        assert!(prompt.contains("Dependencies: T1"));
        assert!(prompt.contains("Cycle: T1 -> T2"));
        assert!(prompt.contains("merges"));
        assert!(prompt.contains("extractions"));
        assert!(prompt.contains("reorders"));
    }

    #[test]
    fn prompt_omits_circular_section_when_empty() {
        let specs = vec![sample_spec("T1", "Task 1", &["mod_a"], &[])];
        let prompt = build_reconciliation_prompt(&specs, &[]);
        assert!(!prompt.contains("Circular Dependencies"));
    }

    // --- parse_reconciliation_response tests ---

    #[test]
    fn parse_response_with_all_suggestions() {
        let response = serde_json::to_string(&json!({
            "merges": [{
                "task_ids": ["T1", "T2"],
                "reason": "Both modify auth module",
                "merged_title": "Unified auth implementation"
            }],
            "extractions": [{
                "task_ids": ["T1", "T3"],
                "abstraction": "Shared validation logic",
                "suggested_task_title": "Extract validation utilities"
            }],
            "reorders": [{
                "task_id": "T3",
                "should_precede": "T1",
                "reason": "T1 uses types defined in T3"
            }]
        }))
        .unwrap();

        let result = parse_reconciliation_response(&response, vec![]).unwrap();
        assert_eq!(result.suggested_merges.len(), 1);
        assert_eq!(result.suggested_merges[0].task_ids, vec!["T1", "T2"]);
        assert_eq!(result.suggested_merges[0].merged_title, "Unified auth implementation");

        assert_eq!(result.suggested_extractions.len(), 1);
        assert_eq!(result.suggested_extractions[0].abstraction, "Shared validation logic");

        assert_eq!(result.suggested_reorders.len(), 1);
        assert_eq!(result.suggested_reorders[0].task_id, "T3");
        assert_eq!(result.suggested_reorders[0].should_precede, "T1");

        assert!(result.circular_dependencies.is_empty());
    }

    #[test]
    fn parse_response_empty_suggestions() {
        let response = serde_json::to_string(&json!({
            "merges": [],
            "extractions": [],
            "reorders": []
        }))
        .unwrap();

        let result = parse_reconciliation_response(&response, vec![]).unwrap();
        assert!(result.suggested_merges.is_empty());
        assert!(result.suggested_extractions.is_empty());
        assert!(result.suggested_reorders.is_empty());
    }

    #[test]
    fn parse_response_preserves_circular_deps() {
        let response =
            serde_json::to_string(&json!({"merges": [], "extractions": [], "reorders": []}))
                .unwrap();
        let circular = vec![vec!["T1".into(), "T2".into()]];
        let result = parse_reconciliation_response(&response, circular).unwrap();
        assert_eq!(result.circular_dependencies.len(), 1);
        assert_eq!(result.circular_dependencies[0], vec!["T1", "T2"]);
    }

    #[test]
    fn parse_response_rejects_invalid_json() {
        let result = parse_reconciliation_response("not json", vec![]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to parse"));
    }

    // --- reconcile integration tests ---

    #[tokio::test]
    async fn reconcile_detects_issues() {
        let dir = std::env::temp_dir().join("speck_reconcile_test_issues");
        std::fs::create_dir_all(&dir).unwrap();

        let llm_response = serde_json::to_string(&json!({
            "merges": [{
                "task_ids": ["T1", "T2"],
                "reason": "Both modify auth module",
                "merged_title": "Unified auth"
            }],
            "extractions": [{
                "task_ids": ["T1", "T3"],
                "abstraction": "Shared error types",
                "suggested_task_title": "Extract error types"
            }],
            "reorders": [{
                "task_id": "T3",
                "should_precede": "T1",
                "reason": "T1 uses types from T3"
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
                    "text": llm_response,
                    "prompt_tokens": 500,
                    "completion_tokens": 200
                }
            }),
        }];

        let cassette_path = write_cassette(&dir, "reconcile_issues", interactions);
        let ctx = ServiceContext::replaying(&cassette_path).unwrap();

        let specs = vec![
            sample_spec("T1", "Auth module", &["auth"], &[]),
            sample_spec("T2", "Login handler", &["auth"], &[]),
            sample_spec("T3", "Error types", &["errors"], &[]),
        ];

        let result = reconcile(&ctx, &specs).await.unwrap();
        assert_eq!(result.suggested_merges.len(), 1);
        assert_eq!(result.suggested_extractions.len(), 1);
        assert_eq!(result.suggested_reorders.len(), 1);
        assert!(result.circular_dependencies.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn reconcile_no_issues() {
        let dir = std::env::temp_dir().join("speck_reconcile_test_clean");
        std::fs::create_dir_all(&dir).unwrap();

        let llm_response = serde_json::to_string(&json!({
            "merges": [],
            "extractions": [],
            "reorders": []
        }))
        .unwrap();

        let interactions = vec![Interaction {
            seq: 0,
            port: "llm".into(),
            method: "complete".into(),
            input: json!({}),
            output: json!({
                "ok": {
                    "text": llm_response,
                    "prompt_tokens": 300,
                    "completion_tokens": 50
                }
            }),
        }];

        let cassette_path = write_cassette(&dir, "reconcile_clean", interactions);
        let ctx = ServiceContext::replaying(&cassette_path).unwrap();

        let specs = vec![
            sample_spec("T1", "Auth module", &["auth"], &[]),
            sample_spec("T2", "UI components", &["ui"], &["T1"]),
        ];

        let result = reconcile(&ctx, &specs).await.unwrap();
        assert!(result.suggested_merges.is_empty());
        assert!(result.suggested_extractions.is_empty());
        assert!(result.suggested_reorders.is_empty());
        assert!(result.circular_dependencies.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn reconcile_with_circular_deps() {
        let dir = std::env::temp_dir().join("speck_reconcile_test_circular");
        std::fs::create_dir_all(&dir).unwrap();

        let llm_response = serde_json::to_string(&json!({
            "merges": [],
            "extractions": [],
            "reorders": [{
                "task_id": "T2",
                "should_precede": "T1",
                "reason": "Break circular dependency"
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
                    "text": llm_response,
                    "prompt_tokens": 400,
                    "completion_tokens": 100
                }
            }),
        }];

        let cassette_path = write_cassette(&dir, "reconcile_circular", interactions);
        let ctx = ServiceContext::replaying(&cassette_path).unwrap();

        let specs = vec![
            sample_spec("T1", "Module A", &["mod_a"], &["T2"]),
            sample_spec("T2", "Module B", &["mod_b"], &["T1"]),
        ];

        let result = reconcile(&ctx, &specs).await.unwrap();
        assert!(!result.circular_dependencies.is_empty());
        assert_eq!(result.suggested_reorders.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
