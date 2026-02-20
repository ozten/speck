//! Spec store — persistence layer for task specs and requirements.
//!
//! The store lives outside the target project's git repo and uses the
//! `FileSystem` port trait for all I/O. Directory layout:
//!
//! ```text
//! <root>/
//!   ├── requirements/
//!   ├── tasks/
//!   └── history/
//! ```

use std::path::{Path, PathBuf};

use crate::context::ServiceContext;
use crate::spec::TaskSpec;

/// Persistence layer for task specs and requirements.
///
/// All I/O goes through `ctx.fs` so that the store works with live,
/// replaying, and recording adapters.
pub struct SpecStore<'a> {
    ctx: &'a ServiceContext,
    root: PathBuf,
}

impl<'a> SpecStore<'a> {
    /// Creates a new store rooted at the given path.
    #[must_use]
    pub fn new(ctx: &'a ServiceContext, root: &Path) -> Self {
        Self { ctx, root: root.to_path_buf() }
    }

    /// Saves a task spec as YAML in `<root>/tasks/<id>.yaml`.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or file writing fails.
    pub fn save_task_spec(&self, spec: &TaskSpec) -> Result<(), String> {
        let yaml = serde_yaml::to_string(spec)
            .map_err(|e| format!("Failed to serialize task spec {}: {e}", spec.id))?;
        let path = self.task_path(&spec.id);
        self.ctx
            .fs
            .write(&path, &yaml)
            .map_err(|e| format!("Failed to write task spec {}: {e}", spec.id))
    }

    /// Loads a task spec by ID from `<root>/tasks/<id>.yaml`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load_task_spec(&self, id: &str) -> Result<TaskSpec, String> {
        let path = self.task_path(id);
        let contents = self
            .ctx
            .fs
            .read_to_string(&path)
            .map_err(|e| format!("Failed to read task spec {id}: {e}"))?;
        serde_yaml::from_str(&contents).map_err(|e| format!("Failed to parse task spec {id}: {e}"))
    }

    /// Lists all task spec IDs in the store.
    ///
    /// Returns IDs derived from filenames (without the `.yaml` extension).
    ///
    /// # Errors
    ///
    /// Returns an error if the tasks directory cannot be listed.
    pub fn list_task_specs(&self) -> Result<Vec<String>, String> {
        let tasks_dir = self.root.join("tasks");
        if !self.ctx.fs.exists(&tasks_dir) {
            return Ok(Vec::new());
        }
        let entries = self
            .ctx
            .fs
            .list_dir(&tasks_dir)
            .map_err(|e| format!("Failed to list tasks directory: {e}"))?;
        Ok(entries
            .into_iter()
            .filter_map(|name| name.strip_suffix(".yaml").map(String::from))
            .collect())
    }

    /// Saves a requirement document as YAML in `<root>/requirements/<id>.yaml`.
    ///
    /// # Errors
    ///
    /// Returns an error if file writing fails.
    pub fn save_requirement(&self, id: &str, content: &str) -> Result<(), String> {
        let path = self.root.join("requirements").join(format!("{id}.yaml"));
        self.ctx
            .fs
            .write(&path, content)
            .map_err(|e| format!("Failed to write requirement {id}: {e}"))
    }

    fn task_path(&self, id: &str) -> PathBuf {
        self.root.join("tasks").join(format!("{id}.yaml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{SignalType, VerificationCheck, VerificationStrategy};

    /// In-memory filesystem for testing the store without touching disk.
    struct MemFs {
        files: std::sync::Mutex<std::collections::HashMap<PathBuf, String>>,
    }

    impl MemFs {
        fn new() -> Self {
            Self { files: std::sync::Mutex::new(std::collections::HashMap::new()) }
        }
    }

    impl crate::ports::filesystem::FileSystem for MemFs {
        fn read_to_string(
            &self,
            path: &Path,
        ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
            let files = self.files.lock().unwrap();
            files
                .get(path)
                .cloned()
                .ok_or_else(|| format!("File not found: {}", path.display()).into())
        }

        fn write(
            &self,
            path: &Path,
            contents: &str,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let mut files = self.files.lock().unwrap();
            files.insert(path.to_path_buf(), contents.to_string());
            Ok(())
        }

        fn exists(&self, path: &Path) -> bool {
            let files = self.files.lock().unwrap();
            // Check exact path or if any file is "under" this directory.
            files.contains_key(path) || files.keys().any(|k| k.starts_with(path) && k != path)
        }

        fn list_dir(
            &self,
            path: &Path,
        ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
            let files = self.files.lock().unwrap();
            let mut names: Vec<String> = files
                .keys()
                .filter_map(|k| {
                    if k.parent() == Some(path) {
                        k.file_name().map(|n| n.to_string_lossy().into_owned())
                    } else {
                        None
                    }
                })
                .collect();
            names.sort();
            Ok(names)
        }
    }

    fn make_test_context(fs: MemFs) -> ServiceContext {
        use crate::cassette::config::CassetteConfig;
        // Start from panicking defaults, then replace fs.
        let mut ctx = ServiceContext::replaying_from(&CassetteConfig::panic_on_unspecified())
            .expect("panic config should always succeed");
        ctx.fs = Box::new(fs);
        ctx
    }

    fn sample_spec(id: &str) -> TaskSpec {
        TaskSpec {
            id: id.to_string(),
            title: format!("Test task {id}"),
            requirement: Some("test-req".to_string()),
            context: None,
            acceptance_criteria: vec!["it works".to_string()],
            signal_type: SignalType::Clear,
            verification: VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::TestSuite {
                    command: "cargo test".to_string(),
                    expected: "all pass".to_string(),
                }],
            },
        }
    }

    #[test]
    fn save_and_load_round_trips() {
        let fs = MemFs::new();
        let ctx = make_test_context(fs);
        let store = SpecStore::new(&ctx, Path::new("/store"));

        let spec = sample_spec("TASK-1");
        store.save_task_spec(&spec).unwrap();
        let loaded = store.load_task_spec("TASK-1").unwrap();

        assert_eq!(spec, loaded);
    }

    #[test]
    fn list_task_specs_returns_all_saved() {
        let fs = MemFs::new();
        let ctx = make_test_context(fs);
        let store = SpecStore::new(&ctx, Path::new("/store"));

        store.save_task_spec(&sample_spec("ALPHA")).unwrap();
        store.save_task_spec(&sample_spec("BETA")).unwrap();
        store.save_task_spec(&sample_spec("GAMMA")).unwrap();

        let mut ids = store.list_task_specs().unwrap();
        ids.sort();
        assert_eq!(ids, vec!["ALPHA", "BETA", "GAMMA"]);
    }

    #[test]
    fn list_task_specs_empty_store() {
        let fs = MemFs::new();
        let ctx = make_test_context(fs);
        let store = SpecStore::new(&ctx, Path::new("/store"));

        let ids = store.list_task_specs().unwrap();
        assert!(ids.is_empty());
    }

    #[test]
    fn save_requirement() {
        let fs = MemFs::new();
        let ctx = make_test_context(fs);
        let store = SpecStore::new(&ctx, Path::new("/store"));

        store.save_requirement("req-1", "title: My Requirement\n").unwrap();

        // Verify it was written by reading through the fs port.
        let content = ctx.fs.read_to_string(Path::new("/store/requirements/req-1.yaml")).unwrap();
        assert!(content.contains("My Requirement"));
    }
}
