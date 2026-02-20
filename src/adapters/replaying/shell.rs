//! Replaying adapter for the `ShellExecutor` port.

use std::sync::Mutex;

use crate::cassette::replayer::CassetteReplayer;
use crate::ports::shell::{ShellExecutor, ShellOutput};

/// Replays recorded shell command results from a cassette.
pub struct ReplayingShellExecutor {
    replayer: Mutex<CassetteReplayer>,
}

impl ReplayingShellExecutor {
    /// Creates a new replaying shell executor from a cassette replayer.
    #[must_use]
    pub fn new(replayer: CassetteReplayer) -> Self {
        Self { replayer: Mutex::new(replayer) }
    }
}

impl ShellExecutor for ReplayingShellExecutor {
    fn run(&self, _command: &str) -> Result<ShellOutput, Box<dyn std::error::Error + Send + Sync>> {
        let output = {
            let mut replayer = self.replayer.lock().expect("replayer lock poisoned");
            let interaction = replayer.next_interaction("shell", "run");
            interaction.output.clone()
        };
        if let Some(err) = output.get("err") {
            let msg = err.as_str().unwrap_or("unknown error").to_string();
            return Err(msg.into());
        }
        let value = output.get("ok").unwrap_or(&output);
        let exit_code = value.get("exit_code").and_then(serde_json::Value::as_i64).unwrap_or(0);
        let stdout =
            value.get("stdout").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
        let stderr =
            value.get("stderr").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
        Ok(ShellOutput { exit_code: i32::try_from(exit_code).unwrap_or(1), stdout, stderr })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
    use chrono::Utc;
    use serde_json::json;

    fn make_replayer(interactions: Vec<Interaction>) -> CassetteReplayer {
        let cassette = Cassette {
            name: "test".into(),
            recorded_at: Utc::now(),
            commit: "abc".into(),
            interactions,
        };
        CassetteReplayer::new(&cassette)
    }

    #[test]
    fn replaying_shell_run() {
        let replayer = make_replayer(vec![Interaction {
            seq: 0,
            port: "shell".into(),
            method: "run".into(),
            input: json!({"command": "echo hello"}),
            output: json!({"ok": {"exit_code": 0, "stdout": "hello\n", "stderr": ""}}),
        }]);
        let shell = ReplayingShellExecutor::new(replayer);
        let result = shell.run("echo hello").unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "hello\n");
    }

    #[test]
    fn replaying_shell_run_error() {
        let replayer = make_replayer(vec![Interaction {
            seq: 0,
            port: "shell".into(),
            method: "run".into(),
            input: json!({"command": "bad_cmd"}),
            output: json!({"err": "command not found"}),
        }]);
        let shell = ReplayingShellExecutor::new(replayer);
        let result = shell.run("bad_cmd");
        assert!(result.is_err());
    }
}
