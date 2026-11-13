//! Tests for `assertionDeclaration` in XMI (SACM23-XMI-017; formal/23-05-08
//! §11.8, §11.10; XMI 2.5.1 §9.4.1, §7.8.4, §8.2.1 rule 8; XML 1.0 §3.3.3;
//! ADR 0015, ADR 0017 amendment 2).
//!
//! `xmi::read` reads the attribute on a Claim, the one readable Assertion
//! kind: after XML attribute-value normalisation, with no trimming or case
//! folding, the value is one of the six literals, and an absent attribute is
//! `asserted`. Every other value is `ReadError::Unsupported` at the first
//! character of the attribute's name, with the value as read, quoted with
//! `{:?}`, in the reason. `xmi::write` writes `assertionDeclaration="<literal>"`
//! directly after `xmi:id` on every Assertion whose declaration is not
//! `asserted`, and nothing for `asserted`.
//!
//! Inputs are inline strings. The expected location of a refusal is marked in
//! the input with [`MARK`] (U+2038), which the test removes before reading,
//! and is counted here: lines end at LF, CRLF or a lone CR, and columns count
//! characters. Every marked input puts the construct after a CRLF and after a
//! non-ASCII character (`é`, two bytes in UTF-8) on its own line.
//!
//! Documents are built with `edit::apply`, and with the `test-support` seams
//! `Document::insert` (for the Assertion kinds no operation creates yet) and
//! `Document::set_assertion_declaration`. "The same document" is compared on
//! a projection ([`shape`]), because `Document` has no `PartialEq`.
//!
//! A read reports no diagnostic, except SACM-XMI-010 for each `xsi:type`
//! (LIB-COMPAT-001), which the tests that read one expect on the Claim. The
//! documents are written in tolerant mode, because the Assertion kinds other
//! than Claim are inserted without a name (LIB-COMPAT-007).
//!
//! Not checked here: an unknown literal reported as SACM-ENUM-001
//! (SACM23-VAL-006), the element form of the value (SACM23-XMI-014), and the
//! Assertion kinds other than Claim on read (their read rows inherit this
//! rule).

use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim};
use sacm::model::{
    AbstractClass, AssertionDeclaration, Document, ElementId, ElementKind, InsertError, LangString,
    SacmElement,
};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteOptions};

const ROW: &str = "SACM23-XMI-017";

/// Marks, in a test input, where the refusal is expected. Removed before
/// reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// What comes before the root of a marked input: a declaration ended by
/// CRLF, then on line 2 a comment holding `é` and two spaces (12 characters,
/// 13 bytes).
const LEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<!-- \u{e9} -->  ";

/// Line 1 of every output.
const DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";

/// The attributes the document element carries before any `xmi:id`, as
/// written.
const DOCUMENT_ATTRIBUTES: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The golden file of the `needsSupport` Claim.
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/needs-support-claim.golden.sacm.xmi"
);

// --- Reading ----------------------------------------------------------------

/// Reads `input` with the default options.
fn read(input: &str) -> Result<xmi::ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default())
}

/// Reads `input`, which the test expects to be read without diagnostics.
fn read_ok(what: &str, input: &str) -> Document {
    read_ok_reporting(what, input, &[])
}

/// Reads `input`, which the test expects to be read with exactly one
/// SACM-XMI-010 per id in `xsi_typed`, in order: the element whose
/// `xsi:type` is reported, a Warning in the default tolerant mode
/// (LIB-COMPAT-001).
fn read_ok_reporting(what: &str, input: &str, xsi_typed: &[&str]) -> Document {
    match read(input) {
        Ok(output) => {
            let found: Vec<(Code, Severity, Vec<&str>)> = output
                .diagnostics()
                .iter()
                .map(|found| {
                    (
                        found.code(),
                        found.severity(),
                        found.affected().iter().map(ElementId::as_str).collect(),
                    )
                })
                .collect();
            let wanted: Vec<(Code, Severity, Vec<&str>)> = xsi_typed
                .iter()
                .map(|&id| (Code::XMI_XSI_TYPE, Severity::Warning, vec![id]))
                .collect();
            assert_eq!(
                found,
                wanted,
                "{ROW}: {what}: reading reported {:?}",
                output.diagnostics()
            );
            output.into_document()
        }
        Err(error) => panic!("{ROW}: {what}: read refused {input:?}: {error:?} ({error})"),
    }
}

/// Reads [`readable_claim`]'s input with `type_attribute`: an `xsi:type` on
/// the Claim `c1` is reported (LIB-COMPAT-001), an `xmi:type` is not.
fn read_claim(what: &str, type_attribute: &str, input: &str) -> Document {
    let reported: &[&str] = if type_attribute == "xsi:type" {
        &["c1"]
    } else {
        &[]
    };
    read_ok_reporting(what, input, reported)
}

/// A 1-based location.
fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its one [`MARK`], and the location of the mark: lines
/// end at LF, CRLF or a lone CR, and columns count characters.
fn marked(template: &str) -> (String, SourceLocation) {
    let Some(offset) = template.find(MARK) else {
        panic!("{ROW}: setup: the test input {template:?} has no mark");
    };
    let text = template.replacen(MARK, "", 1);
    assert!(
        !text.contains(MARK),
        "{ROW}: setup: the test input {template:?} has more than one mark"
    );
    let (mut line, mut column, mut after_cr) = (1_u32, 1_u32, false);
    for c in template[..offset].chars() {
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
    (text, loc(line, column))
}

/// Reads `template` without its mark, and returns the refusal, failing the
/// test unless it is `Unsupported` with no code, at the mark.
fn assert_unsupported_at_mark(what: &str, template: &str) -> ReadError {
    let (input, expected) = marked(template);
    let error = match read(&input) {
        Ok(output) => panic!(
            "{ROW}: {what}: a document was returned, with {} elements, for {input:?}",
            output.document().element_count()
        ),
        Err(error) => error,
    };
    assert!(
        matches!(error, ReadError::Unsupported { .. }),
        "{ROW}: {what}: expected Unsupported, got {error:?}"
    );
    assert_eq!(
        error.code(),
        None,
        "{ROW}: {what}: an Unsupported refusal has no catalog code"
    );
    assert_eq!(
        error.location(),
        Some(expected),
        "{ROW}: {what}: refused at the wrong place: {error:?}"
    );
    error
}

/// `content` in an ArgumentPackage root `p`, after [`LEAD`].
fn in_ap(content: &str) -> String {
    format!("{LEAD}<sacm:ArgumentPackage{NS} xmi:id=\"p\">{content}</sacm:ArgumentPackage>")
}

/// `content` in an AssuranceCasePackage root `case`, after [`LEAD`].
fn in_acp(content: &str) -> String {
    format!(
        "{LEAD}<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">{content}</sacm:AssuranceCasePackage>"
    )
}

/// A Claim `c1` in an ArgumentPackage root, the Claim on line 3 after a CRLF
/// and a comment holding `é`, with `attributes` after its `xmi:id`. Unmarked,
/// the attributes start at line 3, column 65.
fn claim_with(attributes: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
         <sacm:ArgumentPackage{NS} xmi:id=\"p\">\r\n  \
         <!-- \u{e9} --> <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {attributes}/>\r\n\
         </sacm:ArgumentPackage>"
    )
}

/// One readable Claim `c1` in an ArgumentPackage root `p`, with the type
/// attribute `type_attribute` and the extra attributes `attributes`.
fn readable_claim(type_attribute: &str, attributes: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  \
         <argumentElement {type_attribute}=\"sacm:Claim\" xmi:id=\"c1\"{attributes}/>\n\
         </sacm:ArgumentPackage>"
    )
}

/// The declaration of the element `id` of `doc`.
fn declaration_of(what: &str, doc: &Document, id: &str) -> Option<AssertionDeclaration> {
    let Some(element) = doc.get(id) else {
        panic!("{ROW}: {what}: the document has no element {id:?}");
    };
    element.assertion_declaration()
}

// --- Building and writing ---------------------------------------------------

/// Applies `operation`, or fails the test as a setup error. Returns the id of
/// the element created.
fn create(doc: &mut Document, operation: impl Into<edit::Operation>) -> ElementId {
    match edit::apply(doc, operation) {
        Ok(mutation) => mutation.created_ids()[0].clone(),
        Err(error) => panic!("{ROW}: setup: edit::apply refused an operation: {error}"),
    }
}

/// Inserts one nameless element, or fails the test as a setup error.
fn insert(doc: &mut Document, parent: &str, kind: ElementKind, id: &str) {
    if let Err(error) = doc.insert(
        Some(&ElementId::from(parent)),
        kind,
        ElementId::from(id),
        None,
    ) {
        panic!("{ROW}: setup: could not insert the {kind} {id:?} under {parent:?}: {error}");
    }
}

/// Sets an Assertion's declaration, or fails the test as a setup error.
fn declare(doc: &mut Document, id: &str, declaration: AssertionDeclaration) {
    if let Err(error) = doc.set_assertion_declaration(&ElementId::from(id), declaration) {
        panic!("{ROW}: setup: could not declare {id:?} {declaration}: {error}");
    }
}

/// A name with `lang` "en".
fn name(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// The options of every write here: tolerant, because the Assertion kinds
/// other than Claim are inserted without a name and a strict write refuses a
/// validation Error (LIB-COMPAT-007). The output form is the same in both
/// modes.
fn write_options() -> WriteOptions {
    WriteOptions::default().with_mode(Mode::Tolerant)
}

/// Writes `doc` with [`write_options`], which the test expects to be written.
fn write_ok(what: &str, doc: &Document) -> String {
    match xmi::write(doc, &write_options()) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: write refused the document: {error} ({error:?})"),
    }
}

/// The six Assertion kinds, in `ElementKind::ALL` order.
fn assertion_kinds() -> Vec<ElementKind> {
    let kinds: Vec<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_a(AbstractClass::Assertion))
        .collect();
    assert_eq!(
        kinds.len(),
        6,
        "{ROW}: setup: Claim and the five asserted relationships"
    );
    kinds
}

/// The six literals, checked against the spellings of §11.8.
fn literals() -> &'static [AssertionDeclaration] {
    let spelled: Vec<&str> = AssertionDeclaration::ALL
        .iter()
        .map(|declaration| declaration.as_str())
        .collect();
    assert_eq!(
        spelled,
        [
            "asserted",
            "needsSupport",
            "assumed",
            "axiomatic",
            "defeated",
            "asCited"
        ],
        "{ROW}: setup: the six literals of §11.8"
    );
    AssertionDeclaration::ALL
}

/// Everything a document shows through its public API, per element in
/// document order.
#[derive(Debug, PartialEq)]
struct Shown {
    id: String,
    kind: ElementKind,
    parent: Option<String>,
    children: Vec<String>,
    name: Option<LangString>,
    declaration: Option<AssertionDeclaration>,
}

/// The [`Shown`] projection of a document, with its root ids.
fn shape(doc: &Document) -> (Vec<String>, Vec<Shown>) {
    let roots = doc
        .roots()
        .map(|root| root.id().as_str().to_owned())
        .collect();
    let elements = doc
        .elements()
        .map(|element| Shown {
            id: element.id().as_str().to_owned(),
            kind: element.kind(),
            parent: element
                .parent()
                .map(|parent| parent.id().as_str().to_owned()),
            children: element
                .children()
                .map(|child| child.id().as_str().to_owned())
                .collect(),
            name: element.name().cloned(),
            declaration: element.assertion_declaration(),
        })
        .collect();
    (roots, elements)
}

/// Asserts that `text` is `expected` byte for byte, showing the first line
/// that differs.
fn assert_bytes(what: &str, text: &str, expected: &str) {
    if text == expected {
        return;
    }
    let mismatch = text
        .split('\n')
        .zip(expected.split('\n'))
        .enumerate()
        .find(|(_, (got, want))| got != want);
    match mismatch {
        Some((index, (got, want))) => panic!(
            "{ROW}: {what}: line {} differs\n   got: {got:?}\nwanted: {want:?}\n--- got ---\n{text}",
            index + 1
        ),
        None => panic!(
            "{ROW}: {what}: the lines agree but the lengths differ ({} bytes, wanted {})\n{text:?}",
            text.len(),
            expected.len()
        ),
    }
}

// --- Read: the six literals --------------------------------------------------

#[test]
fn sacm23_xmi_017_each_literal_on_a_claim_reads_to_that_literal_with_xmi_type_and_xsi_type() {
    let mut failures = Vec::new();
    for &declaration in literals() {
        for type_attribute in ["xmi:type", "xsi:type"] {
            let literal = declaration.as_str();
            let input = readable_claim(
                type_attribute,
                &format!(" assertionDeclaration=\"{literal}\""),
            );
            match read(&input) {
                Ok(output) => {
                    let doc = output.document();
                    let read_as = doc.get("c1").and_then(SacmElement::assertion_declaration);
                    if read_as != Some(declaration) {
                        failures.push(format!(
                            "{literal:?} with {type_attribute} reads as {read_as:?}"
                        ));
                    }
                    // An `xsi:type` is one SACM-XMI-010 on the Claim, a
                    // Warning in the default tolerant mode (LIB-COMPAT-001).
                    let reported: Vec<(Code, Severity, Vec<&str>)> = output
                        .diagnostics()
                        .iter()
                        .map(|found| {
                            (
                                found.code(),
                                found.severity(),
                                found.affected().iter().map(ElementId::as_str).collect(),
                            )
                        })
                        .collect();
                    let expected = if type_attribute == "xsi:type" {
                        vec![(Code::XMI_XSI_TYPE, Severity::Warning, vec!["c1"])]
                    } else {
                        Vec::new()
                    };
                    if reported != expected {
                        failures.push(format!(
                            "{literal:?} with {type_attribute} reports {:?}",
                            output.diagnostics()
                        ));
                    }
                    let root = doc.get("p").and_then(SacmElement::assertion_declaration);
                    if root.is_some() {
                        failures.push(format!(
                            "{literal:?} with {type_attribute}: the package has the declaration {root:?}"
                        ));
                    }
                }
                Err(error) => failures.push(format!(
                    "{literal:?} with {type_attribute} is refused: {error}"
                )),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} literal cases do not read as their literal:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn sacm23_xmi_017_the_attribute_reads_wherever_it_stands_among_the_claim_attributes_and_with_a_name()
 {
    let cases = [
        (
            "before the type and the id",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\
                 <argumentElement assertionDeclaration=\"defeated\" xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\
                 </sacm:ArgumentPackage>"
            ),
            AssertionDeclaration::Defeated,
        ),
        (
            "between the type and the id, single-quoted",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\
                 <argumentElement xsi:type=\"sacm:Claim\" assertionDeclaration='axiomatic' xmi:id=\"c1\"/>\
                 </sacm:ArgumentPackage>"
            ),
            AssertionDeclaration::Axiomatic,
        ),
        (
            "on a named Claim at depth 3",
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\
                 <argumentPackage xmi:id=\"ap\">\
                 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" assertionDeclaration=\"asCited\">\
                 <name lang=\"en\" content=\"Cited\"/></argumentElement>\
                 </argumentPackage></sacm:AssuranceCasePackage>"
            ),
            AssertionDeclaration::AsCited,
        ),
    ];
    for (what, input, expected) in cases {
        // The single-quoted case types the Claim with `xsi:type`, which is
        // reported (LIB-COMPAT-001).
        let type_attribute = if input.contains("xsi:type") {
            "xsi:type"
        } else {
            "xmi:type"
        };
        let doc = read_claim(what, type_attribute, &input);
        assert_eq!(
            declaration_of(what, &doc, "c1"),
            Some(expected),
            "{ROW}: {what}: the Claim reads as another declaration"
        );
    }
}

#[test]
fn sacm23_xmi_017_an_absent_attribute_reads_as_asserted_and_an_explicit_asserted_reads_equal_to_it()
{
    for type_attribute in ["xmi:type", "xsi:type"] {
        let absent = read_claim(
            "no attribute",
            type_attribute,
            &readable_claim(type_attribute, ""),
        );
        assert_eq!(
            declaration_of("no attribute", &absent, "c1"),
            Some(AssertionDeclaration::Asserted),
            "{ROW}: a Claim with no attribute does not read as asserted"
        );
        let explicit = read_claim(
            "an explicit asserted",
            type_attribute,
            &readable_claim(type_attribute, " assertionDeclaration=\"asserted\""),
        );
        assert_eq!(
            shape(&explicit),
            shape(&absent),
            "{ROW}: with {type_attribute}, an explicit \"asserted\" does not read equal to no attribute"
        );
    }
}

#[test]
fn sacm23_xmi_017_a_value_whose_character_references_resolve_to_a_literal_reads_as_that_literal() {
    let cases = [
        ("&#x61;sserted", AssertionDeclaration::Asserted),
        ("needs&#x53;upport", AssertionDeclaration::NeedsSupport),
        ("&#97;ssumed", AssertionDeclaration::Assumed),
        ("axiomati&#x63;", AssertionDeclaration::Axiomatic),
        ("&#x64;&#x65;feated", AssertionDeclaration::Defeated),
        ("as&#67;ited", AssertionDeclaration::AsCited),
    ];
    for (written, expected) in cases {
        let what = format!("assertionDeclaration=\"{written}\"");
        let doc = read_ok(
            &what,
            &readable_claim("xmi:type", &format!(" assertionDeclaration=\"{written}\"")),
        );
        assert_eq!(
            declaration_of(&what, &doc, "c1"),
            Some(expected),
            "{ROW}: {what} does not read as {expected}"
        );
    }
}

// --- Refuse: any other value -------------------------------------------------

/// Every value that must be refused, as written in the attribute and as read
/// after XML 1.0 §3.3.3 normalisation (a literal TAB, LF, CR or CRLF is one
/// space; a character reference is kept).
const BAD_VALUES: [(&str, &str); 25] = [
    ("Asserted", "Asserted"),
    ("NeedsSupport", "NeedsSupport"),
    ("needssupport", "needssupport"),
    ("ASCITED", "ASCITED"),
    ("", ""),
    (" asserted", " asserted"),
    ("asserted ", "asserted "),
    ("needs Support", "needs Support"),
    ("\tasserted", " asserted"),
    ("needs\tSupport", "needs Support"),
    ("assumed\t", "assumed "),
    ("\naxiomatic", " axiomatic"),
    ("defe\nated", "defe ated"),
    ("asCited\n", "asCited "),
    ("\rasserted", " asserted"),
    ("needs\rSupport", "needs Support"),
    ("assumed\r", "assumed "),
    ("asserted\r\n", "asserted "),
    ("&#9;asserted", "\tasserted"),
    ("asserted&#xA;", "asserted\n"),
    ("&#xD;asserted", "\rasserted"),
    ("&#x20;asserted", " asserted"),
    ("justification", "justification"),
    ("byRule", "byRule"),
    ("undeveloped", "undeveloped"),
];

#[test]
fn sacm23_xmi_017_every_other_value_is_unsupported_at_the_attribute_name_quoting_the_value_as_read()
{
    // Counted by hand, as a check on `marked`: line 3 holds two spaces,
    // `<!-- é -->` (10 characters, 11 bytes), a space, `<argumentElement`
    // (16), ` xmi:type="sacm:Claim"` (22), ` xmi:id="c1"` (12) and a space: 64
    // characters, so the attribute's name is at column 65.
    let (_, counted) = marked(&claim_with(&format!("{MARK}assertionDeclaration=\"x\"")));
    assert_eq!(
        counted,
        loc(3, 65),
        "{ROW}: setup: the mark is not counted as by hand"
    );
    let mut failures = Vec::new();
    for (written, as_read) in BAD_VALUES {
        let template = claim_with(&format!("{MARK}assertionDeclaration=\"{written}\""));
        let (input, expected) = marked(&template);
        let error = match read(&input) {
            Ok(output) => {
                failures.push(format!(
                    "{written:?} is read, as {:?}",
                    output
                        .document()
                        .get("c1")
                        .and_then(SacmElement::assertion_declaration)
                ));
                continue;
            }
            Err(error) => error,
        };
        let ReadError::Unsupported { reason, .. } = &error else {
            failures.push(format!(
                "{written:?} is refused as {error:?}, not Unsupported"
            ));
            continue;
        };
        if error.code().is_some() {
            failures.push(format!("{written:?}: the refusal has a code"));
        }
        if error.location() != Some(expected) {
            failures.push(format!(
                "{written:?} is refused at {:?}, not at the attribute's name {expected}",
                error.location()
            ));
        }
        let quoted = format!("{as_read:?}");
        if !reason.contains(&quoted) {
            failures.push(format!(
                "{written:?}: the reason {reason:?} does not hold the value as read, {quoted}"
            ));
        }
        let shown = error.to_string();
        if !shown.contains(&quoted) || shown.contains('\n') {
            failures.push(format!(
                "{written:?}: the message {shown:?} is not one line quoting {quoted}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} problems across the {} bad values:\n{}",
        failures.len(),
        BAD_VALUES.len(),
        failures.join("\n")
    );
}

#[test]
fn sacm23_xmi_017_no_bad_value_is_ever_read_as_asserted_or_dropped() {
    // Each bad value, beside a valid sibling Claim declared `assumed`: no
    // document is returned at all, so neither Claim can be read as `asserted`
    // or lose its value.
    for (written, _) in BAD_VALUES {
        let input = in_ap(&format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"ok\" assertionDeclaration=\"assumed\"/>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {MARK}assertionDeclaration=\"{written}\"/>"
        ));
        assert_unsupported_at_mark(&format!("{written:?} after a valid sibling"), &input);
    }
}

// --- Refuse: elsewhere ------------------------------------------------------

#[test]
fn sacm23_xmi_017_the_attribute_on_a_package_root_or_child_is_unsupported_at_its_name() {
    for value in ["asserted", "needsSupport", "assumed", "Asserted"] {
        let attribute = format!("{MARK}assertionDeclaration=\"{value}\"");
        let cases = [
            (
                "an ArgumentPackage root",
                format!("{LEAD}<sacm:ArgumentPackage{NS} xmi:id=\"p\" {attribute}/>"),
            ),
            (
                "an AssuranceCasePackage root",
                format!("{LEAD}<sacm:AssuranceCasePackage{NS} xmi:id=\"case\" {attribute}/>"),
            ),
            (
                "an ArgumentPackage child in argumentPackage",
                in_acp(&format!("<argumentPackage xmi:id=\"x\" {attribute}/>")),
            ),
            (
                "an ArgumentPackage child in argumentElement",
                in_ap(&format!(
                    "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\" {attribute}/>"
                )),
            ),
            (
                "an AssuranceCasePackage child",
                in_acp(&format!("<assuranceCasePackage xmi:id=\"x\" {attribute}/>")),
            ),
            (
                "an ArgumentPackage child beside a declared Claim",
                in_ap(&format!(
                    "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" assertionDeclaration=\"assumed\"/>\
                     <argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\" {attribute}/>"
                )),
            ),
        ];
        for (what, template) in cases {
            assert_unsupported_at_mark(&format!("{value:?} on {what}"), &template);
        }
    }
}

#[test]
fn sacm23_xmi_017_a_namespace_qualified_attribute_on_a_claim_is_unsupported_at_its_name() {
    let claim = |attributes: &str| {
        in_ap(&format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {attributes}/>"
        ))
    };
    let cases = [
        (
            "sacm:assertionDeclaration=\"needsSupport\"",
            claim(&format!("{MARK}sacm:assertionDeclaration=\"needsSupport\"")),
        ),
        (
            "sacm:assertionDeclaration=\"asserted\"",
            claim(&format!("{MARK}sacm:assertionDeclaration=\"asserted\"")),
        ),
        (
            "xmi:assertionDeclaration=\"assumed\"",
            claim(&format!("{MARK}xmi:assertionDeclaration=\"assumed\"")),
        ),
        (
            "a foreign-namespace assertionDeclaration",
            claim(&format!(
                "xmlns:o=\"urn:o\" {MARK}o:assertionDeclaration=\"assumed\""
            )),
        ),
        // The unqualified attribute is read; the qualified one after it is
        // still refused, at its own name.
        (
            "sacm:assertionDeclaration after a readable assertionDeclaration",
            claim(&format!(
                "assertionDeclaration=\"assumed\" {MARK}sacm:assertionDeclaration=\"assumed\""
            )),
        ),
    ];
    for (what, template) in cases {
        assert_unsupported_at_mark(what, &template);
    }
}

#[test]
fn sacm23_xmi_017_the_element_form_in_a_claim_is_unsupported_at_its_start_tag() {
    let element = "<assertionDeclaration>needsSupport</assertionDeclaration>";
    let claim = |attributes: &str, content: &str| {
        in_ap(&format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"{attributes}>{content}</argumentElement>"
        ))
    };
    let cases = [
        (
            "the element form alone",
            claim("", &format!("{MARK}{element}")),
        ),
        (
            "the element form with the attribute needsSupport",
            claim(
                " assertionDeclaration=\"needsSupport\"",
                &format!("{MARK}{element}"),
            ),
        ),
        (
            "the element form with the attribute asserted",
            claim(
                " assertionDeclaration=\"asserted\"",
                &format!("{MARK}{element}"),
            ),
        ),
        (
            "the element form after a name, with the attribute",
            claim(
                " assertionDeclaration=\"assumed\"",
                &format!("<name lang=\"en\" content=\"N\"/>\r\n  {MARK}{element}"),
            ),
        ),
        (
            "the element form, empty, with the attribute",
            claim(
                " assertionDeclaration=\"defeated\"",
                &format!("{MARK}<assertionDeclaration/>"),
            ),
        ),
    ];
    for (what, template) in cases {
        assert_unsupported_at_mark(what, &template);
    }
}

// --- Write --------------------------------------------------------------------

/// An ArgumentPackage root `arguments`, named "Arguments", holding one
/// element `x` of the Assertion kind `kind`, declared `declaration`. A Claim
/// is created with `edit::apply` and named "X"; the other kinds, which no
/// operation creates yet, are inserted with no name.
fn one_assertion(kind: ElementKind, declaration: AssertionDeclaration) -> Document {
    let mut doc = Document::new();
    let arguments = create(
        &mut doc,
        CreateArgumentPackage::root(name("Arguments")).with_id("arguments"),
    );
    if kind == ElementKind::Claim {
        create(
            &mut doc,
            CreateClaim::new(&arguments, name("X")).with_id("x"),
        );
    } else {
        insert(&mut doc, "arguments", kind, "x");
    }
    declare(&mut doc, "x", declaration);
    doc
}

#[test]
fn sacm23_xmi_017_write_puts_each_non_default_literal_directly_after_the_id_on_each_assertion_kind()
{
    let mut failures = Vec::new();
    for kind in assertion_kinds() {
        for &declaration in literals() {
            let doc = one_assertion(kind, declaration);
            let attribute = if declaration == AssertionDeclaration::Asserted {
                String::new()
            } else {
                format!(" assertionDeclaration=\"{}\"", declaration.as_str())
            };
            let start =
                format!("  <argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"{attribute}");
            let element = if kind == ElementKind::Claim {
                format!("{start}>\n    <name lang=\"en\" content=\"X\" />\n  </argumentElement>\n")
            } else {
                format!("{start} />\n")
            };
            let expected = format!(
                "{DECLARATION}\n<sacm:ArgumentPackage{DOCUMENT_ATTRIBUTES} xmi:id=\"arguments\">\n  \
                 <name lang=\"en\" content=\"Arguments\" />\n{element}</sacm:ArgumentPackage>\n"
            );
            match xmi::write(&doc, &write_options()) {
                Ok(text) if text == expected => {}
                Ok(text) => failures.push(format!(
                    "a {kind} declared {declaration} is written as\n{text}wanted\n{expected}"
                )),
                Err(error) => failures.push(format!(
                    "a {kind} declared {declaration} is refused: {error}"
                )),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} of 36 (kind, literal) cases are not written as pinned:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A document with every `ElementKind`: each of the twelve package kinds as
/// a root, under each one child of every kind its roles admit, and under each
/// `ModelElement` child a Note. Every Assertion is declared, the literals
/// taken in turn, so each literal is on several Assertions.
fn every_kind_declared() -> Document {
    let mut doc = Document::new();
    let mut turn = 0_usize;
    for &root in ElementKind::ALL.iter().filter(|kind| kind.is_package()) {
        let root_id = format!("{root}.root");
        if let Err(error) = doc.insert(None, root, ElementId::from(root_id.as_str()), None) {
            panic!("{ROW}: setup: could not insert the root {root_id:?}: {error}");
        }
        for &kind in ElementKind::ALL {
            let child = format!("{root_id}.{kind}");
            let parent = ElementId::from(root_id.as_str());
            match doc.insert(Some(&parent), kind, ElementId::from(child.as_str()), None) {
                Ok(()) => {}
                Err(InsertError::ContainmentNotAllowed { .. }) => continue,
                Err(error) => panic!("{ROW}: setup: could not insert {child:?}: {error}"),
            }
            if kind.is_a(AbstractClass::Assertion) {
                declare(&mut doc, &child, literals()[turn % literals().len()]);
                turn += 1;
            }
            if kind.is_a(AbstractClass::ModelElement) {
                insert(
                    &mut doc,
                    &child,
                    ElementKind::Note,
                    &format!("{child}.Note"),
                );
            }
        }
    }
    assert!(
        turn >= 2 * literals().len(),
        "{ROW}: setup: only {turn} Assertions in the every-kind document"
    );
    doc
}

/// The `xmi:id` value of a written start tag, if it has one. The ids of
/// [`every_kind_declared`] need no escape.
fn written_id(line: &str) -> Option<&str> {
    let (_, rest) = line.split_once(" xmi:id=\"")?;
    rest.split_once('"').map(|(id, _)| id)
}

#[test]
fn sacm23_xmi_017_no_non_assertion_element_carries_the_attribute() {
    let doc = every_kind_declared();
    let text = write_ok("the every-kind document", &doc);
    let mut written = 0;
    let mut failures = Vec::new();
    for line in text.lines() {
        let Some(id) = written_id(line) else {
            if line.contains("assertionDeclaration") {
                failures.push(format!("a line with no xmi:id has the attribute: {line:?}"));
            }
            continue;
        };
        let Some(element) = doc.get(id) else {
            failures.push(format!(
                "the output has an element the document lacks: {line:?}"
            ));
            continue;
        };
        let expected = element
            .assertion_declaration()
            .filter(|&declaration| declaration != AssertionDeclaration::Asserted)
            .map(|declaration| {
                format!(
                    " xmi:id=\"{id}\" assertionDeclaration=\"{}\"",
                    declaration.as_str()
                )
            });
        match expected {
            Some(expected) => {
                written += 1;
                if !line.contains(&expected) || line.matches("assertionDeclaration").count() != 1 {
                    failures.push(format!(
                        "the {} {id:?} is not written with {expected:?}: {line:?}",
                        element.kind()
                    ));
                }
            }
            None if line.contains("assertionDeclaration") => failures.push(format!(
                "the {} {id:?} carries the attribute: {line:?}",
                element.kind()
            )),
            None => {}
        }
    }
    let declared = doc
        .elements()
        .filter(|element| {
            element
                .assertion_declaration()
                .is_some_and(|declaration| declaration != AssertionDeclaration::Asserted)
        })
        .count();
    assert!(
        failures.is_empty(),
        "{ROW}: {} lines are wrong:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert_eq!(
        (written, text.matches("assertionDeclaration=").count()),
        (declared, declared),
        "{ROW}: the output does not hold one attribute per non-default Assertion"
    );
}

#[test]
fn sacm23_xmi_017_a_needs_support_claim_matches_the_golden_file() {
    let golden = std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {GOLDEN}: {error}"));
    let mut doc = Document::new();
    let arguments = create(
        &mut doc,
        CreateArgumentPackage::root(name("Arguments")).with_id("arguments"),
    );
    create(
        &mut doc,
        CreateClaim::new(&arguments, name("The system is safe")).with_id("c1"),
    );
    declare(&mut doc, "c1", AssertionDeclaration::NeedsSupport);
    let text = write_ok("the needsSupport Claim", &doc);
    assert_bytes("needs-support-claim.golden.sacm.xmi", &text, &golden);

    // The golden file reads back as the document, and writes to itself.
    let back = read_ok("the golden file", &golden);
    assert_eq!(
        shape(&back),
        shape(&doc),
        "{ROW}: the golden file does not read back as the document"
    );
    assert_bytes(
        "the golden file read and written",
        &write_ok("the golden file read back", &back),
        &golden,
    );
}

// --- Round trip ---------------------------------------------------------------

/// A document built with `edit::apply` and the declaration seam: an
/// AssuranceCasePackage `case` holding an ArgumentPackage `ap` (a Claim
/// declared `needsSupport`, then an ArgumentPackage `inner` holding six
/// sibling Claims, one per literal, and an ArgumentPackage `deep` holding a
/// Claim declared `defeated`), and a second root ArgumentPackage `second`
/// whose sibling Claims are declared `asCited`, `asserted` and `axiomatic`,
/// with a generated id among them.
fn declared_document() -> Document {
    use AssertionDeclaration as D;
    let mut doc = Document::new();
    let case = create(
        &mut doc,
        CreateAssuranceCasePackage::root(name("Case")).with_id("case"),
    );
    let ap = create(
        &mut doc,
        CreateArgumentPackage::new(&case, LangString::default()).with_id("ap"),
    );
    let top = create(
        &mut doc,
        CreateClaim::new(&ap, name("Top")).with_id("c_top"),
    );
    declare(&mut doc, top.as_str(), D::NeedsSupport);
    let inner = create(
        &mut doc,
        CreateArgumentPackage::new(&ap, LangString::new("")).with_id("inner"),
    );
    for &declaration in literals() {
        let id = format!("c_{}", declaration.as_str());
        let content = format!("S\u{e4}kerhet {}", declaration.as_str());
        let claim = create(
            &mut doc,
            CreateClaim::new(&inner, LangString::new(content).with_lang("sv")).with_id(id),
        );
        declare(&mut doc, claim.as_str(), declaration);
    }
    let deep = create(
        &mut doc,
        CreateArgumentPackage::new(&inner, name("Deep")).with_id("deep"),
    );
    let deepest = create(&mut doc, CreateClaim::new(&deep, name("Deepest")));
    declare(&mut doc, deepest.as_str(), D::Defeated);
    let second = create(
        &mut doc,
        CreateArgumentPackage::root(name("Second")).with_id("second"),
    );
    for (index, declaration) in [D::AsCited, D::Asserted, D::Axiomatic]
        .into_iter()
        .enumerate()
    {
        let claim = if index == 1 {
            create(&mut doc, CreateClaim::new(&second, name("Generated")))
        } else {
            create(
                &mut doc,
                CreateClaim::new(&second, name("Sibling")).with_id(format!("s{index}")),
            )
        };
        declare(&mut doc, claim.as_str(), declaration);
    }
    doc
}

/// The deepest element's depth, a root being at depth 1.
fn depth(doc: &Document) -> usize {
    doc.elements()
        .map(|element| std::iter::successors(Some(element), |e| e.parent()).count())
        .max()
        .unwrap_or_default()
}

#[test]
fn sacm23_xmi_017_claims_with_each_literal_round_trip_at_depth_three_and_write_again_to_the_same_bytes()
 {
    let doc = declared_document();
    assert!(
        depth(&doc) >= 3,
        "{ROW}: setup: the document is too shallow"
    );
    let declared: Vec<AssertionDeclaration> = doc
        .elements()
        .filter_map(SacmElement::assertion_declaration)
        .collect();
    for &literal in literals() {
        assert!(
            declared.contains(&literal),
            "{ROW}: setup: no Claim is declared {literal}"
        );
    }
    let text = write_ok("the declared document", &doc);
    let back = read_ok("the written declared document", &text);
    assert_eq!(
        shape(&back),
        shape(&doc),
        "{ROW}: the declared document does not read back the same\n{text}"
    );
    assert_bytes(
        "the declared document read back and written again",
        &write_ok("the declared document read back", &back),
        &text,
    );
}

#[test]
fn sacm23_xmi_017_a_file_with_an_explicit_asserted_is_written_with_no_attribute() {
    let explicit = read_claim(
        "an explicit asserted",
        "xsi:type",
        &readable_claim("xsi:type", " assertionDeclaration=\"asserted\""),
    );
    let absent = read_claim("no attribute", "xsi:type", &readable_claim("xsi:type", ""));
    let text = write_ok("the explicit asserted read back", &explicit);
    assert!(
        !text.contains("assertionDeclaration"),
        "{ROW}: an explicit \"asserted\" is written back\n{text}"
    );
    assert_bytes(
        "the explicit asserted, written",
        &text,
        &write_ok("no attribute, read back", &absent),
    );
}
