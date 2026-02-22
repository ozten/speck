//! Feedback classification for validation failures.
//!
//! Ingests a `ValidationResult` and classifies each failure as either an
//! implementation failure (the agent needs to iterate on the code) or a
//! spec flaw (the plan should revise the contract). This closes the
//! recursive feedback loop between `speck validate` and `speck plan`.

use crate::validate::{CheckCategory, ValidationResult};

/// The type of failure detected for a single check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureType {
    /// The implementation doesn't meet the spec — the agent should fix the code.
    ImplementationFailure {
        /// A hint about what needs to be fixed.
        fix_hint: String,
    },
    /// The spec itself is flawed — the plan should revise the contract.
    SpecFlaw {
        /// A hint about how the spec should be revised.
        revision_hint: String,
    },
}

/// A classified failure from a validation check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedFailure {
    /// The name of the check that failed.
    pub check_name: String,
    /// The type of failure and suggested action.
    pub failure_type: FailureType,
    /// The detail message from the original check.
    pub detail: String,
    /// What was expected.
    pub expected: String,
    /// What was actually observed.
    pub actual: String,
}

/// The result of classifying all failures in a `ValidationResult`.
#[derive(Debug, Clone)]
pub struct FeedbackClassification {
    /// The spec ID that was validated.
    pub spec_id: String,
    /// Classified failures.
    pub failures: Vec<ClassifiedFailure>,
}

impl FeedbackClassification {
    /// Returns `true` if all failures are implementation failures (agent should iterate).
    #[must_use]
    pub fn all_implementation_failures(&self) -> bool {
        !self.failures.is_empty()
            && self
                .failures
                .iter()
                .all(|f| matches!(f.failure_type, FailureType::ImplementationFailure { .. }))
    }

    /// Returns `true` if any failure is a spec flaw (plan should revise).
    #[must_use]
    pub fn has_spec_flaws(&self) -> bool {
        self.failures.iter().any(|f| matches!(f.failure_type, FailureType::SpecFlaw { .. }))
    }

    /// Returns only the implementation failures.
    #[must_use]
    pub fn implementation_failures(&self) -> Vec<&ClassifiedFailure> {
        self.failures
            .iter()
            .filter(|f| matches!(f.failure_type, FailureType::ImplementationFailure { .. }))
            .collect()
    }

    /// Returns only the spec flaws.
    #[must_use]
    pub fn spec_flaws(&self) -> Vec<&ClassifiedFailure> {
        self.failures
            .iter()
            .filter(|f| matches!(f.failure_type, FailureType::SpecFlaw { .. }))
            .collect()
    }
}

/// Classifies all failures in a `ValidationResult` as implementation failures or spec flaws.
///
/// Classification heuristics:
/// - **Executable** checks (test suites, commands) that fail with nonzero exit → implementation failure
/// - **Executable** checks that fail to run at all → implementation failure
/// - **Drift** warnings → spec flaw (codebase changed, spec is outdated)
/// - **`ManualReview`** checks → spec flaw (verification strategy needs redesign)
#[must_use]
pub fn classify_failures(result: &ValidationResult) -> FeedbackClassification {
    let failures = result
        .failed_checks()
        .into_iter()
        .map(|check| {
            let failure_type = match check.category {
                CheckCategory::Executable => FailureType::ImplementationFailure {
                    fix_hint: format!(
                        "Check '{}' failed (expected: {}, got: {}). Fix the implementation to pass this check.",
                        check.name, check.expected, check.actual,
                    ),
                },
                CheckCategory::Drift => FailureType::SpecFlaw {
                    revision_hint: format!(
                        "Codebase drift detected for '{}'. Run `speck plan` to update the spec against the current codebase.",
                        check.name,
                    ),
                },
                CheckCategory::ManualReview => FailureType::SpecFlaw {
                    revision_hint: format!(
                        "Check '{}' requires manual review and cannot be automated. Consider revising the verification strategy to use executable checks.",
                        check.name,
                    ),
                },
            };

            ClassifiedFailure {
                check_name: check.name.clone(),
                failure_type,
                detail: check.detail.clone(),
                expected: check.expected.clone(),
                actual: check.actual.clone(),
            }
        })
        .collect();

    FeedbackClassification { spec_id: result.spec_id.clone(), failures }
}

/// Proposes spec revisions based on a feedback classification.
///
/// Returns a list of suggested changes to the spec, suitable for feeding
/// back into the planning pass.
#[must_use]
pub fn propose_revisions(classification: &FeedbackClassification) -> Vec<SpecRevision> {
    classification
        .spec_flaws()
        .into_iter()
        .map(|flaw| {
            let action = match &flaw.failure_type {
                FailureType::SpecFlaw { revision_hint } => revision_hint.clone(),
                FailureType::ImplementationFailure { .. } => unreachable!(),
            };
            SpecRevision {
                spec_id: classification.spec_id.clone(),
                check_name: flaw.check_name.clone(),
                action,
            }
        })
        .collect()
}

/// A proposed revision to a spec based on validation feedback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecRevision {
    /// The spec ID to revise.
    pub spec_id: String,
    /// The check that triggered this revision.
    pub check_name: String,
    /// Description of the revision action to take.
    pub action: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::{CheckCategory, CheckResult, ValidationResult};

    fn make_result(checks: Vec<CheckResult>) -> ValidationResult {
        ValidationResult { spec_id: "TASK-1".to_string(), checks }
    }

    fn exec_pass(name: &str) -> CheckResult {
        CheckResult {
            name: name.to_string(),
            passed: true,
            detail: "exit code 0".to_string(),
            expected: "all pass".to_string(),
            actual: "exit code 0".to_string(),
            category: CheckCategory::Executable,
        }
    }

    fn exec_fail(name: &str) -> CheckResult {
        CheckResult {
            name: name.to_string(),
            passed: false,
            detail: "exit code 1\nstderr: test failed".to_string(),
            expected: "all pass".to_string(),
            actual: "exit code 1".to_string(),
            category: CheckCategory::Executable,
        }
    }

    fn drift_fail(name: &str) -> CheckResult {
        CheckResult {
            name: name.to_string(),
            passed: false,
            detail: "Module has changed since spec was written".to_string(),
            expected: "module unchanged since spec creation".to_string(),
            actual: "module has been modified".to_string(),
            category: CheckCategory::Drift,
        }
    }

    fn manual_fail(name: &str) -> CheckResult {
        CheckResult {
            name: name.to_string(),
            passed: false,
            detail: "requires manual review".to_string(),
            expected: "manual review completed".to_string(),
            actual: "not yet reviewed".to_string(),
            category: CheckCategory::ManualReview,
        }
    }

    #[test]
    fn classify_all_passing_returns_empty() {
        let result = make_result(vec![exec_pass("test-1"), exec_pass("test-2")]);
        let classification = classify_failures(&result);
        assert!(classification.failures.is_empty());
    }

    #[test]
    fn classify_executable_failure_as_implementation() {
        let result = make_result(vec![exec_fail("cargo test"), exec_pass("echo hello")]);
        let classification = classify_failures(&result);
        assert_eq!(classification.failures.len(), 1);
        assert!(classification.all_implementation_failures());
        assert!(!classification.has_spec_flaws());
        assert!(matches!(
            &classification.failures[0].failure_type,
            FailureType::ImplementationFailure { .. }
        ));
    }

    #[test]
    fn classify_drift_failure_as_spec_flaw() {
        let result = make_result(vec![drift_fail("drift-warning: src/api.rs")]);
        let classification = classify_failures(&result);
        assert_eq!(classification.failures.len(), 1);
        assert!(classification.has_spec_flaws());
        assert!(!classification.all_implementation_failures());
        assert!(matches!(&classification.failures[0].failure_type, FailureType::SpecFlaw { .. }));
    }

    #[test]
    fn classify_manual_review_as_spec_flaw() {
        let result = make_result(vec![manual_fail("refactor-to-expose: decision_point")]);
        let classification = classify_failures(&result);
        assert_eq!(classification.failures.len(), 1);
        assert!(classification.has_spec_flaws());
    }

    #[test]
    fn classify_mixed_failures() {
        let result = make_result(vec![
            exec_fail("cargo test"),
            drift_fail("drift-warning: src/api.rs"),
            manual_fail("custom: manual check"),
        ]);
        let classification = classify_failures(&result);
        assert_eq!(classification.failures.len(), 3);
        assert!(!classification.all_implementation_failures());
        assert!(classification.has_spec_flaws());
        assert_eq!(classification.implementation_failures().len(), 1);
        assert_eq!(classification.spec_flaws().len(), 2);
    }

    #[test]
    fn propose_revisions_for_spec_flaws() {
        let result = make_result(vec![
            drift_fail("drift-warning: src/api.rs"),
            manual_fail("refactor-to-expose: auth logic"),
        ]);
        let classification = classify_failures(&result);
        let revisions = propose_revisions(&classification);
        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[0].spec_id, "TASK-1");
        assert!(revisions[0].action.contains("drift"));
        assert!(revisions[1].action.contains("manual review"));
    }

    #[test]
    fn propose_revisions_empty_for_impl_failures() {
        let result = make_result(vec![exec_fail("cargo test")]);
        let classification = classify_failures(&result);
        let revisions = propose_revisions(&classification);
        assert!(revisions.is_empty());
    }

    #[test]
    fn feedback_classification_preserves_spec_id() {
        let result =
            ValidationResult { spec_id: "MY-SPEC-42".to_string(), checks: vec![exec_fail("test")] };
        let classification = classify_failures(&result);
        assert_eq!(classification.spec_id, "MY-SPEC-42");
    }

    #[test]
    fn classified_failure_includes_expected_and_actual() {
        let result = make_result(vec![exec_fail("cargo test")]);
        let classification = classify_failures(&result);
        let failure = &classification.failures[0];
        assert_eq!(failure.expected, "all pass");
        assert_eq!(failure.actual, "exit code 1");
        assert_eq!(failure.check_name, "cargo test");
    }
}
