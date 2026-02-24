//! Linkage resolution: maps abstract module references to concrete file paths.

use crate::map::{CodebaseMap, ModuleSummary};
use crate::spec::TaskSpec;

/// A single resolved link from an abstract module reference to a concrete path.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLink {
    /// The abstract module name from the spec (e.g., "`MetricsService`").
    pub module_ref: String,
    /// The concrete file path in the codebase map, if found.
    pub resolved_path: Option<String>,
}

/// Result of resolving all module references in a spec.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkageResult {
    /// The spec ID whose references were resolved.
    pub spec_id: String,
    /// Per-reference resolution results.
    pub links: Vec<ResolvedLink>,
}

impl LinkageResult {
    /// Returns `true` if every module reference was resolved.
    #[must_use]
    pub fn fully_resolved(&self) -> bool {
        self.links.iter().all(|l| l.resolved_path.is_some())
    }

    /// Returns module references that could not be resolved.
    #[must_use]
    pub fn unresolved(&self) -> Vec<&str> {
        self.links
            .iter()
            .filter(|l| l.resolved_path.is_none())
            .map(|l| l.module_ref.as_str())
            .collect()
    }
}

/// Derives directory-level glob patterns from a linkage result.
///
/// For resolved paths, takes the parent directory and appends `/**`.
/// For unresolved module references, generates a best-effort glob from the name.
///
/// Returns `(globs, unresolved_refs)` where `unresolved_refs` lists module names
/// that could not be matched in the codebase map.
#[must_use]
pub fn derive_globs(linkage: &LinkageResult) -> (Vec<String>, Vec<String>) {
    let mut globs: Vec<String> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();

    for link in &linkage.links {
        let glob = if let Some(path) = &link.resolved_path {
            path_to_dir_glob(path)
        } else {
            unresolved.push(link.module_ref.clone());
            module_ref_to_best_effort_glob(&link.module_ref)
        };
        if !globs.contains(&glob) {
            globs.push(glob);
        }
    }

    (globs, unresolved)
}

/// Converts a resolved file path to a directory-level glob.
///
/// For example, `"src/services/metrics.rs"` → `"src/services/**"`.
/// If the path has no parent directory, returns `"**"`.
fn path_to_dir_glob(path: &str) -> String {
    let p = std::path::Path::new(path);
    match p.parent() {
        Some(parent) if parent != std::path::Path::new("") => {
            format!("{}/**", parent.display())
        }
        _ => "**".to_string(),
    }
}

/// Generates a best-effort glob from an unresolvable module reference name.
///
/// Converts `CamelCase` to `snake_case` and wraps in a glob pattern.
/// For example, `"MetricsService"` → `"**/metrics_service/**"`.
fn module_ref_to_best_effort_glob(module_ref: &str) -> String {
    let snake = camel_to_snake(module_ref);
    format!("**/{snake}/**")
}

/// Converts a `CamelCase` or `PascalCase` identifier to `snake_case`.
fn camel_to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        for lc in ch.to_lowercase() {
            result.push(lc);
        }
    }
    result
}

/// Resolves abstract module references in a spec against a codebase map.
///
/// For each module name in `spec.context.modules`, searches the codebase map
/// for a matching module by checking path components and public items.
/// A match is found when the module reference appears as a substring (case-insensitive)
/// in the module path or among its public items.
#[must_use]
pub fn resolve(spec: &TaskSpec, codebase_map: &CodebaseMap) -> LinkageResult {
    let modules = spec.context.as_ref().map(|ctx| ctx.modules.as_slice()).unwrap_or_default();

    let links = modules
        .iter()
        .map(|module_ref| {
            let resolved_path = find_matching_module(module_ref, &codebase_map.modules);
            ResolvedLink { module_ref: module_ref.clone(), resolved_path }
        })
        .collect();

    LinkageResult { spec_id: spec.id.clone(), links }
}

/// Finds the best matching module for an abstract reference.
///
/// Matching strategy (in priority order):
/// 1. Exact match in public items (case-insensitive)
/// 2. Substring match in module path (case-insensitive)
/// 3. Substring match in public items (case-insensitive)
fn find_matching_module(module_ref: &str, modules: &[ModuleSummary]) -> Option<String> {
    let needle = module_ref.to_lowercase();

    // Priority 1: exact match in public items
    for module in modules {
        if module.public_items.iter().any(|item| item.to_lowercase() == needle) {
            return Some(module.path.clone());
        }
    }

    // Priority 2: substring match in module path
    for module in modules {
        if module.path.to_lowercase().contains(&needle) {
            return Some(module.path.clone());
        }
    }

    // Priority 3: substring match in public items
    for module in modules {
        if module.public_items.iter().any(|item| item.to_lowercase().contains(&needle)) {
            return Some(module.path.clone());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::CodebaseMap;
    use crate::spec::{SignalType, TaskContext, VerificationCheck, VerificationStrategy};
    use chrono::Utc;

    fn sample_map() -> CodebaseMap {
        CodebaseMap {
            commit_hash: "abc123".to_string(),
            generated_at: Utc::now(),
            modules: vec![
                ModuleSummary {
                    path: "src/services/metrics.rs".to_string(),
                    public_items: vec!["MetricsService".to_string(), "Counter".to_string()],
                    dependencies: vec![],
                },
                ModuleSummary {
                    path: "src/handlers/api.rs".to_string(),
                    public_items: vec!["ApiHandler".to_string(), "Router".to_string()],
                    dependencies: vec!["metrics".to_string()],
                },
                ModuleSummary {
                    path: "src/db/connection.rs".to_string(),
                    public_items: vec!["ConnectionPool".to_string()],
                    dependencies: vec![],
                },
            ],
            directory_tree: vec![
                "src/services/metrics.rs".to_string(),
                "src/handlers/api.rs".to_string(),
                "src/db/connection.rs".to_string(),
            ],
            test_infrastructure: vec![],
        }
    }

    fn sample_spec_with_modules(id: &str, modules: Vec<String>) -> TaskSpec {
        TaskSpec {
            id: id.to_string(),
            title: format!("Task {id}"),
            requirement: None,
            context: Some(TaskContext { modules, patterns: None, dependencies: vec![] }),
            acceptance_criteria: vec!["done".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::Custom { description: "manual check".to_string() }],
            },
            affected_globs: None,
        }
    }

    #[test]
    fn resolves_by_public_item_exact_match() {
        let map = sample_map();
        let spec = sample_spec_with_modules("T-1", vec!["MetricsService".to_string()]);
        let result = resolve(&spec, &map);

        assert!(result.fully_resolved());
        assert_eq!(result.links.len(), 1);
        assert_eq!(result.links[0].resolved_path.as_deref(), Some("src/services/metrics.rs"));
    }

    #[test]
    fn resolves_by_path_substring() {
        let map = sample_map();
        let spec = sample_spec_with_modules("T-2", vec!["connection".to_string()]);
        let result = resolve(&spec, &map);

        assert!(result.fully_resolved());
        assert_eq!(result.links[0].resolved_path.as_deref(), Some("src/db/connection.rs"));
    }

    #[test]
    fn resolves_case_insensitive() {
        let map = sample_map();
        let spec = sample_spec_with_modules("T-3", vec!["metricsservice".to_string()]);
        let result = resolve(&spec, &map);

        assert!(result.fully_resolved());
        assert_eq!(result.links[0].resolved_path.as_deref(), Some("src/services/metrics.rs"));
    }

    #[test]
    fn unresolved_module_returns_none() {
        let map = sample_map();
        let spec = sample_spec_with_modules("T-4", vec!["NonExistentService".to_string()]);
        let result = resolve(&spec, &map);

        assert!(!result.fully_resolved());
        assert_eq!(result.unresolved(), vec!["NonExistentService"]);
    }

    #[test]
    fn derive_globs_from_resolved_paths() {
        let map = sample_map();
        let spec = sample_spec_with_modules(
            "T-G1",
            vec!["MetricsService".to_string(), "ApiHandler".to_string()],
        );
        let linkage = resolve(&spec, &map);
        let (globs, unresolved) = derive_globs(&linkage);

        assert!(unresolved.is_empty());
        assert_eq!(globs.len(), 2);
        assert!(globs.contains(&"src/services/**".to_string()));
        assert!(globs.contains(&"src/handlers/**".to_string()));
    }

    #[test]
    fn derive_globs_deduplicates() {
        // Both "MetricsService" and "Counter" resolve to src/services/metrics.rs → same dir glob
        let map = sample_map();
        let spec = sample_spec_with_modules(
            "T-G2",
            vec!["MetricsService".to_string(), "Counter".to_string()],
        );
        let linkage = resolve(&spec, &map);
        let (globs, unresolved) = derive_globs(&linkage);

        assert!(unresolved.is_empty());
        assert_eq!(globs.len(), 1, "both resolve to same directory, should deduplicate");
        assert_eq!(globs[0], "src/services/**");
    }

    #[test]
    fn derive_globs_best_effort_for_unresolved() {
        let map = sample_map();
        let spec = sample_spec_with_modules("T-G3", vec!["NonExistentService".to_string()]);
        let linkage = resolve(&spec, &map);
        let (globs, unresolved) = derive_globs(&linkage);

        assert_eq!(unresolved, vec!["NonExistentService"]);
        assert_eq!(globs.len(), 1);
        assert_eq!(globs[0], "**/non_existent_service/**");
    }

    #[test]
    fn derive_globs_empty_links() {
        let map = sample_map();
        let spec = TaskSpec {
            id: "T-G4".to_string(),
            title: "No context".to_string(),
            requirement: None,
            context: None,
            acceptance_criteria: vec![],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion { checks: vec![] },
            affected_globs: None,
        };
        let linkage = resolve(&spec, &map);
        let (globs, unresolved) = derive_globs(&linkage);

        assert!(globs.is_empty());
        assert!(unresolved.is_empty());
    }

    #[test]
    fn camel_to_snake_converts_pascal_case() {
        assert_eq!(camel_to_snake("MetricsService"), "metrics_service");
        assert_eq!(camel_to_snake("ApiHandler"), "api_handler");
        assert_eq!(camel_to_snake("lowercase"), "lowercase");
        assert_eq!(camel_to_snake("A"), "a");
    }

    #[test]
    fn path_to_dir_glob_extracts_parent() {
        assert_eq!(path_to_dir_glob("src/services/metrics.rs"), "src/services/**");
        assert_eq!(path_to_dir_glob("src/handlers/api.rs"), "src/handlers/**");
        assert_eq!(path_to_dir_glob("lib.rs"), "**");
    }

    #[test]
    fn multiple_modules_mixed_resolution() {
        let map = sample_map();
        let spec = sample_spec_with_modules(
            "T-5",
            vec!["MetricsService".to_string(), "Unknown".to_string(), "ApiHandler".to_string()],
        );
        let result = resolve(&spec, &map);

        assert!(!result.fully_resolved());
        assert_eq!(result.links.len(), 3);
        assert!(result.links[0].resolved_path.is_some());
        assert!(result.links[1].resolved_path.is_none());
        assert!(result.links[2].resolved_path.is_some());
    }

    #[test]
    fn spec_without_context_returns_empty_links() {
        let map = sample_map();
        let spec = TaskSpec {
            id: "T-6".to_string(),
            title: "No context".to_string(),
            requirement: None,
            context: None,
            acceptance_criteria: vec!["done".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion { checks: vec![] },
            affected_globs: None,
        };
        let result = resolve(&spec, &map);

        assert!(result.fully_resolved());
        assert!(result.links.is_empty());
    }
}
