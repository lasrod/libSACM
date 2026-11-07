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

/// A closed pipe on stdout is success: stdout is one end of a Unix socket
/// pair whose other end is closed before `sacm` starts, so the write fails
/// with EPIPE (`BrokenPipe`). Linux only: the mechanism is EPIPE; Windows
/// reports a closed pipe differently (os error 232) and is not covered here.
#[cfg(target_os = "linux")]
#[test]
fn lib_cli_001_closed_stdout_is_success() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;

    let (peer, stdout) = UnixStream::pair().expect("create a Unix socket pair");
    drop(peer);
    let output = sacm()
        .arg("version")
        .stdout(Stdio::from(OwnedFd::from(stdout)))
        .stderr(Stdio::piped())
        .output()
        .expect("run sacm version");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "LIB-CLI-001: a closed stdout pipe must exit 0, got {:?}; stderr: {stderr}",
        output.status
    );
    assert!(
        stderr.is_empty(),
        "LIB-CLI-001: a closed stdout pipe must write nothing to stderr, got: {stderr}"
    );
}

/// Any other stdout write error is a failure with a message: stdout is
/// `/dev/full`, so the write fails with ENOSPC.
#[cfg(target_os = "linux")]
#[test]
fn lib_cli_001_stdout_write_error_fails_with_message() {
    use std::fs::OpenOptions;
    use std::process::Stdio;

    let full = OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .expect("open /dev/full for writing");
    let output = sacm()
        .arg("version")
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
        .expect("run sacm version");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "LIB-CLI-001: a stdout write error must exit non-zero, got {:?}; stderr: {stderr}",
        output.status
    );
    assert!(
        stderr.starts_with("sacm: "),
        "LIB-CLI-001: a stdout write error must print a `sacm: ` message on stderr, got: {stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "LIB-CLI-001: a stdout write error must not panic, got: {stderr}"
    );
}
