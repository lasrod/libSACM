//! Tests that the diagnostics catalog and the crate's codes agree
//! (LIB-DIAG-002; D-DIAG; ADR 0011; `docs/sacm/diagnostics.md`: "Adding a
//! code: append the row here in the same change that adds its constant to the
//! crate").
//!
//! Every code the crate can construct is a `pub const NAME: Code` of
//! `src/diagnostic.rs`: the test scans `src/**/*.rs` and finds every
//! `Code::from_static("...")` there and nowhere else. A catalog row
//! `| SACM-<AREA>-<NNN> | <severity> | <meaning> |` is live when its meaning
//! names `` `Code::NAME` `` for its own code, and reserved when it names none.
//! The live rows are exactly the constants, one row per code.
//!
//! The check is a pure function, [`check`], over the scanned constants and the
//! catalog's text, so each way it fails is shown on a small synthetic catalog.

use std::fmt;
use std::path::{Path, PathBuf};

use sacm::diagnostic::Code;

const ROW: &str = "LIB-DIAG-002";

/// Every `Code` constant, by name. A new constant must be added here, which
/// the scan checks; each text is checked against `as_str()`.
const CONSTANTS: [(&str, Code); 20] = [
    ("XML_MALFORMED", Code::XML_MALFORMED),
    ("XML_DOCTYPE_REJECTED", Code::XML_DOCTYPE_REJECTED),
    ("XMI_INVALID_ROOT", Code::XMI_INVALID_ROOT),
    ("XMI_MISSING_ID", Code::XMI_MISSING_ID),
    ("XMI_XSI_TYPE", Code::XMI_XSI_TYPE),
    ("XMI_VERSION_IGNORED", Code::XMI_VERSION_IGNORED),
    ("MULTIPLICITY_VIOLATION", Code::MULTIPLICITY_VIOLATION),
    ("ENUM_INVALID_LITERAL", Code::ENUM_INVALID_LITERAL),
    ("CMD_TARGET_NOT_FOUND", Code::CMD_TARGET_NOT_FOUND),
    ("CMD_DUPLICATE_ID", Code::CMD_DUPLICATE_ID),
    ("CMD_INVALID_PARENT", Code::CMD_INVALID_PARENT),
    ("ID_DUPLICATE", Code::ID_DUPLICATE),
    ("ID_INVALID", Code::ID_INVALID),
    ("GID_DUPLICATE", Code::GID_DUPLICATE),
    ("ID_TRIMMED", Code::ID_TRIMMED),
    ("PACKAGE_CONTENT_INVALID", Code::PACKAGE_CONTENT_INVALID),
    ("REF_DANGLING", Code::REF_DANGLING),
    ("REF_WRONG_TYPE", Code::REF_WRONG_TYPE),
    ("CITATION_INVALID", Code::CITATION_INVALID),
    ("ABSTRACT_FORM_INVALID", Code::ABSTRACT_FORM_INVALID),
];

/// The live codes after slice 0044: the 16 of slice 0041, SACM-REF-001,
/// SACM-CITE-001 and SACM-ABS-001 of slice 0042, and SACM-REF-002
/// (SACM23-VAL-004).
const LIVE: [&str; 20] = [
    "SACM-ABS-001",
    "SACM-CITE-001",
    "SACM-CMD-001",
    "SACM-CMD-004",
    "SACM-CMD-005",
    "SACM-ENUM-001",
    "SACM-ID-001",
    "SACM-ID-002",
    "SACM-ID-003",
    "SACM-ID-004",
    "SACM-MULT-001",
    "SACM-PKG-001",
    "SACM-REF-001",
    "SACM-REF-002",
    "SACM-SEC-001",
    "SACM-XMI-001",
    "SACM-XMI-004",
    "SACM-XMI-010",
    "SACM-XMI-011",
    "SACM-XML-001",
];

/// The file that holds the constants, relative to `src`.
const CONSTANTS_FILE: &str = "diagnostic.rs";

// --- The check ---------------------------------------------------------------------

/// A `pub const NAME: Code = Code::from_static("TEXT");` found in the source.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Constant {
    name: String,
    text: String,
}

fn constant(name: &str, text: &str) -> Constant {
    Constant {
        name: name.to_owned(),
        text: text.to_owned(),
    }
}

/// A catalog table row: its 1-based line, code, and the constants its meaning
/// names, in order, each once.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    line: usize,
    code: String,
    named: Vec<String>,
}

/// One way the crate and the catalog disagree.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Problem {
    /// A code is built in `file` other than as the value of a constant of
    /// [`CONSTANTS_FILE`]: `call` is what builds it.
    BuiltOutsideConstant { file: String, call: String },
    /// Two constants have one text.
    SameText { code: String, names: Vec<String> },
    /// A line starting `| SACM-` is not `| <code> | <severity> | <meaning> |`.
    MalformedRow { line: usize },
    /// Two or more rows have one code.
    DuplicateRows { code: String, lines: Vec<usize> },
    /// A constant's text has no row.
    NoRow { constant: String, code: String },
    /// The row of a constant's text does not name it.
    RowDoesNotName {
        constant: String,
        code: String,
        line: usize,
    },
    /// A row names a constant the crate does not have.
    UnknownConstant {
        constant: String,
        code: String,
        line: usize,
    },
    /// A row names the constant of another code.
    OtherCodesConstant {
        constant: String,
        code: String,
        line: usize,
        its_code: String,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuiltOutsideConstant { file, call } => {
                write!(
                    f,
                    "src/{file}: `{call}` builds a code outside a constant of src/{CONSTANTS_FILE}"
                )
            }
            Self::SameText { code, names } => {
                write!(f, "{code}: the constants {names:?} have the same text")
            }
            Self::MalformedRow { line } => write!(
                f,
                "line {line}: not a row `| <code> | <severity> | <meaning> |`"
            ),
            Self::DuplicateRows { code, lines } => {
                write!(f, "{code}: rows on lines {lines:?}, not one")
            }
            Self::NoRow { constant, code } => {
                write!(f, "{code}: `Code::{constant}` has no catalog row")
            }
            Self::RowDoesNotName {
                constant,
                code,
                line,
            } => write!(
                f,
                "{code} (line {line}): the row does not name `Code::{constant}`"
            ),
            Self::UnknownConstant {
                constant,
                code,
                line,
            } => write!(
                f,
                "{code} (line {line}): the row names `Code::{constant}`, which the crate does not have"
            ),
            Self::OtherCodesConstant {
                constant,
                code,
                line,
                its_code,
            } => write!(
                f,
                "{code} (line {line}): the row names `Code::{constant}`, which is {its_code}"
            ),
        }
    }
}

/// Whether `text` is a catalog code: `SACM-`, one or more ASCII capitals, `-`
/// and three digits.
fn is_code(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("SACM-") else {
        return false;
    };
    let Some((area, number)) = rest.split_once('-') else {
        return false;
    };
    !area.is_empty()
        && area.bytes().all(|b| b.is_ascii_uppercase())
        && number.len() == 3
        && number.bytes().all(|b| b.is_ascii_digit())
}

/// Whether `c` can be in a constant's name.
fn is_name_char(c: char) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'
}

/// The code of a Rust file: its lines that are not comments, joined with one
/// space wherever there was white space.
fn code_of(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The name `NAME` when `before` ends with `pub const NAME: Code = Code::`.
fn constant_name(before: &str) -> Option<&str> {
    let rest = before.strip_suffix("Code::")?.trim_end();
    let rest = rest.strip_suffix('=')?.trim_end();
    let rest = rest.strip_suffix("Code")?.trim_end();
    let rest = rest.strip_suffix(':')?.trim_end();
    let name_start = rest
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_name_char(*c))
        .last()
        .map(|(at, _)| at)?;
    let (head, name) = rest.split_at(name_start);
    head.trim_end().ends_with("pub const").then_some(name)
}

/// The only two places in [`CONSTANTS_FILE`] where `from_static(` is not
/// followed by a literal, each as (the code just before `from_static(`, the
/// code just after it), white space as [`code_of`] leaves it: the
/// constructor's own definition, and the whole of `new`, which delegates to
/// it.
const CONSTRUCTOR_CONTEXTS: [(&str, &str); 2] = [
    ("pub(crate) const fn ", "code: &'static str) -> Self {"),
    (
        "pub const fn new(code: &'static str) -> Self { Self::",
        "code) }",
    ),
];

/// Whether the `from_static(` between `before` and `after` is one of
/// [`CONSTRUCTOR_CONTEXTS`].
fn is_constructor_context(before: &str, after: &str) -> bool {
    CONSTRUCTOR_CONTEXTS
        .iter()
        .any(|(head, tail)| before.ends_with(head) && after.starts_with(tail))
}

/// The constants in `files`, each `(path relative to src, text)`, and every
/// code built some other way.
fn scan_sources(files: &[(String, String)]) -> (Vec<Constant>, Vec<Problem>) {
    let mut constants = Vec::new();
    let mut problems = Vec::new();
    for (file, text) in files {
        let code = code_of(text);
        let in_constants_file = file == CONSTANTS_FILE;
        for (at, _) in code.match_indices("from_static(") {
            let after = &code[at + "from_static(".len()..];
            let literal = after
                .strip_prefix('"')
                .and_then(|rest| rest.split_once('"'))
                .map(|(literal, _)| literal);
            let name = constant_name(code[..at].trim_end());
            match (in_constants_file, literal, name) {
                (true, Some(literal), Some(name)) => constants.push(constant(name, literal)),
                // The constructor's own definition and `new`'s call of it,
                // and nothing else.
                (true, None, _) if is_constructor_context(&code[..at], after) => {}
                _ => problems.push(Problem::BuiltOutsideConstant {
                    file: file.clone(),
                    call: snippet(&code[at..]),
                }),
            }
        }
        for (at, _) in code.match_indices("Code::new(") {
            problems.push(Problem::BuiltOutsideConstant {
                file: file.clone(),
                call: snippet(&code[at..]),
            });
        }
    }
    let mut texts: Vec<&str> = constants.iter().map(|c| c.text.as_str()).collect();
    texts.sort_unstable();
    texts.dedup();
    for text in texts {
        let names: Vec<String> = constants
            .iter()
            .filter(|c| c.text == text)
            .map(|c| c.name.clone())
            .collect();
        if names.len() > 1 {
            problems.push(Problem::SameText {
                code: text.to_owned(),
                names,
            });
        }
    }
    (constants, problems)
}

/// The call at the start of `code`, up to its first `)`.
fn snippet(code: &str) -> String {
    match code.find(')') {
        Some(end) => code[..=end].to_owned(),
        None => code.chars().take(40).collect(),
    }
}

/// The table rows of `catalog`, and every line starting `| SACM-` that is not
/// one.
fn parse_catalog(catalog: &str) -> (Vec<Row>, Vec<Problem>) {
    let mut rows = Vec::new();
    let mut problems = Vec::new();
    for (index, raw) in catalog.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if !trimmed.starts_with("| SACM-") {
            continue;
        }
        let cells: Option<Vec<&str>> = trimmed
            .strip_prefix('|')
            .and_then(|rest| rest.strip_suffix('|'))
            .map(|inner| inner.split('|').map(str::trim).collect());
        let Some(cells) = cells else {
            problems.push(Problem::MalformedRow { line });
            continue;
        };
        let [code, severity, meaning] = cells.as_slice() else {
            problems.push(Problem::MalformedRow { line });
            continue;
        };
        if !is_code(code) || severity.is_empty() || meaning.is_empty() {
            problems.push(Problem::MalformedRow { line });
            continue;
        }
        let mut named: Vec<String> = Vec::new();
        for (at, _) in meaning.match_indices("`Code::") {
            let rest = &meaning[at + "`Code::".len()..];
            let end = rest.find(|c: char| !is_name_char(c)).unwrap_or(rest.len());
            let name = &rest[..end];
            if !name.is_empty() && rest[end..].starts_with('`') && !named.iter().any(|n| n == name)
            {
                named.push(name.to_owned());
            }
        }
        rows.push(Row {
            line,
            code: (*code).to_owned(),
            named,
        });
    }
    (rows, problems)
}

/// Every way `constants` and `catalog` disagree, in a fixed order: malformed
/// rows, repeated codes, then each constant, then each row.
fn check(constants: &[Constant], catalog: &str) -> Vec<Problem> {
    let (rows, mut problems) = parse_catalog(catalog);

    let mut codes: Vec<&str> = rows.iter().map(|row| row.code.as_str()).collect();
    codes.sort_unstable();
    codes.dedup();
    for code in codes {
        let lines: Vec<usize> = rows
            .iter()
            .filter(|row| row.code == code)
            .map(|row| row.line)
            .collect();
        if lines.len() > 1 {
            problems.push(Problem::DuplicateRows {
                code: code.to_owned(),
                lines,
            });
        }
    }

    for c in constants {
        let mut own = rows.iter().filter(|row| row.code == c.text).peekable();
        if own.peek().is_none() {
            problems.push(Problem::NoRow {
                constant: c.name.clone(),
                code: c.text.clone(),
            });
        }
        for row in own {
            if !row.named.contains(&c.name) {
                problems.push(Problem::RowDoesNotName {
                    constant: c.name.clone(),
                    code: c.text.clone(),
                    line: row.line,
                });
            }
        }
    }

    for row in &rows {
        for name in &row.named {
            match constants.iter().find(|c| &c.name == name) {
                None => problems.push(Problem::UnknownConstant {
                    constant: name.clone(),
                    code: row.code.clone(),
                    line: row.line,
                }),
                Some(c) if c.text != row.code => problems.push(Problem::OtherCodesConstant {
                    constant: name.clone(),
                    code: row.code.clone(),
                    line: row.line,
                    its_code: c.text.clone(),
                }),
                Some(_) => {}
            }
        }
    }
    problems
}

/// The codes of the rows that name a constant, sorted: the live rows.
fn live_codes(catalog: &str) -> Vec<String> {
    let (rows, _) = parse_catalog(catalog);
    let mut live: Vec<String> = rows
        .into_iter()
        .filter(|row| !row.named.is_empty())
        .map(|row| row.code)
        .collect();
    live.sort();
    live
}

// --- The real crate and catalog ------------------------------------------------------

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()))
}

/// Every `.rs` file under `crates/sacm/src`, as (path relative to `src` with
/// `/` separators, text), sorted by path.
fn source_files() -> Vec<(String, String)> {
    let src = manifest_dir().join("src");
    let mut pending = vec![src.clone()];
    let mut files = Vec::new();
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("{ROW}: cannot list {}: {error}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("{ROW}: cannot list {}: {error}", dir.display()))
                .path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let relative = path
                    .strip_prefix(&src)
                    .unwrap_or_else(|_| panic!("{ROW}: {} is not under src", path.display()))
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                files.push((relative, read(&path)));
            }
        }
    }
    files.sort();
    files
}

fn catalog() -> String {
    read(&manifest_dir().join("../../docs/sacm/diagnostics.md"))
}

/// The crate's constants, which the scan must find without a problem.
fn crate_constants() -> Vec<Constant> {
    let files = source_files();
    assert!(
        files.iter().any(|(file, _)| file == CONSTANTS_FILE),
        "{ROW}: setup: the scan did not find src/{CONSTANTS_FILE}"
    );
    let (constants, problems) = scan_sources(&files);
    let report: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert!(
        problems.is_empty(),
        "{ROW}: the crate builds codes outside the constants:\n{}",
        report.join("\n")
    );
    constants
}

fn report(problems: &[Problem]) -> String {
    problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn lib_diag_002_every_code_the_crate_builds_is_a_constant_of_diagnostic_rs() {
    let constants = crate_constants();
    let mut scanned: Vec<&str> = constants.iter().map(|c| c.name.as_str()).collect();
    scanned.sort_unstable();
    let mut listed: Vec<&str> = CONSTANTS.iter().map(|(name, _)| *name).collect();
    listed.sort_unstable();
    assert_eq!(
        scanned, listed,
        "{ROW}: the constants in src/diagnostic.rs are not those this test lists"
    );
    for (name, code) in CONSTANTS {
        let Some(found) = constants.iter().find(|c| c.name == name) else {
            panic!("{ROW}: the scan did not find Code::{name}");
        };
        assert_eq!(
            found.text,
            code.as_str(),
            "{ROW}: the scanned text of Code::{name} is not its as_str()"
        );
    }
}

#[test]
fn lib_diag_002_the_catalog_and_the_crate_agree_in_both_directions() {
    let constants = crate_constants();
    let catalog = catalog();
    let problems = check(&constants, &catalog);
    assert!(
        problems.is_empty(),
        "{ROW}: docs/sacm/diagnostics.md and the crate disagree:\n{}",
        report(&problems)
    );
    assert_eq!(
        live_codes(&catalog),
        LIVE,
        "{ROW}: the live catalog rows are not the 20 expected"
    );
}

#[test]
fn lib_diag_002_the_status_note_states_the_rule_and_lists_no_code() {
    let catalog = catalog();
    let note: Vec<&str> = catalog
        .lines()
        .skip_while(|line| !line.starts_with("> **Status.**"))
        .take_while(|line| line.starts_with('>'))
        .collect();
    assert!(
        !note.is_empty(),
        "{ROW}: docs/sacm/diagnostics.md has no `> **Status.**` note"
    );
    let note = note.join("\n");
    let listed: Vec<&str> = note
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|word| is_code(word))
        .collect();
    assert!(
        listed.is_empty(),
        "{ROW}: the Status note lists codes {listed:?} by hand instead of stating the rule:\n{note}"
    );
    for part in ["live", "reserved", "`Code::"] {
        assert!(
            note.contains(part),
            "{ROW}: the Status note does not say {part:?}, so it does not state the rule:\n{note}"
        );
    }
}

// --- The check on synthetic inputs ------------------------------------------------

/// A catalog of the given rows, each `(code, severity, meaning)`, under a
/// heading, some prose and a table header; the first row is on line 6.
fn synthetic(rows: &[(&str, &str, &str)]) -> String {
    let mut text = String::from(
        "# Catalog\n\nProse naming SACM-A-001 and `Code::A` outside a table.\n\
         | Code | Severity | Meaning |\n",
    );
    text.push_str("| --- | --- | --- |\n");
    for (code, severity, meaning) in rows {
        text.push_str(&["| ", code, " | ", severity, " | ", meaning, " |\n"].concat());
    }
    text
}

/// The first row of [`synthetic`] is on this line.
const FIRST_ROW: usize = 6;

#[test]
fn lib_diag_002_a_matching_catalog_with_a_reserved_row_has_no_problem() {
    let constants = [constant("A", "SACM-A-001"), constant("B", "SACM-B-002")];
    let catalog = synthetic(&[
        ("SACM-A-001", "Error", "Meaning A. `Code::A` (ROW-1)."),
        ("SACM-B-002", "Error / Warning", "Meaning B. `Code::B`."),
        ("SACM-C-003", "Warning", "Reserved: no constant yet."),
    ]);
    assert_eq!(
        check(&constants, &catalog),
        [],
        "{ROW}: a catalog that agrees with its constants has problems"
    );
    assert_eq!(
        live_codes(&catalog),
        ["SACM-A-001", "SACM-B-002"],
        "{ROW}: the reserved row is counted as live, or a live one is not"
    );
}

#[test]
fn lib_diag_002_a_constant_with_no_row_is_named_with_its_code() {
    let constants = [constant("A", "SACM-A-001"), constant("B", "SACM-B-002")];
    let catalog = synthetic(&[("SACM-A-001", "Error", "`Code::A`")]);
    let problems = check(&constants, &catalog);
    assert_eq!(
        problems,
        [Problem::NoRow {
            constant: "B".into(),
            code: "SACM-B-002".into()
        }],
        "{ROW}: a constant with no row is not the one problem"
    );
    let text = problems[0].to_string();
    assert!(
        text.contains("SACM-B-002") && text.contains("Code::B"),
        "{ROW}: {text:?} does not name the code and the constant"
    );
}

#[test]
fn lib_diag_002_a_row_not_naming_its_constant_is_named_with_its_code_and_line() {
    let constants = [constant("A", "SACM-A-001")];
    let catalog = synthetic(&[("SACM-A-001", "Error", "Meaning A, the constant unnamed.")]);
    let problems = check(&constants, &catalog);
    assert_eq!(
        problems,
        [Problem::RowDoesNotName {
            constant: "A".into(),
            code: "SACM-A-001".into(),
            line: FIRST_ROW
        }],
        "{ROW}: a row that does not name its constant is not the one problem"
    );
    let text = problems[0].to_string();
    assert!(
        text.contains("SACM-A-001") && text.contains(&format!("line {FIRST_ROW}")),
        "{ROW}: {text:?} does not name the code and the row's line"
    );
}

#[test]
fn lib_diag_002_a_row_naming_an_unknown_constant_is_named_with_its_code_and_line() {
    let constants = [constant("A", "SACM-A-001")];
    let catalog = synthetic(&[
        ("SACM-A-001", "Error", "`Code::A`"),
        (
            "SACM-B-002",
            "Error",
            "`Code::B`, which is not in the crate.",
        ),
    ]);
    let problems = check(&constants, &catalog);
    assert_eq!(
        problems,
        [Problem::UnknownConstant {
            constant: "B".into(),
            code: "SACM-B-002".into(),
            line: FIRST_ROW + 1
        }],
        "{ROW}: a row naming an unknown constant is not the one problem"
    );
    let text = problems[0].to_string();
    assert!(
        text.contains("SACM-B-002")
            && text.contains(&format!("line {}", FIRST_ROW + 1))
            && text.contains("Code::B"),
        "{ROW}: {text:?} does not name the code, the row's line and the constant"
    );
}

#[test]
fn lib_diag_002_a_row_naming_another_codes_constant_is_named_with_both_codes() {
    let constants = [constant("A", "SACM-A-001"), constant("B", "SACM-B-002")];
    let catalog = synthetic(&[
        ("SACM-A-001", "Error", "`Code::A`"),
        ("SACM-B-002", "Error", "`Code::B`; see also `Code::A`."),
    ]);
    let problems = check(&constants, &catalog);
    assert_eq!(
        problems,
        [Problem::OtherCodesConstant {
            constant: "A".into(),
            code: "SACM-B-002".into(),
            line: FIRST_ROW + 1,
            its_code: "SACM-A-001".into()
        }],
        "{ROW}: a row naming another code's constant is not the one problem"
    );
    let text = problems[0].to_string();
    assert!(
        text.contains("SACM-B-002")
            && text.contains("SACM-A-001")
            && text.contains(&format!("line {}", FIRST_ROW + 1)),
        "{ROW}: {text:?} does not name both codes and the row's line"
    );

    // A constant named in the row of another code, and not in its own.
    let catalog = synthetic(&[
        ("SACM-A-001", "Error", "Reserved-looking, names nothing."),
        ("SACM-B-002", "Error", "`Code::A` and `Code::B`."),
    ]);
    assert_eq!(
        check(&constants, &catalog),
        [
            Problem::RowDoesNotName {
                constant: "A".into(),
                code: "SACM-A-001".into(),
                line: FIRST_ROW
            },
            Problem::OtherCodesConstant {
                constant: "A".into(),
                code: "SACM-B-002".into(),
                line: FIRST_ROW + 1,
                its_code: "SACM-A-001".into()
            },
        ],
        "{ROW}: a constant named only in another code's row is not both problems"
    );
}

#[test]
fn lib_diag_002_two_rows_for_one_code_are_named_with_the_code_and_both_lines() {
    let constants = [constant("A", "SACM-A-001")];
    let catalog = synthetic(&[
        ("SACM-A-001", "Error", "`Code::A`"),
        ("SACM-B-002", "Error", "Reserved."),
        ("SACM-A-001", "Warning", "Again. `Code::A`"),
    ]);
    let problems = check(&constants, &catalog);
    assert_eq!(
        problems,
        [Problem::DuplicateRows {
            code: "SACM-A-001".into(),
            lines: vec![FIRST_ROW, FIRST_ROW + 2]
        }],
        "{ROW}: two rows for one code are not the one problem"
    );
    let text = problems[0].to_string();
    assert!(
        text.contains("SACM-A-001")
            && text.contains(&FIRST_ROW.to_string())
            && text.contains(&(FIRST_ROW + 2).to_string()),
        "{ROW}: {text:?} does not name the code and both lines"
    );

    // Two reserved rows for one code are a problem too.
    let catalog = synthetic(&[
        ("SACM-A-001", "Error", "`Code::A`"),
        ("SACM-B-002", "Error", "Reserved."),
        ("SACM-B-002", "Error", "Reserved again."),
    ]);
    assert_eq!(
        check(&constants, &catalog),
        [Problem::DuplicateRows {
            code: "SACM-B-002".into(),
            lines: vec![FIRST_ROW + 1, FIRST_ROW + 2]
        }],
        "{ROW}: two reserved rows for one code are not the one problem"
    );
}

#[test]
fn lib_diag_002_a_malformed_row_is_named_with_its_line() {
    let constants = [constant("A", "SACM-A-001")];
    for row in [
        "| SACM-A-001 | Error |",
        "| SACM-A-1 | Error | `Code::A` |",
        "| SACM-A-001 | Error | a | b |",
        "| SACM-a-001 | Error | `Code::A` |",
        "| SACM-A-001 |  | `Code::A` |",
    ] {
        let catalog = synthetic(&[]) + row + "\n";
        let problems = check(&constants, &catalog);
        assert!(
            problems.contains(&Problem::MalformedRow { line: FIRST_ROW }),
            "{ROW}: {row:?} is not reported as malformed: {problems:?}"
        );
    }
}

#[test]
fn lib_diag_002_the_scan_finds_constants_and_codes_built_any_other_way() {
    let constants_file = "\
//! Codes.
pub struct Code(&'static str);
impl Code {
    /// SACM-A-001 docs mention `Code::from_static(\"SACM-Z-999\")`.
    pub const A: Code = Code::from_static(\"SACM-A-001\");
    pub const LONG_NAME_2: Code =
        Code::from_static(\"SACM-B-002\");
    pub(crate) const fn from_static(code: &'static str) -> Self {
        Self(code)
    }
    pub const fn new(code: &'static str) -> Self {
        Self::from_static(code)
    }
}
";
    let (constants, problems) = scan_sources(&[(CONSTANTS_FILE.to_owned(), constants_file.into())]);
    assert_eq!(
        (constants, problems),
        (
            vec![
                constant("A", "SACM-A-001"),
                constant("LONG_NAME_2", "SACM-B-002")
            ],
            vec![]
        ),
        "{ROW}: the scan of a clean constants file is wrong"
    );

    let files = [
        (CONSTANTS_FILE.to_owned(), constants_file.to_owned()),
        (
            "xmi/roots.rs".to_owned(),
            "fn f() -> Code { Code::from_static(\"SACM-C-003\") }".to_owned(),
        ),
        (
            "validation.rs".to_owned(),
            "fn g() -> Code { Code::new(\"SACM-D-004\") }".to_owned(),
        ),
        (
            "edit.rs".to_owned(),
            "fn h(text: &'static str) -> Code { Code::from_static(text) }".to_owned(),
        ),
        (
            CONSTANTS_FILE.to_owned(),
            "fn i() -> Code { let c = Code::from_static(\"SACM-E-005\"); c }".to_owned(),
        ),
    ];
    let (_, problems) = scan_sources(&files);
    let expected = [
        ("xmi/roots.rs", "from_static(\"SACM-C-003\")"),
        ("validation.rs", "Code::new(\"SACM-D-004\")"),
        ("edit.rs", "from_static(text)"),
        (CONSTANTS_FILE, "from_static(\"SACM-E-005\")"),
    ]
    .map(|(file, call)| Problem::BuiltOutsideConstant {
        file: file.to_owned(),
        call: call.to_owned(),
    });
    assert_eq!(
        problems, expected,
        "{ROW}: the scan does not report each code built outside a constant"
    );
    assert!(
        problems[0].to_string().contains("xmi/roots.rs")
            && problems[0].to_string().contains("SACM-C-003"),
        "{ROW}: {} does not name the file and the code",
        problems[0]
    );

    let twice = "\
pub const A: Code = Code::from_static(\"SACM-A-001\");
pub const B: Code = Code::from_static(\"SACM-A-001\");
";
    let (_, problems) = scan_sources(&[(CONSTANTS_FILE.to_owned(), twice.to_owned())]);
    assert_eq!(
        problems,
        [Problem::SameText {
            code: "SACM-A-001".into(),
            names: vec!["A".into(), "B".into()]
        }],
        "{ROW}: two constants with one text are not reported"
    );
}

#[test]
fn lib_diag_002_a_non_constant_construction_in_diagnostic_rs_is_a_problem() {
    let clean = "\
pub struct Code(&'static str);
impl Code {
    pub const A: Code = Code::from_static(\"SACM-A-001\");
    pub(crate) const fn from_static(code: &'static str) -> Self {
        Self(code)
    }
    pub const fn new(code: &'static str) -> Self {
        Self::from_static(code)
    }
}
";
    // Each hidden construction, and the call the problem must name.
    for (hidden, call) in [
        (
            "fn hidden() -> Code { let text = \"SACM-Z-999\"; Code::from_static(text) }",
            "from_static(text)",
        ),
        (
            "impl Code { fn hidden() -> Self { let text = \"SACM-Z-999\"; Self::from_static(text) } }",
            "from_static(text)",
        ),
        // `new`'s signature with another body, and its body under another
        // parameter name, are not `new`.
        (
            "impl Other { pub const fn new(code: &'static str) -> Self { let c = Code::from_static(code); c } }",
            "from_static(code)",
        ),
        (
            "impl Other { pub const fn new(text: &'static str) -> Self { Self::from_static(text) } }",
            "from_static(text)",
        ),
    ] {
        let file = format!("{clean}{hidden}\n");
        let (constants, problems) = scan_sources(&[(CONSTANTS_FILE.to_owned(), file)]);
        assert_eq!(
            (constants, problems),
            (
                vec![constant("A", "SACM-A-001")],
                vec![Problem::BuiltOutsideConstant {
                    file: CONSTANTS_FILE.to_owned(),
                    call: call.to_owned(),
                }]
            ),
            "{ROW}: {hidden:?} in src/{CONSTANTS_FILE} builds a code the scan does not report"
        );
    }
}
