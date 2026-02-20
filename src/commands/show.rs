//! `speck show` command.

use std::path::PathBuf;

use crate::context::ServiceContext;
use crate::spec::{SignalType, VerificationCheck, VerificationStrategy};
use crate::store::SpecStore;

/// Execute the `show` command.
///
/// When `id` is provided, pretty-prints the full task spec.
/// When no `id` is given, lists all available spec IDs.
///
/// # Errors
///
/// Returns an error string if spec loading fails.
pub fn run(id: Option<&str>) -> Result<(), String> {
    let ctx = ServiceContext::live();
    let store_root = store_root();
    let store = SpecStore::new(&ctx, &store_root);

    if let Some(spec_id) = id {
        let spec = store.load_task_spec(spec_id)?;
        print_spec(&spec);
        Ok(())
    } else {
        let ids = store.list_task_specs()?;
        if ids.is_empty() {
            println!("No specs found in store.");
        } else {
            println!("Available specs:");
            for id in &ids {
                println!("  {id}");
            }
            println!("\nUse `speck show <SPEC_ID>` to view details.");
        }
        Ok(())
    }
}

fn print_spec(spec: &crate::spec::TaskSpec) {
    println!("Spec: {}", spec.id);
    println!("Title: {}", spec.title);

    if let Some(req) = &spec.requirement {
        println!("Requirement: {req}");
    }

    println!("Signal: {}", format_signal(&spec.signal_type));

    if let Some(ctx) = &spec.context {
        if !ctx.modules.is_empty() {
            println!("Modules: {}", ctx.modules.join(", "));
        }
        if let Some(patterns) = &ctx.patterns {
            println!("Patterns: {patterns}");
        }
        if !ctx.dependencies.is_empty() {
            println!("Dependencies: {}", ctx.dependencies.join(", "));
        }
    }

    println!("\nAcceptance Criteria:");
    for (i, criterion) in spec.acceptance_criteria.iter().enumerate() {
        println!("  {}. {criterion}", i + 1);
    }

    println!("\nVerification:");
    print_verification(&spec.verification);
}

fn format_signal(signal: &SignalType) -> &'static str {
    match signal {
        SignalType::Clear => "clear",
        SignalType::Fuzzy => "fuzzy",
        SignalType::InternalLogic => "internal_logic",
    }
}

fn print_verification(verification: &VerificationStrategy) {
    match verification {
        VerificationStrategy::DirectAssertion { checks } => {
            println!("  Strategy: direct_assertion");
            for check in checks {
                print_check(check);
            }
        }
        VerificationStrategy::RefactorToExpose { decision_point, required_structure, .. } => {
            println!("  Strategy: refactor_to_expose");
            println!("  Decision point: {decision_point}");
            println!("  Required structure: {required_structure}");
        }
        VerificationStrategy::TraceAssertion { trace_point, test_input, .. } => {
            println!("  Strategy: trace_assertion");
            println!("  Trace point: {trace_point}");
            println!("  Test input: {test_input}");
        }
    }
}

fn print_check(check: &VerificationCheck) {
    match check {
        VerificationCheck::TestSuite { command, expected } => {
            println!("  - [test_suite] {command} (expect: {expected})");
        }
        VerificationCheck::SqlAssertion { query, expected } => {
            println!("  - [sql] {query} (expect: {expected})");
        }
        VerificationCheck::CommandOutput { command, expected } => {
            println!("  - [command] {command} (expect: {expected})");
        }
        VerificationCheck::MigrationRollback { description } => {
            println!("  - [migration_rollback] {description}");
        }
        VerificationCheck::Custom { description } => {
            println!("  - [custom] {description}");
        }
    }
}

fn store_root() -> PathBuf {
    std::env::var("SPECK_STORE").map_or_else(|_| PathBuf::from(".speck"), PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_command_no_id_empty_store() {
        std::env::set_var("SPECK_STORE", "/tmp/speck_test_show_empty_nonexistent");
        let result = run(None);
        std::env::remove_var("SPECK_STORE");
        assert!(result.is_ok());
    }

    #[test]
    fn show_command_with_nonexistent_id() {
        std::env::set_var("SPECK_STORE", "/tmp/speck_test_show_empty_nonexistent");
        let result = run(Some("NONEXISTENT"));
        std::env::remove_var("SPECK_STORE");
        assert!(result.is_err());
    }

    #[test]
    fn show_command_displays_spec() {
        use crate::spec::{TaskSpec, VerificationCheck, VerificationStrategy};

        let dir = std::env::temp_dir().join("speck_cli_show_display");
        let tasks_dir = dir.join("tasks");
        std::fs::create_dir_all(&tasks_dir).unwrap();

        let spec = TaskSpec {
            id: "TASK-1".to_string(),
            title: "Test task".to_string(),
            requirement: Some("req-1".to_string()),
            context: None,
            acceptance_criteria: vec!["it works".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::TestSuite {
                    command: "cargo test".to_string(),
                    expected: "pass".to_string(),
                }],
            },
        };

        let yaml = serde_yaml::to_string(&spec).unwrap();
        std::fs::write(tasks_dir.join("TASK-1.yaml"), &yaml).unwrap();

        std::env::set_var("SPECK_STORE", dir.to_str().unwrap());
        let result = run(Some("TASK-1"));
        std::env::remove_var("SPECK_STORE");

        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_ok());
    }

    #[test]
    fn format_signal_returns_correct_strings() {
        assert_eq!(format_signal(&SignalType::Clear), "clear");
        assert_eq!(format_signal(&SignalType::Fuzzy), "fuzzy");
        assert_eq!(format_signal(&SignalType::InternalLogic), "internal_logic");
    }
}
