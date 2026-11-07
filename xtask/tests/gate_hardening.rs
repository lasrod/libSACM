//! Acceptance tests for the hardened matrix gate (slice 0004).
//!
//! - LIB-TOOL-005: a verified row needs exactly one `- Row verdict: <ROW-ID>:
//!   PASS` line in its record; mentioning the id is no longer enough.
//! - LIB-TOOL-006: every cited path is a relative path inside the repository,
//!   and a rejected path is never asked of the [`Repo`].
//! - LIB-TOOL-009: area codes are one or more uppercase ASCII letters.
//! - LIB-TOOL-010: a failing `matrix check` returns every problem in its
//!   error, one per line, escaped, in a deterministic order.
//!
//! The gate is driven through the public `xtask::matrix` API against an
//! in-memory repository that records every path the gate asks for, through
//! `xtask::run` on temporary trees, and through the `xtask` binary.
//!
//! Many tests compare against a *baseline*: the problems the gate reports for
//! a record with no verdict line at all. That pins "this line does not count"
//! without fixing the wording of the "no verdict line" message, which the
//! acceptance criteria leave open.
//!
//! Fixture source text is written on one line with `\n` escapes on purpose: a
//! line of this file that began with `#[test]` inside a string literal would
//! be picked up by the real repository's orphan scan.

mod common;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use common::{GATE_TOML, TempTree, consistent_tree};
use xtask::matrix::{
    FsRepo, MATRIX_PATH, Matrix, RENDERED_PATH, Repo, Status, check, load, render, traced_tests,
};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// An in-memory repository that records every path passed to `exists` and
/// `read`, in order.
#[derive(Debug, Default)]
struct FakeRepo {
    files: BTreeMap<String, String>,
    asked: RefCell<Vec<String>>,
}

impl FakeRepo {
    fn with(mut self, path: &str, text: &str) -> Self {
        self.files.insert(path.into(), text.into());
        self
    }

    /// Every path the gate asked for, in order.
    fn asked(&self) -> Vec<String> {
        self.asked.borrow().clone()
    }
}

impl Repo for FakeRepo {
    fn exists(&self, rel: &str) -> bool {
        self.asked.borrow_mut().push(rel.to_owned());
        self.files.contains_key(rel)
    }

    fn read(&self, rel: &str) -> Option<String> {
        self.asked.borrow_mut().push(rel.to_owned());
        self.files.get(rel).cloned()
    }

    fn traced_functions(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (path, text) in &self.files {
            let python = Path::new(path)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("py"));
            for name in traced_tests(text, python) {
                out.push((path.clone(), name));
            }
        }
        out
    }
}

const HEADER: &str = r#"
schema_version = 1
[[milestone]]
id = "M0"
title = "Foundation"
goal = "Build."
[[area]]
code = "BASE"
prefix = "SACM23"
title = "Base"
[[area]]
code = "TOOL"
prefix = "LIB"
title = "Tooling"
"#;

/// The row most tests are about.
const ROW: &str = "LIB-TOOL-001";
/// A second row, for records shared between rows.
const OTHER: &str = "LIB-TOOL-002";
/// The verification record the verified rows cite.
const RECORD: &str = "docs/verification/v.md";

/// `text` as a TOML basic string, with `"`, `\` and control characters
/// escaped.
fn toml_str(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `LIB-TOOL-001` -> `lib_tool_001`.
fn snake(id: &str) -> String {
    id.to_ascii_lowercase().replace('-', "_")
}

/// The traced test cited for row `id`.
fn test_name(id: &str) -> String {
    format!("{}_x", snake(id))
}

/// A Rust test file defining the cited test of each row in `ids`.
fn test_file(ids: &[&str]) -> String {
    let mut out = String::new();
    for id in ids {
        let _ = write!(out, "#[test]\nfn {}() {{}}\n", test_name(id));
    }
    out
}

/// A row in milestone M0 whose area is the id's area segment. `extra` holds
/// further TOML keys (one per line).
fn row(id: &str, status: &str, extra: &str) -> String {
    let area = id.split('-').nth(1).unwrap_or_default();
    format!(
        "\n[[row]]\nid = \"{id}\"\narea = \"{area}\"\nmilestone = \"M0\"\ntitle = \"Title\"\n\
         requirement = \"Requirement.\"\nsource = \"Source\"\nkind = \"tooling\"\n\
         status = \"{status}\"\n{extra}\n"
    )
}

fn matrix(rows: &str) -> Matrix {
    load(&format!("{HEADER}{rows}")).expect("LIB-TOOL-005: test matrix parses")
}

/// Check `m` against `repo` with the rendered page in sync, so that the tests
/// see only the rules under test. Returns the problems and the repository, so
/// that the paths it was asked for can be inspected.
fn gate(m: &Matrix, repo: FakeRepo) -> (Vec<String>, FakeRepo) {
    let repo = repo.with(RENDERED_PATH, &render(m));
    let problems = check(m, &repo);
    (problems, repo)
}

// ---------------------------------------------------------------------------
// LIB-TOOL-005: per-row verdict lines
// ---------------------------------------------------------------------------

/// A verified row `id` with all of its evidence, citing [`RECORD`].
fn verified_row(id: &str) -> String {
    row(
        id,
        "verified",
        &format!(
            "acceptance = [\"a\"]\ncode = [\"src/lib.rs\"]\ntests = [\"t.rs::{}\"]\n\
             verification = \"{RECORD}\"",
            test_name(id)
        ),
    )
}

/// The repository for the verified rows `ids`, with `record` as [`RECORD`].
fn verified_repo(ids: &[&str], record: &str) -> FakeRepo {
    FakeRepo::default()
        .with("src/lib.rs", "")
        .with("t.rs", &test_file(ids))
        .with(RECORD, record)
}

/// The problems for the verified row [`ROW`] whose record reads `record`.
fn record_problems(record: &str) -> Vec<String> {
    let m = matrix(&verified_row(ROW));
    gate(&m, verified_repo(&[ROW], record)).0
}

/// The problems for the verified rows [`ROW`] and [`OTHER`], which share
/// [`RECORD`].
fn shared_record_problems(record: &str) -> Vec<String> {
    let m = matrix(&format!("{}{}", verified_row(ROW), verified_row(OTHER)));
    gate(&m, verified_repo(&[ROW, OTHER], record)).0
}

/// The one problem reported for [`ROW`] when its record has no verdict line:
/// it starts with the row id and names the record.
fn no_verdict_problem() -> String {
    let p = record_problems("# Verification\n\nNothing was judged here.\n");
    assert_eq!(
        p.len(),
        1,
        "LIB-TOOL-005: a record without a verdict line is exactly one problem, got {p:#?}"
    );
    let problem = p[0].clone();
    assert!(
        problem.starts_with(&format!("{ROW}: ")) && problem.contains(RECORD),
        "LIB-TOOL-005: the missing verdict is reported with the row id and the record path, \
         got {problem:?}"
    );
    problem
}

/// The record `record` gives [`ROW`] no verdict: the gate reports exactly the
/// baseline "no verdict line" problem, and nothing else.
fn assert_no_verdict(record: &str, why: &str) {
    let expected = no_verdict_problem();
    assert_eq!(
        record_problems(record),
        [expected],
        "LIB-TOOL-005: {why}; the record is {record:?}"
    );
}

/// `problem` names line `n` of [`RECORD`], as `<record>:<n>` or `line <n>`,
/// with no further digit after `n`.
fn names_line(problem: &str, n: usize) -> bool {
    [format!("{RECORD}:{n}"), format!("line {n}")]
        .iter()
        .any(|needle| {
            problem.match_indices(needle.as_str()).any(|(at, _)| {
                !problem[at + needle.len()..].starts_with(|c: char| c.is_ascii_digit())
            })
        })
}

/// The numbers in `problem` once the row id and the record path are removed.
fn numbers_in(problem: &str) -> Vec<usize> {
    problem
        .replace(ROW, "")
        .replace(RECORD, "")
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse().ok())
        .collect()
}

#[test]
fn lib_tool_005_a_single_pass_line_for_the_row_passes() {
    for record in [
        "- Row verdict: LIB-TOOL-001: PASS",
        "- Row verdict: LIB-TOOL-001: PASS\n",
        "# Verification 0001\n\n- Row verdict: LIB-TOOL-001: PASS\n\n## Findings\n\nNone.\n",
        "# Verification\r\n\r\n- Row verdict: LIB-TOOL-001: PASS\r\n",
        "- Row verdict: LIB-TOOL-002: FAIL\n- Row verdict: LIB-TOOL-001: PASS\n\
         - Row verdict: SACM23-BASE-001: OPEN\n",
    ] {
        assert_eq!(
            record_problems(record),
            Vec::<String>::new(),
            "LIB-TOOL-005: one PASS line for the row passes it; the record is {record:?}"
        );
    }
}

#[test]
fn lib_tool_005_a_record_without_a_verdict_line_fails_the_row() {
    // The assertions live in the helper: exactly one problem, with the row id
    // and the record path.
    let problem = no_verdict_problem();
    assert!(
        !problem.contains("does not mention"),
        "LIB-TOOL-005: the mention check is removed; the problem is about the verdict line, \
         got {problem:?}"
    );
}

#[test]
fn lib_tool_005_a_record_that_only_mentions_the_row_fails() {
    for record in [
        "LIB-TOOL-001: PASS\n",
        "Row verdict: LIB-TOOL-001: PASS\n",
        "* Row verdict: LIB-TOOL-001: PASS\n",
        "+ Row verdict: LIB-TOOL-001: PASS\n",
        "- row verdict: LIB-TOOL-001: PASS\n",
        "- Verdict: LIB-TOOL-001: PASS\n",
        "| LIB-TOOL-001 | PASS |\n",
        "LIB-TOOL-001 passes. Verdict: PASS.\n",
    ] {
        assert_no_verdict(record, "mentioning the row id is not a verdict line");
    }
}

#[test]
fn lib_tool_005_more_than_one_verdict_line_is_reported_with_the_count() {
    // The verdict lines sit on lines 5, 7 and 9, so a line number in the
    // message cannot be mistaken for the count.
    let filler = "# Verification\n\nText.\n\n";
    let two =
        format!("{filler}- Row verdict: LIB-TOOL-001: PASS\n\n- Row verdict: LIB-TOOL-001: PASS\n");
    let three = format!("{two}\n- Row verdict: LIB-TOOL-001: PASS\n");
    for (record, count, other) in [(&two, 2, 3), (&three, 3, 2)] {
        let p = record_problems(record);
        assert_eq!(
            p.len(),
            1,
            "LIB-TOOL-005: {count} PASS lines for the row are one problem, got {p:#?}"
        );
        let numbers = numbers_in(&p[0]);
        assert!(
            p[0].starts_with(&format!("{ROW}: "))
                && p[0].contains(RECORD)
                && numbers.contains(&count)
                && !numbers.contains(&other),
            "LIB-TOOL-005: the problem names the row, the record and the count {count}, \
             got {:?}",
            p[0]
        );
    }

    // Two lines with different verdicts are still more than one line.
    let mixed =
        format!("{filler}- Row verdict: LIB-TOOL-001: PASS\n\n- Row verdict: LIB-TOOL-001: FAIL\n");
    let p = record_problems(&mixed);
    assert!(
        p.iter().any(|m| m.starts_with(&format!("{ROW}: "))
            && m.contains(RECORD)
            && numbers_in(m).contains(&2)),
        "LIB-TOOL-005: a PASS and a FAIL line for the row are reported as two verdict lines, \
         got {p:#?}"
    );
}

#[test]
fn lib_tool_005_a_verdict_other_than_pass_is_reported_by_name() {
    for (verdict, not) in [("FAIL", "OPEN"), ("OPEN", "FAIL")] {
        let record = format!("# Verification\n\n- Row verdict: LIB-TOOL-001: {verdict}\n");
        let p = record_problems(&record);
        assert_eq!(
            p.len(),
            1,
            "LIB-TOOL-005: a {verdict} verdict is exactly one problem, got {p:#?}"
        );
        assert!(
            p[0].starts_with(&format!("{ROW}: "))
                && p[0].contains(RECORD)
                && p[0].contains(verdict)
                && !p[0].contains(not),
            "LIB-TOOL-005: the problem names the row, the record and the verdict {verdict}, \
             got {:?}",
            p[0]
        );
    }
}

#[test]
fn lib_tool_005_a_verdict_line_for_a_similar_id_does_not_count() {
    for similar in [
        "LIB-TOOL-0011",
        "LIB-TOOL-001x",
        "LIB-TOOL-00",
        "XLIB-TOOL-001",
        "lib-tool-001",
        "LIB-TOOL-001-2",
    ] {
        let alone = format!("# Verification\n\n- Row verdict: {similar}: PASS\n");
        let p = record_problems(&alone);
        assert!(
            p.len() == 1 && p[0].starts_with(&format!("{ROW}: ")) && p[0].contains(RECORD),
            "LIB-TOOL-005: a PASS line for {similar} does not pass {ROW}: the row fails with \
             one problem naming it and the record, got {p:#?}"
        );

        // Beside the row's own PASS line, the similar line is not a second
        // verdict line for the row.
        let beside = format!(
            "# Verification\n\n- Row verdict: {ROW}: PASS\n- Row verdict: {similar}: PASS\n"
        );
        assert_eq!(
            record_problems(&beside),
            Vec::<String>::new(),
            "LIB-TOOL-005: a line for {similar} is not a verdict line for {ROW}; the record \
             is {beside:?}"
        );
    }
}

#[test]
fn lib_tool_005_a_verdict_line_for_a_different_row_does_not_count() {
    // The record mentions the row, so the old mention check would pass it.
    assert_no_verdict(
        "# Verification\n\nLIB-TOOL-001 was not judged.\n\n- Row verdict: LIB-TOOL-002: PASS\n",
        "a PASS line for another row is not a verdict for this one",
    );

    // A record shared by two rows: each row reads its own line.
    assert_eq!(
        shared_record_problems(
            "- Row verdict: LIB-TOOL-001: PASS\n- Row verdict: LIB-TOOL-002: PASS\n"
        ),
        Vec::<String>::new(),
        "LIB-TOOL-005: two rows sharing a record each pass on their own PASS line"
    );
    let p = shared_record_problems(
        "- Row verdict: LIB-TOOL-002: PASS\n- Row verdict: LIB-TOOL-001: FAIL\n",
    );
    assert!(
        p.len() == 1
            && p[0].starts_with(&format!("{ROW}: "))
            && p[0].contains(RECORD)
            && p[0].contains("FAIL"),
        "LIB-TOOL-005: only the row whose line says FAIL fails, got {p:#?}"
    );
}

#[test]
fn lib_tool_005_indented_and_block_quoted_lines_are_not_verdict_lines() {
    for record in [
        " - Row verdict: LIB-TOOL-001: PASS\n",
        "  - Row verdict: LIB-TOOL-001: PASS\n",
        "    - Row verdict: LIB-TOOL-001: PASS\n",
        "\t- Row verdict: LIB-TOOL-001: PASS\n",
        "> - Row verdict: LIB-TOOL-001: PASS\n",
        ">- Row verdict: LIB-TOOL-001: PASS\n",
        "> > - Row verdict: LIB-TOOL-001: PASS\n",
    ] {
        assert_no_verdict(
            record,
            "an indented or block-quoted line is neither a verdict line nor malformed",
        );
    }
}

#[test]
fn lib_tool_005_lines_in_fenced_code_blocks_are_not_verdict_lines() {
    for record in [
        "```\n- Row verdict: LIB-TOOL-001: PASS\n```\n",
        "```text\n- Row verdict: LIB-TOOL-001: PASS\n```\n",
        "```markdown\n# Example record\n\n- Row verdict: LIB-TOOL-001: PASS\n```\n",
        "~~~\n- Row verdict: LIB-TOOL-001: PASS\n~~~\n",
        "Text.\n\n```\n- Row verdict: LIB-TOOL-001: PASS\n",
    ] {
        assert_no_verdict(
            record,
            "a line inside a fenced code block is neither a verdict line nor malformed",
        );
    }
}

/// The verdict line the fence tests hide or reveal.
const PASS_LINE: &str = "- Row verdict: LIB-TOOL-001: PASS";

/// `template` with every `{PASS}` placeholder replaced by [`PASS_LINE`].
fn with_pass(template: &str) -> String {
    template.replace("{PASS}", PASS_LINE)
}

/// Each case whose record, `template` with [`with_pass`] applied, does not
/// give [`ROW`] exactly the problems `expected`, described by its `why`.
fn fence_mismatches(cases: &[(&str, &str)], expected: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (template, why) in cases {
        let record = with_pass(template);
        let p = record_problems(&record);
        if p != expected {
            out.push(format!("{why}: the record {record:?} gave {p:?}"));
        }
    }
    out
}

// CommonMark fences (verification 0004, P-005): an opening fence has 0 to 3
// spaces of indent and a run of 3 or more backticks or 3 or more tildes; it
// is closed only by a line of 0 to 3 spaces, a run of the same character at
// least as long, and then only spaces or tabs. Inside an open fence every
// other line is content, and an unclosed fence runs to the end. Each test
// lists every failing case, not only the first.

#[test]
fn lib_tool_005_a_pass_line_inside_a_nested_mismatched_or_indented_fence_does_not_count() {
    let failures = fence_mismatches(
        &[
            (
                "```\n~~~\n{PASS}\n~~~\n```\n",
                "a `~~~` line inside a ``` fence is content, not a closer",
            ),
            (
                "~~~\n```\n{PASS}\n```\n~~~\n",
                "a ``` line inside a `~~~` fence is content, not a closer",
            ),
            (
                "````\n```\n{PASS}\n```\n````\n",
                "a ``` line inside a ```` fence is a shorter run, so content",
            ),
            (
                "~~~~\n~~~\n{PASS}\n~~~\n~~~~\n",
                "a `~~~` line inside a `~~~~` fence is a shorter run, so content",
            ),
            (
                "```\n```text\n{PASS}\n```\n",
                "a ``` line with an info string inside an open fence is content, not a closer",
            ),
            (
                "~~~\n~~~ text\n{PASS}\n~~~\n",
                "a `~~~` line with an info string inside an open fence is content, not a closer",
            ),
            (
                " ```\n{PASS}\n ```\n",
                "a fence indented 1 space is a fence",
            ),
            (
                "  ```\n{PASS}\n  ```\n",
                "a fence indented 2 spaces is a fence",
            ),
            (
                "   ```\n{PASS}\n   ```\n",
                "a fence indented 3 spaces is a fence",
            ),
            (
                "   ~~~\n{PASS}\n   ~~~\n",
                "a `~~~` fence indented 3 spaces is a fence",
            ),
            (
                "~~~\n```\n{PASS}\n",
                "an unclosed fence runs to the end, past a fence line of the other character",
            ),
        ],
        &[no_verdict_problem()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a PASS line inside a fence is neither a verdict line nor malformed, so \
         the row gets exactly the no-verdict problem:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_fence_closes_only_on_the_same_character_with_a_run_at_least_as_long() {
    let failures = fence_mismatches(
        &[
            (
                "````\n```\n````\n{PASS}\n",
                "```` closes the ```` fence that holds a ``` line",
            ),
            (
                "```\ncode\n`````\n{PASS}\n",
                "a longer run closes the fence",
            ),
            (
                "~~~~\ncode\n~~~~~\n{PASS}\n",
                "a longer `~` run closes the fence",
            ),
            (
                "```\ncode\n```  \n{PASS}\n",
                "a closing fence with trailing spaces closes",
            ),
            (
                "```\ncode\n```\t\n{PASS}\n",
                "a closing fence with a trailing tab closes",
            ),
            (
                "~~~\ncode\n~~~ \t\n{PASS}\n",
                "a closing `~~~` with trailing spaces and a tab closes",
            ),
            (
                "```\r\ncode\r\n```\r\n{PASS}\r\n",
                "a closing fence with a CRLF ending closes",
            ),
            (
                "```\ncode\n ```\n{PASS}\n",
                "a closing fence indented 1 space closes",
            ),
            (
                "```\ncode\n  ```\n{PASS}\n",
                "a closing fence indented 2 spaces closes",
            ),
            (
                "```\ncode\n   ```\n{PASS}\n",
                "a closing fence indented 3 spaces closes",
            ),
            (
                "~~~\ncode\n   ~~~~\n{PASS}\n",
                "a longer closing `~~~~` indented 3 spaces closes",
            ),
            (
                "  ```\ncode\n```\n{PASS}\n",
                "an opening fence indented 2 spaces is closed by an unindented one",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: the fence closes, so the PASS line after it passes the row:\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_fence_indented_four_spaces_is_not_a_fence() {
    let mut failures = fence_mismatches(
        &[
            (
                "    ```\n{PASS}\n",
                "``` indented 4 spaces does not open a fence",
            ),
            (
                "    ```\n{PASS}\n    ```\n",
                "``` indented 4 spaces neither opens nor closes a fence",
            ),
            (
                "    ~~~\n{PASS}\n",
                "`~~~` indented 4 spaces does not open a fence",
            ),
            (
                "\t```\n{PASS}\n",
                "``` after a tab is not indented 0 to 3 spaces, so it does not open a fence",
            ),
        ],
        &[],
    );
    failures.extend(fence_mismatches(
        &[(
            "```\n    ```\n{PASS}\n",
            "inside an open fence, ``` indented 4 spaces is content, not a closer",
        )],
        &[no_verdict_problem()],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a fence line indented 4 spaces or a tab is not a fence line:\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_run_of_two_backticks_or_tildes_is_not_a_fence() {
    let mut failures = fence_mismatches(
        &[
            ("``\n{PASS}\n``\n", "two backticks do not open a fence"),
            ("~~\n{PASS}\n~~\n", "two tildes do not open a fence"),
            (
                "``text\n{PASS}\n",
                "two backticks and an info string do not open a fence",
            ),
            ("  ~~\n{PASS}\n", "two indented tildes do not open a fence"),
        ],
        &[],
    );
    failures.extend(fence_mismatches(
        &[
            (
                "```\n``\n{PASS}\n",
                "two backticks do not close a fence of three",
            ),
            (
                "~~~\n~~\n{PASS}\n",
                "two tildes do not close a fence of three",
            ),
        ],
        &[no_verdict_problem()],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a run of two backticks or tildes is not a fence line:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_fail_line_beside_a_fenced_pass_example_is_reported_as_fail() {
    let fail_line = "- Row verdict: LIB-TOOL-001: FAIL";
    let expected = record_problems(&format!("{fail_line}\n"));
    assert!(
        expected.len() == 1 && expected[0].contains("FAIL"),
        "LIB-TOOL-005: a lone FAIL line is the single FAIL problem, got {expected:#?}"
    );
    let cases: Vec<(String, &str)> = [
        (
            "{FAIL}\n````\n```\n{PASS}\n```\n````\n",
            "a FAIL line, then a ```` fence holding a ``` pair around a PASS line",
        ),
        (
            "{FAIL}\n~~~\n```\n{PASS}\n```\n~~~\n",
            "a FAIL line, then a `~~~` fence holding a ``` pair around a PASS line",
        ),
        (
            "````\n```\n{PASS}\n```\n````\n{FAIL}\n",
            "a ```` fence holding a ``` pair around a PASS line, then a FAIL line",
        ),
    ]
    .into_iter()
    .map(|(template, why)| (template.replace("{FAIL}", fail_line), why))
    .collect();
    let cases: Vec<(&str, &str)> = cases.iter().map(|(t, why)| (t.as_str(), *why)).collect();
    let failures = fence_mismatches(&cases, &expected);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: the fenced PASS example is not a verdict line, so the row's verdict is \
         its FAIL line alone:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_lines_after_a_closed_fence_count() {
    for record in [
        "```\ncode\n```\n- Row verdict: LIB-TOOL-001: PASS\n",
        "```\n- Row verdict: LIB-TOOL-001: FAIL\n```\n- Row verdict: LIB-TOOL-001: PASS\n",
        "- Row verdict: LIB-TOOL-001: PASS\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n",
        "```\na\n```\n\n```\nb\n```\n- Row verdict: LIB-TOOL-001: PASS\n",
    ] {
        assert_eq!(
            record_problems(record),
            Vec::<String>::new(),
            "LIB-TOOL-005: exactly one PASS line outside the fences passes the row; the record \
             is {record:?}"
        );
    }
}

#[test]
fn lib_tool_005_trailing_spaces_and_tabs_are_accepted() {
    for record in [
        "- Row verdict: LIB-TOOL-001: PASS \n",
        "- Row verdict: LIB-TOOL-001: PASS\t\n",
        "- Row verdict: LIB-TOOL-001: PASS \t \t\n",
        "- Row verdict: LIB-TOOL-001: PASS  \r\n",
        "- Row verdict: LIB-TOOL-001: PASS\t",
    ] {
        assert_eq!(
            record_problems(record),
            Vec::<String>::new(),
            "LIB-TOOL-005: trailing spaces and tabs are removed before matching; the record is \
             {record:?}"
        );
    }
}

/// Malformed verdict lines: each starts with `- Row verdict:` and is not
/// `- Row verdict: <ROW-ID>: <PASS|FAIL|OPEN>`.
const MALFORMED: &[&str] = &[
    // A bad verdict word.
    "- Row verdict: LIB-TOOL-001: PASSED",
    "- Row verdict: LIB-TOOL-001: pass",
    "- Row verdict: LIB-TOOL-001: Pass",
    "- Row verdict: LIB-TOOL-001: MAYBE",
    "- Row verdict: LIB-TOOL-001: PASS.",
    "- Row verdict: LIB-TOOL-001: PASS (round 2)",
    "- Row verdict: LIB-TOOL-001: PASS: again",
    "- Row verdict: LIB-TOOL-001: ",
    // A missing colon or separator.
    "- Row verdict: LIB-TOOL-001 PASS",
    "- Row verdict: LIB-TOOL-001",
    "- Row verdict: LIB-TOOL-001:PASS",
    "- Row verdict:LIB-TOOL-001: PASS",
    "- Row verdict: LIB-TOOL-001:  PASS",
    // An empty id.
    "- Row verdict: : PASS",
    "- Row verdict:",
];

#[test]
fn lib_tool_005_malformed_verdict_lines_are_reported_with_the_record_and_line_number() {
    for malformed in MALFORMED {
        // The row passes on line 3; the malformed line is line 4.
        let record = format!("# Verification\n\n- Row verdict: LIB-TOOL-001: PASS\n{malformed}\n");
        let p = record_problems(&record);
        assert_eq!(
            p.len(),
            1,
            "LIB-TOOL-005: {malformed:?} is one problem, and the row's own PASS line stands; \
             got {p:#?}"
        );
        assert!(
            p[0].contains(RECORD)
                && p[0].contains("malformed")
                && names_line(&p[0], 4)
                && !names_line(&p[0], 3)
                && !names_line(&p[0], 5),
            "LIB-TOOL-005: {malformed:?} is reported as malformed with the record path and its \
             1-based line number 4, got {:?}",
            p[0]
        );
    }
}

#[test]
fn lib_tool_005_a_malformed_line_is_not_a_verdict_for_the_row() {
    let expected = no_verdict_problem();
    for malformed in MALFORMED {
        let p = record_problems(&format!("{malformed}\n"));
        assert!(
            p.contains(&expected),
            "LIB-TOOL-005: the malformed line {malformed:?} does not pass the row, got {p:#?}"
        );
        assert!(
            p.iter()
                .any(|m| m.contains("malformed") && m.contains(RECORD) && names_line(m, 1)),
            "LIB-TOOL-005: the malformed line {malformed:?} on line 1 is reported, got {p:#?}"
        );
        assert_eq!(
            p.len(),
            2,
            "LIB-TOOL-005: a lone malformed line gives the malformed problem and the missing \
             verdict, got {p:#?}"
        );
    }
}

#[test]
fn lib_tool_005_line_numbers_count_every_line_of_the_record() {
    // Line 2 and line 14 are malformed. The fenced and indented look-alikes on
    // lines 5 and 7 are not reported, and neither is the look-alike on line
    // 10, inside a ``` line pair nested in a ```` fence (lines 8 to 12). CRLF
    // endings do not change the count.
    let record = "# V\r\n- Row verdict: LIB-TOOL-001: pass\r\n\r\n```\r\n\
                  - Row verdict: LIB-TOOL-001: pass\r\n```\r\n  - Row verdict: x\r\n\
                  ````\r\n```\r\n- Row verdict: LIB-TOOL-001: pass\r\n```\r\n````\r\n\
                  - Row verdict: LIB-TOOL-001: PASS\r\n- Row verdict:\r\n";
    let p = record_problems(record);
    let malformed: Vec<&String> = p.iter().filter(|m| m.contains("malformed")).collect();
    assert_eq!(
        malformed.len(),
        2,
        "LIB-TOOL-005: exactly the two unfenced, unindented malformed lines are reported, \
         got {p:#?}"
    );
    assert!(
        names_line(malformed[0], 2) && names_line(malformed[1], 14),
        "LIB-TOOL-005: the malformed lines are lines 2 and 14, in that order, got {malformed:#?}"
    );
    assert_eq!(
        p.len(),
        2,
        "LIB-TOOL-005: the PASS line on line 13 passes the row, got {p:#?}"
    );
}

#[test]
fn lib_tool_005_a_missing_record_is_reported_once() {
    let m = matrix(&verified_row(ROW));
    let mut repo = verified_repo(&[ROW], "");
    repo.files.remove(RECORD);
    assert_eq!(
        gate(&m, repo).0,
        [format!(
            "{ROW}: verification record {RECORD} does not exist"
        )],
        "LIB-TOOL-005: a missing record is one problem, not also a missing verdict"
    );
}

/// Decision D5 of slice 0004: a record's malformed verdict line is a problem
/// of every verified row that cites the record.
#[test]
fn lib_tool_005_a_malformed_line_in_a_shared_record_is_reported_once_per_citing_row() {
    // Both rows pass on their own lines; line 3 is malformed.
    let record =
        "- Row verdict: LIB-TOOL-001: PASS\n- Row verdict: LIB-TOOL-002: PASS\n- Row verdict: x\n";
    let p = shared_record_problems(record);
    assert_eq!(
        p.len(),
        2,
        "LIB-TOOL-005: the malformed line is reported once for each of the two citing rows, \
         got {p:#?}"
    );
    for (problem, id) in p.iter().zip([ROW, OTHER]) {
        assert!(
            problem.starts_with(&format!("{id}: "))
                && problem.contains("malformed")
                && problem.contains(RECORD)
                && names_line(problem, 3),
            "LIB-TOOL-005: the report for {id} is prefixed with its id and names the record and \
             line 3, got {problem:?}"
        );
    }
    assert_eq!(
        p[0].strip_prefix(&format!("{ROW}: ")),
        p[1].strip_prefix(&format!("{OTHER}: ")),
        "LIB-TOOL-005: the two reports differ only in the row id, got {p:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-006: cited paths stay inside the repository
// ---------------------------------------------------------------------------

/// Where a path can be cited.
#[derive(Debug, Clone, Copy)]
enum Field {
    /// A `code` entry.
    Code,
    /// The path part of a `path::function` test citation.
    TestPath,
    /// The path of a `doctest:path` citation.
    Doctest,
    /// `verification`.
    Verification,
}

const FIELDS: [Field; 4] = [
    Field::Code,
    Field::TestPath,
    Field::Doctest,
    Field::Verification,
];

/// [`ROW`] citing `path` in `field`, otherwise consistent, and a repository
/// in which every other citation resolves and `path` holds what the field
/// needs.
fn cite(field: Field, path: &str) -> (Matrix, FakeRepo) {
    let name = test_name(ROW);
    let p = toml_str(path);
    let (status, extra, file) = match field {
        Field::Code => (
            "implemented",
            format!("code = [{p}]\ntests = [\"t.rs::{name}\"]"),
            String::new(),
        ),
        Field::TestPath => (
            "tests-failing",
            format!("tests = [{}]", toml_str(&format!("{path}::{name}"))),
            test_file(&[ROW]),
        ),
        Field::Doctest => (
            "tests-failing",
            format!("tests = [{}]", toml_str(&format!("doctest:{path}"))),
            "//! ```\n//! assert!(true);\n//! ```\n".to_owned(),
        ),
        Field::Verification => (
            "verified",
            format!("code = [\"src/lib.rs\"]\ntests = [\"t.rs::{name}\"]\nverification = {p}"),
            "- Row verdict: LIB-TOOL-001: PASS\n".to_owned(),
        ),
    };
    let m = matrix(&row(ROW, status, &format!("acceptance = [\"a\"]\n{extra}")));
    let mut repo = FakeRepo::default().with(path, &file);
    if !matches!(field, Field::TestPath | Field::Doctest) {
        repo = repo.with("t.rs", &test_file(&[ROW]));
    }
    if matches!(field, Field::Verification) {
        repo = repo.with("src/lib.rs", "");
    }
    (m, repo)
}

fn rejection(path: &str) -> String {
    format!("{ROW}: cited path `{path}` must be a relative path inside the repository")
}

/// Each path, cited in each field, is reported once with the rejection
/// message and nothing else, and is never asked of the repository.
fn assert_rejected(class: &str, paths: &[&str]) {
    for field in FIELDS {
        for path in paths {
            let (m, repo) = cite(field, path);
            let (p, repo) = gate(&m, repo);
            assert_eq!(
                p,
                [rejection(path)],
                "LIB-TOOL-006: {class}: {path:?} cited as {field:?} is rejected, and its \
                 existence, content and test checks are skipped"
            );
            let asked = repo.asked();
            assert!(
                !asked.iter().any(|a| a == path),
                "LIB-TOOL-006: {class}: the rejected {path:?} cited as {field:?} is never read \
                 or statted; the gate asked for {asked:?}"
            );
        }
    }
}

#[test]
fn lib_tool_006_rejects_an_empty_path() {
    assert_rejected("an empty path", &[""]);
}

#[test]
fn lib_tool_006_rejects_an_absolute_path() {
    assert_rejected(
        "a path starting with `/`",
        &["/", "/etc/passwd", "/docs/a.md", "//server/share/a.md"],
    );
}

#[test]
fn lib_tool_006_rejects_a_path_containing_a_backslash() {
    assert_rejected(
        "a path containing `\\`",
        &["docs\\a.md", "..\\a.md", "\\docs\\a.md", "docs/a\\"],
    );
}

#[test]
fn lib_tool_006_rejects_a_path_containing_a_colon() {
    assert_rejected(
        "a path containing `:`",
        &["C:/x.md", "C:x.md", "docs/a:b.md", "file:docs/a.md"],
    );
}

#[test]
fn lib_tool_006_rejects_a_path_with_an_empty_component() {
    assert_rejected(
        "a path with an empty component",
        &["a//b", "docs//a.md", "a/", "docs/a/", "a///b"],
    );
}

#[test]
fn lib_tool_006_rejects_a_path_with_a_dot_component() {
    assert_rejected(
        "a path with a `.` component",
        &[".", "./a", "./docs/a.md", "docs/./a.md", "docs/."],
    );
}

#[test]
fn lib_tool_006_rejects_a_path_with_a_dot_dot_component() {
    assert_rejected(
        "a path with a `..` component",
        &["..", "../a", "../../etc/passwd", "docs/../a.md", "docs/.."],
    );
}

#[test]
fn lib_tool_006_a_path_breaking_several_rules_is_reported_once() {
    assert_rejected("a path breaking every rule", &["/a/../b\\c:d//"]);
}

#[test]
fn lib_tool_006_accepts_dot_names_and_double_dots_inside_a_component() {
    for path in [
        ".github/workflows/ci.yml",
        ".cargo/mutants.toml",
        ".gitignore",
        "docs/.hidden/a.md",
        "a..b.md",
        "docs/a..b.md",
        "..a/b.md",
        "a../b.md",
        "docs/a.",
        "docs/.a",
    ] {
        for field in FIELDS {
            let (m, repo) = cite(field, path);
            let (p, repo) = gate(&m, repo);
            assert_eq!(
                p,
                Vec::<String>::new(),
                "LIB-TOOL-006: {path:?} cited as {field:?} is a real relative path and passes"
            );
            assert!(
                repo.asked().iter().any(|a| a == path),
                "LIB-TOOL-006: the accepted {path:?} cited as {field:?} is still checked against \
                 the repository"
            );
        }
    }
}

#[test]
fn lib_tool_006_a_rejected_path_does_not_hide_the_other_citations() {
    let name = test_name(ROW);
    let m = matrix(&row(
        ROW,
        "implemented",
        &format!(
            "acceptance = [\"a\"]\ncode = [\"../x.rs\", \"src/missing.rs\"]\n\
             tests = [\"../t.rs::{name}\", \"t.rs::{name}\", \"t.rs::lib_tool_001_missing\", \
             \"doctest:/abs.md\", \"doctest:src/missing.md\"]"
        ),
    ));
    let repo = FakeRepo::default().with("t.rs", &test_file(&[ROW]));
    let (mut p, repo) = gate(&m, repo);
    p.sort();
    let mut expected = vec![
        rejection("../x.rs"),
        rejection("../t.rs"),
        rejection("/abs.md"),
        format!("{ROW}: cited code path src/missing.rs does not exist"),
        format!("{ROW}: test function `lib_tool_001_missing` is not defined in t.rs"),
        format!("{ROW}: doctest file src/missing.md does not exist"),
    ];
    expected.sort();
    assert_eq!(
        p, expected,
        "LIB-TOOL-006: each rejected citation is reported and skipped; the row's other \
         citations are still checked"
    );
    let asked = repo.asked();
    for rejected in ["../x.rs", "../t.rs", "/abs.md"] {
        assert!(
            !asked.iter().any(|a| a == rejected),
            "LIB-TOOL-006: the rejected {rejected:?} is never asked for; the gate asked for \
             {asked:?}"
        );
    }
}

/// Every status except `verified`, as written in the matrix.
const NOT_VERIFIED: [&str; 7] = [
    "planned",
    "analyzed",
    "tests-failing",
    "implemented",
    "candidate",
    "deferred",
    "out-of-scope",
];

/// [`ROW`] with `status` and the evidence that status requires, plus
/// `verification` when it is given, and a repository in which that evidence
/// resolves.
fn status_row(status: &str, verification: Option<&str>) -> (Matrix, FakeRepo) {
    let mut extra = String::from("acceptance = [\"a\"]\nnotes = \"why\"\n");
    let mut repo = FakeRepo::default();
    if matches!(status, "tests-failing" | "implemented") {
        let _ = writeln!(extra, "tests = [\"t.rs::{}\"]", test_name(ROW));
        repo = repo.with("t.rs", &test_file(&[ROW]));
    }
    if status == "implemented" {
        extra.push_str("code = [\"src/lib.rs\"]\n");
        repo = repo.with("src/lib.rs", "");
    }
    if let Some(path) = verification {
        let _ = writeln!(extra, "verification = {}", toml_str(path));
    }
    (matrix(&row(ROW, status, &extra)), repo)
}

/// Verification 0004, P-006: `verification` is a cited path at every status,
/// so a non-verified row citing a path outside the repository gets both the
/// rejection and the "cites a verification record" problem.
#[test]
fn lib_tool_006_a_bad_verification_path_on_a_non_verified_row_is_rejected_and_never_read() {
    // Each row is otherwise consistent, so the comparison below sees only the
    // two problems under test.
    for status in NOT_VERIFIED {
        let (m, repo) = status_row(status, None);
        assert_eq!(
            gate(&m, repo).0,
            Vec::<String>::new(),
            "LIB-TOOL-006: a {status} row with its required evidence and no record passes"
        );
    }
    let mut failures = Vec::new();
    for status in NOT_VERIFIED {
        for path in ["", "/x", "a\\b", "C:/x", "a//b", "./a", "../a"] {
            let (m, repo) = status_row(status, Some(path));
            let (mut p, repo) = gate(&m, repo);
            p.sort();
            let mut expected = vec![
                rejection(path),
                format!("{ROW}: is {status} but cites a verification record"),
            ];
            expected.sort();
            if p != expected {
                failures.push(format!("{status} citing {path:?}: got {p:?}"));
            }
            let asked = repo.asked();
            if asked.iter().any(|a| a == path) {
                failures.push(format!(
                    "{status} citing {path:?}: the rejected path was asked for: {asked:?}"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "LIB-TOOL-006: a non-verified row citing a verification record outside the repository \
         gets the rejection once and the \"cites a verification record\" problem, and the path \
         is never read or statted:\n{failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-009: area codes
// ---------------------------------------------------------------------------

/// The problems of a matrix that declares one more area, with code `code`.
fn area_problems(code: &str) -> Vec<String> {
    let area = format!(
        "\n[[area]]\ncode = {}\nprefix = \"LIB\"\ntitle = \"Extra\"\n",
        toml_str(code)
    );
    let m = load(&format!("{HEADER}{area}")).expect("LIB-TOOL-009: test matrix parses");
    gate(&m, FakeRepo::default()).0
}

fn area_code_problem(code: &str) -> String {
    format!("area code `{code}` must be one or more uppercase ASCII letters")
}

#[test]
fn lib_tool_009_rejects_area_codes_that_are_not_uppercase_ascii_letters() {
    for code in [
        // Empty.
        "", // Lowercase, alone or mixed.
        "a", "z", "tool", "Tool", "TOOl", // Digits.
        "1", "T1", "1T", "SACM23",
        // Punctuation, including the neighbours of `A` and `Z` in ASCII.
        "T-X", "T_X", "@", "[", "`", "{", "T.", // Spaces.
        " ", "T X", " TOOL", "TOOL ", // Non-ASCII letters, uppercase in Unicode.
        "\u{c4}", "T\u{c4}", "\u{3a3}A", "\u{ff21}",
    ] {
        assert_eq!(
            area_problems(code),
            [area_code_problem(code)],
            "LIB-TOOL-009: area code {code:?} is rejected"
        );
    }
}

#[test]
fn lib_tool_009_accepts_uppercase_ascii_area_codes() {
    for code in ["A", "Z", "AZ", "QA", "ARGUMENTATION"] {
        assert_eq!(
            area_problems(code),
            Vec::<String>::new(),
            "LIB-TOOL-009: area code {code:?} is one or more uppercase ASCII letters"
        );
    }
}

#[test]
fn lib_tool_009_a_duplicate_bad_code_reports_both_problems() {
    let p = area_problems("base");
    assert_eq!(
        p,
        [area_code_problem("base")],
        "LIB-TOOL-009: `base` differs from `BASE`, so only the code itself is wrong"
    );

    let twice = "\n[[area]]\ncode = \"t-1\"\nprefix = \"LIB\"\ntitle = \"One\"\n\
                 [[area]]\ncode = \"t-1\"\nprefix = \"LIB\"\ntitle = \"Two\"\n";
    let m = load(&format!("{HEADER}{twice}")).expect("LIB-TOOL-009: test matrix parses");
    let p = gate(&m, FakeRepo::default()).0;
    assert!(
        p.iter().any(|m| m == "area codes must be unique"),
        "LIB-TOOL-009: the uniqueness check stays, got {p:#?}"
    );
    assert!(
        p.iter().any(|m| *m == area_code_problem("t-1")),
        "LIB-TOOL-009: a duplicated bad code is also reported as bad, got {p:#?}"
    );

    let good_twice = "\n[[area]]\ncode = \"BASE\"\nprefix = \"SACM23\"\ntitle = \"Again\"\n";
    let m = load(&format!("{HEADER}{good_twice}")).expect("LIB-TOOL-009: test matrix parses");
    assert_eq!(
        gate(&m, FakeRepo::default()).0,
        ["area codes must be unique"],
        "LIB-TOOL-009: a duplicated good code is only a uniqueness problem"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-010: every problem reaches the user
// ---------------------------------------------------------------------------

/// The environment variable that points the `xtask` binary at another
/// repository root. **Not yet designed:** the binary resolves its root at
/// compile time, so a test cannot run it on a temporary tree; this name is the
/// test engineer's proposal to the lead.
const ROOT_ENV: &str = "XTASK_ROOT";

/// [`GATE_TOML`] plus a planned row with an unknown milestone and an unknown
/// dependency.
fn two_problem_toml() -> String {
    format!(
        "{GATE_TOML}\n[[row]]\nid = \"LIB-TOOL-002\"\narea = \"TOOL\"\nmilestone = \"M9\"\n\
         title = \"Second\"\nrequirement = \"Requirement.\"\nsource = \"Source\"\n\
         kind = \"tooling\"\nstatus = \"planned\"\ndepends_on = [\"LIB-TOOL-999\"]\n"
    )
}

/// A consistent tree, then: the matrix of [`two_problem_toml`] with its page
/// in sync, and three uncited traced tests in two files, written in reverse
/// order.
fn failing_tree(name: &str) -> TempTree {
    let text = two_problem_toml();
    let m = load(&text).expect("LIB-TOOL-010: the failing fixture parses");
    let tree = consistent_tree(name);
    tree.write(MATRIX_PATH, &text)
        .write(RENDERED_PATH, &render(&m))
        .write("xtask/tests/a.rs", "#[test]\nfn lib_tool_002_m() {}\n")
        .write(
            "crates/demo/tests/z.rs",
            "#[test]\nfn lib_tool_002_z() {}\n#[test]\nfn lib_tool_002_a() {}\n",
        );
    tree
}

/// The problems of [`failing_tree`], in the order `check` returns them.
const FAILING_TREE_PROBLEMS: [&str; 5] = [
    "LIB-TOOL-002: unknown milestone `M9`",
    "LIB-TOOL-002: depends on unknown row LIB-TOOL-999",
    "crates/demo/tests/z.rs: traced test `lib_tool_002_a` is not cited by any matrix row",
    "crates/demo/tests/z.rs: traced test `lib_tool_002_z` is not cited by any matrix row",
    "xtask/tests/a.rs: traced test `lib_tool_002_m` is not cited by any matrix row",
];

/// The error `matrix check` must return for `problems`.
fn expected_error<S: AsRef<str>>(problems: &[S]) -> String {
    let mut out = format!("functionality matrix: {} problem(s)", problems.len());
    for problem in problems {
        out.push_str("\n  ");
        out.push_str(problem.as_ref());
    }
    out
}

/// Run `matrix check` on `root` through `xtask::run`: its error, and what it
/// wrote to `out`.
fn run_check(root: &Path) -> (Result<(), String>, String) {
    let mut out = Vec::new();
    let result = xtask::run(&["matrix", "check"], root, &mut out);
    (result, String::from_utf8_lossy(&out).into_owned())
}

#[test]
fn lib_tool_010_run_matrix_check_returns_every_problem_in_its_error() {
    let tree = failing_tree("hardening-every-problem");
    let (result, out) = run_check(tree.root());
    assert_eq!(
        result,
        Err(expected_error(&FAILING_TREE_PROBLEMS)),
        "LIB-TOOL-010: the error is the count line followed by every problem, one per line, \
         indented by two spaces"
    );
    assert!(
        out.is_empty(),
        "LIB-TOOL-010: a failing check writes nothing to `out`, wrote {out:?}"
    );

    // The error lists exactly what `check` returns, in its order.
    let m = load(&two_problem_toml()).expect("LIB-TOOL-010: the failing fixture parses");
    let problems = check(&m, &FsRepo::new(tree.root()));
    assert_eq!(
        result,
        Err(expected_error(&problems)),
        "LIB-TOOL-010: the error carries the problems in the order `check` returns them"
    );
}

#[test]
fn lib_tool_010_each_problem_is_on_its_own_line_exactly_once() {
    let tree = failing_tree("hardening-once");
    let (result, _) = run_check(tree.root());
    let error = result.expect_err("LIB-TOOL-010: the failing tree fails the check");
    let lines: Vec<&str> = error.split('\n').collect();
    assert_eq!(
        lines.first().copied(),
        Some("functionality matrix: 5 problem(s)"),
        "LIB-TOOL-010: the first line counts the problems, got {error:?}"
    );
    assert_eq!(
        lines.len(),
        FAILING_TREE_PROBLEMS.len() + 1,
        "LIB-TOOL-010: one line per problem after the count, got {error:?}"
    );
    for problem in FAILING_TREE_PROBLEMS {
        let line = format!("  {problem}");
        assert_eq!(
            lines.iter().filter(|l| **l == line).count(),
            1,
            "LIB-TOOL-010: {problem:?} is on a line of its own exactly once, got {error:?}"
        );
    }
}

/// Values that would split or forge a line if printed raw: the text before
/// the first control character, the rest, and a marker found only in the
/// rest.
const FORGERIES: [(&str, &str, &str); 4] = [
    (
        "M0",
        "\n  LIB-TOOL-001: forged milestone",
        "forged milestone",
    ),
    (
        "LIB-TOOL-9",
        "\r\n  LIB-TOOL-001: forged dependency",
        "forged dependency",
    ),
    ("LIB-TOOL-8", "\u{1b}[2K\rforged escape", "forged escape"),
    (
        "crates/x",
        "\u{b}\u{c}\u{85}\u{7f}\tforged path.rs",
        "forged path",
    ),
];

/// A matrix whose row LIB-TOOL-002 carries every value of [`FORGERIES`]: an
/// unknown milestone, two unknown dependencies and a missing test file.
fn forged_toml() -> String {
    let [milestone, dep_a, dep_b, test_path] =
        FORGERIES.map(|(head, tail, _)| format!("{head}{tail}"));
    format!(
        "{GATE_TOML}\n[[row]]\nid = \"LIB-TOOL-002\"\narea = \"TOOL\"\nmilestone = {}\n\
         title = \"Second\"\nrequirement = \"Requirement.\"\nsource = \"Source\"\n\
         kind = \"tooling\"\nstatus = \"tests-failing\"\nacceptance = [\"a\"]\n\
         depends_on = [{}, {}]\ntests = [{}]\n",
        toml_str(&milestone),
        toml_str(&dep_a),
        toml_str(&dep_b),
        toml_str(&format!("{test_path}::lib_tool_002_x")),
    )
}

#[test]
fn lib_tool_010_a_control_character_in_a_value_cannot_split_or_forge_a_line() {
    let text = forged_toml();
    let m = load(&text).expect("LIB-TOOL-010: the forged fixture parses");
    let tree = consistent_tree("hardening-forged");
    tree.write(MATRIX_PATH, &text)
        .write(RENDERED_PATH, &render(&m));
    let n = check(&m, &FsRepo::new(tree.root())).len();
    assert!(
        n >= FORGERIES.len(),
        "LIB-TOOL-010: precondition: each forged value is in a problem, got {n} problems"
    );

    let (result, _) = run_check(tree.root());
    let error = result.expect_err("LIB-TOOL-010: the forged tree fails the check");
    assert!(
        !error.chars().any(|c| c.is_control() && c != '\n'),
        "LIB-TOOL-010: no control character other than the line breaks between problems \
         reaches the error, got {error:?}"
    );
    let lines: Vec<&str> = error.split('\n').collect();
    assert_eq!(
        lines.len(),
        n + 1,
        "LIB-TOOL-010: {n} problems take exactly {n} lines after the count, got {error:?}"
    );
    assert_eq!(
        lines[0],
        format!("functionality matrix: {n} problem(s)"),
        "LIB-TOOL-010: the count line, got {error:?}"
    );
    for (head, tail, marker) in FORGERIES {
        let holding: Vec<&&str> = lines.iter().filter(|l| l.contains(marker)).collect();
        assert!(
            holding.len() == 1
                && holding[0].starts_with("  LIB-TOOL-002: ")
                && holding[0].contains(&format!("{head}\\")),
            "LIB-TOOL-010: the value {head:?} + {tail:?} stays on its row's line, with the \
             control characters after {head:?} shown escaped; got {error:?}"
        );
    }
}

/// A repository whose traced functions come out in a different order on
/// every call, none of them sorted.
#[derive(Debug)]
struct ShuffledRepo {
    inner: FakeRepo,
    traced: Vec<(String, String)>,
    calls: Cell<usize>,
}

impl Repo for ShuffledRepo {
    fn exists(&self, rel: &str) -> bool {
        self.inner.exists(rel)
    }

    fn read(&self, rel: &str) -> Option<String> {
        self.inner.read(rel)
    }

    fn traced_functions(&self) -> Vec<(String, String)> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        let mut out = self.traced.clone();
        let len = out.len().max(1);
        out.rotate_left(call % len);
        if call % 2 == 1 {
            out.reverse();
        }
        out
    }
}

/// Uncited traced tests, out of order. Sorting the messages as strings would
/// order them differently from sorting by file and then by function: `a.rs.rs:`
/// sorts before `a.rs:`, and `` `lib_tool_001_a` `` before `` `lib_tool_001` ``.
const UNSORTED_ORPHANS: [(&str, &str); 6] = [
    ("b.rs", "lib_tool_001_b"),
    ("a.rs.rs", "lib_tool_001_a"),
    ("a.rs", "lib_tool_001_z"),
    ("b.rs", "lib_tool_001_a"),
    ("a.rs", "lib_tool_001"),
    ("a.rs", "lib_tool_001_a"),
];

fn shuffled_repo(m: &Matrix) -> ShuffledRepo {
    ShuffledRepo {
        inner: FakeRepo::default().with(RENDERED_PATH, &render(m)),
        traced: UNSORTED_ORPHANS
            .iter()
            .map(|(f, n)| ((*f).to_owned(), (*n).to_owned()))
            .collect(),
        calls: Cell::new(0),
    }
}

fn orphan(file: &str, name: &str) -> String {
    format!("{file}: traced test `{name}` is not cited by any matrix row")
}

#[test]
fn lib_tool_010_orphan_problems_are_sorted_by_file_then_function() {
    let m = matrix(&row(ROW, "planned", ""));
    let repo = shuffled_repo(&m);
    assert_eq!(
        check(&m, &repo),
        [
            orphan("a.rs", "lib_tool_001"),
            orphan("a.rs", "lib_tool_001_a"),
            orphan("a.rs", "lib_tool_001_z"),
            orphan("a.rs.rs", "lib_tool_001_a"),
            orphan("b.rs", "lib_tool_001_a"),
            orphan("b.rs", "lib_tool_001_b"),
        ],
        "LIB-TOOL-010: orphan problems are sorted by file and then by function name, \
         whatever order the repository lists them in"
    );
}

#[test]
fn lib_tool_010_the_report_does_not_depend_on_listing_order() {
    let m = matrix(&format!(
        "{}{}",
        row(ROW, "planned", "depends_on = [\"LIB-TOOL-998\"]"),
        row(OTHER, "planned", "depends_on = [\"LIB-TOOL-997\"]")
    ));
    let repo = shuffled_repo(&m);
    let runs: Vec<Vec<String>> = (0..4).map(|_| check(&m, &repo)).collect();
    for later in &runs[1..] {
        assert_eq!(
            later, &runs[0],
            "LIB-TOOL-010: every check of the same repository returns the same problems in the \
             same order"
        );
    }

    let tree = failing_tree("hardening-twice");
    let first = run_check(tree.root());
    let second = run_check(tree.root());
    assert!(first.0.is_err(), "LIB-TOOL-010: the failing tree fails");
    assert_eq!(
        first, second,
        "LIB-TOOL-010: running `matrix check` twice gives identical output"
    );
}

#[test]
fn lib_tool_010_binary_prints_every_problem_on_stderr_and_fails() {
    let tree = failing_tree("hardening-binary");
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["matrix", "check"])
        .env(ROOT_ENV, tree.root())
        .output()
        .expect("LIB-TOOL-010: the xtask binary can be started");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "LIB-TOOL-010: a failing `matrix check` exits with a failure status, got {:?}",
        output.status
    );
    for problem in FAILING_TREE_PROBLEMS {
        assert!(
            stderr.lines().any(|l| l == format!("  {problem}")),
            "LIB-TOOL-010: {problem:?} appears on stderr, got {stderr:?}"
        );
    }
    assert_eq!(
        stderr,
        format!("xtask: {}\n", expected_error(&FAILING_TREE_PROBLEMS)),
        "LIB-TOOL-010: stderr is the whole error once, with the `xtask: ` prefix"
    );
    assert!(
        stdout.is_empty(),
        "LIB-TOOL-010: a failing check prints nothing on stdout, got {stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// The real repository
// ---------------------------------------------------------------------------

/// The real matrix.
fn real_matrix() -> Matrix {
    let text = std::fs::read_to_string(xtask::repo_root().join(MATRIX_PATH))
        .expect("LIB-TOOL-005: the real matrix can be read");
    load(&text).expect("LIB-TOOL-005: the real matrix parses")
}

/// The problems the gate finds in the real repository.
fn real_problems() -> Vec<String> {
    check(&real_matrix(), &FsRepo::new(&xtask::repo_root()))
}

#[test]
fn lib_tool_005_real_matrix_passes_the_gate() {
    let (result, out) = run_check(&xtask::repo_root());
    assert_eq!(
        result,
        Ok(()),
        "LIB-TOOL-005: `matrix check` passes on the real repository"
    );
    assert!(
        out.starts_with("functionality matrix OK: "),
        "LIB-TOOL-005: the real repository passes with the OK line, wrote {out:?}"
    );
}

/// `(line number, id, verdict)` for every line of `text` that starts with
/// `- Row verdict:` outside a fence, after trailing spaces and tabs are
/// removed. A line that does not split into an id and a verdict has empty
/// parts.
fn verdict_lines(text: &str) -> Vec<(usize, String, String)> {
    let mut fenced = false;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.starts_with("```") || line.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        let line = line.trim_end_matches([' ', '\t']);
        if fenced || !line.starts_with("- Row verdict:") {
            continue;
        }
        let (id, verdict) = line
            .strip_prefix("- Row verdict: ")
            .and_then(|rest| rest.split_once(": "))
            .unwrap_or_default();
        out.push((i + 1, id.to_owned(), verdict.to_owned()));
    }
    out
}

#[test]
fn lib_tool_005_every_verified_row_in_the_real_matrix_has_one_pass_verdict_line() {
    let root = xtask::repo_root();
    let mut failures = Vec::new();
    for row in real_matrix()
        .rows
        .iter()
        .filter(|r| r.status == Status::Verified)
    {
        let Some(record) = &row.verification else {
            failures.push(format!("{} cites no record", row.id));
            continue;
        };
        let Ok(text) = std::fs::read_to_string(root.join(record)) else {
            failures.push(format!("{}: {record} cannot be read", row.id));
            continue;
        };
        let lines = verdict_lines(&text);
        for (n, id, verdict) in &lines {
            if id.is_empty() || !["PASS", "FAIL", "OPEN"].contains(&verdict.as_str()) {
                failures.push(format!("{record}:{n} is a malformed verdict line"));
            }
        }
        let own: Vec<&String> = lines
            .iter()
            .filter(|(_, id, _)| *id == row.id)
            .map(|(_, _, v)| v)
            .collect();
        if own != ["PASS"] {
            failures.push(format!("{}: {record} has the verdicts {own:?}", row.id));
        }
    }
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: every verified row's record has exactly one PASS line for it:\n\
         {failures:#?}"
    );
    let gate: Vec<String> = real_problems()
        .into_iter()
        .filter(|p| p.contains("docs/verification/"))
        .collect();
    assert!(
        gate.is_empty(),
        "LIB-TOOL-005: the gate finds no problem with the real verification records: {gate:#?}"
    );
}

/// Whether `path` is a relative, `/`-separated path inside the repository
/// (decision D2 of slice 0004).
fn is_inside(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains(['\\', ':'])
        && path
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
}

#[test]
fn lib_tool_006_every_path_cited_in_the_real_matrix_is_inside_the_repository() {
    let mut outside = Vec::new();
    for row in &real_matrix().rows {
        let tests = row.tests.iter().filter_map(|t| {
            t.strip_prefix("doctest:")
                .or_else(|| t.split_once("::").map(|(p, _)| p))
        });
        for path in row
            .code
            .iter()
            .map(String::as_str)
            .chain(tests)
            .chain(row.verification.as_deref())
        {
            if !is_inside(path) {
                outside.push(format!("{}: {path}", row.id));
            }
        }
    }
    assert!(
        outside.is_empty(),
        "LIB-TOOL-006: every cited path in the real matrix is relative and inside: {outside:#?}"
    );
    let gate: Vec<String> = real_problems()
        .into_iter()
        .filter(|p| p.contains("must be a relative path inside the repository"))
        .collect();
    assert!(
        gate.is_empty(),
        "LIB-TOOL-006: the gate rejects no path of the real matrix: {gate:#?}"
    );
}

#[test]
fn lib_tool_009_every_area_in_the_real_matrix_has_an_uppercase_ascii_code() {
    let m = real_matrix();
    let bad: Vec<&str> = m
        .areas
        .iter()
        .map(|a| a.code.as_str())
        .filter(|c| c.is_empty() || !c.chars().all(|ch| ch.is_ascii_uppercase()))
        .collect();
    assert!(
        bad.is_empty(),
        "LIB-TOOL-009: every real area code is uppercase ASCII letters: {bad:?}"
    );
    let gate: Vec<String> = real_problems()
        .into_iter()
        .filter(|p| p.starts_with("area code"))
        .collect();
    assert!(
        gate.is_empty(),
        "LIB-TOOL-009: the gate rejects no area of the real matrix: {gate:#?}"
    );
}
