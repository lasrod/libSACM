//! End-to-end tests for `sacm roundtrip` (LIB-CLI-003; ADR 0023).
//!
//! The binary runs in `tests/data`, with the fixture's file name as its
//! argument, so that the file name it prints is the same on every machine.
//! `-o` files go to `CARGO_TARGET_TMPDIR`, one name per test. The expected
//! output bytes and Error lines are taken from the library (`xmi::read`
//! tolerant, then `xmi::write` with the default, strict, options).
//!
//! Not checked here: the difference report (each `Difference` line, then
//! `round trip failed: N differences`, exit 1). A correct library gives no
//! difference, so the binary cannot be driven into it; it needs a unit test
//! in the CLI crate.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sacm::model::AssertionDeclaration;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteError, WriteOptions};

const ROW: &str = "LIB-CLI-003";

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

/// Runs `sacm roundtrip` with `args` in `tests/data`.
fn roundtrip(args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_sacm"))
        .arg("roundtrip")
        .args(args)
        .current_dir(data_dir())
        .output()
        .unwrap_or_else(|error| panic!("{ROW}: cannot run sacm roundtrip {args:?}: {error}"));
    let text = |bytes: Vec<u8>, stream: &str| {
        String::from_utf8(bytes).unwrap_or_else(|error| {
            panic!("{ROW}: sacm roundtrip {args:?} wrote non-UTF-8 to {stream}: {error}")
        })
    };
    let stdout = text(output.stdout, "stdout");
    let stderr = text(output.stderr, "stderr");
    assert!(
        !stderr.contains("panicked"),
        "{ROW}: sacm roundtrip {args:?} panicked: {stderr}"
    );
    Run {
        code: output.status.code(),
        stdout,
        stderr,
    }
}

fn fixture(name: &str) -> Vec<u8> {
    let path = data_dir().join(name);
    fs::read(&path).unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()))
}

/// The document of a fixture read tolerantly, as `roundtrip` reads it.
fn document(name: &str) -> sacm::model::Document {
    xmi::read(&fixture(name), &ReadOptions::default())
        .unwrap_or_else(|error| panic!("{ROW}: setup: {name} is not readable: {error}"))
        .into_document()
}

/// A fresh path for an `-o` file: in its own directory under
/// `CARGO_TARGET_TMPDIR`, which exists, and with no file there yet.
fn out_path(test: &str, file: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    fs::create_dir_all(&dir)
        .unwrap_or_else(|error| panic!("{ROW}: cannot create {}: {error}", dir.display()));
    let path = dir.join(file);
    if path.exists() {
        fs::remove_file(&path)
            .unwrap_or_else(|error| panic!("{ROW}: cannot remove {}: {error}", path.display()));
    }
    path
}

fn as_arg(path: &Path) -> &str {
    path.to_str()
        .unwrap_or_else(|| panic!("{ROW}: setup: {} is not UTF-8", path.display()))
}

#[test]
fn lib_cli_003_a_clean_fixture_exits_0_with_only_the_round_trip_ok_line() {
    // Setup: the fixture holds packages, Claims, names and every
    // assertionDeclaration literal.
    let doc = document("clean.sacm.xmi");
    let declarations: Vec<AssertionDeclaration> = doc
        .elements()
        .filter_map(sacm::model::SacmElement::assertion_declaration)
        .collect();
    for literal in [
        "asserted",
        "assumed",
        "axiomatic",
        "defeated",
        "needsSupport",
        "asCited",
    ] {
        assert!(
            declarations
                .iter()
                .any(|declaration| declaration.as_str() == literal),
            "{ROW}: setup: the clean fixture has no {literal:?} Claim"
        );
    }
    let run = roundtrip(&["clean.sacm.xmi"]);
    assert_eq!(
        run.code,
        Some(0),
        "{ROW}: exit code; stderr: {}",
        run.stderr
    );
    assert_eq!(
        run.stdout, "clean.sacm.xmi: round trip ok\n",
        "{ROW}: stdout"
    );
    assert_eq!(run.stderr, "", "{ROW}: stderr is not empty");
}

#[test]
fn lib_cli_003_o_writes_the_strict_output_byte_for_byte_and_identically_across_runs() {
    let expected = xmi::write(&document("clean.sacm.xmi"), &WriteOptions::default())
        .unwrap_or_else(|error| panic!("{ROW}: setup: the clean fixture does not write: {error}"));
    let mut outputs = Vec::new();
    for file in ["first.sacm.xmi", "second.sacm.xmi"] {
        let out = out_path("lib-cli-003-output", file);
        let run = roundtrip(&["clean.sacm.xmi", "-o", as_arg(&out)]);
        assert_eq!(
            run.code,
            Some(0),
            "{ROW}: exit code with -o; stderr: {}",
            run.stderr
        );
        let written = fs::read(&out)
            .unwrap_or_else(|error| panic!("{ROW}: -o wrote no {}: {error}", out.display()));
        assert_eq!(
            String::from_utf8_lossy(&written),
            expected,
            "{ROW}: the -o file is not the strict output of xmi::write"
        );
        assert_eq!(
            written,
            expected.as_bytes(),
            "{ROW}: the -o file differs in its bytes"
        );
        outputs.push(written);
    }
    assert_eq!(
        outputs[0], outputs[1],
        "{ROW}: two runs wrote different bytes"
    );
}

#[test]
fn lib_cli_003_a_refused_strict_save_exits_1_with_each_error_and_writes_no_output() {
    let error = match xmi::write(
        &document("nameless-claim.sacm.xmi"),
        &WriteOptions::default(),
    ) {
        Ok(text) => panic!("{ROW}: setup: the nameless Claim was written strictly: {text}"),
        Err(error) => error,
    };
    assert!(
        matches!(error, WriteError::Invalid { .. }),
        "{ROW}: setup: the strict write is not Invalid: {error:?}"
    );
    let expected: Vec<String> = error
        .diagnostics()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        expected
            .iter()
            .any(|line| line.starts_with("error SACM-MULT-001 ")),
        "{ROW}: setup: the refusal holds no SACM-MULT-001: {expected:?}"
    );
    let out = out_path("lib-cli-003-refused", "out.sacm.xmi");
    let run = roundtrip(&["nameless-claim.sacm.xmi", "-o", as_arg(&out)]);
    assert_eq!(
        run.code,
        Some(1),
        "{ROW}: exit code; stderr: {}",
        run.stderr
    );
    let lines: Vec<&str> = run.stdout.lines().collect();
    let Some((summary, errors)) = lines.split_last() else {
        panic!("{ROW}: stdout is empty");
    };
    assert_eq!(
        errors, expected,
        "{ROW}: the Error lines are not the refusal's, in order"
    );
    assert!(
        summary.starts_with("nameless-claim.sacm.xmi: round trip failed: "),
        "{ROW}: the last line is not the failure: {summary:?}"
    );
    assert!(
        !out.exists(),
        "{ROW}: -o wrote {} for a refused save",
        out.display()
    );
}

#[test]
fn lib_cli_003_an_xsi_type_input_round_trips_tolerantly_and_its_output_uses_xmi_type() {
    let out = out_path("lib-cli-003-xsi-type", "out.sacm.xmi");
    let run = roundtrip(&["xsi-type.sacm.xmi", "-o", as_arg(&out)]);
    assert_eq!(
        run.code,
        Some(0),
        "{ROW}: exit code; stderr: {}",
        run.stderr
    );
    assert_eq!(
        run.stdout, "xsi-type.sacm.xmi: round trip ok\n",
        "{ROW}: stdout"
    );
    let written = fs::read_to_string(&out)
        .unwrap_or_else(|error| panic!("{ROW}: -o wrote no {}: {error}", out.display()));
    assert!(
        written.contains("xmi:type=\"sacm:Claim\"") && !written.contains("xsi:type"),
        "{ROW}: the output does not type the Claim with xmi:type alone:\n{written}"
    );
}

#[test]
fn lib_cli_003_an_unreadable_input_or_an_unwritable_output_exits_2() {
    let error: ReadError = match xmi::read(
        &fixture("malformed.sacm.xmi"),
        &ReadOptions::default().with_mode(Mode::Tolerant),
    ) {
        Ok(_) => panic!("{ROW}: setup: the malformed fixture is readable"),
        Err(error) => error,
    };
    let run = roundtrip(&["malformed.sacm.xmi"]);
    assert_eq!(
        run.code,
        Some(2),
        "{ROW}: malformed: exit code; stderr: {}",
        run.stderr
    );
    assert_eq!(run.stdout, "", "{ROW}: malformed: stdout is not empty");
    assert_eq!(
        run.stderr.lines().collect::<Vec<_>>(),
        [format!("sacm: malformed.sacm.xmi: SACM-XML-001 {error}")],
        "{ROW}: malformed: stderr is not validate's refusal line"
    );

    let dir = out_path("lib-cli-003-unwritable", "unused");
    let out = dir.with_file_name("no-such-directory").join("out.sacm.xmi");
    assert!(
        !out.parent().is_some_and(Path::exists),
        "{ROW}: setup: {} exists",
        out.display()
    );
    let run = roundtrip(&["clean.sacm.xmi", "-o", as_arg(&out)]);
    assert_eq!(
        run.code,
        Some(2),
        "{ROW}: unwritable -o: exit code; stderr: {}",
        run.stderr
    );
    assert!(
        run.stderr.contains(as_arg(&out)),
        "{ROW}: unwritable -o: stderr does not name {}: {}",
        out.display(),
        run.stderr
    );
    assert!(!out.exists(), "{ROW}: unwritable -o: the file exists");
}

#[test]
fn lib_cli_003_invalid_arguments_exit_2_with_usage() {
    let cases = [
        &[][..],
        &["--no-such-flag", "clean.sacm.xmi"],
        &["clean.sacm.xmi", "-o"],
    ];
    for args in cases {
        let run = roundtrip(args);
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
                .any(|line| line.starts_with("Usage: sacm") && line.contains(" roundtrip")),
            "{ROW}: {args:?}: stderr has no roundtrip usage: {}",
            run.stderr
        );
    }
}
