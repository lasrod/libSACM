//! The messages of 0.1.0 stay byte for byte (LIB-API-017 and LIB-API-018,
//! acceptance 3 of each; ADR 0030 amendment 3).
//!
//! Slice 0063 gives every refusal a template and arguments, and slice 0064
//! every diagnostic, and renders each message from them. Neither may change
//! a word of the English. `tests/data/golden/messages-0.1.0.txt` was recorded
//! from the 0.1.0 code before either slice changed it: for every XMI fixture
//! under `crates/sacm/tests/data` and `crates/sacm-cli/tests/data`, the
//! `Display` of what a strict read, a tolerant read and `validate` on each
//! document read give. This test renders the same lines from today's code
//! and compares them with the file, every line exactly.
//!
//! # Line format
//!
//! The file starts with `#` comment lines that restate this format. Every
//! other line is
//!
//! ```text
//! <fixture> <step>: <message>
//! ```
//!
//! - `<fixture>`: the fixture's path from the repository root, with `/`
//!   separators; fixtures are the files whose name ends in `.xmi`, sorted by
//!   that path.
//! - `<step>`, in this order for each fixture: `strict-read`,
//!   `strict-validate`, `tolerant-read`, `tolerant-validate`. A `*-validate`
//!   step is the `validate` of the document its read returned, and is left
//!   out when that read refused the input.
//! - `<message>`: for a read that refuses, `refusal ` and the `ReadError`'s
//!   `Display`; otherwise one line per diagnostic, in the order the step
//!   returns them, with the `Diagnostic`'s `Display`; and `(none)` for a step
//!   that gives no diagnostic, so that a step that starts to report shows.
//!
//! Every `Display` is one line: a diagnostic escapes line breaks, and a
//! refusal quotes the input's names with `{:?}`. The test refuses a message
//! with a line break rather than write an ambiguous line.
//!
//! The file is never re-recorded: a line that changes is a change of the
//! 0.1.0 English, which LIB-API-017 and LIB-API-018 forbid. When the lines
//! differ, the test writes what it rendered to
//! `<CARGO_TARGET_TMPDIR>/messages-0.1.0.actual.txt` for a diff, and fails.

use std::fs;
use std::path::{Path, PathBuf};

use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions};

const ROW: &str = "LIB-API-017";

/// The row of the diagnostics' lines (slice 0064).
const ROW_018: &str = "LIB-API-018";

/// The golden file, relative to this crate's directory.
const GOLDEN: &str = "tests/data/golden/messages-0.1.0.txt";

/// The fixture directories, relative to the repository root.
const FIXTURE_DIRS: [&str; 2] = ["crates/sacm/tests/data", "crates/sacm-cli/tests/data"];

/// The comment lines the file starts with: the format above.
const HEADER: &str = "\
# The messages of sacm 0.1.0 for every XMI fixture (LIB-API-017, acceptance 3).
# Recorded before slice 0063 changed the code; never re-recorded.
# Line: <fixture> <step>: <message>
# fixture: the path from the repository root, with / separators, sorted.
# step: strict-read, strict-validate, tolerant-read, tolerant-validate, in that order;
#   a validate step is left out when its read refused the input.
# message: 'refusal ' and the ReadError Display; or one Diagnostic Display per line,
#   in the order the step returns them; or '(none)' when the step gives none.
";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(
            || panic!("{ROW}: setup: no repository root above the crate"),
            Path::to_path_buf,
        )
}

/// Every file under `dir` whose name ends in `.xmi`, by its path relative to
/// `root` with `/` separators.
fn fixtures_under(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{ROW}: setup: list {}: {error}", dir.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("{ROW}: setup: list {}: {error}", dir.display()))
            .path();
        if path.is_dir() {
            fixtures_under(root, &path, out);
        } else if path.extension().is_some_and(|extension| extension == "xmi") {
            let relative = path
                .strip_prefix(root)
                .unwrap_or_else(|error| panic!("{ROW}: setup: {}: {error}", path.display()));
            let parts: Vec<String> = relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            out.push(parts.join("/"));
        }
    }
}

/// Every fixture, sorted by its path.
fn fixtures(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for dir in FIXTURE_DIRS {
        fixtures_under(root, &root.join(dir), &mut out);
    }
    out.sort();
    out
}

/// Appends `<fixture> <step>: <message>`, refusing a message that is not one
/// line.
fn push_line(lines: &mut Vec<String>, fixture: &str, step: &str, message: &str) {
    assert!(
        !message.contains(['\n', '\r']),
        "{ROW}: {fixture} {step}: the message is not one line: {message:?}"
    );
    lines.push(format!("{fixture} {step}: {message}"));
}

/// Appends one line per message, or `(none)`.
fn push_messages(lines: &mut Vec<String>, fixture: &str, step: &str, messages: &[String]) {
    if messages.is_empty() {
        push_line(lines, fixture, step, "(none)");
    }
    for message in messages {
        push_line(lines, fixture, step, message);
    }
}

/// The lines of one fixture, in the step order of the format.
fn fixture_lines(root: &Path, fixture: &str, lines: &mut Vec<String>) {
    let bytes = fs::read(root.join(fixture))
        .unwrap_or_else(|error| panic!("{ROW}: setup: read {fixture}: {error}"));
    for (mode, name) in [(Mode::Strict, "strict"), (Mode::Tolerant, "tolerant")] {
        let read = format!("{name}-read");
        match xmi::read(&bytes, &ReadOptions::default().with_mode(mode)) {
            Err(error) => push_line(lines, fixture, &read, &format!("refusal {error}")),
            Ok(output) => {
                let messages: Vec<String> = output
                    .diagnostics()
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                push_messages(lines, fixture, &read, &messages);
                let found: Vec<String> = validation::validate(output.document())
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                push_messages(lines, fixture, &format!("{name}-validate"), &found);
            }
        }
    }
}

/// The whole file as today's code renders it: the header, then the lines.
fn rendered(root: &Path) -> String {
    let mut lines = Vec::new();
    for fixture in fixtures(root) {
        fixture_lines(root, &fixture, &mut lines);
    }
    let mut text = HEADER.to_owned();
    for line in lines {
        text.push_str(&line);
        text.push('\n');
    }
    text
}

#[test]
fn lib_api_017_every_message_of_0_1_0_for_every_fixture_is_unchanged() {
    let root = repository_root();
    let actual = rendered(&root);
    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN);
    let expected = fs::read_to_string(&golden_path).unwrap_or_default();
    if actual == expected {
        return;
    }
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("messages-0.1.0.actual.txt");
    let written = fs::write(&out, &actual).map_or_else(
        |error| format!("(could not write {}: {error})", out.display()),
        |()| format!("rendered lines written to {}", out.display()),
    );
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    let first = expected_lines
        .iter()
        .zip(&actual_lines)
        .position(|(golden, now)| golden != now)
        .unwrap_or(expected_lines.len().min(actual_lines.len()));
    panic!(
        "{ROW}: the messages differ from {GOLDEN} ({} golden lines, {} rendered), first at \
         line {}:\n  golden:   {:?}\n  rendered: {:?}\n{written}",
        expected_lines.len(),
        actual_lines.len(),
        first + 1,
        expected_lines.get(first),
        actual_lines.get(first),
    );
}

/// The lines of `text` that hold a diagnostic: neither a `#` comment nor a
/// refusal, each with its line number in `text`.
fn diagnostic_lines(text: &str) -> Vec<(usize, &str)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.starts_with('#'))
        .filter(|(_, line)| {
            line.split_once(": ")
                .is_none_or(|(_, message)| !message.starts_with("refusal "))
        })
        .map(|(index, line)| (index + 1, line))
        .collect()
}

/// Every line of the golden file that is not a refusal's, the `Display` of a
/// diagnostic or `(none)`, is rendered unchanged, by the renderer of the
/// LIB-API-017 test (LIB-API-018, acceptance 3).
#[test]
fn lib_api_018_every_diagnostic_line_of_the_messages_golden_is_unchanged() {
    let root = repository_root();
    let actual = rendered(&root);
    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN);
    let expected = fs::read_to_string(&golden_path)
        .unwrap_or_else(|error| panic!("{ROW_018}: setup: read {GOLDEN}: {error}"));
    let golden = diagnostic_lines(&expected);
    let now = diagnostic_lines(&actual);
    assert!(
        golden.iter().any(|(_, line)| line.contains(" error SACM-")
            || line.contains(" warning SACM-")
            || line.contains(" info SACM-")),
        "{ROW_018}: setup: {GOLDEN} holds no diagnostic line"
    );
    let first = golden
        .iter()
        .zip(&now)
        .position(|((_, golden), (_, now))| golden != now);
    if let Some(first) = first {
        panic!(
            "{ROW_018}: the diagnostic lines differ from {GOLDEN}, first at its line {}:\n  \
             golden:   {:?}\n  rendered: {:?}",
            golden[first].0, golden[first].1, now[first].1,
        );
    }
    assert_eq!(
        golden.len(),
        now.len(),
        "{ROW_018}: {GOLDEN} holds {} diagnostic lines, the code renders {}",
        golden.len(),
        now.len()
    );
}
