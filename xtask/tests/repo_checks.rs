//! Regression tests for repository configuration: the CI gates, the MSRV
//! declaration and the dependency policy (rows LIB-QA-001..003), and the
//! git-ignore that keeps OMG documents out of the repository (LIB-TOOL-002).
//!
//! These guard against a gate being removed or drifting out of step with the
//! manifest — the kind of change that passes review because nothing fails.
//!
//! The LIB-QA-002 and LIB-QA-003 checks are functions over the configuration
//! text (or its parsed TOML) that return the problems they find. Each check is
//! applied to the real file, which must produce no problems, and to in-memory
//! mutations of it — the bypasses the verifier listed in
//! `docs/verification/0002-m0-qa-and-cli-verification.md` — each of which
//! must produce at least one problem. The mutations never touch the files on
//! disk.

use std::fs;
use std::path::Path;

use xtask::fetch::DEFAULT_DEST;

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

// ---------------------------------------------------------------------------
// Workflow (`ci.yml`) reading: a line-based job-block extractor.
// ---------------------------------------------------------------------------

/// One job of a GitHub Actions workflow: its key and every line of its block,
/// from the job-key line up to (not including) the next job key or the end of
/// the top-level `jobs:` mapping. Comment and blank lines are kept; the checks
/// skip them through [`active`].
#[derive(Debug)]
struct Job<'a> {
    key: &'a str,
    lines: Vec<&'a str>,
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The jobs of a workflow. A job key is a non-comment line at exactly two
/// spaces of indentation inside the column-0 `jobs:` mapping.
fn jobs(ci: &str) -> Vec<Job<'_>> {
    let mut jobs: Vec<Job<'_>> = Vec::new();
    let mut in_jobs = false;
    for line in ci.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            if in_jobs {
                if let Some(job) = jobs.last_mut() {
                    job.lines.push(line);
                }
            }
            continue;
        }
        match indent(line) {
            0 => in_jobs = strip_comment(trimmed) == "jobs:",
            _ if !in_jobs => {}
            2 => jobs.push(Job {
                key: mapping_entry(trimmed).map_or(trimmed, |(key, _)| key),
                lines: vec![line],
            }),
            _ => {
                if let Some(job) = jobs.last_mut() {
                    job.lines.push(line);
                }
            }
        }
    }
    jobs
}

/// A YAML line with a trailing ` # comment` removed (only used for structural
/// lines such as `jobs:`; step lines are matched whole).
fn strip_comment(trimmed: &str) -> &str {
    trimmed
        .find(" #")
        .map_or(trimmed, |at| trimmed[..at].trim_end())
}

/// The trimmed, uncommented, non-blank lines of a block.
fn active<'a>(lines: &'a [&'a str]) -> impl Iterator<Item = &'a str> + 'a {
    lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

/// The key and value of a trimmed YAML mapping line, with a leading sequence
/// marker (`- `) and surrounding quotes removed: `- if: false` gives
/// `("if", "false")`.
fn mapping_entry(trimmed: &str) -> Option<(&str, &str)> {
    let item = trimmed.strip_prefix("- ").unwrap_or(trimmed).trim_start();
    let (key, value) = item.split_once(':')?;
    let key = key.trim().trim_matches(|c| c == '"' || c == '\'');
    let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
    (!key.is_empty() && !key.contains(char::is_whitespace)).then_some((key, value))
}

/// Problems that make a job's steps conditional or allowed to fail: any `if:`
/// or `continue-on-error:` in the block, at job or step level; and duplicate
/// job-level keys, which mean a commented-out job key merged two jobs.
fn guard_problems(job: &Job<'_>) -> Vec<String> {
    let mut problems = Vec::new();
    for line in active(&job.lines) {
        if let Some((key @ ("if" | "continue-on-error"), _)) = mapping_entry(line) {
            problems.push(format!(
                "job `{}` has `{key}:` (`{line}`); it must be unconditional and not allowed to fail",
                job.key
            ));
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    for line in &job.lines {
        let trimmed = line.trim();
        if indent(line) != 4 || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        if let Some((key, _)) = mapping_entry(trimmed) {
            if seen.contains(&key) {
                problems.push(format!(
                    "job `{}` repeats the key `{key}:`; is a job key commented out?",
                    job.key
                ));
            }
            seen.push(key);
        }
    }
    problems
}

/// The exact MSRV step required by LIB-QA-002.
fn msrv_step(msrv: &str) -> String {
    format!("- run: cargo +{msrv} test -p sacm -p sacm-cli")
}

/// LIB-QA-002 (Q2-1): some job has the uncommented line, trimmed, exactly
/// [`msrv_step`], and every job that has it is unconditional and not allowed
/// to fail.
fn check_msrv_job(ci: &str, msrv: &str) -> Vec<String> {
    let step = msrv_step(msrv);
    let mut problems = Vec::new();
    let mut found = false;
    for job in jobs(ci) {
        if active(&job.lines).any(|line| line == step) {
            found = true;
            problems.extend(guard_problems(&job));
        }
    }
    if !found {
        problems.push(format!(
            "no CI job has the exact, uncommented step `{step}`"
        ));
    }
    problems
}

/// The start of the cargo-deny step required by LIB-QA-003.
const DENY_ACTION: &str = "- uses: EmbarkStudios/cargo-deny-action@";

/// `arguments` (global cargo-deny flags) that do not narrow what is checked.
const DENY_ARGUMENTS_ALLOWED: [&str; 2] = ["--all-features", "--workspace"];

/// LIB-QA-003 (Q3-5): some job has an uncommented step line starting with
/// [`DENY_ACTION`]; every such job is unconditional and not allowed to fail,
/// and the step runs every check over the whole workspace.
fn check_deny_job(ci: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut found = false;
    for job in jobs(ci) {
        let lines = &job.lines;
        let Some(start) = lines.iter().position(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with('#') && trimmed.starts_with(DENY_ACTION)
        }) else {
            continue;
        };
        found = true;
        problems.extend(guard_problems(&job));
        let step_indent = indent(lines[start]);
        for line in &lines[start + 1..] {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if indent(line) <= step_indent {
                break; // the next step, or the end of `steps:`
            }
            let Some((key, value)) = mapping_entry(trimmed) else {
                continue;
            };
            let narrowing = match key {
                "command" => value != "check",
                "command-arguments" => true,
                "arguments" => value
                    .split_whitespace()
                    .any(|arg| !DENY_ARGUMENTS_ALLOWED.contains(&arg)),
                "manifest-path" => !matches!(value, "Cargo.toml" | "./Cargo.toml"),
                _ => false,
            };
            if narrowing {
                problems.push(format!(
                    "job `{}`: the cargo-deny step must run every check over the whole workspace, found `{trimmed}`",
                    job.key
                ));
            }
        }
    }
    if !found {
        problems.push(format!(
            "no CI job has an uncommented step starting `{DENY_ACTION}`"
        ));
    }
    problems
}

// ---------------------------------------------------------------------------
// Manifest and `deny.toml` checks.
// ---------------------------------------------------------------------------

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

fn lookup<'a>(table: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Value> {
    let (first, rest) = path.split_first()?;
    rest.iter()
        .try_fold(table.get(*first)?, |value, key| value.get(*key))
}

fn is_absent_or_empty(value: Option<&toml::Value>) -> bool {
    value.is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
}

/// Licences in `value` (an array of strings) outside [`PERMISSIVE`].
fn non_permissive(value: Option<&toml::Value>, what: &str) -> Vec<String> {
    let Some(array) = value.and_then(toml::Value::as_array) else {
        return vec![format!("{what} must be an array of licence identifiers")];
    };
    array
        .iter()
        .filter(|l| l.as_str().is_none_or(|l| !PERMISSIVE.contains(&l)))
        .map(|l| format!("{what} contains `{l}`, which is not in the permissive set"))
        .collect()
}

/// LIB-QA-003 (Q3-1 to Q3-4, and the original criteria): the dependency
/// policy in a parsed `deny.toml`.
fn check_deny_policy(deny: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    let mut require = |ok: bool, problem: &str| {
        if !ok {
            problems.push(problem.to_owned());
        }
    };

    // Q3-1: licences.
    let allow = lookup(deny, &["licenses", "allow"]);
    require(
        allow
            .and_then(toml::Value::as_array)
            .is_some_and(|a| a.iter().any(|l| l.as_str() == Some("MIT"))),
        "licenses.allow must allow MIT",
    );
    // Q3-2: exceptions and graph exclusions.
    let exceptions = lookup(deny, &["licenses", "exceptions"]);
    let exception_problems: Vec<String> = match exceptions {
        None => Vec::new(),
        Some(toml::Value::Array(entries)) => entries
            .iter()
            .flat_map(|entry| non_permissive(entry.get("allow"), "a licenses.exceptions entry"))
            .collect(),
        Some(other) => vec![format!(
            "licenses.exceptions must be an array, found `{other}`"
        )],
    };
    require(
        is_absent_or_empty(lookup(deny, &["graph", "exclude"])),
        "graph.exclude must be absent or empty: no crate may be excluded from the checks",
    );
    // Q3-3: sources.
    require(
        lookup(deny, &["sources", "unknown-git"]).and_then(toml::Value::as_str) == Some("deny"),
        "sources.unknown-git must be \"deny\"",
    );
    require(
        lookup(deny, &["sources", "unknown-registry"]).and_then(toml::Value::as_str)
            == Some("deny"),
        "sources.unknown-registry must be \"deny\"",
    );
    require(
        is_absent_or_empty(lookup(deny, &["sources", "allow-git"])),
        "sources.allow-git must be absent or empty",
    );
    require(
        lookup(deny, &["sources", "allow-org"]).is_none_or(|org| {
            org.as_table()
                .is_some_and(|t| t.values().all(|v| v.as_array().is_some_and(Vec::is_empty)))
        }),
        "sources.allow-org must be absent or have only empty lists",
    );
    require(
        lookup(deny, &["sources", "allow-registry"]).is_none_or(|r| {
            r.as_array()
                .is_some_and(|r| r.len() == 1 && r[0].as_str() == Some(CRATES_IO_INDEX))
        }),
        "sources.allow-registry must be absent or exactly the crates.io index",
    );
    // Q3-4: advisories.
    require(
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
        "every advisories.ignore entry must be a table with a non-empty `reason`",
    );
    require(
        lookup(deny, &["advisories", "db-urls"]).is_none(),
        "advisories.db-urls must be absent: no alternative advisory database",
    );
    require(
        lookup(deny, &["advisories", "yanked"]).and_then(toml::Value::as_str) == Some("deny"),
        "advisories.yanked must be \"deny\"",
    );

    problems.extend(non_permissive(allow, "licenses.allow"));
    problems.extend(exception_problems);
    problems
}

// ---------------------------------------------------------------------------
// Mutation helpers: in-memory edits of the real text.
// ---------------------------------------------------------------------------

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

fn leading(line: &str) -> &str {
    &line[..indent(line)]
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

/// `ci` without the job `key` (its whole block).
fn delete_job(ci: &str, key: &str) -> String {
    let job_lines: Vec<&str> = jobs(ci)
        .into_iter()
        .find(|job| job.key == key)
        .unwrap_or_else(|| panic!("job `{key}` exists"))
        .lines;
    let start = ci
        .lines()
        .position(|line| line.trim() == format!("{key}:") && indent(line) == 2)
        .expect("job key line");
    let mut out = String::new();
    for (i, line) in ci.lines().enumerate() {
        if !(start..start + job_lines.len()).contains(&i) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The key of the one job whose active lines satisfy `has`.
fn job_key_with(ci: &str, has: impl Fn(&str) -> bool) -> String {
    let keys: Vec<&str> = jobs(ci)
        .into_iter()
        .filter(|job| active(&job.lines).any(&has))
        .map(|job| job.key)
        .collect();
    assert_eq!(keys.len(), 1, "exactly one job matches, found {keys:?}");
    keys[0].to_owned()
}

/// Assert that `check` reports every mutation.
fn assert_all_reported(
    row: &str,
    mutations: &[(&str, String)],
    check: impl Fn(&str) -> Vec<String>,
) {
    for (name, mutated) in mutations {
        assert!(
            !check(mutated).is_empty(),
            "{row}: the check must report the mutation `{name}`; mutated text:\n{mutated}"
        );
    }
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
// LIB-QA-002
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
    let ci = read(".github/workflows/ci.yml");
    let problems = check_msrv_job(&ci, &msrv);
    assert!(
        problems.is_empty(),
        "LIB-QA-002: CI must test the published crates with the {msrv} toolchain: {problems:#?}"
    );
    let adr = read("docs/adr/0007-msrv-and-edition.md");
    assert!(
        adr.contains(&msrv),
        "LIB-QA-002: ADR 0007 must state the MSRV {msrv}"
    );
}

#[test]
fn lib_qa_002_msrv_job_check_rejects_known_bypasses() {
    let msrv = workspace_msrv();
    let ci = read(".github/workflows/ci.yml");
    assert!(
        check_msrv_job(&ci, &msrv).is_empty(),
        "LIB-QA-002: the real ci.yml must pass before mutations are meaningful"
    );
    let step = msrv_step(&msrv);
    let job = job_key_with(&ci, |line| line == step);
    let job_line = format!("{job}:");
    let mutations = [
        ("step commented out", comment_line(&ci, &step)),
        (
            "--no-run",
            replace_line(
                &ci,
                &step,
                &format!("- run: cargo +{msrv} test --no-run -p sacm -p sacm-cli"),
            ),
        ),
        (
            "|| true",
            replace_line(&ci, &step, &format!("{step} || true")),
        ),
        (
            "appended --lib",
            replace_line(&ci, &step, &format!("{step} --lib")),
        ),
        (
            "appended test-binary arguments",
            replace_line(&ci, &step, &format!("{step} -- --skip lib_")),
        ),
        (
            "renamed package",
            replace_line(
                &ci,
                &step,
                &format!("- run: cargo +{msrv} test -p sacm -p sacm-tools"),
            ),
        ),
        (
            "package dropped",
            replace_line(&ci, &step, &format!("- run: cargo +{msrv} test -p sacm")),
        ),
        (
            "stable toolchain",
            replace_line(&ci, &step, "- run: cargo +stable test -p sacm -p sacm-cli"),
        ),
        (
            "job deleted, command left in a comment in another job",
            delete_job(&ci, &job) + &format!("      # {step}\n"),
        ),
        (
            "job deleted, command left in a comment at the top",
            format!("# {step}\n") + &delete_job(&ci, &job),
        ),
        (
            "job-level continue-on-error: true",
            insert_after(&ci, &job_line, 2, &["continue-on-error: true"]),
        ),
        (
            "step-level continue-on-error: true",
            insert_after(&ci, &step, 2, &["continue-on-error: true"]),
        ),
        (
            "job-level if: false",
            insert_after(&ci, &job_line, 2, &["if: false"]),
        ),
        (
            "step-level if: false",
            insert_after(&ci, &step, 2, &["if: false"]),
        ),
        (
            "job key commented out (merges into the previous job)",
            comment_line(&ci, &job_line),
        ),
    ];
    assert_all_reported("LIB-QA-002", &mutations, |text| check_msrv_job(text, &msrv));
    assert!(
        !check_msrv_job(&ci, "1.86").is_empty(),
        "LIB-QA-002: a rust-version the CI job does not test must be reported"
    );
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

#[test]
fn lib_qa_002_rust_version_check_rejects_member_declarations() {
    let msrv = workspace_msrv();
    for path in ["crates/sacm", "crates/sacm-cli"] {
        let manifest = read(&format!("{path}/Cargo.toml"));
        assert!(
            check_inherits_rust_version(&manifest).is_empty(),
            "LIB-QA-002: {path} must pass before mutations are meaningful"
        );
        let target = manifest
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("rust-version"))
            .expect("the manifest has a rust-version line")
            .to_owned();
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
        assert_all_reported(
            &format!("LIB-QA-002 ({path})"),
            &mutations,
            check_inherits_rust_version,
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-QA-003
// ---------------------------------------------------------------------------

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
}

/// A TOML value written inline, e.g. `toml_value("[1, 2]")`.
fn toml_value(inline: &str) -> toml::Value {
    let table: toml::Table = format!("v = {inline}")
        .parse()
        .unwrap_or_else(|e| panic!("parse `{inline}`: {e}"));
    table["v"].clone()
}

/// The real `deny.toml`, with `value` set at `section.key`.
fn deny_with(section: &str, key: &str, value: &str) -> toml::Table {
    let mut deny = toml_table("deny.toml");
    let section = deny
        .entry(section)
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .expect("a deny.toml section is a table");
    section.insert(key.to_owned(), toml_value(value));
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
fn lib_qa_003_policy_check_accepts_approved_configuration() {
    let mut approved: Vec<(String, toml::Table)> = PERMISSIVE
        .iter()
        .map(|l| (format!("licenses.allow + {l}"), deny_allowing(l)))
        .collect();
    for (name, section, key, value) in [
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
    ] {
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

#[test]
fn lib_qa_003_ci_deny_job_runs_every_check() {
    let ci = read(".github/workflows/ci.yml");
    let problems = check_deny_job(&ci);
    assert!(
        problems.is_empty(),
        "LIB-QA-003: CI must run every cargo-deny check, unconditionally: {problems:#?}"
    );
}

#[test]
fn lib_qa_003_deny_job_check_rejects_known_bypasses() {
    let ci = read(".github/workflows/ci.yml");
    assert!(
        check_deny_job(&ci).is_empty(),
        "LIB-QA-003: the real ci.yml must pass before mutations are meaningful"
    );
    let step = ci
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(DENY_ACTION))
        .expect("ci.yml has the cargo-deny step")
        .to_owned();
    let job = job_key_with(&ci, |line| line == step);
    let job_line = format!("{job}:");
    let with = |inputs: &[&str]| {
        let mut lines = vec!["with:".to_owned()];
        lines.extend(inputs.iter().map(|input| format!("  {input}")));
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        insert_after(&ci, &step, 2, &lines)
    };
    let mutations = [
        ("step deleted", delete_line(&ci, &step)),
        ("step commented out", comment_line(&ci, &step)),
        ("job deleted", delete_job(&ci, &job)),
        ("command: check bans", with(&["command: check bans"])),
        (
            "command: check licenses",
            with(&["command: check licenses"]),
        ),
        (
            "command: every check named but bans",
            with(&["command: check advisories licenses sources"]),
        ),
        (
            "command: all four checks named",
            with(&["command: check advisories bans licenses sources"]),
        ),
        ("command: fetch", with(&["command: fetch"])),
        (
            "command-arguments",
            with(&["command: check", "command-arguments: --disable-fetch"]),
        ),
        (
            "arguments --exclude",
            with(&["arguments: --exclude sacm-cli"]),
        ),
        (
            "arguments --exclude-dev",
            with(&["arguments: --exclude-dev"]),
        ),
        (
            "arguments --exclude-unpublished",
            with(&["arguments: --all-features --exclude-unpublished"]),
        ),
        (
            "arguments --manifest-path",
            with(&["arguments: --manifest-path crates/sacm/Cargo.toml"]),
        ),
        (
            "manifest-path input",
            with(&["manifest-path: crates/sacm/Cargo.toml"]),
        ),
        (
            "job-level continue-on-error: true",
            insert_after(&ci, &job_line, 2, &["continue-on-error: true"]),
        ),
        (
            "step-level continue-on-error: true",
            insert_after(&ci, &step, 2, &["continue-on-error: true"]),
        ),
        (
            "job-level if: false",
            insert_after(&ci, &job_line, 2, &["if: false"]),
        ),
        (
            "step-level if: false",
            insert_after(&ci, &step, 2, &["if: false"]),
        ),
    ];
    assert_all_reported("LIB-QA-003", &mutations, check_deny_job);
    for (name, accepted) in [
        ("command: check", with(&["command: check"])),
        ("command: \"check\"", with(&["command: \"check\""])),
        (
            "arguments: --all-features",
            with(&["arguments: --all-features"]),
        ),
    ] {
        let problems = check_deny_job(&accepted);
        assert!(
            problems.is_empty(),
            "LIB-QA-003: `{name}` runs every check, but was reported: {problems:#?}"
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
