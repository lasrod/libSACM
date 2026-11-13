//! `sacm` — validate and convert OMG SACM 2.3 assurance cases.
//!
//! The command-line front end to the [`sacm`] library (ADR 0023). Subcommands
//! are added as the library gains the capability behind them; see the
//! functionality matrix (`docs/matrix/functionality-matrix.md`) for what is
//! planned.
//!
//! Exit codes: 0 when the check passed; 1 when it ran and failed (for
//! `validate`, an Error diagnostic; for `roundtrip`, a refused strict save,
//! output that does not read back, or a difference); 2 when
//! the tool could not do the job (a usage error, an I/O error, or input that
//! `xmi::read` refuses). stdout carries the result; stderr carries only what
//! stopped the command.

mod json;

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use sacm::compare::{self, Difference};
use sacm::diagnostic::{self, Diagnostic, Severity};
use sacm::model::Document;
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteOptions};

use json::Refusal;

/// The check passed.
const PASSED: u8 = 0;
/// The check ran and failed.
const FAILED: u8 = 1;
/// The tool could not do the job.
const NOT_DONE: u8 = 2;

/// Validate and convert OMG SACM 2.3 assurance cases.
#[derive(Debug, Parser)]
#[command(name = "sacm", bin_name = "sacm", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the tool version and the SACM revision it implements.
    Version,
    /// Read a SACM XMI file, validate it and report every diagnostic.
    ///
    /// Exits 0 when no diagnostic is an Error, 1 when one is, and 2 when the
    /// file cannot be read.
    Validate(ValidateArgs),
    /// Check that a SACM XMI file survives a strict save and reload unchanged.
    ///
    /// Reads the file, writes it in strict mode to memory, reads that back in
    /// strict mode and compares the two documents. Exits 0 when the round
    /// trip is faithful, 1 when it is not, and 2 when the file cannot be read
    /// or the output cannot be written.
    Roundtrip(RoundtripArgs),
}

#[derive(Debug, Args)]
struct ValidateArgs {
    /// The SACM XMI file to check.
    file: PathBuf,
    /// Read in strict mode, where a recognised variation such as `xsi:type`
    /// is an Error.
    #[arg(long)]
    strict: bool,
    /// How to report.
    #[arg(long, value_enum, default_value_t = Format::Human)]
    format: Format,
}

#[derive(Debug, Args)]
struct RoundtripArgs {
    /// The SACM XMI file to check.
    file: PathBuf,
    /// Read the input in strict mode (read diagnostics are not checked; see
    /// `sacm validate`).
    #[arg(long)]
    strict: bool,
    /// Write the strict output here, only when the round trip is faithful.
    #[arg(short = 'o', value_name = "OUT")]
    output: Option<PathBuf>,
}

/// The report format of `validate`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Format {
    /// One line per diagnostic, then a summary.
    Human,
    /// One JSON object on one line (see the sacm-cli README).
    Json,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return ExitCode::from(usage_error(&error)),
    };
    let result = run(&cli, &mut io::stdout().lock(), &mut io::stderr().lock());
    ExitCode::from(exit_code(result))
}

/// Reports an argument error and returns its exit code. Help and the version
/// go to stdout and pass. Any other error goes to stderr with the usage of
/// the subcommand it concerns, which clap leaves out of some errors (such as
/// an invalid `--format`), and exits 2.
fn usage_error(error: &clap::Error) -> u8 {
    if !error.use_stderr() {
        return match error.print() {
            Ok(()) => PASSED,
            Err(_) => NOT_DONE,
        };
    }
    let mut text = error.render().to_string();
    if !text.contains("Usage:") {
        let mut command = <Cli as clap::CommandFactory>::command();
        command.build();
        let usage = std::env::args_os()
            .nth(1)
            .and_then(|name| {
                let name = name.to_str()?.to_owned();
                command
                    .find_subcommand_mut(name)
                    .map(clap::Command::render_usage)
            })
            .unwrap_or_else(|| command.render_usage());
        text = format!("{}\n\n{usage}\n", text.trim_end());
    }
    let _ = io::stderr().write_all(text.as_bytes());
    NOT_DONE
}

/// The exit code for the result of [`run`]. A closed stdout pipe
/// (`sacm version | head -0`) is not an error worth reporting, so the command
/// keeps the code it had; any other failure to write is an I/O error.
fn exit_code(result: Result<u8, Failure>) -> u8 {
    match result {
        Ok(code) => code,
        Err(Failure { code, error }) if error.kind() == io::ErrorKind::BrokenPipe => code,
        Err(Failure { error, .. }) => {
            let _ = writeln!(io::stderr(), "sacm: {error}");
            NOT_DONE
        }
    }
}

/// A failure to write the result, and the exit code the command had reached.
struct Failure {
    code: u8,
    error: io::Error,
}

/// Runs the command, writing its result to `out` and what stopped it to
/// `err`, and returns its exit code.
fn run(cli: &Cli, out: &mut impl Write, err: &mut impl Write) -> Result<u8, Failure> {
    let (code, written) = match &cli.command {
        Command::Version => (
            PASSED,
            writeln!(out, "sacm {} (SACM {})", sacm::VERSION, sacm::SACM_VERSION),
        ),
        Command::Validate(args) => run_validate(args, out, err),
        Command::Roundtrip(args) => run_roundtrip(args, out, err),
    };
    written
        .map(|()| code)
        .map_err(|error| Failure { code, error })
}

/// The name a report gives `file`: as it was given on the command line.
fn file_name(file: &std::path::Path) -> String {
    file.to_string_lossy().into_owned()
}

/// `mode` as the report spells it.
fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Strict => "strict",
        _ => "tolerant",
    }
}

/// The refusal of a file that cannot be read from disk.
fn io_refusal(error: &io::Error) -> Refusal {
    Refusal {
        code: None,
        location: None,
        locations: Vec::new(),
        message: format!("cannot read the file: {error}"),
    }
}

/// The refusal of a file that `xmi::read` refuses.
fn read_refusal(error: &ReadError) -> Refusal {
    Refusal {
        code: error.code().map(sacm::diagnostic::Code::as_str),
        location: error.location(),
        locations: error.locations().to_vec(),
        message: error.to_string(),
    }
}

/// Reads `file` in `mode`, or the refusal of it.
fn read_file(file: &std::path::Path, mode: Mode) -> Result<xmi::ReadOutput, Refusal> {
    let bytes = fs::read(file).map_err(|error| io_refusal(&error))?;
    xmi::read(&bytes, &ReadOptions::default().with_mode(mode)).map_err(|error| read_refusal(&error))
}

/// Writes the stderr line of a file that cannot be read:
/// `sacm: <file>: <code|error> <message>`.
fn report_unreadable(name: &str, refusal: &Refusal, err: &mut impl Write) -> io::Result<()> {
    let code = refusal.code.unwrap_or("error");
    writeln!(err, "sacm: {name}: {code} {}", refusal.message)
}

/// `sacm validate` (LIB-CLI-002).
fn run_validate(
    args: &ValidateArgs,
    out: &mut impl Write,
    err: &mut impl Write,
) -> (u8, io::Result<()>) {
    let name = file_name(&args.file);
    let mode = if args.strict {
        Mode::Strict
    } else {
        Mode::Tolerant
    };
    let output = match read_file(&args.file, mode) {
        Ok(output) => output,
        Err(refusal) => {
            let written = match args.format {
                Format::Human => report_unreadable(&name, &refusal, err),
                Format::Json => out.write_all(
                    json::report(&name, mode_name(mode), "unreadable", Some(&refusal), &[])
                        .as_bytes(),
                ),
            };
            return (NOT_DONE, written);
        }
    };
    let mut found = output.diagnostics().to_vec();
    found.extend(validation::validate(output.document()));
    found.sort();
    let valid = !diagnostic::has_errors(&found);
    let code = if valid { PASSED } else { FAILED };
    let result = if valid { "valid" } else { "invalid" };
    let written = match args.format {
        Format::Human => write_human(&name, result, &found, out),
        Format::Json => {
            out.write_all(json::report(&name, mode_name(mode), result, None, &found).as_bytes())
        }
    };
    (code, written)
}

/// Writes each diagnostic's line, then
/// `<file>: valid|invalid (E errors, W warnings, I info)`.
fn write_human(
    name: &str,
    result: &str,
    found: &[Diagnostic],
    out: &mut impl Write,
) -> io::Result<()> {
    for diagnostic in found {
        writeln!(out, "{diagnostic}")?;
    }
    let count = |severity: Severity| {
        found
            .iter()
            .filter(|diagnostic| diagnostic.severity() == severity)
            .count()
    };
    let errors = count(Severity::Error);
    let warnings = count(Severity::Warning);
    writeln!(
        out,
        "{name}: {result} ({errors} {}, {warnings} {}, {} info)",
        plural(errors, "error", "errors"),
        plural(warnings, "warning", "warnings"),
        count(Severity::Info)
    )
}

/// `one` for a count of 1, `many` otherwise.
fn plural(count: usize, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}

/// `sacm roundtrip` (LIB-CLI-003).
fn run_roundtrip(
    args: &RoundtripArgs,
    out: &mut impl Write,
    err: &mut impl Write,
) -> (u8, io::Result<()>) {
    let name = file_name(&args.file);
    let mode = if args.strict {
        Mode::Strict
    } else {
        Mode::Tolerant
    };
    let document = match read_file(&args.file, mode) {
        Ok(output) => output.into_document(),
        Err(refusal) => return (NOT_DONE, report_unreadable(&name, &refusal, err)),
    };

    // The strict save, to memory.
    let text = match xmi::write(&document, &WriteOptions::default()) {
        Ok(text) => text,
        Err(error) => {
            let written = error
                .diagnostics()
                .iter()
                .try_for_each(|diagnostic| writeln!(out, "{diagnostic}"))
                .and_then(|()| writeln!(out, "{name}: round trip failed: {error}"));
            return (FAILED, written);
        }
    };

    // The strict reload.
    let strict = ReadOptions::default().with_mode(Mode::Strict);
    let reloaded = xmi::read(text.as_bytes(), &strict);
    let back = match check_reload(&name, &reloaded, out) {
        Ok(back) => back,
        Err(failed) => return failed,
    };

    let differences = compare::semantic_compare(&document, back);
    if !differences.is_empty() {
        return (FAILED, write_differences(&name, &differences, out));
    }

    if let Some(path) = &args.output {
        if let Err(error) = fs::write(path, &text) {
            let written = writeln!(
                err,
                "sacm: {}: cannot write the output: {error}",
                path.display()
            );
            return (NOT_DONE, written);
        }
    }
    (PASSED, writeln!(out, "{name}: round trip ok"))
}

/// Checks the strict reload of the output: the document it read back, when
/// it read with no Error diagnostic. Otherwise it writes each Error's line,
/// then `<file>: round trip failed: the strict output reads back with N
/// errors`, or, when it does not read at all, `<file>: round trip failed:
/// the strict output does not read back: <refusal>`, and returns the exit
/// code with the result of writing.
fn check_reload<'a>(
    name: &str,
    reloaded: &'a Result<xmi::ReadOutput, ReadError>,
    out: &mut impl Write,
) -> Result<&'a Document, (u8, io::Result<()>)> {
    let back = match reloaded {
        Ok(back) => back,
        Err(error) => {
            let written = writeln!(
                out,
                "{name}: round trip failed: the strict output does not read back: {error}"
            );
            return Err((FAILED, written));
        }
    };
    let errors: Vec<&Diagnostic> = back
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity() == Severity::Error)
        .collect();
    if errors.is_empty() {
        return Ok(back.document());
    }
    let written = errors
        .iter()
        .try_for_each(|diagnostic| writeln!(out, "{diagnostic}"))
        .and_then(|()| {
            writeln!(
                out,
                "{name}: round trip failed: the strict output reads back with {} {}",
                errors.len(),
                plural(errors.len(), "error", "errors")
            )
        });
    Err((FAILED, written))
}

/// Writes each difference's line, then
/// `<file>: round trip failed: N differences`.
fn write_differences(
    name: &str,
    differences: &[Difference],
    out: &mut impl Write,
) -> io::Result<()> {
    for difference in differences {
        writeln!(out, "{difference}")?;
    }
    let count = differences.len();
    writeln!(
        out,
        "{name}: round trip failed: {count} {}",
        plural(count, "difference", "differences")
    )
}

#[cfg(test)]
mod tests {
    use sacm::model::{ElementId, ElementKind, LangString};

    use super::*;

    // The difference report of `roundtrip` (LIB-CLI-003). A correct library
    // gives no difference, so the binary cannot be driven into it.
    #[test]
    fn the_difference_report_gives_each_difference_then_the_count() {
        let differences = vec![
            Difference::RightOnly {
                id: ElementId::from("c1"),
                kind: ElementKind::Claim,
            },
            Difference::Name {
                id: ElementId::from("p"),
                left: None,
                right: Some(LangString::new("P")),
            },
        ];
        let mut out = Vec::new();
        assert!(write_differences("case.sacm.xmi", &differences, &mut out).is_ok());
        let text = String::from_utf8(out).unwrap_or_default();
        let mut expected: Vec<String> = differences.iter().map(ToString::to_string).collect();
        expected.push("case.sacm.xmi: round trip failed: 2 differences".to_owned());
        assert_eq!(text.lines().collect::<Vec<_>>(), expected);

        let mut out = Vec::new();
        assert!(write_differences("case.sacm.xmi", &differences[..1], &mut out).is_ok());
        let text = String::from_utf8(out).unwrap_or_default();
        assert_eq!(
            text.lines().last(),
            Some("case.sacm.xmi: round trip failed: 1 difference")
        );
    }

    /// An `ArgumentPackage` with `claims` Claims typed by `xsi:type`, each a
    /// SACM-XMI-010 diagnostic, and an `xmi:version`, a SACM-XMI-011 Info
    /// diagnostic, when `versioned`.
    fn typed(claims: usize, versioned: bool) -> String {
        use std::fmt::Write as _;

        let version = if versioned {
            r#" xmi:version="2.0""#
        } else {
            ""
        };
        let mut text = format!(
            r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmi:id="arguments"{version}>"#
        );
        for claim in 0..claims {
            let _ = write!(
                text,
                r#"<argumentElement xsi:type="sacm:Claim" xmi:id="c{claim}"/>"#
            );
        }
        text.push_str("</sacm:ArgumentPackage>");
        text
    }

    /// The diagnostics of reading `input` in `mode`.
    fn read_diagnostics(input: &str, mode: Mode) -> Vec<Diagnostic> {
        xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
            .map(|output| output.diagnostics().to_vec())
            .unwrap_or_default()
    }

    /// The lines `write_human` writes for `found`.
    fn human(found: &[Diagnostic]) -> Vec<String> {
        let mut out = Vec::new();
        assert!(write_human("f", "invalid", found, &mut out).is_ok());
        String::from_utf8(out)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn the_human_summary_counts_each_severity() {
        let mut out = Vec::new();
        assert!(write_human("f", "valid", &[], &mut out).is_ok());
        assert_eq!(
            String::from_utf8(out).unwrap_or_default(),
            "f: valid (0 errors, 0 warnings, 0 info)\n"
        );

        let mut found = read_diagnostics(&typed(3, false), Mode::Strict);
        found.extend(read_diagnostics(&typed(2, true), Mode::Tolerant));
        found.sort();
        let severities = |severity: Severity| {
            found
                .iter()
                .filter(|diagnostic| diagnostic.severity() == severity)
                .count()
        };
        assert_eq!(
            (
                severities(Severity::Error),
                severities(Severity::Warning),
                severities(Severity::Info)
            ),
            (3, 2, 1)
        );
        let mut expected: Vec<String> = found.iter().map(ToString::to_string).collect();
        expected.push("f: invalid (3 errors, 2 warnings, 1 info)".to_owned());
        assert_eq!(human(&found), expected);

        let mut found = read_diagnostics(&typed(1, false), Mode::Strict);
        found.extend(read_diagnostics(&typed(1, false), Mode::Tolerant));
        assert_eq!(
            human(&found).last().map(String::as_str),
            Some("f: invalid (1 error, 1 warning, 0 info)")
        );
    }

    /// The lines `check_reload` writes for `reloaded`, and its exit code
    /// when it fails.
    fn reload(reloaded: &Result<xmi::ReadOutput, ReadError>) -> (Option<u8>, Vec<String>) {
        let mut out = Vec::new();
        let code = match check_reload("f", reloaded, &mut out) {
            Ok(_) => None,
            Err((code, written)) => {
                assert!(written.is_ok());
                Some(code)
            }
        };
        let lines = String::from_utf8(out)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect();
        (code, lines)
    }

    // The reload check of `roundtrip` (LIB-CLI-003). A correct writer gives
    // output that reads back cleanly, so the binary cannot be driven into it.
    #[test]
    fn the_reload_check_passes_warnings_and_fails_errors() {
        let input = typed(1, true);
        let tolerant = ReadOptions::default().with_mode(Mode::Tolerant);
        let warned = xmi::read(input.as_bytes(), &tolerant);
        assert!(
            warned
                .as_ref()
                .is_ok_and(|output| !output.diagnostics().is_empty())
        );
        assert_eq!(reload(&warned), (None, Vec::new()));

        let strict = ReadOptions::default().with_mode(Mode::Strict);
        let failed = xmi::read(input.as_bytes(), &strict);
        let errors: Vec<String> = failed
            .as_ref()
            .map(|output| {
                output
                    .diagnostics()
                    .iter()
                    .filter(|diagnostic| diagnostic.severity() == Severity::Error)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(errors.len(), 1);
        let mut expected = errors;
        expected.push("f: round trip failed: the strict output reads back with 1 error".to_owned());
        assert_eq!(reload(&failed), (Some(FAILED), expected));

        let two = xmi::read(typed(2, false).as_bytes(), &strict);
        let (code, lines) = reload(&two);
        assert_eq!(code, Some(FAILED));
        assert_eq!(
            lines.last().map(String::as_str),
            Some("f: round trip failed: the strict output reads back with 2 errors")
        );

        let unreadable = xmi::read(b"<r></s>", &strict);
        let refusal = unreadable
            .as_ref()
            .err()
            .map(ToString::to_string)
            .unwrap_or_default();
        assert_ne!(refusal, "");
        assert_eq!(
            reload(&unreadable),
            (
                Some(FAILED),
                vec![format!(
                    "f: round trip failed: the strict output does not read back: {refusal}"
                )]
            )
        );
    }
}
