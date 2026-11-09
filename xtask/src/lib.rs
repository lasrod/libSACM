//! Repository automation for libSACM, run as `cargo xtask <command>`.
//!
//! Every command lives in this library, including the dispatcher [`run_from`]
//! that maps the command-line arguments to a command. The binary in
//! `src/main.rs` only decodes its arguments and `XTASK_ROOT`, calls
//! [`run_from`] with the repository root, where that root came from
//! ([`RootSource`]) and standard output, and turns the result into an exit
//! status. The acceptance tests in `xtask/tests/` therefore exercise the same
//! code through a public API.
//!
//! - [`run_from`] and [`run`]: the dispatcher and the usage text.
//! - [`matrix`]: the functionality-matrix gate (`cargo xtask matrix check`,
//!   `render`, `next`, `status`).
//! - [`fetch`]: hash-pinned download of the OMG SACM 2.3 reference artifacts
//!   (`cargo xtask fetch-spec`).
//! - [`mutants`]: the check that each mutation exclusion excludes exactly its
//!   triaged mutants (`cargo xtask mutants exclusions`).
//!
//! ```text
//! cargo xtask matrix check        Gate: validate the functionality matrix against the repo
//! cargo xtask matrix render       Regenerate docs/matrix/functionality-matrix.md
//! cargo xtask matrix next [-n N]  List the next ready rows for the slice loop
//! cargo xtask matrix status       Summary of row statuses per milestone
//! cargo xtask mutants exclusions  Check each mutation exclusion against its triage rows
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
//! Under `XTASK_ROOT`, `matrix check` names the root it checked: the first
//! line of its report ends with ` (root: <root>)`, so a check of another tree
//! cannot pass for a check of the repository. The other commands report as
//! usual. An `XTASK_ROOT` that is set but empty is an error, and no command
//! runs.
//!
//! This crate is repository tooling, not part of the published library: it is
//! `publish = false` and its API serves the binary and its tests only.

pub mod fetch;
pub mod matrix;
pub mod mutants;

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
  mutants exclusions      check that each exclusion in .cargo/mutants.toml excludes
                          exactly the mutants docs/quality/mutants-triage.md assigns
                          to it (runs cargo-mutants)
  fetch-spec [--dest DIR] download the pinned OMG SACM 2.3 reference artifacts
                          (default DIR: third_party/sacm-2.3)

environment:
  XTASK_ROOT              repository to work on (default: the one xtask was built in);
                          `matrix check` names it, and an empty value is an error";

/// How many rows `matrix next` lists when no count is given.
const DEFAULT_NEXT_LIMIT: usize = 10;

/// Where the repository root came from.
///
/// [`run_from`] passes it on to the command. Only `matrix check` reports
/// differently by source: it names a root that came from
/// [`Environment`](RootSource::Environment). The decision depends on the
/// source alone, never on the root's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootSource {
    /// The repository xtask was built in ([`repo_root`]).
    Build,
    /// The `XTASK_ROOT` environment variable.
    Environment,
}

/// Run the `cargo xtask` command named by `args` on the repository at `root`,
/// writing its output to `out`.
///
/// This is [`run_from`] with [`RootSource::Build`], so no report names the
/// root.
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
/// As [`run_from`].
pub fn run(args: &[&str], root: &Path, out: &mut dyn Write) -> Result<(), String> {
    run_from(args, root, RootSource::Build, out)
}

/// Run the `cargo xtask` command named by `args` on the repository at `root`,
/// which came from `source`, writing its output to `out`.
///
/// `args` are the command-line arguments without the program name, for
/// example `["matrix", "next", "-n", "3"]`. `help`, `--help`, `-h` and no
/// arguments write the usage text. Paths given to `fetch-spec --dest` are
/// resolved against `root`. What a successful command reports goes to `out`,
/// which is flushed before returning; a failing command writes nothing to
/// standard error, and the problems `matrix check` finds are in its error.
///
/// With [`RootSource::Environment`], `matrix check` names the root: the first
/// line of its report, which is the OK line, the problem-count header or the
/// one-line load error, ends with ` (root: <root>)`. `<root>` is `root` as
/// [`Path::display`] shows it, as given (not canonicalised), with control
/// characters escaped as in the problem lines. Every other command, and
/// `matrix check` with [`RootSource::Build`], reports exactly what [`run`]
/// reports.
///
/// `run_from` neither prefixes its messages with `xtask:` nor exits the
/// process; that is the binary's job.
///
/// ```
/// use std::path::Path;
/// use xtask::RootSource;
///
/// // No matrix under this root: the read error names the root it came from.
/// let root = Path::new("no-such-tree");
/// let error = xtask::run_from(&["matrix", "check"], root, RootSource::Environment, &mut Vec::new())
///     .unwrap_err();
/// assert!(error.starts_with("read no-such-tree"));
/// assert!(error.ends_with(" (root: no-such-tree)"));
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
///   [`matrix::cmd_next`], [`matrix::cmd_status`],
///   [`mutants::cmd_exclusions`] and [`fetch::cmd_fetch`]);
///   a matrix that cannot be read or parsed is one line, as [`matrix::load`]
///   describes;
/// - writing to or flushing `out` fails.
pub fn run_from(
    args: &[&str],
    root: &Path,
    source: RootSource,
    out: &mut dyn Write,
) -> Result<(), String> {
    let result = dispatch(args, root, source, out);
    // Keep the command's own error when both it and the flush fail.
    let flushed = out.flush().map_err(output_error);
    result.and(flushed)
}

fn dispatch(
    args: &[&str],
    root: &Path,
    source: RootSource,
    out: &mut dyn Write,
) -> Result<(), String> {
    match args {
        ["matrix", "check"] => matrix::cmd_check_from(root, source, out),
        ["matrix", "render"] => matrix::cmd_render(root, out),
        ["matrix", "next"] => matrix::cmd_next(root, DEFAULT_NEXT_LIMIT, out),
        ["matrix", "next", "-n" | "--limit", n] => match n.parse() {
            Ok(n) => matrix::cmd_next(root, n, out),
            Err(_) => Err(format!("invalid row count: {n}")),
        },
        ["matrix", "status"] => matrix::cmd_status(root, out),
        ["mutants", "exclusions"] => mutants::cmd_exclusions(root, out),
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn mutants_exclusions_is_a_command() {
        let error = super::run(
            &["mutants", "exclusions"],
            Path::new("no-such-xtask-root"),
            &mut Vec::new(),
        )
        .expect_err("there is no configuration under the root");
        assert!(error.starts_with("read no-such-xtask-root"), "{error}");
    }
}
