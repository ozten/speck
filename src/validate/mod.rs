//! Validation logic and types.
//!
//! Loads a task spec and runs its verification checks, returning
//! a per-check pass/fail report.

use crate::context::ServiceContext;
use crate::linkage;
use crate::map::CodebaseMap;
use crate::spec::{TaskSpec, VerificationCheck, VerificationStrategy};

/// The category of a verification check, used for feedback classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckCategory {
    /// A test suite or command that was executed.
    Executable,
    /// A check that requires manual review (SQL, migration, custom, refactor, trace).
    ManualReview,
    /// A drift warning from codebase map comparison.
    Drift,
}

/// Result of a single verification check.
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Human-readable name describing the check.
    pub name: String,
    /// Whether the check passed.
    pub passed: bool,
    /// Detail message (e.g. error output on failure).
    pub detail: String,
    /// What was expected (from the spec).
    pub expected: String,
    /// What was actually observed.
    pub actual: String,
    /// Category of this check for feedback classification.
    pub category: CheckCategory,
}

/// Aggregated result of validating all checks in a task spec.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// The spec ID that was validated.
    pub spec_id: String,
    /// Per-check results.
    pub checks: Vec<CheckResult>,
}

impl ValidationResult {
    /// Returns `true` if every check passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }

    /// Returns only the checks that failed.
    #[must_use]
    pub fn failed_checks(&self) -> Vec<&CheckResult> {
        self.checks.iter().filter(|c| !c.passed).collect()
    }
}

/// Validates a task spec by running its verification checks.
///
/// For `TestSuite` and `CommandOutput` checks the command is executed
/// via `ctx.shell`. Other check types are recorded as skipped.
#[must_use]
pub fn validate(ctx: &ServiceContext, spec: &TaskSpec) -> ValidationResult {
    let checks = match &spec.verification {
        VerificationStrategy::DirectAssertion { checks } => {
            checks.iter().map(|check| run_check(ctx, check)).collect()
        }
        VerificationStrategy::RefactorToExpose { decision_point, .. } => {
            vec![CheckResult {
                name: format!("refactor-to-expose: {decision_point}"),
                passed: false,
                detail: "RefactorToExpose checks require manual review".to_string(),
                expected: "manual refactoring completed".to_string(),
                actual: "not yet reviewed".to_string(),
                category: CheckCategory::ManualReview,
            }]
        }
        VerificationStrategy::TraceAssertion { trace_point, .. } => {
            vec![CheckResult {
                name: format!("trace-assertion: {trace_point}"),
                passed: false,
                detail: "TraceAssertion checks require manual review".to_string(),
                expected: "trace matches expected output".to_string(),
                actual: "not yet reviewed".to_string(),
                category: CheckCategory::ManualReview,
            }]
        }
    };

    ValidationResult { spec_id: spec.id.clone(), checks }
}

fn run_check(ctx: &ServiceContext, check: &VerificationCheck) -> CheckResult {
    match check {
        VerificationCheck::TestSuite { command, expected } => {
            run_shell_check(ctx, &format!("test-suite: {command}"), command, expected)
        }
        VerificationCheck::CommandOutput { command, expected } => {
            run_shell_check(ctx, &format!("command-output: {command}"), command, expected)
        }
        VerificationCheck::SqlAssertion { query, expected } => CheckResult {
            name: format!("sql-assertion: {query}"),
            passed: false,
            detail: format!("SQL assertion checks not yet supported (expected: {expected})"),
            expected: expected.clone(),
            actual: "not executed".to_string(),
            category: CheckCategory::ManualReview,
        },
        VerificationCheck::MigrationRollback { description } => CheckResult {
            name: format!("migration-rollback: {description}"),
            passed: false,
            detail: "Migration rollback checks require manual review".to_string(),
            expected: "rollback succeeds".to_string(),
            actual: "not yet reviewed".to_string(),
            category: CheckCategory::ManualReview,
        },
        VerificationCheck::Custom { description } => CheckResult {
            name: format!("custom: {description}"),
            passed: false,
            detail: "Custom checks require manual review".to_string(),
            expected: description.clone(),
            actual: "not yet reviewed".to_string(),
            category: CheckCategory::ManualReview,
        },
    }
}

fn run_shell_check(ctx: &ServiceContext, name: &str, command: &str, expected: &str) -> CheckResult {
    match ctx.shell.run(command) {
        Ok(output) => {
            let passed = output.exit_code == 0;
            let actual = if passed {
                "exit code 0".to_string()
            } else {
                format!("exit code {}", output.exit_code)
            };
            let detail = if passed {
                format!("exit code 0 (expected: {expected})")
            } else {
                format!(
                    "exit code {} (expected: {expected})\nstderr: {}",
                    output.exit_code, output.stderr
                )
            };
            CheckResult {
                name: name.to_string(),
                passed,
                detail,
                expected: expected.to_string(),
                actual,
                category: CheckCategory::Executable,
            }
        }
        Err(e) => CheckResult {
            name: name.to_string(),
            passed: false,
            detail: format!("failed to run command: {e}"),
            expected: expected.to_string(),
            actual: format!("error: {e}"),
            category: CheckCategory::Executable,
        },
    }
}

/// Validates a task spec and includes drift warnings if codebase maps are provided.
///
/// When `old_map` and `new_map` are both `Some`, runs drift detection before
/// validation and prepends any drift warnings as check results.
#[must_use]
pub fn validate_with_drift(
    ctx: &ServiceContext,
    spec: &TaskSpec,
    old_map: Option<&CodebaseMap>,
    new_map: Option<&CodebaseMap>,
) -> ValidationResult {
    let mut result = validate(ctx, spec);

    if let (Some(old), Some(new)) = (old_map, new_map) {
        let drift_report = linkage::detect_drift(std::slice::from_ref(spec), old, new);
        if !drift_report.is_clean() {
            for entry in &drift_report.entries {
                for path in &entry.changed_modules {
                    result.checks.insert(
                        0,
                        CheckResult {
                            name: format!("drift-warning: {path}"),
                            passed: false,
                            detail: "Module has changed since spec was written".to_string(),
                            expected: "module unchanged since spec creation".to_string(),
                            actual: "module has been modified".to_string(),
                            category: CheckCategory::Drift,
                        },
                    );
                }
                for path in &entry.removed_modules {
                    result.checks.insert(
                        0,
                        CheckResult {
                            name: format!("drift-warning: {path}"),
                            passed: false,
                            detail: "Module has been removed from the codebase".to_string(),
                            expected: "module exists in codebase".to_string(),
                            actual: "module has been removed".to_string(),
                            category: CheckCategory::Drift,
                        },
                    );
                }
                if entry.replan_recommended {
                    result.checks.insert(
                        0,
                        CheckResult {
                            name: "drift-warning: re-plan recommended".to_string(),
                            passed: false,
                            detail: "Significant drift detected; re-planning is recommended"
                                .to_string(),
                            expected: "codebase stable since spec creation".to_string(),
                            actual: "significant drift detected".to_string(),
                            category: CheckCategory::Drift,
                        },
                    );
                }
            }
        }
    }

    result
}

/// Formats a `ValidationResult` as a human-readable report.
#[must_use]
pub fn format_report(result: &ValidationResult) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Spec: {}", result.spec_id));
    lines.push(String::new());
    for check in &result.checks {
        let status = if check.passed { "PASS" } else { "FAIL" };
        lines.push(format!("  [{status}] {}", check.name));
        if !check.passed {
            for detail_line in check.detail.lines() {
                lines.push(format!("         {detail_line}"));
            }
            if !check.expected.is_empty() || !check.actual.is_empty() {
                lines.push(format!("         expected: {}", check.expected));
                lines.push(format!("         actual:   {}", check.actual));
            }
        }
    }
    lines.push(String::new());
    let overall = if result.passed() { "PASSED" } else { "FAILED" };
    lines.push(format!("Result: {overall}"));

    if !result.passed() {
        lines.push(String::new());
        lines.push("Next steps:".to_string());
        let next_steps = suggest_next_steps(result);
        for step in &next_steps {
            lines.push(format!("  - {step}"));
        }
    }

    lines.join("\n")
}

/// Suggests actionable next steps based on failure types in a `ValidationResult`.
#[must_use]
pub fn suggest_next_steps(result: &ValidationResult) -> Vec<String> {
    use crate::plan::feedback::{classify_failures, FailureType};

    let classification = classify_failures(result);
    let mut steps = Vec::new();

    for failure in &classification.failures {
        match &failure.failure_type {
            FailureType::ImplementationFailure { fix_hint } => {
                steps.push(format!(
                    "[impl] Fix failing check '{}': {}",
                    failure.check_name, fix_hint,
                ));
            }
            FailureType::SpecFlaw { revision_hint } => {
                steps.push(format!(
                    "[spec] Revise spec for '{}': {}",
                    failure.check_name, revision_hint,
                ));
            }
        }
    }

    if steps.is_empty() && !result.passed() {
        steps.push("Review failing checks and determine whether the implementation or spec needs updating.".to_string());
    }

    steps
}
