//! Tests for `xsi:nil` on a value element (SACM23-XMI-026; XMI 2.5.1 §9.4.1,
//! rule 2b, Table 7.1 `includeNils`, §7.11.2, production 4d, §7.15; XML
//! Schema 1.0 Part 1 §3.3.4 cvc-elt 3.1 and 3.2.1, §3.2.7; spec defect D16;
//! ADR 0022 amendment 1).
//!
//! The value elements are those of a DataType-typed property: `gid`,
//! `isCitation`, `isAbstract`, `isCounter`, `assertionDeclaration`, and `lang`
//! and `content` in a `name` or a `value`. On one, `xsi:nil` is read in both
//! modes as an `xsd:boolean`: `true` or `1` (white space around it collapsed)
//! reads exactly as if the property were not written, `false` or `0` reads the
//! element's content as usual, and any other value is `InvalidLiteral`
//! (SACM-ENUM-001). A nil element with a character or element child is
//! refused, and `xsi:nil` on any other element stays refused. A nil occurrence
//! counts for `RepeatedValue`. Nothing is reported, and `xmi::write` writes the
//! property as omitted.
//!
//! "Reads as omitted" is `==` to the same file without the element. The
//! refusal of a nil element with content is checked by its location, in both
//! modes; its variant is the implementer's (the lead confirms).
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, SourceLocation};
use sacm::model::{AssertionDeclaration, Document, LangString, MultiLangString, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-XMI-026";

/// Both modes, strict first.
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

// --- Inputs ------------------------------------------------------------------

/// An ArgumentPackage root `p` named "P", holding `body` on line 3.
fn ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  {body}\n\
         </sacm:ArgumentPackage>"
    )
}

/// A Claim `c1` named "C", holding `body` after its name, in [`ap`].
fn claim(body: &str) -> String {
    ap(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/>{body}\
         </argumentElement>"
    ))
}

/// A Claim `c1` holding `name` as its name, in [`ap`].
fn named(name: &str) -> String {
    ap(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">{name}</argumentElement>"
    ))
}

/// A Description `d` of `c1` whose `content` holds `values`.
fn content_values(values: &str) -> String {
    claim(&format!(
        "<description xmi:id=\"d\"><content>{values}</content></description>"
    ))
}

/// An `AssertedInference` `i1` from `c1` to `c1` holding `body`, beside the
/// Claim `c1` named "C".
fn inference(body: &str) -> String {
    ap(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/></argumentElement>\n  \
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" source=\"c1\" target=\"c1\">\
         <name content=\"I\"/>{body}</argumentElement>"
    ))
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order.
fn marked(template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column, mut after_cr) = (1_u32, 1_u32, false);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(line, column));
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

/// A `LangString` with exactly the given `lang` and `content`.
fn ls(lang: Option<&str>, content: Option<&str>) -> LangString {
    let mut value = LangString::default();
    value.set_lang(lang.map(str::to_owned));
    value.set_content(content.map(str::to_owned));
    value
}

// --- Reading and writing -------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to `==` documents, and returns the strict one.
fn read_ok(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => {
                assert_eq!(
                    output.diagnostics(),
                    [],
                    "{ROW}: {what}: a {mode:?} read of {input:?} reported diagnostics"
                );
                documents.push(output.into_document());
            }
            Err(error) => {
                panic!("{ROW}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
            }
        }
    }
    assert_eq!(
        documents[0], documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// Reads `input` in both modes, which the test expects to be refused the same
/// in both, and returns the refusal.
fn refused(what: &str, input: &str) -> ReadError {
    let mut errors = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => panic!(
                "{ROW}: {what}: a {mode:?} read returned a document of {} elements for {input:?}",
                output.document().element_count()
            ),
            Err(error) => errors.push(error),
        }
    }
    assert_eq!(
        errors[0], errors[1],
        "{ROW}: {what}: the modes refuse differently"
    );
    errors.swap_remove(0)
}

/// Reads `template` without its one mark in both modes and returns the
/// refusal, which the test expects at the mark.
fn refused_at_mark(what: &str, template: &str) -> ReadError {
    let (input, marks) = marked(template);
    assert_eq!(marks.len(), 1, "{ROW}: setup: {what} has not one mark");
    let error = refused(what, &input);
    assert_eq!(
        error.location(),
        Some(marks[0]),
        "{ROW}: {what}: not refused at the construct: {error:?} ({error})"
    );
    error
}

fn element<'a>(what: &str, doc: &'a Document, id: &str) -> SacmElement<'a> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: {what}: the document has no element {id:?}");
    };
    found
}

/// Writes `doc` in `mode`, which the test expects to succeed.
fn write_in(what: &str, mode: Mode, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused: {error:?} ({error})"),
    }
}

// --- The nil inputs, each with its omitted and its empty twin -------------------------

/// A nil value element: what, the input, the same file without the element,
/// and, where the element can be empty, the same file with it empty.
struct Nil {
    what: &'static str,
    input: String,
    omitted: String,
    empty: Option<String>,
}

#[expect(
    clippy::too_many_lines,
    reason = "one flat table is easier to review than several"
)]
fn nils() -> Vec<Nil> {
    let nil = |what, input: String, omitted: String, empty: Option<String>| Nil {
        what,
        input,
        omitted,
        empty,
    };
    vec![
        nil(
            "<gid xsi:nil=\"true\"/>",
            claim("<gid xsi:nil=\"true\"/>"),
            claim(""),
            Some(claim("<gid/>")),
        ),
        nil(
            "<gid xsi:nil=\"1\"></gid>",
            claim("<gid xsi:nil=\"1\"></gid>"),
            claim(""),
            Some(claim("<gid></gid>")),
        ),
        nil(
            "a nil gid on a root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
                 <gid xsi:nil=\"true\"/></sacm:ArgumentPackage>"
            ),
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/></sacm:ArgumentPackage>"
            ),
            Some(format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/><gid/></sacm:ArgumentPackage>"
            )),
        ),
        nil(
            "a nil gid on a Note",
            claim("<note xmi:id=\"n\"><gid xsi:nil=\"true\"/></note>"),
            claim("<note xmi:id=\"n\"/>"),
            Some(claim("<note xmi:id=\"n\"><gid/></note>")),
        ),
        nil(
            "a nil gid under another prefix",
            ap(
                "<argumentElement xmlns:i=\"http://www.w3.org/2001/XMLSchema-instance\" \
                 xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/><gid i:nil=\"true\"/>\
                 </argumentElement>",
            ),
            claim(""),
            None,
        ),
        nil(
            "a nil gid holding a comment and a processing instruction",
            claim("<gid xsi:nil=\"true\"><!--c--><?pi x?></gid>"),
            claim(""),
            None,
        ),
        nil(
            "<name><lang xsi:nil=\"1\"/></name>",
            named("<name><lang xsi:nil=\"1\"/></name>"),
            named("<name/>"),
            Some(named("<name><lang/></name>")),
        ),
        nil(
            "a nil lang beside a content",
            named("<name content=\"C\"><lang xsi:nil=\"true\"/></name>"),
            named("<name content=\"C\"/>"),
            Some(named("<name content=\"C\"><lang/></name>")),
        ),
        nil(
            "a nil content in a name",
            named("<name lang=\"en\"><content xsi:nil=\"true\"/></name>"),
            named("<name lang=\"en\"/>"),
            Some(named("<name lang=\"en\"><content/></name>")),
        ),
        nil(
            "<value><content xsi:nil=\" true \"/></value>",
            content_values("<value><content xsi:nil=\" true \"/></value>"),
            content_values("<value/>"),
            Some(content_values("<value><content/></value>")),
        ),
        nil(
            "a nil lang in a value",
            content_values("<value content=\"x\"><lang xsi:nil=\"true\"/></value>"),
            content_values("<value content=\"x\"/>"),
            Some(content_values("<value content=\"x\"><lang/></value>")),
        ),
        nil(
            "a nil lang in a key value",
            claim(
                "<taggedValue xmi:id=\"t\"><key><value content=\"k\">\
                 <lang xsi:nil=\"true\"/></value></key></taggedValue>",
            ),
            claim("<taggedValue xmi:id=\"t\"><key><value content=\"k\"/></key></taggedValue>"),
            Some(claim(
                "<taggedValue xmi:id=\"t\"><key><value content=\"k\"><lang/></value></key></taggedValue>",
            )),
        ),
        nil(
            "<isCitation xsi:nil=\"true\"/>",
            claim("<isCitation xsi:nil=\"true\"/>"),
            claim(""),
            None,
        ),
        nil(
            "<isAbstract xsi:nil=\"true\"/>",
            claim("<isAbstract xsi:nil=\"true\"/>"),
            claim(""),
            None,
        ),
        nil(
            "<isCounter xsi:nil=\"true\"/>",
            inference("<isCounter xsi:nil=\"true\"/>"),
            inference(""),
            None,
        ),
        nil(
            "<assertionDeclaration xsi:nil=\"true\"/>",
            claim("<assertionDeclaration xsi:nil=\"true\"/>"),
            claim(""),
            None,
        ),
        nil(
            "a nil assertionDeclaration on an inference",
            inference("<assertionDeclaration xsi:nil=\"1\"/>"),
            inference(""),
            None,
        ),
    ]
}

// --- Acceptance items 1 and 3: nil reads as omitted --------------------------------------

#[test]
fn sacm23_xmi_026_a_nil_value_element_reads_as_if_the_property_were_not_written() {
    for case in nils() {
        let what = case.what;
        let doc = read_ok(what, &case.input);
        let omitted = read_ok(&format!("{what}: omitted"), &case.omitted);
        assert_eq!(
            semantic_compare(&doc, &omitted),
            [],
            "{ROW}: {what}: does not read as the file without the element"
        );
        assert_eq!(
            doc, omitted,
            "{ROW}: {what}: is not == the file without the element"
        );
        if let Some(empty) = &case.empty {
            let empty = read_ok(&format!("{what}: empty"), empty);
            assert_ne!(
                doc, empty,
                "{ROW}: {what}: reads as the empty element, not as absent"
            );
        }
    }
}

#[test]
fn sacm23_xmi_026_a_nil_value_reads_as_absent_a_nil_flag_as_false_and_a_nil_declaration_as_asserted()
 {
    let doc = read_ok("nil gid", &claim("<gid xsi:nil=\"true\"/>"));
    assert_eq!(
        element("nil gid", &doc, "c1").gid(),
        None,
        "{ROW}: a nil gid is not absent"
    );
    let doc = read_ok("nil lang", &named("<name><lang xsi:nil=\"1\"/></name>"));
    assert_eq!(
        element("nil lang", &doc, "c1").name(),
        Some(&ls(None, None)),
        "{ROW}: a nil lang is not absent"
    );
    let doc = read_ok(
        "nil value content",
        &content_values("<value><content xsi:nil=\" true \"/></value>"),
    );
    assert_eq!(
        element("nil value content", &doc, "d").content(),
        Some(&[ls(None, None)].into_iter().collect::<MultiLangString>()),
        "{ROW}: a nil value content is not absent"
    );
    let doc = read_ok(
        "nil flags",
        &claim("<isCitation xsi:nil=\"true\"/><isAbstract xsi:nil=\"1\"/>"),
    );
    let c1 = element("nil flags", &doc, "c1");
    assert_eq!(
        (c1.is_citation(), c1.is_abstract()),
        (false, false),
        "{ROW}: a nil isCitation or isAbstract is not false"
    );
    let doc = read_ok("nil isCounter", &inference("<isCounter xsi:nil=\"true\"/>"));
    assert!(
        !element("nil isCounter", &doc, "i1").is_counter(),
        "{ROW}: a nil isCounter is not false"
    );
    let doc = read_ok(
        "nil declaration",
        &claim("<assertionDeclaration xsi:nil=\"true\"/>"),
    );
    assert_eq!(
        element("nil declaration", &doc, "c1").assertion_declaration(),
        Some(AssertionDeclaration::Asserted),
        "{ROW}: a nil assertionDeclaration is not asserted"
    );
}

// --- Acceptance item 2: nil false -------------------------------------------------------

#[test]
fn sacm23_xmi_026_nil_false_reads_the_element_content_as_usual() {
    let doc = read_ok("nil false gid x", &claim("<gid xsi:nil=\"false\">x</gid>"));
    assert_eq!(
        element("nil false gid x", &doc, "c1").gid(),
        Some("x"),
        "{ROW}: <gid xsi:nil=\"false\">x</gid> is not \"x\""
    );
    let doc = read_ok("nil false empty gid", &claim("<gid xsi:nil=\"false\"/>"));
    assert_eq!(
        element("nil false empty gid", &doc, "c1").gid(),
        Some(""),
        "{ROW}: <gid xsi:nil=\"false\"/> is not \"\""
    );
    let doc = read_ok(
        "nil 0 declaration",
        &claim("<assertionDeclaration xsi:nil=\"0\">assumed</assertionDeclaration>"),
    );
    assert_eq!(
        element("nil 0 declaration", &doc, "c1").assertion_declaration(),
        Some(AssertionDeclaration::Assumed),
        "{ROW}: xsi:nil=\"0\" on assumed is not assumed"
    );
    for (what, input, plain) in [
        (
            "nil false on a flag",
            claim("<isCitation xsi:nil=\" false \">true</isCitation>"),
            claim("<isCitation>true</isCitation>"),
        ),
        (
            "nil 0 on a name's content",
            named("<name><content xsi:nil=\"0\"> X </content></name>"),
            named("<name><content> X </content></name>"),
        ),
        (
            "nil false on a value's lang",
            content_values("<value content=\"x\"><lang xsi:nil=\"false\">en</lang></value>"),
            content_values("<value lang=\"en\" content=\"x\"/>"),
        ),
        (
            "nil false on isCounter",
            inference("<isCounter xsi:nil=\"false\">1</isCounter>"),
            inference("<isCounter>1</isCounter>"),
        ),
    ] {
        let doc = read_ok(what, &input);
        let reference = read_ok(&format!("{what}: plain"), &plain);
        assert_eq!(
            doc, reference,
            "{ROW}: {what}: does not read as the element without xsi:nil"
        );
    }
}

// --- Acceptance item 4: a nil value that is not a Boolean ------------------------------------

#[test]
fn sacm23_xmi_026_an_xsi_nil_that_is_not_a_boolean_literal_is_invalid_literal_at_the_attribute() {
    let mut cases = Vec::new();
    for value in ["TRUE", "yes", ""] {
        cases.push((
            format!("xsi:nil={value:?} on gid"),
            claim(&format!("<gid {MARK}xsi:nil=\"{value}\"/>")),
            "c1",
            value,
        ));
        cases.push((
            format!("xsi:nil={value:?} on a name's lang"),
            named(&format!("<name><lang {MARK}xsi:nil=\"{value}\"/></name>")),
            "c1",
            value,
        ));
        cases.push((
            format!("xsi:nil={value:?} on isCitation"),
            claim(&format!("<isCitation {MARK}xsi:nil=\"{value}\"/>")),
            "c1",
            value,
        ));
        cases.push((
            format!("xsi:nil={value:?} on a value's content"),
            content_values(&format!(
                "<value><content {MARK}xsi:nil=\"{value}\"/></value>"
            )),
            "d",
            value,
        ));
    }
    for (what, template, owner, value) in &cases {
        let error = refused_at_mark(what, template);
        let ReadError::InvalidLiteral {
            element,
            property,
            value: as_read,
            ..
        } = &error
        else {
            panic!("{ROW}: {what}: not InvalidLiteral: {error:?} ({error})");
        };
        assert_eq!(
            (element.as_str(), property.as_str(), as_read.as_str()),
            (*owner, "xsi:nil", *value),
            "{ROW}: {what}: the refusal does not name the owner, xsi:nil and the value"
        );
        assert_eq!(
            error.code(),
            Some(Code::ENUM_INVALID_LITERAL),
            "{ROW}: {what}: not SACM-ENUM-001"
        );
    }
}

// --- Acceptance item 5: a nil element with content --------------------------------------------

#[test]
fn sacm23_xmi_026_a_nil_element_with_a_character_or_element_child_is_refused_at_it() {
    for (what, template) in [
        (
            "text",
            claim(&format!("<gid xsi:nil=\"true\">{MARK}x</gid>")),
        ),
        (
            "white space",
            claim(&format!("<gid xsi:nil=\"true\">{MARK} </gid>")),
        ),
        (
            "a reference",
            claim(&format!("<gid xsi:nil=\"1\">{MARK}&#x20;</gid>")),
        ),
        (
            "a CDATA section",
            claim(&format!("<gid xsi:nil=\"true\">{MARK}<![CDATA[x]]></gid>")),
        ),
        (
            "text after a comment",
            claim(&format!("<gid xsi:nil=\"true\"><!--c-->{MARK}x</gid>")),
        ),
        (
            "an element",
            claim(&format!("<gid xsi:nil=\"true\">{MARK}<b/></gid>")),
        ),
        (
            "a literal in a nil declaration",
            claim(&format!(
                "<assertionDeclaration xsi:nil=\"true\">{MARK}assumed</assertionDeclaration>"
            )),
        ),
        (
            "a literal in a nil flag",
            claim(&format!(
                "<isAbstract xsi:nil=\"true\">{MARK}true</isAbstract>"
            )),
        ),
        (
            "text in a nil content of a name",
            named(&format!(
                "<name><content xsi:nil=\"true\">{MARK}X</content></name>"
            )),
        ),
    ] {
        let error = refused_at_mark(what, &template);
        assert!(
            !matches!(error, ReadError::InvalidLiteral { .. }),
            "{ROW}: {what}: refused as a bad literal: {error:?}"
        );
    }
}

// --- Acceptance item 6: xsi:nil elsewhere -------------------------------------------------------

#[test]
fn sacm23_xmi_026_xsi_nil_on_an_element_that_is_not_a_value_element_stays_refused_at_the_attribute()
{
    let cases = [
        ("a name", named(&format!("<name {MARK}xsi:nil=\"true\"/>"))),
        (
            "a name with nil false",
            named(&format!("<name content=\"C\" {MARK}xsi:nil=\"false\"/>")),
        ),
        (
            "a value",
            content_values(&format!("<value {MARK}xsi:nil=\"true\"/>")),
        ),
        (
            "a content role element",
            claim(&format!(
                "<description xmi:id=\"d\"><content {MARK}xsi:nil=\"true\"/></description>"
            )),
        ),
        (
            "a key role element",
            claim(&format!(
                "<taggedValue xmi:id=\"t\"><key {MARK}xsi:nil=\"true\"/></taggedValue>"
            )),
        ),
        (
            "a child element",
            ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {MARK}xsi:nil=\"true\"/>"
            )),
        ),
        (
            "a UtilityElement",
            claim(&format!("<note xmi:id=\"n\" {MARK}xsi:nil=\"true\"/>")),
        ),
        (
            "a root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\" {MARK}xsi:nil=\"true\"><name content=\"P\"/>\
                 </sacm:ArgumentPackage>"
            ),
        ),
        (
            "a reference element",
            ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/>\
                 <citedElement xmi:idref=\"c1\" {MARK}xsi:nil=\"true\"/></argumentElement>"
            )),
        ),
        (
            "an unqualified nil on a gid",
            claim(&format!("<gid {MARK}nil=\"true\">g</gid>")),
        ),
        (
            "an unqualified nil on a name's lang",
            named(&format!("<name><lang {MARK}nil=\"true\"/></name>")),
        ),
    ];
    for (what, template) in &cases {
        let error = refused_at_mark(what, template);
        assert!(
            matches!(error, ReadError::Unsupported { .. }) && error.code().is_none(),
            "{ROW}: {what}: not Unsupported: {error:?} ({error})"
        );
    }
}

// --- Acceptance item 7: a nil occurrence counts --------------------------------------------------

#[test]
fn sacm23_xmi_026_a_nil_occurrence_counts_for_repeated_value() {
    let cases = [
        (
            "gid= and a nil gid element",
            ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {MARK}gid=\"x\">\
                 <name content=\"C\"/>{MARK}<gid xsi:nil=\"true\"/></argumentElement>"
            )),
            "c1",
            "gid",
        ),
        (
            "two nil gid elements",
            claim(&format!(
                "{MARK}<gid xsi:nil=\"true\"/>{MARK}<gid xsi:nil=\"1\"/>"
            )),
            "c1",
            "gid",
        ),
        (
            "a nil gid and a gid",
            claim(&format!("{MARK}<gid xsi:nil=\"true\"/>{MARK}<gid>x</gid>")),
            "c1",
            "gid",
        ),
        (
            "isCitation= and a nil isCitation element",
            ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {MARK}isCitation=\"false\">\
                 <name content=\"C\"/>{MARK}<isCitation xsi:nil=\"true\"/></argumentElement>"
            )),
            "c1",
            "isCitation",
        ),
        (
            "lang= on a name and a nil lang element",
            named(&format!(
                "<name {MARK}lang=\"en\">{MARK}<lang xsi:nil=\"true\"/></name>"
            )),
            "c1",
            "name.lang",
        ),
        (
            "two nil assertionDeclaration elements",
            claim(&format!(
                "{MARK}<assertionDeclaration xsi:nil=\"true\"/>\
                 {MARK}<assertionDeclaration xsi:nil=\"true\"/>"
            )),
            "c1",
            "assertionDeclaration",
        ),
    ];
    for (what, template, owner, expected) in &cases {
        let (input, marks) = marked(template);
        assert_eq!(marks.len(), 2, "{ROW}: setup: {what} has not two marks");
        let error = refused(what, &input);
        let ReadError::RepeatedValue {
            element, property, ..
        } = &error
        else {
            panic!("{ROW}: {what}: not RepeatedValue: {error:?} ({error})");
        };
        assert_eq!(
            (element.as_str(), property.as_str()),
            (*owner, *expected),
            "{ROW}: {what}: the refusal does not name the owner and the property"
        );
        assert_eq!(
            (error.code(), error.locations()),
            (Some(Code::MULTIPLICITY_VIOLATION), marks.as_slice()),
            "{ROW}: {what}: not SACM-MULT-001 at every occurrence"
        );
    }
}

// --- Acceptance item 8: writing a nil-read document -------------------------------------------------

#[test]
fn sacm23_xmi_026_a_nil_read_document_is_written_with_the_property_omitted_and_reads_back_equal() {
    for case in nils() {
        let what = case.what;
        let doc = read_ok(what, &case.input);
        let omitted = read_ok(&format!("{what}: omitted"), &case.omitted);
        for mode in MODES {
            let text = write_in(what, mode, &doc);
            assert!(
                !text.contains("nil") && !text.contains("XMLSchema-instance"),
                "{ROW}: {what}: a {mode:?} write writes xsi:nil:\n{text}"
            );
            assert_eq!(
                text,
                write_in(&format!("{what}: omitted"), mode, &omitted),
                "{ROW}: {what}: a {mode:?} write is not the write of the file without the element"
            );
            let back = read_ok(&format!("{what}: written in {mode:?} mode"), &text);
            assert_eq!(
                back, doc,
                "{ROW}: {what}: the {mode:?} output does not read back =="
            );
        }
    }
}
