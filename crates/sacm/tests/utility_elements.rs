//! Tests for descriptions, notes and implementation constraints, the
//! `UtilityElement`s of SACM 2.3 §8.7-§8.11 with a `content` `MultiLangString`
//! (SACM23-BASE-004; §8.5, §8.6; XMI 2.5.1 §7.8.3, §7.8.6; D-DESC-MULT;
//! D-ORDER; ADR 0012, ADR 0024, ADR 0025).
//!
//! Every `ModelElement` holds its `description`, `implementationConstraint` and
//! `note` children as elements in the one ordered child list. A
//! `UtilityElement`'s `content` is absent (`None`) or a `MultiLangString`, held in
//! order with duplicates; `<content/>` is an empty one. An `xmi:id` on
//! `content` or on a `value` is kept by role and position, shares the file's
//! id space and is written first on its element. Single-language shorthands
//! stay `Unsupported` until LIB-COMPAT-008.
//!
//! Also here, for the rows this slice rewords:
//! - SACM23-VAL-020: a `content` or `value` id that is not an `NCName` is one
//!   SACM-ID-002 on the `UtilityElement`;
//! - SACM23-XMI-004: a `content` or `value` id is written escaped and reads
//!   back exactly; an edit-built `MultiLangString` is written with no `xmi:id`.
//!
//! The `key` role of a `TaggedValue` is tested in `tagged_values.rs`
//! (SACM23-BASE-005), `gid` in `gid.rs` (SACM23-BASE-006) and the
//! `MultiLangString` rules of `validate` in `multi_lang_string_validation.rs`
//! (SACM23-VAL-010).
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading, and are counted here: lines end at LF,
//! CRLF or a lone CR, and columns count characters.
//!
//! The written form is checked line by line with the indentation trimmed: an
//! empty element is written `<content />`, as the writer writes `<name />`
//! (SACM23-XMI-012).

use std::path::Path;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{self, Code, Severity, SourceLocation};
use sacm::edit::{
    self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim, CreateDescription,
    CreateImplementationConstraint, CreateNote, EditError, Operation,
};
use sacm::model::{
    AbstractClass, Document, ElementId, ElementKind, LangString, MultiLangString, SacmElement,
};
use sacm::validation;
use sacm::xmi::{
    self, Mode, ReadError, ReadOptions, ReadOutput, WriteAttribute, WriteError, WriteOptions,
};

const ROW: &str = "SACM23-BASE-004";
const VAL_020: &str = "SACM23-VAL-020";
const XMI_004: &str = "SACM23-XMI-004";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a start tag, an attribute or text is
/// expected. Removed before reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The three roles of this row: the role element, the kind and the snake-case
/// kind of a generated id.
const ROLES: [(&str, ElementKind, &str); 3] = [
    ("description", ElementKind::Description, "description"),
    (
        "implementationConstraint",
        ElementKind::ImplementationConstraint,
        "implementation_constraint",
    ),
    ("note", ElementKind::Note, "note"),
];

/// The valid fixture of this row and SACM23-BASE-005.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/utility-elements-valid.sacm.xmi"
);

// --- Inputs ------------------------------------------------------------------

/// An ArgumentPackage root `p` named "P", holding `body` from line 3.
fn in_ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  {body}\n\
         </sacm:ArgumentPackage>"
    )
}

/// A Description `d` holding `body`, in [`in_ap`].
fn in_description(body: &str) -> String {
    in_ap(&format!("<description xmi:id=\"d\">{body}</description>"))
}

/// A Description `d` whose `content` holds `values`, in [`in_ap`].
fn description_with(values: &str) -> String {
    in_description(&format!("<content>{values}</content>"))
}

/// An AssuranceCasePackage root `case` named "Case" holding `body`.
fn acp_root(body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>{body}\
         </sacm:AssuranceCasePackage>"
    )
}

/// An ArgumentPackage root `arguments` named "Arguments" holding `body`.
fn ap_root(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"arguments\"><name content=\"Arguments\"/>{body}\
         </sacm:ArgumentPackage>"
    )
}

/// The readable parents of every kind this release reads, as a root and as a
/// child: what, the parent's kind, and the input with a parent `p` named "P"
/// holding `body`.
fn contexts(body: &str) -> Vec<(&'static str, ElementKind, String)> {
    let named = |open: &str, close: &str| format!("{open}<name content=\"P\"/>{body}{close}");
    vec![
        (
            "an AssuranceCasePackage root",
            ElementKind::AssuranceCasePackage,
            named(
                &format!("<sacm:AssuranceCasePackage{NS} xmi:id=\"p\">"),
                "</sacm:AssuranceCasePackage>",
            ),
        ),
        (
            "an ArgumentPackage root",
            ElementKind::ArgumentPackage,
            named(
                &format!("<sacm:ArgumentPackage{NS} xmi:id=\"p\">"),
                "</sacm:ArgumentPackage>",
            ),
        ),
        (
            "a wrapped ArgumentPackage root",
            ElementKind::ArgumentPackage,
            named(
                "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
                 xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\
                 <sacm:ArgumentPackage xmi:id=\"p\">",
                "</sacm:ArgumentPackage></xmi:XMI>",
            ),
        ),
        (
            "an AssuranceCasePackage child",
            ElementKind::AssuranceCasePackage,
            acp_root(&named(
                "<assuranceCasePackage xmi:id=\"p\">",
                "</assuranceCasePackage>",
            )),
        ),
        (
            "an ArgumentPackage in argumentPackage",
            ElementKind::ArgumentPackage,
            acp_root(&named(
                "<argumentPackage xmi:id=\"p\">",
                "</argumentPackage>",
            )),
        ),
        (
            "an ArgumentPackage in argumentElement",
            ElementKind::ArgumentPackage,
            ap_root(&named(
                "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"p\">",
                "</argumentElement>",
            )),
        ),
        (
            "a Claim",
            ElementKind::Claim,
            ap_root(&named(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"p\">",
                "</argumentElement>",
            )),
        ),
    ]
}

fn loc(row: &str, line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{row}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order.
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

/// A `LangString` with exactly the given `lang` and `content`.
fn ls(lang: Option<&str>, content: Option<&str>) -> LangString {
    let mut value = LangString::default();
    value.set_lang(lang.map(str::to_owned));
    value.set_content(content.map(str::to_owned));
    value
}

/// `content` in the language `lang`.
fn en(lang: &str, content: &str) -> LangString {
    LangString::new(content).with_lang(lang)
}

fn mls(values: &[LangString]) -> MultiLangString {
    values.iter().cloned().collect()
}

// --- Reading and writing -------------------------------------------------------

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

/// Reads `template` without its marks in both modes and asserts that it is
/// refused as `Unsupported`, with no code, at its one mark. Returns the
/// reason.
fn assert_unsupported_at_mark(row: &str, what: &str, template: &str) -> String {
    let (input, marks) = marked(row, template);
    assert_eq!(marks.len(), 1, "{row}: setup: {what} has not one mark");
    let error = refused(row, what, &input);
    let ReadError::Unsupported { reason, .. } = &error else {
        panic!("{row}: {what}: not Unsupported: {error:?} ({error})");
    };
    assert!(
        error.code().is_none(),
        "{row}: {what}: an Unsupported refusal has a code: {error:?}"
    );
    assert_eq!(
        error.location(),
        Some(marks[0]),
        "{row}: {what}: not refused at the mark: {error}"
    );
    reason.clone()
}

/// Reads `template` without its marks in both modes and asserts that it is
/// refused as `RepeatedValue` of `property` on `element`, with one location
/// per mark, in order.
fn assert_repeated(row: &str, what: &str, template: &str, element: &str, property: &str) {
    let (input, marks) = marked(row, template);
    let error = refused(row, what, &input);
    let ReadError::RepeatedValue {
        element: found,
        property: found_property,
        ..
    } = &error
    else {
        panic!("{row}: {what}: not RepeatedValue: {error:?} ({error})");
    };
    assert_eq!(
        (found.as_str(), found_property.as_str()),
        (element, property),
        "{row}: {what}: the refusal names the wrong element or property"
    );
    assert_eq!(
        error.code(),
        Some(Code::MULTIPLICITY_VIOLATION),
        "{row}: {what}: the code is not SACM-MULT-001"
    );
    assert_eq!(
        error.locations(),
        marks.as_slice(),
        "{row}: {what}: not every occurrence, in document order"
    );
}

fn write_in(row: &str, what: &str, mode: Mode, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// Writes `doc` in `mode`, reads the output back, and asserts that it reads
/// equal and writes the same bytes again. Returns the output.
fn assert_round_trip(row: &str, what: &str, doc: &Document, mode: Mode) -> String {
    let text = write_in(row, what, mode, doc);
    let back = match read(Mode::Tolerant, &text) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{row}: {what}: the output does not read back: {error:?}\n{text}"),
    };
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{row}: {what}: the output reads back different\n{text}"
    );
    assert_eq!(
        back, *doc,
        "{row}: {what}: the output reads back != \n{text}"
    );
    assert_eq!(
        write_in(row, what, mode, &back),
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

fn ids<'d>(elements: impl Iterator<Item = SacmElement<'d>>) -> Vec<String> {
    elements.map(|e| e.id().as_str().to_owned()).collect()
}

/// The ids `element`'s accessor for `role` returns, in order.
fn role_ids(row: &str, element: SacmElement<'_>, role: &str) -> Vec<String> {
    match role {
        "description" => ids(element.descriptions()),
        "implementationConstraint" => ids(element.implementation_constraints()),
        "note" => ids(element.notes()),
        "taggedValue" => ids(element.tagged_values()),
        other => panic!("{row}: test oracle: no accessor for the role {other:?}"),
    }
}

/// The content of the element `id`, cloned.
fn content_of(row: &str, doc: &Document, id: &str) -> Option<MultiLangString> {
    element(row, doc, id).content().cloned()
}

fn some_ids(values: &[Option<&str>]) -> Vec<Option<ElementId>> {
    values
        .iter()
        .map(|value| value.map(ElementId::from))
        .collect()
}

/// The lines of `text`, trimmed.
fn lines(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).collect()
}

/// Whether `text` has `expected` as consecutive lines, trimmed.
fn has_lines(text: &str, expected: &[&str]) -> bool {
    lines(text)
        .windows(expected.len())
        .any(|window| window == expected)
}

/// The index, in `text`, of the first line carrying `xmi:id="<id>"`.
fn line_of(row: &str, text: &str, id: &str) -> usize {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(index) = text.lines().position(|line| line.contains(&attribute)) else {
        panic!("{row}: the output has no line with {attribute}:\n{text}");
    };
    index
}

// --- Item 1: each role on each readable kind --------------------------------------

#[test]
fn sacm23_base_004_each_utility_role_reads_on_each_readable_kind_as_root_and_child() {
    for (role, kind, _) in ROLES {
        let body = format!(
            "<{role} xmi:id=\"u\"><content><value lang=\"en\" content=\"x\"/></content></{role}>"
        );
        for (context, parent_kind, input) in contexts(&body) {
            let what = format!("a {role} in {context}");
            let doc = read_ok(ROW, &what, &input);
            let u = element(ROW, &doc, "u");
            assert_eq!(u.kind(), kind, "{ROW}: {what}: the kind");
            assert_eq!(
                u.parent().map(|p| (p.id().as_str().to_owned(), p.kind())),
                Some(("p".to_owned(), parent_kind)),
                "{ROW}: {what}: the parent"
            );
            assert_eq!(
                u.content(),
                Some(&mls(&[en("en", "x")])),
                "{ROW}: {what}: the content is not [en:x]"
            );
            assert_eq!(
                (u.content_id(), u.content_value_ids()),
                (None, [None].as_slice()),
                "{ROW}: {what}: ids that the input does not have"
            );
            assert_eq!(
                (u.name(), u.gid(), u.children().count()),
                (None, None, 0),
                "{ROW}: {what}: a name, a gid or children that the input does not have"
            );
            let p = element(ROW, &doc, "p");
            for (other, _, _) in ROLES {
                let expected = if other == role {
                    vec!["u".to_owned()]
                } else {
                    Vec::new()
                };
                assert_eq!(
                    role_ids(ROW, p, other),
                    expected,
                    "{ROW}: {what}: the parent's {other} accessor"
                );
            }
            assert_eq!(
                role_ids(ROW, p, "taggedValue"),
                Vec::<String>::new(),
                "{ROW}: {what}: the parent's taggedValue accessor"
            );
            let text = assert_round_trip(ROW, &what, &doc, Mode::Strict);
            let start = format!("<{role} xmi:id=\"u\">");
            let end = format!("</{role}>");
            assert!(
                has_lines(
                    &text,
                    &[
                        start.as_str(),
                        "<content>",
                        "<value lang=\"en\" content=\"x\" />",
                        "</content>",
                        end.as_str()
                    ]
                ),
                "{ROW}: {what}: not written as the role element, content and one value:\n{text}"
            );
        }
    }
}

#[test]
fn sacm23_base_004_the_fixture_reads_with_every_utility_element_and_round_trips() {
    let bytes = match std::fs::read(Path::new(FIXTURE)) {
        Ok(bytes) => bytes,
        Err(error) => panic!("{ROW}: cannot read {FIXTURE}: {error}"),
    };
    let input = String::from_utf8_lossy(&bytes).into_owned();
    let doc = read_ok(ROW, "the fixture", &input);
    let arguments = element(ROW, &doc, "arguments");
    assert_eq!(
        role_ids(ROW, arguments, "description"),
        ["d1"],
        "{ROW}: the fixture: the root's descriptions"
    );
    let d1 = element(ROW, &doc, "d1");
    assert_eq!(
        (
            d1.content().cloned(),
            d1.content_id().map(ElementId::as_str),
            d1.content_value_ids().to_vec()
        ),
        (
            Some(mls(&[
                en("en", "Why the brakes are safe"),
                en("sv", "Varför bromsarna är säkra")
            ])),
            Some("d1_content"),
            some_ids(&[Some("d1_en"), None])
        ),
        "{ROW}: the fixture: d1"
    );
    let c1 = element(ROW, &doc, "c1");
    assert_eq!(
        ids(c1.children()),
        ["n1", "i1", "d2", "t1"],
        "{ROW}: the fixture: the Claim's children in document order"
    );
    assert_eq!(
        (
            role_ids(ROW, c1, "note"),
            role_ids(ROW, c1, "implementationConstraint"),
            role_ids(ROW, c1, "description"),
            role_ids(ROW, c1, "taggedValue")
        ),
        (
            vec!["n1".to_owned()],
            vec!["i1".to_owned()],
            vec!["d2".to_owned()],
            vec!["t1".to_owned()]
        ),
        "{ROW}: the fixture: the Claim's accessors"
    );
    assert!(
        !diagnostic::has_errors(&validation::validate(&doc)),
        "{ROW}: the fixture does not validate: {:?}",
        validation::validate(&doc)
    );
    assert_round_trip(ROW, "the fixture", &doc, Mode::Strict);
}

// --- Item 2: document order across roles -------------------------------------------

#[test]
fn sacm23_base_004_interleaved_utility_children_keep_document_order_and_several_descriptions_are_kept()
 {
    let doc = read_ok(
        ROW,
        "interleaved children",
        &in_ap(
            "<description xmi:id=\"d1\"><content><value content=\"one\"/></content></description>\n  \
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/></argumentElement>\n  \
             <note xmi:id=\"n1\"><content><value content=\"n\"/></content></note>\n  \
             <description xmi:id=\"d2\"><content><value content=\"two\"/></content></description>\n  \
             <implementationConstraint xmi:id=\"i1\"><content><value content=\"i\"/></content></implementationConstraint>",
        ),
    );
    let p = element(ROW, &doc, "p");
    assert_eq!(
        ids(p.children()),
        ["d1", "c1", "n1", "d2", "i1"],
        "{ROW}: the children are not in document order across roles"
    );
    assert_eq!(
        role_ids(ROW, p, "description"),
        ["d1", "d2"],
        "{ROW}: descriptions() does not return both descriptions in order"
    );
    assert_eq!(role_ids(ROW, p, "note"), ["n1"], "{ROW}: notes()");
    assert_eq!(
        role_ids(ROW, p, "implementationConstraint"),
        ["i1"],
        "{ROW}: implementation_constraints()"
    );
    assert!(
        role_ids(ROW, p, "taggedValue").is_empty(),
        "{ROW}: tagged_values() returns elements that are not TaggedValues"
    );
    let c1 = element(ROW, &doc, "c1");
    assert!(
        c1.descriptions().next().is_none() && c1.notes().next().is_none(),
        "{ROW}: a Claim without utility children returns some"
    );
    let text = assert_round_trip(ROW, "interleaved children", &doc, Mode::Strict);
    let order: Vec<usize> = ["d1", "c1", "n1", "d2", "i1"]
        .iter()
        .map(|id| line_of(ROW, &text, id))
        .collect();
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "{ROW}: the children are not written in document order:\n{text}"
    );
}

// --- Item 3: values in order with duplicates ---------------------------------------

#[test]
fn sacm23_base_004_a_content_keeps_its_values_in_order_with_duplicates() {
    let doc = read_ok(
        ROW,
        "[en:a, fr:b, en:a]",
        &description_with(
            "<value lang=\"en\" content=\"a\"/><value lang=\"fr\" content=\"b\"/>\
             <value lang=\"en\" content=\"a\"/>",
        ),
    );
    let expected = mls(&[en("en", "a"), en("fr", "b"), en("en", "a")]);
    assert_eq!(
        content_of(ROW, &doc, "d"),
        Some(expected.clone()),
        "{ROW}: the three values are not kept in order"
    );
    // Tolerant: the repeated `en` is a validation Error (SACM23-VAL-010).
    let text = assert_round_trip(ROW, "[en:a, fr:b, en:a]", &doc, Mode::Tolerant);
    assert!(
        has_lines(
            &text,
            &[
                "<content>",
                "<value lang=\"en\" content=\"a\" />",
                "<value lang=\"fr\" content=\"b\" />",
                "<value lang=\"en\" content=\"a\" />",
                "</content>"
            ]
        ),
        "{ROW}: the three values are not written in order:\n{text}"
    );
    let back = read_ok(ROW, "written", &text);
    assert_eq!(
        content_of(ROW, &back, "d"),
        Some(expected),
        "{ROW}: the values do not survive a write"
    );
}

// --- Item 4: absent, empty and the value states --------------------------------------

#[test]
fn sacm23_base_004_no_content_child_is_none_and_writes_nothing() {
    for (role, _, _) in ROLES {
        let what = format!("a {role} with no content");
        let doc = read_ok(ROW, &what, &in_ap(&format!("<{role} xmi:id=\"u\"/>")));
        let u = element(ROW, &doc, "u");
        assert_eq!(
            (u.content(), u.content_id(), u.content_value_ids()),
            (None, None, [].as_slice()),
            "{ROW}: {what}: content() is not None"
        );
        let text = assert_round_trip(ROW, &what, &doc, Mode::Strict);
        let expected = format!("<{role} xmi:id=\"u\" />");
        assert!(
            lines(&text).contains(&expected.as_str()) && !text.contains("<content"),
            "{ROW}: {what}: not written as {expected:?} with no content:\n{text}"
        );
    }
}

#[test]
fn sacm23_base_004_an_empty_content_in_any_form_is_an_empty_multi_lang_string_written_as_one_empty_element()
 {
    let forms = [
        ("<content/>", "<content/>"),
        ("<content></content>", "<content></content>"),
        ("white space", "<content> \n\t\r\n </content>"),
        ("a comment", "<content><!-- c --></content>"),
        ("a processing instruction", "<content><?tool x?></content>"),
        (
            "comments, PIs and white space",
            "<content>\n  <!-- c -->\n  <?tool x?>\n</content>",
        ),
    ];
    for (what, form) in forms {
        let what = format!("{what} as the content");
        let doc = read_ok(ROW, &what, &in_description(form));
        let d = element(ROW, &doc, "d");
        assert_eq!(
            (d.content(), d.content_value_ids()),
            (Some(&MultiLangString::default()), [].as_slice()),
            "{ROW}: {what}: not an empty MultiLangString"
        );
        // Tolerant: an empty MultiLangString is a validation Error (SACM23-VAL-010).
        let text = assert_round_trip(ROW, &what, &doc, Mode::Tolerant);
        assert!(
            has_lines(
                &text,
                &[
                    "<description xmi:id=\"d\">",
                    "<content />",
                    "</description>"
                ]
            ),
            "{ROW}: {what}: not written as one empty content element:\n{text}"
        );
    }
}

#[test]
fn sacm23_base_004_the_four_absent_and_empty_lang_and_content_states_of_a_value_round_trip() {
    let states = [
        ("<value/>", ls(None, None), "<value />"),
        (
            "<value content=\"\"/>",
            ls(None, Some("")),
            "<value content=\"\" />",
        ),
        (
            "<value lang=\"\"/>",
            ls(Some(""), None),
            "<value lang=\"\" />",
        ),
        (
            "<value lang=\"\" content=\"\"/>",
            ls(Some(""), Some("")),
            "<value lang=\"\" content=\"\" />",
        ),
    ];
    for (form, expected, written) in states {
        let doc = read_ok(ROW, form, &description_with(form));
        assert_eq!(
            content_of(ROW, &doc, "d"),
            Some(mls(std::slice::from_ref(&expected))),
            "{ROW}: {form} is not one value {expected:?}"
        );
        let text = assert_round_trip(ROW, form, &doc, Mode::Strict);
        assert!(
            has_lines(&text, &["<content>", written, "</content>"]),
            "{ROW}: {form} is not written as {written:?}:\n{text}"
        );
    }
    assert_eq!(
        content_of(
            ROW,
            &read_ok(ROW, "<value/>", &description_with("<value/>")),
            "d"
        ),
        Some(mls(&[LangString::default()])),
        "{ROW}: <content><value/></content> is not one LangString::default()"
    );
}

// --- Item 5: the element form of a value --------------------------------------------

#[test]
fn sacm23_base_004_a_value_in_the_element_form_reads_equal_to_the_attribute_form() {
    let attribute = read_ok(
        ROW,
        "the attribute form",
        &description_with("<value lang=\"en\" content=\"x\"/>"),
    );
    for form in [
        "<value><lang>en</lang><content>x</content></value>",
        "<value><content>x</content><lang>en</lang></value>",
        "<value lang=\"en\"><content>x</content></value>",
        "<value content=\"x\">\n  <lang>en</lang>\n</value>",
    ] {
        let doc = read_ok(ROW, form, &description_with(form));
        assert_eq!(
            semantic_compare(&doc, &attribute),
            [],
            "{ROW}: {form} does not read equal to the attribute form"
        );
    }
    let empty = read_ok(
        ROW,
        "empty value elements",
        &description_with("<value><lang/><content></content></value>"),
    );
    assert_eq!(
        content_of(ROW, &empty, "d"),
        Some(mls(&[ls(Some(""), Some(""))])),
        "{ROW}: empty value elements are not empty strings"
    );
}

#[test]
fn sacm23_base_004_a_value_field_given_as_an_attribute_and_an_element_is_repeated_value() {
    for (what, template) in [
        (
            "lang as an attribute and an element",
            description_with(&format!(
                "<value {MARK}lang=\"en\" content=\"x\">{MARK}<lang>en</lang></value>"
            )),
        ),
        (
            "content as an attribute and an element",
            description_with(&format!(
                "<value {MARK}content=\"x\">{MARK}<content>y</content></value>"
            )),
        ),
        (
            "two lang elements",
            description_with(&format!(
                "<value>{MARK}<lang>en</lang>{MARK}<lang>en</lang></value>"
            )),
        ),
    ] {
        let (input, marks) = marked(ROW, &template);
        let error = refused(ROW, what, &input);
        let ReadError::RepeatedValue {
            element, property, ..
        } = &error
        else {
            panic!("{ROW}: {what}: not RepeatedValue: {error:?}");
        };
        assert_eq!(
            element.as_str(),
            "d",
            "{ROW}: {what}: the refusal does not name the Description"
        );
        let field = if what.starts_with("content") {
            "content"
        } else {
            "lang"
        };
        assert!(
            property.contains(field),
            "{ROW}: {what}: the property {property:?} does not name {field:?}"
        );
        assert_eq!(
            (error.code(), error.locations()),
            (Some(Code::MULTIPLICITY_VIOLATION), marks.as_slice()),
            "{ROW}: {what}: not SACM-MULT-001 at both occurrences"
        );
    }
}

// --- Item 6: a second content -----------------------------------------------------

#[test]
fn sacm23_base_004_two_content_children_are_repeated_value_with_both_locations() {
    for (role, _, _) in ROLES {
        assert_repeated(
            ROW,
            &format!("two content children of a {role}"),
            &in_ap(&format!(
                "<{role} xmi:id=\"u\">{MARK}<content><value content=\"a\"/></content>\n  \
                 {MARK}<content><value content=\"a\"/></content></{role}>"
            )),
            "u",
            "content",
        );
    }
    assert_repeated(
        ROW,
        "an empty content then another",
        &in_description(&format!("{MARK}<content/>{MARK}<content/>")),
        "d",
        "content",
    );
}

// --- Item 7: type attributes ------------------------------------------------------

#[test]
fn sacm23_base_004_type_attributes_naming_the_classes_are_accepted_and_none_is_written() {
    let plain = read_ok(
        ROW,
        "untyped",
        &in_ap(
            "<description xmi:id=\"d\"><content><value lang=\"en\" content=\"x\"/></content></description>\
             <note xmi:id=\"n\"/><implementationConstraint xmi:id=\"i\"/>",
        ),
    );
    let typed = read_ok(
        ROW,
        "typed",
        &in_ap(
            "<description xmi:type=\"sacm:Description\" xmi:id=\"d\">\
             <content xmi:type=\"sacm:MultiLangString\">\
             <value xmi:type=\"sacm:LangString\" lang=\"en\" content=\"x\"/></content></description>\
             <note xmi:type=\"sacm:Note\" xmi:id=\"n\"/>\
             <implementationConstraint xmi:type=\"sacm:ImplementationConstraint\" xmi:id=\"i\"/>",
        ),
    );
    assert_eq!(
        semantic_compare(&typed, &plain),
        [],
        "{ROW}: the typed input does not read equal to the untyped one"
    );
    let text = assert_round_trip(ROW, "typed", &typed, Mode::Strict);
    for line in lines(&text) {
        let utility = [
            "<description",
            "<note",
            "<implementationConstraint",
            "<content",
            "<value",
        ]
        .iter()
        .any(|start| line.starts_with(*start));
        assert!(
            !utility || !line.contains("xmi:type"),
            "{ROW}: a type attribute is written on {line:?}"
        );
    }
}

#[test]
fn sacm23_base_004_a_type_naming_another_class_is_unsupported_at_its_element() {
    let cases = [
        (
            "sacm:Note on description",
            in_ap(&format!(
                "{MARK}<description xmi:type=\"sacm:Note\" xmi:id=\"d\"/>"
            )),
        ),
        (
            "sacm:Description on note",
            in_ap(&format!(
                "{MARK}<note xmi:type=\"sacm:Description\" xmi:id=\"n\"/>"
            )),
        ),
        (
            "sacm:TaggedValue on implementationConstraint",
            in_ap(&format!(
                "{MARK}<implementationConstraint xmi:type=\"sacm:TaggedValue\" xmi:id=\"i\"/>"
            )),
        ),
        (
            "sacm:ExpressionLangString on value",
            description_with(&format!(
                "{MARK}<value xmi:type=\"sacm:ExpressionLangString\" content=\"x\"/>"
            )),
        ),
        (
            "sacm:LangString on content",
            in_description(&format!("{MARK}<content xmi:type=\"sacm:LangString\"/>")),
        ),
    ];
    for (what, template) in &cases {
        assert_unsupported_at_mark(ROW, what, template);
    }
}

// --- Item 8: ids ------------------------------------------------------------------

#[test]
fn sacm23_base_004_a_utility_element_without_an_id_gets_a_generated_id_with_one_xmi_004_info() {
    for (role, kind, snake) in ROLES {
        let what = format!("a {role} without an id");
        let (input, marks) = marked(ROW, &in_ap(&format!("{MARK}<{role}/>")));
        for mode in MODES {
            let output = match read(mode, &input) {
                Ok(output) => output,
                Err(error) => panic!("{ROW}: {what} ({mode:?}): refused: {error:?}"),
            };
            let generated = format!("{snake}_1");
            let found = element(ROW, output.document(), &generated);
            assert_eq!(found.kind(), kind, "{ROW}: {what} ({mode:?}): the kind");
            let [info] = output.diagnostics() else {
                panic!(
                    "{ROW}: {what} ({mode:?}): not one diagnostic: {:?}",
                    output.diagnostics()
                );
            };
            assert_eq!(
                (info.code(), info.severity(), info.requirement_id()),
                (Code::XMI_MISSING_ID, Severity::Info, "SACM23-XMI-020"),
                "{ROW}: {what} ({mode:?}): not the SACM-XMI-004 Info"
            );
            assert_eq!(
                (info.affected(), info.location()),
                (
                    [ElementId::from(generated.as_str())].as_slice(),
                    Some(marks[0])
                ),
                "{ROW}: {what} ({mode:?}): not the generated id at the start tag"
            );
        }
    }
}

#[test]
fn sacm23_base_004_a_padded_utility_id_is_trimmed_an_empty_one_is_invalid_and_a_repeat_is_duplicate()
 {
    let (input, marks) = marked(ROW, &in_ap(&format!("<note {MARK}xmi:id=\" n1 \"/>")));
    for mode in MODES {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!("{ROW}: a padded note id ({mode:?}): refused: {error:?}"),
        };
        assert!(
            output.document().get("n1").is_some(),
            "{ROW}: a padded note id ({mode:?}) is not read trimmed"
        );
        let [info] = output.diagnostics() else {
            panic!(
                "{ROW}: a padded note id ({mode:?}): {:?}",
                output.diagnostics()
            );
        };
        assert_eq!(
            (info.code(), info.affected(), info.location()),
            (
                Code::ID_TRIMMED,
                [ElementId::from("n1")].as_slice(),
                Some(marks[0])
            ),
            "{ROW}: a padded note id ({mode:?}): not the SACM-ID-004 Info"
        );
    }

    let (input, marks) = marked(ROW, &in_ap(&format!("{MARK}<description xmi:id=\"\"/>")));
    let error = refused(ROW, "an empty description id", &input);
    assert!(
        matches!(&error, ReadError::InvalidId { id, .. } if id.is_empty())
            && error.location() == Some(marks[0]),
        "{ROW}: an empty description id is not InvalidId at its start tag: {error:?}"
    );

    for (what, template, id) in [
        (
            "a description repeating a Claim's id",
            in_ap(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/>\
                 </argumentElement>{MARK}<description xmi:id=\"c1\"/>"
            )),
            "c1",
        ),
        (
            "a description repeating its parent's id",
            in_ap(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/>\
                 {MARK}<description xmi:id=\"c1\"/></argumentElement>"
            )),
            "c1",
        ),
        (
            "two notes with one id",
            in_ap(&format!(
                "{MARK}<note xmi:id=\"n\"/>{MARK}<note xmi:id=\"n\"/>"
            )),
            "n",
        ),
    ] {
        let (input, marks) = marked(ROW, &template);
        let error = refused(ROW, what, &input);
        assert!(
            matches!(&error, ReadError::DuplicateId { id: found, .. } if found.as_str() == id),
            "{ROW}: {what}: not DuplicateId for {id:?}: {error:?}"
        );
        assert_eq!(
            (error.code(), error.locations()),
            (Some(Code::ID_DUPLICATE), marks.as_slice()),
            "{ROW}: {what}: not SACM-ID-001 at both start tags"
        );
    }
}

#[test]
fn sacm23_base_004_content_and_value_ids_are_kept_by_role_and_position_and_written_first() {
    let doc = read_ok(
        ROW,
        "ids on content and values",
        &description_with(
            "<value lang=\"en\" content=\"a\" xmi:id=\"v1\"/><value lang=\"sv\" content=\"b\"/>\
             <value xmi:id=\"v3\"/>",
        )
        .replace("<content>", "<content xmi:id=\"w\">"),
    );
    let d = element(ROW, &doc, "d");
    assert_eq!(
        d.content_id().map(ElementId::as_str),
        Some("w"),
        "{ROW}: content_id() is not the content element's id"
    );
    assert_eq!(
        d.content_value_ids(),
        some_ids(&[Some("v1"), None, Some("v3")]).as_slice(),
        "{ROW}: content_value_ids() does not hold each value's id by position"
    );
    assert_eq!(
        d.content(),
        Some(&mls(&[en("en", "a"), en("sv", "b"), ls(None, None)])),
        "{ROW}: the ids changed the values"
    );
    for id in ["w", "v1", "v3"] {
        assert!(
            doc.get(id).is_none() && !doc.contains(id),
            "{ROW}: the id {id:?} identifies an element"
        );
    }
    assert_eq!(doc.element_count(), 2, "{ROW}: an id became an element");
    let text = assert_round_trip(ROW, "ids on content and values", &doc, Mode::Strict);
    assert!(
        has_lines(
            &text,
            &[
                "<description xmi:id=\"d\">",
                "<content xmi:id=\"w\">",
                "<value xmi:id=\"v1\" lang=\"en\" content=\"a\" />",
                "<value lang=\"sv\" content=\"b\" />",
                "<value xmi:id=\"v3\" />",
                "</content>",
                "</description>"
            ]
        ),
        "{ROW}: the ids are not written first on their elements:\n{text}"
    );
    // An empty content with an id keeps it.
    let doc = read_ok(
        ROW,
        "an empty content with an id",
        &in_description("<content xmi:id=\"w\"/>"),
    );
    let text = assert_round_trip(ROW, "an empty content with an id", &doc, Mode::Tolerant);
    assert!(
        lines(&text).contains(&"<content xmi:id=\"w\" />"),
        "{ROW}: an empty content with an id is not written with it:\n{text}"
    );
}

#[test]
fn sacm23_base_004_a_content_or_value_id_repeating_any_id_is_duplicate_id_at_both_start_tags() {
    let cases = [
        (
            "the content id is the owner's id",
            in_ap(&format!(
                "{MARK}<description xmi:id=\"d\">{MARK}<content xmi:id=\"d\"/></description>"
            )),
            "d",
        ),
        (
            "a value id is the content id",
            in_ap(&format!(
                "<description xmi:id=\"d\">{MARK}<content xmi:id=\"w\">\
                 {MARK}<value xmi:id=\"w\"/></content></description>"
            )),
            "w",
        ),
        (
            "two value ids",
            description_with(&format!(
                "{MARK}<value xmi:id=\"v\"/>{MARK}<value xmi:id=\"v\"/>"
            )),
            "v",
        ),
        (
            "a value id is a later Claim's id",
            in_ap(&format!(
                "<note xmi:id=\"n\"><content>{MARK}<value xmi:id=\"c9\"/></content></note>\
                 {MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c9\"><name content=\"C\"/>\
                 </argumentElement>"
            )),
            "c9",
        ),
        (
            "a content id is a name id",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">{MARK}<name xmi:id=\"x\" content=\"C\"/>\
                 <note xmi:id=\"n\">{MARK}<content xmi:id=\"x\"/></note></argumentElement>"
            )),
            "x",
        ),
    ];
    for (what, template, id) in &cases {
        let (input, marks) = marked(ROW, template);
        let error = refused(ROW, what, &input);
        assert!(
            matches!(&error, ReadError::DuplicateId { id: found, .. } if found.as_str() == *id),
            "{ROW}: {what}: not DuplicateId for {id:?}: {error:?}"
        );
        assert_eq!(
            (error.code(), error.locations()),
            (Some(Code::ID_DUPLICATE), marks.as_slice()),
            "{ROW}: {what}: not SACM-ID-001 at both start tags"
        );
    }
}

#[test]
fn sacm23_base_004_an_empty_content_or_value_id_is_invalid_id_and_a_padded_one_is_trimmed_on_the_owner()
 {
    for (what, template) in [
        (
            "an empty content id",
            in_description(&format!("{MARK}<content xmi:id=\"\"/>")),
        ),
        (
            "an empty value id",
            description_with(&format!("{MARK}<value xmi:id=\"\" content=\"x\"/>")),
        ),
    ] {
        let (input, marks) = marked(ROW, &template);
        let error = refused(ROW, what, &input);
        assert!(
            matches!(&error, ReadError::InvalidId { id, .. } if id.is_empty()),
            "{ROW}: {what}: not InvalidId: {error:?}"
        );
        assert_eq!(
            (error.code(), error.location()),
            (Some(Code::ID_INVALID), Some(marks[0])),
            "{ROW}: {what}: not SACM-ID-002 at the start tag"
        );
    }
    let (input, marks) = marked(
        ROW,
        &in_description(&format!(
            "<content {MARK}xmi:id=\" w \"><value {MARK}xmi:id=\"v&#9;\" content=\"x\"/></content>"
        )),
    );
    for mode in MODES {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!("{ROW}: padded content ids ({mode:?}): refused: {error:?}"),
        };
        let d = element(ROW, output.document(), "d");
        assert_eq!(
            (d.content_id().map(ElementId::as_str), d.content_value_ids()),
            (Some("w"), some_ids(&[Some("v")]).as_slice()),
            "{ROW}: padded content ids ({mode:?}) are not read trimmed"
        );
        let found: Vec<_> = output
            .diagnostics()
            .iter()
            .map(|info| {
                (
                    info.code(),
                    info.severity(),
                    info.affected().to_vec(),
                    info.location(),
                )
            })
            .collect();
        let expected: Vec<_> = marks
            .iter()
            .map(|&at| {
                (
                    Code::ID_TRIMMED,
                    Severity::Info,
                    vec![ElementId::from("d")],
                    Some(at),
                )
            })
            .collect();
        assert_eq!(
            found, expected,
            "{ROW}: padded content ids ({mode:?}): not one SACM-ID-004 each, on the owner"
        );
    }
}

#[test]
fn sacm23_base_004_generated_ids_skip_content_and_value_ids_on_read_and_on_edit() {
    let idless = "<description><content><value content=\"new\"/></content></description>";
    let holder = "<note xmi:id=\"n\"><content xmi:id=\"description_1\">\
                  <value xmi:id=\"description_2\" content=\"x\"/></content></note>";
    for (what, body) in [
        (
            "ids before the id-less description",
            format!("{holder}{idless}"),
        ),
        (
            "ids after the id-less description",
            format!("{idless}{holder}"),
        ),
    ] {
        for mode in MODES {
            let output = match read(mode, &in_ap(&body)) {
                Ok(output) => output,
                Err(error) => panic!("{ROW}: {what} ({mode:?}): refused: {error:?}"),
            };
            let doc = output.document();
            assert!(
                doc.get("description_1").is_none()
                    && doc.get("description_2").is_none()
                    && doc
                        .get("description_3")
                        .is_some_and(|d| d.kind() == ElementKind::Description),
                "{ROW}: {what} ({mode:?}): the id-less description is not description_3: {doc:?}"
            );
        }
    }
    let mut doc = read_ok(ROW, "a content id", &in_ap(holder));
    let created = match edit::apply(&mut doc, CreateDescription::new("p", en("en", "x"))) {
        Ok(mutation) => mutation.created_ids().to_vec(),
        Err(error) => panic!("{ROW}: CreateDescription was refused: {error:?}"),
    };
    assert_eq!(
        created,
        [ElementId::from("description_3")],
        "{ROW}: an edit generated a content or value id"
    );
}

#[test]
fn sacm23_base_004_a_caller_id_equal_to_a_content_or_value_id_is_duplicate_id() {
    let mut doc = read_ok(
        ROW,
        "content and value ids",
        &in_ap(
            "<note xmi:id=\"n\"><content xmi:id=\"w\"><value xmi:id=\"v\" content=\"x\"/>\
             </content></note>",
        ),
    );
    let before = doc.clone();
    for id in ["w", "v"] {
        let answers = [
            edit::apply(&mut doc, CreateNote::new("p", en("en", "x")).with_id(id)),
            edit::apply(
                &mut doc,
                CreateClaim::new("p", LangString::new("C")).with_id(id),
            ),
        ];
        for answer in answers {
            assert!(
                matches!(&answer, Err(EditError::DuplicateId { id: found, .. }) if found.as_str() == id),
                "{ROW}: creating {id:?} is not DuplicateId: {answer:?}"
            );
        }
    }
    assert_eq!(doc, before, "{ROW}: a refused edit changed the document");
}

// --- Item 9: refused, never dropped ------------------------------------------------

#[test]
fn sacm23_base_004_content_a_utility_element_cannot_hold_is_unsupported_where_it_starts() {
    let cases = [
        (
            "a name on a Description",
            in_description(&format!("{MARK}<name content=\"x\"/>")),
        ),
        (
            "a name on a Note",
            in_ap(&format!("<note xmi:id=\"n\">{MARK}<name/></note>")),
        ),
        (
            "a description in a Description",
            in_description(&format!("{MARK}<description xmi:id=\"e\"/>")),
        ),
        (
            "a note in a Description",
            in_description(&format!("{MARK}<note xmi:id=\"e\"/>")),
        ),
        (
            "a taggedValue in a Description",
            in_description(&format!("{MARK}<taggedValue xmi:id=\"e\"/>")),
        ),
        (
            "a key in a Description",
            in_description(&format!("{MARK}<key><value content=\"k\"/></key>")),
        ),
        (
            "an implementationConstraint in a Note",
            in_ap(&format!(
                "<note xmi:id=\"n\">{MARK}<implementationConstraint xmi:id=\"e\"/></note>"
            )),
        ),
        (
            "sacm:content",
            in_description(&format!("{MARK}<sacm:content/>")),
        ),
        (
            "sacm:value",
            description_with(&format!("{MARK}<sacm:value content=\"x\"/>")),
        ),
        (
            "another child of content",
            description_with(&format!("<value content=\"a\"/>{MARK}<foo/>")),
        ),
        (
            "a name in content",
            description_with(&format!("{MARK}<name content=\"x\"/>")),
        ),
        (
            "a lang element in content",
            description_with(&format!("{MARK}<lang>en</lang>")),
        ),
        (
            "a value in a value",
            description_with(&format!("<value>{MARK}<value/></value>")),
        ),
        (
            "an element in a value's lang element",
            description_with(&format!("<value><lang>en{MARK}<b/></lang></value>")),
        ),
    ];
    for (what, template) in &cases {
        assert_unsupported_at_mark(ROW, what, template);
    }
}

#[test]
fn sacm23_base_004_text_or_an_attribute_a_utility_element_cannot_hold_is_unsupported_where_it_starts()
 {
    let cases = [
        (
            "text after a value in content",
            description_with(&format!("<value content=\"a\"/> {MARK}x")),
        ),
        (
            "text in a value",
            description_with(&format!("<value lang=\"en\">{MARK}x</value>")),
        ),
        (
            "xml:lang on content",
            in_description(&format!("<content {MARK}xml:lang=\"en\"/>")),
        ),
        (
            "foo on content",
            in_description(&format!("<content {MARK}foo=\"1\"/>")),
        ),
        (
            "xml:lang on a value",
            description_with(&format!("<value content=\"x\" {MARK}xml:lang=\"en\"/>")),
        ),
        (
            "foo on a value",
            description_with(&format!("<value {MARK}foo=\"1\"/>")),
        ),
        (
            "an xmi:id on a value's content element",
            description_with(&format!(
                "<value><content {MARK}xmi:id=\"x\">x</content></value>"
            )),
        ),
        (
            "a foreign attribute on a Description",
            in_ap(&format!("<description xmi:id=\"d\" {MARK}foo=\"1\"/>")),
        ),
        (
            "an assertionDeclaration on a Note",
            in_ap(&format!(
                "<note xmi:id=\"n\" {MARK}assertionDeclaration=\"asserted\"/>"
            )),
        ),
    ];
    for (what, template) in &cases {
        assert_unsupported_at_mark(ROW, what, template);
    }
}

#[test]
fn sacm23_base_004_the_single_language_shorthand_is_unsupported_naming_lib_compat_008() {
    let cases = [
        (
            "<content lang=\"en\" content=\"x\"/>",
            in_description(&format!("<content {MARK}lang=\"en\" content=\"x\"/>")),
        ),
        (
            "<content content=\"x\"/>",
            in_description(&format!("<content {MARK}content=\"x\"/>")),
        ),
        (
            "<content>x</content>",
            in_description(&format!("<content>{MARK}x</content>")),
        ),
        (
            "<description content=\"x\"/>",
            in_ap(&format!("<description xmi:id=\"d\" {MARK}content=\"x\"/>")),
        ),
        (
            "<description>x</description>",
            in_ap(&format!("<description xmi:id=\"d\">{MARK}x</description>")),
        ),
        (
            "<note content=\"x\"/>",
            in_ap(&format!("<note xmi:id=\"n\" {MARK}content=\"x\"/>")),
        ),
    ];
    for (what, template) in &cases {
        let reason = assert_unsupported_at_mark(ROW, what, template);
        assert!(
            reason.contains("LIB-COMPAT-008"),
            "{ROW}: {what}: the reason {reason:?} does not name LIB-COMPAT-008"
        );
    }
}

// --- Item 10: comparison -----------------------------------------------------------

#[test]
fn sacm23_base_004_documents_differing_only_in_content_give_one_content_difference() {
    let doc = |what: &str, body: &str| read_ok(ROW, what, &in_ap(body));
    let absent = doc("absent", "<description xmi:id=\"d\"/>");
    let empty = doc(
        "empty",
        "<description xmi:id=\"d\"><content/></description>",
    );
    let ab = doc(
        "[en:a, fr:b]",
        "<description xmi:id=\"d\"><content><value lang=\"en\" content=\"a\"/>\
         <value lang=\"fr\" content=\"b\"/></content></description>",
    );
    let ba = doc(
        "[fr:b, en:a]",
        "<description xmi:id=\"d\"><content><value lang=\"fr\" content=\"b\"/>\
         <value lang=\"en\" content=\"a\"/></content></description>",
    );
    let no_lang = doc(
        "[x]",
        "<description xmi:id=\"d\"><content><value content=\"x\"/></content></description>",
    );
    let empty_lang = doc(
        "[\"\":x]",
        "<description xmi:id=\"d\"><content><value lang=\"\" content=\"x\"/></content></description>",
    );
    let cases = [
        ("None and empty", &absent, &empty),
        ("a different value order", &ab, &ba),
        ("lang absent and empty", &no_lang, &empty_lang),
    ];
    for (what, left, right) in cases {
        assert_ne!(left, right, "{ROW}: {what}: the documents are ==");
        let expected = Difference::Content {
            id: ElementId::from("d"),
            left: content_of(ROW, left, "d"),
            right: content_of(ROW, right, "d"),
        };
        assert_eq!(
            semantic_compare(left, right),
            std::slice::from_ref(&expected),
            "{ROW}: {what}: not one Content difference"
        );
        assert_eq!(
            expected.element_id(),
            Some(&ElementId::from("d")),
            "{ROW}: {what}: element_id() is not the Description"
        );
        let shown = expected.to_string();
        assert!(
            !shown.contains('\n') && shown.contains("\"d\"") && shown.contains("content"),
            "{ROW}: {what}: Display {shown:?} is not one line naming \"d\" and content"
        );
        let swapped = semantic_compare(right, left);
        assert!(
            matches!(swapped.as_slice(), [Difference::Content { .. }]),
            "{ROW}: {what}: the swapped comparison is not one Content: {swapped:?}"
        );
    }
    assert_eq!(
        semantic_compare(&ab, &ab.clone()),
        [],
        "{ROW}: equal contents differ"
    );
}

#[test]
fn sacm23_base_004_documents_differing_only_in_content_or_value_ids_give_content_id_or_content_value_ids()
 {
    let doc = |what: &str, content: &str| read_ok(ROW, what, &in_description(content));
    let w1 = doc(
        "w1",
        "<content xmi:id=\"w1\"><value content=\"x\"/></content>",
    );
    let w2 = doc(
        "w2",
        "<content xmi:id=\"w2\"><value content=\"x\"/></content>",
    );
    let none = doc("no ids", "<content><value content=\"x\"/></content>");
    let v1 = doc(
        "v1",
        "<content><value xmi:id=\"v1\" content=\"x\"/></content>",
    );
    let id = |value: &str| Some(ElementId::from(value));
    let d = || ElementId::from("d");
    let cases = [
        (
            "a content id present and absent",
            &w1,
            &none,
            Difference::ContentId {
                id: d(),
                left: id("w1"),
                right: None,
            },
        ),
        (
            "different content ids",
            &w1,
            &w2,
            Difference::ContentId {
                id: d(),
                left: id("w1"),
                right: id("w2"),
            },
        ),
        (
            "a value id absent and present",
            &none,
            &v1,
            Difference::ContentValueIds {
                id: d(),
                left: vec![None],
                right: vec![id("v1")],
            },
        ),
    ];
    for (what, left, right, expected) in cases {
        assert_ne!(left, right, "{ROW}: {what}: the documents are ==");
        assert_eq!(
            semantic_compare(left, right),
            std::slice::from_ref(&expected),
            "{ROW}: {what}: not the one id difference"
        );
        assert_eq!(
            expected.element_id(),
            Some(&d()),
            "{ROW}: {what}: element_id() is not the Description"
        );
        let shown = expected.to_string();
        assert!(
            !shown.contains('\n') && shown.contains("\"d\""),
            "{ROW}: {what}: Display {shown:?} is not one line naming \"d\""
        );
    }
    // Content, then its id, then its value ids, for one element.
    let all = doc(
        "all three",
        "<content xmi:id=\"w1\"><value xmi:id=\"v1\" content=\"y\"/></content>",
    );
    let differences = semantic_compare(&all, &none);
    assert!(
        matches!(
            differences.as_slice(),
            [
                Difference::Content { .. },
                Difference::ContentId { .. },
                Difference::ContentValueIds { .. }
            ]
        ),
        "{ROW}: not Content, ContentId, ContentValueIds in that order: {differences:?}"
    );
}

// --- Item 11: edits ----------------------------------------------------------------

/// Creates one `UtilityElement` of the role `role` under `parent` with
/// `content`, and an optional caller id.
fn create_operation(
    role: &str,
    parent: &str,
    content: MultiLangString,
    id: Option<&str>,
) -> Operation {
    match (role, id) {
        ("description", None) => CreateDescription::new(parent, content).into(),
        ("description", Some(id)) => CreateDescription::new(parent, content).with_id(id).into(),
        ("implementationConstraint", None) => {
            CreateImplementationConstraint::new(parent, content).into()
        }
        ("implementationConstraint", Some(id)) => {
            CreateImplementationConstraint::new(parent, content)
                .with_id(id)
                .into()
        }
        ("note", None) => CreateNote::new(parent, content).into(),
        ("note", Some(id)) => CreateNote::new(parent, content).with_id(id).into(),
        (other, _) => panic!("{ROW}: test oracle: no create operation for {other:?}"),
    }
}

/// A document built by edits: an AssuranceCasePackage `case`, an
/// ArgumentPackage `arguments` in it, and a Claim `c1` in that.
fn built() -> Document {
    let mut doc = Document::new();
    for operation in [
        Operation::from(CreateAssuranceCasePackage::root(en("en", "Case")).with_id("case")),
        CreateArgumentPackage::new("case", en("en", "Arguments"))
            .with_id("arguments")
            .into(),
        CreateClaim::new("arguments", en("en", "Claim"))
            .with_id("c1")
            .into(),
    ] {
        if let Err(error) = edit::apply(&mut doc, operation) {
            panic!("{ROW}: setup: {error:?}");
        }
    }
    doc
}

#[test]
fn sacm23_base_004_create_operations_create_with_generated_ids_and_report_their_fields() {
    let content = mls(&[en("en", "x"), en("sv", "y")]);
    for (role, kind, snake) in ROLES {
        let mut doc = built();
        for (n, parent) in ["case", "arguments", "c1"].into_iter().enumerate() {
            let mutation = match edit::apply(
                &mut doc,
                create_operation(role, parent, content.clone(), None),
            ) {
                Ok(mutation) => mutation,
                Err(error) => panic!("{ROW}: creating a {role} under {parent}: {error:?}"),
            };
            let expected = format!("{snake}_{}", n + 1);
            assert_eq!(
                (
                    mutation.created_ids(),
                    mutation.changed_ids(),
                    mutation.deleted_ids()
                ),
                (
                    [ElementId::from(expected.as_str())].as_slice(),
                    [].as_slice(),
                    [].as_slice()
                ),
                "{ROW}: a {role} under {parent}: the mutation's ids"
            );
            let u = element(ROW, &doc, &expected);
            assert_eq!(
                (
                    u.kind(),
                    u.parent().map(|p| p.id().as_str().to_owned()),
                    u.content().cloned(),
                    u.content_id(),
                    u.content_value_ids().to_vec(),
                    u.gid(),
                    u.name()
                ),
                (
                    kind,
                    Some(parent.to_owned()),
                    Some(content.clone()),
                    None,
                    vec![None, None],
                    None,
                    None
                ),
                "{ROW}: a {role} under {parent}: the created element"
            );
            assert_eq!(
                role_ids(ROW, element(ROW, &doc, parent), role),
                std::slice::from_ref(&expected),
                "{ROW}: a {role} under {parent}: the parent's accessor"
            );
        }
        let created = create_operation(role, "c1", content.clone(), Some("mine"));
        assert!(
            edit::apply(&mut doc, created)
                .is_ok_and(|m| m.created_ids() == [ElementId::from("mine")]),
            "{ROW}: a {role} with a caller id is not created with it"
        );
    }
}

#[test]
fn sacm23_base_004_create_operations_report_their_parent_id_and_content_and_their_operation_name() {
    let content = MultiLangString::from(en("en", "x"));
    assert_eq!(
        content.values(),
        [en("en", "x")],
        "{ROW}: MultiLangString::from(LangString) is not that one value"
    );
    let description = CreateDescription::new("c1", content.clone());
    assert_eq!(
        (
            description.parent(),
            description.id(),
            description.content()
        ),
        (&ElementId::from("c1"), None, &content),
        "{ROW}: CreateDescription's fields"
    );
    let note = CreateNote::new("c1", en("en", "x")).with_id("n");
    assert_eq!(
        (note.parent(), note.id(), note.content()),
        (
            &ElementId::from("c1"),
            Some(&ElementId::from("n")),
            &content
        ),
        "{ROW}: CreateNote's fields"
    );
    let constraint = CreateImplementationConstraint::new("c1", MultiLangString::default());
    assert_eq!(
        (constraint.parent(), constraint.id(), constraint.content()),
        (&ElementId::from("c1"), None, &MultiLangString::default()),
        "{ROW}: CreateImplementationConstraint's fields"
    );
    for (operation, name) in [
        (Operation::from(description), "CreateDescription"),
        (
            Operation::from(constraint),
            "CreateImplementationConstraint",
        ),
        (Operation::from(note), "CreateNote"),
    ] {
        assert_eq!(
            operation.operation_name(),
            name,
            "{ROW}: operation_name() of {operation:?}"
        );
    }
}

/// A document holding an element `p` of `kind`: a root if it is a package,
/// otherwise in an ArtifactPackage root, which admits every `ModelElement`
/// (ADR 0014).
fn holding(kind: ElementKind) -> Document {
    let mut doc = Document::new();
    let p = ElementId::from("p");
    let result = if kind.is_package() {
        doc.insert(None, kind, p, None)
    } else {
        let root = ElementId::from("root");
        doc.insert(None, ElementKind::ArtifactPackage, root.clone(), None)
            .and_then(|()| doc.insert(Some(&root), kind, p, None))
    };
    if let Err(error) = result {
        panic!("{ROW}: setup could not insert the {kind} \"p\": {error}");
    }
    doc
}

#[test]
fn sacm23_base_004_create_operations_accept_every_model_element_parent_and_refuse_a_utility_element()
 {
    let mut model_elements = 0;
    for &parent in ElementKind::ALL {
        for (role, kind, _) in ROLES {
            let mut doc = holding(parent);
            let answer = edit::apply(
                &mut doc,
                create_operation(role, "p", mls(&[en("en", "x")]), Some("u")),
            );
            if parent.is_a(AbstractClass::ModelElement) {
                assert!(
                    answer.is_ok(),
                    "{ROW}: a {role} under a {parent} is refused: {answer:?}"
                );
                assert_eq!(
                    role_ids(ROW, element(ROW, &doc, "p"), role),
                    ["u"],
                    "{ROW}: a {role} under a {parent} is not in its role"
                );
            } else {
                let before = doc.clone();
                assert!(
                    matches!(
                        &answer,
                        Err(EditError::InvalidParent { parent: p, parent_kind, kind: k, .. })
                            if p.as_str() == "p" && *parent_kind == parent && *k == kind
                    ),
                    "{ROW}: a {role} under a {parent} is not InvalidParent: {answer:?}"
                );
                assert_eq!(doc, before, "{ROW}: a refusal changed the document");
            }
        }
        if parent.is_a(AbstractClass::ModelElement) {
            model_elements += 1;
        }
    }
    assert_eq!(model_elements, 34, "{ROW}: setup: the ModelElement kinds");
}

#[test]
fn sacm23_base_004_create_operations_refuse_parent_not_found_invalid_id_and_duplicate_id_in_order()
{
    for (role, _, _) in ROLES {
        let mut doc = built();
        let content = || mls(&[en("en", "x")]);
        let before = doc.clone();
        let refusals = [
            (
                "an unknown parent",
                create_operation(role, "nowhere", content(), Some("1bad")),
            ),
            (
                "an id that is not an NCName",
                create_operation(role, "c1", content(), Some("1bad")),
            ),
            (
                "an id already in the document",
                create_operation(role, "c1", content(), Some("arguments")),
            ),
        ];
        let answers: Vec<_> = refusals
            .into_iter()
            .map(|(what, operation)| (what, edit::apply(&mut doc, operation)))
            .collect();
        assert!(
            matches!(&answers[0].1, Err(EditError::ParentNotFound { parent, .. }) if parent.as_str() == "nowhere"),
            "{ROW}: {role}: {}: {:?}",
            answers[0].0,
            answers[0].1
        );
        assert!(
            matches!(&answers[1].1, Err(EditError::InvalidId { id, .. }) if id.as_str() == "1bad"),
            "{ROW}: {role}: {}: {:?}",
            answers[1].0,
            answers[1].1
        );
        assert!(
            matches!(&answers[2].1, Err(EditError::DuplicateId { id, .. }) if id.as_str() == "arguments"),
            "{ROW}: {role}: {}: {:?}",
            answers[2].0,
            answers[2].1
        );
        assert_eq!(
            doc, before,
            "{ROW}: {role}: a refused edit changed the document"
        );
        // A UtilityElement parent is InvalidParent before the bad id.
        let mut with_note = doc.clone();
        if let Err(error) = edit::apply(
            &mut with_note,
            CreateNote::new("c1", content()).with_id("n"),
        ) {
            panic!("{ROW}: setup: {error:?}");
        }
        let answer = edit::apply(
            &mut with_note,
            create_operation(role, "n", content(), Some("1bad")),
        );
        assert!(
            matches!(
                &answer,
                Err(EditError::InvalidParent {
                    parent_kind: ElementKind::Note,
                    ..
                })
            ),
            "{ROW}: {role}: under a Note, with a bad id: {answer:?}"
        );
    }
}

#[test]
fn sacm23_base_004_a_document_built_by_the_create_operations_writes_reads_back_equal_and_validates()
{
    let mut doc = built();
    let operations = [
        create_operation("description", "case", mls(&[en("en", "A case")]), None),
        create_operation(
            "note",
            "arguments",
            mls(&[en("en", "n"), en("sv", "n"), ls(None, Some(""))]),
            None,
        ),
        create_operation(
            "implementationConstraint",
            "c1",
            mls(&[ls(Some(""), None)]),
            Some("ic"),
        ),
        create_operation("description", "c1", mls(&[en("en", "Säkert ✓")]), None),
    ];
    for operation in operations {
        if let Err(error) = edit::apply(&mut doc, operation) {
            panic!("{ROW}: setup: {error:?}");
        }
    }
    let found = validation::validate(&doc);
    assert!(
        !diagnostic::has_errors(&found),
        "{ROW}: a built document has validation Errors: {found:?}"
    );
    assert_round_trip(ROW, "a built document", &doc, Mode::Strict);
}

#[test]
fn sacm23_base_004_a_character_xml_cannot_represent_in_a_content_value_fails_the_write() {
    for (field, value, attribute) in [
        (
            "lang",
            ls(Some("e\u{1}n"), Some("x")),
            WriteAttribute::ContentValueLang,
        ),
        (
            "content",
            ls(Some("sv"), Some("x\u{FFFE}")),
            WriteAttribute::ContentValueContent,
        ),
    ] {
        let mut doc = built();
        if let Err(error) = edit::apply(
            &mut doc,
            CreateDescription::new("c1", mls(&[en("en", "ok"), value])).with_id("d"),
        ) {
            panic!("{ROW}: setup: {error:?}");
        }
        for mode in MODES {
            let error = match xmi::write(&doc, &WriteOptions::default().with_mode(mode)) {
                Ok(text) => panic!("{ROW}: a {field} the XML cannot hold was written:\n{text}"),
                Err(error) => error,
            };
            assert!(
                matches!(
                    &error,
                    WriteError::UnrepresentableCharacter { element, attribute: found, .. }
                        if element.as_str() == "d" && *found == attribute
                ),
                "{ROW}: a value's {field} ({mode:?}): not UnrepresentableCharacter on d, \
                 {attribute:?}: {error:?}"
            );
            assert_eq!(
                error.element(),
                Some(&ElementId::from("d")),
                "{ROW}: element() is not the Description"
            );
        }
        let shown = attribute.to_string();
        assert!(
            shown.contains("content") && shown.contains(field),
            "{ROW}: WriteAttribute {attribute:?} displays as {shown:?}"
        );
    }
}

// --- SACM23-VAL-020: content and value ids that are not NCNames ----------------------

#[test]
fn sacm23_val_020_a_content_or_value_id_that_is_not_an_ncname_is_one_id_002_on_the_utility_element()
{
    for (what, content, bad) in [
        (
            "a content id",
            "<content xmi:id=\"1c\"><value content=\"x\"/></content>",
            "1c",
        ),
        (
            "a value id",
            "<content><value xmi:id=\"a b\" content=\"x\"/></content>",
            "a b",
        ),
        (
            "a second value's id",
            "<content><value content=\"x\"/><value xmi:id=\"a:b\" lang=\"sv\" content=\"y\"/></content>",
            "a:b",
        ),
    ] {
        let doc = read_ok(VAL_020, what, &in_description(content));
        let found = validation::validate(&doc);
        let [finding] = found.as_slice() else {
            panic!("{VAL_020}: {what}: not one finding: {found:?}");
        };
        assert_eq!(
            (
                finding.code(),
                finding.severity(),
                finding.requirement_id(),
                finding.clause()
            ),
            (Code::ID_INVALID, Severity::Error, VAL_020, Some("7.6.1")),
            "{VAL_020}: {what}: not the SACM-ID-002 Error"
        );
        assert_eq!(
            (finding.affected(), finding.location()),
            ([ElementId::from("d")].as_slice(), None),
            "{VAL_020}: {what}: not affecting the Description, with no location"
        );
        let quoted = format!("{bad:?}");
        assert!(
            finding.message().contains(&quoted),
            "{VAL_020}: {what}: the message {:?} does not quote {quoted}",
            finding.message()
        );
        let error = match xmi::write(&doc, &WriteOptions::default()) {
            Ok(text) => panic!("{VAL_020}: {what}: a strict write succeeded:\n{text}"),
            Err(error) => error,
        };
        assert!(
            matches!(&error, WriteError::Invalid { .. }) && error.diagnostics() == found.as_slice(),
            "{VAL_020}: {what}: the strict refusal does not hold the finding: {error:?}"
        );
        let text = write_in(VAL_020, what, Mode::Tolerant, &doc);
        let back = read_ok(VAL_020, what, &text);
        assert_eq!(
            back, doc,
            "{VAL_020}: {what}: a tolerant write does not keep the id"
        );
    }
    // Two bad ids on one element: two findings, both on it.
    let doc = read_ok(
        VAL_020,
        "two bad ids",
        &in_description("<content xmi:id=\"1c\"><value xmi:id=\"-v\" content=\"x\"/></content>"),
    );
    let found = validation::validate(&doc);
    assert_eq!(
        found
            .iter()
            .map(|f| (f.code(), f.affected().to_vec()))
            .collect::<Vec<_>>(),
        vec![(Code::ID_INVALID, vec![ElementId::from("d")]); 2],
        "{VAL_020}: two bad ids on one Description are not two findings on it"
    );
}

// --- SACM23-XMI-004: content and value ids are written escaped ------------------------

#[test]
fn sacm23_xmi_004_a_content_or_value_id_is_written_escaped_and_reads_back_exactly() {
    let written = "a&amp;b&lt;c&gt;d&quot;e&#9;f&#10;g&#13;h";
    let held = "a&b<c>d\"e\tf\ng\rh";
    for (what, content, line) in [
        (
            "a content id",
            format!("<content xmi:id=\"{written}\"><value content=\"x\"/></content>"),
            format!("<content xmi:id=\"{written}\">"),
        ),
        (
            "a value id",
            format!("<content><value xmi:id=\"{written}\" content=\"x\"/></content>"),
            format!("<value xmi:id=\"{written}\" content=\"x\" />"),
        ),
    ] {
        let doc = match read(Mode::Tolerant, &in_description(&content)) {
            Ok(output) => output.into_document(),
            Err(error) => panic!("{XMI_004}: {what}: refused: {error:?}"),
        };
        let d = element(XMI_004, &doc, "d");
        let read_id = d
            .content_id()
            .or_else(|| d.content_value_ids().first().and_then(Option::as_ref))
            .map(|id| id.as_str().to_owned());
        assert_eq!(
            read_id.as_deref(),
            Some(held),
            "{XMI_004}: {what}: setup: not read unescaped"
        );
        // Not an NCName: a strict write refuses it, a tolerant one keeps it.
        let text = write_in(XMI_004, what, Mode::Tolerant, &doc);
        assert!(
            lines(&text).contains(&line.as_str()),
            "{XMI_004}: {what}: the output has no line {line:?}:\n{text}"
        );
        let back = match read(Mode::Tolerant, &text) {
            Ok(output) => output.into_document(),
            Err(error) => panic!("{XMI_004}: {what}: the output does not read back: {error:?}"),
        };
        assert_eq!(
            back, doc,
            "{XMI_004}: {what}: the id does not read back exactly"
        );
    }
}

#[test]
fn sacm23_xmi_004_an_edit_built_content_and_its_values_are_written_without_an_xmi_id() {
    let mut doc = built();
    if let Err(error) = edit::apply(
        &mut doc,
        CreateDescription::new("c1", mls(&[en("en", "x"), en("sv", "y")])),
    ) {
        panic!("{XMI_004}: setup: {error:?}");
    }
    let text = write_in(XMI_004, "a built content", Mode::Strict, &doc);
    let value_lines: Vec<&str> = lines(&text)
        .into_iter()
        .filter(|line| line.starts_with("<content") || line.starts_with("<value"))
        .collect();
    assert_eq!(
        value_lines,
        [
            "<content>",
            "<value lang=\"en\" content=\"x\" />",
            "<value lang=\"sv\" content=\"y\" />"
        ],
        "{XMI_004}: an edit-built content is not written without ids:\n{text}"
    );
}
