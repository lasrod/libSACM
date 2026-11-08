//! Tests of how the matrix gate finds function definitions (LIB-TOOL-001).
//!
//! When a row cites `path::name`, the gate looks for `fn name` or `def name`
//! in the cited file. That scan decides between "test function `name` is not
//! defined" and "`name` is not a test", and finds the function that a
//! `#[test]`-style attribute marks. These tests drive it through
//! `xtask::matrix::check` with an in-memory repository holding the cited file.
//!
//! # Why every test here runs under a deadline, in its own test binary
//!
//! A scanner that stops advancing through the text loops forever. Each test
//! therefore runs the check on a worker thread and fails when it has not
//! finished within [`DEADLINE`], so that a hang is reported as a failure and
//! not as a stalled run. That only helps if the hang is seen before any other
//! test hangs on the same scanner: a test binary finishes only when all of its
//! tests do, and `cargo test` (as cargo-mutants runs it, without
//! `--no-fail-fast`) runs the test binaries one after another in name order
//! and stops at the first that fails. So these tests live in a binary of their
//! own, whose name sorts before `matrix`, `repo_checks` and `run`, the binaries
//! whose tests reach the scanner without a deadline; and every test in this
//! binary uses the deadline.
//!
//! The same holds for the comment and literal masking of LIB-TOOL-011: a
//! lexer state that stops advancing hangs only on text that reaches that
//! state. `gate_hardening`, `matrix`, `repo_checks`, `reporting`, `run` and
//! `scanner` all reach the masking without a deadline, and the real
//! repository holds every form. So
//! `lib_tool_011_function_scan_finishes_on_every_comment_and_literal_form`
//! drives every form, closed and left open at the end of the text, through
//! the citation check and through `traced_tests` and `ignored_traced_tests`
//! here, under the deadline.
//!
//! Fixture source text is written on one line with `\n` escapes on purpose: a
//! line of this file that began with `#[test]` inside a string literal would
//! be picked up by the real repository's orphan scan.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use xtask::matrix::{RENDERED_PATH, Repo, check, ignored_traced_tests, load, render, traced_tests};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// How long one check may take. A check of a one-row matrix over a one-line
/// file takes microseconds; a scanner that loops never finishes. The deadline
/// stays well below cargo-mutants' minimum test timeout of 20 seconds.
const DEADLINE: Duration = Duration::from_secs(5);

/// An in-memory repository: relative path to file text. The orphan scan is
/// not under test here, so it finds no traced functions, running or ignored.
#[derive(Debug)]
struct FakeRepo {
    files: BTreeMap<String, String>,
}

impl Repo for FakeRepo {
    fn exists(&self, rel: &str) -> bool {
        self.files.contains_key(rel)
    }

    fn read(&self, rel: &str) -> Option<String> {
        self.files.get(rel).cloned()
    }

    fn traced_functions(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn ignored_functions(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}

/// One tests-failing row, LIB-TOOL-001, whose `tests` key is appended.
const MATRIX: &str = "schema_version = 1\n\
     [[milestone]]\nid = \"M0\"\ntitle = \"Foundation\"\ngoal = \"Build.\"\n\
     [[area]]\ncode = \"TOOL\"\nprefix = \"LIB\"\ntitle = \"Tooling\"\n\
     [[row]]\nid = \"LIB-TOOL-001\"\narea = \"TOOL\"\nmilestone = \"M0\"\n\
     title = \"Gate\"\nrequirement = \"The gate holds.\"\nsource = \"ADR 0004\"\n\
     kind = \"tooling\"\nstatus = \"tests-failing\"\nacceptance = [\"a\"]\n";

const RUST_FILE: &str = "crates/demo/tests/gate.rs";
const PYTHON_FILE: &str = ".claude/hooks/test_gate.py";

/// Run `work` on a worker thread and return its result, failing the test when
/// it panics or does not finish within [`DEADLINE`]. Failure messages start
/// with `row`.
fn within_deadline<T: Send + 'static>(
    row: &str,
    what: &str,
    work: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        // The receiver is gone only after the deadline has already failed the test.
        let _ = sender.send(work());
    });
    match receiver.recv_timeout(DEADLINE) {
        Ok(value) => value,
        Err(RecvTimeoutError::Timeout) => panic!(
            "{row}: {what} did not finish within {DEADLINE:?}: the function scan \
             does not advance through the text"
        ),
        Err(RecvTimeoutError::Disconnected) => {
            panic!("{row}: {what} panicked instead of returning its problems")
        }
    }
}

/// The problems `check` reports for a matrix whose one row cites
/// `path::name`, when the file at `path` holds `text`.
fn citation_problems(path: &str, name: &str, text: &str) -> Vec<String> {
    citation_problems_for("LIB-TOOL-001", path, name, text)
}

/// [`citation_problems`], with failure messages that start with `row`.
fn citation_problems_for(row: &str, path: &str, name: &str, text: &str) -> Vec<String> {
    let what = format!("the check of `{path}::{name}` over {text:?}");
    let (path, name, text) = (path.to_owned(), name.to_owned(), text.to_owned());
    let parse_failure = format!("{row}: the fixture matrix parses");
    within_deadline(row, &what, move || {
        let matrix =
            load(&format!("{MATRIX}tests = [\"{path}::{name}\"]\n")).expect(&parse_failure);
        let files = BTreeMap::from([(RENDERED_PATH.to_owned(), render(&matrix)), (path, text)]);
        check(&matrix, &FakeRepo { files })
    })
}

/// `traced_tests` and `ignored_traced_tests` of `text`, under the deadline.
fn scan(row: &str, text: &str, python: bool) -> (Vec<String>, Vec<String>) {
    let what = format!("the scan of {text:?}");
    let text = text.to_owned();
    within_deadline(row, &what, move || {
        (
            traced_tests(&text, python),
            ignored_traced_tests(&text, python),
        )
    })
}

fn not_defined(path: &str, name: &str) -> String {
    format!("LIB-TOOL-001: test function `{name}` is not defined in {path}")
}

fn rust_not_a_test(name: &str) -> String {
    format!(
        "LIB-TOOL-001: function `{name}` in {RUST_FILE} is not a test: it has no \
         `#[test]`-style attribute"
    )
}

fn python_not_a_test(name: &str) -> String {
    format!(
        "LIB-TOOL-001: function `{name}` in {PYTHON_FILE} is not a test: unittest runs \
         only `test_...` methods"
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_001_function_scan_finds_the_test_an_attribute_marks() {
    for text in [
        // The definition starts the line.
        "#[test]\nfn lib_tool_001_cited() {}\n",
        // It follows the attribute on the same line.
        "#[test] fn lib_tool_001_cited() {}\n",
        // It follows other words on the line.
        "#[test]\npub fn lib_tool_001_cited() {}\n",
        "#[test]\npub(crate) async fn lib_tool_001_cited() {}\n",
        // It follows the definition of a helper.
        "fn helper() {}\n#[test]\nfn lib_tool_001_cited() {}\n",
    ] {
        assert_eq!(
            citation_problems(RUST_FILE, "lib_tool_001_cited", text),
            Vec::<String>::new(),
            "LIB-TOOL-001: in {text:?}, `lib_tool_001_cited` is a test, so citing it is fine"
        );
    }
}

#[test]
fn lib_tool_001_function_scan_finds_definitions_without_a_test_attribute() {
    for text in [
        // `fn` at the very start of the text.
        "fn lib_tool_001_cited() {}\n",
        // The second definition in the text, and on its line.
        "fn helper() {}\nfn lib_tool_001_cited() {}\n",
        "fn helper() {} fn lib_tool_001_cited() {}\n",
        // After an indent and a visibility.
        "mod m {\n    pub(crate) fn lib_tool_001_cited() {}\n}\n",
    ] {
        assert_eq!(
            citation_problems(RUST_FILE, "lib_tool_001_cited", text),
            [rust_not_a_test("lib_tool_001_cited")],
            "LIB-TOOL-001: in {text:?}, `lib_tool_001_cited` is defined but is not a test"
        );
    }
    for text in [
        // `def` at the very start of the text.
        "def lib_tool_001_helper(self):\n    pass\n",
        // An indented method after a class line.
        "class T:\n    def lib_tool_001_helper(self):\n        pass\n",
    ] {
        assert_eq!(
            citation_problems(PYTHON_FILE, "lib_tool_001_helper", text),
            [python_not_a_test("lib_tool_001_helper")],
            "LIB-TOOL-001: in {text:?}, `lib_tool_001_helper` is defined but is not a test"
        );
    }
}

#[test]
fn lib_tool_001_function_scan_ignores_fn_and_def_at_the_end_of_a_longer_word() {
    // `myfn name` defines nothing: the keyword must not follow a letter,
    // digit or underscore.
    for text in [
        "// myfn lib_tool_001_cited\n",
        "// my_fn lib_tool_001_cited\n",
        "// v2fn lib_tool_001_cited\n",
        "// undef lib_tool_001_cited\n",
    ] {
        assert_eq!(
            citation_problems(RUST_FILE, "lib_tool_001_cited", text),
            [not_defined(RUST_FILE, "lib_tool_001_cited")],
            "LIB-TOOL-001: {text:?} does not define `lib_tool_001_cited`"
        );
    }
    for text in [
        "# undef lib_tool_001_helper\n",
        "x = my_def lib_tool_001_helper\n",
    ] {
        assert_eq!(
            citation_problems(PYTHON_FILE, "lib_tool_001_helper", text),
            [not_defined(PYTHON_FILE, "lib_tool_001_helper")],
            "LIB-TOOL-001: {text:?} does not define `lib_tool_001_helper`"
        );
    }
}

/// Every Rust comment and literal form of LIB-TOOL-011, each closed. The
/// tests written inside them are not tests.
const RUST_FORMS: [&str; 14] = [
    "// a line comment holding /* and \" and ' and r#\"",
    "/// a doc comment holding /*",
    "//! an inner doc comment holding \"",
    "/* a block comment /* nested */ holding \" ' // and\n#[test]\nfn lib_tool_001_in_block() {}\n*/",
    "/*/ opens and does not close */",
    "/**/ /***/ /*!*/",
    "let s = \"spans\n#[test]\nfn lib_tool_001_in_string() {}\n\\\" \\\\\";",
    "let r = r\"\\\";",
    "let h = r##\"\n\"#\n#[test]\nfn lib_tool_001_in_raw() {}\n\"##;",
    "let b = br#\"\n\"\n\"#; let cr = cr#\"\n\"#;",
    "let c = c\"\\\"\"; let bs = b\"\\\"\";",
    "let q = '\"'; let e = '\\''; let bq = b'\"'; let be = b'\\''; let u = '\\u{22}'; let m = 'é';",
    "fn f<'a>(x: &'a str) -> &'static str { 'l: loop { break 'l; } }",
    "let r#match = 1;",
];

/// Every Python comment, string and skip form of LIB-TOOL-011, each closed.
const PYTHON_FORMS: [&str; 11] = [
    "# a comment holding \"\"\" and '",
    "x = \"'''\" + '\"\"\"' + \"#\" + \"\\\"\" + r'\\'' + ''",
    "y = \"\"\"spans \\\"\"\" \"\" \"\ndef test_lib_tool_001_in_string(self):\n'''\n\"\"\"",
    "z = rb'''\ndef test_lib_tool_001_in_bytes(self):\n'''",
    "@unittest.skip(\"x\")",
    "class Skipped(unittest.TestCase):",
    "    async def test_lib_tool_001_skipped(self):",
    "        pass",
    "@pytest.mark.skipif(True, reason=\"x\")",
    "def test_lib_tool_001_also_skipped():",
    "    pass",
];

/// Rust text that ends inside a literal, a comment or an attribute.
const RUST_TAILS: [&str; 26] = [
    "\"",
    "\"\\",
    "r",
    "r#",
    "r#\"",
    "r##\"x\"#",
    "br#\"",
    "b\"",
    "c\"",
    "cr#\"",
    "'",
    "'\\",
    "b'",
    "b'\\",
    "'a",
    "/",
    "/*",
    "/* /* */",
    "/**",
    "//",
    "#[",
    "#[test",
    "#[test]",
    "#[test]\n#[ignore",
    "#[cfg_attr(miri, ignore",
    "#[test_case(\"a]",
];

/// Python text that ends inside a string, a comment or a decorator.
const PYTHON_TAILS: [&str; 16] = [
    "\"",
    "'",
    "\"\"\"",
    "'''",
    "\"\\",
    "\"\"\"\\",
    "r\"\\",
    "rb'''",
    "#",
    "@",
    "@unittest.skip(",
    "@unittest.skip(\"x\")",
    "@unittest.skip(\"x\")\nclass T:",
    "async",
    "async def",
    "def",
];

#[test]
fn lib_tool_011_function_scan_finishes_on_every_comment_and_literal_form() {
    const ROW: &str = "LIB-TOOL-011";
    let rust_cited = "#[test]\nfn lib_tool_001_cited() {}\n";
    let python_cited = "def test_lib_tool_001_cited(self):\n    pass\n";

    let rust = format!("{}\n{rust_cited}", RUST_FORMS.join("\n"));
    let python = format!("{}\n{python_cited}", PYTHON_FORMS.join("\n"));
    for (rust, python) in [
        (rust.clone(), python.clone()),
        (rust.replace('\n', "\r\n"), python.replace('\n', "\r\n")),
    ] {
        assert_eq!(
            citation_problems_for(ROW, RUST_FILE, "lib_tool_001_cited", &rust),
            Vec::<String>::new(),
            "LIB-TOOL-011: the test after every closed Rust form is found: {rust:?}"
        );
        assert_eq!(
            scan(ROW, &rust, false),
            (vec!["lib_tool_001_cited".to_owned()], Vec::new()),
            "LIB-TOOL-011: only the test after every closed Rust form runs: {rust:?}"
        );
        assert_eq!(
            citation_problems_for(ROW, PYTHON_FILE, "test_lib_tool_001_cited", &python),
            Vec::<String>::new(),
            "LIB-TOOL-011: the test after every closed Python form is found: {python:?}"
        );
        assert_eq!(
            scan(ROW, &python, true),
            (
                vec!["test_lib_tool_001_cited".to_owned()],
                vec![
                    "test_lib_tool_001_skipped".to_owned(),
                    "test_lib_tool_001_also_skipped".to_owned()
                ]
            ),
            "LIB-TOOL-011: the test after every closed Python form runs, and the two \
             skipped ones are ignored: {python:?}"
        );
    }

    for tail in RUST_TAILS {
        let text = format!("{rust_cited}{tail}");
        assert_eq!(
            citation_problems_for(ROW, RUST_FILE, "lib_tool_001_cited", &text),
            Vec::<String>::new(),
            "LIB-TOOL-011: Rust text ending in {tail:?} is scanned to its end without a panic"
        );
        assert_eq!(
            scan(ROW, &text, false),
            (vec!["lib_tool_001_cited".to_owned()], Vec::new()),
            "LIB-TOOL-011: Rust text ending in {tail:?} keeps the test before it"
        );
    }
    for tail in PYTHON_TAILS {
        let text = format!("{python_cited}{tail}");
        assert_eq!(
            citation_problems_for(ROW, PYTHON_FILE, "test_lib_tool_001_cited", &text),
            Vec::<String>::new(),
            "LIB-TOOL-011: Python text ending in {tail:?} is scanned to its end without a panic"
        );
        assert_eq!(
            scan(ROW, &text, true),
            (vec!["test_lib_tool_001_cited".to_owned()], Vec::new()),
            "LIB-TOOL-011: Python text ending in {tail:?} keeps the test before it"
        );
    }
}
