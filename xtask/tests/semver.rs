//! Acceptance tests for `cargo xtask semver` (LIB-QA-006, ADR 0046 D4).
//!
//! The command checks `sacm`'s public API against the newest release with
//! cargo-semver-checks. What it runs is decided by a pure function,
//! [`semver_plan`], from the CHANGELOG, the workspace version and the release
//! tags, and [`semver_args`] gives the exact arguments. Most tests drive both
//! with literal inputs. The last three run the xtask binary on a scratch git
//! repository, with a stub in place of cargo, so that what the command reads,
//! runs, reports and writes to the job summary is checked too; nothing here
//! runs cargo-semver-checks.
//!
//! The rules under test:
//!
//! - the baseline is the highest `vX.Y.Z` tag, compared numerically, without
//!   pre-releases;
//! - a break is declared when the `## [Unreleased]` section has a line
//!   exactly `### Breaking` followed by a `- ` entry before the next `### `
//!   or `## ` heading;
//! - a version other than the baseline lets the versions decide (no
//!   `--release-type`); an unchanged version runs at `major` before 1.0 when a
//!   break is declared, else at `minor`; from 1.0 a declared break needs the
//!   major bump.

use std::path::Path;

use xtask::semver::{ReleaseType, SemverPlan, semver_args, semver_plan};

/// The error for a declared break after 1.0 without the major bump.
const AFTER_1_0: &str =
    "a breaking change after 1.0 needs the major version bump in this pull request";

/// The tags of the releases so far, with the newest `v0.4.0`.
const TAGS: [&str; 4] = ["v0.1.0", "v0.2.0", "v0.3.0", "v0.4.0"];

/// A CHANGELOG whose `## [Unreleased]` section is `unreleased`. The released
/// section after it declares a break of its own, which must never count.
fn changelog(unreleased: &str) -> String {
    format!(
        "# Changelog\n\
         \n\
         All notable changes to this project are documented here. Before 1.0, a breaking\n\
         change bumps the minor version and is listed under **Breaking**.\n\
         \n\
         ## [Unreleased]\n\
         \n\
         {unreleased}\
         \n\
         ## [0.4.0] - 2026-11-26\n\
         \n\
         ### Breaking\n\
         \n\
         - an old break, already released\n\
         \n\
         ### Added\n\
         \n\
         - an old addition\n"
    )
}

/// An `[Unreleased]` section that declares a break.
const DECLARED: &str = "\
### Breaking

- `AddReference::href` is removed [LIB-API-025].

### Added

- `cargo xtask semver` [LIB-QA-006].
";

/// An `[Unreleased]` section that declares no break.
const UNDECLARED: &str = "\
### Added

- `cargo xtask semver` [LIB-QA-006].

### Fixed

- A **Breaking** word in an entry is not a heading.
";

fn plan(baseline: &str, release_type: Option<ReleaseType>, report_breaks: bool) -> SemverPlan {
    SemverPlan {
        baseline: baseline.to_owned(),
        release_type,
        report_breaks,
    }
}

/// The last two arguments: the release type, when there is one.
fn last_two(args: &[String]) -> &[String] {
    &args[args.len().saturating_sub(2)..]
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|&arg| arg.to_owned()).collect()
}

#[test]
fn lib_qa_006_the_baseline_is_the_newest_release_tag() {
    let undeclared = changelog(UNDECLARED);
    let result = semver_plan(
        &undeclared,
        "0.4.0",
        &["v0.1.0", "v0.10.0-rc.1", "v0.4.0", "v0.3.0", "x"],
    );
    assert_eq!(
        result.map(|plan| plan.baseline),
        Ok("0.4.0".to_owned()),
        "LIB-QA-006: the baseline is the highest vX.Y.Z tag, without its `v`; a pre-release \
         (v0.10.0-rc.1) and a tag that is no version (x) are not releases"
    );
    let result = semver_plan(&undeclared, "0.10.0", &["v0.9.0", "v0.10.0"]);
    assert_eq!(
        result.map(|plan| plan.baseline),
        Ok("0.10.0".to_owned()),
        "LIB-QA-006: tags compare numerically: v0.10.0 is newer than v0.9.0"
    );
    let result = semver_plan(&undeclared, "0.10.0", &["v0.10.0", "v0.9.0"]);
    assert_eq!(
        result.map(|plan| plan.baseline),
        Ok("0.10.0".to_owned()),
        "LIB-QA-006: the baseline does not depend on the order of the tags"
    );
    let result = semver_plan(
        &undeclared,
        "1.0.0",
        &["v0.4.0", "v1.0.0-rc.1", "v1.0", "v0.4.1"],
    );
    assert_eq!(
        result.map(|plan| plan.baseline),
        Ok("0.4.1".to_owned()),
        "LIB-QA-006: v1.0.0-rc.1 and v1.0 are not release tags; v0.4.1 is the newest"
    );
}

#[test]
fn lib_qa_006_before_1_0_a_declared_break_runs_at_major_and_is_reported() {
    assert_eq!(
        semver_plan(&changelog(DECLARED), "0.4.0", &TAGS),
        Ok(plan("0.4.0", Some(ReleaseType::Major), true)),
        "LIB-QA-006: before 1.0, an unchanged version with a break listed under \
         [Unreleased] ### Breaking runs at `major` and reports the breaks"
    );
    // The heading need not be the section's first.
    let later = "### Added\n\n- something\n\n### Breaking\n\n- a break\n";
    assert_eq!(
        semver_plan(&changelog(later), "0.4.0", &TAGS),
        Ok(plan("0.4.0", Some(ReleaseType::Major), true)),
        "LIB-QA-006: a ### Breaking subsection after another one declares the break"
    );
    assert_eq!(
        last_two(&semver_args("0.4.0", Some(ReleaseType::Major))),
        strings(&["--release-type", "major"]),
        "LIB-QA-006: a declared break runs cargo-semver-checks with `--release-type major`"
    );
}

#[test]
fn lib_qa_006_before_1_0_an_undeclared_break_fails_at_minor() {
    assert_eq!(
        semver_plan(&changelog(UNDECLARED), "0.4.0", &TAGS),
        Ok(plan("0.4.0", Some(ReleaseType::Minor), false)),
        "LIB-QA-006: before 1.0, an unchanged version without a declared break runs at \
         `minor` (so a break fails), and nothing is reported; the released section's \
         ### Breaking does not count"
    );
    assert_eq!(
        semver_plan(&changelog(""), "0.4.0", &TAGS),
        Ok(plan("0.4.0", Some(ReleaseType::Minor), false)),
        "LIB-QA-006: an empty [Unreleased] section declares no break"
    );
    assert_eq!(
        last_two(&semver_args("0.4.0", Some(ReleaseType::Minor))),
        strings(&["--release-type", "minor"]),
        "LIB-QA-006: an undeclared break runs cargo-semver-checks with `--release-type minor`"
    );
}

#[test]
fn lib_qa_006_a_breaking_heading_with_no_entry_declares_nothing() {
    for (what, unreleased) in [
        (
            "followed by another subsection",
            "### Breaking\n\n### Added\n\n- an addition\n",
        ),
        (
            "at the end of the section, before the released entries",
            "### Added\n\n- an addition\n\n### Breaking\n\n",
        ),
        (
            "with prose but no `- ` entry",
            "### Breaking\n\nNone in this release.\n",
        ),
        (
            "named otherwise (`### Breaking changes`)",
            "### Breaking changes\n\n- a break\n",
        ),
        (
            "at another level (`#### Breaking`)",
            "#### Breaking\n\n- a break\n",
        ),
    ] {
        assert_eq!(
            semver_plan(&changelog(unreleased), "0.4.0", &TAGS),
            Ok(plan("0.4.0", Some(ReleaseType::Minor), false)),
            "LIB-QA-006: a breaking heading {what} declares nothing:\n{unreleased}"
        );
    }
}

#[test]
fn lib_qa_006_a_version_bump_lets_the_versions_decide() {
    for (baseline, version, breaking) in [
        ("0.4.0", "0.5.0", true),
        ("0.4.0", "0.4.1", false),
        ("0.4.0", "1.0.0", true),
        ("1.2.3", "1.2.4", false),
        ("1.2.3", "1.3.0", false),
        ("1.2.3", "2.0.0", true),
    ] {
        let tags = [format!("v{baseline}")];
        let tags: Vec<&str> = tags.iter().map(String::as_str).collect();
        for unreleased in [DECLARED, UNDECLARED] {
            assert_eq!(
                semver_plan(&changelog(unreleased), version, &tags),
                Ok(plan(baseline, None, breaking)),
                "LIB-QA-006: {baseline} -> {version}: the versions decide (no release type), \
                 and the breaks are reported only for a breaking bump (0.x minor, or major), \
                 declared or not"
            );
        }
    }
    assert_eq!(
        semver_args("0.4.0", None),
        strings(&[
            "semver-checks",
            "--package",
            "sacm",
            "--baseline-version",
            "0.4.0",
            "--only-explicit-features",
            "--features",
            "serde",
        ]),
        "LIB-QA-006: when the versions decide, no `--release-type` is passed"
    );
}

#[test]
fn lib_qa_006_from_1_0_a_declared_break_needs_the_major_bump() {
    let tags = ["v0.4.0", "v1.0.0"];
    assert_eq!(
        semver_plan(&changelog(DECLARED), "1.0.0", &tags),
        Err(AFTER_1_0.to_owned()),
        "LIB-QA-006: from 1.0, a declared break with the version unchanged is an error"
    );
    assert_eq!(
        semver_plan(&changelog(UNDECLARED), "1.0.0", &tags),
        Ok(plan("1.0.0", Some(ReleaseType::Minor), false)),
        "LIB-QA-006: from 1.0, an unchanged version without a declared break runs at `minor`"
    );
    assert_eq!(
        semver_plan(&changelog(DECLARED), "2.0.0", &tags),
        Ok(plan("1.0.0", None, true)),
        "LIB-QA-006: from 1.0, the major bump lets the versions decide and reports the breaks"
    );
}

#[test]
fn lib_qa_006_semver_checks_sacm_with_the_serde_feature_only() {
    let base = [
        "semver-checks",
        "--package",
        "sacm",
        "--baseline-version",
        "0.4.0",
        "--only-explicit-features",
        "--features",
        "serde",
    ];
    assert_eq!(
        semver_args("0.4.0", None),
        strings(&base),
        "LIB-QA-006: sacm against the baseline from crates.io, the serde feature only \
         (ADR 0045 D7)"
    );
    for (release_type, name) in [(ReleaseType::Major, "major"), (ReleaseType::Minor, "minor")] {
        assert_eq!(
            release_type.as_str(),
            name,
            "LIB-QA-006: the release type's cargo-semver-checks name"
        );
        let expected: Vec<&str> = base
            .iter()
            .copied()
            .chain(["--release-type", name])
            .collect();
        assert_eq!(
            semver_args("0.4.0", Some(release_type)),
            strings(&expected),
            "LIB-QA-006: `--release-type {name}` follows the features"
        );
    }
    assert_eq!(
        semver_args("1.2.3", None)[4],
        "1.2.3",
        "LIB-QA-006: the baseline version is passed as given"
    );
}

#[test]
fn lib_qa_006_errors_name_what_is_missing() {
    // In order: the [Unreleased] line comes first, even without a tag.
    let without_unreleased = changelog(DECLARED).replace("## [Unreleased]\n", "");
    let error = semver_plan(&without_unreleased, "0.4.0", &[])
        .expect_err("LIB-QA-006: a CHANGELOG without `## [Unreleased]` is an error");
    assert!(
        error.contains("## [Unreleased]"),
        "LIB-QA-006: the error names the missing `## [Unreleased]` line, got {error:?}"
    );
    let lookalike = changelog(DECLARED).replace("## [Unreleased]\n", "## Unreleased\n");
    let error = semver_plan(&lookalike, "0.4.0", &TAGS)
        .expect_err("LIB-QA-006: `## Unreleased` is not the `## [Unreleased]` line");
    assert!(
        error.contains("## [Unreleased]"),
        "LIB-QA-006: the error names the missing `## [Unreleased]` line, got {error:?}"
    );

    // Then a release tag, before the version is looked at.
    for tags in [&[][..], &["x", "v1.0", "v1.0.0-rc.1", "0.4.0"][..]] {
        let error = semver_plan(&changelog(UNDECLARED), "0.5", tags)
            .expect_err("LIB-QA-006: no release tag is an error");
        assert!(
            error.contains("tag") && error.contains("vX.Y.Z"),
            "LIB-QA-006: with tags {tags:?}, the error names the missing `vX.Y.Z` tag, \
             got {error:?}"
        );
    }

    // Then the workspace version.
    for version in ["0.5", "", "v0.5.0", "0.5.0.1", "0.x.0"] {
        let error = semver_plan(&changelog(UNDECLARED), version, &TAGS)
            .expect_err("LIB-QA-006: a workspace version that is no X.Y.Z is an error");
        assert!(
            error.contains("version") && error.contains("X.Y.Z") && error.contains(version),
            "LIB-QA-006: the error names the workspace version {version:?}, got {error:?}"
        );
    }

    // Then a version below the baseline, even with a declared break from 1.0.
    let error = semver_plan(&changelog(DECLARED), "1.1.0", &["v1.2.0"])
        .expect_err("LIB-QA-006: a workspace version below the baseline is an error");
    assert!(
        error.contains("1.1.0") && error.contains("1.2.0"),
        "LIB-QA-006: the error names the version and the baseline, got {error:?}"
    );
    assert_ne!(
        error, AFTER_1_0,
        "LIB-QA-006: below the baseline is reported before the 1.0 rule"
    );

    // The command reads CHANGELOG.md first, and says so when it is missing.
    let root = Path::new("no-such-tree-lib-qa-006");
    let error = xtask::run(&["semver"], root, &mut Vec::new())
        .expect_err("LIB-QA-006: `cargo xtask semver` without a CHANGELOG.md fails");
    assert!(
        error.contains("CHANGELOG.md") && !error.starts_with("unrecognized arguments"),
        "LIB-QA-006: `cargo xtask semver` is a command, and its error names CHANGELOG.md, \
         got {error:?}"
    );
}

#[test]
fn lib_qa_006_cargo_xtask_help_lists_the_semver_command() {
    let mut out = Vec::new();
    xtask::run(&["help"], Path::new("."), &mut out).expect("LIB-QA-006: `cargo xtask help`");
    let usage = String::from_utf8_lossy(&out);
    assert!(
        usage.lines().any(|line| line
            == "  semver                  check sacm's public API against the last release \
                (cargo-semver-checks)"),
        "LIB-QA-006: the usage lists `semver`:\n{usage}"
    );
}

// ---------------------------------------------------------------------------
// `cargo xtask semver` end to end, with a stub for cargo
// ---------------------------------------------------------------------------
//
// The xtask binary runs on a scratch git repository (`XTASK_ROOT`) with its
// own CHANGELOG.md, Cargo.toml and tags. `CARGO` names a stub script that
// records the arguments of each call in `STUB_LOG`. It answers
// `semver-checks --version` with `STUB_VERSION_STATUS`; any other call prints
// `STUB_REPORT` and exits with `STUB_MINOR_STATUS` when its last argument is
// `minor`, else with `STUB_STATUS`. `GITHUB_STEP_SUMMARY` names a scratch
// file. The stub is a batch file on Windows and a POSIX shell script
// elsewhere.

/// What the stub prints for a check.
const STUB_REPORT: &str = "stub: a break was found";

/// The stub for cargo on Windows.
#[cfg(windows)]
const STUB: (&str, &str) = (
    "cargo-stub.cmd",
    "@echo off\r\n\
     >>\"%STUB_LOG%\" echo %*\r\n\
     if \"%~2\"==\"--version\" exit /b %STUB_VERSION_STATUS%\r\n\
     echo stub: a break was found\r\n\
     set \"last=\"\r\n\
     :next\r\n\
     if \"%~1\"==\"\" goto done\r\n\
     set \"last=%~1\"\r\n\
     shift\r\n\
     goto next\r\n\
     :done\r\n\
     if \"%last%\"==\"minor\" exit /b %STUB_MINOR_STATUS%\r\n\
     exit /b %STUB_STATUS%\r\n",
);

/// The stub for cargo elsewhere.
#[cfg(not(windows))]
const STUB: (&str, &str) = (
    "cargo-stub",
    "#!/bin/sh\n\
     echo \"$*\" >> \"$STUB_LOG\"\n\
     if [ \"$2\" = \"--version\" ]; then exit \"$STUB_VERSION_STATUS\"; fi\n\
     echo \"stub: a break was found\"\n\
     last=\n\
     for last; do :; done\n\
     if [ \"$last\" = minor ]; then exit \"$STUB_MINOR_STATUS\"; fi\n\
     exit \"$STUB_STATUS\"\n",
);

/// The exit statuses the stub gives: `semver-checks --version`, a check at
/// `minor`, and any other check.
#[derive(Clone, Copy)]
struct Statuses {
    version: u8,
    minor: u8,
    other: u8,
}

/// Every call succeeds.
const ALL_PASS: Statuses = Statuses {
    version: 0,
    minor: 0,
    other: 0,
};

/// A scratch repository and the stub, removed on drop.
struct Scratch {
    dir: std::path::PathBuf,
}

impl Scratch {
    /// A git repository with `changelog`, a workspace at `version` and one
    /// commit carrying `tags`.
    fn new(test: &str, changelog: &str, version: &str, tags: &[&str]) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("xtask-{test}-{}-{nanos}", std::process::id()));
        let scratch = Self { dir };
        let repo = scratch.repo();
        write(&repo.join("CHANGELOG.md"), changelog);
        write(
            &repo.join("Cargo.toml"),
            &format!("[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"{version}\"\n"),
        );
        git(&repo, &["init", "--quiet"]);
        git(
            &repo,
            &["commit", "--allow-empty", "--quiet", "-m", "release"],
        );
        for tag in tags {
            git(&repo, &["tag", tag]);
        }
        let stub = scratch.stub();
        write(&stub, STUB.1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|e| panic!("LIB-QA-006: chmod {}: {e}", stub.display()));
        }
        scratch
    }

    fn repo(&self) -> std::path::PathBuf {
        self.dir.join("repo")
    }

    fn stub(&self) -> std::path::PathBuf {
        self.dir.join(STUB.0)
    }

    fn summary(&self) -> std::path::PathBuf {
        self.dir.join("summary.md")
    }

    fn log(&self) -> std::path::PathBuf {
        self.dir.join("calls.log")
    }

    /// Run `xtask semver` on the repository with the stub as cargo.
    fn run(&self, statuses: Statuses) -> Run {
        self.run_with_cargo(&self.stub(), statuses)
    }

    /// Run `xtask semver` on the repository with `cargo` as cargo.
    fn run_with_cargo(&self, cargo: &Path, statuses: Statuses) -> Run {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_xtask"))
            .arg("semver")
            .env("XTASK_ROOT", self.repo())
            .env("CARGO", cargo)
            .env("GITHUB_STEP_SUMMARY", self.summary())
            .env("STUB_LOG", self.log())
            .env("STUB_VERSION_STATUS", statuses.version.to_string())
            .env("STUB_MINOR_STATUS", statuses.minor.to_string())
            .env("STUB_STATUS", statuses.other.to_string())
            .output()
            .expect("LIB-QA-006: the xtask binary can be started");
        let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).replace("\r\n", "\n");
        Run {
            success: output.status.success(),
            stdout: text(&output.stdout),
            stderr: text(&output.stderr),
            calls: std::fs::read_to_string(self.log())
                .unwrap_or_default()
                .lines()
                .map(|line| line.trim().to_owned())
                .filter(|line| !line.is_empty())
                .collect(),
            summary: std::fs::read_to_string(self.summary())
                .unwrap_or_default()
                .replace("\r\n", "\n"),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// What a run of `xtask semver` did.
#[derive(Debug)]
struct Run {
    success: bool,
    stdout: String,
    stderr: String,
    /// The arguments of each call of the stub, in order.
    calls: Vec<String>,
    /// The job summary, empty when nothing was written.
    summary: String,
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("LIB-QA-006: create {}: {e}", parent.display()));
    }
    std::fs::write(path, text)
        .unwrap_or_else(|e| panic!("LIB-QA-006: write {}: {e}", path.display()));
}

/// Run git in `repo`, with an identity and without signing.
fn git(repo: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=xtask",
            "-c",
            "user.email=xtask@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(repo)
        .status()
        .unwrap_or_else(|e| panic!("LIB-QA-006: run git {args:?}: {e}"));
    assert!(status.success(), "LIB-QA-006: git {args:?}: {status}");
}

/// The stub's record of a check of `sacm` against `baseline`.
fn check_call(baseline: &str, release_type: Option<ReleaseType>) -> String {
    semver_args(baseline, release_type).join(" ")
}

/// The stub's record of `cargo semver-checks --version`.
const VERSION_CALL: &str = "semver-checks --version";

/// The plan line `cargo xtask semver` writes first.
fn plan_line(version: &str, baseline: &str, release_type: &str) -> String {
    format!(
        "semver: sacm {version} against {baseline} (crates.io), features: serde only, \
         release type: {release_type}"
    )
}

#[test]
fn lib_qa_006_cargo_xtask_semver_reports_a_declared_break_in_the_job_summary() {
    let scratch = Scratch::new(
        "lib-qa-006-declared",
        &changelog(DECLARED),
        "0.4.0",
        &["v0.3.0", "v0.4.0", "v0.10.0-rc.1"],
    );
    // The check at `minor` fails: the declared break is there.
    let run = scratch.run(Statuses {
        minor: 1,
        ..ALL_PASS
    });

    assert!(
        run.success,
        "LIB-QA-006: a declared break before 1.0 passes at `major`, and the reporting run's \
         status is ignored: {run:#?}"
    );
    assert_eq!(
        run.stdout.lines().next(),
        Some(plan_line("0.4.0", "0.4.0", "major").as_str()),
        "LIB-QA-006: the plan line names the workspace version from Cargo.toml and the \
         newest release tag: {run:#?}"
    );
    assert_eq!(
        run.calls,
        [
            VERSION_CALL.to_owned(),
            check_call("0.4.0", Some(ReleaseType::Major)),
            check_call("0.4.0", Some(ReleaseType::Minor)),
        ],
        "LIB-QA-006: the tool is looked for, the check runs at `major`, then again at \
         `minor` to list the breaks: {run:#?}"
    );
    let heading = "### Breaking changes against 0.4.0 (declared; LIB-QA-006)";
    assert!(
        run.stdout.contains(&format!("{heading}\n{STUB_REPORT}\n")),
        "LIB-QA-006: the breaks found at `minor` are written under their heading: {run:#?}"
    );
    assert_eq!(
        run.summary,
        format!("{heading}\n\n```text\n{STUB_REPORT}\n```\n"),
        "LIB-QA-006: the declared breaks are appended to the job summary in a text block"
    );
}

#[test]
fn lib_qa_006_cargo_xtask_semver_names_the_check_that_failed() {
    let failing = Statuses {
        minor: 1,
        other: 1,
        ..ALL_PASS
    };
    for (what, unreleased, version, tags, release_type, error) in [
        (
            "an undeclared break before 1.0",
            UNDECLARED,
            "0.4.0",
            &["v0.3.0", "v0.4.0"][..],
            "minor",
            "a breaking change CHANGELOG.md does not list under [Unreleased] ### Breaking",
        ),
        (
            "an undeclared break from 1.0",
            UNDECLARED,
            "1.0.0",
            &["v0.4.0", "v1.0.0"][..],
            "minor",
            AFTER_1_0,
        ),
        (
            "a break larger than the version bump",
            UNDECLARED,
            "0.5.0",
            &["v0.4.0"][..],
            "from the versions",
            "the change needs a larger version bump than 0.4.0 → 0.5.0",
        ),
        (
            "a failed check at `major`",
            DECLARED,
            "0.4.0",
            &["v0.4.0"][..],
            "major",
            "cargo semver-checks exit",
        ),
    ] {
        let scratch = Scratch::new("lib-qa-006-failed", &changelog(unreleased), version, tags);
        let run = scratch.run(failing);
        let baseline = tags[tags.len() - 1].trim_start_matches('v');
        assert!(
            !run.success,
            "LIB-QA-006: {what} fails the command: {run:#?}"
        );
        assert_eq!(
            run.stdout.lines().next(),
            Some(plan_line(version, baseline, release_type).as_str()),
            "LIB-QA-006: {what}: the plan line: {run:#?}"
        );
        let first = run.stderr.lines().next().unwrap_or_default();
        assert!(
            first.starts_with("xtask: ") && first.contains(error),
            "LIB-QA-006: {what} is reported as `{error}`: {run:#?}"
        );
        if error == AFTER_1_0 {
            assert_eq!(
                first,
                format!("xtask: {AFTER_1_0}"),
                "LIB-QA-006: {what}: the whole message"
            );
        }
        let release_type = match release_type {
            "minor" => Some(ReleaseType::Minor),
            "major" => Some(ReleaseType::Major),
            _ => None,
        };
        assert_eq!(
            run.calls,
            [VERSION_CALL.to_owned(), check_call(baseline, release_type)],
            "LIB-QA-006: {what}: a failed check is not run again to report: {run:#?}"
        );
        assert_eq!(
            run.summary, "",
            "LIB-QA-006: {what}: nothing is written to the job summary"
        );
    }
}

#[test]
fn lib_qa_006_cargo_xtask_semver_says_when_cargo_semver_checks_is_missing() {
    let scratch = Scratch::new(
        "lib-qa-006-missing",
        &changelog(DECLARED),
        "0.4.0",
        &["v0.4.0"],
    );

    let run = scratch.run(Statuses {
        version: 101,
        ..ALL_PASS
    });
    assert!(
        !run.success
            && run.stderr.lines().next()
                == Some(
                    "xtask: cargo-semver-checks is not installed: `cargo semver-checks --version` \
                     failed"
                ),
        "LIB-QA-006: cargo without cargo-semver-checks is reported as such, not as a break: \
         {run:#?}"
    );
    assert_eq!(
        run.calls,
        [VERSION_CALL],
        "LIB-QA-006: no check runs without the tool"
    );

    let run = scratch.run_with_cargo(&scratch.dir.join("no-such-cargo"), ALL_PASS);
    assert!(
        !run.success
            && run.stderr.starts_with(
                "xtask: could not run cargo semver-checks (is cargo-semver-checks installed?): "
            ),
        "LIB-QA-006: a cargo that cannot be started is reported: {run:#?}"
    );
    assert_eq!(
        run.summary, "",
        "LIB-QA-006: nothing is written to the job summary"
    );
}
