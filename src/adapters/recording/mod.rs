//! Recording adapters that capture interactions to cassettes.

pub mod clock;
pub mod filesystem;
pub mod git;
pub mod id_gen;
pub mod issues;
pub mod llm;
pub mod shell;

use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::cassette::recorder::CassetteRecorder;

/// Record an interaction with a simple (non-Result) return value.
///
/// Mirror of `replaying::next_output` - records input/output instead of reading.
pub(crate) fn record_interaction<I, O>(
    recorder: &Arc<Mutex<CassetteRecorder>>,
    port: &str,
    method: &str,
    input: &I,
    output: &O,
) where
    I: Serialize,
    O: Serialize,
{
    let input_json =
        serde_json::to_value(input).expect("failed to serialize recording input");
    let output_json =
        serde_json::to_value(output).expect("failed to serialize recording output");

    let mut guard = recorder.lock().expect("recorder lock poisoned");
    guard.record(port, method, input_json, output_json);
}

/// Record a `Result<T, E>` interaction using the Ok/Err JSON convention.
///
/// Mirror of `replaying::replay_result` - serializes Result for recording.
///
/// Convention:
/// - `Ok(v)` is serialized as `{"Ok": v}`
/// - `Err(e)` is serialized as `{"Err": e.to_string()}`
pub(crate) fn record_result<T, E, I>(
    recorder: &Arc<Mutex<CassetteRecorder>>,
    port: &str,
    method: &str,
    input: &I,
    result: &Result<T, E>,
) where
    T: Serialize,
    E: std::fmt::Display,
    I: Serialize,
{
    let input_json =
        serde_json::to_value(input).expect("failed to serialize recording input");

    let output_json = match result {
        Ok(v) => {
            let inner = serde_json::to_value(v).expect("failed to serialize Ok value");
            serde_json::json!({ "Ok": inner })
        }
        Err(e) => serde_json::json!({ "Err": e.to_string() }),
    };

    let mut guard = recorder.lock().expect("recorder lock poisoned");
    guard.record(port, method, input_json, output_json);
}
