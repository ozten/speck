//! Integration tests for top-level CLI behavior.

use std::process::Command;

fn run_speck(args: &[&str]) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_speck");
    Command::new(bin).args(args).output().expect("failed to run speck binary")
}

#[test]
fn plan_subcommand_prints_stub_message() {
    let output = run_speck(&["plan"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("not yet implemented"));
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
fn map_subcommand_prints_stub_message() {
    let output = run_speck(&["map"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("not yet implemented"));
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
    assert!(stdout.contains("No specs found") || stdout.contains("Dependency Graph"));
}

#[test]
fn invalid_subcommand_exits_with_error() {
    let output = run_speck(&["nonsense"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("unrecognized subcommand"));
}
