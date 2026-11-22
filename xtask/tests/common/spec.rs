//! Where the pinned OMG references are, for the `xtask` tests that read them
//! (LIB-TOOL-004, LIB-TOOL-023).
//!
//! `cargo xtask fetch-spec` puts the files of `xtask::fetch::ARTIFACTS` in
//! `third_party/sacm-2.3/`; they are never committed. A test finds them at
//! `$SACM_SPEC_DIR` when that is set, and otherwise at
//! `<repo>/third_party/sacm-2.3`:
//!
//! - not set, and a file absent: the test prints
//!   ``<ROW>: SKIPPED: <path> is absent; run `cargo xtask fetch-spec` `` on
//!   stderr and passes, as the 3-OS test job does;
//! - set, and a file absent, or set but empty: the test fails, so CI's
//!   `SACM XML Schema` job, which sets it, cannot pass by skipping;
//! - a file whose SHA-256 is not its pin: the test fails, set or not.
//!
//! Included with `#[path]` by the test files that use it, so that a test file
//! that does not is not given dead code.

use std::ffi::OsString;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use xtask::fetch::{ARTIFACTS, DEFAULT_DEST, sha256_hex};

/// The fetch command, quoted in every message.
const FETCH: &str = "run `cargo xtask fetch-spec`";

/// The environment variable that names the directory.
const SPEC_DIR_VAR: &str = "SACM_SPEC_DIR";

/// Where the references are, or why the test does not run.
#[derive(Debug, PartialEq, Eq)]
enum Located {
    /// The directory that holds every file asked for.
    Found(PathBuf),
    /// A file is absent and no directory was configured: the notice to
    /// print.
    Skip(String),
}

/// The directory: `configured` (the value of `SACM_SPEC_DIR`) if set, else
/// `default`. When one of `files` is absent from it: an error when the
/// directory was configured, a skip otherwise.
fn locate(
    row: &str,
    configured: Option<OsString>,
    default: PathBuf,
    files: &[&str],
) -> Result<Located, String> {
    let (dir, was_configured) = match configured {
        Some(value) if value.is_empty() => {
            return Err(format!(
                "{row}: {SPEC_DIR_VAR} is set but empty; unset it or {FETCH}"
            ));
        }
        Some(value) => (PathBuf::from(value), true),
        None => (default, false),
    };
    let Some(missing) = files.iter().find(|name| !dir.join(name).is_file()) else {
        return Ok(Located::Found(dir));
    };
    let path = dir.join(missing);
    if was_configured {
        Err(format!(
            "{row}: {} is missing ({SPEC_DIR_VAR} is set); {FETCH}",
            path.display()
        ))
    } else {
        Ok(Located::Skip(format!(
            "{row}: SKIPPED: {} is absent; {FETCH}",
            path.display()
        )))
    }
}

/// Checks that each of `files` in `dir` has its SHA-256 pin in
/// `xtask::fetch::ARTIFACTS`.
fn check_hashes(row: &str, dir: &Path, files: &[&str]) -> Result<(), String> {
    for name in files {
        let Some(artifact) = ARTIFACTS.iter().find(|a| a.file_name == *name) else {
            return Err(format!(
                "{row}: setup: {name} is not in xtask::fetch::ARTIFACTS"
            ));
        };
        let path = dir.join(name);
        let bytes = fs::read(&path).map_err(|e| format!("{row}: read {}: {e}", path.display()))?;
        let actual = sha256_hex(&bytes);
        if actual != artifact.sha256 {
            return Err(format!(
                "{row}: {} has SHA-256 {actual}, pinned {}; delete it and {FETCH}",
                path.display(),
                artifact.sha256
            ));
        }
    }
    Ok(())
}

/// The directory that holds `files`, their hashes checked, or `None` after
/// printing the SKIPPED notice. Panics, naming `row`, when a configured
/// directory lacks a file, `SACM_SPEC_DIR` is empty, or a file has another
/// hash.
///
/// The notice goes straight to the process's stderr: the test harness
/// captures `eprintln!` and hides it for a passing test, so a skip would look
/// like a pass.
pub(crate) fn spec_dir(row: &str, files: &[&str]) -> Option<PathBuf> {
    let default = xtask::repo_root().join(DEFAULT_DEST);
    match locate(row, std::env::var_os(SPEC_DIR_VAR), default, files) {
        Ok(Located::Found(dir)) => {
            if let Err(message) = check_hashes(row, &dir, files) {
                panic!("{message}");
            }
            Some(dir)
        }
        Ok(Located::Skip(notice)) => {
            // Nothing to do if stderr itself is gone.
            let _ = std::io::stderr().write_all(format!("{notice}\n").as_bytes());
            None
        }
        Err(message) => panic!("{message}"),
    }
}
