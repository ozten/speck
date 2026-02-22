//! `speck plan` command.

use std::path::PathBuf;

use crate::context::ServiceContext;
use crate::plan::survey::{broad_survey, SurveyResult};

/// Execute the `plan` command.
///
/// Runs the Pass 1 broad codebase survey using the given requirement text.
/// The requirement can come from a positional argument or from a file via `--from`.
///
/// # Errors
///
/// Returns an error string if the requirement is missing or the survey fails.
pub fn run(
    ctx: &ServiceContext,
    requirement: Option<&str>,
    from: Option<&PathBuf>,
) -> Result<(), String> {
    let requirement_text = resolve_requirement(requirement, from)?;

    let root =
        std::env::current_dir().map_err(|e| format!("failed to get current directory: {e}"))?;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("failed to create async runtime: {e}"))?;

    let result = rt.block_on(broad_survey(ctx, &root, &requirement_text))?;

    print_survey_result(&result);

    Ok(())
}

/// Resolve the requirement text from either a positional argument or a file.
fn resolve_requirement(
    requirement: Option<&str>,
    from: Option<&PathBuf>,
) -> Result<String, String> {
    match (requirement, from) {
        (Some(text), _) => Ok(text.to_string()),
        (None, Some(path)) => std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read requirement file '{}': {e}", path.display())),
        (None, None) => {
            Err("requirement text is required: provide it as an argument or use --from <file>"
                .into())
        }
    }
}

/// Print a `SurveyResult` to stdout in a human-readable format.
fn print_survey_result(result: &SurveyResult) {
    println!("=== Routing Table ===");
    let mut paths: Vec<_> = result.routing_table.keys().collect();
    paths.sort();
    for path in paths {
        println!("  {path}: {}", result.routing_table[path]);
    }

    println!("\n=== Cross-Cutting Concerns ===");
    if result.cross_cutting_concerns.is_empty() {
        println!("  (none identified)");
    } else {
        for concern in &result.cross_cutting_concerns {
            println!("  - {concern}");
        }
    }

    println!("\n=== Foundational Gaps ===");
    if result.foundational_gaps.is_empty() {
        println!("  (none identified)");
    } else {
        for gap in &result.foundational_gaps {
            println!("  - {gap}");
        }
    }

    if !result.dependency_graph.is_empty() {
        println!("\n=== Dependency Graph ===");
        let mut modules: Vec<_> = result.dependency_graph.keys().collect();
        modules.sort();
        for module in modules {
            let deps = &result.dependency_graph[module];
            println!("  {module} -> {}", deps.join(", "));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn resolve_requirement_from_arg() {
        let text = resolve_requirement(Some("add auth"), None).unwrap();
        assert_eq!(text, "add auth");
    }

    #[test]
    fn resolve_requirement_from_file() {
        let dir = std::env::temp_dir().join("speck_plan_test_req");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("req.txt");
        std::fs::write(&file, "requirement from file").unwrap();
        let text = resolve_requirement(None, Some(&file)).unwrap();
        assert_eq!(text, "requirement from file");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolve_requirement_missing() {
        let err = resolve_requirement(None, None).unwrap_err();
        assert!(err.contains("requirement text is required"));
    }

    #[test]
    fn resolve_requirement_arg_takes_precedence() {
        let dir = std::env::temp_dir().join("speck_plan_test_prec");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("req.txt");
        std::fs::write(&file, "from file").unwrap();
        let text = resolve_requirement(Some("from arg"), Some(&file)).unwrap();
        assert_eq!(text, "from arg");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn print_survey_result_formats_output() {
        let mut routing_table = HashMap::new();
        routing_table.insert("src/auth".into(), "Authentication module".into());
        routing_table.insert("src/db".into(), "Database layer".into());

        let mut dependency_graph = HashMap::new();
        dependency_graph.insert("src/auth".into(), vec!["src/db".into()]);

        let result = SurveyResult {
            routing_table,
            cross_cutting_concerns: vec!["error handling".into()],
            foundational_gaps: vec!["notification system".into()],
            dependency_graph,
        };

        // Just verify it doesn't panic
        print_survey_result(&result);
    }
}
