//! The functionality-matrix gate.
//!
//! `docs/matrix/functionality-matrix.toml` is the canonical record of what
//! libSACM implements, requirement by requirement. This module checks it
//! against the repository, renders it to Markdown, and answers "what is the
//! next work?" for the slice loop. The rules it enforces are documented in
//! `docs/matrix/README.md`; keep the two in step.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub(crate) const MATRIX_PATH: &str = "docs/matrix/functionality-matrix.toml";
pub(crate) const RENDERED_PATH: &str = "docs/matrix/functionality-matrix.md";

/// Directories scanned for traced test functions (orphan detection).
const TEST_SCAN_ROOTS: &[&str] = &["crates", "xtask", ".claude/hooks"];

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Matrix {
    pub(crate) schema_version: u32,
    #[serde(rename = "milestone", default)]
    pub(crate) milestones: Vec<Milestone>,
    #[serde(rename = "area", default)]
    pub(crate) areas: Vec<Area>,
    #[serde(rename = "row", default)]
    pub(crate) rows: Vec<Row>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Milestone {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) goal: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Area {
    pub(crate) code: String,
    pub(crate) prefix: Prefix,
    pub(crate) title: String,
}

/// `SACM23` rows are obligations of the standard; `LIB` rows are library
/// capabilities and quality commitments the standard does not state.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Prefix {
    #[serde(rename = "SACM23")]
    Sacm23,
    #[serde(rename = "LIB")]
    Lib,
}

impl Prefix {
    fn as_str(self) -> &'static str {
        match self {
            Prefix::Sacm23 => "SACM23",
            Prefix::Lib => "LIB",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Row {
    pub(crate) id: String,
    pub(crate) area: String,
    pub(crate) milestone: String,
    pub(crate) title: String,
    pub(crate) requirement: String,
    pub(crate) source: String,
    pub(crate) kind: Kind,
    pub(crate) status: Status,
    #[serde(default)]
    pub(crate) depends_on: Vec<String>,
    #[serde(default)]
    pub(crate) acceptance: Vec<String>,
    #[serde(default)]
    pub(crate) code: Vec<String>,
    #[serde(default)]
    pub(crate) tests: Vec<String>,
    #[serde(default)]
    pub(crate) verification: Option<String>,
    #[serde(default)]
    pub(crate) notes: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Status {
    Planned,
    Analyzed,
    TestsFailing,
    Implemented,
    Verified,
    Candidate,
    Deferred,
    OutOfScope,
}

impl Status {
    const ALL: [Status; 8] = [
        Status::Verified,
        Status::Implemented,
        Status::TestsFailing,
        Status::Analyzed,
        Status::Planned,
        Status::Candidate,
        Status::Deferred,
        Status::OutOfScope,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Status::Planned => "planned",
            Status::Analyzed => "analyzed",
            Status::TestsFailing => "tests-failing",
            Status::Implemented => "implemented",
            Status::Verified => "verified",
            Status::Candidate => "candidate",
            Status::Deferred => "deferred",
            Status::OutOfScope => "out-of-scope",
        }
    }

    /// Scheduled work that is not finished: what the loop picks from.
    fn is_open(self) -> bool {
        matches!(
            self,
            Status::Planned | Status::Analyzed | Status::TestsFailing | Status::Implemented
        )
    }

    /// Not scheduled; must say why in `notes`.
    fn needs_reason(self) -> bool {
        matches!(
            self,
            Status::Candidate | Status::Deferred | Status::OutOfScope
        )
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Kind {
    Model,
    Xmi,
    Validation,
    Editing,
    Api,
    Security,
    Cli,
    Docs,
    Quality,
    Tooling,
    Compliance,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Model => "model",
            Kind::Xmi => "xmi",
            Kind::Validation => "validation",
            Kind::Editing => "editing",
            Kind::Api => "api",
            Kind::Security => "security",
            Kind::Cli => "cli",
            Kind::Docs => "docs",
            Kind::Quality => "quality",
            Kind::Tooling => "tooling",
            Kind::Compliance => "compliance",
        }
    }
}

// ---------------------------------------------------------------------------
// Repository access (a trait so the checks are testable without a filesystem)
// ---------------------------------------------------------------------------

pub(crate) trait Repo {
    fn exists(&self, rel: &str) -> bool;
    fn read(&self, rel: &str) -> Option<String>;
    /// Every function name in the scanned test roots that looks like a traced
    /// test (`sacm23_<area>_<nnn>...` or `lib_<area>_<nnn>...`), with its file.
    fn traced_functions(&self) -> Vec<(String, String)>;
}

pub(crate) struct FsRepo {
    root: PathBuf,
}

impl FsRepo {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }
}

/// Rust and Python source files under `dir`, skipping build output.
fn walk_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk_sources(&path, out);
        } else if has_extension(&path, "rs") || has_extension(&path, "py") {
            out.push(path);
        }
    }
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

impl Repo for FsRepo {
    fn exists(&self, rel: &str) -> bool {
        self.root.join(rel).exists()
    }

    fn read(&self, rel: &str) -> Option<String> {
        fs::read_to_string(self.root.join(rel)).ok()
    }

    fn traced_functions(&self) -> Vec<(String, String)> {
        let mut files = Vec::new();
        for scan_root in TEST_SCAN_ROOTS {
            walk_sources(&self.root.join(scan_root), &mut files);
        }
        let mut found = Vec::new();
        for file in files {
            let Ok(text) = fs::read_to_string(&file) else {
                continue;
            };
            let rel = file
                .strip_prefix(&self.root)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            for name in traced_tests(&text, has_extension(&file, "py")) {
                found.push((rel.clone(), name));
            }
        }
        found
    }
}

/// Test functions whose names trace to a matrix row: Rust functions under a
/// `#[test]`-style attribute, and Python `def test_<row>...` functions.
fn traced_tests(text: &str, python: bool) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending_test_attr = false;
    for line in text.lines() {
        let line = line.trim();
        if python {
            if let Some(rest) = line.strip_prefix("def ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if name.starts_with("test_") && looks_traced(&name) {
                    names.push(name);
                }
            }
            continue;
        }
        if line.starts_with("#[") && line.ends_with("test]") {
            pending_test_attr = true;
        } else if line.is_empty() || line.starts_with("#[") || line.starts_with("//") {
            // Attributes and comments may sit between `#[test]` and the function.
        } else {
            if pending_test_attr {
                if let Some(name) = defined_functions(line).into_iter().next() {
                    if looks_traced(&name) {
                        names.push(name);
                    }
                }
            }
            pending_test_attr = false;
        }
    }
    names
}

/// Names following `fn ` or `def ` in source text.
fn defined_functions(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for keyword in ["fn ", "def "] {
        let mut rest = text;
        while let Some(pos) = rest.find(keyword) {
            let preceded_ok = pos == 0
                || !rest[..pos]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
            let after = &rest[pos + keyword.len()..];
            let name: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if preceded_ok && !name.is_empty() {
                names.push(name);
            }
            rest = after;
        }
    }
    names
}

/// The part of a test name that carries the row ID. Python's unittest requires
/// a `test_` prefix, which is not part of the trace.
fn trace_key(name: &str) -> &str {
    name.strip_prefix("test_").unwrap_or(name)
}

/// `sacm23_base_001...` / `lib_cli_001...` (optionally `test_`-prefixed).
fn looks_traced(name: &str) -> bool {
    let mut parts = trace_key(name).splitn(4, '_');
    let (Some(prefix), Some(area), Some(number)) = (parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    matches!(prefix, "sacm23" | "lib")
        && !area.is_empty()
        && area.chars().all(|c| c.is_ascii_lowercase())
        && number.len() == 3
        && number.chars().all(|c| c.is_ascii_digit())
}

/// `SACM23-BASE-001` -> `sacm23_base_001`.
fn test_prefix(id: &str) -> String {
    id.to_ascii_lowercase().replace('-', "_")
}

fn parse_id(id: &str) -> Option<(&str, &str, &str)> {
    let mut parts = id.split('-');
    let (prefix, area, number) = (parts.next()?, parts.next()?, parts.next()?);
    let well_formed = parts.next().is_none()
        && matches!(prefix, "SACM23" | "LIB")
        && !area.is_empty()
        && area.chars().all(|c| c.is_ascii_uppercase())
        && number.len() == 3
        && number.chars().all(|c| c.is_ascii_digit());
    well_formed.then_some((prefix, area, number))
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

pub(crate) fn load(text: &str) -> Result<Matrix, String> {
    toml::from_str(text).map_err(|e| format!("{MATRIX_PATH}: {e}"))
}

/// Every rule violation in the matrix, as human-readable messages.
pub(crate) fn check(matrix: &Matrix, repo: &dyn Repo) -> Vec<String> {
    let mut errors = Vec::new();
    if matrix.schema_version != 1 {
        errors.push(format!(
            "schema_version {} is not supported (expected 1)",
            matrix.schema_version
        ));
    }

    let milestone_order: HashMap<&str, usize> = matrix
        .milestones
        .iter()
        .enumerate()
        .map(|(i, m)| (m.id.as_str(), i))
        .collect();
    if milestone_order.len() != matrix.milestones.len() {
        errors.push("milestone ids must be unique".into());
    }
    let areas: HashMap<&str, &Area> = matrix.areas.iter().map(|a| (a.code.as_str(), a)).collect();
    if areas.len() != matrix.areas.len() {
        errors.push("area codes must be unique".into());
    }

    let mut rows: HashMap<&str, &Row> = HashMap::new();
    for row in &matrix.rows {
        if rows.insert(row.id.as_str(), row).is_some() {
            errors.push(format!("{}: duplicate row id", row.id));
        }
    }

    let index = Index {
        areas: &areas,
        milestone_order: &milestone_order,
        rows: &rows,
        repo,
    };
    for row in &matrix.rows {
        check_row(row, &index, &mut errors);
    }
    check_cycles(matrix, &rows, &mut errors);
    check_orphans(matrix, repo, &mut errors);

    match repo.read(RENDERED_PATH) {
        Some(on_disk) if normalize(&on_disk) == normalize(&render(matrix)) => {}
        Some(_) => errors.push(format!(
            "{RENDERED_PATH} is out of date; run `cargo xtask matrix render`"
        )),
        None => errors.push(format!(
            "{RENDERED_PATH} is missing; run `cargo xtask matrix render`"
        )),
    }
    errors
}

/// Lookups shared by the per-row checks.
struct Index<'a> {
    areas: &'a HashMap<&'a str, &'a Area>,
    milestone_order: &'a HashMap<&'a str, usize>,
    rows: &'a HashMap<&'a str, &'a Row>,
    repo: &'a dyn Repo,
}

/// Every rule that concerns a single row; messages are prefixed with its id.
fn check_row(row: &Row, index: &Index<'_>, errors: &mut Vec<String>) {
    let mut problems = check_identity(row, index);
    problems.extend(check_dependencies(row, index));
    problems.extend(check_evidence(row, index));
    problems.extend(check_citations(row, index));
    errors.extend(problems.into_iter().map(|p| format!("{}: {p}", row.id)));
}

/// Id form, area, prefix, milestone and required text fields.
fn check_identity(row: &Row, index: &Index<'_>) -> Vec<String> {
    let mut out = Vec::new();
    match parse_id(&row.id) {
        None => out.push("id must look like SACM23-<AREA>-<NNN> or LIB-<AREA>-<NNN>".into()),
        Some((prefix, area, _)) => {
            if area != row.area {
                out.push(format!(
                    "id area `{area}` does not match area field `{}`",
                    row.area
                ));
            }
            match index.areas.get(row.area.as_str()) {
                None => out.push(format!("unknown area `{}`", row.area)),
                Some(a) if a.prefix.as_str() != prefix => out.push(format!(
                    "area {} is a {} area; id prefix must be {}",
                    a.code,
                    a.prefix.as_str(),
                    a.prefix.as_str()
                )),
                Some(_) => {}
            }
        }
    }
    for (field, value) in [
        ("title", &row.title),
        ("requirement", &row.requirement),
        ("source", &row.source),
    ] {
        if value.trim().is_empty() {
            out.push(format!("`{field}` must not be empty"));
        }
    }
    if !index.milestone_order.contains_key(row.milestone.as_str()) {
        out.push(format!("unknown milestone `{}`", row.milestone));
    }
    out
}

/// Dependencies resolve, are schedulable, respect milestone order, and are
/// verified before the row itself is.
fn check_dependencies(row: &Row, index: &Index<'_>) -> Vec<String> {
    let mut out = Vec::new();
    let own_milestone = index.milestone_order.get(row.milestone.as_str());
    for dep in &row.depends_on {
        if *dep == row.id {
            out.push("depends on itself".into());
            continue;
        }
        let Some(target) = index.rows.get(dep.as_str()) else {
            out.push(format!("depends on unknown row {dep}"));
            continue;
        };
        if matches!(target.status, Status::OutOfScope | Status::Candidate) {
            out.push(format!(
                "depends on {dep}, which is {}",
                target.status.as_str()
            ));
        }
        let target_milestone = index.milestone_order.get(target.milestone.as_str());
        if let (Some(own), Some(theirs)) = (own_milestone, target_milestone) {
            if theirs > own {
                out.push(format!(
                    "depends on {dep} from a later milestone ({}); reorder or move one of them",
                    target.milestone
                ));
            }
        }
        if row.status == Status::Verified && target.status != Status::Verified {
            out.push(format!(
                "is verified but depends on {dep}, which is {}",
                target.status.as_str()
            ));
        }
    }
    out
}

/// The evidence each status requires or forbids (docs/matrix/README.md).
fn check_evidence(row: &Row, index: &Index<'_>) -> Vec<String> {
    let mut out = Vec::new();
    let status = row.status;
    let s = status.as_str();
    let needs_acceptance = !matches!(
        status,
        Status::Planned | Status::Candidate | Status::Deferred | Status::OutOfScope
    );
    if needs_acceptance && row.acceptance.is_empty() {
        out.push(format!("is {s} but lists no acceptance criteria"));
    }
    let needs_tests = matches!(
        status,
        Status::TestsFailing | Status::Implemented | Status::Verified
    );
    if needs_tests && row.tests.is_empty() {
        out.push(format!("is {s} but cites no tests"));
    }
    if !needs_tests && !row.tests.is_empty() {
        out.push(format!(
            "is {s} but cites tests; its status has fallen behind reality"
        ));
    }
    let needs_code = matches!(status, Status::Implemented | Status::Verified);
    if needs_code && row.code.is_empty() {
        out.push(format!("is {s} but cites no code"));
    }
    if matches!(
        status,
        Status::Planned | Status::Analyzed | Status::Candidate
    ) && !row.code.is_empty()
    {
        out.push(format!(
            "is {s} but cites code; its status has fallen behind reality"
        ));
    }
    match (&row.verification, status) {
        (None, Status::Verified) => {
            out.push("is verified but cites no verification record".into());
        }
        (Some(_), st) if st != Status::Verified => {
            out.push(format!("is {s} but cites a verification record"));
        }
        (Some(record), _) => match index.repo.read(record) {
            None => out.push(format!("verification record {record} does not exist")),
            Some(text) if !text.contains(row.id.as_str()) => {
                out.push(format!(
                    "verification record {record} does not mention {}",
                    row.id
                ));
            }
            Some(_) => {}
        },
        (None, _) => {}
    }
    if status.needs_reason() && row.notes.as_deref().is_none_or(|n| n.trim().is_empty()) {
        out.push(format!("is {s} and must say why in `notes`"));
    }
    out
}

/// Cited code paths exist; cited tests exist and are named for this row.
fn check_citations(row: &Row, index: &Index<'_>) -> Vec<String> {
    let mut out = Vec::new();
    for path in &row.code {
        if !index.repo.exists(path) {
            out.push(format!("cited code path {path} does not exist"));
        }
    }
    let prefix = test_prefix(&row.id);
    for test in &row.tests {
        if let Some(path) = test.strip_prefix("doctest:") {
            match index.repo.read(path) {
                None => out.push(format!("doctest file {path} does not exist")),
                Some(text) if !text.contains("```") => {
                    out.push(format!("doctest file {path} contains no code block"));
                }
                Some(_) => {}
            }
            continue;
        }
        let Some((path, name)) = test.split_once("::") else {
            out.push(format!(
                "test citation `{test}` must be `path::function` or `doctest:path`"
            ));
            continue;
        };
        let key = trace_key(name);
        if key != prefix && !key.starts_with(&format!("{prefix}_")) {
            out.push(format!(
                "test `{name}` must be named `{prefix}_...` to trace to this row"
            ));
        }
        match index.repo.read(path) {
            None => out.push(format!("test file {path} does not exist")),
            Some(text) if !defined_functions(&text).iter().any(|f| f == name) => {
                out.push(format!("test function `{name}` is not defined in {path}"));
            }
            Some(_) => {}
        }
    }
    out
}

fn check_cycles(matrix: &Matrix, rows: &HashMap<&str, &Row>, errors: &mut Vec<String>) {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    fn visit<'a>(
        id: &'a str,
        rows: &HashMap<&'a str, &'a Row>,
        marks: &mut HashMap<&'a str, Mark>,
        stack: &mut Vec<&'a str>,
        errors: &mut Vec<String>,
    ) {
        match marks.get(id) {
            Some(Mark::Done) => return,
            Some(Mark::Visiting) => {
                let start = stack.iter().position(|s| *s == id).unwrap_or(0);
                errors.push(format!(
                    "dependency cycle: {} -> {id}",
                    stack[start..].join(" -> ")
                ));
                return;
            }
            None => {}
        }
        marks.insert(id, Mark::Visiting);
        stack.push(id);
        if let Some(row) = rows.get(id) {
            for dep in &row.depends_on {
                if rows.contains_key(dep.as_str()) {
                    visit(dep.as_str(), rows, marks, stack, errors);
                }
            }
        }
        stack.pop();
        marks.insert(id, Mark::Done);
    }
    let mut marks = HashMap::new();
    for row in &matrix.rows {
        visit(row.id.as_str(), rows, &mut marks, &mut Vec::new(), errors);
    }
}

/// A traced test that no row cites is invisible to everyone downstream.
fn check_orphans(matrix: &Matrix, repo: &dyn Repo, errors: &mut Vec<String>) {
    let cited: HashSet<(&str, &str)> = matrix
        .rows
        .iter()
        .flat_map(|r| r.tests.iter())
        .filter_map(|t| t.split_once("::"))
        .collect();
    for (file, name) in repo.traced_functions() {
        if !cited.contains(&(file.as_str(), name.as_str())) {
            errors.push(format!(
                "{file}: traced test `{name}` is not cited by any matrix row"
            ));
        }
    }
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end().to_owned()
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn cell(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

pub(crate) fn render(matrix: &Matrix) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<!-- Generated by `cargo xtask matrix render` from functionality-matrix.toml. \
         Do not edit by hand. -->\n"
    );
    out.push_str("# Functionality matrix\n\n");
    out.push_str(
        "What libSACM implements, requirement by requirement. The canonical source is \
         [`functionality-matrix.toml`](functionality-matrix.toml); the row format, status \
         vocabulary and gate rules are described in [README.md](README.md). `SACM23-*` rows \
         are obligations of the OMG SACM 2.3 standard; `LIB-*` rows are library capabilities \
         and quality commitments.\n\n",
    );
    render_summary(&mut out, matrix);
    render_areas(&mut out, matrix);
    for milestone in &matrix.milestones {
        render_milestone(&mut out, matrix, milestone);
    }
    out
}

fn count_cell(n: usize, bold: bool) -> String {
    match (n, bold) {
        (0, _) => String::new(),
        (n, true) => format!("**{n}**"),
        (n, false) => n.to_string(),
    }
}

fn render_summary(out: &mut String, matrix: &Matrix) {
    out.push_str("## Summary\n\n| Milestone | Rows |");
    for status in Status::ALL {
        let _ = write!(out, " {} |", status.as_str());
    }
    out.push_str("\n|---|---:|");
    for _ in Status::ALL {
        out.push_str("---:|");
    }
    out.push('\n');
    let mut totals: BTreeMap<Status, usize> = BTreeMap::new();
    for milestone in &matrix.milestones {
        let rows: Vec<&Row> = matrix
            .rows
            .iter()
            .filter(|r| r.milestone == milestone.id)
            .collect();
        let _ = write!(
            out,
            "| {} — {} | {} |",
            milestone.id,
            cell(&milestone.title),
            rows.len()
        );
        for status in Status::ALL {
            let n = rows.iter().filter(|r| r.status == status).count();
            *totals.entry(status).or_default() += n;
            let _ = write!(out, " {} |", count_cell(n, false));
        }
        out.push('\n');
    }
    let _ = write!(out, "| **Total** | **{}** |", matrix.rows.len());
    for status in Status::ALL {
        let n = totals.get(&status).copied().unwrap_or(0);
        let _ = write!(out, " {} |", count_cell(n, true));
    }
    out.push_str("\n\n");
}

fn render_areas(out: &mut String, matrix: &Matrix) {
    out.push_str("## Areas\n\n| Code | Prefix | Area |\n|---|---|---|\n");
    for area in &matrix.areas {
        let _ = writeln!(
            out,
            "| `{}` | `{}` | {} |",
            area.code,
            area.prefix.as_str(),
            cell(&area.title)
        );
    }
    out.push('\n');
}

fn render_milestone(out: &mut String, matrix: &Matrix, milestone: &Milestone) {
    let _ = writeln!(
        out,
        "## {} — {}\n\n{}\n",
        milestone.id,
        cell(&milestone.title),
        cell(&milestone.goal)
    );
    out.push_str("| ID | Requirement | Kind | Source | Status | Depends on | Tests |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for row in matrix.rows.iter().filter(|r| r.milestone == milestone.id) {
        let mut requirement = format!("**{}.** {}", cell(&row.title), cell(&row.requirement));
        if let Some(notes) = row.notes.as_deref().filter(|n| !n.trim().is_empty()) {
            let _ = write!(requirement, "<br>_Note: {}_", cell(notes));
        }
        let tests = row
            .tests
            .iter()
            .map(|t| {
                let name = t.split_once("::").map_or(t.as_str(), |(_, n)| n);
                format!("`{name}`")
            })
            .collect::<Vec<_>>()
            .join("<br>");
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | `{}` | {} | {} |",
            row.id,
            requirement,
            row.kind.as_str(),
            cell(&row.source),
            row.status.as_str(),
            row.depends_on.join(", "),
            tests
        );
    }
    out.push('\n');
}

// ---------------------------------------------------------------------------
// Loop support
// ---------------------------------------------------------------------------

/// Open rows whose dependencies are all verified, in milestone then file order.
pub(crate) fn ready(matrix: &Matrix) -> Vec<&Row> {
    let status: HashMap<&str, Status> = matrix
        .rows
        .iter()
        .map(|r| (r.id.as_str(), r.status))
        .collect();
    let milestone_order: HashMap<&str, usize> = matrix
        .milestones
        .iter()
        .enumerate()
        .map(|(i, m)| (m.id.as_str(), i))
        .collect();
    let mut rows: Vec<(usize, usize, &Row)> = matrix
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.status.is_open())
        .filter(|(_, r)| {
            r.depends_on
                .iter()
                .all(|d| status.get(d.as_str()) == Some(&Status::Verified))
        })
        .map(|(i, r)| {
            (
                milestone_order
                    .get(r.milestone.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
                i,
                r,
            )
        })
        .collect();
    rows.sort_by_key(|(m, i, _)| (*m, *i));
    rows.into_iter().map(|(_, _, r)| r).collect()
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn load_from(root: &Path) -> Result<Matrix, String> {
    let path = root.join(MATRIX_PATH);
    let text = fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    load(&text)
}

pub(crate) fn cmd_check(root: &Path) -> Result<(), String> {
    let matrix = load_from(root)?;
    let errors = check(&matrix, &FsRepo::new(root));
    if errors.is_empty() {
        println!("functionality matrix OK: {} rows", matrix.rows.len());
        Ok(())
    } else {
        for error in &errors {
            eprintln!("  {error}");
        }
        Err(format!("functionality matrix: {} problem(s)", errors.len()))
    }
}

pub(crate) fn cmd_render(root: &Path) -> Result<(), String> {
    let matrix = load_from(root)?;
    let path = root.join(RENDERED_PATH);
    fs::write(&path, render(&matrix)).map_err(|e| format!("write {}: {e}", path.display()))?;
    println!("wrote {RENDERED_PATH}");
    Ok(())
}

pub(crate) fn cmd_next(root: &Path, limit: usize) -> Result<(), String> {
    let matrix = load_from(root)?;
    let ready = ready(&matrix);
    let open = matrix.rows.iter().filter(|r| r.status.is_open()).count();
    if ready.is_empty() {
        println!("no ready rows ({open} open rows, all blocked on unverified dependencies)");
        return Ok(());
    }
    println!(
        "{} ready of {open} open rows; next {}:",
        ready.len(),
        limit.min(ready.len())
    );
    for row in ready.into_iter().take(limit) {
        println!(
            "  {:<17} {:<4} {:<14} {}",
            row.id,
            row.milestone,
            row.status.as_str(),
            row.title
        );
    }
    Ok(())
}

pub(crate) fn cmd_status(root: &Path) -> Result<(), String> {
    let matrix = load_from(root)?;
    for milestone in &matrix.milestones {
        let rows: Vec<&Row> = matrix
            .rows
            .iter()
            .filter(|r| r.milestone == milestone.id)
            .collect();
        let verified = rows.iter().filter(|r| r.status == Status::Verified).count();
        let scheduled = rows.iter().filter(|r| !r.status.needs_reason()).count();
        let mut parts = Vec::new();
        for status in Status::ALL {
            let n = rows.iter().filter(|r| r.status == status).count();
            if n > 0 {
                parts.push(format!("{n} {}", status.as_str()));
            }
        }
        println!(
            "{:<4} {:<44} {verified:>3}/{scheduled:<3} verified  ({})",
            milestone.id,
            milestone.title,
            parts.join(", ")
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeRepo {
        files: HashMap<String, String>,
    }

    impl FakeRepo {
        fn with(mut self, path: &str, text: &str) -> Self {
            self.files.insert(path.into(), text.into());
            self
        }
    }

    impl Repo for FakeRepo {
        fn exists(&self, rel: &str) -> bool {
            self.files.contains_key(rel)
        }
        fn read(&self, rel: &str) -> Option<String> {
            self.files.get(rel).cloned()
        }
        fn traced_functions(&self) -> Vec<(String, String)> {
            let mut out = Vec::new();
            for (path, text) in &self.files {
                for name in traced_tests(text, has_extension(Path::new(path), "py")) {
                    out.push((path.clone(), name));
                }
            }
            out
        }
    }

    const HEADER: &str = r#"
schema_version = 1
[[milestone]]
id = "M0"
title = "Foundation"
goal = "Build."
[[milestone]]
id = "M1"
title = "Core"
goal = "Model."
[[area]]
code = "BASE"
prefix = "SACM23"
title = "Base"
[[area]]
code = "CLI"
prefix = "LIB"
title = "CLI"
"#;

    fn matrix(rows: &str) -> Matrix {
        load(&format!("{HEADER}{rows}")).expect("test matrix parses")
    }

    /// Check with the rendered page in sync, so tests see only the rule under test.
    fn problems(m: &Matrix, repo: FakeRepo) -> Vec<String> {
        let repo = repo.with(RENDERED_PATH, &render(m));
        check(m, &repo)
    }

    fn assert_problem(problems: &[String], needle: &str) {
        assert!(
            problems.iter().any(|p| p.contains(needle)),
            "expected a problem containing {needle:?}, got {problems:#?}"
        );
    }

    const PLANNED: &str = r#"
[[row]]
id = "SACM23-BASE-001"
area = "BASE"
milestone = "M1"
title = "LangString"
requirement = "Represent LangString."
source = "§8.3"
kind = "model"
status = "planned"
"#;

    #[test]
    fn lib_tool_001_accepts_a_consistent_matrix() {
        assert_eq!(
            problems(&matrix(PLANNED), FakeRepo::default()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn lib_tool_001_rejects_malformed_and_mismatched_ids() {
        let m = matrix(&PLANNED.replace("SACM23-BASE-001", "LIB-BASE-1"));
        let p = problems(&m, FakeRepo::default());
        assert_problem(&p, "id must look like");

        let m = matrix(&PLANNED.replace("SACM23-BASE-001", "LIB-BASE-001"));
        assert_problem(
            &problems(&m, FakeRepo::default()),
            "id prefix must be SACM23",
        );
    }

    #[test]
    fn lib_tool_001_rejects_duplicate_ids() {
        let m = matrix(&format!("{PLANNED}{PLANNED}"));
        assert_problem(&problems(&m, FakeRepo::default()), "duplicate row id");
    }

    #[test]
    fn lib_tool_001_planned_rows_must_not_cite_tests() {
        let row = PLANNED.replace(
            "status = \"planned\"",
            "status = \"planned\"\ntests = [\"t.rs::sacm23_base_001_x\"]",
        );
        let repo = FakeRepo::default().with("t.rs", "fn sacm23_base_001_x() {}");
        assert_problem(&problems(&matrix(&row), repo), "fallen behind reality");
    }

    #[test]
    fn lib_tool_001_implemented_rows_need_acceptance_code_and_tests() {
        let row = PLANNED.replace("status = \"planned\"", "status = \"implemented\"");
        let p = problems(&matrix(&row), FakeRepo::default());
        assert_problem(&p, "no acceptance criteria");
        assert_problem(&p, "cites no tests");
        assert_problem(&p, "cites no code");
    }

    #[test]
    fn lib_tool_001_test_citations_must_resolve_and_trace() {
        let row = PLANNED.replace(
            "status = \"planned\"",
            "status = \"tests-failing\"\nacceptance = [\"a\"]\n\
             tests = [\"t.rs::sacm23_base_001_ok\", \"t.rs::sacm23_base_001_missing\", \
             \"t.rs::lib_cli_001_wrong_row\"]",
        );
        let repo = FakeRepo::default().with(
            "t.rs",
            "fn sacm23_base_001_ok() {}\nfn lib_cli_001_wrong_row() {}",
        );
        let p = problems(&matrix(&row), repo);
        assert_problem(&p, "`sacm23_base_001_missing` is not defined");
        assert_problem(&p, "must be named `sacm23_base_001_...`");
        assert!(
            !p.iter().any(|e| e.contains("sacm23_base_001_ok")),
            "{p:#?}"
        );
    }

    #[test]
    fn lib_tool_001_verified_rows_need_record_and_verified_dependencies() {
        let rows = format!(
            "{PLANNED}{}",
            r#"
[[row]]
id = "SACM23-BASE-002"
area = "BASE"
milestone = "M1"
title = "MultiLangString"
requirement = "Represent MultiLangString."
source = "§8.5"
kind = "model"
status = "verified"
depends_on = ["SACM23-BASE-001"]
acceptance = ["a"]
code = ["src/lib.rs"]
tests = ["t.rs::sacm23_base_002_x"]
verification = "docs/verification/v.md"
"#
        );
        let repo = FakeRepo::default()
            .with("src/lib.rs", "")
            .with("t.rs", "fn sacm23_base_002_x() {}")
            .with("docs/verification/v.md", "mentions nothing");
        let p = problems(&matrix(&rows), repo);
        assert_problem(&p, "depends on SACM23-BASE-001, which is planned");
        assert_problem(&p, "does not mention SACM23-BASE-002");
    }

    #[test]
    fn lib_tool_001_rejects_cycles_and_backward_milestone_dependencies() {
        let a = PLANNED.replace(
            "status = \"planned\"",
            "status = \"planned\"\ndepends_on = [\"SACM23-BASE-002\"]",
        );
        let b = PLANNED
            .replace("SACM23-BASE-001", "SACM23-BASE-002")
            .replace(
                "status = \"planned\"",
                "status = \"planned\"\ndepends_on = [\"SACM23-BASE-001\"]",
            );
        let p = problems(&matrix(&format!("{a}{b}")), FakeRepo::default());
        assert_problem(&p, "dependency cycle");

        let early = PLANNED
            .replace("milestone = \"M1\"", "milestone = \"M0\"")
            .replace(
                "status = \"planned\"",
                "status = \"planned\"\ndepends_on = [\"SACM23-BASE-002\"]",
            );
        let late = PLANNED.replace("SACM23-BASE-001", "SACM23-BASE-002");
        let p = problems(&matrix(&format!("{early}{late}")), FakeRepo::default());
        assert_problem(&p, "from a later milestone");
    }

    #[test]
    fn lib_tool_001_unscheduled_rows_must_give_a_reason() {
        let row = PLANNED.replace("status = \"planned\"", "status = \"out-of-scope\"");
        assert_problem(
            &problems(&matrix(&row), FakeRepo::default()),
            "must say why",
        );
    }

    #[test]
    fn lib_tool_001_reports_orphaned_traced_tests() {
        let repo = FakeRepo::default().with("t.rs", "#[test]\nfn sacm23_base_001_uncited() {}");
        assert_problem(
            &problems(&matrix(PLANNED), repo),
            "not cited by any matrix row",
        );
    }

    #[test]
    fn lib_tool_001_detects_stale_rendered_page() {
        let m = matrix(PLANNED);
        let repo = FakeRepo::default().with(RENDERED_PATH, "stale");
        assert_problem(&check(&m, &repo), "out of date");
    }

    #[test]
    fn lib_tool_001_next_lists_open_rows_with_verified_dependencies_in_order() {
        let blocked = PLANNED
            .replace("SACM23-BASE-001", "SACM23-BASE-003")
            .replace(
                "status = \"planned\"",
                "status = \"planned\"\ndepends_on = [\"SACM23-BASE-001\"]",
            );
        let early = PLANNED
            .replace("SACM23-BASE-001", "LIB-CLI-001")
            .replace("area = \"BASE\"", "area = \"CLI\"")
            .replace("milestone = \"M1\"", "milestone = \"M0\"");
        let m = matrix(&format!("{PLANNED}{blocked}{early}"));
        let ids: Vec<&str> = ready(&m).iter().map(|r| r.id.as_str()).collect();
        assert_eq!(
            ids,
            ["LIB-CLI-001", "SACM23-BASE-001"],
            "LIB-TOOL-001: M0 first, blocked row hidden"
        );
    }

    #[test]
    fn lib_tool_001_finds_function_definitions_not_mentions() {
        let names = defined_functions(
            "fn lib_cli_001_a() {}\n// call lib_cli_001_b()\ndef lib_cli_001_c(x):",
        );
        assert_eq!(names, ["lib_cli_001_a", "lib_cli_001_c"]);
        assert!(looks_traced("sacm23_base_001_x"));
        assert!(looks_traced("test_lib_tool_004_x"));
        assert!(!looks_traced("sacm23_base_1_x"));
        assert!(!looks_traced("helper"));
    }

    #[test]
    fn lib_tool_001_only_marked_tests_count_as_traced() {
        let rust = "#[test]\n#[ignore]\nfn lib_cli_001_a() {}\nfn lib_cli_001_helper() {}\n\
                    let s = \"#[test] fn lib_cli_001_in_string() {}\";";
        assert_eq!(traced_tests(rust, false), ["lib_cli_001_a"]);
        let python = "def test_lib_tool_004_denies(self):\ndef lib_tool_004_helper():";
        assert_eq!(traced_tests(python, true), ["test_lib_tool_004_denies"]);
    }
}
