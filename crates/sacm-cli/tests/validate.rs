//! End-to-end tests for `sacm validate` (LIB-CLI-002; ADR 0023 and its
//! amendment 1).
//!
//! `sacm validate` reads strictly by default; `--strict` names the default
//! and `--tolerant` reads in tolerant mode, whose summary says `readable`,
//! never `valid`. A file a strict read refuses but a tolerant read opens is
//! `invalid` (exit 1, the refusal on stdout); a file neither mode opens is
//! unreadable (exit 2, the refusal on stderr).
//!
//! The binary runs in `tests/data`, with the fixture's file name as its
//! argument, so that the file name it prints is the same on every machine.
//! The expected diagnostic lines are taken from the library (`xmi::read` and
//! `validation::validate` on the same bytes, in the mode the case reads in),
//! so these tests check what the command adds: the streams, the summary, the
//! exit code and the JSON form.
//!
//! The JSON form is pinned by golden files in `tests/data/golden/`, one line
//! each. ADR 0023 fixes the top-level keys and the diagnostic keys; the shape
//! of a location (`{"line":L,"column":C}`) and of the refusal
//! (`{"code","location","locations","message"}`, with the `ReadError`
//! `Display` as the message) are this test's reading of it, raised with the
//! lead. `lib_cli_002_json_goldens_agree_with_the_library` checks that each
//! golden's values are the library's.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sacm::diagnostic::{Diagnostic, SourceLocation};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions};
use serde_json::Value;

const ROW: &str = "LIB-CLI-002";

// --- Running the binary -------------------------------------------------------

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

/// Runs `sacm` with `args` in `dir`.
fn sacm_in(dir: &Path, args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_sacm"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|error| panic!("{ROW}: cannot run sacm {args:?}: {error}"));
    let text = |bytes: Vec<u8>, stream: &str| {
        String::from_utf8(bytes).unwrap_or_else(|error| {
            panic!("{ROW}: sacm {args:?} wrote non-UTF-8 to {stream}: {error}")
        })
    };
    let stdout = text(output.stdout, "stdout");
    let stderr = text(output.stderr, "stderr");
    assert!(
        !stderr.contains("panicked"),
        "{ROW}: sacm {args:?} panicked: {stderr}"
    );
    Run {
        code: output.status.code(),
        stdout,
        stderr,
    }
}

/// Runs `sacm validate` with `args` in `tests/data`.
fn validate(args: &[&str]) -> Run {
    let mut all = vec!["validate"];
    all.extend_from_slice(args);
    sacm_in(&data_dir(), &all)
}

// --- What the library says about a fixture -----------------------------------

fn fixture(name: &str) -> Vec<u8> {
    let path = data_dir().join(name);
    fs::read(&path).unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()))
}

fn read_fixture(name: &str, mode: Mode) -> Result<xmi::ReadOutput, ReadError> {
    xmi::read(&fixture(name), &ReadOptions::default().with_mode(mode))
}

/// The read and validation diagnostics of a readable fixture, together and
/// sorted, as the command must report them.
fn findings(name: &str, mode: Mode) -> Vec<Diagnostic> {
    let output = read_fixture(name, mode)
        .unwrap_or_else(|error| panic!("{ROW}: setup: {name} is not readable: {error}"));
    let mut all = output.diagnostics().to_vec();
    all.extend(validation::validate(output.document()));
    all.sort();
    all
}

/// The library's refusal of a fixture that a read in `mode` refuses.
fn refusal(name: &str, mode: Mode) -> ReadError {
    match read_fixture(name, mode) {
        Ok(_) => panic!("{ROW}: setup: {name} is readable in {mode:?} mode"),
        Err(error) => error,
    }
}

/// The library's strict refusal of a fixture that a tolerant read opens.
fn strict_only_refusal(name: &str) -> ReadError {
    let error = refusal(name, Mode::Strict);
    if let Err(tolerant) = read_fixture(name, Mode::Tolerant) {
        panic!("{ROW}: setup: a tolerant read refuses {name}: {tolerant}");
    }
    error
}

/// The JSON of a refusal: `{"code","location","locations","message"}`.
fn json_refusal(error: &ReadError) -> String {
    let locations: Vec<String> = error
        .locations()
        .iter()
        .copied()
        .map(json_location)
        .collect();
    // Every refusal has a code since slice 0063 (LIB-API-017).
    let code = json_string(error.code().as_str());
    format!(
        "{{\"code\":{},\"location\":{},\"locations\":[{}],\"message\":{}}}",
        code,
        error
            .location()
            .map_or_else(|| "null".to_owned(), json_location),
        locations.join(","),
        json_string(&error.to_string())
    )
}

/// The `Display` lines of `diagnostics`.
fn lines_of(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics.iter().map(ToString::to_string).collect()
}

fn golden(name: &str) -> String {
    let path = data_dir().join("golden").join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()))
}

/// `text` as a JSON string, escaped per RFC 8259 §7: `"` and `\`, the short
/// forms for backspace, form feed, LF, CR and TAB, and `\u00XX` for every
/// other control character. Used only for values this test cannot pin
/// (texts the library produces).
fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_location(location: SourceLocation) -> String {
    format!(
        "{{\"line\":{},\"column\":{}}}",
        location.line(),
        location.column()
    )
}

/// Asserts that `run` is an unreadable file: exit 2, nothing on stdout, and
/// one stderr line `sacm: <file>: <code|error> <ReadError>` (ADR 0023).
fn assert_unreadable(what: &str, run: &Run, file: &str, code: &str, error: &ReadError) {
    assert_eq!(
        run.code,
        Some(2),
        "{ROW}: {what}: exit code; stderr: {}",
        run.stderr
    );
    assert_eq!(run.stdout, "", "{ROW}: {what}: stdout is not empty");
    assert_eq!(
        run.stderr.lines().collect::<Vec<_>>(),
        [format!("sacm: {file}: {code} {error}")],
        "{ROW}: {what}: stderr is not the one refusal line"
    );
}

// --- Human format --------------------------------------------------------------

/// Asserts that `run` is the human report of a readable file: exit `code`,
/// stdout the library's diagnostic lines `expected` then `summary`, and
/// stderr empty.
fn assert_report(args: &[&str], run: &Run, code: i32, expected: &[String], summary: &str) {
    assert_eq!(
        run.code,
        Some(code),
        "{ROW}: {args:?}: exit code; stdout: {}; stderr: {}",
        run.stdout,
        run.stderr
    );
    let lines: Vec<&str> = run.stdout.lines().collect();
    let Some((last, diagnostics)) = lines.split_last() else {
        panic!("{ROW}: {args:?}: stdout is empty; stderr: {}", run.stderr);
    };
    assert_eq!(
        diagnostics, expected,
        "{ROW}: {args:?}: the diagnostic lines are not the library's, in order"
    );
    assert_eq!(
        *last, summary,
        "{ROW}: {args:?}: the last line is not the summary with its counts"
    );
    assert_eq!(run.stderr, "", "{ROW}: {args:?}: stderr is not empty");
}

#[test]
fn lib_cli_002_a_clean_file_exits_0_and_prints_only_the_valid_summary() {
    assert!(
        findings("clean.sacm.xmi", Mode::Strict).is_empty(),
        "{ROW}: setup: the clean fixture has findings in strict mode"
    );
    for args in [
        &["clean.sacm.xmi"][..],
        &["--strict", "clean.sacm.xmi"],
        &["--format", "human", "clean.sacm.xmi"],
    ] {
        let run = validate(args);
        assert_eq!(
            run.code,
            Some(0),
            "{ROW}: {args:?}: exit code; stderr: {}",
            run.stderr
        );
        assert_eq!(
            run.stdout, "clean.sacm.xmi: valid (0 errors, 0 warnings, 0 info)\n",
            "{ROW}: {args:?}: stdout is not only the summary"
        );
        assert_eq!(run.stderr, "", "{ROW}: {args:?}: stderr is not empty");
    }
}

#[test]
fn lib_cli_002_with_tolerant_the_summary_says_readable_never_valid() {
    let cases = [
        (
            "clean.sacm.xmi",
            0,
            None,
            "clean.sacm.xmi: readable (0 errors, 0 warnings, 0 info)",
        ),
        (
            "xsi-type.sacm.xmi",
            0,
            Some("warning SACM-XMI-010 "),
            "xsi-type.sacm.xmi: readable (0 errors, 1 warning, 0 info)",
        ),
        (
            "nameless-claim.sacm.xmi",
            1,
            Some("error SACM-MULT-001 "),
            "nameless-claim.sacm.xmi: invalid (1 error, 0 warnings, 0 info)",
        ),
        (
            "bad-literal.sacm.xmi",
            0,
            Some("warning SACM-ENUM-001 "),
            "bad-literal.sacm.xmi: readable (0 errors, 1 warning, 0 info)",
        ),
    ];
    for (file, code, line_start, summary) in cases {
        let expected = lines_of(&findings(file, Mode::Tolerant));
        match line_start {
            None => assert!(
                expected.is_empty(),
                "{ROW}: setup: {file} has findings in tolerant mode: {expected:?}"
            ),
            Some(start) => assert!(
                expected.iter().any(|line| line.starts_with(start)),
                "{ROW}: setup: the library reports no {start:?} for {file} in tolerant mode: \
                 {expected:?}"
            ),
        }
        let args = ["--tolerant", file];
        let run = validate(&args);
        assert!(
            !run.stdout.lines().any(|line| line.contains(": valid (")),
            "{ROW}: {args:?}: a tolerant read is reported as valid: {:?}",
            run.stdout
        );
        assert_report(&args, &run, code, &expected, summary);
    }
}

#[test]
fn lib_cli_002_a_nameless_claim_exits_1_with_the_mult_001_line_and_an_invalid_summary() {
    let expected = lines_of(&findings("nameless-claim.sacm.xmi", Mode::Strict));
    assert!(
        expected
            .iter()
            .any(|line| line.starts_with("error SACM-MULT-001 ")),
        "{ROW}: setup: the library reports no SACM-MULT-001: {expected:?}"
    );
    let args = ["nameless-claim.sacm.xmi"];
    assert_report(
        &args,
        &validate(&args),
        1,
        &expected,
        "nameless-claim.sacm.xmi: invalid (1 error, 0 warnings, 0 info)",
    );
}

#[test]
fn lib_cli_002_xsi_type_is_an_error_by_default_and_a_warning_with_tolerant() {
    let cases = [
        (
            &["xsi-type.sacm.xmi"][..],
            Mode::Strict,
            1,
            "error SACM-XMI-010 ",
            "xsi-type.sacm.xmi: invalid (1 error, 0 warnings, 0 info)",
        ),
        (
            &["--strict", "xsi-type.sacm.xmi"],
            Mode::Strict,
            1,
            "error SACM-XMI-010 ",
            "xsi-type.sacm.xmi: invalid (1 error, 0 warnings, 0 info)",
        ),
        (
            &["--tolerant", "xsi-type.sacm.xmi"],
            Mode::Tolerant,
            0,
            "warning SACM-XMI-010 ",
            "xsi-type.sacm.xmi: readable (0 errors, 1 warning, 0 info)",
        ),
    ];
    for (args, mode, code, line_start, wanted_summary) in cases {
        let expected = lines_of(&findings("xsi-type.sacm.xmi", mode));
        assert!(
            expected.iter().any(|line| line.starts_with(line_start)),
            "{ROW}: setup: the library reports no {line_start:?} in {mode:?} mode: {expected:?}"
        );
        assert_report(args, &validate(args), code, &expected, wanted_summary);
    }
}

#[test]
fn lib_cli_002_a_strict_refusal_of_a_file_a_tolerant_read_opens_exits_1_on_stdout() {
    // An Unsupported refusal prints its code since slice 0063 (LIB-API-017).
    let cases = [
        ("bad-literal.sacm.xmi", "SACM-ENUM-001"),
        ("xmi-attribute.sacm.xmi", "SACM-XMI-003"),
    ];
    for (file, code) in cases {
        let error = strict_only_refusal(file);
        assert_eq!(
            error.code().as_str(),
            code,
            "{ROW}: setup: the strict refusal of {file} has another code: {error:?}"
        );
        let expected = format!(
            "{file}: {code} {error}\n{file}: invalid (refused by a strict read; --tolerant opens it)\n"
        );
        for args in [&[file][..], &["--strict", file]] {
            let run = validate(args);
            assert_eq!(
                run.code,
                Some(1),
                "{ROW}: {args:?}: exit code; stdout: {}; stderr: {}",
                run.stdout,
                run.stderr
            );
            assert_eq!(
                run.stdout, expected,
                "{ROW}: {args:?}: stdout is not the strict refusal and the invalid summary"
            );
            assert_eq!(run.stderr, "", "{ROW}: {args:?}: stderr is not empty");
        }
    }
}

#[test]
fn lib_cli_002_a_missing_malformed_or_duplicate_id_file_exits_2_naming_the_file_and_code() {
    for (mode, flags) in [
        (Mode::Strict, &[][..]),
        (Mode::Strict, &["--strict"]),
        (Mode::Tolerant, &["--tolerant"]),
    ] {
        let args = |file: &'static str| {
            let mut all = flags.to_vec();
            all.push(file);
            all
        };
        let malformed = refusal("malformed.sacm.xmi", mode);
        assert_unreadable(
            &format!("malformed XML, {flags:?}"),
            &validate(&args("malformed.sacm.xmi")),
            "malformed.sacm.xmi",
            "SACM-XML-001",
            &malformed,
        );

        let run = validate(&args("duplicate-id.sacm.xmi"));
        assert!(
            run.stderr
                .starts_with("sacm: duplicate-id.sacm.xmi: SACM-ID-001 "),
            "{ROW}: {flags:?}: a duplicate id does not name the file and SACM-ID-001 on stderr: \
             {:?}",
            run.stderr
        );
        assert_unreadable(
            &format!("a duplicate id, {flags:?}"),
            &run,
            "duplicate-id.sacm.xmi",
            "SACM-ID-001",
            &refusal("duplicate-id.sacm.xmi", mode),
        );

        let missing = validate(&args("missing.sacm.xmi"));
        assert_eq!(
            missing.code,
            Some(2),
            "{ROW}: {flags:?}: a missing file: exit code; stderr: {}",
            missing.stderr
        );
        assert_eq!(
            missing.stdout, "",
            "{ROW}: {flags:?}: a missing file: stdout is not empty"
        );
        assert!(
            missing.stderr.starts_with("sacm: missing.sacm.xmi: ")
                && missing.stderr.lines().count() == 1,
            "{ROW}: {flags:?}: a missing file is not one stderr line naming it: {:?}",
            missing.stderr
        );
    }
}

#[test]
fn lib_cli_002_a_file_neither_mode_opens_exits_2_with_its_code() {
    for (mode, flags) in [
        (Mode::Strict, &[][..]),
        (Mode::Strict, &["--strict"]),
        (Mode::Tolerant, &["--tolerant"]),
    ] {
        let error = refusal("latin-1.sacm.xmi", mode);
        // An unread encoding is SACM-XML-002 since slice 0063 (LIB-API-017).
        assert!(
            matches!(error, ReadError::Unsupported { .. })
                && error.code() == sacm::diagnostic::Code::XML_ENCODING_NOT_READ,
            "{ROW}: setup: latin-1.sacm.xmi is not refused as Unsupported with SACM-XML-002 in \
             {mode:?} mode: {error:?}"
        );
        let printed = "SACM-XML-002";
        let mut args = flags.to_vec();
        args.push("latin-1.sacm.xmi");
        assert_unreadable(
            &format!("an ISO-8859-1 file, {flags:?}"),
            &validate(&args),
            "latin-1.sacm.xmi",
            printed,
            &error,
        );
    }
}

// --- JSON format -----------------------------------------------------------------

#[test]
fn lib_cli_002_json_output_equals_the_golden_text_and_is_identical_across_runs() {
    let cases = [
        (
            &["--format", "json", "clean.sacm.xmi"][..],
            "validate-clean.json",
            0,
        ),
        (
            &["--tolerant", "--format", "json", "clean.sacm.xmi"],
            "validate-clean-tolerant.json",
            0,
        ),
        (
            &["--format", "json", "nameless-claim.sacm.xmi"],
            "validate-nameless-claim.json",
            1,
        ),
        (
            &["--format", "json", "xsi-type.sacm.xmi"],
            "validate-xsi-type.json",
            1,
        ),
        (
            &["--strict", "--format", "json", "xsi-type.sacm.xmi"],
            "validate-xsi-type.json",
            1,
        ),
        (
            &["--tolerant", "--format", "json", "xsi-type.sacm.xmi"],
            "validate-xsi-type-tolerant.json",
            0,
        ),
        (
            &["--format", "json", "bad-literal.sacm.xmi"],
            "validate-bad-literal.json",
            1,
        ),
        (
            &["--tolerant", "--format", "json", "bad-literal.sacm.xmi"],
            "validate-bad-literal-tolerant.json",
            0,
        ),
        (
            &["--format", "json", "malformed.sacm.xmi"],
            "validate-malformed.json",
            2,
        ),
    ];
    for (args, name, code) in cases {
        let expected = golden(name);
        let first = validate(args);
        assert_eq!(
            first.code,
            Some(code),
            "{ROW}: {args:?}: exit code; stderr: {}",
            first.stderr
        );
        assert_eq!(
            first.stdout, expected,
            "{ROW}: {args:?}: stdout is not {name}"
        );
        // ADR 0023: in JSON mode every result, an unreadable file included,
        // is the object on stdout; only a usage error goes to stderr.
        assert_eq!(first.stderr, "", "{ROW}: {args:?}: stderr is not empty");
        let second = validate(args);
        assert_eq!(
            second.stdout, first.stdout,
            "{ROW}: {args:?}: two runs differ"
        );
    }
}

#[test]
fn lib_cli_002_json_for_an_unsupported_strict_refusal_is_invalid_or_unreadable_by_what_a_tolerant_read_opens()
 {
    // A tolerant read opens xmi-attribute.sacm.xmi: the strict refusal is a
    // failed check.
    let error = strict_only_refusal("xmi-attribute.sacm.xmi");
    // A defect on the wrapper is SACM-XMI-003 since slice 0063 (LIB-API-017).
    assert!(
        matches!(error, ReadError::Unsupported { .. })
            && error.code() == sacm::diagnostic::Code::XMI_UNKNOWN_CONTENT
            && error.location().map(|at| (at.line(), at.column())) == Some((3, 110)),
        "{ROW}: setup: xmi-attribute.sacm.xmi is not refused strictly as Unsupported with \
         SACM-XMI-003 at line 3, column 110: {error:?}"
    );
    let expected = format!(
        "{{\"format\":1,\"file\":\"xmi-attribute.sacm.xmi\",\"mode\":\"strict\",\"result\":\"invalid\",\
         \"refusal\":{},\"diagnostics\":[]}}\n",
        json_refusal(&error)
    );
    assert!(
        expected.contains(r#""refusal":{"code":"SACM-XMI-003","location":{"line":3,"column":110},"locations":[{"line":3,"column":110}],"#),
        "{ROW}: setup: the expected refusal is not SACM-XMI-003 at line 3, column 110: {expected}"
    );
    let run = validate(&["--format", "json", "xmi-attribute.sacm.xmi"]);
    assert_eq!(
        run.code,
        Some(1),
        "{ROW}: xmi-attribute: exit code; stdout: {}; stderr: {}",
        run.stdout,
        run.stderr
    );
    assert_eq!(run.stdout, expected, "{ROW}: the JSON of xmi-attribute");
    assert_eq!(run.stderr, "", "{ROW}: xmi-attribute: stderr is not empty");

    // Neither mode opens latin-1.sacm.xmi: it is unreadable.
    let error = refusal("latin-1.sacm.xmi", Mode::Strict);
    assert!(
        read_fixture("latin-1.sacm.xmi", Mode::Tolerant).is_err()
            && error.code() == sacm::diagnostic::Code::XML_ENCODING_NOT_READ,
        "{ROW}: setup: latin-1.sacm.xmi is read tolerantly, or its strict refusal is not \
         SACM-XML-002: {error:?}"
    );
    let expected = format!(
        "{{\"format\":1,\"file\":\"latin-1.sacm.xmi\",\"mode\":\"strict\",\"result\":\"unreadable\",\
         \"refusal\":{},\"diagnostics\":[]}}\n",
        json_refusal(&error)
    );
    let run = validate(&["--format", "json", "latin-1.sacm.xmi"]);
    assert_eq!(
        run.code,
        Some(2),
        "{ROW}: latin-1: exit code; stdout: {}; stderr: {}",
        run.stdout,
        run.stderr
    );
    assert_eq!(run.stdout, expected, "{ROW}: the JSON of latin-1");
    assert_eq!(run.stderr, "", "{ROW}: latin-1: stderr is not empty");
}

#[test]
fn lib_cli_002_json_for_a_file_a_tolerant_read_refuses_names_the_tolerant_mode() {
    // The refusal reported is the one in the mode asked for, and `mode` says
    // which: `tolerant` with `--tolerant`, for both a refusal and an I/O error.
    let error = refusal("latin-1.sacm.xmi", Mode::Tolerant);
    let expected = format!(
        "{{\"format\":1,\"file\":\"latin-1.sacm.xmi\",\"mode\":\"tolerant\",\"result\":\"unreadable\",\
         \"refusal\":{},\"diagnostics\":[]}}\n",
        json_refusal(&error)
    );
    let run = validate(&["--tolerant", "--format", "json", "latin-1.sacm.xmi"]);
    assert_eq!(
        run.code,
        Some(2),
        "{ROW}: latin-1, --tolerant: exit code; stdout: {}; stderr: {}",
        run.stdout,
        run.stderr
    );
    assert_eq!(
        run.stdout, expected,
        "{ROW}: the JSON of latin-1 read with --tolerant"
    );
    assert_eq!(
        run.stderr, "",
        "{ROW}: latin-1, --tolerant: stderr is not empty"
    );

    let run = validate(&["--tolerant", "--format", "json", "missing.sacm.xmi"]);
    assert_eq!(
        run.code,
        Some(2),
        "{ROW}: a missing file, --tolerant: exit code; stderr: {}",
        run.stderr
    );
    assert!(
        run.stdout.starts_with(
            "{\"format\":1,\"file\":\"missing.sacm.xmi\",\"mode\":\"tolerant\",\"result\":\"unreadable\",\"refusal\":{\"code\":null,"
        ),
        "{ROW}: a missing file read with --tolerant is not an unreadable result in tolerant mode: \
         {:?}",
        run.stdout
    );
}

#[test]
fn lib_cli_002_json_for_a_duplicate_id_has_the_refusal_with_its_code_and_every_location() {
    let error = refusal("duplicate-id.sacm.xmi", Mode::Strict);
    // The Claims carrying c1 start at line 5, column 3 and line 8, column 3;
    // the second is the first repeat.
    let first = r#"{"line":5,"column":3}"#;
    let repeat = r#"{"line":8,"column":3}"#;
    let expected = format!(
        "{{\"format\":1,\"file\":\"duplicate-id.sacm.xmi\",\"mode\":\"strict\",\"result\":\"unreadable\",\
         \"refusal\":{{\"code\":\"SACM-ID-001\",\"location\":{repeat},\"locations\":[{first},{repeat}],\"message\":{}}},\
         \"diagnostics\":[]}}\n",
        json_string(&error.to_string()),
    );
    let run = validate(&["--format", "json", "duplicate-id.sacm.xmi"]);
    assert_eq!(
        run.code,
        Some(2),
        "{ROW}: exit code; stderr: {}",
        run.stderr
    );
    assert_eq!(run.stdout, expected, "{ROW}: the JSON of a duplicate id");
    assert_eq!(
        run.stderr, "",
        "{ROW}: a duplicate id in JSON mode writes to stderr"
    );
}

#[test]
fn lib_cli_002_json_for_a_missing_file_is_unreadable_with_a_refusal() {
    let run = validate(&["--format", "json", "missing.sacm.xmi"]);
    assert_eq!(
        run.code,
        Some(2),
        "{ROW}: exit code; stderr: {}",
        run.stderr
    );
    assert_eq!(
        run.stderr, "",
        "{ROW}: a missing file in JSON mode writes to stderr"
    );
    let value: Value = serde_json::from_str(&run.stdout)
        .unwrap_or_else(|error| panic!("{ROW}: stdout is not JSON ({error}): {:?}", run.stdout));
    let keys: Vec<&str> = value
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default();
    // serde_json sorts the keys of a map, so this compares the set of keys.
    let mut wanted = ["format", "file", "mode", "result", "refusal", "diagnostics"];
    wanted.sort_unstable();
    assert_eq!(keys, wanted, "{ROW}: the keys of the JSON object");
    assert!(
        run.stdout.starts_with("{\"format\":1,\"file\":\"missing.sacm.xmi\",\"mode\":\"strict\",\"result\":\"unreadable\",\"refusal\":{"),
        "{ROW}: the JSON does not start as an unreadable result with a refusal: {:?}",
        run.stdout
    );
    assert_eq!(
        value["refusal"]["code"],
        Value::Null,
        "{ROW}: an I/O error has a code"
    );
    assert_eq!(
        value["diagnostics"],
        Value::Array(Vec::new()),
        "{ROW}: diagnostics"
    );
}

#[test]
fn lib_cli_002_json_escapes_quotes_backslashes_and_line_breaks_per_rfc_8259() {
    let run = validate(&["--format", "json", "escaped-ids.sacm.xmi"]);
    assert_eq!(
        run.code,
        Some(1),
        "{ROW}: exit code; stderr: {}",
        run.stderr
    );
    let Some(line) = run.stdout.strip_suffix('\n') else {
        panic!(
            "{ROW}: the JSON does not end with a newline: {:?}",
            run.stdout
        );
    };
    assert!(
        !line.contains(['\n', '\r']),
        "{ROW}: the JSON holds a raw line break: {line:?}"
    );
    // The ids, raw in `affected`: `a\b` and `a`, CR, LF, `b`.
    for escaped in [r#""affected":["a\\b"]"#, r#""affected":["a\r\nb"]"#] {
        assert!(
            line.contains(escaped),
            "{ROW}: the JSON does not hold {escaped}: {line}"
        );
    }
    // The messages quote the ids with `{:?}`, whose `"` and `\` are escaped again.
    for escaped in [r#"\"a\\\\b\""#, r#"\"a\\r\\nb\""#] {
        assert!(
            line.contains(escaped),
            "{ROW}: no message holds {escaped}: {line}"
        );
    }
    let value: Value = serde_json::from_str(line)
        .unwrap_or_else(|error| panic!("{ROW}: not JSON ({error}): {line}"));
    let affected: Vec<&str> = value["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|diagnostic| diagnostic["affected"][0].as_str())
        .collect();
    assert_eq!(
        affected,
        // Diagnostics are sorted, and "a\r\nb" sorts before "a\\b".
        ["a\r\nb", "a\\b"],
        "{ROW}: the parsed affected ids"
    );
}

/// U+0001, CR and LF cannot reach a message through the library, and a
/// Windows file name cannot hold them, so the file name carries them here.
#[cfg(unix)]
#[test]
fn lib_cli_002_json_escapes_control_characters_in_the_file_name() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("lib-cli-002-escapes");
    fs::create_dir_all(&dir).expect("create the test directory");
    let name = "q\"b\\s\r\n\u{1}.sacm.xmi";
    fs::write(dir.join(name), fixture("clean.sacm.xmi")).expect("write the oddly named file");
    let run = sacm_in(&dir, &["validate", "--format", "json", name]);
    assert_eq!(
        run.code,
        Some(0),
        "{ROW}: exit code; stderr: {}",
        run.stderr
    );
    assert_eq!(
        run.stdout,
        concat!(
            r#"{"format":1,"file":"q\"b\\s\r\n\u0001.sacm.xmi","mode":"strict","result":"valid","#,
            r#""refusal":null,"diagnostics":[]}"#,
            "\n"
        ),
        "{ROW}: the file name is not escaped per RFC 8259"
    );
}

#[test]
fn lib_cli_002_json_goldens_agree_with_the_library() {
    // A check on the golden files themselves: each value in them is the one
    // the library gives for the fixture.
    let readable = [
        ("validate-clean.json", "clean.sacm.xmi", Mode::Strict),
        (
            "validate-clean-tolerant.json",
            "clean.sacm.xmi",
            Mode::Tolerant,
        ),
        (
            "validate-nameless-claim.json",
            "nameless-claim.sacm.xmi",
            Mode::Strict,
        ),
        ("validate-xsi-type.json", "xsi-type.sacm.xmi", Mode::Strict),
        (
            "validate-xsi-type-tolerant.json",
            "xsi-type.sacm.xmi",
            Mode::Tolerant,
        ),
        (
            "validate-bad-literal-tolerant.json",
            "bad-literal.sacm.xmi",
            Mode::Tolerant,
        ),
    ];
    for (name, file, mode) in readable {
        let text = golden(name);
        let mut objects = Vec::new();
        for diagnostic in findings(file, mode) {
            let affected: Vec<String> = diagnostic
                .affected()
                .iter()
                .map(|id| json_string(id.as_str()))
                .collect();
            objects.push(format!(
                "{{\"code\":{},\"severity\":{},\"requirement\":{},\"clause\":{},\"location\":{},\"affected\":[{}],\"message\":{}}}",
                json_string(diagnostic.code().as_str()),
                json_string(diagnostic.severity().as_str()),
                json_string(diagnostic.requirement_id()),
                diagnostic.clause().map_or_else(|| "null".to_owned(), json_string),
                diagnostic.location().map_or_else(|| "null".to_owned(), json_location),
                affected.join(","),
                json_string(diagnostic.message()),
            ));
        }
        let expected = objects.join(",");
        assert!(
            text.contains(&format!("\"diagnostics\":[{expected}]}}\n")),
            "{ROW}: {name} does not hold the library's diagnostics:\n{expected}\n{text}"
        );
    }
    let error = refusal("malformed.sacm.xmi", Mode::Strict);
    let location = error.location().expect("a located refusal");
    let refusal_text = format!(
        "\"refusal\":{{\"code\":\"SACM-XML-001\",\"location\":{},\"locations\":[{}],\"message\":{}}}",
        json_location(location),
        json_location(location),
        json_string(&error.to_string())
    );
    let text = golden("validate-malformed.json");
    assert!(
        text.contains(&refusal_text),
        "{ROW}: validate-malformed.json does not hold the library's refusal:\n{refusal_text}\n{text}"
    );

    // The strict refusal of a file a tolerant read opens.
    let error = strict_only_refusal("bad-literal.sacm.xmi");
    let location = error.location().expect("a located refusal");
    let refusal_text = format!(
        "\"refusal\":{{\"code\":\"SACM-ENUM-001\",\"location\":{},\"locations\":[{}],\"message\":{}}},\"diagnostics\":[]}}\n",
        json_location(location),
        json_location(location),
        json_string(&error.to_string())
    );
    let text = golden("validate-bad-literal.json");
    assert!(
        text.contains(&refusal_text),
        "{ROW}: validate-bad-literal.json does not hold the library's strict refusal:\n\
         {refusal_text}\n{text}"
    );
}

// --- Arguments ---------------------------------------------------------------------

#[test]
fn lib_cli_002_an_unknown_flag_a_bad_format_or_no_file_exits_2_with_usage() {
    let cases = [
        &["--no-such-flag", "clean.sacm.xmi"][..],
        &["--format", "xml", "clean.sacm.xmi"],
        &["--strict", "--tolerant", "clean.sacm.xmi"],
        &[],
    ];
    // Both flags are known, and they conflict: the error names the two, not
    // an unknown argument.
    let both = validate(&["--strict", "--tolerant", "clean.sacm.xmi"]);
    assert!(
        both.stderr.lines().any(|line| line.starts_with("error:")
            && line.contains("--strict")
            && line.contains("--tolerant")),
        "{ROW}: --strict with --tolerant is not an error naming both flags: {}",
        both.stderr
    );
    for args in cases {
        let run = validate(args);
        assert_eq!(
            run.code,
            Some(2),
            "{ROW}: {args:?}: exit code; stderr: {}",
            run.stderr
        );
        assert_eq!(run.stdout, "", "{ROW}: {args:?}: stdout is not empty");
        assert!(
            // The program name is `sacm` or, on Windows, `sacm.exe`.
            run.stderr
                .lines()
                .any(|line| line.starts_with("Usage: sacm") && line.contains(" validate")),
            "{ROW}: {args:?}: stderr has no validate usage: {}",
            run.stderr
        );
    }
}

// --- LIB-API-017: every refusal has a code ------------------------------------------

/// The row of the refusal codes (slice 0063).
const API_017: &str = "LIB-API-017";

/// The JSON of a refusal whose code is `code`, as the command writes it.
fn json_refusal_with(code: &str, error: &ReadError) -> String {
    let locations: Vec<String> = error
        .locations()
        .iter()
        .copied()
        .map(json_location)
        .collect();
    format!(
        "{{\"code\":{},\"location\":{},\"locations\":[{}],\"message\":{}}}",
        json_string(code),
        error
            .location()
            .map_or_else(|| "null".to_owned(), json_location),
        locations.join(","),
        json_string(&error.to_string())
    )
}

#[test]
fn lib_api_017_sacm_validate_prints_the_codes_of_unsupported_refusals() {
    // A tolerant read opens xmi-attribute.sacm.xmi: its strict refusal, a
    // defect on the XMI element, is printed on stdout with SACM-XMI-003.
    let file = "xmi-attribute.sacm.xmi";
    let error = strict_only_refusal(file);
    let expected = format!(
        "{file}: SACM-XMI-003 {error}\n{file}: invalid (refused by a strict read; --tolerant opens it)\n"
    );
    for args in [&[file][..], &["--strict", file]] {
        let run = validate(args);
        assert_eq!(
            (run.code, run.stdout.as_str(), run.stderr.as_str()),
            (Some(1), expected.as_str(), ""),
            "{API_017}: {args:?}: the strict refusal is not printed with SACM-XMI-003, where 0.1.0 \
             printed error"
        );
    }
    // Neither mode opens latin-1.sacm.xmi: its refusal, an unread encoding,
    // is printed on stderr with SACM-XML-002 in every mode.
    let file = "latin-1.sacm.xmi";
    for (mode, flags) in [
        (Mode::Strict, &[][..]),
        (Mode::Strict, &["--strict"]),
        (Mode::Tolerant, &["--tolerant"]),
    ] {
        let error = refusal(file, mode);
        let mut args = flags.to_vec();
        args.push(file);
        let run = validate(&args);
        assert_eq!(
            (run.code, run.stdout.as_str()),
            (Some(2), ""),
            "{API_017}: {args:?}: exit code and stdout; stderr: {}",
            run.stderr
        );
        assert_eq!(
            run.stderr.lines().collect::<Vec<_>>(),
            [format!("sacm: {file}: SACM-XML-002 {error}")],
            "{API_017}: {args:?}: the refusal is not printed with SACM-XML-002, where 0.1.0 \
             printed error"
        );
    }
}

#[test]
fn lib_api_017_sacm_validate_json_writes_the_codes_of_unsupported_refusals() {
    let file = "xmi-attribute.sacm.xmi";
    let error = strict_only_refusal(file);
    let expected = format!(
        "{{\"format\":1,\"file\":\"{file}\",\"mode\":\"strict\",\"result\":\"invalid\",\
         \"refusal\":{},\"diagnostics\":[]}}\n",
        json_refusal_with("SACM-XMI-003", &error)
    );
    let run = validate(&["--format", "json", file]);
    assert_eq!(
        (run.code, run.stdout.as_str(), run.stderr.as_str()),
        (Some(1), expected.as_str(), ""),
        "{API_017}: the JSON of {file} does not write SACM-XMI-003, where 0.1.0 wrote null"
    );

    let file = "latin-1.sacm.xmi";
    for (mode, name, flags) in [
        (Mode::Strict, "strict", &["--format", "json"][..]),
        (
            Mode::Tolerant,
            "tolerant",
            &["--tolerant", "--format", "json"],
        ),
    ] {
        let error = refusal(file, mode);
        let expected = format!(
            "{{\"format\":1,\"file\":\"{file}\",\"mode\":\"{name}\",\"result\":\"unreadable\",\
             \"refusal\":{},\"diagnostics\":[]}}\n",
            json_refusal_with("SACM-XML-002", &error)
        );
        let mut args = flags.to_vec();
        args.push(file);
        let run = validate(&args);
        assert_eq!(
            (run.code, run.stdout.as_str(), run.stderr.as_str()),
            (Some(2), expected.as_str(), ""),
            "{API_017}: {args:?}: the JSON of {file} does not write SACM-XML-002, where 0.1.0 \
             wrote null"
        );
    }
}
