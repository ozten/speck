//! Recording adapter for the `IdGenerator` port.

use std::sync::{Arc, Mutex};

use super::record_interaction;
use crate::cassette::recorder::CassetteRecorder;
use crate::ports::IdGenerator;

/// Records ID generator interactions while delegating to an inner implementation.
pub struct RecordingIdGenerator {
    inner: Box<dyn IdGenerator>,
    recorder: Arc<Mutex<CassetteRecorder>>,
}

impl RecordingIdGenerator {
    /// Creates a new recording ID generator wrapping the given implementation.
    pub fn new(inner: Box<dyn IdGenerator>, recorder: Arc<Mutex<CassetteRecorder>>) -> Self {
        Self { inner, recorder }
    }
}

impl IdGenerator for RecordingIdGenerator {
    fn generate_id(&self) -> String {
        let result = self.inner.generate_id();
        record_interaction(&self.recorder, "id_gen", "generate_id", &(), &result);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeIdGenerator;

    impl IdGenerator for FakeIdGenerator {
        fn generate_id(&self) -> String {
            "fake-id-001".into()
        }
    }

    #[test]
    fn records_generate_id_interaction() {
        let dir = std::env::temp_dir().join("speck_rec_id_gen_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("id_gen.cassette.yaml");

        let recorder = Arc::new(Mutex::new(CassetteRecorder::new(&path, "test", "abc")));

        // Scope the adapter so it's dropped before we try to unwrap
        let id = {
            let gen =
                RecordingIdGenerator::new(Box::new(FakeIdGenerator), Arc::clone(&recorder));
            gen.generate_id()
        };
        assert_eq!(id, "fake-id-001");

        let recorder = Arc::try_unwrap(recorder).unwrap().into_inner().unwrap();
        recorder.finish().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("id_gen"));
        assert!(content.contains("generate_id"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
