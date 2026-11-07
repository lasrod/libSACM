//! Regression tests for repository configuration: the CI gates, the MSRV
//! declaration and the dependency policy (rows LIB-QA-001..003), and the
//! git-ignore that keeps OMG documents out of the repository (LIB-TOOL-002).
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
    for (number, step) in &job.steps {
        for key in step.keys().filter_map(Yaml::as_str) {
            if STEP_GUARDS.contains(&key) {
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

fn is_deny_step(step: &Hash) -> bool {
    get(step, "uses")
        .and_then(Yaml::as_str)
        .is_some_and(|uses| uses.starts_with(DENY_ACTION))
}

/// Y3-3: the cargo-deny step's inputs run every check over the whole
/// workspace. Input names are compared without regard to case, as GitHub
/// does. A `with` that is not a mapping is a `[shape]` problem reported by
/// [`read_workflow`].
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
    for key in ["include-dev", "include-build"] {
        require(
            &mut problems,
            !matches!(
                lookup(deny, &["licenses", key]),
                Some(toml::Value::Boolean(false))
            ),
            "licenses",
            &format!("licenses.{key} must not be false"),
        );
    }
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
    let job_line = format!("{job}:");
    let gated = with_gate_job(&ci, &job);
    let mutations = [
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
            insert_after(&gated, &step, 2, &["<<: *skip_step"]),
            &["step-key", "merge-key"],
        ),
        mutation(
            "step-level shell: \"true {0}\"",
            insert_after(&ci, &step, 2, &["shell: \"true {0}\""]),
            STEP_KEY_AND_GUARD,
        ),
        mutation(
            "continue-on-error on another step of the job",
            insert_after(
                &ci,
                "- uses: dtolnay/rust-toolchain@master",
                2,
                &["continue-on-error: true"],
            ),
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
            insert_after(&ci, &step, 2, &["working-directory: crates/sacm"]),
            &["step-key"],
        ),
    ];
    assert_yaml_mutations_reported("LIB-QA-002", &mutations, |text| check_msrv_job(text, &msrv));
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
                "licenses.include-build = false",
                "licenses",
                "include-build",
                "false",
            ),
        ],
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
/// unchecked `[bans]` and `[output]` sections.
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
    (
        "licenses.include-dev = true",
        "licenses",
        "include-dev",
        "true",
    ),
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

/// The cargo-deny `command` inputs of job `job`, as GitHub reads them.
fn deny_commands(ci: &str, job: &str) -> Vec<String> {
    step_values(ci, job, "with")
        .iter()
        .filter_map(Yaml::as_hash)
        .filter_map(|with| get(with, "command").and_then(Yaml::as_str))
        .map(str::to_owned)
        .collect()
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
            "a harmless input",
            after_step(&["with: { command: check, log-level: warn }"]),
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
