//! `speck map` command.

use std::env;
use std::path::Path;

use crate::context::ServiceContext;
use crate::map::diff;
use crate::map::generator;

/// Output path for the generated codebase map (relative to project root).
const MAP_OUTPUT_PATH: &str = ".spec-cache/codebase_map.yaml";

/// Execute the `map` command.
///
/// When `diff` is false, generates a new codebase map and writes it to
/// `.spec-cache/codebase_map.yaml`.
///
/// When `diff` is true, loads the previous map, generates a new one, and
/// displays the differences.
///
/// # Errors
///
/// Returns an error string if map generation or diffing fails.
pub fn run(show_diff: bool) -> Result<(), String> {
    let ctx = ServiceContext::live();
    let root = env::current_dir().map_err(|e| format!("failed to get current directory: {e}"))?;

    if show_diff {
        run_diff(&ctx, &root)
    } else {
        run_generate(&ctx, &root)
    }
}

/// Generate a new map and print a summary.
fn run_generate(ctx: &ServiceContext, root: &Path) -> Result<(), String> {
    let map = generator::generate(ctx, root)?;
    println!(
        "Map generated: {} modules, {} files, {} test files",
        map.modules.len(),
        map.directory_tree.len(),
        map.test_infrastructure.len(),
    );
    println!("Written to {MAP_OUTPUT_PATH}");
    Ok(())
}

/// Load the previous map, generate a new one, and display the diff.
fn run_diff(ctx: &ServiceContext, root: &Path) -> Result<(), String> {
    let map_path = root.join(MAP_OUTPUT_PATH);
    let old_yaml = ctx
        .fs
        .read_to_string(&map_path)
        .map_err(|e| format!("failed to read previous map at {}: {e}", map_path.display()))?;
    let old_map: crate::map::CodebaseMap = serde_yaml::from_str(&old_yaml)
        .map_err(|e| format!("failed to parse previous map: {e}"))?;

    let new_map = generator::generate(ctx, root)?;

    let d = diff::diff_maps(&old_map, &new_map);
    println!("{}", diff::format_diff(&d));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
    use chrono::Utc;
    use serde_json::json;

    /// Build a cassette that simulates a small project for map generation.
    fn make_generate_cassette() -> Cassette {
        Cassette {
            name: "cli-map-generate".into(),
            recorded_at: Utc::now(),
            commit: "abc123".into(),
            interactions: vec![
                Interaction {
                    seq: 0,
                    port: "clock".into(),
                    method: "now".into(),
                    input: json!({}),
                    output: json!("2025-06-15T10:00:00Z"),
                },
                Interaction {
                    seq: 1,
                    port: "git".into(),
                    method: "current_commit".into(),
                    input: json!({}),
                    output: json!("abc123"),
                },
                Interaction {
                    seq: 2,
                    port: "git".into(),
                    method: "list_files".into(),
                    input: json!({"path": "/project"}),
                    output: json!(["src/lib.rs", "src/map/mod.rs"]),
                },
                Interaction {
                    seq: 3,
                    port: "fs".into(),
                    method: "read_to_string".into(),
                    input: json!({"path": "/project/src/lib.rs"}),
                    output: json!("pub fn run() {}\n"),
                },
                Interaction {
                    seq: 4,
                    port: "fs".into(),
                    method: "read_to_string".into(),
                    input: json!({"path": "/project/src/map/mod.rs"}),
                    output: json!("pub fn generate() {}\n"),
                },
                Interaction {
                    seq: 5,
                    port: "fs".into(),
                    method: "write".into(),
                    input: json!({"path": "/project/.spec-cache/codebase_map.yaml"}),
                    output: json!(null),
                },
            ],
        }
    }

    #[test]
    fn cli_map_generate() {
        let cassette = make_generate_cassette();
        let yaml = serde_yaml::to_string(&cassette).unwrap();
        let dir = std::env::temp_dir().join("speck_cli_map_gen");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cli_map_gen.cassette.yaml");
        std::fs::write(&path, &yaml).unwrap();

        let ctx = ServiceContext::replaying(&path).unwrap();
        let result = run_generate(&ctx, std::path::Path::new("/project"));
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Build a cassette for diff mode: read previous map, then generate a new one.
    fn make_diff_cassette() -> Cassette {
        // The previous map has one module (src with fn run).
        // The new map adds src/map module.
        let old_map = crate::map::CodebaseMap {
            commit_hash: "old123".to_string(),
            generated_at: chrono::DateTime::parse_from_rfc3339("2025-06-14T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            modules: vec![crate::map::ModuleSummary {
                path: "src".to_string(),
                public_items: vec!["fn run".to_string()],
                dependencies: vec![],
            }],
            directory_tree: vec!["src/lib.rs".to_string()],
            test_infrastructure: vec![],
        };
        let old_yaml = serde_yaml::to_string(&old_map).unwrap();

        Cassette {
            name: "cli-map-diff".into(),
            recorded_at: Utc::now(),
            commit: "new456".into(),
            interactions: vec![
                // fs.read_to_string — read previous map
                Interaction {
                    seq: 0,
                    port: "fs".into(),
                    method: "read_to_string".into(),
                    input: json!({"path": "/project/.spec-cache/codebase_map.yaml"}),
                    output: json!(old_yaml),
                },
                // clock.now
                Interaction {
                    seq: 1,
                    port: "clock".into(),
                    method: "now".into(),
                    input: json!({}),
                    output: json!("2025-06-15T10:00:00Z"),
                },
                // git.current_commit
                Interaction {
                    seq: 2,
                    port: "git".into(),
                    method: "current_commit".into(),
                    input: json!({}),
                    output: json!("new456"),
                },
                // git.list_files
                Interaction {
                    seq: 3,
                    port: "git".into(),
                    method: "list_files".into(),
                    input: json!({"path": "/project"}),
                    output: json!(["src/lib.rs", "src/map/mod.rs"]),
                },
                // fs.read_to_string — src/lib.rs
                Interaction {
                    seq: 4,
                    port: "fs".into(),
                    method: "read_to_string".into(),
                    input: json!({"path": "/project/src/lib.rs"}),
                    output: json!("pub fn run() {}\n"),
                },
                // fs.read_to_string — src/map/mod.rs
                Interaction {
                    seq: 5,
                    port: "fs".into(),
                    method: "read_to_string".into(),
                    input: json!({"path": "/project/src/map/mod.rs"}),
                    output: json!("pub fn generate() {}\n"),
                },
                // fs.write — new map
                Interaction {
                    seq: 6,
                    port: "fs".into(),
                    method: "write".into(),
                    input: json!({"path": "/project/.spec-cache/codebase_map.yaml"}),
                    output: json!(null),
                },
            ],
        }
    }

    #[test]
    fn cli_map_diff() {
        let cassette = make_diff_cassette();
        let yaml = serde_yaml::to_string(&cassette).unwrap();
        let dir = std::env::temp_dir().join("speck_cli_map_diff");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cli_map_diff.cassette.yaml");
        std::fs::write(&path, &yaml).unwrap();

        let ctx = ServiceContext::replaying(&path).unwrap();
        let result = run_diff(&ctx, std::path::Path::new("/project"));
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
