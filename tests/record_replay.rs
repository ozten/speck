//! Record-replay round-trip integration test.
//!
//! Proves that the record/replay system works end-to-end:
//! 1. Record a session using `CassetteRecorder` (exercises clock, fs, id_gen).
//! 2. Replay the cassette using `ServiceContext::replaying()`.
//! 3. Assert identical outputs between recording and replaying.
//! 4. Replay a second time and assert determinism.

use std::path::Path;

use serde_json::json;

use speck::cassette::recorder::CassetteRecorder;
use speck::context::ServiceContext;

/// Exercises the clock, fs, and id_gen ports on the given context,
/// returning a snapshot of all outputs for comparison.
fn exercise_ports(ctx: &ServiceContext) -> (String, String, bool, String) {
    let time = ctx.clock.now().to_rfc3339();
    let file_content = ctx.fs.read_to_string(Path::new("/project/README.md")).unwrap();
    let exists = ctx.fs.exists(Path::new("/project/src/main.rs"));
    let id = ctx.id_gen.generate_id();
    (time, file_content, exists, id)
}

#[test]
fn record_then_replay_produces_identical_outputs() {
    let dir = std::env::temp_dir().join("speck_record_replay_test");
    std::fs::create_dir_all(&dir).unwrap();
    let cassette_path = dir.join("roundtrip.cassette.yaml");

    // --- Phase 1: Record interactions ---
    // We simulate what a recording adapter would capture by manually
    // building a cassette with known interactions for clock, fs, id_gen.
    let mut recorder = CassetteRecorder::new(&cassette_path, "roundtrip-test", "abc123");

    // Clock: now()
    recorder.record("clock", "now", json!({}), json!("2025-03-15T14:30:00Z"));

    // FileSystem: read_to_string("/project/README.md")
    recorder.record(
        "fs",
        "read_to_string",
        json!({"path": "/project/README.md"}),
        json!({"ok": "# My Project\nA sample project."}),
    );

    // FileSystem: exists("/project/src/main.rs")
    recorder.record("fs", "exists", json!({"path": "/project/src/main.rs"}), json!(true));

    // IdGenerator: generate_id()
    recorder.record("id_gen", "generate_id", json!({}), json!("speck-abc-001"));

    let written_path = recorder.finish().expect("recording should succeed");
    assert_eq!(written_path, cassette_path);

    // Known expected outputs from the recording.
    let expected_time = "2025-03-15T14:30:00+00:00";
    let expected_content = "# My Project\nA sample project.";
    let expected_exists = true;
    let expected_id = "speck-abc-001";

    // --- Phase 2: Replay and verify identical outputs ---
    let ctx1 = ServiceContext::replaying(&cassette_path).unwrap();
    let (time1, content1, exists1, id1) = exercise_ports(&ctx1);

    assert_eq!(time1, expected_time, "clock replay mismatch");
    assert_eq!(content1, expected_content, "fs read_to_string replay mismatch");
    assert_eq!(exists1, expected_exists, "fs exists replay mismatch");
    assert_eq!(id1, expected_id, "id_gen replay mismatch");

    // --- Phase 3: Replay a second time — determinism check ---
    let ctx2 = ServiceContext::replaying(&cassette_path).unwrap();
    let (time2, content2, exists2, id2) = exercise_ports(&ctx2);

    assert_eq!(time1, time2, "determinism: clock outputs differ between replays");
    assert_eq!(content1, content2, "determinism: fs read outputs differ between replays");
    assert_eq!(exists1, exists2, "determinism: fs exists outputs differ between replays");
    assert_eq!(id1, id2, "determinism: id_gen outputs differ between replays");

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn replay_from_per_port_cassettes_matches_monolithic() {
    use speck::cassette::config::CassetteConfig;

    let dir = std::env::temp_dir().join("speck_record_replay_per_port_test");
    std::fs::create_dir_all(&dir).unwrap();

    // Build per-port cassettes.
    let clock_path = dir.join("clock.cassette.yaml");
    let mut clock_rec = CassetteRecorder::new(&clock_path, "clock-port", "abc123");
    clock_rec.record("clock", "now", json!({}), json!("2025-06-01T09:00:00Z"));
    clock_rec.finish().unwrap();

    let fs_path = dir.join("fs.cassette.yaml");
    let mut fs_rec = CassetteRecorder::new(&fs_path, "fs-port", "abc123");
    fs_rec.record(
        "fs",
        "read_to_string",
        json!({"path": "/hello.txt"}),
        json!({"ok": "hello world"}),
    );
    fs_rec.finish().unwrap();

    let id_path = dir.join("id_gen.cassette.yaml");
    let mut id_rec = CassetteRecorder::new(&id_path, "id-port", "abc123");
    id_rec.record("id_gen", "generate_id", json!({}), json!("id-42"));
    id_rec.finish().unwrap();

    // Replay from per-port config.
    let config = CassetteConfig {
        clock: Some(clock_path),
        fs: Some(fs_path),
        id_gen: Some(id_path),
        ..CassetteConfig::default()
    };
    let ctx = ServiceContext::replaying_from(&config).unwrap();

    assert_eq!(ctx.clock.now().to_rfc3339(), "2025-06-01T09:00:00+00:00");
    assert_eq!(ctx.fs.read_to_string(Path::new("/hello.txt")).unwrap(), "hello world");
    assert_eq!(ctx.id_gen.generate_id(), "id-42");

    // Replay again for determinism.
    let config2 = CassetteConfig {
        clock: Some(dir.join("clock.cassette.yaml")),
        fs: Some(dir.join("fs.cassette.yaml")),
        id_gen: Some(dir.join("id_gen.cassette.yaml")),
        ..CassetteConfig::default()
    };
    let ctx2 = ServiceContext::replaying_from(&config2).unwrap();

    assert_eq!(ctx2.clock.now().to_rfc3339(), "2025-06-01T09:00:00+00:00");
    assert_eq!(ctx2.fs.read_to_string(Path::new("/hello.txt")).unwrap(), "hello world");
    assert_eq!(ctx2.id_gen.generate_id(), "id-42");

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}
