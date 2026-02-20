//! Splits a monolithic cassette YAML file into per-port cassette files.
//!
//! Usage: `cassette_split <input.yaml> <output_dir>`

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::{env, fs, process};

use chrono::Utc;
use speck::cassette::format::{Cassette, Interaction};

/// A per-port cassette that links back to the original recording session.
#[derive(serde::Serialize)]
struct PerPortCassette {
    name: String,
    recorded_at: chrono::DateTime<Utc>,
    commit: String,
    source_session: String,
    interactions: Vec<Interaction>,
}

fn split_cassette(input: &str, output_dir: &str) -> Result<(), String> {
    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output_dir);

    let content = fs::read_to_string(&input_path)
        .map_err(|e| format!("Failed to read {}: {e}", input_path.display()))?;
    let cassette: Cassette = serde_yaml::from_str(&content)
        .map_err(|e| format!("Failed to parse {}: {e}", input_path.display()))?;

    // Group interactions by port
    let mut by_port: BTreeMap<String, Vec<Interaction>> = BTreeMap::new();
    for interaction in &cassette.interactions {
        by_port.entry(interaction.port.clone()).or_default().push(interaction.clone());
    }

    // Write per-port cassette files (skip ports with zero interactions)
    for (port_name, interactions) in &by_port {
        if interactions.is_empty() {
            continue;
        }

        // Renumber sequences starting from 0
        let renumbered: Vec<Interaction> = interactions
            .iter()
            .enumerate()
            .map(|(i, orig)| Interaction {
                seq: i as u64,
                port: orig.port.clone(),
                method: orig.method.clone(),
                input: orig.input.clone(),
                output: orig.output.clone(),
            })
            .collect();

        let per_port = PerPortCassette {
            name: format!("{}-{}", cassette.name, port_name),
            recorded_at: cassette.recorded_at,
            commit: cassette.commit.clone(),
            source_session: cassette.name.clone(),
            interactions: renumbered,
        };

        let port_dir = output_path.join(port_name);
        fs::create_dir_all(&port_dir)
            .map_err(|e| format!("Failed to create {}: {e}", port_dir.display()))?;

        let file_path = port_dir.join(format!("{}.yaml", cassette.name));
        let yaml = serde_yaml::to_string(&per_port)
            .map_err(|e| format!("Failed to serialize cassette for port {port_name}: {e}"))?;
        fs::write(&file_path, yaml)
            .map_err(|e| format!("Failed to write {}: {e}", file_path.display()))?;

        println!("Wrote {}", file_path.display());
    }

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: cassette_split <input.yaml> <output_dir>");
        process::exit(1);
    }

    if let Err(e) = split_cassette(&args[1], &args[2]) {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write_monolithic_fixture(path: &std::path::Path) {
        let cassette = Cassette {
            name: "test-session".into(),
            recorded_at: chrono::Utc::now(),
            commit: "abc123".into(),
            interactions: vec![
                Interaction {
                    seq: 0,
                    port: "llm".into(),
                    method: "complete".into(),
                    input: json!({"prompt": "hello"}),
                    output: json!({"text": "world"}),
                },
                Interaction {
                    seq: 1,
                    port: "fs".into(),
                    method: "read".into(),
                    input: json!({"path": "/tmp/test"}),
                    output: json!({"content": "data"}),
                },
                Interaction {
                    seq: 2,
                    port: "llm".into(),
                    method: "complete".into(),
                    input: json!({"prompt": "second"}),
                    output: json!({"text": "response"}),
                },
                Interaction {
                    seq: 3,
                    port: "git".into(),
                    method: "status".into(),
                    input: json!({}),
                    output: json!({"clean": true}),
                },
            ],
        };
        let yaml = serde_yaml::to_string(&cassette).unwrap();
        std::fs::write(path, yaml).unwrap();
    }

    #[test]
    fn split_creates_per_port_files_with_correct_contents() {
        let dir = std::env::temp_dir().join("speck_cassette_split_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let input = dir.join("monolithic.yaml");
        let output = dir.join("split_output");

        write_monolithic_fixture(&input);
        split_cassette(input.to_str().unwrap(), output.to_str().unwrap()).unwrap();

        // Check llm port cassette
        let llm_path = output.join("llm").join("test-session.yaml");
        assert!(llm_path.exists(), "llm cassette should exist");
        let llm_content: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(&llm_path).unwrap()).unwrap();
        let llm_interactions = llm_content["interactions"].as_sequence().unwrap();
        assert_eq!(llm_interactions.len(), 2, "llm should have 2 interactions");
        // Verify renumbering: seq should be 0, 1
        assert_eq!(llm_interactions[0]["seq"].as_u64().unwrap(), 0);
        assert_eq!(llm_interactions[1]["seq"].as_u64().unwrap(), 1);
        // Verify source_session
        assert_eq!(llm_content["source_session"].as_str().unwrap(), "test-session");

        // Check fs port cassette
        let fs_path = output.join("fs").join("test-session.yaml");
        assert!(fs_path.exists(), "fs cassette should exist");
        let fs_content: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(&fs_path).unwrap()).unwrap();
        let fs_interactions = fs_content["interactions"].as_sequence().unwrap();
        assert_eq!(fs_interactions.len(), 1, "fs should have 1 interaction");
        assert_eq!(fs_interactions[0]["seq"].as_u64().unwrap(), 0);

        // Check git port cassette
        let git_path = output.join("git").join("test-session.yaml");
        assert!(git_path.exists(), "git cassette should exist");
        let git_content: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(&git_path).unwrap()).unwrap();
        let git_interactions = git_content["interactions"].as_sequence().unwrap();
        assert_eq!(git_interactions.len(), 1, "git should have 1 interaction");

        // Ports with no interactions should not have files
        assert!(!output.join("clock").exists(), "clock dir should not exist (no interactions)");
        assert!(!output.join("shell").exists(), "shell dir should not exist (no interactions)");

        let _ = fs::remove_dir_all(&dir);
    }
}
