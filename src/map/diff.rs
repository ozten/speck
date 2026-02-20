//! Diffing logic for codebase maps.

use crate::map::{CodebaseMap, ModuleSummary};

/// Differences between two codebase maps.
#[derive(Debug, PartialEq)]
pub struct MapDiff {
    /// Modules present in new but not old.
    pub added_modules: Vec<String>,
    /// Modules present in old but not new.
    pub removed_modules: Vec<String>,
    /// Modules whose public items or dependencies changed.
    pub changed_modules: Vec<ModuleChange>,
}

/// Describes changes within a single module.
#[derive(Debug, PartialEq)]
pub struct ModuleChange {
    /// Path of the module.
    pub path: String,
    /// Public items added.
    pub added_items: Vec<String>,
    /// Public items removed.
    pub removed_items: Vec<String>,
    /// Dependencies added.
    pub added_deps: Vec<String>,
    /// Dependencies removed.
    pub removed_deps: Vec<String>,
}

/// Compute differences between an old and new codebase map.
#[must_use]
pub fn diff_maps(old: &CodebaseMap, new: &CodebaseMap) -> MapDiff {
    let old_paths: Vec<&str> = old.modules.iter().map(|m| m.path.as_str()).collect();
    let new_paths: Vec<&str> = new.modules.iter().map(|m| m.path.as_str()).collect();

    let added_modules: Vec<String> = new_paths
        .iter()
        .filter(|p| !old_paths.contains(p))
        .map(std::string::ToString::to_string)
        .collect();

    let removed_modules: Vec<String> = old_paths
        .iter()
        .filter(|p| !new_paths.contains(p))
        .map(std::string::ToString::to_string)
        .collect();

    let mut changed_modules = Vec::new();
    for new_mod in &new.modules {
        if let Some(old_mod) = old.modules.iter().find(|m| m.path == new_mod.path) {
            if let Some(change) = diff_module(old_mod, new_mod) {
                changed_modules.push(change);
            }
        }
    }

    MapDiff { added_modules, removed_modules, changed_modules }
}

/// Compare two module summaries, returning `Some(change)` if they differ.
fn diff_module(old: &ModuleSummary, new: &ModuleSummary) -> Option<ModuleChange> {
    let added_items: Vec<String> =
        new.public_items.iter().filter(|i| !old.public_items.contains(i)).cloned().collect();
    let removed_items: Vec<String> =
        old.public_items.iter().filter(|i| !new.public_items.contains(i)).cloned().collect();
    let added_deps: Vec<String> =
        new.dependencies.iter().filter(|d| !old.dependencies.contains(d)).cloned().collect();
    let removed_deps: Vec<String> =
        old.dependencies.iter().filter(|d| !new.dependencies.contains(d)).cloned().collect();

    if added_items.is_empty()
        && removed_items.is_empty()
        && added_deps.is_empty()
        && removed_deps.is_empty()
    {
        return None;
    }

    Some(ModuleChange {
        path: new.path.clone(),
        added_items,
        removed_items,
        added_deps,
        removed_deps,
    })
}

/// Format a `MapDiff` for human-readable display.
#[must_use]
pub fn format_diff(diff: &MapDiff) -> String {
    if diff.added_modules.is_empty()
        && diff.removed_modules.is_empty()
        && diff.changed_modules.is_empty()
    {
        return "No changes since last map.".to_string();
    }

    let mut lines = Vec::new();

    if !diff.added_modules.is_empty() {
        lines.push("Added modules:".to_string());
        for m in &diff.added_modules {
            lines.push(format!("  + {m}"));
        }
    }
    if !diff.removed_modules.is_empty() {
        lines.push("Removed modules:".to_string());
        for m in &diff.removed_modules {
            lines.push(format!("  - {m}"));
        }
    }
    for change in &diff.changed_modules {
        lines.push(format!("Changed: {}", change.path));
        for item in &change.added_items {
            lines.push(format!("  + {item}"));
        }
        for item in &change.removed_items {
            lines.push(format!("  - {item}"));
        }
        for dep in &change.added_deps {
            lines.push(format!("  +dep {dep}"));
        }
        for dep in &change.removed_deps {
            lines.push(format!("  -dep {dep}"));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_map(modules: Vec<ModuleSummary>) -> CodebaseMap {
        CodebaseMap {
            commit_hash: "abc123".to_string(),
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

    #[test]
    fn diff_detects_added_module() {
        let old = make_map(vec![make_module("src", vec!["fn run"], vec![])]);
        let new = make_map(vec![
            make_module("src", vec!["fn run"], vec![]),
            make_module("src/map", vec!["fn generate"], vec!["context"]),
        ]);
        let d = diff_maps(&old, &new);
        assert_eq!(d.added_modules, vec!["src/map"]);
        assert!(d.removed_modules.is_empty());
        assert!(d.changed_modules.is_empty());
    }

    #[test]
    fn diff_detects_removed_module() {
        let old = make_map(vec![
            make_module("src", vec!["fn run"], vec![]),
            make_module("src/old", vec!["fn legacy"], vec![]),
        ]);
        let new = make_map(vec![make_module("src", vec!["fn run"], vec![])]);
        let d = diff_maps(&old, &new);
        assert!(d.added_modules.is_empty());
        assert_eq!(d.removed_modules, vec!["src/old"]);
    }

    #[test]
    fn diff_detects_changed_items() {
        let old = make_map(vec![make_module("src", vec!["fn run", "struct App"], vec!["config"])]);
        let new =
            make_map(vec![make_module("src", vec!["fn run", "fn new_fn"], vec!["config", "map"])]);
        let d = diff_maps(&old, &new);
        assert!(d.added_modules.is_empty());
        assert!(d.removed_modules.is_empty());
        assert_eq!(d.changed_modules.len(), 1);
        let c = &d.changed_modules[0];
        assert_eq!(c.path, "src");
        assert_eq!(c.added_items, vec!["fn new_fn"]);
        assert_eq!(c.removed_items, vec!["struct App"]);
        assert_eq!(c.added_deps, vec!["map"]);
        assert!(c.removed_deps.is_empty());
    }

    #[test]
    fn diff_no_changes() {
        let m = make_map(vec![make_module("src", vec!["fn run"], vec!["config"])]);
        let d = diff_maps(&m, &m);
        assert!(d.added_modules.is_empty());
        assert!(d.removed_modules.is_empty());
        assert!(d.changed_modules.is_empty());
    }

    #[test]
    fn format_diff_no_changes() {
        let d = MapDiff { added_modules: vec![], removed_modules: vec![], changed_modules: vec![] };
        assert_eq!(format_diff(&d), "No changes since last map.");
    }

    #[test]
    fn format_diff_with_changes() {
        let d = MapDiff {
            added_modules: vec!["src/new".to_string()],
            removed_modules: vec!["src/old".to_string()],
            changed_modules: vec![ModuleChange {
                path: "src".to_string(),
                added_items: vec!["fn foo".to_string()],
                removed_items: vec![],
                added_deps: vec![],
                removed_deps: vec!["legacy".to_string()],
            }],
        };
        let output = format_diff(&d);
        assert!(output.contains("+ src/new"));
        assert!(output.contains("- src/old"));
        assert!(output.contains("+ fn foo"));
        assert!(output.contains("-dep legacy"));
    }
}
