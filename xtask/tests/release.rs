//! Repository checks for the release process (LIB-QA-005, slice 0059).
//!
//! Four checks keep the published packages and the version bookkeeping fit
//! for a release:
//!
//! - each package that is published (`sacm`, `sacm-cli`) holds its
//!   `Cargo.toml`, `README.md`, `LICENSE` and its `src/`, plus the files
//!   cargo adds, and no tests, benches or examples (`cargo package --list`);
//! - each package's `LICENSE` is the repository's, byte for byte, and each
//!   manifest's `license` is `MIT`;
//! - the workspace version, the version of its `sacm` dependency and the
//!   first section of `CHANGELOG.md` agree;
//! - docs.rs builds `sacm` with the `serde` feature only, not `test-support`
//!   (slice 0060).
//!
//! The checks read the real files and run the cargo the tests run under;
//! nothing on disk is changed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The packages `cargo publish` publishes, in publish order.
const PUBLISHED: [&str; 2] = ["sacm", "sacm-cli"];

/// The files every published package must hold, besides its `src/`.
const REQUIRED: [&str; 3] = ["Cargo.toml", "README.md", "LICENSE"];

/// The files `cargo package` adds to a package by itself.
const CARGO_ADDED: [&str; 3] = ["Cargo.toml.orig", "Cargo.lock", ".cargo_vcs_info.json"];

/// The directories no published package may hold anything under.
const FORBIDDEN_DIRS: [&str; 3] = ["tests/", "benches/", "examples/"];

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the xtask manifest directory has the repository root as its parent")
}

/// The directory of the published package `name`.
fn package_dir(name: &str) -> PathBuf {
    repository_root().join("crates").join(name)
}

fn read(rel: &str) -> String {
    fs::read_to_string(repository_root().join(rel))
        .unwrap_or_else(|e| panic!("LIB-QA-005: read {rel}: {e}"))
}

fn toml_table(rel: &str) -> toml::Table {
    read(rel)
        .parse()
        .unwrap_or_else(|e| panic!("LIB-QA-005: parse {rel}: {e}"))
}

/// The value at the dotted `path` of `table`, if every step exists.
fn lookup<'a>(table: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Value> {
    let (first, rest) = path.split_first()?;
    let mut value = table.get(*first)?;
    for key in rest {
        value = value.get(*key)?;
    }
    Some(value)
}

/// The cargo these tests run under, or `cargo` from `PATH`.
fn cargo() -> PathBuf {
    std::env::var_os("CARGO").map_or_else(|| PathBuf::from("cargo"), PathBuf::from)
}

/// The `/`-separated paths `cargo package --list` gives for `package`.
fn package_list(package: &str) -> Vec<String> {
    let output = Command::new(cargo())
        .current_dir(repository_root())
        .args([
            "package",
            "--list",
            "--allow-dirty",
            "--offline",
            "-p",
            package,
        ])
        .output()
        .unwrap_or_else(|e| panic!("LIB-QA-005: run cargo package --list -p {package}: {e}"));
    assert!(
        output.status.success(),
        "LIB-QA-005: cargo package --list -p {package} failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap_or_else(|e| panic!("LIB-QA-005: cargo package --list -p {package}: {e}"))
        .lines()
        .map(|line| line.trim().replace('\\', "/"))
        .filter(|line| !line.is_empty())
        .collect()
}

/// What is wrong with the package `package`, given the paths it holds: one
/// line per missing required file and per entry that does not belong.
fn package_problems(package: &str, paths: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    for required in REQUIRED {
        if !paths.iter().any(|path| path == required) {
            problems.push(format!(
                "LIB-QA-005: package `{package}` lacks `{required}`"
            ));
        }
    }
    if !paths.iter().any(|path| path.starts_with("src/")) {
        problems.push(format!(
            "LIB-QA-005: package `{package}` holds nothing under `src/`"
        ));
    }
    for path in paths {
        if let Some(dir) = FORBIDDEN_DIRS.iter().find(|dir| path.starts_with(**dir)) {
            problems.push(format!(
                "LIB-QA-005: package `{package}` holds `{path}`, which is under `{dir}`"
            ));
        } else if !path.starts_with("src/")
            && !REQUIRED.contains(&path.as_str())
            && !CARGO_ADDED.contains(&path.as_str())
        {
            problems.push(format!(
                "LIB-QA-005: package `{package}` holds `{path}`, which is neither under \
                 `src/`, nor one of {REQUIRED:?}, nor a file cargo adds ({CARGO_ADDED:?})"
            ));
        }
    }
    problems
}

#[test]
fn lib_qa_005_each_package_holds_its_sources_readme_and_license_only() {
    let mut problems = Vec::new();
    for package in PUBLISHED {
        let paths = package_list(package);
        assert!(
            !paths.is_empty(),
            "LIB-QA-005: cargo package --list -p {package} listed no file"
        );
        problems.extend(package_problems(package, &paths));
    }
    assert!(
        problems.is_empty(),
        "LIB-QA-005: the published packages hold the wrong files:\n{}",
        problems.join("\n")
    );
}

#[test]
fn lib_qa_005_each_package_license_is_the_repository_license() {
    let root_license = fs::read(repository_root().join("LICENSE"))
        .unwrap_or_else(|e| panic!("LIB-QA-005: read the repository's LICENSE: {e}"));
    assert!(
        root_license.starts_with(b"MIT License"),
        "LIB-QA-005: the repository's LICENSE is not the MIT License"
    );
    let workspace = toml_table("Cargo.toml");
    let workspace_license =
        lookup(&workspace, &["workspace", "package", "license"]).and_then(toml::Value::as_str);

    let mut problems = Vec::new();
    for package in PUBLISHED {
        let license_path = package_dir(package).join("LICENSE");
        match fs::read(&license_path) {
            Ok(bytes) if bytes == root_license => {}
            Ok(_) => problems.push(format!(
                "LIB-QA-005: `crates/{package}/LICENSE` differs from the repository's LICENSE"
            )),
            Err(e) => problems.push(format!(
                "LIB-QA-005: `crates/{package}/LICENSE` cannot be read: {e}"
            )),
        }

        let manifest = toml_table(&format!("crates/{package}/Cargo.toml"));
        let license = match lookup(&manifest, &["package", "license"]) {
            Some(toml::Value::String(license)) => Some(license.as_str()),
            Some(value) if value.get("workspace").and_then(toml::Value::as_bool) == Some(true) => {
                workspace_license
            }
            _ => None,
        };
        if license != Some("MIT") {
            problems.push(format!(
                "LIB-QA-005: `crates/{package}/Cargo.toml` has license {license:?}, not \"MIT\""
            ));
        }
        if lookup(&manifest, &["package", "license-file"]).is_some() {
            problems.push(format!(
                "LIB-QA-005: `crates/{package}/Cargo.toml` sets `license-file` as well as \
                 `license`"
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "LIB-QA-005: the packages' licenses are not the repository's:\n{}",
        problems.join("\n")
    );
}

#[test]
fn lib_qa_005_docs_rs_builds_without_test_support() {
    // docs.rs builds `sacm` with the `serde` feature only, so the unstable
    // `test-support` API does not appear there (the maintainer, 2026-11-17).
    let manifest = toml_table("crates/sacm/Cargo.toml");
    let Some(docs_rs) = lookup(&manifest, &["package", "metadata", "docs", "rs"]) else {
        panic!("LIB-QA-005: `crates/sacm/Cargo.toml` has no `[package.metadata.docs.rs]`");
    };
    let features: Option<Vec<&str>> = docs_rs
        .get("features")
        .and_then(toml::Value::as_array)
        .map(|features| features.iter().filter_map(toml::Value::as_str).collect());
    assert_eq!(
        features,
        Some(vec!["serde"]),
        "LIB-QA-005: `[package.metadata.docs.rs] features` is not [\"serde\"]"
    );
    assert_ne!(
        docs_rs.get("all-features").and_then(toml::Value::as_bool),
        Some(true),
        "LIB-QA-005: `[package.metadata.docs.rs]` sets `all-features = true`, which builds \
         `test-support`"
    );
    let text = docs_rs.to_string();
    assert!(
        !text.contains("test-support"),
        "LIB-QA-005: `[package.metadata.docs.rs]` names `test-support`:\n{text}"
    );
}

/// Whether `date` is an ISO 8601 calendar date `YYYY-MM-DD`, a day that
/// exists in the Gregorian calendar.
fn is_iso_date(date: &str) -> bool {
    let parts: Vec<&str> = date.split('-').collect();
    let [year, month, day] = parts.as_slice() else {
        return false;
    };
    let digits = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    if !(digits(year, 4) && digits(month, 2) && digits(day, 2)) {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) =
        (year.parse::<u16>(), month.parse::<u8>(), day.parse::<u8>())
    else {
        return false;
    };
    // The Gregorian calendar of ISO 8601: February has 29 days in a year
    // divisible by 4, except a century not divisible by 400.
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

#[test]
fn lib_qa_005_the_versions_and_the_changelog_agree() {
    let workspace = toml_table("Cargo.toml");
    let version = lookup(&workspace, &["workspace", "package", "version"])
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| {
            panic!("LIB-QA-005: Cargo.toml has no string `[workspace.package] version`")
        });
    let dependency = lookup(
        &workspace,
        &["workspace", "dependencies", "sacm", "version"],
    )
    .and_then(toml::Value::as_str);
    assert_eq!(
        dependency,
        Some(version),
        "LIB-QA-005: `[workspace.dependencies] sacm` has version {dependency:?}, but \
         `[workspace.package] version` is {version:?}"
    );

    for package in PUBLISHED {
        let manifest = toml_table(&format!("crates/{package}/Cargo.toml"));
        let inherits =
            lookup(&manifest, &["package", "version", "workspace"]).and_then(toml::Value::as_bool);
        assert_eq!(
            inherits,
            Some(true),
            "LIB-QA-005: `crates/{package}/Cargo.toml` does not take its version from the \
             workspace (`version.workspace = true`)"
        );
    }

    let changelog = read("CHANGELOG.md");
    let first = changelog
        .lines()
        .find(|line| line.starts_with("## "))
        .unwrap_or_else(|| panic!("LIB-QA-005: CHANGELOG.md has no `## ` section"));
    let released = first
        .strip_prefix(&format!("## [{version}] - "))
        .is_some_and(is_iso_date);
    assert!(
        first == "## [Unreleased]" || released,
        "LIB-QA-005: CHANGELOG.md's first section is {first:?}, neither `## [Unreleased]` \
         nor `## [{version}] - YYYY-MM-DD`"
    );
    let latest = changelog
        .lines()
        .filter(|line| line.starts_with("## "))
        .find(|line| *line != "## [Unreleased]");
    if let Some(latest) = latest {
        assert!(
            latest
                .strip_prefix(&format!("## [{version}] - "))
                .is_some_and(is_iso_date),
            "LIB-QA-005: CHANGELOG.md's latest release section is {latest:?}, not \
             `## [{version}] - YYYY-MM-DD`"
        );
    }
}

#[test]
fn lib_qa_005_the_changelog_date_check_accepts_only_iso_dates() {
    for good in [
        "2026-11-17",
        "2027-01-01",
        "2026-12-31",
        "2028-02-29",
        "2000-02-29",
        "2026-04-30",
    ] {
        assert!(is_iso_date(good), "LIB-QA-005: {good} is an ISO date");
    }
    for bad in [
        "2026-02-31",
        "2026-04-31",
        "2027-02-29",
        "2100-02-29",
        "2026-02-00",
        "",
        "2026-11",
        "2026-13-01",
        "2026-00-10",
        "2026-11-32",
        "26-11-17",
        "2026-1-17",
        "2026-11-17 ",
        "2026/11/17",
        "2026-11-1a",
        "2026-11-17-01",
    ] {
        assert!(!is_iso_date(bad), "LIB-QA-005: {bad:?} is not an ISO date");
    }
}

#[test]
fn lib_qa_005_the_package_check_reports_each_offending_path() {
    let good: Vec<String> = [
        ".cargo_vcs_info.json",
        "Cargo.lock",
        "Cargo.toml",
        "Cargo.toml.orig",
        "LICENSE",
        "README.md",
        "src/lib.rs",
    ]
    .map(str::to_owned)
    .to_vec();
    assert_eq!(
        package_problems("sacm", &good),
        Vec::<String>::new(),
        "LIB-QA-005: a package of its sources, README, LICENSE and cargo's files passes"
    );

    let mut bad = good.clone();
    bad.retain(|path| path != "LICENSE");
    bad.extend(
        [
            "tests/data/a.sacm.xmi",
            "benches/b.rs",
            "examples/c.rs",
            "deny.toml",
        ]
        .map(str::to_owned),
    );
    let problems = package_problems("sacm-cli", &bad);
    for offending in [
        "lacks `LICENSE`",
        "`tests/data/a.sacm.xmi`, which is under `tests/`",
        "`benches/b.rs`, which is under `benches/`",
        "`examples/c.rs`, which is under `examples/`",
        "`deny.toml`, which is neither",
    ] {
        assert!(
            problems
                .iter()
                .any(|p| p.contains("`sacm-cli`") && p.contains(offending)),
            "LIB-QA-005: no problem names `sacm-cli` and {offending}: {problems:#?}"
        );
    }
    assert_eq!(
        problems.len(),
        5,
        "LIB-QA-005: one problem per offending path: {problems:#?}"
    );
}

/// The `major.minor.patch` triple of the version `version`; a pre-release or
/// build suffix after the patch number is ignored.
fn version_triple(version: &str) -> Option<(u64, u64, u64)> {
    let core = version.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((major, minor, patch))
}

#[test]
fn lib_qa_019_the_changelog_keeps_the_dated_0_1_0_section() {
    const SECTION: &str = "## [0.1.0] - 2026-11-17";
    let changelog = read("CHANGELOG.md");
    let lines: Vec<&str> = changelog.lines().collect();

    let sections: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == SECTION)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        sections.len(),
        1,
        "LIB-QA-019: CHANGELOG.md must hold the line {SECTION:?} exactly once, as published; \
         it holds it on lines {:?}",
        sections.iter().map(|index| index + 1).collect::<Vec<_>>()
    );

    let unreleased = lines.iter().position(|line| *line == "## [Unreleased]");
    assert!(
        unreleased.is_some_and(|unreleased| unreleased < sections[0]),
        "LIB-QA-019: {SECTION:?} (line {}) must come after the first `## [Unreleased]` \
         (line {:?})",
        sections[0] + 1,
        unreleased.map(|index| index + 1)
    );

    let workspace = toml_table("Cargo.toml");
    let version = lookup(&workspace, &["workspace", "package", "version"])
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| {
            panic!("LIB-QA-019: Cargo.toml has no string `[workspace.package] version`")
        });
    let triple = version_triple(version).unwrap_or_else(|| {
        panic!("LIB-QA-019: `[workspace.package] version` {version:?} is not `major.minor.patch`")
    });
    assert!(
        triple >= (0, 1, 0),
        "LIB-QA-019: `[workspace.package] version` {version:?} is older than the published 0.1.0"
    );
}

#[test]
fn lib_qa_019_the_version_triple_compares_numerically() {
    assert_eq!(
        version_triple("0.1.0"),
        Some((0, 1, 0)),
        "LIB-QA-019: 0.1.0"
    );
    assert_eq!(
        version_triple("0.10.2"),
        Some((0, 10, 2)),
        "LIB-QA-019: 0.10.2"
    );
    assert_eq!(
        version_triple("1.0.0-rc.1"),
        Some((1, 0, 0)),
        "LIB-QA-019: 1.0.0-rc.1"
    );
    assert!(
        version_triple("0.10.0") > version_triple("0.9.9"),
        "LIB-QA-019: 0.10.0 is later than 0.9.9, which a string comparison gets wrong"
    );
    for bad in ["", "0.1", "0.1.0.0", "0.x.0", "v0.1.0"] {
        assert_eq!(
            version_triple(bad),
            None,
            "LIB-QA-019: {bad:?} is not a version"
        );
    }
}
