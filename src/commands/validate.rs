//! `speck validate` command.

use std::path::PathBuf;

use crate::context::ServiceContext;
use crate::store::SpecStore;
use crate::validate;

/// Execute the `validate` command with a provided context.
///
/// When `spec_id` is provided, validates a single spec.
/// When `--all` is set, validates every spec in the store.
/// Returns an error (non-zero exit) when any check fails.
///
/// # Errors
///
/// Returns an error string if no spec is specified (and `--all` is not set),
/// or if loading/validation fails.
pub fn run_with_context(ctx: &ServiceContext, spec_id: Option<&str>, all: bool) -> Result<(), String> {
    if spec_id.is_none() && !all {
        return Err("Provide a SPEC_ID or use --all to validate all specs".to_string());
    }

    let store_root = store_root()?;
    let store = SpecStore::new(ctx, &store_root);

    let mut results = Vec::new();

    if all {
        let ids = store.list_task_specs()?;
        if ids.is_empty() {
            println!("No specs found in store.");
            return Ok(());
        }
        for id in &ids {
            let spec = store.load_task_spec(id)?;
            results.push(validate::validate(ctx, &spec));
        }
    } else if let Some(id) = spec_id {
        let spec = store.load_task_spec(id)?;
        results.push(validate::validate(ctx, &spec));
    }

    let mut any_failed = false;
    for result in &results {
        println!("{}", validate::format_report(result));
        if !result.passed() {
            any_failed = true;
        }
    }

    if any_failed {
        Err("One or more validation checks failed".to_string())
    } else {
        Ok(())
    }
}

/// Execute the `validate` command with a default live context.
///
/// # Errors
///
/// Returns an error string if no spec is specified (and `--all` is not set),
/// or if loading/validation fails.
pub fn run(spec_id: Option<&str>, all: bool) -> Result<(), String> {
    let ctx = ServiceContext::live();
    run_with_context(&ctx, spec_id, all)
}

/// Resolve the spec store root directory.
fn store_root() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("SPECK_STORE") {
        return Ok(PathBuf::from(path));
    }
    let cwd = std::env::current_dir().map_err(|e| format!("Cannot determine cwd: {e}"))?;
    Ok(cwd.join(".speck"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_validate_requires_spec_id_or_all() {
        let result = run(None, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SPEC_ID"));
    }

    #[test]
    fn cli_validate_all_empty_store() {
        std::env::set_var("SPECK_STORE", "/tmp/speck_test_empty_store_nonexistent");
        let result = run(None, true);
        std::env::remove_var("SPECK_STORE");
        assert!(result.is_ok());
    }

    #[test]
    fn cli_validate_single_spec_not_found() {
        std::env::set_var("SPECK_STORE", "/tmp/speck_test_empty_store_nonexistent");
        let result = run(Some("NONEXISTENT"), false);
        std::env::remove_var("SPECK_STORE");
        assert!(result.is_err());
    }

    #[test]
    fn cli_validate_single_spec_passes() {
        use crate::spec::{SignalType, TaskSpec, VerificationCheck, VerificationStrategy};

        let dir = std::env::temp_dir().join("speck_cli_validate_pass");
        let tasks_dir = dir.join("tasks");
        std::fs::create_dir_all(&tasks_dir).unwrap();

        let spec = TaskSpec {
            id: "TASK-1".to_string(),
            title: "Test task".to_string(),
            requirement: None,
            context: None,
            acceptance_criteria: vec!["it works".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::CommandOutput {
                    command: "echo hello".to_string(),
                    expected: "hello".to_string(),
                }],
            },
        };

        let yaml = serde_yaml::to_string(&spec).unwrap();
        std::fs::write(tasks_dir.join("TASK-1.yaml"), &yaml).unwrap();

        std::env::set_var("SPECK_STORE", dir.to_str().unwrap());
        let result = run(Some("TASK-1"), false);
        std::env::remove_var("SPECK_STORE");

        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_ok());
    }

    #[test]
    fn cli_validate_single_spec_fails() {
        use crate::spec::{SignalType, TaskSpec, VerificationCheck, VerificationStrategy};

        let dir = std::env::temp_dir().join("speck_cli_validate_fail");
        let tasks_dir = dir.join("tasks");
        std::fs::create_dir_all(&tasks_dir).unwrap();

        let spec = TaskSpec {
            id: "TASK-2".to_string(),
            title: "Failing task".to_string(),
            requirement: None,
            context: None,
            acceptance_criteria: vec!["it works".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::TestSuite {
                    command: "false".to_string(),
                    expected: "pass".to_string(),
                }],
            },
        };

        let yaml = serde_yaml::to_string(&spec).unwrap();
        std::fs::write(tasks_dir.join("TASK-2.yaml"), &yaml).unwrap();

        std::env::set_var("SPECK_STORE", dir.to_str().unwrap());
        let result = run(Some("TASK-2"), false);
        std::env::remove_var("SPECK_STORE");

        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed"));
    }
}
