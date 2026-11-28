//! The semver check of `sacm`'s public API, `cargo xtask semver`
//! (LIB-QA-006; ADR 0046 D4).
//!
//! cargo-semver-checks compares the crate with the newest release on
//! crates.io, with the `serde` feature only (ADR 0045 D7). [`semver_plan`]
//! decides how, from three inputs:
//!
//! - **Baseline.** The highest `vX.Y.Z` tag, compared numerically; tags with
//!   a pre-release are not releases.
//! - **Declared break.** The `## [Unreleased]` section of `CHANGELOG.md` has a
//!   line `### Breaking` followed by a `- ` entry before the next `### ` or
//!   `## ` heading.
//! - **Workspace version.** When it differs from the baseline, the versions
//!   decide. When it is the same, a declared break runs at `major` before 1.0
//!   and is an error from 1.0; otherwise the check runs at `minor`, so any
//!   break fails.
//!
//! cargo-semver-checks' `--release-type` is absolute: at `minor`, a 0.x
//! break fails even though Cargo would accept it in a 0.x minor bump.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::output_error;

/// The error for a declared break after 1.0 without the major bump.
const AFTER_1_0: &str =
    "a breaking change after 1.0 needs the major version bump in this pull request";

/// The heading of a declared break in the `## [Unreleased]` section.
const BREAKING: &str = "### Breaking";

/// The release type cargo-semver-checks checks against (`--release-type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseType {
    /// Any change is allowed: a declared break before 1.0.
    Major,
    /// A breaking change fails the check.
    Minor,
}

impl ReleaseType {
    /// The name cargo-semver-checks takes: `"major"` or `"minor"`.
    ///
    /// ```
    /// assert_eq!(xtask::semver::ReleaseType::Major.as_str(), "major");
    /// ```
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Major => "major",
            Self::Minor => "minor",
        }
    }
}

/// How `cargo xtask semver` runs cargo-semver-checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemverPlan {
    /// The release to compare with, `X.Y.Z` without the tag's `v`.
    pub baseline: String,
    /// The release type to pass, or `None` when the versions decide.
    pub release_type: Option<ReleaseType>,
    /// Whether to list the breaks in the output and the job summary: for a
    /// declared break before 1.0, or a breaking version bump.
    pub report_breaks: bool,
}

/// Decide how to check the API, from the text of `CHANGELOG.md`, the
/// workspace version and the repository's tags.
///
/// ```
/// use xtask::semver::{ReleaseType, semver_plan};
///
/// let changelog = "## [Unreleased]\n\n### Breaking\n\n- removed `x`\n\n## [0.4.0] - 2026-11-26\n";
/// let plan = semver_plan(changelog, "0.4.0", &["v0.3.0", "v0.4.0"])?;
/// assert_eq!(plan.baseline, "0.4.0");
/// assert_eq!(plan.release_type, Some(ReleaseType::Major));
/// assert!(plan.report_breaks);
/// # Ok::<(), String>(())
/// ```
///
/// # Errors
///
/// Returns a message naming what is missing, the first that applies of:
/// `CHANGELOG.md` has no `## [Unreleased]` line; no tag is `vX.Y.Z`; the
/// workspace version is no `X.Y.Z`; it is below the baseline; from 1.0, a
/// break is declared with the version unchanged.
pub fn semver_plan(
    changelog: &str,
    workspace_version: &str,
    tags: &[&str],
) -> Result<SemverPlan, String> {
    let declared = declares_break(changelog)
        .ok_or_else(|| "CHANGELOG.md has no `## [Unreleased]` line".to_owned())?;
    let (base, baseline) = tags
        .iter()
        .filter_map(|tag| tag.strip_prefix('v'))
        .filter_map(|tag| version(tag).map(|parsed| (parsed, tag)))
        .max()
        .ok_or_else(|| {
            "no release tag vX.Y.Z, so there is no baseline to check against".to_owned()
        })?;
    let current = version(workspace_version).ok_or_else(|| {
        format!("the workspace version {workspace_version:?} is no X.Y.Z version")
    })?;
    if current < base {
        return Err(format!(
            "the workspace version {workspace_version} is below the baseline {baseline}"
        ));
    }
    let plan = |release_type, report_breaks| SemverPlan {
        baseline: (*baseline).to_owned(),
        release_type,
        report_breaks,
    };
    if current != base {
        let breaking = current.0 != base.0 || (current.0 == 0 && current.1 != base.1);
        return Ok(plan(None, breaking));
    }
    match (declared, base.0 == 0) {
        (true, true) => Ok(plan(Some(ReleaseType::Major), true)),
        (true, false) => Err(AFTER_1_0.to_owned()),
        (false, _) => Ok(plan(Some(ReleaseType::Minor), false)),
    }
}

/// Whether the `## [Unreleased]` section declares a break, or `None` when
/// there is no such section.
fn declares_break(changelog: &str) -> Option<bool> {
    let mut lines = changelog
        .lines()
        .skip_while(|line| *line != "## [Unreleased]");
    lines.next()?;
    let mut in_breaking = false;
    for line in lines.take_while(|line| !line.starts_with("## ")) {
        if line == BREAKING {
            in_breaking = true;
        } else if line.starts_with("### ") {
            in_breaking = false;
        } else if in_breaking && line.starts_with("- ") {
            return Some(true);
        }
    }
    Some(false)
}

/// `X.Y.Z` as numbers, or `None` for anything else.
fn version(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.split('.');
    let mut number = || {
        let part = parts.next()?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        part.parse().ok()
    };
    let parsed = (number()?, number()?, number()?);
    parts.next().is_none().then_some(parsed)
}

/// The cargo arguments that check `sacm` against `baseline` with the `serde`
/// feature only, at `release_type` when given.
///
/// ```
/// use xtask::semver::{ReleaseType, semver_args};
///
/// let args = semver_args("0.4.0", Some(ReleaseType::Minor));
/// assert_eq!(args[..2], ["semver-checks", "--package"]);
/// assert_eq!(args[args.len() - 2..], ["--release-type", "minor"]);
/// ```
#[must_use]
pub fn semver_args(baseline: &str, release_type: Option<ReleaseType>) -> Vec<String> {
    let mut args: Vec<String> = [
        "semver-checks",
        "--package",
        "sacm",
        "--baseline-version",
        baseline,
        "--only-explicit-features",
        "--features",
        "serde",
    ]
    .iter()
    .map(|&arg| arg.to_owned())
    .collect();
    if let Some(release_type) = release_type {
        args.push("--release-type".to_owned());
        args.push(release_type.as_str().to_owned());
    }
    args
}

/// `cargo xtask semver`: check `sacm`'s public API in the repository at
/// `root` against the last release, as [`semver_plan`] decides.
///
/// Writes the plan to `out`, then runs `cargo semver-checks` with its output
/// on the terminal. When the plan reports breaks, it runs the check again at
/// `minor`, writes what it found to `out` and, when `GITHUB_STEP_SUMMARY` is
/// set, appends it to that file; this second run never fails the command.
///
/// # Errors
///
/// Returns a message when `CHANGELOG.md`, the workspace version in
/// `Cargo.toml` or the tags cannot be read, when [`semver_plan`] fails, when
/// cargo-semver-checks is not installed, cannot be run or finds a break the
/// plan does not allow, or when writing to `out` or the job summary fails.
pub fn cmd_semver(root: &Path, out: &mut dyn Write) -> Result<(), String> {
    let changelog = read(&root.join("CHANGELOG.md"))?;
    let workspace_version = workspace_version(root)?;
    let tags = tags(root)?;
    let tags: Vec<&str> = tags.iter().map(String::as_str).collect();
    let plan = semver_plan(&changelog, &workspace_version, &tags)?;
    let release_type = plan
        .release_type
        .map_or("from the versions", ReleaseType::as_str);
    writeln!(
        out,
        "semver: sacm {workspace_version} against {} (crates.io), features: serde only, \
         release type: {release_type}",
        plan.baseline
    )
    .and_then(|()| out.flush())
    .map_err(output_error)?;

    // Without the tool, cargo fails as a break would; say which it is.
    let installed = cargo(root)
        .args(["semver-checks", "--version"])
        .output()
        .map_err(could_not_run)?;
    if !installed.status.success() {
        return Err(
            "cargo-semver-checks is not installed: `cargo semver-checks --version` failed"
                .to_owned(),
        );
    }
    let status = cargo(root)
        .args(semver_args(&plan.baseline, plan.release_type))
        .status()
        .map_err(could_not_run)?;
    if !status.success() {
        return Err(match plan.release_type {
            Some(ReleaseType::Minor) if version(&plan.baseline).is_some_and(|v| v.0 == 0) => {
                format!(
                    "cargo semver-checks {status}: a breaking change CHANGELOG.md does not list \
                     under [Unreleased] ### Breaking"
                )
            }
            Some(ReleaseType::Minor) => AFTER_1_0.to_owned(),
            Some(ReleaseType::Major) => format!("cargo semver-checks {status}"),
            None => format!(
                "the change needs a larger version bump than {} → {workspace_version}",
                plan.baseline
            ),
        });
    }
    if plan.report_breaks {
        report_breaks(root, &plan.baseline, out)?;
    }
    Ok(())
}

/// Run the check at `minor`, ignoring its status, and write what it found to
/// `out` and the job summary.
fn report_breaks(root: &Path, baseline: &str, out: &mut dyn Write) -> Result<(), String> {
    let output = cargo(root)
        .args(semver_args(baseline, Some(ReleaseType::Minor)))
        .output()
        .map_err(could_not_run)?;
    let mut found = String::from_utf8_lossy(&output.stdout).into_owned();
    found.push_str(&String::from_utf8_lossy(&output.stderr));
    let heading = format!("### Breaking changes against {baseline} (declared; LIB-QA-006)");
    writeln!(out, "{heading}\n{found}").map_err(output_error)?;
    if let Some(summary) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        let summary = Path::new(&summary);
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(summary)
            .and_then(|mut file| {
                let fence = if found.ends_with('\n') { "" } else { "\n" };
                write!(file, "{heading}\n\n```text\n{found}{fence}```\n")
            })
            .map_err(|e| format!("write {}: {e}", summary.display()))?;
    }
    Ok(())
}

/// The cargo that runs xtask, in `root`.
fn cargo(root: &Path) -> Command {
    let program = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(program);
    command.current_dir(root);
    command
}

/// The error for a cargo-semver-checks that cannot be started.
// Takes the error by value so that it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn could_not_run(error: std::io::Error) -> String {
    format!("could not run cargo semver-checks (is cargo-semver-checks installed?): {error}")
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))
}

/// The `[workspace.package] version` of the root `Cargo.toml`.
fn workspace_version(root: &Path) -> Result<String, String> {
    let path = root.join("Cargo.toml");
    let manifest: toml::Table = read(&path)?
        .parse()
        .map_err(|e| format!("parse {}: {e}", path.display()))?;
    manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{} has no [workspace.package] version", path.display()))
}

/// The tags `git tag --list v*` gives in `root`.
fn tags(root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["tag", "--list", "v*"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run git tag --list v*: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git tag --list v* {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::version;

    #[test]
    fn version_is_three_runs_of_digits() {
        assert_eq!(version("0.10.2"), Some((0, 10, 2)));
        // `u64::from_str` alone would take a leading `+`.
        for bad in [
            "+0.4.0", "0.+4.0", "0.4.+0", "", "0.4", "0.4.0.1", "0..0", "v0.4.0",
        ] {
            assert_eq!(version(bad), None, "{bad:?} is no X.Y.Z version");
        }
    }
}
