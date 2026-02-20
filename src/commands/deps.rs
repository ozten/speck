//! `speck deps` command.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::context::ServiceContext;
use crate::store::SpecStore;

/// Execute the `deps` command.
///
/// Displays the dependency graph for all task specs. Each task shows
/// which other tasks it depends on and which tasks depend on it.
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

    // Build dependency maps: depends_on[id] = Vec<dep_ids>, depended_by[id] = Vec<dependent_ids>.
    let mut depends_on: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut depended_by: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut titles: BTreeMap<String, String> = BTreeMap::new();

    for id in &ids {
        let spec = store.load_task_spec(id)?;
        titles.insert(spec.id.clone(), spec.title.clone());

        let deps = spec.context.as_ref().map(|c| c.dependencies.clone()).unwrap_or_default();
        for dep in &deps {
            depended_by.entry(dep.clone()).or_default().push(spec.id.clone());
        }
        depends_on.insert(spec.id.clone(), deps);
    }

    // Find roots (no dependencies).
    let roots: Vec<&String> =
        ids.iter().filter(|id| depends_on.get(*id).is_none_or(std::vec::Vec::is_empty)).collect();

    if roots.len() == ids.len() {
        println!("No dependencies found among {} spec(s).", ids.len());
        println!("\nAll specs are independent:");
        for id in &ids {
            let title = titles.get(id).map_or("", |t| t.as_str());
            println!("  {id} — {title}");
        }
        return Ok(());
    }

    println!("Dependency Graph:");
    println!();

    for id in &ids {
        let title = titles.get(id).map_or("", |t| t.as_str());
        let deps = depends_on.get(id).cloned().unwrap_or_default();
        let dependents = depended_by.get(id).cloned().unwrap_or_default();

        println!("{id} — {title}");
        if deps.is_empty() {
            println!("  depends on: (none)");
        } else {
            println!("  depends on: {}", deps.join(", "));
        }
        if dependents.is_empty() {
            println!("  blocks: (none)");
        } else {
            println!("  blocks: {}", dependents.join(", "));
        }
        println!();
    }

    // Print topological summary.
    if !roots.is_empty() {
        println!(
            "Roots (no dependencies): {}",
            roots.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        );
    }
    let leaves: Vec<&String> =
        ids.iter().filter(|id| depended_by.get(*id).is_none_or(std::vec::Vec::is_empty)).collect();
    if !leaves.is_empty() {
        println!(
            "Leaves (nothing depends on them): {}",
            leaves.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        );
    }

    Ok(())
}

fn store_root() -> PathBuf {
    std::env::var("SPECK_STORE").map_or_else(|_| PathBuf::from(".speck"), PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deps_command_empty_store() {
        std::env::set_var("SPECK_STORE", "/tmp/speck_test_deps_empty_nonexistent");
        let result = run();
        std::env::remove_var("SPECK_STORE");
        assert!(result.is_ok());
    }

    #[test]
    fn deps_command_with_independent_specs() {
        use crate::spec::{SignalType, TaskSpec, VerificationCheck, VerificationStrategy};

        let dir = std::env::temp_dir().join("speck_cli_deps_independent");
        let tasks_dir = dir.join("tasks");
        std::fs::create_dir_all(&tasks_dir).unwrap();

        let spec = TaskSpec {
            id: "TASK-1".to_string(),
            title: "Standalone".to_string(),
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

        std::fs::write(tasks_dir.join("TASK-1.yaml"), serde_yaml::to_string(&spec).unwrap())
            .unwrap();

        std::env::set_var("SPECK_STORE", dir.to_str().unwrap());
        let result = run();
        std::env::remove_var("SPECK_STORE");

        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_ok());
    }

    #[test]
    fn deps_command_with_dependency_graph() {
        use crate::spec::{
            SignalType, TaskContext, TaskSpec, VerificationCheck, VerificationStrategy,
        };

        let dir = std::env::temp_dir().join("speck_cli_deps_graph");
        let tasks_dir = dir.join("tasks");
        std::fs::create_dir_all(&tasks_dir).unwrap();

        let spec1 = TaskSpec {
            id: "TASK-A".to_string(),
            title: "Base task".to_string(),
            requirement: None,
            context: None,
            acceptance_criteria: vec!["done".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::TestSuite {
                    command: "cargo test".to_string(),
                    expected: "pass".to_string(),
                }],
            },
        };
        let spec2 = TaskSpec {
            id: "TASK-B".to_string(),
            title: "Dependent task".to_string(),
            requirement: None,
            context: Some(TaskContext {
                modules: vec![],
                patterns: None,
                dependencies: vec!["TASK-A".to_string()],
            }),
            acceptance_criteria: vec!["done".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::TestSuite {
                    command: "cargo test".to_string(),
                    expected: "pass".to_string(),
                }],
            },
        };

        std::fs::write(tasks_dir.join("TASK-A.yaml"), serde_yaml::to_string(&spec1).unwrap())
            .unwrap();
        std::fs::write(tasks_dir.join("TASK-B.yaml"), serde_yaml::to_string(&spec2).unwrap())
            .unwrap();

        std::env::set_var("SPECK_STORE", dir.to_str().unwrap());
        let result = run();
        std::env::remove_var("SPECK_STORE");

        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_ok());
    }
}
