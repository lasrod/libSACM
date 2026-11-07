//! The functionality-matrix gate.
//!
//! `docs/matrix/functionality-matrix.toml` is the canonical record of what
//! libSACM implements, requirement by requirement. This module checks it
//! against the repository, renders it to Markdown, and answers "what is the
//! next work?" for the slice loop. The rules it enforces are documented in
//! `docs/matrix/README.md`; keep the two in step.
//!
//! The checks run against the [`Repo`] trait rather than the filesystem, so
//! they can be driven with an in-memory repository:
//!
//! ```
//! use xtask::matrix::{check, load, render, traced_tests, Repo, RENDERED_PATH};
//!
//! /// A repository holding only the rendered page.
//! struct OnlyPage(String);
//!
//! impl Repo for OnlyPage {
//!     fn exists(&self, rel: &str) -> bool {
//!         rel == RENDERED_PATH
//!     }
//!     fn read(&self, rel: &str) -> Option<String> {
//!         (rel == RENDERED_PATH).then(|| self.0.clone())
//!     }
//!     fn traced_functions(&self) -> Vec<(String, String)> {
//!         Vec::new()
//!     }
//! }
//!
//! let matrix = load("schema_version = 1")?;
//! assert!(check(&matrix, &OnlyPage(render(&matrix))).is_empty());
//! assert_eq!(
//!     traced_tests("#[test] fn lib_cli_001_runs() {}", false),
//!     ["lib_cli_001_runs"]
//! );
//! # Ok::<(), String>(())
//! ```

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::output_error;

/// The canonical matrix, relative to the repository root.
pub const MATRIX_PATH: &str = "docs/matrix/functionality-matrix.toml";
/// The Markdown rendering of the matrix, relative to the repository root.
pub const RENDERED_PATH: &str = "docs/matrix/functionality-matrix.md";

/// Directories scanned for traced test functions (orphan detection).
const TEST_SCAN_ROOTS: &[&str] = &["crates", "xtask", ".claude/hooks"];

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

/// The parsed `functionality-matrix.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Matrix {
    /// Format version of the file; only `1` is supported.
    pub schema_version: u32,
    /// Milestones in delivery order (`[[milestone]]` tables).
    #[serde(rename = "milestone", default)]
    pub milestones: Vec<Milestone>,
    /// Requirement areas (`[[area]]` tables).
    #[serde(rename = "area", default)]
    pub areas: Vec<Area>,
    /// Requirement rows in file order (`[[row]]` tables).
    #[serde(rename = "row", default)]
    pub rows: Vec<Row>,
}

/// A delivery milestone such as `M0`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Milestone {
    /// Unique milestone id, e.g. `M0`.
    pub id: String,
    /// Short title.
    pub title: String,
    /// What the milestone delivers.
    pub goal: String,
}

/// A requirement area: the middle segment of a row id.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Area {
    /// Unique uppercase area code, e.g. `ARG`.
    pub code: String,
    /// The id prefix that rows of this area must use.
    pub prefix: Prefix,
    /// Human-readable area name.
    pub title: String,
}

/// `SACM23` rows are obligations of the standard; `LIB` rows are library
/// capabilities and quality commitments the standard does not state.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Prefix {
    /// An obligation of OMG SACM 2.3 (`SACM23-...`).
    #[serde(rename = "SACM23")]
    Sacm23,
    /// A library capability or quality commitment (`LIB-...`).
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

/// One requirement of the matrix.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// `SACM23-<AREA>-<NNN>` or `LIB-<AREA>-<NNN>`.
    pub id: String,
    /// Area code; must equal the id's area segment.
    pub area: String,
    /// Milestone id.
    pub milestone: String,
    /// Short title.
    pub title: String,
    /// The requirement itself.
    pub requirement: String,
    /// Where the requirement comes from (a clause of the standard, an ADR, ...).
    pub source: String,
    /// The kind of work.
    pub kind: Kind,
    /// Where the row stands.
    pub status: Status,
    /// Rows that must be verified before this one can be.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Acceptance criteria.
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Paths of the code that implements the row.
    #[serde(default)]
    pub code: Vec<String>,
    /// Test citations: `path::function` or `doctest:path`.
    #[serde(default)]
    pub tests: Vec<String>,
    /// Path of the verification record that passed the row.
    #[serde(default)]
    pub verification: Option<String>,
    /// Free-form notes; required for unscheduled statuses.
    #[serde(default)]
    pub notes: Option<String>,
}

/// The status vocabulary of `docs/matrix/README.md`.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Scheduled; not analysed yet.
    Planned,
    /// Acceptance criteria are written.
    Analyzed,
    /// Failing tests encode the criteria.
    TestsFailing,
    /// Tests pass; not yet independently verified.
    Implemented,
    /// The verifier passed it; recorded.
    Verified,
    /// Under consideration, not scheduled.
    Candidate,
    /// Scheduled for later than its milestone suggests.
    Deferred,
    /// Deliberately not done.
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

    /// The status as written in the matrix, e.g. `tests-failing`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
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

/// The kind of work a row describes.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Typed model of metamodel elements.
    Model,
    /// XMI import or export.
    Xmi,
    /// Validation rules.
    Validation,
    /// Editing operations.
    Editing,
    /// Public API shape.
    Api,
    /// Security properties.
    Security,
    /// The command-line tool.
    Cli,
    /// Documentation.
    Docs,
    /// Quality gates.
    Quality,
    /// Repository tooling.
    Tooling,
    /// Conformance to the standard.
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

/// The repository as the checks see it. Paths are relative to the repository
/// root and use `/` separators.
pub trait Repo {
    /// Whether a file or directory exists at `rel`.
    fn exists(&self, rel: &str) -> bool;
    /// The text of the file at `rel`, or `None` if it cannot be read.
    fn read(&self, rel: &str) -> Option<String>;
    /// Every test function in the scanned test roots whose name traces to a
    /// row (see [`traced_tests`]), as `(file, name)` pairs.
    fn traced_functions(&self) -> Vec<(String, String)>;
}

/// [`Repo`] over a checkout on disk. Traced tests are collected from the
/// Rust and Python sources under `crates/`, `xtask/` and `.claude/hooks/`.
#[derive(Debug)]
pub struct FsRepo {
    root: PathBuf,
}

impl FsRepo {
    /// A repository rooted at `root`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
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

/// Test functions in `text` whose names trace to a matrix row, in source
/// order.
///
/// A Rust test is a function under a `#[test]`-style attribute (`#[test]`,
/// `#[tokio::test]`, ...), on the line before the function or on the same
/// line; other attributes, blank lines and comments may sit in between. With
/// `python`, a test is a line-leading `def test_...`. A name traces when, with
/// any `test_` prefix removed, it starts `sacm23_<area>_<nnn>` or
/// `lib_<area>_<nnn>`.
///
/// ```
/// use xtask::matrix::traced_tests;
///
/// let rust = "#[test]\nfn lib_cli_001_parses() {}\nfn lib_cli_001_helper() {}";
/// assert_eq!(traced_tests(rust, false), ["lib_cli_001_parses"]);
/// assert_eq!(traced_tests("#[test] fn sacm23_base_001_x() {}", false), ["sacm23_base_001_x"]);
///
/// let python = "def test_lib_tool_003_denies(self):\ndef lib_tool_003_helper():";
/// assert_eq!(traced_tests(python, true), ["test_lib_tool_003_denies"]);
/// ```
#[must_use]
pub fn traced_tests(text: &str, python: bool) -> Vec<String> {
    test_functions(text, python)
        .into_iter()
        .filter(|name| looks_traced(name))
        .collect()
}

/// Every test function in `text`, whether or not its name traces to a row.
fn test_functions(text: &str, python: bool) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending_test_attr = false;
    for line in text.lines() {
        let line = line.trim();
        if python {
            if let Some(rest) = line.strip_prefix("def ") {
                let name = identifier(rest);
                if name.starts_with("test_") {
                    names.push(name);
                }
            }
            continue;
        }
        let (test_attr, rest) = strip_attributes(line);
        if rest.is_empty() || rest.starts_with("//") {
            // Attributes and comments may sit between `#[test]` and the function.
            pending_test_attr |= test_attr;
            continue;
        }
        if pending_test_attr || test_attr {
            if let Some(name) = defined_functions(rest).into_iter().next() {
                names.push(name);
            }
        }
        pending_test_attr = false;
    }
    names
}

/// Strip the attributes that start a (trimmed) Rust line. Returns whether one
/// of them is `#[test]`-style, and the rest of the line. An attribute that is
/// not closed on this line takes the rest of the line.
fn strip_attributes(line: &str) -> (bool, &str) {
    let mut test_attr = false;
    let mut rest = line;
    while let Some(body) = rest.strip_prefix("#[") {
        let mut depth = 1_usize;
        let close = body.char_indices().find_map(|(i, c)| {
            match c {
                '[' => depth += 1,
                ']' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(i)
        });
        let Some(close) = close else {
            return (test_attr, "");
        };
        test_attr |= body[..close].trim_end().ends_with("test");
        rest = body[close + 1..].trim_start();
    }
    (test_attr, rest)
}

/// Whether `c` can be part of an identifier: a letter, a digit or `_`.
fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The identifier at the start of `text`.
fn identifier(text: &str) -> String {
    text.chars()
        .take_while(|c| is_identifier_char(*c))
        .collect()
}

/// Names following `fn ` or `def ` in source text: all `fn` names in text
/// order, then all `def` names.
///
/// A keyword counts only where it starts a word (`myfn x` defines nothing),
/// and only when an identifier follows it. The occurrences come from
/// [`str::match_indices`], so the scan always terminates: no index arithmetic
/// decides where the next search starts. Neither keyword can overlap itself,
/// so non-overlapping matches are all the matches.
fn defined_functions(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for keyword in ["fn ", "def "] {
        for (pos, _) in text.match_indices(keyword) {
            let (before, from_keyword) = text.split_at(pos);
            let starts_word = !before.chars().next_back().is_some_and(is_identifier_char);
            let name = identifier(&from_keyword[keyword.len()..]);
            if starts_word && !name.is_empty() {
                names.push(name);
            }
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

/// Parse the text of `functionality-matrix.toml`.
///
/// # Errors
///
/// Returns the TOML or schema error, prefixed with [`MATRIX_PATH`]. Unknown
/// keys are errors.
pub fn load(text: &str) -> Result<Matrix, String> {
    toml::from_str(text).map_err(|e| format!("{MATRIX_PATH}: {e}"))
}

/// Every rule violation in the matrix, as human-readable messages; empty when
/// the matrix passes the gate. Row-level messages start with the row id.
///
/// The rules are those of `docs/matrix/README.md`. The order is
/// deterministic: matrix-wide problems, then each row's in file order, then
/// dependency cycles, then uncited traced tests sorted by file and function
/// name, then the rendered page. Values are quoted as they are, control
/// characters included; [`cmd_check`] escapes them.
#[must_use]
pub fn check(matrix: &Matrix, repo: &dyn Repo) -> Vec<String> {
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
    // One problem per `[[area]]` entry, so a duplicated bad code is reported
    // twice, once for each entry.
    for area in &matrix.areas {
        if area.code.is_empty() || !area.code.chars().all(|c| c.is_ascii_uppercase()) {
            errors.push(format!(
                "area code `{}` must be one or more uppercase ASCII letters",
                area.code
            ));
        }
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
        (Some(record), Status::Verified) => out.extend(check_record(&row.id, record, index.repo)),
        (Some(record), _) => {
            out.push(format!("is {s} but cites a verification record"));
            out.extend(check_path(record).err());
        }
        (None, _) => {}
    }
    if status.needs_reason() && row.notes.as_deref().is_none_or(|n| n.trim().is_empty()) {
        out.push(format!("is {s} and must say why in `notes`"));
    }
    out
}

/// The start of every verdict line: `- Row verdict: <ROW-ID>: <VERDICT>`.
const VERDICT_PREFIX: &str = "- Row verdict:";

/// A line of a verification record that starts with [`VERDICT_PREFIX`].
#[derive(Debug)]
enum VerdictLine<'a> {
    /// `- Row verdict: <id>: <verdict>`, where the verdict is PASS, FAIL or
    /// OPEN.
    WellFormed { id: &'a str, verdict: &'a str },
    /// Any other line starting with [`VERDICT_PREFIX`], by its 1-based line
    /// number.
    Malformed(usize),
}

/// A fence line: 0 to 3 spaces of indent (a tab is not one of them), then a
/// run of 3 or more of one fence character, a backtick or a tilde. Returns
/// the character, the length of the run and the rest of the line.
fn fence(line: &str) -> Option<(char, usize, &str)> {
    let body = line.trim_start_matches(' ');
    if line.len() - body.len() > 3 {
        return None;
    }
    let c = body.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let rest = body.trim_start_matches(c);
    let run = body.len() - rest.len();
    (run >= 3).then_some((c, run, rest))
}

/// The verdict lines of a verification record, in line order.
///
/// A line counts once trailing spaces and tabs are removed, and only when it
/// starts with [`VERDICT_PREFIX`] itself: an indented or block-quoted line
/// does not. Lines inside a fenced code block never count, and the fence lines
/// themselves are not verdict lines. Fences follow `CommonMark`: a [`fence`]
/// line opens a block when none is open; only a fence line of the same
/// character, with a run at least as long and nothing after it but spaces and
/// tabs, closes it; every other line inside is content; and an unclosed block
/// runs to the end of the record.
fn verdict_lines(text: &str) -> Vec<VerdictLine<'_>> {
    let mut open: Option<(char, usize)> = None;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if let Some((c, run)) = open {
            if fence(line).is_some_and(|(close, len, rest)| {
                close == c && len >= run && rest.trim_end_matches([' ', '\t']).is_empty()
            }) {
                open = None;
            }
            continue;
        }
        if let Some((c, run, _)) = fence(line) {
            open = Some((c, run));
            continue;
        }
        let Some(rest) = line
            .trim_end_matches([' ', '\t'])
            .strip_prefix(VERDICT_PREFIX)
        else {
            continue;
        };
        let parsed = rest
            .strip_prefix(' ')
            .and_then(|rest| rest.split_once(": "))
            .filter(|(id, verdict)| !id.is_empty() && matches!(*verdict, "PASS" | "FAIL" | "OPEN"));
        out.push(match parsed {
            Some((id, verdict)) => VerdictLine::WellFormed { id, verdict },
            None => VerdictLine::Malformed(i + 1),
        });
    }
    out
}

/// The verification record of the verified row `id`: its path is inside the
/// repository, it exists, every verdict line in it is well formed, and it has
/// exactly one verdict line for `id`, saying PASS.
fn check_record(id: &str, record: &str, repo: &dyn Repo) -> Vec<String> {
    if let Err(problem) = check_path(record) {
        return vec![problem];
    }
    let Some(text) = repo.read(record) else {
        return vec![format!("verification record {record} does not exist")];
    };
    let mut out = Vec::new();
    let mut verdicts = Vec::new();
    for line in verdict_lines(&text) {
        match line {
            VerdictLine::Malformed(n) => out.push(format!(
                "verification record {record}:{n}: malformed verdict line; expected \
                 `{VERDICT_PREFIX} <ROW-ID>: <PASS|FAIL|OPEN>`"
            )),
            VerdictLine::WellFormed {
                id: line_id,
                verdict,
            } if line_id == id => {
                verdicts.push(verdict);
            }
            VerdictLine::WellFormed { .. } => {}
        }
    }
    match verdicts.as_slice() {
        ["PASS"] => {}
        [] => out.push(format!(
            "verification record {record} has no `{VERDICT_PREFIX} {id}: PASS` line"
        )),
        [verdict] => out.push(format!(
            "verification record {record} gives the verdict {verdict}, not PASS"
        )),
        many => out.push(format!(
            "verification record {record} has {} verdict lines for this row; expected exactly one",
            many.len()
        )),
    }
    out
}

/// `Err` with the problem to report when `path` is not a relative,
/// `/`-separated path inside the repository: it contains `\` or `:`, or has
/// an empty, `.` or `..` component. An empty path and a path starting with
/// `/` both have an empty component. A rejected path must not be asked of the
/// [`Repo`].
fn check_path(path: &str) -> Result<(), String> {
    if path.contains(['\\', ':']) || path.split('/').any(|c| matches!(c, "" | "." | "..")) {
        Err(format!(
            "cited path `{path}` must be a relative path inside the repository"
        ))
    } else {
        Ok(())
    }
}

/// Cited paths are inside the repository (see [`check_path`]); cited code
/// paths exist; cited tests are named for this row, exist, and are test
/// functions (see [`traced_tests`]). A rejected path is reported and its
/// citation's other checks are skipped, except the test name check.
fn check_citations(row: &Row, index: &Index<'_>) -> Vec<String> {
    let mut out = Vec::new();
    for path in &row.code {
        if let Err(problem) = check_path(path) {
            out.push(problem);
        } else if !index.repo.exists(path) {
            out.push(format!("cited code path {path} does not exist"));
        }
    }
    let prefix = test_prefix(&row.id);
    for test in &row.tests {
        if let Some(path) = test.strip_prefix("doctest:") {
            if let Err(problem) = check_path(path) {
                out.push(problem);
                continue;
            }
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
        if let Err(problem) = check_path(path) {
            out.push(problem);
            continue;
        }
        let Some(text) = index.repo.read(path) else {
            out.push(format!("test file {path} does not exist"));
            continue;
        };
        let python = has_extension(Path::new(path), "py");
        if test_functions(&text, python).iter().any(|f| f == name) {
            continue;
        }
        if !defined_functions(&text).iter().any(|f| f == name) {
            out.push(format!("test function `{name}` is not defined in {path}"));
        } else if python {
            out.push(format!(
                "function `{name}` in {path} is not a test: unittest runs only `test_...` methods"
            ));
        } else {
            out.push(format!(
                "function `{name}` in {path} is not a test: it has no `#[test]`-style attribute"
            ));
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
/// Reported by file and then by function name, whatever order the repository
/// lists them in.
fn check_orphans(matrix: &Matrix, repo: &dyn Repo, errors: &mut Vec<String>) {
    let cited: HashSet<(&str, &str)> = matrix
        .rows
        .iter()
        .flat_map(|r| r.tests.iter())
        .filter_map(|t| t.split_once("::"))
        .collect();
    let mut orphans: Vec<(String, String)> = repo
        .traced_functions()
        .into_iter()
        .filter(|(file, name)| !cited.contains(&(file.as_str(), name.as_str())))
        .collect();
    orphans.sort();
    for (file, name) in orphans {
        errors.push(format!(
            "{file}: traced test `{name}` is not cited by any matrix row"
        ));
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

/// The Markdown page for [`RENDERED_PATH`]: a summary, the areas, and one
/// table per milestone. Deterministic for a given matrix.
#[must_use]
pub fn render(matrix: &Matrix) -> String {
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

/// Open rows (planned, analyzed, tests-failing, implemented) whose
/// dependencies are all verified, in milestone order and then file order.
#[must_use]
pub fn ready(matrix: &Matrix) -> Vec<&Row> {
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

/// `text` with every control character ([`char::is_control`]) escaped as by
/// [`char::escape_default`], so that it fits on one line.
fn escape_controls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_control() {
            out.extend(c.escape_default());
        } else {
            out.push(c);
        }
    }
    out
}

/// `cargo xtask matrix check`: run [`check`] on the repository at `root`.
///
/// On success, writes `functionality matrix OK: <n> rows` to `out`. On
/// failure, writes nothing: every problem is in the returned error.
///
/// # Errors
///
/// Returns an error when the matrix cannot be read or parsed, or when writing
/// to `out` fails. When the matrix has problems, the error's first line is
/// `functionality matrix: <n> problem(s)`, followed by each problem on a line
/// of its own, indented by two spaces, in the order [`check`] returns them.
/// Control characters in a problem are escaped (for example `\n` and
/// `\u{1b}`), so that each problem takes exactly one line.
pub fn cmd_check(root: &Path, out: &mut dyn io::Write) -> Result<(), String> {
    let matrix = load_from(root)?;
    let problems = check(&matrix, &FsRepo::new(root));
    if problems.is_empty() {
        return writeln!(out, "functionality matrix OK: {} rows", matrix.rows.len())
            .map_err(output_error);
    }
    let mut report = format!("functionality matrix: {} problem(s)", problems.len());
    for problem in &problems {
        report.push_str("\n  ");
        report.push_str(&escape_controls(problem));
    }
    Err(report)
}

/// `cargo xtask matrix render`: write [`render`] of the matrix to
/// [`RENDERED_PATH`] under `root`, then report `wrote <RENDERED_PATH>` to
/// `out`.
///
/// # Errors
///
/// Returns an error when the matrix cannot be read or parsed, the page
/// cannot be written (`write <path>: ...`), or writing to `out` fails.
pub fn cmd_render(root: &Path, out: &mut dyn io::Write) -> Result<(), String> {
    let matrix = load_from(root)?;
    let path = root.join(RENDERED_PATH);
    fs::write(&path, render(&matrix)).map_err(|e| format!("write {}: {e}", path.display()))?;
    writeln!(out, "wrote {RENDERED_PATH}").map_err(output_error)
}

/// `cargo xtask matrix next`: write up to `limit` rows of [`ready`] to `out`,
/// after a header line counting the ready and open rows.
///
/// # Errors
///
/// Returns an error when the matrix cannot be read or parsed, or writing to
/// `out` fails.
pub fn cmd_next(root: &Path, limit: usize, out: &mut dyn io::Write) -> Result<(), String> {
    let matrix = load_from(root)?;
    let ready = ready(&matrix);
    let open = matrix.rows.iter().filter(|r| r.status.is_open()).count();
    if ready.is_empty() {
        return writeln!(
            out,
            "no ready rows ({open} open rows, all blocked on unverified dependencies)"
        )
        .map_err(output_error);
    }
    writeln!(
        out,
        "{} ready of {open} open rows; next {}:",
        ready.len(),
        limit.min(ready.len())
    )
    .map_err(output_error)?;
    for row in ready.into_iter().take(limit) {
        writeln!(
            out,
            "  {:<17} {:<4} {:<14} {}",
            row.id,
            row.milestone,
            row.status.as_str(),
            row.title
        )
        .map_err(output_error)?;
    }
    Ok(())
}

/// `cargo xtask matrix status`: write one line of status counts per
/// milestone to `out`.
///
/// # Errors
///
/// Returns an error when the matrix cannot be read or parsed, or writing to
/// `out` fails.
pub fn cmd_status(root: &Path, out: &mut dyn io::Write) -> Result<(), String> {
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
        writeln!(
            out,
            "{:<4} {:<44} {verified:>3}/{scheduled:<3} verified  ({})",
            milestone.id,
            milestone.title,
            parts.join(", ")
        )
        .map_err(output_error)?;
    }
    Ok(())
}
