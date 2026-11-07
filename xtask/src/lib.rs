//! Repository automation for libSACM, run as `cargo xtask <command>`.
//!
//! Every command lives in this library, including the dispatcher [`run`] that
//! maps the command-line arguments to a command. The binary in `src/main.rs`
//! only decodes its arguments, calls [`run`] with the repository root and
//! standard output, and turns the result into an exit status. The acceptance
//! tests in `xtask/tests/` therefore exercise the same code through a public
//! API.
//!
//! - [`run`]: the dispatcher and the usage text.
//! - [`matrix`]: the functionality-matrix gate (`cargo xtask matrix check`,
//!   `render`, `next`, `status`).
//! - [`fetch`]: hash-pinned download of the OMG SACM 2.3 reference artifacts
//!   (`cargo xtask fetch-spec`).
//!
//! ```text
//! cargo xtask matrix check        Gate: validate the functionality matrix against the repo
//! cargo xtask matrix render       Regenerate docs/matrix/functionality-matrix.md
//! cargo xtask matrix next [-n N]  List the next ready rows for the slice loop
//! cargo xtask matrix status       Summary of row statuses per milestone
//! cargo xtask fetch-spec [--dest DIR]
//!                                 Download the pinned OMG SACM 2.3 reference artifacts
//! ```
//!
//! The binary works on the repository it was built in. Set the `XTASK_ROOT`
//! environment variable to run it on another tree, for example a temporary
//! copy:
//!
//! ```text
//! XTASK_ROOT=/tmp/checkout cargo xtask matrix check
//! ```
//!
//! This crate is repository tooling, not part of the published library: it is
//! `publish = false` and its API serves the binary and its tests only.

pub mod fetch;
pub mod matrix;

use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The usage text written by `cargo xtask help` and appended to the
/// unrecognized-arguments error.
const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  matrix check            validate the functionality matrix against the repository
  matrix render           regenerate docs/matrix/functionality-matrix.md
  matrix next [-n N]      list the next N ready rows (default 10)
  matrix status           summarize row statuses per milestone
  fetch-spec [--dest DIR] download the pinned OMG SACM 2.3 reference artifacts
                          (default DIR: third_party/sacm-2.3)

environment:
  XTASK_ROOT              repository to work on (default: the one xtask was built in)";

/// How many rows `matrix next` lists when no count is given.
const DEFAULT_NEXT_LIMIT: usize = 10;

/// Run the `cargo xtask` command named by `args` on the repository at `root`,
/// writing its output to `out`.
///
/// `args` are the command-line arguments without the program name, for
/// example `["matrix", "next", "-n", "3"]`. `help`, `--help`, `-h` and no
/// arguments write the usage text. Paths given to `fetch-spec --dest` are
/// resolved against `root`. What a successful command reports goes to `out`,
/// which is flushed before returning; a failing command writes nothing to
/// standard error, and the problems `matrix check` finds are in its error.
///
/// `run` neither prefixes its messages with `xtask:` nor exits the process;
/// that is the binary's job.
///
/// ```
/// use std::path::Path;
///
/// let mut out = Vec::new();
/// xtask::run(&["help"], Path::new("."), &mut out)?;
/// assert!(String::from_utf8_lossy(&out).starts_with("usage: cargo xtask <command>"));
///
/// let error = xtask::run(&["frobnicate"], Path::new("."), &mut Vec::new()).unwrap_err();
/// assert!(error.starts_with("unrecognized arguments: frobnicate\n\nusage: cargo xtask"));
/// # Ok::<(), String>(())
/// ```
///
/// # Errors
///
/// Returns a message, without the `xtask:` prefix, when:
///
/// - the arguments name no command: `unrecognized arguments: <args>` followed
///   by a blank line and the usage text;
/// - the count given to `matrix next -n`/`--limit` is not a number:
///   `invalid row count: <count>`;
/// - the command fails (see [`matrix::cmd_check`], [`matrix::cmd_render`],
///   [`matrix::cmd_next`], [`matrix::cmd_status`] and [`fetch::cmd_fetch`]);
/// - writing to or flushing `out` fails.
pub fn run(args: &[&str], root: &Path, out: &mut dyn Write) -> Result<(), String> {
    let result = dispatch(args, root, out);
    // Keep the command's own error when both it and the flush fail.
    let flushed = out.flush().map_err(output_error);
    result.and(flushed)
}

fn dispatch(args: &[&str], root: &Path, out: &mut dyn Write) -> Result<(), String> {
    match args {
        ["matrix", "check"] => matrix::cmd_check(root, out),
        ["matrix", "render"] => matrix::cmd_render(root, out),
        ["matrix", "next"] => matrix::cmd_next(root, DEFAULT_NEXT_LIMIT, out),
        ["matrix", "next", "-n" | "--limit", n] => match n.parse() {
            Ok(n) => matrix::cmd_next(root, n, out),
            Err(_) => Err(format!("invalid row count: {n}")),
        },
        ["matrix", "status"] => matrix::cmd_status(root, out),
        ["fetch-spec"] => fetch::cmd_fetch(&root.join(fetch::DEFAULT_DEST), out),
        ["fetch-spec", "--dest", dest] => fetch::cmd_fetch(&root.join(dest), out),
        ["help" | "--help" | "-h"] | [] => writeln!(out, "{USAGE}").map_err(output_error),
        _ => Err(format!(
            "unrecognized arguments: {}\n\n{USAGE}",
            args.join(" ")
        )),
    }
}

/// The error message for a failed write to a command's output.
// Takes the error by value so that it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn output_error(error: io::Error) -> String {
    format!("write output: {error}")
}

/// The repository root: the parent of the `xtask` crate directory.
///
/// Resolved from the crate's manifest directory at compile time, so it does
/// not depend on the current working directory.
///
/// # Panics
///
/// Never in this repository: the `xtask` crate always lives one level below
/// the repository root, so its manifest directory has a parent.
#[must_use]
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the repository root")
        .to_path_buf()
}
