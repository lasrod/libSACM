//! The W3C XML Conformance Test Suite 20130923 through `sacm::xmi::read`
//! (LIB-SEC-006).
//!
//! Every not-well-formed XML 1.0 and Namespaces 1.0 case that the reader can
//! meet must be refused: `Doctype`, `Malformed`, or `Unsupported` from the
//! encoding steps. The suite's documents are not SACM, so a result from the
//! root stage (`Ok`, `InvalidRoot`, `NoPackage` or any other `Unsupported`)
//! means the input got past the well-formedness checks: it counts as
//! accepted.
//!
//! The suite is never committed; no case file is copied into the repository
//! (fetch-only use, the maintainer's decision of 2026-11-11). It is found at
//! `$SACM_XMLCONF_DIR` when that is set, and otherwise at
//! `<workspace>/third_party/xmlconf/xmlconf`, where
//! `cargo xtask fetch-spec --only xmlconf` puts it. Without it, every test here
//! fails when the `CI` environment variable is set, and otherwise prints a
//! SKIPPED notice to stderr and returns.
//!
//! Not checked here:
//! - Item 1 (fetching): `xtask/tests/fetch.rs` and `xtask/tests/run.rs`.
//! - Items 8-10 (`xmlns:xmlns` and the tree-oracle sweep):
//!   `xml_wellformedness.rs`.
//! - Item 11's CI wiring and item 12's documentation: checked by the
//!   verifier.

// The run reports its case count on stdout (item 11).
#![allow(clippy::print_stdout)]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use sacm::xmi::{self, ReadError, ReadOptions, ReadOutput};

const ROW: &str = "LIB-SEC-006";

/// The catalog files, as the master `xmlconf.xml` declares them as external
/// entities, in its order (item 2).
const CATALOGS: [&str; 21] = [
    "sun/sun-valid.xml",
    "sun/sun-invalid.xml",
    "sun/sun-not-wf.xml",
    "sun/sun-error.xml",
    "xmltest/xmltest.xml",
    "japanese/japanese.xml",
    "oasis/oasis.xml",
    "ibm/ibm_oasis_invalid.xml",
    "ibm/ibm_oasis_not-wf.xml",
    "ibm/ibm_oasis_valid.xml",
    "ibm/xml-1.1/ibm_invalid.xml",
    "ibm/xml-1.1/ibm_not-wf.xml",
    "ibm/xml-1.1/ibm_valid.xml",
    "eduni/errata-2e/errata2e.xml",
    "eduni/xml-1.1/xml11.xml",
    "eduni/namespaces/1.0/rmt-ns10.xml",
    "eduni/namespaces/1.1/rmt-ns11.xml",
    "eduni/errata-3e/errata3e.xml",
    "eduni/namespaces/errata-1e/errata1e.xml",
    "eduni/errata-4e/errata4e.xml",
    "eduni/misc/ht-bh.xml",
];

/// The selected cases that the reader is known to accept, by ID. Empty: the
/// parser's leniencies within the suite are a closed list, and each is
/// refused (item 5). A case listed here would fail the run once refused, so
/// the list stays exact.
const KNOWN_ACCEPTED: &[&str] = &[];

/// How long one `read` may take. Each case is a few hundred bytes; a scan
/// that stops advancing never finishes. Well below cargo-mutants' minimum
/// test timeout of 20 seconds, as in `xml_wellformedness.rs`.
const DEADLINE: Duration = Duration::from_secs(5);

/// How long all the reads of one run may take together (item 7). A debug
/// run takes well under a second.
const RUN_DEADLINE: Duration = Duration::from_secs(15);

// --- Finding the suite ----------------------------------------------------

/// Where the suite is, or why the test does not run.
#[derive(Debug, PartialEq, Eq)]
enum Located {
    /// The directory that holds `xmlconf.xml`.
    Found(PathBuf),
    /// The suite is absent and the run is local: the message to print.
    Skip(String),
}

/// The fetch command that puts the suite in place, quoted in every message.
const FETCH: &str = "run `cargo xtask fetch-spec --only xmlconf`";

/// The suite directory: `configured` (the value of `SACM_XMLCONF_DIR`) if
/// set, else `default`. When it holds no `xmlconf.xml`: an error under CI
/// or when the directory was configured, a skip otherwise (item 11).
fn locate(configured: Option<OsString>, ci: bool, default: PathBuf) -> Result<Located, String> {
    let (dir, was_configured) = match configured {
        Some(value) if value.is_empty() => {
            return Err(format!(
                "{ROW}: SACM_XMLCONF_DIR is set but empty; unset it or {FETCH}"
            ));
        }
        Some(value) => (PathBuf::from(value), true),
        None => (default, false),
    };
    if dir.join("xmlconf.xml").is_file() {
        return Ok(Located::Found(dir));
    }
    let message = format!(
        "{ROW}: the W3C XML Conformance Test Suite is not at {}; {FETCH}",
        dir.display()
    );
    if ci || was_configured {
        Err(message)
    } else {
        Ok(Located::Skip(format!("SKIPPED {message}")))
    }
}

/// The suite directory for this run, or `None` after printing a SKIPPED
/// notice. Panics when the suite is absent under CI.
///
/// The notice goes straight to the process's stderr: the test harness
/// captures `eprintln!` and hides it for a passing test, so a skip would
/// look like a pass.
fn suite() -> Option<PathBuf> {
    // CARGO_MANIFEST_DIR is <workspace>/crates/sacm.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.parent().and_then(Path::parent).unwrap_or(manifest);
    let default = workspace
        .join("third_party")
        .join("xmlconf")
        .join("xmlconf");
    let ci = std::env::var_os("CI").is_some();
    match locate(std::env::var_os("SACM_XMLCONF_DIR"), ci, default) {
        Ok(Located::Found(dir)) => Some(dir),
        Ok(Located::Skip(notice)) => {
            // Nothing to do if stderr itself is gone.
            let _ = std::io::stderr().write_all(
                format!(
                    "{notice}
"
                )
                .as_bytes(),
            );
            None
        }
        Err(message) => panic!("{message}"),
    }
}

// --- Reading the catalogs -------------------------------------------------

/// One TEST element of a catalog.
#[derive(Debug, Clone)]
struct Case {
    /// The catalog that lists it, relative to the suite directory.
    catalog: &'static str,
    id: String,
    kind: String,
    recommendation: String,
    version: Option<String>,
    edition: Option<String>,
    namespace: String,
    sections: String,
    /// The case file, relative to the suite directory, with `/` separators.
    file: String,
}

impl Case {
    /// Whether VERSION is absent or lists 1.0.
    fn version_1_0(&self) -> bool {
        self.version
            .as_deref()
            .is_none_or(|v| v.split_whitespace().any(|t| t == "1.0"))
    }

    /// Whether EDITION is absent or lists 5.
    fn edition_5(&self) -> bool {
        self.edition
            .as_deref()
            .is_none_or(|e| e.split_whitespace().any(|t| t == "5"))
    }

    fn xml_1_0(&self) -> bool {
        matches!(
            self.recommendation.as_str(),
            "XML1.0" | "XML1.0-errata2e" | "XML1.0-errata3e" | "XML1.0-errata4e"
        )
    }

    fn ns_1_0(&self) -> bool {
        matches!(self.recommendation.as_str(), "NS1.0" | "NS1.0-errata1e")
    }

    /// The case's group, by item 3's selection.
    fn group(&self) -> Group {
        let applicable = self.version_1_0() && self.edition_5();
        match self.kind.as_str() {
            "not-wf" if self.xml_1_0() && applicable => Group::SelectedXml10,
            "not-wf" if self.ns_1_0() && applicable => Group::SelectedNs10,
            "not-wf" if self.xml_1_0() && self.version_1_0() => Group::EditionsOneToFourOnly,
            "not-wf" if matches!(self.recommendation.as_str(), "XML1.1" | "NS1.1") => {
                Group::Xml11OrNs11
            }
            "error" if self.ns_1_0() => Group::Ns10Error,
            "valid" | "invalid"
                if (self.xml_1_0() || self.ns_1_0()) && applicable && self.namespace == "yes" =>
            {
                Group::ValidOrInvalid10
            }
            _ => Group::Other,
        }
    }

    fn path(&self, suite: &Path) -> PathBuf {
        suite.join(&self.file)
    }

    /// The case for a failure message: ID, file, SECTIONS.
    fn describe(&self) -> String {
        format!("{} ({}, SECTIONS {:?})", self.id, self.file, self.sections)
    }
}

/// What item 3 does with a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    /// Selected: not-wf, XML 1.0 or an erratum, VERSION 1.0, EDITION 5.
    SelectedXml10,
    /// Selected: not-wf, Namespaces 1.0 or its erratum, VERSION 1.0, EDITION 5.
    SelectedNs10,
    /// Excluded: not-wf XML 1.0 cases for editions 1 to 4 only.
    EditionsOneToFourOnly,
    /// Excluded: not-wf XML 1.1 or Namespaces 1.1 cases.
    Xml11OrNs11,
    /// Excluded: Namespaces 1.0 `error` cases.
    Ns10Error,
    /// Not selected; run by the over-strictness guard (item 6).
    ValidOrInvalid10,
    /// Neither selected nor run.
    Other,
}

/// The external entity declarations of the master `xmlconf.xml`: the system
/// literal of each `<!ENTITY name SYSTEM "...">`, in order.
fn declared_catalogs(master: &str) -> Vec<String> {
    let mut catalogs = Vec::new();
    let mut rest = master;
    while let Some(start) = rest.find("<!ENTITY") {
        rest = &rest[start + "<!ENTITY".len()..];
        let Some(end) = rest.find('>') else {
            panic!("{ROW}: an entity declaration in xmlconf.xml does not end");
        };
        let declaration = &rest[..end];
        rest = &rest[end..];
        let mut words = declaration.split_whitespace();
        let (_name, keyword, literal) = (words.next(), words.next(), words.next());
        assert_eq!(
            keyword,
            Some("SYSTEM"),
            "{ROW}: the entity declaration {declaration:?} in xmlconf.xml is not external"
        );
        let Some(literal) = literal
            .and_then(|l| l.strip_prefix('"'))
            .and_then(|l| l.strip_suffix('"'))
        else {
            panic!("{ROW}: the entity declaration {declaration:?} has no quoted system literal");
        };
        catalogs.push(literal.to_owned());
    }
    catalogs
}

/// The TEST elements of the catalog `catalog`, a fragment: its XML
/// declaration is dropped and the rest is wrapped in a dummy root. URIs
/// resolve against the catalog file's own directory, never against the
/// master's `xml:base` (which is wrong for `ht-bh.xml`).
fn read_catalog(suite: &Path, catalog: &'static str) -> Vec<Case> {
    let path = suite.join(catalog);
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{ROW}: read the catalog {}: {e}", path.display()));
    let body = match raw.strip_prefix("<?xml") {
        Some(rest) => match rest.find("?>") {
            Some(end) => &rest[end + 2..],
            None => panic!("{ROW}: the XML declaration of {catalog} does not end"),
        },
        None => raw.as_str(),
    };
    let wrapped = format!("<CATALOG>{body}</CATALOG>");
    let document = roxmltree::Document::parse(&wrapped)
        .unwrap_or_else(|e| panic!("{ROW}: parse the catalog {catalog}: {e}"));
    let directory = catalog.rsplit_once('/').map_or("", |(dir, _)| dir);
    document
        .descendants()
        .filter(|node| node.has_tag_name("TEST"))
        .map(|test| {
            let attribute = |name: &str| test.attribute(name).map(str::to_owned);
            let required = |name: &str| {
                attribute(name).unwrap_or_else(|| {
                    panic!(
                        "{ROW}: a TEST in {catalog} has no {name}: {:?}",
                        test.range()
                    )
                })
            };
            let uri = required("URI");
            Case {
                catalog,
                id: required("ID"),
                kind: required("TYPE"),
                // The defaults of testcases.dtd.
                recommendation: attribute("RECOMMENDATION").unwrap_or_else(|| "XML1.0".into()),
                version: attribute("VERSION"),
                edition: attribute("EDITION"),
                namespace: attribute("NAMESPACE").unwrap_or_else(|| "yes".into()),
                sections: attribute("SECTIONS").unwrap_or_default(),
                file: format!("{directory}/{uri}"),
            }
        })
        .collect()
}

/// Every TEST of every catalog, in catalog order.
fn read_cases(suite: &Path) -> Vec<Case> {
    CATALOGS
        .into_iter()
        .flat_map(|catalog| read_catalog(suite, catalog))
        .collect()
}

// --- Running a case ---------------------------------------------------------

/// What `read` did with a case's bytes.
#[derive(Debug)]
enum Outcome {
    Returned(Result<ReadOutput, ReadError>),
    Panicked,
}

/// `xmi::read` on a worker thread, with panics caught. Fails the test, naming
/// the case, when the read does not return within [`DEADLINE`].
fn read_case(case: &Case, bytes: Vec<u8>) -> Outcome {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            xmi::read(&bytes, &ReadOptions::default())
        }));
        // The receiver is gone only after the deadline has already failed the test.
        let _ = sender.send(result);
    });
    match receiver.recv_timeout(DEADLINE) {
        Ok(Ok(result)) => Outcome::Returned(result),
        Ok(Err(_)) | Err(RecvTimeoutError::Disconnected) => Outcome::Panicked,
        Err(RecvTimeoutError::Timeout) => panic!(
            "{ROW}: read did not return within {DEADLINE:?} for {}: a scan does not advance",
            case.describe()
        ),
    }
}

/// How a case's result counts (item 4).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Class {
    Doctype,
    /// `Unsupported` for input the test itself sees in another encoding.
    Encoding,
    Malformed,
    /// Anything else, with the result: the case got past the checks.
    Accepted(String),
}

/// Whether `bytes` start with a UTF-16 byte order mark: `FF FE` or `FE FF`,
/// not followed by `00 00` (which makes a UCS-4 mark).
fn utf16_bom(bytes: &[u8]) -> bool {
    (bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]))
        && bytes.get(2..4) != Some(&[0x00, 0x00])
}

/// The encoding this test sees in `bytes` that the reader still refuses as
/// `Unsupported`, if any (item 4, after SACM23-XMI-018): a UCS-4 byte order
/// mark, first bytes that are `<` in UTF-16 or UCS-4 without a byte order
/// mark, or an XML declaration naming an encoding other than UTF-8 or ASCII
/// in input without a UTF-16 byte order mark. UTF-16 with a byte order mark
/// is read, so its `Unsupported` is never one from the encoding steps.
fn other_encoding(bytes: &[u8]) -> Option<String> {
    if utf16_bom(bytes) {
        return None;
    }
    if bytes.starts_with(&[0x00, 0x00, 0xFE, 0xFF])
        || bytes.starts_with(&[0xFF, 0xFE, 0x00, 0x00])
        || bytes.starts_with(&[0x00, 0x00, 0xFF, 0xFE])
        || bytes.starts_with(&[0xFE, 0xFF, 0x00, 0x00])
    {
        return Some("a UCS-4 byte order mark".to_owned());
    }
    let first = bytes.get(..4).unwrap_or(bytes);
    if first.contains(&0x00) && first.contains(&b'<') {
        return Some("`<` in UTF-16 or UCS-4 without a byte order mark".to_owned());
    }
    let text = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let declaration = text.strip_prefix(b"<?xml")?;
    let end = declaration
        .windows(2)
        .position(|w| w == b"?>")
        .unwrap_or(declaration.len());
    let declaration = String::from_utf8_lossy(&declaration[..end]);
    let after = &declaration[declaration.find("encoding")? + "encoding".len()..];
    let after = after.trim_start().strip_prefix('=')?.trim_start();
    let quote = after.chars().next().filter(|q| matches!(q, '"' | '\''))?;
    let name = &after[1..];
    let name = &name[..name.find(quote)?];
    let supported = ["UTF-8", "US-ASCII", "ASCII"]
        .iter()
        .any(|s| name.eq_ignore_ascii_case(s));
    (!supported).then(|| format!("the declared encoding {name:?}"))
}

fn classify(bytes: &[u8], outcome: &Outcome) -> Class {
    match outcome {
        Outcome::Returned(Err(ReadError::Doctype { .. })) => Class::Doctype,
        Outcome::Returned(Err(ReadError::Malformed { .. })) => Class::Malformed,
        Outcome::Returned(Err(error @ ReadError::Unsupported { .. })) => {
            if other_encoding(bytes).is_some() {
                Class::Encoding
            } else {
                Class::Accepted(format!("{error:?}"))
            }
        }
        Outcome::Returned(Err(error)) => Class::Accepted(format!("{error:?}")),
        Outcome::Returned(Ok(_)) => Class::Accepted("Ok: read as a document".to_owned()),
        Outcome::Panicked => Class::Accepted("read panicked".to_owned()),
    }
}

/// Runs every case of `cases` through `read`, in order, and returns each
/// case with its bytes and the outcome. Fails the test when a case file is
/// missing, when one read overruns [`DEADLINE`] (naming the case) or when
/// the reads together overrun [`RUN_DEADLINE`].
fn read_all<'a>(suite: &Path, cases: &[&'a Case]) -> Vec<(&'a Case, Vec<u8>, Outcome)> {
    let started = Instant::now();
    let results: Vec<(&Case, Vec<u8>, Outcome)> = cases
        .iter()
        .map(|case| {
            let path = case.path(suite);
            let bytes = fs::read(&path)
                .unwrap_or_else(|e| panic!("{ROW}: read the case {}: {e}", case.describe()));
            let outcome = read_case(case, bytes.clone());
            (*case, bytes, outcome)
        })
        .collect();
    let elapsed = started.elapsed();
    assert!(
        elapsed <= RUN_DEADLINE,
        "{ROW}: {} cases took {elapsed:?}, more than {RUN_DEADLINE:?}",
        cases.len()
    );
    results
}

/// [`read_all`], with each outcome classified.
fn run<'a>(suite: &Path, cases: &[&'a Case]) -> Vec<(&'a Case, Class)> {
    read_all(suite, cases)
        .into_iter()
        .map(|(case, bytes, outcome)| (case, classify(&bytes, &outcome)))
        .collect()
}

fn in_group<'a>(cases: &'a [Case], groups: &[Group]) -> Vec<&'a Case> {
    cases
        .iter()
        .filter(|c| groups.contains(&c.group()))
        .collect()
}

// --- Item 2. Catalogs ---------------------------------------------------------

#[test]
fn lib_sec_006_the_catalog_list_is_the_masters_external_entities() {
    let Some(suite) = suite() else { return };
    let master = suite.join("xmlconf.xml");
    let text = fs::read_to_string(&master)
        .unwrap_or_else(|e| panic!("{ROW}: read {}: {e}", master.display()));
    assert_eq!(
        declared_catalogs(&text),
        CATALOGS,
        "{ROW}: the test's catalog list must equal xmlconf.xml's external entity declarations"
    );
}

#[test]
fn lib_sec_006_every_test_uri_resolves_to_a_file_beside_its_catalog() {
    let Some(suite) = suite() else { return };
    let cases = read_cases(&suite);
    let missing: Vec<String> = cases
        .iter()
        .filter(|case| !case.path(&suite).is_file())
        .map(|case| format!("{} in {}", case.describe(), case.catalog))
        .collect();
    assert!(
        missing.is_empty(),
        "{ROW}: {} of {} TEST URIs do not resolve against their catalog's directory:\n{}",
        missing.len(),
        cases.len(),
        missing.join("\n")
    );
    // The master's xml:base for ht-bh.xml names eduni/namespaces/misc/,
    // which does not exist: its cases resolve to eduni/misc/.
    let ht_bh: Vec<&str> = cases
        .iter()
        .filter(|c| c.catalog == "eduni/misc/ht-bh.xml")
        .map(|c| c.file.as_str())
        .collect();
    assert!(
        !ht_bh.is_empty() && ht_bh.iter().all(|f| f.starts_with("eduni/misc/")),
        "{ROW}: the ht-bh.xml cases resolve under eduni/misc/, got {ht_bh:?}"
    );
}

// --- Item 3. Counts -------------------------------------------------------------

#[test]
fn lib_sec_006_the_selection_and_the_named_exclusions_have_their_pinned_counts() {
    let Some(suite) = suite() else { return };
    let cases = read_cases(&suite);
    assert_eq!(
        cases.len(),
        2_585,
        "{ROW}: TEST elements in the 21 catalogs"
    );
    let mut counts: BTreeMap<Group, usize> = BTreeMap::new();
    for case in &cases {
        *counts.entry(case.group()).or_default() += 1;
    }
    let count = |group| counts.get(&group).copied().unwrap_or(0);
    assert_eq!(
        (count(Group::SelectedXml10), count(Group::SelectedNs10)),
        (993, 24),
        "{ROW}: selected not-wf cases (XML 1.0, Namespaces 1.0), 1,017 in all"
    );
    assert_eq!(
        (
            count(Group::EditionsOneToFourOnly),
            count(Group::Xml11OrNs11),
            count(Group::Ns10Error)
        ),
        (309, 172, 3),
        "{ROW}: excluded and named: edition-1-4-only, XML 1.1 or NS 1.1, NS1.0 error"
    );
    // Every not-wf case is selected or excluded by name: none is dropped
    // without a reason.
    let unnamed: Vec<String> = cases
        .iter()
        .filter(|c| c.kind == "not-wf" && c.group() == Group::Other)
        .map(Case::describe)
        .collect();
    assert!(
        unnamed.is_empty(),
        "{ROW}: not-wf cases neither selected nor excluded by name: {unnamed:?}"
    );
    // IDs are unique, so the known-accepted list names cases exactly.
    let mut ids = BTreeSet::new();
    let duplicates: Vec<&str> = cases
        .iter()
        .filter(|c| !ids.insert(c.id.as_str()))
        .map(|c| c.id.as_str())
        .collect();
    assert!(
        duplicates.is_empty(),
        "{ROW}: duplicate TEST IDs: {duplicates:?}"
    );
}

// --- Items 4, 5, 7 and 11. Every selected case is refused -------------------------

#[test]
fn lib_sec_006_every_selected_not_well_formed_case_is_refused() {
    let Some(suite) = suite() else { return };
    let cases = read_cases(&suite);
    let selected = in_group(&cases, &[Group::SelectedXml10, Group::SelectedNs10]);
    let results = run(&suite, &selected);
    // Straight to the process's stderr, as in `suite()`: the harness captures
    // `println!` and hides it for a passing test.
    let _ = writeln!(std::io::stderr(), "{ROW}: ran {} cases", results.len());

    let accepted: Vec<String> = results
        .iter()
        .filter_map(|(case, class)| match class {
            Class::Accepted(result) => Some(format!("{}: {result}", case.describe())),
            _ => None,
        })
        .collect();
    let accepted_ids: BTreeSet<&str> = results
        .iter()
        .filter(|(_, class)| matches!(class, Class::Accepted(_)))
        .map(|(case, _)| case.id.as_str())
        .collect();
    let known: BTreeSet<&str> = KNOWN_ACCEPTED.iter().copied().collect();
    assert!(
        accepted_ids == known,
        "{ROW}: the accepted cases differ from the known list {KNOWN_ACCEPTED:?}; \
         {} accepted:\n{}",
        accepted.len(),
        accepted.join("\n")
    );

    let tally = |wanted: &Class| results.iter().filter(|(_, c)| c == wanted).count();
    assert_eq!(
        (
            tally(&Class::Doctype),
            tally(&Class::Encoding),
            tally(&Class::Malformed)
        ),
        (719, 2, 296),
        "{ROW}: the refusals by kind: Doctype, Unsupported from the encoding steps, Malformed \
         (after SACM23-XMI-018)"
    );
    assert_eq!(results.len(), 1_017, "{ROW}: every selected case ran");
}

#[test]
fn lib_sec_006_rmt_ns10_032_declaring_xmlns_is_malformed() {
    let Some(suite) = suite() else { return };
    let cases = read_cases(&suite);
    let Some(case) = cases.iter().find(|c| c.id == "rmt-ns10-032") else {
        panic!("{ROW}: rmt-ns10-032 is in rmt-ns10.xml");
    };
    let results = run(&suite, &[case]);
    assert_eq!(
        results.first().map(|(_, class)| class),
        Some(&Class::Malformed),
        "{ROW}: {} (`xmlns:xmlns` declared, Namespaces 1.0 §3) is Malformed",
        case.describe()
    );
}

// --- Item 6. Guard against over-strictness ---------------------------------------

#[test]
fn lib_sec_006_valid_and_invalid_cases_that_reach_the_root_stage_are_never_malformed() {
    let Some(suite) = suite() else { return };
    let cases = read_cases(&suite);
    let candidates = in_group(&cases, &[Group::ValidOrInvalid10]);
    // A read that times out has already failed the test in `read_case`,
    // naming the case. A panic is rejected here, before classification:
    // classified, it would count as reaching the root stage.
    let outcomes = read_all(&suite, &candidates);
    let panicked: Vec<String> = outcomes
        .iter()
        .filter(|(_, _, outcome)| matches!(outcome, Outcome::Panicked))
        .map(|(case, _, _)| case.describe())
        .collect();
    assert!(
        panicked.is_empty(),
        "{ROW}: read panicked on {} well-formed cases instead of returning:
{}",
        panicked.len(),
        panicked.join(
            "
"
        )
    );
    let results: Vec<(&Case, Class)> = outcomes
        .iter()
        .map(|(case, bytes, outcome)| (*case, classify(bytes, outcome)))
        .collect();
    // A case reaches the root stage unless the DOCTYPE or the encoding
    // steps refuse it first; a Malformed result stays in the count.
    let reaching: Vec<&(&Case, Class)> = results
        .iter()
        .filter(|(_, class)| !matches!(class, Class::Doctype | Class::Encoding))
        .collect();
    let malformed: Vec<String> = reaching
        .iter()
        .filter(|(_, class)| *class == Class::Malformed)
        .map(|(case, _)| case.describe())
        .collect();
    assert!(
        malformed.is_empty(),
        "{ROW}: {} well-formed cases are refused as Malformed (the scan is over-strict):\n{}",
        malformed.len(),
        malformed.join("\n")
    );
    assert_eq!(
        reaching.len(),
        73,
        "{ROW}: valid and invalid XML 1.0 and NS 1.0 cases with NAMESPACE yes that reach \
         the root stage (after SACM23-XMI-018)"
    );
}

// --- SACM23-XMI-018 item 10. The UTF-16 cases of the suite --------------------

/// The selected cases whose result SACM23-XMI-018 settles, with the class
/// each gets: the 31 `o-p02fail*` cases (an illegal character in UTF-16LE
/// with a byte order mark), `hst-lhs-008` (a UTF-16 byte order mark and
/// `utf-8` declared in UTF-16) and `hst-lhs-009` (the same mark before UTF-8
/// bytes, an odd count) are `Malformed`; `rmt-e2e-61` (UTF-8 declaring
/// `UTF-16`) and `hst-lhs-007` (a UTF-8 mark and `iso-8859-1`) stay
/// `Unsupported` from the encoding steps.
fn utf16_case_class(id: &str) -> Option<Class> {
    match id {
        "hst-lhs-008" | "hst-lhs-009" => Some(Class::Malformed),
        "rmt-e2e-61" | "hst-lhs-007" => Some(Class::Encoding),
        _ if id.starts_with("o-p02fail") => Some(Class::Malformed),
        _ => None,
    }
}

#[test]
fn sacm23_xmi_018_the_utf16_cases_of_the_suite_have_their_pinned_results() {
    const XMI_018: &str = "SACM23-XMI-018";
    let Some(suite) = suite() else { return };
    let cases = read_cases(&suite);
    let named: Vec<&Case> = cases
        .iter()
        .filter(|c| utf16_case_class(&c.id).is_some())
        .collect();
    assert_eq!(
        named.len(),
        35,
        "{XMI_018}: the 31 o-p02fail cases, hst-lhs-007, -008, -009 and rmt-e2e-61 are in the \
         catalogs"
    );
    assert!(
        named
            .iter()
            .all(|c| matches!(c.group(), Group::SelectedXml10 | Group::SelectedNs10)),
        "{XMI_018}: every named case is selected"
    );
    let wrong: Vec<String> = run(&suite, &named)
        .into_iter()
        .filter_map(|(case, class)| {
            let expected = utf16_case_class(&case.id)?;
            (class != expected)
                .then(|| format!("{}: expected {expected:?}, got {class:?}", case.describe()))
        })
        .collect();
    assert!(
        wrong.is_empty(),
        "{XMI_018}: {} named UTF-16 cases have another result:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

// --- Item 11. An absent suite ----------------------------------------------------

#[test]
fn lib_sec_006_an_absent_suite_fails_under_ci_and_is_skipped_locally() {
    let nowhere = std::env::temp_dir().join("lib-sec-006-no-such-suite");
    let fails = locate(None, true, nowhere.clone());
    assert!(
        fails
            .as_ref()
            .is_err_and(|m| m.contains("cargo xtask fetch-spec --only xmlconf")),
        "{ROW}: under CI an absent suite fails and names the fetch command, got {fails:?}"
    );
    let skips = locate(None, false, nowhere.clone());
    assert!(
        matches!(&skips, Ok(Located::Skip(notice)) if notice.starts_with("SKIPPED ")
            && notice.contains("cargo xtask fetch-spec --only xmlconf")),
        "{ROW}: locally an absent suite is skipped with a notice, got {skips:?}"
    );
    let configured = locate(Some(nowhere.clone().into_os_string()), false, nowhere);
    assert!(
        configured.is_err(),
        "{ROW}: a SACM_XMLCONF_DIR that holds no suite fails, CI or not, got {configured:?}"
    );
    let empty = locate(Some(OsString::new()), false, PathBuf::from("."));
    assert!(
        empty.is_err(),
        "{ROW}: an empty SACM_XMLCONF_DIR fails, got {empty:?}"
    );
}
