//! `speck plan` command.

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::context::ServiceContext;
use crate::plan::signal::{
    self, ClassificationResult, SignalType as PlanSignalType,
    VerificationStrategy as PlanVerificationStrategy,
};
use crate::plan::survey::{broad_survey, SurveyResult};
use crate::spec::{SignalType, TaskSpec, VerificationCheck, VerificationStrategy};

/// Execute the `plan` command.
///
/// Runs the Pass 1 broad codebase survey, then Pass 2 signal classification.
/// The requirement can come from a positional argument or from a file via `--from`.
///
/// # Errors
///
/// Returns an error string if the requirement is missing, the survey fails,
/// or signal classification fails.
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

    let survey = rt.block_on(broad_survey(ctx, &root, &requirement_text))?;

    print_survey_result(&survey);

    // Pass 2: Signal classification
    let codebase_context = build_codebase_context(&survey);
    let classification = rt
        .block_on(signal::classify(ctx.llm.as_ref(), &requirement_text, &codebase_context))
        .map_err(|e| format!("signal classification failed: {e}"))?;

    match classification {
        ClassificationResult::Classified { signal_type, strategy } => {
            let task_spec = build_task_spec(&requirement_text, &signal_type, strategy);
            print_classification(&task_spec);
        }
        ClassificationResult::PushbackRequired { reason } => {
            eprintln!("Warning: pushback required — {reason}");
        }
    }

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

/// Build a codebase context string from the survey result for signal classification.
fn build_codebase_context(survey: &SurveyResult) -> String {
    let mut ctx = String::new();
    let mut paths: Vec<_> = survey.routing_table.keys().collect();
    paths.sort();
    for path in paths {
        let _ = writeln!(ctx, "{path}: {}", survey.routing_table[path]);
    }
    if !survey.cross_cutting_concerns.is_empty() {
        let _ = writeln!(ctx, "\nCross-cutting: {}", survey.cross_cutting_concerns.join(", "));
    }
    if !survey.foundational_gaps.is_empty() {
        let _ = writeln!(ctx, "Gaps: {}", survey.foundational_gaps.join(", "));
    }
    ctx
}

/// Map a plan signal type to a spec signal type.
fn map_signal_type(plan_signal: &PlanSignalType) -> SignalType {
    match plan_signal {
        PlanSignalType::Clear => SignalType::Clear,
        PlanSignalType::FuzzyButConstrainable => SignalType::Fuzzy,
        PlanSignalType::InternalLogic => SignalType::InternalLogic,
    }
}

/// Map a plan verification strategy to a spec verification strategy.
fn map_verification_strategy(plan_strategy: PlanVerificationStrategy) -> VerificationStrategy {
    match plan_strategy {
        PlanVerificationStrategy::DirectAssertion { checks } => {
            VerificationStrategy::DirectAssertion {
                checks: checks
                    .into_iter()
                    .map(|description| VerificationCheck::Custom { description })
                    .collect(),
            }
        }
        PlanVerificationStrategy::StructuralDecomposition { sub_assertions } => {
            VerificationStrategy::DirectAssertion {
                checks: sub_assertions
                    .into_iter()
                    .map(|sa| VerificationCheck::Custom {
                        description: format!("{}: {}", sa.description, sa.check),
                    })
                    .collect(),
            }
        }
        PlanVerificationStrategy::RefactorToExpose { description } => {
            VerificationStrategy::RefactorToExpose {
                decision_point: description,
                required_structure: String::new(),
                cases: vec![],
            }
        }
        PlanVerificationStrategy::TraceAssertion { description } => {
            VerificationStrategy::TraceAssertion {
                trace_point: description,
                test_input: String::new(),
                expected_trace: vec![],
            }
        }
    }
}

/// Build an initial `TaskSpec` skeleton from classification results.
fn build_task_spec(
    requirement: &str,
    plan_signal: &PlanSignalType,
    plan_strategy: PlanVerificationStrategy,
) -> TaskSpec {
    let signal_type = map_signal_type(plan_signal);
    let verification = map_verification_strategy(plan_strategy);

    TaskSpec {
        id: String::new(),
        title: requirement.to_string(),
        requirement: Some(requirement.to_string()),
        context: None,
        acceptance_criteria: vec![],
        signal_type,
        verification,
    }
}

/// Print the signal classification and verification strategy.
fn print_classification(task_spec: &TaskSpec) {
    println!("\n=== Signal Classification ===");
    println!("  Type: {:?}", task_spec.signal_type);
    println!("  Verification: {:?}", task_spec.verification);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::signal::{SubAssertion, VerificationStrategy as PlanVS};
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

    #[test]
    fn build_codebase_context_includes_routing_and_concerns() {
        let mut routing_table = HashMap::new();
        routing_table.insert("src/auth".into(), "Authentication module".into());

        let survey = SurveyResult {
            routing_table,
            cross_cutting_concerns: vec!["logging".into()],
            foundational_gaps: vec!["caching".into()],
            dependency_graph: HashMap::new(),
        };

        let ctx = build_codebase_context(&survey);
        assert!(ctx.contains("src/auth: Authentication module"));
        assert!(ctx.contains("Cross-cutting: logging"));
        assert!(ctx.contains("Gaps: caching"));
    }

    #[test]
    fn map_signal_type_clear() {
        assert_eq!(map_signal_type(&PlanSignalType::Clear), SignalType::Clear);
    }

    #[test]
    fn map_signal_type_fuzzy() {
        assert_eq!(map_signal_type(&PlanSignalType::FuzzyButConstrainable), SignalType::Fuzzy);
    }

    #[test]
    fn map_signal_type_internal() {
        assert_eq!(map_signal_type(&PlanSignalType::InternalLogic), SignalType::InternalLogic);
    }

    #[test]
    fn map_strategy_direct_assertion() {
        let plan_strategy =
            PlanVS::DirectAssertion { checks: vec!["check1".into(), "check2".into()] };
        let spec_strategy = map_verification_strategy(plan_strategy);
        match spec_strategy {
            VerificationStrategy::DirectAssertion { checks } => {
                assert_eq!(checks.len(), 2);
                assert_eq!(checks[0], VerificationCheck::Custom { description: "check1".into() });
            }
            other => panic!("expected DirectAssertion, got {other:?}"),
        }
    }

    #[test]
    fn map_strategy_structural_decomposition() {
        let plan_strategy = PlanVS::StructuralDecomposition {
            sub_assertions: vec![SubAssertion {
                description: "ordered".into(),
                check: "assert sorted".into(),
            }],
        };
        let spec_strategy = map_verification_strategy(plan_strategy);
        match spec_strategy {
            VerificationStrategy::DirectAssertion { checks } => {
                assert_eq!(checks.len(), 1);
                assert_eq!(
                    checks[0],
                    VerificationCheck::Custom { description: "ordered: assert sorted".into() }
                );
            }
            other => panic!("expected DirectAssertion, got {other:?}"),
        }
    }

    #[test]
    fn map_strategy_refactor_to_expose() {
        let plan_strategy = PlanVS::RefactorToExpose { description: "extract branching".into() };
        let spec_strategy = map_verification_strategy(plan_strategy);
        match spec_strategy {
            VerificationStrategy::RefactorToExpose { decision_point, .. } => {
                assert_eq!(decision_point, "extract branching");
            }
            other => panic!("expected RefactorToExpose, got {other:?}"),
        }
    }

    #[test]
    fn map_strategy_trace_assertion() {
        let plan_strategy = PlanVS::TraceAssertion { description: "trace eviction".into() };
        let spec_strategy = map_verification_strategy(plan_strategy);
        match spec_strategy {
            VerificationStrategy::TraceAssertion { trace_point, .. } => {
                assert_eq!(trace_point, "trace eviction");
            }
            other => panic!("expected TraceAssertion, got {other:?}"),
        }
    }

    #[test]
    fn build_task_spec_creates_skeleton() {
        let spec = build_task_spec(
            "add CSV export",
            &PlanSignalType::Clear,
            PlanVS::DirectAssertion { checks: vec!["CLI exports CSV".into()] },
        );
        assert_eq!(spec.title, "add CSV export");
        assert_eq!(spec.requirement, Some("add CSV export".into()));
        assert_eq!(spec.signal_type, SignalType::Clear);
        assert!(spec.id.is_empty());
        assert!(spec.acceptance_criteria.is_empty());
    }

    #[test]
    fn print_classification_does_not_panic() {
        let spec = build_task_spec(
            "test req",
            &PlanSignalType::FuzzyButConstrainable,
            PlanVS::StructuralDecomposition {
                sub_assertions: vec![SubAssertion { description: "d".into(), check: "c".into() }],
            },
        );
        print_classification(&spec);
    }
}
