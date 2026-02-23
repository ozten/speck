//! Integration tests for top-level CLI behavior.

use std::process::Command;

fn run_speck(args: &[&str]) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_speck");
    Command::new(bin).args(args).output().expect("failed to run speck binary")
}

#[test]
fn plan_subcommand_without_requirement_errors() {
    let output = run_speck(&["plan"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("requirement text is required"));
}

#[test]
fn validate_without_args_shows_error() {
    let output = run_speck(&["validate"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("SPEC_ID") || stderr.contains("--all"));
}

#[test]
fn validate_help_shows_usage() {
    let output = run_speck(&["validate", "--help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("spec-id") || stdout.contains("SPEC_ID") || stdout.contains("spec_id"));
    assert!(stdout.contains("--all"));
}

#[test]
fn map_subcommand_generates_map() {
    let output = run_speck(&["map"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("Map generated:"));
}

#[test]
fn map_help_shows_diff_flag() {
    let output = run_speck(&["map", "--help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("--diff"));
}

#[test]
fn show_subcommand_empty_store() {
    let output = run_speck(&["show"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("No specs found") || stdout.contains("Available specs"));
}

#[test]
fn status_subcommand_empty_store() {
    let output = run_speck(&["status"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("No specs found") || stdout.contains("ID"));
}

#[test]
fn deps_subcommand_empty_store() {
    let output = run_speck(&["deps"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(
        stdout.contains("No specs found")
            || stdout.contains("Dependency Graph")
            || stdout.contains("No dependencies found")
    );
}

#[test]
fn invalid_subcommand_exits_with_error() {
    let output = run_speck(&["nonsense"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("unrecognized subcommand"));
}

#[test]
fn plan_with_cassette_produces_specs() {
    let cassette_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test_fixtures/plan_session.yaml");

    let bin = env!("CARGO_BIN_EXE_speck");
    let output = Command::new(bin)
        .args(["plan", "Add user authentication"])
        .env("SPECK_REPLAY", &cassette_path)
        .env("SPECK_STORE", "/tmp/speck-plan-test-store")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|child| child.wait_with_output())
        .expect("failed to run speck binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "plan command failed.\nstdout: {stdout}\nstderr: {stderr}");

    // Verify survey output
    assert!(stdout.contains("Routing Table"), "should print routing table.\nstdout: {stdout}");

    // Verify signal classification output
    assert!(
        stdout.contains("Signal Classification"),
        "should print signal classification.\nstdout: {stdout}"
    );

    // Verify reconciliation output
    assert!(
        stdout.contains("Reconciliation"),
        "should print reconciliation results.\nstdout: {stdout}"
    );

    // Verify conversation loop completed
    assert!(
        stdout.contains("All task specs have verification strategies"),
        "conversation loop should complete.\nstdout: {stdout}"
    );

    // Verify specs were saved
    assert!(stdout.contains("Saved Specs"), "should print saved specs.\nstdout: {stdout}");
    assert!(stdout.contains("TASK-PLAN-1"), "should show generated spec ID.\nstdout: {stdout}");
}
