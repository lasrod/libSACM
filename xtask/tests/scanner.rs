//! Acceptance tests for the precise traced-test scanner (slice 0006).
//!
//! - LIB-TOOL-011: the scanner never reads text inside a comment or a literal,
//!   in Rust or in Python; it recognises `#[test_case]`, `#[rstest]` and other
//!   attributes whose last path segment is `test`, and Python `async def
//!   test_`; and an ignored or skipped traced test is never evidence. It is
//!   reported once: `<file>: traced test `<name>` is ignored or skipped, so it
//!   is not evidence`.
//! - LIB-TOOL-007: a traced test in a file with a path component `src` is
//!   reported once: `<file>: traced test `<name>` is under a `src` directory;
//!   traced tests belong in test files`.
//!
//! - Fix round 1 (D9, D10): a `target` directory is build output only when a
//!   `Cargo.toml` sits directly beside it, and a test citation of a file the
//!   scan does not cover is the row problem `test file <path> is outside the
//!   scanned test files`, so every cited test gets the problems above.
//!
//! The scanner is driven through `traced_tests` and `ignored_traced_tests`,
//! through `check` with in-memory repositories, through `FsRepo` and
//! `cmd_check` on temporary trees, and over the real repository.
//!
//! # Harness
//!
//! Every scan of fixture text runs on a worker thread under [`DEADLINE`], so
//! that a lexer that stops advancing fails the test instead of stalling the
//! run. The cases that guard against such a hang under cargo-mutants live in
//! `function_scan.rs`, which runs first (see its module doc).
//!
//! **Keep this target's name sorting after `function_scan`.** The last tests
//! here scan the real repository. Under a scanner mutant that makes the scan
//! quadratic, a target that sorts first turns a kill into a timeout
//! (`docs/quality/mutants-triage.md`, slice 0005).
//!
//! # Fixtures
//!
//! Fixture source text is written as single-line string literals with `\n`
//! escapes, never as multi-line or raw strings: a line of this file that began
//! with `#[test]` inside a string literal would be picked up by the real
//! repository's orphan scan as long as the scanner is line-based. Trees on
//! disk are written at run time.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use common::{GATE_TOML, TempTree, consistent_tree};
use xtask::matrix::{
    FsRepo, MATRIX_PATH, RENDERED_PATH, Repo, check, cmd_check, ignored_traced_tests, load, render,
    traced_tests,
};
use xtask::repo_root;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

const ROW_007: &str = "LIB-TOOL-007";
const ROW_011: &str = "LIB-TOOL-011";

/// How long one scan or check of fixture text may take. It takes
/// microseconds; a lexer that stops advancing never finishes.
const DEADLINE: Duration = Duration::from_secs(5);

/// Run `work` on a worker thread and return its result, failing the test when
/// it panics or does not finish within [`DEADLINE`]. Messages start with `row`.
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
            "{row}: {what} did not finish within {DEADLINE:?}: the scanner does not \
             advance through the text"
        ),
        Err(RecvTimeoutError::Disconnected) => {
            panic!("{row}: {what} panicked instead of returning")
        }
    }
}

/// The traced tests the scanner finds in one text.
#[derive(Debug, PartialEq, Eq)]
struct Found {
    /// `traced_tests`: the ones that run.
    running: Vec<String>,
    /// `ignored_traced_tests`: the ignored or skipped ones.
    ignored: Vec<String>,
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

/// [`Found`] for `text`, under the deadline.
fn scan(row: &str, text: &str, python: bool) -> Found {
    let what = format!("the scan of {text:?}");
    let text = text.to_owned();
    within_deadline(row, &what, move || Found {
        running: traced_tests(&text, python),
        ignored: ignored_traced_tests(&text, python),
    })
}

#[track_caller]
fn assert_scan(row: &str, text: &str, python: bool, running: &[&str], ignored: &[&str], why: &str) {
    assert_eq!(
        scan(row, text, python),
        Found {
            running: owned(running),
            ignored: owned(ignored),
        },
        "{row}: {why}; in {text:?}"
    );
}

/// The Rust scan of `text` finds exactly `running` and `ignored`.
#[track_caller]
fn assert_rust(text: &str, running: &[&str], ignored: &[&str], why: &str) {
    assert_scan(ROW_011, text, false, running, ignored, why);
}

/// The Python scan of `text` finds exactly `running` and `ignored`.
#[track_caller]
fn assert_python(text: &str, running: &[&str], ignored: &[&str], why: &str) {
    assert_scan(ROW_011, text, true, running, ignored, why);
}

/// A Rust traced test that runs, to follow a form: when it is found, the
/// scanner has resumed after the form.
const RUN: &str = "#[test]\nfn lib_tool_011_after() {}\n";
const AFTER: &str = "lib_tool_011_after";
/// A Rust traced test that runs, to precede a form.
const BEFORE: &str = "#[test]\nfn lib_tool_011_before() {}\n";
const BEFORE_NAME: &str = "lib_tool_011_before";

/// The Python counterparts of [`RUN`] and [`BEFORE`].
const PY_RUN: &str = "def test_lib_tool_011_after(self):\n    pass\n";
const PY_AFTER: &str = "test_lib_tool_011_after";
const PY_BEFORE: &str = "def test_lib_tool_011_before(self):\n    pass\n";
const PY_BEFORE_NAME: &str = "test_lib_tool_011_before";

/// `form`, then [`RUN`] on the next line.
fn then_run(form: &str) -> String {
    format!("{form}\n{RUN}")
}

/// `form`, then [`PY_RUN`] on the next line.
fn then_py_run(form: &str) -> String {
    format!("{form}\n{PY_RUN}")
}

/// Whether the fixture path `file` names a Python file.
fn is_python(file: &str) -> bool {
    Path::new(file).extension().is_some_and(|e| e == "py")
}

/// An in-memory repository: relative path to file text. Its `.rs` and `.py`
/// files are scanned with `traced_tests` and `ignored_traced_tests`.
#[derive(Debug, Default)]
struct FakeRepo {
    files: BTreeMap<String, String>,
}

impl FakeRepo {
    fn scan(&self, find: fn(&str, bool) -> Vec<String>) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (path, text) in &self.files {
            let extension = Path::new(path).extension().and_then(|e| e.to_str());
            let python = match extension {
                Some("rs") => false,
                Some("py") => true,
                _ => continue,
            };
            for name in find(text, python) {
                out.push((path.clone(), name));
            }
        }
        out
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

/// A repository that lists the given traced functions, in the given order,
/// and holds only the rendered page.
#[derive(Debug)]
struct ListedRepo {
    page: String,
    running: Vec<(String, String)>,
    ignored: Vec<(String, String)>,
}

impl Repo for ListedRepo {
    fn exists(&self, rel: &str) -> bool {
        rel == RENDERED_PATH
    }

    fn read(&self, rel: &str) -> Option<String> {
        (rel == RENDERED_PATH).then(|| self.page.clone())
    }

    fn traced_functions(&self) -> Vec<(String, String)> {
        self.running.clone()
    }

    fn ignored_functions(&self) -> Vec<(String, String)> {
        self.ignored.clone()
    }
}

const HEAD: &str = "schema_version = 1\n[[milestone]]\nid = \"M0\"\ntitle = \"Foundation\"\n\
                    goal = \"Build.\"\n[[area]]\ncode = \"TOOL\"\nprefix = \"LIB\"\n\
                    title = \"Tooling\"\n";

fn row_toml(id: &str, status: &str, extra: &str) -> String {
    format!(
        "[[row]]\nid = \"{id}\"\narea = \"TOOL\"\nmilestone = \"M0\"\ntitle = \"Row\"\n\
         requirement = \"R.\"\nsource = \"S\"\nkind = \"tooling\"\nstatus = \"{status}\"\n\
         {extra}\n"
    )
}

/// A matrix whose one row, `id`, is planned and cites nothing.
fn uncited(id: &str) -> String {
    format!("{HEAD}{}", row_toml(id, "planned", ""))
}

/// A matrix whose one row, `id`, is tests-failing and cites `tests`.
fn citing(id: &str, tests: &[&str]) -> String {
    let list: Vec<String> = tests.iter().map(|t| format!("\"{t}\"")).collect();
    let extra = format!("acceptance = [\"a\"]\ntests = [{}]", list.join(", "));
    format!("{HEAD}{}", row_toml(id, "tests-failing", &extra))
}

/// The problems `check` reports for the matrix `toml` over `files`, with the
/// rendered page in sync, under the deadline.
fn problems(row: &str, toml: &str, files: &[(&str, &str)]) -> Vec<String> {
    let what = format!("the check over {files:?}");
    let toml = toml.to_owned();
    let files: Vec<(String, String)> = files
        .iter()
        .map(|(p, t)| ((*p).to_owned(), (*t).to_owned()))
        .collect();
    let parse_failure = format!("{row}: the fixture matrix parses");
    within_deadline(row, &what, move || {
        let matrix = load(&toml).expect(&parse_failure);
        let mut repo = FakeRepo::default();
        repo.files.insert(RENDERED_PATH.to_owned(), render(&matrix));
        repo.files.extend(files);
        check(&matrix, &repo)
    })
}

fn under_src(file: &str, name: &str) -> String {
    format!(
        "{file}: traced test `{name}` is under a `src` directory; traced tests belong in test files"
    )
}

fn ignored(file: &str, name: &str) -> String {
    format!("{file}: traced test `{name}` is ignored or skipped, so it is not evidence")
}

fn orphan(file: &str, name: &str) -> String {
    format!("{file}: traced test `{name}` is not cited by any matrix row")
}

/// The row problem for a test citation of a file the scan does not cover
/// (D10).
fn outside_scan(row: &str, file: &str) -> String {
    format!("{row}: test file {file} is outside the scanned test files")
}

// ---------------------------------------------------------------------------
// LIB-TOOL-011: Rust comments and literals
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_011_rust_line_and_doc_comments_hide_test_like_text() {
    for text in [
        "#[test] // fn lib_tool_011_in_comment() {}\nfn helper() {}\n",
        "#[test]\n/// fn lib_tool_011_in_doc() {}\nfn helper() {}\n",
        "//! #[test] fn lib_tool_011_in_inner_doc() {}\n",
        "// #[test]\nfn lib_tool_011_unmarked() {}\n",
        "/// #[test]\nfn lib_tool_011_unmarked() {}\n",
        "let x = 1; // #[test] fn lib_tool_011_in_comment() {}\n",
    ] {
        assert_rust(
            text,
            &[],
            &[],
            "a `#[test]` or `fn` in a line comment marks and defines nothing",
        );
    }
    assert_rust(
        "#[test]\n// fn lib_tool_011_in_comment() {}\nfn lib_tool_011_after() {}\n",
        &[AFTER],
        &[],
        "a line comment between the attribute and the function changes nothing",
    );
    // The comment ends at its newline, whatever it holds.
    for comment in [
        "// /* opens nothing",
        "// \" opens nothing",
        "// ' opens nothing",
        "/// r#\" opens nothing",
        "//! /* opens nothing",
        "let x = 1; // \" opens nothing",
    ] {
        assert_rust(
            &then_run(comment),
            &[AFTER],
            &[],
            "a line comment ends at its newline",
        );
    }
}

#[test]
fn lib_tool_011_rust_block_comments_nest_and_hide_test_like_text() {
    for text in [
        "/*\n#[test]\nfn lib_tool_011_in() {}\n*/\n",
        "/*\n#[test]\n*/ fn lib_tool_011_unmarked() {}\n",
        "#[test]\n/* fn lib_tool_011_in() {} */ fn helper() {}\n",
        "/** #[test]\nfn lib_tool_011_in() {}\n*/\n",
        "/*! #[test]\nfn lib_tool_011_in() {}\n*/\n",
    ] {
        assert_rust(
            text,
            &[],
            &[],
            "a `#[test]` or `fn` in a block comment marks and defines nothing",
        );
    }
    for comment in [
        "/*\n#[test]\nfn lib_tool_011_in() {}\n*/",
        // Nested comments: the comment ends at the `*/` that closes the first `/*`.
        "/* /* */\n#[test]\nfn lib_tool_011_in() {}\n*/",
        "/* /* /* */ */\n#[test]\nfn lib_tool_011_in() {}\n*/",
        "/* /* */ /* */\n#[test]\nfn lib_tool_011_in() {}\n*/",
        // `/*/` opens a comment and does not close it.
        "/*/\n#[test]\nfn lib_tool_011_in() {}\n*/",
        "/**/",
        "/***/",
        // Quotes and `//` inside a block comment open nothing.
        "/* \" ' // r#\" */",
        "/* \"*/",
    ] {
        assert_rust(
            &then_run(comment),
            &[AFTER],
            &[],
            "only the test after the closed block comment counts",
        );
    }
}

#[test]
fn lib_tool_011_rust_a_block_comment_left_open_hides_the_rest_of_the_file() {
    for open in ["/*", "/* /* */", "/* /* */ */ /*", "/**", "/*/"] {
        assert_rust(
            &format!("{BEFORE}{open}\n{RUN}"),
            &[BEFORE_NAME],
            &[],
            "a block comment that is never closed runs to the end of the file",
        );
    }
}

#[test]
fn lib_tool_011_rust_strings_that_span_lines_hide_test_like_text() {
    for text in [
        "let s = \"\n#[test]\nfn lib_tool_011_in() {}\n\";\n",
        "let s = \"\n#[test]\n\"; fn lib_tool_011_unmarked() {}\n",
        "#[test]\nconst S: &str = \"fn lib_tool_011_in() {}\"; fn helper() {}\n",
    ] {
        assert_rust(
            text,
            &[],
            &[],
            "a `#[test]` or `fn` in a string marks and defines nothing",
        );
    }
    for string in [
        "let s = \"\n#[test]\nfn lib_tool_011_in() {}\n\";",
        // An escaped quote does not end the string.
        "let s = \"\\\"\n#[test]\nfn lib_tool_011_in() {}\n\";",
        "let s = \"\\\\\\\"\n#[test]\nfn lib_tool_011_in() {}\n\";",
        // An escaped backslash does not escape the quote after it.
        "let s = \"\\\\\";",
        // A backslash at the end of a line continues the string.
        "let s = \"\\\n#[test]\nfn lib_tool_011_in() {}\n\";",
        "let s = \"/*\";",
        "let s = \"'\"; let t = \"//\";",
        "let s = b\"\n#[test]\nfn lib_tool_011_in() {}\n\";",
        "let s = b\"\\\"\";",
        "let s = c\"\n#[test]\nfn lib_tool_011_in() {}\n\";",
        "let s = c\"\\\"\";",
    ] {
        assert_rust(
            &then_run(string),
            &[AFTER],
            &[],
            "only the test after the closed string counts",
        );
    }
}

#[test]
fn lib_tool_011_rust_raw_strings_end_only_at_a_quote_with_as_many_hashes() {
    for raw in [
        // A backslash is not an escape in a raw string.
        "let s = r\"\\\";",
        "let s = r#\"\\\"#;",
        "let s = r\"\n#[test]\nfn lib_tool_011_in() {}\n\";",
        // A quote with fewer hashes than the opening does not close it.
        "let s = r#\"\n\"\n#[test]\nfn lib_tool_011_in() {}\n\"#;",
        "let s = r##\"\n\"#\n#[test]\nfn lib_tool_011_in() {}\n\"##;",
        "let s = r###\"\n\"##\n#[test]\nfn lib_tool_011_in() {}\n\"###;",
        // It closes at the first quote followed by as many hashes.
        "let s = r#\"x\"##;",
    ] {
        assert_rust(
            &then_run(raw),
            &[AFTER],
            &[],
            "only the test after the closed raw string counts",
        );
    }
}

#[test]
fn lib_tool_011_rust_byte_and_c_raw_strings_are_raw() {
    for raw in [
        "let s = br\"\\\";",
        "let s = br#\"\n\"\n#[test]\nfn lib_tool_011_in() {}\n\"#;",
        "let s = cr\"\\\";",
        "let s = cr##\"\n\"#\n#[test]\nfn lib_tool_011_in() {}\n\"##;",
    ] {
        assert_rust(
            &then_run(raw),
            &[AFTER],
            &[],
            "`br` and `cr` strings are raw: only the test after the closed string counts",
        );
    }
}

#[test]
fn lib_tool_011_rust_char_and_byte_literals_hold_quotes() {
    for literal in [
        "let q = '\"';",
        "let q = '\\'';",
        "let q = '\\''; let s = \"\n#[test]\nfn lib_tool_011_in() {}\n\";",
        "let q = '\\\\'; let r = '\"';",
        "let q = b'\"';",
        "let q = b'\\'';",
        "let q = '\\u{22}'; let r = '\\x22';",
        "let q = 'é'; let s = \"ü\"; // ß",
        "let q = '/'; let r = '*'; let t = '#';",
    ] {
        assert_rust(
            &then_run(literal),
            &[AFTER],
            &[],
            "a quote in a char or byte literal opens no string",
        );
    }
}

#[test]
fn lib_tool_011_rust_lifetimes_labels_and_raw_identifiers_open_no_literal() {
    for code in [
        "fn f<'a>(x: &'a str) {}",
        "fn g(x: &'static str) {}",
        "fn g(x: &'static str) { let _ = \"'\"; }",
        "fn h() { 'outer: loop { break 'outer; } }",
        "fn k() { 'l: loop {} }",
        "struct S<'a>(&'a str);\nimpl<'a> S<'a> {\n    fn get(&self) -> &'a str { self.0 }\n}",
        "fn p(x: &'_ str) {}",
        // `'a'` is a char literal, not a lifetime.
        "let c = 'a'; let s = \"\n#[test]\nfn lib_tool_011_in() {}\n\";",
        // `r#match` is a raw identifier, not a raw string.
        "let r#match = 1; let r#type = r#match;",
        "let br = 1; let cr = br; let r = cr; let b = r;",
    ] {
        assert_rust(
            &then_run(code),
            &[AFTER],
            &[],
            "a lifetime, a label or a raw identifier opens no literal",
        );
    }
}

#[test]
fn lib_tool_011_rust_an_unterminated_literal_hides_the_rest_without_a_panic() {
    for open in [
        "let s = \"",
        "let s = \"\\\"",
        "let s = r\"",
        "let s = r#\"\n\"",
        "let s = r##\"\n\"#",
        "let s = br#\"",
        "let s = b\"",
        "let s = c\"",
        "let s = cr#\"",
    ] {
        assert_rust(
            &format!("{BEFORE}{open}\n{RUN}"),
            &[BEFORE_NAME],
            &[],
            "a string that is never closed runs to the end of the file",
        );
    }
    assert_rust(
        "#[test]\n#[ignore]\nfn lib_tool_011_ignored_before() {}\nlet s = r#\"\n#[test]\n#[ignore]\nfn lib_tool_011_in() {}\n",
        &[],
        &["lib_tool_011_ignored_before"],
        "nothing after an unterminated raw string is counted as ignored either",
    );
    // Text that ends inside a literal, a comment or an attribute is scanned
    // to its end without a panic.
    for tail in [
        "'",
        "'\\",
        "b'",
        "b'\\",
        "'a",
        "r",
        "r#",
        "br",
        "/",
        "#[",
        "#[test",
        "#[test]",
        "#[test]\n#[ignore",
        "#[cfg_attr(miri, ignore",
        "#[test_case(\"a]",
    ] {
        assert_rust(
            &format!("{BEFORE}{tail}"),
            &[BEFORE_NAME],
            &[],
            "text that ends early keeps the test before it",
        );
    }
}

/// A comment is whitespace: it does not separate an attribute from its
/// function. (Beyond the letter of the criteria; see the slice report.)
#[test]
fn lib_tool_011_rust_comments_between_an_attribute_and_its_function_are_whitespace() {
    for text in [
        "#[test]\n/* note */\nfn lib_tool_011_x() {}\n",
        "#[test]\n/*\n * note\n */\nfn lib_tool_011_x() {}\n",
        "#[test] /* note */ fn lib_tool_011_x() {}\n",
        "/* note */ #[test] fn lib_tool_011_x() {}\n",
        "#[test]\n// note\n/// doc\nfn lib_tool_011_x() {}\n",
    ] {
        assert_rust(
            text,
            &["lib_tool_011_x"],
            &[],
            "a comment between the attribute and the function does not unmark it",
        );
    }
    assert_rust(
        "#[test]\n/* why */\n#[ignore]\n/* note */\nfn lib_tool_011_x() {}\n",
        &[],
        &["lib_tool_011_x"],
        "a comment between the attributes and the function does not hide the ignore",
    );
}

#[test]
fn lib_tool_011_masking_keeps_every_line() {
    // The `#[test]` marks the `const`; the function after the literal, on the
    // next line, is not a test. Joining the literal's lines would put the
    // function on the line after the attribute.
    for text in [
        "#[test]\nconst S: &str = \"\n\"; fn lib_tool_011_x() {}\n",
        "#[test]\nconst S: &str = r#\"\n\"#; fn lib_tool_011_x() {}\n",
        "#[test]\nconst S: () = /*\n*/ (); fn lib_tool_011_x() {}\n",
    ] {
        assert_rust(
            text,
            &[],
            &[],
            "masking keeps the newlines inside a literal or a comment",
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-TOOL-011: Python comments and strings
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_011_python_comments_hide_test_like_text() {
    for text in [
        "# def test_lib_tool_011_in(self):\n",
        "    # async def test_lib_tool_011_in(self):\n",
        "x = 1  # def test_lib_tool_011_in(self):\n",
    ] {
        assert_python(text, &[], &[], "a `def test_` in a comment is not a test");
    }
    for comment in [
        "# \"\"\" opens nothing",
        "# ''' opens nothing",
        "x = 1  # \" opens nothing",
        "x = 1  # ' opens nothing",
    ] {
        assert_python(
            &then_py_run(comment),
            &[PY_AFTER],
            &[],
            "a comment ends at its newline",
        );
    }
}

#[test]
fn lib_tool_011_python_triple_quoted_strings_hide_test_like_text() {
    for text in [
        "\"\"\"\ndef test_lib_tool_011_in(self):\n\"\"\"\n",
        "'''\ndef test_lib_tool_011_in(self):\n'''\n",
        "class T(unittest.TestCase):\n    \"\"\"\n    def test_lib_tool_011_in(self):\n    \"\"\"\n",
        "def helper():\n    '''\n    async def test_lib_tool_011_in(self):\n    '''\n",
    ] {
        assert_python(text, &[], &[], "a `def test_` in a docstring is not a test");
    }
    for string in [
        "\"\"\"\ndef test_lib_tool_011_in(self):\n\"\"\"",
        "x = '''\ndef test_lib_tool_011_in(self):\n'''",
        // One or two quotes do not close a triple-quoted string.
        "\"\"\" \" \"\" \ndef test_lib_tool_011_in(self):\n\"\"\"",
        // Nor does the other kind of triple quote.
        "\"\"\"\n'''\ndef test_lib_tool_011_in(self):\n\"\"\"",
        "'''\n\"\"\"\ndef test_lib_tool_011_in(self):\n'''",
        "x = \"\"\"\"\"\"",
        "x = ''''''",
    ] {
        assert_python(
            &then_py_run(string),
            &[PY_AFTER],
            &[],
            "only the test after the closed triple-quoted string counts",
        );
    }
}

#[test]
fn lib_tool_011_python_single_quoted_strings_hide_quotes_and_comments() {
    for string in [
        "x = \"'''\"",
        "x = '\"\"\"'",
        "x = \"def test_lib_tool_011_in(self):\"",
        // `#` in a string opens no comment, so the triple quote after it opens a string.
        "x = \"#\"; y = \"\"\"\ndef test_lib_tool_011_in(self):\n\"\"\"",
        "x = '#'; y = '''\ndef test_lib_tool_011_in(self):\n'''",
        // An empty string is not the start of a triple-quoted one.
        "x = \"\"",
        "x = ''",
        "x = \"\" + \"\"\"\ndef test_lib_tool_011_in(self):\n\"\"\"",
    ] {
        assert_python(
            &then_py_run(string),
            &[PY_AFTER],
            &[],
            "only the test after the closed string counts",
        );
    }
}

/// Every string prefix of the criteria, in every case.
const PYTHON_PREFIXES: [&str; 24] = [
    "r", "R", "b", "B", "f", "F", "u", "U", "rb", "rB", "Rb", "RB", "br", "bR", "Br", "BR", "fr",
    "fR", "Fr", "FR", "rf", "rF", "Rf", "RF",
];

#[test]
fn lib_tool_011_python_string_prefixes_in_any_case_open_a_string() {
    for prefix in PYTHON_PREFIXES {
        for quotes in ["\"\"\"", "'''"] {
            assert_python(
                &then_py_run(&format!(
                    "x = {prefix}{quotes}\ndef test_lib_tool_011_in(self):\n{quotes}"
                )),
                &[PY_AFTER],
                &[],
                "a prefixed triple-quoted string hides what it holds",
            );
        }
        for string in [
            format!("x = {prefix}\"'''\""),
            format!("x = {prefix}'\"\"\"'"),
        ] {
            assert_python(
                &then_py_run(&string),
                &[PY_AFTER],
                &[],
                "a prefixed single-quoted string hides the quotes it holds",
            );
        }
    }
}

#[test]
fn lib_tool_011_python_a_backslash_escaped_quote_does_not_end_a_string() {
    for prefix in ["", "r", "R", "b", "rb", "Br", "f", "u"] {
        for string in [
            format!("x = {prefix}\"\\\"\"; y = \"\"\"\ndef test_lib_tool_011_in(self):\n\"\"\""),
            format!("x = {prefix}'\\''; y = '''\ndef test_lib_tool_011_in(self):\n'''"),
            format!("{prefix}\"\"\"\\\"\"\"\ndef test_lib_tool_011_in(self):\n\"\"\""),
            // An escaped backslash does not escape the quote after it.
            format!("x = {prefix}\"\\\\\"; y = \"\"\"\ndef test_lib_tool_011_in(self):\n\"\"\""),
        ] {
            assert_python(
                &then_py_run(&string),
                &[PY_AFTER],
                &[],
                "a backslash escapes the next character, raw strings included",
            );
        }
    }
}

#[test]
fn lib_tool_011_python_an_unterminated_string_hides_the_rest_without_a_panic() {
    for open in [
        "\"\"\"",
        "'''",
        "x = rb'''",
        "x = \"\"\"\\\"\"\"",
        "x = f\"\"\"\n\"\"",
    ] {
        assert_python(
            &format!("{PY_BEFORE}{open}\n{PY_RUN}"),
            &[PY_BEFORE_NAME],
            &[],
            "a triple-quoted string that is never closed runs to the end of the file",
        );
    }
    for tail in [
        "\"",
        "'",
        "\"\\",
        "'\\",
        "r\"\\",
        "\"\"\"\\",
        "#",
        "@",
        "@unittest.skip(",
        "@unittest.skip(\"x\")",
        "@unittest.skip(\"x\")\nclass T:",
        "class T:",
        "async",
        "async def",
        "def",
    ] {
        assert_python(
            &format!("{PY_BEFORE}{tail}"),
            &[PY_BEFORE_NAME],
            &[],
            "text that ends early keeps the test before it",
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-TOOL-011: what marks a test (D4) and Python `async def`
// ---------------------------------------------------------------------------

/// Where a test-style attribute `ATTR` may stand above `lib_tool_011_x`.
const TEST_STYLE_TEMPLATES: [&str; 3] = [
    "ATTR\nfn lib_tool_011_x() {}\n",
    "ATTR fn lib_tool_011_x() {}\n",
    "ATTR\n#[should_panic]\npub async fn lib_tool_011_x() {}\n",
];

#[test]
fn lib_tool_011_test_style_attributes_are_named_by_their_last_path_segment() {
    for attribute in [
        "#[test]",
        "#[tokio::test]",
        "#[tokio::test(flavor = \"multi_thread\")]",
        "#[test_log::test]",
        "#[::core::prelude::v1::test]",
        "#[test_case(1; \"a]b\")]",
        "#[test_case(1)]",
        "#[test_case]",
        "#[rstest]",
        "#[rstest::rstest]",
    ] {
        for template in TEST_STYLE_TEMPLATES {
            assert_rust(
                &template.replace("ATTR", attribute),
                &["lib_tool_011_x"],
                &[],
                "an attribute whose last path segment is `test`, `test_case` or `rstest` marks a test",
            );
        }
    }
    assert_rust(
        "#[test_case(1; \"a]b\")]\n#[test_case(2; \"c[d\")]\nfn lib_tool_011_x(n: u8) {}\n",
        &["lib_tool_011_x"],
        &[],
        "brackets in attribute arguments do not unbalance the attribute",
    );
    // Brackets that are code, not literal content, so masking keeps them and
    // only their nesting decides where the attribute closes. An attribute
    // closed early leaves its tail as code on its line, and the `fn` on a
    // later line then does not follow the attribute.
    for (text, why) in [
        (
            "#[test_case([1, 2])]\nfn lib_tool_011_x(pair: [u8; 2]) {}\n",
            "a `]` that closes a nested `[` does not close the attribute",
        ),
        (
            "#[test_case([[1], [2, [3]]])] fn lib_tool_011_x(v: [u8; 2]) {}\n",
            "deeply nested brackets close the attribute at its own `]`, on the line of the `fn`",
        ),
        (
            "#[test_case(\n    [1, [2]],\n)]\nfn lib_tool_011_x(v: [u8; 2]) {}\n",
            "the bracket depth of an attribute carries across its lines",
        ),
        (
            "#[allow([[1], [2, [3]]])] #[test_case([4])] fn lib_tool_011_x(v: [u8; 1]) {}\n",
            "a nested attribute closes at its own `]`, so the test attribute after it starts there",
        ),
    ] {
        assert_rust(text, &["lib_tool_011_x"], &[], why);
    }
    let file = "crates/demo/tests/cases.rs";
    let text = "#[test_case(1; \"a]b\")]\nfn lib_tool_011_x(n: u8) {}\n";
    assert_eq!(
        problems(
            ROW_011,
            &citing(ROW_011, &["crates/demo/tests/cases.rs::lib_tool_011_x"]),
            &[(file, text)]
        ),
        Vec::<String>::new(),
        "LIB-TOOL-011: a cited `#[test_case]` test resolves"
    );
    assert_eq!(
        problems(ROW_011, &uncited(ROW_011), &[(file, text)]),
        [orphan(file, "lib_tool_011_x")],
        "LIB-TOOL-011: an uncited `#[test_case]` test is an orphan"
    );
}

#[test]
fn lib_tool_011_other_attributes_do_not_mark_a_test() {
    for attribute in [
        "#[cfg(test)]",
        "#[mytest]",
        "#[testing]",
        "#[tests]",
        "#[attest]",
        "#[test_helper]",
        "#[test::helper]",
        "#[my_test_case]",
        "#[test_cases]",
        "#[nonrstest]",
        "#[rstest_reuse::template]",
        "#[doc = \"test\"]",
        "#[should_panic(expected = \"test\")]",
        "#[cfg_attr(test, allow(dead_code))]",
        "#[allow(dead_code, reason = \"#[test]\")]",
        // Controls for the whitespace forms (Copilot review of PR #13,
        // `matrix.rs:857`): reading the path past whitespace must still
        // compare whole segments.
        "#[tokio :: testing]",
        "#[tokio :: tests]",
        "#[tokio/* c */::testing]",
        "#[tokio ::\n testing]",
        "#[tokio :: test :: helper]",
        "#[my :: test_helper]",
        "#[my :: attest]",
    ] {
        for template in [
            "ATTR\nfn lib_tool_011_x() {}\n",
            "ATTR fn lib_tool_011_x() {}\n",
        ] {
            assert_rust(
                &template.replace("ATTR", attribute),
                &[],
                &[],
                "only the last path segment decides, and only `test`, `test_case` and `rstest` mark",
            );
        }
    }
    for text in [
        "let s = \"\n#[test_case]\nfn lib_tool_011_in() {}\n\";\n",
        "let s = \"\n#[rstest]\nfn lib_tool_011_in() {}\n\";\n",
        "let s = \"\n#[test_case(1)]\n\"; fn lib_tool_011_in() {}\n",
    ] {
        assert_rust(
            text,
            &[],
            &[],
            "an attribute written in a string marks nothing",
        );
    }
}

#[test]
fn lib_tool_011_python_async_def_test_is_a_test() {
    for text in [
        "async def test_lib_tool_011_x(self):\n    pass\n",
        "class T(unittest.IsolatedAsyncioTestCase):\n    async def test_lib_tool_011_x(self):\n        pass\n",
        // Any run of spaces or tabs may separate `async` from `def`.
        "async  def test_lib_tool_011_x(self):\n    pass\n",
        "async\tdef test_lib_tool_011_x(self):\n    pass\n",
        "class T(unittest.IsolatedAsyncioTestCase):\n    async \t def test_lib_tool_011_x(self):\n        pass\n",
    ] {
        assert_python(
            text,
            &["test_lib_tool_011_x"],
            &[],
            "`async def test_...` is a test",
        );
    }
    for text in [
        "asyncdef test_lib_tool_011_x(self):\n    pass\n",
        "async_def test_lib_tool_011_x(self):\n    pass\n",
        "async def lib_tool_011_helper():\n    pass\n",
        "async def test_helper(self):\n    pass\n",
        "x = 1  # async def test_lib_tool_011_x(self):\n",
        "x = 'async def test_lib_tool_011_x(self):'\n",
        "\"\"\"\nasync def test_lib_tool_011_x(self):\n\"\"\"\n",
    ] {
        assert_python(
            text,
            &[],
            &[],
            "`asyncdef`, and an untraced, helper or non-code `async def`, is not a traced test",
        );
    }
    let file = ".claude/hooks/test_async.py";
    let text = "async def test_lib_tool_011_x(self):\n    pass\n";
    assert_eq!(
        problems(
            ROW_011,
            &citing(
                ROW_011,
                &[".claude/hooks/test_async.py::test_lib_tool_011_x"]
            ),
            &[(file, text)]
        ),
        Vec::<String>::new(),
        "LIB-TOOL-011: a cited `async def test_` resolves"
    );
    assert_eq!(
        problems(ROW_011, &uncited(ROW_011), &[(file, text)]),
        [orphan(file, "test_lib_tool_011_x")],
        "LIB-TOOL-011: an uncited `async def test_` is an orphan"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-011: ignored Rust tests (D4)
// ---------------------------------------------------------------------------

/// Where an ignore-style attribute `ATTR` may stand beside the test-style
/// attribute of `lib_tool_011_x`.
const IGNORE_TEMPLATES: [&str; 6] = [
    "#[test]\nATTR\nfn lib_tool_011_x() {}\n",
    "ATTR\n#[test]\nfn lib_tool_011_x() {}\n",
    "#[test] ATTR fn lib_tool_011_x() {}\n",
    "#[test]\n// why\n\nATTR\nfn lib_tool_011_x() {}\n",
    "#[tokio::test]\nATTR\nasync fn lib_tool_011_x() {}\n",
    "#[test_case(1; \"a]b\")]\nATTR\nfn lib_tool_011_x(n: u8) {}\n",
];

#[test]
fn lib_tool_011_an_ignore_attribute_makes_a_test_ignored() {
    for attribute in [
        "#[ignore]",
        "#[ignore = \"slow\"]",
        "#[cfg_attr(miri, ignore)]",
        "#[cfg_attr(miri, ignore = \"slow under miri\")]",
        "#[cfg_attr(not(target_os = \"linux\"), ignore)]",
        "#[cfg_attr(all(unix, miri), ignore)]",
        "#[cfg_attr(miri, allow(dead_code), ignore)]",
    ] {
        for template in IGNORE_TEMPLATES {
            assert_rust(
                &template.replace("ATTR", attribute),
                &[],
                &["lib_tool_011_x"],
                "an `ignore` attribute, or a `cfg_attr` that lists `ignore`, ignores the test",
            );
        }
    }
}

#[test]
fn lib_tool_011_only_ignore_style_attributes_ignore_a_test() {
    for attribute in [
        "#[ignored]",
        "#[ignore_if(miri)]",
        "#[my::ignore]",
        "#[doc = \"ignore\"]",
        "#[should_panic(expected = \"ignore\")]",
        // `ignore` as or inside the predicate is not listed after it.
        "#[cfg_attr(ignore, allow(dead_code))]",
        "#[cfg_attr(all(miri, ignore), allow(dead_code))]",
        "#[cfg_attr(feature = \"ignore\", allow(dead_code))]",
        "#[cfg_attr(miri, ignored)]",
        "#[cfg_attr(miri, doc = \"ignore\")]",
        "#[cfg_attr(miri, should_panic(expected = \"ignore\"))]",
        // Controls for the whitespace forms (Copilot review of PR #13,
        // `matrix.rs:857`): `ignore` must be the whole path, read past
        // whitespace.
        "#[my_ignore]",
        "#[ignore_x]",
        "#[ ignore_x ]",
        "#[ignore::x]",
        "#[my :: ignore]",
        "#[my/* c */::ignore]",
        "#[cfg_attr(miri, my_ignore)]",
        "#[cfg_attr( miri , ignore_x )]",
        "#[cfg_attr(miri, my :: ignore)]",
        // Not code at all.
        "// #[ignore]",
        "/* #[ignore] */",
        "#[doc = \"#[ignore]\"]",
    ] {
        assert_rust(
            &format!("#[test]\n{attribute}\nfn lib_tool_011_x() {{}}\n"),
            &["lib_tool_011_x"],
            &[],
            "only an `ignore` path, or `ignore` listed after a `cfg_attr` predicate, ignores",
        );
    }
}

#[test]
fn lib_tool_011_an_ignore_attribute_applies_to_one_function_only() {
    assert_rust(
        "#[test]\n#[ignore]\nfn lib_tool_011_a() {}\n#[test]\nfn lib_tool_011_b() {}\n",
        &["lib_tool_011_b"],
        &["lib_tool_011_a"],
        "the ignore does not carry to the next test",
    );
    assert_rust(
        "#[test] #[ignore] fn lib_tool_011_a() {}\n#[test] fn lib_tool_011_b() {}\n",
        &["lib_tool_011_b"],
        &["lib_tool_011_a"],
        "the ignore does not carry to the next test on its own line",
    );
    assert_rust(
        "#[ignore]\nfn helper() {}\n#[test]\nfn lib_tool_011_b() {}\n",
        &["lib_tool_011_b"],
        &[],
        "an ignore on a function that is not a test does not carry",
    );
    assert_rust(
        "#[ignore]\nfn lib_tool_011_not_a_test() {}\n#[test]\n#[ignore]\nfn parses() {}\n",
        &[],
        &[],
        "an ignored function that is not a test, or a test that is not traced, is in neither list",
    );
}

/// The fixture `lib_tool_001_only_marked_tests_count_as_traced` used before
/// slice 0006, which counted the `#[ignore]`d test as a test.
#[test]
fn lib_tool_011_the_ignored_test_lib_tool_001_used_to_count_is_ignored() {
    assert_rust(
        "#[test]\n#[ignore]\nfn lib_cli_001_a() {}\nfn lib_cli_001_helper() {}\nlet s = \"#[test] fn lib_cli_001_in_string() {}\";",
        &[],
        &["lib_cli_001_a"],
        "an `#[ignore]`d test is no longer counted as a running test",
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-011: skipped Python tests (D5)
// ---------------------------------------------------------------------------

/// Every skip decorator form of the criteria.
const SKIPS: [&str; 10] = [
    "@unittest.skip(\"x\")",
    "@skip(\"x\")",
    "@skip",
    "@unittest.skipIf(True, \"x\")",
    "@skipIf(sys.platform == \"win32\", \"x\")",
    "@unittest.skipUnless(False, \"x\")",
    "@pytest.mark.skip(reason=\"x\")",
    "@pytest.mark.skipif(True, reason=\"x\")",
    "@unittest.expectedFailure",
    "@expectedFailure",
];

/// Where a skip decorator `SKIP` may stand above `test_lib_tool_011_x`.
const SKIP_DEF_TEMPLATES: [&str; 6] = [
    "SKIP\ndef test_lib_tool_011_x(self):\n    pass\n",
    "class T(unittest.TestCase):\n    SKIP\n    def test_lib_tool_011_x(self):\n        pass\n",
    "SKIP\n\n# why\n\ndef test_lib_tool_011_x(self):\n    pass\n",
    "SKIP\n@mock.patch(\"a.b\")\ndef test_lib_tool_011_x(self, b):\n    pass\n",
    "@mock.patch(\"a.b\")\nSKIP\ndef test_lib_tool_011_x(self, b):\n    pass\n",
    "SKIP\nasync def test_lib_tool_011_x(self):\n    pass\n",
];

#[test]
fn lib_tool_011_a_skip_decorator_skips_the_function_below_it() {
    for skip in SKIPS {
        for template in SKIP_DEF_TEMPLATES {
            assert_python(
                &template.replace("SKIP", skip),
                &[],
                &["test_lib_tool_011_x"],
                "a skip decorator on the lines directly above a test skips it",
            );
        }
    }
}

#[test]
fn lib_tool_011_other_decorators_and_text_do_not_skip() {
    for decorator in [
        "@unittest.skipped",
        "@my_skip",
        "@skip_if_slow",
        "@unittest.skip_helper",
        "@expectedFailures",
        "@skip.helper",
        "@mock.patch(\"a.skip\")",
        "@pytest.mark.parametrize(\"skip\", [1])",
        "@pytest.mark.parametrize(\"x\", [pytest.param(1, marks=pytest.mark.skip)])",
        // Controls for the whitespace forms (Copilot review of PR #13,
        // `matrix.rs:914`): reading the dotted name past whitespace must
        // still compare the whole last segment.
        "@unittest.skipper",
        "@unittest. skipper",
        "@unittest .skipper(\"x\")",
        "@unittest . skip_helper",
        "@pytest.mark . skipped",
        "@ my_skip",
        "@ skip_if_slow",
    ] {
        assert_python(
            &format!("{decorator}\ndef test_lib_tool_011_x(self):\n    pass\n"),
            &["test_lib_tool_011_x"],
            &[],
            "only the last segment of the decorator's dotted name decides",
        );
    }
    for text in [
        "# @unittest.skip(\"x\")\ndef test_lib_tool_011_x(self):\n    pass\n",
        "x = \"\"\"\n@unittest.skip(\"x\")\n\"\"\"\ndef test_lib_tool_011_x(self):\n    pass\n",
        "@unittest.skip(\"x\")\ndef helper():\n    pass\ndef test_lib_tool_011_x(self):\n    pass\n",
        "@unittest.skip(\"x\")\nx = 1\ndef test_lib_tool_011_x(self):\n    pass\n",
        "@unittest.skip(\"x\")\nclass T:\n    pass\ndef test_lib_tool_011_x(self):\n    pass\n",
    ] {
        assert_python(
            text,
            &["test_lib_tool_011_x"],
            &[],
            "a skip in a comment or a string, or on something else, does not skip the test",
        );
    }
}

#[test]
fn lib_tool_011_a_skip_decorator_on_a_class_skips_its_test_methods() {
    assert_python(
        "@unittest.skip(\"x\")\nclass T(unittest.TestCase):\n    def test_lib_tool_011_a(self):\n        pass\n\n    async def test_lib_tool_011_b(self):\n        pass\n\n    @mock.patch(\"a.b\")\n    def test_lib_tool_011_c(self, b):\n        pass\n\n    def helper(self):\n        def test_lib_tool_011_d():\n            pass\n",
        &[],
        &[
            "test_lib_tool_011_a",
            "test_lib_tool_011_b",
            "test_lib_tool_011_c",
            "test_lib_tool_011_d",
        ],
        "every `def test_` indented under a skipped class is skipped, in source order",
    );
    assert_python(
        "@pytest.mark.skipif(True, reason=\"x\")\n\n# why\n@dataclass\nclass T:\n    def test_lib_tool_011_a(self):\n        pass\n",
        &[],
        &["test_lib_tool_011_a"],
        "a skip decorator above other decorators, blank and comment lines skips the class",
    );
    for skip in SKIPS {
        assert_python(
            &format!(
                "{skip}\nclass T(unittest.TestCase):\n    def test_lib_tool_011_a(self):\n        pass\n"
            ),
            &[],
            &["test_lib_tool_011_a"],
            "every skip decorator skips a class",
        );
    }
}

#[test]
fn lib_tool_011_a_skipped_class_ends_at_the_next_line_at_its_indent_or_less() {
    for (text, running, skipped) in [
        (
            "@unittest.skip(\"x\")\nclass T(unittest.TestCase):\n    def test_lib_tool_011_a(self):\n        pass\nclass U(unittest.TestCase):\n    def test_lib_tool_011_b(self):\n        pass\n",
            &["test_lib_tool_011_b"][..],
            &["test_lib_tool_011_a"][..],
        ),
        (
            "@unittest.skip(\"x\")\nclass T:\n    def test_lib_tool_011_a(self):\n        pass\n\ndef test_lib_tool_011_b():\n    pass\n",
            &["test_lib_tool_011_b"][..],
            &["test_lib_tool_011_a"][..],
        ),
        (
            "@unittest.skip(\"x\")\nclass T:\n    def test_lib_tool_011_a(self):\n        pass\n@mock.patch(\"a.b\")\ndef test_lib_tool_011_b(b):\n    pass\n",
            &["test_lib_tool_011_b"][..],
            &["test_lib_tool_011_a"][..],
        ),
        (
            "@unittest.skip(\"x\")\nclass T:\n    pass\nx = 1\ndef test_lib_tool_011_b():\n    pass\n",
            &["test_lib_tool_011_b"][..],
            &[][..],
        ),
        (
            "class Outer:\n    @unittest.skip(\"x\")\n    class Inner(unittest.TestCase):\n        def test_lib_tool_011_a(self):\n            pass\n    def test_lib_tool_011_b(self):\n        pass\ndef test_lib_tool_011_c():\n    pass\n",
            &["test_lib_tool_011_b", "test_lib_tool_011_c"][..],
            &["test_lib_tool_011_a"][..],
        ),
        (
            "if True:\n    @unittest.skip(\"x\")\n    class Inner(unittest.TestCase):\n        def test_lib_tool_011_a(self):\n            pass\ndef test_lib_tool_011_b():\n    pass\n",
            &["test_lib_tool_011_b"][..],
            &["test_lib_tool_011_a"][..],
        ),
        (
            "@skip\nclass A:\n    def test_lib_tool_011_a(self): pass\n@skip\nclass B:\n    def test_lib_tool_011_b(self): pass\ndef test_lib_tool_011_c(): pass\n",
            &["test_lib_tool_011_c"][..],
            &["test_lib_tool_011_a", "test_lib_tool_011_b"][..],
        ),
    ] {
        assert_python(
            text,
            running,
            skipped,
            "a skipped class ends at the next non-blank line at its indent or less",
        );
    }
}

/// Comment lines and string contents are not lines of code, so they do not
/// end a skipped class. (Python agrees; see the slice report.)
#[test]
fn lib_tool_011_comment_and_string_lines_do_not_end_a_skipped_class() {
    assert_python(
        "@unittest.skip(\"x\")\nclass T(unittest.TestCase):\n    def test_lib_tool_011_a(self):\n        pass\n# a comment at the class's indent\n    def test_lib_tool_011_b(self):\n        x = \"\"\"\nnot indented\n        \"\"\"\n\n    def test_lib_tool_011_c(self):\n        pass\ndef test_lib_tool_011_d():\n    pass\n",
        &["test_lib_tool_011_d"],
        &[
            "test_lib_tool_011_a",
            "test_lib_tool_011_b",
            "test_lib_tool_011_c",
        ],
        "a comment line or a string line at column 0 does not end the class",
    );
}

/// A class nested in a skipped class, skipped or not, does not move the end
/// of the outer skip to its own indent: the outer class's later tests stay
/// skipped (verification 0006, F2 and R6).
#[test]
fn lib_tool_011_a_class_nested_in_a_skipped_class_keeps_the_outer_skip() {
    for (text, why) in [
        (
            "@unittest.skip(\"x\")\nclass Outer(unittest.TestCase):\n    @unittest.skip(\"y\")\n    class Inner(unittest.TestCase):\n        def test_lib_tool_011_a(self):\n            pass\n    def test_lib_tool_011_b(self):\n        pass\ndef test_lib_tool_011_c():\n    pass\n",
            "a skipped class nested in a skipped class keeps the outer class's indent as the end \
             of the skip",
        ),
        (
            "@unittest.skip(\"x\")\nclass Outer(unittest.TestCase):\n    class Inner(unittest.TestCase):\n        def test_lib_tool_011_a(self):\n            pass\n    def test_lib_tool_011_b(self):\n        pass\ndef test_lib_tool_011_c():\n    pass\n",
            "an undecorated class nested in a skipped class neither ends nor moves the skip",
        ),
    ] {
        assert_python(
            text,
            &["test_lib_tool_011_c"],
            &["test_lib_tool_011_a", "test_lib_tool_011_b"],
            why,
        );
    }
}

/// The control for `lib_tool_011_a_class_nested_in_a_skipped_class_keeps_the_outer_skip`:
/// a nested class line, at a deeper indent, starts no skip of its own.
#[test]
fn lib_tool_011_a_class_nested_in_a_class_that_is_not_skipped_starts_no_skip() {
    assert_python(
        "class Outer(unittest.TestCase):\n    class Inner(unittest.TestCase):\n        def test_lib_tool_011_a(self):\n            pass\n    def test_lib_tool_011_b(self):\n        pass\ndef test_lib_tool_011_c():\n    pass\n",
        &[
            "test_lib_tool_011_a",
            "test_lib_tool_011_b",
            "test_lib_tool_011_c",
        ],
        &[],
        "a nested class without a skip decorator, in a class without one, skips nothing",
    );
    assert_python(
        "class Outer(unittest.TestCase):\n    @unittest.skip(\"x\")\n    def test_lib_tool_011_a(self):\n        pass\n    class Inner(unittest.TestCase):\n        def test_lib_tool_011_b(self):\n            pass\n    def test_lib_tool_011_c(self):\n        pass\n",
        &["test_lib_tool_011_b", "test_lib_tool_011_c"],
        &["test_lib_tool_011_a"],
        "a skip decorator on a method does not carry to a nested class after it",
    );
}

/// A skip decorator or an ignore attribute that spans lines, as formatters
/// write long ones, never leaves its test counted as running. Whether it is
/// reported as ignored is left open (see the slice report). The first case
/// has the shape of a test in `.claude/hooks/test_agent_write_guard.py`.
#[test]
fn lib_tool_011_a_skip_or_ignore_spanning_lines_never_counts_as_running() {
    for text in [
        "class T(unittest.TestCase):\n    @unittest.skipUnless(\n        os.name == \"nt\", \"Windows only\"\n    )\n    def test_lib_tool_011_x(self):\n        pass\n",
        "@pytest.mark.skipif(\n    True,\n    reason=\"x\",\n)\ndef test_lib_tool_011_x():\n    pass\n",
        "@unittest.skipIf(\n    True, \"x\"\n)\nclass T(unittest.TestCase):\n    def test_lib_tool_011_x(self):\n        pass\n",
    ] {
        let found = scan(ROW_011, text, true);
        assert!(
            !found.running.iter().any(|n| n == "test_lib_tool_011_x"),
            "LIB-TOOL-011: a skip decorator spanning lines must not leave its test running; \
             in {text:?} found {found:?}"
        );
    }
    for text in [
        "#[test]\n#[cfg_attr(\n    miri,\n    ignore = \"slow under miri\"\n)]\nfn lib_tool_011_x() {}\n",
        "#[test]\n#[ignore = \"a long reason \\\n    that continues\"]\nfn lib_tool_011_x() {}\n",
    ] {
        let found = scan(ROW_011, text, false);
        assert!(
            !found.running.iter().any(|n| n == "lib_tool_011_x"),
            "LIB-TOOL-011: an ignore attribute spanning lines must not leave its test running; \
             in {text:?} found {found:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-TOOL-011: whitespace inside attribute paths, decorator names and class
// lines (Copilot review of PR #13)
// ---------------------------------------------------------------------------

/// Test-style attributes whose path has whitespace or a comment between its
/// segments, or is split across lines. Rust reads a path as tokens, so each
/// is the same attribute as its unspaced form (`matrix.rs:857`).
const SPACED_TEST_ATTRIBUTES: [&str; 16] = [
    "#[tokio :: test]",
    "#[tokio ::test]",
    "#[tokio:: test]",
    "#[tokio\t::\ttest]",
    "#[tokio  ::  test]",
    "#[tokio/* comment */::test]",
    "#[tokio::/* comment */test]",
    "#[tokio /* a */ :: /* b */ test]",
    "#[tokio::\n test]",
    "#[tokio\n    ::test]",
    "#[tokio ::\n    test(flavor = \"multi_thread\")]",
    "#[tokio :: test(flavor = \"multi_thread\")]",
    "#[test_log :: test]",
    "#[:: core :: prelude :: v1 :: test]",
    "#[rstest :: rstest]",
    "#[test_case :: test_case(1; \"a]b\")]",
];

#[test]
fn lib_tool_011_whitespace_and_comments_inside_an_attribute_path_do_not_split_it() {
    for attribute in SPACED_TEST_ATTRIBUTES {
        for template in TEST_STYLE_TEMPLATES {
            assert_rust(
                &template.replace("ATTR", attribute),
                &["lib_tool_011_x"],
                &[],
                "whitespace or a comment between the segments of an attribute path does not \
                 end the path, so its last segment `test`, `test_case` or `rstest` marks a test",
            );
        }
        for template in [
            "ATTR\n#[ignore]\nfn lib_tool_011_x() {}\n",
            "#[cfg_attr(miri, ignore)]\nATTR\nasync fn lib_tool_011_x() {}\n",
            "ATTR #[ignore = \"slow\"] fn lib_tool_011_x() {}\n",
        ] {
            assert_rust(
                &template.replace("ATTR", attribute),
                &[],
                &["lib_tool_011_x"],
                "a test marked by a spaced attribute path is seen, so its ignore attribute \
                 makes it ignored",
            );
        }
    }
}

/// The over-accepting side of `matrix.rs:857`: a path whose first segment is
/// `test`, `rstest` or `ignore` but which goes on past whitespace is a
/// longer path, and is judged by its whole self.
#[test]
fn lib_tool_011_an_attribute_path_is_read_past_whitespace_before_it_is_judged() {
    for attribute in [
        "#[test :: helper]",
        "#[test/* c */::helper]",
        "#[test ::\n helper]",
        "#[rstest :: fixture]",
        "#[test_case :: helper]",
    ] {
        for template in TEST_STYLE_TEMPLATES {
            assert_rust(
                &template.replace("ATTR", attribute),
                &[],
                &[],
                "a path that goes on past `test`, `test_case` or `rstest` is not test-style",
            );
        }
    }
    for attribute in [
        "#[ignore :: x]",
        "#[ignore/* c */::x]",
        "#[ignore ::\n x]",
        "#[cfg_attr(miri, ignore :: x)]",
        "#[cfg_attr( miri , ignore /* c */ :: x )]",
    ] {
        for template in IGNORE_TEMPLATES {
            assert_rust(
                &template.replace("ATTR", attribute),
                &["lib_tool_011_x"],
                &[],
                "a path that goes on past `ignore` is not ignore-style, in an attribute or \
                 listed in a `cfg_attr`",
            );
        }
    }
}

/// Ignore-style attributes with whitespace or comments wherever Rust allows
/// them. They hold today; they guard the `matrix.rs:857` fix.
#[test]
fn lib_tool_011_whitespace_inside_an_ignore_attribute_still_ignores() {
    for attribute in [
        "#[ ignore ]",
        "#[ignore ]",
        "#[\tignore\t]",
        "#[ ignore = \"slow\" ]",
        "#[ignore=\"slow\"]",
        "#[ignore/* c */]",
        "#[/* c */ignore]",
        "#[\nignore\n]",
        "#[cfg_attr( miri , ignore )]",
        "#[cfg_attr (miri, ignore)]",
        "#[cfg_attr(miri,ignore)]",
        "#[cfg_attr(\tmiri,\tignore\t)]",
        "#[cfg_attr(miri, /* c */ ignore)]",
        "#[cfg_attr( miri , ignore = \"slow\" )]",
        "#[cfg_attr( all( unix , miri ) , ignore )]",
        "#[ cfg_attr(miri, ignore) ]",
        "#[cfg_attr(\n    miri ,\n    ignore ,\n)]",
    ] {
        for template in IGNORE_TEMPLATES {
            assert_rust(
                &template.replace("ATTR", attribute),
                &[],
                &["lib_tool_011_x"],
                "whitespace and comments inside an `ignore` or `cfg_attr(.., ignore)` attribute \
                 do not hide the ignore",
            );
        }
    }
}

/// Rust also allows whitespace between the `#` and the `[` of an attribute
/// (beyond the letter of the review comment; see the report).
#[test]
fn lib_tool_011_whitespace_between_the_hash_and_the_bracket_still_opens_an_attribute() {
    for attribute in [
        "# [test]",
        "#\t[test]",
        "#  [tokio::test]",
        "#/* c */[rstest]",
    ] {
        for template in TEST_STYLE_TEMPLATES {
            assert_rust(
                &template.replace("ATTR", attribute),
                &["lib_tool_011_x"],
                &[],
                "`# [` opens an attribute, so the test-style attribute marks a test",
            );
        }
    }
    for attribute in [
        "# [ignore]",
        "#\t[ignore = \"slow\"]",
        "# [cfg_attr(miri, ignore)]",
    ] {
        for template in IGNORE_TEMPLATES {
            assert_rust(
                &template.replace("ATTR", attribute),
                &[],
                &["lib_tool_011_x"],
                "`# [` opens an attribute, so the ignore attribute ignores the test",
            );
        }
    }
}

/// Skip decorators whose dotted name has whitespace around a `.`, or after
/// the `@`. Python reads the name as tokens, so each is the same decorator
/// as its unspaced form (`matrix.rs:914`).
const SPACED_SKIPS: [&str; 13] = [
    "@unittest. skip(\"x\")",
    "@unittest .skip(\"x\")",
    "@unittest . skip(\"x\")",
    "@unittest\t.\tskip(\"x\")",
    "@unittest .skipIf(True, \"x\")",
    "@unittest. skipUnless(False, \"x\")",
    "@pytest.mark . skip(reason=\"x\")",
    "@pytest . mark . skipif(True, reason=\"x\")",
    "@unittest .expectedFailure",
    "@ unittest.skip(\"x\")",
    "@\tskip",
    "@ skip(\"x\")",
    "@unittest.skip (\"x\")",
];

#[test]
fn lib_tool_011_whitespace_inside_a_dotted_decorator_name_does_not_split_it() {
    for skip in SPACED_SKIPS {
        for template in SKIP_DEF_TEMPLATES {
            assert_python(
                &template.replace("SKIP", skip),
                &[],
                &["test_lib_tool_011_x"],
                "whitespace in a skip decorator's dotted name does not end the name, so the \
                 function below is skipped",
            );
        }
        assert_python(
            &format!(
                "{skip}\nclass T(unittest.TestCase):\n    def test_lib_tool_011_a(self):\n        pass\n\n    async def test_lib_tool_011_b(self):\n        pass\ndef test_lib_tool_011_c():\n    pass\n"
            ),
            &["test_lib_tool_011_c"],
            &["test_lib_tool_011_a", "test_lib_tool_011_b"],
            "whitespace in a skip decorator's dotted name does not end the name, so the class \
             below is skipped",
        );
        assert_python(
            &format!(
                "class Outer:\n    {skip}\n    class Inner(unittest.TestCase):\n        def test_lib_tool_011_a(self):\n            pass\n    def test_lib_tool_011_b(self):\n        pass\n"
            ),
            &["test_lib_tool_011_b"],
            &["test_lib_tool_011_a"],
            "a spaced skip decorator on a nested class skips that class only",
        );
    }
}

/// The over-accepting side of `matrix.rs:914`: a dotted name that goes on
/// past `skip` after whitespace has a different last segment.
#[test]
fn lib_tool_011_a_decorator_name_is_read_past_whitespace_before_it_is_judged() {
    for decorator in [
        "@skip .helper",
        "@skip\t.helper",
        "@skip. helper",
        "@unittest.skip .helper",
        "@unittest . skip . helper",
        "@expectedFailure . wrapped",
        "@pytest.mark.skipif .helper(True)",
    ] {
        assert_python(
            &format!("{decorator}\ndef test_lib_tool_011_x(self):\n    pass\n"),
            &["test_lib_tool_011_x"],
            &[],
            "a dotted name that goes on past a skip segment is not a skip",
        );
        assert_python(
            &format!(
                "{decorator}\nclass T(unittest.TestCase):\n    def test_lib_tool_011_x(self):\n        pass\n"
            ),
            &["test_lib_tool_011_x"],
            &[],
            "a dotted name that goes on past a skip segment does not skip a class",
        );
    }
}

/// Class lines with a tab or several spaces after `class`. Python accepts
/// any run of spaces and tabs there (`matrix.rs:928`).
#[test]
fn lib_tool_011_any_whitespace_after_class_starts_a_skipped_class() {
    for class in [
        "class\tT(unittest.TestCase):",
        "class\t\tT(unittest.TestCase):",
        "class \tT(unittest.TestCase):",
        "class\t T:",
        "class  T(unittest.TestCase):",
        "class    T:",
    ] {
        assert_python(
            &format!(
                "@unittest.skip(\"x\")\n{class}\n    def test_lib_tool_011_a(self):\n        pass\n\n    def test_lib_tool_011_b(self):\n        pass\ndef test_lib_tool_011_c():\n    pass\n"
            ),
            &["test_lib_tool_011_c"],
            &["test_lib_tool_011_a", "test_lib_tool_011_b"],
            "a skip decorator above a class line with tabs or several spaces after `class` \
             skips every traced method of the class",
        );
        assert_python(
            &format!(
                "if True:\n    @pytest.mark.skipif(True, reason=\"x\")\n    {class}\n        def test_lib_tool_011_a(self):\n            pass\n    def test_lib_tool_011_b(self):\n        pass\n"
            ),
            &["test_lib_tool_011_b"],
            &["test_lib_tool_011_a"],
            "an indented class line with tabs or several spaces after `class` is a class too",
        );
    }
}

/// A line that starts with the letters `class` but not the keyword is not a
/// class line, so a skip above it opens no skipped class. The lines after it
/// are lexical fixtures: Python itself would reject a decorator there. They
/// hold today; they guard the `matrix.rs:928` fix.
#[test]
fn lib_tool_011_a_word_that_starts_with_class_is_not_a_class_line() {
    for line in [
        "classy = 1",
        "class_ = 1",
        "classify(x)",
        "classify = staticmethod(f)",
        "classes: list = []",
        "classmethod(f)",
        "class_\t= 1",
    ] {
        assert_python(
            &format!(
                "@unittest.skip(\"x\")\n{line}\n    def test_lib_tool_011_x(self):\n        pass\n"
            ),
            &["test_lib_tool_011_x"],
            &[],
            "an identifier that starts with `class` is not the keyword, so no class is skipped",
        );
    }
    for def in ["def classify(self):", "async def classify(self):"] {
        assert_python(
            &format!(
                "@unittest.skip(\"x\")\n{def}\n    def test_lib_tool_011_x(self):\n        pass\n"
            ),
            &["test_lib_tool_011_x"],
            &[],
            "a skip on a function named `classify` skips that function, not a class",
        );
    }
}

/// The three review comments through `check`: the spaced and tabbed forms
/// give the same problems as their plain forms.
#[test]
fn lib_tool_011_spaced_attribute_decorator_and_class_forms_give_the_plain_forms_problems() {
    let spaced_rs = (
        "crates/demo/tests/spaced.rs",
        "#[tokio :: test]\nasync fn lib_tool_011_spaced() {}\n",
    );
    assert_eq!(
        problems(
            ROW_011,
            &citing(
                ROW_011,
                &["crates/demo/tests/spaced.rs::lib_tool_011_spaced"]
            ),
            &[spaced_rs]
        ),
        Vec::<String>::new(),
        "LIB-TOOL-011: a cited `#[tokio :: test]` test resolves"
    );
    assert_eq!(
        problems(ROW_011, &uncited(ROW_011), &[spaced_rs]),
        [orphan(spaced_rs.0, "lib_tool_011_spaced")],
        "LIB-TOOL-011: an uncited `#[tokio :: test]` test is an orphan"
    );
    let spaced_py = (
        ".claude/hooks/test_spaced.py",
        "@unittest. skip(\"x\")\ndef test_lib_tool_011_spaced(self):\n    pass\n",
    );
    let tabbed_py = (
        ".claude/hooks/test_tabbed.py",
        "@unittest.skip(\"x\")\nclass\tT(unittest.TestCase):\n    def test_lib_tool_011_tabbed(self):\n        pass\n",
    );
    assert_eq!(
        problems(ROW_011, &uncited(ROW_011), &[spaced_py, tabbed_py]),
        [
            ignored(spaced_py.0, "test_lib_tool_011_spaced"),
            ignored(tabbed_py.0, "test_lib_tool_011_tabbed"),
        ],
        "LIB-TOOL-011: a test under `@unittest. skip` or in a skipped `class\\t` is one \
         ignored-or-skipped problem, not an orphan"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-011: whitespace after `def` and `fn` (D12)
// ---------------------------------------------------------------------------

/// Python `def` lines with tabs or several spaces after `def`, or between
/// `async` and `def` and after `def`. Python reads keywords as tokens, so each
/// defines `test_lib_tool_011_x`.
const SPACED_DEFS: [&str; 6] = [
    "def\ttest_lib_tool_011_x(self):",
    "def  test_lib_tool_011_x(self):",
    "def \ttest_lib_tool_011_x(self):",
    "async def\ttest_lib_tool_011_x(self):",
    "async  def  test_lib_tool_011_x(self):",
    "async\tdef\t\ttest_lib_tool_011_x(self):",
];

#[test]
fn lib_tool_011_python_any_whitespace_after_def_defines_a_test() {
    for def in SPACED_DEFS {
        assert_python(
            &format!("{def}\n    pass\n"),
            &["test_lib_tool_011_x"],
            &[],
            "a run of spaces or tabs after `def` still defines the test",
        );
        assert_python(
            &format!("class T(unittest.TestCase):\n    {def}\n        pass\n"),
            &["test_lib_tool_011_x"],
            &[],
            "a method with spaces or tabs after `def` is a test too",
        );
        for skip in [
            "@unittest.skip(\"x\")",
            "@pytest.mark.skipif(True, reason=\"x\")",
        ] {
            assert_python(
                &format!("{skip}\n{def}\n    pass\n"),
                &[],
                &["test_lib_tool_011_x"],
                "a skip above a `def` with spaces or tabs after the keyword is honoured",
            );
            assert_python(
                &format!(
                    "{skip}\nclass T(unittest.TestCase):\n    {def}\n        pass\ndef test_lib_tool_011_after():\n    pass\n"
                ),
                &["test_lib_tool_011_after"],
                &["test_lib_tool_011_x"],
                "a skipped class skips a method with spaces or tabs after `def`",
            );
        }
        let file = ".claude/hooks/test_spaced_def.py";
        let text = format!("{def}\n    pass\n");
        assert_eq!(
            problems(
                ROW_011,
                &citing(
                    ROW_011,
                    &[".claude/hooks/test_spaced_def.py::test_lib_tool_011_x"]
                ),
                &[(file, &text)]
            ),
            Vec::<String>::new(),
            "LIB-TOOL-011: a cited test defined by {def:?} resolves with no problem"
        );
    }
}

/// Rust `fn` lines under `#[test]` with tabs or several spaces around `fn`.
/// Rust reads keywords as tokens, so each defines `lib_tool_011_x`.
const SPACED_FNS: [&str; 6] = [
    "fn\tlib_tool_011_x() {}",
    "fn  lib_tool_011_x() {}",
    "fn \tlib_tool_011_x() {}",
    "pub  fn\tlib_tool_011_x() {}",
    "pub(crate)  fn  lib_tool_011_x() {}",
    "async\tfn\tlib_tool_011_x() {}",
];

#[test]
fn lib_tool_011_rust_any_whitespace_after_fn_defines_a_test() {
    for function in SPACED_FNS {
        for text in [
            format!("#[test]\n{function}\n"),
            format!("#[test] {function}\n"),
            format!("#[tokio::test]\n#[should_panic]\n{function}\n"),
        ] {
            assert_rust(
                &text,
                &["lib_tool_011_x"],
                &[],
                "a run of spaces or tabs after `fn` still defines the test",
            );
        }
        assert_rust(
            &format!("#[test]\n#[ignore]\n{function}\n"),
            &[],
            &["lib_tool_011_x"],
            "an ignore above an `fn` with spaces or tabs after the keyword is honoured",
        );
        let file = "crates/demo/tests/spaced_fn.rs";
        let text = format!("#[test]\n{function}\n");
        assert_eq!(
            problems(
                ROW_011,
                &citing(ROW_011, &["crates/demo/tests/spaced_fn.rs::lib_tool_011_x"]),
                &[(file, &text)]
            ),
            Vec::<String>::new(),
            "LIB-TOOL-011: a cited test defined by {function:?} resolves with no problem"
        );
    }
}

/// Words that contain `def` or `fn` but are not the keyword define nothing.
/// These are lexical fixtures: neither language accepts an identifier right
/// after `define` or `fnord`, nor `fn` as a field name, so no valid source has
/// these lines; they show what reading past whitespace must not accept.
#[test]
fn lib_tool_011_a_word_that_contains_def_or_fn_is_not_the_keyword() {
    for line in [
        "define test_lib_tool_011_x(self):",
        "defx test_lib_tool_011_x(self):",
        "define\ttest_lib_tool_011_x(self):",
        "async define test_lib_tool_011_x(self):",
        "self.def test_lib_tool_011_x(self):",
    ] {
        assert_python(
            &format!("{line}\n    pass\n"),
            &[],
            &[],
            "a word that only starts with `def`, or a `def` after a `.`, defines nothing",
        );
        assert_python(
            &format!("@unittest.skip(\"x\")\n{line}\n    pass\n"),
            &[],
            &[],
            "a skip above a word that only starts with `def` lists nothing",
        );
    }
    for line in [
        "fnord lib_tool_011_x() {}",
        "fnord\tlib_tool_011_x() {}",
        "myfn  lib_tool_011_x() {}",
        "fn_ lib_tool_011_x() {}",
        "self.fn lib_tool_011_x() {}",
        "self.fn\tlib_tool_011_x() {}",
        "r#fn lib_tool_011_x() {}",
    ] {
        for text in [format!("#[test]\n{line}\n"), format!("#[test] {line}\n")] {
            assert_rust(
                &text,
                &[],
                &[],
                "a word that only contains `fn`, a field `.fn` or the raw identifier `r#fn` \
                 defines nothing",
            );
        }
    }
}

/// Python lines where `def` is glued to a traced test name (F6, P1). With no
/// space or tab after it, `def` is not the keyword: `deftest_lib_tool_011_x`
/// is one identifier, so the first line is a call statement. Unlike `define`,
/// the text after `def` is itself a test name, so only the whitespace
/// boundary keeps these from defining `test_lib_tool_011_x`.
const GLUED_DEFS: [&str; 3] = [
    "deftest_lib_tool_011_x()",
    "deftest_lib_tool_011_x(self):",
    "async deftest_lib_tool_011_x(self):",
];

#[test]
fn lib_tool_011_python_def_glued_to_a_test_name_defines_no_test() {
    for line in GLUED_DEFS {
        assert_python(
            &format!("{line}\n    pass\n"),
            &[],
            &[],
            "`def` glued to a test name is not the keyword, so it defines no test",
        );
        assert_python(
            &format!("class T(unittest.TestCase):\n    {line}\n        pass\n"),
            &[],
            &[],
            "in a class body, `def` glued to a test name defines no test method",
        );
        assert_python(
            &format!("@unittest.skip(\"x\")\n{line}\n    pass\n"),
            &[],
            &[],
            "a skip above `def` glued to a test name lists no skipped test",
        );
        assert_python(
            &format!(
                "class T(unittest.TestCase):\n    @unittest.skip(\"x\")\n    {line}\n        pass\n"
            ),
            &[],
            &[],
            "a skip above a class-body `def` glued to a test name lists no skipped test",
        );
        assert_python(
            &format!(
                "@unittest.skip(\"x\")\nclass T(unittest.TestCase):\n    {line}\n        pass\n"
            ),
            &[],
            &[],
            "a skipped class whose body has `def` glued to a test name lists no skipped test",
        );
    }
}

/// Rust items where `fn` is glued to a traced test name (F6, P2). With no
/// space or tab after it, `fn` is not the keyword: `fnlib_tool_011_x` is one
/// identifier. Unlike `fnord`, the text after `fn` is itself a test name, so
/// only the whitespace boundary keeps these from defining `lib_tool_011_x`.
const GLUED_FNS: [(&str, &str); 4] = [
    ("#[test]\nfnlib_tool_011_x() {}\n", "below `#[test]`"),
    ("#[test] fnlib_tool_011_x() {}\n", "on the `#[test]` line"),
    ("#[test]\npub fnlib_tool_011_x() {}\n", "after `pub`"),
    (
        "#[test]\n#[ignore]\nfnlib_tool_011_x() {}\n",
        "below `#[ignore]`",
    ),
];

#[test]
fn lib_tool_011_rust_fn_glued_to_a_test_name_defines_no_test() {
    for (text, place) in GLUED_FNS {
        assert_rust(
            text,
            &[],
            &[],
            &format!(
                "`fn` glued to a test name {place} is not the keyword, so it defines no running \
                 and no ignored test"
            ),
        );
    }
}

/// F6, P3: a citation of a name that appears only glued to `fn` or `def` is
/// the "is not defined" problem, not a resolved test and not "is not a test".
#[test]
fn lib_tool_011_a_citation_of_a_name_glued_to_def_or_fn_is_not_defined() {
    for (file, name, text) in [
        (
            "crates/demo/tests/x.rs",
            "lib_tool_011_x",
            "#[test]\nfnlib_tool_011_x() {}\n",
        ),
        (
            ".claude/hooks/test_x.py",
            "test_lib_tool_011_x",
            "deftest_lib_tool_011_x()\n",
        ),
    ] {
        let citation = format!("{file}::{name}");
        assert_eq!(
            problems(
                ROW_011,
                &citing(ROW_011, &[citation.as_str()]),
                &[(file, text)]
            ),
            [format!(
                "{ROW_011}: test function `{name}` is not defined in {file}"
            )],
            "LIB-TOOL-011: a name glued to `fn` or `def` is not defined by it; in {text:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-TOOL-011: the API
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_011_running_and_ignored_traced_tests_are_listed_apart_in_source_order() {
    assert_rust(
        "#[test] fn lib_cli_001_runs() {}",
        &["lib_cli_001_runs"],
        &[],
        "`traced_tests` keeps its signature and its example",
    );
    let rust = "#[test]\nfn lib_tool_011_z() {}\n/* #[test] fn lib_tool_011_hidden() {} */\n#[test]\n#[ignore]\nfn lib_tool_011_y() {}\nconst S: &str = r#\"\n#[test]\nfn lib_tool_011_hidden() {}\n\"#;\n#[test]\nfn lib_tool_011_m() {}\n#[test]\n#[ignore = \"slow\"]\nfn lib_tool_011_b() {}\n#[test]\n#[ignore]\nfn helper_ignored() {}\n#[test]\nfn helper() {}\n#[test]\nfn lib_tool_011_a() {}\n";
    let python = "def test_lib_tool_011_z(self):\n    pass\n@unittest.skip(\"x\")\ndef test_lib_tool_011_y(self):\n    pass\n\"\"\"\ndef test_lib_tool_011_hidden(self):\n\"\"\"\ndef test_lib_tool_011_m(self):\n    pass\n@unittest.skip(\"x\")\nclass T(unittest.TestCase):\n    def test_lib_tool_011_b(self):\n        pass\n    def test_skipped_helper(self):\n        pass\ndef test_lib_tool_011_a(self):\n    pass\n";
    for (rust, python) in [
        (rust.to_owned(), python.to_owned()),
        (rust.replace('\n', "\r\n"), python.replace('\n', "\r\n")),
    ] {
        assert_rust(
            &rust,
            &["lib_tool_011_z", "lib_tool_011_m", "lib_tool_011_a"],
            &["lib_tool_011_y", "lib_tool_011_b"],
            "running and ignored traced tests are listed apart, each in source order, \
             with LF or CRLF line ends",
        );
        assert_python(
            &python,
            &[
                "test_lib_tool_011_z",
                "test_lib_tool_011_m",
                "test_lib_tool_011_a",
            ],
            &["test_lib_tool_011_y", "test_lib_tool_011_b"],
            "running and skipped traced tests are listed apart, each in source order, \
             with LF or CRLF line ends",
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-TOOL-011: problems for ignored and skipped tests (D3)
// ---------------------------------------------------------------------------

const SLOW_RS: (&str, &str) = (
    "crates/demo/tests/slow.rs",
    "#[test]\n#[ignore]\nfn lib_tool_011_slow() {}\n",
);
const SKIPPED_PY: (&str, &str) = (
    ".claude/hooks/test_skipped.py",
    "@unittest.skip(\"x\")\ndef test_lib_tool_011_skipped(self):\n    pass\n",
);

#[test]
fn lib_tool_011_an_ignored_or_skipped_traced_test_is_one_problem_cited_or_not() {
    let slow = ignored(SLOW_RS.0, "lib_tool_011_slow");
    let skipped = ignored(SKIPPED_PY.0, "test_lib_tool_011_skipped");
    let uncited_matrix = uncited(ROW_011);
    assert_eq!(
        problems(ROW_011, &uncited_matrix, &[SLOW_RS]),
        std::slice::from_ref(&slow),
        "LIB-TOOL-011: an uncited ignored test is one problem, not also an orphan"
    );
    assert_eq!(
        problems(ROW_011, &uncited_matrix, &[SKIPPED_PY]),
        std::slice::from_ref(&skipped),
        "LIB-TOOL-011: an uncited skipped test is one problem, not also an orphan"
    );
    assert_eq!(
        problems(ROW_011, &uncited_matrix, &[SLOW_RS, SKIPPED_PY]),
        [skipped, slow],
        "LIB-TOOL-011: one problem per ignored or skipped test, sorted by file"
    );
}

#[test]
fn lib_tool_011_a_citation_of_an_ignored_test_gives_no_row_problem() {
    let slow = ignored(SLOW_RS.0, "lib_tool_011_slow");
    let skipped = ignored(SKIPPED_PY.0, "test_lib_tool_011_skipped");
    let runs = (
        "crates/demo/tests/runs.rs",
        "#[test]\nfn lib_tool_011_runs() {}\n",
    );
    let matrix = citing(
        ROW_011,
        &[
            "crates/demo/tests/slow.rs::lib_tool_011_slow",
            ".claude/hooks/test_skipped.py::test_lib_tool_011_skipped",
            "crates/demo/tests/runs.rs::lib_tool_011_runs",
        ],
    );
    assert_eq!(
        problems(ROW_011, &matrix, &[SLOW_RS, SKIPPED_PY, runs]),
        [skipped, slow],
        "LIB-TOOL-011: a cited ignored or skipped test gives the same one problem, and no \
         row-level citation problem"
    );
}

#[test]
fn lib_tool_011_comment_and_literal_text_defines_no_cited_test() {
    for (file, text) in [
        ("crates/demo/tests/t.rs", "/*\nfn lib_tool_011_x() {}\n*/\n"),
        ("crates/demo/tests/t.rs", "// fn lib_tool_011_x() {}\n"),
        ("crates/demo/tests/t.rs", "/// fn lib_tool_011_x() {}\n"),
        (
            "crates/demo/tests/t.rs",
            "let s = \"\nfn lib_tool_011_x() {}\n\";\n",
        ),
        (
            "crates/demo/tests/t.rs",
            "let s = r#\"\nfn lib_tool_011_x() {}\n\"#;\n",
        ),
        (
            "crates/demo/tests/t.rs",
            "const S: &str = \"fn lib_tool_011_x() {}\";\n",
        ),
        (
            ".claude/hooks/h.py",
            "\"\"\"\ndef test_lib_tool_011_x(self):\n\"\"\"\n",
        ),
        (".claude/hooks/h.py", "# def test_lib_tool_011_x(self):\n"),
        (
            ".claude/hooks/h.py",
            "x = 'def test_lib_tool_011_x(self):'\n",
        ),
    ] {
        let name = if is_python(file) {
            "test_lib_tool_011_x"
        } else {
            "lib_tool_011_x"
        };
        let citation = format!("{file}::{name}");
        assert_eq!(
            problems(
                ROW_011,
                &citing(ROW_011, &[citation.as_str()]),
                &[(file, text)]
            ),
            [format!(
                "{ROW_011}: test function `{name}` is not defined in {file}"
            )],
            "LIB-TOOL-011: a definition in a comment or a literal defines nothing; in {text:?}"
        );
    }
    for text in [
        "/*\n#[test]\n*/\nfn lib_tool_011_x() {}\n",
        "let s = \"\n#[test]\n\"; fn lib_tool_011_x() {}\n",
        "// #[test]\nfn lib_tool_011_x() {}\n",
    ] {
        assert_eq!(
            problems(
                ROW_011,
                &citing(ROW_011, &["crates/demo/tests/t.rs::lib_tool_011_x"]),
                &[("crates/demo/tests/t.rs", text)]
            ),
            [format!(
                "{ROW_011}: function `lib_tool_011_x` in crates/demo/tests/t.rs is not a test: it has no \
                 `#[test]`-style attribute"
            )],
            "LIB-TOOL-011: an attribute in a comment or a literal marks nothing; in {text:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-TOOL-007: traced tests under `src`
// ---------------------------------------------------------------------------

const INLINE_RS: &str = "#[test]\nfn lib_tool_007_inline() {}\n";
const INLINE_IGNORED_RS: &str = "#[test]\n#[ignore]\nfn lib_tool_007_inline() {}\n";
const INLINE_PY: &str = "def test_lib_tool_007_inline(self):\n    pass\n";
const INLINE_SKIPPED_PY: &str =
    "@unittest.skip(\"x\")\ndef test_lib_tool_007_inline(self):\n    pass\n";

/// The test name and the running and ignored fixture texts for a file.
fn inline_fixture(file: &str) -> (&'static str, [&'static str; 2]) {
    if is_python(file) {
        ("test_lib_tool_007_inline", [INLINE_PY, INLINE_SKIPPED_PY])
    } else {
        ("lib_tool_007_inline", [INLINE_RS, INLINE_IGNORED_RS])
    }
}

#[test]
fn lib_tool_007_a_traced_test_under_src_is_one_problem_cited_ignored_or_not() {
    for file in [
        "crates/sacm/src/lib.rs",
        "crates/x/src/a/b.rs",
        "xtask/src/matrix.rs",
        "crates/x/src/tests/t.rs",
        "crates/x/src/test_inline.py",
        ".claude/hooks/src/test_inline.py",
    ] {
        let (name, texts) = inline_fixture(file);
        let citation = format!("{file}::{name}");
        for text in texts {
            for matrix in [uncited(ROW_007), citing(ROW_007, &[citation.as_str()])] {
                assert_eq!(
                    problems(ROW_007, &matrix, &[(file, text)]),
                    [under_src(file, name)],
                    "LIB-TOOL-007: a traced test under `src` is exactly one problem, whether \
                     or not it is cited or ignored; in {text:?}, matrix {matrix:?}"
                );
            }
        }
    }
    // A leading `src` component counts too. `src/lib.rs` is outside the
    // scanned test files (D10), so citing it is also a row problem; the
    // in-memory repository lists its test all the same.
    let file = "src/lib.rs";
    for text in [INLINE_RS, INLINE_IGNORED_RS] {
        assert_eq!(
            problems(ROW_007, &uncited(ROW_007), &[(file, text)]),
            [under_src(file, "lib_tool_007_inline")],
            "LIB-TOOL-007: a leading `src` component counts; in {text:?}"
        );
        assert_eq!(
            problems(
                ROW_007,
                &citing(ROW_007, &["src/lib.rs::lib_tool_007_inline"]),
                &[(file, text)]
            ),
            [
                outside_scan(ROW_007, file),
                under_src(file, "lib_tool_007_inline")
            ],
            "LIB-TOOL-007: a cited test under a leading `src` gives the scan-scope row problem \
             and the one `src` problem; in {text:?}"
        );
    }
}

#[test]
fn lib_tool_007_only_a_path_component_equal_to_src_counts() {
    for file in [
        "crates/x/source/a.rs",
        "crates/x/srcs/a.rs",
        "crates/x/my_src/a.rs",
        "crates/x/src_old/a.rs",
        "crates/x/tests/src.rs",
        "srcs/a.rs",
        "crates/x/srcs/test_a.py",
    ] {
        let (name, [running, ignored_text]) = inline_fixture(file);
        assert_eq!(
            problems(ROW_007, &uncited(ROW_007), &[(file, running)]),
            [orphan(file, name)],
            "LIB-TOOL-007: {file} is not under a `src` directory, so its uncited test is an \
             orphan"
        );
        assert_eq!(
            problems(ROW_007, &uncited(ROW_007), &[(file, ignored_text)]),
            [ignored(file, name)],
            "LIB-TOOL-007: {file} is not under a `src` directory, so its ignored test is \
             reported as ignored"
        );
    }
}

#[test]
fn lib_tool_007_untraced_unit_tests_and_doctests_under_src_are_allowed() {
    let lib = (
        "crates/x/src/lib.rs",
        "//! ```\n//! assert!(true);\n//! ```\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn parses() {}\n\n    #[test]\n    #[ignore]\n    fn parses_slowly() {}\n}\n",
    );
    let python = (
        "crates/x/src/test_unit.py",
        "def test_parses(self):\n    pass\n",
    );
    for matrix in [
        uncited(ROW_007),
        citing(ROW_007, &["doctest:crates/x/src/lib.rs"]),
    ] {
        assert_eq!(
            problems(ROW_007, &matrix, &[lib, python]),
            Vec::<String>::new(),
            "LIB-TOOL-007: untraced unit tests and a doctest citation under `src` are allowed; \
             matrix {matrix:?}"
        );
    }
}

/// Pairs as owned strings.
fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(f, n)| ((*f).to_owned(), (*n).to_owned()))
        .collect()
}

#[test]
fn lib_tool_007_scan_problems_come_together_sorted_by_file_then_name() {
    // Sorting the messages as strings would order these differently: `t.rs.rs:`
    // sorts before `t.rs:`, and `` `lib_tool_007_a` `` before `` `lib_tool_007` ``.
    let running = [
        ("crates/b/src/lib.rs", "lib_tool_007_a"),
        ("crates/a/tests/t.rs.rs", "lib_tool_007_a"),
        ("crates/a/tests/t.rs", "lib_tool_007_b"),
        ("crates/a/tests/u.rs", "lib_tool_007_a"),
        ("crates/a/src/lib.rs", "lib_tool_007"),
    ];
    let ignored_list = [
        ("crates/a/tests/u.rs", "lib_tool_007_b"),
        ("crates/b/src/lib.rs", "lib_tool_007"),
        ("crates/a/tests/t.rs", "lib_tool_007_a"),
        ("crates/a/src/lib.rs", "lib_tool_007_a"),
    ];
    let scan_problems = [
        under_src("crates/a/src/lib.rs", "lib_tool_007"),
        under_src("crates/a/src/lib.rs", "lib_tool_007_a"),
        ignored("crates/a/tests/t.rs", "lib_tool_007_a"),
        orphan("crates/a/tests/t.rs", "lib_tool_007_b"),
        orphan("crates/a/tests/t.rs.rs", "lib_tool_007_a"),
        orphan("crates/a/tests/u.rs", "lib_tool_007_a"),
        ignored("crates/a/tests/u.rs", "lib_tool_007_b"),
        under_src("crates/b/src/lib.rs", "lib_tool_007"),
        under_src("crates/b/src/lib.rs", "lib_tool_007_a"),
    ];
    let matrix = load(&format!(
        "{HEAD}{}",
        row_toml(ROW_007, "planned", "depends_on = [\"LIB-TOOL-007\"]")
    ))
    .expect("LIB-TOOL-007: the fixture matrix parses");
    let mut expected = vec![
        "LIB-TOOL-007: depends on itself".to_owned(),
        "dependency cycle: LIB-TOOL-007 -> LIB-TOOL-007".to_owned(),
    ];
    expected.extend(scan_problems.iter().cloned());
    expected.push(format!(
        "{RENDERED_PATH} is out of date; run `cargo xtask matrix render`"
    ));
    let mut reversed_running = pairs(&running);
    reversed_running.reverse();
    let mut reversed_ignored = pairs(&ignored_list);
    reversed_ignored.reverse();
    for (running, ignored_pairs) in [
        (pairs(&running), pairs(&ignored_list)),
        (reversed_running, reversed_ignored),
    ] {
        let repo = ListedRepo {
            page: "stale".to_owned(),
            running,
            ignored: ignored_pairs,
        };
        assert_eq!(
            check(&matrix, &repo),
            expected,
            "LIB-TOOL-007: the `src`, ignored and orphan problems come together after the \
             row and cycle problems and before the page problem, sorted by file and then by \
             function name, whatever order the repository lists them in"
        );
    }
}

// ---------------------------------------------------------------------------
// Through `FsRepo` and `cmd_check` on a tree on disk
// ---------------------------------------------------------------------------

/// The problems `check` finds on the tree through `FsRepo`.
fn problems_on_disk(tree: &TempTree) -> Vec<String> {
    let matrix = load(GATE_TOML).expect("LIB-TOOL-007: the gate fixture parses");
    check(&matrix, &FsRepo::new(tree.root()))
}

fn sorted(mut list: Vec<(String, String)>) -> Vec<(String, String)> {
    list.sort();
    list
}

#[test]
fn lib_tool_007_fs_repo_reports_traced_tests_under_src_directories() {
    let tree = consistent_tree("scanner-src");
    tree.write(
        "crates/demo/src/lib.rs",
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn parses() {}\n\n    #[test]\n    fn lib_tool_007_inline() {}\n}\n",
    )
    .write(
        "crates/demo/src/a/b.rs",
        "#[test]\n#[ignore]\nfn lib_tool_007_ignored_inline() {}\n",
    )
    .write(
        "xtask/src/extra.rs",
        "#[test] fn lib_tool_007_in_xtask_src() {}\n",
    )
    .write(
        ".claude/hooks/src/test_inline.py",
        "def test_lib_tool_007_python_in_src(self):\n    pass\n",
    )
    .write(
        "crates/demo/src/test_skipped.py",
        "@unittest.skip(\"x\")\ndef test_lib_tool_007_skipped_in_src(self):\n    pass\n",
    );
    assert_eq!(
        problems_on_disk(&tree),
        [
            under_src(
                ".claude/hooks/src/test_inline.py",
                "test_lib_tool_007_python_in_src"
            ),
            under_src("crates/demo/src/a/b.rs", "lib_tool_007_ignored_inline"),
            under_src("crates/demo/src/lib.rs", "lib_tool_007_inline"),
            under_src(
                "crates/demo/src/test_skipped.py",
                "test_lib_tool_007_skipped_in_src"
            ),
            under_src("xtask/src/extra.rs", "lib_tool_007_in_xtask_src"),
        ],
        "LIB-TOOL-007: every traced test under a `src` directory on disk is reported once, \
         running or ignored, Rust or Python, and the untraced unit test is not"
    );
    assert!(
        cmd_check(tree.root(), &mut io::sink()).is_err(),
        "LIB-TOOL-007: `matrix check` fails when a traced test is under `src`"
    );
}

#[test]
fn lib_tool_007_fs_repo_near_miss_directories_are_not_src() {
    let tree = consistent_tree("scanner-near-src");
    let rust_test = "#[test]\nfn lib_tool_007_near_miss() {}\n";
    tree.write("crates/demo/source/a.rs", rust_test)
        .write("crates/demo/srcs/a.rs", rust_test)
        .write("crates/demo/my_src/a.rs", rust_test)
        .write("crates/demo/tests/src.rs", rust_test)
        .write("xtask/src_old/a.rs", rust_test)
        .write(
            "crates/demo/srcs/test_a.py",
            "def test_lib_tool_007_near_miss(self):\n    pass\n",
        )
        .write(
            "crates/demo/src/lib.rs",
            "#[cfg(test)]\nmod tests {\n    #[test]\n    fn parses() {}\n}\n",
        );
    assert_eq!(
        problems_on_disk(&tree),
        [
            orphan("crates/demo/my_src/a.rs", "lib_tool_007_near_miss"),
            orphan("crates/demo/source/a.rs", "lib_tool_007_near_miss"),
            orphan("crates/demo/srcs/a.rs", "lib_tool_007_near_miss"),
            orphan("crates/demo/srcs/test_a.py", "test_lib_tool_007_near_miss"),
            orphan("crates/demo/tests/src.rs", "lib_tool_007_near_miss"),
            orphan("xtask/src_old/a.rs", "lib_tool_007_near_miss"),
        ],
        "LIB-TOOL-007: only a path component equal to `src` counts; these are orphans, and \
         the untraced unit test under `src` is allowed"
    );
}

#[test]
fn lib_tool_011_fs_repo_lists_ignored_and_skipped_tests_in_every_scan_root() {
    let ignored_rust = "#[test]\n#[ignore]\nfn lib_tool_011_hidden() {}\n";
    let skipped_python = "@unittest.skip(\"x\")\ndef test_lib_tool_011_hidden(self):\n    pass\n";
    let tree = TempTree::new("scanner-ignored");
    tree.write(
        "crates/demo/tests/slow.rs",
        "#[test]\nfn lib_tool_011_runs() {}\n#[test]\n#[ignore]\nfn lib_tool_011_slow() {}\n",
    )
    .write(
        ".claude/hooks/test_skipped.py",
        "import unittest\n\n@unittest.skip(\"x\")\nclass T(unittest.TestCase):\n    def test_lib_tool_011_b(self):\n        pass\n\n    def test_lib_tool_011_a(self):\n        pass\n",
    )
    .write(
        "xtask/tests/flaky.rs",
        "#[test]\n#[cfg_attr(miri, ignore)]\nfn lib_tool_011_flaky() {}\n",
    )
    // Build output (a `target` directory beside a `Cargo.toml`, D9),
    // non-source files and other roots are not scanned.
    .write("crates/demo/Cargo.toml", "[package]\n")
    .write("crates/demo/target/debug/x.rs", ignored_rust)
    .write("crates/demo/tests/x.txt", ignored_rust)
    .write("docs/x.rs", ignored_rust)
    .write(".claude/agents/test_x.py", skipped_python)
    // No `Cargo.toml` sits beside this `target`, so it is scanned (D9).
    .write(".claude/hooks/target/test_x.py", skipped_python);
    let repo = FsRepo::new(tree.root());
    assert_eq!(
        sorted(repo.ignored_functions()),
        pairs(&[
            (".claude/hooks/target/test_x.py", "test_lib_tool_011_hidden"),
            (".claude/hooks/test_skipped.py", "test_lib_tool_011_a"),
            (".claude/hooks/test_skipped.py", "test_lib_tool_011_b"),
            ("crates/demo/tests/slow.rs", "lib_tool_011_slow"),
            ("xtask/tests/flaky.rs", "lib_tool_011_flaky"),
        ]),
        "LIB-TOOL-011: `FsRepo::ignored_functions` finds the ignored and skipped traced tests \
         of the scan roots, as `/`-separated repository-relative paths"
    );
    assert_eq!(
        sorted(repo.traced_functions()),
        pairs(&[("crates/demo/tests/slow.rs", "lib_tool_011_runs")]),
        "LIB-TOOL-011: `FsRepo::traced_functions` lists only the traced tests that run"
    );
}

#[test]
fn lib_tool_011_cmd_check_rejects_ignored_and_skipped_traced_tests_on_disk() {
    let tree = consistent_tree("scanner-ignored-check");
    tree.write(
        "crates/demo/tests/slow.rs",
        "#[test]\nfn lib_tool_011_runs() {}\n#[test]\n#[ignore]\nfn lib_tool_011_slow() {}\n",
    )
    .write(
        ".claude/hooks/test_skipped.py",
        "@unittest.skip(\"x\")\nclass T(unittest.TestCase):\n    def test_lib_tool_011_b(self):\n        pass\n\n    def test_lib_tool_011_a(self):\n        pass\n",
    );
    assert_eq!(
        problems_on_disk(&tree),
        [
            ignored(".claude/hooks/test_skipped.py", "test_lib_tool_011_a"),
            ignored(".claude/hooks/test_skipped.py", "test_lib_tool_011_b"),
            orphan("crates/demo/tests/slow.rs", "lib_tool_011_runs"),
            ignored("crates/demo/tests/slow.rs", "lib_tool_011_slow"),
        ],
        "LIB-TOOL-011: on disk, each ignored or skipped traced test is one problem, sorted \
         with the orphans"
    );
    assert!(
        cmd_check(tree.root(), &mut io::sink()).is_err(),
        "LIB-TOOL-011: `matrix check` fails on an ignored or skipped traced test"
    );
}

// ---------------------------------------------------------------------------
// Fix round 1: build output (D9) and the scanned test files (D10), on disk
// ---------------------------------------------------------------------------

/// The `Cargo.toml` of the workspace root.
const ROOT_CARGO_TOML: &str = "Cargo.toml";
/// The `Cargo.toml` of the package `crates/demo`.
const DEMO_CARGO_TOML: &str = "crates/demo/Cargo.toml";

/// A tree whose matrix is `toml`, with its rendered page in sync, and an
/// empty `Cargo.toml` at each path in `cargo_tomls`.
fn matrix_tree(test: &str, toml: &str, cargo_tomls: &[&str]) -> TempTree {
    let matrix = load(toml).expect("LIB-TOOL-011: the fixture matrix parses");
    let tree = TempTree::new(test);
    tree.write(MATRIX_PATH, toml)
        .write(RENDERED_PATH, &render(&matrix));
    for cargo_toml in cargo_tomls {
        tree.write(cargo_toml, "[package]\n");
    }
    tree
}

/// `check` through `FsRepo` on `tree`, whose matrix is `toml`, gives exactly
/// `expected`, and `cmd_check` fails exactly when that list is not empty.
#[track_caller]
fn assert_on_disk(row: &str, tree: &TempTree, toml: &str, expected: &[String], why: &str) {
    let matrix = load(toml).unwrap_or_else(|e| panic!("{row}: the fixture matrix parses: {e}"));
    assert_eq!(
        check(&matrix, &FsRepo::new(tree.root())),
        expected,
        "{row}: {why}; matrix {toml:?}"
    );
    let result = cmd_check(tree.root(), &mut io::sink());
    assert_eq!(
        result.is_ok(),
        expected.is_empty(),
        "{row}: `matrix check` passes exactly when there is no problem; {why}; got {result:?}"
    );
}

/// The uncited and the citing matrix for the test `file::name` of `row`.
fn uncited_and_citing(row: &str, file: &str, name: &str) -> [String; 2] {
    let citation = format!("{file}::{name}");
    [uncited(row), citing(row, &[citation.as_str()])]
}

/// R3, cases 1 and 2: a `target` module directory under `src` is compiled,
/// not build output, so its traced test is scanned and gets the `src`
/// problem, running or ignored, cited or not, and no row problem.
#[test]
fn lib_tool_007_a_traced_test_in_a_target_module_under_src_is_one_problem() {
    let file = "crates/demo/src/target/mod.rs";
    let name = "lib_tool_007_in_target";
    for text in [
        "#[test]\nfn lib_tool_007_in_target() {}\n",
        "#[test]\n#[ignore]\nfn lib_tool_007_in_target() {}\n",
    ] {
        for toml in uncited_and_citing(ROW_007, file, name) {
            // `crates/demo/target` would be build output; `crates/demo/src/target` is not.
            let tree = matrix_tree(
                "scanner-src-target",
                &toml,
                &[ROOT_CARGO_TOML, DEMO_CARGO_TOML],
            );
            tree.write(file, text);
            assert_on_disk(
                ROW_007,
                &tree,
                &toml,
                &[under_src(file, name)],
                &format!(
                    "a traced test in {file} is scanned (no `Cargo.toml` sits beside that \
                     `target`) and is exactly the `src` problem; in {text:?}"
                ),
            );
        }
    }
}

/// R3, cases 3 and 4: an ignored or skipped traced test in a `target`
/// directory that no `Cargo.toml` sits beside is scanned and reported once,
/// cited or not.
#[test]
fn lib_tool_011_an_ignored_test_in_a_target_directory_without_a_cargo_toml_is_one_problem() {
    for (file, name, text) in [
        (
            "crates/demo/tests/target/mod.rs",
            "lib_tool_011_in_target",
            "#[test]\n#[ignore]\nfn lib_tool_011_in_target() {}\n",
        ),
        (
            "crates/demo/tests/a/target/mod.rs",
            "lib_tool_011_in_target",
            "#[test]\n#[ignore]\nfn lib_tool_011_in_target() {}\n",
        ),
        (
            ".claude/hooks/target/test_x.py",
            "test_lib_tool_011_in_target",
            "@unittest.skip(\"x\")\ndef test_lib_tool_011_in_target(self):\n    pass\n",
        ),
    ] {
        for toml in uncited_and_citing(ROW_011, file, name) {
            let tree = matrix_tree(
                "scanner-ignored-target",
                &toml,
                &[ROOT_CARGO_TOML, DEMO_CARGO_TOML],
            );
            tree.write(file, text);
            assert_on_disk(
                ROW_011,
                &tree,
                &toml,
                &[ignored(file, name)],
                &format!(
                    "{file} is scanned (no `Cargo.toml` sits beside that `target`) and its \
                     ignored or skipped test is exactly one problem"
                ),
            );
        }
    }
}

/// Build output in every place it occurs, holding running, ignored and
/// skipped traced tests.
const BUILD_OUTPUT: [(&str, &str); 6] = [
    (
        "target/debug/x.rs",
        "#[test]\n#[ignore]\nfn lib_tool_011_built() {}\n",
    ),
    (
        "crates/demo/target/x.rs",
        "#[test]\n#[ignore]\nfn lib_tool_011_built() {}\n",
    ),
    (
        "crates/demo/target/debug/deps/x.rs",
        "#[test]\nfn lib_tool_011_built() {}\n",
    ),
    (
        "crates/demo/target/test_x.py",
        "@unittest.skip(\"x\")\ndef test_lib_tool_011_built(self):\n    pass\n",
    ),
    (
        "xtask/target/x.rs",
        "#[test]\n#[ignore]\nfn lib_tool_011_built() {}\n",
    ),
    (
        ".claude/hooks/target/test_x.py",
        "@unittest.skip(\"x\")\ndef test_lib_tool_011_built(self):\n    pass\n",
    ),
];

/// R3, case 5: a `target` directory beside a `Cargo.toml` is build output
/// and is not scanned, at the root and in every scan root.
#[test]
fn lib_tool_011_build_output_beside_a_cargo_toml_is_not_scanned() {
    let tree = consistent_tree("scanner-build-output");
    for cargo_toml in [
        ROOT_CARGO_TOML,
        DEMO_CARGO_TOML,
        "xtask/Cargo.toml",
        ".claude/hooks/Cargo.toml",
    ] {
        tree.write(cargo_toml, "[package]\n");
    }
    for (file, text) in BUILD_OUTPUT {
        tree.write(file, text);
    }
    let repo = FsRepo::new(tree.root());
    assert_eq!(
        repo.ignored_functions(),
        Vec::<(String, String)>::new(),
        "LIB-TOOL-011: build output lists no ignored or skipped test"
    );
    assert_eq!(
        sorted(repo.traced_functions()),
        sorted(pairs(&[
            ("crates/demo/tests/gate.rs", "lib_tool_001_cited"),
            (".claude/hooks/test_gate.py", "test_lib_tool_001_cited"),
        ])),
        "LIB-TOOL-011: build output lists no running traced test"
    );
    assert_on_disk(
        ROW_011,
        &tree,
        GATE_TOML,
        &[],
        "a `target` directory beside a `Cargo.toml` is build output: nothing in it is scanned",
    );
}

/// The control for `lib_tool_011_build_output_beside_a_cargo_toml_is_not_scanned`:
/// the same `target` directories, with a `Cargo.toml` only above them, in
/// them or beside their parent, are scanned.
#[test]
fn lib_tool_011_a_target_directory_without_a_cargo_toml_beside_it_is_scanned() {
    let tree = consistent_tree("scanner-not-build-output");
    for cargo_toml in [
        ROOT_CARGO_TOML,
        "crates/Cargo.toml",
        "crates/demo/target/Cargo.toml",
        "xtask/target/Cargo.toml",
    ] {
        tree.write(cargo_toml, "[package]\n");
    }
    let rust = "#[test]\n#[ignore]\nfn lib_tool_011_built() {}\n";
    tree.write("crates/demo/target/x.rs", rust)
        .write("xtask/target/debug/x.rs", rust);
    assert_on_disk(
        ROW_011,
        &tree,
        GATE_TOML,
        &[
            ignored("crates/demo/target/x.rs", "lib_tool_011_built"),
            ignored("xtask/target/debug/x.rs", "lib_tool_011_built"),
        ],
        "a `target` directory with no `Cargo.toml` directly beside it is scanned",
    );
}

/// Whether `file` is read as Python: its extension is `.py` in any case.
fn reads_as_python(file: &str) -> bool {
    Path::new(file)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("py"))
}

/// A running traced test of LIB-TOOL-011 for `file`: its name and its text,
/// in the language `file` is read as.
fn running_fixture(file: &str) -> (&'static str, &'static str) {
    if reads_as_python(file) {
        (
            "test_lib_tool_011_x",
            "def test_lib_tool_011_x(self):\n    pass\n",
        )
    } else {
        ("lib_tool_011_x", "#[test]\nfn lib_tool_011_x() {}\n")
    }
}

/// `(cited file, Cargo.toml files)`: test files the scan does not cover
/// (D10), each beside its `Cargo.toml` where that makes it build output.
const OUTSIDE_THE_SCAN: [(&str, &[&str]); 16] = [
    // Not under `crates/`, `xtask/` or `.claude/hooks/`.
    ("tests/x.rs", &[ROOT_CARGO_TOML]),
    ("x.rs", &[ROOT_CARGO_TOML]),
    ("docs/x.rs", &[]),
    ("cratesx/tests/x.rs", &[]),
    ("xtask2/tests/x.rs", &[]),
    (".claude/hooks_old/test_x.py", &[]),
    (".claude/agents/test_x.py", &[]),
    // Not `.rs` or `.py`.
    ("crates/demo/tests/x.txt", &[]),
    ("crates/demo/tests/x.rs.txt", &[]),
    ("crates/demo/tests/x", &[]),
    (".claude/hooks/test_x.pyc", &[]),
    // Under build output (D9).
    ("crates/demo/target/x.rs", &[DEMO_CARGO_TOML]),
    ("crates/demo/target/debug/deps/x.rs", &[DEMO_CARGO_TOML]),
    ("crates/demo/target/test_x.py", &[DEMO_CARGO_TOML]),
    ("xtask/target/x.rs", &["xtask/Cargo.toml"]),
    (
        ".claude/hooks/target/test_x.py",
        &[".claude/hooks/Cargo.toml"],
    ),
];

/// R4: a test citation of an existing file that holds the cited test, but
/// that the scan does not cover, is exactly the scan-scope row problem.
#[test]
fn lib_tool_011_a_citation_of_a_file_outside_the_scanned_test_files_is_a_row_problem() {
    for (file, cargo_tomls) in OUTSIDE_THE_SCAN {
        let (name, text) = running_fixture(file);
        let citation = format!("{file}::{name}");
        let toml = citing(ROW_011, &[citation.as_str()]);
        let tree = matrix_tree("scanner-outside", &toml, cargo_tomls);
        tree.write(file, text);
        assert_on_disk(
            ROW_011,
            &tree,
            &toml,
            &[outside_scan(ROW_011, file)],
            &format!(
                "{file} holds the cited test, but the scan does not cover it, so the citation \
                 is the one scan-scope problem"
            ),
        );
    }
}

/// `(cited file, Cargo.toml files)`: test files the scan covers.
const INSIDE_THE_SCAN: [(&str, &[&str]); 9] = [
    (
        "crates/demo/tests/x.rs",
        &[ROOT_CARGO_TOML, DEMO_CARGO_TOML],
    ),
    ("crates/x.rs", &[]),
    ("xtask/tests/x.rs", &["xtask/Cargo.toml"]),
    (".claude/hooks/test_x.py", &[]),
    // The extension is compared in any case.
    ("crates/demo/tests/x.RS", &[DEMO_CARGO_TOML]),
    (".claude/hooks/test_x.PY", &[]),
    // A `target` directory with no `Cargo.toml` directly beside it (D9).
    (
        "crates/demo/tests/target/x.rs",
        &[ROOT_CARGO_TOML, DEMO_CARGO_TOML],
    ),
    ("crates/demo/target/x.rs", &[ROOT_CARGO_TOML]),
    (".claude/hooks/target/test_x.py", &[ROOT_CARGO_TOML]),
];

/// R4, the other way: a citation of a file the scan covers gives no
/// scan-scope problem, and its running test passes the check.
#[test]
fn lib_tool_011_a_citation_of_a_scanned_test_file_is_no_problem() {
    for (file, cargo_tomls) in INSIDE_THE_SCAN {
        let (name, text) = running_fixture(file);
        let citation = format!("{file}::{name}");
        let toml = citing(ROW_011, &[citation.as_str()]);
        let tree = matrix_tree("scanner-inside", &toml, cargo_tomls);
        tree.write(file, text);
        assert_on_disk(
            ROW_011,
            &tree,
            &toml,
            &[],
            &format!("{file} is a scanned test file, so citing its running test is no problem"),
        );
    }
}

/// R4: a citation of a missing file outside the scanned test files is the
/// scan-scope problem alone. Like a path that `check_path` rejects, it skips
/// the citation's other checks (the file is not looked up), except the test
/// name check, which comes first. A missing file inside the scanned test
/// files is still "does not exist".
#[test]
fn lib_tool_011_a_missing_file_outside_the_scanned_test_files_is_only_the_scan_scope_problem() {
    for (file, cargo_tomls) in [
        ("tests/missing.rs", &[ROOT_CARGO_TOML][..]),
        ("crates/demo/tests/missing.txt", &[][..]),
        ("crates/demo/target/missing.rs", &[DEMO_CARGO_TOML][..]),
    ] {
        let citation = format!("{file}::lib_tool_011_x");
        let toml = citing(ROW_011, &[citation.as_str()]);
        let tree = matrix_tree("scanner-outside-missing", &toml, cargo_tomls);
        assert_on_disk(
            ROW_011,
            &tree,
            &toml,
            &[outside_scan(ROW_011, file)],
            &format!(
                "the missing {file} is outside the scanned test files: that is the one problem, \
                 not also \"does not exist\""
            ),
        );
    }

    let toml = citing(ROW_011, &["crates/demo/tests/missing.rs::lib_tool_011_x"]);
    let tree = matrix_tree("scanner-inside-missing", &toml, &[DEMO_CARGO_TOML]);
    assert_on_disk(
        ROW_011,
        &tree,
        &toml,
        &[format!(
            "{ROW_011}: test file crates/demo/tests/missing.rs does not exist"
        )],
        "a missing file inside the scanned test files is still reported as missing",
    );

    let toml = citing(ROW_011, &["tests/x.rs::lib_tool_001_x"]);
    let tree = matrix_tree("scanner-outside-misnamed", &toml, &[]);
    tree.write("tests/x.rs", "#[test]\nfn lib_tool_001_x() {}\n");
    assert_on_disk(
        ROW_011,
        &tree,
        &toml,
        &[
            format!(
                "{ROW_011}: test `lib_tool_001_x` must be named `lib_tool_011_...` to trace to \
                 this row"
            ),
            outside_scan(ROW_011, "tests/x.rs"),
        ],
        "the test name check still runs for a citation outside the scanned test files",
    );
}

// ---------------------------------------------------------------------------
// The real repository
// ---------------------------------------------------------------------------

fn read_real(rel: &str, row: &str) -> String {
    fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("{row}: read {rel} in the real repository: {e}"))
}

#[test]
fn lib_tool_007_the_real_repository_has_no_traced_test_under_src() {
    let repo = FsRepo::new(&repo_root());
    let mut found = repo.traced_functions();
    assert!(
        !found.is_empty(),
        "LIB-TOOL-007: precondition: the real repository has traced tests"
    );
    found.extend(repo.ignored_functions());
    let under: Vec<(String, String)> = found
        .into_iter()
        .filter(|(file, _)| file.split('/').any(|c| c == "src"))
        .collect();
    assert_eq!(
        under,
        Vec::<(String, String)>::new(),
        "LIB-TOOL-007: the real repository has no traced test under a `src` directory"
    );

    let matrix_rs = read_real("xtask/src/matrix.rs", ROW_007);
    assert!(
        matrix_rs.contains("#[test] fn lib_cli_001_"),
        "LIB-TOOL-007: precondition: the doc comments of xtask/src/matrix.rs show a traced test"
    );
    assert_eq!(
        (
            traced_tests(&matrix_rs, false),
            ignored_traced_tests(&matrix_rs, false)
        ),
        (Vec::new(), Vec::new()),
        "LIB-TOOL-007: the test-like text in the doc comments of xtask/src/matrix.rs is no test"
    );
}

#[test]
fn lib_tool_011_the_real_repository_has_no_ignored_or_skipped_traced_test() {
    assert_eq!(
        sorted(FsRepo::new(&repo_root()).ignored_functions()),
        Vec::<(String, String)>::new(),
        "LIB-TOOL-011: no traced test in the real repository is ignored or skipped"
    );
}

/// The two test files full of test-like fixtures are scanned exactly: every
/// line that is `#[test]` marks one traced test, and nothing in a fixture
/// counts.
#[test]
fn lib_tool_011_the_scanner_finds_exactly_the_tests_of_its_own_test_files() {
    for rel in ["xtask/tests/scanner.rs", "xtask/tests/function_scan.rs"] {
        let text = read_real(rel, ROW_011);
        let marked = text.lines().filter(|l| l.trim() == "#[test]").count();
        let running = traced_tests(&text, false);
        assert_eq!(
            running.len(),
            marked,
            "LIB-TOOL-011: {rel} has {marked} `#[test]` lines; the scanner found {running:?}"
        );
        assert!(
            running.iter().all(|n| n.starts_with("lib_tool_0")),
            "LIB-TOOL-011: every test the scanner finds in {rel} is one of its own, got \
             {running:?}"
        );
        assert_eq!(
            ignored_traced_tests(&text, false),
            Vec::<String>::new(),
            "LIB-TOOL-011: {rel} has no ignored test"
        );
    }
}
