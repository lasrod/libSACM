//! Tests for the strict save's refusal of preserved content (SACM23-XMI-008;
//! ADR 0033).
//!
//! A strict `xmi::write` of a document holding preserved content
//! (LIB-COMPAT-003), however it was read, fails with
//! `WriteError::PreservedContent { elements }`: SACM-XMI-006, no element, no
//! diagnostics, each holder once in `Document::elements()` order, and no
//! element when only the document holds content outside its elements. The
//! write checks run in the order `NoRoot`, `PreservedContent`, `Invalid`
//! (the schema check: SACM-ID-002 and SACM-REF-001, never `validate`),
//! unrepresentable characters (ADR 0035 D4). A tolerant write writes the
//! content.
//!
//! What preserved content is, and its exact tolerant output, are
//! LIB-COMPAT-003's, in `preserved_content.rs`.

use sacm::diagnostic::{Code, Severity};
use sacm::edit::{self, DeleteElement, PackageDeletePolicy, SetName};
use sacm::model::{Document, LangString};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteAttribute, WriteError, WriteOptions};

const ROW: &str = "SACM23-XMI-008";
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The ADR 0033 worked fixture, whose holders are p, c1, c2 and c3.
const WORKED: &[u8] =
    include_bytes!("data/sacm23/xmi/invalid/argument-package-unknown-content.sacm.xmi");

/// The golden files the reader reads, which hold no preserved content.
const GOLDEN: [&str; 6] = [
    "artifact-relationships.golden.sacm.xmi",
    "asserted-relationships.golden.sacm.xmi",
    "named.golden.sacm.xmi",
    "needs-support-claim.golden.sacm.xmi",
    "several-roots.golden.sacm.xmi",
    "single-root.golden.sacm.xmi",
];
const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23/xmi/golden/");

/// A valid package `p` holding the Claims `c1` and `c2`, both named; each
/// `@SLOT@` is filled by [`fill`]. With the defaults it holds no preserved
/// content and validates without an Error.
const TEMPLATE: &str = r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:v="urn:v" xmi:id="p"@ROOT_ATTR@>
  <name content="P"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c1"@C1_ATTR@>
    <name content="C1"/>@C1_CHILD@
    <note xmi:id="n1"@NOTE_ATTR@/>
  </argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c2"@C2_ATTR@>
    <name content="C2"/>@C2_CHILD@
  </argumentElement>
</sacm:ArgumentPackage>
"#;
const SLOTS: [&str; 6] = [
    "ROOT_ATTR",
    "C1_ATTR",
    "C1_CHILD",
    "NOTE_ATTR",
    "C2_ATTR",
    "C2_CHILD",
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// [`TEMPLATE`] with each slot set from `slots`, else empty.
fn fill(slots: &[(&str, &str)]) -> String {
    let mut text = TEMPLATE.to_owned();
    for slot in SLOTS {
        let value = slots
            .iter()
            .find(|(name, _)| *name == slot)
            .map_or("", |(_, value)| *value);
        text = text.replace(&format!("@{slot}@"), value);
    }
    for (slot, _) in slots {
        assert!(
            SLOTS.contains(slot),
            "{ROW}: setup: the template has no slot {slot}"
        );
    }
    text
}

/// Reads `input` in `mode`, which the test expects to be read.
fn read_in(what: &str, mode: Mode, input: &[u8]) -> Document {
    match xmi::read(input, &ReadOptions::default().with_mode(mode)) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: {what}: a {mode:?} read refused the input: {error}"),
    }
}

fn strict() -> WriteOptions {
    WriteOptions::default().with_mode(Mode::Strict)
}

fn tolerant() -> WriteOptions {
    WriteOptions::default().with_mode(Mode::Tolerant)
}

/// A strict write of `doc`, which the test expects to be refused.
fn strict_err(what: &str, doc: &Document) -> WriteError {
    match xmi::write(doc, &strict()) {
        Ok(text) => panic!("{ROW}: {what}: a strict write wrote the document:\n{text}"),
        Err(error) => error,
    }
}

/// A write of `doc` with `options`, which the test expects to succeed.
fn write_ok(what: &str, doc: &Document, options: &WriteOptions) -> String {
    match xmi::write(doc, options) {
        Ok(text) => text,
        Err(error) => panic!(
            "{ROW}: {what}: a {:?} write refused the document: {error}",
            options.mode()
        ),
    }
}

/// The holders `error` lists, if it is `PreservedContent`.
fn holders(error: &WriteError) -> Option<Vec<String>> {
    match error {
        WriteError::PreservedContent { elements, .. } => Some(
            elements
                .iter()
                .map(|element| element.as_str().to_owned())
                .collect(),
        ),
        _ => None,
    }
}

/// Asserts that `error` is `PreservedContent` listing `expected`, with
/// SACM-XMI-006, no element and no diagnostics.
fn assert_refused_for(what: &str, error: &WriteError, expected: &[&str]) {
    assert_eq!(
        holders(error),
        Some(expected.iter().map(|&id| id.to_owned()).collect()),
        "{ROW}: {what}: the strict write is not PreservedContent listing {expected:?}: {error:?}"
    );
    assert_eq!(
        error.code(),
        Some(Code::XMI_STRICT_SAVE_REFUSED),
        "{ROW}: {what}: the refusal's code is not SACM-XMI-006"
    );
    assert_eq!(
        error.element(),
        None,
        "{ROW}: {what}: PreservedContent names one element"
    );
    assert_eq!(
        error.diagnostics(),
        [],
        "{ROW}: {what}: PreservedContent carries diagnostics"
    );
}

/// Deletes `element` from `doc`, with everything it holds.
fn delete(doc: &mut Document, element: &str) {
    let operation =
        DeleteElement::new(element).with_package_policy(PackageDeletePolicy::DeleteRecursively);
    if let Err(error) = edit::apply(doc, operation) {
        panic!("{ROW}: setup: deleting {element}: {error}");
    }
}

// ---------------------------------------------------------------------------
// The refusal
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_008_the_strict_save_refused_code_is_sacm_xmi_006() {
    assert_eq!(
        Code::XMI_STRICT_SAVE_REFUSED.as_str(),
        "SACM-XMI-006",
        "{ROW}: Code::XMI_STRICT_SAVE_REFUSED is not SACM-XMI-006"
    );
}

#[test]
fn sacm23_xmi_008_a_strict_write_of_the_worked_fixture_lists_every_holder_in_document_order() {
    for mode in MODES {
        let doc = read_in("the worked fixture", mode, WORKED);
        let error = strict_err("the worked fixture", &doc);
        assert_refused_for(
            &format!("the worked fixture read in {mode:?} mode"),
            &error,
            &["p", "c1", "c2", "c3"],
        );
        let order: Vec<String> = doc
            .elements()
            .filter(|element| element.has_preserved_content())
            .map(|element| element.id().as_str().to_owned())
            .collect();
        assert_eq!(
            holders(&error),
            Some(order),
            "{ROW}: the holders are not listed in Document::elements() order"
        );
    }
}

/// The writer cannot show that content it did not read is schema-valid, so
/// preserved content is refused before the schema check (ADR 0035 D4).
#[test]
fn sacm23_xmi_008_preserved_content_is_refused_before_the_schema_check() {
    let doc = read_in("the worked fixture", Mode::Tolerant, WORKED);
    let error = strict_err("the worked fixture", &doc);
    assert_refused_for("the worked fixture", &error, &["p", "c1", "c2", "c3"]);
    // A holder and a schema error: a reference naming no xmi:id
    // (SACM-REF-001), and an xmi:id that is no NCName (SACM-ID-002).
    let input = fill(&[
        ("C1_ATTR", " v:w=\"1\""),
        ("C2_ATTR", " isCitation=\"true\" citedElement=\"zz\""),
        ("NOTE_ATTR", " gid=\"g\""),
    ])
    .replace("xmi:id=\"n1\"", "xmi:id=\"1n\"");
    let doc = read_in(
        "a holder and two schema errors",
        Mode::Tolerant,
        input.as_bytes(),
    );
    let schema: Vec<Code> = validation::validate(&doc)
        .iter()
        .filter(|found| found.severity() == Severity::Error)
        .map(sacm::diagnostic::Diagnostic::code)
        .filter(|&code| code == Code::REF_DANGLING || code == Code::ID_INVALID)
        .collect();
    assert_eq!(
        schema,
        [Code::ID_INVALID, Code::REF_DANGLING],
        "{ROW}: setup: the document does not have the two schema errors"
    );
    let error = strict_err("a holder and two schema errors", &doc);
    assert!(
        !matches!(error, WriteError::Invalid { .. }),
        "{ROW}: the strict write reports a schema error before the preserved content: {error:?}"
    );
    assert_refused_for("a holder and two schema errors", &error, &["c1"]);
}

/// Content that is outside every element (on or under `xmi:XMI`) is
/// refused with no element listed (ADR 0035 D3, D4).
#[test]
fn sacm23_xmi_008_only_document_level_content_is_refused_with_no_elements() {
    let root = "<sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/></sacm:ArgumentPackage>";
    let wrapper = |attributes: &str, body: &str| {
        format!(
            "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
             xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" xmlns:v=\"urn:v\"{attributes}>\
             {body}</xmi:XMI>"
        )
    };
    for (what, input) in [
        ("a vendor attribute on xmi:XMI", wrapper(" v:w=\"1\"", root)),
        (
            "an xmi:Documentation beside the root",
            wrapper("", &format!("<xmi:Documentation/>{root}")),
        ),
        (
            "a TerminologyPackage root beside the root",
            wrapper(
                "",
                &format!(
                    "{root}<sacm:TerminologyPackage xmi:id=\"t\"><name content=\"T\"/>\
                     </sacm:TerminologyPackage>"
                ),
            ),
        ),
    ] {
        let doc = read_in(what, Mode::Tolerant, input.as_bytes());
        assert!(
            doc.elements()
                .all(|element| !element.has_preserved_content()),
            "{ROW}: {what}: setup: an element holds preserved content"
        );
        let error = strict_err(what, &doc);
        assert!(
            matches!(&error, WriteError::PreservedContent { elements, .. } if elements.is_empty()),
            "{ROW}: {what}: the strict write is not PreservedContent with no element: {error:?}"
        );
        assert_eq!(
            (error.code(), error.element(), error.diagnostics()),
            (Some(Code::XMI_STRICT_SAVE_REFUSED), None, &[][..]),
            "{ROW}: {what}: not SACM-XMI-006 with no element and no diagnostics"
        );
        assert_eq!(
            error.to_string(),
            "the document cannot be written in strict mode: the document holds preserved \
             content outside its elements",
            "{ROW}: {what}: the Display"
        );
        write_ok(what, &doc, &tolerant());
    }
    // With an element that holds some too, the elements are listed.
    let input = wrapper(
        " v:w=\"1\"",
        "<sacm:ArgumentPackage xmi:id=\"p\" v:x=\"2\"><name content=\"P\"/></sacm:ArgumentPackage>",
    );
    let doc = read_in("the wrapper and p", Mode::Tolerant, input.as_bytes());
    assert_refused_for(
        "the wrapper and p",
        &strict_err("the wrapper and p", &doc),
        &["p"],
    );
}

#[test]
fn sacm23_xmi_008_a_holder_of_only_an_attribute_only_text_or_only_a_child_and_a_root_are_each_refused()
 {
    let plain = fill(&[]);
    for mode in MODES {
        let doc = read_in("the plain document", mode, plain.as_bytes());
        if let Err(error) = xmi::write(&doc, &strict()) {
            panic!("{ROW}: setup: the plain document does not write strictly: {error}");
        }
    }
    for (what, slots, holder, kept) in [
        (
            "only an attribute",
            &[("C1_ATTR", " v:w=\"1\"")][..],
            "c1",
            "v:w=\"1\"",
        ),
        (
            "only text",
            &[("C1_CHILD", "\n    Stray text")][..],
            "c1",
            "Stray text",
        ),
        (
            "only a child",
            &[("C1_CHILD", "\n    <v:x/>")][..],
            "c1",
            "<v:x",
        ),
        (
            "a root",
            &[("ROOT_ATTR", " v:w=\"root\"")][..],
            "p",
            "v:w=\"root\"",
        ),
        (
            "a UtilityElement",
            &[("NOTE_ATTR", " v:w=\"note\"")][..],
            "n1",
            "v:w=\"note\"",
        ),
    ] {
        let input = fill(slots);
        for mode in MODES {
            let doc = read_in(what, mode, input.as_bytes());
            let error = strict_err(what, &doc);
            assert_refused_for(
                &format!("a holder of {what}, read {mode:?}"),
                &error,
                &[holder],
            );
            let text = write_ok(what, &doc, &tolerant());
            assert!(
                text.contains(kept),
                "{ROW}: a tolerant write of a holder of {what} omits it:\n{text}"
            );
        }
    }
}

#[test]
fn sacm23_xmi_008_each_holder_is_listed_once_however_much_it_holds() {
    let input = fill(&[
        ("C1_ATTR", " v:w=\"1\" isCounter=\"true\""),
        ("C1_CHILD", "\n    Stray text\n    <v:x/>\n    <v:y/>"),
        ("C2_CHILD", "\n    <v:z/>"),
    ]);
    let doc = read_in("two holders", Mode::Tolerant, input.as_bytes());
    let error = strict_err("two holders", &doc);
    assert_refused_for(
        "two holders, one holding five constructs",
        &error,
        &["c1", "c2"],
    );
}

#[test]
fn sacm23_xmi_008_display_names_one_holder_or_counts_several_and_names_the_first() {
    let one = fill(&[("C1_ATTR", " v:w=\"1\"")]);
    let doc = read_in("one holder", Mode::Tolerant, one.as_bytes());
    assert_eq!(
        strict_err("one holder", &doc).to_string(),
        "the document cannot be written in strict mode: the element \"c1\" holds preserved \
         content",
        "{ROW}: the refusal of one holder does not display the pinned text"
    );
    let two = fill(&[("C2_ATTR", " v:w=\"2\""), ("C1_CHILD", "\n    <v:x/>")]);
    let doc = read_in("two holders", Mode::Tolerant, two.as_bytes());
    assert_eq!(
        strict_err("two holders", &doc).to_string(),
        "the document cannot be written in strict mode: 2 elements hold preserved content, \
         the first is \"c1\"",
        "{ROW}: the refusal of two holders does not display the pinned text"
    );
    let doc = read_in("the worked fixture", Mode::Tolerant, WORKED);
    assert_eq!(
        strict_err("the worked fixture", &doc).to_string(),
        "the document cannot be written in strict mode: 4 elements hold preserved content, \
         the first is \"p\"",
        "{ROW}: the refusal of the worked fixture does not display the pinned text"
    );
}

// ---------------------------------------------------------------------------
// The check order
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_008_no_root_is_reported_before_preserved_content() {
    for options in [strict(), tolerant()] {
        assert_eq!(
            xmi::write(&Document::new(), &options),
            Err(WriteError::NoRoot),
            "{ROW}: an empty document is not NoRoot in {:?} mode",
            options.mode()
        );
    }
    // Deleting the root holder removes its preserved content with it.
    let mut doc = read_in("the worked fixture", Mode::Tolerant, WORKED);
    delete(&mut doc, "p");
    for options in [strict(), tolerant()] {
        assert_eq!(
            xmi::write(&doc, &options),
            Err(WriteError::NoRoot),
            "{ROW}: the worked fixture without its root is not NoRoot in {:?} mode",
            options.mode()
        );
    }
}

#[test]
fn sacm23_xmi_008_a_document_whose_only_finding_is_a_preserved_target_is_still_refused() {
    let input = fill(&[
        ("C1_ATTR", " isCitation=\"true\" citedElement=\"a1\""),
        ("C2_CHILD", "\n    <v:x xmi:id=\"a1\"/>"),
    ]);
    let doc = read_in("a preserved target", Mode::Tolerant, input.as_bytes());
    let found: Vec<(Code, Severity)> = validation::validate(&doc)
        .iter()
        .map(|diagnostic| (diagnostic.code(), diagnostic.severity()))
        .collect();
    assert_eq!(
        found,
        [(Code::REF_PRESERVED_TARGET, Severity::Warning)],
        "{ROW}: setup: the document's only finding is not SACM-REF-003"
    );
    let error = strict_err("a preserved target", &doc);
    assert_refused_for(
        "a document whose only finding is SACM-REF-003",
        &error,
        &["c2"],
    );
    write_ok("a preserved target", &doc, &tolerant());
}

#[test]
fn sacm23_xmi_008_preserved_content_is_refused_before_an_unrepresentable_character() {
    let input = fill(&[("C2_ATTR", " v:w=\"2\"")]);
    let mut doc = read_in("a holder", Mode::Tolerant, input.as_bytes());
    if let Err(error) = edit::apply(&mut doc, SetName::new("c1", LangString::new("bad\u{1}"))) {
        panic!("{ROW}: setup: SetName: {error}");
    }
    let error = strict_err("a holder and a control character", &doc);
    assert_refused_for("a holder and a control character", &error, &["c2"]);
    // Tolerant mode has no PreservedContent check: the character is refused.
    let error = match xmi::write(&doc, &tolerant()) {
        Ok(text) => panic!("{ROW}: a tolerant write wrote U+0001:\n{text}"),
        Err(error) => error,
    };
    assert!(
        matches!(
            &error,
            WriteError::UnrepresentableCharacter {
                element,
                character: '\u{1}',
                attribute: WriteAttribute::NameContent,
                ..
            } if element.as_str() == "c1"
        ),
        "{ROW}: a tolerant write of the same document is not UnrepresentableCharacter on c1: \
         {error:?}"
    );
}

#[test]
fn sacm23_xmi_008_without_preserved_content_the_schema_check_comes_before_an_unrepresentable_character()
 {
    // c2 cites an id no element has: SACM-REF-001, a schema error.
    let input = fill(&[("C2_ATTR", " isCitation=\"true\" citedElement=\"zz\"")]);
    let mut doc = read_in("a dangling reference", Mode::Tolerant, input.as_bytes());
    if let Err(error) = edit::apply(&mut doc, SetName::new("c1", LangString::new("bad\u{1}"))) {
        panic!("{ROW}: setup: SetName: {error}");
    }
    let error = strict_err("a dangling reference and a control character", &doc);
    assert!(
        matches!(&error, WriteError::Invalid { diagnostics, .. }
            if diagnostics.iter().map(sacm::diagnostic::Diagnostic::code).eq([Code::REF_DANGLING])),
        "{ROW}: with a schema error and a control character, a strict write is not Invalid \
         with the one SACM-REF-001: {error:?}"
    );
    let error = match xmi::write(&doc, &tolerant()) {
        Ok(text) => panic!("{ROW}: a tolerant write wrote U+0001:\n{text}"),
        Err(error) => error,
    };
    assert!(
        matches!(error, WriteError::UnrepresentableCharacter { .. }),
        "{ROW}: a tolerant write of the same document is not UnrepresentableCharacter: {error:?}"
    );
}

// ---------------------------------------------------------------------------
// What is written
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_008_a_tolerant_write_of_the_same_documents_succeeds_and_keeps_the_content() {
    for mode in MODES {
        let doc = read_in("the worked fixture", mode, WORKED);
        let text = write_ok("the worked fixture", &doc, &tolerant());
        let back = read_in(
            "the worked fixture, written",
            Mode::Tolerant,
            text.as_bytes(),
        );
        assert!(
            back == doc,
            "{ROW}: the tolerant write of the worked fixture read {mode:?} does not keep its \
             preserved content:\n{text}"
        );
        assert_refused_for(
            "the worked fixture, written and read back",
            &strict_err("the worked fixture, written", &back),
            &["p", "c1", "c2", "c3"],
        );
    }
}

#[test]
fn sacm23_xmi_008_strict_output_without_preserved_content_is_unchanged() {
    let mut written = 0_usize;
    let mut drafts = 0_usize;
    for name in GOLDEN {
        let path = format!("{GOLDEN_DIR}{name}");
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{ROW}: setup: cannot read {path}: {error}"));
        for mode in MODES {
            let doc = read_in(name, mode, expected.as_bytes());
            assert!(
                doc.elements()
                    .all(|element| !element.has_preserved_content()),
                "{ROW}: the golden file {name} reads with preserved content"
            );
            // Some golden files are drafts (unnamed packages): a strict write
            // checks schema validity only, so it writes them too (ADR 0035).
            if validation::validate(&doc)
                .iter()
                .any(|diagnostic| diagnostic.severity() == Severity::Error)
            {
                drafts += 1;
            }
            assert_eq!(
                write_ok(name, &doc, &strict()),
                expected,
                "{ROW}: the strict write of the golden file {name}, read {mode:?}, is not the \
                 golden file"
            );
            written += 1;
        }
    }
    assert!(
        written == GOLDEN.len() * MODES.len() && drafts > 0,
        "{ROW}: setup: not every golden file was written ({written}), or none is a draft"
    );
}

#[test]
fn sacm23_xmi_008_after_every_holder_is_deleted_a_strict_write_succeeds() {
    let with = fill(&[
        ("C1_CHILD", "\n    <v:x xmi:id=\"kept\"><v:y/></v:x>"),
        ("C2_ATTR", " v:w=\"2\""),
    ]);
    let mut doc = read_in("two holders", Mode::Tolerant, with.as_bytes());
    assert_refused_for(
        "two holders",
        &strict_err("two holders", &doc),
        &["c1", "c2"],
    );
    delete(&mut doc, "c1");
    assert_refused_for(
        "one holder left",
        &strict_err("one holder left", &doc),
        &["c2"],
    );
    delete(&mut doc, "c2");
    assert!(
        doc.elements()
            .all(|element| !element.has_preserved_content()),
        "{ROW}: an element holds preserved content after every holder is deleted"
    );
    let text = write_ok("no holder left", &doc, &strict());
    assert!(
        !text.contains("urn:v") && !text.contains("v:"),
        "{ROW}: the strict write after the deletes writes preserved content:\n{text}"
    );
    // It is the strict output of the same document read without them.
    let without = concat!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"\n",
        "    xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\n",
        "  <name content=\"P\"/>\n",
        "</sacm:ArgumentPackage>\n",
    );
    let plain = read_in("no Claims", Mode::Tolerant, without.as_bytes());
    assert_eq!(
        text,
        write_ok("no Claims", &plain, &strict()),
        "{ROW}: the strict write after the deletes differs from that of the same document read \
         without the holders"
    );
}
