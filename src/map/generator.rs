//! Map generation logic: walks project files via `ServiceContext` ports.

use std::path::Path;

use crate::context::ServiceContext;
use crate::map::{CodebaseMap, ModuleSummary};

/// Output path for the generated codebase map (relative to project root).
const MAP_OUTPUT_PATH: &str = ".spec-cache/codebase_map.yaml";

/// Generates a [`CodebaseMap`] for the project rooted at `root`.
///
/// Walks the file tree via `ctx.git.list_files`, identifies module boundaries
/// (directories containing `mod.rs` or `lib.rs`), extracts public items from
/// Rust source files, and writes the result as YAML.
///
/// # Errors
///
/// Returns an error if git queries, file reads, or YAML serialization fail.
pub fn generate(ctx: &ServiceContext, root: &Path) -> Result<CodebaseMap, String> {
    let commit_hash =
        ctx.git.current_commit().map_err(|e| format!("failed to get current commit: {e}"))?;

    let generated_at = ctx.clock.now();

    let files = ctx.git.list_files(root).map_err(|e| format!("failed to list files: {e}"))?;

    let directory_tree: Vec<String> = files.clone();

    // Identify test infrastructure files.
    let test_infrastructure: Vec<String> =
        files.iter().filter(|f| is_test_file(f)).cloned().collect();

    // Identify module boundaries: directories containing mod.rs or lib.rs.
    let module_roots = find_module_roots(&files);

    // Build module summaries.
    let mut modules = Vec::new();
    for module_path in &module_roots {
        modules.push(build_module_summary(ctx, root, module_path, &files));
    }

    let map =
        CodebaseMap { commit_hash, generated_at, modules, directory_tree, test_infrastructure };

    // Serialize and write to .spec-cache/codebase_map.yaml.
    let yaml = serde_yaml::to_string(&map).map_err(|e| format!("failed to serialize map: {e}"))?;
    let output = root.join(MAP_OUTPUT_PATH);
    ctx.fs
        .write(&output, &yaml)
        .map_err(|e| format!("failed to write map to {}: {e}", output.display()))?;

    Ok(map)
}

/// Returns `true` if the file path looks like a test file.
fn is_test_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.starts_with("test_")
        || name.ends_with("_test.rs")
        || name.ends_with("_tests.rs")
        || path.contains("/tests/")
        || path.contains("/test/")
        || path.starts_with("tests/")
}

/// Finds directories that contain `mod.rs` or `lib.rs`, indicating module boundaries.
fn find_module_roots(files: &[String]) -> Vec<String> {
    let mut roots = Vec::new();
    for file in files {
        let name = file.rsplit('/').next().unwrap_or(file);
        if name == "mod.rs" || name == "lib.rs" {
            if let Some(dir) = file.rsplit_once('/') {
                if !roots.contains(&dir.0.to_string()) {
                    roots.push(dir.0.to_string());
                }
            }
        }
    }
    roots.sort();
    roots
}

/// Builds a [`ModuleSummary`] by reading Rust source files in the module directory.
fn build_module_summary(
    ctx: &ServiceContext,
    root: &Path,
    module_path: &str,
    all_files: &[String],
) -> ModuleSummary {
    let prefix = format!("{module_path}/");
    let module_files: Vec<&String> = all_files
        .iter()
        .filter(|f| {
            f.starts_with(&prefix)
                && Path::new(f).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
                && !f[prefix.len()..].contains('/')
        })
        .collect();

    let mut public_items = Vec::new();
    let mut dependencies = Vec::new();

    for file in &module_files {
        let full_path = root.join(file);
        let Ok(content) = ctx.fs.read_to_string(&full_path) else {
            continue;
        };
        extract_public_items(&content, &mut public_items);
        extract_dependencies(&content, &mut dependencies);
    }

    public_items.sort();
    public_items.dedup();
    dependencies.sort();
    dependencies.dedup();

    ModuleSummary { path: module_path.to_string(), public_items, dependencies }
}

/// Extracts `pub fn`, `pub struct`, and `pub trait` names from Rust source.
fn extract_public_items(content: &str, items: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("pub fn ") {
            if let Some(name) = rest.split('(').next() {
                items.push(format!("fn {name}"));
            }
        } else if let Some(rest) = trimmed.strip_prefix("pub struct ") {
            if let Some(name) = rest.split([' ', '{', '(', '<']).next() {
                items.push(format!("struct {name}"));
            }
        } else if let Some(rest) = trimmed.strip_prefix("pub trait ") {
            if let Some(name) = rest.split([' ', '{', '<', ':']).next() {
                items.push(format!("trait {name}"));
            }
        }
    }
}

/// Extracts `use crate::` dependency paths from Rust source.
fn extract_dependencies(content: &str, deps: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("use crate::") {
            if let Some(module) = rest.split("::").next() {
                let module = module.trim_end_matches(';');
                deps.push(module.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
    use chrono::Utc;
    use serde_json::json;

    fn make_cassette() -> Cassette {
        // Simulate a small project:
        //   src/lib.rs        — module root with pub struct App
        //   src/map/mod.rs    — module root with pub fn generate
        //   src/map/utils.rs  — helper (not a module root)
        //   tests/integration_test.rs — test infrastructure
        Cassette {
            name: "map-gen-test".into(),
            recorded_at: Utc::now(),
            commit: "abc123def".into(),
            interactions: vec![
                // clock.now
                Interaction {
                    seq: 0,
                    port: "clock".into(),
                    method: "now".into(),
                    input: json!({}),
                    output: json!("2025-06-15T10:00:00Z"),
                },
                // git.current_commit
                Interaction {
                    seq: 1,
                    port: "git".into(),
                    method: "current_commit".into(),
                    input: json!({}),
                    output: json!("abc123def"),
                },
                // git.list_files
                Interaction {
                    seq: 2,
                    port: "git".into(),
                    method: "list_files".into(),
                    input: json!({"path": "/project"}),
                    output: json!([
                        "src/lib.rs",
                        "src/map/mod.rs",
                        "src/map/utils.rs",
                        "tests/integration_test.rs"
                    ]),
                },
                // fs.read_to_string — src/lib.rs
                Interaction {
                    seq: 3,
                    port: "fs".into(),
                    method: "read_to_string".into(),
                    input: json!({"path": "/project/src/lib.rs"}),
                    output: json!(
                        "pub struct App {\n    pub name: String,\n}\n\npub fn run() {}\n"
                    ),
                },
                // fs.read_to_string — src/map/mod.rs
                Interaction {
                    seq: 4,
                    port: "fs".into(),
                    method: "read_to_string".into(),
                    input: json!({"path": "/project/src/map/mod.rs"}),
                    output: json!(
                        "use crate::context;\n\npub fn generate() {}\npub trait Generator {}\n"
                    ),
                },
                // fs.read_to_string — src/map/utils.rs
                Interaction {
                    seq: 5,
                    port: "fs".into(),
                    method: "read_to_string".into(),
                    input: json!({"path": "/project/src/map/utils.rs"}),
                    output: json!("fn helper() {}\n"),
                },
                // fs.write — .spec-cache/codebase_map.yaml
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
    fn generate_map_from_cassette() {
        let cassette = make_cassette();
        let yaml = serde_yaml::to_string(&cassette).unwrap();
        let dir = std::env::temp_dir().join("speck_map_gen_test");
        std::fs::create_dir_all(&dir).unwrap();
        let cassette_path = dir.join("map_gen.cassette.yaml");
        std::fs::write(&cassette_path, &yaml).unwrap();

        let ctx = ServiceContext::replaying(&cassette_path).unwrap();
        let map = generate(&ctx, Path::new("/project")).unwrap();

        assert_eq!(map.commit_hash, "abc123def");
        assert_eq!(map.directory_tree.len(), 4);
        assert!(map.test_infrastructure.contains(&"tests/integration_test.rs".to_string()));

        // Two module roots: src (has lib.rs) and src/map (has mod.rs)
        assert_eq!(map.modules.len(), 2);

        let src_module = map.modules.iter().find(|m| m.path == "src").unwrap();
        assert!(src_module.public_items.contains(&"fn run".to_string()));
        assert!(src_module.public_items.contains(&"struct App".to_string()));

        let map_module = map.modules.iter().find(|m| m.path == "src/map").unwrap();
        assert!(map_module.public_items.contains(&"fn generate".to_string()));
        assert!(map_module.public_items.contains(&"trait Generator".to_string()));
        assert!(map_module.dependencies.contains(&"context".to_string()));

        // Verify YAML serialization works
        let map_yaml = serde_yaml::to_string(&map).unwrap();
        let deserialized: crate::map::CodebaseMap = serde_yaml::from_str(&map_yaml).unwrap();
        assert_eq!(map, deserialized);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_test_file_detects_test_patterns() {
        assert!(is_test_file("tests/integration.rs"));
        assert!(is_test_file("src/foo_test.rs"));
        assert!(is_test_file("src/foo_tests.rs"));
        assert!(is_test_file("test_helper.rs"));
        assert!(!is_test_file("src/main.rs"));
        assert!(!is_test_file("src/map/mod.rs"));
    }

    #[test]
    fn find_module_roots_identifies_boundaries() {
        let files = vec![
            "src/lib.rs".to_string(),
            "src/map/mod.rs".to_string(),
            "src/map/generator.rs".to_string(),
            "src/cli.rs".to_string(),
        ];
        let roots = find_module_roots(&files);
        assert_eq!(roots, vec!["src", "src/map"]);
    }

    #[test]
    fn extract_public_items_finds_pub_declarations() {
        let code = r"
pub fn hello() {}
fn private() {}
pub struct Foo {
    name: String,
}
pub trait Bar {}
struct Hidden;
";
        let mut items = Vec::new();
        extract_public_items(code, &mut items);
        assert_eq!(items, vec!["fn hello", "struct Foo", "trait Bar"]);
    }

    #[test]
    fn extract_dependencies_finds_crate_uses() {
        let code = r"
use crate::context;
use crate::ports::filesystem;
use std::path::Path;
";
        let mut deps = Vec::new();
        extract_dependencies(code, &mut deps);
        assert_eq!(deps, vec!["context", "ports"]);
    }
}
