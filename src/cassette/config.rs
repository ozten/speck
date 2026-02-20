//! Cassette configuration for composable per-port replay.

use std::path::{Path, PathBuf};

use super::format::Cassette;
use super::replayer::CassetteReplayer;

/// Per-port cassette file paths. Each port can optionally have its own
/// cassette file for replay. Ports without a cassette path will panic
/// if called during replay.
#[derive(Debug, Clone, Default)]
pub struct CassetteConfig {
    /// Path to the LLM port cassette file.
    pub llm: Option<PathBuf>,
    /// Path to the filesystem port cassette file.
    pub fs: Option<PathBuf>,
    /// Path to the git port cassette file.
    pub git: Option<PathBuf>,
    /// Path to the clock port cassette file.
    pub clock: Option<PathBuf>,
    /// Path to the shell port cassette file.
    pub shell: Option<PathBuf>,
    /// Path to the ID generator port cassette file.
    pub id_gen: Option<PathBuf>,
    /// Path to the issues port cassette file.
    pub issues: Option<PathBuf>,
}

/// Per-port replayers, each with its own interaction stream.
pub struct PortReplayers {
    /// Replayer for the LLM port.
    pub llm: Option<CassetteReplayer>,
    /// Replayer for the filesystem port.
    pub fs: Option<CassetteReplayer>,
    /// Replayer for the git port.
    pub git: Option<CassetteReplayer>,
    /// Replayer for the clock port.
    pub clock: Option<CassetteReplayer>,
    /// Replayer for the shell port.
    pub shell: Option<CassetteReplayer>,
    /// Replayer for the ID generator port.
    pub id_gen: Option<CassetteReplayer>,
    /// Replayer for the issues port.
    pub issues: Option<CassetteReplayer>,
}

impl CassetteConfig {
    /// Returns a config where all port paths are `None`. Any port called
    /// during replay will panic because no cassette is loaded.
    #[must_use]
    pub fn panic_on_unspecified() -> Self {
        Self::default()
    }

    /// Load a monolithic cassette file and create a single replayer.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load_monolithic(path: &Path) -> Result<CassetteReplayer, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read cassette file {}: {e}", path.display()))?;
        let cassette: Cassette = serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse cassette file {}: {e}", path.display()))?;
        Ok(CassetteReplayer::new(&cassette))
    }

    /// Load a single per-port cassette file and create a replayer.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load_port_cassette(path: &Path) -> Result<CassetteReplayer, String> {
        Self::load_monolithic(path)
    }

    /// Load all configured per-port cassette files and create replayers.
    ///
    /// # Errors
    ///
    /// Returns an error if any configured cassette file cannot be read or parsed.
    pub fn load_all(&self) -> Result<PortReplayers, String> {
        Ok(PortReplayers {
            llm: self.llm.as_deref().map(Self::load_port_cassette).transpose()?,
            fs: self.fs.as_deref().map(Self::load_port_cassette).transpose()?,
            git: self.git.as_deref().map(Self::load_port_cassette).transpose()?,
            clock: self.clock.as_deref().map(Self::load_port_cassette).transpose()?,
            shell: self.shell.as_deref().map(Self::load_port_cassette).transpose()?,
            id_gen: self.id_gen.as_deref().map(Self::load_port_cassette).transpose()?,
            issues: self.issues.as_deref().map(Self::load_port_cassette).transpose()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
    use chrono::Utc;
    use serde_json::json;

    fn write_cassette(path: &Path, interactions: Vec<Interaction>) {
        let cassette = Cassette {
            name: "test".into(),
            recorded_at: Utc::now(),
            commit: "abc".into(),
            interactions,
        };
        let yaml = serde_yaml::to_string(&cassette).unwrap();
        std::fs::write(path, yaml).unwrap();
    }

    #[test]
    fn panic_on_unspecified_returns_all_none() {
        let config = CassetteConfig::panic_on_unspecified();
        assert!(config.llm.is_none());
        assert!(config.fs.is_none());
        assert!(config.git.is_none());
        assert!(config.clock.is_none());
        assert!(config.shell.is_none());
        assert!(config.id_gen.is_none());
        assert!(config.issues.is_none());
    }

    #[test]
    fn load_monolithic_cassette() {
        let dir = std::env::temp_dir().join("speck_config_test_mono");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("full.cassette.yaml");

        write_cassette(
            &path,
            vec![
                Interaction {
                    seq: 0,
                    port: "llm".into(),
                    method: "complete".into(),
                    input: json!({"prompt": "a"}),
                    output: json!({"text": "1"}),
                },
                Interaction {
                    seq: 1,
                    port: "fs".into(),
                    method: "read".into(),
                    input: json!({"path": "/x"}),
                    output: json!({"data": "y"}),
                },
            ],
        );

        let mut replayer = CassetteConfig::load_monolithic(&path).unwrap();
        let i1 = replayer.next_interaction("llm", "complete");
        assert_eq!(i1.output, json!({"text": "1"}));
        let i2 = replayer.next_interaction("fs", "read");
        assert_eq!(i2.output, json!({"data": "y"}));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_per_port_cassettes() {
        let dir = std::env::temp_dir().join("speck_config_test_ports");
        std::fs::create_dir_all(&dir).unwrap();

        let llm_path = dir.join("llm.cassette.yaml");
        write_cassette(
            &llm_path,
            vec![Interaction {
                seq: 0,
                port: "llm".into(),
                method: "complete".into(),
                input: json!({"prompt": "hello"}),
                output: json!({"text": "world"}),
            }],
        );

        let fs_path = dir.join("fs.cassette.yaml");
        write_cassette(
            &fs_path,
            vec![Interaction {
                seq: 0,
                port: "fs".into(),
                method: "read".into(),
                input: json!({"path": "/a"}),
                output: json!({"content": "b"}),
            }],
        );

        let config =
            CassetteConfig { llm: Some(llm_path), fs: Some(fs_path), ..CassetteConfig::default() };

        let mut replayers = config.load_all().unwrap();

        // LLM replayer works
        let llm = replayers.llm.as_mut().unwrap();
        let i1 = llm.next_interaction("llm", "complete");
        assert_eq!(i1.output, json!({"text": "world"}));

        // FS replayer works
        let fs = replayers.fs.as_mut().unwrap();
        let i2 = fs.next_interaction("fs", "read");
        assert_eq!(i2.output, json!({"content": "b"}));

        // Unconfigured ports are None
        assert!(replayers.git.is_none());
        assert!(replayers.clock.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_all_with_no_cassettes() {
        let config = CassetteConfig::panic_on_unspecified();
        let replayers = config.load_all().unwrap();
        assert!(replayers.llm.is_none());
        assert!(replayers.fs.is_none());
        assert!(replayers.git.is_none());
    }
}
