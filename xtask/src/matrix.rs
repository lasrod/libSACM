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
//!     fn ignored_functions(&self) -> Vec<(String, String)> {
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

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{RootSource, output_error};

/// The canonical matrix, relative to the repository root.
pub const MATRIX_PATH: &str = "docs/matrix/functionality-matrix.toml";
/// The Markdown rendering of the matrix, relative to the repository root.
pub const RENDERED_PATH: &str = "docs/matrix/functionality-matrix.md";

/// Directories whose Rust and Python files are scanned for traced test
/// functions (see [`is_scanned_test_file`]).
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
///
/// The scanned test files are the files under `crates/`, `xtask/` or
/// `.claude/hooks/` whose extension is `.rs` or `.py`, in any case, and that
/// are not under build output. Build output is a directory named `target`
/// with a `Cargo.toml` directly beside it; [`check`] asks [`Repo::exists`]
/// for that `Cargo.toml`. Any other `target` directory, such as a module
/// directory `src/target/`, is not build output. [`check`] holds a test
/// citation of any other file to be a row problem, so every cited test is
/// one that [`Repo::traced_functions`] or [`Repo::ignored_functions`] can
/// list.
pub trait Repo {
    /// Whether a file or directory exists at `rel`.
    fn exists(&self, rel: &str) -> bool;
    /// The text of the file at `rel`, or `None` if it cannot be read.
    fn read(&self, rel: &str) -> Option<String>;
    /// Every test function in the scanned test files whose name traces to a
    /// row and that runs (see [`traced_tests`]), as `(file, name)` pairs.
    fn traced_functions(&self) -> Vec<(String, String)>;
    /// Every test function in the scanned test files whose name traces to a
    /// row and that is ignored or skipped (see [`ignored_traced_tests`]), as
    /// `(file, name)` pairs. Such a test is never evidence: [`check`] reports
    /// each one.
    fn ignored_functions(&self) -> Vec<(String, String)>;
}

/// [`Repo`] over a checkout on disk. Traced tests, running or ignored, are
/// collected from the scanned test files (see [`Repo`]): the Rust and Python
/// sources under `crates/`, `xtask/` and `.claude/hooks/`.
///
/// Build output is skipped: a directory named `target` is not entered when a
/// `Cargo.toml` sits directly beside it, as with the workspace's `target/`
/// or a package's. Any other `target` directory is scanned, so a test in a
/// module directory such as `crates/x/src/target/` or
/// `crates/x/tests/target/` is found.
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

    /// `(file, name)` for every name `find` returns for a scanned test file
    /// (see [`is_scanned_test_file`]), read as Python when its extension is
    /// `.py`.
    fn scan(&self, find: fn(&str, bool) -> Vec<String>) -> Vec<(String, String)> {
        let mut files = Vec::new();
        for scan_root in TEST_SCAN_ROOTS {
            self.walk(scan_root, &mut files);
        }
        let mut found = Vec::new();
        for (file, rel) in files {
            let Ok(text) = fs::read_to_string(&file) else {
                continue;
            };
            for name in find(&text, has_extension(&file, "py")) {
                found.push((rel.clone(), name));
            }
        }
        found
    }

    /// The scanned test files under the repository-relative directory `dir`,
    /// each as its path on disk and its path relative to the root. Build
    /// output (see [`is_build_output`]) is not entered.
    fn walk(&self, dir: &str, out: &mut Vec<(PathBuf, String)>) {
        let Ok(entries) = fs::read_dir(self.root.join(dir)) else {
            return;
        };
        for entry in entries.flatten() {
            let rel = format!("{dir}/{}", entry.file_name().to_string_lossy());
            let path = entry.path();
            if path.is_dir() {
                if !is_build_output(&rel, self) {
                    self.walk(&rel, out);
                }
            } else if is_scanned_test_file(&rel, self) {
                out.push((path, rel));
            }
        }
    }
}

/// Whether the repository-relative file `rel` is a scanned test file: it is
/// under one of [`TEST_SCAN_ROOTS`], its extension is `.rs` or `.py` in any
/// case, and no directory above it is build output (see
/// [`is_build_output`]). The walk of [`FsRepo`] lists exactly these files,
/// and a `path::function` citation of any other file is a row problem.
fn is_scanned_test_file(rel: &str, repo: &dyn Repo) -> bool {
    let path = Path::new(rel);
    TEST_SCAN_ROOTS
        .iter()
        .any(|root| rel.strip_prefix(root).is_some_and(|r| r.starts_with('/')))
        && (has_extension(path, "rs") || has_extension(path, "py"))
        && !rel
            .match_indices('/')
            .any(|(slash, _)| is_build_output(&rel[..slash], repo))
}

/// Whether the repository-relative directory `dir` is build output: it is
/// named `target` and a `Cargo.toml` sits directly beside it, as with the
/// workspace's own `target/` or a package's. Any other `target` directory,
/// such as a `src/target/` module directory, is not.
fn is_build_output(dir: &str, repo: &dyn Repo) -> bool {
    let name_start = dir.rfind('/').map_or(0, |slash| slash + 1);
    let (parent, name) = dir.split_at(name_start);
    name == "target" && repo.exists(&format!("{parent}Cargo.toml"))
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
        self.scan(traced_tests)
    }

    fn ignored_functions(&self) -> Vec<(String, String)> {
        self.scan(ignored_traced_tests)
    }
}

// ---------------------------------------------------------------------------
// The traced-test scanner
// ---------------------------------------------------------------------------

/// Test functions in `text` whose names trace to a matrix row and that run,
/// in source order. `python` selects the Python rules; otherwise `text` is
/// read as Rust.
///
/// A name traces when, with any `test_` prefix removed, it starts
/// `sacm23_<area>_<nnn>` or `lib_<area>_<nnn>`. A traced test that is
/// ignored or skipped is not listed here but by [`ignored_traced_tests`].
///
/// # Comments and literals
///
/// Text inside a comment or a literal is never read: a `#[test]`, `fn` or
/// `def` there neither marks nor defines anything. The line breaks inside
/// them still count, so a literal that spans lines does not join the lines
/// around it. A literal or comment that is never closed runs to the end of
/// the text.
///
/// - **Rust:** line and doc comments (`//`, `///`, `//!`); block comments
///   (`/* */`, `/** */`, `/*! */`), which nest; strings (`"..."`, `b"..."`,
///   `c"..."`), in which a backslash escapes the next character; raw strings
///   (`r"..."`, and `r#"..."#` with any number of `#`, also with `br` and
///   `cr`), which end only at a quote followed by as many `#`; and char and
///   byte literals (`'"'`, `'\''`, `b'"'`). A lifetime or label (`'a`,
///   `'static`) and a raw identifier (`r#match`) open nothing.
/// - **Python:** `#` comments, and strings in single or double quotes, single
///   or triple (`"""`, `'''`), with an optional prefix (`r`, `b`, `f`, `u`,
///   `rb`, `br`, `fr`, `rf`, in any case). A backslash escapes the next
///   character, in raw strings too, so `\"` never ends a string.
///
/// # Rust tests
///
/// A Rust test is a function under a test-style attribute: one whose path
/// ends in the segment `test`, `test_case` or `rstest`, with or without
/// arguments (`#[test]`, `#[tokio::test(flavor = "multi_thread")]`,
/// `#[test_case(1)]`, `#[rstest]`). The attribute is on a line before the
/// function or on the same line. Other attributes, blank lines and comments
/// may sit in between, and an attribute may span lines. The `fn` must be on
/// the first line after the attributes. The test is ignored when one of its
/// attributes is `ignore` (`#[ignore]`, `#[ignore = "..."]`), or is
/// `cfg_attr` and lists `ignore` after its predicate
/// (`#[cfg_attr(miri, ignore)]`).
///
/// # Python tests
///
/// A Python test is a line-leading `def test_...` or `async def test_...`, at
/// any indent. Any run of spaces or tabs may separate `async` and `def`. The
/// test is skipped under a decorator whose dotted name ends in `skip`,
/// `skipIf`, `skipUnless`, `skipif` or `expectedFailure`, in two cases:
///
/// - the decorator is on the lines directly above the test, where blank
///   lines, comment lines and other decorators may sit between;
/// - the decorator is on a `class` that the test is indented under. A
///   skipped class ends at the next line of code at the class's indent or
///   less. Comment lines, and lines that hold only string content, do not
///   end it.
///
/// A decorator's arguments may span lines.
///
/// # Known limits
///
/// - A macro that generates tests (`macro_rules!`, or a procedural macro
///   other than the attributes above) does not put them in the text, so they
///   are not found.
/// - A Python test that is skipped at run time (`self.skipTest(...)`,
///   `pytest.skip()`) cannot be seen, so it counts as running.
/// - A Python line continued with a backslash, rather than inside brackets,
///   is read as two lines. A tab in a Python indent counts as one column.
/// - A Rust attribute after other code on its line is not read, and a
///   `cfg_attr` nested in a `cfg_attr` does not ignore.
/// - A prelude-qualified `ignore` (`#[::core::prelude::v1::ignore]`,
///   `#[std::prelude::v1::ignore]`) does not ignore, so its test counts as
///   running (LIB-TOOL-016).
///
/// ```
/// use xtask::matrix::traced_tests;
///
/// let rust = "#[test]\nfn lib_cli_001_parses() {}\nfn lib_cli_001_helper() {}";
/// assert_eq!(traced_tests(rust, false), ["lib_cli_001_parses"]);
/// assert_eq!(traced_tests("#[test] fn sacm23_base_001_x() {}", false), ["sacm23_base_001_x"]);
/// assert!(traced_tests("/* #[test] fn lib_cli_001_x() {} */", false).is_empty());
/// // Brackets in an attribute nest, so its first `]` need not close it.
/// let case = "#[test_case([1, 2])]\nfn lib_cli_001_pair(pair: [u8; 2]) {}";
/// assert_eq!(traced_tests(case, false), ["lib_cli_001_pair"]);
///
/// let python = "def test_lib_tool_003_denies(self):\ndef lib_tool_003_helper():";
/// assert_eq!(traced_tests(python, true), ["test_lib_tool_003_denies"]);
/// ```
#[must_use]
pub fn traced_tests(text: &str, python: bool) -> Vec<String> {
    traced(text, python, false)
}

/// Test functions in `text` whose names trace to a matrix row and that are
/// ignored (Rust) or skipped (Python), in source order. The forms
/// recognised, and their limits, are those of [`traced_tests`].
///
/// ```
/// use xtask::matrix::{ignored_traced_tests, traced_tests};
///
/// let rust = "#[test]\n#[ignore = \"slow\"]\nfn lib_cli_001_slow() {}";
/// assert_eq!(ignored_traced_tests(rust, false), ["lib_cli_001_slow"]);
/// assert!(traced_tests(rust, false).is_empty());
///
/// let python = "@unittest.skip(\"x\")\ndef test_lib_tool_003_x(self):";
/// assert_eq!(ignored_traced_tests(python, true), ["test_lib_tool_003_x"]);
/// ```
#[must_use]
pub fn ignored_traced_tests(text: &str, python: bool) -> Vec<String> {
    traced(text, python, true)
}

/// The traced tests in `text` that are ignored, or the ones that run.
fn traced(text: &str, python: bool, ignored: bool) -> Vec<String> {
    test_functions(&mask(text, python), python)
        .into_iter()
        .filter(|test| test.ignored == ignored && looks_traced(&test.name))
        .map(|test| test.name)
        .collect()
}

/// A test function the scanner found.
struct Test {
    name: String,
    /// Ignored (Rust) or skipped (Python).
    ignored: bool,
}

/// Every test function in `masked`, a text that [`mask`] masked, whether or
/// not its name traces to a row, in source order.
fn test_functions(masked: &str, python: bool) -> Vec<Test> {
    if python {
        python_tests(masked)
    } else {
        rust_tests(masked)
    }
}

/// `text` with every comment and literal, delimiters included, blanked: each
/// character in them that is not whitespace becomes a space. The masked text
/// has the same lines as `text`. See [`traced_tests`] for the forms. A
/// string prefix other than a
/// Rust raw string's (`b"`, `c"`, `b'`, Python's `rb"`) is left in place:
/// the quote after it opens the literal.
fn mask(text: &str, python: bool) -> String {
    let chars: Vec<char> = text.chars().collect();
    if python {
        mask_python(&chars)
    } else {
        mask_rust(&chars)
    }
}

/// Push `chars[from..to]` to `out`, with every character that is not
/// whitespace replaced by a space, so that each newline stays. `to` may lie
/// past the end of `chars`.
fn blank(chars: &[char], from: usize, to: usize, out: &mut String) {
    for &c in chars.iter().take(to).skip(from) {
        out.push(if c.is_whitespace() { c } else { ' ' });
    }
}

/// Where the identifier ends that starts at `chars[from]`.
fn word_end(chars: &[char], from: usize) -> usize {
    from + chars
        .iter()
        .skip(from)
        .take_while(|c| is_identifier_char(**c))
        .count()
}

/// The index of the newline that ends the line holding `chars[from]`, or the
/// length of `chars`.
fn line_end(chars: &[char], from: usize) -> usize {
    chars
        .iter()
        .skip(from)
        .position(|&c| c == '\n')
        .map_or(chars.len(), |n| from + n)
}

/// Whether `width` copies of `quote` start at `chars[at]`.
fn quote_run(chars: &[char], at: usize, quote: char, width: usize) -> bool {
    chars
        .iter()
        .skip(at)
        .take(width)
        .take_while(|&&c| c == quote)
        .count()
        == width
}

/// The Rust masking of [`mask`]. Each step consumes at least one character,
/// and each literal or comment is read once, so the work is linear.
fn mask_rust(chars: &[char]) -> String {
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        let end = match (c, chars.get(i + 1)) {
            ('/', Some('/')) => line_end(chars, i),
            ('/', Some('*')) => block_comment_end(chars, i + 2),
            ('"', _) => escaped_end(chars, i + 1, '"'),
            ('\'', _) if opens_char_literal(chars, i) => escaped_end(chars, i + 1, '\''),
            _ if is_identifier_char(c) => {
                let after = word_end(chars, i);
                let word = &chars[i..after];
                if let Some(end) = raw_string_end(word, chars, after) {
                    end
                } else {
                    out.extend(word);
                    i = after;
                    continue;
                }
            }
            _ => {
                out.push(c);
                i += 1;
                continue;
            }
        };
        blank(chars, i, end, &mut out);
        i = end;
    }
    out
}

/// Where a Rust block comment ends whose body starts at `chars[from]`.
/// Comments nest, so it ends after the `*/` that closes the first `/*`.
fn block_comment_end(chars: &[char], from: usize) -> usize {
    let mut depth = 1_usize;
    let mut j = from;
    while let Some(&c) = chars.get(j) {
        match (c, chars.get(j + 1)) {
            ('/', Some('*')) => {
                depth += 1;
                j += 2;
            }
            ('*', Some('/')) => {
                depth -= 1;
                j += 2;
                if depth == 0 {
                    return j;
                }
            }
            _ => j += 1,
        }
    }
    j
}

/// Where a literal ends whose body starts at `chars[from]`: after the next
/// `quote` that no backslash escapes. Past the end of `chars` when the text
/// ends first.
fn escaped_end(chars: &[char], from: usize, quote: char) -> usize {
    let mut j = from;
    while let Some(&c) = chars.get(j) {
        j += if c == '\\' { 2 } else { 1 };
        if c == quote {
            return j;
        }
    }
    j
}

/// Where a raw string ends whose body starts at `chars[from]`: after the
/// first quote that `hashes` `#` follow.
fn raw_end(chars: &[char], from: usize, hashes: usize) -> usize {
    let mut j = from;
    while let Some(&c) = chars.get(j) {
        j += 1;
        if c == '"' && quote_run(chars, j, '#', hashes) {
            return j + hashes;
        }
    }
    j
}

/// Whether the `'` at `chars[at]` opens a char literal rather than a
/// lifetime or a label: an escape follows it, or a quote closes it after one
/// character.
fn opens_char_literal(chars: &[char], at: usize) -> bool {
    matches!(chars.get(at + 1..at + 3), Some(['\\', _] | [_, '\'']))
}

/// Where the raw string ends that starts at `chars[after]`, when the Rust
/// identifier `word` before it is a raw string's prefix (`r`, `br`, `cr`)
/// and `#`s and a quote follow it; `None` otherwise. (The prefixes `b` and
/// `c` of other literals need no reading of their own: the quote after them
/// opens the literal.)
fn raw_string_end(word: &[char], chars: &[char], after: usize) -> Option<usize> {
    if !matches!(word, ['r'] | ['b' | 'c', 'r']) {
        return None;
    }
    let hashes = chars.iter().skip(after).take_while(|&&c| c == '#').count();
    let quote = after + hashes;
    (chars.get(quote) == Some(&'"')).then(|| raw_end(chars, quote + 1, hashes))
}

/// The Python masking of [`mask`]. Each step consumes at least one
/// character, and each string or comment is read once, so the work is
/// linear. Every quote outside a comment or a string opens a string, so a
/// prefix needs no reading of its own.
fn mask_python(chars: &[char]) -> String {
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        let end = match c {
            '#' => line_end(chars, i),
            '"' | '\'' => python_string_end(chars, i, c),
            _ => {
                out.push(c);
                i += 1;
                continue;
            }
        };
        blank(chars, i, end, &mut out);
        i = end;
    }
    out
}

/// Where the Python string ends that the `quote` at `chars[at]` opens. Three
/// quotes open a triple-quoted string, which only three close. A backslash
/// escapes the next character.
fn python_string_end(chars: &[char], at: usize, quote: char) -> usize {
    let width = if quote_run(chars, at, quote, 3) { 3 } else { 1 };
    let mut j = at + width;
    while let Some(&c) = chars.get(j) {
        if quote_run(chars, j, quote, width) {
            return j + width;
        }
        j += if c == '\\' { 2 } else { 1 };
    }
    j
}

/// The Rust tests in masked text, by the rules of [`traced_tests`].
fn rust_tests(masked: &str) -> Vec<Test> {
    let mut tests = Vec::new();
    let mut marks = Marks::default();
    // The body of an attribute still open at the end of a line, and its
    // bracket depth.
    let mut open: Option<(String, usize)> = None;
    for line in masked.lines() {
        let mut rest = line.trim();
        loop {
            if let Some((mut body, mut depth)) = open.take() {
                let Some(close) = attribute_close(rest, &mut depth) else {
                    body.push_str(rest);
                    body.push(' ');
                    open = Some((body, depth));
                    rest = "";
                    break;
                };
                body.push_str(&rest[..close]);
                marks.add(&body);
                rest = rest[close + 1..].trim_start();
            } else if let Some(after) = rest.strip_prefix("#[") {
                open = Some((String::new(), 1));
                rest = after;
            } else {
                break;
            }
        }
        if rest.is_empty() {
            // Attributes, blank lines and comments may sit between `#[test]`
            // and the function.
            continue;
        }
        if marks.test {
            if let Some(name) = defined_functions(rest).into_iter().next() {
                tests.push(Test {
                    name,
                    ignored: marks.ignore,
                });
            }
        }
        marks = Marks::default();
    }
    tests
}

/// What the attributes above a Rust item say about it.
#[derive(Default)]
struct Marks {
    /// One of them is test-style.
    test: bool,
    /// One of them is ignore-style.
    ignore: bool,
}

impl Marks {
    /// Take in the attribute whose text between `#[` and `]` is `body`.
    fn add(&mut self, body: &str) {
        let (path, args) = attribute_path(body);
        self.test |= matches!(
            path.rsplit(':').next(),
            Some("test" | "test_case" | "rstest")
        );
        self.ignore |= path == "ignore"
            || (path == "cfg_attr"
                && args.trim_start().strip_prefix('(').is_some_and(|list| {
                    top_level_items(list)
                        .into_iter()
                        .skip(1)
                        .any(|item| attribute_path(item).0 == "ignore")
                }));
    }
}

/// Where the attribute open at `depth` brackets closes in `text`: the index
/// of its `]`. Brackets inside it nest.
fn attribute_close(text: &str, depth: &mut usize) -> Option<usize> {
    text.char_indices().find_map(|(i, c)| {
        match c {
            '[' => *depth += 1,
            ']' => *depth -= 1,
            _ => {}
        }
        (*depth == 0).then_some(i)
    })
}

/// The path that starts an attribute body (`tokio::test`), and the rest of
/// the body.
fn attribute_path(body: &str) -> (&str, &str) {
    let body = body.trim_start();
    let end = body
        .find(|c: char| !is_identifier_char(c) && c != ':')
        .unwrap_or(body.len());
    body.split_at(end)
}

/// The comma-separated items at the top level of `list`, up to the bracket
/// that closes the list. An item that no comma or closing bracket ends is
/// left out.
fn top_level_items(list: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let (mut depth, mut start) = (0_usize, 0);
    for (i, c) in list.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => {
                items.push(&list[start..i]);
                return items;
            }
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                items.push(&list[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    items
}

/// The last segments of the decorator names that skip a Python test.
const SKIP_DECORATORS: [&str; 5] = ["skip", "skipIf", "skipUnless", "skipif", "expectedFailure"];

/// The Python tests in masked text, by the rules of [`traced_tests`].
fn python_tests(masked: &str) -> Vec<Test> {
    let mut tests = Vec::new();
    // Brackets that earlier lines left open: the line continues theirs.
    let mut depth = 0_usize;
    // A skip decorator sits above the next `def` or `class`.
    let mut skip_next = false;
    // The indent of the skipped class the line is in.
    let mut skipped_class: Option<usize> = None;
    for line in masked.lines() {
        let continues = depth > 0;
        depth = line.chars().fold(depth, |depth, c| match c {
            '(' | '[' | '{' => depth + 1,
            ')' | ']' | '}' => depth.saturating_sub(1),
            _ => depth,
        });
        let code = line.trim_start();
        if continues || code.is_empty() {
            continue;
        }
        let indent = line.len() - code.len();
        if skipped_class.is_some_and(|class| indent <= class) {
            skipped_class = None;
        }
        if let Some(decorator) = code.strip_prefix('@') {
            let name: String = decorator
                .chars()
                .take_while(|&c| is_identifier_char(c) || c == '.')
                .collect();
            skip_next |= name
                .rsplit('.')
                .next()
                .is_some_and(|last| SKIP_DECORATORS.contains(&last));
            continue;
        }
        if let Some(name) = python_def(code) {
            if name.starts_with("test_") {
                tests.push(Test {
                    name,
                    ignored: skip_next || skipped_class.is_some(),
                });
            }
        } else if skip_next && code.starts_with("class ") {
            skipped_class.get_or_insert(indent);
        }
        skip_next = false;
    }
    tests
}

/// The name that a Python `def` or `async def` line defines.
fn python_def(code: &str) -> Option<String> {
    let code = match code.strip_prefix("async") {
        Some(rest) if rest.starts_with([' ', '\t']) => rest.trim_start_matches([' ', '\t']),
        _ => code,
    };
    code.strip_prefix("def ").map(identifier)
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
/// ```
/// use xtask::matrix::load;
///
/// let error = load("schema_version = 1\n[[milestone]]\nid = \"M0\"\n").map(|_| ()).unwrap_err();
/// assert_eq!(
///     error,
///     "docs/matrix/functionality-matrix.toml:2:1: missing field `title`"
/// );
/// ```
///
/// # Errors
///
/// Returns the TOML or schema error as exactly one line,
/// `docs/matrix/functionality-matrix.toml:<line>:<column>: <message>`.
/// `<line>` and `<column>` are 1-based, the column counted in characters,
/// and locate the start of the error's span. `<message>` is the parser's
/// message ([`toml::de::Error::message`]) without its source excerpt, with
/// control characters escaped as by [`char::escape_default`]. An error
/// without a usable span is `docs/matrix/functionality-matrix.toml: <message>`.
/// Unknown keys are errors.
pub fn load(text: &str) -> Result<Matrix, String> {
    toml::from_str(text).map_err(|e| load_error(text, &e))
}

/// The one-line form of `error`, a failure to parse `text`; see [`load`].
fn load_error(text: &str, error: &toml::de::Error) -> String {
    let message = escape_controls(error.message());
    // `get` is `None` for a span start past the end or inside a character.
    let Some(before) = error.span().and_then(|span| text.get(..span.start)) else {
        return format!("{MATRIX_PATH}: {message}");
    };
    let (mut line, mut column) = (1, 1);
    for c in before.chars() {
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    format!("{MATRIX_PATH}:{line}:{column}: {message}")
}

/// Every rule violation in the matrix, as human-readable messages; empty when
/// the matrix passes the gate. Row-level messages start with the row id.
///
/// The rules are those of `docs/matrix/README.md`. The order is
/// deterministic: matrix-wide problems, then each row's in file order, then
/// dependency cycles, then the problems of the traced tests that
/// [`Repo::traced_functions`] and [`Repo::ignored_functions`] list, sorted
/// by file and function name, then the rendered page.
///
/// A traced test has at most one problem. It is under a `src` directory, if
/// a component of its path is `src`; otherwise it is ignored or skipped, if
/// it is; otherwise it is cited by no row, if no row cites it:
///
/// - ``<file>: traced test `<name>` is under a `src` directory; traced tests belong in test files``
/// - ``<file>: traced test `<name>` is ignored or skipped, so it is not evidence``
/// - ``<file>: traced test `<name>` is not cited by any matrix row``
///
/// A `path::function` test citation must name a scanned test file (see
/// [`Repo`]); otherwise the row has the problem
/// `test file <path> is outside the scanned test files`, after the check of
/// the test's name and instead of the citation's other checks, so the file
/// is not looked up. Every cited test is therefore in a scanned file, and a
/// row that cites an ignored or skipped test has no problem for it: the
/// scan problem above reports it. A `doctest:path` citation may name any
/// file, since doctests also live in READMEs and other docs.
///
/// Values are quoted as they are, control characters included;
/// [`cmd_check`] escapes them.
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
        test_files: RefCell::new(BTreeMap::new()),
    };
    for row in &matrix.rows {
        check_row(row, &index, &mut errors);
    }
    check_cycles(matrix, &rows, &mut errors);
    check_scanned_tests(matrix, repo, &mut errors);

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
    /// Each cited test file, read and scanned once per check (`None` when it
    /// cannot be read): many rows cite the same file.
    test_files: RefCell<BTreeMap<String, Option<TestFile>>>,
}

/// A cited test file as the citation checks see it.
struct TestFile {
    /// Its test functions, running or ignored (see [`traced_tests`]).
    tests: Vec<String>,
    /// Every function it defines outside comments and literals.
    defined: Vec<String>,
}

impl TestFile {
    fn scan(text: &str, python: bool) -> Self {
        let masked = mask(text, python);
        Self {
            tests: test_functions(&masked, python)
                .into_iter()
                .map(|test| test.name)
                .collect(),
            defined: defined_functions(&masked),
        }
    }
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
/// paths exist; cited tests are named for this row, are in a scanned test
/// file (see [`is_scanned_test_file`]), exist, and are test functions (see
/// [`traced_tests`]). A rejected path, or a test file outside the scan, is
/// reported and its citation's other checks are skipped, except the test
/// name check.
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
        // Only a scanned test file gets the scan problems of
        // `check_scanned_tests`, so a cited test must be in one.
        if !is_scanned_test_file(path, index.repo) {
            out.push(format!(
                "test file {path} is outside the scanned test files"
            ));
            continue;
        }
        let python = has_extension(Path::new(path), "py");
        let mut test_files = index.test_files.borrow_mut();
        let file = test_files.entry(path.to_owned()).or_insert_with(|| {
            index
                .repo
                .read(path)
                .map(|text| TestFile::scan(&text, python))
        });
        let Some(file) = file else {
            out.push(format!("test file {path} does not exist"));
            continue;
        };
        // An ignored or skipped test is still a test here: the scan problems
        // of `check_scanned_tests` report it once, whether or not it is cited.
        if file.tests.iter().any(|test| test == name) {
            continue;
        }
        if !file.defined.iter().any(|f| f == name) {
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

/// The problems of the traced tests that the repository scan finds, at most
/// one per test, in this order of precedence:
///
/// 1. A traced test in a file with a path component `src` is in production
///    source, where the implementer can add or change it, whether or not it
///    runs or is cited.
/// 2. An ignored or skipped traced test is not evidence, whether or not it is
///    cited.
/// 3. A traced test that no row cites is invisible to everyone downstream.
///
/// Reported by file and then by function name, whatever order the repository
/// lists them in.
fn check_scanned_tests(matrix: &Matrix, repo: &dyn Repo, errors: &mut Vec<String>) {
    let cited: HashSet<(&str, &str)> = matrix
        .rows
        .iter()
        .flat_map(|r| r.tests.iter())
        .filter_map(|t| t.split_once("::"))
        .collect();
    let running = repo
        .traced_functions()
        .into_iter()
        .map(|test| (test, false));
    let ignored = repo
        .ignored_functions()
        .into_iter()
        .map(|test| (test, true));
    let mut problems = Vec::new();
    for ((file, name), ignored) in running.chain(ignored) {
        let problem = if file.split('/').any(|component| component == "src") {
            "is under a `src` directory; traced tests belong in test files"
        } else if ignored {
            "is ignored or skipped, so it is not evidence"
        } else if cited.contains(&(file.as_str(), name.as_str())) {
            continue;
        } else {
            "is not cited by any matrix row"
        };
        problems.push((file, name, problem));
    }
    problems.sort_unstable();
    for (file, name, problem) in problems {
        errors.push(format!("{file}: traced test `{name}` {problem}"));
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

/// Read and [`load`] the matrix under `root`. A read error is one line,
/// `read <path>: <error>`, with control characters escaped.
fn load_from(root: &Path) -> Result<Matrix, String> {
    let path = root.join(MATRIX_PATH);
    let text = fs::read_to_string(&path)
        .map_err(|e| escape_controls(&format!("read {}: {e}", path.display())))?;
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
/// The root is not named in the report: this is the check of a root from
/// [`RootSource::Build`]. [`run_from`](crate::run_from) with
/// [`RootSource::Environment`] adds ` (root: <root>)` to the report's first
/// line.
///
/// # Errors
///
/// Returns an error when the matrix cannot be read or parsed, or when writing
/// to `out` fails. A matrix that cannot be read is one line,
/// `read <path>: <error>`, with control characters escaped; one that cannot
/// be parsed is the one line [`load`] returns. When the matrix has problems,
/// the error's first line is `functionality matrix: <n> problem(s)`, followed
/// by each problem on a line of its own, indented by two spaces, in the order
/// [`check`] returns them. Control characters in a problem are escaped (for
/// example `\n` and `\u{1b}`), so that each problem takes exactly one line.
pub fn cmd_check(root: &Path, out: &mut dyn io::Write) -> Result<(), String> {
    cmd_check_from(root, RootSource::Build, out)
}

/// [`cmd_check`] of a root that came from `source`. With
/// [`RootSource::Environment`], the first line of the report (the OK line,
/// the problem-count header or the load error) ends with ` (root: <root>)`,
/// `<root>` shown by [`Path::display`] with control characters escaped.
pub(crate) fn cmd_check_from(
    root: &Path,
    source: RootSource,
    out: &mut dyn io::Write,
) -> Result<(), String> {
    let named = match source {
        RootSource::Build => String::new(),
        RootSource::Environment => {
            format!(" (root: {})", escape_controls(&root.display().to_string()))
        }
    };
    let matrix = load_from(root).map_err(|e| e + &named)?;
    let problems = check(&matrix, &FsRepo::new(root));
    if problems.is_empty() {
        return writeln!(
            out,
            "functionality matrix OK: {} rows{named}",
            matrix.rows.len()
        )
        .map_err(output_error);
    }
    let mut report = format!("functionality matrix: {} problem(s){named}", problems.len());
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
