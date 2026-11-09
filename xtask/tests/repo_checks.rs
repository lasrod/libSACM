//! Regression tests for repository configuration: the CI gates, the MSRV
//! declaration and the dependency policy (rows LIB-QA-001..003), mutation
//! testing with cargo-mutants (LIB-QA-007), the `sacm-slice` skill's
//! PR-reuse and full-run-dispatch rules (LIB-QA-011), the CI gates as live
//! steps (LIB-QA-008), the pinning and reviewed inputs of third-party CI
//! actions (LIB-QA-009), and the git-ignore that keeps OMG documents out of
//! the repository (LIB-TOOL-002).
//!
//! These guard against a gate being removed or drifting out of step with the
//! manifest — the kind of change that passes review because nothing fails.
//!
//! The LIB-QA-002 and LIB-QA-003 checks are functions over the configuration
//! text (or its parsed TOML) that return the problems they find. Every problem
//! starts with a stable code in brackets (`[job-key] ...`), so a test can
//! assert *which* check reported a mutation, not only that something did.
//!
//! `ci.yml` is read as YAML data with `yaml-rust2` (YAML 1.2; duplicate keys
//! are a parse error; aliases resolved; `<<` kept as an ordinary key), the way
//! GitHub reads it: comments, block scalars, flow collections, continuation
//! lines and quoting are resolved before any value is compared. A parse
//! failure or a document count other than one is a reported problem, never a
//! skip; so is every shape error.
//!
//! Each check is applied to the real file, which must produce no problems, and
//! to in-memory mutations of its text — the bypasses listed in
//! `docs/verification/0002-m0-qa-and-cli-verification*.md` — each of which must
//! produce the expected problem codes. A YAML mutation must itself still parse
//! as one document (unless it is about a parse failure), so the check, not the
//! parser, is what rejects it. The mutations never touch the files on disk.

use std::fs;
use std::path::Path;

use xtask::fetch::DEFAULT_DEST;
use yaml_rust2::yaml::Hash;
use yaml_rust2::{Yaml, YamlLoader};

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

/// A problem: `[code] text`.
fn problem(code: &str, text: impl std::fmt::Display) -> String {
    format!("[{code}] {text}")
}

fn has_code(problems: &[String], code: &str) -> bool {
    let tag = format!("[{code}] ");
    problems.iter().any(|p| p.starts_with(&tag))
}

// ---------------------------------------------------------------------------
// Workflow (`ci.yml`) reading: YAML data, not lines.
// ---------------------------------------------------------------------------

/// The one YAML document of a workflow. A parse failure (including a
/// duplicate key or an unknown alias) is `[yaml-parse]`; zero or several
/// documents is `[yaml-documents]`.
fn load_workflow(ci: &str) -> Result<Yaml, String> {
    let docs = YamlLoader::load_from_str(ci)
        .map_err(|e| problem("yaml-parse", format!("ci.yml is not valid YAML: {e}")))?;
    match <[Yaml; 1]>::try_from(docs) {
        Ok([doc]) => Ok(doc),
        Err(docs) => Err(problem(
            "yaml-documents",
            format!(
                "ci.yml must be exactly one YAML document, found {}",
                docs.len()
            ),
        )),
    }
}

/// The value of the string key `key` in `map`. Keys are compared as strings
/// after parsing, so `if`, `"if"`, `'if'` and `? if` are the same key, and a
/// non-string key (`true:`, `1:`) is never equal to any of them.
fn get<'a>(map: &'a Hash, key: &str) -> Option<&'a Yaml> {
    map.iter()
        .find_map(|(k, v)| (k.as_str() == Some(key)).then_some(v))
}

/// A node for a message: a string as itself, anything else in debug form.
fn show(node: &Yaml) -> String {
    node.as_str()
        .map_or_else(|| format!("{node:?}"), str::to_owned)
}

/// A job of the workflow and its well-formed steps, numbered from 1.
struct Job<'a> {
    id: String,
    map: &'a Hash,
    steps: Vec<(usize, &'a Hash)>,
}

struct Workflow<'a> {
    top: &'a Hash,
    jobs: Vec<Job<'a>>,
}

/// The jobs of a parsed workflow. Every shape error is pushed to `problems`
/// as `[shape]`: the workflow or `jobs` not a mapping, a job id not a string,
/// a job not a mapping, `steps` not a sequence, a step not a mapping, and in
/// any step `run` or `uses` not a string or `with` not a mapping.
fn read_workflow<'a>(doc: &'a Yaml, problems: &mut Vec<String>) -> Option<Workflow<'a>> {
    let Some(top) = doc.as_hash() else {
        problems.push(problem("shape", "the workflow is not a mapping"));
        return None;
    };
    let mut jobs = Vec::new();
    match get(top, "jobs") {
        None => problems.push(problem("shape", "the workflow has no `jobs` mapping")),
        Some(Yaml::Hash(map)) => {
            for (key, value) in map {
                if let Some(job) = read_job(key, value, problems) {
                    jobs.push(job);
                }
            }
        }
        Some(other) => problems.push(problem(
            "shape",
            format!("`jobs` is not a mapping: {other:?}"),
        )),
    }
    Some(Workflow { top, jobs })
}

fn read_job<'a>(key: &Yaml, value: &'a Yaml, problems: &mut Vec<String>) -> Option<Job<'a>> {
    let id = show(key);
    if key.as_str().is_none() {
        problems.push(problem(
            "shape",
            format!("the job id `{id}` is not a string"),
        ));
    }
    let Some(map) = value.as_hash() else {
        problems.push(problem("shape", format!("job `{id}` is not a mapping")));
        return None;
    };
    let mut steps = Vec::new();
    match get(map, "steps") {
        None => {}
        Some(Yaml::Array(items)) => {
            for (index, item) in items.iter().enumerate() {
                let number = index + 1;
                let Some(step) = item.as_hash() else {
                    problems.push(problem(
                        "shape",
                        format!("job `{id}`, step {number} is not a mapping: {item:?}"),
                    ));
                    continue;
                };
                for (field, want_string) in [("run", true), ("uses", true), ("with", false)] {
                    let Some(value) = get(step, field) else {
                        continue;
                    };
                    let ok = if want_string {
                        value.as_str().is_some()
                    } else {
                        value.as_hash().is_some()
                    };
                    if !ok {
                        let kind = if want_string { "string" } else { "mapping" };
                        problems.push(problem(
                            "shape",
                            format!(
                                "job `{id}`, step {number}: `{field}` is not a {kind}: {value:?}"
                            ),
                        ));
                    }
                }
                steps.push((number, step));
            }
        }
        Some(other) => problems.push(problem(
            "shape",
            format!("job `{id}`: `steps` is not a sequence: {other:?}"),
        )),
    }
    Some(Job { id, map, steps })
}

/// Every `<<` merge key anywhere in the workflow, as `[merge-key]`. GitHub's
/// handling of merge keys is not something these checks reason about, so a
/// merge is never accepted, wherever it is.
fn merge_key_problems(node: &Yaml, path: &str, problems: &mut Vec<String>) {
    match node {
        Yaml::Hash(map) => {
            for (key, value) in map {
                let here = format!("{path}.{}", show(key));
                if key.as_str() == Some("<<") {
                    problems.push(problem(
                        "merge-key",
                        format!("`{here}` is a `<<` merge key"),
                    ));
                }
                merge_key_problems(value, &here, problems);
            }
        }
        Yaml::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                merge_key_problems(item, &format!("{path}[{index}]"), problems);
            }
        }
        _ => {}
    }
}

/// Every key of `map` outside `allowed` (a non-string key is always outside).
fn key_allowlist(map: &Hash, allowed: &[&str], code: &str, what: &str, problems: &mut Vec<String>) {
    for key in map.keys() {
        if !key.as_str().is_some_and(|k| allowed.contains(&k)) {
            problems.push(problem(
                code,
                format!("{what} has the key `{}`; allowed: {allowed:?}", show(key)),
            ));
        }
    }
}

/// The keys a job running the MSRV or the cargo-deny step may have (Y2-2,
/// Y3-2): nothing that makes it conditional (`if`, `needs`), allowed to fail
/// (`continue-on-error`), or changes how its steps run (`defaults`,
/// `container`, `strategy`, ...).
const JOB_KEYS: [&str; 5] = ["name", "runs-on", "steps", "env", "permissions"];

/// The keys the MSRV step may have (Y2-3).
const MSRV_STEP_KEYS: [&str; 4] = ["run", "name", "id", "env"];

/// The keys the cargo-deny step may have (Y3-2).
const DENY_STEP_KEYS: [&str; 4] = ["uses", "name", "id", "with"];

/// Keys no step of the MSRV or cargo-deny job may have (Y2-3, Y3-2).
const STEP_GUARDS: [&str; 3] = ["if", "continue-on-error", "shell"];

fn step_guard_problems(job: &Job<'_>, problems: &mut Vec<String>) {
    guarded_step_problems(job, &STEP_GUARDS, |_, _| false, problems);
}

/// Every key in `guards` on any step of `job`, as `[step-guard]`, except
/// where `exempt(step, key)` holds. [`step_guard_problems`] is this with
/// [`STEP_GUARDS`] and no exemption; LIB-QA-007 adds `working-directory` and
/// `<<`, and exempts the upload step's `if` (whose value it checks itself).
fn guarded_step_problems(
    job: &Job<'_>,
    guards: &[&str],
    exempt: impl Fn(&Hash, &str) -> bool,
    problems: &mut Vec<String>,
) {
    for (number, step) in &job.steps {
        for key in step.keys().filter_map(Yaml::as_str) {
            if guards.contains(&key) && !exempt(step, key) {
                problems.push(problem(
                    "step-guard",
                    format!(
                        "job `{}`, step {number} has `{key}`; every step must be unconditional, not allowed to fail, and use the default shell",
                        job.id
                    ),
                ));
            }
        }
    }
}

/// The exact MSRV command required by LIB-QA-002 (Y2-1).
fn msrv_command(msrv: &str) -> String {
    format!("cargo +{msrv} test -p sacm -p sacm-cli")
}

/// LIB-QA-002 (Y2-1 to Y2-3 and design conditions 2 to 5).
fn check_msrv_job(ci: &str, msrv: &str) -> Vec<String> {
    let doc = match load_workflow(ci) {
        Ok(doc) => doc,
        Err(problem) => return vec![problem],
    };
    let mut problems = Vec::new();
    merge_key_problems(&doc, "ci.yml", &mut problems);
    let Some(workflow) = read_workflow(&doc, &mut problems) else {
        return problems;
    };
    if get(workflow.top, "defaults").is_some() {
        problems.push(problem(
            "workflow-defaults",
            "the workflow has a top-level `defaults:`, which can change the shell of every `run` step",
        ));
    }
    let command = msrv_command(msrv);
    let mut found = false;
    for job in &workflow.jobs {
        let msrv_steps: Vec<&(usize, &Hash)> = job
            .steps
            .iter()
            .filter(|(_, step)| get(step, "run").and_then(Yaml::as_str) == Some(command.as_str()))
            .collect();
        if msrv_steps.is_empty() {
            continue;
        }
        found = true;
        key_allowlist(
            job.map,
            &JOB_KEYS,
            "job-key",
            &format!("the MSRV job `{}`", job.id),
            &mut problems,
        );
        step_guard_problems(job, &mut problems);
        for (number, step) in msrv_steps {
            key_allowlist(
                step,
                &MSRV_STEP_KEYS,
                "step-key",
                &format!("job `{}`, MSRV step {number}", job.id),
                &mut problems,
            );
        }
    }
    if !found {
        problems.push(problem(
            "msrv-step-missing",
            format!("no CI step has a `run` value exactly `{command}`"),
        ));
    }
    problems
}

/// The start of the `uses` value of the cargo-deny step (Y3-1).
const DENY_ACTION: &str = "EmbarkStudios/cargo-deny-action@";

/// `arguments` (global cargo-deny flags) that do not narrow what is checked.
const DENY_ARGUMENTS_ALLOWED: [&str; 2] = ["--all-features", "--workspace"];

/// The values cargo-deny accepts for `--log-level`, exactly as it accepts
/// them (it rejects `TRACE` and `Warn`). cargo-deny-action@v2 passes the
/// `log-level` input unquoted into `cargo-deny $*`, so any other value could
/// smuggle in further arguments such as `--exclude`.
const DENY_LOG_LEVELS: [&str; 6] = ["off", "error", "warn", "info", "debug", "trace"];

fn is_deny_step(step: &Hash) -> bool {
    get(step, "uses")
        .and_then(Yaml::as_str)
        .is_some_and(|uses| uses.starts_with(DENY_ACTION))
}

/// Y3-3: the cargo-deny step's inputs run every check over the whole
/// workspace. Input names are compared without regard to case, as GitHub
/// does. The action passes `command`, `command-arguments`, `arguments`,
/// `manifest-path` and `log-level` unquoted into `cargo-deny $*`, so each is
/// compared whole (or, for `arguments`, word by word against an allowlist):
/// a space in any of them would add arguments. A `with` that is not a mapping
/// is a `[shape]` problem reported by [`read_workflow`].
fn deny_input_problems(job: &str, number: usize, step: &Hash, problems: &mut Vec<String>) {
    let Some(Yaml::Hash(with)) = get(step, "with") else {
        return;
    };
    for (name, value) in with {
        let Some(name) = name.as_str() else {
            continue;
        };
        let text = value.as_str();
        let narrowing = match name.to_ascii_lowercase().as_str() {
            "command" => text != Some("check"),
            "command-arguments" => true,
            "arguments" => text.is_none_or(|arguments| {
                arguments
                    .split_whitespace()
                    .any(|arg| !DENY_ARGUMENTS_ALLOWED.contains(&arg))
            }),
            "manifest-path" => !matches!(text, Some("Cargo.toml" | "./Cargo.toml")),
            "log-level" => !text.is_some_and(|level| DENY_LOG_LEVELS.contains(&level)),
            _ => false,
        };
        if narrowing {
            problems.push(problem(
                "deny-input",
                format!(
                    "job `{job}`, step {number}: the cargo-deny step must run every check over the whole workspace, found `{name}: {}`",
                    show(value)
                ),
            ));
        }
    }
}

/// LIB-QA-003 (Y3-1 to Y3-3 and design conditions 2 to 5).
fn check_deny_job(ci: &str) -> Vec<String> {
    let doc = match load_workflow(ci) {
        Ok(doc) => doc,
        Err(problem) => return vec![problem],
    };
    let mut problems = Vec::new();
    merge_key_problems(&doc, "ci.yml", &mut problems);
    let Some(workflow) = read_workflow(&doc, &mut problems) else {
        return problems;
    };
    let mut found = false;
    for job in &workflow.jobs {
        let deny_steps: Vec<&(usize, &Hash)> = job
            .steps
            .iter()
            .filter(|(_, step)| is_deny_step(step))
            .collect();
        if deny_steps.is_empty() {
            continue;
        }
        found = true;
        key_allowlist(
            job.map,
            &JOB_KEYS,
            "job-key",
            &format!("the cargo-deny job `{}`", job.id),
            &mut problems,
        );
        step_guard_problems(job, &mut problems);
        for (number, step) in deny_steps {
            key_allowlist(
                step,
                &DENY_STEP_KEYS,
                "step-key",
                &format!("job `{}`, cargo-deny step {number}", job.id),
                &mut problems,
            );
            deny_input_problems(&job.id, *number, step, &mut problems);
        }
    }
    if !found {
        problems.push(problem(
            "deny-step-missing",
            format!("no CI step has a `uses` value starting `{DENY_ACTION}`"),
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

fn lookup<'a>(table: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Value> {
    let (first, rest) = path.split_first()?;
    rest.iter()
        .try_fold(table.get(*first)?, |value, key| value.get(*key))
}

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

fn require(problems: &mut Vec<String>, ok: bool, code: &str, text: &str) {
    if !ok {
        problems.push(problem(code, text));
    }
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

// ---------------------------------------------------------------------------
// Mutation helpers: in-memory edits of the real text.
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

/// `text` with the one line equal (trimmed) to `target` replaced by `lines`,
/// each indented like `target` plus its own leading spaces.
fn replace_lines(text: &str, target: &str, lines: &[&str]) -> String {
    edit_line(text, target, |line| {
        lines
            .iter()
            .map(|new| format!("{}{new}", leading(line)))
            .collect()
    })
}

/// `text` with the one line equal (trimmed) to `target` replaced by
/// `replacement`, keeping its indentation.
fn replace_line(text: &str, target: &str, replacement: &str) -> String {
    replace_lines(text, target, &[replacement])
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

fn padded(line: &str, deeper: usize, extra: &[&str]) -> Vec<String> {
    let pad = format!("{}{}", leading(line), " ".repeat(deeper));
    extra.iter().map(|e| format!("{pad}{e}")).collect()
}

/// `text` with `extra` inserted after the line equal (trimmed) to `target`,
/// indented `deeper` spaces more than it.
fn insert_after(text: &str, target: &str, deeper: usize, extra: &[&str]) -> String {
    edit_line(text, target, |line| {
        std::iter::once(line.to_owned())
            .chain(padded(line, deeper, extra))
            .collect()
    })
}

/// `text` with `extra` inserted before the line equal (trimmed) to `target`,
/// indented like it.
fn insert_before(text: &str, target: &str, extra: &[&str]) -> String {
    edit_line(text, target, |line| {
        padded(line, 0, extra)
            .into_iter()
            .chain(std::iter::once(line.to_owned()))
            .collect()
    })
}

/// `ci` with `lines` appended as a job of the top-level `jobs:` mapping (the
/// last top-level key of the real file), indented two spaces.
fn append_job(ci: &str, lines: &[&str]) -> String {
    let mut out = ci.to_owned();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    for line in lines {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// `ci` without the job `key`: its key line at two spaces and every following
/// line up to the next non-blank, non-comment line at two spaces or less.
fn delete_job(ci: &str, key: &str) -> String {
    let lines: Vec<&str> = ci.lines().collect();
    let start = lines
        .iter()
        .position(|line| indent(line) == 2 && line.trim() == format!("{key}:"))
        .unwrap_or_else(|| panic!("job `{key}` exists"));
    let end = lines[start + 1..]
        .iter()
        .position(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && indent(line) <= 2
        })
        .map_or(lines.len(), |offset| start + 1 + offset);
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if !(start..end).contains(&i) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The id of the one job of `ci` with a step satisfying `is`.
fn job_with_step(ci: &str, is: impl Fn(&Hash) -> bool) -> String {
    let doc = load_workflow(ci).expect("ci.yml parses");
    let mut problems = Vec::new();
    let workflow = read_workflow(&doc, &mut problems).expect("ci.yml is a mapping");
    let ids: Vec<String> = workflow
        .jobs
        .iter()
        .filter(|job| job.steps.iter().any(|(_, step)| is(step)))
        .map(|job| job.id.clone())
        .collect();
    assert_eq!(ids.len(), 1, "exactly one job matches, found {ids:?}");
    ids[0].clone()
}

/// The values of `field` in the steps of job `job`, as the YAML parser — and
/// so GitHub — reads them. Used to show what a mutation really does.
fn step_values(ci: &str, job: &str, field: &str) -> Vec<Yaml> {
    let doc = load_workflow(ci).expect("the mutation parses");
    let mut problems = Vec::new();
    let workflow = read_workflow(&doc, &mut problems).expect("the mutation is a mapping");
    workflow
        .jobs
        .iter()
        .filter(|j| j.id == job)
        .flat_map(|j| {
            j.steps
                .iter()
                .filter_map(|(_, step)| get(step, field).cloned())
        })
        .collect()
}

/// `ci` with a job `gate` inserted before the job `before`. The gate is always
/// skipped, and carries anchors later jobs can merge or alias: `&skip_job`
/// (the job), `&skip_step` (a step with `if: false`) and `&bans` (a mapping
/// `command: check bans`).
fn with_gate_job(ci: &str, before: &str) -> String {
    insert_before(
        ci,
        &format!("{before}:"),
        &[
            "gate: &skip_job",
            "  if: false",
            "  runs-on: ubuntu-latest",
            "  steps:",
            "    - &skip_step",
            "      if: false",
            "      run: echo skipped",
            "      env: &bans",
            "        command: check bans",
        ],
    )
}

/// Problem codes a mutation is expected to produce.
const MSRV_MISSING: &[&str] = &["msrv-step-missing"];
const DENY_MISSING: &[&str] = &["deny-step-missing"];
const DENY_INPUT: &[&str] = &["deny-input"];
const SHAPE: &[&str] = &["shape"];
const JOB_KEY: &[&str] = &["job-key"];
const STEP_KEY_AND_GUARD: &[&str] = &["step-key", "step-guard"];

/// A mutation of a configuration text and the problem codes the check must
/// report for it.
struct Mutation {
    name: String,
    text: String,
    codes: &'static [&'static str],
}

fn mutation(name: impl Into<String>, text: String, codes: &'static [&'static str]) -> Mutation {
    Mutation {
        name: name.into(),
        text,
        codes,
    }
}

fn parses_as_one_document(text: &str) -> bool {
    YamlLoader::load_from_str(text).is_ok_and(|docs| docs.len() == 1)
}

fn assert_reported(row: &str, m: &Mutation, check: &impl Fn(&str) -> Vec<String>) {
    assert!(
        !m.codes.is_empty(),
        "{row}: the mutation `{}` names no expected problem code",
        m.name
    );
    let problems = check(&m.text);
    for code in m.codes {
        assert!(
            has_code(&problems, code),
            "{row}: the check must report the mutation `{}` as `[{code}]`; it reported {problems:#?}; mutated text:\n{}",
            m.name,
            m.text
        );
    }
}

/// Assert that `check` reports every mutation with its codes. Each mutation
/// must still parse as one YAML document, so the check, not the parser,
/// rejects it.
fn assert_yaml_mutations_reported(
    row: &str,
    mutations: &[Mutation],
    check: impl Fn(&str) -> Vec<String>,
) {
    for m in mutations {
        assert!(
            parses_as_one_document(&m.text),
            "{row}: the mutation `{}` must parse as one YAML document, so that the check and not the parser rejects it; mutated text:\n{}",
            m.name,
            m.text
        );
        assert_reported(row, m, &check);
    }
}

/// Assert that `check` reports every mutation that is not one YAML document.
fn assert_unparsable_reported(
    row: &str,
    mutations: &[Mutation],
    check: impl Fn(&str) -> Vec<String>,
) {
    for m in mutations {
        assert!(
            !parses_as_one_document(&m.text),
            "{row}: the mutation `{}` is meant not to be one YAML document; mutated text:\n{}",
            m.name,
            m.text
        );
        assert_reported(row, m, &check);
    }
}

/// Assert that `check` accepts every control, each of which parses.
fn assert_yaml_accepted(
    row: &str,
    controls: &[(&str, String)],
    check: impl Fn(&str) -> Vec<String>,
) {
    for (name, text) in controls {
        assert!(
            parses_as_one_document(text),
            "{row}: the control `{name}` must parse as one YAML document; text:\n{text}"
        );
        let problems = check(text);
        assert!(
            problems.is_empty(),
            "{row}: `{name}` meets the requirement, but was reported: {problems:#?}; text:\n{text}"
        );
    }
}

/// Mutations of `ci` that are not one YAML document: each is `[yaml-parse]`
/// or `[yaml-documents]`, never a skip (design condition 2).
fn unparsable_mutations(ci: &str, job: &str) -> Vec<Mutation> {
    let job_line = format!("{job}:");
    let commented_out: String = ci.lines().flat_map(|line| ["# ", line, "\n"]).collect();
    vec![
        mutation(
            format!("duplicate key in job `{job}`"),
            insert_after(ci, &job_line, 2, &["runs-on: windows-latest"]),
            &["yaml-parse"],
        ),
        mutation(
            "unterminated flow sequence",
            replace_line(ci, "CARGO_INCREMENTAL: 0", "CARGO_INCREMENTAL: [0"),
            &["yaml-parse"],
        ),
        mutation(
            "tab indentation",
            replace_line(ci, "CARGO_INCREMENTAL: 0", "\tCARGO_INCREMENTAL: 0"),
            &["yaml-parse"],
        ),
        mutation(
            "alias of an unknown anchor",
            replace_line(ci, "CARGO_INCREMENTAL: 0", "CARGO_INCREMENTAL: *nowhere"),
            &["yaml-parse"],
        ),
        mutation(
            "a second document",
            format!("{ci}---\nname: second\n"),
            &["yaml-documents"],
        ),
        mutation("empty text", String::new(), &["yaml-documents"]),
        mutation(
            "every line commented out",
            commented_out,
            &["yaml-documents"],
        ),
    ]
}

/// Mutations of `ci` that parse but are malformed somewhere other than the
/// checked job, which stays intact: each is `[shape]` (design condition 4).
fn shape_mutations(ci: &str) -> Vec<Mutation> {
    let wrapped: String = ci
        .lines()
        .enumerate()
        .map(|(i, line)| match (i, line.is_empty()) {
            (0, _) => format!("- {line}\n"),
            (_, true) => "\n".to_owned(),
            (_, false) => format!("  {line}\n"),
        })
        .collect();
    let extra_job = |steps: &[&str]| {
        let mut lines = vec!["extra:", "  runs-on: ubuntu-latest", "  steps:"];
        lines.extend(steps);
        append_job(ci, &lines)
    };
    vec![
        mutation("workflow wrapped in a sequence", wrapped, &["shape"]),
        mutation(
            "`jobs` renamed",
            replace_line(ci, "jobs:", "job:"),
            &["shape"],
        ),
        mutation(
            "`jobs` is a sequence",
            replace_line(ci, "jobs:", "old-jobs:") + "jobs: [fmt, msrv, deny]\n",
            &["shape"],
        ),
        mutation(
            "a job that is not a mapping",
            append_job(ci, &["extra: just a string"]),
            &["shape"],
        ),
        mutation(
            "a job id that is not a string",
            append_job(
                ci,
                &[
                    "1:",
                    "  runs-on: ubuntu-latest",
                    "  steps:",
                    "    - run: echo",
                ],
            ),
            &["shape"],
        ),
        mutation(
            "`steps` is a mapping",
            append_job(
                ci,
                &[
                    "extra:",
                    "  runs-on: ubuntu-latest",
                    "  steps:",
                    "    run: echo",
                ],
            ),
            &["shape"],
        ),
        mutation(
            "a step that is a string",
            extra_job(&["    - echo hi"]),
            &["shape"],
        ),
        mutation(
            "`run` is a sequence",
            extra_job(&["    - run: [cargo, test]"]),
            &["shape"],
        ),
        mutation(
            "`run` is an integer",
            extra_job(&["    - run: 1"]),
            &["shape"],
        ),
        mutation(
            "`uses` is a mapping",
            extra_job(&["    - uses: { name: actions/checkout@v7 }"]),
            &["shape"],
        ),
        mutation(
            "`with` is a string",
            extra_job(&["    - uses: actions/checkout@v7", "      with: fetch-depth"]),
            &["shape"],
        ),
    ]
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

/// The real `ci.yml`, the MSRV, the MSRV command, its step line as written
/// in the file (trimmed), and the MSRV job's id. The real file must pass.
fn msrv_fixture() -> (String, String, String, String, String) {
    let msrv = workspace_msrv();
    let ci = read(".github/workflows/ci.yml");
    let command = msrv_command(&msrv);
    let step = format!("- run: {command}");
    let job = job_with_step(&ci, |s| {
        get(s, "run").and_then(Yaml::as_str) == Some(command.as_str())
    });
    let problems = check_msrv_job(&ci, &msrv);
    assert!(
        problems.is_empty(),
        "LIB-QA-002: the real ci.yml must pass before mutations are meaningful: {problems:#?}"
    );
    (ci, msrv, command, step, job)
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
    let (ci, msrv, _, step, job) = msrv_fixture();
    let job_line = format!("{job}:");
    let mutations = [
        mutation("step commented out", comment_line(&ci, &step), MSRV_MISSING),
        mutation(
            "--no-run",
            replace_line(
                &ci,
                &step,
                &format!("- run: cargo +{msrv} test --no-run -p sacm -p sacm-cli"),
            ),
            MSRV_MISSING,
        ),
        mutation(
            "|| true",
            replace_line(&ci, &step, &format!("{step} || true")),
            MSRV_MISSING,
        ),
        mutation(
            "appended --lib",
            replace_line(&ci, &step, &format!("{step} --lib")),
            MSRV_MISSING,
        ),
        mutation(
            "appended test-binary arguments",
            replace_line(&ci, &step, &format!("{step} -- --skip lib_")),
            MSRV_MISSING,
        ),
        mutation(
            "renamed package",
            replace_line(
                &ci,
                &step,
                &format!("- run: cargo +{msrv} test -p sacm -p sacm-tools"),
            ),
            MSRV_MISSING,
        ),
        mutation(
            "package dropped",
            replace_line(&ci, &step, &format!("- run: cargo +{msrv} test -p sacm")),
            MSRV_MISSING,
        ),
        mutation(
            "stable toolchain",
            replace_line(&ci, &step, "- run: cargo +stable test -p sacm -p sacm-cli"),
            MSRV_MISSING,
        ),
        mutation(
            "job deleted, command left in a comment in another job",
            delete_job(&ci, &job) + &format!("      # {step}\n"),
            MSRV_MISSING,
        ),
        mutation(
            "job deleted, command left in a comment at the top",
            format!("# {step}\n") + &delete_job(&ci, &job),
            MSRV_MISSING,
        ),
        mutation(
            "job-level continue-on-error: true",
            insert_after(&ci, &job_line, 2, &["continue-on-error: true"]),
            JOB_KEY,
        ),
        mutation(
            "step-level continue-on-error: true",
            insert_after(&ci, &step, 2, &["continue-on-error: true"]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "job-level if: false",
            insert_after(&ci, &job_line, 2, &["if: false"]),
            JOB_KEY,
        ),
        mutation(
            "step-level if: false",
            insert_after(&ci, &step, 2, &["if: false"]),
            STEP_KEY_AND_GUARD,
        ),
    ];
    assert_yaml_mutations_reported("LIB-QA-002", &mutations, |text| check_msrv_job(text, &msrv));
    // Commenting out the job key merges the job's keys into the previous job:
    // duplicate keys, which the YAML parser (like GitHub) rejects.
    assert_unparsable_reported(
        "LIB-QA-002",
        &[mutation(
            "job key commented out (merges into the previous job)",
            comment_line(&ci, &job_line),
            &["yaml-parse"],
        )],
        |text| check_msrv_job(text, &msrv),
    );
    assert!(
        has_code(&check_msrv_job(&ci, "1.86"), "msrv-step-missing"),
        "LIB-QA-002: a rust-version the CI job does not test must be reported"
    );
}

/// Y2-1: the `run` value is compared after parsing, so no YAML syntax can
/// make a step look like the MSRV step while GitHub runs something else.
#[test]
fn lib_qa_002_msrv_job_check_rejects_yaml_run_bypasses() {
    let (ci, msrv, command, step, job) = msrv_fixture();
    let indented = format!("    {command}");

    let heredoc = replace_lines(
        &ci,
        &step,
        &["- run: |", "    cat <<'EOF'", &indented, "    EOF"],
    );
    let elsewhere = delete_job(
        &replace_lines(
            &ci,
            "- run: cargo test -p xtask",
            &["- run: |", "    cargo test -p xtask", &indented],
        ),
        &job,
    );
    let no_run_continued = replace_lines(&ci, &step, &[&step, "    --no-run"]);
    let or_true_continued = replace_lines(&ci, &step, &[&step, "    || true"]);
    let no_run_folded = replace_lines(&ci, &step, &["- run: >-", &indented, "    --no-run"]);
    // What GitHub runs for each of them: never the exact command.
    for (name, text, runs) in [
        (
            "heredoc",
            &heredoc,
            format!("cat <<'EOF'\n{command}\nEOF\n"),
        ),
        (
            "continuation --no-run",
            &no_run_continued,
            format!("{command} --no-run"),
        ),
        (
            "continuation || true",
            &or_true_continued,
            format!("{command} || true"),
        ),
        (
            "folded --no-run",
            &no_run_folded,
            format!("{command} --no-run"),
        ),
    ] {
        assert!(
            step_values(text, &job, "run").contains(&Yaml::String(runs.clone())),
            "LIB-QA-002: the mutation `{name}` must make GitHub run `{runs:?}`"
        );
    }
    assert!(
        step_values(&elsewhere, "test", "run")
            .contains(&Yaml::String(format!("cargo test -p xtask\n{command}\n"))),
        "LIB-QA-002: the mutation `line inside another job's run: |` must put the command in a block"
    );

    let mutations = [
        mutation(
            "heredoc that only prints the line (Copilot 1)",
            heredoc,
            MSRV_MISSING,
        ),
        mutation(
            "echo of the line",
            replace_line(&ci, &step, &format!("- run: echo \"{command}\"")),
            MSRV_MISSING,
        ),
        mutation(
            "line inside another job's run: |, MSRV job deleted",
            elsewhere,
            MSRV_MISSING,
        ),
        mutation("continuation line --no-run", no_run_continued, MSRV_MISSING),
        mutation("continuation line || true", or_true_continued, MSRV_MISSING),
        mutation("run: >- folding in --no-run", no_run_folded, MSRV_MISSING),
        mutation(
            "alias of another step's command",
            replace_line(
                &replace_line(
                    &ci,
                    "- run: cargo test -p xtask",
                    "- run: &xtask cargo test -p xtask",
                ),
                &step,
                "- run: *xtask",
            ),
            MSRV_MISSING,
        ),
    ];
    assert_yaml_mutations_reported("LIB-QA-002", &mutations, |text| check_msrv_job(text, &msrv));
}

/// Y2-2, Y2-3 and design condition 5: nothing in YAML form can make the MSRV
/// job or step conditional, allowed to fail, or run under another shell.
#[test]
fn lib_qa_002_msrv_job_check_rejects_yaml_guard_bypasses() {
    let (ci, msrv, _, step, job) = msrv_fixture();
    let mutations = msrv_guard_bypasses(&ci, &step, &job);
    assert_yaml_mutations_reported("LIB-QA-002", &mutations, |text| check_msrv_job(text, &msrv));
}

/// `text` with `extra` inserted after the one line of job `job` whose
/// trimmed form starts with `prefix`, indented `deeper` spaces more than it.
/// The job is its key line at two spaces and every line up to the next
/// non-blank, non-comment line at two spaces or less. Panics if the job has
/// not exactly one such line, so a mutation can never silently be a no-op.
fn insert_after_in_job(
    text: &str,
    job: &str,
    prefix: &str,
    deeper: usize,
    extra: &[&str],
) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|line| indent(line) == 2 && line.trim() == format!("{job}:"))
        .unwrap_or_else(|| panic!("job `{job}` exists"));
    let end = lines[start + 1..]
        .iter()
        .position(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && indent(line) <= 2
        })
        .map_or(lines.len(), |offset| start + 1 + offset);
    let hits: Vec<usize> = (start..end)
        .filter(|&i| lines[i].trim().starts_with(prefix))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "mutation target `{prefix}` must occur exactly once in job `{job}`"
    );
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        out.push_str(line);
        out.push('\n');
        if i == hits[0] {
            for new in padded(line, deeper, extra) {
                out.push_str(&new);
                out.push('\n');
            }
        }
    }
    out
}

/// The Y2-2 and Y2-3 guard bypasses of the MSRV job `job` of `ci`, whose MSRV
/// step line (trimmed) is `step`. Every edit is found by structure, not by
/// the text of a `uses`, so the same mutations apply to the unpinned and the
/// pinned `ci.yml` (slice 0009).
fn msrv_guard_bypasses(ci: &str, step: &str, job: &str) -> Vec<Mutation> {
    let job_line = format!("{job}:");
    let gated = with_gate_job(ci, job);
    let toolchain_step = format!("- uses: {TOOLCHAIN_ACTION}");
    let ci = ci.to_owned();
    vec![
        mutation(
            "needs: a skipped job",
            insert_after(&gated, &job_line, 2, &["needs: gate"]),
            JOB_KEY,
        ),
        mutation(
            "needs: [a skipped job]",
            insert_after(&gated, &job_line, 2, &["needs: [gate]"]),
            JOB_KEY,
        ),
        mutation(
            "job-level <<: *anchor of a skipped job",
            insert_after(&gated, &job_line, 2, &["<<: *skip_job"]),
            &["job-key", "merge-key"],
        ),
        mutation(
            "step-level <<: *anchor of a skipped step",
            insert_after(&gated, step, 2, &["<<: *skip_step"]),
            &["step-key", "merge-key"],
        ),
        mutation(
            "step-level shell: \"true {0}\"",
            insert_after(&ci, step, 2, &["shell: \"true {0}\""]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "continue-on-error on another step of the job",
            insert_after_in_job(&ci, job, &toolchain_step, 2, &["continue-on-error: true"]),
            &["step-guard"],
        ),
        mutation(
            "job-level defaults.run.shell",
            insert_after(
                &ci,
                &job_line,
                2,
                &["defaults:", "  run:", "    shell: \"true {0}\""],
            ),
            JOB_KEY,
        ),
        mutation(
            "job-level defaults.run.shell, flow style",
            insert_after(
                &ci,
                &job_line,
                2,
                &["defaults: { run: { shell: \"true {0}\" } }"],
            ),
            JOB_KEY,
        ),
        mutation(
            "workflow-level defaults.run.shell",
            insert_before(
                &ci,
                "jobs:",
                &["defaults:", "  run:", "    shell: \"true {0}\""],
            ),
            &["workflow-defaults"],
        ),
        mutation(
            "timeout-minutes: 0 on the job",
            insert_after(&ci, &job_line, 2, &["timeout-minutes: 0"]),
            JOB_KEY,
        ),
        mutation(
            "working-directory on the step",
            insert_after(&ci, step, 2, &["working-directory: crates/sacm"]),
            &["step-key"],
        ),
    ]
}

/// Y2-2, Y2-3: keys are compared as strings after parsing, so quoting, an
/// explicit `?` key or a YAML 1.1 boolean spelling changes nothing, and a
/// non-string key is never mistaken for an allowed one.
#[test]
fn lib_qa_002_msrv_job_check_compares_keys_as_strings() {
    let (ci, msrv, _, step, job) = msrv_fixture();
    let job_line = format!("{job}:");
    let mutations = [
        mutation(
            "double-quoted \"if\" key on the step",
            insert_after(&ci, &step, 2, &["\"if\": false"]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "explicit ? if key on the step",
            insert_after(&ci, &step, 2, &["? if", ": false"]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "single-quoted 'continue-on-error' key on the job, value yes",
            insert_after(&ci, &job_line, 2, &["'continue-on-error': yes"]),
            JOB_KEY,
        ),
        mutation(
            "step-level continue-on-error: yes",
            insert_after(&ci, &step, 2, &["continue-on-error: yes"]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "a non-string key on the job",
            insert_after(&ci, &job_line, 2, &["true: false"]),
            JOB_KEY,
        ),
    ];
    assert_yaml_mutations_reported("LIB-QA-002", &mutations, |text| check_msrv_job(text, &msrv));
}

#[test]
fn lib_qa_002_msrv_job_check_reports_parse_and_shape_errors() {
    let (ci, msrv, _, _, job) = msrv_fixture();
    assert_unparsable_reported("LIB-QA-002", &unparsable_mutations(&ci, &job), |text| {
        check_msrv_job(text, &msrv)
    });
    assert_yaml_mutations_reported("LIB-QA-002", &shape_mutations(&ci), |text| {
        check_msrv_job(text, &msrv)
    });
}

#[test]
fn lib_qa_002_msrv_job_check_accepts_equivalent_yaml() {
    let (ci, msrv, command, step, job) = msrv_fixture();
    let job_line = format!("{job}:");
    let indented = format!("    {command}");
    let (head, tail) = command
        .split_once(" -p ")
        .expect("the command has a -p argument");
    let head_line = format!("- run: {head}");
    let tail_line = format!("    -p {tail}");
    let folded_head = format!("    {head}");
    let controls = [
        ("the real ci.yml", ci.clone()),
        (
            "a trailing comment on the MSRV step",
            replace_line(&ci, &step, &format!("{step} # the MSRV gate")),
        ),
        (
            "double-quoted run",
            replace_line(&ci, &step, &format!("- run: \"{command}\"")),
        ),
        (
            "single-quoted run",
            replace_line(&ci, &step, &format!("- run: '{command}'")),
        ),
        (
            "run: >- folding to the exact command",
            replace_lines(&ci, &step, &["- run: >-", &folded_head, &tail_line]),
        ),
        (
            "run: |- with the exact command",
            replace_lines(&ci, &step, &["- run: |-", &indented]),
        ),
        (
            "plain continuation folding to the exact command",
            replace_lines(&ci, &step, &[&head_line, &tail_line]),
        ),
        (
            "flow-style step",
            replace_line(
                &ci,
                &step,
                &format!("- {{ name: MSRV tests, run: {command} }}"),
            ),
        ),
        (
            "allowed step keys name, id, env",
            insert_after(
                &ci,
                &step,
                2,
                &[
                    "name: MSRV tests",
                    "id: msrv-tests",
                    "env:",
                    "  RUST_BACKTRACE: 1",
                ],
            ),
        ),
        (
            "allowed job keys permissions, env",
            insert_after(
                &ci,
                &job_line,
                2,
                &[
                    "permissions:",
                    "  contents: read",
                    "env:",
                    "  RUST_BACKTRACE: 1",
                ],
            ),
        ),
        (
            "alias resolving to the exact command",
            replace_line(
                &insert_after(
                    &ci,
                    &job_line,
                    2,
                    &["env:", &format!("  MSRV_TEST: &msrv_test {command}")],
                ),
                &step,
                "- run: *msrv_test",
            ),
        ),
    ];
    assert_yaml_accepted("LIB-QA-002", &controls, |text| check_msrv_job(text, &msrv));
}

#[test]
fn lib_qa_002_workflow_reader_keeps_yaml_1_2_scalars() {
    let doc = load_workflow("on: yes\nyes: no\nno: off\noff: on\ny: n\nn: y\n")
        .expect("LIB-QA-002: a plain YAML mapping parses");
    let map = doc
        .as_hash()
        .expect("LIB-QA-002: the document is a mapping");
    for key in ["on", "yes", "no", "off", "y", "n"] {
        let value = get(map, key);
        assert!(
            value.is_some(),
            "LIB-QA-002: `{key}` must stay a string key under YAML 1.2, found {map:?}"
        );
        assert!(
            value.and_then(Yaml::as_str).is_some(),
            "LIB-QA-002: the value of `{key}` must stay a string under YAML 1.2, found {value:?}"
        );
    }
    let ci = load_workflow(&read(".github/workflows/ci.yml"))
        .expect("LIB-QA-002: the real ci.yml is one YAML document");
    let top = ci.as_hash().expect("LIB-QA-002: ci.yml is a mapping");
    assert!(
        get(top, "on").is_some_and(|on| on.as_hash().is_some()),
        "LIB-QA-002: the real ci.yml's `on` key must be read as the string `on`"
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
    assert_eq!(
        deny["licenses"]
            .get("include-dev")
            .and_then(toml::Value::as_bool),
        Some(true),
        "LIB-QA-003: dev-dependencies must be licence-checked (`licenses.include-dev = true`)"
    );
}

/// A TOML value written inline, e.g. `toml_value("[1, 2]")`.
fn toml_value(inline: &str) -> toml::Value {
    let table: toml::Table = format!("v = {inline}")
        .parse()
        .unwrap_or_else(|e| panic!("parse `{inline}`: {e}"));
    table["v"].clone()
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

/// The real `ci.yml`, its cargo-deny step line (trimmed) and the deny job id.
/// The real file must pass.
fn deny_fixture() -> (String, String, String) {
    let ci = read(".github/workflows/ci.yml");
    let step = ci
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(&format!("- uses: {DENY_ACTION}")))
        .expect("ci.yml has the cargo-deny step")
        .to_owned();
    let job = job_with_step(&ci, is_deny_step);
    let problems = check_deny_job(&ci);
    assert!(
        problems.is_empty(),
        "LIB-QA-003: the real ci.yml must pass before mutations are meaningful: {problems:#?}"
    );
    (ci, step, job)
}

/// The cargo-deny `input` values of job `job`, as GitHub reads them.
fn deny_input_values(ci: &str, job: &str, input: &str) -> Vec<String> {
    step_values(ci, job, "with")
        .iter()
        .filter_map(Yaml::as_hash)
        .filter_map(|with| get(with, input).and_then(Yaml::as_str))
        .map(str::to_owned)
        .collect()
}

/// The cargo-deny `command` inputs of job `job`, as GitHub reads them.
fn deny_commands(ci: &str, job: &str) -> Vec<String> {
    deny_input_values(ci, job, "command")
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
    let (ci, step, job) = deny_fixture();
    let job_line = format!("{job}:");
    let with = |inputs: &[&str]| {
        let mut lines = vec!["with:".to_owned()];
        lines.extend(inputs.iter().map(|input| format!("  {input}")));
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        insert_after(&ci, &step, 2, &lines)
    };
    let mutations = [
        mutation("step deleted", delete_line(&ci, &step), DENY_MISSING),
        mutation("step commented out", comment_line(&ci, &step), DENY_MISSING),
        mutation("job deleted", delete_job(&ci, &job), DENY_MISSING),
        mutation(
            "command: check bans",
            with(&["command: check bans"]),
            DENY_INPUT,
        ),
        mutation(
            "command: check licenses",
            with(&["command: check licenses"]),
            DENY_INPUT,
        ),
        mutation(
            "command: every check named but bans",
            with(&["command: check advisories licenses sources"]),
            DENY_INPUT,
        ),
        mutation(
            "command: all four checks named",
            with(&["command: check advisories bans licenses sources"]),
            DENY_INPUT,
        ),
        mutation("command: fetch", with(&["command: fetch"]), DENY_INPUT),
        mutation(
            "command-arguments",
            with(&["command: check", "command-arguments: --disable-fetch"]),
            DENY_INPUT,
        ),
        mutation(
            "arguments --exclude",
            with(&["arguments: --exclude sacm-cli"]),
            DENY_INPUT,
        ),
        mutation(
            "arguments --exclude-dev",
            with(&["arguments: --exclude-dev"]),
            DENY_INPUT,
        ),
        mutation(
            "arguments --exclude-unpublished",
            with(&["arguments: --all-features --exclude-unpublished"]),
            DENY_INPUT,
        ),
        mutation(
            "arguments --manifest-path",
            with(&["arguments: --manifest-path crates/sacm/Cargo.toml"]),
            DENY_INPUT,
        ),
        mutation(
            "manifest-path input",
            with(&["manifest-path: crates/sacm/Cargo.toml"]),
            DENY_INPUT,
        ),
        mutation(
            "job-level continue-on-error: true",
            insert_after(&ci, &job_line, 2, &["continue-on-error: true"]),
            JOB_KEY,
        ),
        mutation(
            "step-level continue-on-error: true",
            insert_after(&ci, &step, 2, &["continue-on-error: true"]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "job-level if: false",
            insert_after(&ci, &job_line, 2, &["if: false"]),
            JOB_KEY,
        ),
        mutation(
            "step-level if: false",
            insert_after(&ci, &step, 2, &["if: false"]),
            STEP_KEY_AND_GUARD,
        ),
    ];
    assert_yaml_mutations_reported("LIB-QA-003", &mutations, check_deny_job);
    assert_yaml_accepted(
        "LIB-QA-003",
        &[
            ("command: check", with(&["command: check"])),
            ("command: \"check\"", with(&["command: \"check\""])),
            (
                "arguments: --all-features",
                with(&["arguments: --all-features"]),
            ),
        ],
        check_deny_job,
    );
}

/// `ci` with the gate job of [`with_gate_job`] inserted before the MSRV job,
/// so its anchors are defined before the cargo-deny job.
fn gated_before_msrv(ci: &str) -> String {
    let command = msrv_command(&workspace_msrv());
    let msrv_job = job_with_step(ci, |s| {
        get(s, "run").and_then(Yaml::as_str) == Some(command.as_str())
    });
    with_gate_job(ci, &msrv_job)
}

/// Y3-1 and Y3-3: the cargo-deny step and its inputs are read after parsing,
/// so no YAML syntax can narrow the checks while looking like it does not.
#[test]
fn lib_qa_003_deny_job_check_rejects_yaml_input_bypasses() {
    let (ci, step, job) = deny_fixture();
    let gated = gated_before_msrv(&ci);
    let after_step = |text: &str, lines: &[&str]| insert_after(text, &step, 2, lines);
    let block_with = |inputs: &[&str]| {
        let mut lines = vec!["with:".to_owned()];
        lines.extend(inputs.iter().map(|input| format!("  {input}")));
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        after_step(&ci, &lines)
    };
    let printed = format!("    {step}");

    let flow_quoted = after_step(&ci, &["with: { command: \"check bans\" }"]);
    let flow_single = after_step(&ci, &["with: { command: 'check bans' }"]);
    let flow_plain = after_step(&ci, &["with: { command: check bans }"]);
    let block_continued = block_with(&["command: check", "  bans"]);
    let flow_continued = after_step(&ci, &["with: { command: check", "    bans }"]);
    let block_folded = block_with(&["command: >-", "  check", "  bans"]);
    let aliased = after_step(&gated, &["with: *bans"]);
    // What GitHub passes as `command` for each of them: `check bans`.
    for (name, text) in [
        ("flow, double-quoted", &flow_quoted),
        ("flow, single-quoted", &flow_single),
        ("flow, plain", &flow_plain),
        ("block, continuation line", &block_continued),
        ("flow, continuation line", &flow_continued),
        ("block, folded scalar", &block_folded),
        ("alias of a mapping", &aliased),
    ] {
        assert_eq!(
            deny_commands(text, &job),
            ["check bans"],
            "LIB-QA-003: the mutation `{name}` must make GitHub pass `command: check bans`"
        );
    }

    let mutations = [
        mutation(
            "heredoc that only prints the uses: line (Copilot 1)",
            replace_lines(
                &ci,
                &step,
                &["- run: |", "    cat <<'EOF'", &printed, "    EOF"],
            ),
            DENY_MISSING,
        ),
        mutation(
            "flow command: \"check bans\" (Copilot 2)",
            flow_quoted,
            DENY_INPUT,
        ),
        mutation("flow command: 'check bans'", flow_single, DENY_INPUT),
        mutation("flow command: check bans, unquoted", flow_plain, DENY_INPUT),
        mutation(
            "flow arguments: --exclude sacm-cli",
            after_step(&ci, &["with: { arguments: --exclude sacm-cli }"]),
            DENY_INPUT,
        ),
        mutation(
            "flow arguments: --exclude-dev",
            after_step(&ci, &["with: { arguments: --exclude-dev }"]),
            DENY_INPUT,
        ),
        mutation(
            "flow manifest-path elsewhere",
            after_step(&ci, &["with: { manifest-path: crates/sacm/Cargo.toml }"]),
            DENY_INPUT,
        ),
        mutation(
            "flow command-arguments",
            after_step(
                &ci,
                &["with: { command: check, command-arguments: --hide-inclusion-graph }"],
            ),
            DENY_INPUT,
        ),
        mutation(
            "continuation line on command, block style",
            block_continued,
            DENY_INPUT,
        ),
        mutation(
            "continuation line on command, flow style",
            flow_continued,
            DENY_INPUT,
        ),
        mutation("command: >- folding in bans", block_folded, DENY_INPUT),
        mutation("with: an alias of a narrowing mapping", aliased, DENY_INPUT),
        mutation(
            "input name in another case (COMMAND)",
            after_step(&ci, &["with: { COMMAND: check bans }"]),
            DENY_INPUT,
        ),
    ];
    assert_yaml_mutations_reported("LIB-QA-003", &mutations, check_deny_job);
}

/// Y3-3: cargo-deny-action@v2 passes `log-level`, `manifest-path`,
/// `arguments`, `command` and `command-arguments` into `cargo-deny $*`
/// unquoted, so a value with spaces becomes several arguments and can narrow
/// the checks (Copilot, PR #7). `log-level` must be exactly one level that
/// cargo-deny accepts; it accepts them in lower case only.
#[test]
fn lib_qa_003_deny_job_check_rejects_unquoted_input_injection() {
    let (ci, step, job) = deny_fixture();
    let after_step = |lines: &[&str]| insert_after(&ci, &step, 2, lines);
    let block_with = |inputs: &[&str]| {
        let mut lines = vec!["with:".to_owned()];
        lines.extend(inputs.iter().map(|input| format!("  {input}")));
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        after_step(&lines)
    };
    let flow_log_level = after_step(&["with: { log-level: \"warn --exclude sacm-cli\" }"]);
    let block_log_level = block_with(&["log-level: warn --exclude sacm-cli"]);
    for (name, text) in [
        ("flow, double-quoted", &flow_log_level),
        ("block, plain", &block_log_level),
    ] {
        assert_eq!(
            deny_input_values(text, &job, "log-level"),
            ["warn --exclude sacm-cli"],
            "LIB-QA-003: the mutation `{name}` must make GitHub pass `log-level: warn --exclude sacm-cli`"
        );
    }
    let mutations = [
        mutation(
            "flow log-level: \"warn --exclude sacm-cli\" (Copilot, PR #7)",
            flow_log_level,
            DENY_INPUT,
        ),
        mutation(
            "block log-level: warn --exclude sacm-cli",
            block_log_level,
            DENY_INPUT,
        ),
        mutation(
            "log-level: \"warn --exclude-dev\"",
            after_step(&["with: { log-level: \"warn --exclude-dev\" }"]),
            DENY_INPUT,
        ),
        mutation(
            "log-level: \"--exclude sacm-cli\"",
            after_step(&["with: { log-level: \"--exclude sacm-cli\" }"]),
            DENY_INPUT,
        ),
        mutation(
            "log-level: verbose, not a cargo-deny level",
            block_with(&["log-level: verbose"]),
            DENY_INPUT,
        ),
        mutation(
            "log-level: TRACE, which cargo-deny rejects",
            block_with(&["log-level: TRACE"]),
            DENY_INPUT,
        ),
        mutation(
            "log-level given as a number",
            block_with(&["log-level: 5"]),
            DENY_INPUT,
        ),
        mutation(
            "manifest-path: \"Cargo.toml --exclude sacm-cli\"",
            after_step(&["with: { manifest-path: \"Cargo.toml --exclude sacm-cli\" }"]),
            DENY_INPUT,
        ),
        mutation(
            "command: \"check --exclude sacm-cli\"",
            after_step(&["with: { command: \"check --exclude sacm-cli\" }"]),
            DENY_INPUT,
        ),
        mutation(
            "arguments: \"--workspace --exclude sacm-cli\"",
            after_step(&["with: { arguments: \"--workspace --exclude sacm-cli\" }"]),
            DENY_INPUT,
        ),
    ];
    assert_yaml_mutations_reported("LIB-QA-003", &mutations, check_deny_job);
}

/// Round 3 F1: the `log-level` rule holds however the input is spelled: under
/// an upper-case input name (GitHub matches input names without regard to
/// case), through `with: *anchor` (an alias of a mapping anchored in a skipped
/// job before the cargo-deny job), and as an empty value.
#[test]
fn lib_qa_003_deny_job_check_rejects_log_level_spellings() {
    let (ci, step, job) = deny_fixture();
    let after_step = |lines: &[&str]| insert_after(&ci, &step, 2, lines);
    let upper_case = after_step(&["with: { LOG-LEVEL: \"warn --exclude-dev\" }"]);
    assert_eq!(
        deny_input_values(&upper_case, &job, "LOG-LEVEL"),
        ["warn --exclude-dev"],
        "LIB-QA-003: the mutation `LOG-LEVEL: \"warn --exclude-dev\"` must pass `LOG-LEVEL: warn --exclude-dev`"
    );
    let command = msrv_command(&workspace_msrv());
    let msrv_job = job_with_step(&ci, |s| {
        get(s, "run").and_then(Yaml::as_str) == Some(command.as_str())
    });
    let anchored = insert_before(
        &ci,
        &format!("{msrv_job}:"),
        &[
            "anchor-holder:",
            "  if: false",
            "  runs-on: ubuntu-latest",
            "  steps:",
            "    - run: echo skipped",
            "      env: &exclude_dev",
            "        log-level: \"warn --exclude-dev\"",
        ],
    );
    let aliased = insert_after(&anchored, &step, 2, &["with: *exclude_dev"]);
    assert_eq!(
        deny_input_values(&aliased, &job, "log-level"),
        ["warn --exclude-dev"],
        "LIB-QA-003: the mutation `with: *exclude_dev` must make GitHub pass `log-level: warn --exclude-dev`"
    );
    let empty = after_step(&["with: { log-level: \"\" }"]);
    assert_eq!(
        deny_input_values(&empty, &job, "log-level"),
        [""],
        "LIB-QA-003: the mutation `log-level: \"\"` must pass an empty `log-level`"
    );
    let mutations = [
        mutation(
            "LOG-LEVEL: \"warn --exclude-dev\" (input name in upper case)",
            upper_case,
            DENY_INPUT,
        ),
        mutation(
            "with: *exclude_dev, an alias of a mapping carrying log-level: \"warn --exclude-dev\"",
            aliased,
            DENY_INPUT,
        ),
        mutation("log-level: \"\"", empty, DENY_INPUT),
    ];
    assert_yaml_mutations_reported("LIB-QA-003", &mutations, check_deny_job);
}

/// Y3-2 and design condition 5: nothing in YAML form can make the cargo-deny
/// job or step conditional or allowed to fail, or merge narrowing inputs in.
#[test]
fn lib_qa_003_deny_job_check_rejects_yaml_guard_bypasses() {
    let (ci, step, job) = deny_fixture();
    let job_line = format!("{job}:");
    let gated = gated_before_msrv(&ci);
    let after_step = |text: &str, lines: &[&str]| insert_after(text, &step, 2, lines);
    let mutations = [
        mutation(
            "needs: a skipped job",
            insert_after(&gated, &job_line, 2, &["needs: gate"]),
            JOB_KEY,
        ),
        mutation(
            "job-level <<: *anchor of a skipped job",
            insert_after(&gated, &job_line, 2, &["<<: *skip_job"]),
            &["job-key", "merge-key"],
        ),
        mutation(
            "step-level <<: *anchor of a skipped step",
            after_step(&gated, &["<<: *skip_step"]),
            &["step-key", "merge-key"],
        ),
        mutation(
            "with: { <<: *anchor } merging a narrowing command",
            after_step(&gated, &["with: { <<: *bans }"]),
            &["merge-key"],
        ),
        mutation(
            "with: block <<: *anchor merging a narrowing command",
            after_step(&gated, &["with:", "  <<: *bans"]),
            &["merge-key"],
        ),
        mutation(
            "shell on another step of the job",
            insert_before(&ci, &step, &["- run: echo before", "  shell: \"true {0}\""]),
            &["step-guard"],
        ),
        mutation(
            "double-quoted \"continue-on-error\" key on the step",
            after_step(&ci, &["\"continue-on-error\": true"]),
            &["step-key", "step-guard"],
        ),
        mutation(
            "env on the cargo-deny step",
            after_step(&ci, &["env:", "  INPUT_COMMAND: check bans"]),
            &["step-key"],
        ),
    ];
    assert_yaml_mutations_reported("LIB-QA-003", &mutations, check_deny_job);
}

#[test]
fn lib_qa_003_deny_job_check_reports_parse_and_shape_errors() {
    let (ci, step, job) = deny_fixture();
    assert_unparsable_reported(
        "LIB-QA-003",
        &unparsable_mutations(&ci, &job),
        check_deny_job,
    );
    assert_yaml_mutations_reported("LIB-QA-003", &shape_mutations(&ci), check_deny_job);
    // Y3-3: `with` on the cargo-deny step itself is not a mapping.
    let after_step = |text: &str, lines: &[&str]| insert_after(text, &step, 2, lines);
    let mutations = [
        mutation(
            "with given as a string",
            after_step(&ci, &["with: check bans"]),
            SHAPE,
        ),
        mutation(
            "with given as a sequence",
            after_step(&ci, &["with: [command, check bans]"]),
            SHAPE,
        ),
        mutation("with given as null", after_step(&ci, &["with:"]), SHAPE),
    ];
    assert_yaml_mutations_reported("LIB-QA-003", &mutations, check_deny_job);
}

#[test]
fn lib_qa_003_deny_job_check_accepts_equivalent_inputs() {
    let (ci, step, job) = deny_fixture();
    let after_step = |lines: &[&str]| insert_after(&ci, &step, 2, lines);
    let flow_check = after_step(&["with: { command: check }"]);
    assert_eq!(
        deny_commands(&flow_check, &job),
        ["check"],
        "LIB-QA-003: `with: {{ command: check }}` must pass `command: check`"
    );
    // Round 3 F1: `off` is a YAML 1.1 boolean but a YAML 1.2 string, and
    // GitHub passes it as the string `off`; it must stay a string here.
    let block_off = after_step(&["with:", "  log-level: off"]);
    let flow_off = after_step(&["with: { log-level: off }"]);
    for (name, text) in [("block", &block_off), ("flow", &flow_off)] {
        assert_eq!(
            deny_input_values(text, &job, "log-level"),
            ["off"],
            "LIB-QA-003: {name} `log-level: off` must be read as the string `off`"
        );
    }
    let controls = [
        ("the real ci.yml", ci.clone()),
        ("with: { command: check }", flow_check),
        (
            "block command: check",
            after_step(&["with:", "  command: check"]),
        ),
        (
            "block command: \"check\"",
            after_step(&["with:", "  command: \"check\""]),
        ),
        (
            "flow command: \"check\"",
            after_step(&["with: { command: \"check\" }"]),
        ),
        (
            "flow command: 'check'",
            after_step(&["with: { command: 'check' }"]),
        ),
        (
            "block arguments: --all-features",
            after_step(&["with:", "  arguments: --all-features"]),
        ),
        (
            "flow arguments: --all-features",
            after_step(&["with: { arguments: --all-features }"]),
        ),
        (
            "arguments: --all-features --workspace",
            after_step(&["with: { command: check, arguments: \"--all-features --workspace\" }"]),
        ),
        (
            "manifest-path: ./Cargo.toml",
            after_step(&["with: { manifest-path: ./Cargo.toml }"]),
        ),
        (
            "manifest-path: Cargo.toml",
            after_step(&["with:", "  manifest-path: Cargo.toml"]),
        ),
        ("with: {}", after_step(&["with: {}"])),
        (
            "log-level: warn, with command: check",
            after_step(&["with: { command: check, log-level: warn }"]),
        ),
        (
            "block log-level: warn",
            after_step(&["with:", "  log-level: warn"]),
        ),
        (
            "log-level: \"info\"",
            after_step(&["with: { log-level: \"info\" }"]),
        ),
        (
            "log-level: trace",
            after_step(&["with:", "  log-level: trace"]),
        ),
        ("block log-level: off", block_off),
        ("flow log-level: off", flow_off),
        (
            "an input passed quoted (rust-version)",
            after_step(&["with: { rust-version: \"1.85.0\" }"]),
        ),
        (
            "allowed step keys name, id",
            after_step(&["name: cargo-deny", "id: deny"]),
        ),
        (
            "a trailing comment on the cargo-deny step",
            replace_line(&ci, &step, &format!("{step} # the dependency policy")),
        ),
    ];
    assert_yaml_accepted("LIB-QA-003", &controls, check_deny_job);
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
// LIB-QA-007: mutation testing with cargo-mutants
// ---------------------------------------------------------------------------
//
// The specification is docs/verification/0003-m0-mutation-testing-prescription.md
// (P1 to P7, with the maintainer decisions D2, D10 and D14) and the row's
// criteria. As for LIB-QA-002/003, each check is a function over the
// configuration text that returns `[code] text` problems.
//
// The configuration did not exist when these tests were written. So each test
// first shows that a *reference fixture* passes; the fixtures below are
// exactly the shape the configuration is to have. Then it shows that every
// prescribed mutation of the fixture is reported with its code, and that every
// control is accepted. Last, it shows that the real file passes. Until the
// configuration exists, that last step fails with the code of what is missing
// (P7).

/// D11: the one command that installs cargo-mutants, identical in `ci.yml`,
/// `mutants.yml` and `CONTRIBUTING.md`.
const MUTANTS_INSTALL: &str = "cargo install cargo-mutants --locked --version 27.1.0";

/// D1 and D4: the PR job's command.
const MUTANTS_PR_COMMAND: &str = "cargo mutants --workspace --in-diff mutants.diff";

/// D3: the PR job's diff command, run with `BASE_REF` set to
/// [`BASE_REF_VALUE`].
const MUTANTS_DIFF_COMMAND: &str = r#"git diff "origin/${BASE_REF}...HEAD" > mutants.diff"#;

/// D3: the value of `BASE_REF` on the diff step.
const BASE_REF_VALUE: &str = "${{ github.base_ref }}";

/// D2: the PR job's only condition (bare or inside `${{ }}`).
const MUTANTS_PR_CONDITION: &str = "github.event_name == 'pull_request'";

/// D10: the full-run command, up to the shard count N.
const FULL_RUN_COMMAND_PREFIX: &str = "cargo mutants --workspace --shard ${{ matrix.shard }}/";

/// D10: the full-run command of the reference fixture (4 shards).
const FULL_RUN_COMMAND: &str = "cargo mutants --workspace --shard ${{ matrix.shard }}/4";

/// D10: the paths whose change runs the full run on a pull request.
const FULL_RUN_PATHS: [&str; 6] = [
    ".cargo/mutants.toml",
    ".github/workflows/mutants.yml",
    "crates/**",
    "xtask/**",
    "Cargo.toml",
    "Cargo.lock",
];

const CI_WORKFLOW: &str = ".github/workflows/ci.yml";
const FULL_RUN_WORKFLOW: &str = ".github/workflows/mutants.yml";
const MUTANTS_CONFIG: &str = ".cargo/mutants.toml";
const MUTANTS_TRIAGE: &str = "docs/quality/mutants-triage.md";
const MATRIX_TOML: &str = "docs/matrix/functionality-matrix.toml";

/// D12: the `.gitignore` rule for cargo-mutants' output.
const MUTANTS_GITIGNORE_RULE: &str = "/mutants.out*";

/// The reference shape of the `mutants` job in `ci.yml` (P1; D1 to D9 and
/// D11). It sits in a minimal workflow that has the real file's triggers,
/// permissions and environment, plus one other job for mutations to use. The
/// `mutants` job is what `ci.yml` is to contain. `timeout-minutes` is a
/// placeholder, to be sized from the measured run (D16).
const CI_MUTANTS_FIXTURE: &str = r#"name: CI

on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: 0

jobs:
  test:
    runs-on: ubuntu-24.04
    steps:
      - run: cargo test -p xtask

  mutants:
    name: mutants (PR diff)
    if: github.event_name == 'pull_request'
    runs-on: ubuntu-latest
    timeout-minutes: 60
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo install cargo-mutants --locked --version 27.1.0
      - name: Write the pull request's diff
        env:
          BASE_REF: ${{ github.base_ref }}
        run: git diff "origin/${BASE_REF}...HEAD" > mutants.diff
      - run: cargo mutants --workspace --in-diff mutants.diff
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: mutants.out
          path: mutants.out
          if-no-files-found: warn
"#;

/// The reference `on:` block of [`CI_MUTANTS_FIXTURE`] (the real `ci.yml`'s).
const CI_ON: &str = "on:\n  push:\n    branches: [main]\n  pull_request:\n  workflow_dispatch:\n";

// Lines (trimmed) of `CI_MUTANTS_FIXTURE` that mutations edit.
const PR_STEP: &str = "- run: cargo mutants --workspace --in-diff mutants.diff";
const INSTALL_STEP: &str = "- run: cargo install cargo-mutants --locked --version 27.1.0";
const DIFF_STEP: &str = "- name: Write the pull request's diff";
const DIFF_RUN: &str = r#"run: git diff "origin/${BASE_REF}...HEAD" > mutants.diff"#;
const BASE_REF_LINE: &str = "BASE_REF: ${{ github.base_ref }}";
const CHECKOUT_STEP: &str = "- uses: actions/checkout@v7";
const TOOLCHAIN_STEP: &str = "- uses: dtolnay/rust-toolchain@stable";
const UPLOAD_STEP: &str = "- uses: actions/upload-artifact@v4";
const PR_JOB_IF: &str = "if: github.event_name == 'pull_request'";
const OTHER_STEP: &str = "- run: cargo test -p xtask";

/// The reference shape of `.github/workflows/mutants.yml` (P2; D1, D5, D10,
/// D11): weekly, on demand, and on pull requests that change the D10 paths,
/// over 4 shards. `timeout-minutes` is a placeholder, to be sized from the
/// measured run (D16).
const FULL_RUN_FIXTURE: &str = r#"name: mutants

on:
  schedule:
    - cron: "0 3 * * 1"
  workflow_dispatch:
  pull_request:
    paths:
      - .cargo/mutants.toml
      - .github/workflows/mutants.yml
      - "crates/**"
      - "xtask/**"
      - Cargo.toml
      - Cargo.lock

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: 0

jobs:
  mutants:
    name: mutants (shard ${{ matrix.shard }})
    runs-on: ubuntu-latest
    timeout-minutes: 60
    strategy:
      fail-fast: false
      matrix:
        shard: [0, 1, 2, 3]
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo install cargo-mutants --locked --version 27.1.0
      - run: cargo mutants --workspace --shard ${{ matrix.shard }}/4
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: mutants-shard-${{ matrix.shard }}
          path: mutants.out
          if-no-files-found: error
"#;

// Lines and blocks of `FULL_RUN_FIXTURE` that mutations edit.
const FULL_STEP: &str = "- run: cargo mutants --workspace --shard ${{ matrix.shard }}/4";
const FULL_SHARDS: &str = "shard: [0, 1, 2, 3]";
const FULL_CRON: &str = r#"- cron: "0 3 * * 1""#;
const FULL_ARTIFACT_NAME: &str = "name: mutants-shard-${{ matrix.shard }}";
const FULL_PR_BLOCK: &str = "  pull_request:\n    paths:\n      - .cargo/mutants.toml\n      - .github/workflows/mutants.yml\n      - \"crates/**\"\n      - \"xtask/**\"\n      - Cargo.toml\n      - Cargo.lock\n";
const FULL_PATH_LINES: [&str; 6] = [
    "- .cargo/mutants.toml",
    "- .github/workflows/mutants.yml",
    "- \"crates/**\"",
    "- \"xtask/**\"",
    "- Cargo.toml",
    "- Cargo.lock",
];
const FULL_STRATEGY_BLOCK: &str =
    "    strategy:\n      fail-fast: false\n      matrix:\n        shard: [0, 1, 2, 3]\n";

/// Which workflow a mutants job belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MutantsRun {
    /// `ci.yml`'s `mutants` job, over the pull request's diff (P1).
    PullRequest,
    /// `mutants.yml`'s sharded run over the whole workspace (P2).
    Full,
}

/// The reviewed steps of a mutants job (P1.5): each is recognised by its
/// `uses` prefix or its exact `run`. Anything else is `Unknown`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MutantsStep {
    Checkout,
    Toolchain,
    Cache,
    Install,
    Diff,
    Mutants,
    Upload,
    Unknown,
}

const CHECKOUT_ACTION: &str = "actions/checkout@";
const TOOLCHAIN_ACTION: &str = "dtolnay/rust-toolchain@";
const CACHE_ACTION: &str = "Swatinem/rust-cache@";
const UPLOAD_ACTION: &str = "actions/upload-artifact@";

/// The keys the PR mutants job may have (P1.7): no `continue-on-error`,
/// `needs`, `strategy`, `container`, `services` or `defaults`.
const PR_JOB_KEYS: [&str; 7] = [
    "name",
    "runs-on",
    "steps",
    "env",
    "permissions",
    "if",
    "timeout-minutes",
];

/// The keys a full-run job may have (P2.7): as [`PR_JOB_KEYS`], with
/// `strategy` for the shards and no `if`.
const FULL_RUN_JOB_KEYS: [&str; 7] = [
    "name",
    "runs-on",
    "steps",
    "env",
    "permissions",
    "strategy",
    "timeout-minutes",
];

/// The keys of a reviewed step, by kind (P1.9).
const USES_STEP_KEYS: [&str; 5] = ["uses", "with", "name", "id", "env"];
const RUN_STEP_KEYS: [&str; 4] = ["run", "name", "id", "env"];
const UPLOAD_STEP_KEYS: [&str; 6] = ["uses", "with", "if", "name", "id", "env"];

/// Keys no step of a mutants job may have (P1.9), except the upload step's
/// `if`, whose value must be `always()` (P1.10).
const MUTANTS_STEP_GUARDS: [&str; 5] = [
    "if",
    "continue-on-error",
    "shell",
    "working-directory",
    "<<",
];

/// The environment variables a mutants job may set, at any level (P1.11).
const MUTANTS_ENV: [&str; 2] = ["CARGO_TERM_COLOR", "CARGO_INCREMENTAL"];

/// ... and the diff step, which also sets `BASE_REF` (D3).
const DIFF_STEP_ENV: [&str; 3] = ["CARGO_TERM_COLOR", "CARGO_INCREMENTAL", "BASE_REF"];

/// A GitHub expression's text: the inside of `${{ ... }}` if `value` is
/// wrapped in one, else `value`, trimmed.
fn expression(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_prefix("${{")
        .and_then(|inner| inner.strip_suffix("}}"))
        .map_or(value, str::trim)
}

/// The N of a full-run command `cargo mutants --workspace --shard ${{
/// matrix.shard }}/N`, if `command` is exactly that.
fn full_run_shard_count(command: &str) -> Option<usize> {
    let digits = command.strip_prefix(FULL_RUN_COMMAND_PREFIX)?;
    let count: usize = digits.parse().ok()?;
    (count.to_string() == digits).then_some(count)
}

fn mutants_step_kind(step: &Hash, run: MutantsRun) -> MutantsStep {
    let uses = get(step, "uses").and_then(Yaml::as_str);
    let command = get(step, "run").and_then(Yaml::as_str);
    let has_run = get(step, "run").is_some();
    let has_uses = get(step, "uses").is_some();
    match (uses, command) {
        (Some(action), None) if !has_run => {
            if action.starts_with(CHECKOUT_ACTION) {
                MutantsStep::Checkout
            } else if action.starts_with(TOOLCHAIN_ACTION) {
                MutantsStep::Toolchain
            } else if action.starts_with(CACHE_ACTION) {
                MutantsStep::Cache
            } else if action.starts_with(UPLOAD_ACTION) {
                MutantsStep::Upload
            } else {
                MutantsStep::Unknown
            }
        }
        (None, Some(command)) if !has_uses => match (command, run) {
            (MUTANTS_INSTALL, _) => MutantsStep::Install,
            (MUTANTS_DIFF_COMMAND, MutantsRun::PullRequest) => MutantsStep::Diff,
            (MUTANTS_PR_COMMAND, MutantsRun::PullRequest) => MutantsStep::Mutants,
            (command, MutantsRun::Full) if full_run_shard_count(command).is_some() => {
                MutantsStep::Mutants
            }
            _ => MutantsStep::Unknown,
        },
        _ => MutantsStep::Unknown,
    }
}

/// The steps of a mutants job: (number from 1, step, kind).
type MutantsSteps<'a> = [(usize, &'a Hash, MutantsStep)];

/// `[workflow-defaults]`: a top-level `defaults:` can change every step's shell.
fn workflow_defaults_problems(top: &Hash, problems: &mut Vec<String>) {
    if get(top, "defaults").is_some() {
        problems.push(problem(
            "workflow-defaults",
            "the workflow has a top-level `defaults:`, which can change the shell of every `run` step",
        ));
    }
}

/// `[mutants-env]`: every `env` key of `map` outside `allowed`, or an `env`
/// that is not a mapping (P1.11).
fn mutants_env_problems(map: &Hash, allowed: &[&str], what: &str, problems: &mut Vec<String>) {
    match get(map, "env") {
        None => {}
        Some(Yaml::Hash(env)) => {
            for key in env.keys() {
                if !key.as_str().is_some_and(|k| allowed.contains(&k)) {
                    problems.push(problem(
                        "mutants-env",
                        format!(
                            "{what} sets `{}`, which can change what cargo-mutants builds or reports; allowed: {allowed:?}",
                            show(key)
                        ),
                    ));
                }
            }
        }
        Some(other) => problems.push(problem(
            "mutants-env",
            format!("{what} has an `env` that is not a mapping: {other:?}"),
        )),
    }
}

/// The workflow's triggers as a mapping from event name to its filter.
/// `on: name` and `on: [a, b]` become mappings to null.
fn triggers(top: &Hash) -> Hash {
    let mut out = Hash::new();
    match get(top, "on") {
        Some(Yaml::Hash(map)) => return map.clone(),
        Some(Yaml::String(name)) => {
            out.insert(Yaml::String(name.clone()), Yaml::Null);
        }
        Some(Yaml::Array(items)) => {
            for name in items.iter().filter_map(Yaml::as_str) {
                out.insert(Yaml::String(name.to_owned()), Yaml::Null);
            }
        }
        _ => {}
    }
    out
}

/// `[pr-trigger]` (P1.13): `ci.yml` runs on every pull request, with no
/// branch, path or type filter.
fn pr_trigger_problems(top: &Hash, problems: &mut Vec<String>) {
    let on = triggers(top);
    match get(&on, "pull_request") {
        Some(Yaml::Null) => {}
        Some(Yaml::Hash(filters)) => {
            for key in filters.keys() {
                problems.push(problem(
                    "pr-trigger",
                    format!(
                        "`on.pull_request` has the filter `{}`; the mutants job must run on every pull request",
                        show(key)
                    ),
                ));
            }
        }
        None => problems.push(problem(
            "pr-trigger",
            "the workflow does not run on `pull_request`",
        )),
        Some(other) => problems.push(problem(
            "pr-trigger",
            format!("`on.pull_request` must be empty, found {other:?}"),
        )),
    }
}

/// A weekly cron (P2.2): five fields, a single minute and hour, `*` for the
/// day of the month and the month, and a single day of the week from 0 to 6.
fn is_weekly_cron(cron: &str) -> bool {
    let fields: Vec<&str> = cron.split_whitespace().collect();
    let number_below = |field: &str, limit: u32| {
        field.chars().all(|c| c.is_ascii_digit()) && field.parse::<u32>().is_ok_and(|n| n < limit)
    };
    matches!(
        fields.as_slice(),
        [minute, hour, "*", "*", day]
            if number_below(minute, 60) && number_below(hour, 24) && day.len() == 1 && number_below(day, 7)
    )
}

/// P2.2 to P2.4: the full run's schedule, manual dispatch and pull-request
/// trigger.
fn full_run_trigger_problems(top: &Hash, problems: &mut Vec<String>) {
    let on = triggers(top);
    let weekly = match get(&on, "schedule") {
        Some(Yaml::Array(entries)) if entries.len() == 1 => {
            entries[0].as_hash().is_some_and(|entry| {
                entry.len() == 1
                    && get(entry, "cron")
                        .and_then(Yaml::as_str)
                        .is_some_and(is_weekly_cron)
            })
        }
        _ => false,
    };
    require(
        problems,
        weekly,
        "full-run-schedule",
        "`on.schedule` must be exactly one weekly `cron` (one day of the week, 0 to 6)",
    );
    require(
        problems,
        matches!(
            get(&on, "workflow_dispatch"),
            Some(Yaml::Null | Yaml::Hash(_))
        ),
        "full-run-dispatch",
        "the full run must be startable by hand (`on.workflow_dispatch`)",
    );
    let Some(Yaml::Hash(filters)) = get(&on, "pull_request") else {
        problems.push(problem(
            "full-run-pr-trigger",
            format!("the full run must run on pull requests that change any of {FULL_RUN_PATHS:?}"),
        ));
        return;
    };
    key_allowlist(
        filters,
        &["paths"],
        "full-run-pr-trigger",
        "the full run's `on.pull_request`",
        problems,
    );
    let paths: Vec<&str> = match get(filters, "paths") {
        Some(Yaml::Array(items)) => items.iter().filter_map(Yaml::as_str).collect(),
        _ => Vec::new(),
    };
    for path in FULL_RUN_PATHS {
        if !paths.contains(&path) {
            problems.push(problem(
                "full-run-pr-trigger",
                format!("the full run's `on.pull_request.paths` must include `{path}` (D10)"),
            ));
        }
    }
    for path in paths.iter().filter(|path| path.starts_with('!')) {
        problems.push(problem(
            "full-run-pr-trigger",
            format!("the full run's `on.pull_request.paths` excludes `{path}`"),
        ));
    }
}

/// `permissions: contents: read` and nothing else (P2.11).
fn is_read_only_permissions(node: Option<&Yaml>) -> bool {
    node.and_then(Yaml::as_hash).is_some_and(|permissions| {
        permissions.len() == 1
            && get(permissions, "contents").and_then(Yaml::as_str) == Some("read")
    })
}

/// P1.7 and P2.6: the job's condition (PR job) or its shards (full run).
fn pr_condition_problems(job: &Job<'_>, label: &str, problems: &mut Vec<String>) {
    let condition = get(job.map, "if");
    if condition
        .and_then(Yaml::as_str)
        .is_none_or(|c| expression(c) != MUTANTS_PR_CONDITION)
    {
        problems.push(problem(
            "mutants-job-if",
            format!(
                "{label}'s only condition must be exactly `if: {MUTANTS_PR_CONDITION}`, found {}",
                condition.map_or_else(|| "none".to_owned(), show)
            ),
        ));
    }
}

fn shard_index(node: &Yaml) -> Option<i64> {
    match node {
        Yaml::Integer(n) => Some(*n),
        Yaml::String(text) => text.parse::<i64>().ok().filter(|n| n.to_string() == *text),
        _ => None,
    }
}

/// `[full-run-shards]` (P2.6): `fail-fast: false`, `matrix.shard` exactly
/// 0..N-1 with no other matrix key, and every command's denominator N.
fn shard_problems(
    job: &Job<'_>,
    steps: &MutantsSteps<'_>,
    label: &str,
    problems: &mut Vec<String>,
) {
    let code = "full-run-shards";
    let Some(strategy) = get(job.map, "strategy").and_then(Yaml::as_hash) else {
        problems.push(problem(code, format!("{label} has no `strategy` mapping")));
        return;
    };
    key_allowlist(
        strategy,
        &["fail-fast", "matrix"],
        code,
        &format!("{label}'s strategy"),
        problems,
    );
    require(
        problems,
        get(strategy, "fail-fast") == Some(&Yaml::Boolean(false)),
        code,
        &format!("{label} must set `fail-fast: false`, so every shard reports"),
    );
    let Some(matrix) = get(strategy, "matrix").and_then(Yaml::as_hash) else {
        problems.push(problem(
            code,
            format!("{label}'s `strategy.matrix` is not a mapping"),
        ));
        return;
    };
    key_allowlist(
        matrix,
        &["shard"],
        code,
        &format!("{label}'s matrix"),
        problems,
    );
    let shards: Option<Vec<i64>> = get(matrix, "shard")
        .and_then(Yaml::as_vec)
        .and_then(|items| items.iter().map(shard_index).collect());
    let Some(mut shards) = shards else {
        problems.push(problem(
            code,
            format!("{label}'s `matrix.shard` must be a sequence of shard indices"),
        ));
        return;
    };
    shards.sort_unstable();
    let count = shards.len();
    let expected = 0..i64::try_from(count).unwrap_or(i64::MAX);
    if count == 0 || shards.iter().copied().ne(expected) {
        problems.push(problem(
            code,
            format!("{label}'s `matrix.shard` must be exactly 0..N-1, found {shards:?}"),
        ));
    }
    for &(number, step, kind) in steps {
        if kind != MutantsStep::Mutants {
            continue;
        }
        let denominator = get(step, "run")
            .and_then(Yaml::as_str)
            .and_then(full_run_shard_count);
        if denominator != Some(count) {
            problems.push(problem(
                code,
                format!("{label}, step {number}: the shard denominator {denominator:?} must equal the {count} shards"),
            ));
        }
    }
}

/// `[mutants-runner]` and `[mutants-timeout]` (P1.8, D5, D17).
fn runner_and_timeout_problems(job: &Job<'_>, label: &str, problems: &mut Vec<String>) {
    let runner = get(job.map, "runs-on");
    if !runner
        .and_then(Yaml::as_str)
        .is_some_and(|r| r.starts_with("ubuntu-") && !r.contains("${{"))
    {
        problems.push(problem(
            "mutants-runner",
            format!(
                "{label} must run on an ubuntu runner (D17), found {}",
                runner.map_or_else(|| "none".to_owned(), show)
            ),
        ));
    }
    if !matches!(get(job.map, "timeout-minutes"), Some(Yaml::Integer(n)) if *n > 0) {
        problems.push(problem(
            "mutants-timeout",
            format!("{label} must set `timeout-minutes` to a positive number (D5)"),
        ));
    }
}

fn step_keys(kind: MutantsStep) -> &'static [&'static str] {
    match kind {
        MutantsStep::Checkout | MutantsStep::Toolchain | MutantsStep::Cache => &USES_STEP_KEYS,
        MutantsStep::Install | MutantsStep::Diff | MutantsStep::Mutants => &RUN_STEP_KEYS,
        MutantsStep::Upload => &UPLOAD_STEP_KEYS,
        MutantsStep::Unknown => &[],
    }
}

/// `[mutants-step-unknown]`, `[step-key]` and `[mutants-env]` on every step
/// (P1.5, P1.9, P1.11).
fn step_problems(steps: &MutantsSteps<'_>, label: &str, problems: &mut Vec<String>) {
    for &(number, step, kind) in steps {
        let what = format!("{label}, step {number}");
        if kind == MutantsStep::Unknown {
            let shown = get(step, "run")
                .or_else(|| get(step, "uses"))
                .map_or_else(|| format!("{step:?}"), show);
            problems.push(problem(
                "mutants-step-unknown",
                format!("{what} (`{shown}`) is none of checkout, toolchain, cache, install, diff, mutants and upload"),
            ));
        } else {
            key_allowlist(step, step_keys(kind), "step-key", &what, problems);
        }
        let env: &[&str] = if kind == MutantsStep::Diff {
            &DIFF_STEP_ENV
        } else {
            &MUTANTS_ENV
        };
        mutants_env_problems(step, env, &what, problems);
    }
}

/// `[mutants-install]` (P1.4): the exact install, before the mutants step,
/// and no other install of anything.
fn install_problems(
    steps: &MutantsSteps<'_>,
    first: usize,
    label: &str,
    problems: &mut Vec<String>,
) {
    let code = "mutants-install";
    match steps.iter().position(|s| s.2 == MutantsStep::Install) {
        None => problems.push(problem(
            code,
            format!("{label} has no step whose `run` is exactly `{MUTANTS_INSTALL}`"),
        )),
        Some(at) if at > first => problems.push(problem(
            code,
            format!("{label} installs cargo-mutants after the step that runs it"),
        )),
        Some(_) => {}
    }
    for &(number, step, kind) in steps {
        let text = |key: &str| get(step, key).and_then(Yaml::as_str).unwrap_or_default();
        if kind != MutantsStep::Install
            && (text("run").contains("install") || text("uses").contains("install"))
        {
            problems.push(problem(
                code,
                format!("{label}, step {number} installs something other than exactly `{MUTANTS_INSTALL}`"),
            ));
        }
    }
}

/// `[mutants-checkout]` (P1.6): the PR job checks out with exactly
/// `fetch-depth: 0`; a full-run job with no input but `fetch-depth`.
fn checkout_problems(
    steps: &MutantsSteps<'_>,
    run: MutantsRun,
    label: &str,
    problems: &mut Vec<String>,
) {
    let code = "mutants-checkout";
    let checkouts: Vec<&(usize, &Hash, MutantsStep)> = steps
        .iter()
        .filter(|s| s.2 == MutantsStep::Checkout)
        .collect();
    if checkouts.is_empty() {
        problems.push(problem(
            code,
            format!("{label} has no `{CHECKOUT_ACTION}` step"),
        ));
    }
    for &&(number, step, _) in &checkouts {
        let with = get(step, "with");
        let ok = match run {
            MutantsRun::PullRequest => with.and_then(Yaml::as_hash).is_some_and(|with| {
                with.len() == 1
                    && match get(with, "fetch-depth") {
                        Some(Yaml::Integer(depth)) => *depth == 0,
                        Some(Yaml::String(depth)) => depth == "0",
                        _ => false,
                    }
            }),
            MutantsRun::Full => with.is_none_or(|with| {
                with.as_hash()
                    .is_some_and(|with| with.keys().all(|k| k.as_str() == Some("fetch-depth")))
            }),
        };
        if !ok {
            problems.push(problem(
                code,
                format!("{label}, step {number}: the checkout's `with` must be exactly `fetch-depth: 0` (PR job) or have no input but `fetch-depth` (full run), found {with:?}"),
            ));
        }
    }
}

/// `[mutants-diff]` and `[mutants-step-order]` (P1.3): the exact diff step,
/// with `BASE_REF` from `github.base_ref`, before the mutants step.
fn diff_problems(steps: &MutantsSteps<'_>, first: usize, label: &str, problems: &mut Vec<String>) {
    let diffs: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, s)| s.2 == MutantsStep::Diff)
        .map(|(at, _)| at)
        .collect();
    if diffs.is_empty() {
        problems.push(problem(
            "mutants-diff",
            format!("{label} has no step whose `run` is exactly `{MUTANTS_DIFF_COMMAND}`"),
        ));
        return;
    }
    for &at in &diffs {
        let (number, step, _) = steps[at];
        let base_ref = get(step, "env")
            .and_then(Yaml::as_hash)
            .and_then(|env| get(env, "BASE_REF"))
            .and_then(Yaml::as_str)
            .map(str::trim);
        let ok = base_ref.is_some_and(|value| {
            value.starts_with("${{")
                && value.ends_with("}}")
                && expression(value) == "github.base_ref"
        });
        if !ok {
            problems.push(problem(
                "mutants-diff",
                format!("{label}, step {number} must set `BASE_REF: {BASE_REF_VALUE}`, found {base_ref:?}"),
            ));
        }
    }
    if !diffs.iter().any(|&at| at < first) || diffs.iter().any(|&at| at > first) {
        problems.push(problem(
            "mutants-step-order",
            format!("{label} must write `mutants.diff` before the mutants step, and not after it"),
        ));
    }
}

/// `[mutants-artifact]` (P1.10, P1.12) or `[full-run-artifact]` (P2.9): the
/// upload of `mutants.out`, after the mutants step, with `if: always()`.
fn artifact_problems(
    steps: &MutantsSteps<'_>,
    first: usize,
    run: MutantsRun,
    label: &str,
    problems: &mut Vec<String>,
) {
    let code = match run {
        MutantsRun::PullRequest => "mutants-artifact",
        MutantsRun::Full => "full-run-artifact",
    };
    let uploads: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, s)| s.2 == MutantsStep::Upload)
        .map(|(at, _)| at)
        .collect();
    if uploads.is_empty() {
        problems.push(problem(
            code,
            format!("{label} does not upload `mutants.out`"),
        ));
    }
    for &at in &uploads {
        let (number, step, _) = steps[at];
        let what = format!("{label}, upload step {number}");
        let with = get(step, "with").and_then(Yaml::as_hash);
        let input = |name: &str| with.and_then(|with| get(with, name));
        let no_files = input("if-no-files-found");
        let mut fault =
            |ok: bool, text: &str| require(problems, ok, code, &format!("{what} {text}"));
        fault(at > first, "must come after the mutants step");
        fault(
            get(step, "if")
                .and_then(Yaml::as_str)
                .is_some_and(|c| expression(c) == "always()"),
            "must have `if: always()`",
        );
        fault(
            get(step, "continue-on-error").is_none(),
            "must not have `continue-on-error`",
        );
        fault(
            input("path").and_then(Yaml::as_str) == Some("mutants.out"),
            "must upload `path: mutants.out`",
        );
        match run {
            MutantsRun::PullRequest => fault(
                no_files.is_none_or(|v| matches!(v.as_str(), Some("ignore" | "warn"))),
                "must have `if-no-files-found` absent, `ignore` or `warn` (D9)",
            ),
            MutantsRun::Full => {
                fault(
                    no_files.and_then(Yaml::as_str) == Some("error"),
                    "must have `if-no-files-found: error` (D10)",
                );
                fault(
                    input("name")
                        .and_then(Yaml::as_str)
                        .is_some_and(|name| name.contains("${{") && name.contains("matrix.shard")),
                    "must name the artifact for its shard (`${{ matrix.shard }}`)",
                );
            }
        }
    }
}

/// Check `job` if it is a mutants job (it has a step classified
/// [`MutantsStep::Mutants`]); return whether it is.
fn mutants_job_problems(job: &Job<'_>, run: MutantsRun, problems: &mut Vec<String>) -> bool {
    let steps: Vec<(usize, &Hash, MutantsStep)> = job
        .steps
        .iter()
        .map(|&(number, step)| (number, step, mutants_step_kind(step, run)))
        .collect();
    let Some(first) = steps.iter().position(|s| s.2 == MutantsStep::Mutants) else {
        return false;
    };
    let (label, job_keys): (String, &[&str]) = match run {
        MutantsRun::PullRequest => (format!("the mutants job `{}`", job.id), &PR_JOB_KEYS),
        MutantsRun::Full => (format!("the full-run job `{}`", job.id), &FULL_RUN_JOB_KEYS),
    };
    key_allowlist(job.map, job_keys, "job-key", &label, problems);
    match run {
        MutantsRun::PullRequest => pr_condition_problems(job, &label, problems),
        MutantsRun::Full => {
            shard_problems(job, &steps, &label, problems);
            if let Some(permissions) = get(job.map, "permissions") {
                require(
                    problems,
                    is_read_only_permissions(Some(permissions)),
                    "full-run-permissions",
                    &format!("{label}'s `permissions` must be exactly `contents: read`"),
                );
            }
        }
    }
    runner_and_timeout_problems(job, &label, problems);
    mutants_env_problems(job.map, &MUTANTS_ENV, &label, problems);
    guarded_step_problems(
        job,
        &MUTANTS_STEP_GUARDS,
        |step, key| key == "if" && mutants_step_kind(step, run) == MutantsStep::Upload,
        problems,
    );
    step_problems(&steps, &label, problems);
    install_problems(&steps, first, &label, problems);
    checkout_problems(&steps, run, &label, problems);
    if run == MutantsRun::PullRequest {
        diff_problems(&steps, first, &label, problems);
    }
    artifact_problems(&steps, first, run, &label, problems);
    true
}

/// LIB-QA-007 P1: `ci.yml`'s `mutants` job. The job is found by a step whose
/// `run` is exactly [`MUTANTS_PR_COMMAND`]; every such job is checked.
fn check_mutants_pr_job(ci: &str) -> Vec<String> {
    let doc = match load_workflow(ci) {
        Ok(doc) => doc,
        Err(problem) => return vec![problem],
    };
    let mut problems = Vec::new();
    merge_key_problems(&doc, "ci.yml", &mut problems);
    let Some(workflow) = read_workflow(&doc, &mut problems) else {
        return problems;
    };
    workflow_defaults_problems(workflow.top, &mut problems);
    mutants_env_problems(workflow.top, &MUTANTS_ENV, "the workflow", &mut problems);
    pr_trigger_problems(workflow.top, &mut problems);
    let mut found = false;
    for job in &workflow.jobs {
        found |= mutants_job_problems(job, MutantsRun::PullRequest, &mut problems);
    }
    if !found {
        problems.push(problem(
            "mutants-step-missing",
            format!("no CI step has a `run` value exactly `{MUTANTS_PR_COMMAND}`"),
        ));
    }
    problems
}

/// LIB-QA-007 P2: the full-run workflow `mutants.yml` (`None`: the file does
/// not exist). Its jobs are found by a step whose `run` is exactly
/// `cargo mutants --workspace --shard ${{ matrix.shard }}/N`.
fn check_mutants_full_run(workflow: Option<&str>) -> Vec<String> {
    let Some(text) = workflow else {
        return vec![problem(
            "full-run-missing",
            format!("{FULL_RUN_WORKFLOW} does not exist"),
        )];
    };
    let doc = match load_workflow(text) {
        Ok(doc) => doc,
        Err(problem) => return vec![problem],
    };
    let mut problems = Vec::new();
    merge_key_problems(&doc, "mutants.yml", &mut problems);
    let Some(workflow) = read_workflow(&doc, &mut problems) else {
        return problems;
    };
    workflow_defaults_problems(workflow.top, &mut problems);
    mutants_env_problems(workflow.top, &MUTANTS_ENV, "the workflow", &mut problems);
    full_run_trigger_problems(workflow.top, &mut problems);
    require(
        &mut problems,
        is_read_only_permissions(get(workflow.top, "permissions")),
        "full-run-permissions",
        "the full-run workflow must have top-level `permissions: contents: read`",
    );
    let mut found = false;
    for job in &workflow.jobs {
        found |= mutants_job_problems(job, MutantsRun::Full, &mut problems);
    }
    if !found {
        problems.push(problem(
            "full-run-step-missing",
            format!("no step of {FULL_RUN_WORKFLOW} has a `run` value exactly `{FULL_RUN_COMMAND_PREFIX}N`"),
        ));
    }
    problems
}
/// The number of exact [`MUTANTS_INSTALL`] commands in `line`: not followed
/// by more of a version or flag (`27.1.0.1`, `27.1.0-beta`).
fn exact_installs(line: &str) -> usize {
    line.match_indices(MUTANTS_INSTALL)
        .filter(|(at, _)| {
            !line[at + MUTANTS_INSTALL.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || ".-+_".contains(c))
        })
        .count()
}

/// LIB-QA-007 P3 (D11): the one install command appears exactly in `ci.yml`,
/// `mutants.yml` (`None`: absent) and `CONTRIBUTING.md`, and no line of them
/// installs cargo-mutants any other way.
fn check_install_versions(ci: &str, full_run: Option<&str>, contributing: &str) -> Vec<String> {
    let code = "mutants-version-mismatch";
    let mut problems = Vec::new();
    for (name, text) in [
        ("ci.yml", Some(ci)),
        ("mutants.yml", full_run),
        ("CONTRIBUTING.md", Some(contributing)),
    ] {
        let Some(text) = text else {
            problems.push(problem(
                code,
                format!("{name} does not exist, so it cannot give `{MUTANTS_INSTALL}`"),
            ));
            continue;
        };
        if !text.lines().any(|line| exact_installs(line) > 0) {
            problems.push(problem(
                code,
                format!("{name} does not contain `{MUTANTS_INSTALL}`"),
            ));
        }
        for (index, line) in text.lines().enumerate() {
            let installs = line.matches("cargo install").count() + line.matches("binstall").count();
            if line.contains("mutants") && installs > exact_installs(line) {
                problems.push(problem(
                    code,
                    format!("{name}:{} installs cargo-mutants other than with exactly `{MUTANTS_INSTALL}`: `{}`", index + 1, line.trim()),
                ));
            }
        }
    }
    problems
}

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

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
}

/// The text of `rel`, or `None` if it does not exist.
fn read_optional(rel: &str) -> Option<String> {
    match fs::read_to_string(repository_root().join(rel)) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!("read {rel}: {e}"),
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
/// `examine_globs` and `examine_re`, is `[unreviewed-key]`.
const REVIEWED_MUTANTS_KEYS: [&str; 12] = [
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
    for key in table.keys() {
        if !REVIEWED_MUTANTS_KEYS.contains(&key.as_str()) {
            problems.push(problem(
                "unreviewed-key",
                format!("{MUTANTS_CONFIG} has the key `{key}`, which has not been reviewed"),
            ));
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

/// The paragraphs (blank-line separated) of a Markdown text, each with its
/// whitespace collapsed and in lower case.
fn paragraphs(text: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    for line in text.lines() {
        if line.trim().is_empty() {
            out.push(String::new());
        } else if let Some(paragraph) = out.last_mut() {
            for word in line.split_whitespace() {
                if !paragraph.is_empty() {
                    paragraph.push(' ');
                }
                paragraph.push_str(&word.to_lowercase());
            }
        }
    }
    out.retain(|p| !p.is_empty());
    out
}

/// Whether one paragraph of `text` contains every one of `words` (lower case).
fn has_paragraph_with(text: &str, words: &[&str]) -> bool {
    paragraphs(text)
        .iter()
        .any(|p| words.iter().all(|w| p.contains(w)))
}

/// LIB-QA-007 P6 (criterion 6): `CONTRIBUTING.md` gives the install command,
/// a whole-workspace and an `--in-diff` run, that `mutants.out` is
/// git-ignored, and that the authoritative run is on Linux.
fn check_contributing_mutants(text: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut need = |ok: bool, what: &str| {
        require(
            &mut problems,
            ok,
            "doc-contributing",
            &format!("CONTRIBUTING.md must {what}"),
        );
    };
    need(
        text.contains(MUTANTS_INSTALL),
        &format!("give the exact install command `{MUTANTS_INSTALL}`"),
    );
    need(
        text.contains("`cargo mutants --workspace`")
            || text
                .lines()
                .any(|l| l.trim() == "cargo mutants --workspace"),
        "show the whole-workspace run, `cargo mutants --workspace`",
    );
    need(text.contains("--in-diff"), "show a local `--in-diff` run");
    need(
        has_paragraph_with(text, &["mutants.out", "ignored"]),
        "say that `mutants.out` is git-ignored",
    );
    need(
        has_paragraph_with(
            text,
            &["authoritative", "linux", "windows", "macos", "missed"],
        ),
        "say that Linux-only tests make some mutants show as missed on Windows and macOS, so the authoritative run is on Linux",
    );
    problems
}

/// The titles of the verifier's ten required checks, none of which may be
/// lost (P6).
const VERIFIER_CHECK_TITLES: [&str; 10] = [
    "**Gates.**",
    "**Traceability.**",
    "**Test adequacy.**",
    "**Negative coverage.**",
    "**Losslessness.**",
    "**XMI.**",
    "**Editing.**",
    "**API boundary.**",
    "**Strict vs tolerant.**",
    "**Regression.**",
];

/// LIB-QA-007 P6 (criterion 6): `sacm-verifier.md` requires the Linux report
/// for the commit under verification from the draft PR, names its missed,
/// timeout and unviable lists, keeps hand-reasoned criterion mutations,
/// writes local runs under `target/` and bans `--in-place`. Fix round 1 adds:
/// exclusions are re-derived and the excluded set is compared with
/// `--no-config` (F1); no PASS without a Linux report, and a Windows run is
/// no substitute (F2); hand-reasoned mutations only in a scratch copy under
/// `target/`, never the working tree (F3).
fn check_verifier_mutants(text: &str) -> Vec<String> {
    let flat = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mut problems = Vec::new();
    let mut need = |ok: bool, what: &str| {
        require(
            &mut problems,
            ok,
            "doc-verifier",
            &format!("sacm-verifier.md must {what}"),
        );
    };
    need(
        has_paragraph_with(text, &["linux", "commit under verification", "draft"]),
        "require the Linux report for the commit under verification, from the slice's draft PR",
    );
    need(
        has_paragraph_with(text, &["missed", "timeout", "unviable"]),
        "name the missed, timeout and unviable lists",
    );
    need(
        text.contains("--output target/"),
        "write local runs with `--output target/...`",
    );
    need(
        has_paragraph_with(text, &["--in-place", "never"]),
        "ban `--in-place`",
    );
    need(
        flat.contains("in addition to hand-reasoned criterion mutations"),
        "keep \"in addition to hand-reasoned criterion mutations\"",
    );
    need(
        !flat.contains("once it is adopted"),
        "not keep the stale \"cargo mutants --file ... once it is adopted\" sentence",
    );
    need(
        has_paragraph_with(
            text,
            &[
                "exclusion",
                "mutants.toml",
                "triage",
                "re-derive",
                "--no-config",
            ],
        ),
        "say, in one paragraph, that the verifier re-derives every exclusion a slice adds or changes (the `.cargo/mutants.toml` diff and the triage rows) and compares `--list` with `--list --no-config` (fix F1)",
    );
    need(
        has_paragraph_with(text, &["no linux report", "may not pass", "windows"]),
        "say, in one paragraph, that with no Linux report for the commit under verification test adequacy is not shown, so a mutated row may not PASS, and a local Windows run is no substitute (fix F2)",
    );
    need(
        has_paragraph_with(
            text,
            &[
                "hand-reasoned mutation",
                "scratch copy under",
                "target/",
                "never the working tree",
            ],
        ),
        "say, in one paragraph, that a hand-reasoned mutation is applied only in a scratch copy under `target/`, never the working tree (fix F3)",
    );
    for title in VERIFIER_CHECK_TITLES {
        need(
            text.contains(title),
            &format!("keep the required check {title}"),
        );
    }
    problems
}

/// LIB-QA-007 P6 (criterion 6): the `sacm-slice` skill's step 7 says, before
/// the verifier runs, how the Linux report for the commit under verification
/// is obtained (the draft PR's `mutants` job and `mutants.yml` shards), and
/// that under `mode=local` no mutated row may PASS without one (fix F2).
fn check_skill_mutants(text: &str) -> Vec<String> {
    let code = "doc-skill";
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|l| l.starts_with("## 7.")) else {
        return vec![problem(code, "SKILL.md has no `## 7.` (verification) step")];
    };
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.starts_with("## "))
        .map_or(lines.len(), |offset| start + 1 + offset);
    let Some(verifier) = lines[start..end]
        .iter()
        .position(|l| l.contains("**sacm-verifier**"))
    else {
        return vec![problem(
            code,
            "SKILL.md's step 7 does not run **sacm-verifier**",
        )];
    };
    let before = lines[start..start + verifier].join("\n");
    let mut problems = Vec::new();
    require(
        &mut problems,
        has_paragraph_with(
            &before,
            &[
                "draft",
                "commit under verification",
                "mutants.out",
                "mutants.yml",
            ],
        ),
        code,
        "SKILL.md's step 7 must say, before the verifier runs, that the draft PR's CI gives the Linux report for the commit under verification (`mutants.out`, `mutants.yml`)",
    );
    require(
        &mut problems,
        has_paragraph_with(&before, &["mode=local", "linux report", "may not pass"]),
        code,
        "SKILL.md's step 7 must say, before the verifier runs, that under `mode=local` no row cargo-mutants mutates may PASS until a Linux report for that commit exists (fix F2)",
    );
    problems
}

// Mutation helpers for steps and blocks (LIB-QA-007).

/// The lines of `text` and the line range of the sequence item (a step)
/// whose first line, trimmed, is `first`: that line and every following line
/// indented deeper than its `-`. Panics unless `first` occurs exactly once.
fn step_range<'a>(text: &'a str, first: &str) -> (Vec<&'a str>, std::ops::Range<usize>) {
    let lines: Vec<&str> = text.lines().collect();
    let hits: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].trim() == first)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "mutation target `{first}` must occur exactly once"
    );
    let start = hits[0];
    let marker = indent(lines[start]);
    let end = lines[start + 1..]
        .iter()
        .position(|l| !l.trim().is_empty() && indent(l) <= marker)
        .map_or(lines.len(), |offset| start + 1 + offset);
    (lines, start..end)
}

fn joined(lines: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    lines.into_iter().fold(String::new(), |mut out, line| {
        out.push_str(line.as_ref());
        out.push('\n');
        out
    })
}

/// `text` with the step starting `first` replaced by `new`, each line
/// indented like the step's first line.
fn replace_step(text: &str, first: &str, new: &[&str]) -> String {
    let (lines, range) = step_range(text, first);
    let pad = leading(lines[range.start]);
    let replaced = new.iter().map(|line| format!("{pad}{line}"));
    joined(
        lines[..range.start]
            .iter()
            .map(|l| (*l).to_owned())
            .chain(replaced)
            .chain(lines[range.end..].iter().map(|l| (*l).to_owned())),
    )
}

fn delete_step(text: &str, first: &str) -> String {
    replace_step(text, first, &[])
}

/// `text` with the step starting `first` moved to just after the step
/// starting `after` (which may be in another job).
fn move_step_after(text: &str, first: &str, after: &str) -> String {
    let (lines, range) = step_range(text, first);
    let block: Vec<&str> = lines[range].to_vec();
    let without = delete_step(text, first);
    let (lines, range) = step_range(&without, after);
    joined(
        lines[..range.end]
            .iter()
            .chain(block.iter())
            .chain(lines[range.end..].iter()),
    )
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
const QA7: &str = "LIB-QA-007";

/// Problem codes LIB-QA-007 mutations are expected to produce.
const MUTANTS_MISSING: &[&str] = &["mutants-step-missing"];
const MUTANTS_DIFF: &[&str] = &["mutants-diff"];
const MUTANTS_ORDER: &[&str] = &["mutants-step-order"];
const MUTANTS_INSTALL_CODE: &[&str] = &["mutants-install"];
const MUTANTS_UNKNOWN: &[&str] = &["mutants-step-unknown"];
const MUTANTS_CHECKOUT: &[&str] = &["mutants-checkout"];
const MUTANTS_JOB_IF: &[&str] = &["mutants-job-if"];
const MUTANTS_RUNNER: &[&str] = &["mutants-runner"];
const MUTANTS_TIMEOUT: &[&str] = &["mutants-timeout"];
const MUTANTS_ARTIFACT: &[&str] = &["mutants-artifact"];
const MUTANTS_ENV_CODE: &[&str] = &["mutants-env"];
const PR_TRIGGER: &[&str] = &["pr-trigger"];
const WORKFLOW_DEFAULTS: &[&str] = &["workflow-defaults"];
const JOB_KEY_MERGE: &[&str] = &["job-key", "merge-key"];
const STEP_KEY_GUARD_MERGE: &[&str] = &["step-key", "step-guard", "merge-key"];

/// P1.2: flags that change what cargo-mutants tests or how it reports, each
/// of which makes the command inexact.
const ADDED_MUTANTS_FLAGS: [&str; 19] = [
    "--check",
    "--list",
    "--list-files",
    "--no-config",
    "--config x.toml",
    "-e xtask/src/matrix.rs",
    "--exclude xtask/src/matrix.rs",
    "-E matrix",
    "--exclude-re matrix",
    "-f xtask/src/lib.rs",
    "--file xtask/src/lib.rs",
    "-F matrix",
    "--re matrix",
    "--skip-calls with_capacity",
    "--shard 0/2",
    "--baseline=skip",
    "--iterate",
    "--test-tool nextest",
    "--cap-lints false",
];

/// P1.2: endings that let the step pass when cargo-mutants fails (D4).
const FAILURE_MASKS: [&str; 4] = [" || true", " || exit 0", "; true", " || [ $? -eq 3 ]"];

/// The real `ci.yml` must pass P1. Before the `mutants` job exists this fails
/// with `[mutants-step-missing]` (P7).
fn assert_real_ci_mutants_job() {
    let problems = check_mutants_pr_job(&read(CI_WORKFLOW));
    assert!(
        problems.is_empty(),
        "LIB-QA-007: {CI_WORKFLOW} must run the pinned `mutants` job on every pull request: {problems:#?}"
    );
}

/// P1 applied three ways: the reference job passes, every mutation is
/// reported with its codes, and then the real `ci.yml` passes.
fn assert_pr_job_mutations(mutations: &[Mutation]) {
    let problems = check_mutants_pr_job(CI_MUTANTS_FIXTURE);
    assert!(
        problems.is_empty(),
        "LIB-QA-007: the reference mutants job must pass before its mutations are meaningful: {problems:#?}"
    );
    assert_yaml_mutations_reported(QA7, mutations, check_mutants_pr_job);
    assert_real_ci_mutants_job();
}

/// `text` with the mutants step's `run` replaced by `command`.
fn with_pr_command(text: &str, command: &str) -> String {
    replace_line(text, PR_STEP, &format!("- run: {command}"))
}

/// P1.1: the mutants step is missing.
fn pr_step_missing_mutations() -> Vec<Mutation> {
    let ci = CI_MUTANTS_FIXTURE;
    let indented = format!("    {MUTANTS_PR_COMMAND}");
    vec![
        mutation("job deleted", delete_job(ci, "mutants"), MUTANTS_MISSING),
        mutation(
            "step commented out",
            comment_line(ci, PR_STEP),
            MUTANTS_MISSING,
        ),
        mutation(
            "job deleted, command left in a comment",
            delete_job(ci, "mutants") + &format!("      # {PR_STEP}\n"),
            MUTANTS_MISSING,
        ),
        mutation(
            "command inside another job's run: |, mutants job deleted",
            delete_job(
                &replace_lines(
                    ci,
                    OTHER_STEP,
                    &["- run: |", "    cargo test -p xtask", &indented],
                ),
                "mutants",
            ),
            MUTANTS_MISSING,
        ),
    ]
}

/// P1.1 and P1.2.
fn pr_command_mutations() -> Vec<Mutation> {
    let ci = CI_MUTANTS_FIXTURE;
    let indented = format!("    {MUTANTS_PR_COMMAND}");
    let command = |c: &str| with_pr_command(ci, c);
    let mut mutations = pr_step_missing_mutations();
    mutations.extend([
        mutation(
            "--in-diff mutants.diff dropped",
            command("cargo mutants --workspace"),
            MUTANTS_MISSING,
        ),
        mutation(
            "--workspace dropped",
            command("cargo mutants --in-diff mutants.diff"),
            MUTANTS_MISSING,
        ),
        mutation(
            "--workspace replaced by -p sacm -p sacm-cli",
            command("cargo mutants -p sacm -p sacm-cli --in-diff mutants.diff"),
            MUTANTS_MISSING,
        ),
        mutation(
            "set +e in a run: |",
            replace_lines(ci, PR_STEP, &["- run: |", "    set +e", &indented]),
            MUTANTS_MISSING,
        ),
        mutation(
            "continuation line || true",
            replace_lines(ci, PR_STEP, &[PR_STEP, "    || true"]),
            MUTANTS_MISSING,
        ),
        mutation(
            "run: >- folding in --check",
            replace_lines(ci, PR_STEP, &["- run: >-", &indented, "    --check"]),
            MUTANTS_MISSING,
        ),
        mutation(
            "heredoc that only prints the command",
            replace_lines(
                ci,
                PR_STEP,
                &["- run: |", "    cat <<'EOF'", &indented, "    EOF"],
            ),
            MUTANTS_MISSING,
        ),
        mutation(
            "echo of the command",
            command(&format!("echo \"{MUTANTS_PR_COMMAND}\"")),
            MUTANTS_MISSING,
        ),
        mutation(
            "alias of another step's command",
            replace_line(
                &replace_line(ci, OTHER_STEP, "- run: &xtask cargo test -p xtask"),
                PR_STEP,
                "- run: *xtask",
            ),
            MUTANTS_MISSING,
        ),
        mutation(
            "--in-diff /dev/null",
            command("cargo mutants --workspace --in-diff /dev/null"),
            MUTANTS_MISSING,
        ),
        mutation(
            "--in-diff of a file the diff step does not write",
            command("cargo mutants --workspace --in-diff pr.diff"),
            MUTANTS_MISSING,
        ),
        mutation(
            "a flag before --in-diff",
            command("cargo mutants --workspace --check --in-diff mutants.diff"),
            MUTANTS_MISSING,
        ),
    ]);
    for mask in FAILURE_MASKS {
        mutations.push(mutation(
            format!("`{mask}` appended"),
            command(&format!("{MUTANTS_PR_COMMAND}{mask}")),
            MUTANTS_MISSING,
        ));
    }
    for flag in ADDED_MUTANTS_FLAGS {
        mutations.push(mutation(
            format!("`{flag}` added"),
            command(&format!("{MUTANTS_PR_COMMAND} {flag}")),
            MUTANTS_MISSING,
        ));
    }
    mutations
}

#[test]
fn lib_qa_007_ci_mutants_job_is_pinned() {
    assert_pr_job_mutations(&[]);
}

#[test]
fn lib_qa_007_mutants_job_check_rejects_missing_or_inexact_command() {
    let mutations = pr_command_mutations();
    // What GitHub runs for the YAML-syntax mutations: never the exact command.
    for (name, runs) in [
        (
            "continuation line || true",
            format!("{MUTANTS_PR_COMMAND} || true"),
        ),
        (
            "run: >- folding in --check",
            format!("{MUTANTS_PR_COMMAND} --check"),
        ),
        (
            "set +e in a run: |",
            format!("set +e\n{MUTANTS_PR_COMMAND}\n"),
        ),
    ] {
        let m = mutations
            .iter()
            .find(|m| m.name == name)
            .expect("the mutation exists");
        assert!(
            step_values(&m.text, "mutants", "run").contains(&Yaml::String(runs.clone())),
            "LIB-QA-007: the mutation `{name}` must make GitHub run {runs:?}"
        );
    }
    assert_pr_job_mutations(&mutations);
}

/// P1.3.
fn pr_diff_mutations() -> Vec<Mutation> {
    let ci = CI_MUTANTS_FIXTURE;
    let diff = |command: &str| replace_line(ci, DIFF_RUN, &format!("run: {command}"));
    let base_ref = |line: &str| replace_line(ci, BASE_REF_LINE, line);
    vec![
        mutation(
            "diff step missing",
            delete_step(ci, DIFF_STEP),
            MUTANTS_DIFF,
        ),
        mutation(
            "HEAD...HEAD",
            diff(r#"git diff "HEAD...HEAD" > mutants.diff"#),
            MUTANTS_DIFF,
        ),
        mutation(
            "origin/main...origin/main",
            diff(r#"git diff "origin/main...origin/main" > mutants.diff"#),
            MUTANTS_DIFF,
        ),
        mutation(
            "two dots",
            diff(r#"git diff "origin/${BASE_REF}..HEAD" > mutants.diff"#),
            MUTANTS_DIFF,
        ),
        mutation(
            "--stat",
            diff(r#"git diff --stat "origin/${BASE_REF}...HEAD" > mutants.diff"#),
            MUTANTS_DIFF,
        ),
        mutation(
            "--name-only",
            diff(r#"git diff --name-only "origin/${BASE_REF}...HEAD" > mutants.diff"#),
            MUTANTS_DIFF,
        ),
        mutation(
            "a path filter",
            diff(r#"git diff "origin/${BASE_REF}...HEAD" -- crates > mutants.diff"#),
            MUTANTS_DIFF,
        ),
        mutation(
            "| tee",
            diff(r#"git diff "origin/${BASE_REF}...HEAD" | tee mutants.diff"#),
            MUTANTS_DIFF,
        ),
        mutation(
            "> mutants.diff || true",
            diff(r#"git diff "origin/${BASE_REF}...HEAD" > mutants.diff || true"#),
            MUTANTS_DIFF,
        ),
        mutation(
            ": > mutants.diff",
            diff(r#"": > mutants.diff""#),
            MUTANTS_DIFF,
        ),
        mutation(
            "touch mutants.diff",
            diff("touch mutants.diff"),
            MUTANTS_DIFF,
        ),
        mutation(
            "BASE_REF removed",
            replace_step(ci, DIFF_STEP, &[DIFF_STEP, &format!("  {DIFF_RUN}")]),
            MUTANTS_DIFF,
        ),
        mutation(
            "BASE_REF from github.head_ref",
            base_ref("BASE_REF: ${{ github.head_ref }}"),
            MUTANTS_DIFF,
        ),
        mutation(
            "BASE_REF the literal text github.base_ref",
            base_ref("BASE_REF: github.base_ref"),
            MUTANTS_DIFF,
        ),
        mutation(
            "diff step after the mutants step",
            move_step_after(ci, DIFF_STEP, PR_STEP),
            MUTANTS_ORDER,
        ),
        mutation(
            "diff step in another job",
            move_step_after(ci, DIFF_STEP, OTHER_STEP),
            MUTANTS_DIFF,
        ),
    ]
}

#[test]
fn lib_qa_007_mutants_job_check_rejects_diff_bypasses() {
    assert_pr_job_mutations(&pr_diff_mutations());
}

/// P1.4 on `text`, a workflow whose mutants step's first line is `step`.
fn install_mutations(text: &str, step: &str) -> Vec<Mutation> {
    let install = |command: &str| replace_line(text, INSTALL_STEP, &format!("- run: {command}"));
    let mut mutations: Vec<Mutation> = [
        (
            "--version dropped",
            "cargo install cargo-mutants --locked".to_owned(),
        ),
        (
            "^27.1.0",
            "cargo install cargo-mutants --locked --version ^27.1.0".to_owned(),
        ),
        (
            ">=27",
            "cargo install cargo-mutants --locked --version >=27".to_owned(),
        ),
        (
            "27",
            "cargo install cargo-mutants --locked --version 27".to_owned(),
        ),
        (
            "another version",
            "cargo install cargo-mutants --locked --version 27.2.0".to_owned(),
        ),
        (
            "--locked dropped",
            "cargo install cargo-mutants --version 27.1.0".to_owned(),
        ),
        (
            "--git",
            format!("{MUTANTS_INSTALL} --git https://github.com/sourcefrog/cargo-mutants"),
        ),
        (
            "--path",
            format!("{MUTANTS_INSTALL} --path ../cargo-mutants"),
        ),
        ("--registry", format!("{MUTANTS_INSTALL} --registry mirror")),
        (
            "--index",
            format!("{MUTANTS_INSTALL} --index sparse+https://example.com/index/"),
        ),
        (
            "cargo binstall",
            "cargo binstall --no-confirm cargo-mutants@27.1.0".to_owned(),
        ),
    ]
    .into_iter()
    .map(|(name, command)| mutation(name, install(&command), MUTANTS_INSTALL_CODE))
    .collect();
    mutations.extend([
        mutation(
            "install placed after the mutants step",
            move_step_after(text, INSTALL_STEP, step),
            MUTANTS_INSTALL_CODE,
        ),
        mutation(
            "a second, unpinned install before the mutants step",
            insert_after(
                text,
                INSTALL_STEP,
                0,
                &["- run: cargo install cargo-mutants --force"],
            ),
            MUTANTS_INSTALL_CODE,
        ),
        mutation(
            "a second, unpinned install after the mutants step",
            insert_after(
                text,
                step,
                0,
                &["- run: cargo install cargo-mutants --force"],
            ),
            MUTANTS_INSTALL_CODE,
        ),
    ]);
    mutations
}

/// P1.5 on `text`, a workflow whose mutants step's first line is `step`.
fn unknown_step_mutations(text: &str, step: &str) -> Vec<Mutation> {
    let before = |lines: &[&str]| insert_before(text, step, lines);
    vec![
        mutation(
            "a step writing $GITHUB_ENV",
            before(&[r#"- run: echo "RUSTFLAGS=--cfg mutants" >> "$GITHUB_ENV""#]),
            MUTANTS_UNKNOWN,
        ),
        mutation(
            "a step writing $GITHUB_PATH",
            before(&[r#"- run: echo "$HOME/bin" >> "$GITHUB_PATH""#]),
            MUTANTS_UNKNOWN,
        ),
        mutation(
            "a step rewriting the diff",
            before(&[r#"- run: ": > mutants.diff""#]),
            MUTANTS_UNKNOWN,
        ),
        mutation(
            "an install action",
            before(&[
                "- uses: taiki-e/install-action@v2",
                "  with:",
                "    tool: cargo-mutants",
            ]),
            &["mutants-step-unknown", "mutants-install"],
        ),
        mutation(
            "another action",
            before(&["- uses: actions/cache@v4", "  with:", "    path: target"]),
            MUTANTS_UNKNOWN,
        ),
        mutation(
            "a step with both uses and run",
            before(&["- uses: actions/checkout@v7", "  run: cargo build"]),
            MUTANTS_UNKNOWN,
        ),
    ]
}

#[test]
fn lib_qa_007_mutants_job_check_rejects_install_bypasses() {
    assert_pr_job_mutations(&install_mutations(CI_MUTANTS_FIXTURE, PR_STEP));
}

#[test]
fn lib_qa_007_mutants_job_check_rejects_unknown_steps() {
    assert_pr_job_mutations(&unknown_step_mutations(CI_MUTANTS_FIXTURE, PR_STEP));
}

#[test]
fn lib_qa_007_mutants_job_check_rejects_checkout_inputs() {
    let ci = CI_MUTANTS_FIXTURE;
    let input = |line: &str| insert_after(ci, "fetch-depth: 0", 0, &[line]);
    assert_pr_job_mutations(&[
        mutation("ref added", input("ref: main"), MUTANTS_CHECKOUT),
        mutation(
            "repository added",
            input("repository: someone/libSACM"),
            MUTANTS_CHECKOUT,
        ),
        mutation("path added", input("path: elsewhere"), MUTANTS_CHECKOUT),
        mutation(
            "fetch-depth: 1",
            replace_line(ci, "fetch-depth: 0", "fetch-depth: 1"),
            MUTANTS_CHECKOUT,
        ),
        mutation(
            "with removed",
            replace_step(ci, CHECKOUT_STEP, &[CHECKOUT_STEP]),
            MUTANTS_CHECKOUT,
        ),
        mutation(
            "checkout removed",
            delete_step(ci, CHECKOUT_STEP),
            MUTANTS_CHECKOUT,
        ),
    ]);
}

/// P1.7 and P1.8 (and D5's `timeout-minutes`) on `text`, whose mutants job
/// is `mutants`. `condition` mutations are the PR job's only.
fn job_key_mutations(text: &str) -> Vec<Mutation> {
    let job = |lines: &[&str]| insert_after(text, "mutants:", 2, lines);
    let gated = with_gate_job(text, "mutants");
    let runner = |r: &str| replace_line(text, "runs-on: ubuntu-latest", &format!("runs-on: {r}"));
    vec![
        mutation(
            "continue-on-error: true",
            job(&["continue-on-error: true"]),
            JOB_KEY,
        ),
        mutation(
            "continue-on-error: ${{ true }}",
            job(&["continue-on-error: ${{ true }}"]),
            JOB_KEY,
        ),
        mutation(
            "needs: a skipped job",
            insert_after(&gated, "mutants:", 2, &["needs: gate"]),
            JOB_KEY,
        ),
        mutation("container", job(&["container: rust:1.85"]), JOB_KEY),
        mutation(
            "services",
            job(&["services:", "  db:", "    image: postgres"]),
            JOB_KEY,
        ),
        mutation(
            "defaults.run.shell",
            job(&["defaults:", "  run:", "    shell: \"true {0}\""]),
            JOB_KEY,
        ),
        mutation(
            "defaults.run.working-directory",
            job(&["defaults:", "  run:", "    working-directory: crates/sacm"]),
            JOB_KEY,
        ),
        mutation(
            "<<: *anchor of a skipped job",
            insert_after(&gated, "mutants:", 2, &["<<: *skip_job"]),
            JOB_KEY_MERGE,
        ),
        mutation(
            "runs-on: windows-latest",
            runner("windows-latest"),
            MUTANTS_RUNNER,
        ),
        mutation(
            "runs-on: macos-latest",
            runner("macos-latest"),
            MUTANTS_RUNNER,
        ),
        mutation(
            "runs-on: self-hosted",
            runner("self-hosted"),
            MUTANTS_RUNNER,
        ),
        mutation(
            "runs-on: ${{ matrix.os }}",
            runner("${{ matrix.os }}"),
            MUTANTS_RUNNER,
        ),
        mutation(
            "runs-on: [self-hosted, linux]",
            runner("[self-hosted, linux]"),
            MUTANTS_RUNNER,
        ),
        mutation(
            "timeout-minutes removed",
            delete_line(text, "timeout-minutes: 60"),
            MUTANTS_TIMEOUT,
        ),
        mutation(
            "timeout-minutes: 0",
            replace_line(text, "timeout-minutes: 60", "timeout-minutes: 0"),
            MUTANTS_TIMEOUT,
        ),
    ]
}

#[test]
fn lib_qa_007_mutants_job_check_rejects_job_condition_and_key_bypasses() {
    let ci = CI_MUTANTS_FIXTURE;
    let condition = |c: &str| replace_line(ci, PR_JOB_IF, &format!("if: {c}"));
    let mut mutations = vec![
        mutation("if: false", condition("false"), MUTANTS_JOB_IF),
        mutation(
            "if: ${{ false }}",
            condition("${{ false }}"),
            MUTANTS_JOB_IF,
        ),
        mutation(
            "== 'push'",
            condition("github.event_name == 'push'"),
            MUTANTS_JOB_IF,
        ),
        mutation(
            "== 'pull_request' && false",
            condition("github.event_name == 'pull_request' && false"),
            MUTANTS_JOB_IF,
        ),
        mutation(
            "== 'pull_request' || false",
            condition("github.event_name == 'pull_request' || false"),
            MUTANTS_JOB_IF,
        ),
        mutation(
            "${{ }} with trailing text",
            condition("${{ github.event_name == 'pull_request' }} && ${{ false }}"),
            MUTANTS_JOB_IF,
        ),
        mutation("if removed", delete_line(ci, PR_JOB_IF), MUTANTS_JOB_IF),
        // `strategy` is the PR job's only: the full-run job needs one (P2.6).
        mutation(
            "strategy",
            insert_after(
                ci,
                "mutants:",
                2,
                &["strategy:", "  matrix:", "    os: [ubuntu-latest]"],
            ),
            JOB_KEY,
        ),
    ];
    mutations.extend(job_key_mutations(ci));
    assert_pr_job_mutations(&mutations);
}

/// P1.9 on `text`, whose mutants step's first line is `step`.
fn step_guard_mutations(text: &str, step: &str) -> Vec<Mutation> {
    let after = |target: &str, lines: &[&str]| insert_after(text, target, 2, lines);
    let gated = with_gate_job(text, "mutants");
    vec![
        mutation(
            "if: false on the mutants step",
            after(step, &["if: false"]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "continue-on-error on the mutants step",
            after(step, &["continue-on-error: true"]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "shell on the mutants step",
            after(step, &["shell: \"true {0}\""]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "working-directory on the mutants step",
            after(step, &["working-directory: crates/sacm"]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "<<: *anchor of a skipped step on the mutants step",
            insert_after(&gated, step, 2, &["<<: *skip_step"]),
            STEP_KEY_GUARD_MERGE,
        ),
        mutation(
            "if: false on the install step",
            after(INSTALL_STEP, &["if: false"]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "continue-on-error on the checkout step",
            after(CHECKOUT_STEP, &["continue-on-error: true"]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "if: false on the toolchain step",
            after(TOOLCHAIN_STEP, &["if: false"]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "double-quoted \"if\" key on the mutants step",
            after(step, &["\"if\": false"]),
            STEP_KEY_AND_GUARD,
        ),
    ]
}

#[test]
fn lib_qa_007_mutants_job_check_rejects_step_guards() {
    let ci = CI_MUTANTS_FIXTURE;
    let mut mutations = step_guard_mutations(ci, PR_STEP);
    mutations.extend([
        mutation(
            "shell on the diff step",
            insert_after(ci, DIFF_STEP, 2, &["shell: \"true {0}\""]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "working-directory on the diff step",
            insert_after(ci, DIFF_STEP, 2, &["working-directory: crates"]),
            STEP_KEY_AND_GUARD,
        ),
    ]);
    assert_pr_job_mutations(&mutations);
}

#[test]
fn lib_qa_007_mutants_job_check_rejects_artifact_bypasses() {
    let ci = CI_MUTANTS_FIXTURE;
    let condition = |c: &str| replace_line(ci, "if: always()", &format!("if: {c}"));
    assert_pr_job_mutations(&[
        mutation(
            "upload if removed",
            delete_line(ci, "if: always()"),
            MUTANTS_ARTIFACT,
        ),
        mutation("if: failure()", condition("failure()"), MUTANTS_ARTIFACT),
        mutation("if: success()", condition("success()"), MUTANTS_ARTIFACT),
        mutation(
            "if: ${{ !cancelled() }}",
            condition("${{ !cancelled() }}"),
            MUTANTS_ARTIFACT,
        ),
        mutation("if: false", condition("false"), MUTANTS_ARTIFACT),
        mutation(
            "upload removed",
            delete_step(ci, UPLOAD_STEP),
            MUTANTS_ARTIFACT,
        ),
        mutation(
            "path not mutants.out",
            replace_line(ci, "path: mutants.out", "path: target/mutants.out"),
            MUTANTS_ARTIFACT,
        ),
        mutation(
            "path removed",
            delete_line(ci, "path: mutants.out"),
            MUTANTS_ARTIFACT,
        ),
        mutation(
            "continue-on-error on the upload",
            insert_after(ci, UPLOAD_STEP, 2, &["continue-on-error: true"]),
            &["mutants-artifact", "step-guard", "step-key"],
        ),
        mutation(
            "if-no-files-found: error",
            replace_line(ci, "if-no-files-found: warn", "if-no-files-found: error"),
            MUTANTS_ARTIFACT,
        ),
        mutation(
            "upload before the mutants step",
            move_step_after(ci, UPLOAD_STEP, INSTALL_STEP),
            MUTANTS_ARTIFACT,
        ),
    ]);
}

/// P1.11 on `text`, whose mutants step's first line is `step` and whose
/// install step is [`INSTALL_STEP`].
fn env_mutations(text: &str, step: &str) -> Vec<Mutation> {
    let job = |lines: &[&str]| insert_after(text, "mutants:", 2, lines);
    let on_step = |target: &str, variable: &str| insert_after(text, target, 2, &["env:", variable]);
    vec![
        mutation(
            "job-level RUSTFLAGS",
            job(&["env:", "  RUSTFLAGS: --cfg mutants"]),
            MUTANTS_ENV_CODE,
        ),
        mutation(
            "workflow-level RUSTFLAGS",
            insert_after(text, "CARGO_INCREMENTAL: 0", 0, &["RUSTFLAGS: -D warnings"]),
            MUTANTS_ENV_CODE,
        ),
        mutation(
            "CARGO_ENCODED_RUSTFLAGS on the mutants step",
            on_step(step, "  CARGO_ENCODED_RUSTFLAGS: --cfg=mutants"),
            MUTANTS_ENV_CODE,
        ),
        mutation(
            "CARGO_BUILD_RUSTFLAGS on the install step",
            on_step(INSTALL_STEP, "  CARGO_BUILD_RUSTFLAGS: --cfg=mutants"),
            MUTANTS_ENV_CODE,
        ),
        mutation(
            "job-level CARGO_MUTANTS_OUTPUT",
            job(&["env:", "  CARGO_MUTANTS_OUTPUT: elsewhere"]),
            MUTANTS_ENV_CODE,
        ),
        mutation(
            "CARGO_MUTANTS_JOBS on the mutants step",
            on_step(step, "  CARGO_MUTANTS_JOBS: \"1\""),
            MUTANTS_ENV_CODE,
        ),
        mutation(
            "BASE_REF on the mutants step",
            on_step(step, &format!("  BASE_REF: {BASE_REF_VALUE}")),
            MUTANTS_ENV_CODE,
        ),
        mutation(
            "job-level BASE_REF",
            job(&["env:", &format!("  BASE_REF: {BASE_REF_VALUE}")]),
            MUTANTS_ENV_CODE,
        ),
        mutation(
            "job-level env from an expression",
            job(&["env: ${{ fromJSON(vars.MUTANTS_ENV) }}"]),
            MUTANTS_ENV_CODE,
        ),
    ]
}

#[test]
fn lib_qa_007_mutants_job_check_rejects_env_overrides() {
    let ci = CI_MUTANTS_FIXTURE;
    let mut mutations = env_mutations(ci, PR_STEP);
    mutations.push(mutation(
        "another variable on the diff step",
        insert_after(ci, BASE_REF_LINE, 0, &["GIT_DIR: elsewhere"]),
        MUTANTS_ENV_CODE,
    ));
    assert_pr_job_mutations(&mutations);
}

#[test]
fn lib_qa_007_mutants_job_check_rejects_pr_trigger_filters() {
    let ci = CI_MUTANTS_FIXTURE;
    let filter = |line: &str| insert_after(ci, "pull_request:", 2, &[line]);
    assert_pr_job_mutations(&[
        mutation(
            "on.pull_request removed",
            delete_line(ci, "pull_request:"),
            PR_TRIGGER,
        ),
        mutation(
            "on: [push]",
            replace_once(ci, CI_ON, "on: [push]\n"),
            PR_TRIGGER,
        ),
        mutation("paths", filter("paths: [\"crates/**\"]"), PR_TRIGGER),
        mutation(
            "paths-ignore",
            filter("paths-ignore: [\"docs/**\"]"),
            PR_TRIGGER,
        ),
        mutation("branches", filter("branches: [main]"), PR_TRIGGER),
        mutation(
            "branches-ignore",
            filter("branches-ignore: [main]"),
            PR_TRIGGER,
        ),
        mutation("types: [closed]", filter("types: [closed]"), PR_TRIGGER),
        mutation(
            "workflow-level defaults.run.shell",
            insert_before(
                ci,
                "jobs:",
                &["defaults:", "  run:", "    shell: \"true {0}\""],
            ),
            WORKFLOW_DEFAULTS,
        ),
    ]);
}

#[test]
fn lib_qa_007_mutants_job_check_reports_parse_and_shape_errors() {
    let ci = CI_MUTANTS_FIXTURE;
    assert_unparsable_reported(
        QA7,
        &unparsable_mutations(ci, "mutants"),
        check_mutants_pr_job,
    );
    assert_pr_job_mutations(&shape_mutations(ci));
}

/// P1 controls: the same job written in other YAML forms.
fn pr_job_yaml_form_controls() -> Vec<(&'static str, String)> {
    let ci = CI_MUTANTS_FIXTURE;
    vec![
        ("the reference job", ci.to_owned()),
        (
            "double-quoted \"if\" key and value",
            replace_line(
                ci,
                PR_JOB_IF,
                r#""if": "github.event_name == 'pull_request'""#,
            ),
        ),
        (
            "${{ }} job condition",
            replace_line(
                ci,
                PR_JOB_IF,
                "if: ${{ github.event_name == 'pull_request' }}",
            ),
        ),
        (
            "single-quoted runs-on key and value",
            replace_line(ci, "runs-on: ubuntu-latest", "'runs-on': 'ubuntu-latest'"),
        ),
        (
            "single-quoted mutants run",
            with_pr_command(ci, &format!("'{MUTANTS_PR_COMMAND}'")),
        ),
        (
            "double-quoted install run",
            replace_line(
                ci,
                INSTALL_STEP,
                &format!("- \"run\": \"{MUTANTS_INSTALL}\""),
            ),
        ),
        (
            "single-quoted diff run",
            replace_line(ci, DIFF_RUN, &format!("run: '{MUTANTS_DIFF_COMMAND}'")),
        ),
        (
            "double-quoted BASE_REF",
            replace_line(ci, BASE_REF_LINE, "BASE_REF: \"${{ github.base_ref }}\""),
        ),
        (
            "flow-style checkout with",
            replace_step(
                ci,
                CHECKOUT_STEP,
                &[CHECKOUT_STEP, "  with: { fetch-depth: 0 }"],
            ),
        ),
        (
            "fetch-depth: \"0\"",
            replace_line(ci, "fetch-depth: 0", "fetch-depth: \"0\""),
        ),
        (
            "${{ always() }} on the upload",
            replace_line(ci, "if: always()", "if: ${{ always() }}"),
        ),
        (
            "on: [push, pull_request]",
            replace_once(ci, CI_ON, "on: [push, pull_request]\n"),
        ),
    ]
}

/// P1 controls: allowed additions and values.
fn pr_job_value_controls() -> Vec<(&'static str, String)> {
    let ci = CI_MUTANTS_FIXTURE;
    let job = |lines: &[&str]| insert_after(ci, "mutants:", 2, lines);
    vec![
        (
            "name and id on the mutants step",
            insert_after(
                ci,
                PR_STEP,
                2,
                &["name: cargo-mutants on the diff", "id: mutants"],
            ),
        ),
        (
            "name on the install step",
            insert_after(ci, INSTALL_STEP, 2, &["name: Install cargo-mutants"]),
        ),
        (
            "a trailing comment on the mutants step",
            replace_line(ci, PR_STEP, &format!("{PR_STEP} # D1, D4")),
        ),
        (
            "another job timeout-minutes",
            replace_line(ci, "timeout-minutes: 60", "timeout-minutes: 120"),
        ),
        (
            "if-no-files-found: ignore",
            replace_line(ci, "if-no-files-found: warn", "if-no-files-found: ignore"),
        ),
        (
            "if-no-files-found absent",
            delete_line(ci, "if-no-files-found: warn"),
        ),
        (
            "job-level CARGO_INCREMENTAL: \"1\" (D16)",
            job(&["env:", "  CARGO_INCREMENTAL: \"1\""]),
        ),
        (
            "job permissions contents: read",
            job(&["permissions:", "  contents: read"]),
        ),
        (
            "runs-on: ubuntu-24.04",
            replace_line(ci, "runs-on: ubuntu-latest", "runs-on: ubuntu-24.04"),
        ),
        (
            "a toolchain input",
            insert_after(
                ci,
                TOOLCHAIN_STEP,
                2,
                &["with:", "  components: llvm-tools"],
            ),
        ),
    ]
}

#[test]
fn lib_qa_007_mutants_job_check_accepts_equivalent_yaml() {
    assert_yaml_accepted(QA7, &pr_job_yaml_form_controls(), check_mutants_pr_job);
    assert_yaml_accepted(QA7, &pr_job_value_controls(), check_mutants_pr_job);
    assert_real_ci_mutants_job();
}

const FULL_SCHEDULE: &[&str] = &["full-run-schedule"];
const FULL_DISPATCH: &[&str] = &["full-run-dispatch"];
const FULL_PR_TRIGGER: &[&str] = &["full-run-pr-trigger"];
const FULL_MISSING: &[&str] = &["full-run-step-missing"];
const FULL_SHARDS_CODE: &[&str] = &["full-run-shards"];
const FULL_ARTIFACT: &[&str] = &["full-run-artifact"];
const FULL_PERMISSIONS: &[&str] = &["full-run-permissions"];

fn check_full_run_text(text: &str) -> Vec<String> {
    check_mutants_full_run(Some(text))
}

/// The real `mutants.yml` must pass P2. Before it exists this fails with
/// `[full-run-missing]` (P7).
fn assert_real_full_run() {
    let problems = check_mutants_full_run(read_optional(FULL_RUN_WORKFLOW).as_deref());
    assert!(
        problems.is_empty(),
        "LIB-QA-007: {FULL_RUN_WORKFLOW} must run the pinned sharded full run: {problems:#?}"
    );
}

/// P2 applied three ways: the reference workflow passes, every mutation is
/// reported with its codes, and then the real `mutants.yml` passes.
fn assert_full_run_mutations(mutations: &[Mutation]) {
    let problems = check_full_run_text(FULL_RUN_FIXTURE);
    assert!(
        problems.is_empty(),
        "LIB-QA-007: the reference full-run workflow must pass before its mutations are meaningful: {problems:#?}"
    );
    assert_yaml_mutations_reported(QA7, mutations, check_full_run_text);
    assert_real_full_run();
}

#[test]
fn lib_qa_007_full_run_workflow_is_pinned() {
    assert_full_run_mutations(&[]);
}

#[test]
fn lib_qa_007_full_run_check_reports_a_missing_workflow() {
    let problems = check_mutants_full_run(None);
    assert!(
        has_code(&problems, "full-run-missing"),
        "LIB-QA-007: a missing {FULL_RUN_WORKFLOW} must be reported as `[full-run-missing]`; it reported {problems:#?}"
    );
    assert_real_full_run();
}

/// P2.2 to P2.4.
fn full_run_trigger_mutations() -> Vec<Mutation> {
    let w = FULL_RUN_FIXTURE;
    let cron = |c: &str| replace_line(w, FULL_CRON, &format!("- cron: \"{c}\""));
    let pr = |line: &str| insert_after(w, "pull_request:", 2, &[line]);
    let mut mutations = vec![
        mutation(
            "schedule removed",
            delete_line(&delete_line(w, FULL_CRON), "schedule:"),
            FULL_SCHEDULE,
        ),
        mutation(
            "schedule empty",
            replace_line(&delete_line(w, FULL_CRON), "schedule:", "schedule: []"),
            FULL_SCHEDULE,
        ),
        mutation(
            "an entry with no cron",
            replace_line(w, FULL_CRON, "- timezone: UTC"),
            FULL_SCHEDULE,
        ),
        mutation(
            "a cron with another key",
            insert_after(w, FULL_CRON, 2, &["timezone: UTC"]),
            FULL_SCHEDULE,
        ),
        mutation("daily: 0 3 * * *", cron("0 3 * * *"), FULL_SCHEDULE),
        mutation("never: 0 0 30 2 *", cron("0 0 30 2 *"), FULL_SCHEDULE),
        mutation("weekdays: 0 3 * * 1-5", cron("0 3 * * 1-5"), FULL_SCHEDULE),
        mutation("two days: 0 3 * * 1,4", cron("0 3 * * 1,4"), FULL_SCHEDULE),
        mutation("day 7", cron("0 3 * * 7"), FULL_SCHEDULE),
        mutation("a named day", cron("0 3 * * MON"), FULL_SCHEDULE),
        mutation(
            "every five minutes on Mondays",
            cron("*/5 3 * * 1"),
            FULL_SCHEDULE,
        ),
        mutation("every hour on Mondays", cron("0 * * * 1"), FULL_SCHEDULE),
        mutation("four fields", cron("0 3 * *"), FULL_SCHEDULE),
        mutation("six fields", cron("0 3 * * 1 2026"), FULL_SCHEDULE),
        mutation(
            "two cron entries",
            insert_after(w, FULL_CRON, 0, &["- cron: \"0 4 * * 1\""]),
            FULL_SCHEDULE,
        ),
        mutation(
            "workflow_dispatch removed",
            delete_line(w, "workflow_dispatch:"),
            FULL_DISPATCH,
        ),
        mutation(
            "pull_request removed",
            replace_once(w, FULL_PR_BLOCK, ""),
            FULL_PR_TRIGGER,
        ),
        mutation("branches added", pr("branches: [main]"), FULL_PR_TRIGGER),
        mutation(
            "paths-ignore added",
            pr("paths-ignore: [\"docs/**\"]"),
            FULL_PR_TRIGGER,
        ),
        mutation(
            "types: [closed] added",
            pr("types: [closed]"),
            FULL_PR_TRIGGER,
        ),
        mutation(
            "a negated path",
            insert_after(w, "- Cargo.lock", 0, &["- \"!crates/**\""]),
            FULL_PR_TRIGGER,
        ),
    ];
    for line in FULL_PATH_LINES {
        mutations.push(mutation(
            format!("path `{line}` dropped"),
            delete_line(w, line),
            FULL_PR_TRIGGER,
        ));
    }
    mutations
}

#[test]
fn lib_qa_007_full_run_check_rejects_trigger_bypasses() {
    assert_full_run_mutations(&full_run_trigger_mutations());
}

#[test]
fn lib_qa_007_full_run_check_rejects_inexact_command() {
    let w = FULL_RUN_FIXTURE;
    let indented = format!("    {FULL_RUN_COMMAND}");
    let command = |c: &str| replace_line(w, FULL_STEP, &format!("- run: {c}"));
    let mut mutations = vec![
        mutation(
            "--workspace dropped",
            command("cargo mutants --shard ${{ matrix.shard }}/4"),
            FULL_MISSING,
        ),
        mutation(
            "-p sacm -p sacm-cli instead of --workspace",
            command("cargo mutants -p sacm -p sacm-cli --shard ${{ matrix.shard }}/4"),
            FULL_MISSING,
        ),
        mutation(
            "--in-diff added",
            command(&format!("{FULL_RUN_COMMAND} --in-diff mutants.diff")),
            FULL_MISSING,
        ),
        mutation(
            "--shard dropped",
            command("cargo mutants --workspace"),
            FULL_MISSING,
        ),
        mutation(
            "set +e in a run: |",
            replace_lines(w, FULL_STEP, &["- run: |", "    set +e", &indented]),
            FULL_MISSING,
        ),
        mutation(
            "continuation line || true",
            replace_lines(w, FULL_STEP, &[FULL_STEP, "    || true"]),
            FULL_MISSING,
        ),
        mutation(
            "heredoc that only prints the command",
            replace_lines(
                w,
                FULL_STEP,
                &["- run: |", "    cat <<'EOF'", &indented, "    EOF"],
            ),
            FULL_MISSING,
        ),
        mutation(
            "echo of the command",
            command(&format!("echo \"{FULL_RUN_COMMAND}\"")),
            FULL_MISSING,
        ),
    ];
    for mask in FAILURE_MASKS {
        mutations.push(mutation(
            format!("`{mask}` appended"),
            command(&format!("{FULL_RUN_COMMAND}{mask}")),
            FULL_MISSING,
        ));
    }
    for flag in ADDED_MUTANTS_FLAGS {
        mutations.push(mutation(
            format!("`{flag}` added"),
            command(&format!("{FULL_RUN_COMMAND} {flag}")),
            FULL_MISSING,
        ));
    }
    assert_full_run_mutations(&mutations);
}

#[test]
fn lib_qa_007_full_run_check_rejects_shard_bypasses() {
    let w = FULL_RUN_FIXTURE;
    let shards = |s: &str| replace_line(w, FULL_SHARDS, &format!("shard: {s}"));
    let denominator = |n: &str| {
        replace_line(
            w,
            FULL_STEP,
            &format!("- run: {FULL_RUN_COMMAND_PREFIX}{n}"),
        )
    };
    assert_full_run_mutations(&[
        mutation(
            "denominator 3 for 4 shards",
            denominator("3"),
            FULL_SHARDS_CODE,
        ),
        mutation(
            "denominator 5 for 4 shards",
            denominator("5"),
            FULL_SHARDS_CODE,
        ),
        mutation("an index missing", shards("[0, 1, 2]"), FULL_SHARDS_CODE),
        mutation(
            "an index duplicated",
            shards("[0, 1, 1, 3]"),
            FULL_SHARDS_CODE,
        ),
        mutation("indices from 1", shards("[1, 2, 3, 4]"), FULL_SHARDS_CODE),
        mutation(
            "indices that are not numbers",
            shards("[a, b, c, d]"),
            FULL_SHARDS_CODE,
        ),
        mutation(
            "include",
            insert_after(w, FULL_SHARDS, 0, &["include:", "  - shard: 4"]),
            FULL_SHARDS_CODE,
        ),
        mutation(
            "exclude",
            insert_after(w, FULL_SHARDS, 0, &["exclude:", "  - shard: 3"]),
            FULL_SHARDS_CODE,
        ),
        mutation(
            "another matrix dimension",
            insert_after(w, FULL_SHARDS, 0, &["os: [ubuntu-latest, ubuntu-24.04]"]),
            FULL_SHARDS_CODE,
        ),
        mutation(
            "fail-fast: true",
            replace_line(w, "fail-fast: false", "fail-fast: true"),
            FULL_SHARDS_CODE,
        ),
        mutation(
            "fail-fast absent",
            delete_line(w, "fail-fast: false"),
            FULL_SHARDS_CODE,
        ),
        mutation(
            "strategy removed",
            replace_once(w, FULL_STRATEGY_BLOCK, ""),
            FULL_SHARDS_CODE,
        ),
    ]);
}

#[test]
fn lib_qa_007_full_run_check_rejects_job_key_bypasses() {
    let w = FULL_RUN_FIXTURE;
    let job = |lines: &[&str]| insert_after(w, "mutants:", 2, lines);
    let mut mutations = vec![
        mutation("if: false", job(&["if: false"]), JOB_KEY),
        mutation("if: ${{ false }}", job(&["if: ${{ false }}"]), JOB_KEY),
        mutation(
            "if: github.event_name == 'pull_request'",
            job(&["if: github.event_name == 'pull_request'"]),
            JOB_KEY,
        ),
        mutation(
            "continue-on-error: ${{ matrix.shard == 3 }}",
            job(&["continue-on-error: ${{ matrix.shard == 3 }}"]),
            JOB_KEY,
        ),
    ];
    mutations.extend(job_key_mutations(w));
    assert_full_run_mutations(&mutations);
}

#[test]
fn lib_qa_007_full_run_check_rejects_step_bypasses() {
    let w = FULL_RUN_FIXTURE;
    let mut mutations = install_mutations(w, FULL_STEP);
    mutations.extend(unknown_step_mutations(w, FULL_STEP));
    mutations.extend(step_guard_mutations(w, FULL_STEP));
    mutations.extend(env_mutations(w, FULL_STEP));
    mutations.extend([
        mutation(
            "the PR job's diff step",
            insert_before(
                w,
                FULL_STEP,
                &[
                    DIFF_STEP,
                    "  env:",
                    &format!("    {BASE_REF_LINE}"),
                    &format!("  {DIFF_RUN}"),
                ],
            ),
            &["mutants-step-unknown", "mutants-env"],
        ),
        mutation(
            "checkout of another ref",
            insert_after(w, CHECKOUT_STEP, 2, &["with:", "  ref: v0.0.0"]),
            MUTANTS_CHECKOUT,
        ),
        mutation(
            "checkout removed",
            delete_step(w, CHECKOUT_STEP),
            MUTANTS_CHECKOUT,
        ),
    ]);
    assert_full_run_mutations(&mutations);
}

#[test]
fn lib_qa_007_full_run_check_rejects_artifact_bypasses() {
    let w = FULL_RUN_FIXTURE;
    let no_files = |v: &str| {
        replace_line(
            w,
            "if-no-files-found: error",
            &format!("if-no-files-found: {v}"),
        )
    };
    assert_full_run_mutations(&[
        mutation("upload removed", delete_step(w, UPLOAD_STEP), FULL_ARTIFACT),
        mutation(
            "if: always() removed",
            delete_line(w, "if: always()"),
            FULL_ARTIFACT,
        ),
        mutation(
            "if: failure()",
            replace_line(w, "if: always()", "if: failure()"),
            FULL_ARTIFACT,
        ),
        mutation(
            "path not mutants.out",
            replace_line(w, "path: mutants.out", "path: mutants.out/outcomes.json"),
            FULL_ARTIFACT,
        ),
        mutation("if-no-files-found: warn", no_files("warn"), FULL_ARTIFACT),
        mutation(
            "if-no-files-found: ignore",
            no_files("ignore"),
            FULL_ARTIFACT,
        ),
        mutation(
            "if-no-files-found absent",
            delete_line(w, "if-no-files-found: error"),
            FULL_ARTIFACT,
        ),
        mutation(
            "a name without matrix.shard",
            replace_line(w, FULL_ARTIFACT_NAME, "name: mutants.out"),
            FULL_ARTIFACT,
        ),
        mutation(
            "name absent",
            delete_line(w, FULL_ARTIFACT_NAME),
            FULL_ARTIFACT,
        ),
        mutation(
            "continue-on-error on the upload",
            insert_after(w, UPLOAD_STEP, 2, &["continue-on-error: true"]),
            &["full-run-artifact", "step-guard", "step-key"],
        ),
        mutation(
            "upload before the mutants step",
            move_step_after(w, UPLOAD_STEP, INSTALL_STEP),
            FULL_ARTIFACT,
        ),
    ]);
}

#[test]
fn lib_qa_007_full_run_check_rejects_defaults_merge_parse_and_shape() {
    let w = FULL_RUN_FIXTURE;
    let gated = with_gate_job(w, "mutants");
    assert_unparsable_reported(
        QA7,
        &unparsable_mutations(w, "mutants"),
        check_full_run_text,
    );
    let mut mutations = shape_mutations(w);
    mutations.extend([
        mutation(
            "workflow-level defaults.run.shell",
            insert_before(
                w,
                "jobs:",
                &["defaults:", "  run:", "    shell: \"true {0}\""],
            ),
            WORKFLOW_DEFAULTS,
        ),
        mutation(
            "job-level <<: *anchor of a skipped job",
            insert_after(&gated, "mutants:", 2, &["<<: *skip_job"]),
            JOB_KEY_MERGE,
        ),
        mutation(
            "<< in the strategy",
            insert_after(w, "fail-fast: false", 0, &["<<: { fail-fast: true }"]),
            &["merge-key", "full-run-shards"],
        ),
    ]);
    assert_full_run_mutations(&mutations);
}

#[test]
fn lib_qa_007_full_run_check_rejects_broad_permissions() {
    let w = FULL_RUN_FIXTURE;
    assert_full_run_mutations(&[
        mutation(
            "permissions: write-all",
            replace_once(
                w,
                "permissions:\n  contents: read\n",
                "permissions: write-all\n",
            ),
            FULL_PERMISSIONS,
        ),
        mutation(
            "no top-level permissions",
            replace_once(w, "permissions:\n  contents: read\n\n", ""),
            FULL_PERMISSIONS,
        ),
        mutation(
            "contents: write",
            replace_line(w, "contents: read", "contents: write"),
            FULL_PERMISSIONS,
        ),
        mutation(
            "an added write permission",
            insert_after(w, "contents: read", 0, &["pull-requests: write"]),
            FULL_PERMISSIONS,
        ),
        mutation(
            "job-level permissions: write-all",
            insert_after(w, "mutants:", 2, &["permissions: write-all"]),
            FULL_PERMISSIONS,
        ),
    ]);
}

/// P2 controls: other shard counts and strategy forms.
fn full_run_shard_controls() -> Vec<(&'static str, String)> {
    let w = FULL_RUN_FIXTURE;
    let sharded = |shards: &str, n: &str| {
        replace_line(
            &replace_line(w, FULL_SHARDS, &format!("shard: {shards}")),
            FULL_STEP,
            &format!("- run: {FULL_RUN_COMMAND_PREFIX}{n}"),
        )
    };
    vec![
        ("two shards", sharded("[0, 1]", "2")),
        (
            "eight shards, listed backwards",
            sharded("[7, 6, 5, 4, 3, 2, 1, 0]", "8"),
        ),
        (
            "shard indices as strings",
            sharded("[\"0\", \"1\", \"2\", \"3\"]", "4"),
        ),
        (
            "a block sequence of shards",
            replace_lines(
                w,
                FULL_SHARDS,
                &["shard:", "  - 0", "  - 1", "  - 2", "  - 3"],
            ),
        ),
        (
            "a flow-style strategy",
            replace_once(
                w,
                FULL_STRATEGY_BLOCK,
                "    strategy: { fail-fast: false, matrix: { shard: [0, 1, 2, 3] } }\n",
            ),
        ),
        (
            "a single-quoted fail-fast key",
            replace_line(w, "fail-fast: false", "'fail-fast': false"),
        ),
    ]
}

#[test]
fn lib_qa_007_full_run_check_accepts_equivalent_yaml() {
    let w = FULL_RUN_FIXTURE;
    let job = |lines: &[&str]| insert_after(w, "mutants:", 2, lines);
    let mut controls = vec![
        ("the reference workflow", w.to_owned()),
        (
            "single-quoted cron",
            replace_line(w, FULL_CRON, "- cron: '0 3 * * 1'"),
        ),
        (
            "another weekly time",
            replace_line(w, FULL_CRON, "- cron: \"30 4 * * 0\""),
        ),
        (
            "an extra path",
            insert_after(w, "- Cargo.lock", 0, &["- rust-toolchain.toml"]),
        ),
        (
            "a single-quoted path",
            replace_line(w, "- Cargo.toml", "- 'Cargo.toml'"),
        ),
        (
            "workflow_dispatch: {}",
            replace_line(w, "workflow_dispatch:", "workflow_dispatch: {}"),
        ),
        (
            "name and id on the full-run step",
            insert_after(
                w,
                FULL_STEP,
                2,
                &["name: cargo-mutants shard", "id: mutants"],
            ),
        ),
        (
            "${{ always() }} on the upload",
            replace_line(w, "if: always()", "if: ${{ always() }}"),
        ),
        (
            "another artifact name",
            replace_line(
                w,
                FULL_ARTIFACT_NAME,
                "name: mutants-out-${{ matrix.shard }}",
            ),
        ),
        (
            "runs-on: ubuntu-24.04",
            replace_line(w, "runs-on: ubuntu-latest", "runs-on: ubuntu-24.04"),
        ),
        (
            "job-level CARGO_INCREMENTAL: \"1\" (D16)",
            job(&["env:", "  CARGO_INCREMENTAL: \"1\""]),
        ),
        (
            "job-level permissions contents: read",
            job(&["permissions:", "  contents: read"]),
        ),
        (
            "checkout with fetch-depth: 0",
            insert_after(w, CHECKOUT_STEP, 2, &["with:", "  fetch-depth: 0"]),
        ),
        (
            "another timeout-minutes",
            replace_line(w, "timeout-minutes: 60", "timeout-minutes: 45"),
        ),
    ];
    controls.extend(full_run_shard_controls());
    assert_yaml_accepted(QA7, &controls, check_full_run_text);
    assert_real_full_run();
}
/// The reference mutation-testing section of `CONTRIBUTING.md` (P3, P6;
/// criterion 6). One requirement per paragraph, so each can be deleted.
const CONTRIBUTING_FIXTURE: &str = "## Mutation testing

CI runs cargo-mutants on every pull request. Install the version CI uses:

```sh
cargo install cargo-mutants --locked --version 27.1.0
```

Run the whole workspace with `cargo mutants --workspace`.

Run only the mutants in your change with `git diff origin/main...HEAD > mutants.diff` and `cargo mutants --workspace --in-diff mutants.diff`.

The report goes to `mutants.out/`, which is git-ignored.

The authoritative run is on Linux: some sacm-cli tests run only on Linux, so a few sacm-cli mutants show as missed on Windows and macOS.
";

// Lines of `CONTRIBUTING_FIXTURE` that mutations delete.
const CONTRIBUTING_WORKSPACE: &str = "Run the whole workspace with `cargo mutants --workspace`.";
const CONTRIBUTING_IN_DIFF: &str = "Run only the mutants in your change with `git diff origin/main...HEAD > mutants.diff` and `cargo mutants --workspace --in-diff mutants.diff`.";
const CONTRIBUTING_IGNORED: &str = "The report goes to `mutants.out/`, which is git-ignored.";
const CONTRIBUTING_LINUX: &str = "The authoritative run is on Linux: some sacm-cli tests run only on Linux, so a few sacm-cli mutants show as missed on Windows and macOS.";

#[test]
fn lib_qa_007_install_command_is_identical_everywhere() {
    let (ci, full, contributing) = (CI_MUTANTS_FIXTURE, FULL_RUN_FIXTURE, CONTRIBUTING_FIXTURE);
    let bump = |text: &str| {
        replace_once(
            text,
            MUTANTS_INSTALL,
            "cargo install cargo-mutants --locked --version 27.2.0",
        )
    };
    let check = |c: &str, f: Option<&str>, d: &str| check_install_versions(c, f, d);
    assert!(
        check(ci, Some(full), contributing).is_empty(),
        "LIB-QA-007: the reference files must agree before their mutations are meaningful: {:#?}",
        check(ci, Some(full), contributing)
    );
    let extra = format!("{contributing}\nOr: `cargo install cargo-mutants --version 27.1.0`.\n");
    let longer = replace_once(
        contributing,
        MUTANTS_INSTALL,
        &format!("{MUTANTS_INSTALL}.1"),
    );
    for (name, problems) in [
        (
            "the version changed in ci.yml only",
            check(&bump(ci), Some(full), contributing),
        ),
        (
            "the version changed in mutants.yml only",
            check(ci, Some(&bump(full)), contributing),
        ),
        (
            "the version changed in CONTRIBUTING.md only",
            check(ci, Some(full), &bump(contributing)),
        ),
        (
            "a second, different install line in CONTRIBUTING.md",
            check(ci, Some(full), &extra),
        ),
        (
            "a longer version in CONTRIBUTING.md",
            check(ci, Some(full), &longer),
        ),
        (
            "cargo binstall in mutants.yml",
            check(
                ci,
                Some(&format!("{full}# cargo binstall cargo-mutants\n")),
                contributing,
            ),
        ),
        ("mutants.yml missing", check(ci, None, contributing)),
    ] {
        assert!(
            has_code(&problems, "mutants-version-mismatch"),
            "LIB-QA-007: the mutation `{name}` must be reported as `[mutants-version-mismatch]`; it reported {problems:#?}"
        );
    }
    let prose = format!("{contributing}\nTo install cargo-mutants, use the command above.\n");
    let problems = check(ci, Some(full), &prose);
    assert!(
        problems.is_empty(),
        "LIB-QA-007: prose that mentions installing cargo-mutants is not a second command, but was reported: {problems:#?}"
    );
    let problems = check_install_versions(
        &read(CI_WORKFLOW),
        read_optional(FULL_RUN_WORKFLOW).as_deref(),
        &read("CONTRIBUTING.md"),
    );
    assert!(
        problems.is_empty(),
        "LIB-QA-007: ci.yml, mutants.yml and CONTRIBUTING.md must all give exactly `{MUTANTS_INSTALL}`: {problems:#?}"
    );
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
/// must pass P4. Before the files exist this fails with `[config-missing]`
/// and `[triage-missing]` (P7).
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

/// P4.2: keys outside [`REVIEWED_MUTANTS_KEYS`], each with a value that
/// would narrow or redirect the run.
const UNREVIEWED_MUTANTS_KEYS: [(&str, &str); 13] = [
    ("additional_cargo_args", r#"["--release"]"#),
    ("additional_cargo_test_args", r#"["--", "--skip", "lib_"]"#),
    ("output", r#""elsewhere""#),
    ("profile", r#""mutants""#),
    ("test_tool", r#""nextest""#),
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
    cases.push(config_plus(
        "every source of xtask, one by one",
        r#"exclude_globs = ["xtask/src/fetch.rs", "xtask/src/lib.rs", "xtask/src/main.rs", "xtask/src/matrix.rs"]"#,
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
    cases.extend([
        dependency(
            "xtask dev-depends on mutants",
            "xtask",
            "[dev-dependencies]\n",
            "[dev-dependencies]\nmutants = \"0.0.3\"\n",
        ),
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

/// Assert that `check` accepts `reference`, reports `code` for each
/// `mutations` entry, and accepts the real file `rel`.
fn assert_doc_check<N: std::fmt::Display>(
    code: &str,
    reference: &str,
    mutations: &[(N, String)],
    check: impl Fn(&str) -> Vec<String>,
    rel: &str,
) {
    let problems = check(reference);
    assert!(
        problems.is_empty(),
        "LIB-QA-007: the reference text must pass before its mutations are meaningful: {problems:#?}"
    );
    for (name, text) in mutations {
        let problems = check(text);
        assert!(
            has_code(&problems, code),
            "LIB-QA-007: the mutation `{name}` must be reported as `[{code}]`; it reported {problems:#?}; text:\n{text}"
        );
    }
    let problems = check(&read(rel));
    assert!(
        problems.is_empty(),
        "LIB-QA-007: {rel} must describe the mutation-testing process: {problems:#?}"
    );
}

#[test]
fn lib_qa_007_contributing_documents_mutation_testing() {
    let c = CONTRIBUTING_FIXTURE;
    assert_doc_check(
        "doc-contributing",
        c,
        &[
            ("install command removed", delete_line(c, MUTANTS_INSTALL)),
            (
                "install command with another version",
                replace_line(
                    c,
                    MUTANTS_INSTALL,
                    "cargo install cargo-mutants --locked --version 27.0.0",
                ),
            ),
            (
                "whole-workspace run removed",
                delete_line(c, CONTRIBUTING_WORKSPACE),
            ),
            (
                "--in-diff run removed",
                delete_line(c, CONTRIBUTING_IN_DIFF),
            ),
            (
                "git-ignored mutants.out removed",
                delete_line(c, CONTRIBUTING_IGNORED),
            ),
            (
                "authoritative Linux run removed",
                delete_line(c, CONTRIBUTING_LINUX),
            ),
            (
                "Windows and macOS no longer named",
                replace_line(c, CONTRIBUTING_LINUX, "The authoritative run is on Linux."),
            ),
        ],
        check_contributing_mutants,
        "CONTRIBUTING.md",
    );
}

/// The reference mutation-testing parts of `sacm-verifier.md` (P6), with the
/// ten required-check titles that must survive.
const VERIFIER_FIXTURE: &str = "## Required checks

1. **Gates.** Run them.
2. **Traceability.** Each criterion has a test.
3. **Test adequacy.** Review the cargo-mutants report for the slice's code.

   The report is the Linux run of the slice's draft PR at the commit under verification.

   Review its missed, timeout and unviable lists.

   It is the primary evidence, in addition to hand-reasoned criterion mutations for behaviours its operators do not generate.

   Write any local run with `--output target/mutants`, and never with `--in-place`.

   For every exclusion the slice adds or changes (the `.cargo/mutants.toml` diff, and triage rows marked equivalent or accepted), re-derive the claim against P8.2, and show that the excluded set equals the triaged set by comparing `cargo mutants --workspace --list` with `cargo mutants --workspace --list --no-config`.

   If no Linux report for the commit under verification is provided, test adequacy is not shown: a row whose code cargo-mutants mutates may not PASS, and a local Windows run is no substitute.

   Apply a hand-reasoned mutation only in a scratch copy under `target/`, never the working tree, or else list it for the lead.

4. **Negative coverage.** Diagnostics, not panics.
5. **Losslessness.** Nothing is lost.
6. **XMI.** Strict output.
7. **Editing.** Atomic.
8. **API boundary.** Nothing unapproved.
9. **Strict vs tolerant.** Kept apart.
10. **Regression.** Nothing weakened.
";

/// Mutations of the verifier fixture's fix-round-1 paragraphs (F1, F2, F3):
/// each deletes a paragraph or drops one of its required phrases.
fn verifier_fix_round_1_mutations(v: &str) -> Vec<(String, String)> {
    let exclusions = "For every exclusion the slice adds or changes (the `.cargo/mutants.toml` diff, and triage rows marked equivalent or accepted), re-derive the claim against P8.2, and show that the excluded set equals the triaged set by comparing `cargo mutants --workspace --list` with `cargo mutants --workspace --list --no-config`.";
    let no_report = "If no Linux report for the commit under verification is provided, test adequacy is not shown: a row whose code cargo-mutants mutates may not PASS, and a local Windows run is no substitute.";
    let scratch = "Apply a hand-reasoned mutation only in a scratch copy under `target/`, never the working tree, or else list it for the lead.";
    [
        ("F1: the exclusion paragraph removed", delete_line(v, exclusions)),
        (
            "F1: --no-config dropped",
            replace_line(v, exclusions, &exclusions.replace(" --no-config", "")),
        ),
        (
            "F1: re-derive dropped",
            replace_line(
                v,
                exclusions,
                &exclusions.replace("re-derive the claim against P8.2, and show", "show"),
            ),
        ),
        (
            "F1: the exclusion paragraph split in two",
            replace_lines(
                v,
                exclusions,
                &[
                    "For every exclusion the slice adds or changes (the `.cargo/mutants.toml` diff, and triage rows marked equivalent or accepted), re-derive the claim against P8.2.",
                    "",
                    "Show that the excluded set equals the triaged set by comparing `cargo mutants --workspace --list` with `cargo mutants --workspace --list --no-config`.",
                ],
            ),
        ),
        ("F2: the no-Linux-report paragraph removed", delete_line(v, no_report)),
        (
            "F2: may not pass dropped",
            replace_line(
                v,
                no_report,
                &no_report.replace("may not PASS", "needs more evidence"),
            ),
        ),
        (
            "F2: the Windows run no longer excluded",
            replace_line(
                v,
                no_report,
                &no_report.replace(", and a local Windows run is no substitute", ""),
            ),
        ),
        ("F3: the scratch-copy paragraph removed", delete_line(v, scratch)),
        (
            "F3: never the working tree dropped",
            replace_line(
                v,
                scratch,
                &scratch.replace(", never the working tree,", ","),
            ),
        ),
    ]
    .into_iter()
    .map(|(name, text)| (name.to_owned(), text))
    .collect()
}

#[test]
fn lib_qa_007_verifier_definition_requires_the_linux_mutants_report() {
    let v = VERIFIER_FIXTURE;
    let draft =
        "The report is the Linux run of the slice's draft PR at the commit under verification.";
    let lists = "Review its missed, timeout and unviable lists.";
    let reasoned = "It is the primary evidence, in addition to hand-reasoned criterion mutations for behaviours its operators do not generate.";
    let local = "Write any local run with `--output target/mutants`, and never with `--in-place`.";
    let mut mutations: Vec<(String, String)> = [
        ("the draft-PR Linux report removed", delete_line(v, draft)),
        (
            "the missed/timeout/unviable lists removed",
            delete_line(v, lists),
        ),
        (
            "unviable no longer named",
            replace_line(v, lists, "Review its missed and timeout lists."),
        ),
        ("hand-reasoned mutations removed", delete_line(v, reasoned)),
        ("the local-run rule removed", delete_line(v, local)),
        (
            "--output target/ removed",
            replace_line(v, local, "Never write a local run with `--in-place`."),
        ),
        (
            "the --in-place ban removed",
            replace_line(
                v,
                local,
                "Write any local run with `--output target/mutants`.",
            ),
        ),
        (
            "the stale sentence kept",
            format!("{v}\nRun `cargo mutants --file <path>` once it is adopted.\n"),
        ),
    ]
    .into_iter()
    .map(|(name, text)| (name.to_owned(), text))
    .collect();
    mutations.extend(verifier_fix_round_1_mutations(v));
    for title in VERIFIER_CHECK_TITLES {
        let line = v
            .lines()
            .find(|l| l.contains(title))
            .expect("the fixture has every title")
            .trim();
        mutations.push((
            format!("the required check {title} removed"),
            delete_line(v, line),
        ));
    }
    assert_doc_check(
        "doc-verifier",
        v,
        &mutations,
        check_verifier_mutants,
        ".claude/agents/sacm-verifier.md",
    );
}

/// The reference step 7 of the `sacm-slice` skill (P6, D14).
const SKILL_FIXTURE: &str = "## 6. Implementation

Run the implementer.

## 7. Verification → `verified`

**Before verifying, under `mode=pr`:** push the slice branch and open its PR as a **draft**. Its CI gives the Linux report for the commit under verification: the `mutants` job's `mutants.out` artifact and the `mutants.yml` shards.

**Under `mode=local`:** a row whose code cargo-mutants mutates may not PASS until a Linux report for that commit exists; a local Windows run is not a substitute.

Run **sacm-verifier** with the row IDs and the reports.

## 8. Close the slice
";

#[test]
fn lib_qa_007_slice_skill_obtains_the_report_before_verification() {
    let s = SKILL_FIXTURE;
    let before = "**Before verifying, under `mode=pr`:** push the slice branch and open its PR as a **draft**. Its CI gives the Linux report for the commit under verification: the `mutants` job's `mutants.out` artifact and the `mutants.yml` shards.";
    let verifier = "Run **sacm-verifier** with the row IDs and the reports.";
    let local = "**Under `mode=local`:** a row whose code cargo-mutants mutates may not PASS until a Linux report for that commit exists; a local Windows run is not a substitute.";
    assert_doc_check(
        "doc-skill",
        s,
        &[
            (
                "F2: the mode=local paragraph removed",
                delete_line(s, local),
            ),
            (
                "F2: mode=local dropped",
                replace_line(
                    s,
                    local,
                    &local.replace("**Under `mode=local`:** a row", "A row"),
                ),
            ),
            (
                "F2: the mode=local paragraph moved after the verifier runs",
                replace_lines(&delete_line(s, local), verifier, &[verifier, "", local]),
            ),
            ("the draft-PR paragraph removed", delete_line(s, before)),
            (
                "the paragraph moved after the verifier runs",
                replace_lines(&delete_line(s, before), verifier, &[verifier, "", before]),
            ),
            (
                "the full-run shards no longer named",
                replace_line(
                    s,
                    before,
                    &before.replace(" and the `mutants.yml` shards", ""),
                ),
            ),
            (
                "not a draft PR",
                replace_line(s, before, &before.replace(" as a **draft**", "")),
            ),
            (
                "the verification step renamed",
                replace_line(s, "## 7. Verification → `verified`", "## Verification"),
            ),
            ("the verifier step removed", delete_line(s, verifier)),
            (
                "the paragraph moved to step 8",
                replace_lines(
                    &delete_line(s, before),
                    "## 8. Close the slice",
                    &["## 8. Close the slice", "", before],
                ),
            ),
        ],
        check_skill_mutants,
        ".claude/skills/sacm-slice/SKILL.md",
    );
}

// ---------------------------------------------------------------------------
// LIB-QA-011: the sacm-slice skill reuses the slice's PR and dispatches the
// full mutation run.
// ---------------------------------------------------------------------------
//
// Three rules, each looked for only inside its own step:
//
// - step 7 (`## 7.`): reuse the slice's existing PR. The existing-PR query
//   must govern the draft creation: one sentence has `gh pr create --draft`
//   and, after it, one of `PR_QUERY_CONDITIONS` (`unless` followed directly
//   by the code span `gh pr list --head <branch>` and "already lists one" or
//   "lists one"). Step 7 must also say "do not create another";
// - step 7: when `mutants.yml` did not run on the head commit, dispatch it on
//   the slice branch: the code span `gh workflow run mutants.yml --ref
//   <branch>`. The condition and the command must be in one block (a
//   paragraph or a list item), so the dispatch stays tied to its condition;
// - step 8.5 (item `5.` of `## 8.`): one sentence has one of
//   `CREATE_ONLY_WITHOUT_PR`: the code span `gh pr create` followed directly
//   by "only if" (or "only when") "the branch has no PR".
//
// Commands are matched as whole code spans, so a command whose `<branch>`
// argument is dropped or replaced (by `main` or any other word), or that
// gains another argument, no longer matches. `<branch>` is the placeholder
// the real skill uses. A phrase that ends in a letter must end at a word
// boundary, so "has no PR" does not match "has no previous review".
//
// Text is compared in lower case with whitespace collapsed, so a re-wrapped
// line still matches. Sentences are split at ". " inside a block. A step that
// cannot be found counts as missing all of its rules. The check reports one
// fixed problem per missing rule, so a test can assert exactly which rule was
// lost.

const SLICE_SKILL: &str = ".claude/skills/sacm-slice/SKILL.md";

/// The problem for each rule, in the order `check_slice_pr_rules` reports them.
const PR_REUSE_PROBLEM: &str = "[skill-pr-reuse] SKILL.md's step 7 must reuse the slice's existing PR, found with `gh pr list --head <branch>`, and not create another";
const FULL_RUN_DISPATCH_PROBLEM: &str = "[skill-full-run-dispatch] SKILL.md's step 7 must dispatch `mutants.yml` on the slice branch (`gh workflow run mutants.yml --ref <branch>`) when it did not run on the head commit";
const PR_CREATE_PROBLEM: &str =
    "[skill-pr-create] SKILL.md's step 8.5 must run `gh pr create` only when the branch has no PR";

/// The lines of the section of `text` whose heading line starts with
/// `heading` (such as `## 7.`), up to the next `## ` heading.
fn markdown_section<'a>(text: &'a str, heading: &str) -> Option<Vec<&'a str>> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| l.starts_with(heading))?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.starts_with("## "))
        .map_or(lines.len(), |offset| start + 1 + offset);
    Some(lines[start..end].to_vec())
}

/// Whether `line` starts a Markdown list item (`- `, `* `, `+ ` or `N. `).
fn is_list_item(line: &str) -> bool {
    let item = line.trim_start();
    let digits = item.chars().take_while(char::is_ascii_digit).count();
    ["- ", "* ", "+ "].iter().any(|m| item.starts_with(m))
        || (digits > 0 && item[digits..].starts_with(". "))
}

/// Item `number` of the numbered list in `section`: its `N. ` line and every
/// following line indented deeper (blank lines included).
fn numbered_item<'a>(section: &[&'a str], number: usize) -> Option<Vec<&'a str>> {
    let marker = format!("{number}. ");
    let start = section
        .iter()
        .position(|l| l.trim_start().starts_with(&marker))?;
    let depth = indent(section[start]);
    let end = section[start + 1..]
        .iter()
        .position(|l| !l.trim().is_empty() && indent(l) <= depth)
        .map_or(section.len(), |offset| start + 1 + offset);
    Some(section[start..end].to_vec())
}

/// `lines` as one lower-case string with whitespace collapsed.
fn flat_lines(lines: &[&str]) -> String {
    lines
        .iter()
        .flat_map(|l| l.split_whitespace())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The blocks of `lines`, each flattened: a new block starts at every blank
/// line and at every list item.
fn markdown_blocks(lines: &[&str]) -> Vec<String> {
    let mut blocks: Vec<Vec<&str>> = vec![Vec::new()];
    for &line in lines {
        if line.trim().is_empty() || is_list_item(line) {
            blocks.push(Vec::new());
        }
        if !line.trim().is_empty() {
            if let Some(block) = blocks.last_mut() {
                block.push(line);
            }
        }
    }
    blocks
        .iter()
        .filter(|b| !b.is_empty())
        .map(|b| flat_lines(b))
        .collect()
}

/// The sentences of `lines`: each block (flattened), split at every ". ".
fn markdown_sentences(lines: &[&str]) -> Vec<String> {
    markdown_blocks(lines)
        .iter()
        .flat_map(|block| block.split(". ").map(str::to_owned).collect::<Vec<_>>())
        .collect()
}

/// Whether `text` contains `phrase` ending at a word boundary: the next
/// character, if any, is not a letter, digit, `-` or `_`.
fn has_phrase(text: &str, phrase: &str) -> bool {
    text.match_indices(phrase).any(|(at, _)| {
        text[at + phrase.len()..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '-' || c == '_'))
    })
}

/// Step 7's draft creation, as a code span (lower case).
const DRAFT_CREATE: &str = "`gh pr create --draft`";

/// The conditions, one of which must follow `DRAFT_CREATE` in its sentence,
/// so that the existing-PR query governs the draft creation.
const PR_QUERY_CONDITIONS: [&str; 2] = [
    "unless `gh pr list --head <branch>` already lists one",
    "unless `gh pr list --head <branch>` lists one",
];

/// Step 7's full-run dispatch on the slice branch, as a code span.
const DISPATCH_COMMAND: &str = "`gh workflow run mutants.yml --ref <branch>`";

/// Step 8.5's rule, one of which must be in one sentence (lower case).
const CREATE_ONLY_WITHOUT_PR: [&str; 2] = [
    "`gh pr create` only if the branch has no pr",
    "`gh pr create` only when the branch has no pr",
];

/// LIB-QA-011: the problems with the `sacm-slice` skill's PR-reuse and
/// full-run-dispatch rules; empty when all three hold.
fn check_slice_pr_rules(text: &str) -> Vec<String> {
    let step_7 = markdown_section(text, "## 7.");
    let step_8_5 = markdown_section(text, "## 8.").and_then(|s| numbered_item(&s, 5));
    let reuses_pr = step_7.as_ref().is_some_and(|step| {
        let query_governs_draft = markdown_sentences(step).iter().any(|sentence| {
            sentence.find(DRAFT_CREATE).is_some_and(|at| {
                let rest = &sentence[at + DRAFT_CREATE.len()..];
                PR_QUERY_CONDITIONS.iter().any(|c| has_phrase(rest, c))
            })
        });
        query_governs_draft && has_phrase(&flat_lines(step), "do not create another")
    });
    let dispatches_full_run = step_7.as_ref().is_some_and(|step| {
        markdown_blocks(step).iter().any(|block| {
            block.contains(DISPATCH_COMMAND) && block.contains("did not run on the head commit")
        })
    });
    let creates_only_without_pr = step_8_5.as_ref().is_some_and(|item| {
        markdown_sentences(item).iter().any(|sentence| {
            CREATE_ONLY_WITHOUT_PR
                .iter()
                .any(|rule| has_phrase(sentence, rule))
        })
    });
    [
        (reuses_pr, PR_REUSE_PROBLEM),
        (dispatches_full_run, FULL_RUN_DISPATCH_PROBLEM),
        (creates_only_without_pr, PR_CREATE_PROBLEM),
    ]
    .into_iter()
    .filter(|(holds, _)| !holds)
    .map(|(_, problem)| problem.to_owned())
    .collect()
}

/// A reference skill text with the three rules, shaped like the real
/// `SKILL.md`: steps 6 to 9, the rules in steps 7 and 8.5, and item 8.5
/// continued on an indented line.
const PR_RULES_FIXTURE: &str = "## 6. Implementation

Run **sacm-implementer**.

## 7. Verification → `verified`

**Before verifying, under `mode=pr`:** push the slice branch and open its PR as a **draft** (`gh pr create --draft`) unless `gh pr list --head <branch>` already lists one.
- **If the branch already has its PR** (on re-verification), push to that PR; do not create another.
- **Wait for the PR's CI.**
- **If `mutants.yml` did not run on the head commit:** dispatch it on the slice branch (`gh workflow run mutants.yml --ref <branch>`) and wait for that run.

Run **sacm-verifier** with the row IDs and the reports.

## 8. Close the slice

4. Commit.
5. `mode=pr`: push the branch to the slice's PR, which step 7 opened.
   Run `gh pr create` only if the branch has no PR.
6. `mode=pr`: wait for the Copilot review.

## 9. Checkpoints

Stop and report.
";

// Lines (trimmed) of `PR_RULES_FIXTURE` that the controls edit.
const FIXTURE_OPEN_DRAFT: &str = "**Before verifying, under `mode=pr`:** push the slice branch and open its PR as a **draft** (`gh pr create --draft`) unless `gh pr list --head <branch>` already lists one.";
const FIXTURE_REUSE: &str = "- **If the branch already has its PR** (on re-verification), push to that PR; do not create another.";
const FIXTURE_DISPATCH: &str = "- **If `mutants.yml` did not run on the head commit:** dispatch it on the slice branch (`gh workflow run mutants.yml --ref <branch>`) and wait for that run.";
const FIXTURE_CREATE: &str = "Run `gh pr create` only if the branch has no PR.";
const FIXTURE_STEP_6_BODY: &str = "Run **sacm-implementer**.";
const FIXTURE_STEP_7: &str = "## 7. Verification → `verified`";
const FIXTURE_VERIFIER: &str = "Run **sacm-verifier** with the row IDs and the reports.";
const FIXTURE_STEP_8: &str = "## 8. Close the slice";
const FIXTURE_ITEM_4: &str = "4. Commit.";
const FIXTURE_ITEM_5: &str =
    "5. `mode=pr`: push the branch to the slice's PR, which step 7 opened.";
const FIXTURE_ITEM_6: &str = "6. `mode=pr`: wait for the Copilot review.";
const FIXTURE_STEP_9_BODY: &str = "Stop and report.";

/// `text` with the lines `moved` deleted (each found by its trimmed form) and
/// inserted, in order and with the indentation given in `moved`, after the
/// line `after`.
fn move_lines_after(text: &str, moved: &[&str], after: &str) -> String {
    let without = moved
        .iter()
        .fold(text.to_owned(), |t, m| delete_line(&t, m.trim()));
    let mut lines = vec![after];
    lines.extend_from_slice(moved);
    replace_lines(&without, after, &lines)
}

/// Assert that every control gives exactly `expected` (in the check's order).
fn assert_pr_rule_controls(controls: &[(&str, String, &[&str])]) {
    assert!(
        check_slice_pr_rules(PR_RULES_FIXTURE).is_empty(),
        "LIB-QA-011: the reference text must pass before its controls mean anything: {:#?}",
        check_slice_pr_rules(PR_RULES_FIXTURE)
    );
    for (name, text, expected) in controls {
        assert_ne!(
            text, PR_RULES_FIXTURE,
            "LIB-QA-011: precondition: the control `{name}` changes the reference text"
        );
        assert_eq!(
            check_slice_pr_rules(text),
            *expected,
            "LIB-QA-011: the control `{name}` must report exactly the problems for the rules it breaks; text:\n{text}"
        );
    }
}

#[test]
fn lib_qa_011_the_real_slice_skill_keeps_the_pr_and_full_run_rules() {
    let problems = check_slice_pr_rules(&read(SLICE_SKILL));
    assert!(
        problems.is_empty(),
        "LIB-QA-011: {SLICE_SKILL} must keep its PR-reuse and full-run-dispatch rules: {problems:#?}"
    );
}

/// `text` without every line that contains `phrase`; panics unless one does.
fn without_lines_containing(text: &str, phrase: &str) -> String {
    assert!(
        text.lines().any(|l| l.contains(phrase)),
        "LIB-QA-011: precondition: some line contains `{phrase}`"
    );
    joined(text.lines().filter(|l| !l.contains(phrase)))
}

#[test]
fn lib_qa_011_a_rule_removed_from_the_real_skill_is_named_exactly() {
    let real = read(SLICE_SKILL);
    for (phrase, expected) in [
        ("gh pr list --head", PR_REUSE_PROBLEM),
        ("do not create another", PR_REUSE_PROBLEM),
        ("gh workflow run mutants.yml", FULL_RUN_DISPATCH_PROBLEM),
        ("the branch has no PR", PR_CREATE_PROBLEM),
    ] {
        let text = without_lines_containing(&real, phrase);
        assert_eq!(
            check_slice_pr_rules(&text),
            [expected],
            "LIB-QA-011: {SLICE_SKILL} without its lines containing `{phrase}` must report exactly that rule"
        );
    }
}

#[test]
fn lib_qa_011_a_removed_rule_is_named_exactly() {
    let s = PR_RULES_FIXTURE;
    assert_pr_rule_controls(&[
        // Step 7: reuse the slice's PR.
        (
            "the PR-reuse rule removed",
            delete_line(&delete_line(s, FIXTURE_OPEN_DRAFT), FIXTURE_REUSE),
            &[PR_REUSE_PROBLEM],
        ),
        (
            "`gh pr list --head` removed",
            replace_line(
                s,
                FIXTURE_OPEN_DRAFT,
                "**Before verifying, under `mode=pr`:** push the slice branch and open its PR as a **draft** (`gh pr create --draft`).",
            ),
            &[PR_REUSE_PROBLEM],
        ),
        (
            "the paragraph naming `gh pr list --head` removed",
            delete_line(s, FIXTURE_OPEN_DRAFT),
            &[PR_REUSE_PROBLEM],
        ),
        (
            "\"do not create another\" removed",
            delete_line(s, FIXTURE_REUSE),
            &[PR_REUSE_PROBLEM],
        ),
        // Step 7: dispatch the full run.
        (
            "the full-run dispatch rule removed",
            delete_line(s, FIXTURE_DISPATCH),
            &[FULL_RUN_DISPATCH_PROBLEM],
        ),
        (
            "the dispatch without `--ref`, so it runs on the default branch",
            replace_line(
                s,
                FIXTURE_DISPATCH,
                "- **If `mutants.yml` did not run on the head commit:** dispatch it (`gh workflow run mutants.yml`) and wait for that run.",
            ),
            &[FULL_RUN_DISPATCH_PROBLEM],
        ),
        (
            "the dispatch without its condition",
            replace_line(
                s,
                FIXTURE_DISPATCH,
                "- Dispatch `mutants.yml` on the slice branch (`gh workflow run mutants.yml --ref <branch>`) and wait for that run.",
            ),
            &[FULL_RUN_DISPATCH_PROBLEM],
        ),
        (
            "the condition and the dispatch split into separate items",
            replace_lines(
                s,
                FIXTURE_DISPATCH,
                &[
                    "- **If `mutants.yml` did not run on the head commit:** say so in the report.",
                    "- Dispatch it on the slice branch (`gh workflow run mutants.yml --ref <branch>`).",
                ],
            ),
            &[FULL_RUN_DISPATCH_PROBLEM],
        ),
        // Step 8.5: create a PR only when there is none.
        (
            "the create-only-without-a-PR rule removed",
            delete_line(s, FIXTURE_CREATE),
            &[PR_CREATE_PROBLEM],
        ),
        (
            "`gh pr create` made unconditional",
            replace_line(s, FIXTURE_CREATE, "Run `gh pr create`."),
            &[PR_CREATE_PROBLEM],
        ),
        (
            "item 8.5 removed",
            delete_line(&delete_line(s, FIXTURE_ITEM_5), FIXTURE_CREATE),
            &[PR_CREATE_PROBLEM],
        ),
    ]);
}

/// The fixture with its draft-PR paragraph's second sentence replaced by
/// `sentence` (and the controls' extra lines, if any, after it).
fn with_draft_sentence(lines: &[&str]) -> String {
    let first = format!(
        "**Before verifying, under `mode=pr`:** push the slice branch. {}",
        lines[0]
    );
    let mut new = vec![first.as_str()];
    new.extend_from_slice(&lines[1..]);
    replace_lines(PR_RULES_FIXTURE, FIXTURE_OPEN_DRAFT, &new)
}

#[test]
fn lib_qa_011_the_pr_query_must_govern_the_draft_creation() {
    let reuse: &[&str] = &[PR_REUSE_PROBLEM];
    assert_pr_rule_controls(&[
        (
            "the draft creation made unconditional, the PR query kept as information",
            with_draft_sentence(&[
                "Open the PR as a draft (`gh pr create --draft`). Run `gh pr list --head <branch>` to see it.",
            ]),
            reuse,
        ),
        (
            "the PR query run before an unconditional draft creation",
            with_draft_sentence(&[
                "Run `gh pr list --head <branch>`, then open the PR as a draft (`gh pr create --draft`).",
            ]),
            reuse,
        ),
        (
            "the PR query's condition moved to its own list item",
            with_draft_sentence(&[
                "Open the PR as a draft (`gh pr create --draft`).",
                "- Unless `gh pr list --head <branch>` already lists one, say so.",
            ]),
            reuse,
        ),
        (
            "\"unless\" governing something else",
            with_draft_sentence(&[
                "Open the PR as a draft (`gh pr create --draft`) unless CI is red, and run `gh pr list --head <branch>`.",
            ]),
            reuse,
        ),
        (
            "the condition inverted to \"lists none\"",
            with_draft_sentence(&[
                "Open the PR as a draft (`gh pr create --draft`) unless `gh pr list --head <branch>` lists none.",
            ]),
            reuse,
        ),
        (
            "the condition's result clause dropped",
            with_draft_sentence(&[
                "Open the PR as a draft (`gh pr create --draft`) unless `gh pr list --head <branch>`.",
            ]),
            reuse,
        ),
        (
            "the draft creation no longer a draft",
            with_draft_sentence(&[
                "Open the PR (`gh pr create`) unless `gh pr list --head <branch>` already lists one.",
            ]),
            reuse,
        ),
    ]);
}

#[test]
fn lib_qa_011_the_commands_must_name_the_slice_branch() {
    let s = PR_RULES_FIXTURE;
    let reuse: &[&str] = &[PR_REUSE_PROBLEM];
    let dispatch: &[&str] = &[FULL_RUN_DISPATCH_PROBLEM];
    let query = |arg: &str| {
        with_draft_sentence(&[&format!(
            "Open its PR as a draft (`gh pr create --draft`) unless `gh pr list --head{arg}` already lists one."
        )])
    };
    let dispatch_on = |arg: &str| {
        replace_line(
            s,
            FIXTURE_DISPATCH,
            &format!(
                "- **If `mutants.yml` did not run on the head commit:** dispatch it on the slice branch (`gh workflow run mutants.yml --ref{arg}`) and wait for that run."
            ),
        )
    };
    assert!(
        check_slice_pr_rules(&query(" <branch>")).is_empty()
            && check_slice_pr_rules(&dispatch_on(" <branch>")).is_empty(),
        "LIB-QA-011: precondition: the control builders give a passing text with `<branch>`"
    );
    assert_pr_rule_controls(&[
        ("`gh pr list --head` without its branch", query(""), reuse),
        ("`gh pr list --head main`", query(" main"), reuse),
        ("`gh pr list --head <base>`", query(" <base>"), reuse),
        ("`gh pr list --head HEAD`", query(" HEAD"), reuse),
        (
            "`gh pr list --head <branch>` with an extra filter",
            query(" <branch> --state closed"),
            reuse,
        ),
        (
            "`gh workflow run mutants.yml --ref` without its branch",
            dispatch_on(""),
            dispatch,
        ),
        (
            "`gh workflow run mutants.yml --ref main`",
            dispatch_on(" main"),
            dispatch,
        ),
        (
            "`gh workflow run mutants.yml --ref <base>`",
            dispatch_on(" <base>"),
            dispatch,
        ),
        (
            "`gh workflow run mutants.yml --ref <branch>` with an extra input",
            dispatch_on(" <branch> -f shard=0"),
            dispatch,
        ),
    ]);
}

#[test]
fn lib_qa_011_pr_create_must_be_conditioned_on_no_pr() {
    let s = PR_RULES_FIXTURE;
    let create: &[&str] = &[PR_CREATE_PROBLEM];
    assert_pr_rule_controls(&[
        (
            "the condition weakened to a longer word",
            replace_line(
                s,
                FIXTURE_CREATE,
                "Run `gh pr create` only if the branch has no previous review.",
            ),
            create,
        ),
        (
            "the condition governing the reuse, not the creation",
            replace_line(
                s,
                FIXTURE_CREATE,
                "Run `gh pr create`; reuse the PR only if the branch has no PR.",
            ),
            create,
        ),
        (
            "the condition in another sentence than the creation",
            replace_line(
                s,
                FIXTURE_CREATE,
                "Run `gh pr create`. That is safe only if the branch has no PR.",
            ),
            create,
        ),
        (
            "the condition dropped to \"if needed\"",
            replace_line(s, FIXTURE_CREATE, "Run `gh pr create` if needed."),
            create,
        ),
    ]);
}

// Verification 0008 round 1, F1: the negative controls that kill the
// surviving loosening mutations Q2, Q10, Q29, Q30, Q32, Q33, Q34 and Q31.

#[test]
fn lib_qa_011_the_condition_must_follow_the_draft_creation_in_its_sentence() {
    let reuse: &[&str] = &[PR_REUSE_PROBLEM];
    assert_pr_rule_controls(&[
        (
            // F1 item 1 (Q10): one paragraph, two sentences.
            "the draft creation and its condition in two sentences of one paragraph",
            with_draft_sentence(&[
                "Open its PR as a **draft** (`gh pr create --draft`). Do so unless `gh pr list --head <branch>` already lists one.",
            ]),
            reuse,
        ),
        (
            // F1 item 2 (Q2): the condition first, governing something else.
            "the condition before the draft creation in one sentence, not governing it",
            with_draft_sentence(&[
                "Wait unless `gh pr list --head <branch>` already lists one, then open its PR as a **draft** (`gh pr create --draft`).",
            ]),
            reuse,
        ),
    ]);
}

#[test]
fn lib_qa_011_weakened_rule_wording_is_named_exactly() {
    let s = PR_RULES_FIXTURE;
    let reuse: &[&str] = &[PR_REUSE_PROBLEM];
    let dispatch: &[&str] = &[FULL_RUN_DISPATCH_PROBLEM];
    let create: &[&str] = &[PR_CREATE_PROBLEM];
    assert_pr_rule_controls(&[
        (
            // F1 item 3 (Q32).
            "\"already lists one\" weakened to \"already lists none\"",
            replace_line(
                s,
                FIXTURE_OPEN_DRAFT,
                &FIXTURE_OPEN_DRAFT.replace("already lists one", "already lists none"),
            ),
            reuse,
        ),
        (
            // F1 item 3 (Q32).
            "the condition cut off after \"already\"",
            replace_line(
                s,
                FIXTURE_OPEN_DRAFT,
                &FIXTURE_OPEN_DRAFT.replace("already lists one", "already"),
            ),
            reuse,
        ),
        (
            // F1 item 4 (Q34).
            "\"do not create another\" weakened to \"do not create a new branch\"",
            replace_line(
                s,
                FIXTURE_REUSE,
                &FIXTURE_REUSE.replace("do not create another", "do not create a new branch"),
            ),
            reuse,
        ),
        (
            // F1 item 5 (Q30).
            "\"only when\" with the condition weakened to \"needed\"",
            replace_line(s, FIXTURE_CREATE, "Run `gh pr create` only when needed."),
            create,
        ),
        (
            // F1 item 5 (Q30).
            "\"only when\" with the condition weakened to a longer word",
            replace_line(
                s,
                FIXTURE_CREATE,
                "Run `gh pr create` only when the branch has no previous review.",
            ),
            create,
        ),
        (
            // F1 item 6 (Q33).
            "\"did not run on the head commit\" weakened to \"did not run on the base branch\"",
            replace_line(
                s,
                FIXTURE_DISPATCH,
                &FIXTURE_DISPATCH.replace(
                    "did not run on the head commit",
                    "did not run on the base branch",
                ),
            ),
            dispatch,
        ),
    ]);
}

#[test]
fn lib_qa_011_a_blank_line_separates_the_dispatch_from_its_condition() {
    // F1 item 7 (Q29): the condition and the command in two paragraphs.
    assert_pr_rule_controls(&[(
        "the dispatch condition and the command in two paragraphs",
        replace_lines(
            PR_RULES_FIXTURE,
            FIXTURE_DISPATCH,
            &[
                "- **If `mutants.yml` did not run on the head commit:** see below.",
                "",
                "Dispatch it on the slice branch (`gh workflow run mutants.yml --ref <branch>`) and wait for that run.",
            ],
        ),
        &[FULL_RUN_DISPATCH_PROBLEM],
    )]);
}

#[test]
fn lib_qa_011_each_problem_names_its_own_rule_and_command() {
    // F1 item 8 (Q31): the tags, steps and commands as literals, independent
    // of the constants, both in the constants and in what the check reports.
    let rules = [
        (
            PR_REUSE_PROBLEM,
            "[skill-pr-reuse] ",
            "step 7",
            "`gh pr list --head <branch>`",
        ),
        (
            FULL_RUN_DISPATCH_PROBLEM,
            "[skill-full-run-dispatch] ",
            "step 7",
            "`gh workflow run mutants.yml --ref <branch>`",
        ),
        (
            PR_CREATE_PROBLEM,
            "[skill-pr-create] ",
            "step 8.5",
            "`gh pr create`",
        ),
    ];
    for (constant, tag, step, command) in rules {
        assert!(
            constant.starts_with(tag) && constant.contains(step) && constant.contains(command),
            "LIB-QA-011: the problem `{constant}` must start with `{tag}` and name {step} and {command}"
        );
    }
    let s = PR_RULES_FIXTURE;
    for (removed, tag, step, command) in [
        (
            delete_line(s, FIXTURE_REUSE),
            "[skill-pr-reuse] ",
            "step 7",
            "`gh pr list --head <branch>`",
        ),
        (
            delete_line(s, FIXTURE_DISPATCH),
            "[skill-full-run-dispatch] ",
            "step 7",
            "`gh workflow run mutants.yml --ref <branch>`",
        ),
        (
            delete_line(s, FIXTURE_CREATE),
            "[skill-pr-create] ",
            "step 8.5",
            "`gh pr create`",
        ),
    ] {
        let problems = check_slice_pr_rules(&removed);
        assert!(
            problems.len() == 1
                && problems[0].starts_with(tag)
                && problems[0].contains(step)
                && problems[0].contains(command),
            "LIB-QA-011: removing the {tag}rule must report one problem that starts with `{tag}` and names {step} and {command}; it reported {problems:#?}"
        );
    }
}

#[test]
fn lib_qa_011_a_missing_step_names_each_of_its_rules() {
    let s = PR_RULES_FIXTURE;
    assert_pr_rule_controls(&[
        (
            "step 7's heading renamed, so step 7 is gone",
            replace_line(s, FIXTURE_STEP_7, "## Verification"),
            &[PR_REUSE_PROBLEM, FULL_RUN_DISPATCH_PROBLEM],
        ),
        (
            "step 8's heading renamed, so step 8.5 is gone",
            replace_line(s, FIXTURE_STEP_8, "## Close the slice"),
            &[PR_CREATE_PROBLEM],
        ),
        (
            "every rule removed",
            [
                FIXTURE_OPEN_DRAFT,
                FIXTURE_REUSE,
                FIXTURE_DISPATCH,
                FIXTURE_CREATE,
            ]
            .iter()
            .fold(s.to_owned(), |t, line| delete_line(&t, line)),
            &[
                PR_REUSE_PROBLEM,
                FULL_RUN_DISPATCH_PROBLEM,
                PR_CREATE_PROBLEM,
            ],
        ),
    ]);
}

#[test]
fn lib_qa_011_a_rule_moved_to_another_step_is_named_exactly() {
    let s = PR_RULES_FIXTURE;
    let reuse = [FIXTURE_OPEN_DRAFT, FIXTURE_REUSE];
    // Indented as a continuation line of the item it is moved into.
    let create_in_item = format!("   {FIXTURE_CREATE}");
    assert_pr_rule_controls(&[
        (
            "the PR-reuse rule moved to step 6",
            move_lines_after(s, &reuse, FIXTURE_STEP_6_BODY),
            &[PR_REUSE_PROBLEM],
        ),
        (
            "the PR-reuse rule moved to step 8",
            move_lines_after(s, &reuse, FIXTURE_STEP_8),
            &[PR_REUSE_PROBLEM],
        ),
        (
            "\"do not create another\" moved to step 8",
            move_lines_after(s, &[FIXTURE_REUSE], FIXTURE_ITEM_6),
            &[PR_REUSE_PROBLEM],
        ),
        (
            "the full-run dispatch rule moved to step 6",
            move_lines_after(s, &[FIXTURE_DISPATCH], FIXTURE_STEP_6_BODY),
            &[FULL_RUN_DISPATCH_PROBLEM],
        ),
        (
            "the full-run dispatch rule moved to step 8",
            move_lines_after(s, &[FIXTURE_DISPATCH], FIXTURE_ITEM_6),
            &[FULL_RUN_DISPATCH_PROBLEM],
        ),
        (
            "the full-run dispatch rule moved to step 9",
            move_lines_after(s, &[FIXTURE_DISPATCH], FIXTURE_STEP_9_BODY),
            &[FULL_RUN_DISPATCH_PROBLEM],
        ),
        (
            "the create-only-without-a-PR rule moved to step 7",
            move_lines_after(s, &[FIXTURE_CREATE], FIXTURE_VERIFIER),
            &[PR_CREATE_PROBLEM],
        ),
        (
            "the create-only-without-a-PR rule moved to step 8.4",
            move_lines_after(s, &[&create_in_item], FIXTURE_ITEM_4),
            &[PR_CREATE_PROBLEM],
        ),
        (
            "the create-only-without-a-PR rule moved to step 8.6",
            move_lines_after(s, &[&create_in_item], FIXTURE_ITEM_6),
            &[PR_CREATE_PROBLEM],
        ),
    ]);
}

#[test]
fn lib_qa_011_the_check_accepts_equivalent_wording_and_wrapping() {
    let s = PR_RULES_FIXTURE;
    let controls = [
        (
            "\"lists one\" without \"already\"",
            replace_line(
                s,
                FIXTURE_OPEN_DRAFT,
                &FIXTURE_OPEN_DRAFT.replace("already lists one", "lists one"),
            ),
        ),
        (
            "\"only when\" instead of \"only if\"",
            replace_line(
                s,
                FIXTURE_CREATE,
                "Run `gh pr create` only when the branch has no PR.",
            ),
        ),
        (
            "the dispatch item wrapped over three lines",
            replace_lines(
                s,
                FIXTURE_DISPATCH,
                &[
                    "- **If `mutants.yml` did not run on the head",
                    "  commit:** dispatch it on the slice branch (`gh workflow run",
                    "  mutants.yml --ref <branch>`) and wait for that run.",
                ],
            ),
        ),
        (
            "item 8.5 on one line",
            delete_line(
                &replace_line(
                    s,
                    FIXTURE_ITEM_5,
                    "5. `mode=pr`: push the branch to the slice's PR. Run `gh pr create` only if the branch has no PR.",
                ),
                FIXTURE_CREATE,
            ),
        ),
    ];
    for (name, text) in controls {
        assert!(
            check_slice_pr_rules(&text).is_empty(),
            "LIB-QA-011: the equivalent text `{name}` must pass: {:#?}; text:\n{text}",
            check_slice_pr_rules(&text)
        );
    }
}

// ---------------------------------------------------------------------------
// Exact controls (LIB-QA-008 and LIB-QA-009)
// ---------------------------------------------------------------------------

const GATE_MISSING: &[&str] = &["gate-missing"];
const GATE_MISSING_TWICE: &[&str] = &["gate-missing", "gate-missing"];
const GATE_MISSING_AND_STEP: &[&str] = &["gate-missing", "gate-step"];
const GATE_STEP: &[&str] = &["gate-step"];
const GATE_ACTION: &[&str] = &["gate-action"];
const GATE_ENV_AND_STEP_KEY: &[&str] = &["gate-env", "step-key"];
const STEP_KEY: &[&str] = &["step-key"];
const STEP_GUARD: &[&str] = &["step-guard"];
const GATE_ENV: &[&str] = &["gate-env"];
const GATE_OS: &[&str] = &["gate-os"];
const GATE_OS_TWICE: &[&str] = &["gate-os", "gate-os"];
const GATE_TRIGGER: &[&str] = &["gate-trigger"];
const MERGE_KEY: &[&str] = &["merge-key"];
const YAML_PARSE: &[&str] = &["yaml-parse"];
const YAML_DOCUMENTS: &[&str] = &["yaml-documents"];
const ACTION_UNPINNED: &[&str] = &["action-unpinned"];
const ACTION_UNREVIEWED: &[&str] = &["action-unreviewed"];
const ACTION_INPUT: &[&str] = &["action-input"];
const ACTION_INPUT_TWICE: &[&str] = &["action-input", "action-input"];

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

/// Assert that `reference` passes, and that each control, which must differ
/// from it, gives exactly the problem codes it names: no fewer, no others,
/// and each as many times as named. A control is one YAML document unless it
/// names `yaml-parse` or `yaml-documents`, so that the check and not the
/// parser is what rejects it.
fn assert_exact_controls(
    row: &str,
    reference: &str,
    controls: &[(&str, String, &[&str])],
    check: impl Fn(&str) -> Vec<String>,
) {
    let problems = check(reference);
    assert!(
        problems.is_empty(),
        "{row}: the reference fixture must pass before its controls mean anything: {problems:#?}"
    );
    for (name, text, expected) in controls {
        assert_ne!(
            text, reference,
            "{row}: precondition: the control `{name}` changes the fixture"
        );
        let unparsable = expected
            .iter()
            .any(|code| matches!(*code, "yaml-parse" | "yaml-documents"));
        assert_eq!(
            parses_as_one_document(text),
            !unparsable,
            "{row}: precondition: the control `{name}` is one YAML document exactly when it is not about a parse failure; text:\n{text}"
        );
        let mut want = expected.to_vec();
        want.sort_unstable();
        let problems = check(text);
        assert_eq!(
            problem_codes(&problems),
            want,
            "{row}: the control `{name}` must give exactly {want:?}; it gave {problems:#?}; text:\n{text}"
        );
    }
}

/// `text` wrapped in a one-item sequence, so the workflow is not a mapping.
fn wrapped_in_sequence(text: &str) -> String {
    text.lines()
        .enumerate()
        .map(|(i, line)| match (i, line.is_empty()) {
            (0, _) => format!("- {line}\n"),
            (_, true) => "\n".to_owned(),
            (_, false) => format!("  {line}\n"),
        })
        .collect()
}

/// `text` with a job appended (`jobs:` is its last top-level key) whose `env`
/// merges an anchored mapping with `<<`. The job runs no gate and no
/// third-party action, so only `[merge-key]` concerns it.
fn with_merge_key_job(text: &str) -> String {
    append_job(
        text,
        &[
            "merged:",
            "  runs-on: ubuntu-latest",
            "  env:",
            "    base: &base",
            "      A: \"1\"",
            "    more:",
            "      <<: *base",
            "  steps:",
            "    - run: echo merged",
        ],
    )
}

// ---------------------------------------------------------------------------
// LIB-QA-009: third-party CI actions are pinned and their inputs reviewed
// ---------------------------------------------------------------------------
//
// Every `uses` of a step or of a job, in every workflow in
// `.github/workflows/`, whose owner is not GitHub's `actions` (compared
// without regard to case) must name a commit:
// `<owner>/<repo>@<40 lowercase hex digits>`, or it is `[action-unpinned]`.
// The pair must be in the reviewed table (`[action-unreviewed]`), and every
// `with` key of a step that uses it must be among that entry's inputs,
// compared without regard to case (`[action-input]`). A version bump changes
// the SHA, so it fails this check until someone reviews the action's
// `action.yml` at the new SHA and adds an entry.
//
// This check judges which inputs are passed. The values of cargo-deny's
// inputs are LIB-QA-003's to judge.

/// A third-party action at a reviewed commit, and the `with` inputs the
/// repository may pass it there.
struct ReviewedAction {
    /// `<owner>/<repo>`. Compared without regard to case, as GitHub resolves
    /// repository names.
    action: &'static str,
    /// The full commit SHA: 40 lowercase hex digits.
    sha: &'static str,
    /// The tag or branch the SHA was resolved from. The workflow names it in
    /// a `# <ref>` comment after the pin.
    resolved_from: &'static str,
    /// The inputs the repository may pass, in lower case, each with a
    /// comment on how it reaches the action's command line at this SHA.
    inputs: &'static [&'static str],
}

/// The reviewed table (slice 0009 D1 and D2), resolved on 2026-11-09.
static REVIEWED_ACTIONS: [ReviewedAction; 3] = [
    // A docker action. Its `args` are `rust-version credentials ssh-key
    // ssh-known-hosts --log-level <log-level> --manifest-path <manifest-path>
    // <arguments> <command> <command-arguments>`. The entrypoint uses the
    // first four quoted: `rustup default "$1"` switches the toolchain, `"$2"`
    // is written to `~/.git-credentials`, and `"$3"` and `"$4"` are written to
    // `~/.ssh`, which adds `insteadOf` URL rewrites. Then `shift 4` drops
    // them, and only the remaining five reach `cargo-deny $*`, unquoted. So
    // `rust-version`, `credentials`, `ssh-key` and `ssh-known-hosts` are not
    // reviewed inputs: they switch the toolchain, store git credentials and
    // rewrite URLs. `use-git-cli` is deprecated and not in `args`; it is not
    // reviewed either.
    ReviewedAction {
        action: "EmbarkStudios/cargo-deny-action",
        sha: "3c6349835b2b7b196a839186cb8b78e02f7b5f25",
        resolved_from: "v2.1.1",
        // Each of the five is spliced unquoted into `cargo-deny $*`, which is
        // why LIB-QA-003 judges its value.
        inputs: &[
            // `<command>`, unquoted. LIB-QA-003 requires exactly `check`.
            "command",
            // `<arguments>`, unquoted, before the command. LIB-QA-003 allows
            // only `--all-features` and `--workspace`.
            "arguments",
            // `<command-arguments>`, unquoted, after the command. LIB-QA-003
            // refuses any value.
            "command-arguments",
            // `--manifest-path <manifest-path>`, unquoted. LIB-QA-003 allows
            // only the root `Cargo.toml`.
            "manifest-path",
            // `--log-level <log-level>`, unquoted. LIB-QA-003 allows only
            // cargo-deny's six level names.
            "log-level",
        ],
    },
    ReviewedAction {
        action: "Swatinem/rust-cache",
        sha: "d1527c8f67ef9af357290475d4af4b0fab8f12e0",
        resolved_from: "v2.9.3",
        // None: the repository passes it no input, so none is reviewed.
        inputs: &[],
    },
    // At `master` the action cannot take the toolchain from the ref name, so
    // `toolchain` is required. `targets` is not a reviewed input.
    ReviewedAction {
        action: "dtolnay/rust-toolchain",
        sha: "683b588c487a69d01f6c6ddf40f036ff682cc2c8",
        resolved_from: "master",
        inputs: &[
            // Passed through `env`, not spliced into the script, and quoted
            // where it is used: `rustup toolchain install "$toolchain"`.
            "toolchain",
            // Passed through `env`, as `toolchain` is.
            "components",
        ],
    },
];

/// The directory GitHub reads workflows from.
const WORKFLOWS_DIR: &str = ".github/workflows";

/// Whether `uses` names an action of GitHub's own `actions` owner, compared
/// without regard to case. Those stay on major tags, which Dependabot
/// updates, and are not judged (slice 0009, out of scope). `actions-rs` and
/// `actionsx` are other owners.
fn is_github_owned(uses: &str) -> bool {
    uses.split_once('/')
        .is_some_and(|(owner, _)| owner.eq_ignore_ascii_case("actions"))
}

/// An owner as GitHub allows it: letters, digits and `-`.
fn is_owner_name(part: &str) -> bool {
    !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// A repository name as GitHub allows it: letters, digits, `-`, `_` and `.`,
/// but not `.` or `..`.
fn is_repo_name(part: &str) -> bool {
    !matches!(part, "" | "." | "..")
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// `(action, sha)` if `uses` is exactly `<owner>/<repo>@<40 lowercase hex
/// digits>` (acceptance item 1). A tag, a branch, a short or uppercase SHA, a
/// sub-path, a `docker://` image and a `./` local action are not.
fn pinned_action(uses: &str) -> Option<(&str, &str)> {
    let (action, sha) = uses.split_once('@')?;
    let (owner, repo) = action.split_once('/')?;
    let full_sha = sha.len() == 40 && sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    (is_owner_name(owner) && is_repo_name(repo) && full_sha).then_some((action, sha))
}

/// The reviewed entry for `action` at `sha` (acceptance item 2).
fn reviewed_action(action: &str, sha: &str) -> Option<&'static ReviewedAction> {
    REVIEWED_ACTIONS
        .iter()
        .find(|entry| entry.action.eq_ignore_ascii_case(action) && entry.sha == sha)
}

/// Acceptance items 1 to 3 for one `uses` value of `file` at `at` (a job, or
/// a job's step), with that job's or step's `with`. A `with` that is not a
/// mapping is a `[shape]` problem reported elsewhere.
fn action_use_problems(
    file: &str,
    at: &str,
    uses: &str,
    with: Option<&Yaml>,
    problems: &mut Vec<String>,
) {
    if is_github_owned(uses) {
        return;
    }
    // Item 1: the pinned form.
    let Some((action, sha)) = pinned_action(uses) else {
        problems.push(problem(
            "action-unpinned",
            format!(
                "{file}: {at} uses `{uses}`; a third-party action must be pinned as `<owner>/<repo>@<40 lowercase hex digits>`"
            ),
        ));
        return;
    };
    // Item 2: the pair is reviewed.
    let Some(reviewed) = reviewed_action(action, sha) else {
        problems.push(problem(
            "action-unreviewed",
            format!(
                "{file}: {at} uses `{action}` at `{sha}`, which the reviewed table does not list; review `{action}`'s `action.yml` at `{sha}`, then add the pair with the inputs the repository may pass"
            ),
        ));
        return;
    };
    // Item 3: only reviewed inputs, compared without regard to case.
    let Some(Yaml::Hash(with)) = with else {
        return;
    };
    for key in with.keys() {
        let reviewed_input = key.as_str().is_some_and(|key| {
            reviewed
                .inputs
                .iter()
                .any(|input| input.eq_ignore_ascii_case(key))
        });
        if !reviewed_input {
            problems.push(problem(
                "action-input",
                format!(
                    "{file}: {at} passes `{}` to `{action}` at `{sha}` (resolved from `{}`), which is not a reviewed input; reviewed: {:?}",
                    show(key),
                    reviewed.resolved_from,
                    reviewed.inputs
                ),
            ));
        }
    }
}

/// A job-level `uses` (a reusable workflow), judged as a step's is. A job's
/// `uses` that is not a string, or its `with` that is not a mapping, is
/// `[shape]`, as [`read_workflow`] reports for a step.
fn job_uses_problems(file: &str, job: &Job<'_>, problems: &mut Vec<String>) {
    let at = format!("job `{}`", job.id);
    match get(job.map, "uses") {
        None => {}
        Some(Yaml::String(uses)) => {
            action_use_problems(file, &at, uses, get(job.map, "with"), problems);
        }
        Some(other) => problems.push(problem(
            "shape",
            format!("{file}: {at}: `uses` is not a string: {other:?}"),
        )),
    }
    if let Some(with) = get(job.map, "with").filter(|with| with.as_hash().is_none()) {
        problems.push(problem(
            "shape",
            format!("{file}: {at}: `with` is not a mapping: {with:?}"),
        ));
    }
}

/// LIB-QA-009 over `workflows`, each `(file, text)`. Every problem names its
/// file. A workflow that does not parse is `[yaml-parse]` (or
/// `[yaml-documents]`) and the others are still judged. A `<<` anywhere is
/// `[merge-key]`; every shape error is `[shape]`.
fn check_action_pins(workflows: &[(&str, &str)]) -> Vec<String> {
    let mut problems = Vec::new();
    for &(file, text) in workflows {
        let doc = match load_workflow(text) {
            Ok(doc) => doc,
            Err(parse) => {
                problems.push(format!("{parse} (in {file})"));
                continue;
            }
        };
        merge_key_problems(&doc, file, &mut problems);
        let mut shape = Vec::new();
        let workflow = read_workflow(&doc, &mut shape);
        problems.extend(shape.into_iter().map(|p| format!("{p} (in {file})")));
        let Some(workflow) = workflow else {
            continue;
        };
        for job in &workflow.jobs {
            job_uses_problems(file, job, &mut problems);
            for (number, step) in &job.steps {
                if let Some(uses) = get(step, "uses").and_then(Yaml::as_str) {
                    let at = format!("job `{}`, step {number}", job.id);
                    action_use_problems(file, &at, uses, get(step, "with"), &mut problems);
                }
            }
        }
    }
    problems
}

/// Every workflow in `.github/workflows/`, as `(path, text)`, sorted by path:
/// each `*.yml`, and each `*.yaml`, which GitHub reads too.
fn real_workflows() -> Vec<(String, String)> {
    workflow_files(&repository_root().join(WORKFLOWS_DIR))
        .into_iter()
        .map(|(name, text)| (format!("{WORKFLOWS_DIR}/{name}"), text))
        .collect()
}

/// Every workflow file in `dir`, as `(file name, text)`, sorted by name: each
/// `*.yml` and each `*.yaml`.
fn workflow_files(dir: &Path) -> Vec<(String, String)> {
    let shown = dir.display();
    let mut workflows: Vec<(String, String)> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {shown}: {e}"))
        .map(|entry| entry.unwrap_or_else(|e| panic!("read {shown}: {e}")).path())
        .filter(|path| {
            path.is_file()
                && matches!(
                    path.extension().and_then(std::ffi::OsStr::to_str),
                    Some("yml" | "yaml")
                )
        })
        .map(|path| {
            let name = path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .expect("a workflow file name is UTF-8")
                .to_owned();
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            (name, text)
        })
        .collect();
    workflows.sort();
    workflows
}

/// Acceptance item 1 (verification 0009 Q9-41): the check reads `*.yaml`
/// workflows as well as `*.yml`, and nothing else in the directory.
#[test]
fn lib_qa_009_yaml_and_yml_workflows_are_read() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("lib_qa_009_workflow_files");
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("LIB-QA-009: clear the scratch directory");
    }
    fs::create_dir_all(dir.join("nested.yml")).expect("LIB-QA-009: create the scratch directory");
    let unpinned = with_cache_uses("Swatinem/rust-cache@v2");
    for (name, text) in [
        ("a.yml", PINS_FIXTURE),
        ("b.yaml", unpinned.as_str()),
        ("c.txt", unpinned.as_str()),
        ("d.yml.bak", unpinned.as_str()),
    ] {
        fs::write(dir.join(name), text).expect("LIB-QA-009: write a scratch workflow");
    }
    let files = workflow_files(&dir);
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        ["a.yml", "b.yaml"],
        "LIB-QA-009: `*.yml` and `*.yaml` files are workflows, and nothing else is"
    );
    let pairs: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, text)| (name.as_str(), text.as_str()))
        .collect();
    let problems = check_action_pins(&pairs);
    assert_eq!(
        problem_codes(&problems),
        ["action-unpinned"],
        "LIB-QA-009: the `.yaml` workflow is judged: {problems:#?}"
    );
    assert!(
        problems[0].contains("b.yaml"),
        "LIB-QA-009: the problem names the `.yaml` file: {}",
        problems[0]
    );
}

/// A pinned mini-workflow: each reviewed action at its reviewed SHA with
/// reviewed inputs, and GitHub's own actions on major tags. It must pass.
const PINS_FIXTURE: &str = r#"name: pins

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8 # master
        with:
          toolchain: stable
          components: clippy
      - uses: Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0 # v2.9.3
      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings
      - uses: actions/upload-artifact@v7
        with:
          name: out
          path: out

  msrv:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8
        with:
          toolchain: "1.85"
      - run: cargo +1.85 test -p sacm -p sacm-cli

  deny:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25 # v2.1.1
        with:
          command: check
          arguments: --all-features
          manifest-path: Cargo.toml
          log-level: warn
"#;

// Lines (trimmed) of `PINS_FIXTURE` that the controls edit.
const PIN_TOOLCHAIN: &str =
    "- uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8 # master";
const PIN_CACHE: &str =
    "- uses: Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0 # v2.9.3";
const PIN_DENY: &str =
    "- uses: EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25 # v2.1.1";
const PIN_UPLOAD: &str = "- uses: actions/upload-artifact@v7";
const PIN_TOOLCHAIN_INPUT: &str = "toolchain: stable";
const PIN_COMPONENTS: &str = "components: clippy";
const PIN_LOG_LEVEL: &str = "log-level: warn";

// The reviewed SHAs, for building controls.
const TOOLCHAIN_SHA: &str = "683b588c487a69d01f6c6ddf40f036ff682cc2c8";
const CACHE_SHA: &str = "d1527c8f67ef9af357290475d4af4b0fab8f12e0";
const DENY_SHA: &str = "3c6349835b2b7b196a839186cb8b78e02f7b5f25";

const QA9: &str = "LIB-QA-009";

/// LIB-QA-009 over one workflow, `pins.yml`.
fn pins_check(text: &str) -> Vec<String> {
    check_action_pins(&[("pins.yml", text)])
}

fn assert_pin_controls(controls: &[(&str, String, &[&str])]) {
    assert_exact_controls(QA9, PINS_FIXTURE, controls, pins_check);
}

/// `PINS_FIXTURE` with the toolchain step's `uses` at `reference` instead of
/// the reviewed SHA. Its `with` stays.
fn with_toolchain_ref(reference: &str) -> String {
    replace_line(
        PINS_FIXTURE,
        PIN_TOOLCHAIN,
        &format!("- uses: dtolnay/rust-toolchain@{reference}"),
    )
}

/// `PINS_FIXTURE` with the rust-cache step's `uses` replaced by `uses`.
fn with_cache_uses(uses: &str) -> String {
    replace_line(PINS_FIXTURE, PIN_CACHE, &format!("- uses: {uses}"))
}

/// `PINS_FIXTURE` with a job appended whose keys are `lines` (indented as
/// keys of the job).
fn with_called_workflow(lines: &[&str]) -> String {
    let mut job = vec!["call:"];
    job.extend_from_slice(lines);
    append_job(PINS_FIXTURE, &job)
}

/// `PINS_FIXTURE` with `lines` added to the cargo-deny step's `with`.
fn with_deny_inputs(lines: &[&str]) -> String {
    insert_after(PINS_FIXTURE, PIN_LOG_LEVEL, 0, lines)
}

/// `PINS_FIXTURE` with `line` added to the toolchain step's `with`.
fn with_toolchain_input(line: &str) -> String {
    insert_after(PINS_FIXTURE, PIN_COMPONENTS, 0, &[line])
}

/// `PINS_FIXTURE` with a `with` of `lines` given to the rust-cache step.
fn with_cache_inputs(lines: &[&str]) -> String {
    let mut with = vec!["with:".to_owned()];
    with.extend(lines.iter().map(|line| format!("  {line}")));
    let with: Vec<&str> = with.iter().map(String::as_str).collect();
    insert_after(PINS_FIXTURE, PIN_CACHE, 2, &with)
}

/// Acceptance item 4: every real workflow passes. It fails until the lead
/// pins the third-party actions (slice 0009 D4).
#[test]
fn lib_qa_009_every_workflow_pins_and_reviews_its_third_party_actions() {
    let workflows = real_workflows();
    let files: Vec<&str> = workflows.iter().map(|(file, _)| file.as_str()).collect();
    for required in [CI_WORKFLOW, FULL_RUN_WORKFLOW] {
        assert!(
            files.contains(&required),
            "LIB-QA-009: the check must read {required}; it read {files:?}"
        );
    }
    let pairs: Vec<(&str, &str)> = workflows
        .iter()
        .map(|(file, text)| (file.as_str(), text.as_str()))
        .collect();
    let problems = check_action_pins(&pairs);
    assert!(
        problems.is_empty(),
        "LIB-QA-009: every third-party action must be pinned by a reviewed full commit SHA and passed only reviewed inputs: {problems:#?}"
    );
}

/// Acceptance items 2 and 3: the table records each pin, the ref it was
/// resolved from and exactly the reviewed inputs, and refuses the
/// cargo-deny-action inputs that switch the toolchain, store git credentials
/// or rewrite URLs, and the deprecated `use-git-cli`.
#[test]
fn lib_qa_009_the_reviewed_table_records_each_pin_and_its_inputs() {
    let table: Vec<(&str, &str, &str, &[&str])> = REVIEWED_ACTIONS
        .iter()
        .map(|entry| (entry.action, entry.sha, entry.resolved_from, entry.inputs))
        .collect();
    let expected: [(&str, &str, &str, &[&str]); 3] = [
        (
            "EmbarkStudios/cargo-deny-action",
            DENY_SHA,
            "v2.1.1",
            &[
                "command",
                "arguments",
                "command-arguments",
                "manifest-path",
                "log-level",
            ],
        ),
        ("Swatinem/rust-cache", CACHE_SHA, "v2.9.3", &[]),
        (
            "dtolnay/rust-toolchain",
            TOOLCHAIN_SHA,
            "master",
            &["toolchain", "components"],
        ),
    ];
    assert_eq!(
        table, expected,
        "LIB-QA-009: the reviewed table must hold exactly the reviewed pins and inputs"
    );
    for entry in &REVIEWED_ACTIONS {
        let uses = format!("{}@{}", entry.action, entry.sha);
        assert_eq!(
            pinned_action(&uses),
            Some((entry.action, entry.sha)),
            "LIB-QA-009: the reviewed pin `{uses}` must have the pinned form"
        );
        for input in entry.inputs {
            assert_eq!(
                *input,
                input.to_ascii_lowercase(),
                "LIB-QA-009: the reviewed input `{input}` of `{}` must be in lower case",
                entry.action
            );
        }
    }
    let deny = &REVIEWED_ACTIONS[0];
    for refused in [
        "rust-version",
        "credentials",
        "ssh-key",
        "ssh-known-hosts",
        "use-git-cli",
    ] {
        assert!(
            !deny.inputs.contains(&refused),
            "LIB-QA-009: cargo-deny-action's `{refused}` switches the toolchain, stores git credentials, rewrites URLs or is deprecated; it must not be a reviewed input"
        );
    }
}

/// Acceptance item 1: a tag, a branch, a short or uppercase SHA, 39 or 41 hex
/// digits, or no ref is `[action-unpinned]`, even for a reviewed action.
#[test]
fn lib_qa_009_a_ref_other_than_a_full_lowercase_sha_is_unpinned() {
    let sha = TOOLCHAIN_SHA;
    let upper = sha.to_ascii_uppercase();
    let one_upper = format!("{}B{}", &sha[..3], &sha[4..]);
    assert_eq!(&sha[3..4], "b", "precondition: the fourth digit is `b`");
    assert_pin_controls(&[
        ("a tag", with_toolchain_ref("v1"), ACTION_UNPINNED),
        (
            "a major tag of a reviewed action",
            with_cache_uses("Swatinem/rust-cache@v2"),
            ACTION_UNPINNED,
        ),
        (
            "the ref the SHA was resolved from",
            with_cache_uses("Swatinem/rust-cache@v2.9.3"),
            ACTION_UNPINNED,
        ),
        ("a branch", with_toolchain_ref("master"), ACTION_UNPINNED),
        (
            "the `stable` ref",
            with_toolchain_ref("stable"),
            ACTION_UNPINNED,
        ),
        (
            "a short SHA",
            with_toolchain_ref(&sha[..7]),
            ACTION_UNPINNED,
        ),
        (
            "an uppercase SHA",
            with_toolchain_ref(&upper),
            ACTION_UNPINNED,
        ),
        (
            "a SHA with one uppercase digit",
            with_toolchain_ref(&one_upper),
            ACTION_UNPINNED,
        ),
        (
            "39 hex digits",
            with_toolchain_ref(&sha[..39]),
            ACTION_UNPINNED,
        ),
        (
            "41 hex digits",
            with_toolchain_ref(&format!("{sha}0")),
            ACTION_UNPINNED,
        ),
        (
            "40 characters that are not hex",
            with_toolchain_ref(&"g".repeat(40)),
            ACTION_UNPINNED,
        ),
        (
            "a SHA followed by a second ref",
            with_toolchain_ref(&format!("{sha}@v1")),
            ACTION_UNPINNED,
        ),
        (
            "no ref",
            with_cache_uses("Swatinem/rust-cache"),
            ACTION_UNPINNED,
        ),
        (
            "an empty ref",
            with_cache_uses("Swatinem/rust-cache@"),
            ACTION_UNPINNED,
        ),
        (
            "a quoted pin with a trailing space",
            with_cache_uses(&format!("\"Swatinem/rust-cache@{CACHE_SHA} \"")),
            ACTION_UNPINNED,
        ),
    ]);
}

/// Acceptance item 1: a sub-path, a `docker://` image and a `./` local
/// action are `[action-unpinned]`, even at a reviewed SHA.
#[test]
fn lib_qa_009_sub_path_docker_and_local_actions_are_unpinned() {
    let digest = "a".repeat(64);
    assert_pin_controls(&[
        (
            "a sub-path at a reviewed SHA",
            with_cache_uses(&format!("Swatinem/rust-cache/sub@{CACHE_SHA}")),
            ACTION_UNPINNED,
        ),
        (
            "a workflow path at a reviewed SHA",
            with_cache_uses(&format!(
                "Swatinem/rust-cache/.github/workflows/ci.yml@{CACHE_SHA}"
            )),
            ACTION_UNPINNED,
        ),
        (
            "a docker image by tag",
            with_cache_uses("docker://alpine:3.20"),
            ACTION_UNPINNED,
        ),
        (
            "a docker image by digest",
            with_cache_uses(&format!("docker://alpine@sha256:{digest}")),
            ACTION_UNPINNED,
        ),
        (
            "a docker image named like a reviewed pin",
            with_cache_uses(&format!("docker://Swatinem/rust-cache@{CACHE_SHA}")),
            ACTION_UNPINNED,
        ),
        (
            "a local action",
            with_cache_uses("./.github/actions/cache"),
            ACTION_UNPINNED,
        ),
        (
            "a local action at a SHA",
            with_cache_uses(&format!("./cache@{CACHE_SHA}")),
            ACTION_UNPINNED,
        ),
        (
            "a parent-relative action at a SHA",
            with_cache_uses(&format!("../cache@{CACHE_SHA}")),
            ACTION_UNPINNED,
        ),
        (
            "an empty owner at a SHA",
            with_cache_uses(&format!("/rust-cache@{CACHE_SHA}")),
            ACTION_UNPINNED,
        ),
    ]);
}

/// Acceptance item 1: a job-level `uses` (a reusable workflow) is judged as a
/// step's is; GitHub's own are not.
#[test]
fn lib_qa_009_a_job_level_uses_is_judged() {
    assert_pin_controls(&[
        (
            "a reusable workflow at a tag",
            with_called_workflow(&["  uses: octo-org/workflows/.github/workflows/ci.yml@v1"]),
            ACTION_UNPINNED,
        ),
        (
            "a reusable workflow at a full SHA",
            with_called_workflow(&[&format!(
                "  uses: octo-org/workflows/.github/workflows/ci.yml@{CACHE_SHA}"
            )]),
            ACTION_UNPINNED,
        ),
        (
            "a local reusable workflow",
            with_called_workflow(&["  uses: ./.github/workflows/ci.yml"]),
            ACTION_UNPINNED,
        ),
        (
            "a job-level `uses` of a reviewed action at a tag",
            with_called_workflow(&["  uses: Swatinem/rust-cache@v2"]),
            ACTION_UNPINNED,
        ),
        (
            "a job-level `uses` that is not a string",
            with_called_workflow(&["  uses: [octo-org/workflows]"]),
            SHAPE,
        ),
        (
            "a job-level `with` that is not a mapping",
            with_called_workflow(&[
                "  uses: actions/reusable/.github/workflows/ci.yml@v1",
                "  with: shared",
            ]),
            SHAPE,
        ),
    ]);
    assert_yaml_accepted(
        QA9,
        &[(
            "a reusable workflow of GitHub's `actions`",
            with_called_workflow(&["  uses: actions/reusable/.github/workflows/ci.yml@v1"]),
        )],
        pins_check,
    );
}

/// Acceptance items 1 and 4: only the owner `actions`, in any case, is not
/// judged; `actions-rs` and other look-alikes are third-party.
#[test]
fn lib_qa_009_only_github_s_actions_owner_is_not_judged() {
    assert_pin_controls(&[
        (
            "an `actions-rs` action at a tag",
            with_cache_uses("actions-rs/toolchain@v1"),
            ACTION_UNPINNED,
        ),
        (
            "an `actions-rs` action at a SHA",
            with_cache_uses(&format!("actions-rs/toolchain@{CACHE_SHA}")),
            ACTION_UNREVIEWED,
        ),
        (
            "an owner that starts with `actions`",
            with_cache_uses("actionsx/checkout@v7"),
            ACTION_UNPINNED,
        ),
        (
            "the owner `action`",
            with_cache_uses("action/checkout@v7"),
            ACTION_UNPINNED,
        ),
        (
            "`actions` with no repository",
            with_cache_uses("actions@v7"),
            ACTION_UNPINNED,
        ),
    ]);
    let upload = |uses: &str| replace_line(PINS_FIXTURE, PIN_UPLOAD, &format!("- uses: {uses}"));
    assert_yaml_accepted(
        QA9,
        &[
            ("`actions/checkout` at a tag", upload("actions/checkout@v7")),
            ("`Actions/` at a tag", upload("Actions/upload-artifact@v7")),
            (
                "`ACTIONS/` at a branch",
                upload("ACTIONS/upload-artifact@main"),
            ),
            (
                "a sub-path of GitHub's `actions`",
                upload("actions/upload-artifact/merge@v7"),
            ),
            (
                "an input of GitHub's `actions` that no reviewed action has",
                insert_after(PINS_FIXTURE, "path: out", 0, &["credentials: anything"]),
            ),
        ],
        pins_check,
    );
}

/// Acceptance item 2: a pinned pair that the table does not list is
/// `[action-unreviewed]`, and its inputs are not judged.
#[test]
fn lib_qa_009_a_pin_missing_from_the_reviewed_table_is_unreviewed() {
    let zeros = "0".repeat(40);
    assert_pin_controls(&[
        (
            "a reviewed action at an unreviewed SHA",
            with_cache_uses(&format!("Swatinem/rust-cache@{zeros}")),
            ACTION_UNREVIEWED,
        ),
        (
            "a reviewed action at another reviewed action's SHA",
            with_cache_uses(&format!("Swatinem/rust-cache@{DENY_SHA}")),
            ACTION_UNREVIEWED,
        ),
        (
            "a reviewed action at an unreviewed SHA, with inputs",
            with_toolchain_ref(&zeros),
            ACTION_UNREVIEWED,
        ),
        (
            "an unknown action at a reviewed SHA",
            with_cache_uses(&format!("octo-org/octo-action@{CACHE_SHA}")),
            ACTION_UNREVIEWED,
        ),
        (
            "an unknown action",
            with_cache_uses("octo-org/octo-action@0123456789abcdef0123456789abcdef01234567"),
            ACTION_UNREVIEWED,
        ),
        (
            "a fork of a reviewed action at its SHA",
            with_cache_uses(&format!("Swatinem-fork/rust-cache@{CACHE_SHA}")),
            ACTION_UNREVIEWED,
        ),
    ]);
}

/// Acceptance item 2: the `[action-unreviewed]` problem names the action and
/// the SHA, says to review the action's `action.yml` at that SHA, and names
/// the file and step.
#[test]
fn lib_qa_009_an_unreviewed_pin_names_the_action_the_sha_and_action_yml() {
    let other = "0123456789abcdef0123456789abcdef01234567";
    let text = with_cache_uses(&format!("Swatinem/rust-cache@{other}"));
    let problems = pins_check(&text);
    assert_eq!(
        problem_codes(&problems),
        ["action-unreviewed"],
        "LIB-QA-009: one unreviewed pin gives one problem: {problems:#?}"
    );
    for part in [
        "Swatinem/rust-cache",
        other,
        "review",
        "action.yml",
        "pins.yml",
        "job `build`, step 3",
    ] {
        assert!(
            problems[0].contains(part),
            "LIB-QA-009: the problem must contain `{part}`: {}",
            problems[0]
        );
    }
}

/// Acceptance item 3: a `with` key outside the entry's inputs, in any case,
/// is `[action-input]`, once per key.
#[test]
fn lib_qa_009_an_input_outside_the_reviewed_inputs_is_reported() {
    let mut controls: Vec<(&str, String, &[&str])> = [
        "rust-version: \"1.85\"",
        "credentials: token",
        "ssh-key: key",
        "ssh-known-hosts: hosts",
        "use-git-cli: true",
        "Credentials: token",
        "RUST-VERSION: \"1.85\"",
        "exclude: sacm-cli",
        "1: one",
    ]
    .into_iter()
    .map(|line| {
        (
            "a cargo-deny input",
            with_deny_inputs(&[line]),
            ACTION_INPUT,
        )
    })
    .collect();
    controls.extend([
        (
            "two unreviewed cargo-deny inputs",
            with_deny_inputs(&["credentials: token", "ssh-key: key"]),
            ACTION_INPUT_TWICE,
        ),
        (
            "`targets` for rust-toolchain",
            with_toolchain_input("targets: wasm32-unknown-unknown"),
            ACTION_INPUT,
        ),
        (
            "`Targets` for rust-toolchain",
            with_toolchain_input("Targets: wasm32-unknown-unknown"),
            ACTION_INPUT,
        ),
        (
            "`override` for rust-toolchain",
            with_toolchain_input("override: true"),
            ACTION_INPUT,
        ),
        (
            // Verification 0009 F9-2: inputs are compared whole, not as
            // prefixes in either direction.
            "`toolchain-file` for rust-toolchain, a reviewed input as a prefix",
            with_toolchain_input("toolchain-file: rust-toolchain.toml"),
            ACTION_INPUT,
        ),
        (
            "`components-extra` for rust-toolchain, a reviewed input as a prefix",
            with_toolchain_input("components-extra: x"),
            ACTION_INPUT,
        ),
        (
            "`tool` for rust-toolchain, a prefix of a reviewed input",
            with_toolchain_input("tool: x"),
            ACTION_INPUT,
        ),
        (
            "an input for rust-cache",
            with_cache_inputs(&["shared-key: ci"]),
            ACTION_INPUT,
        ),
        (
            "`cache-on-failure` for rust-cache",
            with_cache_inputs(&["cache-on-failure: true"]),
            ACTION_INPUT,
        ),
    ]);
    assert_pin_controls(&controls);
}

/// Acceptance item 3 (positive control): the reviewed inputs are accepted in
/// any case, as are equivalent spellings of a reviewed pin and the pinned
/// form of the CI gate jobs.
#[test]
fn lib_qa_009_reviewed_inputs_in_any_case_and_equivalent_pins_are_accepted() {
    let deny_in_other_cases = [
        ("command: check", "Command: check"),
        ("arguments: --all-features", "ARGUMENTS: --all-features"),
        ("manifest-path: Cargo.toml", "Manifest-Path: Cargo.toml"),
        (PIN_LOG_LEVEL, "Log-Level: warn"),
    ]
    .into_iter()
    .fold(PINS_FIXTURE.to_owned(), |text, (from, to)| {
        replace_line(&text, from, to)
    });
    let deny_in_other_cases = insert_after(
        &deny_in_other_cases,
        "Log-Level: warn",
        0,
        &["COMMAND-ARGUMENTS: --hide-inclusion-graph"],
    );
    assert_yaml_accepted(
        QA9,
        &[
            (
                "the rust-toolchain inputs in other cases",
                replace_line(
                    &replace_line(PINS_FIXTURE, PIN_TOOLCHAIN_INPUT, "Toolchain: stable"),
                    PIN_COMPONENTS,
                    "COMPONENTS: clippy",
                ),
            ),
            ("the cargo-deny inputs in other cases", deny_in_other_cases),
            (
                "every reviewed cargo-deny input",
                with_deny_inputs(&["command-arguments: --hide-inclusion-graph"]),
            ),
            (
                "the action name in another case",
                with_cache_uses(&format!("swatinem/Rust-Cache@{CACHE_SHA}")),
            ),
            (
                "a quoted pin with no comment",
                with_cache_uses(&format!("\"Swatinem/rust-cache@{CACHE_SHA}\"")),
            ),
            (
                "a step as a flow mapping",
                replace_line(
                    PINS_FIXTURE,
                    PIN_CACHE,
                    &format!("- {{ uses: Swatinem/rust-cache@{CACHE_SHA} }}"),
                ),
            ),
            (
                "an empty `with`",
                insert_after(PINS_FIXTURE, PIN_CACHE, 2, &["with: {}"]),
            ),
            ("the CI gate jobs, pinned", CI_GATES_FIXTURE.to_owned()),
        ],
        pins_check,
    );
}

/// The shared `[shape]` problems: a `with` or `uses` of the wrong type, or a
/// workflow that is not a mapping.
#[test]
fn lib_qa_009_a_with_or_uses_of_the_wrong_type_is_a_shape_problem() {
    assert_pin_controls(&[
        (
            "`with` a string on a reviewed action",
            insert_after(PINS_FIXTURE, PIN_CACHE, 2, &["with: shared-key"]),
            SHAPE,
        ),
        (
            "`with` a sequence on a reviewed action",
            insert_after(PINS_FIXTURE, PIN_CACHE, 2, &["with: [shared-key]"]),
            SHAPE,
        ),
        (
            "`uses` a sequence",
            replace_line(
                PINS_FIXTURE,
                PIN_CACHE,
                &format!("- uses: [Swatinem/rust-cache@{CACHE_SHA}]"),
            ),
            SHAPE,
        ),
        (
            "the workflow wrapped in a sequence",
            wrapped_in_sequence(PINS_FIXTURE),
            SHAPE,
        ),
    ]);
}

/// The shared `[yaml-parse]`, `[yaml-documents]` and `[merge-key]` problems.
#[test]
fn lib_qa_009_parse_and_merge_key_problems_are_reported() {
    assert_pin_controls(&[
        (
            "an unterminated flow sequence",
            replace_line(PINS_FIXTURE, "contents: read", "contents: [read"),
            YAML_PARSE,
        ),
        (
            "a duplicate key",
            insert_after(PINS_FIXTURE, "build:", 2, &["runs-on: windows-latest"]),
            YAML_PARSE,
        ),
        (
            "a second document",
            format!("{PINS_FIXTURE}---\nname: second\n"),
            YAML_DOCUMENTS,
        ),
        ("a merge key", with_merge_key_job(PINS_FIXTURE), MERGE_KEY),
    ]);
}

/// Acceptance item 1: every workflow given is judged, and each problem names
/// its file.
#[test]
fn lib_qa_009_each_workflow_is_judged_and_named() {
    let bad = with_cache_uses("Swatinem/rust-cache@v2");
    let problems = check_action_pins(&[("good.yml", PINS_FIXTURE), ("bad.yml", &bad)]);
    assert_eq!(
        problem_codes(&problems),
        ["action-unpinned"],
        "LIB-QA-009: only the second workflow has a problem: {problems:#?}"
    );
    assert!(
        problems[0].contains("bad.yml") && problems[0].contains("Swatinem/rust-cache@v2"),
        "LIB-QA-009: the problem must name the file and the `uses`: {}",
        problems[0]
    );
    let unparsable = check_action_pins(&[("broken.yml", "a: [\n"), ("bad.yml", &bad)]);
    assert_eq!(
        problem_codes(&unparsable),
        ["action-unpinned", "yaml-parse"],
        "LIB-QA-009: a workflow that does not parse does not stop the others being judged: {unparsable:#?}"
    );
    assert!(
        unparsable.iter().any(|p| p.contains("broken.yml")),
        "LIB-QA-009: the parse problem must name its file: {unparsable:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-QA-008: CI gate checks match live steps, not text
// ---------------------------------------------------------------------------
//
// Each gate of LIB-QA-001 must be some step's whole `run` value in the
// parsed `ci.yml` (`[gate-missing]`). A step whose `run` equals a gate is
// that gate's step wherever it is, so its job is judged: it is neither
// conditional nor allowed to fail (`[job-key]`), no step of it is guarded
// (`[step-guard]`), every `run` step of it is a gate (`[gate-step]`), and
// the gate step has nothing beside its `run` (`[step-key]`). The environment (`[gate-env]`), the OS matrix of the
// `cargo test` gates (`[gate-os]`), the triggers (`[gate-trigger]`) and a
// top-level `defaults` (`[workflow-defaults]`) cannot weaken a gate. The
// cargo-deny gate is LIB-QA-003's deny-job check, whose problems are merged.

const GATE_FMT: &str = "cargo fmt --all -- --check";
const GATE_CLIPPY: &str = "cargo clippy --workspace --all-targets --all-features -- -D warnings";
const GATE_TEST: &str = "cargo test --workspace --all-features";
const GATE_TEST_XTASK: &str = "cargo test -p xtask";
const GATE_DOC: &str = "cargo doc --workspace --no-deps --all-features";
const GATE_MATRIX: &str = "cargo xtask matrix check";
const GATE_HOOKS: &str = r#"python -m unittest discover -s .claude/hooks -p "test_*.py" -v"#;

/// Acceptance item 1: the gates, each an exact `run` value.
const GATES: [&str; 7] = [
    GATE_FMT,
    GATE_CLIPPY,
    GATE_TEST,
    GATE_TEST_XTASK,
    GATE_DOC,
    GATE_MATRIX,
    GATE_HOOKS,
];

/// Acceptance item 4: the gates that run on every OS.
const TEST_GATES: [&str; 2] = [GATE_TEST, GATE_TEST_XTASK];

/// Acceptance item 2: the keys of the job that runs a `cargo test` gate:
/// [`JOB_KEYS`] and `strategy`.
const TEST_JOB_KEYS: [&str; 6] = ["name", "runs-on", "steps", "env", "permissions", "strategy"];

/// Acceptance item 2: the keys a gate step may have.
const GATE_STEP_KEYS: [&str; 3] = ["run", "name", "id"];

/// Acceptance item 2: keys no step of a gate job may have.
const GATE_STEP_GUARDS: [&str; 4] = ["if", "continue-on-error", "shell", "working-directory"];

/// Acceptance item 3: the variables the workflow and a gate job may set.
const GATE_ENV_KEYS: [&str; 3] = ["CARGO_TERM_COLOR", "CARGO_INCREMENTAL", "RUSTDOCFLAGS"];

/// Acceptance item 3: the one value of `RUSTDOCFLAGS`.
const RUSTDOCFLAGS_VALUE: &str = "-D warnings";

/// Acceptance item 4: the runner of the `cargo test` gates, the keys of its
/// `strategy`, and the OSes of its matrix, in order.
const MATRIX_OS: &str = "${{ matrix.os }}";
const GATE_STRATEGY_KEYS: [&str; 2] = ["fail-fast", "matrix"];
const GATE_OSES: [&str; 3] = ["ubuntu-latest", "windows-latest", "macos-latest"];

/// The gate whose command is `step`'s `run` value, if any.
fn gate_of(step: &Hash) -> Option<&'static str> {
    let run = get(step, "run")?.as_str()?;
    GATES.into_iter().find(|gate| *gate == run)
}

/// Acceptance item 3: an `env` (of the workflow or of a gate job) sets only
/// [`GATE_ENV_KEYS`], and `RUSTDOCFLAGS`, wherever it is set, is exactly
/// [`RUSTDOCFLAGS_VALUE`]. A null `env` sets nothing.
fn gate_env_problems(env: Option<&Yaml>, what: &str, problems: &mut Vec<String>) {
    let env = match env {
        None | Some(Yaml::Null) => return,
        Some(Yaml::Hash(env)) => env,
        Some(other) => {
            problems.push(problem(
                "gate-env",
                format!("the `env` of {what} is not a mapping: {other:?}"),
            ));
            return;
        }
    };
    key_allowlist(
        env,
        &GATE_ENV_KEYS,
        "gate-env",
        &format!("the `env` of {what}"),
        problems,
    );
    if let Some(value) = get(env, "RUSTDOCFLAGS").filter(|v| v.as_str() != Some(RUSTDOCFLAGS_VALUE))
    {
        problems.push(problem(
            "gate-env",
            format!(
                "the `env` of {what} sets `RUSTDOCFLAGS` to `{}`; it must be exactly `{RUSTDOCFLAGS_VALUE}`",
                show(value)
            ),
        ));
    }
}

/// The `RUSTDOCFLAGS` a step of `job` runs with: the job's `env` takes
/// precedence over the workflow's. (A gate step has no `env` of its own.)
fn effective_rustdocflags<'a>(top: &'a Hash, job: &Job<'a>) -> Option<&'a Yaml> {
    let from = |map: &'a Hash| {
        get(map, "env")
            .and_then(Yaml::as_hash)
            .and_then(|env| get(env, "RUSTDOCFLAGS"))
    };
    from(job.map).or_else(|| from(top))
}

/// Acceptance item 5: `on.push` is exactly `branches: [main]`, and
/// `on.pull_request` has no filter (null or an empty mapping). Other triggers
/// are allowed.
fn gate_trigger_problems(top: &Hash, problems: &mut Vec<String>) {
    let on = triggers(top);
    let main_only = Yaml::Array(vec![Yaml::String("main".to_owned())]);
    match get(&on, "push") {
        Some(Yaml::Hash(push)) if push.len() == 1 && get(push, "branches") == Some(&main_only) => {}
        None => problems.push(problem(
            "gate-trigger",
            "the workflow does not run on `push`",
        )),
        Some(other) => problems.push(problem(
            "gate-trigger",
            format!("`on.push` must be exactly `branches: [main]`, found {other:?}"),
        )),
    }
    match get(&on, "pull_request") {
        Some(Yaml::Null) => {}
        Some(Yaml::Hash(filters)) if filters.is_empty() => {}
        None => problems.push(problem(
            "gate-trigger",
            "the workflow does not run on `pull_request`",
        )),
        Some(other) => problems.push(problem(
            "gate-trigger",
            format!(
                "`on.pull_request` must have no filter, so the gates run on every pull request; found {other:?}"
            ),
        )),
    }
}

/// Acceptance item 4: the job running a `cargo test` gate runs on
/// `${{ matrix.os }}`, and its `strategy` has only a boolean `fail-fast` and
/// a `matrix` of only `os`, exactly [`GATE_OSES`].
fn gate_os_problems(job: &Job<'_>, problems: &mut Vec<String>) {
    let what = format!("the `cargo test` job `{}`", job.id);
    let runs_on = get(job.map, "runs-on");
    if runs_on.and_then(Yaml::as_str) != Some(MATRIX_OS) {
        problems.push(problem(
            "gate-os",
            format!("{what} must have `runs-on: {MATRIX_OS}`, found {runs_on:?}"),
        ));
    }
    let Some(Yaml::Hash(strategy)) = get(job.map, "strategy") else {
        problems.push(problem(
            "gate-os",
            format!("{what} must have a `strategy` mapping with the OS matrix"),
        ));
        return;
    };
    let in_strategy = format!("the `strategy` of {what}");
    key_allowlist(
        strategy,
        &GATE_STRATEGY_KEYS,
        "gate-os",
        &in_strategy,
        problems,
    );
    if let Some(fail_fast) = get(strategy, "fail-fast").filter(|v| !matches!(v, Yaml::Boolean(_))) {
        problems.push(problem(
            "gate-os",
            format!("{in_strategy}: `fail-fast` must be a boolean, found {fail_fast:?}"),
        ));
    }
    let Some(Yaml::Hash(matrix)) = get(strategy, "matrix") else {
        problems.push(problem(
            "gate-os",
            format!("{in_strategy} must have a `matrix` mapping"),
        ));
        return;
    };
    key_allowlist(
        matrix,
        &["os"],
        "gate-os",
        &format!("the matrix of {what}"),
        problems,
    );
    let oses = Yaml::Array(
        GATE_OSES
            .iter()
            .map(|os| Yaml::String((*os).to_owned()))
            .collect(),
    );
    let os = get(matrix, "os");
    if os != Some(&oses) {
        problems.push(problem(
            "gate-os",
            format!("the matrix of {what} must have `os: {GATE_OSES:?}`, found {os:?}"),
        ));
    }
}

/// Acceptance item 2: every `run` step of a gate job (the cargo-deny job
/// included) is a gate, so no other step can write `$GITHUB_ENV` or
/// `$GITHUB_PATH` for the gates (`[gate-step]`). Steps with `uses` are judged
/// by [`gate_uses_problems`]. A `run` that is not a
/// string is a `[shape]` problem reported by [`read_workflow`].
fn gate_step_problems(job: &Job<'_>, problems: &mut Vec<String>) {
    for (number, step) in &job.steps {
        let Some(run) = get(step, "run").and_then(Yaml::as_str) else {
            continue;
        };
        if gate_of(step).is_none() {
            problems.push(problem(
                "gate-step",
                format!(
                    "job `{}`, step {number} runs `{run}`, which is not a gate; every `run` step of a gate job must be a gate, so that no other step can write `$GITHUB_ENV` or `$GITHUB_PATH`",
                    job.id
                ),
            ));
        }
    }
}

/// Acceptance item 2: GitHub's own actions a gate job may use, at any ref
/// (Dependabot moves their major tags), with the inputs it may pass them.
static GATE_GITHUB_ACTIONS: [(&str, &[&str]); 2] = [
    ("actions/checkout", &[]),
    ("actions/setup-python", &["python-version"]),
];

/// The inputs a gate job may pass to the action `uses` names, or `None` if
/// it is not a gate action. The action is its `owner/repo`, compared without
/// regard to case, at any ref: GitHub's own from [`GATE_GITHUB_ACTIONS`], the
/// third-party ones from LIB-QA-009's [`REVIEWED_ACTIONS`] (the entry at that
/// SHA if there is one). Pinning is LIB-QA-009's to judge, not this check's.
/// A sub-path, a `docker://` image or a `./` action is never a gate action.
fn gate_action_inputs(uses: &str) -> Option<&'static [&'static str]> {
    let (action, reference) = uses.split_once('@').unwrap_or((uses, ""));
    if let Some(entry) = reviewed_action(action, reference) {
        return Some(entry.inputs);
    }
    GATE_GITHUB_ACTIONS
        .iter()
        .copied()
        .chain(
            REVIEWED_ACTIONS
                .iter()
                .map(|entry| (entry.action, entry.inputs)),
        )
        .find(|(name, _)| name.eq_ignore_ascii_case(action))
        .map(|(_, inputs)| inputs)
}

/// Acceptance item 2: every `uses` step of a gate job (the cargo-deny job
/// included) names a gate action and passes it only its gate inputs,
/// compared without regard to case (`[gate-action]`). No step of a gate job,
/// the gate step included, has `env` (`[gate-env]`), so `BASH_ENV` cannot run
/// a script inside an action. A `uses` that is not a string, or a `with` that
/// is not a mapping, is a `[shape]` problem reported by [`read_workflow`].
fn gate_uses_problems(job: &Job<'_>, problems: &mut Vec<String>) {
    for (number, step) in &job.steps {
        let at = format!("job `{}`, step {number}", job.id);
        if get(step, "env").is_some() {
            problems.push(problem(
                "gate-env",
                format!(
                    "{at} has a step-level `env`; no step of a gate job may set variables, so that `BASH_ENV` cannot run a script inside an action"
                ),
            ));
        }
        let Some(uses) = get(step, "uses").and_then(Yaml::as_str) else {
            continue;
        };
        let Some(inputs) = gate_action_inputs(uses) else {
            problems.push(problem(
                "gate-action",
                format!(
                    "{at} uses `{uses}`, which is not a gate action; a gate job may use only `actions/checkout`, `actions/setup-python` and the reviewed actions of LIB-QA-009"
                ),
            ));
            continue;
        };
        let Some(Yaml::Hash(with)) = get(step, "with") else {
            continue;
        };
        for key in with.keys() {
            let allowed = key
                .as_str()
                .is_some_and(|key| inputs.iter().any(|input| input.eq_ignore_ascii_case(key)));
            if !allowed {
                problems.push(problem(
                    "gate-action",
                    format!(
                        "{at} passes `{}` to `{uses}`, which is not one of its gate inputs {inputs:?}",
                        show(key)
                    ),
                ));
            }
        }
    }
}

/// Acceptance items 2 to 4 for one job, if it runs a gate. The cargo-deny job
/// is judged by [`check_deny_job`], and here only by items 2 (`[gate-step]`,
/// `[gate-action]` and step `env`) and 3 (its `env`).
fn gate_job_problems(top: &Hash, job: &Job<'_>, problems: &mut Vec<String>) {
    let gates: Vec<(usize, &Hash, &str)> = job
        .steps
        .iter()
        .filter_map(|(number, step)| gate_of(step).map(|gate| (*number, *step, gate)))
        .collect();
    if gates.is_empty() {
        if job.steps.iter().any(|(_, step)| is_deny_step(step)) {
            gate_step_problems(job, problems);
            gate_uses_problems(job, problems);
            let what = format!("the cargo-deny job `{}`", job.id);
            gate_env_problems(get(job.map, "env"), &what, problems);
        }
        return;
    }
    gate_step_problems(job, problems);
    gate_uses_problems(job, problems);
    let runs_tests = gates.iter().any(|(_, _, gate)| TEST_GATES.contains(gate));
    let keys: &[&str] = if runs_tests {
        &TEST_JOB_KEYS
    } else {
        &JOB_KEYS
    };
    let what = format!("the gate job `{}`", job.id);
    // Item 2: the job, its steps and its gate steps.
    key_allowlist(job.map, keys, "job-key", &what, problems);
    guarded_step_problems(job, &GATE_STEP_GUARDS, |_, _| false, problems);
    for (number, step, gate) in &gates {
        key_allowlist(
            step,
            &GATE_STEP_KEYS,
            "step-key",
            &format!("job `{}`, step {number} (the gate `{gate}`)", job.id),
            problems,
        );
    }
    // Item 3: the job's `env`, and the `RUSTDOCFLAGS` `cargo doc` runs with.
    gate_env_problems(get(job.map, "env"), &what, problems);
    if gates.iter().any(|(_, _, gate)| *gate == GATE_DOC)
        && effective_rustdocflags(top, job).is_none()
    {
        problems.push(problem(
            "gate-env",
            format!(
                "`{GATE_DOC}` in job `{}` runs without `RUSTDOCFLAGS: {RUSTDOCFLAGS_VALUE}` in its job's or the workflow's `env`",
                job.id
            ),
        ));
    }
    // Item 4: the OS matrix.
    if runs_tests {
        gate_os_problems(job, problems);
    }
}

/// LIB-QA-008: every gate is a live step of `ci`, and nothing in the
/// workflow weakens it. The cargo-deny gate is [`check_deny_job`], whose
/// problems are merged; a problem both checks find (`[merge-key]`,
/// `[shape]`) is reported once.
fn check_ci_gates(ci: &str) -> Vec<String> {
    let doc = match load_workflow(ci) {
        Ok(doc) => doc,
        Err(parse) => return vec![parse],
    };
    let mut problems = Vec::new();
    merge_key_problems(&doc, "ci.yml", &mut problems);
    if let Some(workflow) = read_workflow(&doc, &mut problems) {
        let top = workflow.top;
        // Item 2: a top-level `defaults` can change every step's shell.
        workflow_defaults_problems(top, &mut problems);
        // Items 3 and 5: the workflow's `env` and triggers.
        gate_env_problems(get(top, "env"), "the workflow", &mut problems);
        gate_trigger_problems(top, &mut problems);
        // Item 1: each gate is some step's whole `run` value.
        for gate in GATES {
            let live = workflow.jobs.iter().any(|job| {
                job.steps
                    .iter()
                    .any(|(_, step)| gate_of(step) == Some(gate))
            });
            if !live {
                problems.push(problem(
                    "gate-missing",
                    format!("no step of ci.yml has a `run` value exactly `{gate}`"),
                ));
            }
        }
        // Items 2 to 4: every job that runs a gate.
        for job in &workflow.jobs {
            gate_job_problems(top, job, &mut problems);
        }
    }
    // Item 6: the cargo-deny gate.
    for deny in check_deny_job(ci) {
        if !problems.contains(&deny) {
            problems.push(deny);
        }
    }
    problems
}

/// A copy of the gate jobs of `ci.yml`, in the pinned form slice 0009 gives
/// the third-party actions. It must pass.
const CI_GATES_FIXTURE: &str = r#"name: CI

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: 0

jobs:
  fmt:
    name: rustfmt
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8 # master
        with:
          toolchain: stable
          components: rustfmt
      - run: cargo fmt --all -- --check

  clippy:
    name: clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8 # master
        with:
          toolchain: stable
          components: clippy
      - uses: Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0 # v2.9.3
      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings

  test:
    name: test (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8 # master
        with:
          toolchain: stable
      - uses: Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0 # v2.9.3
      - run: cargo test --workspace --all-features
      - run: cargo test -p xtask

  docs:
    name: rustdoc
    runs-on: ubuntu-latest
    env:
      RUSTDOCFLAGS: -D warnings
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8 # master
        with:
          toolchain: stable
      - uses: Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0 # v2.9.3
      - run: cargo doc --workspace --no-deps --all-features

  functionality:
    name: functionality matrix
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8 # master
        with:
          toolchain: stable
      - uses: Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0 # v2.9.3
      - run: cargo xtask matrix check

  deny:
    name: cargo-deny
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25 # v2.1.1

  hooks:
    name: agent hook tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: actions/setup-python@v7
        with:
          python-version: "3.x"
      - run: python -m unittest discover -s .claude/hooks -p "test_*.py" -v
"#;

// Lines (trimmed) of `CI_GATES_FIXTURE` that the controls edit.
const RUSTDOCFLAGS_LINE: &str = "RUSTDOCFLAGS: -D warnings";
const OS_LINE: &str = "os: [ubuntu-latest, windows-latest, macos-latest]";
const FAIL_FAST_LINE: &str = "fail-fast: false";
const MATRIX_RUNS_ON: &str = "runs-on: ${{ matrix.os }}";
const PUSH_BRANCHES: &str = "branches: [main]";
const WORKFLOW_ENV_LAST: &str = "CARGO_INCREMENTAL: 0";
const SETUP_PYTHON_STEP: &str = "- uses: actions/setup-python@v7";

const QA8: &str = "LIB-QA-008";

/// The fixture line of `gate`'s step.
fn gate_step(gate: &str) -> String {
    format!("- run: {gate}")
}

fn assert_gate_controls(controls: &[(&str, String, &[&str])]) {
    assert_exact_controls(QA8, CI_GATES_FIXTURE, controls, check_ci_gates);
}

/// `CI_GATES_FIXTURE` with `lines` inserted as keys of the job `id`.
fn with_job_keys(id: &str, lines: &[&str]) -> String {
    insert_after(CI_GATES_FIXTURE, &format!("{id}:"), 2, lines)
}

/// `CI_GATES_FIXTURE` with the step of `gate` moved to just after the step
/// of `after`.
fn with_gate_moved(gate: &str, after: &str) -> String {
    let step = gate_step(gate);
    insert_after(
        &delete_line(CI_GATES_FIXTURE, &step),
        &gate_step(after),
        0,
        &[&step],
    )
}

/// Acceptance item 6: the real `ci.yml` passes. This pins behaviour that
/// exists, and does not depend on the text of any `uses`.
#[test]
fn lib_qa_008_the_real_ci_runs_every_gate_as_a_live_step() {
    let problems = check_ci_gates(&read(CI_WORKFLOW));
    assert!(
        problems.is_empty(),
        "LIB-QA-008: every CI gate must be a live, unconditional step that nothing weakens: {problems:#?}"
    );
}

/// Acceptance items 1 and 6: a gate commented out, wrapped in `echo`, given
/// `|| true`, folded into a multi-line `run`, or spelled differently is
/// `[gate-missing]`, and the problem names the gate.
#[test]
fn lib_qa_008_each_gate_must_be_a_whole_run_value() {
    let f = CI_GATES_FIXTURE;
    for gate in GATES {
        let text = comment_line(f, &gate_step(gate));
        let problems = check_ci_gates(&text);
        assert_eq!(
            problem_codes(&problems),
            ["gate-missing"],
            "LIB-QA-008: the gate `{gate}` commented out must give exactly `[gate-missing]`: {problems:#?}"
        );
        assert!(
            problems[0].contains(gate),
            "LIB-QA-008: the problem must name the gate `{gate}`: {}",
            problems[0]
        );
    }
    let fmt = gate_step(GATE_FMT);
    let clippy = gate_step(GATE_CLIPPY);
    assert_gate_controls(&[
        ("the gate deleted", delete_line(f, &fmt), GATE_MISSING),
        (
            "the gate wrapped in `echo`",
            replace_line(f, &fmt, &format!("- run: echo {GATE_FMT}")),
            GATE_MISSING,
        ),
        (
            "the gate given `|| true`",
            replace_line(f, &fmt, &format!("- run: {GATE_FMT} || true")),
            GATE_MISSING,
        ),
        (
            "the gate given `; true`",
            replace_line(f, &fmt, &format!("- run: {GATE_FMT}; true")),
            GATE_MISSING,
        ),
        (
            "the gate folded into a multi-line `run`",
            replace_lines(
                f,
                &fmt,
                &["- run: |", &format!("    {GATE_FMT}"), "    true"],
            ),
            GATE_MISSING,
        ),
        (
            "two gates folded into one multi-line `run`",
            replace_lines(
                &delete_line(f, &clippy),
                &fmt,
                &[
                    "- run: |",
                    &format!("    {GATE_FMT}"),
                    &format!("    {GATE_CLIPPY}"),
                ],
            ),
            GATE_MISSING_TWICE,
        ),
        (
            // The test job still runs `cargo test -p xtask`, so it is a gate
            // job, and its other `run` step is not a gate.
            "a flag dropped",
            replace_line(f, &gate_step(GATE_TEST), "- run: cargo test --workspace"),
            GATE_MISSING_AND_STEP,
        ),
        (
            "the hooks gate without `-v`",
            replace_line(
                f,
                &gate_step(GATE_HOOKS),
                r#"- run: python -m unittest discover -s .claude/hooks -p "test_*.py""#,
            ),
            GATE_MISSING,
        ),
        (
            "a trailing space inside quotes",
            replace_line(f, &fmt, &format!("- run: \"{GATE_FMT} \"")),
            GATE_MISSING,
        ),
        (
            "the gate as a step's `name`",
            replace_line(f, &fmt, &format!("- name: {GATE_FMT}")),
            GATE_MISSING,
        ),
    ]);
}

/// Acceptance items 2 and 6: a job-level `if`, `continue-on-error`, `needs`
/// or any other key outside the allowed ones is `[job-key]`; a gate present
/// only in such a job is reported by that, not as `[gate-missing]`.
#[test]
fn lib_qa_008_a_gate_job_is_unconditional_and_not_allowed_to_fail() {
    assert_gate_controls(&[
        (
            "a job-level `if`",
            with_job_keys("clippy", &["if: false"]),
            JOB_KEY,
        ),
        (
            "a job-level `if` on the test job",
            with_job_keys("test", &["if: github.event_name == 'push'"]),
            JOB_KEY,
        ),
        (
            "a job-level `continue-on-error`",
            with_job_keys("docs", &["continue-on-error: true"]),
            JOB_KEY,
        ),
        ("`needs`", with_job_keys("clippy", &["needs: fmt"]), JOB_KEY),
        (
            "`strategy` on a job with no `cargo test` gate",
            with_job_keys(
                "fmt",
                &["strategy:", "  matrix:", "    rust: [stable, beta]"],
            ),
            JOB_KEY,
        ),
        (
            "`timeout-minutes`",
            with_job_keys("functionality", &["timeout-minutes: 1"]),
            JOB_KEY,
        ),
        (
            "`container`",
            with_job_keys("hooks", &["container: python:2"]),
            JOB_KEY,
        ),
        (
            "a job-level `defaults`",
            with_job_keys("fmt", &["defaults:", "  run:", "    shell: bash"]),
            JOB_KEY,
        ),
        (
            "the hooks gate only in a conditional job",
            with_job_keys("hooks", &["if: github.event_name == 'push'"]),
            JOB_KEY,
        ),
        (
            "a copy of a gate in a conditional job",
            append_job(
                CI_GATES_FIXTURE,
                &[
                    "extra:",
                    "  if: false",
                    "  runs-on: ubuntu-latest",
                    "  steps:",
                    &format!("    {}", gate_step(GATE_FMT)),
                ],
            ),
            JOB_KEY,
        ),
    ]);
}

/// Acceptance items 2 and 6: a step-level `if` or `continue-on-error`, a
/// `shell` or a `working-directory` on any step of a gate job is
/// `[step-guard]`; anything but `run`, `name` and `id` on the gate step is
/// `[step-key]`.
#[test]
fn lib_qa_008_no_step_of_a_gate_job_is_guarded() {
    let fmt = gate_step(GATE_FMT);
    let on_gate = |lines: &[&str]| insert_after(CI_GATES_FIXTURE, &fmt, 2, lines);
    let on_other = |lines: &[&str]| insert_after(CI_GATES_FIXTURE, SETUP_PYTHON_STEP, 2, lines);
    let before_gate = |lines: &[&str]| insert_before(CI_GATES_FIXTURE, &fmt, lines);
    assert_gate_controls(&[
        (
            "a step-level `if` on the gate step",
            on_gate(&["if: false"]),
            STEP_KEY_AND_GUARD,
        ),
        (
            "a step-level `continue-on-error` on the gate step",
            on_gate(&["continue-on-error: true"]),
            STEP_KEY_AND_GUARD,
        ),
        (
            "`shell` on the gate step",
            on_gate(&["shell: bash"]),
            STEP_KEY_AND_GUARD,
        ),
        (
            "`working-directory` on the gate step",
            on_gate(&["working-directory: crates/sacm"]),
            STEP_KEY_AND_GUARD,
        ),
        (
            // `env` is outside the gate step's keys, and no step of a gate
            // job may have `env`.
            "`env` on the gate step",
            on_gate(&["env:", "  RUSTFLAGS: --cap-lints warn"]),
            GATE_ENV_AND_STEP_KEY,
        ),
        (
            "`timeout-minutes` on the gate step",
            on_gate(&["timeout-minutes: 1"]),
            STEP_KEY,
        ),
        (
            "a step-level `if` on another step of a gate job",
            on_other(&["if: false"]),
            STEP_GUARD,
        ),
        (
            "a step-level `continue-on-error` on another step",
            on_other(&["continue-on-error: true"]),
            STEP_GUARD,
        ),
        (
            "`working-directory` on another step",
            before_gate(&["- uses: actions/checkout@v7", "  working-directory: crates"]),
            STEP_GUARD,
        ),
        (
            "`shell` on another step",
            before_gate(&["- uses: actions/checkout@v7", "  shell: bash"]),
            STEP_GUARD,
        ),
    ]);
}

/// Acceptance item 2 (PR #16 review): every `uses` step of a gate job, the
/// cargo-deny job included, names a gate action and passes it only its gate
/// inputs (`[gate-action]`), so no script input can write `$GITHUB_ENV` for
/// the gates. The action is compared without regard to case, at any ref.
#[test]
fn lib_qa_008_a_gate_job_uses_only_gate_actions() {
    let f = CI_GATES_FIXTURE;
    let before_fmt = |lines: &[&str]| insert_before(f, &gate_step(GATE_FMT), lines);
    assert_gate_controls(&[
        (
            "`actions/github-script` with a `script` in the clippy job",
            insert_before(
                f,
                &gate_step(GATE_CLIPPY),
                &[
                    "- uses: actions/github-script@v7",
                    "  with:",
                    "    script: core.exportVariable('RUSTFLAGS', '--cap-lints=allow')",
                ],
            ),
            GATE_ACTION,
        ),
        (
            "an unknown third-party action at a full SHA",
            before_fmt(&["- uses: octo-org/octo-action@0123456789abcdef0123456789abcdef01234567"]),
            GATE_ACTION,
        ),
        (
            "an unknown third-party action at a tag",
            before_fmt(&["- uses: octo-org/octo-action@v1"]),
            GATE_ACTION,
        ),
        (
            "another of GitHub's own actions",
            before_fmt(&["- uses: actions/cache@v4"]),
            GATE_ACTION,
        ),
        (
            "a sub-path of a gate action",
            before_fmt(&["- uses: actions/checkout/sub@v7"]),
            GATE_ACTION,
        ),
        (
            "a docker image",
            before_fmt(&["- uses: docker://alpine:3.20"]),
            GATE_ACTION,
        ),
        (
            "a local action",
            before_fmt(&["- uses: ./.github/actions/setup"]),
            GATE_ACTION,
        ),
    ]);
    assert_gate_controls(&gate_input_controls());
    assert_yaml_accepted(
        QA8,
        &[
            (
                "`Actions/Checkout@v8`",
                before_fmt(&["- uses: Actions/Checkout@v8"]),
            ),
            (
                "`setup-python` with `Python-Version`",
                replace_line(f, "python-version: \"3.x\"", "Python-Version: \"3.x\""),
            ),
            (
                "a rust-toolchain input in another case",
                replace_line(f, "components: rustfmt", "Components: rustfmt"),
            ),
            (
                // Pinning is LIB-QA-009's to judge, not this check's.
                "a reviewed action at a tag",
                before_fmt(&["- uses: Swatinem/rust-cache@v2"]),
            ),
        ],
        check_ci_gates,
    );
}

/// A `with` key outside a gate action's gate inputs, each giving exactly
/// `[gate-action]`.
fn gate_input_controls() -> Vec<(&'static str, String, &'static [&'static str])> {
    let f = CI_GATES_FIXTURE;
    let before_fmt = |lines: &[&str]| insert_before(f, &gate_step(GATE_FMT), lines);
    vec![
        (
            "`actions/checkout` with `fetch-depth`",
            before_fmt(&[
                "- uses: actions/checkout@v7",
                "  with:",
                "    fetch-depth: 0",
            ]),
            GATE_ACTION,
        ),
        (
            "`actions/setup-python` with `cache`",
            insert_after(f, "python-version: \"3.x\"", 0, &["cache: pip"]),
            GATE_ACTION,
        ),
        (
            // Verification 0009 F8-2: gate inputs are compared whole, not as
            // prefixes in either direction.
            "`actions/setup-python` with `python-version-file`",
            insert_after(
                f,
                "python-version: \"3.x\"",
                0,
                &["python-version-file: .python-version"],
            ),
            GATE_ACTION,
        ),
        (
            "`actions/setup-python` with `python`",
            insert_after(f, "python-version: \"3.x\"", 0, &["python: \"3.x\""]),
            GATE_ACTION,
        ),
        (
            // Verification 0009 F8-4.
            "`actions/checkout` with a key that is not a string",
            before_fmt(&["- uses: actions/checkout@v7", "  with: {1: x}"]),
            GATE_ACTION,
        ),
        (
            "`targets` for rust-toolchain",
            insert_after(
                f,
                "components: rustfmt",
                0,
                &["targets: wasm32-unknown-unknown"],
            ),
            GATE_ACTION,
        ),
        (
            "`credentials` for cargo-deny-action",
            insert_after(f, PIN_DENY, 2, &["with:", "  credentials: token"]),
            GATE_ACTION,
        ),
    ]
}

/// Acceptance item 2 (PR #16 review): no step of a gate job, the gate step
/// and the cargo-deny step included, has `env` (`[gate-env]`), so `BASH_ENV`
/// cannot run a script inside an action.
#[test]
fn lib_qa_008_no_step_of_a_gate_job_has_env() {
    let f = CI_GATES_FIXTURE;
    let before_fmt = |lines: &[&str]| insert_before(f, &gate_step(GATE_FMT), lines);
    let toolchain = format!("- uses: {TOOLCHAIN_ACTION}");
    assert_gate_controls(&[
        (
            "`env: BASH_ENV` on the toolchain step",
            insert_after_in_job(f, "fmt", &toolchain, 2, &["env:", "  BASH_ENV: ./x.sh"]),
            GATE_ENV,
        ),
        (
            // Formerly a positive control (with `actions/cache`), before the
            // PR #16 review.
            "a `uses` step with `env` in a gate job",
            before_fmt(&[
                "- uses: actions/checkout@v7",
                "  env:",
                "    PREPARE: \"1\"",
            ]),
            GATE_ENV,
        ),
        (
            "`env` on the checkout step",
            insert_after_in_job(
                f,
                "clippy",
                "- uses: actions/checkout@",
                2,
                &["env:", "  PREPARE: \"1\""],
            ),
            GATE_ENV,
        ),
        (
            // LIB-QA-003's deny-job check also refuses `env` on this step.
            "`env: BASH_ENV` on the cargo-deny step",
            insert_after(f, PIN_DENY, 2, &["env:", "  BASH_ENV: ./x.sh"]),
            GATE_ENV_AND_STEP_KEY,
        ),
    ]);
}

/// Acceptance item 2 (verification of slice 0009's `$GITHUB_ENV` finding):
/// every `run` step of a gate job, the cargo-deny job included, is a gate, so
/// no other step can change the gates' environment or `PATH`.
#[test]
fn lib_qa_008_every_run_step_of_a_gate_job_is_a_gate() {
    let f = CI_GATES_FIXTURE;
    assert_gate_controls(&[
        (
            "a step writing `$GITHUB_ENV` in the clippy job",
            insert_before(
                f,
                &gate_step(GATE_CLIPPY),
                &[r#"- run: echo "RUSTFLAGS=--cap-lints=allow" >> "$GITHUB_ENV""#],
            ),
            GATE_STEP,
        ),
        (
            "a step writing `$GITHUB_PATH` in the test job",
            insert_before(
                f,
                &gate_step(GATE_TEST),
                &[r#"- run: echo "$HOME/shim" >> "$GITHUB_PATH""#],
            ),
            GATE_STEP,
        ),
        (
            "an extra `run` in the hooks job",
            insert_before(f, &gate_step(GATE_HOOKS), &["- run: pip install pytest"]),
            GATE_STEP,
        ),
        (
            "an extra named `run` in the docs job",
            insert_before(
                f,
                &gate_step(GATE_DOC),
                &["- name: prepare", "  run: echo prepare"],
            ),
            GATE_STEP,
        ),
        (
            "an extra `run` in the cargo-deny job",
            insert_after(f, PIN_DENY, 0, &["- run: echo extra"]),
            GATE_STEP,
        ),
    ]);
}

/// Acceptance items 4 and 6: the `cargo test` gates run on exactly the three
/// OSes, with nothing in the matrix that removes or adds a combination.
#[test]
fn lib_qa_008_the_test_gates_run_on_exactly_the_three_oses() {
    let f = CI_GATES_FIXTURE;
    let os = |line: &str| replace_line(f, OS_LINE, line);
    let no_strategy = ["strategy:", FAIL_FAST_LINE, "matrix:", OS_LINE]
        .into_iter()
        .fold(f.to_owned(), |text, line| delete_line(&text, line));
    assert_gate_controls(&[
        (
            "an OS removed",
            os("os: [ubuntu-latest, windows-latest]"),
            GATE_OS,
        ),
        (
            "an OS replaced",
            os("os: [ubuntu-latest, windows-latest, macos-13]"),
            GATE_OS,
        ),
        (
            "an OS added",
            os("os: [ubuntu-latest, windows-latest, macos-latest, ubuntu-22.04]"),
            GATE_OS,
        ),
        (
            "the OSes reordered",
            os("os: [windows-latest, ubuntu-latest, macos-latest]"),
            GATE_OS,
        ),
        (
            "the OSes from an expression",
            os("os: ${{ fromJSON(vars.OSES) }}"),
            GATE_OS,
        ),
        (
            "a matrix `exclude`",
            insert_after(f, OS_LINE, 0, &["exclude:", "  - os: macos-latest"]),
            GATE_OS,
        ),
        (
            "a matrix `include`",
            insert_after(
                f,
                OS_LINE,
                0,
                &[
                    "include:",
                    "  - os: ubuntu-latest",
                    "    experimental: true",
                ],
            ),
            GATE_OS,
        ),
        (
            "another matrix variable",
            insert_after(f, OS_LINE, 0, &["rust: [stable]"]),
            GATE_OS,
        ),
        (
            "`runs-on` a fixed runner",
            replace_line(f, MATRIX_RUNS_ON, "runs-on: ubuntu-latest"),
            GATE_OS,
        ),
        (
            "`runs-on` another matrix variable",
            replace_line(f, MATRIX_RUNS_ON, "runs-on: ${{ matrix.runner }}"),
            GATE_OS,
        ),
        (
            "`fail-fast` a string",
            replace_line(f, FAIL_FAST_LINE, "fail-fast: \"false\""),
            GATE_OS,
        ),
        (
            "`fail-fast` an expression",
            replace_line(
                f,
                FAIL_FAST_LINE,
                "fail-fast: ${{ github.event_name == 'push' }}",
            ),
            GATE_OS,
        ),
        (
            "`max-parallel`",
            insert_after(f, FAIL_FAST_LINE, 0, &["max-parallel: 1"]),
            GATE_OS,
        ),
        ("no `strategy`", no_strategy, GATE_OS),
        (
            "a `cargo test` gate moved out of the OS matrix",
            with_gate_moved(GATE_TEST_XTASK, GATE_FMT),
            GATE_OS_TWICE,
        ),
    ]);
    assert_gate_controls(&strategy_shape_controls());
}

/// Verification 0009 F8-1: a `strategy` or `matrix` that is missing or not a
/// mapping, each giving exactly `[gate-os]`.
fn strategy_shape_controls() -> Vec<(&'static str, String, &'static [&'static str])> {
    let f = CI_GATES_FIXTURE;
    let matrix_from_expression = replace_line(
        &delete_line(f, OS_LINE),
        "matrix:",
        "matrix: ${{ fromJSON(vars.MATRIX) }}",
    );
    let no_matrix = delete_line(&delete_line(f, OS_LINE), "matrix:");
    let strategy_from_expression = replace_line(
        &delete_line(&no_matrix, FAIL_FAST_LINE),
        "strategy:",
        "strategy: ${{ fromJSON(vars.STRATEGY) }}",
    );
    vec![
        (
            "the matrix from an expression",
            matrix_from_expression,
            GATE_OS,
        ),
        (
            "`strategy` with `fail-fast` and no `matrix`",
            no_matrix,
            GATE_OS,
        ),
        (
            "the strategy from an expression",
            strategy_from_expression,
            GATE_OS,
        ),
    ]
}

/// Acceptance items 3 and 6: `RUSTDOCFLAGS` is exactly `-D warnings`
/// wherever it is set, and `cargo doc` runs with it.
#[test]
fn lib_qa_008_rustdocflags_is_exact_and_reaches_cargo_doc() {
    let f = CI_GATES_FIXTURE;
    let removed = delete_line(f, RUSTDOCFLAGS_LINE);
    assert_gate_controls(&[
        (
            "`RUSTDOCFLAGS` changed",
            replace_line(
                f,
                RUSTDOCFLAGS_LINE,
                "RUSTDOCFLAGS: -D warnings --cap-lints allow",
            ),
            GATE_ENV,
        ),
        (
            "`RUSTDOCFLAGS` respelled",
            replace_line(f, RUSTDOCFLAGS_LINE, "RUSTDOCFLAGS: -Dwarnings"),
            GATE_ENV,
        ),
        (
            "`RUSTDOCFLAGS` not a string",
            replace_line(f, RUSTDOCFLAGS_LINE, "RUSTDOCFLAGS: [-D, warnings]"),
            GATE_ENV,
        ),
        (
            "`RUSTDOCFLAGS` removed from the docs job",
            removed.clone(),
            GATE_ENV,
        ),
        (
            "`RUSTDOCFLAGS` moved to another job",
            insert_after(
                &removed,
                "clippy:",
                2,
                &["env:", &format!("  {RUSTDOCFLAGS_LINE}")],
            ),
            GATE_ENV,
        ),
        (
            "`RUSTDOCFLAGS` overridden in the docs job while set correctly in the workflow",
            insert_after(
                &replace_line(f, RUSTDOCFLAGS_LINE, "RUSTDOCFLAGS: \"\""),
                WORKFLOW_ENV_LAST,
                0,
                &[RUSTDOCFLAGS_LINE],
            ),
            GATE_ENV,
        ),
        (
            "a wrong `RUSTDOCFLAGS` in the workflow, which the docs job overrides",
            insert_after(
                f,
                WORKFLOW_ENV_LAST,
                0,
                &["RUSTDOCFLAGS: --cap-lints allow"],
            ),
            GATE_ENV,
        ),
    ]);
}

/// Acceptance items 3 and 6: no other variable, such as an added
/// `RUSTFLAGS`, is set by the workflow or a gate job.
#[test]
fn lib_qa_008_no_other_variable_is_set_for_the_gates() {
    let f = CI_GATES_FIXTURE;
    let in_workflow = |line: &str| insert_after(f, WORKFLOW_ENV_LAST, 0, &[line]);
    let in_docs_job = |line: &str| insert_before(f, RUSTDOCFLAGS_LINE, &[line]);
    assert_gate_controls(&[
        (
            "`RUSTFLAGS` in the workflow",
            in_workflow("RUSTFLAGS: --cap-lints warn"),
            GATE_ENV,
        ),
        (
            "`RUSTFLAGS` in a gate job",
            with_job_keys("clippy", &["env:", "  RUSTFLAGS: --cap-lints warn"]),
            GATE_ENV,
        ),
        (
            "`RUSTFLAGS` beside `RUSTDOCFLAGS` in the docs job",
            in_docs_job("RUSTFLAGS: --cap-lints warn"),
            GATE_ENV,
        ),
        (
            "`CARGO_ENCODED_RUSTFLAGS` in the workflow",
            in_workflow("CARGO_ENCODED_RUSTFLAGS: --cap-lints"),
            GATE_ENV,
        ),
        (
            "`CARGO_BUILD_RUSTDOCFLAGS` in the docs job",
            in_docs_job("CARGO_BUILD_RUSTDOCFLAGS: --cap-lints allow"),
            GATE_ENV,
        ),
        (
            "`RUSTC_WRAPPER` in the test job",
            with_job_keys("test", &["env:", "  RUSTC_WRAPPER: sccache"]),
            GATE_ENV,
        ),
        (
            "a lower-case `rustdocflags` in the docs job",
            in_docs_job("rustdocflags: --cap-lints allow"),
            GATE_ENV,
        ),
        (
            "a gate job's `env` from an expression",
            with_job_keys("clippy", &["env: ${{ fromJSON(vars.CI_ENV) }}"]),
            GATE_ENV,
        ),
        (
            "`RUSTFLAGS` in the cargo-deny job",
            with_job_keys("deny", &["env:", "  RUSTFLAGS: --cap-lints warn"]),
            GATE_ENV,
        ),
        (
            "a wrong `RUSTDOCFLAGS` in the cargo-deny job",
            with_job_keys("deny", &["env:", "  RUSTDOCFLAGS: --cap-lints allow"]),
            GATE_ENV,
        ),
    ]);
}

/// Acceptance items 5 and 6: `push` has only `branches: [main]`, and
/// `pull_request` has no filter.
#[test]
fn lib_qa_008_push_and_pull_request_triggers_are_not_narrowed() {
    let f = CI_GATES_FIXTURE;
    let on_push = |line: &str| insert_after(f, PUSH_BRANCHES, 0, &[line]);
    let on_pr = |line: &str| insert_after(f, "pull_request:", 2, &[line]);
    let on_list = ["push:", PUSH_BRANCHES, "pull_request:"]
        .into_iter()
        .fold(f.to_owned(), |text, line| delete_line(&text, line));
    assert_gate_controls(&[
        (
            "`push` filtered by `paths`",
            on_push(r#"paths: ["crates/**"]"#),
            GATE_TRIGGER,
        ),
        (
            "`push` filtered by `paths-ignore`",
            on_push(r#"paths-ignore: ["docs/**"]"#),
            GATE_TRIGGER,
        ),
        (
            "`push` with `tags`",
            on_push(r#"tags: ["v*"]"#),
            GATE_TRIGGER,
        ),
        (
            "`push` on extra branches",
            replace_line(f, PUSH_BRANCHES, "branches: [main, release]"),
            GATE_TRIGGER,
        ),
        (
            "`push` with `branches-ignore`",
            replace_line(f, PUSH_BRANCHES, "branches-ignore: [dev]"),
            GATE_TRIGGER,
        ),
        (
            "`push` with no branch filter",
            delete_line(f, PUSH_BRANCHES),
            GATE_TRIGGER,
        ),
        (
            "no `push` trigger",
            delete_line(&delete_line(f, PUSH_BRANCHES), "push:"),
            GATE_TRIGGER,
        ),
        (
            "`pull_request` filtered by `paths`",
            on_pr(r#"paths: ["crates/**"]"#),
            GATE_TRIGGER,
        ),
        (
            "`pull_request` with `branches-ignore`",
            on_pr("branches-ignore: [release]"),
            GATE_TRIGGER,
        ),
        (
            "`pull_request` with `branches`",
            on_pr("branches: [main]"),
            GATE_TRIGGER,
        ),
        (
            "`pull_request` with `types`",
            on_pr("types: [opened]"),
            GATE_TRIGGER,
        ),
        (
            "no `pull_request` trigger",
            delete_line(f, "pull_request:"),
            GATE_TRIGGER,
        ),
        (
            // Verification 0009 F8-3.
            "`pull_request` a sequence",
            replace_line(f, "pull_request:", "pull_request: [opened]"),
            GATE_TRIGGER,
        ),
        (
            "`pull_request` a string",
            replace_line(f, "pull_request:", "pull_request: main"),
            GATE_TRIGGER,
        ),
        (
            "`on` a list of events",
            replace_line(&on_list, "on:", "on: [push, pull_request]"),
            GATE_TRIGGER,
        ),
    ]);
}

/// Acceptance item 2: a top-level `defaults` is `[workflow-defaults]`.
#[test]
fn lib_qa_008_a_top_level_defaults_is_refused() {
    let f = CI_GATES_FIXTURE;
    assert_gate_controls(&[
        (
            "a top-level `defaults` shell",
            insert_before(f, "jobs:", &["defaults:", "  run:", "    shell: bash"]),
            WORKFLOW_DEFAULTS,
        ),
        (
            "a top-level `defaults` working directory",
            insert_before(
                f,
                "jobs:",
                &["defaults:", "  run:", "    working-directory: crates"],
            ),
            WORKFLOW_DEFAULTS,
        ),
    ]);
}

/// Acceptance item 6: the cargo-deny gate is LIB-QA-003's deny-job check,
/// whose problems this check reports.
#[test]
fn lib_qa_008_the_cargo_deny_gate_is_the_deny_job_check() {
    let f = CI_GATES_FIXTURE;
    let problems = check_deny_job(f);
    assert!(
        problems.is_empty(),
        "LIB-QA-008: the fixture's cargo-deny job must pass LIB-QA-003: {problems:#?}"
    );
    assert_gate_controls(&[
        (
            "the cargo-deny job deleted",
            delete_job(f, "deny"),
            DENY_MISSING,
        ),
        (
            "the cargo-deny step commented out",
            comment_line(f, PIN_DENY),
            DENY_MISSING,
        ),
        (
            "a conditional cargo-deny job",
            with_job_keys("deny", &["if: false"]),
            JOB_KEY,
        ),
        (
            "a narrowed cargo-deny command",
            insert_after(f, PIN_DENY, 2, &["with:", "  command: check bans"]),
            DENY_INPUT,
        ),
        (
            "a cargo-deny step allowed to fail",
            insert_after(f, PIN_DENY, 2, &["continue-on-error: true"]),
            STEP_KEY_AND_GUARD,
        ),
    ]);
}

/// The shared `[yaml-parse]`, `[yaml-documents]`, `[shape]` and
/// `[merge-key]` problems, each reported once though the deny-job check finds
/// it too.
#[test]
fn lib_qa_008_parse_shape_and_merge_key_problems_are_reported_once() {
    let f = CI_GATES_FIXTURE;
    assert_gate_controls(&[
        (
            "a tab",
            replace_line(f, WORKFLOW_ENV_LAST, "\tCARGO_INCREMENTAL: 0"),
            YAML_PARSE,
        ),
        (
            "a duplicate key",
            with_job_keys("fmt", &["runs-on: windows-latest"]),
            YAML_PARSE,
        ),
        (
            "a second document",
            format!("{f}---\nname: second\n"),
            YAML_DOCUMENTS,
        ),
        (
            "`steps` a mapping in another job",
            append_job(
                f,
                &[
                    "extra:",
                    "  runs-on: ubuntu-latest",
                    "  steps:",
                    "    run: echo",
                ],
            ),
            SHAPE,
        ),
        (
            "the workflow wrapped in a sequence",
            wrapped_in_sequence(f),
            SHAPE,
        ),
        (
            "a merge key in another job",
            with_merge_key_job(f),
            MERGE_KEY,
        ),
    ]);
}

/// Acceptance items 1 to 5 (positive control): extra triggers, step names
/// and ids, equivalent YAML, a gate in another clean job, and changes outside
/// the gate jobs are accepted.
#[test]
fn lib_qa_008_equivalent_and_unrelated_changes_are_accepted() {
    let f = CI_GATES_FIXTURE;
    let fmt = gate_step(GATE_FMT);
    let named_run = format!("  run: {GATE_FMT}");
    let rustdocflags_in_workflow = insert_after(
        &delete_line(f, RUSTDOCFLAGS_LINE),
        WORKFLOW_ENV_LAST,
        0,
        &[RUSTDOCFLAGS_LINE],
    );
    assert_yaml_accepted(
        QA8,
        &[
            (
                "a `workflow_dispatch` trigger",
                insert_after(f, "pull_request:", 0, &["workflow_dispatch:"]),
            ),
            (
                "a `schedule` trigger",
                insert_after(
                    f,
                    "pull_request:",
                    0,
                    &["schedule:", "  - cron: \"0 3 * * 1\""],
                ),
            ),
            (
                "`pull_request` an empty mapping",
                replace_line(f, "pull_request:", "pull_request: {}"),
            ),
            (
                "a name and an id on a gate step",
                replace_lines(f, &fmt, &["- name: rustfmt", "  id: fmt", &named_run]),
            ),
            (
                "a quoted gate",
                replace_line(f, &fmt, &format!("- run: \"{GATE_FMT}\"")),
            ),
            (
                "a gate step as a flow mapping",
                replace_line(f, &fmt, &format!("- {{ run: {GATE_FMT}, name: rustfmt }}")),
            ),
            (
                "a gate moved to another clean job",
                with_gate_moved(GATE_HOOKS, GATE_FMT),
            ),
            (
                "`fail-fast: true`",
                replace_line(f, FAIL_FAST_LINE, "fail-fast: true"),
            ),
            ("no `fail-fast`", delete_line(f, FAIL_FAST_LINE)),
            (
                "`RUSTDOCFLAGS` moved to the workflow",
                rustdocflags_in_workflow,
            ),
            (
                "`RUSTDOCFLAGS` also in another gate job",
                with_job_keys("clippy", &["env:", &format!("  {RUSTDOCFLAGS_LINE}")]),
            ),
            (
                "`permissions` on a gate job",
                with_job_keys("fmt", &["permissions:", "  contents: read"]),
            ),
            (
                "an allowed variable in the cargo-deny job",
                with_job_keys("deny", &["env:", "  CARGO_TERM_COLOR: never"]),
            ),
            (
                "a conditional job that runs no gate",
                append_job(
                    f,
                    &[
                        "extra:",
                        "  if: github.event_name == 'pull_request'",
                        "  runs-on: ubuntu-latest",
                        "  env:",
                        "    RUSTFLAGS: --cfg extra",
                        "  steps:",
                        "    - run: echo extra",
                        "      continue-on-error: true",
                    ],
                ),
            ),
        ],
        check_ci_gates,
    );
}
