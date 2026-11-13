//! Regression tests for the repository's policy files: the CI gate list
//! (LIB-QA-001), the MSRV declaration (LIB-QA-002, LIB-DOC-007), the
//! dependency policy in `deny.toml` (LIB-QA-003), the cargo-mutants
//! configuration and its triage (LIB-QA-007), the nextest settings that make
//! mutation outcomes independent of test order (LIB-QA-012, ADR 0009), the
//! git-ignore rules (LIB-TOOL-002, LIB-QA-007), and the ADR index and the
//! links to ADRs (LIB-DOC-008).
//!
//! Workflow YAML structure is not pinned here: branch protection on `main`
//! and review guard it (slice 0012).
//!
//! Each check is a function over a file's text or parsed TOML that returns the
//! problems it finds. Every problem starts with a stable code in brackets
//! (`[unreviewed-key] ...`), so a test can assert *which* check reported a
//! mutation, not only that something did. Each check is applied to the real
//! file, which must pass, and to in-memory mutations of a reference text, each
//! of which must give its codes. The mutations never touch the files on disk.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use xtask::fetch::DEFAULT_DEST;

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
}

fn read(rel: &str) -> String {
    fs::read_to_string(repository_root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// The text of `rel`, or `None` if it does not exist.
fn read_optional(rel: &str) -> Option<String> {
    match fs::read_to_string(repository_root().join(rel)) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!("read {rel}: {e}"),
    }
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

/// A problem: `[code] text`.
fn problem(code: &str, text: impl std::fmt::Display) -> String {
    format!("[{code}] {text}")
}

fn has_code(problems: &[String], code: &str) -> bool {
    let tag = format!("[{code}] ");
    problems.iter().any(|p| p.starts_with(&tag))
}

/// The codes of `problems`, sorted, with repeats. A problem without a
/// `[code] ` prefix shows as `<uncoded>`, so it can never match a code.
fn problem_codes(problems: &[String]) -> Vec<String> {
    let mut codes: Vec<String> = problems
        .iter()
        .map(|p| {
            p.strip_prefix('[')
                .and_then(|rest| rest.split_once("] "))
                .map_or_else(|| "<uncoded>".to_owned(), |(code, _)| code.to_owned())
        })
        .collect();
    codes.sort();
    codes
}

fn require(problems: &mut Vec<String>, ok: bool, code: &str, text: &str) {
    if !ok {
        problems.push(problem(code, text));
    }
}

fn lookup<'a>(table: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Value> {
    let (first, rest) = path.split_first()?;
    rest.iter()
        .try_fold(table.get(*first)?, |value, key| value.get(*key))
}

/// A TOML value written inline, e.g. `toml_value("[1, 2]")`.
fn toml_value(inline: &str) -> toml::Value {
    let table: toml::Table = format!("v = {inline}")
        .parse()
        .unwrap_or_else(|e| panic!("parse `{inline}`: {e}"));
    table["v"].clone()
}

// ---------------------------------------------------------------------------
// Mutation helpers: in-memory edits of a text, line by line.
// ---------------------------------------------------------------------------

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn leading(line: &str) -> &str {
    &line[..indent(line)]
}

/// Replace the one line of `text` whose trimmed form is `target` by the lines
/// `edit` returns for it. Panics if there is not exactly one such line, so a
/// mutation can never silently be a no-op.
fn edit_line(text: &str, target: &str, edit: impl FnOnce(&str) -> Vec<String>) -> String {
    let hits = text.lines().filter(|line| line.trim() == target).count();
    assert_eq!(
        hits, 1,
        "mutation target `{target}` must occur exactly once"
    );
    let mut edit = Some(edit);
    let mut out = String::new();
    for line in text.lines() {
        if line.trim() == target {
            let edit = edit.take().expect("one target");
            for new in edit(line) {
                out.push_str(&new);
                out.push('\n');
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// `text` with the one line equal (trimmed) to `target` replaced by
/// `replacement`, keeping its indentation.
fn replace_line(text: &str, target: &str, replacement: &str) -> String {
    edit_line(text, target, |line| {
        vec![format!("{}{replacement}", leading(line))]
    })
}

/// `text` with the one line equal (trimmed) to `target` commented out.
fn comment_line(text: &str, target: &str) -> String {
    edit_line(text, target, |line| {
        vec![format!("{}# {}", leading(line), line.trim())]
    })
}

/// `text` with the one line equal (trimmed) to `target` deleted.
fn delete_line(text: &str, target: &str) -> String {
    edit_line(text, target, |_| Vec::new())
}

/// `text` with `extra` inserted after the line equal (trimmed) to `target`,
/// indented `deeper` spaces more than it.
fn insert_after(text: &str, target: &str, deeper: usize, extra: &[&str]) -> String {
    edit_line(text, target, |line| {
        let pad = format!("{}{}", leading(line), " ".repeat(deeper));
        std::iter::once(line.to_owned())
            .chain(extra.iter().map(|e| format!("{pad}{e}")))
            .collect()
    })
}

/// `text` with its one occurrence of `from` replaced by `to`.
fn replace_once(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "mutation target `{from}` must occur exactly once"
    );
    text.replacen(from, to, 1)
}

// ---------------------------------------------------------------------------
// LIB-QA-001
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// LIB-QA-002 and LIB-DOC-007: the MSRV
// ---------------------------------------------------------------------------

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
    let command = format!("cargo +{msrv} test -p sacm -p sacm-cli");
    assert!(
        read(".github/workflows/ci.yml").contains(&command),
        "LIB-QA-002: CI must test the published crates with the {msrv} toolchain (`{command}`)"
    );
    let adr = read("docs/adr/0007-msrv-and-edition.md");
    assert!(
        adr.contains(&msrv),
        "LIB-QA-002: ADR 0007 must state the MSRV {msrv}"
    );
}

/// LIB-DOC-007: README.md's MSRV badge and CONTRIBUTING.md's "Minimum
/// supported Rust version" section state the workspace `rust-version`.
#[test]
fn lib_doc_007_documented_msrv_matches_the_manifest() {
    let msrv = workspace_msrv();
    let readme = read("README.md");
    let badge = readme
        .split("MSRV-")
        .nth(1)
        .and_then(|rest| rest.split('-').next());
    assert_eq!(
        badge,
        Some(msrv.as_str()),
        "LIB-DOC-007: README.md's MSRV badge must state {msrv}"
    );
    let contributing = read("CONTRIBUTING.md");
    let stated = contributing
        .split("## Minimum supported Rust version")
        .nth(1)
        .and_then(|section| {
            section
                .split(|c: char| !c.is_ascii_digit() && c != '.')
                .find(|word| word.contains('.') && word.starts_with(|c: char| c.is_ascii_digit()))
        });
    assert_eq!(
        stated,
        Some(msrv.as_str()),
        "LIB-DOC-007: CONTRIBUTING.md's \"Minimum supported Rust version\" section must state {msrv} first"
    );
}

/// LIB-QA-002 (Q2-2): a member manifest inherits `rust-version` from the
/// workspace and declares none of its own.
fn check_inherits_rust_version(manifest: &str) -> Vec<String> {
    let table: toml::Table = match manifest.parse() {
        Ok(table) => table,
        Err(e) => return vec![format!("manifest does not parse: {e}")],
    };
    let inherited: toml::Table = "workspace = true".parse().expect("literal TOML");
    match table.get("package").and_then(|p| p.get("rust-version")) {
        Some(toml::Value::Table(t)) if *t == inherited => Vec::new(),
        Some(other) => vec![format!(
            "package.rust-version must be `{{ workspace = true }}`, found `{other}`"
        )],
        None => vec!["package.rust-version is missing; it must inherit the workspace MSRV".into()],
    }
}

/// The workspace members that are published (no `publish = false`), as
/// (path, manifest text).
fn published_members() -> Vec<(String, String)> {
    let workspace = toml_table("Cargo.toml");
    let members = workspace["workspace"]["members"]
        .as_array()
        .expect("workspace.members is an array");
    members
        .iter()
        .map(|m| m.as_str().expect("member path is a string"))
        .inspect(|m| {
            assert!(
                !m.contains('*'),
                "LIB-QA-002: member globs are not supported by this test: {m}"
            );
        })
        .map(|m| (m.to_owned(), read(&format!("{m}/Cargo.toml"))))
        .filter(|(_, manifest)| {
            let table: toml::Table = manifest.parse().expect("member manifest parses");
            lookup(&table, &["package", "publish"]).and_then(toml::Value::as_bool) != Some(false)
        })
        .collect()
}

#[test]
fn lib_qa_002_published_crates_inherit_rust_version() {
    let members = published_members();
    for required in ["crates/sacm", "crates/sacm-cli"] {
        assert!(
            members.iter().any(|(path, _)| path == required),
            "LIB-QA-002: {required} must be a published workspace member"
        );
    }
    for (path, manifest) in &members {
        let problems = check_inherits_rust_version(manifest);
        assert!(
            problems.is_empty(),
            "LIB-QA-002: {path} must inherit rust-version from the workspace: {problems:#?}"
        );
    }
}

/// The `rust-version` line of a member manifest, trimmed.
fn rust_version_line(manifest: &str) -> String {
    manifest
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("rust-version"))
        .expect("the manifest has a rust-version line")
        .to_owned()
}

#[test]
fn lib_qa_002_rust_version_check_rejects_member_declarations() {
    let msrv = workspace_msrv();
    for path in ["crates/sacm", "crates/sacm-cli"] {
        let manifest = read(&format!("{path}/Cargo.toml"));
        assert!(
            check_inherits_rust_version(&manifest).is_empty(),
            "LIB-QA-002: {path} must pass before mutations are meaningful"
        );
        let target = rust_version_line(&manifest);
        let mutations = [
            (
                "literal rust-version equal to the workspace's",
                replace_line(&manifest, &target, &format!("rust-version = \"{msrv}\"")),
            ),
            (
                "literal older rust-version",
                replace_line(&manifest, &target, "rust-version = \"1.80\""),
            ),
            ("rust-version removed", delete_line(&manifest, &target)),
            (
                "workspace = false",
                replace_line(&manifest, &target, "rust-version.workspace = false"),
            ),
        ];
        for (name, mutated) in &mutations {
            assert!(
                !check_inherits_rust_version(mutated).is_empty(),
                "LIB-QA-002 ({path}): the check must report the mutation `{name}`; mutated text:\n{mutated}"
            );
        }
    }
}

#[test]
fn lib_qa_002_rust_version_check_accepts_inline_inheritance() {
    for path in ["crates/sacm", "crates/sacm-cli"] {
        let manifest = read(&format!("{path}/Cargo.toml"));
        let target = rust_version_line(&manifest);
        let inline = replace_line(&manifest, &target, "rust-version = { workspace = true }");
        let problems = check_inherits_rust_version(&inline);
        assert!(
            problems.is_empty(),
            "LIB-QA-002 ({path}): an inline `rust-version = {{ workspace = true }}` inherits the MSRV, but was reported: {problems:#?}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-QA-003: the dependency policy in `deny.toml`
// ---------------------------------------------------------------------------

/// The permissive licences an MIT library may depend on. The lead-approved
/// set: the eight in `deny.toml` today plus six more.
const PERMISSIVE: [&str; 14] = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-3.0",
    "Zlib",
    "0BSD",
    "MIT-0",
    "BSL-1.0",
    "CC0-1.0",
    "Unlicense",
    "Unicode-DFS-2016",
];

const CRATES_IO_INDEX: &str = "https://github.com/rust-lang/crates.io-index";

/// R3-3: the legacy top-level graph keys, none of which may be present.
const LEGACY_GRAPH_KEYS: [&str; 6] = [
    "targets",
    "exclude",
    "features",
    "all-features",
    "no-default-features",
    "exclude-dev",
];

/// R3-7: for each reviewed `deny.toml` section (dotted path; `""` is the top
/// level), the keys cargo-deny 0.20.2 accepts there, as listed by its own
/// `unexpected-keys` error. Any other key is reported, so a key a future
/// cargo-deny adds is reviewed before it can narrow the checks. `[bans]` and
/// `[output]` are not reviewed.
const DENY_SECTION_KEYS: [(&str, &[&str]); 7] = [
    (
        "",
        &[
            "advisories",
            "bans",
            "licenses",
            "sources",
            "graph",
            "targets",
            "exclude",
            "features",
            "all-features",
            "no-default-features",
            "exclude-dev",
            "output",
            "feature-depth",
        ],
    ),
    (
        "graph",
        &[
            "targets",
            "exclude",
            "features",
            "all-features",
            "no-default-features",
            "exclude-dev",
            "exclude-unpublished",
        ],
    ),
    (
        "advisories",
        &[
            "version",
            "db-path",
            "db-urls",
            "vulnerability",
            "notice",
            "unmaintained",
            "unsound",
            "yanked",
            "ignore",
            "severity-threshold",
            "git-fetch-with-cli",
            "disable-yank-checking",
            "maximum-db-staleness",
            "unused-ignored-advisory",
        ],
    ),
    (
        "licenses",
        &[
            "version",
            "private",
            "unlicensed",
            "allow-osi-fsf-free",
            "copyleft",
            "default",
            "confidence-threshold",
            "deny",
            "allow",
            "unused-allowed-license",
            "clarify",
            "exceptions",
            "unused-license-exception",
            "include-dev",
            "include-build",
        ],
    ),
    (
        "licenses.private",
        &["ignore", "ignore-sources", "registries"],
    ),
    (
        "sources",
        &[
            "unknown-registry",
            "unknown-git",
            "allow-registry",
            "allow-git",
            "allow-org",
            "private",
            "required-git-spec",
            "unused-allowed-source",
            "unused-allowed-org",
        ],
    ),
    ("sources.allow-org", &["github", "gitlab", "bitbucket"]),
];

fn is_absent_or_empty(value: Option<&toml::Value>) -> bool {
    value.is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
}

fn is_absent_or_false(value: Option<&toml::Value>) -> bool {
    value.is_none_or(|v| v.as_bool() == Some(false))
}

/// Licences in `value` (an array of strings) outside [`PERMISSIVE`].
fn non_permissive(value: Option<&toml::Value>, what: &str) -> Vec<String> {
    let Some(array) = value.and_then(toml::Value::as_array) else {
        return vec![problem(
            "licenses",
            format!("{what} must be an array of licence identifiers"),
        )];
    };
    array
        .iter()
        .filter(|l| l.as_str().is_none_or(|l| !PERMISSIVE.contains(&l)))
        .map(|l| {
            problem(
                "licenses",
                format!("{what} contains `{l}`, which is not in the permissive set"),
            )
        })
        .collect()
}

/// R3-7: every key of a reviewed section outside [`DENY_SECTION_KEYS`], as
/// `[unreviewed-key]`; a reviewed section that is not a table is `[shape]`.
fn unreviewed_key_problems(deny: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    for (section, keys) in DENY_SECTION_KEYS {
        let path: Vec<&str> = section.split('.').filter(|s| !s.is_empty()).collect();
        let table = if path.is_empty() {
            deny
        } else {
            match lookup(deny, &path) {
                None => continue,
                Some(toml::Value::Table(table)) => table,
                Some(other) => {
                    problems.push(problem(
                        "shape",
                        format!("deny.toml `[{section}]` must be a table, found `{other}`"),
                    ));
                    continue;
                }
            }
        };
        for key in table.keys() {
            if !keys.contains(&key.as_str()) {
                let at = if section.is_empty() {
                    "the top level".to_owned()
                } else {
                    format!("`[{section}]`")
                };
                problems.push(problem(
                    "unreviewed-key",
                    format!("deny.toml has the key `{key}` at {at}, which has not been reviewed"),
                ));
            }
        }
    }
    problems
}

/// Q3-1, Q3-2 and R3-6: the licence policy, applied to every crate.
fn licence_problems(deny: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    // Q3-1: licences.
    let allow = lookup(deny, &["licenses", "allow"]);
    require(
        &mut problems,
        allow
            .and_then(toml::Value::as_array)
            .is_some_and(|a| a.iter().any(|l| l.as_str() == Some("MIT"))),
        "licenses",
        "licenses.allow must allow MIT",
    );
    // Q3-2: exceptions.
    let exceptions = lookup(deny, &["licenses", "exceptions"]);
    let exception_problems: Vec<String> = match exceptions {
        None => Vec::new(),
        Some(toml::Value::Array(entries)) => entries
            .iter()
            .flat_map(|entry| non_permissive(entry.get("allow"), "a licenses.exceptions entry"))
            .collect(),
        Some(other) => vec![problem(
            "licenses",
            format!("licenses.exceptions must be an array, found `{other}`"),
        )],
    };
    // R3-6: the licence check covers every crate.
    require(
        &mut problems,
        is_absent_or_empty(lookup(deny, &["licenses", "private", "ignore-sources"])),
        "licenses",
        "licenses.private.ignore-sources must be absent or empty",
    );
    require(
        &mut problems,
        is_absent_or_empty(lookup(deny, &["licenses", "private", "registries"])),
        "licenses",
        "licenses.private.registries must be absent or empty",
    );
    // Round 3 F2: cargo-deny's default for `include-dev` is false, so an
    // absent key leaves dev-dependencies unchecked; it must be exactly `true`
    // (maintainer decision, option (a)). `include-build` defaults to true.
    require(
        &mut problems,
        matches!(
            lookup(deny, &["licenses", "include-dev"]),
            Some(toml::Value::Boolean(true))
        ),
        "licenses",
        "licenses.include-dev must be exactly true (cargo-deny's default is false)",
    );
    require(
        &mut problems,
        !matches!(
            lookup(deny, &["licenses", "include-build"]),
            Some(toml::Value::Boolean(false))
        ),
        "licenses",
        "licenses.include-build must not be false",
    );
    problems.extend(non_permissive(allow, "licenses.allow"));
    problems.extend(exception_problems);
    problems
}

/// Q3-2 and R3-3: no crate is excluded from the graph.
fn graph_problems(deny: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    require(
        &mut problems,
        is_absent_or_empty(lookup(deny, &["graph", "exclude"])),
        "graph",
        "graph.exclude must be absent or empty: no crate may be excluded from the checks",
    );
    require(
        &mut problems,
        is_absent_or_empty(lookup(deny, &["graph", "targets"])),
        "graph",
        "graph.targets must be absent or empty: no target may narrow the graph",
    );
    require(
        &mut problems,
        is_absent_or_false(lookup(deny, &["graph", "exclude-dev"])),
        "graph",
        "graph.exclude-dev must be absent or false",
    );
    require(
        &mut problems,
        is_absent_or_false(lookup(deny, &["graph", "exclude-unpublished"])),
        "graph",
        "graph.exclude-unpublished must be absent or false",
    );
    require(
        &mut problems,
        lookup(deny, &["graph", "all-features"]).and_then(toml::Value::as_bool) == Some(true),
        "graph",
        "graph.all-features must be true",
    );
    for key in LEGACY_GRAPH_KEYS {
        require(
            &mut problems,
            !deny.contains_key(key),
            "graph",
            &format!("the legacy top-level key `{key}` must be absent; it still narrows the graph"),
        );
    }
    problems
}

/// Q3-3 and R3-4: sources.
fn source_problems(deny: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    require(
        &mut problems,
        lookup(deny, &["sources", "unknown-git"]).and_then(toml::Value::as_str) == Some("deny"),
        "sources",
        "sources.unknown-git must be \"deny\"",
    );
    require(
        &mut problems,
        lookup(deny, &["sources", "unknown-registry"]).and_then(toml::Value::as_str)
            == Some("deny"),
        "sources",
        "sources.unknown-registry must be \"deny\"",
    );
    require(
        &mut problems,
        is_absent_or_empty(lookup(deny, &["sources", "allow-git"])),
        "sources",
        "sources.allow-git must be absent or empty",
    );
    require(
        &mut problems,
        lookup(deny, &["sources", "allow-org"]).is_none_or(|org| {
            org.as_table()
                .is_some_and(|t| t.values().all(|v| v.as_array().is_some_and(Vec::is_empty)))
        }),
        "sources",
        "sources.allow-org must be absent or have only empty lists",
    );
    require(
        &mut problems,
        lookup(deny, &["sources", "allow-registry"]).is_none_or(|r| {
            r.as_array()
                .is_some_and(|r| r.len() == 1 && r[0].as_str() == Some(CRATES_IO_INDEX))
        }),
        "sources",
        "sources.allow-registry must be absent or exactly the crates.io index",
    );
    require(
        &mut problems,
        is_absent_or_empty(lookup(deny, &["sources", "private"])),
        "sources",
        "sources.private must be absent or empty",
    );
    problems
}

/// Q3-4 and R3-5: advisories.
fn advisory_problems(deny: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    require(
        &mut problems,
        lookup(deny, &["advisories", "ignore"]).is_none_or(|ignore| {
            ignore.as_array().is_some_and(|entries| {
                entries.iter().all(|entry| {
                    entry
                        .get("reason")
                        .and_then(toml::Value::as_str)
                        .is_some_and(|reason| !reason.trim().is_empty())
                })
            })
        }),
        "advisories",
        "every advisories.ignore entry must be a table with a non-empty `reason`",
    );
    require(
        &mut problems,
        lookup(deny, &["advisories", "db-urls"]).is_none(),
        "advisories",
        "advisories.db-urls must be absent: no alternative advisory database",
    );
    require(
        &mut problems,
        lookup(deny, &["advisories", "yanked"]).and_then(toml::Value::as_str) == Some("deny"),
        "advisories",
        "advisories.yanked must be \"deny\"",
    );
    require(
        &mut problems,
        is_absent_or_false(lookup(deny, &["advisories", "disable-yank-checking"])),
        "advisories",
        "advisories.disable-yank-checking must be absent or false",
    );
    problems
}

/// LIB-QA-003 (Q3-1 to Q3-4, R3-3 to R3-7, and the original criteria): the
/// dependency policy in a parsed `deny.toml`.
fn check_deny_policy(deny: &toml::Table) -> Vec<String> {
    let mut problems = licence_problems(deny);
    problems.extend(graph_problems(deny));
    problems.extend(source_problems(deny));
    problems.extend(advisory_problems(deny));
    problems.extend(unreviewed_key_problems(deny));
    problems
}

#[test]
fn lib_qa_003_dependency_policy_is_enforced() {
    let deny = toml_table("deny.toml");
    let problems = check_deny_policy(&deny);
    assert!(
        problems.is_empty(),
        "LIB-QA-003: deny.toml must enforce the dependency policy: {problems:#?}"
    );
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
    assert_eq!(
        deny["licenses"]
            .get("include-dev")
            .and_then(toml::Value::as_bool),
        Some(true),
        "LIB-QA-003: dev-dependencies must be licence-checked (`licenses.include-dev = true`)"
    );
}

/// The table at the dotted `section` of `deny` (`""` is the top level),
/// created if absent.
fn section_mut<'a>(deny: &'a mut toml::Table, section: &str) -> &'a mut toml::Table {
    let mut table = deny;
    for name in section.split('.').filter(|s| !s.is_empty()) {
        table = table
            .entry(name)
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .expect("a deny.toml section is a table");
    }
    table
}

/// The real `deny.toml`, with `value` set at `section.key` (`section` dotted;
/// `""` is the top level).
fn deny_with(section: &str, key: &str, value: &str) -> toml::Table {
    let mut deny = toml_table("deny.toml");
    section_mut(&mut deny, section).insert(key.to_owned(), toml_value(value));
    deny
}

/// The real `deny.toml`, without `section.key`.
fn deny_without(section: &str, key: &str) -> toml::Table {
    let mut deny = toml_table("deny.toml");
    assert!(
        section_mut(&mut deny, section).remove(key).is_some(),
        "mutation target `{section}.{key}` must exist"
    );
    deny
}

/// The real `deny.toml`, with `licence` appended to `licenses.allow`.
fn deny_allowing(licence: &str) -> toml::Table {
    let mut deny = toml_table("deny.toml");
    deny["licenses"]
        .get_mut("allow")
        .and_then(toml::Value::as_array_mut)
        .expect("licenses.allow is an array")
        .push(licence.into());
    deny
}

fn assert_policy_reports(mutations: &[(String, toml::Table)]) {
    for (name, deny) in mutations {
        assert!(
            !check_deny_policy(deny).is_empty(),
            "LIB-QA-003: the policy check must report the mutation `{name}`"
        );
    }
}

/// Assert that the policy check reports each mutation `(name, section, key,
/// value)` of the real `deny.toml` with `code`.
fn assert_policy_reports_code(code: &str, mutations: &[(&str, &str, &str, &str)]) {
    assert!(
        check_deny_policy(&toml_table("deny.toml")).is_empty(),
        "LIB-QA-003: the real deny.toml must pass before mutations are meaningful"
    );
    for (name, section, key, value) in mutations {
        let problems = check_deny_policy(&deny_with(section, key, value));
        assert!(
            has_code(&problems, code),
            "LIB-QA-003: the policy check must report the mutation `{name}` as `[{code}]`; it reported {problems:#?}"
        );
    }
}

#[test]
fn lib_qa_003_licence_check_rejects_non_permissive_licences() {
    assert!(
        check_deny_policy(&toml_table("deny.toml")).is_empty(),
        "LIB-QA-003: the real deny.toml must pass before mutations are meaningful"
    );
    let mut mutations: Vec<(String, toml::Table)> = [
        "MPL-2.0",
        "LGPL-2.0-or-later",
        "GPL-1.0-or-later",
        "EUPL-1.2",
        "CC-BY-SA-4.0",
        "SSPL-1.0",
        "BUSL-1.1",
        "GPL-3.0-only",
        "AGPL-3.0-only",
    ]
    .iter()
    .map(|l| (format!("licenses.allow + {l}"), deny_allowing(l)))
    .collect();
    let mut without_mit = toml_table("deny.toml");
    without_mit["licenses"]
        .get_mut("allow")
        .and_then(toml::Value::as_array_mut)
        .expect("licenses.allow is an array")
        .retain(|l| l.as_str() != Some("MIT"));
    mutations.push(("MIT removed".into(), without_mit));
    for (name, value) in [
        (
            "exception allowing MPL-2.0",
            r#"[{ crate = "some-crate", allow = ["MPL-2.0"] }]"#,
        ),
        (
            "exception allowing GPL-3.0-only",
            r#"[{ name = "some-crate", allow = ["GPL-3.0-only"] }]"#,
        ),
        (
            "exception mixing permitted and copyleft",
            r#"[{ crate = "some-crate", allow = ["MIT", "LGPL-2.0-or-later"] }]"#,
        ),
        ("exception without allow", r#"[{ crate = "some-crate" }]"#),
    ] {
        mutations.push((name.into(), deny_with("licenses", "exceptions", value)));
    }
    mutations.push((
        "graph.exclude".into(),
        deny_with("graph", "exclude", r#"["some-crate"]"#),
    ));
    assert_policy_reports(&mutations);
}

#[test]
fn lib_qa_003_policy_check_rejects_known_bypasses() {
    let mutations: Vec<(String, toml::Table)> = [
        // Q3-3: sources.
        (
            "allow-git",
            "sources",
            "allow-git",
            r#"["https://github.com/someone/some-crate"]"#,
        ),
        (
            "extra registry",
            "sources",
            "allow-registry",
            r#"["https://github.com/rust-lang/crates.io-index", "https://example.com/index"]"#,
        ),
        (
            "registry replaced",
            "sources",
            "allow-registry",
            r#"["https://example.com/index"]"#,
        ),
        (
            "allow-org",
            "sources",
            "allow-org",
            r#"{ github = ["someone"] }"#,
        ),
        (
            "allow-org, one non-empty list",
            "sources",
            "allow-org",
            r#"{ github = [], gitlab = ["someone"] }"#,
        ),
        (
            "unknown-git = allow",
            "sources",
            "unknown-git",
            r#""allow""#,
        ),
        (
            "unknown-registry = warn",
            "sources",
            "unknown-registry",
            r#""warn""#,
        ),
        // Q3-4: advisories.
        (
            "advisories.ignore, bare id",
            "advisories",
            "ignore",
            r#"["RUSTSEC-2024-0001"]"#,
        ),
        (
            "advisories.ignore, no reason",
            "advisories",
            "ignore",
            r#"[{ id = "RUSTSEC-2024-0001" }]"#,
        ),
        (
            "advisories.ignore, blank reason",
            "advisories",
            "ignore",
            r#"[{ id = "RUSTSEC-2024-0001", reason = "  " }]"#,
        ),
        (
            "advisories.ignore, one of two unjustified",
            "advisories",
            "ignore",
            r#"[{ id = "RUSTSEC-2024-0001", reason = "not reachable" }, "RUSTSEC-2024-0002"]"#,
        ),
        (
            "advisories.db-urls",
            "advisories",
            "db-urls",
            r#"["https://example.com/advisory-db"]"#,
        ),
        ("yanked = warn", "advisories", "yanked", r#""warn""#),
        // Q3-2: graph.
        (
            "graph.exclude",
            "graph",
            "exclude",
            r#"["some-crate@1.0.0"]"#,
        ),
    ]
    .iter()
    .map(|(name, section, key, value)| ((*name).to_owned(), deny_with(section, key, value)))
    .collect();
    assert_policy_reports(&mutations);
}

#[test]
fn lib_qa_003_policy_check_rejects_graph_exclusions() {
    assert_policy_reports_code(
        "graph",
        &[
            (
                "graph.exclude = [\"clap\"]",
                "graph",
                "exclude",
                r#"["clap"]"#,
            ),
            (
                "graph.targets",
                "graph",
                "targets",
                r#"["x86_64-unknown-linux-gnu"]"#,
            ),
            (
                "graph.targets, table form",
                "graph",
                "targets",
                r#"[{ triple = "x86_64-unknown-linux-gnu" }]"#,
            ),
            ("graph.exclude-dev = true", "graph", "exclude-dev", "true"),
            (
                "graph.exclude-unpublished = true",
                "graph",
                "exclude-unpublished",
                "true",
            ),
            (
                "graph.all-features = false",
                "graph",
                "all-features",
                "false",
            ),
            (
                "top-level targets",
                "",
                "targets",
                r#"["x86_64-unknown-linux-gnu"]"#,
            ),
            (
                "top-level exclude = [\"clap\"]",
                "",
                "exclude",
                r#"["clap"]"#,
            ),
            ("top-level features", "", "features", r#"["some-feature"]"#),
            ("top-level all-features", "", "all-features", "true"),
            (
                "top-level no-default-features",
                "",
                "no-default-features",
                "true",
            ),
            ("top-level exclude-dev", "", "exclude-dev", "true"),
        ],
    );
    let mut no_graph = toml_table("deny.toml");
    no_graph.remove("graph");
    for (name, deny) in [
        (
            "graph.all-features removed",
            deny_without("graph", "all-features"),
        ),
        ("[graph] removed", no_graph),
    ] {
        let problems = check_deny_policy(&deny);
        assert!(
            has_code(&problems, "graph"),
            "LIB-QA-003: the policy check must report the mutation `{name}` as `[graph]`; it reported {problems:#?}"
        );
    }
}

#[test]
fn lib_qa_003_policy_check_rejects_private_sources() {
    assert_policy_reports_code(
        "sources",
        &[(
            "sources.private",
            "sources",
            "private",
            r#"["https://example.com/registry"]"#,
        )],
    );
}

#[test]
fn lib_qa_003_policy_check_rejects_disabled_yank_checking() {
    assert_policy_reports_code(
        "advisories",
        &[(
            "advisories.disable-yank-checking = true",
            "advisories",
            "disable-yank-checking",
            "true",
        )],
    );
}

#[test]
fn lib_qa_003_policy_check_rejects_narrowed_licence_check() {
    assert_policy_reports_code(
        "licenses",
        &[
            (
                "licenses.private.ignore-sources = [crates.io]",
                "licenses.private",
                "ignore-sources",
                r#"["https://github.com/rust-lang/crates.io-index"]"#,
            ),
            (
                "licenses.private.registries",
                "licenses.private",
                "registries",
                r#"["some-registry"]"#,
            ),
            (
                "licenses.include-dev = false",
                "licenses",
                "include-dev",
                "false",
            ),
            (
                "licenses.include-dev = \"true\", a string",
                "licenses",
                "include-dev",
                r#""true""#,
            ),
            (
                "licenses.include-build = false",
                "licenses",
                "include-build",
                "false",
            ),
        ],
    );
    // Round 3 F2: cargo-deny 0.20.2 does not licence-check dev-dependencies
    // unless `include-dev = true`, so removing the key narrows the check.
    let problems = check_deny_policy(&deny_without("licenses", "include-dev"));
    assert!(
        has_code(&problems, "licenses"),
        "LIB-QA-003: the mutation `licenses.include-dev removed` must be reported as `[licenses]`; it reported {problems:#?}"
    );
}

#[test]
fn lib_qa_003_policy_check_rejects_unreviewed_keys() {
    let mut in_every_section: Vec<(String, String)> = DENY_SECTION_KEYS
        .iter()
        .map(|(section, _)| ((*section).to_owned(), "future-key".to_owned()))
        .collect();
    // A key that exists in another section is not reviewed here.
    in_every_section.push(("sources".into(), "skip".into()));
    in_every_section.push(("graph".into(), "ignore".into()));
    for (section, key) in &in_every_section {
        let problems = check_deny_policy(&deny_with(section, key, "true"));
        assert!(
            has_code(&problems, "unreviewed-key"),
            "LIB-QA-003: the unreviewed key `{key}` in `[{section}]` must be reported as `[unreviewed-key]`; it reported {problems:#?}"
        );
    }
    for (name, section, key, value) in [
        (
            "[licenses.private] is not a table",
            "licenses",
            "private",
            "true",
        ),
        (
            "[sources.allow-org] is not a table",
            "sources",
            "allow-org",
            r#""github""#,
        ),
    ] {
        let problems = check_deny_policy(&deny_with(section, key, value));
        assert!(
            has_code(&problems, "shape"),
            "LIB-QA-003: the mutation `{name}` must be reported as `[shape]`; it reported {problems:#?}"
        );
    }
}

/// `(name, section, key, value)` settings of `deny.toml` within the approved
/// policy: the controls of round 1, the absent-or-empty and absent-or-false
/// values of R3-3 to R3-6, keys cargo-deny 0.20.2 accepts (R3-7), and the
/// unchecked `[bans]` and `[output]` sections. `licenses.include-dev` has one
/// approved value, `true` (round 3 F2); its absence is not approved.
const APPROVED_DENY_VALUES: &[(&str, &str, &str, &str)] = &[
    (
        "exception within the permissive set",
        "licenses",
        "exceptions",
        r#"[{ crate = "some-crate", allow = ["Unicode-DFS-2016"] }]"#,
    ),
    ("empty exceptions", "licenses", "exceptions", "[]"),
    ("empty graph.exclude", "graph", "exclude", "[]"),
    ("empty allow-git", "sources", "allow-git", "[]"),
    (
        "allow-org with empty lists",
        "sources",
        "allow-org",
        "{ github = [], gitlab = [], bitbucket = [] }",
    ),
    ("empty advisories.ignore", "advisories", "ignore", "[]"),
    (
        "justified advisory ignore",
        "advisories",
        "ignore",
        r#"[{ id = "RUSTSEC-2024-0001", reason = "the affected API is not used" }]"#,
    ),
    // R3-3 to R3-6: the absent-or-empty / absent-or-false values.
    ("empty graph.targets", "graph", "targets", "[]"),
    ("graph.exclude-dev = false", "graph", "exclude-dev", "false"),
    (
        "graph.exclude-unpublished = false",
        "graph",
        "exclude-unpublished",
        "false",
    ),
    ("empty sources.private", "sources", "private", "[]"),
    (
        "advisories.disable-yank-checking = false",
        "advisories",
        "disable-yank-checking",
        "false",
    ),
    (
        "empty licenses.private.ignore-sources",
        "licenses.private",
        "ignore-sources",
        "[]",
    ),
    (
        "empty licenses.private.registries",
        "licenses.private",
        "registries",
        "[]",
    ),
    // Round 3 F2: the only approved value, and the value in the real file.
    (
        "licenses.include-dev = true",
        "licenses",
        "include-dev",
        "true",
    ),
    // `include-build` may be true or absent (the real file), never false.
    (
        "licenses.include-build = true",
        "licenses",
        "include-build",
        "true",
    ),
    // R3-7: reviewed keys cargo-deny 0.20.2 accepts, and the unchecked
    // sections.
    (
        "advisories.unmaintained = \"workspace\"",
        "advisories",
        "unmaintained",
        r#""workspace""#,
    ),
    (
        "advisories.unused-ignored-advisory",
        "advisories",
        "unused-ignored-advisory",
        r#""allow""#,
    ),
    (
        "licenses.unused-allowed-license",
        "licenses",
        "unused-allowed-license",
        r#""allow""#,
    ),
    (
        "sources.required-git-spec",
        "sources",
        "required-git-spec",
        r#""rev""#,
    ),
    ("an unreviewed key in [bans]", "bans", "future-key", "true"),
    (
        "an unreviewed key in [output]",
        "output",
        "future-key",
        "true",
    ),
    ("output.feature-depth", "output", "feature-depth", "1"),
];

#[test]
fn lib_qa_003_policy_check_accepts_approved_configuration() {
    let mut approved: Vec<(String, toml::Table)> = PERMISSIVE
        .iter()
        .map(|l| (format!("licenses.allow + {l}"), deny_allowing(l)))
        .collect();
    for &(name, section, key, value) in APPROVED_DENY_VALUES {
        approved.push((name.into(), deny_with(section, key, value)));
    }
    let mut no_registry = toml_table("deny.toml");
    no_registry["sources"]
        .as_table_mut()
        .expect("sources is a table")
        .remove("allow-registry");
    approved.push(("allow-registry absent".into(), no_registry));
    for (name, deny) in &approved {
        let problems = check_deny_policy(deny);
        assert!(
            problems.is_empty(),
            "LIB-QA-003: `{name}` is within the approved policy, but was reported: {problems:#?}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-TOOL-002
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_002_reference_directory_is_git_ignored() {
    assert_eq!(
        DEFAULT_DEST, "third_party/sacm-2.3",
        "LIB-TOOL-002: the artifacts are fetched into third_party/sacm-2.3"
    );
    let gitignore = read(".gitignore");
    let lines: Vec<&str> = gitignore.lines().map(str::trim).collect();
    let rule = format!("/{DEFAULT_DEST}/");
    assert!(
        lines.contains(&rule.as_str()),
        "LIB-TOOL-002: .gitignore must contain the line `{rule}`"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.starts_with('!') && l.contains("third_party")),
        "LIB-TOOL-002: .gitignore must not re-include anything under third_party/"
    );
}

// ---------------------------------------------------------------------------
// LIB-QA-007: the cargo-mutants configuration and its triage
// ---------------------------------------------------------------------------
//
// The specification is docs/verification/0003-m0-mutation-testing-prescription.md
// (P4 and P5) and the row's criteria. Each test first shows that a *reference
// fixture* passes, then that every prescribed mutation of it is reported with
// its code and every control is accepted, and last that the real file passes.

const MUTANTS_CONFIG: &str = ".cargo/mutants.toml";
const MUTANTS_TRIAGE: &str = "docs/quality/mutants-triage.md";
const MATRIX_TOML: &str = "docs/matrix/functionality-matrix.toml";

/// D12: the `.gitignore` rule for cargo-mutants' output.
const MUTANTS_GITIGNORE_RULE: &str = "/mutants.out*";

/// A workspace member: its directory, manifest text and the `.rs` files
/// under its `src/`, as (path from the repository root, text).
#[derive(Clone, Debug)]
struct MemberPackage {
    dir: String,
    manifest: String,
    sources: Vec<(String, String)>,
}

/// The workspace as the P4 checks see it.
#[derive(Clone, Debug)]
struct WorkspaceSources {
    root_manifest: String,
    members: Vec<MemberPackage>,
}

impl WorkspaceSources {
    fn source_paths(&self) -> Vec<&str> {
        self.members
            .iter()
            .flat_map(|m| m.sources.iter().map(|(path, _)| path.as_str()))
            .collect()
    }

    /// This workspace with the text of the source `path` replaced by
    /// `edit(text)`. Panics if there is no such source.
    fn with_source(&self, path: &str, edit: impl FnOnce(&str) -> String) -> Self {
        let mut out = self.clone();
        let text = out
            .members
            .iter_mut()
            .flat_map(|m| m.sources.iter_mut())
            .find(|(p, _)| p == path)
            .map_or_else(
                || panic!("mutation target `{path}` is a workspace source"),
                |(_, text)| text,
            );
        *text = edit(text);
        out
    }

    /// This workspace with the manifest of member `dir` (`""`: the root
    /// manifest) replaced by `edit(text)`.
    fn with_manifest(&self, dir: &str, edit: impl FnOnce(&str) -> String) -> Self {
        let mut out = self.clone();
        let text = if dir.is_empty() {
            &mut out.root_manifest
        } else {
            out.members.iter_mut().find(|m| m.dir == dir).map_or_else(
                || panic!("mutation target `{dir}` is a workspace member"),
                |m| &mut m.manifest,
            )
        };
        *text = edit(text);
        out
    }
}

/// Every `.rs` file under the directory `rel`, recursively, sorted.
fn rust_sources(rel: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut pending = vec![rel.to_owned()];
    while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(repository_root().join(&dir))
            .unwrap_or_else(|e| panic!("read directory {dir}: {e}"));
        for entry in entries {
            let entry = entry.unwrap_or_else(|e| panic!("read directory {dir}: {e}"));
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = format!("{dir}/{name}");
            let path = entry.path();
            if path.is_dir() {
                pending.push(child);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = read(&child);
                out.push((child, text));
            }
        }
    }
    out.sort();
    out
}

/// The real workspace: the root manifest and every member's manifest and
/// `src/` sources.
fn real_workspace() -> WorkspaceSources {
    let members = toml_table("Cargo.toml")["workspace"]["members"]
        .as_array()
        .expect("workspace.members is an array")
        .iter()
        .map(|m| m.as_str().expect("member path is a string").to_owned())
        .map(|dir| MemberPackage {
            manifest: read(&format!("{dir}/Cargo.toml")),
            sources: rust_sources(&format!("{dir}/src")),
            dir,
        })
        .collect();
    WorkspaceSources {
        root_manifest: read("Cargo.toml"),
        members,
    }
}

/// The row ids of the real matrix.
fn matrix_ids() -> Vec<String> {
    toml_table(MATRIX_TOML)["row"]
        .as_array()
        .expect("the matrix has rows")
        .iter()
        .filter_map(|row| row.get("id").and_then(toml::Value::as_str))
        .map(str::to_owned)
        .collect()
}

fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// If a raw string literal (`r"..."`, `r#"..."#`, `br"..."`) starts at `at`,
/// the index just after it.
fn raw_string_end(chars: &[char], at: usize) -> Option<usize> {
    let prefix_ok = match at.checked_sub(1).map(|i| chars[i]) {
        None => true,
        Some('b') => at < 2 || !is_identifier_char(chars[at - 2]),
        Some(c) => !is_identifier_char(c),
    };
    if chars[at] != 'r' || !prefix_ok {
        return None;
    }
    let hashes = chars[at + 1..].iter().take_while(|&&c| c == '#').count();
    let open = at + 1 + hashes;
    if chars.get(open) != Some(&'"') {
        return None;
    }
    let mut i = open + 1;
    while i < chars.len() {
        if chars[i] == '"'
            && chars[i + 1..]
                .iter()
                .take(hashes)
                .filter(|&&c| c == '#')
                .count()
                == hashes
        {
            return Some(i + 1 + hashes);
        }
        i += 1;
    }
    Some(chars.len())
}

/// Rust source with comments and whitespace removed and every string and
/// character literal emptied (P4.9), so that `#[mutants :: skip]`,
/// `mutants/*c*/::skip` and a multi-line `cfg_attr` all read `mutants::skip`,
/// and a literal or comment that merely mentions it does not.
fn code_tokens(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0_usize;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if let Some(end) = raw_string_end(&chars, i) {
            out.push_str("\"\"");
            i = end;
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += if chars[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push_str("\"\"");
        } else if c == '\'' && next == Some('\\') {
            i += 3;
            while i < chars.len() && chars[i] != '\'' {
                i += 1;
            }
            i += 1;
            out.push_str("''");
        } else if c == '\'' && chars.get(i + 2) == Some(&'\'') {
            i += 3;
            out.push_str("''");
        } else {
            if !c.is_whitespace() {
                out.push(c);
            }
            i += 1;
        }
    }
    out
}

/// Whether a manifest depends on the `mutants` crate in any dependency table
/// (including `[workspace.dependencies]`, `[target.*.dependencies]` and a
/// renamed `package = "mutants"`).
fn depends_on_mutants(table: &toml::Table) -> bool {
    table.iter().any(|(key, value)| {
        let Some(inner) = value.as_table() else {
            return false;
        };
        let direct = key.ends_with("dependencies")
            && inner.iter().any(|(name, spec)| {
                name == "mutants"
                    || spec.get("package").and_then(toml::Value::as_str) == Some("mutants")
            });
        direct || depends_on_mutants(inner)
    })
}

/// `[source-skip]` (P4.9, D13): no source has a `mutants::skip` attribute
/// (cargo-mutants honours `cfg_attr(<anything>, mutants::skip)` without
/// evaluating the condition), and no manifest depends on `mutants`.
fn source_skip_problems(workspace: &WorkspaceSources) -> Vec<String> {
    let mut problems = Vec::new();
    for member in &workspace.members {
        for (path, text) in &member.sources {
            if code_tokens(text).contains("mutants::skip") {
                problems.push(problem(
                    "source-skip",
                    format!("{path} has a `mutants::skip` attribute; exclusions belong in {MUTANTS_CONFIG}"),
                ));
            }
        }
    }
    let manifests = std::iter::once(("Cargo.toml".to_owned(), &workspace.root_manifest)).chain(
        workspace
            .members
            .iter()
            .map(|m| (format!("{}/Cargo.toml", m.dir), &m.manifest)),
    );
    for (path, text) in manifests {
        match text.parse::<toml::Table>() {
            Ok(table) if depends_on_mutants(&table) => problems.push(problem(
                "source-skip",
                format!("{path} depends on the `mutants` crate"),
            )),
            Ok(_) => {}
            Err(e) => problems.push(problem(
                "source-skip",
                format!("{path} does not parse: {e}"),
            )),
        }
    }
    problems
}

/// The keys `.cargo/mutants.toml` may have (P4.2). Any other key, including
/// `examine_globs` and `examine_re`, is `[unreviewed-key]`; so is a key of
/// [`NEXTEST_MUTANTS_KEYS`] with any value but its reviewed one (ADR 0009).
const REVIEWED_MUTANTS_KEYS: [&str; 14] = [
    "test_tool",
    "additional_cargo_test_args",
    "cap_lints",
    "all_features",
    "test_workspace",
    "exclude_re",
    "exclude_globs",
    "skip_calls",
    "skip_calls_defaults",
    "timeout_multiplier",
    "minimum_test_timeout",
    "build_timeout_multiplier",
    "sharding",
    "error_values",
];

/// ADR 0009: the `.cargo/mutants.toml` keys that are reviewed with one value
/// only, as (key, the value written inline, LIB-QA-012's code for it lost).
/// LIB-QA-007 accepts each key absent or with exactly this value; LIB-QA-012
/// requires it.
const NEXTEST_MUTANTS_KEYS: [(&str, &str, &str); 2] = [
    ("test_tool", r#""nextest""#, "nextest-test-tool"),
    (
        "additional_cargo_test_args",
        r#"["--profile=mutants", "--no-tests=warn"]"#,
        "nextest-test-args",
    ),
];

/// The config keys whose entries exclude mutants, each of which the triage
/// document must list (P4.8).
const EXCLUSION_KEYS: [&str; 3] = ["exclude_re", "exclude_globs", "skip_calls"];

/// The strings of the array `key`, or `[config-exclusion]` if it is not an
/// array of strings.
fn config_strings(config: &toml::Table, key: &str, problems: &mut Vec<String>) -> Vec<String> {
    match config.get(key) {
        None => Vec::new(),
        Some(toml::Value::Array(items)) if items.iter().all(toml::Value::is_str) => items
            .iter()
            .filter_map(toml::Value::as_str)
            .map(str::to_owned)
            .collect(),
        Some(other) => {
            problems.push(problem(
                "config-exclusion",
                format!("`{key}` must be an array of strings, found `{other}`"),
            ));
            Vec::new()
        }
    }
}

/// `text` read as a literal in regex syntax: every metacharacter escaped
/// with `\`, and `\` only before ASCII punctuation (so no `\d`, `\w`, `\b`).
fn regex_literal(text: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(escaped) if escaped.is_ascii_punctuation() => out.push(escaped),
                _ => return None,
            },
            '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' => {
                return None;
            }
            _ => out.push(c),
        }
    }
    Some(out)
}

/// `text` without a leading mutant position: `\d+:\d+` or `<line>:<column>`.
fn strip_position(text: &str) -> Option<&str> {
    if let Some(rest) = text.strip_prefix(r"\d+:\d+") {
        return Some(rest);
    }
    let digits = |s: &str| s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let line = digits(text);
    let rest = text[line..].strip_prefix(':')?;
    let column = digits(rest);
    (line > 0 && column > 0).then(|| &rest[column..])
}

/// P4.7: why an `exclude_re` entry does not match exactly one mutant name in
/// an existing source, or `None` if it is well formed. The form is `^` + the
/// escaped path + `:` + a position + `: ` + the escaped description + `$`.
fn exclude_re_fault(entry: &str, sources: &[&str]) -> Option<String> {
    let Some(body) = entry.strip_prefix('^') else {
        return Some("does not start with `^`".to_owned());
    };
    let Some(body) = body.strip_suffix('$') else {
        return Some("does not end with `$`".to_owned());
    };
    if (body.len() - body.trim_end_matches('\\').len()) % 2 == 1 {
        return Some("ends with an escaped `$`".to_owned());
    }
    let Some((path, rest)) = body.split_once(':') else {
        return Some("has no `<path>:` prefix".to_owned());
    };
    let Some(path) = regex_literal(path) else {
        return Some(format!(
            "has a path `{path}` that is not an escaped literal"
        ));
    };
    if !sources.contains(&path.as_str()) {
        return Some(format!("names `{path}`, which is not a workspace source"));
    }
    let Some(description) = strip_position(rest).and_then(|rest| rest.strip_prefix(": ")) else {
        return Some("has no `\\d+:\\d+: ` position after the path".to_owned());
    };
    match regex_literal(description) {
        None => Some(format!(
            "has a description `{description}` that is not an escaped literal"
        )),
        Some(description) if description.trim().is_empty() => {
            Some("has an empty description".to_owned())
        }
        Some(_) => None,
    }
}

/// P4.6 and P4.7: `[config-exclusion]` for every `exclude_globs` and
/// `exclude_re` entry that is broader than one file or one mutant.
fn exclusion_entry_problems(
    workspace: &WorkspaceSources,
    globs: &[String],
    regexes: &[String],
    problems: &mut Vec<String>,
) {
    let sources = workspace.source_paths();
    for glob in globs {
        if glob.contains(['*', '?', '[', ']', '{', '}', '!']) || !sources.contains(&glob.as_str()) {
            problems.push(problem(
                "config-exclusion",
                format!("`exclude_globs` entry `{glob}` must be the exact path of an existing source under a member's src/"),
            ));
        }
    }
    for member in &workspace.members {
        if !member.sources.is_empty() && member.sources.iter().all(|(path, _)| globs.contains(path))
        {
            problems.push(problem(
                "config-exclusion",
                format!("`exclude_globs` excludes every source of {}", member.dir),
            ));
        }
    }
    for entry in regexes {
        if let Some(fault) = exclude_re_fault(entry, &sources) {
            problems.push(problem(
                "config-exclusion",
                format!("`exclude_re` entry `{entry}` {fault}; it must match exactly one mutant"),
            ));
        }
    }
}

// The triage document, docs/quality/mutants-triage.md (P4.8, P8). The test
// reads two things from it; everything else in it is for people.
//
// 1. Exactly one Markdown table whose header row is exactly
//
//        | Mutant | Disposition | Row | Reason | Excluded |
//
//    followed by a separator row (`|---|---|---|---|---|`) and one row per
//    triaged mutant (every missed or timed-out mutant, P8.2), up to the first
//    line that does not start with `|`:
//
//    - Mutant: the mutant's exact name, usually in a code span.
//    - Disposition: `killed`, `equivalent`, `accepted A`, `accepted B` or
//      `accepted C` (the classes of P8.2).
//    - Row: the id of the matrix row that owns the code, e.g. `LIB-TOOL-001`.
//    - Reason: the argument; non-empty for an excluded mutant.
//    - Excluded: `—` (or `-`, `no`, empty) if the configuration does not
//      exclude it; otherwise a code span `<key>: <entry>`, where <key> is
//      `exclude_re`, `exclude_globs` or `skip_calls`, and <entry> is the
//      entry's string exactly as `.cargo/mutants.toml` has it. Only an
//      `equivalent` or `accepted` mutant may be excluded.
//
//    As in GitHub Markdown, a `|` inside a cell, code spans included, is
//    written `\|`. So the regex `\|\|` is written `\\|\\|`.
//
// 2. The `skip_calls_defaults` choice, as the text
//    `skip_calls_defaults = true` or `skip_calls_defaults = false`, matching
//    the configuration (absent there means `true`, cargo-mutants' default).
//
// The cross-check: every `exclude_re`, `exclude_globs` and `skip_calls` entry
// has a row whose Excluded cell names it, with a reason and a matrix row
// (`[exclusion-undocumented]`); every Excluded cell names an entry the
// configuration has (`[exclusion-stale]`); a malformed table is
// `[triage-format]`.

const TRIAGE_HEADER: [&str; 5] = ["Mutant", "Disposition", "Row", "Reason", "Excluded"];
const TRIAGE_DISPOSITIONS: [&str; 5] = [
    "killed",
    "equivalent",
    "accepted A",
    "accepted B",
    "accepted C",
];

/// One row of the triage table.
#[derive(Debug)]
struct TriageEntry {
    line: usize,
    disposition: String,
    row: String,
    reason: String,
    excluded: Option<(String, String)>,
}

/// The cells of a Markdown table row, split on `|` not preceded by `\`, with
/// `\|` read as `|`.
fn table_cells(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = match line.strip_suffix('|') {
        Some(rest) if !rest.ends_with('\\') => rest,
        _ => line,
    };
    let mut cells = vec![String::new()];
    let mut previous = None;
    for c in line.chars() {
        if c == '|' && previous != Some('\\') {
            cells.push(String::new());
        } else if let Some(cell) = cells.last_mut() {
            cell.push(c);
        }
        previous = Some(c);
    }
    cells
        .into_iter()
        .map(|cell| cell.replace("\\|", "|").trim().to_owned())
        .collect()
}

/// The text of a cell without a surrounding code span.
fn code_span_text(cell: &str) -> &str {
    let ticks = cell.len() - cell.trim_start_matches('`').len();
    if ticks == 0 || cell.len() < 2 * ticks || !cell.ends_with(&"`".repeat(ticks)) {
        return cell;
    }
    let inner = &cell[ticks..cell.len() - ticks];
    match inner.strip_prefix(' ').and_then(|i| i.strip_suffix(' ')) {
        Some(stripped) if !stripped.trim().is_empty() => stripped,
        _ => inner,
    }
}

/// The triage table's rows; `[triage-format]` for anything malformed.
fn triage_entries(triage: &str, problems: &mut Vec<String>) -> Vec<TriageEntry> {
    let lines: Vec<&str> = triage.lines().collect();
    let headers: Vec<usize> = (0..lines.len())
        .filter(|&i| {
            lines[i].trim_start().starts_with('|') && table_cells(lines[i]) == TRIAGE_HEADER
        })
        .collect();
    let [header] = headers.as_slice() else {
        problems.push(problem(
            "triage-format",
            format!(
                "the triage document must have exactly one table headed `| {} |`, found {}",
                TRIAGE_HEADER.join(" | "),
                headers.len()
            ),
        ));
        return Vec::new();
    };
    let separator = lines.get(header + 1).map(|l| table_cells(l));
    if !separator.is_some_and(|cells| {
        cells.len() == TRIAGE_HEADER.len()
            && cells
                .iter()
                .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
    }) {
        problems.push(problem(
            "triage-format",
            "the triage table has no separator row",
        ));
    }
    let mut entries = Vec::new();
    for (index, line) in lines.iter().enumerate().skip(header + 2) {
        if !line.trim_start().starts_with('|') {
            break;
        }
        let number = index + 1;
        let cells = table_cells(line);
        let [_, disposition, row, reason, excluded] = cells.as_slice() else {
            problems.push(problem(
                "triage-format",
                format!(
                    "triage line {number} must have 5 cells, found {}",
                    cells.len()
                ),
            ));
            continue;
        };
        if !TRIAGE_DISPOSITIONS.contains(&disposition.as_str()) {
            problems.push(problem("triage-format", format!("triage line {number}: unknown disposition `{disposition}`; allowed: {TRIAGE_DISPOSITIONS:?}")));
        }
        let excluded = code_span_text(excluded);
        let excluded = if ["", "—", "-", "no"].contains(&excluded) {
            None
        } else if let Some((key, entry)) = excluded
            .split_once(": ")
            .filter(|(key, _)| EXCLUSION_KEYS.contains(key))
        {
            Some((key.to_owned(), entry.to_owned()))
        } else {
            problems.push(problem("triage-format", format!("triage line {number}: the Excluded cell must be `—` or `<key>: <entry>` with a key in {EXCLUSION_KEYS:?}, found `{excluded}`")));
            None
        };
        entries.push(TriageEntry {
            line: number,
            disposition: disposition.clone(),
            row: row.clone(),
            reason: reason.clone(),
            excluded,
        });
    }
    entries
}

/// P4.8: every exclusion of the configuration is triaged with a reason and a
/// matrix row, and every triaged exclusion is in the configuration.
fn triage_cross_check(
    config: &toml::Table,
    exclusions: &[(String, String)],
    triage: &str,
    matrix_ids: &[String],
    problems: &mut Vec<String>,
) {
    let entries = triage_entries(triage, problems);
    for (key, entry) in exclusions {
        let documented = entries
            .iter()
            .find(|e| e.excluded.as_ref() == Some(&(key.clone(), entry.clone())));
        let fault = match documented {
            None => Some("has no triage entry".to_owned()),
            Some(e) if e.reason.trim().is_empty() => {
                Some(format!("has an empty reason (triage line {})", e.line))
            }
            Some(e) if !matrix_ids.contains(&e.row) => Some(format!(
                "cites `{}`, which is not a matrix row (triage line {})",
                e.row, e.line
            )),
            Some(e)
                if !e.disposition.starts_with("equivalent")
                    && !e.disposition.starts_with("accepted") =>
            {
                Some(format!(
                    "is triaged as `{}`, but only an equivalent or accepted mutant may be excluded (triage line {})",
                    e.disposition, e.line
                ))
            }
            Some(_) => None,
        };
        if let Some(fault) = fault {
            problems.push(problem(
                "exclusion-undocumented",
                format!("the `{key}` entry `{entry}` {fault}"),
            ));
        }
    }
    for e in &entries {
        if let Some((key, entry)) = &e.excluded {
            if !exclusions.contains(&(key.clone(), entry.clone())) {
                problems.push(problem(
                    "exclusion-stale",
                    format!("triage line {} says `{key}: {entry}` is excluded, but {MUTANTS_CONFIG} has no such entry", e.line),
                ));
            }
        }
    }
    let defaults = match config.get("skip_calls_defaults") {
        None => true,
        Some(toml::Value::Boolean(value)) => *value,
        Some(other) => {
            problems.push(problem(
                "config-exclusion",
                format!("`skip_calls_defaults` must be a boolean, found `{other}`"),
            ));
            true
        }
    };
    let stated = triage.contains(&format!("skip_calls_defaults = {defaults}"));
    let contradicted = triage.contains(&format!("skip_calls_defaults = {}", !defaults));
    if !stated || contradicted {
        problems.push(problem(
            "exclusion-undocumented",
            format!("the triage document must state `skip_calls_defaults = {defaults}`, the configuration's choice, and not the other value"),
        ));
    }
}

/// LIB-QA-007 P4: `.cargo/mutants.toml`, cross-checked with the triage
/// document, and the workspace's sources and manifests scanned for skips.
fn check_mutants_config(
    config: &str,
    workspace: &WorkspaceSources,
    triage: &str,
    matrix_ids: &[String],
) -> Vec<String> {
    let mut problems = source_skip_problems(workspace);
    let table: toml::Table = match config.parse() {
        Ok(table) => table,
        Err(e) => {
            problems.push(problem(
                "config-parse",
                format!("{MUTANTS_CONFIG} does not parse: {e}"),
            ));
            return problems;
        }
    };
    for (key, value) in &table {
        if !REVIEWED_MUTANTS_KEYS.contains(&key.as_str()) {
            problems.push(problem(
                "unreviewed-key",
                format!("{MUTANTS_CONFIG} has the key `{key}`, which has not been reviewed"),
            ));
        } else if let Some((_, reviewed, _)) = NEXTEST_MUTANTS_KEYS
            .iter()
            .find(|(fixed, _, _)| fixed == key)
        {
            if *value != toml_value(reviewed) {
                problems.push(problem(
                    "unreviewed-key",
                    format!("{MUTANTS_CONFIG} has `{key} = {value}`; only `{key} = {reviewed}` has been reviewed (ADR 0009)"),
                ));
            }
        }
    }
    let is_true = |key: &str| matches!(table.get(key), Some(toml::Value::Boolean(true)));
    require(
        &mut problems,
        is_true("cap_lints"),
        "config-cap-lints",
        "`cap_lints` must be exactly true (D5)",
    );
    require(
        &mut problems,
        is_true("all_features"),
        "config-features",
        "`all_features` must be exactly true (D6)",
    );
    require(
        &mut problems,
        matches!(
            table.get("test_workspace"),
            Some(toml::Value::Boolean(false))
        ),
        "config-test-scope",
        "`test_workspace` must be stated, exactly false (D8)",
    );
    let globs = config_strings(&table, "exclude_globs", &mut problems);
    let regexes = config_strings(&table, "exclude_re", &mut problems);
    let calls = config_strings(&table, "skip_calls", &mut problems);
    exclusion_entry_problems(workspace, &globs, &regexes, &mut problems);
    let exclusions: Vec<(String, String)> = [
        ("exclude_re", &regexes),
        ("exclude_globs", &globs),
        ("skip_calls", &calls),
    ]
    .into_iter()
    .flat_map(|(key, entries)| entries.iter().map(move |e| (key.to_owned(), e.clone())))
    .collect();
    triage_cross_check(&table, &exclusions, triage, matrix_ids, &mut problems);
    problems
}

/// LIB-QA-007 P5 (D12): `.gitignore` ignores `/mutants.out*` and re-includes
/// nothing. Each line has its trailing space characters (U+0020) stripped,
/// and nothing else: git strips only unescaped trailing spaces (gitignore(5),
/// dir.c `trim_trailing_spaces`), so a trailing tab, a backslash-escaped
/// trailing space and any leading whitespace stay part of the pattern.
/// Every line whose first character is `!` negates a pattern and is
/// reported, whatever it names: telling which paths a negation re-includes
/// would mean reimplementing git's matching, and D12 allows none. An
/// indented `!` or an escaped `\!` is a literal, not a negation.
fn check_mutants_gitignore(gitignore: &str) -> Vec<String> {
    let lines: Vec<&str> = gitignore.lines().map(|l| l.trim_end_matches(' ')).collect();
    let mut problems = Vec::new();
    require(
        &mut problems,
        lines.contains(&MUTANTS_GITIGNORE_RULE),
        "gitignore-mutants",
        "`.gitignore` must contain the line `/mutants.out*`",
    );
    for line in lines.iter().filter(|l| l.starts_with('!')) {
        problems.push(problem(
            "gitignore-mutants",
            format!("`.gitignore` line `{line}` negates a pattern; D12 re-includes nothing"),
        ));
    }
    problems
}

/// The reference `.cargo/mutants.toml` (P4; D5, D6, D8). With no exclusions,
/// which is what the configuration is to have until the baseline triage
/// excludes an equivalent or accepted mutant.
const CONFIG_FIXTURE: &str =
    "# cargo-mutants configuration (LIB-QA-007). Every exclusion here is listed,
# with its reason and owning row, in docs/quality/mutants-triage.md.

# A lint that `-D warnings` denies must not turn a mutant unviable (D5).
cap_lints = true

# Build and test with every feature, as CI does (D6).
all_features = true

# A mutant must be killed by its own package's tests, not another package's (D8).
test_workspace = false
";

/// ADR 0009 (LIB-QA-012 item 1): the test tool and its arguments, appended to
/// [`CONFIG_FIXTURE`].
const NEXTEST_MUTANTS_LINES: &str = "
# Run the tests under nextest, with the profile that stops every running test
# at the first failure (ADR 0009, LIB-QA-012).
test_tool = \"nextest\"
additional_cargo_test_args = [\"--profile=mutants\", \"--no-tests=warn\"]
";

/// An example of each kind of exclusion, in the form P4.6 and P4.7 accept.
/// (Not real triage: the mutant is made up.)
const CONFIG_EXCLUSIONS: &str = r#"
exclude_re = [
  '^xtask/src/fetch\.rs:\d+:\d+: replace example -> bool with true$',
]
exclude_globs = ["xtask/src/main.rs"]
skip_calls = ["example_call"]
"#;

/// The reference triage document's machine-read parts (P4.8; the format is
/// described above [`TRIAGE_HEADER`]).
const TRIAGE_FIXTURE: &str = "# Mutation-testing triage

`skip_calls_defaults = true`: cargo-mutants' default, so calls to `with_capacity` are not mutated.

| Mutant | Disposition | Row | Reason | Excluded |
|---|---|---|---|---|
| `xtask/src/lib.rs:20:5: replace repo_root -> PathBuf with Default::default()` | killed | LIB-TOOL-001 | A test cited under LIB-TOOL-001 asserts that the root holds `xtask/Cargo.toml`. | — |
";

/// The triage rows for [`CONFIG_EXCLUSIONS`].
const TRIAGE_EXCLUSIONS: &str = r"| `xtask/src/fetch.rs:10:5: replace example -> bool with true` | equivalent | LIB-TOOL-002 | Example: no reachable input tells it apart. | `exclude_re: ^xtask/src/fetch\.rs:\d+:\d+: replace example -> bool with true$` |
| every mutant in `xtask/src/main.rs` | accepted A | LIB-TOOL-001 | Example: compiled out on Linux. | `exclude_globs: xtask/src/main.rs` |
| every call to `example_call` | equivalent | LIB-TOOL-001 | Example: the call has no observable effect. | `skip_calls: example_call` |
";

fn config_with_exclusions() -> String {
    format!("{CONFIG_FIXTURE}{CONFIG_EXCLUSIONS}")
}

fn triage_with_exclusions() -> String {
    format!("{TRIAGE_FIXTURE}{TRIAGE_EXCLUSIONS}")
}

/// A configuration mutation: the config, triage and workspace the check
/// reads, and the codes it must report.
struct ConfigCase {
    name: String,
    config: String,
    triage: String,
    workspace: WorkspaceSources,
    codes: &'static [&'static str],
}

fn config_case(
    name: impl Into<String>,
    config: String,
    triage: String,
    codes: &'static [&'static str],
) -> ConfigCase {
    ConfigCase {
        name: name.into(),
        config,
        triage,
        workspace: real_workspace(),
        codes,
    }
}

/// `CONFIG_FIXTURE` with `lines` appended, and the reference triage.
fn config_plus(name: impl Into<String>, lines: &str, codes: &'static [&'static str]) -> ConfigCase {
    config_case(
        name,
        format!("{CONFIG_FIXTURE}{lines}\n"),
        TRIAGE_FIXTURE.to_owned(),
        codes,
    )
}

/// The real `.cargo/mutants.toml`, triage document, workspace and matrix
/// must pass P4. A missing file is `[config-missing]` or `[triage-missing]`.
fn assert_real_mutants_config() {
    let config = read_optional(MUTANTS_CONFIG);
    let triage = read_optional(MUTANTS_TRIAGE);
    let mut problems = Vec::new();
    require(
        &mut problems,
        config.is_some(),
        "config-missing",
        &format!("{MUTANTS_CONFIG} does not exist"),
    );
    require(
        &mut problems,
        triage.is_some(),
        "triage-missing",
        &format!("{MUTANTS_TRIAGE} does not exist"),
    );
    problems.extend(check_mutants_config(
        config.as_deref().unwrap_or_default(),
        &real_workspace(),
        triage.as_deref().unwrap_or_default(),
        &matrix_ids(),
    ));
    assert!(
        problems.is_empty(),
        "LIB-QA-007: {MUTANTS_CONFIG} must be the reviewed configuration, and agree with {MUTANTS_TRIAGE}: {problems:#?}"
    );
}

/// P4 applied three ways: the reference configurations pass, every case is
/// reported with its codes, and then the real configuration passes.
fn assert_config_cases(cases: &[ConfigCase]) {
    let ids = matrix_ids();
    let workspace = real_workspace();
    for (name, config, triage) in [
        (
            "the reference configuration",
            CONFIG_FIXTURE.to_owned(),
            TRIAGE_FIXTURE.to_owned(),
        ),
        (
            "the reference exclusions",
            config_with_exclusions(),
            triage_with_exclusions(),
        ),
    ] {
        let problems = check_mutants_config(&config, &workspace, &triage, &ids);
        assert!(
            problems.is_empty(),
            "LIB-QA-007: {name} must pass before its mutations are meaningful: {problems:#?}"
        );
    }
    for case in cases {
        assert!(
            !case.codes.is_empty(),
            "LIB-QA-007: the case `{}` names no expected code",
            case.name
        );
        let problems = check_mutants_config(&case.config, &case.workspace, &case.triage, &ids);
        for code in case.codes {
            assert!(
                has_code(&problems, code),
                "LIB-QA-007: the config check must report `{}` as `[{code}]`; it reported {problems:#?}; config:\n{}\ntriage:\n{}",
                case.name,
                case.config,
                case.triage
            );
        }
    }
    assert_real_mutants_config();
}

#[test]
fn lib_qa_007_mutants_config_is_reviewed() {
    assert_config_cases(&[]);
}

/// P4.2: keys outside [`REVIEWED_MUTANTS_KEYS`], or (ADR 0009) a key of
/// [`NEXTEST_MUTANTS_KEYS`] with another value, each with a value that would
/// narrow or redirect the run.
const UNREVIEWED_MUTANTS_KEYS: [(&str, &str); 13] = [
    ("additional_cargo_args", r#"["--release"]"#),
    ("additional_cargo_test_args", r#"["--", "--skip", "lib_"]"#),
    ("output", r#""elsewhere""#),
    ("profile", r#""mutants""#),
    ("test_tool", r#""cargo""#),
    ("test_package", r#"["sacm"]"#),
    ("copy_target", "true"),
    ("copy_vcs", "true"),
    ("gitignore", "false"),
    ("features", r#"["unstable"]"#),
    ("no_default_features", "true"),
    ("examine_globs", r#"["xtask/src/lib.rs"]"#),
    ("examine_re", r#"["matrix"]"#),
];

#[test]
fn lib_qa_007_mutants_config_check_rejects_parse_errors_and_unreviewed_keys() {
    let mut cases = vec![
        config_case(
            "a key without a value",
            "cap_lints =\n".to_owned(),
            TRIAGE_FIXTURE.to_owned(),
            &["config-parse"],
        ),
        config_plus("a duplicate key", "cap_lints = true", &["config-parse"]),
        config_plus(
            "an unterminated table header",
            "[unterminated",
            &["config-parse"],
        ),
        config_case(
            "not TOML at all",
            "{ \"cap_lints\": true }".to_owned(),
            TRIAGE_FIXTURE.to_owned(),
            &["config-parse"],
        ),
        config_plus("a future key", "future_key = true", &["unreviewed-key"]),
    ];
    for (key, value) in UNREVIEWED_MUTANTS_KEYS {
        cases.push(config_plus(
            format!("{key} = {value}"),
            &format!("{key} = {value}"),
            &["unreviewed-key"],
        ));
    }
    assert_config_cases(&cases);
}

/// ADR 0009: values of `test_tool` and `additional_cargo_test_args` other
/// than the reviewed ones, each of which is `[unreviewed-key]`.
const OTHER_NEXTEST_VALUES: [(&str, &str); 16] = [
    ("test_tool", r#""cargo""#),
    ("test_tool", r#""Nextest""#),
    ("test_tool", r#""nextest ""#),
    ("test_tool", r#"["nextest"]"#),
    ("test_tool", "true"),
    ("additional_cargo_test_args", "[]"),
    ("additional_cargo_test_args", r#"["--profile=mutants"]"#),
    ("additional_cargo_test_args", r#"["--no-tests=warn"]"#),
    (
        "additional_cargo_test_args",
        r#"["--profile=default", "--no-tests=warn"]"#,
    ),
    (
        "additional_cargo_test_args",
        r#"["--profile=mutants", "--no-tests=pass"]"#,
    ),
    (
        "additional_cargo_test_args",
        r#"["--no-tests=warn", "--profile=mutants"]"#,
    ),
    (
        "additional_cargo_test_args",
        r#"["--profile", "mutants", "--no-tests=warn"]"#,
    ),
    (
        "additional_cargo_test_args",
        r#"["--profile=mutants", "--no-tests=warn", "-E", "not test(lib_)"]"#,
    ),
    (
        "additional_cargo_test_args",
        r#"["--profile=mutants", "--no-tests=warn", "--no-fail-fast"]"#,
    ),
    (
        "additional_cargo_test_args",
        r#""--profile=mutants --no-tests=warn""#,
    ),
    ("additional_cargo_test_args", r#"["--", "--skip", "lib_"]"#),
];

/// ADR 0009, LIB-QA-012 item 4: the LIB-QA-007 config check accepts
/// `test_tool` and `additional_cargo_test_args` absent or with exactly their
/// reviewed values, and refuses every other value as `[unreviewed-key]`.
#[test]
fn lib_qa_007_mutants_config_check_accepts_only_the_reviewed_nextest_values() {
    let nextest = format!("{CONFIG_FIXTURE}{NEXTEST_MUTANTS_LINES}");
    let cases: Vec<ConfigCase> = OTHER_NEXTEST_VALUES
        .iter()
        .map(|(key, value)| {
            let reviewed = NEXTEST_MUTANTS_KEYS
                .iter()
                .find(|(fixed, _, _)| fixed == key)
                .map(|(_, reviewed, _)| format!("{key} = {reviewed}"))
                .expect("the key has a reviewed value");
            config_case(
                format!("{key} = {value}"),
                replace_once(&nextest, &reviewed, &format!("{key} = {value}")),
                TRIAGE_FIXTURE.to_owned(),
                &["unreviewed-key"],
            )
        })
        .collect();
    let ids = matrix_ids();
    let workspace = real_workspace();
    for case in &cases {
        let problems = check_mutants_config(&case.config, &workspace, &case.triage, &ids);
        assert_eq!(
            problem_codes(&problems),
            ["unreviewed-key"],
            "LIB-QA-007: `{}` must give exactly one `[unreviewed-key]`; it gave {problems:#?}",
            case.name
        );
        let key = case.name.split(" = ").next().unwrap_or_default();
        assert!(
            problems[0].contains(&format!("`{key} = ")),
            "LIB-QA-007: the `[unreviewed-key]` for `{}` must name the key and its value: {problems:#?}",
            case.name
        );
    }
    for (name, config) in [
        ("both keys with their reviewed values", nextest.clone()),
        (
            "the reviewed values as literal strings",
            nextest.replace(r#""nextest""#, "'nextest'").replace(
                r#"["--profile=mutants", "--no-tests=warn"]"#,
                "[\n  '--profile=mutants',\n  '--no-tests=warn',\n]",
            ),
        ),
        (
            "test_tool alone",
            replace_once(
                &nextest,
                "additional_cargo_test_args = [\"--profile=mutants\", \"--no-tests=warn\"]\n",
                "",
            ),
        ),
        ("neither key", CONFIG_FIXTURE.to_owned()),
    ] {
        let problems = check_mutants_config(&config, &workspace, TRIAGE_FIXTURE, &ids);
        assert!(
            problems.is_empty(),
            "LIB-QA-007: `{name}` is reviewed, but was reported: {problems:#?}; config:\n{config}"
        );
    }
    assert_config_cases(&cases);
}

#[test]
fn lib_qa_007_mutants_config_check_requires_cap_lints_features_and_test_scope() {
    let without = |line: &str| replace_once(CONFIG_FIXTURE, &format!("{line}\n"), "");
    let replaced = |line: &str, by: &str| replace_once(CONFIG_FIXTURE, line, by);
    let triage = || TRIAGE_FIXTURE.to_owned();
    assert_config_cases(&[
        config_case(
            "cap_lints absent",
            without("cap_lints = true"),
            triage(),
            &["config-cap-lints"],
        ),
        config_case(
            "cap_lints = false",
            replaced("cap_lints = true", "cap_lints = false"),
            triage(),
            &["config-cap-lints"],
        ),
        config_case(
            "cap_lints = \"true\"",
            replaced("cap_lints = true", "cap_lints = \"true\""),
            triage(),
            &["config-cap-lints"],
        ),
        config_case(
            "all_features absent",
            without("all_features = true"),
            triage(),
            &["config-features"],
        ),
        config_case(
            "all_features = false",
            replaced("all_features = true", "all_features = false"),
            triage(),
            &["config-features"],
        ),
        config_case(
            "test_workspace absent",
            without("test_workspace = false"),
            triage(),
            &["config-test-scope"],
        ),
        config_case(
            "test_workspace = true",
            replaced("test_workspace = false", "test_workspace = true"),
            triage(),
            &["config-test-scope"],
        ),
    ]);
}

#[test]
fn lib_qa_007_mutants_config_check_rejects_broad_exclude_globs() {
    let mut cases: Vec<ConfigCase> = [
        "xtask/**",
        "xtask/src/*.rs",
        "*.rs",
        "**",
        "src",
        "matrix.rs",
        "xtask/src/nonexistent.rs",
        "xtask/src/[lm]*.rs",
        "crates/sacm-cli/src/main.rs",
        "xtask/tests/repo_checks.rs",
    ]
    .iter()
    .map(|glob| {
        let triage = format!("{TRIAGE_FIXTURE}| every mutant in `{glob}` | accepted A | LIB-TOOL-001 | Example. | `exclude_globs: {glob}` |\n");
        config_case(
            format!("exclude_globs = [\"{glob}\"]"),
            format!("{CONFIG_FIXTURE}exclude_globs = [\"{glob}\"]\n"),
            triage,
            &["config-exclusion"],
        )
    })
    .collect();
    // Every source of xtask, read from the real workspace, so that a new file
    // under xtask/src/ cannot leave one out.
    let xtask_sources: Vec<String> = real_workspace()
        .members
        .iter()
        .find(|member| member.dir == "xtask")
        .expect("xtask is a workspace member")
        .sources
        .iter()
        .map(|(path, _)| format!("{path:?}"))
        .collect();
    assert!(
        xtask_sources.len() > 1,
        "LIB-QA-007: precondition: xtask has several sources to exclude one by one: {xtask_sources:?}"
    );
    cases.push(config_plus(
        "every source of xtask, one by one",
        &format!("exclude_globs = [{}]", xtask_sources.join(", ")),
        &["config-exclusion"],
    ));
    cases.push(config_plus(
        "exclude_globs as a string",
        r#"exclude_globs = "xtask/src/fetch.rs""#,
        &["config-exclusion"],
    ));
    assert_config_cases(&cases);
}

#[test]
fn lib_qa_007_mutants_config_check_rejects_broad_exclude_re() {
    let mut cases: Vec<ConfigCase> = [
        "",
        ".*",
        "xtask",
        "matrix",
        "replace .* with true",
        r"^xtask/src/matrix\.rs",
        r"^xtask/src/matrix\.rs:.*$",
        r"^xtask/src/nope\.rs:\d+:\d+: replace example -> bool with true$",
        r"^xtask/src/matrix.rs:\d+:\d+: replace example -> bool with true$",
        r"(?i)^xtask/src/matrix\.rs:\d+:\d+: replace example -> bool with true$",
        r"^xtask/src/matrix\.rs:\d+:\d+: replace .* with true$",
        r"^xtask/src/matrix\.rs:\d+:\d+: replace \w+ -> bool with true$",
        r"^xtask/src/matrix\.rs:\d+:\d+: $",
        r"^xtask/src/matrix\.rs:\d+:\d+: replace example -> bool with true\$",
        r"^xtask/src/matrix\.rs:\d+: replace example -> bool with true$",
        r"^xtask/src/matrix\.rs:\d+:\d+:replace example -> bool with true$",
        r"^xtask/src/matrix\.rs:\d+:\d+: replace example -> bool with true$|^.*$",
    ]
    .iter()
    .map(|entry| {
        let cell = entry.replace('|', "\\|");
        let triage = format!("{TRIAGE_FIXTURE}| `{cell}` | equivalent | LIB-TOOL-001 | Example. | `exclude_re: {cell}` |\n");
        config_case(
            format!("exclude_re = ['{entry}']"),
            format!("{CONFIG_FIXTURE}exclude_re = ['{entry}']\n"),
            triage,
            &["config-exclusion"],
        )
    })
    .collect();
    cases.push(config_plus(
        "exclude_re as a string",
        "exclude_re = 'matrix'",
        &["config-exclusion"],
    ));
    cases.push(config_plus(
        "skip_calls as a string",
        "skip_calls = 'example_call'",
        &["config-exclusion"],
    ));
    assert_config_cases(&cases);
}

/// P4.8: mutations of the reference exclusions and their triage rows.
fn triage_cases() -> Vec<ConfigCase> {
    let config = config_with_exclusions();
    let triage = triage_with_exclusions();
    let rows: Vec<&str> = TRIAGE_EXCLUSIONS.lines().collect();
    let triage_without = |row: &str| replace_once(&triage, &format!("{row}\n"), "");
    let triage_edit = |from: &str, to: &str| replace_once(&triage, from, to);
    let config_without = |text: &str| replace_once(&config, text, "");
    let undocumented: &'static [&'static str] = &["exclusion-undocumented"];
    vec![
        config_case(
            "the exclude_re row removed",
            config.clone(),
            triage_without(rows[0]),
            undocumented,
        ),
        config_case(
            "the exclude_globs row removed",
            config.clone(),
            triage_without(rows[1]),
            undocumented,
        ),
        config_case(
            "the skip_calls row removed",
            config.clone(),
            triage_without(rows[2]),
            undocumented,
        ),
        config_case(
            "an empty reason",
            config.clone(),
            triage_edit("| Example: no reachable input tells it apart. |", "|  |"),
            undocumented,
        ),
        config_case(
            "a row id not in the matrix",
            config.clone(),
            triage_edit(
                "| equivalent | LIB-TOOL-002 |",
                "| equivalent | LIB-NOPE-999 |",
            ),
            undocumented,
        ),
        config_case(
            "an excluded mutant triaged as killed",
            config.clone(),
            triage_edit("| equivalent | LIB-TOOL-002 |", "| killed | LIB-TOOL-002 |"),
            undocumented,
        ),
        config_case(
            "the exclude_re entry differs from its row",
            replace_once(
                &config,
                "replace example -> bool with true$",
                "replace example -> bool with false$",
            ),
            triage.clone(),
            &["exclusion-undocumented", "exclusion-stale"],
        ),
        config_case(
            "a triaged exclusion with no config entry",
            config_without("skip_calls = [\"example_call\"]\n"),
            triage.clone(),
            &["exclusion-stale"],
        ),
        config_case(
            "the skip_calls_defaults statement removed",
            config.clone(),
            triage_edit("`skip_calls_defaults = true`: ", ""),
            undocumented,
        ),
        config_case(
            "skip_calls_defaults stated false, default true",
            config.clone(),
            triage_edit(
                "`skip_calls_defaults = true`",
                "`skip_calls_defaults = false`",
            ),
            undocumented,
        ),
        config_case(
            "skip_calls_defaults = false, stated true",
            format!("{config}skip_calls_defaults = false\n"),
            triage.clone(),
            undocumented,
        ),
    ]
}

/// P4.8: a triage table the cross-check cannot read is `[triage-format]`.
fn triage_format_cases() -> Vec<ConfigCase> {
    let config = config_with_exclusions();
    let triage = triage_with_exclusions();
    let triage_edit = |from: &str, to: &str| replace_once(&triage, from, to);
    let format_code: &'static [&'static str] = &["triage-format"];
    vec![
        config_case(
            "no triage table",
            config.clone(),
            "# Mutation-testing triage\n".to_owned(),
            format_code,
        ),
        config_case(
            "the header changed",
            config.clone(),
            triage_edit("| Reason | Excluded |", "| Excluded |"),
            format_code,
        ),
        config_case(
            "a row with four cells",
            config.clone(),
            triage_edit(
                "| accepted A | LIB-TOOL-001 | Example: compiled out on Linux. |",
                "| accepted A | LIB-TOOL-001 |",
            ),
            format_code,
        ),
        config_case(
            "an unknown disposition",
            config.clone(),
            triage_edit("| accepted A |", "| unlikely |"),
            format_code,
        ),
        config_case(
            "an Excluded cell with an unknown key",
            config,
            triage_edit("`skip_calls: example_call`", "`skip: example_call`"),
            format_code,
        ),
    ]
}

#[test]
fn lib_qa_007_mutants_config_check_cross_checks_the_triage_document() {
    let mut cases = triage_cases();
    cases.extend(triage_format_cases());
    assert_config_cases(&cases);
}

/// P4.9: source-skip spellings, each appended to a workspace source.
const SOURCE_SKIPS: [&str; 13] = [
    "#[mutants::skip]\nfn skipped() {}\n",
    "#![mutants::skip]\n",
    "#[::mutants::skip]\nfn skipped() {}\n",
    "#[mutants :: skip]\nfn skipped() {}\n",
    "#[mutants/*c*/::skip]\nfn skipped() {}\n",
    "#[cfg_attr(test, mutants::skip)]\nfn skipped() {}\n",
    "#[cfg_attr(any(), mutants::skip)]\nfn skipped() {}\n",
    "#[cfg_attr(\n    test,\n    mutants\n        ::skip\n)]\nfn skipped() {}\n",
    "fn quote() -> char { '\"' }\n#[mutants::skip]\nfn skipped() {}\n",
    "const Q: char = '\\'';\n#[mutants::skip]\nfn skipped() {}\n",
    "const S: &str = \"/*\";\n#[mutants::skip]\nfn skipped() {}\n",
    "const R: &str = r#\"\" /*\"#;\n#[mutants::skip]\nfn skipped() {}\n",
    "fn f<'a>(s: &'a str) -> &'a str { s }\n#[mutants::skip]\nfn skipped() {}\n",
];

/// P4.9: text that mentions `mutants::skip` without being an attribute.
const SOURCE_NON_SKIPS: [&str; 6] = [
    "// #[mutants::skip]\n",
    "/// Not `#[mutants::skip]`.\nfn documented() {}\n",
    "/* #[mutants::skip] */\n",
    "/* nested /* #[mutants::skip] */ still a comment */\n",
    "const S: &str = \"#[mutants::skip]\";\n",
    "const R: &str = r#\"#[mutants::skip]\"#;\n",
];

#[test]
fn lib_qa_007_mutants_config_check_rejects_source_skips() {
    let workspace = real_workspace();
    let triage = || TRIAGE_FIXTURE.to_owned();
    let mut cases = Vec::new();
    for path in [
        "xtask/src/lib.rs",
        "crates/sacm-cli/src/main.rs",
        "crates/sacm/src/lib.rs",
    ] {
        for skip in SOURCE_SKIPS {
            cases.push(ConfigCase {
                name: format!("{skip:?} in {path}"),
                config: CONFIG_FIXTURE.to_owned(),
                triage: triage(),
                workspace: workspace.with_source(path, |text| format!("{text}\n{skip}")),
                codes: &["source-skip"],
            });
        }
    }
    let dependency = |name: &str, dir: &str, from: &str, to: &str| ConfigCase {
        name: name.to_owned(),
        config: CONFIG_FIXTURE.to_owned(),
        triage: triage(),
        workspace: workspace.with_manifest(dir, |text| replace_once(text, from, to)),
        codes: &["source-skip"],
    };
    // xtask's manifest may or may not have a `[dev-dependencies]` table, so
    // this control adds the dependency to the parsed manifest, creating the
    // table if needed; the edited manifest still parses, so the code comes
    // from the dependency and not from a parse failure.
    let dev_dependency = workspace.with_manifest("xtask", |text| {
        let mut manifest: toml::Table = text.parse().expect("xtask/Cargo.toml parses");
        manifest
            .entry("dev-dependencies")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .expect("[dev-dependencies] is a table")
            .insert("mutants".to_owned(), toml_value("\"0.0.3\""));
        let edited = manifest.to_string();
        assert!(
            edited.parse::<toml::Table>().is_ok(),
            "LIB-QA-007: precondition: the edited xtask manifest parses:\n{edited}"
        );
        edited
    });
    cases.extend([
        ConfigCase {
            name: "xtask dev-depends on mutants".to_owned(),
            config: CONFIG_FIXTURE.to_owned(),
            triage: triage(),
            workspace: dev_dependency,
            codes: &["source-skip"],
        },
        dependency(
            "sacm-cli depends on mutants, renamed",
            "crates/sacm-cli",
            "[dependencies]\n",
            "[dependencies]\nskip = { package = \"mutants\", version = \"0.0.3\" }\n",
        ),
        dependency(
            "a workspace dependency on mutants",
            "",
            "[workspace.dependencies]\n",
            "[workspace.dependencies]\nmutants = \"0.0.3\"\n",
        ),
        dependency(
            "sacm depends on mutants for one target",
            "crates/sacm",
            "[lints]\n",
            "[target.'cfg(unix)'.dependencies]\nmutants = \"0.0.3\"\n\n[lints]\n",
        ),
        dependency(
            "sacm build-depends on mutants",
            "crates/sacm",
            "[lints]\n",
            "[build-dependencies]\nmutants = \"0.0.3\"\n\n[lints]\n",
        ),
    ]);
    for text in SOURCE_NON_SKIPS {
        let edited =
            workspace.with_source("xtask/src/lib.rs", |source| format!("{source}\n{text}"));
        let problems = source_skip_problems(&edited);
        assert!(
            problems.is_empty(),
            "LIB-QA-007: {text:?} only mentions `mutants::skip`, but was reported: {problems:#?}"
        );
    }
    let problems = source_skip_problems(&workspace);
    assert!(
        problems.is_empty(),
        "LIB-QA-007: the real sources must have no mutants::skip: {problems:#?}"
    );
    assert_config_cases(&cases);
}

#[test]
fn lib_qa_007_mutants_config_check_accepts_reviewed_configuration() {
    let ids = matrix_ids();
    let workspace = real_workspace();
    let literal_position = r"'^xtask/src/fetch\.rs:10:5: replace example -> bool with true$'";
    let pipes_config =
        r"exclude_re = ['^xtask/src/matrix\.rs:\d+:\d+: replace \|\| with && in looks_traced$']";
    let pipes_row = r"| `xtask/src/matrix.rs:492:5: replace \|\| with && in looks_traced` | equivalent | LIB-TOOL-001 | Example. | `exclude_re: ^xtask/src/matrix\.rs:\d+:\d+: replace \\|\\| with && in looks_traced$` |";
    let controls = [
        (
            "the reference configuration",
            CONFIG_FIXTURE.to_owned(),
            TRIAGE_FIXTURE.to_owned(),
        ),
        (
            "the reference exclusions",
            config_with_exclusions(),
            triage_with_exclusions(),
        ),
        (
            "every other reviewed key",
            format!(
                "{CONFIG_FIXTURE}timeout_multiplier = 2.0\nminimum_test_timeout = 60.0\nbuild_timeout_multiplier = 3.0\nsharding = \"round-robin\"\nerror_values = ['::anyhow::anyhow!(\"mutated\")']\nskip_calls_defaults = true\n"
            ),
            TRIAGE_FIXTURE.to_owned(),
        ),
        (
            "skip_calls_defaults = false, stated false",
            format!("{CONFIG_FIXTURE}skip_calls_defaults = false\n"),
            TRIAGE_FIXTURE.replace("skip_calls_defaults = true", "skip_calls_defaults = false"),
        ),
        (
            "empty exclusion lists",
            format!("{CONFIG_FIXTURE}exclude_re = []\nexclude_globs = []\nskip_calls = []\n"),
            TRIAGE_FIXTURE.to_owned(),
        ),
        (
            "an exclude_re with a literal position",
            config_with_exclusions().replace(
                r"'^xtask/src/fetch\.rs:\d+:\d+: replace example -> bool with true$'",
                literal_position,
            ),
            triage_with_exclusions().replace(
                r"`exclude_re: ^xtask/src/fetch\.rs:\d+:\d+:",
                r"`exclude_re: ^xtask/src/fetch\.rs:10:5:",
            ),
        ),
        (
            "an exclude_re with escaped pipes",
            format!("{CONFIG_FIXTURE}{pipes_config}\n"),
            format!("{TRIAGE_FIXTURE}{pipes_row}\n"),
        ),
        (
            "an exclude_globs entry leaving other sources",
            format!("{CONFIG_FIXTURE}exclude_globs = [\"xtask/src/fetch.rs\"]\n"),
            format!(
                "{TRIAGE_FIXTURE}| every mutant in `xtask/src/fetch.rs` | accepted B | LIB-TOOL-002 | Example. | `exclude_globs: xtask/src/fetch.rs` |\n"
            ),
        ),
        (
            "an excluded cell without a code span, and a not-excluded `-`",
            config_with_exclusions(),
            triage_with_exclusions()
                .replace("`skip_calls: example_call`", "skip_calls: example_call")
                .replace("| — |", "| - |"),
        ),
        (
            "the test tool and its arguments of ADR 0009",
            format!("{CONFIG_FIXTURE}{NEXTEST_MUTANTS_LINES}"),
            TRIAGE_FIXTURE.to_owned(),
        ),
    ];
    for (name, config, triage) in &controls {
        let problems = check_mutants_config(config, &workspace, triage, &ids);
        assert!(
            problems.is_empty(),
            "LIB-QA-007: `{name}` is a reviewed configuration, but was reported: {problems:#?}; config:\n{config}\ntriage:\n{triage}"
        );
    }
    assert_real_mutants_config();
}

/// The reference `.gitignore` rule for cargo-mutants' output (P5, D12).
const GITIGNORE_FIXTURE: &str = "/target\n\n# cargo-mutants output (LIB-QA-007).\n/mutants.out*\n";

#[test]
fn lib_qa_007_mutants_output_is_git_ignored() {
    let g = GITIGNORE_FIXTURE;
    let problems = check_mutants_gitignore(g);
    assert!(
        problems.is_empty(),
        "LIB-QA-007: the reference rule must pass: {problems:#?}"
    );
    for (name, text) in [
        ("rule removed", delete_line(g, MUTANTS_GITIGNORE_RULE)),
        (
            "rule commented out",
            comment_line(g, MUTANTS_GITIGNORE_RULE),
        ),
        (
            "rule without its *",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "/mutants.out"),
        ),
        (
            "rule under another directory",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "/target/mutants.out*"),
        ),
        ("a re-include", format!("{g}!/mutants.out/outcomes.json\n")),
        // A negation un-ignores `mutants.out` without naming it literally;
        // every `!` line is reported (D12 re-includes nothing).
        ("a re-include of everything", format!("{g}!*\n")),
        ("a re-include by wildcard", format!("{g}!/mutants.ou?\n")),
        // gitignore(5) strips trailing spaces only: an indented line is a
        // different pattern and does not ignore the root `mutants.out*`.
        (
            "rule indented",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "  /mutants.out*"),
        ),
        (
            "rule indented, with trailing spaces",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "  /mutants.out*  "),
        ),
        // git strips trailing spaces, not tabs (dir.c `trim_trailing_spaces`):
        // `/mutants.out*<TAB>` does not ignore `mutants.out`.
        (
            "rule with a trailing tab",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "/mutants.out*\t"),
        ),
        // A backslash-escaped trailing space is kept by git, so the pattern
        // ends in a literal space and does not ignore `mutants.out`.
        (
            "rule with an escaped trailing space",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "/mutants.out*\\ "),
        ),
    ] {
        let problems = check_mutants_gitignore(&text);
        assert!(
            has_code(&problems, "gitignore-mutants"),
            "LIB-QA-007: the mutation `{name}` must be reported as `[gitignore-mutants]`; it reported {problems:#?}; text:\n{text:?}"
        );
    }
    for (name, text) in [
        // gitignore(5) ignores unescaped trailing spaces.
        (
            "rule with trailing spaces",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "/mutants.out*  "),
        ),
        // An indented `!` line is not a negation (gitignore(5): the prefix
        // `!` negates), so it re-includes nothing.
        (
            "an indented `!` line",
            format!("{g}  !/mutants.out/outcomes.json\n"),
        ),
        // gitignore(5): a backslash before the first `!` makes it literal.
        ("an escaped `\\!` line", format!("{g}\\!/mutants.out\n")),
        // git reads CRLF line endings; `str::lines` strips the `\r`.
        ("CRLF line endings", g.replace('\n', "\r\n")),
    ] {
        let problems = check_mutants_gitignore(&text);
        assert!(
            problems.is_empty(),
            "LIB-QA-007: `{name}` ignores `mutants.out*` and re-includes nothing, but was reported: {problems:#?}; text:\n{text:?}"
        );
    }
    let problems = check_mutants_gitignore(&read(".gitignore"));
    assert!(
        problems.is_empty(),
        "LIB-QA-007: .gitignore must ignore `/mutants.out*`: {problems:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-QA-012: mutation outcomes do not depend on test-target order.
//
// ADR 0009: cargo-mutants runs the tests under nextest, with a profile that
// kills every running test at the first failure. The check reads the two
// files that make that so, and reports each part lost with its own code:
//
// - `.cargo/mutants.toml` (item 1): `test_tool` (`[nextest-test-tool]`) and
//   `additional_cargo_test_args` (`[nextest-test-args]`), each exactly its
//   reviewed value in [`NEXTEST_MUTANTS_KEYS`].
// - `.config/nextest.toml` (item 2): the version requirement
//   (`[nextest-version]`); the `mutants` profile (`[nextest-profile]`), its
//   `fail-fast` table (`[nextest-fail-fast]`), `max-fail = 1`
//   (`[nextest-max-fail]`) and `terminate = "immediate"`
//   (`[nextest-terminate]`); and no `fail-fast` in the `default` profile
//   (`[nextest-default-fail-fast]`). Every other key is
//   `[nextest-unreviewed-key]`: every profile inherits the `default` one, so
//   a key such as `default-filter`, `retries`, `slow-timeout` or `overrides`
//   there, or in the `mutants` profile, changes which tests run or what
//   counts as a failure. Other profiles are not read: `mutants` does not
//   inherit them.
//
// A file missing is `[nextest-config-missing]`, and one that does not parse
// `[nextest-config-parse]`. Versions are compared as numbers, so 0.9.111 is
// above 0.9.99.
// ---------------------------------------------------------------------------

const QA12: &str = "LIB-QA-012";

const NEXTEST_CONFIG: &str = ".config/nextest.toml";

/// A release version `MAJOR.MINOR.PATCH`, compared as numbers.
type Version = (u64, u64, u64);

/// ADR 0009: the first nextest with `terminate = "immediate"`.
const NEXTEST_MINIMUM: Version = (0, 9, 111);

/// The keys `.config/nextest.toml` may have at its top level.
const NEXTEST_TOP_KEYS: [&str; 2] = ["nextest-version", "profile"];

fn show_version((major, minor, patch): Version) -> String {
    format!("{major}.{minor}.{patch}")
}

/// `text` as a plain release version: three decimal numbers, none with a
/// sign or a leading zero, and no pre-release or build suffix.
fn plain_version(text: &str) -> Option<Version> {
    let number = |part: &str| {
        let digits = !part.is_empty() && part.chars().all(|c| c.is_ascii_digit());
        let canonical = part == "0" || !part.starts_with('0');
        if digits && canonical {
            part.parse::<u64>().ok()
        } else {
            None
        }
    };
    let mut parts = text.split('.');
    let version = (
        number(parts.next()?)?,
        number(parts.next()?)?,
        number(parts.next()?)?,
    );
    parts.next().is_none().then_some(version)
}

/// Item 1: `test_tool` and `additional_cargo_test_args` of
/// `.cargo/mutants.toml` (`None`: it does not exist).
fn nextest_mutants_key_problems(config: Option<&str>, problems: &mut Vec<String>) {
    let Some(config) = config else {
        problems.push(problem(
            "nextest-config-missing",
            format!("{MUTANTS_CONFIG} does not exist, so cargo-mutants does not run the tests under nextest"),
        ));
        return;
    };
    let table: toml::Table = match config.parse() {
        Ok(table) => table,
        Err(e) => {
            problems.push(problem(
                "nextest-config-parse",
                format!("{MUTANTS_CONFIG} does not parse: {e}"),
            ));
            return;
        }
    };
    for (key, reviewed, code) in NEXTEST_MUTANTS_KEYS {
        match table.get(key) {
            None => problems.push(problem(
                code,
                format!("{MUTANTS_CONFIG} must set `{key} = {reviewed}`; it does not set `{key}`"),
            )),
            Some(value) if *value != toml_value(reviewed) => problems.push(problem(
                code,
                format!("{MUTANTS_CONFIG} must set `{key} = {reviewed}` exactly, found `{key} = {value}`"),
            )),
            Some(_) => {}
        }
    }
}

/// Item 2: `nextest-version`, written `"X.Y.Z"` or `{ required = "X.Y.Z" }`
/// (with an optional `recommended`), at least [`NEXTEST_MINIMUM`].
fn nextest_version_problems(value: Option<&toml::Value>, problems: &mut Vec<String>) {
    let code = "nextest-version";
    let minimum = show_version(NEXTEST_MINIMUM);
    let Some(value) = value else {
        problems.push(problem(
            code,
            format!("{NEXTEST_CONFIG} must set `nextest-version = {{ required = \"{minimum}\" }}` or later; it does not set `nextest-version`"),
        ));
        return;
    };
    let required = match value {
        toml::Value::String(text) => Some(text.as_str()),
        toml::Value::Table(table) => {
            for key in table
                .keys()
                .filter(|key| !["required", "recommended"].contains(&key.as_str()))
            {
                problems.push(problem(
                    "nextest-unreviewed-key",
                    format!("{NEXTEST_CONFIG}'s `nextest-version` has the key `{key}`, which has not been reviewed"),
                ));
            }
            table.get("required").and_then(toml::Value::as_str)
        }
        _ => None,
    };
    match required.and_then(plain_version) {
        None => problems.push(problem(
            code,
            format!("{NEXTEST_CONFIG}'s `nextest-version` must require one version `X.Y.Z`, at least {minimum}; found `nextest-version = {value}`"),
        )),
        Some(version) if version < NEXTEST_MINIMUM => problems.push(problem(
            code,
            format!("{NEXTEST_CONFIG} requires nextest {}, below {minimum}, the first version with `terminate = \"immediate\"`", show_version(version)),
        )),
        Some(_) => {}
    }
}

/// Item 2: the `mutants` profile sets exactly `fail-fast = { max-fail = 1,
/// terminate = "immediate" }`.
fn mutants_profile_problems(profile: &toml::Table, problems: &mut Vec<String>) {
    for key in profile.keys().filter(|key| key.as_str() != "fail-fast") {
        problems.push(problem(
            "nextest-unreviewed-key",
            format!("{NEXTEST_CONFIG}'s `[profile.mutants]` has the key `{key}`, which has not been reviewed"),
        ));
    }
    let fail_fast = match profile.get("fail-fast") {
        Some(toml::Value::Table(fail_fast)) => fail_fast,
        other => {
            problems.push(problem(
                "nextest-fail-fast",
                format!(
                    "{NEXTEST_CONFIG}'s `[profile.mutants]` must set `fail-fast = {{ max-fail = 1, terminate = \"immediate\" }}`, found {}",
                    other.map_or_else(|| "no `fail-fast`".to_owned(), |v| format!("`fail-fast = {v}`"))
                ),
            ));
            return;
        }
    };
    for key in fail_fast
        .keys()
        .filter(|key| !["max-fail", "terminate"].contains(&key.as_str()))
    {
        problems.push(problem(
            "nextest-unreviewed-key",
            format!("{NEXTEST_CONFIG}'s `[profile.mutants]` fail-fast has the key `{key}`, which has not been reviewed"),
        ));
    }
    let shown = |key: &str| {
        fail_fast
            .get(key)
            .map_or_else(|| "none".to_owned(), |v| format!("`{key} = {v}`"))
    };
    if fail_fast.get("max-fail") != Some(&toml::Value::Integer(1)) {
        problems.push(problem(
            "nextest-max-fail",
            format!("{NEXTEST_CONFIG}'s `[profile.mutants]` fail-fast must have `max-fail = 1`, found {}", shown("max-fail")),
        ));
    }
    if fail_fast.get("terminate").and_then(toml::Value::as_str) != Some("immediate") {
        problems.push(problem(
            "nextest-terminate",
            format!("{NEXTEST_CONFIG}'s `[profile.mutants]` fail-fast must have `terminate = \"immediate\"`, found {}", shown("terminate")),
        ));
    }
}

/// Item 2: `.config/nextest.toml` (`None`: it does not exist).
fn nextest_config_problems(text: Option<&str>, problems: &mut Vec<String>) {
    let Some(text) = text else {
        problems.push(problem(
            "nextest-config-missing",
            format!("{NEXTEST_CONFIG} does not exist, so there is no `mutants` profile"),
        ));
        return;
    };
    let table: toml::Table = match text.parse() {
        Ok(table) => table,
        Err(e) => {
            problems.push(problem(
                "nextest-config-parse",
                format!("{NEXTEST_CONFIG} does not parse: {e}"),
            ));
            return;
        }
    };
    for key in table
        .keys()
        .filter(|key| !NEXTEST_TOP_KEYS.contains(&key.as_str()))
    {
        problems.push(problem(
            "nextest-unreviewed-key",
            format!("{NEXTEST_CONFIG} has the key `{key}`, which has not been reviewed"),
        ));
    }
    nextest_version_problems(table.get("nextest-version"), problems);
    let profiles = table.get("profile").and_then(toml::Value::as_table);
    match profiles
        .and_then(|profiles| profiles.get("mutants"))
        .and_then(toml::Value::as_table)
    {
        Some(profile) => mutants_profile_problems(profile, problems),
        None => problems.push(problem(
            "nextest-profile",
            format!("{NEXTEST_CONFIG} has no `[profile.mutants]` table"),
        )),
    }
    match profiles.and_then(|profiles| profiles.get("default")) {
        None => {}
        Some(toml::Value::Table(default)) => {
            for key in default.keys() {
                if key == "fail-fast" {
                    problems.push(problem(
                        "nextest-default-fail-fast",
                        format!("{NEXTEST_CONFIG}'s `[profile.default]` sets `fail-fast`; developers' `cargo nextest run` keeps nextest's own"),
                    ));
                } else {
                    problems.push(problem(
                        "nextest-unreviewed-key",
                        format!("{NEXTEST_CONFIG}'s `[profile.default]` has the key `{key}`, which every profile, `mutants` too, inherits; it has not been reviewed"),
                    ));
                }
            }
        }
        Some(other) => problems.push(problem(
            "nextest-unreviewed-key",
            format!("{NEXTEST_CONFIG}'s `profile.default` is not a table: `{other}`"),
        )),
    }
}

/// The two files LIB-QA-012 reads (`None`: the file does not exist).
#[derive(Clone, Debug, PartialEq, Eq)]
struct NextestSetup {
    mutants: Option<String>,
    nextest: Option<String>,
}

/// LIB-QA-012 items 1, 2 and 5: every part of ADR 0009's configuration.
fn check_nextest_setup(setup: &NextestSetup) -> Vec<String> {
    let mut problems = Vec::new();
    nextest_mutants_key_problems(setup.mutants.as_deref(), &mut problems);
    nextest_config_problems(setup.nextest.as_deref(), &mut problems);
    problems
}

/// The reference `.config/nextest.toml`.
const NEXTEST_CONFIG_FIXTURE: &str = "# nextest configuration (ADR 0009, LIB-QA-012).
nextest-version = { required = \"0.9.111\" }

# cargo-mutants runs the tests with this profile: at the first failure, every
# running test is killed.
[profile.mutants]
fail-fast = { max-fail = 1, terminate = \"immediate\" }
";

// Lines of `NEXTEST_CONFIG_FIXTURE` that controls edit.
const NEXTEST_VERSION_LINE: &str = "nextest-version = { required = \"0.9.111\" }";
const NEXTEST_PROFILE_LINE: &str = "[profile.mutants]";
const NEXTEST_FAIL_FAST_LINE: &str = "fail-fast = { max-fail = 1, terminate = \"immediate\" }";
const NEXTEST_PROFILE_BLOCK: &str =
    "[profile.mutants]\nfail-fast = { max-fail = 1, terminate = \"immediate\" }\n";

// Lines of the reference `.cargo/mutants.toml` that controls edit.
const TEST_TOOL_LINE: &str = "test_tool = \"nextest\"";
const TEST_ARGS_LINE: &str =
    "additional_cargo_test_args = [\"--profile=mutants\", \"--no-tests=warn\"]";

/// Problem codes LIB-QA-012 controls are expected to produce.
const QA12_ACCEPTED: &[&str] = &[];
const QA12_TEST_TOOL: &[&str] = &["nextest-test-tool"];
const QA12_TEST_ARGS: &[&str] = &["nextest-test-args"];
const QA12_PROFILE: &[&str] = &["nextest-profile"];
const QA12_FAIL_FAST: &[&str] = &["nextest-fail-fast"];
const QA12_MAX_FAIL: &[&str] = &["nextest-max-fail"];
const QA12_TERMINATE: &[&str] = &["nextest-terminate"];
const QA12_DEFAULT_FAIL_FAST: &[&str] = &["nextest-default-fail-fast"];
const QA12_UNREVIEWED: &[&str] = &["nextest-unreviewed-key"];
const QA12_VERSION: &[&str] = &["nextest-version"];
const QA12_CONFIG_MISSING: &[&str] = &["nextest-config-missing"];
const QA12_CONFIG_PARSE: &[&str] = &["nextest-config-parse"];

/// The reference setup: [`CONFIG_FIXTURE`] with the ADR 0009 keys, and
/// [`NEXTEST_CONFIG_FIXTURE`].
fn nextest_fixture() -> NextestSetup {
    NextestSetup {
        mutants: Some(format!("{CONFIG_FIXTURE}{NEXTEST_MUTANTS_LINES}")),
        nextest: Some(NEXTEST_CONFIG_FIXTURE.to_owned()),
    }
}

/// The reference setup with `.cargo/mutants.toml` replaced.
fn with_mutants_toml(text: Option<String>) -> NextestSetup {
    NextestSetup {
        mutants: text,
        ..nextest_fixture()
    }
}

/// The reference setup with `.config/nextest.toml` replaced.
fn with_nextest_toml(text: Option<String>) -> NextestSetup {
    NextestSetup {
        nextest: text,
        ..nextest_fixture()
    }
}

/// The reference `.cargo/mutants.toml` with the line `target` replaced.
fn mutants_toml_line(target: &str, replacement: &str) -> NextestSetup {
    let reference = nextest_fixture()
        .mutants
        .expect("the reference has a mutants.toml");
    with_mutants_toml(Some(replace_line(&reference, target, replacement)))
}

/// The reference `.config/nextest.toml` with the line `target` replaced.
fn nextest_toml_line(target: &str, replacement: &str) -> NextestSetup {
    with_nextest_toml(Some(replace_line(
        NEXTEST_CONFIG_FIXTURE,
        target,
        replacement,
    )))
}

/// The reference `.config/nextest.toml` with `lines` appended.
fn nextest_toml_plus(lines: &str) -> NextestSetup {
    with_nextest_toml(Some(format!("{NEXTEST_CONFIG_FIXTURE}\n{lines}\n")))
}

/// A LIB-QA-012 control: a setup and the exact codes it must give (none: it
/// is accepted).
struct NextestControl {
    name: String,
    setup: NextestSetup,
    codes: &'static [&'static str],
}

fn control(
    name: impl Into<String>,
    setup: NextestSetup,
    codes: &'static [&'static str],
) -> NextestControl {
    NextestControl {
        name: name.into(),
        setup,
        codes,
    }
}

/// Assert that the reference setup passes, and that each control, which
/// must differ from it, gives exactly its codes: no fewer, no others, each as
/// many times as named. A control's TOML files parse, unless it is about a
/// file that cannot be read, so that the check and not the parser rejects it.
fn assert_nextest_controls(controls: &[NextestControl]) {
    let reference = nextest_fixture();
    let problems = check_nextest_setup(&reference);
    assert!(
        problems.is_empty(),
        "{QA12}: the reference setup must pass before its controls mean anything: {problems:#?}"
    );
    for NextestControl { name, setup, codes } in controls {
        assert_ne!(
            setup, &reference,
            "{QA12}: precondition: the control `{name}` changes the reference"
        );
        let unreadable = codes
            .iter()
            .any(|code| matches!(*code, "nextest-config-missing" | "nextest-config-parse"));
        if !unreadable {
            for text in [&setup.mutants, &setup.nextest].into_iter().flatten() {
                assert!(
                    text.parse::<toml::Table>().is_ok(),
                    "{QA12}: precondition: the control `{name}` keeps its TOML parsable:\n{text}"
                );
            }
        }
        let mut want = codes.to_vec();
        want.sort_unstable();
        let problems = check_nextest_setup(setup);
        assert_eq!(
            problem_codes(&problems),
            want,
            "{QA12}: the control `{name}` must give exactly {want:?}; it gave {problems:#?}; setup:\n{setup:#?}"
        );
    }
}

/// Item 5: the real `.cargo/mutants.toml` and `.config/nextest.toml` keep
/// every part of ADR 0009's configuration.
#[test]
fn lib_qa_012_the_real_files_run_mutants_under_nextest() {
    let problems = check_nextest_setup(&NextestSetup {
        mutants: read_optional(MUTANTS_CONFIG),
        nextest: read_optional(NEXTEST_CONFIG),
    });
    assert!(
        problems.is_empty(),
        "{QA12}: {MUTANTS_CONFIG} and {NEXTEST_CONFIG} must run cargo-mutants under nextest with the `mutants` profile (ADR 0009): {problems:#?}"
    );
}

/// Items 1 and 5: `test_tool` and `additional_cargo_test_args` removed or
/// changed, each with its own code.
#[test]
fn lib_qa_012_each_mutants_toml_key_lost_is_named() {
    let reference = nextest_fixture()
        .mutants
        .expect("the reference has a mutants.toml");
    let mut controls = vec![
        control(
            "test_tool removed",
            with_mutants_toml(Some(delete_line(&reference, TEST_TOOL_LINE))),
            QA12_TEST_TOOL,
        ),
        control(
            "additional_cargo_test_args removed",
            with_mutants_toml(Some(delete_line(&reference, TEST_ARGS_LINE))),
            QA12_TEST_ARGS,
        ),
        control(
            "both removed",
            with_mutants_toml(Some(CONFIG_FIXTURE.to_owned())),
            &["nextest-test-tool", "nextest-test-args"],
        ),
        control(
            "test_tool commented out",
            with_mutants_toml(Some(comment_line(&reference, TEST_TOOL_LINE))),
            QA12_TEST_TOOL,
        ),
    ];
    for (key, value) in OTHER_NEXTEST_VALUES {
        let (line, code) = if key == "test_tool" {
            (TEST_TOOL_LINE, QA12_TEST_TOOL)
        } else {
            (TEST_ARGS_LINE, QA12_TEST_ARGS)
        };
        controls.push(control(
            format!("{key} = {value}"),
            mutants_toml_line(line, &format!("{key} = {value}")),
            code,
        ));
    }
    assert_nextest_controls(&controls);
}

/// Items 2 and 5: the `mutants` profile, its fail-fast, `max-fail` and
/// `terminate` removed or changed, each with its own code.
#[test]
fn lib_qa_012_each_part_of_the_mutants_profile_lost_is_named() {
    let fail_fast =
        |value: &str| nextest_toml_line(NEXTEST_FAIL_FAST_LINE, &format!("fail-fast = {value}"));
    let mut controls = vec![
        control(
            "[profile.mutants] removed",
            with_nextest_toml(Some(replace_once(
                NEXTEST_CONFIG_FIXTURE,
                NEXTEST_PROFILE_BLOCK,
                "",
            ))),
            QA12_PROFILE,
        ),
        control(
            "the profile renamed `mutant`",
            nextest_toml_line(NEXTEST_PROFILE_LINE, "[profile.mutant]"),
            QA12_PROFILE,
        ),
        control(
            "the profile renamed `Mutants`",
            nextest_toml_line(NEXTEST_PROFILE_LINE, "[profile.Mutants]"),
            QA12_PROFILE,
        ),
        control(
            "the profile as a string",
            with_nextest_toml(Some(
                replace_once(NEXTEST_CONFIG_FIXTURE, NEXTEST_PROFILE_BLOCK, "").replace(
                    NEXTEST_VERSION_LINE,
                    &format!("{NEXTEST_VERSION_LINE}\nprofile = \"mutants\""),
                ),
            )),
            QA12_PROFILE,
        ),
        control(
            "fail-fast removed",
            with_nextest_toml(Some(delete_line(
                NEXTEST_CONFIG_FIXTURE,
                NEXTEST_FAIL_FAST_LINE,
            ))),
            QA12_FAIL_FAST,
        ),
    ];
    for (value, codes) in [
        ("true", QA12_FAIL_FAST),
        ("false", QA12_FAIL_FAST),
        ("\"immediate\"", QA12_FAIL_FAST),
        ("1", QA12_FAIL_FAST),
        ("[1, \"immediate\"]", QA12_FAIL_FAST),
        ("{ terminate = \"immediate\" }", QA12_MAX_FAIL),
        ("{ max-fail = 2, terminate = \"immediate\" }", QA12_MAX_FAIL),
        ("{ max-fail = 0, terminate = \"immediate\" }", QA12_MAX_FAIL),
        (
            "{ max-fail = -1, terminate = \"immediate\" }",
            QA12_MAX_FAIL,
        ),
        (
            "{ max-fail = \"all\", terminate = \"immediate\" }",
            QA12_MAX_FAIL,
        ),
        (
            "{ max-fail = \"1\", terminate = \"immediate\" }",
            QA12_MAX_FAIL,
        ),
        (
            "{ max-fail = 1.0, terminate = \"immediate\" }",
            QA12_MAX_FAIL,
        ),
        ("{ max-fail = 1 }", QA12_TERMINATE),
        ("{ max-fail = 1, terminate = \"wait\" }", QA12_TERMINATE),
        (
            "{ max-fail = 1, terminate = \"Immediate\" }",
            QA12_TERMINATE,
        ),
        (
            "{ max-fail = 1, terminate = \"immediate \" }",
            QA12_TERMINATE,
        ),
        ("{ max-fail = 1, terminate = true }", QA12_TERMINATE),
        (
            "{ max-fail = 2, terminate = \"wait\" }",
            &["nextest-max-fail", "nextest-terminate"],
        ),
        ("{}", &["nextest-max-fail", "nextest-terminate"]),
        (
            "{ max-fail = 1, terminate = \"immediate\", retries = 1 }",
            QA12_UNREVIEWED,
        ),
    ] {
        controls.push(control(
            format!("fail-fast = {value}"),
            fail_fast(value),
            codes,
        ));
    }
    assert_nextest_controls(&controls);
}

/// Items 2 and 5: the `default` profile changing fail-fast is
/// `[nextest-default-fail-fast]`, whatever the value: developers' runs keep
/// nextest's own.
#[test]
fn lib_qa_012_the_default_profile_changing_fail_fast_is_named() {
    let mut controls = Vec::new();
    for value in [
        "false",
        "true",
        "{ max-fail = 1, terminate = \"immediate\" }",
        "{ max-fail = \"all\" }",
    ] {
        controls.push(control(
            format!("[profile.default] fail-fast = {value}"),
            nextest_toml_plus(&format!("[profile.default]\nfail-fast = {value}")),
            QA12_DEFAULT_FAIL_FAST,
        ));
    }
    controls.extend([
        control(
            "[profile.default.fail-fast] as a table",
            nextest_toml_plus("[profile.default.fail-fast]\nmax-fail = 2"),
            QA12_DEFAULT_FAIL_FAST,
        ),
        control(
            "[profile.default] fail-fast and retries",
            nextest_toml_plus("[profile.default]\nfail-fast = false\nretries = 2"),
            &["nextest-default-fail-fast", "nextest-unreviewed-key"],
        ),
    ]);
    assert_nextest_controls(&controls);
}

/// Item 2: any other key that the `mutants` profile sees (its own, or the
/// `default` profile's, which it inherits) is `[nextest-unreviewed-key]`.
#[test]
fn lib_qa_012_unreviewed_profile_keys_are_named() {
    let in_mutants = |line: &str| {
        with_nextest_toml(Some(insert_after(
            NEXTEST_CONFIG_FIXTURE,
            NEXTEST_FAIL_FAST_LINE,
            0,
            &[line],
        )))
    };
    assert_nextest_controls(&[
        control(
            "[profile.default] default-filter",
            nextest_toml_plus("[profile.default]\ndefault-filter = \"not test(lib_)\""),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.default] slow-timeout that kills tests",
            nextest_toml_plus(
                "[profile.default]\nslow-timeout = { period = \"1s\", terminate-after = 1 }",
            ),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.default] retries",
            nextest_toml_plus("[profile.default]\nretries = 3"),
            QA12_UNREVIEWED,
        ),
        control(
            "[[profile.default.overrides]]",
            nextest_toml_plus("[[profile.default.overrides]]\nfilter = \"all()\"\nretries = 3"),
            QA12_UNREVIEWED,
        ),
        control(
            "profile.default not a table",
            with_nextest_toml(Some(NEXTEST_CONFIG_FIXTURE.replace(
                NEXTEST_PROFILE_LINE,
                "[profile]\ndefault = 1\n\n[profile.mutants]",
            ))),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.mutants] retries",
            in_mutants("retries = 2"),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.mutants] default-filter",
            in_mutants("default-filter = \"not test(lib_)\""),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.mutants] slow-timeout",
            in_mutants("slow-timeout = { period = \"1s\", terminate-after = 1 }"),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.mutants] test-threads",
            in_mutants("test-threads = 1"),
            QA12_UNREVIEWED,
        ),
        control(
            "a second nextest-version, inside [profile.mutants]",
            nextest_toml_plus(NEXTEST_VERSION_LINE),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.mutants] inherits",
            in_mutants("inherits = \"ci\""),
            QA12_UNREVIEWED,
        ),
        control(
            "[[profile.mutants.overrides]]",
            nextest_toml_plus("[[profile.mutants.overrides]]\nfilter = \"all()\"\nretries = 3"),
            QA12_UNREVIEWED,
        ),
    ]);
}

/// Item 2: any top-level key of `.config/nextest.toml` but `nextest-version`
/// and `profile`, or a key of `nextest-version` but `required` and
/// `recommended`, is `[nextest-unreviewed-key]`.
#[test]
fn lib_qa_012_unreviewed_top_level_nextest_keys_are_named() {
    let top_level = |line: &str| {
        with_nextest_toml(Some(insert_after(
            NEXTEST_CONFIG_FIXTURE,
            NEXTEST_VERSION_LINE,
            0,
            &[line],
        )))
    };
    assert_nextest_controls(&[
        control(
            "a top-level experimental",
            top_level("experimental = [\"setup-scripts\"]"),
            QA12_UNREVIEWED,
        ),
        control(
            "[scripts.setup]",
            nextest_toml_plus("[scripts.setup.prepare]\ncommand = \"true\""),
            QA12_UNREVIEWED,
        ),
        control(
            "[test-groups]",
            nextest_toml_plus("[test-groups.serial]\nmax-threads = 1"),
            QA12_UNREVIEWED,
        ),
        control(
            "[store]",
            nextest_toml_plus("[store]\ndir = \"elsewhere\""),
            QA12_UNREVIEWED,
        ),
        control(
            "an unknown key in nextest-version",
            nextest_toml_line(
                NEXTEST_VERSION_LINE,
                "nextest-version = { required = \"0.9.111\", minimum = \"0.9.111\" }",
            ),
            QA12_UNREVIEWED,
        ),
    ]);
}

/// Items 2 and 5: the version requirement missing, below 0.9.111 or not a
/// version is `[nextest-version]`. Versions compare as numbers: `0.9.99` is
/// below `0.9.111`, and `0.9.1000` above it.
#[test]
fn lib_qa_012_the_version_requirement_is_compared_as_numbers() {
    let required = |value: &str| {
        nextest_toml_line(
            NEXTEST_VERSION_LINE,
            &format!("nextest-version = {{ required = {value} }}"),
        )
    };
    let mut controls = vec![
        control(
            "nextest-version removed",
            with_nextest_toml(Some(delete_line(
                NEXTEST_CONFIG_FIXTURE,
                NEXTEST_VERSION_LINE,
            ))),
            QA12_VERSION,
        ),
        control(
            "nextest-version = 111",
            nextest_toml_line(NEXTEST_VERSION_LINE, "nextest-version = 111"),
            QA12_VERSION,
        ),
        control(
            "only a recommended version",
            nextest_toml_line(
                NEXTEST_VERSION_LINE,
                "nextest-version = { recommended = \"0.9.151\" }",
            ),
            QA12_VERSION,
        ),
        control(
            "a string form below 0.9.111",
            nextest_toml_line(NEXTEST_VERSION_LINE, "nextest-version = \"0.9.110\""),
            QA12_VERSION,
        ),
    ];
    for (value, codes) in [
        ("\"0.9.110\"", QA12_VERSION),
        ("\"0.9.99\"", QA12_VERSION),
        ("\"0.9.0\"", QA12_VERSION),
        ("\"0.8.999\"", QA12_VERSION),
        ("\"0.0.0\"", QA12_VERSION),
        ("\"0.9\"", QA12_VERSION),
        ("\"0.9.111.0\"", QA12_VERSION),
        ("\"v0.9.111\"", QA12_VERSION),
        ("\"0.9.111-rc.1\"", QA12_VERSION),
        ("\"0.9.111+build\"", QA12_VERSION),
        ("\"0.9.0111\"", QA12_VERSION),
        ("\" 0.9.111\"", QA12_VERSION),
        ("\"\"", QA12_VERSION),
        ("\"latest\"", QA12_VERSION),
        ("\"0.9.x\"", QA12_VERSION),
        ("\">=0.9.111\"", QA12_VERSION),
        ("111", QA12_VERSION),
        ("\"0.9.120\"", QA12_ACCEPTED),
        ("\"0.9.151\"", QA12_ACCEPTED),
        ("\"0.9.1000\"", QA12_ACCEPTED),
        ("\"0.10.0\"", QA12_ACCEPTED),
        ("\"1.0.0\"", QA12_ACCEPTED),
    ] {
        controls.push(control(
            format!("required = {value}"),
            required(value),
            codes,
        ));
    }
    assert_nextest_controls(&controls);
}

/// Item 5: a file missing or unreadable is named, never a skip.
#[test]
fn lib_qa_012_a_missing_or_unreadable_file_is_named() {
    let reference = nextest_fixture();
    assert_nextest_controls(&[
        control(
            ".cargo/mutants.toml missing",
            with_mutants_toml(None),
            QA12_CONFIG_MISSING,
        ),
        control(
            ".cargo/mutants.toml not TOML",
            with_mutants_toml(Some("{ \"test_tool\": \"nextest\" }".to_owned())),
            QA12_CONFIG_PARSE,
        ),
        control(
            ".cargo/mutants.toml with test_tool twice",
            with_mutants_toml(Some(format!(
                "{}{TEST_TOOL_LINE}\n",
                reference.mutants.clone().unwrap_or_default()
            ))),
            QA12_CONFIG_PARSE,
        ),
        control(
            ".config/nextest.toml missing",
            with_nextest_toml(None),
            QA12_CONFIG_MISSING,
        ),
        control(
            ".config/nextest.toml with an unterminated table header",
            nextest_toml_line(NEXTEST_PROFILE_LINE, "[profile.mutants"),
            QA12_CONFIG_PARSE,
        ),
        control(
            ".config/nextest.toml with nextest-version twice",
            with_nextest_toml(Some(insert_after(
                NEXTEST_CONFIG_FIXTURE,
                NEXTEST_VERSION_LINE,
                0,
                &[NEXTEST_VERSION_LINE],
            ))),
            QA12_CONFIG_PARSE,
        ),
        control(
            "both files missing",
            NextestSetup {
                mutants: None,
                nextest: None,
            },
            &["nextest-config-missing", "nextest-config-missing"],
        ),
    ]);
}

/// Items 1 and 2: the same setup written in other TOML forms, and other
/// profiles, which `mutants` does not inherit, are accepted.
#[test]
fn lib_qa_012_equivalent_forms_are_accepted() {
    let mutants = nextest_fixture()
        .mutants
        .expect("the reference has a mutants.toml");
    assert_nextest_controls(&[
        control(
            "mutants.toml values as literal strings, the array over lines",
            with_mutants_toml(Some(
                mutants
                    .replace(TEST_TOOL_LINE, "test_tool = 'nextest'")
                    .replace(
                        TEST_ARGS_LINE,
                        "additional_cargo_test_args = [\n  '--profile=mutants',\n  '--no-tests=warn',\n]",
                    ),
            )),
            QA12_ACCEPTED,
        ),
        control(
            "mutants.toml keys in another order",
            with_mutants_toml(Some(format!(
                "{TEST_ARGS_LINE}\n{}",
                delete_line(&mutants, TEST_ARGS_LINE)
            ))),
            QA12_ACCEPTED,
        ),
        control(
            "nextest-version as a string",
            nextest_toml_line(NEXTEST_VERSION_LINE, "nextest-version = \"0.9.111\""),
            QA12_ACCEPTED,
        ),
        control(
            "nextest-version with a recommended version",
            nextest_toml_line(
                NEXTEST_VERSION_LINE,
                "nextest-version = { required = \"0.9.111\", recommended = \"0.9.151\" }",
            ),
            QA12_ACCEPTED,
        ),
        control(
            "[profile.mutants.fail-fast] as a table",
            with_nextest_toml(Some(replace_once(
                NEXTEST_CONFIG_FIXTURE,
                NEXTEST_PROFILE_BLOCK,
                "[profile.mutants.fail-fast]\nmax-fail = 1\nterminate = \"immediate\"\n",
            ))),
            QA12_ACCEPTED,
        ),
        control(
            "dotted fail-fast keys",
            nextest_toml_line(
                NEXTEST_FAIL_FAST_LINE,
                "fail-fast.max-fail = 1\nfail-fast.terminate = \"immediate\"",
            ),
            QA12_ACCEPTED,
        ),
        control(
            "the whole file in dotted keys",
            with_nextest_toml(Some(
                "nextest-version.required = \"0.9.111\"\nprofile.mutants.fail-fast = { terminate = \"immediate\", max-fail = 1 }\n".to_owned(),
            )),
            QA12_ACCEPTED,
        ),
        control(
            "another profile, which `mutants` does not inherit",
            nextest_toml_plus("[profile.ci]\nretries = 2\nfail-fast = false"),
            QA12_ACCEPTED,
        ),
        control(
            "an empty [profile.default]",
            nextest_toml_plus("[profile.default]"),
            QA12_ACCEPTED,
        ),
        control(
            "comments",
            nextest_toml_plus("# Developers' runs keep nextest's defaults."),
            QA12_ACCEPTED,
        ),
    ]);
}

/// Item 4: the LIB-QA-007 config check accepts exactly ADR 0009's values.
/// The reference passes it, and every other value of the two keys is
/// LIB-QA-007's `[unreviewed-key]` and LIB-QA-012's code for that key.
#[test]
fn lib_qa_012_the_lib_qa_007_checks_accept_exactly_the_nextest_setup() {
    let ids = matrix_ids();
    let workspace = real_workspace();
    let config = |setup: &NextestSetup| {
        check_mutants_config(
            setup.mutants.as_deref().unwrap_or_default(),
            &workspace,
            TRIAGE_FIXTURE,
            &ids,
        )
    };
    let problems = config(&nextest_fixture());
    assert!(
        problems.is_empty(),
        "{QA12}: LIB-QA-007's config check must accept ADR 0009's setup: {problems:#?}"
    );
    for (key, value) in OTHER_NEXTEST_VALUES {
        let (line, code) = if key == "test_tool" {
            (TEST_TOOL_LINE, "nextest-test-tool")
        } else {
            (TEST_ARGS_LINE, "nextest-test-args")
        };
        let setup = mutants_toml_line(line, &format!("{key} = {value}"));
        let lib_qa_007 = config(&setup);
        let lib_qa_012 = check_nextest_setup(&setup);
        assert!(
            has_code(&lib_qa_007, "unreviewed-key") && problem_codes(&lib_qa_012) == [code],
            "{QA12}: `{key} = {value}` must be LIB-QA-007's `[unreviewed-key]` and LIB-QA-012's `[{code}]`; LIB-QA-007 gave {lib_qa_007:#?}, LIB-QA-012 gave {lib_qa_012:#?}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-DOC-008: the ADR index and the links to ADRs
// ---------------------------------------------------------------------------
//
// The rest of the row's acceptance (what each ADR says) is checked by review.

const ADR_DIR: &str = "docs/adr";
const ADR_INDEX: &str = "docs/adr/README.md";
const DECISIONS: &str = "docs/sacm/decisions.md";

/// The ADR the mutations edit.
const ADR_0021: &str = "0021-xmi-reader-content.md";

/// An ADR file: its name in `docs/adr/` and its text.
type AdrFile = (String, String);

/// The number of an ADR file name `NNNN-*.md`, or `None` for any other name
/// (`README.md`, `template.md`).
fn adr_number(name: &str) -> Option<u32> {
    let digits = name.get(..4)?;
    let rest = &name[4..];
    let is_adr = digits.bytes().all(|b| b.is_ascii_digit())
        && rest.starts_with('-')
        && Path::new(rest).extension().is_some_and(|ext| ext == "md");
    if is_adr { digits.parse().ok() } else { None }
}

/// The names of the files in `docs/adr/`.
fn adr_dir_names() -> BTreeSet<String> {
    let entries = fs::read_dir(repository_root().join(ADR_DIR))
        .unwrap_or_else(|e| panic!("read directory {ADR_DIR}: {e}"));
    entries
        .map(|entry| entry.unwrap_or_else(|e| panic!("read directory {ADR_DIR}: {e}")))
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

/// The ADR files `docs/adr/NNNN-*.md`, sorted by name.
fn adr_files() -> Vec<AdrFile> {
    adr_dir_names()
        .into_iter()
        .filter(|name| adr_number(name).is_some())
        .map(|name| {
            let text = read(&format!("{ADR_DIR}/{name}"));
            (name, text)
        })
        .collect()
}

/// A status as the index check compares it: the first word of `text`,
/// lower-cased and without punctuation (`accepted`, `proposed`), or, for the
/// documented `superseded by NNNN`, those words with the successor's number
/// kept (a link `[NNNN](...)` gives its label). `None` if `text` states no
/// status, or `superseded` without `by NNNN`.
fn normal_status(text: &str) -> Option<String> {
    let word = |w: &str| -> String {
        w.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>()
            .to_lowercase()
    };
    let mut words = text.split_whitespace();
    let first = word(words.next()?);
    if first != "superseded" {
        return (!first.is_empty()).then_some(first);
    }
    if words.next().map(word).as_deref() != Some("by") {
        return None;
    }
    let digits: String = words
        .next()?
        .trim_start_matches('[')
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (digits.len() == 4).then(|| format!("superseded by {digits}"))
}

/// The status an ADR states: the [`normal_status`] of its first `- Status:`
/// line.
fn adr_status(text: &str) -> Option<String> {
    let rest = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("- Status:"))?;
    normal_status(rest)
}

/// The source of the first table that the Markdown `text` renders, or `None`
/// if it renders none. A table in a code block or an HTML comment is not
/// rendered.
fn first_rendered_table(text: &str) -> Option<&str> {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    Parser::new_ext(text, Options::ENABLE_TABLES)
        .into_offset_iter()
        .find_map(|(event, range)| {
            matches!(event, Event::Start(Tag::Table(_))).then(|| &text[range])
        })
}

/// Whether `cells` are a table's delimiter row (`|---|:--:|`).
fn is_delimiter_row(cells: &[String]) -> bool {
    cells
        .iter()
        .all(|cell| !cell.is_empty() && cell.chars().all(|c| matches!(c, '-' | ':')))
}

/// `(label, destination)` of a cell that is exactly one inline link
/// `[label](destination)`.
fn cell_link(cell: &str) -> Option<(&str, &str)> {
    let (label, rest) = cell.strip_prefix('[')?.split_once("](")?;
    Some((label, rest.strip_suffix(')')?))
}

/// A row of the ADR index: the ADR it links, its status cell, and that
/// cell's [`normal_status`].
struct IndexRow {
    number: u32,
    target: String,
    cell: String,
    status: Option<String>,
}

/// The rows of the first table the ADR index `index` renders, with a problem
/// in `problems` for each row that does not link an ADR by its number; or
/// `None`, with an `index-header` problem, if it renders no table with `ADR`
/// and `Status` columns.
fn index_rows(index: &str, problems: &mut Vec<String>) -> Option<Vec<IndexRow>> {
    let mut lines = first_rendered_table(index)
        .into_iter()
        .flat_map(str::lines)
        .filter(|line| !line.trim().is_empty())
        .map(table_cells);
    let Some(header) = lines.next() else {
        problems.push(problem("index-header", "the ADR index renders no table"));
        return None;
    };
    let column = |name: &str| header.iter().position(|cell| cell == name);
    let (Some(adr_column), Some(status_column)) = (column("ADR"), column("Status")) else {
        problems.push(problem(
            "index-header",
            format!("the index table needs `ADR` and `Status` columns; its header is {header:?}"),
        ));
        return None;
    };
    let mut rows: Vec<IndexRow> = Vec::new();
    for cells in lines.filter(|cells| !is_delimiter_row(cells)) {
        let link = cells
            .get(adr_column)
            .and_then(|cell| cell_link(cell))
            .and_then(|(label, target)| adr_number(target).map(|n| (label, target, n)));
        let Some((label, target, number)) = link else {
            problems.push(problem(
                "index-row",
                format!(
                    "the row `{}` does not link an ADR file `NNNN-*.md` in its ADR column",
                    cells.join(" | ")
                ),
            ));
            continue;
        };
        if target.get(..4) != Some(label) {
            problems.push(problem(
                "index-label",
                format!("the link to {target} is labelled `{label}`, not its number"),
            ));
        }
        let cell = cells.get(status_column).cloned().unwrap_or_default();
        rows.push(IndexRow {
            number,
            target: target.to_owned(),
            status: normal_status(&cell),
            cell,
        });
    }
    Some(rows)
}

/// LIB-DOC-008: the first table the ADR index `index` renders has one row per
/// file of `adrs`, linked to that file and labelled with its number, in number
/// order, whose status cell states the file's [`adr_status`]; and no other
/// rows. An ADR `superseded by NNNN` names an ADR file of `adrs`.
fn check_adr_index(index: &str, adrs: &[AdrFile]) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(rows) = index_rows(index, &mut problems) else {
        return problems;
    };
    for pair in rows.windows(2) {
        if pair[1].number < pair[0].number {
            problems.push(problem(
                "index-order",
                format!("{} is listed after {}", pair[1].target, pair[0].target),
            ));
        }
    }
    let mut listed: BTreeMap<&str, Vec<&IndexRow>> = BTreeMap::new();
    for row in &rows {
        listed.entry(&row.target).or_default().push(row);
    }
    for (target, entries) in &listed {
        if entries.len() > 1 {
            problems.push(problem(
                "index-duplicate",
                format!("{target} is listed {} times", entries.len()),
            ));
        }
        if !adrs.iter().any(|(name, _)| name == target) {
            problems.push(problem(
                "index-dangling",
                format!("the index lists {target}, which does not exist"),
            ));
        }
    }
    for (name, text) in adrs {
        let Some(status) = adr_status(text) else {
            problems.push(problem(
                "adr-status",
                format!("{name} has no `- Status:` line with a status"),
            ));
            continue;
        };
        if let Some(successor) = status.strip_prefix("superseded by ") {
            let exists = adrs
                .iter()
                .any(|(other, _)| other.get(..4) == Some(successor));
            if !exists {
                problems.push(problem(
                    "status-successor",
                    format!("{name} is `{status}`, but there is no ADR {successor}"),
                ));
            }
        }
        let Some(entries) = listed.get(name.as_str()) else {
            problems.push(problem(
                "index-missing",
                format!("{name} is not listed in the index"),
            ));
            continue;
        };
        for row in entries
            .iter()
            .filter(|row| row.status.as_deref() != Some(status.as_str()))
        {
            problems.push(problem(
                "index-status",
                format!(
                    "the index gives {name} the status `{}`; the ADR says `{status}`",
                    row.cell
                ),
            ));
        }
    }
    problems
}

/// The one line of `text` that starts (trimmed) with `prefix`, trimmed.
/// Panics if there is not exactly one, so a mutation is never a no-op.
fn line_starting(text: &str, prefix: &str) -> String {
    let hits: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with(prefix))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "mutation target: exactly one line must start with `{prefix}`"
    );
    hits[0].to_owned()
}

/// The index row of ADR `number` (`"0021"`), trimmed.
fn index_row(index: &str, number: &str) -> String {
    line_starting(index, &format!("| [{number}]("))
}

/// `adrs` with the text of `name` replaced by `edit(text)`.
fn adrs_with(adrs: &[AdrFile], name: &str, edit: impl FnOnce(&str) -> String) -> Vec<AdrFile> {
    let mut out = adrs.to_vec();
    let text = out.iter_mut().find(|(n, _)| n == name).map_or_else(
        || panic!("mutation target `{name}` is an ADR file"),
        |(_, text)| text,
    );
    *text = edit(text);
    out
}

/// `adrs` with the `- Status:` line of `name` replaced by `status_line`.
fn adrs_with_status(adrs: &[AdrFile], name: &str, status_line: &str) -> Vec<AdrFile> {
    adrs_with(adrs, name, |text| {
        let line = line_starting(text, "- Status:");
        replace_line(text, &line, status_line)
    })
}

/// The last ADR row of the index table, whatever its number, so that
/// mutations appending a row do not change as ADRs are added.
fn last_index_row(index: &str) -> String {
    index
        .lines()
        .map(str::trim)
        .rfind(|line| line.starts_with("| ["))
        .expect("mutation target: the index table has rows")
        .to_owned()
}

/// The header line of the ADR index table.
const INDEX_HEADER: &str = "| ADR | Title | Status |";

/// `index` with its table between the lines `open` and `close` (a fence, an
/// HTML comment), so that it is no longer rendered as a table.
fn wrap_index_table(index: &str, open: &str, close: &str) -> String {
    let last_row = last_index_row(index);
    let opened = edit_line(index, INDEX_HEADER, |line| {
        vec![open.to_owned(), line.to_owned()]
    });
    edit_line(&opened, &last_row, |line| {
        vec![line.to_owned(), close.to_owned()]
    })
}

/// The successor ADR the superseded mutations add; a number no real ADR
/// will have soon, so the mutations do not change as ADRs are added.
const SUCCESSOR: &str = "9997-a-successor.md";

/// `index` and `adrs` with the ADR [`SUCCESSOR`] added (file and last index
/// row), and with ADR 0021 stating `status_line` and its index row the status
/// cell `cell`.
fn with_superseded(
    index: &str,
    adrs: &[AdrFile],
    status_line: &str,
    cell: &str,
) -> (String, Vec<AdrFile>) {
    let last_row = index_row(index, &adrs.last().expect("ADR files").0[..4]);
    let row_0021 = index_row(index, "0021");
    let index = insert_after(
        index,
        &last_row,
        0,
        &[&format!("| [9997]({SUCCESSOR}) | A successor | proposed |")],
    );
    let index = replace_line(
        &index,
        &row_0021,
        &row_0021.replace("| accepted |", &format!("| {cell} |")),
    );
    let mut adrs = adrs_with_status(adrs, ADR_0021, status_line);
    adrs.push((
        SUCCESSOR.to_owned(),
        "# 9997. A successor\n\n- Status: proposed\n".to_owned(),
    ));
    (index, adrs)
}

/// A mutation of the ADR index or the ADR files: its name, the index, the
/// ADR files, and the codes it must be reported with.
type IndexMutation = (&'static str, String, Vec<AdrFile>, &'static [&'static str]);

/// The mutations for the `superseded by NNNN` status and for an index whose
/// table does not render, each with exactly its codes.
fn adr_index_status_and_rendering_mutations(index: &str, adrs: &[AdrFile]) -> Vec<IndexMutation> {
    let superseded = |name: &'static str,
                      status_line: &str,
                      cell: &str,
                      codes: &'static [&'static str]|
     -> IndexMutation {
        let (index, adrs) = with_superseded(index, adrs, status_line, cell);
        (name, index, adrs, codes)
    };
    vec![
        superseded(
            "the index says superseded without the successor",
            "- Status: superseded by 9997",
            "superseded",
            &["index-status"],
        ),
        superseded(
            "the index names another successor",
            "- Status: superseded by 9997",
            "superseded by 9996",
            &["index-status"],
        ),
        superseded(
            "ADR 0021 superseded by an ADR that does not exist",
            "- Status: superseded by 9996",
            "superseded by 9996",
            &["status-successor"],
        ),
        superseded(
            "ADR 0021 says superseded without the successor",
            "- Status: superseded",
            "superseded",
            &["adr-status"],
        ),
        (
            "the index table in a fenced code block",
            wrap_index_table(index, "```text", "```"),
            adrs.to_vec(),
            &["index-header"],
        ),
        (
            "the index table in an HTML comment",
            wrap_index_table(index, "<!--", "-->"),
            adrs.to_vec(),
            &["index-header"],
        ),
    ]
}

/// The mutations of the real `index` and `adrs` that the index check must
/// report, each with exactly its codes.
fn adr_index_mutations(index: &str, adrs: &[AdrFile]) -> Vec<IndexMutation> {
    let row_0002 = index_row(index, "0002");
    let row_0003 = index_row(index, "0003");
    let row_0005 = index_row(index, "0005");
    let row_0021 = index_row(index, "0021");
    let last_row = last_index_row(index);
    let mut with_new_adr = adrs.to_vec();
    with_new_adr.push((
        "9998-a-new-decision.md".to_owned(),
        "# 9998. A new decision\n\n- Status: proposed\n".to_owned(),
    ));
    vec![
        (
            "the row of 0021 removed",
            delete_line(index, &row_0021),
            adrs.to_vec(),
            &["index-missing"],
        ),
        (
            "an ADR file without a row",
            index.to_owned(),
            with_new_adr,
            &["index-missing"],
        ),
        (
            "a row for a file that does not exist",
            insert_after(
                index,
                &last_row,
                0,
                &["| [9999](9999-not-an-adr.md) | Not an ADR | accepted |"],
            ),
            adrs.to_vec(),
            &["index-dangling"],
        ),
        (
            "0002 listed after 0003",
            insert_after(&delete_line(index, &row_0002), &row_0003, 0, &[&row_0002]),
            adrs.to_vec(),
            &["index-order"],
        ),
        (
            "the row of 0021 listed twice",
            insert_after(index, &row_0021, 0, &[&row_0021]),
            adrs.to_vec(),
            &["index-duplicate"],
        ),
        (
            "0010's link labelled 0011",
            replace_once(
                index,
                "[0010](0010-document-model.md)",
                "[0011](0010-document-model.md)",
            ),
            adrs.to_vec(),
            &["index-label"],
        ),
        (
            "0021's ADR cell not a link",
            replace_once(index, &format!("[0021]({ADR_0021})"), "0021"),
            adrs.to_vec(),
            &["index-missing", "index-row"],
        ),
        (
            "the index gives 0005 the status proposed",
            replace_line(
                index,
                &row_0005,
                &row_0005.replace("| accepted |", "| proposed |"),
            ),
            adrs.to_vec(),
            &["index-status"],
        ),
        (
            "ADR 0021 says proposed, the index accepted",
            index.to_owned(),
            adrs_with_status(adrs, ADR_0021, "- Status: proposed"),
            &["index-status"],
        ),
        (
            "ADR 0021 has no status line",
            index.to_owned(),
            adrs_with(adrs, ADR_0021, |text| {
                delete_line(text, &line_starting(text, "- Status:"))
            }),
            &["adr-status"],
        ),
        (
            "the index has no Status column",
            replace_once(index, "| ADR | Title | Status |", "| ADR | Title | State |"),
            adrs.to_vec(),
            &["index-header"],
        ),
    ]
}

#[test]
fn lib_doc_008_the_adr_index_lists_every_adr_with_its_status() {
    let index = read(ADR_INDEX);
    let adrs = adr_files();
    assert!(
        adrs.len() >= 21,
        "LIB-DOC-008: docs/adr/ must hold the ADR files; found {adrs:?}"
    );
    let problems = check_adr_index(&index, &adrs);
    assert!(
        problems.is_empty(),
        "LIB-DOC-008: {ADR_INDEX} must list every ADR, linked, in order, with its status: {problems:#?}"
    );

    let mut mutations = adr_index_mutations(&index, &adrs);
    mutations.extend(adr_index_status_and_rendering_mutations(&index, &adrs));
    for (name, index, adrs, codes) in &mutations {
        let problems = check_adr_index(index, adrs);
        assert_eq!(
            problem_codes(&problems),
            *codes,
            "LIB-DOC-008: the index check must report the mutation `{name}` as {codes:?}; it reported {problems:#?}"
        );
    }

    // Controls: the status is the first word, lower-cased, without punctuation.
    for status_line in [
        "- Status: Accepted, after review",
        "- Status: accepted (the maintainer, 2026-11-13)",
        "- Status: accepted. A longer note follows.",
    ] {
        let adrs = adrs_with_status(&adrs, ADR_0021, status_line);
        let problems = check_adr_index(&index, &adrs);
        assert!(
            problems.is_empty(),
            "LIB-DOC-008: `{status_line}` states the status `accepted`, but was reported: {problems:#?}"
        );
    }

    // Controls: the documented `superseded by NNNN`, stated alike in the ADR
    // and the index (the successor kept, a link giving its label).
    for (status_line, cell) in [
        ("- Status: superseded by 9997", "superseded by 9997"),
        (
            "- Status: Superseded by [9997](9997-a-successor.md).",
            "superseded by [9997](9997-a-successor.md)",
        ),
    ] {
        let (index, adrs) = with_superseded(&index, &adrs, status_line, cell);
        let problems = check_adr_index(&index, &adrs);
        assert!(
            problems.is_empty(),
            "LIB-DOC-008: `{status_line}` and the index cell `{cell}` state the same status, but were reported: {problems:#?}"
        );
    }

    // Control: the index is the first table that renders; a fenced table
    // before it, with another status, is not the index.
    let decoy = [
        "```text",
        INDEX_HEADER,
        "|---|---|---|",
        "| [0021](0021-xmi-reader-content.md) | Decoy | proposed |",
        "```",
        "",
        INDEX_HEADER,
    ]
    .map(str::to_owned);
    let with_decoy = edit_line(&index, INDEX_HEADER, |_| decoy.to_vec());
    let problems = check_adr_index(&with_decoy, &adrs);
    assert!(
        problems.is_empty(),
        "LIB-DOC-008: a fenced table before the index is not the index, but the check reported: {problems:#?}"
    );
}

/// The destinations of the links in the Markdown `text`, as a renderer
/// resolves them (inline and reference links; not code spans).
fn link_destinations(text: &str) -> Vec<String> {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    Parser::new_ext(text, Options::ENABLE_TABLES)
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url.into_string()),
            _ => None,
        })
        .collect()
}

/// LIB-DOC-008: every link of `text` (the file `doc`) that `adr_name` maps,
/// without its `#fragment`, to a file name in `docs/adr/` names a file of
/// `existing`. Fragments are not checked.
fn dangling_adr_links(
    doc: &str,
    text: &str,
    adr_name: fn(&str) -> Option<&str>,
    existing: &BTreeSet<String>,
) -> Vec<String> {
    link_destinations(text)
        .iter()
        .filter_map(|dest| {
            let path = dest.split_once('#').map_or(dest.as_str(), |(path, _)| path);
            adr_name(path)
                .filter(|name| !existing.contains(*name))
                .map(|_| {
                    problem(
                        "dangling-link",
                        format!("{doc} links `{dest}`, which is not a file in {ADR_DIR}/"),
                    )
                })
        })
        .collect()
}

/// LIB-DOC-008: every ADR file `NNNN-*.md` of `existing` is the destination
/// (without its `#fragment`) of a link that the index `text` renders, so an
/// index whose table does not render (a code block, an HTML comment) is
/// reported.
fn unlinked_adrs(text: &str, existing: &BTreeSet<String>) -> Vec<String> {
    let linked: BTreeSet<String> = link_destinations(text)
        .into_iter()
        .map(|dest| {
            dest.split_once('#')
                .map_or(dest.clone(), |(path, _)| path.to_owned())
        })
        .collect();
    existing
        .iter()
        .filter(|name| adr_number(name).is_some() && !linked.contains(*name))
        .map(|name| {
            problem(
                "index-unlinked",
                format!("{ADR_INDEX} renders no link to {name}"),
            )
        })
        .collect()
}

/// In `docs/sacm/decisions.md`: a link `../adr/<name>`.
fn decisions_adr_link(path: &str) -> Option<&str> {
    path.strip_prefix("../adr/")
}

/// In `docs/adr/README.md`: a link to an ADR file `NNNN-*.md`.
fn index_adr_link(path: &str) -> Option<&str> {
    adr_number(path).map(|_| path)
}

/// The mutations of the real ADR index `index` that the link check must
/// report, each with exactly its codes; `existing` are the files in
/// `docs/adr/`.
fn adr_index_link_mutations(
    index: &str,
    existing: &BTreeSet<String>,
) -> Vec<(&'static str, String, Vec<&'static str>)> {
    let adr_count = existing
        .iter()
        .filter(|name| adr_number(name).is_some())
        .count();
    let all_unlinked = vec!["index-unlinked"; adr_count];
    vec![
        (
            "a table link renamed",
            replace_once(index, "(0016-interchange-roots.md)", "(0016-roots.md)"),
            vec!["dangling-link", "index-unlinked"],
        ),
        (
            "a prose link to an ADR that does not exist",
            format!("{index}\nSee [ADR 9999](9999-not-an-adr.md).\n"),
            vec!["dangling-link"],
        ),
        (
            "the index table in a fenced code block",
            wrap_index_table(index, "```text", "```"),
            all_unlinked.clone(),
        ),
        (
            "the index table in an HTML comment",
            wrap_index_table(index, "<!--", "-->"),
            all_unlinked,
        ),
    ]
}

#[test]
fn lib_doc_008_every_adr_link_in_the_decisions_resolves() {
    let existing = adr_dir_names();
    let decisions = read(DECISIONS);
    let index = read(ADR_INDEX);
    let check_decisions =
        |text: &str| dangling_adr_links(DECISIONS, text, decisions_adr_link, &existing);
    let check_index = |text: &str| {
        let mut problems = dangling_adr_links(ADR_INDEX, text, index_adr_link, &existing);
        problems.extend(unlinked_adrs(text, &existing));
        problems
    };

    let in_decisions = link_destinations(&decisions)
        .iter()
        .filter(|dest| decisions_adr_link(dest).is_some())
        .count();
    assert!(
        in_decisions >= 5,
        "LIB-DOC-008: {DECISIONS} must point to the ADRs it rests on; found {in_decisions} links to ../adr/"
    );
    let problems = check_decisions(&decisions);
    assert!(
        problems.is_empty(),
        "LIB-DOC-008: every ../adr/ link in {DECISIONS} must name an existing file: {problems:#?}"
    );
    let problems = check_index(&index);
    assert!(
        problems.is_empty(),
        "LIB-DOC-008: every NNNN-*.md link in {ADR_INDEX} must name an existing file, and every ADR file must be linked: {problems:#?}"
    );

    let decisions_mutations = [
        (
            "an ADR link renamed",
            replace_once(
                &decisions,
                "../adr/0013-missing-name-stays-missing.md",
                "../adr/0013-missing-name.md",
            ),
        ),
        (
            "a link to an ADR that does not exist",
            format!("{decisions}\nSee [ADR 9999](../adr/9999-not-an-adr.md).\n"),
        ),
        (
            "a dangling link with a fragment",
            format!("{decisions}\nSee [ADR 9999](../adr/9999-not-an-adr.md#d1).\n"),
        ),
        (
            "a dangling reference link",
            format!("{decisions}\nSee [ADR 9999].\n\n[ADR 9999]: ../adr/9999-not-an-adr.md\n"),
        ),
    ];
    for (name, text) in &decisions_mutations {
        let problems = check_decisions(text);
        assert_eq!(
            problem_codes(&problems),
            ["dangling-link"],
            "LIB-DOC-008: the mutation `{name}` of {DECISIONS} must be reported once as `[dangling-link]`; it reported {problems:#?}"
        );
    }
    let index_mutations = adr_index_link_mutations(&index, &existing);
    for (name, text, codes) in &index_mutations {
        let problems = check_index(text);
        assert_eq!(
            problem_codes(&problems),
            *codes,
            "LIB-DOC-008: the mutation `{name}` of {ADR_INDEX} must be reported as {codes:?}; it reported {problems:#?}"
        );
    }

    // Controls: a fragment on an existing ADR, and text in a code span, which
    // is not a link.
    for (doc, problems) in [
        (
            DECISIONS,
            check_decisions(&format!(
                "{decisions}\nSee [ADR 0021](../adr/0021-xmi-reader-content.md#history).\n"
            )),
        ),
        (
            ADR_INDEX,
            check_index(&format!("{index}\nWrite `[9999](9999-not-an-adr.md)`.\n")),
        ),
    ] {
        assert!(
            problems.is_empty(),
            "LIB-DOC-008: a fragment on an existing ADR, or a code span in {doc}, is not a dangling link, but was reported: {problems:#?}"
        );
    }
}
