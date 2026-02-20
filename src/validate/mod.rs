//! Validation logic and types.
//!
//! Loads a task spec from the store and runs its verification checks.
//! Returns structured results that the CLI renders as human-readable output.

use crate::context::ServiceContext;
use crate::spec::{TaskSpec, VerificationCheck, VerificationStrategy};

/// Result of a single verification check.
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Human-readable name of the check.
    pub name: String,
    /// Whether the check passed.
    pub passed: bool,
    /// Detail message (stdout, error, etc.).
    pub message: String,
}

/// Result of validating an entire task spec.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// The spec ID that was validated.
    pub spec_id: String,
    /// Per-check results.
    pub checks: Vec<CheckResult>,
}

impl ValidationResult {
    /// Returns `true` if all checks passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }
}

/// Validate a single task spec by running its verification checks.
///
/// Uses `ctx.shell` for `TestSuite` and `CommandOutput` checks.
/// Other check types are noted as skipped.
#[must_use]
pub fn validate_spec(ctx: &ServiceContext, spec: &TaskSpec) -> ValidationResult {
    let checks = match &spec.verification {
        VerificationStrategy::DirectAssertion { checks } => {
            checks.iter().map(|check| run_check(ctx, check)).collect()
        }
        VerificationStrategy::RefactorToExpose { decision_point, .. } => {
            vec![CheckResult {
                name: "refactor_to_expose".to_string(),
                passed: false,
                message: format!("RefactorToExpose strategy not yet supported: {decision_point}"),
            }]
        }
        VerificationStrategy::TraceAssertion { trace_point, .. } => {
            vec![CheckResult {
                name: "trace_assertion".to_string(),
                passed: false,
                message: format!("TraceAssertion strategy not yet supported: {trace_point}"),
            }]
        }
    };

    ValidationResult { spec_id: spec.id.clone(), checks }
}

/// Run a single verification check.
fn run_check(ctx: &ServiceContext, check: &VerificationCheck) -> CheckResult {
    match check {
        VerificationCheck::TestSuite { command, .. } => match ctx.shell.run(command) {
            Ok(output) => CheckResult {
                name: format!("test_suite: {command}"),
                passed: output.exit_code == 0,
                message: if output.exit_code == 0 {
                    "passed".to_string()
                } else {
                    format!("exit code {}\n{}", output.exit_code, output.stderr)
                },
            },
            Err(e) => CheckResult {
                name: format!("test_suite: {command}"),
                passed: false,
                message: format!("failed to execute: {e}"),
            },
        },
        VerificationCheck::CommandOutput { command, expected } => match ctx.shell.run(command) {
            Ok(output) => {
                let stdout = output.stdout.trim().to_string();
                let matches = stdout.contains(expected);
                CheckResult {
                    name: format!("command_output: {command}"),
                    passed: output.exit_code == 0 && matches,
                    message: if matches {
                        "output matches expected".to_string()
                    } else {
                        format!("expected output containing \"{expected}\", got: {stdout}")
                    },
                }
            }
            Err(e) => CheckResult {
                name: format!("command_output: {command}"),
                passed: false,
                message: format!("failed to execute: {e}"),
            },
        },
        VerificationCheck::SqlAssertion { query, .. } => CheckResult {
            name: format!("sql_assertion: {query}"),
            passed: false,
            message: "SQL assertion checks not yet supported".to_string(),
        },
        VerificationCheck::MigrationRollback { description } => CheckResult {
            name: "migration_rollback".to_string(),
            passed: false,
            message: format!("Migration rollback checks not yet supported: {description}"),
        },
        VerificationCheck::Custom { description } => CheckResult {
            name: "custom".to_string(),
            passed: false,
            message: format!("Custom checks require manual verification: {description}"),
        },
    }
}

/// Format a validation result as human-readable text.
#[must_use]
pub fn format_result(result: &ValidationResult) -> String {
    use std::fmt::Write;

    let mut out = String::new();
    let status = if result.passed() { "PASS" } else { "FAIL" };
    let _ = writeln!(out, "Spec {} — {status}", result.spec_id);

    for check in &result.checks {
        let icon = if check.passed { "  [PASS]" } else { "  [FAIL]" };
        let _ = writeln!(out, "{icon} {}", check.name);
        if !check.passed {
            for line in check.message.lines() {
                let _ = writeln!(out, "         {line}");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::config::CassetteConfig;
    use crate::spec::{SignalType, VerificationCheck, VerificationStrategy};

    /// In-memory filesystem for testing.
    struct MemFs {
        files: std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, String>>,
    }

    impl MemFs {
        fn new() -> Self {
            Self { files: std::sync::Mutex::new(std::collections::HashMap::new()) }
        }
    }

    impl crate::ports::filesystem::FileSystem for MemFs {
        fn read_to_string(
            &self,
            path: &std::path::Path,
        ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
            let files = self.files.lock().unwrap();
            files
                .get(path)
                .cloned()
                .ok_or_else(|| format!("File not found: {}", path.display()).into())
        }

        fn write(
            &self,
            path: &std::path::Path,
            contents: &str,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let mut files = self.files.lock().unwrap();
            files.insert(path.to_path_buf(), contents.to_string());
            Ok(())
        }

        fn exists(&self, path: &std::path::Path) -> bool {
            let files = self.files.lock().unwrap();
            files.contains_key(path) || files.keys().any(|k| k.starts_with(path) && k != path)
        }

        fn list_dir(
            &self,
            path: &std::path::Path,
        ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
            let files = self.files.lock().unwrap();
            let mut names: Vec<String> = files
                .keys()
                .filter_map(|k| {
                    if k.parent() == Some(path) {
                        k.file_name().map(|n| n.to_string_lossy().into_owned())
                    } else {
                        None
                    }
                })
                .collect();
            names.sort();
            Ok(names)
        }
    }

    /// Stub shell that returns a canned response.
    struct StubShell {
        exit_code: i32,
        stdout: String,
    }

    impl crate::ports::shell::ShellExecutor for StubShell {
        fn run(
            &self,
            _command: &str,
        ) -> Result<crate::ports::shell::ShellOutput, Box<dyn std::error::Error + Send + Sync>>
        {
            Ok(crate::ports::shell::ShellOutput {
                exit_code: self.exit_code,
                stdout: self.stdout.clone(),
                stderr: String::new(),
            })
        }
    }

    fn make_ctx(exit_code: i32, stdout: &str) -> ServiceContext {
        let mut ctx = ServiceContext::replaying_from(&CassetteConfig::panic_on_unspecified())
            .expect("panic config should always succeed");
        ctx.fs = Box::new(MemFs::new());
        ctx.shell = Box::new(StubShell { exit_code, stdout: stdout.to_string() });
        ctx
    }

    fn sample_spec(id: &str) -> TaskSpec {
        TaskSpec {
            id: id.to_string(),
            title: format!("Test task {id}"),
            requirement: Some("test-req".to_string()),
            context: None,
            acceptance_criteria: vec!["it works".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::TestSuite {
                    command: "cargo test".to_string(),
                    expected: "all pass".to_string(),
                }],
            },
        }
    }

    #[test]
    fn cli_validate_passing_spec() {
        let ctx = make_ctx(0, "all pass");
        let spec = sample_spec("IMPACT-42");
        let result = validate_spec(&ctx, &spec);
        assert!(result.passed());
        assert_eq!(result.checks.len(), 1);
        assert!(result.checks[0].passed);
    }

    #[test]
    fn cli_validate_failing_spec() {
        let ctx = make_ctx(1, "FAILED");
        let spec = sample_spec("IMPACT-42");
        let result = validate_spec(&ctx, &spec);
        assert!(!result.passed());
        assert!(!result.checks[0].passed);
    }

    #[test]
    fn cli_validate_format_result_passing() {
        let result = ValidationResult {
            spec_id: "TASK-1".to_string(),
            checks: vec![CheckResult {
                name: "test_suite: cargo test".to_string(),
                passed: true,
                message: "passed".to_string(),
            }],
        };
        let output = format_result(&result);
        assert!(output.contains("PASS"));
        assert!(output.contains("TASK-1"));
    }

    #[test]
    fn cli_validate_format_result_failing() {
        let result = ValidationResult {
            spec_id: "TASK-1".to_string(),
            checks: vec![CheckResult {
                name: "test_suite: cargo test".to_string(),
                passed: false,
                message: "exit code 1".to_string(),
            }],
        };
        let output = format_result(&result);
        assert!(output.contains("FAIL"));
        assert!(output.contains("exit code 1"));
    }

    #[test]
    fn cli_validate_command_output_matching() {
        let ctx = make_ctx(0, "hello world");
        let spec = TaskSpec {
            id: "CMD-1".to_string(),
            title: "Command test".to_string(),
            requirement: None,
            context: None,
            acceptance_criteria: vec![],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::CommandOutput {
                    command: "echo hello".to_string(),
                    expected: "hello".to_string(),
                }],
            },
        };
        let result = validate_spec(&ctx, &spec);
        assert!(result.passed());
    }

    #[test]
    fn cli_validate_command_output_not_matching() {
        let ctx = make_ctx(0, "something else");
        let spec = TaskSpec {
            id: "CMD-2".to_string(),
            title: "Command test".to_string(),
            requirement: None,
            context: None,
            acceptance_criteria: vec![],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::CommandOutput {
                    command: "echo hello".to_string(),
                    expected: "hello world".to_string(),
                }],
            },
        };
        let result = validate_spec(&ctx, &spec);
        assert!(!result.passed());
    }
}
