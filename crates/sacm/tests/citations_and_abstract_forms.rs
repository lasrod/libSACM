//! Tests for the `SACMElement` flags and references `isCitation` and
//! `citedElement` (SACM23-BASE-007) and `isAbstract` and `abstractForm`
//! (SACM23-BASE-008) (SACM 2.3 §8.2; XMI 2.5.1 §7.8.4, rule 2i, §9.4.1;
//! XML Schema 1.0 Part 2 §3.2.2.1; ADR 0021 amendment 1, ADR 0026).
//!
//! Every element of every kind holds both flags, Booleans defaulting to
//! false, and both references, each an optional id. A flag is read in both
//! modes from the unqualified attribute or a value element, as an xsd:boolean:
//! white space collapsed, `true`/`1` true, `false`/`0` false, absent false.
//! Any other value is `ReadError::InvalidLiteral` naming the flag; given
//! twice it is `RepeatedValue`. `xmi::write` writes a flag only when true, as
//! `"true"`, after `gid`. The flag and its reference are independent: either
//! may be present without the other. `semantic_compare` reports
//! `Difference::IsCitation`, `IsAbstract`, `CitedElement` and `AbstractForm`,
//! in that order, after `Gid`.
//!
//! SACM23-BASE-008's first acceptance item is SACM23-BASE-007's items 1-8
//! with `isAbstract` and `abstractForm`: each such test runs one body, given
//! the row's [`Feature`], under its own row's name. The reference syntax
//! itself (IDREFS, the element form, dangling ids) is `xmi_references.rs`;
//! the §8.2 rules are `reference_validation.rs`.
//!
//! No edit sets these values yet (LIB-EDIT-008/009), so documents are read
//! from inline XMI. Expected locations are marked in the input with [`MARK`]
//! (U+2038), which the test removes before reading.

use std::path::Path;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, SourceLocation};
use sacm::edit::{self, CreateArgumentPackage, CreateClaim};
use sacm::model::{Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a start tag, an attribute or a child is
/// expected. Removed before reading.
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
    "/tests/data/sacm23/xmi/citations-and-abstract-forms-valid.sacm.xmi"
);

// --- The two features ----------------------------------------------------------

/// One flag and its reference, with the row that owns them.
#[derive(Clone, Copy, Debug)]
struct Feature {
    row: &'static str,
    /// The flag's XMI name: `isCitation` or `isAbstract`.
    flag: &'static str,
    /// The reference end's XMI name: `citedElement` or `abstractForm`.
    end: &'static str,
}

const CITATION: Feature = Feature {
    row: "SACM23-BASE-007",
    flag: "isCitation",
    end: "citedElement",
};

const ABSTRACT: Feature = Feature {
    row: "SACM23-BASE-008",
    flag: "isAbstract",
    end: "abstractForm",
};

impl Feature {
    /// The flag of `element`, through its accessor.
    fn flag_of(self, element: SacmElement<'_>) -> bool {
        if self.flag == "isCitation" {
            element.is_citation()
        } else {
            element.is_abstract()
        }
    }

    /// The reference of `element`, through its accessor, owned.
    fn end_of(self, element: SacmElement<'_>) -> Option<ElementId> {
        if self.end == "citedElement" {
            element.cited_element().cloned()
        } else {
            element.abstract_form().cloned()
        }
    }

    /// The `Difference` of the flag.
    fn flag_difference(self, id: &str, left: bool, right: bool) -> Difference {
        let id = ElementId::from(id);
        if self.flag == "isCitation" {
            Difference::IsCitation { id, left, right }
        } else {
            Difference::IsAbstract { id, left, right }
        }
    }

    /// The `Difference` of the reference.
    fn end_difference(self, id: &str, left: Option<&str>, right: Option<&str>) -> Difference {
        let id = ElementId::from(id);
        let left = left.map(ElementId::from);
        let right = right.map(ElementId::from);
        if self.end == "citedElement" {
            Difference::CitedElement { id, left, right }
        } else {
            Difference::AbstractForm { id, left, right }
        }
    }
}

// --- Inputs ------------------------------------------------------------------------

/// Where a flag is placed: the input given the start-tag attributes and the
/// leading content of the element `x` that carries it.
type Holder = fn(&str, &str) -> String;

/// Every readable kind: an AssuranceCasePackage root, an ArgumentPackage
/// root, a Claim, and each of the four `UtilityElement` kinds. Each is `x`,
/// named or with its `content`/`key`, so that only the tested values differ.
fn holders() -> [(&'static str, Holder); 7] {
    [
        (
            "an AssuranceCasePackage root",
            |attributes: &str, content: &str| {
                format!(
                    "<sacm:AssuranceCasePackage{NS} xmi:id=\"x\"{attributes}>{content}\
                     <name content=\"X\"/></sacm:AssuranceCasePackage>"
                )
            },
        ),
        (
            "an ArgumentPackage root",
            |attributes: &str, content: &str| {
                format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"x\"{attributes}>{content}\
                     <name content=\"X\"/></sacm:ArgumentPackage>"
                )
            },
        ),
        ("a Claim", |attributes: &str, content: &str| {
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"{attributes}>{content}\
                 <name content=\"X\"/></argumentElement>"
            ))
        }),
        ("a Description", |attributes: &str, content: &str| {
            in_ap(&format!(
                "<description xmi:id=\"x\"{attributes}>{content}\
                 <content><value content=\"d\"/></content></description>"
            ))
        }),
        (
            "an ImplementationConstraint",
            |attributes: &str, content: &str| {
                in_ap(&format!(
                    "<implementationConstraint xmi:id=\"x\"{attributes}>{content}\
                     </implementationConstraint>"
                ))
            },
        ),
        ("a Note", |attributes: &str, content: &str| {
            in_ap(&format!("<note xmi:id=\"x\"{attributes}>{content}</note>"))
        }),
        ("a TaggedValue", |attributes: &str, content: &str| {
            in_ap(&format!(
                "<taggedValue xmi:id=\"x\"{attributes}>{content}\
                 <key><value content=\"k\"/></key></taggedValue>"
            ))
        }),
    ]
}

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

// --- Reading and writing -------------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to equal documents, and returns the strict one.
fn read_ok(row: &str, what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => {
                assert!(
                    output.diagnostics().is_empty(),
                    "{row}: {what}: reading in {mode:?} mode reported {:?}",
                    output.diagnostics()
                );
                documents.push(output.into_document());
            }
            Err(error) => {
                panic!("{row}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
            }
        }
    }
    assert_eq!(
        documents[0], documents[1],
        "{row}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
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

/// Writes `doc` tolerantly: the §8.2 rules may report an Error for a
/// combination these tests read on purpose, and the save mode is not what is
/// tested.
fn write(row: &str, what: &str, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: the write refused the document: {error:?}"),
    }
}

/// Writes `doc`, reads the output back and writes that again: the read-back
/// must be equal and the second write must give the same bytes. Returns the
/// first output.
fn round_trip(row: &str, what: &str, doc: &Document) -> String {
    let text = write(row, what, doc);
    let back = read_ok(row, &format!("{what}, written"), &text);
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{row}: {what}: the output does not read back equal\n{text}"
    );
    assert_eq!(&back, doc, "{row}: {what}: the output reads back !=");
    assert_eq!(
        write(row, what, &back),
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

/// The line of `text` carrying `xmi:id="<id>"`, trimmed.
fn line_with<'t>(row: &str, text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{row}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
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

// --- BASE-007/008 item 1: the defaults -------------------------------------------------

/// A document with one element of every kind, each under the ArtifactPackage
/// root "pkg" (which admits every kind, LIB-API-008), with the id `e_<kind>`.
fn every_kind_document(row: &str) -> Document {
    let mut doc = Document::new();
    let pkg = ElementId::from("pkg");
    if let Err(error) = doc.insert(None, ElementKind::ArtifactPackage, pkg.clone(), None) {
        panic!("{row}: setup: the root could not be inserted: {error:?}");
    }
    for &kind in ElementKind::ALL {
        let id = ElementId::from(format!("e_{kind}"));
        if let Err(error) = doc.insert(Some(&pkg), kind, id, None) {
            panic!("{row}: setup: a {kind} could not be inserted: {error:?}");
        }
    }
    doc
}

fn every_kind_starts_false_with_no_target(feature: Feature) {
    let row = feature.row;
    let doc = every_kind_document(row);
    assert_eq!(
        doc.element_count(),
        ElementKind::ALL.len() + 1,
        "{row}: setup: not every kind was inserted"
    );
    for found in doc.elements() {
        assert_eq!(
            (feature.flag_of(found), feature.end_of(found)),
            (false, None),
            "{row}: a new {} starts with {} or {} set",
            found.kind(),
            feature.flag,
            feature.end
        );
    }
    // Elements an edit creates, and elements read without the values.
    let mut built = Document::new();
    let created = edit::apply(
        &mut built,
        CreateArgumentPackage::root(LangString::new("P")).with_id("p"),
    )
    .and_then(|_| edit::apply(&mut built, CreateClaim::new("p", LangString::new("C"))));
    if let Err(error) = created {
        panic!("{row}: setup: an edit was refused: {error:?}");
    }
    for found in built.elements() {
        assert_eq!(
            (feature.flag_of(found), feature.end_of(found)),
            (false, None),
            "{row}: the {} {:?} created by an edit has {} or {} set",
            found.kind(),
            found.id().as_str(),
            feature.flag,
            feature.end
        );
    }
    for (holder, input_of) in holders() {
        let doc = read_ok(row, holder, &input_of("", ""));
        let found = element(row, &doc, "x");
        assert_eq!(
            (feature.flag_of(found), feature.end_of(found)),
            (false, None),
            "{row}: {holder} read without {} or {} has them set",
            feature.flag,
            feature.end
        );
    }
}

#[test]
fn sacm23_base_007_every_kind_starts_with_is_citation_false_and_no_cited_element() {
    every_kind_starts_false_with_no_target(CITATION);
}

#[test]
fn sacm23_base_008_every_kind_starts_with_is_abstract_false_and_no_abstract_form() {
    every_kind_starts_false_with_no_target(ABSTRACT);
}

// --- Item 2: the literals, on every readable kind ---------------------------------------

/// The forms of a flag `f` that read: what, the start-tag attributes, the
/// leading content, and the value read.
fn flag_forms(flag: &str) -> Vec<(String, String, String, bool)> {
    let attribute = |value: &str, expected: bool| {
        (
            format!("{flag}={value:?}"),
            format!(" {flag}=\"{value}\""),
            String::new(),
            expected,
        )
    };
    let element = |value: &str, expected: bool| {
        (
            format!("<{flag}>{value:?}</{flag}>"),
            String::new(),
            format!("<{flag}>{value}</{flag}>"),
            expected,
        )
    };
    vec![
        attribute("true", true),
        attribute("1", true),
        attribute(" true ", true),
        attribute("&#9;1&#10;", true),
        attribute("false", false),
        attribute("0", false),
        attribute(" false ", false),
        element("true", true),
        element("1", true),
        element(" true ", true),
        element("&#9;1&#10;", true),
        element("\n    true\n  ", true),
        element("<![CDATA[true]]>", true),
        element("t<!-- c -->rue", true),
        element("false", false),
        element("0", false),
        ("absent".to_owned(), String::new(), String::new(), false),
    ]
}

fn literals_read_on_every_readable_kind(feature: Feature) {
    let row = feature.row;
    for (holder, input_of) in holders() {
        let absent = read_ok(row, holder, &input_of("", ""));
        for (form, attributes, content, expected) in flag_forms(feature.flag) {
            let what = format!("{form} on {holder}");
            let doc = read_ok(row, &what, &input_of(&attributes, &content));
            let found = element(row, &doc, "x");
            assert_eq!(
                (feature.flag_of(found), feature.end_of(found)),
                (expected, None),
                "{row}: {what}: not read as {expected}, with no {}",
                feature.end
            );
            if !expected {
                assert_eq!(
                    (semantic_compare(&doc, &absent), &doc),
                    (Vec::new(), &absent),
                    "{row}: {what}: false does not compare equal to an absent {}",
                    feature.flag
                );
            }
            round_trip(row, &what, &doc);
        }
    }
}

#[test]
fn sacm23_base_007_true_1_false_0_and_absent_read_on_every_readable_kind() {
    literals_read_on_every_readable_kind(CITATION);
}

#[test]
fn sacm23_base_008_true_1_false_0_and_absent_read_on_every_readable_kind() {
    literals_read_on_every_readable_kind(ABSTRACT);
}

#[test]
fn sacm23_base_007_the_fixture_reads_with_its_citations_and_abstract_forms_and_round_trips() {
    let row = CITATION.row;
    let bytes = std::fs::read(Path::new(FIXTURE))
        .unwrap_or_else(|error| panic!("{row}: cannot read {FIXTURE}: {error}"));
    let doc = read_ok(row, "the fixture", &String::from_utf8_lossy(&bytes));
    let state = |id: &str| {
        let found = element(row, &doc, id);
        (
            found.is_citation(),
            found.cited_element().map(ElementId::as_str),
            found.is_abstract(),
            found.abstract_form().map(ElementId::as_str),
        )
    };
    assert_eq!(
        ["case", "pattern", "abstract_claim", "c1", "c2", "n1"].map(state),
        [
            (false, None, false, None),
            (false, None, true, None),
            (false, None, true, None),
            (false, None, false, Some("abstract_claim")),
            (true, Some("c1"), false, None),
            (true, None, false, None),
        ],
        "{row}: the fixture's flags and references"
    );
    round_trip(row, "the fixture", &doc);
}

// --- Item 3: values that are not literals ------------------------------------------------

fn bad_values_are_invalid_literal(feature: Feature) {
    let row = feature.row;
    let flag = feature.flag;
    for (holder, input_of) in holders() {
        let mut cases: Vec<(String, String, String, &str)> = Vec::new();
        for value in ["TRUE", "True", "yes", "", "t rue", "2", "on", "false true"] {
            cases.push((
                format!("{flag}={value:?}"),
                format!(" {MARK}{flag}=\"{value}\""),
                String::new(),
                value,
            ));
        }
        for value in ["TRUE", "yes", "", "t rue"] {
            cases.push((
                format!("<{flag}>{value:?}</{flag}>"),
                String::new(),
                format!("{MARK}<{flag}>{value}</{flag}>"),
                value,
            ));
        }
        cases.push((
            format!("<{flag}/>"),
            String::new(),
            format!("{MARK}<{flag}/>"),
            "",
        ));
        for (form, attributes, content, value) in cases {
            let what = format!("{form} on {holder}");
            let (input, marks) = marked(row, &input_of(&attributes, &content));
            let error = refused(row, &what, &input);
            assert!(
                matches!(
                    &error,
                    ReadError::InvalidLiteral { element, property, value: read, .. }
                        if element.as_str() == "x" && property == flag && read == value
                ),
                "{row}: {what}: not InvalidLiteral {{ x, {flag}, {value:?} }}: {error:?}"
            );
            assert_eq!(
                (error.code(), error.location()),
                (Some(Code::ENUM_INVALID_LITERAL), marks.first().copied()),
                "{row}: {what}: not SACM-ENUM-001 at the value"
            );
            let shown = error.to_string();
            assert!(
                shown.contains(&format!("{value:?}"))
                    && shown.contains(flag)
                    && !shown.contains('\n'),
                "{row}: {what}: the Display {shown:?} is not one line quoting the value and naming {flag}"
            );
            assert!(
                !shown.contains("needsSupport") && !shown.contains("asCited"),
                "{row}: {what}: the Display {shown:?} lists the assertionDeclaration literals for a Boolean"
            );
        }
    }
}

#[test]
fn sacm23_base_007_a_value_that_is_no_boolean_literal_is_invalid_literal_naming_is_citation() {
    bad_values_are_invalid_literal(CITATION);
}

#[test]
fn sacm23_base_008_a_value_that_is_no_boolean_literal_is_invalid_literal_naming_is_abstract() {
    bad_values_are_invalid_literal(ABSTRACT);
}

// --- Item 4: the element form ------------------------------------------------------------

#[expect(
    clippy::too_many_lines,
    reason = "the item's reading, refusals and repeats read best as one list"
)]
fn the_element_form_reads_and_is_refused_where_malformed(feature: Feature) {
    let row = feature.row;
    let flag = feature.flag;
    // Anywhere among the siblings.
    let attribute = read_ok(
        row,
        "the attribute form",
        &in_ap(&claim(
            "c1",
            &format!(" {flag}=\"true\""),
            "<note xmi:id=\"n\"/>",
        )),
    );
    for (what, input) in [
        (
            "before the name",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><{flag}> true </{flag}>\
                 <name content=\"C\"/><note xmi:id=\"n\"/></argumentElement>"
            )),
        ),
        (
            "after the name",
            in_ap(&claim(
                "c1",
                "",
                &format!("<{flag}> true </{flag}><note xmi:id=\"n\"/>"),
            )),
        ),
        (
            "after a child",
            in_ap(&claim(
                "c1",
                "",
                &format!("<note xmi:id=\"n\"/><{flag}> true </{flag}>"),
            )),
        ),
    ] {
        let doc = read_ok(row, what, &input);
        assert_eq!(
            semantic_compare(&doc, &attribute),
            [],
            "{row}: {what}: <{flag}> true </{flag}> does not read equal to the attribute form"
        );
    }

    // Refused at the construct.
    let unsupported = [
        (
            "an attribute on the value element",
            format!("<{flag} {MARK}xmi:id=\"v\">true</{flag}>"),
        ),
        (
            "xsi:nil on the value element",
            format!("<{flag} {MARK}xsi:nil=\"true\"/>"),
        ),
        (
            "a child element in the value element",
            format!("<{flag}>true{MARK}<b/></{flag}>"),
        ),
        (
            "the value element in the SACM namespace",
            format!("{MARK}<sacm:{flag}>true</sacm:{flag}>"),
        ),
    ];
    for (what, content) in &unsupported {
        let (input, marks) = marked(row, &in_ap(&claim("c1", "", content)));
        let error = refused(row, what, &input);
        assert_unsupported_at(row, what, &error, marks[0]);
    }
    let (input, marks) = marked(
        row,
        &in_ap(&claim("c1", &format!(" {MARK}sacm:{flag}=\"true\""), "")),
    );
    let what = format!("sacm:{flag} as an attribute");
    let error = refused(row, &what, &input);
    assert_unsupported_at(row, &what, &error, marks[0]);

    // Given twice, with every occurrence.
    for (holder, input_of) in holders() {
        for (what, attributes, content) in [
            (
                "the attribute and the element",
                format!(" {MARK}{flag}=\"true\""),
                format!("{MARK}<{flag}>true</{flag}>"),
            ),
            (
                "the attribute and a different element",
                format!(" {MARK}{flag}=\"false\""),
                format!("{MARK}<{flag}>1</{flag}>"),
            ),
            (
                "two elements",
                String::new(),
                format!("{MARK}<{flag}>0</{flag}>{MARK}<{flag}>0</{flag}>"),
            ),
        ] {
            let what = format!("{what} on {holder}");
            let (input, marks) = marked(row, &input_of(&attributes, &content));
            let error = refused(row, &what, &input);
            assert!(
                matches!(
                    &error,
                    ReadError::RepeatedValue { element, property, .. }
                        if element.as_str() == "x" && property == flag
                ),
                "{row}: {what}: not RepeatedValue {{ x, {flag} }}: {error:?}"
            );
            assert_eq!(
                (error.code(), error.locations(), error.location()),
                (
                    Some(Code::MULTIPLICITY_VIOLATION),
                    marks.as_slice(),
                    marks.get(1).copied()
                ),
                "{row}: {what}: not SACM-MULT-001 at every occurrence"
            );
        }
    }
}

#[test]
fn sacm23_base_007_the_is_citation_element_form_reads_and_its_malformed_or_repeated_forms_are_refused()
 {
    the_element_form_reads_and_is_refused_where_malformed(CITATION);
}

#[test]
fn sacm23_base_008_the_is_abstract_element_form_reads_and_its_malformed_or_repeated_forms_are_refused()
 {
    the_element_form_reads_and_is_refused_where_malformed(ABSTRACT);
}

// --- Item 5: writing ---------------------------------------------------------------------

fn written_as_true_after_gid_and_not_at_all_when_false(feature: Feature) {
    let row = feature.row;
    let flag = feature.flag;
    for (holder, input_of) in holders() {
        for (form, attributes, content) in [
            ("\"1\"", format!(" {flag}=\"1\""), String::new()),
            ("\"true\"", format!(" {flag}=\"true\""), String::new()),
            ("<_>1</_>", String::new(), format!("<{flag}>1</{flag}>")),
        ] {
            let what = format!("{flag}={form} with a gid on {holder}");
            let doc = read_ok(
                row,
                &what,
                &input_of(&format!("{attributes} gid=\"g\""), &content),
            );
            let text = round_trip(row, &what, &doc);
            let line = line_with(row, &text, "x");
            assert!(
                line.contains(&format!("xmi:id=\"x\" gid=\"g\" {flag}=\"true\"")),
                "{row}: {what}: not written as {flag}=\"true\" directly after gid: {line}"
            );
            assert_eq!(
                text.matches(flag).count(),
                1,
                "{row}: {what}: {flag} is written more than once:\n{text}"
            );
            let what = format!("{flag}={form} without a gid on {holder}");
            let doc = read_ok(row, &what, &input_of(&attributes, &content));
            let text = round_trip(row, &what, &doc);
            let line = line_with(row, &text, "x");
            assert!(
                line.contains(&format!("xmi:id=\"x\" {flag}=\"true\"")),
                "{row}: {what}: not written directly after xmi:id: {line}"
            );
        }
        for (form, attributes, content) in [
            ("\"false\"", format!(" {flag}=\"false\""), String::new()),
            ("\"0\"", format!(" {flag}=\"0\""), String::new()),
            (
                "<_>false</_>",
                String::new(),
                format!("<{flag}>false</{flag}>"),
            ),
            ("absent", String::new(), String::new()),
        ] {
            let what = format!("{flag}={form} on {holder}");
            let doc = read_ok(row, &what, &input_of(&attributes, &content));
            let text = round_trip(row, &what, &doc);
            assert!(
                !text.contains(flag),
                "{row}: {what}: a false {flag} is written:\n{text}"
            );
        }
    }
}

#[test]
fn sacm23_base_007_is_citation_is_written_as_true_after_gid_and_not_at_all_when_false() {
    written_as_true_after_gid_and_not_at_all_when_false(CITATION);
}

#[test]
fn sacm23_base_008_is_abstract_is_written_as_true_after_gid_and_not_at_all_when_false() {
    written_as_true_after_gid_and_not_at_all_when_false(ABSTRACT);
}

// --- Item 6: independence ------------------------------------------------------------------

fn the_flag_and_the_reference_are_independent(feature: Feature) {
    let row = feature.row;
    let (flag, end) = (feature.flag, feature.end);
    let cases = [
        (
            format!("{flag} true with no {end}"),
            format!(" {flag}=\"true\""),
            true,
            None,
        ),
        (
            format!("{end} with {flag} absent"),
            format!(" {end}=\"c2\""),
            false,
            Some("c2"),
        ),
        (
            format!("{end} with {flag} false"),
            format!(" {flag}=\"false\" {end}=\"c2\""),
            false,
            Some("c2"),
        ),
        (
            format!("{end} with {flag} true"),
            format!(" {flag}=\"true\" {end}=\"c2\""),
            true,
            Some("c2"),
        ),
    ];
    for (what, attributes, flag_value, target) in cases {
        let doc = read_ok(
            row,
            &what,
            &in_ap(&format!(
                "{}{}",
                claim("c1", &attributes, ""),
                claim("c2", "", "")
            )),
        );
        let found = element(row, &doc, "c1");
        assert_eq!(
            (feature.flag_of(found), feature.end_of(found)),
            (flag_value, target.map(ElementId::from)),
            "{row}: {what}: not read as written; nothing may repair it"
        );
        let text = round_trip(row, &what, &doc);
        let line = line_with(row, &text, "c1");
        assert_eq!(
            (
                line.contains(&format!("{flag}=\"true\"")),
                line.contains(&format!("{end}=\"c2\""))
            ),
            (flag_value, target.is_some()),
            "{row}: {what}: not written back unchanged: {line}"
        );
    }
}

#[test]
fn sacm23_base_007_is_citation_and_cited_element_are_independent_and_round_trip_unchanged() {
    the_flag_and_the_reference_are_independent(CITATION);
}

#[test]
fn sacm23_base_008_is_abstract_and_abstract_form_are_independent_and_round_trip_unchanged() {
    the_flag_and_the_reference_are_independent(ABSTRACT);
}

// --- Item 7: not on a LangString or MultiLangString ---------------------------------------

fn not_on_a_name_content_key_or_value(feature: Feature) {
    let row = feature.row;
    let description =
        |body: &str| in_ap(&format!("<description xmi:id=\"d\">{body}</description>"));
    let tagged = |body: &str| in_ap(&format!("<taggedValue xmi:id=\"t\">{body}</taggedValue>"));
    let named = |name: &str| {
        in_ap(&format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">{name}</argumentElement>"
        ))
    };
    for (attribute, element) in [
        (
            format!("{}=\"true\"", feature.flag),
            format!("<{0}>true</{0}>", feature.flag),
        ),
        (
            format!("{}=\"c1\"", feature.end),
            format!("<{} xmi:idref=\"c1\"/>", feature.end),
        ),
    ] {
        let cases = [
            (
                "on a name",
                named(&format!("<name content=\"N\" {MARK}{attribute}/>")),
            ),
            (
                "in a name",
                named(&format!("<name content=\"N\">{MARK}{element}</name>")),
            ),
            (
                "on a content",
                description(&format!("<content {MARK}{attribute}/>")),
            ),
            (
                "in a content",
                description(&format!("<content>{MARK}{element}</content>")),
            ),
            ("on a key", tagged(&format!("<key {MARK}{attribute}/>"))),
            ("in a key", tagged(&format!("<key>{MARK}{element}</key>"))),
            (
                "on a value",
                description(&format!(
                    "<content><value content=\"x\" {MARK}{attribute}/></content>"
                )),
            ),
            (
                "in a value",
                description(&format!(
                    "<content><value content=\"x\">{MARK}{element}</value></content>"
                )),
            ),
        ];
        for (place, template) in &cases {
            let what = format!("{attribute} {place}");
            let (input, marks) = marked(row, template);
            assert_eq!(marks.len(), 1, "{row}: setup: {what} has not one mark");
            let error = refused(row, &what, &input);
            assert_unsupported_at(row, &what, &error, marks[0]);
        }
    }
}

#[test]
fn sacm23_base_007_is_citation_on_a_name_content_key_or_value_is_unsupported() {
    not_on_a_name_content_key_or_value(CITATION);
}

#[test]
fn sacm23_base_008_is_abstract_on_a_name_content_key_or_value_is_unsupported() {
    not_on_a_name_content_key_or_value(ABSTRACT);
}

// --- Item 8: comparison --------------------------------------------------------------------

fn each_value_is_one_difference(feature: Feature) {
    let row = feature.row;
    let (flag, end) = (feature.flag, feature.end);
    let with = |what: &str, attributes: &str| {
        read_ok(
            row,
            what,
            &in_ap(&format!(
                "{}{}{}",
                claim("c1", attributes, ""),
                claim("c2", "", ""),
                claim("c3", "", "")
            )),
        )
    };
    let plain = with("plain", "");
    let flagged = with("flagged", &format!(" {flag}=\"true\""));
    let to_c2 = with("to c2", &format!(" {end}=\"c2\""));
    let to_c3 = with("to c3", &format!(" {end}=\"c3\""));
    let cases = [
        (
            "false and true",
            &plain,
            &flagged,
            feature.flag_difference("c1", false, true),
        ),
        (
            "true and false",
            &flagged,
            &plain,
            feature.flag_difference("c1", true, false),
        ),
        (
            "no target and c2",
            &plain,
            &to_c2,
            feature.end_difference("c1", None, Some("c2")),
        ),
        (
            "c2 and c3",
            &to_c2,
            &to_c3,
            feature.end_difference("c1", Some("c2"), Some("c3")),
        ),
        (
            "c3 and no target",
            &to_c3,
            &plain,
            feature.end_difference("c1", Some("c3"), None),
        ),
    ];
    for (what, left, right, expected) in cases {
        assert_ne!(left, right, "{row}: {what}: the documents are ==");
        assert_eq!(
            semantic_compare(left, right),
            std::slice::from_ref(&expected),
            "{row}: {what}: not one difference"
        );
        assert_eq!(
            expected.element_id(),
            Some(&ElementId::from("c1")),
            "{row}: {what}: element_id()"
        );
        let shown = expected.to_string();
        let named = if what.contains("target") || what.starts_with('c') {
            end
        } else {
            flag
        };
        assert!(
            !shown.contains('\n') && shown.contains("\"c1\"") && shown.contains(named),
            "{row}: {what}: Display {shown:?} is not one line naming \"c1\" and {named}"
        );
    }
}

#[test]
fn sacm23_base_007_semantic_compare_reports_is_citation_and_cited_element() {
    each_value_is_one_difference(CITATION);
}

#[test]
fn sacm23_base_008_semantic_compare_reports_is_abstract_and_abstract_form() {
    each_value_is_one_difference(ABSTRACT);
}

/// The differences of `c1` between a left and a right document in which it
/// moves from `a1` to `a2` and every value from `gid` to the name differs.
fn every_difference_of_c1(row: &str) -> Vec<Difference> {
    let doc = |what: &str, holder: &str, attributes: &str, name: &str| {
        let c1 = format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"{attributes}>\
             <name content=\"{name}\"/></argumentElement>"
        );
        let (a1, a2) = if holder == "a1" {
            (c1.as_str(), "")
        } else {
            ("", c1.as_str())
        };
        read_ok(
            row,
            what,
            &in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"a1\"><name content=\"A\"/>{a1}</argumentElement>\
                 <argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"a2\"><name content=\"A\"/>{a2}</argumentElement>"
            )),
        )
    };
    let left = doc("left", "a1", "", "C");
    let right = doc(
        "right",
        "a2",
        " gid=\"g\" isCitation=\"true\" isAbstract=\"true\" citedElement=\"a1\" abstractForm=\"a2\"",
        "D",
    );
    semantic_compare(&left, &right)
        .into_iter()
        .filter(|d| d.element_id() == Some(&ElementId::from("c1")))
        .collect()
}

#[test]
fn sacm23_base_007_is_citation_and_cited_element_differences_come_after_gid_and_before_the_name() {
    let row = CITATION.row;
    let differences = every_difference_of_c1(row);
    assert!(
        matches!(
            differences.as_slice(),
            [
                Difference::Parent { .. },
                Difference::Gid { .. },
                Difference::IsCitation { .. },
                Difference::IsAbstract { .. },
                Difference::CitedElement { .. },
                Difference::AbstractForm { .. },
                Difference::Name { .. }
            ]
        ),
        "{row}: c1's differences are not Parent, Gid, IsCitation, IsAbstract, CitedElement, \
         AbstractForm, Name: {differences:?}"
    );
    assert!(
        differences.contains(&CITATION.flag_difference("c1", false, true))
            && differences.contains(&CITATION.end_difference("c1", None, Some("a1"))),
        "{row}: the values of the differences are wrong: {differences:?}"
    );
}

#[test]
fn sacm23_base_008_is_abstract_and_abstract_form_differences_come_after_is_citation_and_cited_element()
 {
    let row = ABSTRACT.row;
    let differences = every_difference_of_c1(row);
    let position = |wanted: &Difference| differences.iter().position(|d| d == wanted);
    let is_citation = position(&CITATION.flag_difference("c1", false, true));
    let is_abstract = position(&ABSTRACT.flag_difference("c1", false, true));
    let cited = position(&CITATION.end_difference("c1", None, Some("a1")));
    let abstract_form = position(&ABSTRACT.end_difference("c1", None, Some("a2")));
    assert!(
        matches!(
            (is_citation, is_abstract, cited, abstract_form),
            (Some(a), Some(b), Some(c), Some(d)) if a < b && b < c && c < d
        ),
        "{row}: the differences are not IsCitation, IsAbstract, CitedElement, AbstractForm \
         in that order: {differences:?}"
    );
    // On a UtilityElement too: Gid, then the flag, then the reference, then
    // Content.
    let note = |what: &str, body: &str| read_ok(row, what, &in_ap(body));
    let left = note(
        "a note",
        "<note xmi:id=\"n\"><content><value content=\"a\"/></content></note>",
    );
    let right = note(
        "another note",
        "<note xmi:id=\"n\" gid=\"g\" isAbstract=\"1\" abstractForm=\"p\">\
         <content><value content=\"b\"/></content></note>",
    );
    let differences = semantic_compare(&left, &right);
    assert!(
        matches!(
            differences.as_slice(),
            [
                Difference::Gid { .. },
                Difference::IsAbstract { .. },
                Difference::AbstractForm { .. },
                Difference::Content { .. }
            ]
        ),
        "{row}: a Note's differences are not Gid, IsAbstract, AbstractForm, Content: {differences:?}"
    );
}

// --- BASE-008 item 2: abstract forms the §8.2 rules flag still read --------------------------

#[test]
fn sacm23_base_008_an_abstract_form_of_another_kind_on_an_abstract_element_or_on_itself_round_trips()
 {
    let row = ABSTRACT.row;
    let cases = [
        (
            "an abstractForm naming an ArgumentPackage, on a Claim",
            in_ap(&claim("c1", " abstractForm=\"p\"", "")),
            "c1",
            false,
            "p",
        ),
        (
            "an abstractForm on an element whose isAbstract is true",
            in_ap(&format!(
                "{}{}",
                claim("c1", " isAbstract=\"true\" abstractForm=\"c2\"", ""),
                claim("c2", " isAbstract=\"true\"", "")
            )),
            "c1",
            true,
            "c2",
        ),
        (
            "an abstractForm on itself",
            in_ap(&claim("c1", " abstractForm=\"c1\"", "")),
            "c1",
            false,
            "c1",
        ),
        (
            "an abstractForm on itself, on an abstract Note",
            in_ap("<note xmi:id=\"n\" isAbstract=\"true\"><abstractForm href=\"#n\"/></note>"),
            "n",
            true,
            "n",
        ),
    ];
    for (what, input, id, is_abstract, target) in &cases {
        let doc = read_ok(row, what, input);
        let found = element(row, &doc, id);
        assert_eq!(
            (found.is_abstract(), ABSTRACT.end_of(found)),
            (*is_abstract, Some(ElementId::from(*target))),
            "{row}: {what}: not read as written"
        );
        let text = round_trip(row, what, &doc);
        let line = line_with(row, &text, id);
        assert!(
            line.contains(&format!("abstractForm=\"{target}\"")),
            "{row}: {what}: the abstractForm is not written back: {line}"
        );
    }
}

// --- BASE-008 item 3: ImplementationConstraint on a concrete element --------------------------

#[test]
fn sacm23_base_008_an_implementation_constraint_on_a_concrete_element_reads() {
    let row = ABSTRACT.row;
    let constraint = "<implementationConstraint xmi:id=\"ic\"><content><value content=\"r\"/></content>\
         </implementationConstraint>";
    for (what, attributes) in [
        ("isAbstract absent", ""),
        ("isAbstract=\"false\"", " isAbstract=\"false\""),
        ("isAbstract=\"0\"", " isAbstract=\"0\""),
    ] {
        let what = format!("an ImplementationConstraint on a Claim with {what}");
        let doc = read_ok(row, &what, &in_ap(&claim("c1", attributes, constraint)));
        let found = element(row, &doc, "c1");
        assert!(
            !found.is_abstract(),
            "{row}: {what}: the Claim reads as abstract"
        );
        let constraints: Vec<&str> = found
            .implementation_constraints()
            .map(|c| c.id().as_str())
            .collect();
        assert_eq!(
            constraints,
            ["ic"],
            "{row}: {what}: the ImplementationConstraint is not read"
        );
        round_trip(row, &what, &doc);
    }
}
