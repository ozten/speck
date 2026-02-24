//! Sync task specs to the Beads issue tracker.
//!
//! Idempotent: re-running does not create duplicates.  Issues are matched
//! to specs by looking for the spec ID prefix (`[SPEC-ID]`) in the title.

use std::fmt::Write;

use crate::context::ServiceContext;
use crate::ports::issues::Issue;
use crate::spec::TaskSpec;

/// What the sync will do (or did) for a single spec.
#[derive(Debug, PartialEq)]
pub enum SyncAction {
    /// A new issue will be / was created.
    Create {
        /// The task spec ID.
        spec_id: String,
        /// The issue title that will be created.
        title: String,
    },
    /// An existing issue will be / was updated.
    Update {
        /// The task spec ID.
        spec_id: String,
        /// The existing issue ID being updated.
        issue_id: String,
        /// The updated issue title.
        title: String,
    },
    /// The existing issue is already up-to-date.
    Unchanged {
        /// The task spec ID.
        spec_id: String,
        /// The matching issue ID.
        issue_id: String,
    },
}

/// Builds the issue title for a task spec.
fn issue_title(spec: &TaskSpec) -> String {
    format!("[{}] {}", spec.id, spec.title)
}

/// Builds the issue body from a task spec, including all structured fields
/// needed for an agent to execute the task without reading additional files.
///
/// Format is designed to round-trip: the `## Verification` section uses a
/// YAML fenced block that `speck validate` can parse back into a
/// [`VerificationStrategy`].
fn issue_body(spec: &TaskSpec) -> String {
    let mut body = String::new();

    // Affected globs line — read by blacksmith scheduler for conflict detection.
    if let Some(globs) = &spec.affected_globs {
        if !globs.is_empty() {
            let _ = writeln!(body, "affected: {}", globs.join(", "));
            body.push('\n');
        }
    }

    // Acceptance criteria (already present, kept as-is).
    body.push_str("## Acceptance Criteria\n");
    for criterion in &spec.acceptance_criteria {
        let _ = writeln!(body, "- {criterion}");
    }

    // Module context — which modules are involved and what patterns to follow.
    if let Some(ctx) = &spec.context {
        if !ctx.modules.is_empty() || ctx.patterns.is_some() {
            body.push_str("\n## Module Context\n");
            if !ctx.modules.is_empty() {
                let _ = writeln!(body, "Modules: {}", ctx.modules.join(", "));
            }
            if let Some(patterns) = &ctx.patterns {
                let _ = writeln!(body, "Patterns: {patterns}");
            }
        }
    }

    // Verification checks in a parseable YAML block for round-trip use.
    body.push_str("\n## Verification\n");
    body.push_str("```yaml\n");
    let yaml = serde_yaml::to_string(&spec.verification).unwrap_or_default();
    body.push_str(&yaml);
    body.push_str("```\n");

    // Dependencies (already present, kept as-is).
    if let Some(ctx) = &spec.context {
        if !ctx.dependencies.is_empty() {
            body.push_str("\n## Dependencies\n");
            for dep in &ctx.dependencies {
                let _ = writeln!(body, "- {dep}");
            }
        }
    }

    body
}

/// Finds an existing issue that matches the given spec ID.
///
/// Matches by looking for `[SPEC-ID]` at the start of the issue title.
fn find_matching_issue<'a>(spec_id: &str, issues: &'a [Issue]) -> Option<&'a Issue> {
    let prefix = format!("[{spec_id}]");
    issues.iter().find(|issue| issue.title.starts_with(&prefix))
}

/// Plans sync actions for a list of task specs against existing issues.
#[must_use]
pub fn plan_sync(specs: &[TaskSpec], existing_issues: &[Issue]) -> Vec<SyncAction> {
    specs
        .iter()
        .map(|spec| {
            if let Some(existing) = find_matching_issue(&spec.id, existing_issues) {
                let new_title = issue_title(spec);
                let new_body = issue_body(spec);
                if existing.title == new_title && existing.body == new_body {
                    SyncAction::Unchanged {
                        spec_id: spec.id.clone(),
                        issue_id: existing.id.clone(),
                    }
                } else {
                    SyncAction::Update {
                        spec_id: spec.id.clone(),
                        issue_id: existing.id.clone(),
                        title: new_title,
                    }
                }
            } else {
                SyncAction::Create { spec_id: spec.id.clone(), title: issue_title(spec) }
            }
        })
        .collect()
}

/// Executes the planned sync actions against the issue tracker.
///
/// Actions reference specs by ID; every action's `spec_id` **must** appear
/// in `specs` (this is guaranteed when actions come from [`plan_sync`]).
///
/// # Errors
///
/// Returns an error if any issue creation or update fails.
///
/// # Panics
///
/// Panics if an action references a `spec_id` not present in `specs`.
pub fn execute_sync(
    ctx: &ServiceContext,
    specs: &[TaskSpec],
    actions: &[SyncAction],
) -> Result<(), String> {
    for action in actions {
        match action {
            SyncAction::Create { spec_id, .. } => {
                let spec = specs
                    .iter()
                    .find(|s| s.id == *spec_id)
                    .expect("action references unknown spec");
                let title = issue_title(spec);
                let body = issue_body(spec);
                ctx.issues
                    .create_issue(&title, &body)
                    .map_err(|e| format!("Failed to create issue for {spec_id}: {e}"))?;
            }
            SyncAction::Update { spec_id, issue_id, .. } => {
                let spec = specs
                    .iter()
                    .find(|s| s.id == *spec_id)
                    .expect("action references unknown spec");
                let title = issue_title(spec);
                let body = issue_body(spec);
                ctx.issues
                    .update_issue(issue_id, Some(&title), Some(&body), None)
                    .map_err(|e| format!("Failed to update issue for {spec_id}: {e}"))?;
            }
            SyncAction::Unchanged { .. } => {}
        }
    }
    Ok(())
}

/// Formats sync actions as a human-readable report.
#[must_use]
pub fn format_actions(actions: &[SyncAction]) -> String {
    if actions.is_empty() {
        return "No specs to sync.".to_string();
    }

    let mut lines = Vec::new();
    for action in actions {
        match action {
            SyncAction::Create { spec_id, title } => {
                lines.push(format!("  CREATE {spec_id}: {title}"));
            }
            SyncAction::Update { spec_id, issue_id, title } => {
                lines.push(format!("  UPDATE {spec_id} (issue {issue_id}): {title}"));
            }
            SyncAction::Unchanged { spec_id, issue_id, .. } => {
                lines.push(format!("  UNCHANGED {spec_id} (issue {issue_id})"));
            }
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{SignalType, TaskContext, VerificationCheck, VerificationStrategy};

    fn sample_spec(id: &str, title: &str) -> TaskSpec {
        TaskSpec {
            id: id.to_string(),
            title: title.to_string(),
            requirement: None,
            context: None,
            acceptance_criteria: vec!["it works".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::TestSuite {
                    command: "cargo test".to_string(),
                    expected: "pass".to_string(),
                }],
            },
            affected_globs: None,
        }
    }

    fn sample_spec_with_deps(id: &str, title: &str, deps: Vec<&str>) -> TaskSpec {
        TaskSpec {
            context: Some(TaskContext {
                modules: vec![],
                patterns: None,
                dependencies: deps.into_iter().map(String::from).collect(),
            }),
            ..sample_spec(id, title)
        }
    }

    fn sample_spec_full() -> TaskSpec {
        TaskSpec {
            id: "T-99".to_string(),
            title: "Full spec".to_string(),
            requirement: None,
            context: Some(TaskContext {
                modules: vec!["MetricsService".to_string(), "AuthService".to_string()],
                patterns: Some("Follow existing migration conventions".to_string()),
                dependencies: vec!["T-1".to_string()],
            }),
            acceptance_criteria: vec!["it works".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::TestSuite {
                    command: "cargo test".to_string(),
                    expected: "pass".to_string(),
                }],
            },
            affected_globs: Some(vec![
                "src/services/metrics/**".to_string(),
                "src/lib.rs".to_string(),
            ]),
        }
    }

    #[test]
    fn plan_creates_for_new_specs() {
        let specs = vec![sample_spec("T-1", "First task")];
        let actions = plan_sync(&specs, &[]);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::Create { spec_id, .. } if spec_id == "T-1"));
    }

    #[test]
    fn plan_marks_unchanged_when_matching() {
        let specs = vec![sample_spec("T-1", "First task")];
        let existing = vec![Issue {
            id: "ISS-1".to_string(),
            title: "[T-1] First task".to_string(),
            body: issue_body(&specs[0]),
            status: "open".to_string(),
        }];
        let actions = plan_sync(&specs, &existing);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::Unchanged { spec_id, .. } if spec_id == "T-1"));
    }

    #[test]
    fn plan_marks_update_when_title_differs() {
        let specs = vec![sample_spec("T-1", "Updated title")];
        let existing = vec![Issue {
            id: "ISS-1".to_string(),
            title: "[T-1] Old title".to_string(),
            body: issue_body(&specs[0]),
            status: "open".to_string(),
        }];
        let actions = plan_sync(&specs, &existing);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::Update { spec_id, .. } if spec_id == "T-1"));
    }

    #[test]
    fn plan_marks_update_when_body_differs() {
        let specs = vec![sample_spec("T-1", "First task")];
        let existing = vec![Issue {
            id: "ISS-1".to_string(),
            title: "[T-1] First task".to_string(),
            body: "old body".to_string(),
            status: "open".to_string(),
        }];
        let actions = plan_sync(&specs, &existing);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::Update { spec_id, .. } if spec_id == "T-1"));
    }

    #[test]
    fn issue_body_includes_dependencies() {
        let spec = sample_spec_with_deps("T-1", "Task with deps", vec!["T-0", "T-2"]);
        let body = issue_body(&spec);
        assert!(body.contains("## Dependencies"));
        assert!(body.contains("- T-0"));
        assert!(body.contains("- T-2"));
    }

    #[test]
    fn issue_body_includes_affected_globs() {
        let mut spec = sample_spec("T-1", "Task");
        spec.affected_globs = Some(vec!["src/foo/**".to_string(), "src/bar.rs".to_string()]);
        let body = issue_body(&spec);
        assert!(body.contains("affected: src/foo/**, src/bar.rs"), "body was: {body}");
    }

    #[test]
    fn issue_body_omits_affected_when_none() {
        let spec = sample_spec("T-1", "Task");
        let body = issue_body(&spec);
        assert!(!body.contains("affected:"), "body was: {body}");
    }

    #[test]
    fn issue_body_omits_affected_when_empty() {
        let mut spec = sample_spec("T-1", "Task");
        spec.affected_globs = Some(vec![]);
        let body = issue_body(&spec);
        assert!(!body.contains("affected:"), "body was: {body}");
    }

    #[test]
    fn issue_body_includes_verification_yaml_block() {
        let spec = sample_spec("T-1", "Task");
        let body = issue_body(&spec);
        assert!(body.contains("## Verification"), "body was: {body}");
        assert!(body.contains("```yaml"), "body was: {body}");
        assert!(body.contains("strategy: direct_assertion"), "body was: {body}");
        assert!(body.contains("command: cargo test"), "body was: {body}");
    }

    #[test]
    fn issue_body_verification_yaml_round_trips() {
        use crate::spec::VerificationStrategy;
        let spec = sample_spec("T-1", "Task");
        let body = issue_body(&spec);
        // Extract YAML between ```yaml and ```
        let start = body.find("```yaml\n").expect("yaml block start") + 8;
        let end = body[start..].find("\n```").expect("yaml block end") + start;
        let yaml_str = &body[start..end];
        let parsed: VerificationStrategy =
            serde_yaml::from_str(yaml_str).expect("should parse back");
        assert_eq!(parsed, spec.verification);
    }

    #[test]
    fn issue_body_includes_module_context() {
        let spec = sample_spec_full();
        let body = issue_body(&spec);
        assert!(body.contains("## Module Context"), "body was: {body}");
        assert!(body.contains("Modules: MetricsService, AuthService"), "body was: {body}");
        assert!(
            body.contains("Patterns: Follow existing migration conventions"),
            "body was: {body}"
        );
    }

    #[test]
    fn issue_body_full_spec_section_order() {
        let spec = sample_spec_full();
        let body = issue_body(&spec);
        let affected_pos = body.find("affected:").expect("affected line");
        let criteria_pos = body.find("## Acceptance Criteria").expect("criteria");
        let module_pos = body.find("## Module Context").expect("module context");
        let verify_pos = body.find("## Verification").expect("verification");
        let deps_pos = body.find("## Dependencies").expect("dependencies");
        assert!(
            affected_pos < criteria_pos
                && criteria_pos < module_pos
                && module_pos < verify_pos
                && verify_pos < deps_pos,
            "unexpected section order in body"
        );
    }

    #[test]
    fn format_actions_shows_all_types() {
        let actions = vec![
            SyncAction::Create { spec_id: "T-1".to_string(), title: "[T-1] New".to_string() },
            SyncAction::Update {
                spec_id: "T-2".to_string(),
                issue_id: "ISS-2".to_string(),
                title: "[T-2] Changed".to_string(),
            },
            SyncAction::Unchanged { spec_id: "T-3".to_string(), issue_id: "ISS-3".to_string() },
        ];
        let output = format_actions(&actions);
        assert!(output.contains("CREATE T-1"));
        assert!(output.contains("UPDATE T-2"));
        assert!(output.contains("UNCHANGED T-3"));
    }

    #[test]
    fn format_actions_empty() {
        let output = format_actions(&[]);
        assert_eq!(output, "No specs to sync.");
    }
}
