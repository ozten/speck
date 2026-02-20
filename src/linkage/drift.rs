//! Drift detection: identifies specs whose referenced modules have changed.

use crate::map::CodebaseMap;
use crate::spec::TaskSpec;

use super::resolve::{resolve, LinkageResult};

/// A single spec's drift information.
#[derive(Debug, Clone, PartialEq)]
pub struct DriftEntry {
    /// The spec ID.
    pub spec_id: String,
    /// Module paths that changed between old and new maps.
    pub changed_modules: Vec<String>,
    /// Module paths that were removed from the codebase.
    pub removed_modules: Vec<String>,
    /// Whether re-planning is recommended (true when modules were removed or
    /// multiple modules changed).
    pub replan_recommended: bool,
}

/// Aggregated drift report across multiple specs.
#[derive(Debug, Clone, PartialEq)]
pub struct DriftReport {
    /// Per-spec drift entries (only includes specs with drift).
    pub entries: Vec<DriftEntry>,
    /// The old commit hash.
    pub old_commit: String,
    /// The new commit hash.
    pub new_commit: String,
}

impl DriftReport {
    /// Returns `true` if no specs have drift.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of specs affected by drift.
    #[must_use]
    pub fn affected_count(&self) -> usize {
        self.entries.len()
    }
}

/// Detects drift for a set of specs between two codebase map snapshots.
///
/// For each spec, resolves module references against the old map, then checks
/// whether those modules still exist and are unchanged in the new map.
/// A module is considered "changed" if it exists in both maps but its public
/// items or dependencies differ. A module is "removed" if it no longer appears.
#[must_use]
pub fn detect_drift(
    specs: &[TaskSpec],
    old_map: &CodebaseMap,
    new_map: &CodebaseMap,
) -> DriftReport {
    let entries: Vec<DriftEntry> = specs
        .iter()
        .filter_map(|spec| {
            let linkage = resolve(spec, old_map);
            check_spec_drift(&linkage, old_map, new_map)
        })
        .collect();

    DriftReport {
        entries,
        old_commit: old_map.commit_hash.clone(),
        new_commit: new_map.commit_hash.clone(),
    }
}

/// Checks a single spec for drift based on its resolved linkage.
fn check_spec_drift(
    linkage: &LinkageResult,
    old_map: &CodebaseMap,
    new_map: &CodebaseMap,
) -> Option<DriftEntry> {
    let mut changed_modules = Vec::new();
    let mut removed_modules = Vec::new();

    for link in &linkage.links {
        let Some(ref path) = link.resolved_path else {
            continue;
        };

        let old_module = old_map.modules.iter().find(|m| &m.path == path);
        let new_module = new_map.modules.iter().find(|m| &m.path == path);

        match (old_module, new_module) {
            (Some(_old), None) => {
                removed_modules.push(path.clone());
            }
            (Some(old), Some(new)) => {
                if old.public_items != new.public_items || old.dependencies != new.dependencies {
                    changed_modules.push(path.clone());
                }
            }
            _ => {}
        }
    }

    if changed_modules.is_empty() && removed_modules.is_empty() {
        return None;
    }

    let replan_recommended = !removed_modules.is_empty() || changed_modules.len() > 1;

    Some(DriftEntry {
        spec_id: linkage.spec_id.clone(),
        changed_modules,
        removed_modules,
        replan_recommended,
    })
}

/// Formats a drift report as a human-readable string.
#[must_use]
pub fn format_drift_report(report: &DriftReport) -> String {
    if report.is_clean() {
        return format!(
            "No drift detected between {} and {}.",
            report.old_commit, report.new_commit
        );
    }

    let mut lines = Vec::new();
    lines.push(format!("Drift detected ({} -> {}):", report.old_commit, report.new_commit));
    lines.push(String::new());

    for entry in &report.entries {
        lines.push(format!("  Spec: {}", entry.spec_id));
        for path in &entry.changed_modules {
            lines.push(format!("    [CHANGED] {path}"));
        }
        for path in &entry.removed_modules {
            lines.push(format!("    [REMOVED] {path}"));
        }
        if entry.replan_recommended {
            lines.push("    -> Re-planning recommended".to_string());
        }
        lines.push(String::new());
    }

    let total = report.affected_count();
    lines.push(format!("{total} spec{} affected by drift.", if total == 1 { "" } else { "s" }));

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{CodebaseMap, ModuleSummary};
    use crate::spec::{SignalType, TaskContext, TaskSpec, VerificationCheck, VerificationStrategy};
    use chrono::Utc;

    fn make_map(commit: &str, modules: Vec<ModuleSummary>) -> CodebaseMap {
        CodebaseMap {
            commit_hash: commit.to_string(),
            generated_at: Utc::now(),
            modules,
            directory_tree: vec![],
            test_infrastructure: vec![],
        }
    }

    fn make_module(path: &str, items: Vec<&str>, deps: Vec<&str>) -> ModuleSummary {
        ModuleSummary {
            path: path.to_string(),
            public_items: items.into_iter().map(String::from).collect(),
            dependencies: deps.into_iter().map(String::from).collect(),
        }
    }

    fn make_spec(id: &str, modules: Vec<&str>) -> TaskSpec {
        TaskSpec {
            id: id.to_string(),
            title: format!("Task {id}"),
            requirement: None,
            context: Some(TaskContext {
                modules: modules.into_iter().map(String::from).collect(),
                patterns: None,
                dependencies: vec![],
            }),
            acceptance_criteria: vec!["done".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::Custom { description: "check".to_string() }],
            },
        }
    }

    #[test]
    fn no_drift_when_maps_identical() {
        let modules = vec![make_module("src/service.rs", vec!["MyService"], vec![])];
        let old_map = make_map("aaa", modules.clone());
        let new_map = make_map("bbb", modules);
        let specs = vec![make_spec("T-1", vec!["MyService"])];

        let report = detect_drift(&specs, &old_map, &new_map);
        assert!(report.is_clean());
    }

    #[test]
    fn detects_changed_public_items() {
        let old_map =
            make_map("aaa", vec![make_module("src/service.rs", vec!["MyService"], vec![])]);
        let new_map = make_map(
            "bbb",
            vec![make_module("src/service.rs", vec!["MyService", "NewHelper"], vec![])],
        );
        let specs = vec![make_spec("T-1", vec!["MyService"])];

        let report = detect_drift(&specs, &old_map, &new_map);
        assert!(!report.is_clean());
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].changed_modules, vec!["src/service.rs"]);
        assert!(!report.entries[0].replan_recommended);
    }

    #[test]
    fn detects_removed_module() {
        let old_map =
            make_map("aaa", vec![make_module("src/service.rs", vec!["MyService"], vec![])]);
        let new_map = make_map("bbb", vec![]);
        let specs = vec![make_spec("T-1", vec!["MyService"])];

        let report = detect_drift(&specs, &old_map, &new_map);
        assert!(!report.is_clean());
        assert_eq!(report.entries[0].removed_modules, vec!["src/service.rs"]);
        assert!(report.entries[0].replan_recommended);
    }

    #[test]
    fn replan_recommended_when_multiple_changes() {
        let old_map = make_map(
            "aaa",
            vec![
                make_module("src/a.rs", vec!["ServiceA"], vec![]),
                make_module("src/b.rs", vec!["ServiceB"], vec![]),
            ],
        );
        let new_map = make_map(
            "bbb",
            vec![
                make_module("src/a.rs", vec!["ServiceA", "Extra"], vec![]),
                make_module("src/b.rs", vec!["ServiceB"], vec!["new_dep"]),
            ],
        );
        let specs = vec![make_spec("T-1", vec!["ServiceA", "ServiceB"])];

        let report = detect_drift(&specs, &old_map, &new_map);
        assert_eq!(report.entries[0].changed_modules.len(), 2);
        assert!(report.entries[0].replan_recommended);
    }

    #[test]
    fn spec_without_modules_has_no_drift() {
        let old_map =
            make_map("aaa", vec![make_module("src/service.rs", vec!["MyService"], vec![])]);
        let new_map = make_map("bbb", vec![]);
        let spec = TaskSpec {
            id: "T-NONE".to_string(),
            title: "No context".to_string(),
            requirement: None,
            context: None,
            acceptance_criteria: vec!["done".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion { checks: vec![] },
        };

        let report = detect_drift(&[spec], &old_map, &new_map);
        assert!(report.is_clean());
    }

    #[test]
    fn multiple_specs_only_affected_included() {
        let old_map = make_map(
            "aaa",
            vec![
                make_module("src/a.rs", vec!["ServiceA"], vec![]),
                make_module("src/b.rs", vec!["ServiceB"], vec![]),
            ],
        );
        let new_map = make_map(
            "bbb",
            vec![
                make_module("src/a.rs", vec!["ServiceA", "Changed"], vec![]),
                make_module("src/b.rs", vec!["ServiceB"], vec![]),
            ],
        );
        let specs = vec![make_spec("T-1", vec!["ServiceA"]), make_spec("T-2", vec!["ServiceB"])];

        let report = detect_drift(&specs, &old_map, &new_map);
        assert_eq!(report.affected_count(), 1);
        assert_eq!(report.entries[0].spec_id, "T-1");
    }

    #[test]
    fn format_clean_report() {
        let report = DriftReport {
            entries: vec![],
            old_commit: "aaa".to_string(),
            new_commit: "bbb".to_string(),
        };
        let text = format_drift_report(&report);
        assert!(text.contains("No drift detected"));
    }

    #[test]
    fn format_report_with_entries() {
        let report = DriftReport {
            entries: vec![DriftEntry {
                spec_id: "T-1".to_string(),
                changed_modules: vec!["src/a.rs".to_string()],
                removed_modules: vec!["src/b.rs".to_string()],
                replan_recommended: true,
            }],
            old_commit: "aaa".to_string(),
            new_commit: "bbb".to_string(),
        };
        let text = format_drift_report(&report);
        assert!(text.contains("[CHANGED] src/a.rs"));
        assert!(text.contains("[REMOVED] src/b.rs"));
        assert!(text.contains("Re-planning recommended"));
        assert!(text.contains("1 spec affected"));
    }
}
