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
    assert!(stdout.contains("[stub] speck plan"));
}

#[test]
fn verify_subcommand_prints_stub_message() {
    let output = run_speck(&["verify"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("[stub] speck verify"));
}

#[test]
fn invalid_subcommand_exits_with_error() {
    let output = run_speck(&["nonsense"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("unrecognized subcommand"));
}
