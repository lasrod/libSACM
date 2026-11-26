//! End-to-end tests for `sacm convert` (LIB-CLI-004; ADR 0023 amendment 2;
//! ADR 0044).
//!
//! `sacm convert <IN> -o <OUT>` reads `<IN>` tolerantly, writes it strictly
//! to memory, reads that back strictly and compares, and writes `<OUT>` only
//! when nothing is lost. stdout lists every read Warning and Error, then
//! `<IN>: converted to <OUT>` or `<IN>: not converted: …`.
//!
//! The binary runs in `tests/data`, with the fixture's file name as its
//! argument, so that the input name it prints is the same on every machine.
//! `-o` files go to `CARGO_TARGET_TMPDIR`, one directory per test, and are
//! printed as given. The expected lines and output bytes are taken from the
//! library: `xmi::read` tolerant, then `xmi::write` with the default, strict,
//! options.
//!
//! Not checked here: the reload and difference wording. A correct library
//! gives neither, so the binary cannot be driven into them; a unit test in
//! the CLI crate covers them, as for LIB-CLI-003.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sacm::diagnostic::Severity;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteError, WriteOptions};

const ROW: &str = "LIB-CLI-004";

/// G2 of LIB-COMPAT-002: the strict output of `emf-dialect.sacm.xmi`.
const G2: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sacm/tests/data/compat/golden/emf-dialect.golden.sacm.xmi"
);

/// The doc line of `convert`, which `sacm --help` lists.
const ABOUT: &str =
    "Convert a SACM XMI file to strict SACM 2.3, refusing when that would lose content";

/// The long help of `convert`.
const LONG_HELP: &str = "Reads the file tolerantly, writes it in strict mode, reads that back \
                         strictly and compares, and writes OUT only when nothing is lost. Lists \
                         every read Warning and Error. Exits 0 when converted, 1 when a strict \
                         save refuses the document or its output would not read back unchanged, \
                         and 2 when the file cannot be read or the output cannot be written.";

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

/// Runs `sacm` with `args` in `tests/data`.
fn sacm(args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_sacm"))
        .args(args)
        .current_dir(data_dir())
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

/// Runs `sacm convert` with `args` in `tests/data`.
fn convert(args: &[&str]) -> Run {
    let mut all = vec!["convert"];
    all.extend_from_slice(args);
    sacm(&all)
}

fn fixture(name: &str) -> Vec<u8> {
    let path = data_dir().join(name);
    fs::read(&path).unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()))
}

/// The tolerant read of a fixture, as `convert` reads it.
fn read(name: &str) -> xmi::ReadOutput {
    xmi::read(
        &fixture(name),
        &ReadOptions::default().with_mode(Mode::Tolerant),
    )
    .unwrap_or_else(|error| panic!("{ROW}: setup: {name} is not readable tolerantly: {error}"))
}

/// The `Display` lines of the read diagnostics of `output` that are Warnings
/// or Errors, in `Diagnostic` order: what `convert` lists first.
fn listed(output: &xmi::ReadOutput) -> Vec<String> {
    output
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity() != Severity::Info)
        .map(ToString::to_string)
        .collect()
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

/// stdout as its lines, each with its line end.
fn lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

#[test]
fn lib_cli_004_a_clean_file_converts_to_its_strict_output() {
    let output = read("clean.sacm.xmi");
    assert!(
        output.diagnostics().is_empty(),
        "{ROW}: setup: the clean fixture reads with {:?}",
        output.diagnostics()
    );
    let expected = xmi::write(output.document(), &WriteOptions::default())
        .unwrap_or_else(|error| panic!("{ROW}: setup: the clean fixture does not write: {error}"));
    let mut outputs = Vec::new();
    for file in ["first.sacm.xmi", "second.sacm.xmi"] {
        let out = out_path("lib-cli-004-clean", file);
        let run = convert(&["clean.sacm.xmi", "-o", as_arg(&out)]);
        assert_eq!(
            run.code,
            Some(0),
            "{ROW}: clean: exit code; stdout: {}; stderr: {}",
            run.stdout,
            run.stderr
        );
        assert_eq!(
            run.stdout,
            format!("clean.sacm.xmi: converted to {}\n", as_arg(&out)),
            "{ROW}: clean: stdout"
        );
        assert_eq!(run.stderr, "", "{ROW}: clean: stderr is not empty");
        let written = fs::read(&out)
            .unwrap_or_else(|error| panic!("{ROW}: clean: -o wrote no {}: {error}", out.display()));
        assert_eq!(
            written,
            expected.as_bytes(),
            "{ROW}: clean: the -o file is not the strict output of xmi::write"
        );
        outputs.push(written);
    }
    assert_eq!(
        outputs[0], outputs[1],
        "{ROW}: clean: two runs wrote different bytes"
    );
}

#[test]
fn lib_cli_004_an_emf_file_converts_and_lists_each_read_warning() {
    let name = "emf-dialect.sacm.xmi";
    let out = out_path("lib-cli-004-emf", "brake.sacm.xmi");
    let run = convert(&[name, "-o", as_arg(&out)]);
    assert_eq!(
        run.code,
        Some(0),
        "{ROW}: {name}: exit code; stdout: {}; stderr: {}",
        run.stdout,
        run.stderr
    );
    assert_eq!(run.stderr, "", "{ROW}: {name}: stderr is not empty");
    let output = read(name);
    assert!(
        output
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.severity() == Severity::Info),
        "{ROW}: setup: {name} reads with no Info, so leaving Info out is not exercised"
    );
    let mut expected = listed(&output);
    assert!(
        expected.len() >= 2,
        "{ROW}: setup: {name} reads with fewer than two Warnings: {expected:?}"
    );
    expected.push(format!("{name}: converted to {}", as_arg(&out)));
    assert_eq!(
        lines(&run.stdout),
        expected,
        "{ROW}: {name}: stdout is not each read Warning, in order, then the converted line"
    );
    assert!(
        !run.stdout.lines().any(|line| line.starts_with("info ")),
        "{ROW}: {name}: stdout lists an Info: {}",
        run.stdout
    );
    let written = fs::read(&out)
        .unwrap_or_else(|error| panic!("{ROW}: {name}: -o wrote no {}: {error}", out.display()));
    let golden =
        fs::read(G2).unwrap_or_else(|error| panic!("{ROW}: setup: cannot read {G2}: {error}"));
    assert_eq!(
        String::from_utf8_lossy(&written),
        String::from_utf8_lossy(&golden),
        "{ROW}: {name}: the -o file is not G2"
    );
    assert_eq!(
        written, golden,
        "{ROW}: {name}: the -o file differs from G2 in its bytes"
    );
}

#[test]
fn lib_cli_004_content_strict_output_would_lose_is_refused_with_its_reasons() {
    let name = "xmi-attribute.sacm.xmi";
    let output = read(name);
    let warnings = listed(&output);
    assert!(
        warnings.len() == 1 && warnings[0].starts_with("warning SACM-XMI-003 "),
        "{ROW}: setup: {name} does not read with one SACM-XMI-003 Warning: {warnings:?}"
    );
    let error: WriteError = match xmi::write(output.document(), &WriteOptions::default()) {
        Ok(text) => panic!("{ROW}: setup: {name} was written strictly: {text}"),
        Err(error) => error,
    };
    let summary = format!("{name}: not converted: {error}");
    assert_eq!(
        summary,
        "xmi-attribute.sacm.xmi: not converted: the document cannot be written in strict mode: \
         the document holds preserved content outside its elements",
        "{ROW}: setup: the refusal's line"
    );
    let mut expected = warnings;
    expected.push(summary);

    // An absent OUT is not created.
    let absent = out_path("lib-cli-004-preserved", "absent.sacm.xmi");
    let run = convert(&[name, "-o", as_arg(&absent)]);
    assert_eq!(
        run.code,
        Some(1),
        "{ROW}: {name}: exit code; stdout: {}; stderr: {}",
        run.stdout,
        run.stderr
    );
    assert_eq!(
        lines(&run.stdout),
        expected,
        "{ROW}: {name}: stdout is not the read Warning, then the refusal"
    );
    assert_eq!(run.stderr, "", "{ROW}: {name}: stderr is not empty");
    assert!(
        !absent.exists(),
        "{ROW}: {name}: -o created {} for a refusal",
        absent.display()
    );

    // An existing OUT keeps its bytes.
    let existing = out_path("lib-cli-004-preserved", "existing.sacm.xmi");
    let before = b"an earlier output\n";
    fs::write(&existing, before).unwrap_or_else(|error| {
        panic!("{ROW}: setup: cannot write {}: {error}", existing.display())
    });
    let run = convert(&[name, "-o", as_arg(&existing)]);
    assert_eq!(
        run.code,
        Some(1),
        "{ROW}: {name}, existing -o: exit code; stderr: {}",
        run.stderr
    );
    let after = fs::read(&existing)
        .unwrap_or_else(|error| panic!("{ROW}: {name}: {} is gone: {error}", existing.display()));
    assert_eq!(
        after, before,
        "{ROW}: {name}: a refusal changed the existing -o file"
    );
}

#[test]
fn lib_cli_004_a_dangling_reference_is_refused_with_each_schema_error() {
    let name = "dangling-reference.sacm.xmi";
    let output = read(name);
    let error = match xmi::write(output.document(), &WriteOptions::default()) {
        Ok(text) => panic!("{ROW}: setup: {name} was written strictly: {text}"),
        Err(error) => error,
    };
    assert!(
        matches!(error, WriteError::Invalid { .. }),
        "{ROW}: setup: the strict write is not Invalid: {error:?}"
    );
    let errors: Vec<String> = error
        .diagnostics()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        errors.len() == 1 && errors[0].starts_with("error SACM-REF-001 "),
        "{ROW}: setup: the refusal is not one SACM-REF-001: {errors:?}"
    );
    let summary = format!("{name}: not converted: {error}");
    assert_eq!(
        summary,
        "dangling-reference.sacm.xmi: not converted: the document cannot be written in strict \
         mode: 1 schema error, SACM-REF-001 on \"c1\"",
        "{ROW}: setup: the refusal's line"
    );
    let mut expected = listed(&output);
    expected.extend(errors);
    expected.push(summary);

    let out = out_path("lib-cli-004-dangling", "out.sacm.xmi");
    let run = convert(&[name, "-o", as_arg(&out)]);
    assert_eq!(
        run.code,
        Some(1),
        "{ROW}: {name}: exit code; stdout: {}; stderr: {}",
        run.stdout,
        run.stderr
    );
    assert_eq!(
        lines(&run.stdout),
        expected,
        "{ROW}: {name}: stdout is not the read Warnings, each schema Error, then the refusal"
    );
    assert!(
        !out.exists(),
        "{ROW}: {name}: -o wrote {} for a refusal",
        out.display()
    );
}

#[test]
fn lib_cli_004_an_unreadable_input_or_an_unwritable_output_exits_2() {
    // The same stderr as `roundtrip`'s for the same input.
    let malformed: ReadError =
        match xmi::read(&fixture("malformed.sacm.xmi"), &ReadOptions::default()) {
            Ok(_) => panic!("{ROW}: setup: the malformed fixture is readable"),
            Err(error) => error,
        };
    for name in ["no-such-file.sacm.xmi", "malformed.sacm.xmi"] {
        let out = out_path("lib-cli-004-unreadable", "out.sacm.xmi");
        let run = convert(&[name, "-o", as_arg(&out)]);
        let roundtrip = sacm(&["roundtrip", name]);
        assert_eq!(
            roundtrip.code,
            Some(2),
            "{ROW}: setup: roundtrip {name} does not exit 2; stderr: {}",
            roundtrip.stderr
        );
        assert_eq!(
            run.code,
            Some(2),
            "{ROW}: {name}: exit code; stdout: {}; stderr: {}",
            run.stdout,
            run.stderr
        );
        assert_eq!(run.stdout, "", "{ROW}: {name}: stdout is not empty");
        assert_eq!(
            run.stderr, roundtrip.stderr,
            "{ROW}: {name}: stderr is not roundtrip's"
        );
        assert!(!out.exists(), "{ROW}: {name}: -o wrote {}", out.display());
        if name == "malformed.sacm.xmi" {
            assert_eq!(
                lines(&run.stderr),
                [format!(
                    "sacm: malformed.sacm.xmi: SACM-XML-001 {malformed}"
                )],
                "{ROW}: malformed: stderr is not the refusal line"
            );
        }
    }

    let dir = out_path("lib-cli-004-unwritable", "unused");
    let out = dir.with_file_name("no-such-directory").join("out.sacm.xmi");
    assert!(
        !out.parent().is_some_and(Path::exists),
        "{ROW}: setup: {} exists",
        out.display()
    );
    let run = convert(&["clean.sacm.xmi", "-o", as_arg(&out)]);
    assert_eq!(
        run.code,
        Some(2),
        "{ROW}: unwritable -o: exit code; stdout: {}; stderr: {}",
        run.stdout,
        run.stderr
    );
    assert!(
        run.stderr.starts_with("sacm: ")
            && run
                .stderr
                .contains(&format!("{}: cannot write the output: ", as_arg(&out))),
        "{ROW}: unwritable -o: stderr does not name {}: {}",
        out.display(),
        run.stderr
    );
    assert!(
        !run.stdout.contains("converted to"),
        "{ROW}: unwritable -o: stdout says converted: {}",
        run.stdout
    );
    assert!(!out.exists(), "{ROW}: unwritable -o: the file exists");
}

#[test]
fn lib_cli_004_invalid_arguments_exit_2_with_usage() {
    let out = out_path("lib-cli-004-arguments", "out.sacm.xmi");
    let out = as_arg(&out);
    let cases: [&[&str]; 5] = [
        &[],
        &["clean.sacm.xmi"],
        &["clean.sacm.xmi", "-o"],
        &["clean.sacm.xmi", "--strict", "-o", out],
        &["clean.sacm.xmi", "--tolerant", "-o", out],
    ];
    for args in cases {
        let run = convert(args);
        assert_eq!(
            run.code,
            Some(2),
            "{ROW}: convert {args:?}: exit code; stderr: {}",
            run.stderr
        );
        assert_eq!(
            run.stdout, "",
            "{ROW}: convert {args:?}: stdout is not empty"
        );
        assert!(
            run.stderr.contains("Usage: sacm convert"),
            "{ROW}: convert {args:?}: stderr has no convert usage: {}",
            run.stderr
        );
        assert!(
            !Path::new(out).exists(),
            "{ROW}: convert {args:?}: -o was written"
        );
    }
}

#[test]
fn lib_cli_004_the_help_lists_convert() {
    let run = sacm(&["--help"]);
    assert_eq!(
        run.code,
        Some(0),
        "{ROW}: sacm --help: exit code; stderr: {}",
        run.stderr
    );
    let help = lines(&run.stdout);
    let Some(roundtrip) = help
        .iter()
        .position(|line| line.trim_start().starts_with("roundtrip "))
    else {
        panic!("{ROW}: sacm --help lists no roundtrip: {}", run.stdout);
    };
    let next = help.get(roundtrip + 1).copied().unwrap_or("");
    let listed = next.trim_start().strip_prefix("convert").map(str::trim);
    assert_eq!(
        listed,
        Some(ABOUT),
        "{ROW}: sacm --help does not list convert with its doc line after roundtrip: {}",
        run.stdout
    );

    let run = sacm(&["convert", "--help"]);
    assert_eq!(
        run.code,
        Some(0),
        "{ROW}: sacm convert --help: exit code; stderr: {}",
        run.stderr
    );
    assert_eq!(
        run.stdout.lines().next(),
        Some(format!("{ABOUT}.").as_str()),
        "{ROW}: sacm convert --help does not open with the doc line: {}",
        run.stdout
    );
    assert!(
        run.stdout.contains(LONG_HELP),
        "{ROW}: sacm convert --help does not show the long help: {}",
        run.stdout
    );
    assert!(
        run.stdout.contains("Usage: sacm convert"),
        "{ROW}: sacm convert --help shows no usage: {}",
        run.stdout
    );
}
