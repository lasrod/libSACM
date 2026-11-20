//! Real SACM files written by other tools, read through the public `sacm`
//! API (LIB-COMPAT-015).
//!
//! `cargo xtask fetch-corpus` puts the files of `xtask::fetch::CORPUS` in
//! `third_party/corpus/`, pinned by SHA-256; they are never committed. Each
//! file is read tolerantly and strictly; a document the tolerant read
//! returns is validated, written back tolerantly and read back, and compared
//! with the document read. The report of all that is deterministic, and it
//! must equal the block between `<!-- corpus-report:start -->` and
//! `<!-- corpus-report:end -->` in `docs/compat/corpus.md`.
//!
//! The corpus is found at `$SACM_CORPUS_DIR` when that is set, and otherwise
//! at `<repo>/third_party/corpus`. When a file is absent, a configured
//! directory fails the test, and otherwise the test prints a SKIPPED notice
//! to stderr and passes. A file with another hash always fails it.
//!
//! Not checked here: CI's wiring (the cached fetch and `SACM_CORPUS_DIR`) and
//! the findings table of `docs/compat/corpus.md`, which the verifier reviews.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sacm::compare::semantic_compare;
use sacm::diagnostic::Diagnostic;
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};
use xtask::fetch::{Artifact, CORPUS, CORPUS_DEST, sha256_hex};

const ROW: &str = "LIB-COMPAT-015";

/// The fetch command, quoted in every message.
const FETCH: &str = "run `cargo xtask fetch-corpus`";

/// The document holding the list and the report, relative to the root.
const CORPUS_DOC: &str = "docs/compat/corpus.md";

const REPORT_START: &str = "<!-- corpus-report:start -->";
const REPORT_END: &str = "<!-- corpus-report:end -->";

// --- Finding the corpus --------------------------------------------------------

/// Where the corpus is, or why the test does not run.
#[derive(Debug, PartialEq, Eq)]
enum Located {
    /// The directory that holds every file.
    Found(PathBuf),
    /// A file is absent and no directory was configured: the notice to
    /// print.
    Skip(String),
}

/// The corpus directory: `configured` (the value of `SACM_CORPUS_DIR`) if
/// set, else `default`. When one of `files` is absent from it: an error when
/// the directory was configured, a skip otherwise.
fn locate(
    configured: Option<OsString>,
    default: PathBuf,
    files: &[Artifact],
) -> Result<Located, String> {
    let (dir, was_configured) = match configured {
        Some(value) if value.is_empty() => {
            return Err(format!(
                "{ROW}: SACM_CORPUS_DIR is set but empty; unset it or {FETCH}"
            ));
        }
        Some(value) => (PathBuf::from(value), true),
        None => (default, false),
    };
    let Some(missing) = files.iter().find(|a| !dir.join(a.file_name).is_file()) else {
        return Ok(Located::Found(dir));
    };
    if was_configured {
        Err(format!(
            "{ROW}: {} is missing; {FETCH}",
            dir.join(missing.file_name).display()
        ))
    } else {
        Ok(Located::Skip(format!(
            "SKIPPED {ROW}: the corpus is not at {}; {FETCH}",
            dir.display()
        )))
    }
}

/// Checks that each of `files` in `dir` has its pinned SHA-256.
fn check_hashes(dir: &Path, files: &[Artifact]) -> Result<(), String> {
    for artifact in files {
        let path = dir.join(artifact.file_name);
        let bytes = fs::read(&path).map_err(|e| format!("{ROW}: read {}: {e}", path.display()))?;
        let actual = sha256_hex(&bytes);
        if actual != artifact.sha256 {
            return Err(format!(
                "{ROW}: {} has SHA-256 {actual}, pinned {}; delete it and {FETCH}",
                path.display(),
                artifact.sha256
            ));
        }
    }
    Ok(())
}

/// The corpus directory for this run, its hashes checked, or `None` after
/// printing a SKIPPED notice. Panics when a configured corpus is incomplete
/// or a file has another hash.
///
/// The notice goes straight to the process's stderr: the test harness
/// captures `eprintln!` and hides it for a passing test, so a skip would
/// look like a pass.
fn corpus() -> Option<PathBuf> {
    let default = xtask::repo_root().join(CORPUS_DEST);
    match locate(std::env::var_os("SACM_CORPUS_DIR"), default, CORPUS) {
        Ok(Located::Found(dir)) => {
            if let Err(message) = check_hashes(&dir, CORPUS) {
                panic!("{message}");
            }
            Some(dir)
        }
        Ok(Located::Skip(notice)) => {
            // Nothing to do if stderr itself is gone.
            let _ = std::io::stderr().write_all(format!("{notice}\n").as_bytes());
            None
        }
        Err(message) => panic!("{message}"),
    }
}

// --- The report ----------------------------------------------------------------

/// `none`, or `<CODE> <severity> <count>` for each code and severity, sorted
/// by code then severity, joined with `, `.
fn codes(diagnostics: &[Diagnostic]) -> String {
    let mut counts: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for diagnostic in diagnostics {
        *counts
            .entry((diagnostic.code().as_str(), diagnostic.severity().as_str()))
            .or_default() += 1;
    }
    if counts.is_empty() {
        return "none".to_owned();
    }
    counts
        .iter()
        .map(|((code, severity), count)| format!("{code} {severity} {count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The name of `error`'s variant.
fn variant(error: &ReadError) -> &'static str {
    match error {
        ReadError::Doctype { .. } => "Doctype",
        ReadError::Malformed { .. } => "Malformed",
        ReadError::InvalidRoot { .. } => "InvalidRoot",
        ReadError::NoPackage { .. } => "NoPackage",
        ReadError::DuplicateId { .. } => "DuplicateId",
        ReadError::InvalidId { .. } => "InvalidId",
        ReadError::Unsupported { .. } => "Unsupported",
        ReadError::RepeatedValue { .. } => "RepeatedValue",
        ReadError::InvalidLiteral { .. } => "InvalidLiteral",
        ReadError::LegacyShape { .. } => "LegacyShape",
        ReadError::MissingType { .. } => "MissingType",
        // A read limit (LIB-SEC-003, slice 0072).
        ReadError::LimitExceeded { .. } => "LimitExceeded",
        _ => "other",
    }
}

/// `refused <Variant>[ <code>][ at line L, column C][ <label>]`, where the
/// label, for `Unsupported` only, is the reason's suffix from its last ` (`.
fn refused(error: &ReadError) -> String {
    let mut text = format!("refused {}", variant(error));
    // Every refusal has a code since slice 0063 (LIB-API-017), so the
    // report's `refused Unsupported` lines gain theirs.
    let _ = write!(text, " {}", error.code());
    if let Some(at) = error.location() {
        let _ = write!(text, " at line {}, column {}", at.line(), at.column());
    }
    if let ReadError::Unsupported { reason, .. } = error {
        if let Some(label) = reason.rfind(" (").and_then(|at| reason.get(at..)) {
            text.push_str(label);
        }
    }
    text
}

/// What a read gives: `read; <codes>`, or the refusal.
fn read_outcome(result: &Result<ReadOutput, ReadError>) -> String {
    match result {
        Ok(output) => format!("read; {}", codes(output.diagnostics())),
        Err(error) => refused(error),
    }
}

fn read_in(bytes: &[u8], mode: Mode) -> Result<ReadOutput, ReadError> {
    xmi::read(bytes, &ReadOptions::default().with_mode(mode))
}

/// What a tolerant write of `output`'s document gives, and whether it reads
/// back equal.
fn write_outcome(output: &ReadOutput) -> (String, bool) {
    let document = output.document();
    let text = match xmi::write(document, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => return (format!("refused: {error}"), false),
    };
    match read_in(text.as_bytes(), Mode::Tolerant) {
        Err(error) => (format!("does not read back: {}", refused(&error)), false),
        Ok(back) => match semantic_compare(document, back.document()).len() {
            0 => ("reads back equal".to_owned(), true),
            1 => ("reads back with 1 difference".to_owned(), false),
            n => (format!("reads back with {n} differences"), false),
        },
    }
}

/// The report on the corpus in `dir`, in [`CORPUS`] order.
fn report(dir: &Path) -> String {
    let total = CORPUS.len();
    let (mut tolerant_reads, mut strict_reads, mut equal) = (0, 0, 0);
    let mut text = format!("corpus: {total} files\n");
    for artifact in CORPUS {
        let path = dir.join(artifact.file_name);
        let bytes =
            fs::read(&path).unwrap_or_else(|e| panic!("{ROW}: read {}: {e}", path.display()));
        let tolerant = read_in(&bytes, Mode::Tolerant);
        let strict = read_in(&bytes, Mode::Strict);
        let _ = write!(
            text,
            "\n{}\n  tolerant read: {}\n  strict read: {}\n",
            artifact.file_name,
            read_outcome(&tolerant),
            read_outcome(&strict)
        );
        if strict.is_ok() {
            strict_reads += 1;
        }
        if let Ok(output) = &tolerant {
            tolerant_reads += 1;
            let (write, reads_back_equal) = write_outcome(output);
            if reads_back_equal {
                equal += 1;
            }
            let _ = write!(
                text,
                "  validate: {}\n  tolerant write: {write}\n",
                codes(&validation::validate(output.document()))
            );
        }
    }
    let _ = writeln!(
        text,
        "\nread tolerantly: {tolerant_reads} of {total}; read strictly: {strict_reads} of {total}; \
         tolerant write reads back equal: {equal} of {tolerant_reads}"
    );
    text
}

// --- docs/compat/corpus.md -----------------------------------------------------

fn corpus_doc() -> String {
    let path = xtask::repo_root().join(CORPUS_DOC);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{ROW}: read {}: {e}", path.display()))
}

/// The text of the ```` ```text ```` fence that follows [`REPORT_START`] in
/// `doc`, up to its closing fence, which [`REPORT_END`] follows. Each line
/// ends with `\n`.
fn recorded_report(doc: &str) -> Result<String, String> {
    let mut lines = doc
        .lines()
        .skip_while(|line| line.trim() != REPORT_START)
        .skip(1)
        .skip_while(|line| line.trim().is_empty());
    if lines.next().map(str::trim_end) != Some("```text") {
        return Err(format!(
            "{ROW}: {CORPUS_DOC} has no ```text fence right after {REPORT_START}"
        ));
    }
    let mut text = String::new();
    loop {
        match lines.next() {
            None => {
                return Err(format!(
                    "{ROW}: the report fence in {CORPUS_DOC} is not closed"
                ));
            }
            Some(line) if line.trim_end() == "```" => break,
            Some(line) => {
                text.push_str(line);
                text.push('\n');
            }
        }
    }
    if lines.find(|line| !line.trim().is_empty()).map(str::trim) != Some(REPORT_END) {
        return Err(format!(
            "{ROW}: {REPORT_END} does not follow the report fence in {CORPUS_DOC}"
        ));
    }
    Ok(text)
}

/// The rows of the table under `## Files` in `doc`: its cells, trimmed, for
/// each line after the header and the delimiter row.
fn files_table(doc: &str) -> Vec<Vec<String>> {
    doc.lines()
        .skip_while(|line| line.trim() != "## Files")
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
        .filter(|line| line.trim_start().starts_with('|'))
        .skip(2)
        .map(|line| {
            let line = line.trim();
            let inner = line
                .strip_prefix('|')
                .and_then(|l| l.strip_suffix('|'))
                .unwrap_or(line);
            inner
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect()
        })
        .collect()
}

// --- Tests -----------------------------------------------------------------------

#[test]
fn lib_compat_015_the_corpus_report_equals_the_one_in_docs_compat_corpus_md() {
    let Some(dir) = corpus() else {
        return;
    };
    let actual = report(&dir);
    let recorded = recorded_report(&corpus_doc()).unwrap_or_else(|message| panic!("{message}"));
    assert!(
        actual == recorded,
        "{ROW}: the corpus report differs from the one recorded in {CORPUS_DOC}:\n\
         update the block in docs/compat/corpus.md\n\
         --- the actual report ---\n{actual}--- end ---"
    );
}

#[test]
fn lib_compat_015_the_corpus_report_is_the_same_on_a_second_run() {
    let Some(dir) = corpus() else {
        return;
    };
    let first = report(&dir);
    let second = report(&dir);
    assert!(
        first == second,
        "{ROW}: two runs give different reports:\n--- first ---\n{first}--- second ---\n{second}"
    );
}

/// A fresh directory, removed on drop.
#[derive(Debug)]
struct TempDir(PathBuf);

impl TempDir {
    fn new(test: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path =
            std::env::temp_dir().join(format!("xtask-{test}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path)
            .unwrap_or_else(|e| panic!("{ROW}: create {}: {e}", path.display()));
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn lib_compat_015_an_absent_corpus_is_skipped_unless_configured() {
    let empty = TempDir::new("lib-compat-015-absent");
    let dir = empty.0.clone();
    let first = dir.join(CORPUS[0].file_name);

    // Not configured, a file absent: skipped, with a notice naming the fetch.
    assert_eq!(
        locate(None, dir.clone(), CORPUS),
        Ok(Located::Skip(format!(
            "SKIPPED LIB-COMPAT-015: the corpus is not at {}; run `cargo xtask fetch-corpus`",
            dir.display()
        ))),
        "{ROW}: an absent corpus that is not configured is skipped with the notice"
    );
    // Configured, a file absent: an error naming the first absent file.
    assert_eq!(
        locate(
            Some(dir.clone().into_os_string()),
            PathBuf::from("unused"),
            CORPUS
        ),
        Err(format!(
            "LIB-COMPAT-015: {} is missing; run `cargo xtask fetch-corpus`",
            first.display()
        )),
        "{ROW}: an absent file of a configured corpus fails, naming it"
    );
    // Configured but empty: an error.
    assert_eq!(
        locate(Some(OsString::new()), dir.clone(), CORPUS),
        Err(
            "LIB-COMPAT-015: SACM_CORPUS_DIR is set but empty; unset it or run `cargo xtask \
             fetch-corpus`"
                .to_owned()
        ),
        "{ROW}: an empty SACM_CORPUS_DIR is an error"
    );

    // Every file present: found, configured or not; and a file with another
    // hash fails the hash check, naming it and both hashes.
    for artifact in CORPUS {
        fs::write(dir.join(artifact.file_name), b"not the pinned bytes")
            .unwrap_or_else(|e| panic!("{ROW}: write {}: {e}", artifact.file_name));
    }
    assert_eq!(
        locate(None, dir.clone(), CORPUS),
        Ok(Located::Found(dir.clone())),
        "{ROW}: a complete default corpus is found"
    );
    assert_eq!(
        locate(
            Some(dir.clone().into_os_string()),
            PathBuf::from("unused"),
            CORPUS
        ),
        Ok(Located::Found(dir.clone())),
        "{ROW}: a complete configured corpus is found"
    );
    assert_eq!(
        check_hashes(&dir, CORPUS),
        Err(format!(
            "LIB-COMPAT-015: {} has SHA-256 {}, pinned {}; delete it and run `cargo xtask \
             fetch-corpus`",
            first.display(),
            sha256_hex(b"not the pinned bytes"),
            CORPUS[0].sha256
        )),
        "{ROW}: a file with another hash fails the check, naming it and both hashes"
    );
}

#[test]
fn lib_compat_015_the_files_table_lists_every_corpus_file_in_order() {
    let rows = files_table(&corpus_doc());
    let listed: Vec<&str> = rows
        .iter()
        .map(|cells| cells.get(1).map_or("", |cell| cell.trim_matches('`')))
        .collect();
    let expected: Vec<&str> = CORPUS.iter().map(|a| a.file_name).collect();
    assert_eq!(
        listed, expected,
        "{ROW}: the `## Files` table of {CORPUS_DOC} does not list the CORPUS files in order"
    );
    for (cells, artifact) in rows.iter().zip(CORPUS) {
        // https://raw.githubusercontent.com/<owner>/<repo>/<commit>/...
        let segments: Vec<&str> = artifact.url.split('/').skip(3).take(3).collect();
        let [owner, repository, commit] = segments.as_slice() else {
            panic!(
                "{ROW}: setup: the URL of {} is not pinned to a commit",
                artifact.file_name
            );
        };
        let source = cells.get(2).map_or("", String::as_str);
        let short = commit.get(..12).unwrap_or(commit);
        assert!(
            source.contains(&format!("{owner}/{repository}")) && source.contains(short),
            "{ROW}: the source of {} in {CORPUS_DOC} does not name {owner}/{repository} and the \
             commit {short}: {source:?}",
            artifact.file_name
        );
    }
}
