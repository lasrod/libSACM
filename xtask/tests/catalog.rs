//! Tests for `cargo xtask catalog` and `xtask::catalog::generate`
//! (LIB-DIAG-006; ADR 0011 amendment 5).
//!
//! `generate(root)` reads three files of the repository at `root`:
//!
//! - the `templates!` entries of `crates/sacm/src/diagnostic/template.rs`,
//!   in the syntax `message_templates.rs` checks, with `\"` and `\\` read as
//!   `"` and `\`;
//! - every `pub const NAME: Code = Code::from_static("TEXT");` of
//!   `crates/sacm/src/diagnostic.rs`, and each `serde(rename = "…")` inside
//!   its `pub enum Specification { … }`, in order;
//! - the Severity cell of each code's live row in `docs/sacm/diagnostics.md`
//!   (the row naming `` `Code::NAME` ``): the words Error, Warning and Info
//!   it holds, lowercase, ordered error, warning, info; `—` is none; a cell
//!   with none of them is the problem `[severity]`, and a constant with no
//!   live row `[no-row]`.
//!
//! It gives `serde_json::to_string_pretty` of `{format, specifications,
//! codes, templates}` and a line feed: `format` 1; codes sorted by their
//! text, each with its severities; templates sorted by id, each with its
//! code, carrier, arguments (name and kind) and English. `cargo xtask
//! catalog` writes it to `crates/sacm/src/diagnostic/catalog.json`.
//!
//! The synthetic source text is written as single-line string literals with
//! `\n` escapes, as in `message_templates.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use xtask::catalog::generate;
use xtask::{RootSource, run_from};

const ROW: &str = "LIB-DIAG-006";

/// The template table, relative to the repository root.
const TEMPLATE_RS: &str = "crates/sacm/src/diagnostic/template.rs";

/// The code constants and the specifications.
const DIAGNOSTIC_RS: &str = "crates/sacm/src/diagnostic.rs";

/// The diagnostics catalog, with the code rows.
const CATALOG_MD: &str = "docs/sacm/diagnostics.md";

/// The file `cargo xtask catalog` writes.
const OUTPUT: &str = "crates/sacm/src/diagnostic/catalog.json";

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| panic!("{ROW}: setup: no repository root"))
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel))
        .unwrap_or_else(|e| panic!("{ROW}: read {}: {e}", root.join(rel).display()))
}

/// A fresh directory under the system temporary directory, removed on drop.
struct TempRoot(PathBuf);

impl TempRoot {
    fn new(test: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "xtask-catalog-{test}-{}-{nanos}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&path)
            .unwrap_or_else(|e| panic!("{ROW}: create {}: {e}", path.display()));
        Self(path)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    /// Writes `text` at the `/`-separated path `rel`, creating its parents.
    fn write(&self, rel: &str, text: &str) -> &Self {
        let path = self.0.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("{ROW}: create {}: {e}", parent.display()));
        }
        fs::write(&path, text).unwrap_or_else(|e| panic!("{ROW}: write {}: {e}", path.display()));
        self
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `generate(root)`, which must succeed.
fn generated(what: &str, root: &Path) -> String {
    match generate(root) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: generate refused: {error}"),
    }
}

fn parse(what: &str, text: &str) -> Value {
    match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => panic!("{ROW}: {what} is not JSON: {error}"),
    }
}

/// The array `key` of `value`.
fn list<'v>(value: &'v Value, key: &str) -> &'v [Value] {
    match value.get(key).and_then(Value::as_array) {
        Some(items) => items,
        None => panic!("{ROW}: {key:?} is not an array"),
    }
}

/// The string `key` of each item of the array `array` of `value`.
fn strings<'v>(value: &'v Value, array: &str, key: &str) -> Vec<&'v str> {
    list(value, array)
        .iter()
        .map(|item| item.get(key).and_then(Value::as_str).unwrap_or_default())
        .collect()
}

/// The severities the catalog `value` lists for `code`.
fn severities_of<'v>(value: &'v Value, code: &str) -> Vec<&'v str> {
    let Some(entry) = list(value, "codes")
        .iter()
        .find(|entry| entry.get("code").and_then(Value::as_str) == Some(code))
    else {
        panic!("{ROW}: the catalog lists no {code}");
    };
    list(entry, "severities")
        .iter()
        .map(|severity| severity.as_str().unwrap_or_default())
        .collect()
}

/// `items` hold no repeat and are sorted by their bytes.
fn assert_unique_and_sorted(what: &str, items: &[&str]) {
    let mut sorted = items.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        items,
        sorted.as_slice(),
        "{ROW}: the {what} are not unique and sorted"
    );
}

// ---------------------------------------------------------------------------
// The real files
// ---------------------------------------------------------------------------

#[test]
fn lib_diag_006_the_checked_in_catalog_is_what_xtask_catalog_writes() {
    let root = repository_root();
    let written = generated("the repository", root);
    let checked_in = read(root, OUTPUT);
    assert!(
        written == checked_in,
        "{ROW}: {OUTPUT} is not what `cargo xtask catalog` writes: run it"
    );
    assert!(
        checked_in == sacm::diagnostic::CATALOG_JSON,
        "{ROW}: sacm::diagnostic::CATALOG_JSON is not {OUTPUT}"
    );
    let catalog = parse(OUTPUT, &checked_in);
    assert_eq!(
        catalog.get("format"),
        Some(&Value::from(1)),
        "{ROW}: format"
    );
    let codes = strings(&catalog, "codes", "code");
    assert_eq!(codes.len(), 46, "{ROW}: the number of codes");
    assert_unique_and_sorted("codes", &codes);
    let ids = strings(&catalog, "templates", "id");
    assert_eq!(ids.len(), 217, "{ROW}: the number of templates");
    assert_unique_and_sorted("template ids", &ids);
    let carriers = strings(&catalog, "templates", "carrier");
    let diagnostic = carriers.iter().filter(|c| **c == "Diagnostic").count();
    let write = carriers
        .iter()
        .filter(|c| c.starts_with("WriteError::"))
        .count();
    let read_errors = carriers
        .iter()
        .filter(|c| c.starts_with("ReadError::"))
        .count();
    assert_eq!(
        (diagnostic, write, read_errors),
        (113, 8, 96),
        "{ROW}: the templates of each carrier (Diagnostic, WriteError, ReadError)"
    );
    let template_codes: Vec<&str> = strings(&catalog, "templates", "code");
    for (id, code) in ids.iter().zip(&template_codes) {
        assert!(
            id.split_once('/').map(|(prefix, _)| prefix) == Some(*code) && codes.contains(code),
            "{ROW}: the template {id} has the code {code}, which is not its prefix or not listed"
        );
    }
}

#[test]
fn lib_diag_006_xtask_catalog_writes_the_file() {
    let real = repository_root();
    let tmp = TempRoot::new("writes");
    for rel in [TEMPLATE_RS, DIAGNOSTIC_RS, CATALOG_MD] {
        tmp.write(rel, &read(real, rel));
    }
    // A stale file is replaced.
    tmp.write(OUTPUT, "{}\n");
    let mut out = Vec::new();
    let result = run_from(&["catalog"], tmp.root(), RootSource::Build, &mut out);
    assert_eq!(result, Ok(()), "{ROW}: `cargo xtask catalog` failed");
    let out = String::from_utf8(out).unwrap_or_else(|e| panic!("{ROW}: non-UTF-8 output: {e}"));
    assert_eq!(
        out, "wrote crates/sacm/src/diagnostic/catalog.json: 46 codes, 217 templates\n",
        "{ROW}: what `cargo xtask catalog` prints"
    );
    let written = read(tmp.root(), OUTPUT);
    assert!(
        written == generated("the copy", tmp.root()),
        "{ROW}: the file written is not what generate gives"
    );
    assert!(
        written == generated("the repository", real),
        "{ROW}: the file written from a copy of the three inputs is not the repository's"
    );

    let mut help = Vec::new();
    assert_eq!(
        run_from(&["help"], tmp.root(), RootSource::Build, &mut help),
        Ok(()),
        "{ROW}: help"
    );
    let help = String::from_utf8_lossy(&help);
    assert!(
        help.lines().any(|line| line
            == "  catalog   write crates/sacm/src/diagnostic/catalog.json from the crate's codes and templates"),
        "{ROW}: the usage text does not list `catalog`: {help}"
    );
}

// ---------------------------------------------------------------------------
// Synthetic inputs
// ---------------------------------------------------------------------------

/// A template table of three templates, out of id order: a diagnostic's
/// with a text, a count and a prose argument and a `\"` in its pattern, an
/// `Unread` refusal's, and a `Write` refusal's with a `\\` in its pattern.
const TEMPLATES: &str = concat!(
    "//! The template table.\n",
    "templates! {\n",
    "    XMI_002_SACM_LIKE = XMI_NAMESPACE_NOT_PINNED, Diagnostic, \"SACM-XMI-002/sacm-like\",\n",
    "        [Text(\"id\"), Count(\"count\"), Prose(\"reason\")],\n",
    "        \"the root {id:?} is read {count:04X} times: \\\"{reason}\\\"\";\n",
    "    XMI_002_OTHER_NAMESPACE = XMI_NAMESPACE_NOT_PINNED, Unread, \"SACM-XMI-002/other-namespace\",\n",
    "        [Text(\"element\"), Text(\"row\")],\n",
    "        \"the root element {element:?} is not in the SACM 2.3 namespace\";\n",
    "    XMI_001_NO_ROOT_TO_WRITE = XMI_INVALID_ROOT, Write(\"NoRoot\"), \"SACM-XMI-001/no-root-to-write\",\n",
    "        [],\n",
    "        \"the document has no root to write \\\\ at all\";\n",
    "}\n",
);

/// Two code constants, out of code order; a constant that is not a code;
/// a `serde(rename)` outside `Specification`; and the two specifications,
/// XMI first.
const DIAGNOSTICS: &str = concat!(
    "//! Codes.\n",
    "impl Code {\n",
    "    /// Not pinned.\n",
    "    pub const XMI_NAMESPACE_NOT_PINNED: Code = Code::from_static(\"SACM-XMI-002\");\n",
    "    /// No root.\n",
    "    pub const XMI_INVALID_ROOT: Code = Code::from_static(\"SACM-XMI-001\");\n",
    "}\n",
    "/// The catalog.\n",
    "pub const CATALOG_JSON: &str = include_str!(\"diagnostic/catalog.json\");\n",
    "/// Not a specification.\n",
    "pub enum Other {\n",
    "    #[cfg_attr(feature = \"serde\", serde(rename = \"not a specification\"))]\n",
    "    A,\n",
    "}\n",
    "/// The specifications.\n",
    "pub enum Specification {\n",
    "    /// XMI.\n",
    "    #[cfg_attr(feature = \"serde\", serde(rename = \"XMI 2.5.1\"))]\n",
    "    Xmi251,\n",
    "    /// SACM.\n",
    "    #[cfg_attr(feature = \"serde\", serde(rename = \"SACM 2.3\"))]\n",
    "    Sacm23,\n",
    "}\n",
);

/// The code rows of [`DIAGNOSTICS`]' constants, the second with its
/// severities in the other order, and a reserved row.
const CATALOG: &str = concat!(
    "# Diagnostics catalog\n",
    "\n",
    "## XMI structure\n",
    "\n",
    "| Code | Severity | Meaning |\n",
    "| --- | --- | --- |\n",
    "| SACM-XMI-001 | Error | No root. `Code::XMI_INVALID_ROOT` (SACM23-XMI-001). |\n",
    "| SACM-XMI-002 | Warning (tolerant) / Error (strict) | Not pinned. `Code::XMI_NAMESPACE_NOT_PINNED` (LIB-COMPAT-004). |\n",
    "| SACM-XMI-009 | Info | Reserved: no read gives this code. |\n",
);

/// What `generate` gives for [`TEMPLATES`], [`DIAGNOSTICS`] and
/// [`CATALOG`].
const EXPECTED: &str = r#"{
  "format": 1,
  "specifications": [
    "XMI 2.5.1",
    "SACM 2.3"
  ],
  "codes": [
    {
      "code": "SACM-XMI-001",
      "severities": [
        "error"
      ]
    },
    {
      "code": "SACM-XMI-002",
      "severities": [
        "error",
        "warning"
      ]
    }
  ],
  "templates": [
    {
      "id": "SACM-XMI-001/no-root-to-write",
      "code": "SACM-XMI-001",
      "carrier": "WriteError::NoRoot",
      "arguments": [],
      "english": "the document has no root to write \\ at all"
    },
    {
      "id": "SACM-XMI-002/other-namespace",
      "code": "SACM-XMI-002",
      "carrier": "ReadError::Unsupported",
      "arguments": [
        {
          "name": "element",
          "kind": "text"
        },
        {
          "name": "row",
          "kind": "text"
        }
      ],
      "english": "the root element {element:?} is not in the SACM 2.3 namespace"
    },
    {
      "id": "SACM-XMI-002/sacm-like",
      "code": "SACM-XMI-002",
      "carrier": "Diagnostic",
      "arguments": [
        {
          "name": "id",
          "kind": "text"
        },
        {
          "name": "count",
          "kind": "count"
        },
        {
          "name": "reason",
          "kind": "prose"
        }
      ],
      "english": "the root {id:?} is read {count:04X} times: \"{reason}\""
    }
  ]
}
"#;

/// A temporary root holding the three inputs.
fn synthetic(test: &str, templates: &str, diagnostics: &str, catalog: &str) -> TempRoot {
    let tmp = TempRoot::new(test);
    tmp.write(TEMPLATE_RS, templates)
        .write(DIAGNOSTIC_RS, diagnostics)
        .write(CATALOG_MD, catalog);
    tmp
}

/// `base` with its first `from` replaced by `to`.
fn with(base: &str, from: &str, to: &str) -> String {
    let changed = base.replacen(from, to, 1);
    assert_ne!(changed, base, "{ROW}: setup: {from:?} is not in the input");
    changed
}

/// [`CATALOG`] with its first `from` replaced by `to`.
fn catalog_with(from: &str, to: &str) -> String {
    with(CATALOG, from, to)
}

/// `generate` of `tmp`, which must refuse with an error holding `expected`.
fn assert_refused(what: &str, tmp: &TempRoot, expected: &str) {
    match generate(tmp.root()) {
        Ok(text) => panic!("{ROW}: {what}: generate accepted it: {text}"),
        Err(error) => assert!(
            error.contains(expected),
            "{ROW}: {what}: the error does not hold {expected:?}: {error}"
        ),
    }
}

#[test]
fn lib_diag_006_generate_gives_the_documented_shape() {
    let tmp = synthetic("shape", TEMPLATES, DIAGNOSTICS, CATALOG);
    assert_eq!(
        generated("the synthetic inputs", tmp.root()),
        EXPECTED,
        "{ROW}: the catalog of the synthetic inputs"
    );
}

#[test]
fn lib_diag_006_severities_come_from_the_code_rows() {
    let catalog = parse(
        "the repository's catalog",
        &generated("the repository", repository_root()),
    );
    for (code, expected) in [
        ("SACM-XMI-002", &["error", "warning"][..]),
        ("SACM-XMI-004", &["info"][..]),
        ("SACM-XMI-007", &[][..]),
        ("SACM-CMD-001", &["error"][..]),
        ("SACM-ABS-001", &["error", "warning"][..]),
    ] {
        assert_eq!(
            severities_of(&catalog, code),
            expected,
            "{ROW}: the severities of {code}"
        );
    }

    // A cell of exactly `—` lists none.
    let none = synthetic(
        "dash",
        TEMPLATES,
        DIAGNOSTICS,
        &catalog_with("| SACM-XMI-001 | Error |", "| SACM-XMI-001 | — |"),
    );
    let generated_none = parse("the catalog with —", &generated("a — cell", none.root()));
    assert!(
        severities_of(&generated_none, "SACM-XMI-001").is_empty(),
        "{ROW}: a — cell lists a severity"
    );

    // A cell that names no severity is refused.
    let fatal = synthetic(
        "fatal",
        TEMPLATES,
        DIAGNOSTICS,
        &catalog_with("| SACM-XMI-001 | Error |", "| SACM-XMI-001 | Fatal |"),
    );
    match generate(fatal.root()) {
        Ok(text) => panic!("{ROW}: a Fatal cell was accepted: {text}"),
        Err(error) => assert!(
            error.contains("[severity] SACM-XMI-001: \"Fatal\" names no severity"),
            "{ROW}: a Fatal cell is not the [severity] problem: {error}"
        ),
    }

    // A constant whose row does not name it has no live row.
    let unnamed = synthetic(
        "no-row",
        TEMPLATES,
        DIAGNOSTICS,
        &catalog_with(" `Code::XMI_INVALID_ROOT`", ""),
    );
    match generate(unnamed.root()) {
        Ok(text) => panic!("{ROW}: a constant without a live row was accepted: {text}"),
        Err(error) => assert!(
            error.contains("[no-row] SACM-XMI-001") && !error.contains("[severity]"),
            "{ROW}: a constant without a live row is not the [no-row] problem: {error}"
        ),
    }
}

#[test]
fn lib_diag_006_a_row_naming_another_code_s_constant_is_not_its_live_row() {
    // The SACM-XMI-001 row names SACM-XMI-002's constant first, with
    // another severity: SACM-XMI-002 keeps the severities of its own row.
    let earlier = synthetic(
        "other-row",
        TEMPLATES,
        DIAGNOSTICS,
        &catalog_with(
            "No root. `Code::XMI_INVALID_ROOT`",
            "No root. Unlike `Code::XMI_NAMESPACE_NOT_PINNED`. `Code::XMI_INVALID_ROOT`",
        ),
    );
    assert_eq!(
        generated("another code's constant in an earlier row", earlier.root()),
        EXPECTED,
        "{ROW}: the live row is the one whose first cell is the code"
    );

    // SACM-XMI-001's constant is named only in SACM-XMI-002's row.
    let only_other = synthetic(
        "only-other-row",
        TEMPLATES,
        DIAGNOSTICS,
        &with(
            &catalog_with(" `Code::XMI_INVALID_ROOT`", ""),
            "Not pinned. ",
            "Not pinned. Unlike `Code::XMI_INVALID_ROOT`. ",
        ),
    );
    assert_refused(
        "a constant named only in another code's row",
        &only_other,
        "[no-row] SACM-XMI-001",
    );
}

#[test]
fn lib_diag_006_generate_names_each_input_it_cannot_read() {
    // (a) A template that names no Code constant.
    let unknown = synthetic(
        "unknown-code",
        &with(
            TEMPLATES,
            "XMI_002_OTHER_NAMESPACE = XMI_NAMESPACE_NOT_PINNED,",
            "XMI_002_OTHER_NAMESPACE = NOT_A_CODE,",
        ),
        DIAGNOSTICS,
        CATALOG,
    );
    assert_refused(
        "a template naming NOT_A_CODE",
        &unknown,
        "[unknown-code] SACM-XMI-002/other-namespace",
    );

    // (b) No `pub enum Specification`.
    let no_specification = synthetic(
        "no-specification",
        TEMPLATES,
        &with(
            DIAGNOSTICS,
            "pub enum Specification {",
            "pub enum Specifications {",
        ),
        CATALOG,
    );
    assert_refused(
        "a diagnostic.rs without `pub enum Specification`",
        &no_specification,
        "[no-specification]",
    );

    // (c) Template entries that do not parse.
    for (what, from, to) in [
        (
            "the 2nd of 3 entries without its `;`",
            "namespace\";",
            "namespace\"",
        ),
        (
            "the carrier Edit(\"X\")",
            "Write(\"NoRoot\")",
            "Edit(\"X\")",
        ),
        ("the argument kind Id(\"x\")", "Text(\"row\")", "Id(\"x\")"),
        (
            "the carrier Read(\"Not a name\")",
            "Write(\"NoRoot\")",
            "Read(\"Not a name\")",
        ),
        ("the carrier Read(\"\")", "Write(\"NoRoot\")", "Read(\"\")"),
        ("an unexpected `#`", "[],", "[#],"),
    ] {
        let malformed = synthetic(
            "malformed-entry",
            &with(TEMPLATES, from, to),
            DIAGNOSTICS,
            CATALOG,
        );
        assert_refused(what, &malformed, "[malformed-entry]");
    }
}

#[test]
fn lib_diag_006_generate_reads_only_the_entries_of_its_inputs() {
    for (what, templates, diagnostics) in [
        (
            "a templates! table at the very start of template.rs",
            with(TEMPLATES, "//! The template table.\n", ""),
            DIAGNOSTICS.to_owned(),
        ),
        (
            "a template name that starts with `_`",
            with(
                TEMPLATES,
                "XMI_001_NO_ROOT_TO_WRITE =",
                "_XMI_001_NO_ROOT_TO_WRITE =",
            ),
            DIAGNOSTICS.to_owned(),
        ),
        (
            "a serde(rename) after `pub enum Specification { … }`",
            TEMPLATES.to_owned(),
            format!(
                "{DIAGNOSTICS}#[cfg_attr(feature = \"serde\", serde(rename = \"after the enum\"))]\n\
                 pub struct After;\n"
            ),
        ),
        (
            "Code constants whose names are not names",
            TEMPLATES.to_owned(),
            with(
                DIAGNOSTICS,
                "impl Code {\n",
                "impl Code {\n    pub const NOT A NAME: Code = Code::from_static(\"SACM-XMI-098\");\n    \
                 pub const : Code = Code::from_static(\"SACM-XMI-099\");\n",
            ),
        ),
    ] {
        let tmp = synthetic("only-entries", &templates, &diagnostics, CATALOG);
        assert_eq!(
            generated(what, tmp.root()),
            EXPECTED,
            "{ROW}: {what}: the catalog is not the synthetic inputs' catalog"
        );
    }
}
