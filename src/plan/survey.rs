//! Pass 1 broad codebase survey for the two-pass planning system.
//!
//! Performs a shallow traversal of the codebase to build a routing table
//! mapping modules to purposes, identify cross-cutting concerns, and detect
//! foundational gaps. Reuses a cached codebase map when the commit hash matches.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::context::ServiceContext;
use crate::map::generator;
use crate::map::CodebaseMap;
use crate::ports::llm::{CompletionRequest, CompletionResponse};

/// Path where the cached codebase map is stored relative to project root.
const CACHE_PATH: &str = ".spec-cache/codebase_map.yaml";

/// Result of a Pass 1 broad codebase survey.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurveyResult {
    /// Maps module path to a short purpose description.
    pub routing_table: HashMap<String, String>,
    /// Cross-cutting concerns identified across multiple modules.
    pub cross_cutting_concerns: Vec<String>,
    /// Foundational gaps: capabilities assumed but not yet present.
    pub foundational_gaps: Vec<String>,
    /// Dependency graph: module path -> list of modules it depends on.
    pub dependency_graph: HashMap<String, Vec<String>>,
}

/// Performs a broad codebase survey (Pass 1 of two-pass planning).
///
/// Reads the codebase structure, identifies module boundaries, key interfaces,
/// cross-cutting concerns, and builds a routing table. Uses a cached codebase
/// map from `.spec-cache/` if the commit hash matches, otherwise regenerates.
///
/// # Errors
///
/// Returns an error if codebase traversal, LLM analysis, or map generation fails.
pub async fn broad_survey(
    ctx: &ServiceContext,
    root: &Path,
    requirement: &str,
) -> Result<SurveyResult, String> {
    let map = load_or_generate_map(ctx, root)?;

    let prompt = build_survey_prompt(&map, requirement);
    let request =
        CompletionRequest { model: "claude-sonnet-4-20250514".into(), prompt, max_tokens: 4096 };

    let response: CompletionResponse =
        ctx.llm.complete(&request).await.map_err(|e| format!("LLM survey failed: {e}"))?;

    parse_survey_response(&response.text, &map)
}

/// Loads a cached codebase map if the commit hash matches, otherwise generates a new one.
fn load_or_generate_map(ctx: &ServiceContext, root: &Path) -> Result<CodebaseMap, String> {
    let current_commit =
        ctx.git.current_commit().map_err(|e| format!("failed to get current commit: {e}"))?;

    let cache_path = root.join(CACHE_PATH);
    if ctx.fs.exists(&cache_path) {
        if let Ok(content) = ctx.fs.read_to_string(&cache_path) {
            if let Ok(cached_map) = serde_yaml::from_str::<CodebaseMap>(&content) {
                if cached_map.commit_hash == current_commit {
                    return Ok(cached_map);
                }
            }
        }
    }

    generator::generate(ctx, root)
}

/// Builds the LLM prompt for analyzing the codebase map against a requirement.
fn build_survey_prompt(map: &CodebaseMap, requirement: &str) -> String {
    let mut prompt = String::new();

    prompt.push_str("Analyze this codebase structure and the given requirement.\n\n");
    prompt.push_str("## Codebase Modules\n\n");

    for module in &map.modules {
        let _ = writeln!(prompt, "### {}", module.path);
        let _ = writeln!(prompt, "Public items: {}", module.public_items.join(", "));
        let _ = writeln!(prompt, "Dependencies: {}\n", module.dependencies.join(", "));
    }

    let _ = write!(prompt, "## Requirement\n\n{requirement}\n\n");

    prompt.push_str(
        "## Instructions\n\n\
        Respond in the following JSON format (no markdown fences):\n\
        {\n  \
          \"routing_table\": {\"<module_path>\": \"<purpose>\", ...},\n  \
          \"cross_cutting_concerns\": [\"<concern>\", ...],\n  \
          \"foundational_gaps\": [\"<gap>\", ...]\n\
        }\n\n\
        - routing_table: Map each module to a short description of what kind of work lives there.\n\
        - cross_cutting_concerns: Capabilities needed by multiple modules that may need coordination.\n\
        - foundational_gaps: Infrastructure or capabilities that the requirement assumes but don't exist yet.\n",
    );

    prompt
}

/// Parses the LLM response into a `SurveyResult`, merging with the codebase map's dependency info.
fn parse_survey_response(response_text: &str, map: &CodebaseMap) -> Result<SurveyResult, String> {
    #[derive(Deserialize)]
    struct LlmResponse {
        routing_table: HashMap<String, String>,
        #[serde(default)]
        cross_cutting_concerns: Vec<String>,
        #[serde(default)]
        foundational_gaps: Vec<String>,
    }

    let parsed: LlmResponse = serde_json::from_str(response_text)
        .map_err(|e| format!("failed to parse LLM survey response: {e}"))?;

    // Build dependency graph from the codebase map.
    let mut dependency_graph: HashMap<String, Vec<String>> = HashMap::new();
    for module in &map.modules {
        dependency_graph.insert(module.path.clone(), module.dependencies.clone());
    }

    Ok(SurveyResult {
        routing_table: parsed.routing_table,
        cross_cutting_concerns: parsed.cross_cutting_concerns,
        foundational_gaps: parsed.foundational_gaps,
        dependency_graph,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
    use crate::context::ServiceContext;
    use chrono::Utc;
    use serde_json::json;

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

    fn survey_llm_response() -> serde_json::Value {
        json!({
            "routing_table": {
                "src": "Core application entry point and library root",
                "src/map": "Codebase map generation and structural analysis"
            },
            "cross_cutting_concerns": ["error handling patterns"],
            "foundational_gaps": ["notification system"]
        })
    }

    fn make_survey_cassette_interactions() -> Vec<Interaction> {
        vec![
            // git.current_commit — for cache check
            Interaction {
                seq: 0,
                port: "git".into(),
                method: "current_commit".into(),
                input: json!({}),
                output: json!("abc123def"),
            },
            // fs.exists — cache miss
            Interaction {
                seq: 1,
                port: "fs".into(),
                method: "exists".into(),
                input: json!({"path": "/project/.spec-cache/codebase_map.yaml"}),
                output: json!(false),
            },
            // --- map generation interactions (same as map generator) ---
            // clock.now
            Interaction {
                seq: 2,
                port: "clock".into(),
                method: "now".into(),
                input: json!({}),
                output: json!("2025-06-15T10:00:00Z"),
            },
            // git.current_commit (called again by generator)
            Interaction {
                seq: 3,
                port: "git".into(),
                method: "current_commit".into(),
                input: json!({}),
                output: json!("abc123def"),
            },
            // git.list_files
            Interaction {
                seq: 4,
                port: "git".into(),
                method: "list_files".into(),
                input: json!({"path": "/project"}),
                output: json!(["src/lib.rs", "src/map/mod.rs", "src/map/generator.rs"]),
            },
            // fs.read_to_string — src/lib.rs
            Interaction {
                seq: 5,
                port: "fs".into(),
                method: "read_to_string".into(),
                input: json!({"path": "/project/src/lib.rs"}),
                output: json!("pub struct App {\n    pub name: String,\n}\n\npub fn run() {}\n"),
            },
            // fs.read_to_string — src/map/mod.rs
            Interaction {
                seq: 6,
                port: "fs".into(),
                method: "read_to_string".into(),
                input: json!({"path": "/project/src/map/mod.rs"}),
                output: json!(
                    "use crate::context;\n\npub fn generate() {}\npub trait Generator {}\n"
                ),
            },
            // fs.read_to_string — src/map/generator.rs
            Interaction {
                seq: 7,
                port: "fs".into(),
                method: "read_to_string".into(),
                input: json!({"path": "/project/src/map/generator.rs"}),
                output: json!("use crate::map;\n\nfn helper() {}\n"),
            },
            // fs.write — map output
            Interaction {
                seq: 8,
                port: "fs".into(),
                method: "write".into(),
                input: json!({"path": "/project/.spec-cache/codebase_map.yaml"}),
                output: json!(null),
            },
            // --- LLM call for survey analysis ---
            Interaction {
                seq: 9,
                port: "llm".into(),
                method: "complete".into(),
                input: json!({}),
                output: json!({
                    "ok": {
                        "text": serde_json::to_string(&survey_llm_response()).unwrap(),
                        "prompt_tokens": 500,
                        "completion_tokens": 100
                    }
                }),
            },
        ]
    }

    #[tokio::test]
    async fn broad_survey_generates_map_and_analyzes() {
        let dir = std::env::temp_dir().join("speck_survey_test_broad");
        std::fs::create_dir_all(&dir).unwrap();

        let cassette_path =
            write_cassette(&dir, "survey_broad", make_survey_cassette_interactions());
        let ctx = ServiceContext::replaying(&cassette_path).unwrap();

        let result = broad_survey(&ctx, Path::new("/project"), "Add authentication").await.unwrap();

        assert_eq!(result.routing_table.len(), 2);
        assert!(result.routing_table.contains_key("src"));
        assert!(result.routing_table.contains_key("src/map"));
        assert_eq!(result.cross_cutting_concerns, vec!["error handling patterns"]);
        assert_eq!(result.foundational_gaps, vec!["notification system"]);

        // Dependency graph built from codebase map
        assert!(result.dependency_graph.contains_key("src/map"));
        assert!(result.dependency_graph["src/map"].contains(&"context".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn broad_survey_uses_cached_map() {
        let dir = std::env::temp_dir().join("speck_survey_test_cached");
        std::fs::create_dir_all(&dir).unwrap();

        // Build a cached map that matches the commit hash
        let cached_map = CodebaseMap {
            commit_hash: "cached-commit".into(),
            generated_at: Utc::now(),
            modules: vec![crate::map::ModuleSummary {
                path: "src/cached".into(),
                public_items: vec!["fn cached_fn".into()],
                dependencies: vec!["dep_a".into()],
            }],
            directory_tree: vec!["src/cached/mod.rs".into()],
            test_infrastructure: vec![],
        };
        let cached_yaml = serde_yaml::to_string(&cached_map).unwrap();

        let interactions = vec![
            // git.current_commit — matches cached map
            Interaction {
                seq: 0,
                port: "git".into(),
                method: "current_commit".into(),
                input: json!({}),
                output: json!("cached-commit"),
            },
            // fs.exists — cache hit
            Interaction {
                seq: 1,
                port: "fs".into(),
                method: "exists".into(),
                input: json!({"path": "/project/.spec-cache/codebase_map.yaml"}),
                output: json!(true),
            },
            // fs.read_to_string — read cached map
            Interaction {
                seq: 2,
                port: "fs".into(),
                method: "read_to_string".into(),
                input: json!({"path": "/project/.spec-cache/codebase_map.yaml"}),
                output: json!(cached_yaml),
            },
            // LLM call
            Interaction {
                seq: 3,
                port: "llm".into(),
                method: "complete".into(),
                input: json!({}),
                output: json!({
                    "ok": {
                        "text": serde_json::to_string(&json!({
                            "routing_table": {"src/cached": "Cached module purpose"},
                            "cross_cutting_concerns": [],
                            "foundational_gaps": []
                        })).unwrap(),
                        "prompt_tokens": 200,
                        "completion_tokens": 50
                    }
                }),
            },
        ];

        let cassette_path = write_cassette(&dir, "survey_cached", interactions);
        let ctx = ServiceContext::replaying(&cassette_path).unwrap();

        let result = broad_survey(&ctx, Path::new("/project"), "Some requirement").await.unwrap();

        // Should use the cached map's module structure
        assert!(result.routing_table.contains_key("src/cached"));
        assert!(result.dependency_graph.contains_key("src/cached"));
        assert_eq!(result.dependency_graph["src/cached"], vec!["dep_a"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_survey_response_parses_valid_json() {
        let map = CodebaseMap {
            commit_hash: "abc".into(),
            generated_at: Utc::now(),
            modules: vec![crate::map::ModuleSummary {
                path: "src".into(),
                public_items: vec![],
                dependencies: vec!["ports".into()],
            }],
            directory_tree: vec![],
            test_infrastructure: vec![],
        };

        let response = serde_json::to_string(&json!({
            "routing_table": {"src": "Main entry point"},
            "cross_cutting_concerns": ["logging"],
            "foundational_gaps": ["monitoring"]
        }))
        .unwrap();

        let result = parse_survey_response(&response, &map).unwrap();
        assert_eq!(result.routing_table["src"], "Main entry point");
        assert_eq!(result.cross_cutting_concerns, vec!["logging"]);
        assert_eq!(result.foundational_gaps, vec!["monitoring"]);
        assert_eq!(result.dependency_graph["src"], vec!["ports"]);
    }

    #[test]
    fn parse_survey_response_rejects_invalid_json() {
        let map = CodebaseMap {
            commit_hash: "abc".into(),
            generated_at: Utc::now(),
            modules: vec![],
            directory_tree: vec![],
            test_infrastructure: vec![],
        };
        let result = parse_survey_response("not json", &map);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to parse"));
    }

    #[test]
    fn build_survey_prompt_includes_modules_and_requirement() {
        let map = CodebaseMap {
            commit_hash: "abc".into(),
            generated_at: Utc::now(),
            modules: vec![crate::map::ModuleSummary {
                path: "src/auth".into(),
                public_items: vec!["fn login".into()],
                dependencies: vec!["db".into()],
            }],
            directory_tree: vec![],
            test_infrastructure: vec![],
        };

        let prompt = build_survey_prompt(&map, "Add OAuth support");
        assert!(prompt.contains("src/auth"));
        assert!(prompt.contains("fn login"));
        assert!(prompt.contains("db"));
        assert!(prompt.contains("Add OAuth support"));
        assert!(prompt.contains("routing_table"));
    }
}
