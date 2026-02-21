//! Recording adapter for the `Clock` port.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

use super::record_interaction;
use crate::cassette::recorder::CassetteRecorder;
use crate::ports::Clock;

/// Records clock interactions while delegating to an inner implementation.
pub struct RecordingClock {
    inner: Box<dyn Clock>,
    recorder: Arc<Mutex<CassetteRecorder>>,
}

impl RecordingClock {
    /// Creates a new recording clock wrapping the given implementation.
    pub fn new(inner: Box<dyn Clock>, recorder: Arc<Mutex<CassetteRecorder>>) -> Self {
        Self { inner, recorder }
    }
}

impl Clock for RecordingClock {
    fn now(&self) -> DateTime<Utc> {
        let result = self.inner.now();
        record_interaction(&self.recorder, "clock", "now", &(), &result);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::live::clock::LiveClock;

    #[test]
    fn records_now_interaction() {
        let dir = std::env::temp_dir().join("speck_rec_clock_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clock.cassette.yaml");

        let recorder = Arc::new(Mutex::new(CassetteRecorder::new(&path, "test", "abc")));

        // Scope the adapter so it's dropped before we try to unwrap
        {
            let clock = RecordingClock::new(Box::new(LiveClock), Arc::clone(&recorder));
            let _ = clock.now();
        }

        // Finish and verify file was written
        let recorder = Arc::try_unwrap(recorder).unwrap().into_inner().unwrap();
        recorder.finish().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("clock"));
        assert!(content.contains("now"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
