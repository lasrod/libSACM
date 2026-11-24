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
//!
//! The `lib_compat_005_*` tests check what the corpus shows (LIB-COMPAT-005,
//! "What LIB-COMPAT-005 claims" in `docs/compat/corpus.md`): five files of
//! three tool families, `ACEditor` (7), Epsilon (8, 9) and `wrwei/SACM` (12, 13),
//! read tolerantly, have no dangling reference and write back without loss.
//! Files 12 and 13 are read with the subclass declarations an application
//! would give for their GSN classes (LIB-COMPAT-017), test data here as an
//! application's would be. This is interoperability evidence, not
//! conformance: no file is SACM 2.3 XMI (D-STRICT, D-CORPUS).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sacm::compare::semantic_compare;
use sacm::diagnostic::Diagnostic;
use sacm::model::ElementKind;
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};
use xtask::fetch::{Artifact, CORPUS, CORPUS_DEST, sha256_hex};

const ROW: &str = "LIB-COMPAT-015";

/// The row of the corpus claim (slice 0075b).
const ROW_005: &str = "LIB-COMPAT-005";

/// The GSN namespace that files 12 and 13 type SACM roles with.
const GSN: &str = "http://acwg.org/3.0/gsn";

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
/// the directory was configured, a skip otherwise. Every message names
/// `row`, the row whose test asks.
fn locate(
    row: &str,
    configured: Option<OsString>,
    default: PathBuf,
    files: &[Artifact],
) -> Result<Located, String> {
    let (dir, was_configured) = match configured {
        Some(value) if value.is_empty() => {
            return Err(format!(
                "{row}: SACM_CORPUS_DIR is set but empty; unset it or {FETCH}"
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
            "{row}: {} is missing; {FETCH}",
            dir.join(missing.file_name).display()
        ))
    } else {
        Ok(Located::Skip(format!(
            "SKIPPED {row}: the corpus is not at {}; {FETCH}",
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
/// printing a SKIPPED notice naming `row`. Panics when a configured corpus is
/// incomplete or a file has another hash.
///
/// The notice goes straight to the process's stderr: the test harness
/// captures `eprintln!` and hides it for a passing test, so a skip would
/// look like a pass.
fn corpus(row: &str) -> Option<PathBuf> {
    let default = xtask::repo_root().join(CORPUS_DEST);
    match locate(row, std::env::var_os("SACM_CORPUS_DIR"), default, CORPUS) {
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
    let Some(dir) = corpus(ROW) else {
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
    let Some(dir) = corpus(ROW) else {
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
        locate(ROW, None, dir.clone(), CORPUS),
        Ok(Located::Skip(format!(
            "SKIPPED LIB-COMPAT-015: the corpus is not at {}; run `cargo xtask fetch-corpus`",
            dir.display()
        ))),
        "{ROW}: an absent corpus that is not configured is skipped with the notice"
    );
    // Configured, a file absent: an error naming the first absent file.
    assert_eq!(
        locate(
            ROW,
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
        locate(ROW, Some(OsString::new()), dir.clone(), CORPUS),
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
        locate(ROW, None, dir.clone(), CORPUS),
        Ok(Located::Found(dir.clone())),
        "{ROW}: a complete default corpus is found"
    );
    assert_eq!(
        locate(
            ROW,
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

// --- LIB-COMPAT-005: the corpus claim -------------------------------------------------

/// The declarations an application would give for the GSN classes of files
/// 12 and 13 (LIB-COMPAT-017): test data, as an application's would be.
fn gsn_options() -> ReadOptions {
    [
        ("Module", ElementKind::ArgumentPackage),
        ("Goal", ElementKind::Claim),
        ("Context", ElementKind::Claim),
        ("Assumption", ElementKind::Claim),
        ("Justification", ElementKind::Claim),
        ("AwayGoal", ElementKind::Claim),
        ("Strategy", ElementKind::ArgumentReasoning),
        ("Solution", ElementKind::ArtifactReference),
        ("SupportedBy", ElementKind::AssertedInference),
        ("InContextOf", ElementKind::AssertedContext),
    ]
    .into_iter()
    .fold(ReadOptions::default(), |options, (name, kind)| {
        options.with_subclass(GSN, name, kind)
    })
}

/// The bytes of the corpus file `name` in `dir`.
fn corpus_file(dir: &Path, name: &str) -> Vec<u8> {
    assert!(
        CORPUS.iter().any(|artifact| artifact.file_name == name),
        "{ROW_005}: setup: {name} is not a corpus file"
    );
    let path = dir.join(name);
    fs::read(&path).unwrap_or_else(|e| panic!("{ROW_005}: read {}: {e}", path.display()))
}

/// The tolerant read of `name` with `options`, which must succeed.
fn claimed_read(dir: &Path, name: &str, options: &ReadOptions) -> ReadOutput {
    xmi::read(&corpus_file(dir, name), options)
        .unwrap_or_else(|error| panic!("{ROW_005}: {name}: the tolerant read refuses it: {error}"))
}

/// How many diagnostics of `found` have the template `template`.
fn count_of(found: &[Diagnostic], template: &str) -> usize {
    found
        .iter()
        .filter(|diagnostic| diagnostic.template() == template)
        .count()
}

/// One row of the per-file table of "What LIB-COMPAT-005 claims".
struct Claimed {
    /// The corpus file.
    name: &'static str,
    /// Whether it is read with [`gsn_options`], else with default options.
    gsn: bool,
    /// The `count` of its one SACM-XMI-017 `paths-read`, or `None` for no
    /// SACM-XMI-017.
    paths: Option<u64>,
    /// `document().elements().count()`.
    elements: usize,
    /// The strict write's `PreservedContent`: how many elements, and the
    /// first.
    strict: (usize, &'static str),
    /// How many SACM-XMI-003 `element-kept` Warnings the read reports.
    kept_whole: usize,
}

/// Files 7, 8, 9, 12 and 13 of the corpus.
const CLAIMED: [Claimed; 5] = [
    Claimed {
        name: "aceditor-hbs-default.sacm2",
        gsn: false,
        paths: Some(35),
        elements: 81,
        strict: (20, "implementation_constraint_1"),
        kept_whole: 0,
    },
    Claimed {
        name: "epsilon-sacm-example.model",
        gsn: false,
        paths: None,
        elements: 1,
        strict: (1, "argument_package_1"),
        kept_whole: 39,
    },
    Claimed {
        name: "epsilon-untitled.model",
        gsn: false,
        paths: None,
        elements: 1,
        strict: (1, "_KK-doAP4EeeNssncY22Lrw"),
        kept_whole: 4,
    },
    Claimed {
        name: "sysmline-easy-example.assurancecase",
        gsn: true,
        paths: Some(46),
        elements: 139,
        strict: (46, "argument_package_1"),
        kept_whole: 0,
    },
    Claimed {
        name: "sysmini-quan.assurancecase",
        gsn: true,
        paths: Some(23),
        elements: 75,
        strict: (25, "artifact_1"),
        kept_whole: 1,
    },
];

#[test]
fn lib_compat_005_the_files_of_three_tool_families_read_and_write_back_without_loss() {
    let Some(dir) = corpus(ROW_005) else {
        return;
    };
    for file in &CLAIMED {
        let name = file.name;
        let options = if file.gsn {
            gsn_options()
        } else {
            ReadOptions::default()
        };
        let output = claimed_read(&dir, name, &options);
        let found = output.diagnostics();

        // One SACM-XMI-015, a Warning emf-xmi-namespace-read.
        let namespace: Vec<(&str, &str)> = found
            .iter()
            .filter(|diagnostic| diagnostic.code().as_str() == "SACM-XMI-015")
            .map(|diagnostic| (diagnostic.severity().as_str(), diagnostic.template()))
            .collect();
        assert_eq!(
            namespace,
            [("warning", "SACM-XMI-015/emf-xmi-namespace-read")],
            "{ROW_005}: {name}: not exactly one SACM-XMI-015 Warning emf-xmi-namespace-read"
        );

        // The SACM-XMI-017s: none, or one paths-read with its count.
        let paths: Vec<(&str, Option<u64>)> = found
            .iter()
            .filter(|diagnostic| diagnostic.code().as_str() == "SACM-XMI-017")
            .map(|diagnostic| {
                let count = diagnostic
                    .arguments()
                    .iter()
                    .find(|argument| argument.name() == "count")
                    .and_then(|argument| argument.value().as_count());
                (diagnostic.template(), count)
            })
            .collect();
        let expected: Vec<(&str, Option<u64>)> = file
            .paths
            .map(|count| ("SACM-XMI-017/paths-read", Some(count)))
            .into_iter()
            .collect();
        assert_eq!(
            paths, expected,
            "{ROW_005}: {name}: the SACM-XMI-017 diagnostics"
        );

        // The elements kept whole.
        assert_eq!(
            count_of(found, "SACM-XMI-003/element-kept"),
            file.kept_whole,
            "{ROW_005}: {name}: the number of SACM-XMI-003 element-kept Warnings"
        );

        // The elements read.
        let document = output.document();
        assert_eq!(
            document.elements().count(),
            file.elements,
            "{ROW_005}: {name}: the number of elements read"
        );

        // No dangling reference.
        let dangling = validation::validate(document)
            .iter()
            .filter(|diagnostic| diagnostic.code().as_str() == "SACM-REF-001")
            .count();
        assert_eq!(
            dangling, 0,
            "{ROW_005}: {name}: validate reports SACM-REF-001"
        );

        // A tolerant write reads back, with the same options, with no
        // difference.
        let text = xmi::write(document, &WriteOptions::default().with_mode(Mode::Tolerant))
            .unwrap_or_else(|error| {
                panic!("{ROW_005}: {name}: the tolerant write refuses it: {error}")
            });
        let back = xmi::read(text.as_bytes(), &options).unwrap_or_else(|error| {
            panic!("{ROW_005}: {name}: the tolerant write does not read back: {error}")
        });
        let differences = semantic_compare(document, back.document());
        assert!(
            differences.is_empty(),
            "{ROW_005}: {name}: the tolerant write reads back with differences: {differences:?}"
        );

        // The strict write refuses the preserved content.
        let strict = match xmi::write(document, &WriteOptions::default()) {
            Err(WriteError::PreservedContent { elements, .. }) => (
                elements.len(),
                elements
                    .first()
                    .map(|id| id.as_str().to_owned())
                    .unwrap_or_default(),
            ),
            Err(error) => {
                panic!("{ROW_005}: {name}: the strict write is not PreservedContent: {error:?}")
            }
            Ok(_) => panic!("{ROW_005}: {name}: the strict write writes it"),
        };
        assert_eq!(
            strict,
            (file.strict.0, file.strict.1.to_owned()),
            "{ROW_005}: {name}: the elements the strict write names (how many, the first)"
        );
    }
}

#[test]
fn lib_compat_005_the_gsn_typed_elements_read_as_their_declared_sacm_classes() {
    let Some(dir) = corpus(ROW_005) else {
        return;
    };
    let options = gsn_options();
    for (name, counts, kept, whole) in [
        (
            "sysmline-easy-example.assurancecase",
            &[
                ("Assumption", 1),
                ("Context", 6),
                ("Goal", 8),
                ("InContextOf", 9),
                ("Justification", 1),
                ("Module", 1),
                ("Solution", 4),
                ("Strategy", 2),
                ("SupportedBy", 14),
            ][..],
            46,
            0,
        ),
        (
            "sysmini-quan.assurancecase",
            &[
                ("AwayGoal", 1),
                ("Context", 2),
                ("Goal", 7),
                ("InContextOf", 2),
                ("Module", 1),
                ("Solution", 1),
                ("Strategy", 1),
                ("SupportedBy", 9),
            ][..],
            24,
            1,
        ),
    ] {
        let output = claimed_read(&dir, name, &options);
        let mut by_name: BTreeMap<String, usize> = BTreeMap::new();
        for element in output.document().elements() {
            let Some(class) = element.subclass() else {
                continue;
            };
            assert_eq!(
                class.namespace(),
                GSN,
                "{ROW_005}: {name}: {} has a subclass outside the GSN namespace",
                element.id()
            );
            assert_eq!(
                Some(element.kind()),
                options.subclass(GSN, class.name()),
                "{ROW_005}: {name}: {}, a {}, is not of its declared kind",
                element.id(),
                class.name()
            );
            *by_name.entry(class.name().to_owned()).or_default() += 1;
        }
        let expected: BTreeMap<String, usize> = counts
            .iter()
            .map(|(class, count)| ((*class).to_owned(), *count))
            .collect();
        assert_eq!(
            by_name, expected,
            "{ROW_005}: {name}: the elements read as GSN subclasses, by name"
        );
        let found = output.diagnostics();
        assert_eq!(
            (
                count_of(found, "SACM-XMI-003/subclass-type-kept"),
                count_of(found, "SACM-XMI-003/element-kept"),
            ),
            (kept, whole),
            "{ROW_005}: {name}: the subclass-type-kept and element-kept Warnings"
        );
    }
}

#[test]
fn lib_compat_005_an_absent_corpus_skips_naming_this_row() {
    let empty = TempDir::new("lib-compat-005-absent");
    let dir = empty.0.clone();
    assert_eq!(
        locate(ROW_005, None, dir.clone(), CORPUS),
        Ok(Located::Skip(format!(
            "SKIPPED LIB-COMPAT-005: the corpus is not at {}; run `cargo xtask fetch-corpus`",
            dir.display()
        ))),
        "{ROW_005}: an absent corpus that is not configured is skipped with a notice naming the \
         row"
    );
}
