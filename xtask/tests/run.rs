//! Tests of the command dispatcher `xtask::run` (LIB-TOOL-001).
//!
//! `run` is what the `xtask` binary executes: every command, every argument
//! form and every error message lives there, and the binary only decodes its
//! arguments and maps the result to an exit status. These tests drive `run` on
//! temporary repository trees and capture what it writes, so each arm of the
//! dispatcher is pinned: the right command, on the given root, with the right
//! arguments, and with its output going to the given writer.
//!
//! Fixture source text is written on one line with `\n` escapes on purpose: a
//! line of this file that began with `#[test]` inside a string literal would
//! be picked up by the real repository's orphan scan.

mod common;

use std::fs;
use std::io::{self, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use common::{GATE_TOML, TempTree, consistent_tree};
use xtask::fetch::{ARTIFACTS, DEFAULT_DEST, sha256_hex};
use xtask::matrix::{MATRIX_PATH, RENDERED_PATH, load, render};
use xtask::run;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Run `args` on `root`, returning the result and everything written to `out`.
fn run_captured(args: &[&str], root: &Path) -> (Result<(), String>, String) {
    let mut out = Vec::new();
    let result = run(args, root, &mut out);
    let out = String::from_utf8(out)
        .unwrap_or_else(|e| panic!("LIB-TOOL-001: `{}` wrote non-UTF-8: {e}", args.join(" ")));
    (result, out)
}

/// Run `args` on `root`, expecting `Ok`; returns what was written.
fn run_ok(args: &[&str], root: &Path) -> String {
    let (result, out) = run_captured(args, root);
    assert_eq!(
        result,
        Ok(()),
        "LIB-TOOL-001: `{}` must succeed; wrote {out:?}",
        args.join(" ")
    );
    out
}

/// Run `args` on `root`, expecting `Err` with nothing written; returns the
/// message.
fn run_err(args: &[&str], root: &Path) -> String {
    let (result, out) = run_captured(args, root);
    let Err(message) = result else {
        panic!(
            "LIB-TOOL-001: `{}` must fail, got Ok; wrote {out:?}",
            args.join(" ")
        );
    };
    assert!(
        out.is_empty(),
        "LIB-TOOL-001: `{}` must write nothing when it fails, wrote {out:?}",
        args.join(" ")
    );
    message
}

/// The bytes of `rel` under `root`, or `None` when it cannot be read.
fn bytes(root: &Path, rel: &str) -> Option<Vec<u8>> {
    fs::read(root.join(rel)).ok()
}

/// An empty tree in which `docs/matrix/` exists, so that a command that wrote
/// the rendered page without first loading the matrix would succeed in doing
/// so and be caught.
fn empty_tree(test: &str) -> TempTree {
    let tree = TempTree::new(test);
    let dir = tree.root().join("docs/matrix");
    fs::create_dir_all(&dir)
        .unwrap_or_else(|e| panic!("LIB-TOOL-001: create {}: {e}", dir.display()));
    tree
}

/// The missing-matrix error names the matrix file.
fn assert_names_the_matrix(args: &[&str], message: &str) {
    assert!(
        message.contains("functionality-matrix.toml"),
        "LIB-TOOL-001: `{}` on an empty tree must name functionality-matrix.toml, got {message:?}",
        args.join(" ")
    );
}

// ---------------------------------------------------------------------------
// Item 1: `matrix check`
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_001_run_matrix_check_passes_a_consistent_tree_and_reports_the_row_count() {
    let tree = consistent_tree("run-check-ok");
    // A CRLF page is current for `check`, but `render` would rewrite it with
    // LF endings: the byte comparison below catches an arm that renders.
    let matrix = load(GATE_TOML).expect("LIB-TOOL-001: the gate fixture parses");
    tree.write(RENDERED_PATH, &render(&matrix).replace('\n', "\r\n"));
    let page_before = bytes(tree.root(), RENDERED_PATH);

    let out = run_ok(&["matrix", "check"], tree.root());
    assert!(
        out.contains("functionality matrix OK: 1 rows"),
        "LIB-TOOL-001: `matrix check` reports `functionality matrix OK: 1 rows`, wrote {out:?}"
    );
    assert_eq!(
        bytes(tree.root(), RENDERED_PATH),
        page_before,
        "LIB-TOOL-001: `matrix check` leaves the rendered page byte-identical"
    );
}

#[test]
fn lib_tool_001_run_matrix_check_fails_on_an_uncited_test_under_the_given_root() {
    let tree = consistent_tree("run-check-orphan");
    tree.write(
        "crates/demo/tests/planted.rs",
        "#[test]\nfn lib_tool_001_uncited() {}\n",
    );
    let page_before = bytes(tree.root(), RENDERED_PATH);

    let (result, out) = run_captured(&["matrix", "check"], tree.root());
    assert_eq!(
        result,
        Err("functionality matrix: 1 problem(s)".to_owned()),
        "LIB-TOOL-001: `matrix check` on the given root finds exactly the planted orphan"
    );
    assert!(
        out.is_empty(),
        "LIB-TOOL-001: a failing `matrix check` writes nothing to `out`, wrote {out:?}"
    );
    assert_eq!(
        bytes(tree.root(), RENDERED_PATH),
        page_before,
        "LIB-TOOL-001: a failing `matrix check` leaves the rendered page byte-identical"
    );
}

// ---------------------------------------------------------------------------
// Item 2: `matrix render`
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_001_run_matrix_render_rewrites_a_stale_page() {
    let tree = consistent_tree("run-render");
    tree.write(RENDERED_PATH, "stale");

    let out = run_ok(&["matrix", "render"], tree.root());
    let matrix = load(GATE_TOML).expect("LIB-TOOL-001: the gate fixture parses");
    assert_eq!(
        bytes(tree.root(), RENDERED_PATH),
        Some(render(&matrix).into_bytes()),
        "LIB-TOOL-001: `matrix render` writes `render(&load(toml))` byte for byte"
    );
    assert!(
        out.contains("wrote docs/matrix/functionality-matrix.md"),
        "LIB-TOOL-001: `matrix render` reports `wrote docs/matrix/functionality-matrix.md`, \
         wrote {out:?}"
    );
    run_ok(&["matrix", "check"], tree.root());
}

#[test]
fn lib_tool_001_run_matrix_render_without_a_matrix_fails_and_creates_no_page() {
    let tree = empty_tree("run-render-empty");
    let args = ["matrix", "render"];
    let message = run_err(&args, tree.root());
    assert_names_the_matrix(&args, &message);
    assert!(
        !tree.root().join(RENDERED_PATH).exists(),
        "LIB-TOOL-001: `matrix render` without a matrix creates no page"
    );
}

#[test]
fn lib_tool_001_run_matrix_render_reports_an_unwritable_page() {
    let tree = consistent_tree("run-render-dir");
    let page = tree.root().join(RENDERED_PATH);
    fs::remove_file(&page)
        .unwrap_or_else(|e| panic!("LIB-TOOL-001: remove {}: {e}", page.display()));
    fs::create_dir_all(&page)
        .unwrap_or_else(|e| panic!("LIB-TOOL-001: create {}: {e}", page.display()));

    let message = run_err(&["matrix", "render"], tree.root());
    assert!(
        message.starts_with("write "),
        "LIB-TOOL-001: a page that cannot be written is an error starting `write `, got {message:?}"
    );
}

// ---------------------------------------------------------------------------
// Item 3: `matrix next`
// ---------------------------------------------------------------------------

/// Two milestones and one `LIB` area.
const TWO_MILESTONES: &str = "schema_version = 1\n\
     [[milestone]]\nid = \"M0\"\ntitle = \"Foundation\"\ngoal = \"Build.\"\n\
     [[milestone]]\nid = \"M1\"\ntitle = \"Core\"\ngoal = \"Model.\"\n\
     [[area]]\ncode = \"TOOL\"\nprefix = \"LIB\"\ntitle = \"Tooling\"\n";

/// A `[[row]]` table titled `title`. `extra` holds further TOML keys.
fn row(id: &str, milestone: &str, status: &str, title: &str, extra: &str) -> String {
    format!(
        "[[row]]\nid = \"{id}\"\narea = \"TOOL\"\nmilestone = \"{milestone}\"\n\
         title = \"{title}\"\nrequirement = \"Requirement.\"\nsource = \"Source\"\n\
         kind = \"tooling\"\nstatus = \"{status}\"\n{extra}\n"
    )
}

const X: &str = "LIB-TOOL-006";
const V: &str = "LIB-TOOL-001";
const B: &str = "LIB-TOOL-002";
const C: &str = "LIB-TOOL-003";
const K: &str = "LIB-TOOL-004";
const D: &str = "LIB-TOOL-005";

/// A tree holding only the matrix, with rows in this file order:
///
/// - X: M1, open, ready;
/// - V: M0, verified;
/// - B: M0, open, depends on V: ready;
/// - C: M0, open, depends on B: blocked;
/// - K: M0, candidate;
/// - D: M0, open, ready.
///
/// So the ready rows are B, D, X: milestone order first, then file order.
/// There are four open rows (X, B, C, D). No rendered page exists.
fn next_tree(test: &str) -> TempTree {
    let rows = [
        row(X, "M1", "planned", "Row X", ""),
        row(V, "M0", "verified", "Row V", ""),
        row(
            B,
            "M0",
            "analyzed",
            "Row B",
            &format!("depends_on = [\"{V}\"]"),
        ),
        row(
            C,
            "M0",
            "planned",
            "Row C",
            &format!("depends_on = [\"{B}\"]"),
        ),
        row(K, "M0", "candidate", "Row K", "notes = \"why\""),
        row(D, "M0", "tests-failing", "Row D", ""),
    ]
    .concat();
    let tree = TempTree::new(test);
    tree.write(MATRIX_PATH, &format!("{TWO_MILESTONES}{rows}"));
    tree
}

/// `out` is `header` followed by one line per id in `ids`, each starting
/// with the row id and naming the row's title.
fn assert_next_output(out: &str, header: &str, ids: &[&str]) {
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some(header),
        "LIB-TOOL-001: `matrix next` header, wrote {out:?}"
    );
    let listed: Vec<&str> = lines[1..]
        .iter()
        .map(|l| l.split_whitespace().next().unwrap_or_default())
        .collect();
    assert_eq!(
        listed, ids,
        "LIB-TOOL-001: `matrix next` lists exactly these rows in this order, wrote {out:?}"
    );
    let titles = [(X, "Row X"), (B, "Row B"), (D, "Row D")];
    for (line, id) in lines[1..].iter().zip(ids) {
        let title = titles.iter().find(|(i, _)| i == id).map_or("", |(_, t)| *t);
        assert!(
            line.contains(title),
            "LIB-TOOL-001: the line for {id} names its title {title:?}, got {line:?}"
        );
    }
}

#[test]
fn lib_tool_001_run_matrix_next_lists_ready_rows_in_milestone_then_file_order() {
    let tree = next_tree("run-next");
    let out = run_ok(&["matrix", "next"], tree.root());
    assert_next_output(&out, "3 ready of 4 open rows; next 3:", &[B, D, X]);
}

#[test]
fn lib_tool_001_run_matrix_next_honours_the_row_count_options() {
    let tree = next_tree("run-next-limit");
    let out = run_ok(&["matrix", "next", "-n", "1"], tree.root());
    assert_next_output(&out, "3 ready of 4 open rows; next 1:", &[B]);

    let out = run_ok(&["matrix", "next", "--limit", "2"], tree.root());
    assert_next_output(&out, "3 ready of 4 open rows; next 2:", &[B, D]);
}

#[test]
fn lib_tool_001_run_matrix_next_reports_when_no_row_is_ready() {
    let rows = [
        row(K, "M0", "candidate", "Row K", "notes = \"why\""),
        row(
            B,
            "M0",
            "planned",
            "Row B",
            &format!("depends_on = [\"{K}\"]"),
        ),
    ]
    .concat();
    let tree = TempTree::new("run-next-none");
    tree.write(MATRIX_PATH, &format!("{TWO_MILESTONES}{rows}"));

    let out = run_ok(&["matrix", "next"], tree.root());
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        ["no ready rows (1 open rows, all blocked on unverified dependencies)"],
        "LIB-TOOL-001: `matrix next` with every open row blocked says so"
    );
}

#[test]
fn lib_tool_001_run_matrix_next_rejects_a_missing_matrix_and_bad_row_counts() {
    let empty = empty_tree("run-next-empty");
    let args = ["matrix", "next"];
    let message = run_err(&args, empty.root());
    assert_names_the_matrix(&args, &message);

    let tree = next_tree("run-next-bad");
    assert_eq!(
        run_err(&["matrix", "next", "-n", "notanumber"], tree.root()),
        "invalid row count: notanumber",
        "LIB-TOOL-001: a row count that is not a number is rejected by name"
    );
    let message = run_err(&["matrix", "next", "-n"], tree.root());
    assert!(
        message.starts_with("unrecognized arguments: matrix next -n"),
        "LIB-TOOL-001: `-n` without a count is unrecognized, got {message:?}"
    );
}

// ---------------------------------------------------------------------------
// Item 4: `matrix status`
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_001_run_matrix_status_prints_one_line_per_milestone_and_writes_no_page() {
    let tree = next_tree("run-status");
    let out = run_ok(&["matrix", "status"], tree.root());
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "LIB-TOOL-001: `matrix status` prints one line per milestone, wrote {out:?}"
    );
    for (line, (id, title)) in lines.iter().zip([("M0", "Foundation"), ("M1", "Core")]) {
        assert!(
            line.starts_with(&format!("{id} ")) && line.contains(title),
            "LIB-TOOL-001: the status line for {id} starts with its id and names {title:?}, \
             got {line:?}"
        );
    }
    assert!(
        !tree.root().join(RENDERED_PATH).exists(),
        "LIB-TOOL-001: `matrix status` writes no rendered page"
    );
}

#[test]
fn lib_tool_001_run_matrix_status_rejects_a_missing_matrix() {
    let tree = empty_tree("run-status-empty");
    let args = ["matrix", "status"];
    let message = run_err(&args, tree.root());
    assert_names_the_matrix(&args, &message);
}

// ---------------------------------------------------------------------------
// Item 5: `fetch-spec` (no network)
// ---------------------------------------------------------------------------

const G1: &str = "junk G1: not the pinned artifact";
const G2: &str = "junk G2: not the pinned artifact either";

/// A tree with junk at the first artifact's path in the default destination
/// (G1) and in `custom/` (G2).
///
/// No test here reaches the network: `fetch_into` checks an existing file's
/// hash before it would download anything, so the first artifact fails on
/// its wrong hash and `curl` is never called.
fn fetch_tree(test: &str) -> TempTree {
    let name = ARTIFACTS[0].file_name;
    let tree = TempTree::new(test);
    tree.write(&format!("{DEFAULT_DEST}/{name}"), G1)
        .write(&format!("custom/{name}"), G2);
    tree
}

fn assert_junk_untouched(tree: &TempTree) {
    let name = ARTIFACTS[0].file_name;
    for (rel, junk) in [
        (format!("{DEFAULT_DEST}/{name}"), G1),
        (format!("custom/{name}"), G2),
    ] {
        assert_eq!(
            bytes(tree.root(), &rel),
            Some(junk.as_bytes().to_vec()),
            "LIB-TOOL-001: `fetch-spec` leaves the planted {rel} byte-identical"
        );
    }
}

#[test]
fn lib_tool_001_run_fetch_spec_uses_the_default_destination_under_the_root() {
    let tree = fetch_tree("run-fetch-default");
    let message = run_err(&["fetch-spec"], tree.root());
    assert!(
        message.contains("exists but its SHA-256 is")
            && message.contains(&sha256_hex(G1.as_bytes())),
        "LIB-TOOL-001: `fetch-spec` checks <root>/{DEFAULT_DEST} and reports the hash of the \
         file there, got {message:?}"
    );
    assert_junk_untouched(&tree);
}

#[test]
fn lib_tool_001_run_fetch_spec_dest_selects_a_directory_under_the_root() {
    let tree = fetch_tree("run-fetch-dest");
    let message = run_err(&["fetch-spec", "--dest", "custom"], tree.root());
    assert!(
        message.contains(&sha256_hex(G2.as_bytes())),
        "LIB-TOOL-001: `fetch-spec --dest custom` checks <root>/custom, got {message:?}"
    );
    assert!(
        !message.contains(&sha256_hex(G1.as_bytes())),
        "LIB-TOOL-001: `fetch-spec --dest custom` does not look at the default destination, \
         got {message:?}"
    );
    assert_junk_untouched(&tree);
}

// ---------------------------------------------------------------------------
// Item 6: help, unrecognized and near-miss arguments
// ---------------------------------------------------------------------------

/// A consistent tree, so that a near miss wrongly dispatched to `check`
/// would succeed; with junk at the default fetch destination, so that one
/// wrongly dispatched to `fetch-spec` fails offline instead of downloading.
fn near_miss_tree(test: &str) -> TempTree {
    let tree = consistent_tree(test);
    tree.write(&format!("{DEFAULT_DEST}/{}", ARTIFACTS[0].file_name), G1);
    tree
}

#[test]
fn lib_tool_001_run_help_writes_the_usage() {
    let tree = TempTree::new("run-help");
    let out = run_ok(&["help"], tree.root());
    assert!(
        out.starts_with("usage: cargo xtask <command>"),
        "LIB-TOOL-001: `help` writes the usage to `out`, wrote {out:?}"
    );
}

#[test]
fn lib_tool_001_run_rejects_unrecognized_arguments_with_the_usage() {
    let tree = near_miss_tree("run-unrecognized");
    let usage = run_ok(&["help"], tree.root());
    let message = run_err(&["frobnicate", "--now"], tree.root());
    assert!(
        message.starts_with("unrecognized arguments: frobnicate --now"),
        "LIB-TOOL-001: unrecognized arguments are named, got {message:?}"
    );
    assert!(
        message.contains("usage: cargo xtask <command>") && message.contains(usage.trim_end()),
        "LIB-TOOL-001: the error carries the same usage text that `help` writes, got {message:?}"
    );
}

#[test]
fn lib_tool_001_run_rejects_near_miss_arguments() {
    let tree = near_miss_tree("run-near-miss");
    for args in [
        &["matrix"][..],
        &["matrix", "check", "--extra"],
        &["fetch-spec", "--dest"],
    ] {
        let message = run_err(args, tree.root());
        let expected = format!("unrecognized arguments: {}", args.join(" "));
        assert!(
            message.starts_with(&expected),
            "LIB-TOOL-001: `{}` must be `{expected}`, got {message:?}",
            args.join(" ")
        );
    }
}

// ---------------------------------------------------------------------------
// Item 7: write failures become errors
// ---------------------------------------------------------------------------

/// A writer whose every `write` fails (with a kind `write_all` does not
/// retry).
#[derive(Debug)]
struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("LIB-TOOL-001: this writer always fails"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn lib_tool_001_run_turns_write_failures_into_errors() {
    let consistent = consistent_tree("run-write-fail-check");
    let rendering = consistent_tree("run-write-fail-render");
    let two = next_tree("run-write-fail-next");
    let cases: [(&[&str], &Path); 5] = [
        (&["matrix", "check"], consistent.root()),
        (&["matrix", "render"], rendering.root()),
        (&["matrix", "next"], two.root()),
        (&["matrix", "status"], two.root()),
        (&["help"], two.root()),
    ];
    for (args, root) in cases {
        let outcome = catch_unwind(AssertUnwindSafe(|| run(args, root, &mut FailingWriter)));
        assert!(
            outcome.is_ok(),
            "LIB-TOOL-001: `{}` panicked on a failing writer instead of returning Err",
            args.join(" ")
        );
        assert!(
            outcome.is_ok_and(|result| result.is_err()),
            "LIB-TOOL-001: `{}` returned Ok although every write failed",
            args.join(" ")
        );
    }
}

// ---------------------------------------------------------------------------
// Item 8: `out` is flushed, and a flush failure is an error
// ---------------------------------------------------------------------------

/// A writer that accepts every `write` and keeps the bytes, but whose every
/// `flush` fails.
#[derive(Debug, Default)]
struct FlushFailingWriter {
    written: Vec<u8>,
}

impl Write for FlushFailingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("LIB-TOOL-001: this writer never flushes"))
    }
}

/// Run `args` on `root` into a [`FlushFailingWriter`], failing the test on a
/// panic; returns the result and what was written.
fn run_flush_failing(args: &[&str], root: &Path) -> (Result<(), String>, String) {
    let mut out = FlushFailingWriter::default();
    let outcome = catch_unwind(AssertUnwindSafe(|| run(args, root, &mut out)));
    let Ok(result) = outcome else {
        panic!(
            "LIB-TOOL-001: `{}` panicked on a writer whose flush fails instead of returning",
            args.join(" ")
        );
    };
    (result, String::from_utf8_lossy(&out.written).into_owned())
}

#[test]
fn lib_tool_001_run_turns_a_flush_failure_after_a_successful_command_into_an_error() {
    let consistent = consistent_tree("run-flush-fail-check");
    let rendering = consistent_tree("run-flush-fail-render");
    let two = next_tree("run-flush-fail-next");
    let cases: [(&[&str], &Path); 5] = [
        (&["help"], two.root()),
        (&["matrix", "check"], consistent.root()),
        (&["matrix", "render"], rendering.root()),
        (&["matrix", "next"], two.root()),
        (&["matrix", "status"], two.root()),
    ];
    for (args, root) in cases {
        let (result, written) = run_flush_failing(args, root);
        assert!(
            !written.is_empty(),
            "LIB-TOOL-001: `{}` runs and writes its output before the flush fails",
            args.join(" ")
        );
        let Err(message) = result else {
            panic!(
                "LIB-TOOL-001: `{}` returned Ok although flushing `out` failed; wrote {written:?}",
                args.join(" ")
            );
        };
        assert!(
            message.starts_with("write output: "),
            "LIB-TOOL-001: `{}` reports the failed flush as `write output: ...`, got {message:?}",
            args.join(" ")
        );
        assert!(
            message.contains("this writer never flushes"),
            "LIB-TOOL-001: `{}` carries the flush's own error, got {message:?}",
            args.join(" ")
        );
    }
}

#[test]
fn lib_tool_001_run_keeps_the_command_error_when_flushing_also_fails() {
    let orphan = consistent_tree("run-flush-fail-orphan");
    orphan.write(
        "crates/demo/tests/planted.rs",
        "#[test]\nfn lib_tool_001_uncited() {}\n",
    );
    let two = next_tree("run-flush-fail-bad-count");

    let (result, _) = run_flush_failing(&["matrix", "check"], orphan.root());
    assert_eq!(
        result,
        Err("functionality matrix: 1 problem(s)".to_owned()),
        "LIB-TOOL-001: a failing `matrix check` returns its own error, not the flush error"
    );

    let (result, _) = run_flush_failing(&["matrix", "next", "-n", "notanumber"], two.root());
    assert_eq!(
        result,
        Err("invalid row count: notanumber".to_owned()),
        "LIB-TOOL-001: a bad row count is reported as such, not as the flush error"
    );

    let (result, _) = run_flush_failing(&["frobnicate"], two.root());
    assert!(
        result
            .as_ref()
            .is_err_and(|m| m.starts_with("unrecognized arguments: frobnicate")),
        "LIB-TOOL-001: unrecognized arguments are reported as such, not as the flush error, \
         got {result:?}"
    );
}

/// A writer that succeeds and records the order of writes and flushes.
#[derive(Debug, Default)]
struct FlushRecordingWriter {
    flushes: usize,
    written_since_flush: bool,
}

impl Write for FlushRecordingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !buf.is_empty() {
            self.written_since_flush = true;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        self.written_since_flush = false;
        Ok(())
    }
}

#[test]
fn lib_tool_001_run_flushes_out_after_the_command_writes() {
    let consistent = consistent_tree("run-flush-count-check");
    let two = next_tree("run-flush-count-next");
    let cases: [(&[&str], &Path); 4] = [
        (&["help"], two.root()),
        (&["matrix", "check"], consistent.root()),
        (&["matrix", "next"], two.root()),
        (&["matrix", "status"], two.root()),
    ];
    for (args, root) in cases {
        let mut out = FlushRecordingWriter::default();
        let result = run(args, root, &mut out);
        assert_eq!(
            result,
            Ok(()),
            "LIB-TOOL-001: `{}` succeeds on a writer that flushes",
            args.join(" ")
        );
        assert!(
            out.flushes >= 1,
            "LIB-TOOL-001: `{}` flushes `out` before returning",
            args.join(" ")
        );
        assert!(
            !out.written_since_flush,
            "LIB-TOOL-001: `{}` flushes `out` after its last write, not before",
            args.join(" ")
        );
    }
}
