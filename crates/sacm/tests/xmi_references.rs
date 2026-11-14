//! Tests for references between elements in XMI: the attribute form
//! (SACM23-XMI-005; XMI 2.5.1 rules 2h, 2j, 4i, §7.8.5 `xsd:IDREFS`; XML
//! Schema 1.0 Part 2 §3.3.10, §4.1.2.2) and the element form (SACM23-XMI-015;
//! XMI 2.5.1 production 2c, rules 2c, 2h, 2l, 2m, 2n, §7.10.1) (ADR 0024,
//! ADR 0026).
//!
//! In this release the reference ends are `citedElement` and `abstractForm`
//! (SACM23-BASE-007/008), both single-valued. An attribute in no namespace
//! named after the end holds the target ids separated by XML white space,
//! each held exactly as read. An element in no namespace named after the end,
//! with exactly one of `xmi:idref="id"` or an unqualified `href="#id"`,
//! trimmed, reads to the same model; `write` writes the attribute form. A
//! reference to an id that names no element is kept, written back and
//! reported by `validate` as SACM-REF-001 (SACM23-VAL-003, whose own tests
//! are `reference_validation.rs`); it is never dropped. Generated ids skip
//! every referenced id.
//!
//! No edit sets a reference yet (LIB-EDIT-008/009), so documents are read
//! from inline XMI. Expected locations are marked in the input with [`MARK`]
//! (U+2038), which the test removes before reading. Most tests run on both
//! ends.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{self, CreateClaim};
use sacm::model::{Document, ElementId, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

const XMI_005: &str = "SACM23-XMI-005";
const XMI_015: &str = "SACM23-XMI-015";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a refusal or an occurrence is expected.
/// Removed before reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The valid fixture of both rows.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/references-valid.sacm.xmi"
);

/// The invalid fixture of SACM23-XMI-015: a cross-document `href`.
const CROSS_DOCUMENT_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/invalid/reference-cross-document-href.sacm.xmi"
);

/// The two reference ends of this release, each with the flag that makes a
/// reference on it conform to §8.2 (so that only SACM-REF-001 is tested).
const ENDS: [(&str, &str); 2] = [
    ("citedElement", " isCitation=\"true\""),
    ("abstractForm", ""),
];

// --- Inputs ------------------------------------------------------------------------

/// An ArgumentPackage root `p` named "P", holding `body` from line 3.
fn in_ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  {body}\n\
         </sacm:ArgumentPackage>"
    )
}

/// A Claim `id` named "C" with `attributes` and holding `content` after its
/// name.
fn claim(id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"C\"/>{content}</argumentElement>"
    )
}

/// A Claim `c1` whose `end` is given by `attributes` and `content`, then a
/// Claim `c2`, in the root `p`.
fn c1_then_c2(attributes: &str, content: &str) -> String {
    in_ap(&format!(
        "{}{}",
        claim("c1", attributes, content),
        claim("c2", "", "")
    ))
}

fn loc(row: &str, line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{row}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, CRLF or a lone CR, and columns count characters.
fn marked(row: &str, template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column, mut after_cr) = (1_u32, 1_u32, false);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(row, line, column));
            continue;
        }
        text.push(c);
        match c {
            '\n' if after_cr => {}
            '\n' | '\r' => {
                line += 1;
                column = 1;
            }
            _ => column += 1,
        }
        after_cr = c == '\r';
    }
    (text, marks)
}

// --- Reading, writing and validating --------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to succeed with equal
/// documents and equal diagnostics, and returns the strict output.
fn read_both(row: &str, what: &str, input: &str) -> ReadOutput {
    let mut outputs = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => outputs.push(output),
            Err(error) => {
                panic!("{row}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
            }
        }
    }
    assert_eq!(
        outputs[0].document(),
        outputs[1].document(),
        "{row}: {what}: the modes read different documents"
    );
    assert_eq!(
        outputs[0].diagnostics(),
        outputs[1].diagnostics(),
        "{row}: {what}: the modes report different diagnostics"
    );
    outputs.swap_remove(0)
}

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to equal documents, and returns the strict one.
fn read_ok(row: &str, what: &str, input: &str) -> Document {
    let output = read_both(row, what, input);
    assert!(
        output.diagnostics().is_empty(),
        "{row}: {what}: the read reported {:?}",
        output.diagnostics()
    );
    output.into_document()
}

/// Reads `input` in both modes, which the test expects to be refused the same
/// in both, and returns the refusal.
fn refused(row: &str, what: &str, input: &str) -> ReadError {
    let mut errors = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => panic!(
                "{row}: {what}: a {mode:?} read returned a document of {} elements for {input:?}",
                output.document().element_count()
            ),
            Err(error) => errors.push(error),
        }
    }
    assert_eq!(
        errors[0], errors[1],
        "{row}: {what}: the modes refuse differently"
    );
    errors.swap_remove(0)
}

fn write_in(row: &str, what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// Writes `doc` in `mode`, reads the output back and writes that again: the
/// read-back must be equal and the second write must give the same bytes.
/// Returns the first output.
fn round_trip(row: &str, what: &str, doc: &Document, mode: Mode) -> String {
    let text = write_in(row, what, doc, mode);
    let back = read_both(row, &format!("{what}, written"), &text).into_document();
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{row}: {what}: the output does not read back equal\n{text}"
    );
    assert_eq!(&back, doc, "{row}: {what}: the output reads back !=");
    assert_eq!(
        write_in(row, what, &back, mode),
        text,
        "{row}: {what}: writing what was read back gives other bytes"
    );
    text
}

/// The element `id` of `doc`, which must exist.
fn element<'d>(row: &str, doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{row}: the document has no element {id:?}");
    };
    found
}

/// The target of `end` on the element `id`, owned.
fn target(row: &str, doc: &Document, id: &str, end: &str) -> Option<ElementId> {
    let found = element(row, doc, id);
    match end {
        "citedElement" => found.cited_element().cloned(),
        "abstractForm" => found.abstract_form().cloned(),
        other => panic!("{row}: setup: no end {other:?}"),
    }
}

/// The line of `text` carrying `xmi:id="<id>"`, trimmed.
fn line_with<'t>(row: &str, text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{row}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
}

/// The SACM-REF-001 findings of `validate` on `doc`.
fn dangling(doc: &Document) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|found| found.code() == Code::REF_DANGLING)
        .collect()
}

/// The affected ids of each SACM-REF-001 finding of `doc`.
fn dangling_affected(doc: &Document) -> Vec<Vec<String>> {
    dangling(doc)
        .iter()
        .map(|found| {
            found
                .affected()
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect()
        })
        .collect()
}

/// Asserts that `error` is `Unsupported`, with no code, at `at`.
fn assert_unsupported_at(row: &str, what: &str, error: &ReadError, at: SourceLocation) {
    assert!(
        matches!(error, ReadError::Unsupported { .. }) && error.code().is_none(),
        "{row}: {what}: not Unsupported: {error:?}"
    );
    assert_eq!(
        error.location(),
        Some(at),
        "{row}: {what}: not refused at the construct: {error}"
    );
}

/// Asserts that `error` is `RepeatedValue` for `end` on `id`, with exactly
/// `locations` and `location()` the second.
fn assert_repeated(
    row: &str,
    what: &str,
    error: &ReadError,
    id: &str,
    end: &str,
    locations: &[SourceLocation],
) {
    assert!(
        matches!(
            error,
            ReadError::RepeatedValue { element, property, .. }
                if element.as_str() == id && property == end
        ),
        "{row}: {what}: not RepeatedValue {{ {id}, {end} }}: {error:?}"
    );
    assert_eq!(
        (error.code(), error.locations(), error.location()),
        (
            Some(Code::MULTIPLICITY_VIOLATION),
            locations,
            locations.get(1).copied()
        ),
        "{row}: {what}: not SACM-MULT-001 at every occurrence"
    );
}

/// Runs `job` on a worker thread and fails if it does not return within
/// `deadline` or panics.
fn within<T: Send + 'static>(
    row: &str,
    what: &str,
    deadline: Duration,
    job: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(job());
    });
    match receiver.recv_timeout(deadline) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            panic!("{row}: {what}: did not return within {deadline:?}")
        }
        Err(RecvTimeoutError::Disconnected) => panic!("{row}: {what}: panicked"),
    }
}

// --- SACM23-XMI-005 item 1: read, write, round-trip ------------------------------------

#[test]
fn sacm23_xmi_005_cited_element_on_a_claim_and_abstract_form_on_a_package_read_and_round_trip() {
    let row = XMI_005;
    let input = format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
         <argumentPackage xmi:id=\"p1\" abstractForm=\"p2\"><name content=\"P1\"/>\
         {}{}</argumentPackage>\
         <argumentPackage xmi:id=\"p2\" isAbstract=\"true\"><name content=\"P2\"/></argumentPackage>\
         </sacm:AssuranceCasePackage>",
        claim("c1", " isCitation=\"true\" citedElement=\"c2\"", ""),
        claim("c2", "", "")
    );
    let what = "citedElement=\"c2\" on c1 and abstractForm=\"p2\" on p1";
    let doc = read_ok(row, what, &input);
    assert_eq!(
        (
            target(row, &doc, "c1", "citedElement"),
            target(row, &doc, "p1", "abstractForm")
        ),
        (Some(ElementId::from("c2")), Some(ElementId::from("p2"))),
        "{row}: {what}: the targets are not read"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: {what}: setup: the document does not validate cleanly"
    );
    // A strict write: the document is valid.
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "c1").contains(" citedElement=\"c2\"")
            && line_with(row, &text, "p1").contains(" abstractForm=\"p2\""),
        "{row}: {what}: the attributes are not written:\n{text}"
    );
}

#[test]
fn sacm23_xmi_005_the_fixture_reads_with_its_references_and_round_trips() {
    let row = XMI_005;
    let bytes = std::fs::read(FIXTURE)
        .unwrap_or_else(|error| panic!("{row}: cannot read {FIXTURE}: {error}"));
    let doc = read_ok(row, "the fixture", &String::from_utf8_lossy(&bytes));
    let targets: Vec<Option<ElementId>> = [
        ("c1", "citedElement"),
        ("c2", "citedElement"),
        ("c3", "citedElement"),
        ("c4", "abstractForm"),
        ("n1", "citedElement"),
    ]
    .into_iter()
    .map(|(id, end)| target(row, &doc, id, end))
    .collect();
    assert_eq!(
        targets,
        ["c2", "c1", "c3", "c3", "arguments"].map(|id| Some(ElementId::from(id))),
        "{row}: the fixture's references"
    );
    assert_eq!(
        dangling(&doc),
        [],
        "{row}: the fixture's references do not all resolve"
    );
    round_trip(row, "the fixture", &doc, Mode::Strict);
}

// --- Item 2: forward, cross-root and self references -------------------------------------

#[test]
fn sacm23_xmi_005_forward_cross_root_and_self_references_resolve_and_round_trip() {
    let row = XMI_005;
    for (end, flag) in ENDS {
        let cases = [
            (
                "a forward reference",
                in_ap(&format!(
                    "{}{}",
                    claim("c1", &format!("{flag} {end}=\"c2\""), ""),
                    claim("c2", " isAbstract=\"true\"", "")
                )),
                "c1",
                "c2",
            ),
            (
                "a backward reference",
                in_ap(&format!(
                    "{}{}",
                    claim("c1", " isAbstract=\"true\"", ""),
                    claim("c2", &format!("{flag} {end}=\"c1\""), "")
                )),
                "c2",
                "c1",
            ),
            (
                "a reference into a later root",
                format!(
                    "<xmi:XMI{NS}>\
                     <sacm:ArgumentPackage xmi:id=\"r1\"><name content=\"R1\"/>{}</sacm:ArgumentPackage>\
                     <sacm:ArgumentPackage xmi:id=\"r2\"><name content=\"R2\"/>{}</sacm:ArgumentPackage>\
                     </xmi:XMI>",
                    claim("c1", &format!("{flag} {end}=\"c9\""), ""),
                    claim("c9", " isAbstract=\"true\"", "")
                ),
                "c1",
                "c9",
            ),
            (
                "a reference into an earlier root",
                format!(
                    "<xmi:XMI{NS}>\
                     <sacm:ArgumentPackage xmi:id=\"r1\" isAbstract=\"true\"><name content=\"R1\"/></sacm:ArgumentPackage>\
                     <sacm:AssuranceCasePackage xmi:id=\"r2\"><name content=\"R2\"/>\
                     <argumentPackage xmi:id=\"q\"{flag} {end}=\"r1\"><name content=\"Q\"/></argumentPackage>\
                     </sacm:AssuranceCasePackage></xmi:XMI>"
                ),
                "q",
                "r1",
            ),
            (
                "a self-reference",
                in_ap(&claim("c1", &format!("{flag} {end}=\"c1\""), "")),
                "c1",
                "c1",
            ),
        ];
        for (what, input, referrer, wanted) in &cases {
            let what = format!("{what} on {end}");
            let doc = read_ok(row, &what, input);
            assert_eq!(
                target(row, &doc, referrer, end),
                Some(ElementId::from(*wanted)),
                "{row}: {what}: the target is not read"
            );
            assert_eq!(
                dangling(&doc),
                [],
                "{row}: {what}: the reference is reported as dangling"
            );
            let text = round_trip(row, &what, &doc, Mode::Tolerant);
            assert!(
                line_with(row, &text, referrer).contains(&format!(" {end}=\"{wanted}\"")),
                "{row}: {what}: not written back:\n{text}"
            );
        }
    }
}

// --- Item 3: a dangling reference is kept ------------------------------------------------

#[test]
fn sacm23_xmi_005_a_dangling_reference_is_kept_written_back_and_refused_only_by_a_strict_write() {
    let row = XMI_005;
    for (end, flag) in ENDS {
        let what = format!("{end}=\"nope\"");
        let doc = read_ok(
            row,
            &what,
            &c1_then_c2(&format!("{flag} {end}=\"nope\""), ""),
        );
        assert_eq!(
            target(row, &doc, "c1", end),
            Some(ElementId::from("nope")),
            "{row}: {what}: the reference is not held as nope"
        );
        assert!(
            doc.get("nope").is_none(),
            "{row}: {what}: an element was invented for the missing id"
        );
        let found = dangling(&doc);
        assert_eq!(
            dangling_affected(&doc),
            [["c1".to_owned(), "nope".to_owned()]],
            "{row}: {what}: validate does not give one SACM-REF-001 on [c1, nope]: {found:?}"
        );
        let text = round_trip(row, &what, &doc, Mode::Tolerant);
        assert!(
            line_with(row, &text, "c1").contains(&format!(" {end}=\"nope\"")),
            "{row}: {what}: a tolerant write drops the reference:\n{text}"
        );
        let error = match xmi::write(&doc, &WriteOptions::default()) {
            Ok(text) => panic!("{row}: {what}: a strict write returned:\n{text}"),
            Err(error) => error,
        };
        assert!(
            matches!(&error, WriteError::Invalid { .. })
                && found.iter().all(|f| error.diagnostics().contains(f)),
            "{row}: {what}: the strict refusal is not Invalid holding {found:?}: {error:?}"
        );
    }
}

// --- Item 4: white space around a token -----------------------------------------------------

#[test]
fn sacm23_xmi_005_white_space_around_a_token_is_not_part_of_the_id() {
    let row = XMI_005;
    for (end, flag) in ENDS {
        let plain = read_ok(
            row,
            "plain",
            &c1_then_c2(&format!("{flag} {end}=\"c2\""), ""),
        );
        for value in [" c2 ", "&#9;c2&#10;", "\tc2\n", "&#13;&#10; c2&#32;"] {
            let what = format!("{end}={value:?}");
            let doc = read_ok(
                row,
                &what,
                &c1_then_c2(&format!("{flag} {end}=\"{value}\""), ""),
            );
            assert_eq!(
                target(row, &doc, "c1", end),
                Some(ElementId::from("c2")),
                "{row}: {what}: not read as c2"
            );
            assert_eq!(
                semantic_compare(&doc, &plain),
                [],
                "{row}: {what}: does not read equal to {end}=\"c2\""
            );
            let text = round_trip(row, &what, &doc, Mode::Tolerant);
            assert!(
                line_with(row, &text, "c1").contains(&format!(" {end}=\"c2\"")),
                "{row}: {what}: not written as {end}=\"c2\":\n{text}"
            );
        }
    }
}

// --- Item 5: two tokens, no token, a namespace -------------------------------------------------

#[test]
fn sacm23_xmi_005_two_tokens_on_a_single_valued_end_are_repeated_value_at_the_attribute() {
    let row = XMI_005;
    for (end, flag) in ENDS {
        for (value, tokens) in [("a b", 2), ("c2 c2", 2), ("c2\tc1", 2), (" c1  c2 c1 ", 3)] {
            let what = format!("{end}={value:?}");
            let (input, marks) = marked(
                row,
                &in_ap(&format!(
                    "{}{}",
                    claim("c1", &format!("{flag} {MARK}{end}=\"{value}\""), ""),
                    claim("c2", "", "")
                )),
            );
            let error = refused(row, &what, &input);
            assert!(
                matches!(
                    &error,
                    ReadError::RepeatedValue { element, property, .. }
                        if element.as_str() == "c1" && property == end
                ),
                "{row}: {what}: not RepeatedValue {{ c1, {end} }}: {error:?}"
            );
            assert_eq!(
                (error.code(), error.location()),
                (Some(Code::MULTIPLICITY_VIOLATION), Some(marks[0])),
                "{row}: {what}: not SACM-MULT-001 at the attribute's name"
            );
            assert_eq!(
                error.locations(),
                vec![marks[0]; tokens].as_slice(),
                "{row}: {what}: not one occurrence per token, each at the attribute's name"
            );
        }
    }
}

#[test]
fn sacm23_xmi_005_an_empty_or_qualified_reference_attribute_is_unsupported_never_absent() {
    let row = XMI_005;
    for (end, flag) in ENDS {
        for (what, attribute) in [
            ("an empty value", format!("{MARK}{end}=\"\"")),
            ("a value of spaces", format!("{MARK}{end}=\"   \"")),
            (
                "a value of character-referenced white space",
                format!("{MARK}{end}=\"&#9;&#10;&#13;\""),
            ),
            ("the SACM namespace", format!("{MARK}sacm:{end}=\"c2\"")),
            (
                "another namespace",
                format!("xmlns:o=\"urn:o\" {MARK}o:{end}=\"c2\""),
            ),
        ] {
            let what = format!("{end}: {what}");
            let (input, marks) = marked(row, &c1_then_c2(&format!("{flag} {attribute}"), ""));
            let error = refused(row, &what, &input);
            assert_unsupported_at(row, &what, &error, marks[0]);
        }
    }
}

// --- Item 6: a token is held exactly ---------------------------------------------------------------

#[test]
fn sacm23_xmi_005_a_hash_is_part_of_the_token_and_a_non_ncname_id_resolves_as_written() {
    let row = XMI_005;
    for (end, flag) in ENDS {
        let what = format!("{end}=\"#c2\"");
        let doc = read_ok(
            row,
            &what,
            &c1_then_c2(&format!("{flag} {end}=\"#c2\""), ""),
        );
        assert_eq!(
            target(row, &doc, "c1", end),
            Some(ElementId::from("#c2")),
            "{row}: {what}: the # is stripped"
        );
        assert_eq!(
            dangling_affected(&doc),
            [["c1".to_owned(), "#c2".to_owned()]],
            "{row}: {what}: #c2 is not one dangling reference"
        );
        let text = round_trip(row, &what, &doc, Mode::Tolerant);
        assert!(
            line_with(row, &text, "c1").contains(&format!(" {end}=\"#c2\"")),
            "{row}: {what}: not written back exactly:\n{text}"
        );

        // `1c` is no NCName, so it is read as written (ADR 0024) and a token
        // equal to it resolves.
        let what = format!("{end}=\"1c\" naming the element 1c");
        let doc = read_ok(
            row,
            &what,
            &in_ap(&format!(
                "{}{}",
                claim("c1", &format!("{flag} {end}=\"1c\""), ""),
                claim("1c", "", "")
            )),
        );
        assert_eq!(
            (target(row, &doc, "c1", end), dangling(&doc)),
            (Some(ElementId::from("1c")), Vec::new()),
            "{row}: {what}: does not resolve"
        );
    }
}

// --- Item 7: escaping ----------------------------------------------------------------------------

#[test]
fn sacm23_xmi_005_a_target_id_with_markup_characters_is_written_escaped_and_reads_back_equal() {
    let row = XMI_005;
    for (end, flag) in ENDS {
        let escaped = "a&amp;&lt;&quot;&gt;b";
        let what = format!("{end} naming the id a&<\">b");
        let doc = read_ok(
            row,
            &what,
            &in_ap(&format!(
                "{}{}",
                claim("c1", &format!("{flag} {end}=\"{escaped}\""), ""),
                claim(escaped, "", "")
            )),
        );
        assert_eq!(
            (target(row, &doc, "c1", end), dangling(&doc)),
            (Some(ElementId::from("a&<\">b")), Vec::new()),
            "{row}: {what}: not read and resolved"
        );
        // Tolerant: the id is no NCName, a validation Error (SACM-ID-002).
        let text = round_trip(row, &what, &doc, Mode::Tolerant);
        assert!(
            line_with(row, &text, "c1").contains(&format!(" {end}=\"{escaped}\"")),
            "{row}: {what}: not written escaped:\n{text}"
        );
    }
}

// --- Item 8: generated ids skip referenced ids ---------------------------------------------------

#[test]
fn sacm23_xmi_005_a_generated_id_skips_an_id_that_is_only_referenced() {
    let row = XMI_005;
    for (end, flag) in ENDS {
        let idless =
            "<argumentElement xmi:type=\"sacm:Claim\"><name content=\"C\"/></argumentElement>";
        let referrer = claim("c1", &format!("{flag} {end}=\"claim_1\""), "");
        let element_form = claim("c1", flag, &format!("<{end} xmi:idref=\"claim_1\"/>"));
        for (what, body) in [
            (
                "the reference before the id-less Claim",
                format!("{referrer}{idless}"),
            ),
            (
                "the reference after the id-less Claim",
                format!("{idless}{referrer}"),
            ),
            (
                "an element-form reference after the id-less Claim",
                format!("{idless}{element_form}"),
            ),
            (
                "an href reference after the id-less Claim",
                format!(
                    "{idless}{}",
                    claim("c1", flag, &format!("<{end} href=\"#claim_1\"/>"))
                ),
            ),
            (
                "an href reference with white space after the id-less Claim",
                format!(
                    "{idless}{}",
                    claim("c1", flag, &format!("<{end} href=\" #claim_1 \"/>"))
                ),
            ),
            (
                "an xmi:idref reference with white space after the id-less Claim",
                format!(
                    "{idless}{}",
                    claim("c1", flag, &format!("<{end} xmi:idref=\" claim_1 \"/>"))
                ),
            ),
        ] {
            let what = format!("{end}: {what}");
            let output = read_both(row, &what, &in_ap(&body));
            let doc = output.document();
            let generated: Vec<&str> = doc
                .elements()
                .map(|e| e.id().as_str())
                .filter(|id| *id != "p" && *id != "c1")
                .collect();
            assert_eq!(
                generated,
                ["claim_2"],
                "{row}: {what}: the id-less Claim is not claim_2, the first id no element carries and none references"
            );
            assert_eq!(
                (
                    target(row, doc, "c1", end),
                    doc.contains("claim_1"),
                    dangling_affected(doc)
                ),
                (
                    Some(ElementId::from("claim_1")),
                    false,
                    vec![vec!["c1".to_owned(), "claim_1".to_owned()]]
                ),
                "{row}: {what}: the dangling reference resolved to the generated element"
            );
        }
    }
}

#[test]
fn sacm23_xmi_005_an_edits_generated_id_skips_a_referenced_id() {
    let row = XMI_005;
    for (end, flag) in ENDS {
        let what = format!("{end}=\"claim_1\" then CreateClaim");
        let mut doc = read_ok(
            row,
            &what,
            &in_ap(&claim("c1", &format!("{flag} {end}=\"claim_1\""), "")),
        );
        let created = match edit::apply(&mut doc, CreateClaim::new("p", LangString::new("New"))) {
            Ok(mutation) => mutation.created_ids().to_vec(),
            Err(error) => panic!("{row}: {what}: the edit was refused: {error:?}"),
        };
        assert_eq!(created.len(), 1, "{row}: {what}: not one created id");
        assert_ne!(
            created[0].as_str(),
            "claim_1",
            "{row}: {what}: the edit generated the referenced id, so the dangling reference resolves to it"
        );
        assert_eq!(
            dangling_affected(&doc),
            [["c1".to_owned(), "claim_1".to_owned()]],
            "{row}: {what}: the reference no longer dangles"
        );
    }
}

// --- Item 9: attribute order and determinism -------------------------------------------------------

#[test]
fn sacm23_xmi_005_the_flags_and_references_are_written_after_gid_and_before_the_class_attributes() {
    let row = XMI_005;
    // Each value in another order or form on input.
    let input = format!(
        "<sacm:ArgumentPackage{NS} abstractForm=\"c2\" isAbstract=\"1\" gid=\"g0\" \
         citedElement=\"c1\" xmi:id=\"p\" isCitation=\"true\">\
         <name content=\"P\"/>\
         <argumentElement assertionDeclaration=\"assumed\" abstractForm=\"c2\" isAbstract=\"true\" \
         gid=\"g1\" citedElement=\"c2\" isCitation=\"1\" xmi:id=\"c1\" xmi:type=\"sacm:Claim\">\
         <name content=\"C1\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\">\
         <abstractForm href=\"#c1\"/><isAbstract>true</isAbstract><name content=\"C2\"/>\
         <citedElement xmi:idref=\"c1\"/><isCitation>1</isCitation>\
         <note xmi:id=\"n\" abstractForm=\"p\" isCitation=\"true\"/></argumentElement>\
         </sacm:ArgumentPackage>"
    );
    let doc = read_ok(row, "every value", &input);
    let text = round_trip(row, "every value", &doc, Mode::Tolerant);
    let expected = [
        (
            "p",
            "xmi:id=\"p\" gid=\"g0\" isCitation=\"true\" isAbstract=\"true\" citedElement=\"c1\" \
             abstractForm=\"c2\">",
        ),
        (
            "c1",
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" gid=\"g1\" isCitation=\"true\" \
             isAbstract=\"true\" citedElement=\"c2\" abstractForm=\"c2\" assertionDeclaration=\"assumed\">",
        ),
        (
            "c2",
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\" isCitation=\"true\" \
             isAbstract=\"true\" citedElement=\"c1\" abstractForm=\"c1\">",
        ),
        (
            "n",
            "<note xmi:id=\"n\" isCitation=\"true\" abstractForm=\"p\" />",
        ),
    ];
    for (id, tail) in expected {
        let line = line_with(row, &text, id);
        assert!(
            line.ends_with(tail),
            "{row}: the start tag of {id:?} does not end with {tail:?}: {line}"
        );
    }
    assert!(
        !text.contains("<citedElement")
            && !text.contains("<abstractForm")
            && !text.contains("<isCitation")
            && !text.contains("<isAbstract"),
        "{row}: an element form is written:\n{text}"
    );
    // The same document read again writes the same bytes.
    let again = read_ok(row, "every value, read again", &input);
    assert_eq!(
        write_in(row, "every value, read again", &again, Mode::Tolerant),
        text,
        "{row}: the output is not deterministic"
    );
}

// --- Item 10: large values ------------------------------------------------------------------------

#[test]
fn sacm23_xmi_005_a_one_mib_reference_or_many_tokens_do_not_panic() {
    const MIB: usize = 1 << 20;
    let row = XMI_005;
    let deadline = Duration::from_secs(20);
    for (end, flag) in ENDS {
        // One token of 1 MiB: held exactly, and dangling.
        let long = "a".repeat(MIB);
        let input = c1_then_c2(&format!("{flag} {end}=\"{long}\""), "");
        let what = format!("a 1 MiB {end}");
        let result = within(row, &what, deadline, move || {
            xmi::read(input.as_bytes(), &ReadOptions::default())
        });
        match result {
            Ok(output) => {
                let held = target(row, output.document(), "c1", end);
                assert_eq!(
                    held.map(|id| id.as_str().len()),
                    Some(MIB),
                    "{row}: {what}: not held whole"
                );
            }
            Err(error) => panic!("{row}: {what}: refused: {error:?}"),
        }
        // Many tokens: one value repeated, refused without a panic or a hang.
        let many = "c2 ".repeat(MIB / 3);
        let input = c1_then_c2(&format!("{flag} {end}=\"{many}\""), "");
        let what = format!("{} tokens in {end}", MIB / 3);
        let result = within(row, &what, deadline, move || {
            xmi::read(input.as_bytes(), &ReadOptions::default())
        });
        assert!(
            matches!(result, Err(ReadError::RepeatedValue { .. })),
            "{row}: {what}: not RepeatedValue: {:?}",
            result.as_ref().err()
        );
        // 1 MiB of white space: no token.
        let blank = " ".repeat(MIB);
        let input = c1_then_c2(&format!("{flag} {end}=\"{blank}\""), "");
        let what = format!("1 MiB of white space in {end}");
        let result = within(row, &what, deadline, move || {
            xmi::read(input.as_bytes(), &ReadOptions::default())
        });
        assert!(
            matches!(result, Err(ReadError::Unsupported { .. })),
            "{row}: {what}: not Unsupported: {:?}",
            result.as_ref().err()
        );
    }
}

// --- SACM23-XMI-015 item 1: the element form ---------------------------------------------------

#[test]
fn sacm23_xmi_015_idref_and_href_elements_read_equal_to_the_attribute_anywhere_and_write_as_it() {
    let row = XMI_015;
    for (end, flag) in ENDS {
        let attribute = read_ok(
            row,
            "the attribute form",
            &c1_then_c2(&format!("{flag} {end}=\"c2\""), "<note xmi:id=\"n\"/>"),
        );
        let links = [
            format!("<{end} xmi:idref=\"c2\"/>"),
            format!("<{end} href=\"#c2\"/>"),
            format!("<{end} xmi:idref=\"c2\"></{end}>"),
            format!("<{end} href=\"#c2\">\n  <!-- c -->\n  <?pi x?>\n</{end}>"),
            format!("<{end} x:idref=\"c2\" xmlns:x=\"http://www.omg.org/spec/XMI/20131001\"/>"),
        ];
        for link in &links {
            let places = [
                (
                    "before the name",
                    in_ap(&format!(
                        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"{flag}>{link}\
                         <name content=\"C\"/><note xmi:id=\"n\"/></argumentElement>{}",
                        claim("c2", "", "")
                    )),
                ),
                (
                    "after the name",
                    c1_then_c2(flag, &format!("{link}<note xmi:id=\"n\"/>")),
                ),
                (
                    "after a child",
                    c1_then_c2(flag, &format!("<note xmi:id=\"n\"/>{link}")),
                ),
            ];
            for (place, input) in &places {
                let what = format!("{link} {place}");
                let doc = read_ok(row, &what, input);
                assert_eq!(
                    (semantic_compare(&doc, &attribute), &doc),
                    (Vec::new(), &attribute),
                    "{row}: {what}: does not read equal to {end}=\"c2\""
                );
                let text = round_trip(row, &what, &doc, Mode::Tolerant);
                assert!(
                    line_with(row, &text, "c1").contains(&format!(" {end}=\"c2\""))
                        && !text.contains(&format!("<{end}")),
                    "{row}: {what}: not written in the attribute form:\n{text}"
                );
            }
        }
    }
}

// --- Item 2: the link value is trimmed -----------------------------------------------------------

#[test]
fn sacm23_xmi_015_a_link_value_with_white_space_around_it_reads_trimmed_with_no_diagnostic() {
    let row = XMI_015;
    for (end, flag) in ENDS {
        let attribute = read_ok(
            row,
            "the attribute form",
            &c1_then_c2(&format!("{flag} {end}=\"c2\""), ""),
        );
        for link in [
            format!("<{end} xmi:idref=\" c2 \"/>"),
            format!("<{end} xmi:idref=\"&#9;c2&#10;\"/>"),
            format!("<{end} href=\" #c2 \"/>"),
        ] {
            let doc = read_ok(row, &link, &c1_then_c2(flag, &link));
            assert_eq!(
                (
                    target(row, &doc, "c1", end),
                    semantic_compare(&doc, &attribute)
                ),
                (Some(ElementId::from("c2")), Vec::new()),
                "{row}: {link}: not read as c2"
            );
        }
    }
}

#[test]
fn sacm23_xmi_015_an_idref_or_href_fragment_with_white_space_inside_is_unsupported_at_the_attribute()
 {
    let row = XMI_015;
    for (end, flag) in ENDS {
        // White space inside the trimmed value or fragment: no single id, and
        // it would write back as `end="a b"`, two ids on a single-valued end.
        let idrefs =
            ["a b", "a&#9;b", "a&#10;b", "a&#13;b", " a b "].map(|value| ("xmi:idref", value));
        let hrefs = ["#a b", "#a&#9;b"].map(|value| ("href", value));
        for (attribute, value) in idrefs.into_iter().chain(hrefs) {
            let what = format!("<{end} {attribute}={value:?}/>");
            let (input, marks) = marked(
                row,
                &c1_then_c2(flag, &format!("<{end} {MARK}{attribute}=\"{value}\"/>")),
            );
            assert_eq!(marks.len(), 1, "{row}: setup: {what} has not one mark");
            let error = refused(row, &what, &input);
            assert_unsupported_at(row, &what, &error, marks[0]);
        }
        // Control: white space only around the id is still trimmed, and the
        // id writes back as one token that reads back equal.
        let link = format!("<{end} xmi:idref=\" a \"/>");
        let doc = read_ok(row, &link, &c1_then_c2(flag, &link));
        assert_eq!(
            target(row, &doc, "c1", end),
            Some(ElementId::from("a")),
            "{row}: {link}: not read as a"
        );
        let text = round_trip(row, &link, &doc, Mode::Tolerant);
        assert!(
            line_with(row, &text, "c1").contains(&format!(" {end}=\"a\"")),
            "{row}: {link}: not written back as {end}=\"a\":\n{text}"
        );
    }
}

// --- Item 3: given twice -------------------------------------------------------------------------

#[test]
fn sacm23_xmi_015_an_attribute_and_an_element_or_two_elements_are_repeated_value_with_every_location()
 {
    let row = XMI_015;
    for (end, flag) in ENDS {
        let cases = [
            (
                "the attribute and an element of the same id",
                format!("{flag} {MARK}{end}=\"c2\""),
                format!("{MARK}<{end} xmi:idref=\"c2\"/>"),
            ),
            (
                "the attribute and an element of another id",
                format!("{flag} {MARK}{end}=\"c2\""),
                format!("{MARK}<{end} href=\"#c1\"/>"),
            ),
            (
                "two elements of the same id",
                flag.to_owned(),
                format!("{MARK}<{end} xmi:idref=\"c2\"/>{MARK}<{end} href=\"#c2\"/>"),
            ),
            (
                "two elements of other ids",
                flag.to_owned(),
                format!(
                    "{MARK}<{end} xmi:idref=\"c2\"/><note xmi:id=\"n\"/>{MARK}<{end} xmi:idref=\"c1\"/>"
                ),
            ),
            (
                "three elements",
                flag.to_owned(),
                format!(
                    "{MARK}<{end} xmi:idref=\"c2\"/>{MARK}<{end} xmi:idref=\"c2\"/>{MARK}<{end} xmi:idref=\"c2\"/>"
                ),
            ),
        ];
        for (what, attributes, content) in &cases {
            let what = format!("{end}: {what}");
            let (input, marks) = marked(row, &c1_then_c2(attributes, content));
            let error = refused(row, &what, &input);
            assert_repeated(row, &what, &error, "c1", end, &marks);
        }
    }
}

// --- Item 4: a dangling element reference is kept -------------------------------------------------

#[test]
fn sacm23_xmi_015_an_element_naming_an_unknown_id_is_kept_and_reported_by_validate() {
    let row = XMI_015;
    for (end, flag) in ENDS {
        for link in [
            format!("<{end} xmi:idref=\"nope\"/>"),
            format!("<{end} href=\"#nope\"/>"),
        ] {
            let doc = read_ok(row, &link, &c1_then_c2(flag, &link));
            assert_eq!(
                (target(row, &doc, "c1", end), dangling_affected(&doc)),
                (
                    Some(ElementId::from("nope")),
                    vec![vec!["c1".to_owned(), "nope".to_owned()]]
                ),
                "{row}: {link}: not kept and reported as one SACM-REF-001"
            );
            let found = &dangling(&doc)[0];
            assert_eq!(
                (found.severity(), found.requirement_id()),
                (Severity::Error, "SACM23-VAL-003"),
                "{row}: {link}: the finding is not SACM23-VAL-003's Error"
            );
            let text = round_trip(row, &link, &doc, Mode::Tolerant);
            assert!(
                line_with(row, &text, "c1").contains(&format!(" {end}=\"nope\"")),
                "{row}: {link}: not written back:\n{text}"
            );
        }
    }
}

// --- Item 5: href forms that are not same-document ids ----------------------------------------------

#[test]
fn sacm23_xmi_015_a_cross_document_xpointer_or_empty_href_is_unsupported_at_the_attribute() {
    let row = XMI_015;
    for (end, flag) in ENDS {
        for value in [
            "other.xmi#c2",
            "c2",
            "#xpointer(id('c2'))",
            "#",
            "",
            " ",
            "file:///case.xmi#c2",
        ] {
            let what = format!("<{end} href={value:?}/>");
            let (input, marks) = marked(
                row,
                &c1_then_c2(flag, &format!("<{end} {MARK}href=\"{value}\"/>")),
            );
            let error = refused(row, &what, &input);
            assert_unsupported_at(row, &what, &error, marks[0]);
        }
    }
}

#[test]
fn sacm23_xmi_015_the_cross_document_fixture_is_unsupported_never_dropped() {
    let row = XMI_015;
    let bytes = std::fs::read(CROSS_DOCUMENT_FIXTURE)
        .unwrap_or_else(|error| panic!("{row}: cannot read {CROSS_DOCUMENT_FIXTURE}: {error}"));
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let error = refused(row, "the cross-document fixture", &text);
    let Some(line) = text
        .lines()
        .position(|line| line.contains("href=\"other.sacm.xmi#c9\""))
    else {
        panic!("{row}: setup: the fixture has no cross-document href");
    };
    assert!(
        matches!(error, ReadError::Unsupported { .. })
            && error.location().map(SourceLocation::line) == u32::try_from(line + 1).ok(),
        "{row}: the fixture is not refused as Unsupported on the href's line: {error:?}"
    );
}

// --- Item 6: other malformed reference elements ------------------------------------------------------

#[test]
fn sacm23_xmi_015_a_malformed_reference_element_is_unsupported_at_that_construct() {
    let row = XMI_015;
    for (end, flag) in ENDS {
        let cases = [
            (
                "both link attributes, at the second",
                format!("<{end} xmi:idref=\"c2\" {MARK}href=\"#c2\"/>"),
            ),
            (
                "both link attributes, href first",
                format!("<{end} href=\"#c2\" {MARK}xmi:idref=\"c2\"/>"),
            ),
            ("neither link attribute", format!("{MARK}<{end}/>")),
            (
                "neither link attribute, with content",
                format!("{MARK}<{end}> <!-- c --> </{end}>"),
            ),
            (
                "an xmi:id",
                format!("<{end} {MARK}xmi:id=\"r\" xmi:idref=\"c2\"/>"),
            ),
            (
                "an xmi:type",
                format!("<{end} xmi:idref=\"c2\" {MARK}xmi:type=\"sacm:Claim\"/>"),
            ),
            (
                "an xsi:type",
                format!("<{end} href=\"#c2\" {MARK}xsi:type=\"sacm:Claim\"/>"),
            ),
            (
                "another attribute",
                format!("<{end} xmi:idref=\"c2\" {MARK}foo=\"1\"/>"),
            ),
            (
                "an unqualified idref",
                format!("<{end} {MARK}idref=\"c2\"/>"),
            ),
            (
                "an href in the XMI namespace",
                format!("<{end} {MARK}xmi:href=\"#c2\"/>"),
            ),
            (
                "a child element",
                format!("<{end} xmi:idref=\"c2\">{MARK}<b/></{end}>"),
            ),
            (
                "a nested reference element",
                format!("<{end} xmi:idref=\"c2\">{MARK}<{end} xmi:idref=\"c2\"/></{end}>"),
            ),
            (
                "text",
                format!("<{end} xmi:idref=\"c2\">  {MARK}c2</{end}>"),
            ),
            (
                "the SACM namespace",
                format!("{MARK}<sacm:{end} xmi:idref=\"c2\"/>"),
            ),
        ];
        for (what, content) in &cases {
            let what = format!("<{end}>: {what}");
            let (input, marks) = marked(row, &c1_then_c2(flag, content));
            assert_eq!(marks.len(), 1, "{row}: setup: {what} has not one mark");
            let error = refused(row, &what, &input);
            assert_unsupported_at(row, &what, &error, marks[0]);
        }
        // No id at all: refused, never read as absent. ADR 0026 does not say
        // whether at the element or at the attribute, so either is accepted.
        for (what, content) in [
            (
                "an empty idref",
                format!("{MARK}<{end} {MARK}xmi:idref=\"\"/>"),
            ),
            (
                "an idref of white space",
                format!("{MARK}<{end} {MARK}xmi:idref=\"  \"/>"),
            ),
            (
                "text as the only content",
                format!("{MARK}<{end}>{MARK}c2</{end}>"),
            ),
        ] {
            let what = format!("<{end}>: {what}");
            let (input, marks) = marked(row, &c1_then_c2(flag, &content));
            let error = refused(row, &what, &input);
            assert!(
                matches!(error, ReadError::Unsupported { .. })
                    && error.location().is_some_and(|at| marks.contains(&at)),
                "{row}: {what}: not Unsupported at the element or the construct: {error:?}"
            );
        }
    }
}
