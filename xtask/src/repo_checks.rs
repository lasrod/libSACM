//! Regression tests for repository configuration: the CI gates, the MSRV
//! declaration and the dependency policy (rows LIB-QA-001..003).
//!
//! These guard against a gate being removed or drifting out of step with the
//! manifest — the kind of change that passes review because nothing fails.

use std::fs;
use std::path::Path;

fn read(rel: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root");
    fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

fn toml_table(rel: &str) -> toml::Table {
    read(rel)
        .parse()
        .unwrap_or_else(|e| panic!("parse {rel}: {e}"))
}

fn workspace_msrv() -> String {
    toml_table("Cargo.toml")["workspace"]["package"]["rust-version"]
        .as_str()
        .expect("workspace.package.rust-version is a string")
        .to_owned()
}

#[test]
fn lib_qa_001_ci_runs_every_gate() {
    let ci = read(".github/workflows/ci.yml");
    for gate in [
        "cargo fmt --all -- --check",
        "cargo clippy --workspace --all-targets --all-features -- -D warnings",
        "cargo test --workspace --all-features",
        "cargo test -p xtask",
        "cargo doc --workspace --no-deps --all-features",
        "RUSTDOCFLAGS: -D warnings",
        "cargo xtask matrix check",
        "EmbarkStudios/cargo-deny-action",
        "python -m unittest discover -s .claude/hooks",
    ] {
        assert!(ci.contains(gate), "LIB-QA-001: ci.yml must run `{gate}`");
    }
    for os in ["ubuntu-latest", "windows-latest", "macos-latest"] {
        assert!(ci.contains(os), "LIB-QA-001: tests must run on {os}");
    }
    for trigger in ["push:", "pull_request:"] {
        assert!(
            ci.contains(trigger),
            "LIB-QA-001: CI must trigger on {trigger}"
        );
    }
}

#[test]
fn lib_qa_002_msrv_is_consistent() {
    let msrv = workspace_msrv();
    let clippy_msrv = toml_table("clippy.toml")["msrv"]
        .as_str()
        .expect("clippy.toml msrv is a string")
        .to_owned();
    assert_eq!(
        clippy_msrv, msrv,
        "LIB-QA-002: clippy.toml msrv must equal rust-version"
    );
    let ci = read(".github/workflows/ci.yml");
    assert!(
        ci.contains(&format!("cargo +{msrv} test -p sacm -p sacm-cli")),
        "LIB-QA-002: CI must test the published crates with the {msrv} toolchain"
    );
    let adr = read("docs/adr/0007-msrv-and-edition.md");
    assert!(
        adr.contains(&msrv),
        "LIB-QA-002: ADR 0007 must state the MSRV {msrv}"
    );
}

#[test]
fn lib_qa_003_dependency_policy_is_enforced() {
    let deny = toml_table("deny.toml");
    let allowed: Vec<&str> = deny["licenses"]["allow"]
        .as_array()
        .expect("licenses.allow is an array")
        .iter()
        .filter_map(toml::Value::as_str)
        .collect();
    assert!(allowed.contains(&"MIT"), "LIB-QA-003: MIT must be allowed");
    for copyleft in ["GPL-2.0", "GPL-3.0", "AGPL-3.0", "LGPL-2.1", "LGPL-3.0"] {
        assert!(
            !allowed.iter().any(|l| l.starts_with(copyleft)),
            "LIB-QA-003: {copyleft} must not be allowed for an MIT library"
        );
    }
    assert_eq!(
        deny["sources"]["unknown-git"].as_str(),
        Some("deny"),
        "LIB-QA-003: git dependencies must be denied"
    );
    assert_eq!(
        deny["sources"]["unknown-registry"].as_str(),
        Some("deny"),
        "LIB-QA-003: registries other than crates.io must be denied"
    );
    assert_eq!(
        deny["advisories"]["yanked"].as_str(),
        Some("deny"),
        "LIB-QA-003: yanked crates must be denied"
    );
}
