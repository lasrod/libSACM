//! Acceptance tests for matrix check reporting (slice 0005).
//!
//! - LIB-TOOL-013: when the root comes from `XTASK_ROOT`, `matrix check` names
//!   the root it checked: ` (root: <p>)` ends its OK line and the first line of
//!   its error. The rule depends on where the root came from
//!   ([`xtask::RootSource`]), never on its value, and no other command changes.
//! - LIB-TOOL-014: an error from reading or parsing the matrix is exactly one
//!   line: `docs/matrix/functionality-matrix.toml:<line>:<column>: <message>`
//!   or `read <path>: <error>`, with control characters escaped, and every
//!   command that loads the matrix returns the same line.
//!
//! The library is driven through `xtask::run` and `xtask::run_from` on
//! temporary trees, and the binary through `XTASK_ROOT`.
//!
//! The expected line and column of each parse error are counted by hand from
//! the fixture text and cross-checked against the byte offset of a marker in
//! that text, so they do not depend on the parser. Only the message itself is
//! the parser's: the contract says `<message>` is `toml::de::Error::message`.
//! Each case also checks, as a precondition, that the parser reports the error
//! at the marker, so a fixture that stopped provoking its error is reported as
//! such and not as a wrong position.
//!
//! Fixture source text is written on one line with `\n` escapes on purpose: a
//! line of this file that began with `#[test]` inside a string literal would
//! be picked up by the real repository's orphan scan.
//!
//! **Keep this target's name sorting after `function_scan`.** Several tests
//! here scan the real repository (`repo_root()` checks and binary runs). Under
//! a scanner mutant such as `is_identifier_char -> true`, that scan becomes
//! quadratic and runs for minutes. `cargo test` runs integration-test targets
//! in alphabetical order and stops at the first failing one, so
//! `function_scan` and `matrix` must run first and kill such a mutant in under
//! a second. Otherwise the mutation run reports a TIMEOUT (mutants.yml run
//! 42420891449, under the old name `check_reporting`).

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{TempTree, consistent_tree};
use xtask::fetch::{ARTIFACTS, DEFAULT_DEST};
use xtask::matrix::{MATRIX_PATH, Matrix, RENDERED_PATH, load};
use xtask::{RootSource, repo_root, run, run_from};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// The environment variable that points the `xtask` binary at another root.
const ROOT_ENV: &str = "XTASK_ROOT";

/// The commands that load the matrix.
const LOADING_COMMANDS: [&[&str]; 5] = [
    &["matrix", "check"],
    &["matrix", "render"],
    &["matrix", "next"],
    &["matrix", "next", "-n", "3"],
    &["matrix", "status"],
];

/// How a test enters the library.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    /// `xtask::run(args, root, out)`.
    Run,
    /// `xtask::run_from(args, root, source, out)`.
    From(RootSource),
}

const ENTRIES: [Entry; 3] = [
    Entry::Run,
    Entry::From(RootSource::Build),
    Entry::From(RootSource::Environment),
];

/// Run `args` on `root` through `entry`: the result, and what was written to
/// `out`.
fn call(row: &str, entry: Entry, args: &[&str], root: &Path) -> (Result<(), String>, String) {
    let mut out = Vec::new();
    let result = match entry {
        Entry::Run => run(args, root, &mut out),
        Entry::From(source) => run_from(args, root, source, &mut out),
    };
    let out = String::from_utf8(out).unwrap_or_else(|e| {
        panic!(
            "{row}: `{}` through {entry:?} wrote non-UTF-8: {e}",
            args.join(" ")
        )
    });
    (result, out)
}

/// `matrix check` on `root` through `run_from` with `source`.
fn check_from(row: &str, root: &Path, source: RootSource) -> (Result<(), String>, String) {
    call(row, Entry::From(source), &["matrix", "check"], root)
}

/// The suffix that names `root`, for a root without control characters.
fn suffix(root: &Path) -> String {
    let shown = root.display().to_string();
    assert!(
        !shown.chars().any(char::is_control),
        "LIB-TOOL-013: precondition: {shown:?} holds no control character"
    );
    format!(" (root: {shown})")
}

/// The first line of a command's report: its output when it succeeded, its
/// error when it failed.
fn first_line(result: &(Result<(), String>, String)) -> String {
    let text = match result {
        (Ok(()), out) => out.as_str(),
        (Err(error), _) => error.as_str(),
    };
    text.split('\n').next().unwrap_or_default().to_owned()
}

/// `text` with `tail` appended to its first line.
fn with_first_line_suffix(text: &str, tail: &str) -> String {
    match text.split_once('\n') {
        Some((first, rest)) => format!("{first}{tail}\n{rest}"),
        None => format!("{text}{tail}"),
    }
}

/// Run the `xtask` binary with `args`, with `XTASK_ROOT` set to `root`, or
/// removed when `root` is `None`: whether it succeeded, its stdout and its
/// stderr.
fn binary(row: &str, args: &[&str], root: Option<&Path>) -> (bool, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command.args(args);
    match root {
        Some(root) => command.env(ROOT_ENV, root),
        None => command.env_remove(ROOT_ENV),
    };
    let output = command
        .output()
        .unwrap_or_else(|e| panic!("{row}: the xtask binary can be started: {e}"));
    let text = |bytes: Vec<u8>| {
        String::from_utf8(bytes)
            .unwrap_or_else(|e| panic!("{row}: `xtask {}` printed non-UTF-8: {e}", args.join(" ")))
    };
    (
        output.status.success(),
        text(output.stdout),
        text(output.stderr),
    )
}

/// The uncited traced tests planted by [`failing_tree`], in the order the
/// gate reports them (by file, then by function name).
const FAILING_PROBLEMS: [&str; 2] = [
    "crates/demo/tests/planted.rs: traced test `lib_tool_001_a` is not cited by any matrix row",
    "crates/demo/tests/planted.rs: traced test `lib_tool_001_b` is not cited by any matrix row",
];

/// A consistent tree plus two uncited traced tests, written out of order.
fn failing_tree(name: &str) -> TempTree {
    let tree = consistent_tree(name);
    tree.write(
        "crates/demo/tests/planted.rs",
        "#[test]\nfn lib_tool_001_b() {}\n#[test]\nfn lib_tool_001_a() {}\n",
    );
    tree
}

/// The error of `matrix check` on [`failing_tree`], with `first` appended to
/// its first line.
fn failing_error(first: &str) -> String {
    format!(
        "functionality matrix: 2 problem(s){first}\n  {}\n  {}",
        FAILING_PROBLEMS[0], FAILING_PROBLEMS[1]
    )
}

/// A tree holding only a matrix with the syntax error of [`syntax_error_text`].
fn broken_tree(name: &str) -> TempTree {
    let tree = TempTree::new(name);
    tree.write(MATRIX_PATH, &syntax_error_text());
    tree
}

/// A tree whose matrix path is a directory, so the matrix cannot be read.
fn unreadable_tree(name: &str) -> TempTree {
    let tree = TempTree::new(name);
    let path = tree.root().join(MATRIX_PATH);
    fs::create_dir_all(&path)
        .unwrap_or_else(|e| panic!("LIB-TOOL-014: create {}: {e}", path.display()));
    tree
}

/// The read error for the matrix under `root`, for a root without control
/// characters: `read <path>: <io error>`.
fn read_error(root: &Path) -> String {
    let path = root.join(MATRIX_PATH);
    let error = fs::read_to_string(&path)
        .expect_err("LIB-TOOL-014: precondition: the matrix cannot be read");
    let line = format!("read {}: {error}", path.display());
    assert!(
        !line.chars().any(char::is_control),
        "LIB-TOOL-014: precondition: {line:?} holds no control character"
    );
    line
}

// ---------------------------------------------------------------------------
// Matrix fixtures (LIB-TOOL-014)
// ---------------------------------------------------------------------------

/// Lines 1 to 9 of every fixture matrix.
const HEADER: &str = "schema_version = 1\n[[milestone]]\nid = \"M0\"\ntitle = \"Foundation\"\n\
                      goal = \"Build.\"\n[[area]]\ncode = \"TOOL\"\nprefix = \"LIB\"\ntitle = \"Tooling\"\n";

/// A valid planned row `LIB-TOOL-00<n>` of nine lines; its `status` is the
/// ninth.
fn row(n: usize) -> String {
    format!(
        "[[row]]\nid = \"LIB-TOOL-00{n}\"\narea = \"TOOL\"\nmilestone = \"M0\"\ntitle = \"Gate\"\n\
         requirement = \"R.\"\nsource = \"S\"\nkind = \"tooling\"\nstatus = \"planned\"\n"
    )
}

/// [`row`] without its `title` line: eight lines.
fn row_without_title(n: usize) -> String {
    row(n).replace("title = \"Gate\"\n", "")
}

/// [`HEADER`] and row 1 (lines 1 to 18), then `extra` from line 19 on.
fn one_row(extra: &str) -> String {
    format!("{HEADER}{}{extra}", row(1))
}

/// The matrix of [`broken_tree`]: a second `=` at line 19, column 9.
fn syntax_error_text() -> String {
    one_row("notes = = \"x\"\n")
}

/// The one-line load error for [`syntax_error_text`].
fn syntax_error_line() -> String {
    let text = syntax_error_text();
    format!("{MATRIX_PATH}:19:9: {}", parser_message(&text))
}

/// The byte offset of the first `marker` in `text`.
fn offset_of(text: &str, marker: &str) -> usize {
    text.find(marker)
        .unwrap_or_else(|| panic!("LIB-TOOL-014: fixture: {marker:?} occurs in {text:?}"))
}

/// The 1-based line, and the 1-based column in characters, of byte `offset`
/// in `text`. Used only to cross-check the hand-counted positions.
fn position(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    (
        before.matches('\n').count() + 1,
        before[line_start..].chars().count() + 1,
    )
}

/// The parser's own message for `text`, which must fail to parse.
fn parser_message(text: &str) -> String {
    toml::from_str::<Matrix>(text)
        .expect_err("LIB-TOOL-014: precondition: the fixture fails to parse")
        .message()
        .to_owned()
}

/// One parse-error case.
#[derive(Debug)]
struct Case<'a> {
    /// What the case is.
    what: &'a str,
    /// The matrix text.
    text: String,
    /// Byte offset of the start of the error.
    offset: usize,
    /// Hand-counted 1-based line of `offset`.
    line: usize,
    /// Hand-counted 1-based column, in characters, of `offset`.
    column: usize,
    /// Text the parser's message must contain, so the fixture provokes the
    /// intended error.
    kind: &'a str,
}

/// Check the fixture and the parser's view of it; return the parser's
/// message.
fn preconditions(case: &Case<'_>) -> String {
    let Case {
        what,
        text,
        offset,
        line,
        column,
        kind,
    } = case;
    assert_eq!(
        position(text, *offset),
        (*line, *column),
        "LIB-TOOL-014: fixture {what}: the hand-counted position is that of byte {offset}"
    );
    let error = toml::from_str::<Matrix>(text)
        .expect_err("LIB-TOOL-014: precondition: the fixture fails to parse");
    assert_eq!(
        error.span().map(|s| s.start),
        Some(*offset),
        "LIB-TOOL-014: precondition: {what}: the parser places the error at byte {offset}, got {:?}",
        error.message()
    );
    let message = error.message().to_owned();
    assert!(
        message.contains(kind),
        "LIB-TOOL-014: precondition: {what}: the parser reports {kind:?}, got {message:?}"
    );
    message
}

/// `load(text)` returns exactly `MATRIX_PATH:<line>:<column>: <message>`, on
/// one line.
fn assert_load_error(case: &Case<'_>) {
    let message = preconditions(case);
    assert!(
        !message.chars().any(char::is_control),
        "LIB-TOOL-014: precondition: {}: the message holds no control character, got {message:?}",
        case.what
    );
    let expected = format!("{MATRIX_PATH}:{}:{}: {message}", case.line, case.column);
    assert_eq!(
        load(&case.text).map(|_| ()),
        Err(expected),
        "LIB-TOOL-014: {}: the load error is one line, `path:line:column: message`, at the start \
         of the error's span",
        case.what
    );
}

#[test]
fn lib_tool_014_the_fixtures_parse_when_unbroken() {
    let text = format!("{HEADER}{}{}{}", row(1), row(2), row(3));
    assert_eq!(
        text.lines().count(),
        36,
        "LIB-TOOL-014: fixture: the header takes 9 lines and each row 9"
    );
    assert_eq!(
        load(&text).map(|m| m.rows.len()),
        Ok(3),
        "LIB-TOOL-014: fixture: the unbroken header and rows parse"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-014: parse errors are one line, with their position
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_014_a_syntax_error_is_one_line_with_its_position() {
    let text = syntax_error_text();
    assert_load_error(&Case {
        what: "a second `=`",
        offset: offset_of(&text, "= \"x\""),
        text,
        line: 19,
        column: 9,
        kind: "=",
    });

    // An unterminated string: the error is at the line break that ends it,
    // one column past the last character of the line.
    let text = one_row("notes = \"open\n");
    assert_load_error(&Case {
        what: "an unterminated string",
        offset: offset_of(&text, "notes = \"open") + "notes = \"open".len(),
        text,
        line: 19,
        column: 14,
        kind: "string",
    });
}

#[test]
fn lib_tool_014_an_unknown_key_is_one_line_with_its_position() {
    let text = one_row("  colour = \"blue\"\n");
    assert_load_error(&Case {
        what: "an unknown key in a row",
        offset: offset_of(&text, "colour"),
        text,
        line: 19,
        column: 3,
        kind: "unknown field `colour`",
    });
}

#[test]
fn lib_tool_014_a_missing_required_field_is_one_line_with_its_position() {
    // The error is at the header of the table that lacks the field.
    let text = format!("{HEADER}  {}", row_without_title(1));
    assert_load_error(&Case {
        what: "a row without a title",
        offset: offset_of(&text, "[[row]]"),
        text,
        line: 10,
        column: 3,
        kind: "missing field `title`",
    });

    // A missing top-level field is at the very start of the file: 1:1, not
    // 0:0 or 1:0.
    let text = format!(
        "{}{}",
        HEADER.replacen("schema_version = 1\n", "", 1),
        row(1)
    );
    assert_load_error(&Case {
        what: "a matrix without `schema_version`",
        offset: 0,
        text,
        line: 1,
        column: 1,
        kind: "missing field `schema_version`",
    });
}

#[test]
fn lib_tool_014_a_value_of_the_wrong_type_is_one_line_with_its_position() {
    let text = format!(
        "{}{}",
        HEADER.replacen("schema_version = 1", "schema_version = \"one\"", 1),
        row(1)
    );
    assert_load_error(&Case {
        what: "a string `schema_version`",
        offset: offset_of(&text, "\"one\""),
        text,
        line: 1,
        column: 18,
        kind: "invalid type",
    });

    let text = one_row("acceptance = \"a\"\n");
    assert_load_error(&Case {
        what: "a string where a list is expected",
        offset: offset_of(&text, "\"a\""),
        text,
        line: 19,
        column: 14,
        kind: "invalid type",
    });
}

#[test]
fn lib_tool_014_the_column_counts_characters_not_bytes() {
    // Before the `7`: 14 characters, an 11-character string holding 2-, 3-
    // and 4-byte characters, then `, `. Column 28 in characters, 37 in bytes,
    // 29 in UTF-16 code units.
    let text = one_row("acceptance = [\"ÅÄÖ – ü 🦀\", 7]\n");
    let offset = offset_of(&text, "7]");
    let line_start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
    assert_eq!(
        (
            offset - line_start + 1,
            text[line_start..offset].encode_utf16().count() + 1
        ),
        (37, 29),
        "LIB-TOOL-014: fixture: the byte and UTF-16 columns differ from the character column"
    );
    assert_load_error(&Case {
        what: "an integer after non-ASCII text on the same line",
        offset,
        text,
        line: 19,
        column: 28,
        kind: "invalid type: integer `7`",
    });
}

#[test]
fn lib_tool_014_non_ascii_text_on_earlier_lines_does_not_move_the_position() {
    // Line 4 holds 15 bytes more than characters: a position counted in
    // characters from a byte offset would land past the end of line 19.
    let header = HEADER.replacen(
        "title = \"Foundation\"",
        "title = \"Grundläggning – åäö 🦀🦀🦀\"",
        1,
    );
    let text = format!("{header}{}  colour = 1\n", row(1));
    assert_eq!(
        header.len() - header.chars().count(),
        15,
        "LIB-TOOL-014: fixture: the header's extra bytes"
    );
    assert_load_error(&Case {
        what: "an unknown key after non-ASCII lines",
        offset: offset_of(&text, "colour"),
        text,
        line: 19,
        column: 3,
        kind: "unknown field `colour`",
    });
}

#[test]
fn lib_tool_014_an_error_in_a_later_row_has_that_rows_position() {
    // Rows 1 and 2 are valid; row 3 (lines 28 to 36) has an unknown status.
    let text = format!(
        "{HEADER}{}{}{}",
        row(1),
        row(2),
        row(3).replace("\"planned\"", "\"done\"")
    );
    assert_load_error(&Case {
        what: "an unknown status in the third row",
        offset: offset_of(&text, "\"done\""),
        text,
        line: 36,
        column: 10,
        kind: "unknown variant `done`",
    });

    // Row 2 of 3, indented, lacks its title: the error is at row 2's header,
    // not row 1's.
    let text = format!("{HEADER}{}  {}{}", row(1), row_without_title(2), row(3));
    assert_load_error(&Case {
        what: "the second of three rows without a title",
        offset: offset_of(&text, "[[row]]\nid = \"LIB-TOOL-002\""),
        text,
        line: 19,
        column: 3,
        kind: "missing field `title`",
    });
}

#[test]
fn lib_tool_014_control_characters_in_the_message_are_escaped() {
    // The status is written with TOML escapes, so the file itself holds no
    // control character; the parser quotes the decoded value in its message.
    let text = format!(
        "{HEADER}{}",
        row(1).replace(
            "\"planned\"",
            "\"planned\\t\\u001b[2K\\r\\n  LIB-TOOL-001: forged\\u007f\\u0085\""
        )
    );
    let case = Case {
        what: "an unknown status holding control characters",
        offset: offset_of(&text, "\"planned\\t"),
        text,
        line: 18,
        column: 10,
        kind: "unknown variant `planned",
    };
    let raw = preconditions(&case);
    assert!(
        raw.contains("planned\t\u{1b}[2K\r\n  LIB-TOOL-001: forged\u{7f}\u{85}"),
        "LIB-TOOL-014: precondition: the parser quotes the raw value, got {raw:?}"
    );
    let escaped = raw
        .replace('\t', "\\t")
        .replace('\u{1b}', "\\u{1b}")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace('\u{7f}', "\\u{7f}")
        .replace('\u{85}', "\\u{85}");
    assert!(
        !escaped.chars().any(char::is_control)
            && escaped.contains("planned\\t\\u{1b}[2K\\r\\n  LIB-TOOL-001: forged\\u{7f}\\u{85}"),
        "LIB-TOOL-014: precondition: the expected message is escaped as by \
         `char::escape_default`, got {escaped:?}"
    );
    assert_eq!(
        load(&case.text).map(|_| ()),
        Err(format!("{MATRIX_PATH}:18:10: {escaped}")),
        "LIB-TOOL-014: control characters in the parser's message are escaped as in LIB-TOOL-010, \
         so the error stays one line"
    );

    // A quoted key holding a line break, in an unknown-field message.
    let text = one_row("\"forged\\nkey\" = 1\n");
    let case = Case {
        what: "an unknown key holding a line break",
        offset: offset_of(&text, "\"forged"),
        text,
        line: 19,
        column: 1,
        kind: "unknown field `forged\nkey`",
    };
    let raw = preconditions(&case);
    assert_eq!(
        load(&case.text).map(|_| ()),
        Err(format!("{MATRIX_PATH}:19:1: {}", raw.replace('\n', "\\n"))),
        "LIB-TOOL-014: a line break in a quoted key is shown as `\\n`"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-014: read errors, every command, the binary
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_014_an_unreadable_matrix_is_one_read_line() {
    let tree = unreadable_tree("report-unreadable");
    let expected = read_error(tree.root());
    for args in LOADING_COMMANDS {
        for entry in [Entry::Run, Entry::From(RootSource::Build)] {
            assert_eq!(
                call("LIB-TOOL-014", entry, args, tree.root()),
                (Err(expected.clone()), String::new()),
                "LIB-TOOL-014: `{}` through {entry:?} on a matrix path that is a directory \
                 returns exactly `read <path>: <error>` and writes nothing",
                args.join(" ")
            );
        }
    }
}

/// A root that does not exist and whose name holds a line break that would
/// forge a problem line, and an escape sequence that would erase a line.
fn control_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "xtask-no-such-root-{}\n  LIB-TOOL-001: forged\u{1b}[2K",
        std::process::id()
    ));
    assert!(
        !root.exists(),
        "LIB-TOOL-013: precondition: {} does not exist",
        root.display()
    );
    root
}

/// `text` with the control characters of [`control_root`] escaped.
fn escape_control_root(text: &str) -> String {
    let escaped = text.replace('\n', "\\n").replace('\u{1b}', "\\u{1b}");
    assert!(
        !escaped.chars().any(char::is_control),
        "LIB-TOOL-013: precondition: {text:?} holds no other control character"
    );
    escaped
}

/// The read error for the matrix under [`control_root`], escaped.
fn control_root_read_error(root: &Path) -> String {
    let path = root.join(MATRIX_PATH);
    let error = fs::read_to_string(&path)
        .expect_err("LIB-TOOL-014: precondition: the matrix under a missing root cannot be read");
    assert!(
        !error.to_string().chars().any(char::is_control),
        "LIB-TOOL-014: precondition: the I/O error {error:?} holds no control character"
    );
    format!(
        "read {}: {error}",
        escape_control_root(&path.display().to_string())
    )
}

#[test]
fn lib_tool_014_control_characters_in_the_read_error_are_escaped() {
    let root = control_root();
    let expected = control_root_read_error(&root);
    assert!(
        expected.contains("\\n  LIB-TOOL-001: forged\\u{1b}[2K"),
        "LIB-TOOL-014: precondition: the expected path is escaped, got {expected:?}"
    );
    for args in LOADING_COMMANDS {
        assert_eq!(
            call("LIB-TOOL-014", Entry::Run, args, &root),
            (Err(expected.clone()), String::new()),
            "LIB-TOOL-014: `{}` on a root whose name holds control characters reports the read \
             error on one line, the path's control characters escaped",
            args.join(" ")
        );
    }
}

#[test]
fn lib_tool_014_every_command_returns_the_same_load_error() {
    let tree = broken_tree("report-every-command");
    let expected = syntax_error_line();
    for args in LOADING_COMMANDS {
        for entry in [Entry::Run, Entry::From(RootSource::Build)] {
            assert_eq!(
                call("LIB-TOOL-014", entry, args, tree.root()),
                (Err(expected.clone()), String::new()),
                "LIB-TOOL-014: `{}` through {entry:?} returns the one-line load error and writes \
                 nothing",
                args.join(" ")
            );
        }
    }
    assert!(
        !tree.root().join(RENDERED_PATH).exists(),
        "LIB-TOOL-014: `matrix render` of a matrix that does not parse writes no page"
    );
}

#[test]
fn lib_tool_014_binary_prints_the_load_error_on_one_stderr_line() {
    let tree = broken_tree("report-binary-load");
    let line = syntax_error_line();
    for args in LOADING_COMMANDS {
        // Under `XTASK_ROOT`, only `matrix check` names the root (LIB-TOOL-013).
        let named = if args == ["matrix", "check"] {
            format!("{line}{}", suffix(tree.root()))
        } else {
            line.clone()
        };
        assert_eq!(
            binary("LIB-TOOL-014", args, Some(tree.root())),
            (false, String::new(), format!("xtask: {named}\n")),
            "LIB-TOOL-014: `xtask {}` fails, prints nothing on stdout, and prints the load error \
             as `xtask: <line>` and a newline on stderr",
            args.join(" ")
        );
    }
}

#[test]
fn lib_tool_014_binary_prints_the_read_error_on_one_stderr_line() {
    let tree = unreadable_tree("report-binary-read");
    let line = read_error(tree.root());
    for args in LOADING_COMMANDS {
        let named = if args == ["matrix", "check"] {
            format!("{line}{}", suffix(tree.root()))
        } else {
            line.clone()
        };
        assert_eq!(
            binary("LIB-TOOL-014", args, Some(tree.root())),
            (false, String::new(), format!("xtask: {named}\n")),
            "LIB-TOOL-014: `xtask {}` on an unreadable matrix fails, prints nothing on stdout, \
             and prints `xtask: read <path>: <error>` and a newline on stderr",
            args.join(" ")
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-TOOL-013: the library entry point
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_013_a_passing_check_from_the_build_root_writes_the_unnamed_ok_line() {
    let tree = consistent_tree("report-ok-build");
    for entry in [Entry::Run, Entry::From(RootSource::Build)] {
        assert_eq!(
            call("LIB-TOOL-013", entry, &["matrix", "check"], tree.root()),
            (Ok(()), "functionality matrix OK: 1 rows\n".to_owned()),
            "LIB-TOOL-013: through {entry:?}, a passing check writes exactly the unnamed OK line"
        );
    }
}

#[test]
fn lib_tool_013_a_passing_check_from_xtask_root_names_the_root() {
    let tree = consistent_tree("report-ok-env");
    assert_eq!(
        check_from("LIB-TOOL-013", tree.root(), RootSource::Environment),
        (
            Ok(()),
            format!("functionality matrix OK: 1 rows{}\n", suffix(tree.root()))
        ),
        "LIB-TOOL-013: under `XTASK_ROOT`, a passing check writes exactly \
         `functionality matrix OK: <n> rows (root: <p>)` and a newline"
    );
}

#[test]
fn lib_tool_013_a_failing_check_from_xtask_root_names_the_root_on_the_first_line_only() {
    let tree = failing_tree("report-fail-env");
    let named = check_from("LIB-TOOL-013", tree.root(), RootSource::Environment);
    assert_eq!(
        named,
        (Err(failing_error(&suffix(tree.root()))), String::new()),
        "LIB-TOOL-013: under `XTASK_ROOT`, the first line is \
         `functionality matrix: <n> problem(s) (root: <p>)` and the problem lines follow unchanged"
    );
    for entry in [Entry::Run, Entry::From(RootSource::Build)] {
        assert_eq!(
            call("LIB-TOOL-013", entry, &["matrix", "check"], tree.root()),
            (Err(failing_error("")), String::new()),
            "LIB-TOOL-013: through {entry:?}, the error is the LIB-TOOL-010 error, unnamed"
        );
    }
}

#[test]
fn lib_tool_013_a_load_failure_from_xtask_root_ends_with_the_root() {
    let broken = broken_tree("report-load-env");
    assert_eq!(
        check_from("LIB-TOOL-013", broken.root(), RootSource::Environment),
        (
            Err(format!("{}{}", syntax_error_line(), suffix(broken.root()))),
            String::new()
        ),
        "LIB-TOOL-013: under `XTASK_ROOT`, the one-line parse error ends with ` (root: <p>)`"
    );
    assert_eq!(
        check_from("LIB-TOOL-013", broken.root(), RootSource::Build),
        (Err(syntax_error_line()), String::new()),
        "LIB-TOOL-013: from the build root, the parse error is unnamed"
    );

    let unreadable = unreadable_tree("report-read-env");
    assert_eq!(
        check_from("LIB-TOOL-013", unreadable.root(), RootSource::Environment),
        (
            Err(format!(
                "{}{}",
                read_error(unreadable.root()),
                suffix(unreadable.root())
            )),
            String::new()
        ),
        "LIB-TOOL-013: under `XTASK_ROOT`, the one-line read error ends with ` (root: <p>)`"
    );
    assert_eq!(
        check_from("LIB-TOOL-013", unreadable.root(), RootSource::Build),
        (Err(read_error(unreadable.root())), String::new()),
        "LIB-TOOL-013: from the build root, the read error is unnamed"
    );
}

#[test]
fn lib_tool_013_a_control_character_in_the_root_is_escaped_in_the_suffix() {
    // Windows forbids control characters in file names, so the root does not
    // exist: the check fails to read the matrix, and both the path in the
    // read error and the root in the suffix are escaped.
    let root = control_root();
    let expected = format!(
        "{} (root: {})",
        control_root_read_error(&root),
        escape_control_root(&root.display().to_string())
    );
    assert!(
        expected.ends_with("\\n  LIB-TOOL-001: forged\\u{1b}[2K)"),
        "LIB-TOOL-013: precondition: the expected suffix is escaped, got {expected:?}"
    );
    assert_eq!(
        check_from("LIB-TOOL-013", &root, RootSource::Environment),
        (Err(expected), String::new()),
        "LIB-TOOL-013: control characters in the named root are escaped as in LIB-TOOL-010, so \
         they cannot split or forge a line"
    );
}

#[cfg(unix)]
#[test]
fn lib_tool_013_a_tree_whose_name_holds_control_characters_is_named_escaped() {
    let tree = consistent_tree("report-ctrl-\n  LIB-TOOL-001: forged\u{1b}[2K");
    let shown = escape_control_root(&tree.root().display().to_string());
    assert_eq!(
        check_from("LIB-TOOL-013", tree.root(), RootSource::Environment),
        (
            Ok(()),
            format!("functionality matrix OK: 1 rows (root: {shown})\n")
        ),
        "LIB-TOOL-013: a passing check under a root whose name holds control characters names it \
         escaped, on one line"
    );
}

#[test]
fn lib_tool_013_xtask_root_equal_to_the_build_root_is_still_named() {
    // The real repository: whatever its state, the reports differ only by
    // the suffix on the first line.
    let root = repo_root();
    let build = check_from("LIB-TOOL-013", &root, RootSource::Build);
    let named = check_from("LIB-TOOL-013", &root, RootSource::Environment);
    assert!(
        !first_line(&build).contains(" (root: "),
        "LIB-TOOL-013: from the build root, the first line is unnamed, got {build:?}"
    );
    let tail = suffix(&root);
    let expected = match &build {
        (Ok(()), out) => (Ok(()), with_first_line_suffix(out, &tail)),
        (Err(error), out) => (Err(with_first_line_suffix(error, &tail)), out.clone()),
    };
    assert_eq!(
        named, expected,
        "LIB-TOOL-013: an `XTASK_ROOT` equal to the build root is still named: the decision \
         depends on where the root came from, not on its value"
    );
}

#[test]
fn lib_tool_013_the_named_root_is_shown_as_given() {
    // No canonicalisation: a root given with a `..` component is shown so.
    let tree = consistent_tree("report-as-given");
    let given = tree.root().join("docs").join("..");
    assert_eq!(
        check_from("LIB-TOOL-013", &given, RootSource::Environment),
        (
            Ok(()),
            format!("functionality matrix OK: 1 rows{}\n", suffix(&given))
        ),
        "LIB-TOOL-013: the root is named as `Path::display` shows the root given, not a \
         canonical form"
    );
}

/// A tree with junk at the first artifact's path in the default destination
/// and in `custom/`, so `fetch-spec` fails on the hash before any download.
fn fetch_tree(name: &str) -> TempTree {
    let file = ARTIFACTS[0].file_name;
    let tree = TempTree::new(name);
    tree.write(&format!("{DEFAULT_DEST}/{file}"), "junk")
        .write(&format!("custom/{file}"), "junk");
    tree
}

#[test]
fn lib_tool_013_the_other_commands_are_unchanged_under_xtask_root() {
    let empty = TempTree::new("report-other-empty");
    let consistent = consistent_tree("report-other-consistent");
    let fetch = fetch_tree("report-other-fetch");
    let broken = broken_tree("report-other-broken");
    let unreadable = unreadable_tree("report-other-unreadable");
    let cases: Vec<(&[&str], &Path)> = vec![
        (&["help"], empty.root()),
        (&["--help"], empty.root()),
        (&["-h"], empty.root()),
        (&[], empty.root()),
        (&["frobnicate", "--now"], empty.root()),
        (&["matrix", "check", "--extra"], consistent.root()),
        (&["matrix", "next", "-n", "notanumber"], consistent.root()),
        (&["matrix", "next"], consistent.root()),
        (&["matrix", "next", "-n", "1"], consistent.root()),
        (&["matrix", "status"], consistent.root()),
        (&["fetch-spec"], fetch.root()),
        (&["fetch-spec", "--dest", "custom"], fetch.root()),
        (&["matrix", "render"], broken.root()),
        (&["matrix", "next"], broken.root()),
        (&["matrix", "status"], broken.root()),
        (&["matrix", "render"], unreadable.root()),
        (&["matrix", "next"], unreadable.root()),
        (&["matrix", "status"], unreadable.root()),
    ];
    for (args, root) in cases {
        let reports: Vec<_> = ENTRIES
            .iter()
            .map(|entry| call("LIB-TOOL-013", *entry, args, root))
            .collect();
        for (entry, report) in ENTRIES.iter().zip(&reports) {
            assert_eq!(
                report,
                &reports[0],
                "LIB-TOOL-013: `{}` through {entry:?} reports exactly what `run` reports",
                args.join(" ")
            );
        }
        let (result, out) = &reports[0];
        assert!(
            !out.contains("(root: ") && !result.as_ref().is_err_and(|e| e.contains("(root: ")),
            "LIB-TOOL-013: `{}` names no root, got {:?}",
            args.join(" "),
            reports[0]
        );
    }

    // `matrix render` writes the same page, and reports it the same way.
    let rendering = consistent_tree("report-other-render");
    let page = rendering.root().join(RENDERED_PATH);
    let mut pages = Vec::new();
    for entry in ENTRIES {
        rendering.write(RENDERED_PATH, "stale");
        assert_eq!(
            call(
                "LIB-TOOL-013",
                entry,
                &["matrix", "render"],
                rendering.root()
            ),
            (Ok(()), format!("wrote {RENDERED_PATH}\n")),
            "LIB-TOOL-013: `matrix render` through {entry:?} reports exactly `wrote <page>`"
        );
        pages.push(fs::read(&page).ok());
    }
    assert!(
        pages.iter().all(|p| p.is_some() && *p == pages[0]),
        "LIB-TOOL-013: `matrix render` writes the same page under every source"
    );
}

#[test]
fn lib_tool_013_run_is_run_from_with_the_build_source() {
    let trees = [
        consistent_tree("report-same-ok"),
        failing_tree("report-same-fail"),
        broken_tree("report-same-broken"),
        unreadable_tree("report-same-unreadable"),
    ];
    for tree in &trees {
        let run = call(
            "LIB-TOOL-013",
            Entry::Run,
            &["matrix", "check"],
            tree.root(),
        );
        assert_eq!(
            call(
                "LIB-TOOL-013",
                Entry::From(RootSource::Build),
                &["matrix", "check"],
                tree.root()
            ),
            run,
            "LIB-TOOL-013: `run` behaves exactly like `run_from` with `RootSource::Build`"
        );
        assert!(
            !first_line(&run).contains("(root: "),
            "LIB-TOOL-013: `run` keeps its unnamed output, got {run:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-TOOL-013: the binary
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_013_binary_names_xtask_root_on_a_passing_check() {
    let tree = consistent_tree("report-binary-ok");
    assert_eq!(
        binary("LIB-TOOL-013", &["matrix", "check"], Some(tree.root())),
        (
            true,
            format!("functionality matrix OK: 1 rows{}\n", suffix(tree.root())),
            String::new()
        ),
        "LIB-TOOL-013: with `XTASK_ROOT` set, a passing check exits with success and prints \
         exactly the named OK line on stdout and nothing on stderr"
    );
}

#[test]
fn lib_tool_013_binary_names_xtask_root_on_a_failing_check() {
    let tree = failing_tree("report-binary-fail");
    assert_eq!(
        binary("LIB-TOOL-013", &["matrix", "check"], Some(tree.root())),
        (
            false,
            String::new(),
            format!("xtask: {}\n", failing_error(&suffix(tree.root())))
        ),
        "LIB-TOOL-013: with `XTASK_ROOT` set, a failing check exits with a failure status, prints \
         nothing on stdout, and prints the named error with the `xtask: ` prefix on stderr"
    );
}

/// What the binary prints for a library report: `(success, stdout, stderr)`.
fn as_printed(report: (Result<(), String>, String)) -> (bool, String, String) {
    match report {
        (Ok(()), out) => (true, out, String::new()),
        (Err(error), out) => (false, out, format!("xtask: {error}\n")),
    }
}

#[test]
fn lib_tool_013_binary_names_xtask_root_equal_to_the_build_root() {
    let root = repo_root();
    let printed = binary("LIB-TOOL-013", &["matrix", "check"], Some(&root));
    assert_eq!(
        printed,
        as_printed(check_from("LIB-TOOL-013", &root, RootSource::Environment)),
        "LIB-TOOL-013: an `XTASK_ROOT` equal to the build root reaches the library as \
         `RootSource::Environment`"
    );
    let (ok, stdout, stderr) = &printed;
    let first = if *ok { stdout } else { stderr };
    assert!(
        first
            .split('\n')
            .next()
            .is_some_and(|l| l.ends_with(&suffix(&root))),
        "LIB-TOOL-013: the first line names the build root given in `XTASK_ROOT`, got {printed:?}"
    );
}

#[test]
fn lib_tool_013_binary_without_xtask_root_does_not_name_the_root() {
    let root = repo_root();
    let printed = binary("LIB-TOOL-013", &["matrix", "check"], None);
    assert_eq!(
        printed,
        as_printed(call(
            "LIB-TOOL-013",
            Entry::Run,
            &["matrix", "check"],
            &root
        )),
        "LIB-TOOL-013: without `XTASK_ROOT`, the binary prints exactly the unnamed report of \
         `run` on the build root"
    );
    assert!(
        !printed.1.contains("(root: ") && !printed.2.contains("(root: "),
        "LIB-TOOL-013: without `XTASK_ROOT`, no root is named, got {printed:?}"
    );
}

#[test]
fn lib_tool_013_binary_other_commands_are_unchanged_under_xtask_root() {
    let tree = consistent_tree("report-binary-other");
    for args in [
        &["matrix", "next"][..],
        &["matrix", "status"],
        &["help"],
        &["frobnicate"],
    ] {
        assert_eq!(
            binary("LIB-TOOL-013", args, Some(tree.root())),
            as_printed(call("LIB-TOOL-013", Entry::Run, args, tree.root())),
            "LIB-TOOL-013: with `XTASK_ROOT` set, `xtask {}` prints exactly what `run` reports",
            args.join(" ")
        );
    }
    tree.write(RENDERED_PATH, "stale");
    assert_eq!(
        binary("LIB-TOOL-013", &["matrix", "render"], Some(tree.root())),
        (true, format!("wrote {RENDERED_PATH}\n"), String::new()),
        "LIB-TOOL-013: with `XTASK_ROOT` set, `xtask matrix render` prints exactly `wrote <page>`"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-013: an empty `XTASK_ROOT` is an error, not a root
// ---------------------------------------------------------------------------

/// What the binary prints when `XTASK_ROOT` is set but empty.
const EMPTY_ROOT_ERROR: &str = "xtask: XTASK_ROOT is set but empty\n";

/// A tree on which every command would succeed or fail offline if it ran with
/// the empty path (the current directory) as its root: a consistent matrix
/// with a stale page, so `matrix render` would rewrite it, and junk at the
/// first artifact's path, so `fetch-spec` would fail on its hash without a
/// download.
fn empty_root_cwd(name: &str) -> TempTree {
    let tree = consistent_tree(name);
    let file = ARTIFACTS[0].file_name;
    tree.write(RENDERED_PATH, "stale")
        .write(&format!("{DEFAULT_DEST}/{file}"), "junk")
        .write(&format!("custom/{file}"), "junk");
    tree
}

/// Run the binary with `args` and `XTASK_ROOT` set to the empty string, in
/// the directory `cwd`: whether it succeeded, its stdout and its stderr.
fn binary_with_empty_root(args: &[&str], cwd: &Path) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .env(ROOT_ENV, "")
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|e| panic!("LIB-TOOL-013: the xtask binary can be started: {e}"));
    let text = |bytes: Vec<u8>| {
        String::from_utf8(bytes).unwrap_or_else(|e| {
            panic!(
                "LIB-TOOL-013: `xtask {}` printed non-UTF-8: {e}",
                args.join(" ")
            )
        })
    };
    (
        output.status.success(),
        text(output.stdout),
        text(output.stderr),
    )
}

/// The binary refuses `args` under an empty `XTASK_ROOT`: failure, nothing on
/// stdout, exactly [`EMPTY_ROOT_ERROR`] on stderr.
fn assert_refuses_empty_root(args: &[&str], cwd: &Path) {
    assert_eq!(
        binary_with_empty_root(args, cwd),
        (false, String::new(), EMPTY_ROOT_ERROR.to_owned()),
        "LIB-TOOL-013: with `XTASK_ROOT` set but empty, `xtask {}` fails, prints nothing on \
         stdout and exactly `xtask: XTASK_ROOT is set but empty` on stderr",
        args.join(" ")
    );
}

/// The bytes of `rel` under `tree`.
fn tree_bytes(tree: &TempTree, rel: &str) -> Option<Vec<u8>> {
    fs::read(tree.root().join(rel)).ok()
}

#[test]
fn lib_tool_013_binary_refuses_an_empty_xtask_root_for_matrix_check() {
    // A check that ran on the current directory would report its stale page
    // instead of refusing the root.
    let cwd = empty_root_cwd("report-empty-check");
    assert_refuses_empty_root(&["matrix", "check"], cwd.root());
}

#[test]
fn lib_tool_013_binary_refuses_an_empty_xtask_root_for_matrix_render_and_writes_no_page() {
    let cwd = empty_root_cwd("report-empty-render");
    assert_refuses_empty_root(&["matrix", "render"], cwd.root());
    assert_eq!(
        tree_bytes(&cwd, RENDERED_PATH),
        Some(b"stale".to_vec()),
        "LIB-TOOL-013: with `XTASK_ROOT` set but empty, `matrix render` runs no command: the \
         stale page in the current directory is left as it was"
    );
}

#[test]
fn lib_tool_013_binary_refuses_an_empty_xtask_root_for_help() {
    let cwd = empty_root_cwd("report-empty-help");
    assert_refuses_empty_root(&["help"], cwd.root());
}

#[test]
fn lib_tool_013_binary_refuses_an_empty_xtask_root_for_every_command() {
    let cwd = empty_root_cwd("report-empty-every");
    let file = ARTIFACTS[0].file_name;
    for args in [
        &["matrix", "check"][..],
        &["matrix", "render"],
        &["matrix", "next"],
        &["matrix", "next", "-n", "3"],
        &["matrix", "next", "-n", "notanumber"],
        &["matrix", "status"],
        &["fetch-spec"],
        &["fetch-spec", "--dest", "custom"],
        &["help"],
        &["--help"],
        &["-h"],
        &[],
        &["frobnicate", "--now"],
    ] {
        assert_refuses_empty_root(args, cwd.root());
    }
    for (rel, before) in [
        (RENDERED_PATH.to_owned(), "stale"),
        (format!("{DEFAULT_DEST}/{file}"), "junk"),
        (format!("custom/{file}"), "junk"),
    ] {
        assert_eq!(
            tree_bytes(&cwd, &rel),
            Some(before.as_bytes().to_vec()),
            "LIB-TOOL-013: with `XTASK_ROOT` set but empty, no command runs: {rel} in the \
             current directory is left as it was"
        );
    }
}
