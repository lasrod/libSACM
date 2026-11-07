//! End-to-end tests for the `sacm` binary.

use std::process::Command;

fn sacm() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sacm"))
}

#[test]
fn lib_cli_001_version_reports_tool_and_sacm_revision() {
    let output = sacm().arg("version").output().expect("run sacm version");
    assert!(
        output.status.success(),
        "LIB-CLI-001: `sacm version` must exit 0"
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8 output");
    assert_eq!(
        stdout.trim(),
        format!("sacm {} (SACM 2.3)", env!("CARGO_PKG_VERSION")),
        "LIB-CLI-001: version line names the tool version and SACM revision"
    );
}

#[test]
fn lib_cli_001_unknown_subcommand_fails_with_usage() {
    let output = sacm().arg("no-such-command").output().expect("run sacm");
    assert!(
        !output.status.success(),
        "LIB-CLI-001: unknown subcommands must fail"
    );
    let stderr = String::from_utf8(output.stderr).expect("utf-8 output");
    assert!(
        stderr.contains("Usage"),
        "LIB-CLI-001: failure prints usage, got: {stderr}"
    );
}
