//! Tests of the `xtask` binary itself (LIB-TOOL-001).
//!
//! The library tests in `matrix.rs` prove that the gate finds violations; these
//! prove that the binary turns a failure into a non-zero exit status and a
//! message on stderr, so that `cargo xtask matrix check` in CI fails when the
//! gate does.
//!
//! Only commands whose outcome does not depend on the live repository are run
//! here: the binary resolves the repository root at compile time, so running
//! `matrix check` would test the matrix's current state, not the binary.

use std::process::{Command, Output};

/// Run the real `xtask` binary with `args`.
fn xtask(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .output()
        .expect("LIB-TOOL-001: the xtask binary can be started")
}

/// `xtask <args>` fails: non-zero exit, nothing on stdout, and stderr starting
/// with `xtask: ` followed by `message`.
fn assert_fails_with(args: &[&str], message: &str) {
    let output = xtask(args);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "LIB-TOOL-001: `xtask {}` must exit non-zero, got {:?}; stderr: {stderr}",
        args.join(" "),
        output.status
    );
    assert!(
        stderr.starts_with(&format!("xtask: {message}")),
        "LIB-TOOL-001: `xtask {}` must report `xtask: {message}` on stderr, got {stderr:?}",
        args.join(" ")
    );
    assert!(
        stdout.is_empty(),
        "LIB-TOOL-001: `xtask {}` must print nothing on stdout when it fails, got {stdout:?}",
        args.join(" ")
    );
}

#[test]
fn lib_tool_001_binary_exits_non_zero_on_unrecognized_arguments() {
    assert_fails_with(
        &["frobnicate", "--now"],
        "unrecognized arguments: frobnicate --now",
    );
}

#[test]
fn lib_tool_001_binary_exits_non_zero_on_an_invalid_row_count() {
    assert_fails_with(
        &["matrix", "next", "-n", "notanumber"],
        "invalid row count: notanumber",
    );
}

#[test]
fn lib_tool_001_binary_help_exits_zero_and_prints_usage_on_stdout() {
    let output = xtask(&["help"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "LIB-TOOL-001: `xtask help` must exit zero, got {:?}",
        output.status
    );
    assert!(
        stdout.starts_with("usage: cargo xtask <command>"),
        "LIB-TOOL-001: `xtask help` must print the usage on stdout, got {stdout:?}"
    );
    assert!(
        output.stderr.is_empty(),
        "LIB-TOOL-001: `xtask help` must print nothing on stderr, got {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}
