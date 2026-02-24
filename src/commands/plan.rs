//! `speck plan` command.

use std::fmt::Write as _;
use std::path::Path;

use crate::context::ServiceContext;
use crate::linkage;
use crate::plan::conversation::{self, AnalysisResult};
use crate::plan::reconcile::{self, PlanDiff, ReconciliationResult, SpecMatchAction};
use crate::plan::score::{self, ScoreResult};
use crate::plan::signal::{
    self, ClassificationResult, PlanCheck, SignalType as PlanSignalType,
    VerificationStrategy as PlanVerificationStrategy,
};
use crate::plan::survey::{broad_survey, SurveyResult};
use crate::spec::{SignalType, TaskSpec, VerificationCheck, VerificationStrategy};
use crate::store::SpecStore;

/// Execute the `plan` command.
///
/// Reads a spec document from `doc_path`, then runs all analysis passes
/// non-interactively: survey, signal classification, spec analysis, and
/// reconciliation. Writes derived `TaskSpec`s to `.speck/tasks/` and prints
/// structured feedback to stdout.
///
/// # Errors
///
/// Returns an error string if reading the doc fails, any analysis pass fails,
/// or spec persistence fails.
pub fn run(ctx: &ServiceContext, doc_path: &Path) -> Result<(), String> {
    let requirement_text = std::fs::read_to_string(doc_path)
        .map_err(|e| format!("failed to read spec document '{}': {e}", doc_path.display()))?;

    let root =
        std::env::current_dir().map_err(|e| format!("failed to get current directory: {e}"))?;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("failed to create async runtime: {e}"))?;

    // Pass 0: Score the document for specificity and verifiability
    let score_result = rt
        .block_on(score::score_document(ctx.llm.as_ref(), &requirement_text))
        .map_err(|e| format!("document scoring failed: {e}"))?;

    // Pass 1: Broad codebase survey (also returns the codebase map for reuse)
    let (survey, codebase_map) = rt.block_on(broad_survey(ctx, &root, &requirement_text))?;
    print_survey_result(&survey);

    // Pass 2: Signal classification
    let codebase_context = build_codebase_context(&survey);
    let classification = rt
        .block_on(signal::classify(ctx.llm.as_ref(), &requirement_text, &codebase_context))
        .map_err(|e| format!("signal classification failed: {e}"))?;

    let mut specs = match classification {
        ClassificationResult::Classified { signal_type, strategy } => {
            let task_spec = build_task_spec(&requirement_text, &signal_type, strategy);
            print_classification(&task_spec);
            vec![task_spec]
        }
        ClassificationResult::PushbackRequired { reason } => {
            eprintln!("Note: pushback required — {reason}");
            let task_spec = TaskSpec {
                id: String::new(),
                title: requirement_text.clone(),
                requirement: Some(requirement_text),
                context: None,
                acceptance_criteria: vec![],
                signal_type: SignalType::Fuzzy,
                verification: VerificationStrategy::DirectAssertion { checks: vec![] },
                affected_globs: None,
            };
            vec![task_spec]
        }
    };

    // Pass 2.5: Glob derivation via linkage resolution (reuses map from Pass 1)
    let mut glob_warnings: Vec<String> = Vec::new();
    for spec in &mut specs {
        let linkage_result = linkage::resolve(spec, &codebase_map);
        let (globs, unresolved) = linkage::derive_globs(&linkage_result);
        for module_ref in &unresolved {
            glob_warnings.push(format!(
                "  [spec {}] unresolved module ref '{}': using best-effort glob",
                spec.id, module_ref
            ));
        }
        spec.affected_globs = Some(globs);
    }

    // Pass 2.5a: Single-pass spec analysis (non-interactive feedback)
    let analysis = rt
        .block_on(conversation::analyze_specs(ctx, &specs))
        .map_err(|e| format!("spec analysis failed: {e}"))?;

    // Pass 2.5b: Reconciliation
    let reconciliation = rt
        .block_on(reconcile::reconcile(ctx, &specs))
        .map_err(|e| format!("reconciliation failed: {e}"))?;

    // Load existing specs for idempotent re-plan matching.
    let store_root = store_root()?;
    let store = SpecStore::new(ctx, &store_root);
    let existing_ids = store.list_task_specs().unwrap_or_default();
    let existing_specs: Vec<_> =
        existing_ids.iter().filter_map(|id| store.load_task_spec(id).ok()).collect();

    // Match new specs to existing ones (assigns IDs in-place).
    let diff = reconcile::match_to_existing(&mut specs, &existing_specs, ctx.id_gen.as_ref());

    // Persist final specs to the store.
    for spec in &specs {
        store.save_task_spec(spec)?;
    }

    // Print structured output
    print_structured_output(
        &specs,
        &diff,
        &analysis,
        &reconciliation,
        &score_result,
        &store_root,
        &glob_warnings,
    );

    Ok(())
}

/// Print the full structured output suitable for LLM consumption.
fn print_structured_output(
    specs: &[TaskSpec],
    diff: &PlanDiff,
    analysis: &AnalysisResult,
    reconciliation: &ReconciliationResult,
    score_result: &ScoreResult,
    store_root: &Path,
    glob_warnings: &[String],
) {
    print_score(score_result);

    // --- Derived Tasks ---
    println!("\n=== Derived Tasks ({}) ===", specs.len());
    for (i, spec) in specs.iter().enumerate() {
        println!("{}. {} — {}", i + 1, spec.id, spec.title);
        println!("   Signal: {:?}", spec.signal_type);
        println!("   Verification: {:?}", spec.verification);
        if let Some(globs) = &spec.affected_globs {
            if globs.is_empty() {
                println!("   Affected globs: (none)");
            } else {
                println!("   Affected globs: {}", globs.join(", "));
            }
        }
    }

    // --- Glob Warnings ---
    if !glob_warnings.is_empty() {
        println!("\n=== Glob Resolution Warnings ===");
        for warning in glob_warnings {
            println!("{warning}");
        }
    }

    // --- Feedback ---
    println!("\n=== Feedback ===");
    println!("{}", analysis.summary);

    if analysis.questions.is_empty() {
        println!("\nAll specs have clear verification strategies.");
    } else {
        for (i, q) in analysis.questions.iter().enumerate() {
            println!("\nQ{} [task {}]: {}", i + 1, q.task_id, q.description);
            for (j, opt) in q.options.iter().enumerate() {
                #[allow(clippy::cast_possible_truncation)]
                let label = char::from(b'a' + j as u8);
                let rec = q.recommended.map_or(String::new(), |r| {
                    if r == j {
                        " (recommended)".into()
                    } else {
                        String::new()
                    }
                });
                println!("  {label}) {opt}{rec}");
            }
        }
    }

    // --- Reconciliation ---
    print_reconciliation(reconciliation);

    // --- Plan Diff ---
    print_plan_diff(diff);

    // --- Summary ---
    println!("\n=== Summary ===");
    let new_count =
        diff.actions.iter().filter(|a| matches!(a, SpecMatchAction::New { .. })).count();
    let updated_count =
        diff.actions.iter().filter(|a| matches!(a, SpecMatchAction::Updated { .. })).count();
    println!(
        "{} spec(s) saved to {} ({} new, {} updated, {} orphaned)",
        specs.len(),
        store_root.display(),
        new_count,
        updated_count,
        diff.orphaned.len()
    );
}

/// Print the plan diff (new / updated / orphaned) to stdout.
fn print_plan_diff(diff: &PlanDiff) {
    println!("\n=== Plan Diff ===");
    for action in &diff.actions {
        match action {
            SpecMatchAction::New { id } => println!("  New task {id}"),
            SpecMatchAction::Updated { id } => println!("  Updated {id}"),
        }
    }
    for orphan_id in &diff.orphaned {
        println!("  Orphaned {orphan_id} (no longer in doc)");
    }
    if diff.actions.is_empty() && diff.orphaned.is_empty() {
        println!("  (no changes)");
    }
}

/// Print the plan score in structured, LLM-parseable format.
fn print_score(result: &ScoreResult) {
    println!("\n=== Readiness Score ===");
    println!("Overall: {}% — {}", result.overall_score, result.verdict.label());
    println!(
        "  Specificity:   {}%{}",
        result.specificity.score,
        if result.specificity.issues.is_empty() { String::new() } else { " (issues below)".into() }
    );
    for issue in &result.specificity.issues {
        println!("    - {issue}");
    }
    println!(
        "  Verifiability: {}%{}",
        result.verifiability.score,
        if result.verifiability.issues.is_empty() {
            String::new()
        } else {
            " (issues below)".into()
        }
    );
    for issue in &result.verifiability.issues {
        println!("    - {issue}");
    }

    if !result.questions.is_empty() {
        println!("\n=== Scoring Questions ===");
        for (i, q) in result.questions.iter().enumerate() {
            println!("Q{}: {}", i + 1, q.description);
            for (j, opt) in q.options.iter().enumerate() {
                #[allow(clippy::cast_possible_truncation)]
                let label = char::from(b'a' + j as u8);
                let rec = q.recommended.map_or(String::new(), |r| {
                    if r == j {
                        " (recommended)".into()
                    } else {
                        String::new()
                    }
                });
                println!("  {label}) {opt}{rec}");
            }
        }
    }

    if !result.recommendations.is_empty() {
        println!("\n=== Recommendations ===");
        for rec in &result.recommendations {
            println!("  - {rec}");
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

/// Convert a plan-level `PlanCheck` to a spec-level `VerificationCheck`.
fn plan_check_to_verification(check: PlanCheck) -> VerificationCheck {
    match check {
        PlanCheck::CommandOutput { command, expected } => {
            VerificationCheck::CommandOutput { command, expected }
        }
        PlanCheck::TestSuite { command, expected } => {
            VerificationCheck::TestSuite { command, expected }
        }
        PlanCheck::Custom { description } => VerificationCheck::Custom { description },
    }
}

/// Map a plan verification strategy to a spec verification strategy.
fn map_verification_strategy(plan_strategy: PlanVerificationStrategy) -> VerificationStrategy {
    match plan_strategy {
        PlanVerificationStrategy::DirectAssertion { checks } => {
            VerificationStrategy::DirectAssertion {
                checks: checks.into_iter().map(plan_check_to_verification).collect(),
            }
        }
        PlanVerificationStrategy::StructuralDecomposition { sub_assertions } => {
            VerificationStrategy::DirectAssertion {
                checks: sub_assertions
                    .into_iter()
                    .map(|sa| match sa.check {
                        PlanCheck::Custom { description } => VerificationCheck::Custom {
                            description: format!("{}: {}", sa.description, description),
                        },
                        other => plan_check_to_verification(other),
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
        affected_globs: None,
    }
}

/// Resolve the store root for `.speck/` persistence.
fn store_root() -> Result<std::path::PathBuf, String> {
    if let Ok(val) = std::env::var("SPECK_STORE") {
        return Ok(std::path::PathBuf::from(val));
    }
    let cwd =
        std::env::current_dir().map_err(|e| format!("failed to get current directory: {e}"))?;
    Ok(cwd.join(".speck"))
}

/// Print a `ReconciliationResult` to stdout in a human-readable format.
fn print_reconciliation(result: &ReconciliationResult) {
    println!("\n=== Reconciliation ===");

    if result.suggested_merges.is_empty()
        && result.suggested_extractions.is_empty()
        && result.suggested_reorders.is_empty()
        && result.circular_dependencies.is_empty()
    {
        println!("  No issues found.");
        return;
    }

    if !result.suggested_merges.is_empty() {
        println!("  Merge suggestions:");
        for m in &result.suggested_merges {
            println!("    - {} -> \"{}\" ({})", m.task_ids.join(", "), m.merged_title, m.reason);
        }
    }

    if !result.suggested_extractions.is_empty() {
        println!("  Extraction suggestions:");
        for e in &result.suggested_extractions {
            println!(
                "    - {} -> \"{}\" ({})",
                e.task_ids.join(", "),
                e.suggested_task_title,
                e.abstraction
            );
        }
    }

    if !result.suggested_reorders.is_empty() {
        println!("  Reorder suggestions:");
        for r in &result.suggested_reorders {
            println!("    - {} should precede {} ({})", r.task_id, r.should_precede, r.reason);
        }
    }

    if !result.circular_dependencies.is_empty() {
        println!("  Circular dependencies:");
        for cycle in &result.circular_dependencies {
            println!("    - {}", cycle.join(" -> "));
        }
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
    use crate::plan::signal::{PlanCheck, SubAssertion, VerificationStrategy as PlanVS};
    use std::collections::HashMap;

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
        let plan_strategy = PlanVS::DirectAssertion {
            checks: vec![
                PlanCheck::CommandOutput { command: "ls".into(), expected: "file.txt".into() },
                PlanCheck::Custom { description: "check2".into() },
            ],
        };
        let spec_strategy = map_verification_strategy(plan_strategy);
        match spec_strategy {
            VerificationStrategy::DirectAssertion { checks } => {
                assert_eq!(checks.len(), 2);
                assert_eq!(
                    checks[0],
                    VerificationCheck::CommandOutput {
                        command: "ls".into(),
                        expected: "file.txt".into(),
                    }
                );
                assert_eq!(checks[1], VerificationCheck::Custom { description: "check2".into() });
            }
            other => panic!("expected DirectAssertion, got {other:?}"),
        }
    }

    #[test]
    fn map_strategy_structural_decomposition() {
        let plan_strategy = PlanVS::StructuralDecomposition {
            sub_assertions: vec![
                SubAssertion {
                    description: "ordered".into(),
                    check: PlanCheck::Custom { description: "assert sorted".into() },
                },
                SubAssertion {
                    description: "runs tests".into(),
                    check: PlanCheck::TestSuite {
                        command: "cargo test".into(),
                        expected: "all pass".into(),
                    },
                },
                SubAssertion {
                    description: "check output".into(),
                    check: PlanCheck::CommandOutput {
                        command: "ls".into(),
                        expected: "file.txt".into(),
                    },
                },
            ],
        };
        let spec_strategy = map_verification_strategy(plan_strategy);
        match spec_strategy {
            VerificationStrategy::DirectAssertion { checks } => {
                assert_eq!(checks.len(), 3);
                // Custom checks get description prefixed
                assert_eq!(
                    checks[0],
                    VerificationCheck::Custom { description: "ordered: assert sorted".into() }
                );
                // Executable checks pass through directly
                assert_eq!(
                    checks[1],
                    VerificationCheck::TestSuite {
                        command: "cargo test".into(),
                        expected: "all pass".into(),
                    }
                );
                assert_eq!(
                    checks[2],
                    VerificationCheck::CommandOutput {
                        command: "ls".into(),
                        expected: "file.txt".into(),
                    }
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
            PlanVS::DirectAssertion {
                checks: vec![PlanCheck::Custom { description: "CLI exports CSV".into() }],
            },
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
                sub_assertions: vec![SubAssertion {
                    description: "d".into(),
                    check: PlanCheck::Custom { description: "c".into() },
                }],
            },
        );
        print_classification(&spec);
    }
}
