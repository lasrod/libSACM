//! Acceptance tests for `cargo xtask semver` (LIB-QA-006, ADR 0046 D4).
//!
//! The command checks `sacm`'s public API against the newest release with
//! cargo-semver-checks. What it runs is decided by a pure function,
//! [`semver_plan`], from the CHANGELOG, the workspace version and the release
//! tags, and [`semver_args`] gives the exact arguments. These tests drive
//! both with literal inputs; nothing here runs cargo-semver-checks or git.
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
