//! `speck status` command.

use std::path::PathBuf;

use crate::context::ServiceContext;
use crate::spec::SignalType;
use crate::store::SpecStore;

/// Execute the `status` command.
///
/// Displays a table of all task specs showing ID, title, signal type,
/// and verification strategy.
///
/// # Errors
///
/// Returns an error string if spec listing or loading fails.
pub fn run() -> Result<(), String> {
    let ctx = ServiceContext::live();
    let store_root = store_root();
    let store = SpecStore::new(&ctx, &store_root);

    let mut ids = store.list_task_specs()?;
    if ids.is_empty() {
        println!("No specs found in store.");
        return Ok(());
    }
    ids.sort();

    // Collect rows for column-width calculation.
    let mut rows: Vec<(String, String, String, String)> = Vec::new();
    for id in &ids {
        let spec = store.load_task_spec(id)?;
        let signal = match spec.signal_type {
            SignalType::Clear => "clear",
            SignalType::Fuzzy => "fuzzy",
            SignalType::InternalLogic => "internal_logic",
        };
        let strategy = match &spec.verification {
            crate::spec::VerificationStrategy::DirectAssertion { .. } => "direct_assertion",
            crate::spec::VerificationStrategy::RefactorToExpose { .. } => "refactor_to_expose",
            crate::spec::VerificationStrategy::TraceAssertion { .. } => "trace_assertion",
        };
        rows.push((spec.id.clone(), spec.title.clone(), signal.to_string(), strategy.to_string()));
    }

    // Calculate column widths.
    let id_width = rows.iter().map(|r| r.0.len()).max().unwrap_or(2).max(2);
    let title_width = rows.iter().map(|r| r.1.len()).max().unwrap_or(5).max(5);
    let signal_width = rows.iter().map(|r| r.2.len()).max().unwrap_or(6).max(6);
    let strategy_width = rows.iter().map(|r| r.3.len()).max().unwrap_or(8).max(8);

    // Print header.
    println!(
        "{:<id_width$}  {:<title_width$}  {:<signal_width$}  {:<strategy_width$}",
        "ID", "TITLE", "SIGNAL", "STRATEGY",
    );
    println!(
        "{:-<id_width$}  {:-<title_width$}  {:-<signal_width$}  {:-<strategy_width$}",
        "", "", "", "",
    );

    // Print rows.
    for (id, title, signal, strategy) in &rows {
        println!(
            "{id:<id_width$}  {title:<title_width$}  {signal:<signal_width$}  {strategy:<strategy_width$}",
        );
    }

    println!("\n{} spec(s) total.", rows.len());
    Ok(())
}

fn store_root() -> PathBuf {
    std::env::var("SPECK_STORE").map_or_else(|_| PathBuf::from(".speck"), PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_command_empty_store() {
        std::env::set_var("SPECK_STORE", "/tmp/speck_test_status_empty_nonexistent");
        let result = run();
        std::env::remove_var("SPECK_STORE");
        assert!(result.is_ok());
    }

    #[test]
    fn status_command_with_specs() {
        use crate::spec::{TaskSpec, VerificationCheck, VerificationStrategy};

        let dir = std::env::temp_dir().join("speck_cli_status_with_specs");
        let tasks_dir = dir.join("tasks");
        std::fs::create_dir_all(&tasks_dir).unwrap();

        let spec1 = TaskSpec {
            id: "TASK-1".to_string(),
            title: "First task".to_string(),
            requirement: None,
            context: None,
            acceptance_criteria: vec!["works".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::TestSuite {
                    command: "cargo test".to_string(),
                    expected: "pass".to_string(),
                }],
            },
        };
        let spec2 = TaskSpec {
            id: "TASK-2".to_string(),
            title: "Second task".to_string(),
            requirement: None,
            context: None,
            acceptance_criteria: vec!["also works".to_string()],
            signal_type: SignalType::Fuzzy,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::Custom { description: "manual check".to_string() }],
            },
        };

        std::fs::write(tasks_dir.join("TASK-1.yaml"), serde_yaml::to_string(&spec1).unwrap())
            .unwrap();
        std::fs::write(tasks_dir.join("TASK-2.yaml"), serde_yaml::to_string(&spec2).unwrap())
            .unwrap();

        std::env::set_var("SPECK_STORE", dir.to_str().unwrap());
        let result = run();
        std::env::remove_var("SPECK_STORE");

        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_ok());
    }
}
