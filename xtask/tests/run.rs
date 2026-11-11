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
use xtask::fetch::{ARTIFACTS, DEFAULT_DEST, XMLCONF, sha256_hex};
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
    // The lines after the first list the problems (LIB-TOOL-010).
    assert_eq!(
        result.as_ref().map_err(|e| e.lines().next()),
        Err(Some("functionality matrix: 1 problem(s)")),
        "LIB-TOOL-001: `matrix check` on the given root finds exactly the planted orphan, \
         got {result:?}"
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

/// Three milestones; M2 has no rows.
const THREE_MILESTONES: &str = "schema_version = 1\n\
     [[milestone]]\nid = \"M0\"\ntitle = \"Foundation\"\ngoal = \"Build.\"\n\
     [[milestone]]\nid = \"M1\"\ntitle = \"Core\"\ngoal = \"Model.\"\n\
     [[milestone]]\nid = \"M2\"\ntitle = \"Later\"\ngoal = \"Extend.\"\n\
     [[area]]\ncode = \"TOOL\"\nprefix = \"LIB\"\ntitle = \"Tooling\"\n";

#[test]
fn lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status() {
    // File order interleaves the milestones, so a count that ignored the
    // milestone would differ. M0 has more unverified than verified rows and
    // more scheduled than unscheduled ones, so neither count can be its
    // complement by accident; M1 likewise.
    let rows = [
        row("LIB-TOOL-001", "M0", "verified", "R1", ""),
        row("LIB-TOOL-007", "M1", "tests-failing", "R7", ""),
        row("LIB-TOOL-002", "M0", "verified", "R2", ""),
        row("LIB-TOOL-003", "M0", "implemented", "R3", ""),
        row("LIB-TOOL-008", "M1", "analyzed", "R8", ""),
        row("LIB-TOOL-004", "M0", "planned", "R4", ""),
        row("LIB-TOOL-005", "M0", "candidate", "R5", "notes = \"why\""),
        row("LIB-TOOL-009", "M1", "deferred", "R9", "notes = \"why\""),
        row(
            "LIB-TOOL-006",
            "M0",
            "out-of-scope",
            "R6",
            "notes = \"why\"",
        ),
    ]
    .concat();
    let tree = TempTree::new("run-status-counts");
    tree.write(MATRIX_PATH, &format!("{THREE_MILESTONES}{rows}"));
    let out = run_ok(&["matrix", "status"], tree.root());
    // `<id:<4> <title:<44> <verified:>3>/<scheduled:<3> verified  (<counts>)`:
    // scheduled rows are those not candidate, deferred or out-of-scope; the
    // counts follow the summary's status order and omit statuses with none.
    let expected = [
        format!(
            "M0   {:<44}   2/4   verified  (2 verified, 1 implemented, 1 planned, \
             1 candidate, 1 out-of-scope)",
            "Foundation"
        ),
        format!(
            "M1   {:<44}   0/2   verified  (1 tests-failing, 1 analyzed, 1 deferred)",
            "Core"
        ),
        format!("M2   {:<44}   0/0   verified  ()", "Later"),
    ];
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        expected,
        "LIB-TOOL-001: `matrix status` prints, per milestone, verified/scheduled and the \
         non-zero counts per status"
    );
    assert!(
        out.ends_with('\n'),
        "LIB-TOOL-001: every status line is terminated, wrote {out:?}"
    );
}

// ---------------------------------------------------------------------------
// The repository root the binary passes to `run`
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_001_repo_root_is_the_workspace_root_above_the_xtask_crate() {
    let root = xtask::repo_root();
    assert!(
        root.is_absolute(),
        "LIB-TOOL-001: repo_root is absolute, independent of the working directory, got {}",
        root.display()
    );
    assert_eq!(
        Some(root.as_path()),
        Path::new(env!("CARGO_MANIFEST_DIR")).parent(),
        "LIB-TOOL-001: repo_root is the parent of the xtask crate directory"
    );
    assert!(
        root.join("xtask/Cargo.toml").is_file(),
        "LIB-TOOL-001: repo_root holds xtask/Cargo.toml, got {}",
        root.display()
    );
    let manifest = fs::read_to_string(root.join("Cargo.toml"))
        .unwrap_or_else(|e| panic!("LIB-TOOL-001: read the root Cargo.toml: {e}"));
    assert!(
        manifest.lines().any(|l| l.trim() == "[workspace]"),
        "LIB-TOOL-001: the Cargo.toml at repo_root is the workspace manifest"
    );
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
    // The lines after the first list the problems (LIB-TOOL-010).
    assert_eq!(
        result.as_ref().map_err(|e| e.lines().next()),
        Err(Some("functionality matrix: 1 problem(s)")),
        "LIB-TOOL-001: a failing `matrix check` returns its own error, not the flush error, \
         got {result:?}"
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

// ---------------------------------------------------------------------------
// LIB-TOOL-020 item 3: the `fetch-spec` help names XMI 2.5.1
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_020_run_help_says_fetch_spec_fetches_xmi_2_5_1() {
    let tree = TempTree::new("run-help-xmi");
    let out = run_ok(&["help"], tree.root());
    // The `fetch-spec` entry: its line and the indented continuation lines
    // under it, up to the next command, blank line or section.
    let mut lines = out
        .lines()
        .skip_while(|l| !l.trim_start().starts_with("fetch-spec"));
    let Some(first) = lines.next() else {
        panic!("LIB-TOOL-020: `help` lists `fetch-spec`, wrote {out:?}");
    };
    let entry: Vec<&str> = std::iter::once(first)
        .chain(lines.take_while(|l| l.starts_with("                          ")))
        .collect();
    let entry = entry.join("\n");
    assert!(
        entry.contains("XMI 2.5.1"),
        "LIB-TOOL-020: the `fetch-spec` help names XMI 2.5.1 among the fetched sources, got \
         {entry:?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-SEC-006 item 1: `fetch-spec --only xmlconf` (no network)
// ---------------------------------------------------------------------------

const G3: &str = "junk G3: not the pinned suite archive";

/// Where `fetch-spec` puts the W3C suite archive, relative to the root.
const SUITE_ARCHIVE: &str = "third_party/xmlconf/xmlts20130923.tar.gz";

/// A tree with junk at the first OMG artifact's path in the default
/// destination (G1) and at the suite archive's path (G3). Every `fetch-spec`
/// form fails offline on the first one it checks, since a file with the
/// wrong hash is refused before anything is downloaded.
fn suite_tree(test: &str) -> TempTree {
    let tree = TempTree::new(test);
    tree.write(&format!("{DEFAULT_DEST}/{}", ARTIFACTS[0].file_name), G1)
        .write(SUITE_ARCHIVE, G3);
    tree
}

/// Names of the entries of `rel` under `root`, sorted.
fn entries(root: &Path, rel: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(root.join(rel))
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn assert_suite_tree_untouched(tree: &TempTree) {
    let omg = format!("{DEFAULT_DEST}/{}", ARTIFACTS[0].file_name);
    for (rel, junk) in [(omg.as_str(), G1), (SUITE_ARCHIVE, G3)] {
        assert_eq!(
            bytes(tree.root(), rel),
            Some(junk.as_bytes().to_vec()),
            "LIB-SEC-006: `fetch-spec` leaves the planted {rel} byte-identical"
        );
    }
    assert_eq!(
        entries(tree.root(), DEFAULT_DEST),
        [ARTIFACTS[0].file_name],
        "LIB-SEC-006: nothing is added to {DEFAULT_DEST}"
    );
    assert_eq!(
        entries(tree.root(), "third_party/xmlconf"),
        [XMLCONF.file_name],
        "LIB-SEC-006: nothing is added to third_party/xmlconf"
    );
}

#[test]
fn lib_sec_006_run_fetch_spec_only_xmlconf_checks_only_the_suite_under_the_root() {
    let tree = suite_tree("run-fetch-only-xmlconf");
    let message = run_err(&["fetch-spec", "--only", "xmlconf"], tree.root());
    assert!(
        message.contains("xmlts20130923.tar.gz") && message.contains(&sha256_hex(G3.as_bytes())),
        "LIB-SEC-006: `fetch-spec --only xmlconf` checks <root>/{SUITE_ARCHIVE} and reports \
         the hash of the file there, got {message:?}"
    );
    assert!(
        !message.contains(&sha256_hex(G1.as_bytes()))
            && ARTIFACTS.iter().all(|a| !message.contains(a.file_name)),
        "LIB-SEC-006: `fetch-spec --only xmlconf` does not look at the OMG sources, got \
         {message:?}"
    );
    assert_suite_tree_untouched(&tree);
}

#[test]
fn lib_sec_006_run_fetch_spec_checks_the_omg_sources_before_the_suite() {
    let tree = suite_tree("run-fetch-omg-first");
    for args in [&["fetch-spec"][..], &["fetch-spec", "--dest", DEFAULT_DEST]] {
        let message = run_err(args, tree.root());
        assert!(
            message.contains(&sha256_hex(G1.as_bytes()))
                && !message.contains(&sha256_hex(G3.as_bytes())),
            "LIB-SEC-006: `{}` fails on the OMG sources before it reaches the suite, got \
             {message:?}",
            args.join(" ")
        );
    }
    assert_suite_tree_untouched(&tree);
}

#[test]
fn lib_sec_006_run_help_lists_fetch_spec_only_xmlconf() {
    let tree = TempTree::new("run-help-only-xmlconf");
    let out = run_ok(&["help"], tree.root());
    assert!(
        out.lines()
            .any(|l| l.trim_start().starts_with("fetch-spec --only xmlconf")),
        "LIB-SEC-006: `help` lists `fetch-spec --only xmlconf` as a command form, wrote {out:?}"
    );
}

#[test]
fn lib_sec_006_run_rejects_other_only_forms() {
    let tree = suite_tree("run-only-near-miss");
    for args in [
        &["fetch-spec", "--only"][..],
        &["fetch-spec", "--only", "foo"],
        &["fetch-spec", "--only", "XMLCONF"],
        &["fetch-spec", "--only", "xmlconf", "--extra"],
        &["fetch-spec", "--only", "xmlconf", "--dest", "custom"],
        &["fetch-spec", "--dest", "custom", "--only", "xmlconf"],
    ] {
        let message = run_err(args, tree.root());
        let expected = format!("unrecognized arguments: {}", args.join(" "));
        assert!(
            message.starts_with(&expected),
            "LIB-SEC-006: `{}` must be `{expected}`, got {message:?}",
            args.join(" ")
        );
    }
    assert_suite_tree_untouched(&tree);
}
