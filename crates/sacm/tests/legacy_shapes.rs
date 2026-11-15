//! Tests for the name and text shapes of the assurance-forge C++ library
//! (LIB-COMPAT-008; XMI 2.5.1 §7.8.6; formal/23-05-08 §8.5, §8.6; D-PRESERVE;
//! D-STRICT; ADR 0012; ADR 0022 amendment 1; ADR 0025).
//!
//! A tolerant `xmi::read` reads each shape that maps one to one onto a
//! `LangString` or a `MultiLangString` as its standard form, with one
//! SACM-XMI-012 Warning per occurrence; a strict read refuses it with
//! SACM-XMI-012; both saves write only the standard form. The shapes:
//! - `name="…"` on a `ModelElement`;
//! - text as the `content` of a `name` or a `value`;
//! - a `name` holding one `value`, or one `content` carrying `lang`;
//! - the single-language shorthand of a `content` or `key` role element:
//!   `lang`/`content` attributes or text, with no `value`;
//! - `content=` (with an optional `lang=`) or text directly on a
//!   `UtilityElement`;
//! - `key=` and `value=` on a `taggedValue`.
//!
//! Each readable shape has a *twin*: the same document in the standard form.
//! "Reads as its standard form" is `==` to the twin read in strict mode with
//! no diagnostic, so absent and `""` differ and nothing is trimmed.
//!
//! Assumptions this file makes about the design, which the lead confirms:
//! - The code is `Code::XMI_LEGACY_SHAPE` (SACM-XMI-012), named like
//!   `XMI_XSI_TYPE` after what it reports.
//! - A strict refusal is a coded `ReadError` variant of its own (ADR 0015
//!   amendment 2, like `RepeatedValue` and `InvalidLiteral`). Its variant
//!   name is not pinned: the tests use `code()`, `location()` and
//!   `locations()`, and that it is not `Unsupported`.
//! - The Warning's requirement id is `LIB-COMPAT-008`, as SACM-XMI-010's is
//!   `LIB-COMPAT-001`. Its clause and message are not pinned.
//! - The location of a shape, for its Warning and for its strict refusal, is
//!   where today's `Unsupported` refusal of it is: the shape's first
//!   attribute at its name; otherwise its first text character that is not
//!   literal white space (a CDATA section or a reference at its start);
//!   otherwise, for a `value` in a `name`, the `value`'s `<`.
//! - `key=` and `value=` on one `taggedValue` are two occurrences, so two
//!   Warnings.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::model::{Document, ElementId, LangString, MultiLangString, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "LIB-COMPAT-008";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a Warning, a refusal or an occurrence is
/// expected. Removed before reading.
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

/// A Claim `c1` with the start-tag `attributes`, holding `body`, in [`ap`].
fn claim(attributes: &str, body: &str) -> String {
    ap(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"{attributes}>{body}</argumentElement>"
    ))
}

/// A Claim `c1` holding `name` as its only content, in [`ap`].
fn named(name: &str) -> String {
    claim("", name)
}

/// A Description `d` holding `body`, in [`ap`].
fn description(body: &str) -> String {
    ap(&format!("<description xmi:id=\"d\">{body}</description>"))
}

/// A `TaggedValue` `t` holding `body`, in [`ap`].
fn tagged(body: &str) -> String {
    ap(&format!("<taggedValue xmi:id=\"t\">{body}</taggedValue>"))
}

/// The standard `key` of the `TaggedValue`s here.
const KEY: &str = "<key><value content=\"k\"/></key>";

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

fn mls(values: &[LangString]) -> MultiLangString {
    values.iter().cloned().collect()
}

// --- Reading and writing -------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in `mode`, which the test expects to be read.
fn read_in(what: &str, mode: Mode, input: &str) -> ReadOutput {
    match read(mode, input) {
        Ok(output) => output,
        Err(error) => {
            panic!("{ROW}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
        }
    }
}

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to `==` documents, and returns the strict one.
fn read_plain(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        let output = read_in(what, mode, input);
        assert_eq!(
            output.diagnostics(),
            [],
            "{ROW}: {what}: a {mode:?} read of {input:?} reported diagnostics"
        );
        documents.push(output.into_document());
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

/// Writes `doc` in `mode`, which the test expects to succeed.
fn write_in(what: &str, mode: Mode, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused: {error:?} ({error})"),
    }
}

fn element<'a>(what: &str, doc: &'a Document, id: &str) -> SacmElement<'a> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: {what}: the document has no element {id:?}");
    };
    found
}

/// Whether `text` holds `expected` as consecutive lines, each compared with
/// its indentation trimmed.
fn has_lines(text: &str, expected: &[&str]) -> bool {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    lines
        .windows(expected.len())
        .any(|window| window == expected)
}

// --- Findings ------------------------------------------------------------------

/// A diagnostic without its message and clause.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Finding {
    code: Code,
    severity: Severity,
    requirement: String,
    affected: Vec<String>,
    location: Option<SourceLocation>,
}

fn finding(diagnostic: &Diagnostic) -> Finding {
    Finding {
        code: diagnostic.code(),
        severity: diagnostic.severity(),
        requirement: diagnostic.requirement_id().to_owned(),
        affected: diagnostic
            .affected()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
        location: diagnostic.location(),
    }
}

/// The SACM-XMI-012 Warning of a shape at `at`, affecting `element`.
fn legacy_warning(element: &str, at: SourceLocation) -> Finding {
    Finding {
        code: Code::XMI_LEGACY_SHAPE,
        severity: Severity::Warning,
        requirement: ROW.to_owned(),
        affected: vec![element.to_owned()],
        location: Some(at),
    }
}

// --- The readable shapes ---------------------------------------------------------

/// One readable shape: its input, with one mark per occurrence (each one
/// Warning, the first also the strict refusal), the element each Warning
/// affects, and its standard-form twin.
struct Shape {
    what: &'static str,
    template: String,
    affected: &'static [&'static str],
    twin: String,
}

fn shape(
    what: &'static str,
    template: impl Into<String>,
    affected: &'static [&'static str],
    twin: impl Into<String>,
) -> Shape {
    Shape {
        what,
        template: template.into(),
        affected,
        twin: twin.into(),
    }
}

/// The `name=` attribute (acceptance item 1).
fn name_attribute_shapes() -> Vec<Shape> {
    vec![
        shape(
            "name= on a Claim",
            claim(&format!(" {MARK}name=\"X\""), ""),
            &["c1"],
            named("<name content=\"X\"/>"),
        ),
        shape(
            "name=\"\" on a Claim",
            claim(&format!(" {MARK}name=\"\""), ""),
            &["c1"],
            named("<name content=\"\"/>"),
        ),
        shape(
            "name= with white space around its value",
            claim(&format!(" {MARK}name=\" X \""), ""),
            &["c1"],
            named("<name content=\" X \"/>"),
        ),
        shape(
            "name= with references",
            claim(&format!(" {MARK}name=\"a&amp;b&#x41;\""), ""),
            &["c1"],
            named("<name content=\"a&amp;bA\"/>"),
        ),
        shape(
            "name= on a Claim holding a Note",
            claim(&format!(" {MARK}name=\"X\""), "<note xmi:id=\"n\"/>"),
            &["c1"],
            claim("", "<name content=\"X\"/><note xmi:id=\"n\"/>"),
        ),
        shape(
            "name= on a root",
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"p\"\n    {MARK}name=\"P\"/>"),
            &["p"],
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/></sacm:ArgumentPackage>"
            ),
        ),
        shape(
            "name= on an argumentPackage child",
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\n  \
                 <argumentPackage xmi:id=\"a\" {MARK}name=\"A\"/></sacm:AssuranceCasePackage>"
            ),
            &["a"],
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
                 <argumentPackage xmi:id=\"a\"><name content=\"A\"/></argumentPackage>\
                 </sacm:AssuranceCasePackage>"
            ),
        ),
    ]
}

/// Text as the content of a `name` or a `value` (acceptance item 2).
fn text_shapes() -> Vec<Shape> {
    vec![
        shape(
            "text in a name",
            named(&format!("<name>{MARK}X</name>")),
            &["c1"],
            named("<name content=\"X\"/>"),
        ),
        shape(
            "text with white space around it in a name with lang",
            named(&format!("<name lang=\"en\"> {MARK}X </name>")),
            &["c1"],
            named("<name lang=\"en\" content=\" X \"/>"),
        ),
        shape(
            "text over two lines in a name",
            named(&format!("<name>\n    {MARK}X\n  </name>")),
            &["c1"],
            named("<name content=\"&#10;    X&#10;  \"/>"),
        ),
        shape(
            "a CDATA section in a name",
            named(&format!("<name>{MARK}<![CDATA[a<b]]></name>")),
            &["c1"],
            named("<name content=\"a&lt;b\"/>"),
        ),
        shape(
            "references in a name",
            named(&format!("<name>{MARK}&#x41;&amp;&#x20;</name>")),
            &["c1"],
            named("<name content=\"A&amp; \"/>"),
        ),
        shape(
            "a space reference after a literal space in a name",
            named(&format!("<name> {MARK}&#x20;x</name>")),
            &["c1"],
            named("<name content=\"  x\"/>"),
        ),
        shape(
            "text, CDATA and a reference in a name",
            named(&format!("<name>{MARK}a<![CDATA[b]]>&#x63;</name>")),
            &["c1"],
            named("<name content=\"abc\"/>"),
        ),
        shape(
            "text after a comment in a name",
            named(&format!("<name><!--c-->{MARK}x</name>")),
            &["c1"],
            named("<name content=\"x\"/>"),
        ),
        shape(
            "text in a name with an xmi:id",
            named(&format!("<name xmi:id=\"n1\">{MARK}X</name>")),
            &["c1"],
            named("<name xmi:id=\"n1\" content=\"X\"/>"),
        ),
        shape(
            "text in a root's name",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name>{MARK}P</name>\n</sacm:ArgumentPackage>"
            ),
            &["p"],
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/></sacm:ArgumentPackage>"
            ),
        ),
        shape(
            "text in a value of a content",
            description(&format!(
                "<content><value lang=\"en\">{MARK}x</value></content>"
            )),
            &["d"],
            description("<content><value lang=\"en\" content=\"x\"/></content>"),
        ),
        shape(
            "text in a value of a key",
            tagged(&format!("<key><value>{MARK}k</value></key>")),
            &["t"],
            tagged(KEY),
        ),
    ]
}

/// A `name` holding one `value`, or one `content` carrying `lang`
/// (acceptance item 4).
fn one_entry_name_shapes() -> Vec<Shape> {
    vec![
        shape(
            "a name holding one value",
            named(&format!(
                "<name>{MARK}<value lang=\"en\" content=\"X\"/></name>"
            )),
            &["c1"],
            named("<name lang=\"en\" content=\"X\"/>"),
        ),
        shape(
            "a name holding one value with white space around it",
            named(&format!(
                "<name>\n    {MARK}<value lang=\"en\" content=\"X\"/>\n  </name>"
            )),
            &["c1"],
            named("<name lang=\"en\" content=\"X\"/>"),
        ),
        shape(
            "a name with an xmi:id holding one value",
            named(&format!(
                "<name xmi:id=\"n1\">{MARK}<value content=\"X\"/></name>"
            )),
            &["c1"],
            named("<name xmi:id=\"n1\" content=\"X\"/>"),
        ),
        shape(
            "a name holding one content with lang",
            named(&format!(
                "<name><content {MARK}lang=\"en\">X</content></name>"
            )),
            &["c1"],
            named("<name lang=\"en\" content=\"X\"/>"),
        ),
    ]
}

/// The single-language shorthand of a `content` or `key` role element
/// (acceptance item 5).
fn role_shorthand_shapes() -> Vec<Shape> {
    vec![
        shape(
            "<content lang>text</content>",
            description(&format!("<content {MARK}lang=\"en\">t</content>")),
            &["d"],
            description("<content><value lang=\"en\" content=\"t\"/></content>"),
        ),
        shape(
            "<content lang content/>",
            description(&format!("<content {MARK}lang=\"en\" content=\"t\"/>")),
            &["d"],
            description("<content><value lang=\"en\" content=\"t\"/></content>"),
        ),
        shape(
            "<content content/>",
            description(&format!("<content {MARK}content=\"t\"/>")),
            &["d"],
            description("<content><value content=\"t\"/></content>"),
        ),
        shape(
            "<content content=\"\"/>",
            description(&format!("<content {MARK}content=\"\"/>")),
            &["d"],
            description("<content><value content=\"\"/></content>"),
        ),
        shape(
            "<content>text</content>",
            description(&format!("<content>{MARK}t</content>")),
            &["d"],
            description("<content><value content=\"t\"/></content>"),
        ),
        shape(
            "<content> text </content>, untrimmed",
            description(&format!("<content> {MARK}t </content>")),
            &["d"],
            description("<content><value content=\" t \"/></content>"),
        ),
        shape(
            "a content with an xmi:id and the shorthand",
            description(&format!(
                "<content xmi:id=\"c9\" {MARK}lang=\"en\">t</content>"
            )),
            &["d"],
            description("<content xmi:id=\"c9\"><value lang=\"en\" content=\"t\"/></content>"),
        ),
        shape(
            "<key>text</key>",
            tagged(&format!("<key>{MARK}k</key>")),
            &["t"],
            tagged(KEY),
        ),
        shape(
            "<key lang content/>",
            tagged(&format!("<key {MARK}lang=\"en\" content=\"k\"/>")),
            &["t"],
            tagged("<key><value lang=\"en\" content=\"k\"/></key>"),
        ),
        shape(
            "a key with an xmi:id and text",
            tagged(&format!("<key xmi:id=\"k9\">{MARK}k</key>")),
            &["t"],
            tagged("<key xmi:id=\"k9\"><value content=\"k\"/></key>"),
        ),
    ]
}

/// `content=` (with an optional `lang=`) or text directly on a
/// `UtilityElement` (acceptance item 6).
fn utility_element_shapes() -> Vec<Shape> {
    vec![
        shape(
            "<description content/>",
            ap(&format!("<description xmi:id=\"d\" {MARK}content=\"x\"/>")),
            &["d"],
            description("<content><value content=\"x\"/></content>"),
        ),
        shape(
            "<description content=\"\"/>",
            ap(&format!("<description xmi:id=\"d\" {MARK}content=\"\"/>")),
            &["d"],
            description("<content><value content=\"\"/></content>"),
        ),
        shape(
            "<note lang>text</note>",
            ap(&format!("<note xmi:id=\"n\" {MARK}lang=\"en\">x</note>")),
            &["n"],
            ap("<note xmi:id=\"n\"><content><value lang=\"en\" content=\"x\"/></content></note>"),
        ),
        shape(
            "<implementationConstraint lang content/> on an abstract Claim",
            // An implementation constraint is only valid on an abstract
            // element (SACM23-VAL-021), so the strict write accepts it.
            claim(
                " isAbstract=\"true\"",
                &format!(
                    "<name content=\"C\"/>\
                     <implementationConstraint xmi:id=\"i\" {MARK}lang=\"en\" content=\"x\"/>"
                ),
            ),
            &["i"],
            claim(
                " isAbstract=\"true\"",
                "<name content=\"C\"/><implementationConstraint xmi:id=\"i\"><content>\
                 <value lang=\"en\" content=\"x\"/></content></implementationConstraint>",
            ),
        ),
        shape(
            "<description>text</description>",
            ap(&format!("<description xmi:id=\"d\">{MARK}x</description>")),
            &["d"],
            description("<content><value content=\"x\"/></content>"),
        ),
        shape(
            "CDATA with literal spaces around it in a Note, untrimmed",
            ap(&format!("<note xmi:id=\"n\"> {MARK}<![CDATA[x]]> </note>")),
            &["n"],
            ap("<note xmi:id=\"n\"><content><value content=\" x \"/></content></note>"),
        ),
        shape(
            "<note content/> on a Claim",
            claim(
                "",
                &format!("<name content=\"C\"/><note xmi:id=\"n\" {MARK}content=\"x\"/>"),
            ),
            &["n"],
            claim(
                "",
                "<name content=\"C\"/><note xmi:id=\"n\"><content><value content=\"x\"/></content></note>",
            ),
        ),
    ]
}

/// `key=` and `value=` on a `taggedValue` (acceptance item 7).
fn tagged_value_shapes() -> Vec<Shape> {
    vec![
        shape(
            "key= and value= on a taggedValue",
            ap(&format!(
                "<taggedValue xmi:id=\"t\" {MARK}key=\"k\" {MARK}value=\"v\"/>"
            )),
            &["t", "t"],
            tagged(&format!("{KEY}<content><value content=\"v\"/></content>")),
        ),
        shape(
            "key= on a taggedValue",
            ap(&format!("<taggedValue xmi:id=\"t\" {MARK}key=\"k\"/>")),
            &["t"],
            tagged(KEY),
        ),
        shape(
            "key=\"\" on a taggedValue",
            ap(&format!("<taggedValue xmi:id=\"t\" {MARK}key=\"\"/>")),
            &["t"],
            tagged("<key><value content=\"\"/></key>"),
        ),
        shape(
            "value= on a taggedValue with a key",
            ap(&format!(
                "<taggedValue xmi:id=\"t\" {MARK}value=\"v\">{KEY}</taggedValue>"
            )),
            &["t"],
            tagged(&format!("{KEY}<content><value content=\"v\"/></content>")),
        ),
    ]
}

/// Two shapes in one document. In the last two, the shape the reader finds
/// first is the later one in the text, so the strict refusal must still be
/// the first in the text.
fn combined_shapes() -> Vec<Shape> {
    vec![
        shape(
            "name= on a Claim and content= on its Description",
            claim(
                &format!(" {MARK}name=\"X\""),
                &format!("<description xmi:id=\"d\" {MARK}content=\"x\"/>"),
            ),
            &["c1", "d"],
            claim(
                "",
                "<name content=\"X\"/><description xmi:id=\"d\"><content><value content=\"x\"/>\
                 </content></description>",
            ),
        ),
        shape(
            "value= on a taggedValue and its key as text",
            ap(&format!(
                "<taggedValue xmi:id=\"t\" {MARK}value=\"v\"><key>{MARK}k</key></taggedValue>"
            )),
            &["t", "t"],
            tagged(&format!("{KEY}<content><value content=\"v\"/></content>")),
        ),
        shape(
            "a name holding one value with its content as text",
            named(&format!(
                "<name>{MARK}<value lang=\"en\">{MARK}X</value></name>"
            )),
            &["c1", "c1"],
            named("<name lang=\"en\" content=\"X\"/>"),
        ),
    ]
}

fn all_shapes() -> Vec<Shape> {
    [
        name_attribute_shapes(),
        text_shapes(),
        one_entry_name_shapes(),
        role_shorthand_shapes(),
        utility_element_shapes(),
        tagged_value_shapes(),
        combined_shapes(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Reads `shape` in tolerant mode and asserts that it gives one Warning per
/// mark and its twin. Returns the document.
fn assert_reads_as_twin(shape: &Shape) -> Document {
    let what = shape.what;
    let (input, marks) = marked(&shape.template);
    assert_eq!(
        marks.len(),
        shape.affected.len(),
        "{ROW}: setup: {what} has {} marks for {} affected elements",
        marks.len(),
        shape.affected.len()
    );
    assert!(!marks.is_empty(), "{ROW}: setup: {what} has no mark");
    let output = read_in(what, Mode::Tolerant, &input);
    let expected: Vec<Finding> = shape
        .affected
        .iter()
        .zip(&marks)
        .map(|(element, at)| legacy_warning(element, *at))
        .collect();
    let found: Vec<Finding> = output.diagnostics().iter().map(finding).collect();
    assert_eq!(
        found,
        expected,
        "{ROW}: {what}: the tolerant read does not report one SACM-XMI-012 Warning per \
         occurrence: {:?}",
        output.diagnostics()
    );
    let twin = read_plain(&format!("{what}: the twin"), &shape.twin);
    assert_eq!(
        semantic_compare(output.document(), &twin),
        [],
        "{ROW}: {what}: the tolerant read is not its standard form"
    );
    assert_eq!(
        output.document(),
        &twin,
        "{ROW}: {what}: the tolerant read is not == its standard form"
    );
    output.into_document()
}

// --- Acceptance item 1: name= ----------------------------------------------------------

#[test]
fn lib_compat_008_a_name_attribute_reads_as_name_content_with_no_lang_and_one_warning() {
    for shape in name_attribute_shapes() {
        assert_reads_as_twin(&shape);
    }
    let doc = assert_reads_as_twin(&name_attribute_shapes()[0]);
    assert_eq!(
        element("name=", &doc, "c1").name(),
        Some(&ls(None, Some("X"))),
        "{ROW}: name=\"X\" is not lang None and content Some(\"X\")"
    );
    let doc = assert_reads_as_twin(&name_attribute_shapes()[1]);
    assert_eq!(
        element("name=\"\"", &doc, "c1").name(),
        Some(&ls(None, Some(""))),
        "{ROW}: name=\"\" is not content Some(\"\")"
    );
}

#[test]
fn lib_compat_008_a_name_attribute_is_written_as_a_name_element_that_strict_reads_with_no_diagnostic()
 {
    let doc = assert_reads_as_twin(&name_attribute_shapes()[0]);
    for mode in MODES {
        let text = write_in("name=", mode, &doc);
        assert!(
            has_lines(
                &text,
                &[
                    "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">",
                    "<name content=\"X\" />",
                    "</argumentElement>",
                ]
            ),
            "{ROW}: name=: a {mode:?} write does not write <name content=\"X\" />:\n{text}"
        );
        assert!(
            !text.contains(" name=\""),
            "{ROW}: name=: a {mode:?} write writes a name attribute:\n{text}"
        );
        let back = read_in("name=: written", Mode::Strict, &text);
        assert_eq!(
            back.diagnostics(),
            [],
            "{ROW}: name=: the {mode:?} output reads in strict mode with diagnostics"
        );
        assert_eq!(
            back.document(),
            &doc,
            "{ROW}: name=: the {mode:?} output does not read back =="
        );
    }
}

#[test]
fn lib_compat_008_a_name_attribute_on_a_utility_element_is_not_read_and_is_preserved() {
    // A UtilityElement is no ModelElement and has no name (§8.7).
    for (what, template) in [
        (
            "name= on a Description",
            ap(&format!("<description xmi:id=\"d\" {MARK}name=\"x\"/>")),
        ),
        (
            "name= on a TaggedValue",
            ap(&format!(
                "<taggedValue xmi:id=\"t\" {MARK}name=\"x\">{KEY}</taggedValue>"
            )),
        ),
    ] {
        assert_preserved_at_mark(what, &template);
    }
}

// --- Acceptance item 2: text in a name -------------------------------------------------

#[test]
fn lib_compat_008_text_in_a_name_is_its_content_exactly_with_one_warning() {
    for shape in text_shapes() {
        assert_reads_as_twin(&shape);
    }
    let cases: [(usize, LangString); 7] = [
        (0, ls(None, Some("X"))),
        (1, ls(Some("en"), Some(" X "))),
        (3, ls(None, Some("a<b"))),
        (4, ls(None, Some("A& "))),
        (5, ls(None, Some("  x"))),
        (6, ls(None, Some("abc"))),
        (8, ls(None, Some("X"))),
    ];
    let shapes = text_shapes();
    for (index, expected) in cases {
        let shape = &shapes[index];
        let doc = assert_reads_as_twin(shape);
        assert_eq!(
            element(shape.what, &doc, "c1").name(),
            Some(&expected),
            "{ROW}: {}: the name as read",
            shape.what
        );
    }
    let doc = assert_reads_as_twin(&shapes[8]);
    assert_eq!(
        element("a name id", &doc, "c1")
            .name_id()
            .map(ElementId::as_str),
        Some("n1"),
        "{ROW}: the xmi:id of a name holding text is not kept"
    );
}

#[test]
fn lib_compat_008_white_space_only_text_is_no_shorthand() {
    for (what, input, plain) in [
        (
            "white space in a name with content",
            named("<name content=\"A\">\n    \n  </name>"),
            named("<name content=\"A\"/>"),
        ),
        (
            "white space in an empty name",
            named("<name> \t </name>"),
            named("<name/>"),
        ),
        (
            "white space around a comment in a name",
            named("<name lang=\"en\"> <!--c--> </name>"),
            named("<name lang=\"en\"/>"),
        ),
        (
            "white space in a Description",
            ap("<description xmi:id=\"d\">\n  \n  </description>"),
            ap("<description xmi:id=\"d\"/>"),
        ),
        (
            "white space in a value",
            description("<content><value lang=\"en\"> </value></content>"),
            description("<content><value lang=\"en\"/></content>"),
        ),
    ] {
        let doc = read_plain(what, &input);
        let reference = read_plain(&format!("{what}: plain"), &plain);
        assert_eq!(
            doc, reference,
            "{ROW}: {what}: white space is read as content"
        );
    }
}

// --- Acceptance item 3: given twice ----------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one flat table is easier to review than several"
)]
fn lib_compat_008_a_value_given_by_a_shorthand_and_the_standard_form_is_repeated_value_in_both_modes()
 {
    let cases = [
        (
            "content= and text in a name",
            named(&format!("<name {MARK}content=\"a\">{MARK}b</name>")),
            "c1",
            "name.content",
        ),
        (
            "a content element and text in a name",
            named(&format!("<name>{MARK}<content>a</content>{MARK}b</name>")),
            "c1",
            "name.content",
        ),
        (
            "text and a content element in a name",
            named(&format!("<name>{MARK}b{MARK}<content>a</content></name>")),
            "c1",
            "name.content",
        ),
        (
            "name= and a name element",
            claim(
                &format!(" {MARK}name=\"a\""),
                &format!("{MARK}<name content=\"b\"/>"),
            ),
            "c1",
            "name",
        ),
        (
            "name= and an empty name element",
            claim(&format!(" {MARK}name=\"a\""), &format!("{MARK}<name/>")),
            "c1",
            "name",
        ),
        (
            "name= and a name element on a root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\" {MARK}name=\"P\">\n  {MARK}<name content=\"P\"/>\n\
                 </sacm:ArgumentPackage>"
            ),
            "p",
            "name",
        ),
        (
            "lang= on a name and lang= on its content",
            named(&format!(
                "<name {MARK}lang=\"en\"><content {MARK}lang=\"de\">x</content></name>"
            )),
            "c1",
            "name.lang",
        ),
        (
            "lang= on a name and the same lang= on its content",
            named(&format!(
                "<name {MARK}lang=\"en\"><content {MARK}lang=\"en\">x</content></name>"
            )),
            "c1",
            "name.lang",
        ),
        (
            "content= on a Description and a content element",
            ap(&format!(
                "<description xmi:id=\"d\" {MARK}content=\"x\">{MARK}<content/></description>"
            )),
            "d",
            "content",
        ),
        (
            "content=\"\" on a Description and a content element",
            ap(&format!(
                "<description xmi:id=\"d\" {MARK}content=\"\">\
                 {MARK}<content><value content=\"y\"/></content></description>"
            )),
            "d",
            "content",
        ),
        (
            "lang= and text on a Note and a content element",
            ap(&format!(
                "<note xmi:id=\"n\" {MARK}lang=\"en\">x{MARK}<content><value content=\"y\"/>\
                 </content></note>"
            )),
            "n",
            "content",
        ),
        (
            "text in a Description and a content element",
            ap(&format!(
                "<description xmi:id=\"d\">{MARK}x{MARK}<content/></description>"
            )),
            "d",
            "content",
        ),
        (
            "key= and a key element",
            ap(&format!(
                "<taggedValue xmi:id=\"t\" {MARK}key=\"k\">{MARK}{KEY}</taggedValue>"
            )),
            "t",
            "key",
        ),
        (
            "value= and a content element on a taggedValue",
            ap(&format!(
                "<taggedValue xmi:id=\"t\" {MARK}value=\"v\">{KEY}\
                 {MARK}<content><value content=\"v\"/></content></taggedValue>"
            )),
            "t",
            "content",
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
            (error.code(), error.locations(), error.location()),
            (
                Some(Code::MULTIPLICITY_VIOLATION),
                marks.as_slice(),
                Some(marks[1])
            ),
            "{ROW}: {what}: not SACM-MULT-001 listing every occurrence"
        );
    }
}

#[test]
fn lib_compat_008_a_second_name_given_as_text_is_a_repeated_name_in_tolerant_mode() {
    // The second name is no longer malformed in tolerant mode (SACM23-XMI-021
    // checks a later name in full first), so the repeat is what is refused.
    let (input, marks) = marked(&named(&format!(
        "{MARK}<name content=\"A\"/>{MARK}<name>x</name>"
    )));
    let Err(error) = read(Mode::Tolerant, &input) else {
        panic!("{ROW}: a second name given as text was read");
    };
    assert!(
        matches!(&error, ReadError::RepeatedValue { element, property, .. }
            if element.as_str() == "c1" && property == "name"),
        "{ROW}: a second name given as text is not RepeatedValue for name: {error:?}"
    );
    assert_eq!(
        error.locations(),
        marks.as_slice(),
        "{ROW}: a second name given as text: the locations"
    );
    assert!(
        read(Mode::Strict, &input).is_err(),
        "{ROW}: a second name given as text was read in strict mode"
    );
}

// --- Acceptance item 4: a name holding one entry ------------------------------------------

#[test]
fn lib_compat_008_a_name_holding_one_value_or_one_content_with_lang_reads_as_that_name() {
    for shape in one_entry_name_shapes() {
        let doc = assert_reads_as_twin(&shape);
        let expected = if shape.what.contains("an xmi:id") {
            ls(None, Some("X"))
        } else {
            ls(Some("en"), Some("X"))
        };
        assert_eq!(
            element(shape.what, &doc, "c1").name(),
            Some(&expected),
            "{ROW}: {}: the name as read",
            shape.what
        );
    }
}

#[test]
fn lib_compat_008_a_name_holding_two_entries_is_preserved_whole() {
    // In a SACM 2.3 file a `name` is one LangString: two entries, or a
    // `value` with anything beside it, do not conform. The whole name is kept
    // as preserved content of c1, at its start tag (ADR 0033); LIB-COMPAT-012
    // (M6) is to read the multi-entry form.
    for (what, template) in [
        (
            "two values in a name",
            named(&format!(
                "{MARK}<name><value lang=\"en\" content=\"X\"/><value lang=\"de\" content=\"Y\"/></name>"
            )),
        ),
        (
            "two contents with lang in a name",
            named(&format!(
                "{MARK}<name><content lang=\"en\">X</content><content lang=\"de\">Y</content></name>"
            )),
        ),
        (
            "a value and a content with lang in a name",
            named(&format!(
                "{MARK}<name><value lang=\"en\" content=\"X\"/><content lang=\"de\">Y</content></name>"
            )),
        ),
        (
            "a value and text after it in a name",
            named(&format!(
                "{MARK}<name><value lang=\"en\" content=\"X\"/>y</name>"
            )),
        ),
        (
            "text and a value after it in a name",
            named(&format!(
                "{MARK}<name>y<value lang=\"en\" content=\"X\"/></name>"
            )),
        ),
        (
            "a value in a name with a lang attribute",
            named(&format!(
                "{MARK}<name lang=\"en\"><value content=\"X\"/></name>"
            )),
        ),
        (
            "a value in a name with a content attribute",
            named(&format!(
                "{MARK}<name content=\"X\"><value lang=\"en\"/></name>"
            )),
        ),
        (
            "a value and a lang element in a name",
            named(&format!(
                "{MARK}<name><value content=\"X\"/><lang>en</lang></name>"
            )),
        ),
    ] {
        assert_preserved_at_mark(what, &template);
    }
}

#[test]
fn lib_compat_008_a_repeated_name_or_value_field_lists_exactly_its_occurrences() {
    // A field given twice is refused with every occurrence of that field
    // and of no other: text counts once, for `content`; a `content` carrying
    // `lang`, or a `value`, in a name counts for the field it gives.
    let cases = [
        (
            "lang twice beside text",
            named(&format!(
                "<name {MARK}lang=\"en\">{MARK}<lang>de</lang>X</name>"
            )),
            "name.lang",
        ),
        (
            "content= and text split by a comment",
            named(&format!(
                "<name {MARK}content=\"a\">{MARK}b<!--c-->d</name>"
            )),
            "name.content",
        ),
        (
            "content= and a content carrying lang",
            named(&format!(
                "<name {MARK}content=\"a\">{MARK}<content lang=\"en\">b</content></name>"
            )),
            "name.content",
        ),
        (
            "lang twice in a value beside a content carrying lang",
            description(&format!(
                "<content><value {MARK}lang=\"en\">{MARK}<lang>de</lang>\
                 <content lang=\"fr\">x</content></value></content>"
            )),
            "content.value.lang",
        ),
        (
            "lang twice beside a value giving only content",
            named(&format!(
                "<name {MARK}lang=\"en\">{MARK}<lang>de</lang><value content=\"x\"/></name>"
            )),
            "name.lang",
        ),
        (
            "lang twice beside a value giving lang as an attribute",
            named(&format!(
                "<name {MARK}lang=\"en\">{MARK}<lang>de</lang>{MARK}<value lang=\"fr\"/></name>"
            )),
            "name.lang",
        ),
        (
            "lang twice beside a value giving lang as an element",
            named(&format!(
                "<name {MARK}lang=\"en\">{MARK}<lang>de</lang>{MARK}<value><lang>fr</lang></value></name>"
            )),
            "name.lang",
        ),
        (
            "lang twice beside a value giving content as text",
            named(&format!(
                "<name {MARK}lang=\"en\">{MARK}<lang>de</lang><value>x</value></name>"
            )),
            "name.lang",
        ),
        (
            "content twice beside a value giving content as text",
            named(&format!(
                "<name {MARK}content=\"a\">{MARK}<content>b</content>{MARK}<value>x</value></name>"
            )),
            "name.content",
        ),
        (
            "content twice beside a value giving only lang as an element",
            named(&format!(
                "<name {MARK}content=\"a\">{MARK}<content>b</content><value><lang>fr</lang></value></name>"
            )),
            "name.content",
        ),
    ];
    for (what, template, expected) in &cases {
        let (input, marks) = marked(template);
        let error = refused(what, &input);
        assert!(
            matches!(&error, ReadError::RepeatedValue { property, .. } if property == expected),
            "{ROW}: {what}: not RepeatedValue for {expected}: {error:?} ({error})"
        );
        assert_eq!(
            error.locations(),
            marks.as_slice(),
            "{ROW}: {what}: the locations are not exactly the occurrences of the field"
        );
    }
}

// --- Acceptance item 5: the role-element shorthand -----------------------------------------

#[test]
fn lib_compat_008_the_single_language_shorthand_of_content_and_key_reads_as_one_value() {
    for shape in role_shorthand_shapes() {
        assert_reads_as_twin(&shape);
    }
    let shapes = role_shorthand_shapes();
    for (index, expected) in [
        (0, ls(Some("en"), Some("t"))),
        (1, ls(Some("en"), Some("t"))),
        (2, ls(None, Some("t"))),
        (3, ls(None, Some(""))),
        (4, ls(None, Some("t"))),
        (5, ls(None, Some(" t "))),
    ] {
        let shape = &shapes[index];
        let doc = assert_reads_as_twin(shape);
        assert_eq!(
            element(shape.what, &doc, "d").content(),
            Some(&mls(&[expected])),
            "{ROW}: {}: the content as read",
            shape.what
        );
    }
    let doc = assert_reads_as_twin(&shapes[6]);
    let d = element("a content id", &doc, "d");
    assert_eq!(
        (d.content_id().map(ElementId::as_str), d.content_value_ids()),
        (Some("c9"), &[None][..]),
        "{ROW}: the content's xmi:id is not kept as its content id"
    );
    let doc = assert_reads_as_twin(&shapes[7]);
    assert_eq!(
        element("<key>k</key>", &doc, "t").key(),
        Some(&mls(&[ls(None, Some("k"))])),
        "{ROW}: <key>k</key> is not key [{{None, k}}]"
    );
    let doc = assert_reads_as_twin(&shapes[9]);
    assert_eq!(
        element("a key id", &doc, "t")
            .key_id()
            .map(ElementId::as_str),
        Some("k9"),
        "{ROW}: the key's xmi:id is not kept as its key id"
    );
}

#[test]
fn lib_compat_008_text_beside_value_children_is_preserved_with_its_content_or_key() {
    for (what, template) in [
        (
            "text after a value in content",
            description(&format!("{MARK}<content><value content=\"a\"/>x</content>")),
        ),
        (
            "text after a value in key",
            tagged(&format!("{MARK}<key><value content=\"k\"/>x</key>")),
        ),
    ] {
        assert_preserved_at_mark(what, &template);
    }
}

// --- Acceptance item 6: the UtilityElement shorthand ---------------------------------------

#[test]
fn lib_compat_008_content_or_text_on_a_utility_element_reads_as_its_content() {
    let shapes = utility_element_shapes();
    for shape in &shapes {
        assert_reads_as_twin(shape);
    }
    for (index, id, expected) in [
        (0, "d", ls(None, Some("x"))),
        (1, "d", ls(None, Some(""))),
        (2, "n", ls(Some("en"), Some("x"))),
        (3, "i", ls(Some("en"), Some("x"))),
        (4, "d", ls(None, Some("x"))),
        (5, "n", ls(None, Some(" x "))),
    ] {
        let shape = &shapes[index];
        let doc = assert_reads_as_twin(shape);
        assert_eq!(
            element(shape.what, &doc, id).content(),
            Some(&mls(&[expected])),
            "{ROW}: {}: the content as read",
            shape.what
        );
    }
}

// --- Acceptance item 7: key= and value= -------------------------------------------------------

#[test]
fn lib_compat_008_key_and_value_attributes_on_a_tagged_value_read_as_its_key_and_content() {
    let shapes = tagged_value_shapes();
    for shape in &shapes {
        assert_reads_as_twin(shape);
    }
    let doc = assert_reads_as_twin(&shapes[0]);
    let t = element("key= value=", &doc, "t");
    assert_eq!(
        (t.key().cloned(), t.content().cloned()),
        (
            Some(mls(&[ls(None, Some("k"))])),
            Some(mls(&[ls(None, Some("v"))]))
        ),
        "{ROW}: key=\"k\" value=\"v\" is not key [{{None, k}}] and content [{{None, v}}]"
    );
}

#[test]
fn lib_compat_008_value_or_key_attributes_on_any_other_kind_are_preserved() {
    let claim_with =
        |attribute: &str| claim(&format!(" {MARK}{attribute}"), "<name content=\"C\"/>");
    for (what, template) in [
        (
            "value= on a Description",
            ap(&format!("<description xmi:id=\"d\" {MARK}value=\"v\"/>")),
        ),
        (
            "value= on a Note",
            ap(&format!("<note xmi:id=\"n\" {MARK}value=\"v\"/>")),
        ),
        (
            "value= on an ImplementationConstraint",
            ap(&format!(
                "<implementationConstraint xmi:id=\"i\" {MARK}value=\"v\"/>"
            )),
        ),
        ("value= on a Claim", claim_with("value=\"v\"")),
        (
            "value= on a root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\" {MARK}value=\"v\"><name content=\"P\"/>\
                 </sacm:ArgumentPackage>"
            ),
        ),
        (
            "key= on a Description",
            ap(&format!("<description xmi:id=\"d\" {MARK}key=\"k\"/>")),
        ),
        ("key= on a Claim", claim_with("key=\"k\"")),
    ] {
        assert_preserved_at_mark(what, &template);
    }
}

// --- Acceptance item 8: strict refuses each shape ----------------------------------------------

#[test]
fn lib_compat_008_a_strict_read_refuses_each_shape_with_sacm_xmi_012_at_the_construct() {
    let mut failures = Vec::new();
    for shape in all_shapes() {
        let (input, marks) = marked(&shape.template);
        match read(Mode::Strict, &input) {
            Ok(output) => failures.push(format!(
                "{}: a document of {} elements was returned",
                shape.what,
                output.document().element_count()
            )),
            Err(error) => {
                let is_unsupported = matches!(error, ReadError::Unsupported { .. });
                if is_unsupported
                    || error.code() != Some(Code::XMI_LEGACY_SHAPE)
                    || error.location() != marks.first().copied()
                    || error.locations() != &marks[..1]
                {
                    failures.push(format!(
                        "{}: expected SACM-XMI-012 at {:?}, got {error:?} ({error})",
                        shape.what,
                        marks.first()
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} shapes are not refused in strict mode with SACM-XMI-012 at the construct:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn lib_compat_008_the_strict_refusal_of_a_shape_shows_its_location() {
    let (input, marks) = marked(&claim(&format!(" {MARK}name=\"X\""), ""));
    let Err(error) = read(Mode::Strict, &input) else {
        panic!("{ROW}: name= was read in strict mode");
    };
    assert_eq!(
        error.code(),
        Some(Code::XMI_LEGACY_SHAPE),
        "{ROW}: the strict refusal of name= is not SACM-XMI-012: {error:?}"
    );
    let shown = error.to_string();
    assert!(
        shown.contains(&marks[0].to_string()) && !shown.contains('\n'),
        "{ROW}: the strict refusal {shown:?} is not one line showing {}",
        marks[0]
    );
}

// --- Acceptance item 9: written in the standard form --------------------------------------------

#[test]
fn lib_compat_008_each_tolerant_read_is_written_in_the_standard_form_and_reads_back_strictly_equal()
{
    let mut failures = Vec::new();
    for shape in all_shapes() {
        let doc = assert_reads_as_twin(&shape);
        for mode in MODES {
            let text = write_in(shape.what, mode, &doc);
            match read(Mode::Strict, &text) {
                Ok(back) if back.diagnostics().is_empty() && back.document() == &doc => {}
                Ok(back) => failures.push(format!(
                    "{}: the {mode:?} output reads back with {:?} and differences {:?}:\n{text}",
                    shape.what,
                    back.diagnostics(),
                    semantic_compare(back.document(), &doc)
                )),
                Err(error) => failures.push(format!(
                    "{}: the {mode:?} output is refused in strict mode: {error}\n{text}",
                    shape.what
                )),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} writes are not the standard form:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// --- Acceptance item 10: the catalog and the docs --------------------------------------------------

/// The text of a file under the repository root.
fn repository_file(path: &str) -> String {
    let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path);
    std::fs::read_to_string(&full)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", full.display()))
}

#[test]
fn lib_compat_008_the_catalog_row_of_sacm_xmi_012_is_live_and_names_this_row() {
    assert_eq!(
        Code::XMI_LEGACY_SHAPE.as_str(),
        "SACM-XMI-012",
        "{ROW}: Code::XMI_LEGACY_SHAPE is not SACM-XMI-012"
    );
    let catalog = repository_file("docs/sacm/diagnostics.md");
    let rows: Vec<&str> = catalog
        .lines()
        .filter(|line| line.trim_start().starts_with("| SACM-XMI-012 |"))
        .collect();
    let [row] = rows.as_slice() else {
        panic!(
            "{ROW}: docs/sacm/diagnostics.md has {} rows for SACM-XMI-012, not one",
            rows.len()
        );
    };
    for needle in [
        "`Code::XMI_LEGACY_SHAPE`",
        "LIB-COMPAT-008",
        "Error (strict) / Warning (tolerant)",
    ] {
        assert!(
            row.contains(needle),
            "{ROW}: the catalog row of SACM-XMI-012 does not name {needle}: {row}"
        );
    }
}

#[test]
fn lib_compat_008_the_reader_docs_no_longer_name_this_row_as_pending() {
    for path in [
        "crates/sacm/src/xmi.rs",
        "crates/sacm/src/xmi/roots.rs",
        "crates/sacm/src/xmi/input.rs",
    ] {
        let text = repository_file(path);
        for pending in ["until LIB-COMPAT-008", "LIB-COMPAT-008 is to read"] {
            assert!(
                !text.contains(pending),
                "{ROW}: {path} still says {pending:?}"
            );
        }
    }
}

// --- Controls: text joined across comments and processing instructions, and the refusals ------

/// Asserts that `shape` reads tolerantly as its twin with one Warning per
/// mark, and that a strict read refuses it with SACM-XMI-012 at the first
/// mark.
fn assert_shape_in_both_modes(shape: &Shape) {
    assert_reads_as_twin(shape);
    let (input, marks) = marked(&shape.template);
    let Err(error) = read(Mode::Strict, &input) else {
        panic!("{ROW}: {}: was read in strict mode", shape.what);
    };
    assert_eq!(
        (error.code(), error.location()),
        (Some(Code::XMI_LEGACY_SHAPE), marks.first().copied()),
        "{ROW}: {}: not refused in strict mode with SACM-XMI-012 at the shape: {error:?}",
        shape.what
    );
}

#[test]
fn lib_compat_008_an_xmi_id_on_the_value_of_a_name_is_unsupported_at_it() {
    assert_unsupported_at_mark(
        "an xmi:id on a name's value",
        &named(&format!(
            "<name><value {MARK}xmi:id=\"v\" content=\"X\"/></name>"
        )),
    );
}

#[test]
fn lib_compat_008_text_of_a_utility_element_content_or_key_is_joined_across_comments_and_pis() {
    for shape in [
        shape(
            "text split by a comment on a Description",
            ap(&format!(
                "<description xmi:id=\"d\">{MARK}a<!--c-->b</description>"
            )),
            &["d"],
            description("<content><value content=\"ab\"/></content>"),
        ),
        shape(
            "text split by a processing instruction in content",
            description(&format!("<content>{MARK}a<?pi x?>b</content>")),
            &["d"],
            description("<content><value content=\"ab\"/></content>"),
        ),
        shape(
            "text split by a processing instruction in key",
            tagged(&format!("<key>{MARK}a<?pi x?>b</key>")),
            &["t"],
            tagged("<key><value content=\"ab\"/></key>"),
        ),
    ] {
        assert_shape_in_both_modes(&shape);
    }
}

#[test]
fn lib_compat_008_text_of_a_name_or_value_is_joined_across_comments() {
    for shape in [
        shape(
            "text split by a comment in a name",
            named(&format!("<name>{MARK}b<!--c-->d</name>")),
            &["c1"],
            named("<name content=\"bd\"/>"),
        ),
        shape(
            "text split by a comment in a value",
            description(&format!(
                "<content><value>{MARK}b<!--c-->d</value></content>"
            )),
            &["d"],
            description("<content><value content=\"bd\"/></content>"),
        ),
    ] {
        assert_shape_in_both_modes(&shape);
    }
}

#[test]
fn lib_compat_008_a_lang_alone_on_content_or_key_is_preserved_with_it() {
    for (what, template) in [
        (
            "<content lang/>",
            description(&format!("{MARK}<content lang=\"en\"/>")),
        ),
        ("<key lang/>", tagged(&format!("{MARK}<key lang=\"en\"/>"))),
    ] {
        assert_preserved_at_mark(what, &template);
    }
}

#[test]
fn lib_compat_008_content_and_value_attributes_on_a_tagged_value_are_repeated_value() {
    let (input, marks) = marked(&ap(&format!(
        "<taggedValue xmi:id=\"t\" {MARK}content=\"w\" {MARK}value=\"v\">{KEY}</taggedValue>"
    )));
    let error = refused("content= and value= on a taggedValue", &input);
    assert!(
        matches!(&error, ReadError::RepeatedValue { element, property, .. }
            if element.as_str() == "t" && property == "content.value.content"),
        "{ROW}: content= and value= on a taggedValue: not RepeatedValue for \
         content.value.content of t: {error:?}"
    );
    assert_eq!(
        error.locations(),
        marks.as_slice(),
        "{ROW}: content= and value= on a taggedValue: not both attributes"
    );
}

#[test]
fn lib_compat_008_a_shorthand_attribute_beside_value_children_is_preserved_with_its_content_or_key()
{
    for (what, template) in [
        (
            "content= on a content with a value",
            description(&format!(
                "{MARK}<content content=\"x\"><value content=\"y\"/></content>"
            )),
        ),
        (
            "lang= on a key with a value",
            tagged(&format!(
                "{MARK}<key lang=\"en\"><value content=\"k\"/></key>"
            )),
        ),
    ] {
        assert_preserved_at_mark(what, &template);
    }
}

#[test]
fn lib_compat_008_lang_on_a_content_inside_a_value_is_preserved_with_its_name_content_or_key() {
    for (what, template) in [
        (
            "in a value of a content",
            description(&format!(
                "{MARK}<content><value><content lang=\"en\">x</content></value></content>"
            )),
        ),
        (
            "in a value of a key",
            tagged(&format!(
                "{MARK}<key><value><content lang=\"en\">x</content></value></key>"
            )),
        ),
        (
            "in a name's value",
            named(&format!(
                "{MARK}<name><value><content lang=\"en\">x</content></value></name>"
            )),
        ),
    ] {
        assert_preserved_at_mark(what, &template);
    }
}

// --- Helpers for the refusals that stay, and for what is preserved -----------------------------

/// Asserts that `input` reads in both modes and keeps one construct as
/// preserved content of `holder` (LIB-COMPAT-003, ADR 0033): exactly one
/// SACM-XMI-003 at `at`, affecting `holder`, a Warning in tolerant mode and
/// an Error in strict mode, and the same document in both.
fn assert_preserved_at(what: &str, input: &str, at: SourceLocation, holder: &str) {
    let mut documents = Vec::new();
    for (mode, severity) in [
        (Mode::Tolerant, Severity::Warning),
        (Mode::Strict, Severity::Error),
    ] {
        let output = match read(mode, input) {
            Ok(output) => output,
            Err(error) => panic!(
                "{ROW}: {what}: a {mode:?} read refused it instead of preserving it: {error:?}"
            ),
        };
        let found: Vec<_> = output
            .diagnostics()
            .iter()
            .filter(|found| found.code() == Code::XMI_UNKNOWN_CONTENT)
            .map(|found| {
                (
                    found.severity(),
                    found.location(),
                    found.affected().to_vec(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [(severity, Some(at), vec![ElementId::from(holder)])],
            "{ROW}: {what}: not one SACM-XMI-003 at the construct affecting {holder:?} in \
             {mode:?} mode: {:?}",
            output.diagnostics()
        );
        let doc = output.into_document();
        assert!(
            doc.get(holder)
                .is_some_and(SacmElement::has_preserved_content),
            "{ROW}: {what}: {holder:?} holds no preserved content in {mode:?} mode"
        );
        documents.push(doc);
    }
    assert_eq!(
        documents[0], documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
}

/// Reads `template` without its one mark in both modes and asserts that the
/// construct at the mark is kept as preserved content (LIB-COMPAT-003, ADR
/// 0033) of the element whose `xmi:id` comes last before the mark.
fn assert_preserved_at_mark(what: &str, template: &str) {
    let (input, marks) = marked(template);
    assert_eq!(marks.len(), 1, "{ROW}: setup: {what} has not one mark");
    let before = template.split(MARK).next().unwrap_or_default();
    let Some(holder) = before
        .rsplit("xmi:id=\"")
        .next()
        .filter(|_| before.contains("xmi:id=\""))
        .and_then(|rest| rest.split('"').next())
    else {
        panic!("{ROW}: setup: {what} has no xmi:id before its mark");
    };
    assert_preserved_at(what, &input, marks[0], holder);
}

/// Reads `template` without its one mark in both modes and asserts that it
/// is refused as `Unsupported`, with no code, at the mark.
fn assert_unsupported_at_mark(what: &str, template: &str) {
    let (input, marks) = marked(template);
    assert_eq!(marks.len(), 1, "{ROW}: setup: {what} has not one mark");
    let error = refused(what, &input);
    assert!(
        matches!(error, ReadError::Unsupported { .. }) && error.code().is_none(),
        "{ROW}: {what}: not Unsupported: {error:?} ({error})"
    );
    assert_eq!(
        error.location(),
        Some(marks[0]),
        "{ROW}: {what}: not refused at the construct: {error}"
    );
}
