//! Acceptance tests for the functionality-matrix gate (LIB-TOOL-001).
//!
//! The gate is driven through the public `xtask::matrix` API against an
//! in-memory repository, so every rule of `docs/matrix/README.md` is pinned by
//! a test that fails when that rule is removed.
//!
//! Fixture source text is written on one line with `\n` escapes on purpose: a
//! line of this file that began with `#[test]` inside a string literal would
//! be picked up by the real repository's orphan scan.

mod common;

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use common::{GATE_TOML, TempTree, consistent_tree};
use xtask::matrix::{
    FsRepo, Matrix, RENDERED_PATH, Repo, check, cmd_check, ignored_traced_tests, load, ready,
    render, traced_tests,
};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// An in-memory repository: relative path to file text.
#[derive(Debug, Default)]
struct FakeRepo {
    files: BTreeMap<String, String>,
}

impl FakeRepo {
    fn with(mut self, path: &str, text: &str) -> Self {
        self.files.insert(path.into(), text.into());
        self
    }
}

impl Repo for FakeRepo {
    fn exists(&self, rel: &str) -> bool {
        self.files.contains_key(rel)
    }

    fn read(&self, rel: &str) -> Option<String> {
        self.files.get(rel).cloned()
    }

    fn traced_functions(&self) -> Vec<(String, String)> {
        self.scan(traced_tests)
    }

    fn ignored_functions(&self) -> Vec<(String, String)> {
        self.scan(ignored_traced_tests)
    }
}

impl FakeRepo {
    /// `(file, name)` for every name `find` returns for a file, read as
    /// Python when its extension is `.py`.
    fn scan(&self, find: fn(&str, bool) -> Vec<String>) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (path, text) in &self.files {
            let python = Path::new(path)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("py"));
            for name in find(text, python) {
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
[[milestone]]
id = "M1"
title = "Core"
goal = "Model."
[[area]]
code = "BASE"
prefix = "SACM23"
title = "Base"
[[area]]
code = "CLI"
prefix = "LIB"
title = "CLI"
"#;

fn matrix(rows: &str) -> Matrix {
    load(&format!("{HEADER}{rows}")).expect("LIB-TOOL-001: test matrix parses")
}

/// Check with the rendered page in sync, so tests see only the rule under test.
fn problems(m: &Matrix, repo: FakeRepo) -> Vec<String> {
    let repo = repo.with(RENDERED_PATH, &render(m));
    check(m, &repo)
}

fn assert_problem(problems: &[String], needle: &str) {
    assert!(
        problems.iter().any(|p| p.contains(needle)),
        "LIB-TOOL-001: expected a problem containing {needle:?}, got {problems:#?}"
    );
}

/// The exact message `expected` is among the problems.
fn assert_exact(problems: &[String], expected: &str) {
    assert!(
        problems.iter().any(|p| p == expected),
        "LIB-TOOL-001: expected the problem {expected:?}, got {problems:#?}"
    );
}

fn assert_no_problem_containing(problems: &[String], needle: &str) {
    assert!(
        !problems.iter().any(|p| p.contains(needle)),
        "LIB-TOOL-001: no problem should contain {needle:?}, got {problems:#?}"
    );
}

/// A row in milestone M1 whose area is the id's area segment. `extra` holds
/// further TOML keys (one per line).
fn row(id: &str, status: &str, extra: &str) -> String {
    let area = id.split('-').nth(1).unwrap_or_default();
    format!(
        "\n[[row]]\nid = \"{id}\"\narea = \"{area}\"\nmilestone = \"M1\"\ntitle = \"Title\"\n\
         requirement = \"Requirement.\"\nsource = \"Source\"\nkind = \"model\"\n\
         status = \"{status}\"\n{extra}\n"
    )
}

const PLANNED: &str = r#"
[[row]]
id = "SACM23-BASE-001"
area = "BASE"
milestone = "M1"
title = "LangString"
requirement = "Represent LangString."
source = "§8.3"
kind = "model"
status = "planned"
"#;

/// A traced test for SACM23-BASE-001, marked as a test.
const MARKED_TEST: &str = "#[test]\nfn sacm23_base_001_x() {}";

// ---------------------------------------------------------------------------
// Ported from the inline tests in xtask/src/matrix.rs
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_001_accepts_a_consistent_matrix() {
    assert_eq!(
        problems(&matrix(PLANNED), FakeRepo::default()),
        Vec::<String>::new(),
        "LIB-TOOL-001: a consistent matrix has no problems"
    );
}

#[test]
fn lib_tool_001_rejects_malformed_and_mismatched_ids() {
    let m = matrix(&PLANNED.replace("SACM23-BASE-001", "LIB-BASE-1"));
    let p = problems(&m, FakeRepo::default());
    assert_problem(&p, "id must look like");

    let m = matrix(&PLANNED.replace("SACM23-BASE-001", "LIB-BASE-001"));
    assert_problem(
        &problems(&m, FakeRepo::default()),
        "id prefix must be SACM23",
    );
}

#[test]
fn lib_tool_001_rejects_duplicate_ids() {
    let m = matrix(&format!("{PLANNED}{PLANNED}"));
    assert_problem(&problems(&m, FakeRepo::default()), "duplicate row id");
}

#[test]
fn lib_tool_001_planned_rows_must_not_cite_tests() {
    let row = PLANNED.replace(
        "status = \"planned\"",
        "status = \"planned\"\ntests = [\"crates/x/tests/t.rs::sacm23_base_001_x\"]",
    );
    let repo = FakeRepo::default().with("crates/x/tests/t.rs", MARKED_TEST);
    assert_problem(&problems(&matrix(&row), repo), "fallen behind reality");
}

#[test]
fn lib_tool_001_implemented_rows_need_acceptance_code_and_tests() {
    let row = PLANNED.replace("status = \"planned\"", "status = \"implemented\"");
    let p = problems(&matrix(&row), FakeRepo::default());
    assert_problem(&p, "no acceptance criteria");
    assert_problem(&p, "cites no tests");
    assert_problem(&p, "cites no code");
}

#[test]
fn lib_tool_001_test_citations_must_resolve_and_trace() {
    let row = PLANNED.replace(
        "status = \"planned\"",
        "status = \"tests-failing\"\nacceptance = [\"a\"]\n\
         tests = [\"crates/x/tests/t.rs::sacm23_base_001_ok\", \"crates/x/tests/t.rs::sacm23_base_001_missing\", \
         \"crates/x/tests/t.rs::lib_cli_001_wrong_row\"]",
    );
    let repo = FakeRepo::default().with(
        "crates/x/tests/t.rs",
        "#[test]\nfn sacm23_base_001_ok() {}\n#[test]\nfn lib_cli_001_wrong_row() {}",
    );
    let p = problems(&matrix(&row), repo);
    assert_problem(&p, "`sacm23_base_001_missing` is not defined");
    assert_problem(&p, "must be named `sacm23_base_001_...`");
    assert_no_problem_containing(&p, "sacm23_base_001_ok");
}

#[test]
fn lib_tool_001_verified_rows_need_record_and_verified_dependencies() {
    let rows = format!(
        "{PLANNED}{}",
        r#"
[[row]]
id = "SACM23-BASE-002"
area = "BASE"
milestone = "M1"
title = "MultiLangString"
requirement = "Represent MultiLangString."
source = "§8.5"
kind = "model"
status = "verified"
depends_on = ["SACM23-BASE-001"]
acceptance = ["a"]
code = ["src/lib.rs"]
tests = ["crates/x/tests/t.rs::sacm23_base_002_x"]
verification = "docs/verification/v.md"
"#
    );
    let repo = FakeRepo::default()
        .with("src/lib.rs", "")
        .with("crates/x/tests/t.rs", "#[test]\nfn sacm23_base_002_x() {}")
        .with("docs/verification/v.md", "mentions nothing");
    let p = problems(&matrix(&rows), repo);
    assert_problem(&p, "depends on SACM23-BASE-001, which is planned");
    // The record gives no verdict for the row. The wording of that problem is
    // LIB-TOOL-005's; here it is enough that the row and the record are named.
    assert!(
        p.iter()
            .any(|m| m.starts_with("SACM23-BASE-002: ") && m.contains("docs/verification/v.md")),
        "LIB-TOOL-001: a record without a verdict for the row is a problem of that row naming \
         the record, got {p:#?}"
    );
}

#[test]
fn lib_tool_001_rejects_cycles_and_backward_milestone_dependencies() {
    let a = PLANNED.replace(
        "status = \"planned\"",
        "status = \"planned\"\ndepends_on = [\"SACM23-BASE-002\"]",
    );
    let b = PLANNED
        .replace("SACM23-BASE-001", "SACM23-BASE-002")
        .replace(
            "status = \"planned\"",
            "status = \"planned\"\ndepends_on = [\"SACM23-BASE-001\"]",
        );
    let p = problems(&matrix(&format!("{a}{b}")), FakeRepo::default());
    assert_problem(&p, "dependency cycle");
    assert_exact(
        &p,
        "dependency cycle: SACM23-BASE-001 -> SACM23-BASE-002 -> SACM23-BASE-001",
    );

    let early = PLANNED
        .replace("milestone = \"M1\"", "milestone = \"M0\"")
        .replace(
            "status = \"planned\"",
            "status = \"planned\"\ndepends_on = [\"SACM23-BASE-002\"]",
        );
    let late = PLANNED.replace("SACM23-BASE-001", "SACM23-BASE-002");
    let p = problems(&matrix(&format!("{early}{late}")), FakeRepo::default());
    assert_problem(&p, "from a later milestone");
}

#[test]
fn lib_tool_001_cycle_message_names_only_the_rows_on_the_cycle() {
    // 001 leads into the cycle 002 -> 003 -> 002 but is not on it. The walk
    // starts at 001, so the cycle is found with 001 at the bottom of the
    // path: the message must start where the cycle starts, at 002.
    let rows = [
        row(
            "SACM23-BASE-001",
            "planned",
            "depends_on = [\"SACM23-BASE-002\"]",
        ),
        row(
            "SACM23-BASE-002",
            "planned",
            "depends_on = [\"SACM23-BASE-003\"]",
        ),
        row(
            "SACM23-BASE-003",
            "planned",
            "depends_on = [\"SACM23-BASE-002\"]",
        ),
    ]
    .concat();
    let p = problems(&matrix(&rows), FakeRepo::default());
    let cycles: Vec<&String> = p
        .iter()
        .filter(|m| m.starts_with("dependency cycle"))
        .collect();
    assert_eq!(
        cycles,
        ["dependency cycle: SACM23-BASE-002 -> SACM23-BASE-003 -> SACM23-BASE-002"],
        "LIB-TOOL-001: one cycle is reported, as the path from the row where it \
         starts back to that row; all problems: {p:#?}"
    );
}

#[test]
fn lib_tool_001_unscheduled_rows_must_give_a_reason() {
    let row = PLANNED.replace("status = \"planned\"", "status = \"out-of-scope\"");
    assert_problem(
        &problems(&matrix(&row), FakeRepo::default()),
        "must say why",
    );
}

/// Whitespace is not a reason: `notes = "  "` counts as missing.
#[test]
fn lib_tool_001_whitespace_notes_count_as_missing() {
    for status in ["out-of-scope", "candidate", "deferred"] {
        let m = matrix(&row("SACM23-BASE-001", status, "notes = \"  \""));
        assert_exact(
            &problems(&m, FakeRepo::default()),
            &format!("SACM23-BASE-001: is {status} and must say why in `notes`"),
        );
    }
}

#[test]
fn lib_tool_001_reports_orphaned_traced_tests() {
    let repo = FakeRepo::default().with("t.rs", "#[test]\nfn sacm23_base_001_uncited() {}");
    assert_problem(
        &problems(&matrix(PLANNED), repo),
        "not cited by any matrix row",
    );
}

#[test]
fn lib_tool_001_detects_stale_rendered_page() {
    let m = matrix(PLANNED);
    let repo = FakeRepo::default().with(RENDERED_PATH, "stale");
    assert_problem(&check(&m, &repo), "out of date");
}

#[test]
fn lib_tool_001_next_lists_open_rows_with_verified_dependencies_in_order() {
    let blocked = PLANNED
        .replace("SACM23-BASE-001", "SACM23-BASE-003")
        .replace(
            "status = \"planned\"",
            "status = \"planned\"\ndepends_on = [\"SACM23-BASE-001\"]",
        );
    let early = PLANNED
        .replace("SACM23-BASE-001", "LIB-CLI-001")
        .replace("area = \"BASE\"", "area = \"CLI\"")
        .replace("milestone = \"M1\"", "milestone = \"M0\"");
    let m = matrix(&format!("{PLANNED}{blocked}{early}"));
    let ids: Vec<&str> = ready(&m).iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        ["LIB-CLI-001", "SACM23-BASE-001"],
        "LIB-TOOL-001: M0 first, blocked row hidden"
    );
}

/// Ported: the inline version called the private `defined_functions` and
/// `looks_traced`; through the public API the same facts are observable via
/// `traced_tests`.
#[test]
fn lib_tool_001_finds_function_definitions_not_mentions() {
    // Joined from parts so that no line of this file starts with an attribute.
    let rust = [
        "#[test]",
        "fn lib_cli_001_a() {}",
        "// call lib_cli_001_b()",
        "#[test]",
        "fn helper() {}",
        "#[test]",
        "fn sacm23_base_1_x() {}",
        "#[test]",
        "fn sacm23_base_001_x() {}",
    ]
    .join("\n");
    let rust = rust.as_str();
    assert_eq!(
        traced_tests(rust, false),
        ["lib_cli_001_a", "sacm23_base_001_x"],
        "LIB-TOOL-001: only defined test functions with a row-id name are traced"
    );
    let python = "def test_lib_tool_004_x(self):\ndef test_helper(self):\n\
                  # def test_lib_tool_004_comment(self):\ndef lib_cli_001_c(x):";
    assert_eq!(
        traced_tests(python, true),
        ["test_lib_tool_004_x"],
        "LIB-TOOL-001: Python traced tests are `def test_<row id>...`"
    );
}

#[test]
fn lib_tool_001_only_marked_tests_count_as_traced() {
    // `#[should_panic]` is an attribute between `#[test]` and the function
    // that does not stop it running. Until slice 0006 this fixture used
    // `#[ignore]`, which now makes the test ignored (LIB-TOOL-011); that
    // fixture is pinned in `scanner.rs`.
    let rust = "#[test]\n#[should_panic]\nfn lib_cli_001_a() {}\nfn lib_cli_001_helper() {}\n\
                let s = \"#[test] fn lib_cli_001_in_string() {}\";";
    assert_eq!(
        traced_tests(rust, false),
        ["lib_cli_001_a"],
        "LIB-TOOL-001: unmarked helpers and string contents are not tests"
    );
    let python = "def test_lib_tool_004_denies(self):\ndef lib_tool_004_helper():";
    assert_eq!(
        traced_tests(python, true),
        ["test_lib_tool_004_denies"],
        "LIB-TOOL-001: Python helpers are not tests"
    );
}

#[test]
fn lib_tool_001_only_test_style_attributes_mark_a_test() {
    for text in [
        "#[allow(dead_code)]\nfn sacm23_base_001_x() {}",
        "#[inline] fn sacm23_base_001_x() {}",
        "#[cfg(test)]\nfn sacm23_base_001_x() {}",
    ] {
        assert_eq!(
            traced_tests(text, false),
            Vec::<String>::new(),
            "LIB-TOOL-001: {text:?} is not a test"
        );
    }
    assert_eq!(
        traced_tests("#[tokio::test]\nasync fn sacm23_base_001_x() {}", false),
        ["sacm23_base_001_x"],
        "LIB-TOOL-001: a `#[test]`-style attribute such as `#[tokio::test]` marks a test"
    );
}

#[test]
fn lib_tool_001_brackets_nested_in_an_attribute_do_not_close_it() {
    // An attribute ends at the `]` that matches its `#[`, not at the first
    // `]`: brackets inside it nest. Cut short, the attribute would leave its
    // tail on the line, hiding the attributes and the function after it.
    for text in [
        "#[doc = \"[x]\"] #[test] fn lib_cli_001_x() {}",
        "#[cfg_attr(all(), doc = \"[[x]]\")] #[test] fn lib_cli_001_x() {}",
        "#[test] #[doc = \"[x]\"] fn lib_cli_001_x() {}",
        "#[test]\n#[doc = \"[x]\"]\nfn lib_cli_001_x() {}",
    ] {
        assert_eq!(
            traced_tests(text, false),
            ["lib_cli_001_x"],
            "LIB-TOOL-001: {text:?} is a test: the nested `[x]` does not close the attribute"
        );
    }
    // Here only the text up to the first `]` ends in `test`; the attribute
    // as a whole is a `doc` attribute and marks nothing.
    for text in [
        "#[doc = \"[test]\"] fn lib_cli_001_x() {}",
        "#[doc = \"[test]\"]\nfn lib_cli_001_x() {}",
    ] {
        assert_eq!(
            traced_tests(text, false),
            Vec::<String>::new(),
            "LIB-TOOL-001: {text:?} is not a test: `[test]` sits inside a doc attribute"
        );
    }
}

// ---------------------------------------------------------------------------
// Gate rule 1: schema version, unique milestones and areas
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_001_rejects_unsupported_schema_version() {
    let text = format!(
        "{}{PLANNED}",
        HEADER.replace("schema_version = 1", "schema_version = 2")
    );
    let m = load(&text).expect("LIB-TOOL-001: matrix with schema_version 2 parses");
    assert_exact(
        &problems(&m, FakeRepo::default()),
        "schema_version 2 is not supported (expected 1)",
    );
}

#[test]
fn lib_tool_001_rejects_duplicate_milestone_ids() {
    let header = HEADER.replace(
        "[[area]]\ncode = \"BASE\"",
        "[[milestone]]\nid = \"M1\"\ntitle = \"Again\"\ngoal = \"Twice.\"\n[[area]]\ncode = \"BASE\"",
    );
    let m = load(&format!("{header}{PLANNED}")).expect("LIB-TOOL-001: parses");
    assert_eq!(
        m.milestones.len(),
        3,
        "LIB-TOOL-001: fixture has 3 milestones"
    );
    assert_exact(
        &problems(&m, FakeRepo::default()),
        "milestone ids must be unique",
    );
}

#[test]
fn lib_tool_001_rejects_duplicate_area_codes() {
    let header =
        format!("{HEADER}[[area]]\ncode = \"BASE\"\nprefix = \"SACM23\"\ntitle = \"Again\"\n");
    let m = load(&format!("{header}{PLANNED}")).expect("LIB-TOOL-001: parses");
    assert_eq!(m.areas.len(), 3, "LIB-TOOL-001: fixture has 3 areas");
    assert_exact(
        &problems(&m, FakeRepo::default()),
        "area codes must be unique",
    );
}

// ---------------------------------------------------------------------------
// Gate rules 2 and 3: identity and required text
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_001_rejects_unknown_area() {
    let m = matrix(&row("SACM23-XYZ-001", "planned", ""));
    assert_exact(
        &problems(&m, FakeRepo::default()),
        "SACM23-XYZ-001: unknown area `XYZ`",
    );
}

#[test]
fn lib_tool_001_rejects_unknown_milestone() {
    let m = matrix(
        &row("SACM23-BASE-001", "planned", "").replace("milestone = \"M1\"", "milestone = \"M9\""),
    );
    assert_exact(
        &problems(&m, FakeRepo::default()),
        "SACM23-BASE-001: unknown milestone `M9`",
    );
}

#[test]
fn lib_tool_001_rejects_id_area_segment_that_differs_from_area_field() {
    let m =
        matrix(&row("SACM23-ARG-001", "planned", "").replace("area = \"ARG\"", "area = \"BASE\""));
    assert_exact(
        &problems(&m, FakeRepo::default()),
        "SACM23-ARG-001: id area `ARG` does not match area field `BASE`",
    );
}

/// Rule 2: an id is exactly three segments, the last exactly three digits.
#[test]
fn lib_tool_001_rejects_ids_with_an_extra_segment_or_a_non_digit_number() {
    for id in ["SACM23-BASE-001-X", "SACM23-BASE-0A1"] {
        let m = matrix(&row(id, "planned", ""));
        assert_exact(
            &problems(&m, FakeRepo::default()),
            &format!("{id}: id must look like SACM23-<AREA>-<NNN> or LIB-<AREA>-<NNN>"),
        );
    }
}

/// Rule 2: the area segment of an id is not empty. The fixture declares an
/// area whose code is empty, so the only rule `SACM23--001` can break is the
/// id form itself: it must not slip through, and must not be reported merely
/// as an unknown area.
#[test]
fn lib_tool_001_rejects_id_with_an_empty_area_segment() {
    let empty_area = "\n[[area]]\ncode = \"\"\nprefix = \"SACM23\"\ntitle = \"Empty\"\n";
    let id = "SACM23--001";
    let m = load(&format!("{HEADER}{empty_area}{}", row(id, "planned", "")))
        .expect("LIB-TOOL-001: test matrix with an empty area code parses");
    let p = problems(&m, FakeRepo::default());
    // The empty area code is itself a problem under LIB-TOOL-009; only the
    // row's problems are this test's concern.
    let (row_problems, others): (Vec<&String>, Vec<&String>) =
        p.iter().partition(|m| m.starts_with(&format!("{id}: ")));
    assert_eq!(
        row_problems,
        [&format!(
            "{id}: id must look like SACM23-<AREA>-<NNN> or LIB-<AREA>-<NNN>"
        )],
        "LIB-TOOL-001: an id with an empty area segment is rejected by the id-format rule alone"
    );
    assert!(
        others.iter().all(|m| m.starts_with("area code ``")),
        "LIB-TOOL-001: the only other problem may be the empty area code, got {p:#?}"
    );
}

#[test]
fn lib_tool_001_rejects_empty_title_requirement_and_source() {
    let base = row("SACM23-BASE-001", "planned", "");
    for (field, original) in [
        ("title", "title = \"Title\""),
        ("requirement", "requirement = \"Requirement.\""),
        ("source", "source = \"Source\""),
    ] {
        for blank in ["", "   "] {
            let m = matrix(&base.replace(original, &format!("{field} = \"{blank}\"")));
            assert_exact(
                &problems(&m, FakeRepo::default()),
                &format!("SACM23-BASE-001: `{field}` must not be empty"),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Gate rule 8: dependencies
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_001_rejects_unknown_dependency() {
    let m = matrix(&row(
        "SACM23-BASE-001",
        "planned",
        "depends_on = [\"SACM23-BASE-009\"]",
    ));
    assert_exact(
        &problems(&m, FakeRepo::default()),
        "SACM23-BASE-001: depends on unknown row SACM23-BASE-009",
    );
}

#[test]
fn lib_tool_001_rejects_self_dependency() {
    let m = matrix(&row(
        "SACM23-BASE-001",
        "planned",
        "depends_on = [\"SACM23-BASE-001\"]",
    ));
    assert_exact(
        &problems(&m, FakeRepo::default()),
        "SACM23-BASE-001: depends on itself",
    );
}

#[test]
fn lib_tool_001_rejects_dependency_on_candidate_and_out_of_scope_rows() {
    for status in ["candidate", "out-of-scope"] {
        let rows = format!(
            "{}{}",
            row(
                "SACM23-BASE-001",
                "planned",
                "depends_on = [\"SACM23-BASE-002\"]"
            ),
            row("SACM23-BASE-002", status, "notes = \"why\""),
        );
        assert_exact(
            &problems(&matrix(&rows), FakeRepo::default()),
            &format!("SACM23-BASE-001: depends on SACM23-BASE-002, which is {status}"),
        );
    }
    // Deferred rows are still scheduled: depending on one is allowed.
    let rows = format!(
        "{}{}",
        row(
            "SACM23-BASE-001",
            "planned",
            "depends_on = [\"SACM23-BASE-002\"]"
        ),
        row("SACM23-BASE-002", "deferred", "notes = \"why\""),
    );
    assert_eq!(
        problems(&matrix(&rows), FakeRepo::default()),
        Vec::<String>::new(),
        "LIB-TOOL-001: a dependency on a deferred row is allowed"
    );
}

// ---------------------------------------------------------------------------
// Gate rules 4 and 5: the status table and verification records
// ---------------------------------------------------------------------------

/// One evidence column of the status table in docs/matrix/README.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Evidence {
    Acceptance,
    Tests,
    Code,
    Verification,
    Notes,
}

/// `(status, required evidence, forbidden evidence)`, transcribed from the
/// status table in docs/matrix/README.md. Every row of that table is here,
/// cell for cell; `candidate` forbids tests, code and a verification record.
const STATUS_TABLE: &[(&str, &[Evidence], &[Evidence])] = {
    use Evidence::{Acceptance, Code, Notes, Tests, Verification};
    &[
        ("planned", &[], &[Tests, Code, Verification]),
        ("analyzed", &[Acceptance], &[Tests, Code, Verification]),
        ("tests-failing", &[Acceptance, Tests], &[Verification]),
        ("implemented", &[Acceptance, Tests, Code], &[Verification]),
        ("verified", &[Acceptance, Tests, Code, Verification], &[]),
        ("candidate", &[Notes], &[Tests, Code, Verification]),
        ("deferred", &[Notes], &[Tests, Verification]),
        ("out-of-scope", &[Notes], &[Tests, Verification]),
    ]
};

const RECORD: &str = "docs/verification/v.md";

/// SACM23-BASE-001 with `status` and exactly the given evidence, plus a
/// repository in which every cited path exists and resolves.
fn evidence_case(status: &str, evidence: &[Evidence]) -> (Matrix, FakeRepo) {
    let mut extra = String::new();
    let mut repo = FakeRepo::default();
    for item in evidence {
        match item {
            Evidence::Acceptance => extra.push_str("acceptance = [\"a\"]\n"),
            Evidence::Tests => {
                extra.push_str("tests = [\"crates/x/tests/t.rs::sacm23_base_001_x\"]\n");
                repo = repo.with("crates/x/tests/t.rs", MARKED_TEST);
            }
            Evidence::Code => {
                extra.push_str("code = [\"src/lib.rs\"]\n");
                repo = repo.with("src/lib.rs", "");
            }
            Evidence::Verification => {
                extra.push_str("verification = \"");
                extra.push_str(RECORD);
                extra.push_str("\"\n");
                // A per-row verdict line (LIB-TOOL-005), which also mentions the id.
                repo = repo.with(RECORD, "- Row verdict: SACM23-BASE-001: PASS\n");
            }
            Evidence::Notes => extra.push_str("notes = \"why\"\n"),
        }
    }
    (matrix(&row("SACM23-BASE-001", status, &extra)), repo)
}

fn missing_message(status: &str, item: Evidence) -> String {
    let message = match item {
        Evidence::Acceptance => format!("is {status} but lists no acceptance criteria"),
        Evidence::Tests => format!("is {status} but cites no tests"),
        Evidence::Code => format!("is {status} but cites no code"),
        Evidence::Verification => "is verified but cites no verification record".to_owned(),
        Evidence::Notes => format!("is {status} and must say why in `notes`"),
    };
    format!("SACM23-BASE-001: {message}")
}

fn forbidden_message(status: &str, item: Evidence) -> String {
    let message = match item {
        Evidence::Tests => {
            format!("is {status} but cites tests; its status has fallen behind reality")
        }
        Evidence::Code => {
            format!("is {status} but cites code; its status has fallen behind reality")
        }
        Evidence::Verification => format!("is {status} but cites a verification record"),
        Evidence::Acceptance | Evidence::Notes => {
            unreachable!("acceptance and notes are never forbidden")
        }
    };
    format!("SACM23-BASE-001: {message}")
}

#[test]
fn lib_tool_001_status_table_rows_with_exactly_the_required_evidence_pass() {
    let mut failures = Vec::new();
    for (status, required, _) in STATUS_TABLE {
        let (m, repo) = evidence_case(status, required);
        let p = problems(&m, repo);
        if !p.is_empty() {
            failures.push(format!("{status} with {required:?}: {p:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "LIB-TOOL-001: rows carrying exactly their required evidence were rejected:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_001_status_table_missing_required_evidence_is_rejected() {
    let mut failures = Vec::new();
    for (status, required, _) in STATUS_TABLE {
        for &missing in *required {
            let evidence: Vec<Evidence> =
                required.iter().copied().filter(|e| *e != missing).collect();
            let (m, repo) = evidence_case(status, &evidence);
            let p = problems(&m, repo);
            let expected = missing_message(status, missing);
            if !p.contains(&expected) {
                failures.push(format!(
                    "{status} without {missing:?}: expected {expected:?}, got {p:?}"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "LIB-TOOL-001: required evidence not enforced:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_001_status_table_forbidden_evidence_is_rejected() {
    let mut failures = Vec::new();
    for (status, required, forbidden) in STATUS_TABLE {
        for &extra in *forbidden {
            let mut evidence = required.to_vec();
            evidence.push(extra);
            let (m, repo) = evidence_case(status, &evidence);
            let p = problems(&m, repo);
            let expected = forbidden_message(status, extra);
            if !p.contains(&expected) {
                failures.push(format!(
                    "{status} with {extra:?}: expected {expected:?}, got {p:?}"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "LIB-TOOL-001: forbidden evidence not rejected:\n{failures:#?}"
    );
}

/// Cells the README table neither requires nor forbids are accepted, so the
/// gate is not stricter than the documented rules.
#[test]
fn lib_tool_001_status_table_permitted_evidence_is_accepted() {
    use Evidence::{Acceptance, Code, Notes, Tests, Verification};
    let mut failures = Vec::new();
    for (status, required, forbidden) in STATUS_TABLE {
        for extra in [Acceptance, Tests, Code, Verification, Notes] {
            if required.contains(&extra) || forbidden.contains(&extra) {
                continue;
            }
            let mut evidence = required.to_vec();
            evidence.push(extra);
            let (m, repo) = evidence_case(status, &evidence);
            let p = problems(&m, repo);
            if !p.is_empty() {
                failures.push(format!("{status} with {extra:?}: {p:?}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "LIB-TOOL-001: evidence the status table permits was rejected:\n{failures:#?}"
    );
}

/// The README table forbids a verification record on every status except
/// `verified`; checked here status by status with the exact message.
#[test]
fn lib_tool_001_verification_record_on_a_non_verified_row_is_rejected() {
    for (status, required, _) in STATUS_TABLE {
        if *status == "verified" {
            continue;
        }
        let mut evidence = required.to_vec();
        evidence.push(Evidence::Verification);
        let (m, repo) = evidence_case(status, &evidence);
        assert_exact(
            &problems(&m, repo),
            &format!("SACM23-BASE-001: is {status} but cites a verification record"),
        );
    }
}

#[test]
fn lib_tool_001_verified_rows_must_cite_a_verification_record() {
    use Evidence::{Acceptance, Code, Tests};
    let (m, repo) = evidence_case("verified", &[Acceptance, Tests, Code]);
    assert_exact(
        &problems(&m, repo),
        "SACM23-BASE-001: is verified but cites no verification record",
    );
}

#[test]
fn lib_tool_001_verification_record_must_exist() {
    use Evidence::{Acceptance, Code, Tests, Verification};
    let (m, mut repo) = evidence_case("verified", &[Acceptance, Tests, Code, Verification]);
    repo.files.remove(RECORD);
    assert_exact(
        &problems(&m, repo),
        &format!("SACM23-BASE-001: verification record {RECORD} does not exist"),
    );
}

#[test]
fn lib_tool_001_analyzed_rows_need_acceptance_criteria() {
    let (m, repo) = evidence_case("analyzed", &[]);
    assert_exact(
        &problems(&m, repo),
        "SACM23-BASE-001: is analyzed but lists no acceptance criteria",
    );
}

// ---------------------------------------------------------------------------
// Gate rules 5 and 6: citations
// ---------------------------------------------------------------------------

/// A tests-failing SACM23-BASE-001 citing `tests` (TOML array items).
fn citing(tests: &str) -> Matrix {
    matrix(&row(
        "SACM23-BASE-001",
        "tests-failing",
        &format!("acceptance = [\"a\"]\ntests = [{tests}]"),
    ))
}

#[test]
fn lib_tool_001_rejects_missing_cited_code_path() {
    let m = matrix(&row(
        "SACM23-BASE-001",
        "implemented",
        "acceptance = [\"a\"]\ntests = [\"crates/x/tests/t.rs::sacm23_base_001_x\"]\ncode = [\"src/missing.rs\"]",
    ));
    let repo = FakeRepo::default().with("crates/x/tests/t.rs", MARKED_TEST);
    assert_exact(
        &problems(&m, repo),
        "SACM23-BASE-001: cited code path src/missing.rs does not exist",
    );
}

#[test]
fn lib_tool_001_rejects_test_citation_without_separator() {
    let m = citing("\"t.rs:sacm23_base_001_x\"");
    let repo = FakeRepo::default().with("t.rs", "");
    assert_exact(
        &problems(&m, repo),
        "SACM23-BASE-001: test citation `t.rs:sacm23_base_001_x` must be `path::function` or `doctest:path`",
    );
}

/// Rule 6: the row id must be followed by `_` or end the name, so
/// `sacm23_base_0012_x` does not trace to SACM23-BASE-001; the bare row id is
/// a valid name.
#[test]
fn lib_tool_001_test_name_must_separate_the_row_id_with_an_underscore() {
    let m = citing("\"crates/x/tests/t.rs::sacm23_base_0012_x\"");
    let repo =
        FakeRepo::default().with("crates/x/tests/t.rs", "#[test]\nfn sacm23_base_0012_x() {}");
    assert_eq!(
        problems(&m, repo),
        [
            "SACM23-BASE-001: test `sacm23_base_0012_x` must be named `sacm23_base_001_...` \
             to trace to this row"
        ],
        "LIB-TOOL-001: `sacm23_base_0012_x` does not trace to SACM23-BASE-001"
    );

    let m = citing("\"crates/x/tests/t.rs::sacm23_base_001\"");
    let repo = FakeRepo::default().with("crates/x/tests/t.rs", "#[test]\nfn sacm23_base_001() {}");
    assert_eq!(
        problems(&m, repo),
        Vec::<String>::new(),
        "LIB-TOOL-001: a test named exactly for the row id traces to it"
    );
}

/// Rule 6: unittest's `test_` prefix is not part of the trace, so a Python
/// citation `.claude/hooks/x.py::test_sacm23_base_001_y` is accepted.
#[test]
fn lib_tool_001_python_citation_with_unittest_prefix_is_accepted() {
    let m = citing("\".claude/hooks/x.py::test_sacm23_base_001_y\"");
    let repo = FakeRepo::default().with(
        ".claude/hooks/x.py",
        "def test_sacm23_base_001_y(self):\n    pass\n",
    );
    assert_eq!(
        problems(&m, repo),
        Vec::<String>::new(),
        "LIB-TOOL-001: `.claude/hooks/x.py::test_sacm23_base_001_y` is a valid citation for SACM23-BASE-001"
    );
}

#[test]
fn lib_tool_001_rejects_missing_test_file() {
    let m = citing("\"crates/x/tests/t.rs::sacm23_base_001_x\"");
    assert_exact(
        &problems(&m, FakeRepo::default()),
        "SACM23-BASE-001: test file crates/x/tests/t.rs does not exist",
    );
}

#[test]
fn lib_tool_001_rejects_missing_doctest_file() {
    let m = citing("\"doctest:src/lib.rs\"");
    assert_exact(
        &problems(&m, FakeRepo::default()),
        "SACM23-BASE-001: doctest file src/lib.rs does not exist",
    );
}

#[test]
fn lib_tool_001_rejects_doctest_file_without_code_block() {
    let m = citing("\"doctest:src/lib.rs\"");
    let repo = FakeRepo::default().with("src/lib.rs", "//! Docs without an example.\n");
    assert_exact(
        &problems(&m, repo),
        "SACM23-BASE-001: doctest file src/lib.rs contains no code block",
    );

    let m = citing("\"doctest:src/lib.rs\"");
    let repo = FakeRepo::default().with("src/lib.rs", "//! ```\n//! assert!(true);\n//! ```\n");
    assert_eq!(
        problems(&m, repo),
        Vec::<String>::new(),
        "LIB-TOOL-001: a doctest file with a code block is a valid citation"
    );
}

#[test]
fn lib_tool_001_citation_of_a_helper_without_test_attribute_is_rejected() {
    let m = citing("\"crates/x/tests/t.rs::sacm23_base_001_helper\"");
    let repo = FakeRepo::default().with("crates/x/tests/t.rs", "fn sacm23_base_001_helper() {}");
    let p = problems(&m, repo);
    assert_eq!(
        p.len(),
        1,
        "LIB-TOOL-001: citing a plain helper is exactly one problem, got {p:#?}"
    );
    assert!(
        p[0].starts_with("SACM23-BASE-001: ")
            && p[0].contains("`sacm23_base_001_helper`")
            && p[0].contains("t.rs"),
        "LIB-TOOL-001: the problem names the row, the function and the file, got {p:#?}"
    );

    // The same for a Python helper that lacks unittest's `test_` prefix.
    let m = citing("\".claude/hooks/h.py::sacm23_base_001_helper\"");
    let repo = FakeRepo::default().with(
        ".claude/hooks/h.py",
        "def sacm23_base_001_helper():\n    pass\n",
    );
    let p = problems(&m, repo);
    assert!(
        p.len() == 1 && p[0].contains("`sacm23_base_001_helper`"),
        "LIB-TOOL-001: citing a Python helper is rejected, got {p:#?}"
    );
}

#[test]
fn lib_tool_001_citation_of_a_name_in_a_comment_or_string_is_rejected() {
    for text in [
        "// fn sacm23_base_001_x() {}",
        "/// #[test] fn sacm23_base_001_x() {}",
        "const S: &str = \"#[test] fn sacm23_base_001_x() {}\";",
        "let s = \"fn sacm23_base_001_x() {}\";",
    ] {
        let repo = FakeRepo::default().with("crates/x/tests/t.rs", text);
        let p = problems(&citing("\"crates/x/tests/t.rs::sacm23_base_001_x\""), repo);
        assert!(
            p.iter()
                .any(|e| e.starts_with("SACM23-BASE-001: ") && e.contains("`sacm23_base_001_x`")),
            "LIB-TOOL-001: {text:?} does not define a test, so the citation must be rejected; got {p:#?}"
        );
    }
}

#[test]
fn lib_tool_001_same_line_test_attribute_counts_for_citation_and_orphans() {
    let same_line = "#[test] fn sacm23_base_001_x() {}";
    assert_eq!(
        traced_tests(same_line, false),
        ["sacm23_base_001_x"],
        "LIB-TOOL-001: `#[test] fn` on one line is a traced test"
    );
    assert_eq!(
        traced_tests("#[test] fn helper() {}", false),
        Vec::<String>::new(),
        "LIB-TOOL-001: a same-line test without a row-id name is not traced"
    );

    let repo = FakeRepo::default().with("crates/x/tests/t.rs", same_line);
    assert_eq!(
        problems(&citing("\"crates/x/tests/t.rs::sacm23_base_001_x\""), repo),
        Vec::<String>::new(),
        "LIB-TOOL-001: a same-line test is a valid citation"
    );

    let repo = FakeRepo::default().with("crates/x/tests/t.rs", same_line);
    assert_exact(
        &problems(&matrix(PLANNED), repo),
        "crates/x/tests/t.rs: traced test `sacm23_base_001_x` is not cited by any matrix row",
    );
}

/// The number segment of a traced name is three digits: `lib_cli_0a1_x` has
/// the right length but is not a row-id name, so it is neither traced nor
/// reported as an orphan.
#[test]
fn lib_tool_001_traced_names_need_a_three_digit_number() {
    let digits = "#[test] fn lib_cli_001_x() {}";
    let not_digits = "#[test] fn lib_cli_0a1_x() {}";
    assert_eq!(
        traced_tests(digits, false),
        ["lib_cli_001_x"],
        "LIB-TOOL-001: a name with a three-digit number is traced"
    );
    assert_eq!(
        traced_tests(not_digits, false),
        Vec::<String>::new(),
        "LIB-TOOL-001: a name whose number segment is not all digits is not traced"
    );
    assert_eq!(
        traced_tests("def test_lib_cli_0a1_x(self):", true),
        Vec::<String>::new(),
        "LIB-TOOL-001: a Python name whose number segment is not all digits is not traced"
    );

    let repo = FakeRepo::default().with("t.rs", not_digits);
    assert_eq!(
        problems(&matrix(PLANNED), repo),
        Vec::<String>::new(),
        "LIB-TOOL-001: a test whose name is not a row-id name is not an orphan"
    );
}

// ---------------------------------------------------------------------------
// Gate rule 9: the rendered page
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_001_detects_missing_rendered_page() {
    let p = check(&matrix(PLANNED), &FakeRepo::default());
    assert_exact(
        &p,
        &format!("{RENDERED_PATH} is missing; run `cargo xtask matrix render`"),
    );
    assert_no_problem_containing(&p, "out of date");
}

// ---------------------------------------------------------------------------
// render
// ---------------------------------------------------------------------------

/// The cells of a Markdown table line, trimmed.
fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(|c| c.trim().to_owned())
        .collect()
}

/// The first line that starts with `start`, as cells.
fn table_line(page: &str, start: &str) -> Vec<String> {
    let line = page
        .lines()
        .find(|l| l.starts_with(start))
        .unwrap_or_else(|| {
            panic!("LIB-TOOL-001: rendered page has no line starting {start:?}:\n{page}")
        });
    cells(line)
}

/// Status column -> count from the summary line starting with `start`; zero
/// counts may render blank.
fn summary_counts(page: &str, start: &str) -> BTreeMap<String, usize> {
    let header = table_line(page, "| Milestone |");
    let line = table_line(page, start);
    assert_eq!(
        header.len(),
        line.len(),
        "LIB-TOOL-001: summary line {start:?} has one cell per column"
    );
    header
        .iter()
        .zip(&line)
        .skip(1)
        .map(|(column, value)| {
            let value = value.trim_matches('*');
            let n = if value.is_empty() {
                0
            } else {
                value.parse().unwrap_or_else(|_| {
                    panic!("LIB-TOOL-001: summary cell {value:?} under {column:?} is not a count")
                })
            };
            (column.clone(), n)
        })
        .collect()
}

/// M0: one analyzed LIB row. M1: two planned rows (one with a `|` in its
/// notes) and a tests-failing row with a dependency and a cited test.
fn render_fixture() -> Matrix {
    let early = PLANNED
        .replace("SACM23-BASE-001", "LIB-CLI-001")
        .replace("area = \"BASE\"", "area = \"CLI\"")
        .replace("milestone = \"M1\"", "milestone = \"M0\"")
        .replace(
            "status = \"planned\"",
            "status = \"analyzed\"\nacceptance = [\"a\"]",
        );
    let second = PLANNED
        .replace("SACM23-BASE-001", "SACM23-BASE-002")
        .replace(
            "status = \"planned\"",
            "status = \"planned\"\nnotes = \"Pipes a | b are escaped.\"",
        );
    let failing = row(
        "SACM23-BASE-003",
        "tests-failing",
        "depends_on = [\"SACM23-BASE-001\"]\nacceptance = [\"a\"]\n\
         tests = [\"crates/x/tests/t.rs::sacm23_base_003_x\"]",
    );
    matrix(&format!("{early}{PLANNED}{second}{failing}"))
}

#[test]
fn lib_tool_001_render_reports_summary_counts() {
    let m = render_fixture();
    let page = render(&m);
    assert_eq!(page, render(&m), "LIB-TOOL-001: render is deterministic");

    let header = table_line(&page, "| Milestone |");
    for status in [
        "verified",
        "implemented",
        "tests-failing",
        "analyzed",
        "planned",
        "candidate",
        "deferred",
        "out-of-scope",
    ] {
        assert!(
            header.iter().any(|c| c == status),
            "LIB-TOOL-001: summary has a {status} column, got {header:?}"
        );
    }

    let count = |line: &str, column: &str| summary_counts(&page, line)[column];
    assert_eq!(
        count("| M0 — Foundation |", "Rows"),
        1,
        "LIB-TOOL-001: M0 rows"
    );
    assert_eq!(
        count("| M0 — Foundation |", "analyzed"),
        1,
        "LIB-TOOL-001: M0 analyzed"
    );
    assert_eq!(
        count("| M0 — Foundation |", "planned"),
        0,
        "LIB-TOOL-001: M0 planned"
    );
    assert_eq!(count("| M1 — Core |", "Rows"), 3, "LIB-TOOL-001: M1 rows");
    assert_eq!(
        count("| M1 — Core |", "planned"),
        2,
        "LIB-TOOL-001: M1 planned"
    );
    assert_eq!(
        count("| M1 — Core |", "tests-failing"),
        1,
        "LIB-TOOL-001: M1 tests-failing"
    );
    assert_eq!(
        count("| M1 — Core |", "verified"),
        0,
        "LIB-TOOL-001: M1 verified"
    );
    assert_eq!(
        count("| **Total** |", "Rows"),
        4,
        "LIB-TOOL-001: total rows"
    );
    assert_eq!(
        count("| **Total** |", "planned"),
        2,
        "LIB-TOOL-001: total planned"
    );
    assert_eq!(
        count("| **Total** |", "analyzed"),
        1,
        "LIB-TOOL-001: total analyzed"
    );
}

#[test]
fn lib_tool_001_render_lists_areas_milestones_and_row_lines() {
    let page = render(&render_fixture());

    let area = table_line(&page, "| `BASE` |");
    assert_eq!(
        area,
        ["`BASE`", "`SACM23`", "Base"],
        "LIB-TOOL-001: area line"
    );

    let m0 = page.find("## M0 — Foundation");
    let m1 = page.find("## M1 — Core");
    assert!(
        matches!((m0, m1), (Some(a), Some(b)) if a < b),
        "LIB-TOOL-001: milestone sections in milestone order"
    );

    let line = table_line(&page, "| SACM23-BASE-003 |");
    assert!(
        line[1].contains("Title") && line[1].contains("Requirement."),
        "LIB-TOOL-001: requirement cell carries title and requirement, got {line:?}"
    );
    assert_eq!(line[2], "model", "LIB-TOOL-001: kind cell");
    assert_eq!(line[3], "Source", "LIB-TOOL-001: source cell");
    assert_eq!(line[4], "`tests-failing`", "LIB-TOOL-001: status cell");
    assert_eq!(line[5], "SACM23-BASE-001", "LIB-TOOL-001: depends-on cell");
    assert_eq!(
        line.last().map(String::as_str),
        Some("`sacm23_base_003_x`"),
        "LIB-TOOL-001: tests cell"
    );
    assert!(
        page.contains("Pipes a \\| b are escaped."),
        "LIB-TOOL-001: a `|` in row text is escaped so the table stays intact"
    );
}

#[test]
fn lib_tool_001_render_lists_each_row_under_its_own_milestone_only() {
    let page = render(&render_fixture());
    let m0 = page
        .find("## M0 — Foundation")
        .expect("LIB-TOOL-001: the page has an M0 section");
    let m1 = page
        .find("## M1 — Core")
        .expect("LIB-TOOL-001: the page has an M1 section");
    assert!(m0 < m1, "LIB-TOOL-001: M0 is rendered before M1");
    // M1 is the last milestone, so its section runs to the end of the page.
    let sections = [
        ("M0", &page[m0..m1], vec!["LIB-CLI-001"]),
        (
            "M1",
            &page[m1..],
            vec!["SACM23-BASE-001", "SACM23-BASE-002", "SACM23-BASE-003"],
        ),
    ];
    let all = [
        "LIB-CLI-001",
        "SACM23-BASE-001",
        "SACM23-BASE-002",
        "SACM23-BASE-003",
    ];
    for (milestone, section, own) in sections {
        let listed: Vec<&str> = all
            .into_iter()
            .filter(|id| section.lines().any(|l| l.starts_with(&format!("| {id} |"))))
            .collect();
        assert_eq!(
            listed, own,
            "LIB-TOOL-001: the {milestone} table lists exactly the {milestone} rows:\n{section}"
        );
    }
}

// ---------------------------------------------------------------------------
// ready (`matrix next`)
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_001_next_keeps_file_order_within_a_milestone_and_lists_only_open_rows() {
    let rows = [
        row("SACM23-BASE-005", "planned", ""),
        row("SACM23-BASE-001", "verified", ""),
        row(
            "SACM23-BASE-003",
            "analyzed",
            "depends_on = [\"SACM23-BASE-001\"]",
        ),
        row("SACM23-BASE-002", "candidate", "notes = \"why\""),
        row("SACM23-BASE-004", "deferred", "notes = \"why\""),
        row("SACM23-BASE-006", "out-of-scope", "notes = \"why\""),
        row("SACM23-BASE-008", "implemented", ""),
        row("SACM23-BASE-007", "tests-failing", ""),
    ]
    .concat();
    let m = matrix(&rows);
    let ids: Vec<&str> = ready(&m).iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "SACM23-BASE-005",
            "SACM23-BASE-003",
            "SACM23-BASE-008",
            "SACM23-BASE-007"
        ],
        "LIB-TOOL-001: open rows only, in file order within the milestone (not id order)"
    );
}

// ---------------------------------------------------------------------------
// The filesystem half of the gate: `FsRepo` and `cmd_check` on a real tree
// ---------------------------------------------------------------------------

// `TempTree`, `GATE_TOML` and `consistent_tree` live in `common/mod.rs`,
// shared with `run.rs`.

/// `FsRepo::traced_functions` of the tree, sorted (directory order is not
/// specified).
fn traced_on_disk(tree: &TempTree) -> Vec<(String, String)> {
    let mut found = FsRepo::new(tree.root()).traced_functions();
    found.sort();
    found
}

fn owned_pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = pairs
        .iter()
        .map(|(f, n)| ((*f).to_owned(), (*n).to_owned()))
        .collect();
    out.sort();
    out
}

#[test]
fn lib_tool_001_fs_repo_finds_traced_tests_in_every_scan_root() {
    let tree = TempTree::new("scan-roots");
    tree.write(
        "crates/demo/tests/a.rs",
        "#[test]\nfn lib_cli_001_in_crates() {}\n#[test]\nfn helper() {}\nfn lib_cli_001_unmarked() {}\n",
    )
    .write(
        "crates/demo/tests/b.py",
        "def test_lib_cli_001_python_in_crates(self):\n    pass\n",
    )
    .write("xtask/tests/c.rs", "#[test] fn lib_cli_001_in_xtask() {}\n")
    .write(
        ".claude/hooks/test_d.py",
        "def test_lib_cli_001_in_hooks(self):\n    pass\ndef lib_cli_001_helper():\n    pass\n",
    )
    .write(
        ".claude/hooks/e.rs",
        "#[test]\nfn lib_cli_001_rust_in_hooks() {}\n",
    );
    assert_eq!(
        traced_on_disk(&tree),
        owned_pairs(&[
            ("crates/demo/tests/a.rs", "lib_cli_001_in_crates"),
            (
                "crates/demo/tests/b.py",
                "test_lib_cli_001_python_in_crates"
            ),
            ("xtask/tests/c.rs", "lib_cli_001_in_xtask"),
            (".claude/hooks/test_d.py", "test_lib_cli_001_in_hooks"),
            (".claude/hooks/e.rs", "lib_cli_001_rust_in_hooks"),
        ]),
        "LIB-TOOL-001: traced Rust and Python tests are found under crates/, xtask/ and \
         .claude/hooks/, as `/`-separated repo-relative paths"
    );
}

#[test]
fn lib_tool_001_fs_repo_skips_target_directories_and_non_source_files() {
    let tree = TempTree::new("skips");
    let rust_test = "#[test]\nfn lib_cli_001_skipped() {}\n";
    let python_test = "def test_lib_cli_001_skipped(self):\n    pass\n";
    tree.write("crates/demo/tests/kept.rs", "#[test]\nfn lib_cli_001_kept() {}\n")
        // Build output is never scanned, at any depth and in every root. A
        // `target` directory is build output when a `Cargo.toml` sits
        // directly beside it (slice 0006, D9).
        .write("Cargo.toml", "[workspace]\n")
        .write("crates/demo/Cargo.toml", "[package]\n")
        .write("xtask/Cargo.toml", "[package]\n")
        .write(".claude/hooks/Cargo.toml", "[package]\n")
        .write("crates/demo/target/debug/build.rs", rust_test)
        .write("target/x.rs", rust_test)
        .write("xtask/target/x.rs", rust_test)
        .write(".claude/hooks/target/test_x.py", python_test)
        // Files that are neither `.rs` nor `.py`.
        .write("crates/demo/tests/notes.txt", rust_test)
        .write("crates/demo/tests/x.rs.bak", rust_test)
        .write(".claude/hooks/test_x.json", python_test)
        .write(".claude/hooks/test_x.pyc", python_test)
        // Each language is scanned with its own syntax.
        .write("crates/demo/tests/python_syntax.rs", python_test)
        .write(".claude/hooks/test_rust_syntax.py", rust_test)
        // Outside the three scan roots.
        .write("docs/x.rs", rust_test)
        .write("x.rs", rust_test)
        .write(".claude/agents/test_x.py", python_test);
    assert_eq!(
        traced_on_disk(&tree),
        owned_pairs(&[("crates/demo/tests/kept.rs", "lib_cli_001_kept")]),
        "LIB-TOOL-001: `target/` directories beside a `Cargo.toml`, non-source files, other \
         roots and wrong-language syntax are not scanned"
    );
}

#[test]
fn lib_tool_001_fs_repo_reads_and_finds_paths_relative_to_its_root() {
    let tree = TempTree::new("read-exists");
    tree.write("docs/a.md", "text");
    let repo = FsRepo::new(tree.root());
    assert!(
        repo.exists("docs/a.md"),
        "LIB-TOOL-001: an existing file exists"
    );
    assert!(
        repo.exists("docs"),
        "LIB-TOOL-001: an existing directory exists"
    );
    assert!(
        !repo.exists("docs/b.md"),
        "LIB-TOOL-001: a missing file does not exist"
    );
    assert_eq!(
        repo.read("docs/a.md").as_deref(),
        Some("text"),
        "LIB-TOOL-001: read returns the text of the file"
    );
    assert_eq!(
        repo.read("docs/b.md"),
        None,
        "LIB-TOOL-001: read of a missing file is None"
    );
}

/// The problems `check` finds on the tree through `FsRepo`.
fn problems_on_disk(tree: &TempTree) -> Vec<String> {
    let matrix = load(GATE_TOML).expect("LIB-TOOL-001: the gate fixture parses");
    check(&matrix, &FsRepo::new(tree.root()))
}

#[test]
fn lib_tool_001_cmd_check_accepts_a_minimal_consistent_tree() {
    let tree = consistent_tree("consistent");
    assert_eq!(
        problems_on_disk(&tree),
        Vec::<String>::new(),
        "LIB-TOOL-001: the consistent tree has no problems"
    );
    assert_eq!(
        cmd_check(tree.root(), &mut io::sink()),
        Ok(()),
        "LIB-TOOL-001: `matrix check` passes a consistent tree"
    );
}

/// Plant an uncited traced test at `rel` in an otherwise consistent tree: the
/// gate must report exactly that orphan and `cmd_check` must fail.
fn assert_planted_orphan_fails_the_gate(test: &str, rel: &str, source: &str, name: &str) {
    let tree = consistent_tree(test);
    assert_eq!(
        cmd_check(tree.root(), &mut io::sink()),
        Ok(()),
        "LIB-TOOL-001: precondition: the tree passes before {rel} is planted"
    );
    tree.write(rel, source);
    assert_eq!(
        problems_on_disk(&tree),
        [format!(
            "{rel}: traced test `{name}` is not cited by any matrix row"
        )],
        "LIB-TOOL-001: the planted test {rel}::{name} is reported as an orphan"
    );
    assert!(
        cmd_check(tree.root(), &mut io::sink()).is_err(),
        "LIB-TOOL-001: `matrix check` must fail with the uncited test {rel}::{name}"
    );
}

#[test]
fn lib_tool_001_cmd_check_rejects_an_uncited_python_test_under_claude_hooks() {
    assert_planted_orphan_fails_the_gate(
        "orphan-hooks",
        ".claude/hooks/test_planted.py",
        "def test_lib_tool_001_uncited(self):\n    pass\n",
        "test_lib_tool_001_uncited",
    );
}

#[test]
fn lib_tool_001_cmd_check_rejects_an_uncited_rust_test_under_crates() {
    assert_planted_orphan_fails_the_gate(
        "orphan-crates",
        "crates/demo/tests/planted.rs",
        "#[test]\nfn lib_tool_001_uncited() {}\n",
        "lib_tool_001_uncited",
    );
}

#[test]
fn lib_tool_001_cmd_check_rejects_an_uncited_rust_test_under_xtask() {
    assert_planted_orphan_fails_the_gate(
        "orphan-xtask",
        "xtask/tests/planted.rs",
        "#[test]\nfn lib_tool_001_uncited() {}\n",
        "lib_tool_001_uncited",
    );
}

#[test]
fn lib_tool_001_cmd_check_rejects_a_missing_matrix_and_a_stale_page() {
    let empty = TempTree::new("no-matrix");
    assert!(
        cmd_check(empty.root(), &mut io::sink()).is_err(),
        "LIB-TOOL-001: `matrix check` fails when the matrix file is missing"
    );

    let tree = consistent_tree("stale-page");
    tree.write(RENDERED_PATH, "stale");
    assert!(
        cmd_check(tree.root(), &mut io::sink()).is_err(),
        "LIB-TOOL-001: `matrix check` fails on a stale rendered page"
    );

    // A checkout with CRLF line endings is not stale.
    let matrix = load(GATE_TOML).expect("LIB-TOOL-001: the gate fixture parses");
    tree.write(RENDERED_PATH, &render(&matrix).replace('\n', "\r\n"));
    assert_eq!(
        cmd_check(tree.root(), &mut io::sink()),
        Ok(()),
        "LIB-TOOL-001: a rendered page that differs only in CRLF line endings is current"
    );
}
