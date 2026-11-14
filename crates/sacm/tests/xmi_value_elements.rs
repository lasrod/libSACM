//! Tests for the element form of single-valued values (SACM23-XMI-014; XMI
//! 2.5.1 §7.8.4, §9.3, §9.4.1, production 2b, rules 2b and 2h; XML 1.0
//! §2.11; ADR 0021 amendment 1; ADR 0015 amendment 2).
//!
//! `xmi::read` reads a `name`'s `lang` and `content`, and a Claim's
//! `assertionDeclaration`, from an XML element in no namespace named after
//! the property, as well as from the attribute. The value is the element's
//! character content exactly; an empty element is the empty string. A value
//! element with an attribute, an element child or a namespace is
//! `Unsupported` at that construct. A property given twice (attribute and
//! element, or two elements) is `ReadError::RepeatedValue` (SACM-MULT-001) in
//! both modes, with every occurrence located: an attribute at its name, an
//! element at its `<`.
//!
//! Each element-form input has an attribute-form twin: the same document
//! written as SACM23-XMI-012 and SACM23-XMI-017 read it. "Reads equal" is
//! `semantic_compare` finding no difference. The twins are listed once, in
//! the `*_twins` tables, and the round-trip test runs every one of them.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading, and are counted here: lines end at LF,
//! CRLF or a lone CR, and columns count characters. Most inputs start with
//! [`LEAD`], so the construct follows a CRLF and a non-ASCII character. The
//! pinned `Display` text is counted by hand.
//!
//! Every input is read in both modes, and must give the same result in both.
//! No input here has an `xsi:type` or an `xmi:version`, so a read reports no
//! diagnostic.
//!
//! Not checked here: `xsi:nil` read as absent (SACM23-XMI-026; here it is an
//! attribute, so `Unsupported`), the element forms of `gid`, `isCitation` and
//! `isAbstract` (SACM23-BASE-006/007/008), the legacy `<content lang>` name
//! (LIB-COMPAT-008), and the rest of a bad literal's `InvalidLiteral`
//! (SACM23-VAL-006, `xmi_literals.rs`): here it is checked only as far as
//! its kind, value, code and location.

use std::error::Error;
use std::fmt::{Debug, Display};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, SourceLocation};
use sacm::model::{AssertionDeclaration, Document, LangString};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-XMI-014";

/// Both read modes. A value element is read, and refused, the same in each.
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

/// The pinned SACM 2.3 namespace.
const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";

/// What comes before the root of most inputs: a declaration ended by CRLF,
/// then a comment holding `é` and two spaces.
const LEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<!-- \u{e9} -->  ";

/// The root's name, in the attribute form, where the test is about the
/// Claim.
const ROOT_NAME: &str = "<name lang=\"en\" content=\"Root\"/>";

/// How long one `read` may take: a scan that stops advancing fails its test
/// instead of stalling the run.
const DEADLINE: Duration = Duration::from_secs(5);

// --- Inputs ------------------------------------------------------------------

/// `content` in an ArgumentPackage root `a`, after [`LEAD`].
fn in_ap(content: &str) -> String {
    format!("{LEAD}<sacm:ArgumentPackage{NS} xmi:id=\"a\">{content}</sacm:ArgumentPackage>")
}

/// `content` in an AssuranceCasePackage root `case`, after [`LEAD`].
fn in_acp(content: &str) -> String {
    format!(
        "{LEAD}<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">{content}</sacm:AssuranceCasePackage>"
    )
}

/// `content` in an ArgumentPackage root `a` inside an `xmi:XMI` wrapper.
fn wrapped(content: &str) -> String {
    format!(
        "{LEAD}<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
         xmlns:sacm=\"{SACM_NS}\"><sacm:ArgumentPackage xmi:id=\"a\">{content}\
         </sacm:ArgumentPackage></xmi:XMI>"
    )
}

/// A Claim `c1` with `attributes` after its `xmi:id` and `content` inside,
/// in a root `a` named [`ROOT_NAME`].
fn claim(attributes: &str, content: &str) -> String {
    in_ap(&format!(
        "{ROOT_NAME}\r\n  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"{attributes}>\
         {content}</argumentElement>\r\n"
    ))
}

/// A Claim `c1` named "C", declared by the `content` it holds after its
/// name.
fn declared_claim(content: &str) -> String {
    claim("", &format!("<name lang=\"en\" content=\"C\"/>{content}"))
}

/// A Claim `c1` named "C", declared by the attribute `assertionDeclaration`.
fn declared_claim_attribute(literal: &str) -> String {
    claim(
        &format!(" assertionDeclaration=\"{literal}\""),
        "<name lang=\"en\" content=\"C\"/>",
    )
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, CRLF or a lone CR, and columns count characters.
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

/// A 1-based location.
fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

// --- Reading -------------------------------------------------------------------

/// `xmi::read` in `mode`, on a worker thread. Fails the test when the read
/// panics or does not return within [`DEADLINE`].
fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    let owned = input.as_bytes().to_vec();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        // The receiver is gone only after the deadline has already failed the test.
        let _ = sender.send(xmi::read(&owned, &ReadOptions::default().with_mode(mode)));
    });
    match receiver.recv_timeout(DEADLINE) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            panic!("{ROW}: read did not return within {DEADLINE:?} for {input:?}")
        }
        Err(RecvTimeoutError::Disconnected) => panic!("{ROW}: read panicked on {input:?}"),
    }
}

/// Reads `input` in `mode`, which the test expects to be read with no
/// diagnostic.
fn read_ok_in(what: &str, mode: Mode, input: &str) -> Document {
    match read(mode, input) {
        Ok(output) => {
            assert_eq!(
                output.mode(),
                mode,
                "{ROW}: {what}: the output reports another mode"
            );
            assert!(
                output.diagnostics().is_empty(),
                "{ROW}: {what}: reading in {mode:?} mode reported {:?}",
                output.diagnostics()
            );
            output.into_document()
        }
        Err(error) => {
            panic!("{ROW}: {what}: {mode:?} read refused {input:?}: {error} ({error:?})")
        }
    }
}

/// Reads `input` in both modes, with no diagnostic, to equal documents, and
/// returns the strict one.
fn read_ok(what: &str, input: &str) -> Document {
    let strict = read_ok_in(what, Mode::Strict, input);
    let tolerant = read_ok_in(what, Mode::Tolerant, input);
    assert_same(&format!("{what}, strict and tolerant"), &strict, &tolerant);
    strict
}

/// Asserts that `left` and `right` are the same document.
fn assert_same(what: &str, left: &Document, right: &Document) {
    let differences = semantic_compare(left, right);
    assert!(
        differences.is_empty(),
        "{ROW}: {what}: the documents differ: {differences:?}"
    );
}

/// Reads `input` in `mode`, which the test expects to be refused.
fn refused_in(what: &str, mode: Mode, input: &str) -> ReadError {
    match read(mode, input) {
        Ok(output) => panic!(
            "{ROW}: {what}: {mode:?} read returned a document of {} elements for {input:?}",
            output.document().element_count()
        ),
        Err(error) => error,
    }
}

/// Reads `input` in both modes, which the test expects to be refused the
/// same in both, and returns the refusal.
fn refused(what: &str, input: &str) -> ReadError {
    let strict = refused_in(what, Mode::Strict, input);
    let tolerant = refused_in(what, Mode::Tolerant, input);
    assert_eq!(
        strict, tolerant,
        "{ROW}: {what}: the modes refuse differently"
    );
    strict
}

/// Reads `template` without its one mark, in both modes, and returns the
/// refusal, failing the test unless it is `Unsupported` with no code, at the
/// mark.
fn assert_unsupported_at_mark(what: &str, template: &str) -> ReadError {
    let (input, marks) = marked(template);
    assert_eq!(
        marks.len(),
        1,
        "{ROW}: setup: {what} has {} marks, not one",
        marks.len()
    );
    let error = refused(what, &input);
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
        Some(marks[0]),
        "{ROW}: {what}: refused at the wrong place: {error}"
    );
    error
}

/// Reads `template` without its one mark, in both modes, and returns the
/// refusal, failing the test unless it is `InvalidLiteral` (SACM-ENUM-001,
/// SACM23-VAL-006) for the Claim `c1`'s `assertionDeclaration`, holding
/// `value`, at the mark.
fn assert_invalid_literal_at_mark(what: &str, template: &str, value: &str) -> ReadError {
    let (input, marks) = marked(template);
    assert_eq!(
        marks.len(),
        1,
        "{ROW}: setup: {what} has {} marks, not one",
        marks.len()
    );
    let error = refused(what, &input);
    match &error {
        ReadError::InvalidLiteral {
            element,
            property,
            value: found,
            ..
        } => {
            assert_eq!(element.as_str(), "c1", "{ROW}: {what}: the element");
            assert_eq!(
                property, "assertionDeclaration",
                "{ROW}: {what}: the property"
            );
            assert_eq!(found, value, "{ROW}: {what}: the value as read");
        }
        other => panic!("{ROW}: {what}: expected InvalidLiteral, got {other:?}"),
    }
    assert_eq!(
        error.code(),
        Some(Code::ENUM_INVALID_LITERAL),
        "{ROW}: {what}: the refusal is not SACM-ENUM-001"
    );
    assert_eq!(
        error.location(),
        Some(marks[0]),
        "{ROW}: {what}: refused at the wrong place: {error}"
    );
    error
}

/// The name of the element `id` of `doc`, which must exist.
fn name_of(what: &str, doc: &Document, id: &str) -> Option<LangString> {
    let Some(element) = doc.get(id) else {
        panic!("{ROW}: {what}: the document has no element {id:?}");
    };
    element.name().cloned()
}

/// The declaration of the Claim `c1` of `doc`.
fn declaration_of(what: &str, doc: &Document) -> Option<AssertionDeclaration> {
    let Some(element) = doc.get("c1") else {
        panic!("{ROW}: {what}: the document has no Claim c1");
    };
    element.assertion_declaration()
}

/// A `LangString` with exactly the fields given.
fn lang_string(lang: Option<&str>, content: Option<&str>) -> LangString {
    let mut name = LangString::default();
    name.set_lang(lang.map(str::to_owned));
    name.set_content(content.map(str::to_owned));
    name
}

// --- Twins ---------------------------------------------------------------------

/// One input in the element form, and the same document in the attribute
/// form.
struct Twin {
    what: String,
    element: String,
    attribute: String,
}

fn twin(what: impl Into<String>, element: String, attribute: String) -> Twin {
    Twin {
        what: what.into(),
        element,
        attribute,
    }
}

/// Reads both forms of `twin` in both modes and asserts that they are the
/// same document. Returns the element form's.
fn assert_twin(twin: &Twin) -> Document {
    let element = read_ok(&format!("{} (element form)", twin.what), &twin.element);
    let attribute = read_ok(&format!("{} (attribute form)", twin.what), &twin.attribute);
    assert_same(
        &format!("{}: the element form and its attribute twin", twin.what),
        &element,
        &attribute,
    );
    element
}

/// Item 1: `lang` and `content` elements in either order, on each root form
/// and on a Claim.
fn both_elements_twins() -> Vec<Twin> {
    let attribute = "<name lang=\"en\" content=\"Case\"/>";
    let mut twins = Vec::new();
    for (order, element) in [
        (
            "lang then content",
            "<name><lang>en</lang><content>Case</content></name>",
        ),
        (
            "content then lang",
            "<name><content>Case</content><lang>en</lang></name>",
        ),
        (
            "with white space, comments and a processing instruction between",
            "<name>\r\n  <!-- c --><content>Case</content>\r\n  <?pi x?> <lang>en</lang>\r\n</name>",
        ),
    ] {
        twins.push(twin(
            format!("{order}, on a bare ArgumentPackage"),
            in_ap(element),
            in_ap(attribute),
        ));
        twins.push(twin(
            format!("{order}, on a bare AssuranceCasePackage"),
            in_acp(element),
            in_acp(attribute),
        ));
        twins.push(twin(
            format!("{order}, on a wrapped ArgumentPackage"),
            wrapped(element),
            wrapped(attribute),
        ));
        twins.push(twin(
            format!("{order}, on a Claim"),
            claim("", element),
            claim("", attribute),
        ));
    }
    twins
}

/// Item 1: the two forms mixed across properties and across elements.
fn mixed_twins() -> Vec<Twin> {
    let attribute = "<name lang=\"en\" content=\"Case\"/>";
    vec![
        twin(
            "lang as an attribute, content as an element",
            in_ap("<name lang=\"en\"><content>Case</content></name>"),
            in_ap(attribute),
        ),
        twin(
            "content as an attribute, lang as an element",
            in_ap("<name content=\"Case\"><lang>en</lang></name>"),
            in_ap(attribute),
        ),
        twin(
            "the root in the attribute form, the Claim's name and declaration as elements",
            claim(
                "",
                "<name><lang>en</lang><content>Case</content></name>\
                 <assertionDeclaration>assumed</assertionDeclaration>",
            ),
            claim(" assertionDeclaration=\"assumed\"", attribute),
        ),
        twin(
            "the Claim's declaration as an attribute, its name's content as an element",
            claim(
                " assertionDeclaration=\"defeated\"",
                "<name lang=\"en\"><content>Case</content></name>",
            ),
            claim(" assertionDeclaration=\"defeated\"", attribute),
        ),
        twin(
            "a value element undeclaring the default namespace",
            format!(
                "{LEAD}<ArgumentPackage xmlns=\"{SACM_NS}\" \
                 xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"a\">\
                 <name xmlns=\"\"><lang>en</lang><content>Case</content></name></ArgumentPackage>"
            ),
            in_ap(attribute),
        ),
        twin(
            "a value element with xmlns=\"\" under a prefixed root",
            in_ap("<name><lang xmlns=\"\">en</lang><content>Case</content></name>"),
            in_ap(attribute),
        ),
    ]
}

/// Item 1: one field as an element, the other absent.
fn one_field_twins() -> Vec<Twin> {
    vec![
        twin(
            "content alone",
            in_ap("<name><content>Case</content></name>"),
            in_ap("<name content=\"Case\"/>"),
        ),
        twin(
            "lang alone",
            in_ap("<name><lang>en</lang></name>"),
            in_ap("<name lang=\"en\"/>"),
        ),
    ]
}

/// Item 1: empty value elements, each the empty string.
fn empty_twins() -> Vec<Twin> {
    vec![
        twin(
            "<content/>",
            in_ap("<name><content/></name>"),
            in_ap("<name content=\"\"/>"),
        ),
        twin(
            "<content></content>",
            in_ap("<name><content></content></name>"),
            in_ap("<name content=\"\"/>"),
        ),
        twin(
            "<lang/> and <content></content>",
            in_ap("<name><lang/><content></content></name>"),
            in_ap("<name lang=\"\" content=\"\"/>"),
        ),
        twin(
            "<content><![CDATA[]]></content>",
            in_ap("<name><content><![CDATA[]]></content></name>"),
            in_ap("<name content=\"\"/>"),
        ),
        twin(
            "<content><!--c--></content>",
            in_ap("<name><content><!--c--></content></name>"),
            in_ap("<name content=\"\"/>"),
        ),
    ]
}

/// Item 2: each literal as an element on a Claim, in several positions.
fn declaration_twins() -> Vec<Twin> {
    let mut twins = Vec::new();
    for literal in AssertionDeclaration::ALL.iter().map(|d| d.as_str()) {
        let element = format!("<assertionDeclaration>{literal}</assertionDeclaration>");
        let attribute = declared_claim_attribute(literal);
        twins.push(twin(
            format!("{literal} after the name"),
            declared_claim(&element),
            attribute.clone(),
        ));
        twins.push(twin(
            format!("{literal} before the name"),
            claim("", &format!("{element}<name lang=\"en\" content=\"C\"/>")),
            attribute.clone(),
        ));
        twins.push(twin(
            format!("{literal} between white space and comments"),
            claim(
                "",
                &format!(
                    "\r\n  <!-- before -->\r\n  {element}\r\n  <!-- after -->\r\n  \
                     <name lang=\"en\" content=\"C\"/>\r\n"
                ),
            ),
            attribute.clone(),
        ));
        twins.push(twin(
            format!("{literal} beside a name in the element form"),
            claim(
                "",
                &format!("<name><content>C</content><lang>en</lang></name>{element}"),
            ),
            attribute,
        ));
    }
    twins
}

/// Item 3: the value is the character content, exactly.
fn exact_twins() -> Vec<Twin> {
    let content = |inside: &str, attribute_value: &str| {
        twin(
            format!("<content>{inside}</content>"),
            in_ap(&format!("<name><content>{inside}</content></name>")),
            in_ap(&format!("<name content=\"{attribute_value}\"/>")),
        )
    };
    vec![
        content("a<!--c-->b", "ab"),
        content("a<?pi x?>b", "ab"),
        content("<!--c-->ab<!--d-->", "ab"),
        content("a&amp;<![CDATA[<b>]]>&#13;", "a&amp;&lt;b&gt;&#13;"),
        content("&lt;&gt;&quot;&apos;&#x1F600;", "&lt;&gt;&quot;'\u{1F600}"),
        content("<![CDATA[ x ]]>", " x "),
        content("a\tb\nc", "a&#9;b&#10;c"),
        content("a\r\nb", "a&#10;b"),
        content("a\rb", "a&#10;b"),
        content("  a  ", "  a  "),
        content(" \t\n ", " &#9;&#10; "),
        content("&#32;&#9;", " &#9;"),
        twin(
            "<lang> e<!--c-->n </lang>",
            in_ap("<name><lang> e<!--c-->n </lang></name>"),
            in_ap("<name lang=\" en \"/>"),
        ),
    ]
}

/// Item 3: a declaration element whose content, however written, is one
/// literal.
fn declaration_content_twins() -> Vec<Twin> {
    [
        ("assu<!--c-->med", "assumed"),
        ("<![CDATA[axiomatic]]>", "axiomatic"),
        ("&#x64;efeated", "defeated"),
        ("as<?pi x?>Cited", "asCited"),
        ("needs<![CDATA[Support]]>", "needsSupport"),
    ]
    .into_iter()
    .map(|(inside, literal)| {
        twin(
            format!("<assertionDeclaration>{inside}</assertionDeclaration>"),
            declared_claim(&format!(
                "<assertionDeclaration>{inside}</assertionDeclaration>"
            )),
            declared_claim_attribute(literal),
        )
    })
    .collect()
}

/// Every element-form input of this file that reads.
fn all_twins() -> Vec<Twin> {
    [
        both_elements_twins(),
        mixed_twins(),
        one_field_twins(),
        empty_twins(),
        declaration_twins(),
        exact_twins(),
        declaration_content_twins(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

// --- Item 1: lang and content ---------------------------------------------------

#[test]
fn sacm23_xmi_014_lang_and_content_elements_in_either_order_read_equal_to_the_attribute_form() {
    let twins = both_elements_twins();
    assert_eq!(twins.len(), 12, "{ROW}: setup: three orders on four owners");
    for twin in &twins {
        let doc = assert_twin(twin);
        let id = if twin.what.ends_with("Claim") {
            "c1"
        } else if twin.what.contains("AssuranceCasePackage") {
            "case"
        } else {
            "a"
        };
        assert_eq!(
            name_of(&twin.what, &doc, id),
            Some(LangString::new("Case").with_lang("en")),
            "{ROW}: {}: the name of {id:?}",
            twin.what
        );
    }
}

#[test]
fn sacm23_xmi_014_attribute_and_element_forms_mix_across_properties_and_elements() {
    for twin in &mixed_twins() {
        assert_twin(twin);
    }
    let doc = read_ok(
        "content as an element, lang as an attribute",
        &in_ap("<name lang=\"en\"><content>Case</content></name>"),
    );
    assert_eq!(
        name_of("mixed", &doc, "a"),
        Some(LangString::new("Case").with_lang("en")),
        "{ROW}: `<name lang=\"en\"><content>Case</content></name>` does not read as that name"
    );
}

#[test]
fn sacm23_xmi_014_a_content_element_alone_gives_a_name_with_no_lang() {
    for twin in &one_field_twins() {
        assert_twin(twin);
    }
    let doc = read_ok(
        "content alone",
        &in_ap("<name><content>Case</content></name>"),
    );
    assert_eq!(
        name_of("content alone", &doc, "a"),
        Some(lang_string(None, Some("Case"))),
        "{ROW}: `<name><content>Case</content></name>` must have content and no lang"
    );
    let doc = read_ok("lang alone", &in_ap("<name><lang>en</lang></name>"));
    assert_eq!(
        name_of("lang alone", &doc, "a"),
        Some(lang_string(Some("en"), None)),
        "{ROW}: `<name><lang>en</lang></name>` must have lang and no content"
    );
}

#[test]
fn sacm23_xmi_014_an_empty_value_element_reads_as_the_empty_string_not_as_absent() {
    for twin in &empty_twins() {
        assert_twin(twin);
    }
    for (what, inside, expected) in [
        ("<content/>", "<content/>", lang_string(None, Some(""))),
        (
            "<content></content>",
            "<content></content>",
            lang_string(None, Some("")),
        ),
        ("<lang/>", "<lang/>", lang_string(Some(""), None)),
        (
            "<lang></lang>",
            "<lang></lang>",
            lang_string(Some(""), None),
        ),
    ] {
        let doc = read_ok(what, &in_ap(&format!("<name>{inside}</name>")));
        assert_eq!(
            name_of(what, &doc, "a"),
            Some(expected),
            "{ROW}: {what} must read as Some(\"\"), not as absent"
        );
    }
}

// --- Item 2: assertionDeclaration -------------------------------------------------

#[test]
fn sacm23_xmi_014_each_literal_as_an_element_on_a_claim_reads_equal_to_the_attribute_form() {
    let twins = declaration_twins();
    assert_eq!(
        twins.len(),
        24,
        "{ROW}: setup: six literals in four positions"
    );
    for (twin, &literal) in twins.iter().zip(
        AssertionDeclaration::ALL
            .iter()
            .flat_map(|literal| [literal; 4]),
    ) {
        let doc = assert_twin(twin);
        assert_eq!(
            declaration_of(&twin.what, &doc),
            Some(literal),
            "{ROW}: {}: the Claim's declaration",
            twin.what
        );
        assert_eq!(
            name_of(&twin.what, &doc, "c1"),
            Some(LangString::new("C").with_lang("en")),
            "{ROW}: {}: the Claim's name",
            twin.what
        );
    }
}

#[test]
fn sacm23_xmi_014_an_asserted_element_reads_equal_to_no_declaration() {
    let absent = read_ok("no declaration", &declared_claim(""));
    for (what, inside) in [
        (
            "<assertionDeclaration>asserted</assertionDeclaration>",
            "<assertionDeclaration>asserted</assertionDeclaration>",
        ),
        (
            "asserted written with a reference",
            "<assertionDeclaration>&#x61;sserted</assertionDeclaration>",
        ),
    ] {
        let doc = read_ok(what, &declared_claim(inside));
        assert_same(&format!("{what} and no declaration"), &doc, &absent);
        assert_eq!(
            declaration_of(what, &doc),
            Some(AssertionDeclaration::Asserted),
            "{ROW}: {what}: the Claim's declaration"
        );
    }
}

#[test]
fn sacm23_xmi_014_a_value_element_reads_only_on_its_owner_and_elsewhere_is_unsupported_at_its_start_tag()
 {
    // On its owner it reads.
    let doc = read_ok(
        "a declaration element on a Claim",
        &declared_claim("<assertionDeclaration>assumed</assertionDeclaration>"),
    );
    assert_eq!(
        declaration_of("a declaration element on a Claim", &doc),
        Some(AssertionDeclaration::Assumed),
        "{ROW}: the declaration element on a Claim is not read"
    );
    // Elsewhere it stays refused at its start tag.
    let element = format!("{MARK}<assertionDeclaration>asserted</assertionDeclaration>");
    let cases = [
        ("an ArgumentPackage root", in_ap(&element)),
        ("an AssuranceCasePackage root", in_acp(&element)),
        (
            "an ArgumentPackage in argumentElement",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\">\
                 <name content=\"X\"/>{element}</argumentElement>"
            )),
        ),
        (
            "an ArgumentPackage in argumentPackage",
            in_acp(&format!(
                "<argumentPackage xmi:id=\"x\">{element}</argumentPackage>"
            )),
        ),
        (
            "an AssuranceCasePackage in assuranceCasePackage",
            in_acp(&format!(
                "<assuranceCasePackage xmi:id=\"x\">{element}</assuranceCasePackage>"
            )),
        ),
        (
            "a content element directly in a Claim",
            claim("", &format!("{MARK}<content>x</content>")),
        ),
        (
            "a lang element directly in a root",
            in_ap(&format!("{MARK}<lang>en</lang>")),
        ),
    ];
    for (what, template) in cases {
        assert_unsupported_at_mark(what, &template);
    }
}

// --- Item 3: the value is exact ---------------------------------------------------

#[test]
fn sacm23_xmi_014_comments_and_processing_instructions_in_a_value_element_are_ignored() {
    for (inside, expected) in [
        ("a<!--c-->b", "ab"),
        ("a<?pi x?>b", "ab"),
        ("<!--c-->ab<!--d-->", "ab"),
    ] {
        let doc = read_ok(
            inside,
            &in_ap(&format!("<name><content>{inside}</content></name>")),
        );
        assert_eq!(
            name_of(inside, &doc, "a"),
            Some(lang_string(None, Some(expected))),
            "{ROW}: <content>{inside}</content> must read {expected:?}"
        );
    }
    let doc = read_ok("lang", &in_ap("<name><lang> e<!--c-->n </lang></name>"));
    assert_eq!(
        name_of("lang", &doc, "a"),
        Some(lang_string(Some(" en "), None)),
        "{ROW}: <lang> e<!--c-->n </lang> must read \" en \""
    );
}

#[test]
fn sacm23_xmi_014_cdata_and_references_in_a_value_element_give_their_characters() {
    for (inside, expected) in [
        ("a&amp;<![CDATA[<b>]]>&#13;", "a&<b>\r"),
        ("&lt;&gt;&quot;&apos;&#x1F600;", "<>\"'\u{1F600}"),
        ("<![CDATA[ x ]]>", " x "),
        ("&#32;&#9;", " \t"),
    ] {
        let doc = read_ok(
            inside,
            &in_ap(&format!("<name><content>{inside}</content></name>")),
        );
        assert_eq!(
            name_of(inside, &doc, "a"),
            Some(lang_string(None, Some(expected))),
            "{ROW}: <content>{inside}</content> must read {expected:?}"
        );
    }
}

#[test]
fn sacm23_xmi_014_tab_lf_and_edge_spaces_are_kept_and_a_literal_crlf_or_cr_reads_as_lf() {
    for (inside, expected) in [
        ("a\tb\nc", "a\tb\nc"),
        ("a\r\nb", "a\nb"),
        ("a\rb", "a\nb"),
        ("  a  ", "  a  "),
        (" \t\n ", " \t\n "),
        ("\r\n", "\n"),
    ] {
        let doc = read_ok(
            inside,
            &in_ap(&format!("<name><content>{inside}</content></name>")),
        );
        assert_eq!(
            name_of(inside, &doc, "a"),
            Some(lang_string(None, Some(expected))),
            "{ROW}: <content>{inside:?}</content> must read {expected:?}: nothing is trimmed"
        );
    }
    for twin in &exact_twins() {
        assert_twin(twin);
    }
}

#[test]
fn sacm23_xmi_014_a_declaration_element_whose_content_is_a_literal_however_written_reads_as_it() {
    for twin in &declaration_content_twins() {
        assert_twin(twin);
    }
}

#[test]
fn sacm23_xmi_014_a_declaration_element_holding_no_literal_is_invalid_literal_at_its_start_tag_holding_it()
 {
    let mut failures = Vec::new();
    for (element, as_read) in [
        (
            "<assertionDeclaration> needsSupport </assertionDeclaration>",
            " needsSupport ",
        ),
        (
            "<assertionDeclaration>NeedsSupport</assertionDeclaration>",
            "NeedsSupport",
        ),
        ("<assertionDeclaration/>", ""),
        ("<assertionDeclaration></assertionDeclaration>", ""),
        (
            "<assertionDeclaration>needsSupport&#10;</assertionDeclaration>",
            "needsSupport\n",
        ),
        (
            "<assertionDeclaration>\tassumed</assertionDeclaration>",
            "\tassumed",
        ),
        (
            "<assertionDeclaration>\r\nassumed</assertionDeclaration>",
            "\nassumed",
        ),
        (
            "<assertionDeclaration>justification</assertionDeclaration>",
            "justification",
        ),
    ] {
        let error = assert_invalid_literal_at_mark(
            element,
            &declared_claim(&format!("\r\n  {MARK}{element}")),
            as_read,
        );
        let quoted = format!("{as_read:?}");
        let shown = error.to_string();
        if !shown.contains(&quoted) || shown.contains('\n') {
            failures.push(format!(
                "{element:?}: the message {shown:?} is not one line quoting {quoted}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} problems:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// --- Item 4: shape ----------------------------------------------------------------

/// The three value elements, each in its owner: `place` gives, for an
/// element name, what the case is and the marked element.
fn value_elements(place: &dyn Fn(&str) -> (String, String)) -> Vec<(String, String)> {
    let mut cases = Vec::new();
    for element in ["lang", "content"] {
        let (what, inner) = place(element);
        cases.push((
            format!("{what} on <{element}>"),
            in_ap(&format!("<name>{inner}</name>")),
        ));
    }
    let (what, inner) = place("assertionDeclaration");
    cases.push((
        format!("{what} on <assertionDeclaration>"),
        declared_claim(&inner),
    ));
    cases
}

#[test]
fn sacm23_xmi_014_an_attribute_on_a_value_element_is_unsupported_at_its_name() {
    for attribute in [
        "xmi:id=\"v\"",
        "xmi:type=\"sacm:LangString\"",
        "xsi:type=\"sacm:LangString\"",
        "xsi:nil=\"true\"",
        "xsi:nil=\"false\"",
        "foo=\"1\"",
        "lang=\"en\"",
    ] {
        for (what, template) in value_elements(&|element| {
            let value = if element == "assertionDeclaration" {
                "assumed"
            } else {
                "x"
            };
            (
                attribute.to_owned(),
                format!("<{element}\r\n    {MARK}{attribute}>{value}</{element}>"),
            )
        }) {
            assert_unsupported_at_mark(&what, &template);
        }
    }
    // The legacy multi-language form's `<content lang>`, with its value as
    // text (LIB-COMPAT-008): the attribute is refused, at its name.
    assert_unsupported_at_mark(
        "<content lang> in a name with a lang attribute",
        &in_ap(&format!(
            "<name lang=\"en\"><content {MARK}lang=\"en\">x</content></name>"
        )),
    );
}

#[test]
fn sacm23_xmi_014_an_element_in_a_value_element_is_unsupported_at_its_start_tag() {
    for (form, inner) in [
        ("an empty element", "{MARK}<b/>"),
        ("an element after text", "x{MARK}<b/>"),
        ("an element after a comment", "<!--c-->{MARK}<b>x</b>"),
        ("a value element", "{MARK}<content>x</content>"),
        ("a value child of the legacy form", "{MARK}<value>x</value>"),
    ] {
        for (what, template) in value_elements(&|element| {
            (
                form.to_owned(),
                format!(
                    "<{element}>{}</{element}>",
                    inner.replace("{MARK}", &MARK.to_string())
                ),
            )
        }) {
            assert_unsupported_at_mark(&what, &template);
        }
    }
}

#[test]
fn sacm23_xmi_014_a_value_element_in_a_namespace_is_unsupported_at_its_start_tag() {
    // In no namespace it reads, however the no-namespace is reached.
    for twin in mixed_twins()
        .iter()
        .filter(|twin| twin.what.contains("xmlns=\"\"") || twin.what.contains("default"))
    {
        assert_twin(twin);
    }
    let sacm_default = format!(" xmlns=\"{SACM_NS}\"");
    for (form, open, close) in [
        (
            "sacm-prefixed",
            "sacm:{e}".to_owned(),
            "sacm:{e}".to_owned(),
        ),
        ("xmi-prefixed", "xmi:{e}".to_owned(), "xmi:{e}".to_owned()),
        (
            "in a foreign namespace",
            "o:{e} xmlns:o=\"urn:o\"".to_owned(),
            "o:{e}".to_owned(),
        ),
        (
            "unprefixed in a default SACM namespace",
            format!("{{e}}{sacm_default}"),
            "{e}".to_owned(),
        ),
    ] {
        for (what, template) in value_elements(&|element| {
            let value = if element == "assertionDeclaration" {
                "assumed"
            } else {
                "x"
            };
            (
                form.to_owned(),
                format!(
                    "{MARK}<{}>{value}</{}>",
                    open.replace("{e}", element),
                    close.replace("{e}", element)
                ),
            )
        }) {
            assert_unsupported_at_mark(&what, &template);
        }
    }
}

// --- Item 5: repeats ----------------------------------------------------------------

/// Reads `template` without its marks, in both modes, and asserts that it is
/// refused as `RepeatedValue` of `property` on `element`, with one location
/// per mark, in order, and `location()` the second. Returns the refusal.
fn assert_repeated(what: &str, template: &str, element: &str, property: &str) -> ReadError {
    let (input, marks) = marked(template);
    assert!(
        marks.len() >= 2,
        "{ROW}: setup: {what} marks {} occurrences",
        marks.len()
    );
    let error = refused(what, &input);
    match &error {
        ReadError::RepeatedValue {
            element: found_element,
            property: found_property,
            locations,
            ..
        } => {
            assert_eq!(
                found_element.as_str(),
                element,
                "{ROW}: {what}: the refusal names the wrong element"
            );
            assert_eq!(
                found_property, property,
                "{ROW}: {what}: the refusal names the wrong property"
            );
            assert_eq!(
                locations, &marks,
                "{ROW}: {what}: the field holds other locations"
            );
        }
        other => panic!("{ROW}: {what}: expected RepeatedValue, got {other:?} ({other})"),
    }
    assert_eq!(
        error.code(),
        Some(Code::MULTIPLICITY_VIOLATION),
        "{ROW}: {what}: the code"
    );
    assert_eq!(
        error.code().map(Code::as_str),
        Some("SACM-MULT-001"),
        "{ROW}: {what}: the code's text"
    );
    assert_eq!(
        error.locations(),
        marks.as_slice(),
        "{ROW}: {what}: locations() is not every occurrence in document order"
    );
    assert_eq!(
        error.location(),
        Some(marks[1]),
        "{ROW}: {what}: location() is not the second occurrence"
    );
    error
}

/// The inputs that give a value twice: what, template, element, property.
fn repeats() -> Vec<(&'static str, String, &'static str, &'static str)> {
    let declaration = "<assertionDeclaration>assumed</assertionDeclaration>";
    let name = "<name lang=\"en\" content=\"C\"/>";
    vec![
        (
            "assertionDeclaration=\"assumed\" and an equal element",
            claim(
                &format!(" {MARK}assertionDeclaration=\"assumed\""),
                &format!("{name}{MARK}{declaration}"),
            ),
            "c1",
            "assertionDeclaration",
        ),
        (
            "assertionDeclaration=\"asserted\" and an equal element",
            claim(
                &format!(" {MARK}assertionDeclaration=\"asserted\""),
                &format!("{name}\r\n{MARK}<assertionDeclaration>asserted</assertionDeclaration>"),
            ),
            "c1",
            "assertionDeclaration",
        ),
        (
            "assertionDeclaration=\"assumed\" and a defeated element",
            claim(
                &format!(" {MARK}assertionDeclaration=\"assumed\""),
                &format!("{MARK}<assertionDeclaration>defeated</assertionDeclaration>{name}"),
            ),
            "c1",
            "assertionDeclaration",
        ),
        (
            "two equal assertionDeclaration elements, a name between",
            claim(
                "",
                &format!("{MARK}{declaration}\r\n  {name}\r\n  {MARK}{declaration}"),
            ),
            "c1",
            "assertionDeclaration",
        ),
        (
            "<name lang=\"en\"><lang>en</lang></name> on a root",
            in_ap(&format!(
                "<name {MARK}lang=\"en\">{MARK}<lang>en</lang></name>"
            )),
            "a",
            "name.lang",
        ),
        (
            "<name lang=\"en\"><lang>en</lang></name> on a Claim",
            claim(
                "",
                &format!(
                    "<name content=\"C\"\r\n {MARK}lang=\"en\">\r\n {MARK}<lang>en</lang></name>"
                ),
            ),
            "c1",
            "name.lang",
        ),
        (
            "two different lang elements",
            in_ap(&format!(
                "<name>{MARK}<lang>en</lang>{MARK}<lang>sv</lang></name>"
            )),
            "a",
            "name.lang",
        ),
        (
            "two equal content elements",
            in_ap(&format!(
                "<name>{MARK}<content>A</content><lang>en</lang>{MARK}<content>A</content></name>"
            )),
            "a",
            "name.content",
        ),
        (
            "content=\"\" and <content/>",
            in_acp(&format!("<name {MARK}content=\"\">{MARK}<content/></name>")),
            "case",
            "name.content",
        ),
    ]
}

#[test]
fn sacm23_xmi_014_a_value_given_twice_is_a_repeated_value_in_both_modes_naming_every_occurrence() {
    for (what, template, element, property) in repeats() {
        let error = assert_repeated(what, &template, element, property);
        assert_eq!(error.locations().len(), 2, "{ROW}: {what}: two occurrences");
    }
}

#[test]
fn sacm23_xmi_014_a_value_given_three_times_names_all_three_occurrences() {
    let cases = [
        (
            "a content attribute and two content elements",
            in_ap(&format!(
                "<name {MARK}content=\"A\">{MARK}<content>A</content>\r\n{MARK}<content>B</content></name>"
            )),
            "a",
            "name.content",
        ),
        (
            "three assertionDeclaration elements",
            declared_claim(&format!(
                "{MARK}<assertionDeclaration>assumed</assertionDeclaration>\
                 {MARK}<assertionDeclaration>assumed</assertionDeclaration>\
                 {MARK}<assertionDeclaration>assumed</assertionDeclaration>"
            )),
            "c1",
            "assertionDeclaration",
        ),
    ];
    for (what, template, element, property) in cases {
        let error = assert_repeated(what, &template, element, property);
        assert_eq!(
            error.locations().len(),
            3,
            "{ROW}: {what}: three occurrences"
        );
    }
}

#[test]
fn sacm23_xmi_014_a_repeated_value_element_is_checked_whole_before_it_counts_as_a_repeat() {
    // A value element is checked whole (namespace, attributes, children,
    // value) when it is met, and only then against earlier occurrences: each
    // of these repeats a value, but the repeat is itself not a value, so the
    // read is refused at that construct, not as `RepeatedValue`: a bad
    // literal as `InvalidLiteral` (SACM23-VAL-006), the rest as
    // `Unsupported`.
    let name = "<name lang=\"en\" content=\"C\"/>";
    let literals = [
        (
            "assertionDeclaration=\"defeated\" and an empty declaration element",
            claim(
                " assertionDeclaration=\"defeated\"",
                &format!("{name}{MARK}<assertionDeclaration/>"),
            ),
            "",
        ),
        (
            "a declaration element, then one holding no literal",
            declared_claim(&format!(
                "<assertionDeclaration>assumed</assertionDeclaration>\
                 {MARK}<assertionDeclaration>bogus</assertionDeclaration>"
            )),
            "bogus",
        ),
    ];
    for (what, template, value) in literals {
        let error = assert_invalid_literal_at_mark(what, &template, value);
        assert!(
            !matches!(error, ReadError::RepeatedValue { .. }),
            "{ROW}: {what}: the repeat was counted before the element was checked: {error:?}"
        );
    }
    let cases = [
        (
            "content=\"A\" and a content element with an attribute",
            in_ap(&format!(
                "<name content=\"A\"><content {MARK}x=\"1\">A</content></name>"
            )),
        ),
        (
            "a lang element, then one with an attribute",
            in_ap(&format!(
                "<name><lang>en</lang><lang {MARK}x=\"1\">en</lang></name>"
            )),
        ),
        (
            "a content element, then one holding an element",
            in_ap(&format!(
                "<name><content>A</content><content>{MARK}<b/></content></name>"
            )),
        ),
        (
            "content=\"A\" and a content element holding an element",
            in_ap(&format!(
                "<name content=\"A\"><content>{MARK}<b/></content></name>"
            )),
        ),
        (
            "content=\"A\" and a sacm-prefixed content element",
            in_ap(&format!(
                "<name content=\"A\">{MARK}<sacm:content>A</sacm:content></name>"
            )),
        ),
    ];
    for (what, template) in cases {
        let error = assert_unsupported_at_mark(what, &template);
        assert!(
            !matches!(error, ReadError::RepeatedValue { .. }),
            "{ROW}: {what}: the repeat was counted before the element was checked: {error:?}"
        );
    }
}

// --- Item 6: Display and the error traits -------------------------------------------

#[test]
fn sacm23_xmi_014_repeated_value_display_is_one_lowercase_line_naming_the_property_element_and_every_location()
 {
    let mut failures = Vec::new();
    for (what, template, element, property) in repeats() {
        let error = assert_repeated(what, &template, element, property);
        let shown = error.to_string();
        let mut wanted = vec![format!("{property:?}"), format!("{element:?}")];
        wanted.extend(error.locations().iter().map(ToString::to_string));
        for part in wanted {
            if !shown.contains(&part) {
                failures.push(format!("{what}: {shown:?} does not show {part}"));
            }
        }
        if shown.contains('\n') || !shown.starts_with(char::is_lowercase) {
            failures.push(format!("{what}: {shown:?} is not one lowercase line"));
        }
        if shown.ends_with(['.', '!', ':', ';']) {
            failures.push(format!("{what}: {shown:?} ends in punctuation"));
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} problems:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn sacm23_xmi_014_repeated_value_display_is_pinned() {
    // Counted by hand: line 3 is `  <name content="A">`, so `content` is at
    // column 9; line 4 is `    <content>A</content>`, so `<` is at column 5.
    let input = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n  \
         <name content=\"A\">\n    \
         <content>A</content>\n  \
         </name>\n\
         </argumentElement></sacm:ArgumentPackage>"
    );
    let error = refused("the pinned case", &input);
    assert_eq!(
        error.locations(),
        [loc(3, 9), loc(4, 5)],
        "{ROW}: the pinned case's locations, counted by hand"
    );
    assert_eq!(
        error.to_string(),
        "the single-valued property \"name.content\" of the element \"c1\" is given 2 times, \
         at line 3, column 9; line 4, column 5",
        "{ROW}: the pinned Display text"
    );
}

/// Compiles only if `T` is a shareable error with the designed derives.
fn assert_error_type<T: Error + Clone + Eq + Debug + Display + Send + Sync + 'static>() {}

#[test]
fn sacm23_xmi_014_repeated_value_boxes_and_downcasts_as_a_send_sync_error() {
    assert_error_type::<ReadError>();
    let (what, template, element, property) = repeats().swap_remove(0);
    let error = assert_repeated(what, &template, element, property);
    let boxed: Box<dyn Error + Send + Sync + 'static> = Box::new(error.clone());
    assert_eq!(
        boxed.to_string(),
        error.to_string(),
        "{ROW}: the boxed refusal shows another message"
    );
    assert!(
        boxed.source().is_none(),
        "{ROW}: RepeatedValue wraps a cause"
    );
    assert_eq!(
        boxed.downcast_ref::<ReadError>(),
        Some(&error),
        "{ROW}: a boxed RepeatedValue does not downcast back to itself"
    );
    let moved = thread::spawn(move || boxed.to_string())
        .join()
        .unwrap_or_else(|_| panic!("{ROW}: the boxed refusal could not move to another thread"));
    assert_eq!(moved, error.to_string(), "{ROW}: the moved refusal");
}

// --- Item 7: round trip -------------------------------------------------------------

/// Writes `doc` with the default options: strict, so the document must be
/// valid, as every twin here is (every element is named).
fn write_ok(what: &str, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default()) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: write refused the document: {error} ({error:?})"),
    }
}

#[test]
fn sacm23_xmi_014_each_element_form_input_writes_the_bytes_of_its_attribute_twin_and_reads_back_equal()
 {
    let twins = all_twins();
    let mut failures = Vec::new();
    for twin in &twins {
        for mode in MODES {
            let what = format!("{}, read in {mode:?} mode", twin.what);
            let element = read_ok_in(&what, mode, &twin.element);
            let attribute = read_ok_in(&what, mode, &twin.attribute);
            let text = write_ok(&what, &element);
            let twin_text = write_ok(&what, &attribute);
            if text != twin_text {
                failures.push(format!(
                    "{what}: writes other bytes than its attribute twin\n--- element form ---\n\
                     {text}--- attribute form ---\n{twin_text}"
                ));
                continue;
            }
            if text.contains("<lang")
                || text.contains("<content")
                || text.contains("<assertionDeclaration")
            {
                failures.push(format!("{what}: the output has a value element\n{text}"));
            }
            for back_mode in MODES {
                let back = read_ok_in(&what, back_mode, &text);
                let differences = semantic_compare(&back, &element);
                if !differences.is_empty() {
                    failures.push(format!(
                        "{what}: reads back in {back_mode:?} mode with differences {differences:?}"
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} of {} twins in two modes do not round-trip:\n{}",
        failures.len(),
        twins.len(),
        failures.join("\n")
    );
}
