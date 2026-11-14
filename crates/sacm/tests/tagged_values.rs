//! Tests for `TaggedValue`, the key/value `UtilityElement` of SACM 2.3 §8.12
//! (SACM23-BASE-005; §8.7; XMI 2.5.1 Annex B.5.2; ADR 0025).
//!
//! A `TaggedValue` in `taggedValue` on every `ModelElement` is an element in the
//! one ordered child list, with `key()` and `content()`, each absent or a
//! `MultiLangString` under the SACM23-BASE-004 rules. `key` and `content` are
//! read in either order and written `content` first. A second `key` or
//! `content` is `RepeatedValue`; the `key=`/`value=` attribute shorthand stays
//! `Unsupported` until LIB-COMPAT-008. `CreateTaggedValue` creates one.
//!
//! Also here, for the rows this slice rewords: SACM23-VAL-020 (a `key` or key
//! `value` id that is not an `NCName` is one SACM-ID-002 on the `TaggedValue`) and
//! SACM23-XMI-004 (such an id is written escaped and reads back exactly).
//!
//! A `TaggedValue` with no `key` is a validation Error: that is SACM23-VAL-010,
//! in `multi_lang_string_validation.rs`.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. The written form is checked line by line
//! with the indentation trimmed.

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{self, Code, Severity, SourceLocation};
use sacm::edit::{
    self, CreateArgumentPackage, CreateClaim, CreateNote, CreateTaggedValue, EditError, Operation,
};
use sacm::model::{
    AbstractClass, Document, ElementId, ElementKind, LangString, MultiLangString, SacmElement,
};
use sacm::validation;
use sacm::xmi::{
    self, Mode, ReadError, ReadOptions, ReadOutput, WriteAttribute, WriteError, WriteOptions,
};

const ROW: &str = "SACM23-BASE-005";
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

/// The `key` and `content` of the `TaggedValue` most tests use.
const KEY: &str = "<key><value content=\"k\"/></key>";
const CONTENT: &str = "<content><value lang=\"en\" content=\"v\"/></content>";

// --- Inputs ------------------------------------------------------------------

/// An ArgumentPackage root `p` named "P", holding `body` from line 3.
fn in_ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  {body}\n\
         </sacm:ArgumentPackage>"
    )
}

/// A `TaggedValue` `t` holding `body`, in [`in_ap`].
fn tagged(body: &str) -> String {
    in_ap(&format!("<taggedValue xmi:id=\"t\">{body}</taggedValue>"))
}

/// The readable parents: what, the parent's kind, and the input with a
/// parent `p` named "P" holding `body`.
fn contexts(body: &str) -> Vec<(&'static str, ElementKind, String)> {
    let named = |open: &str, close: &str| format!("{open}<name content=\"P\"/>{body}{close}");
    let in_case = |inner: String| {
        format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>{inner}\
             </sacm:AssuranceCasePackage>"
        )
    };
    let in_arguments = |inner: String| {
        format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"arguments\"><name content=\"A\"/>{inner}\
             </sacm:ArgumentPackage>"
        )
    };
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
            "an AssuranceCasePackage child",
            ElementKind::AssuranceCasePackage,
            in_case(named(
                "<assuranceCasePackage xmi:id=\"p\">",
                "</assuranceCasePackage>",
            )),
        ),
        (
            "an ArgumentPackage in argumentPackage",
            ElementKind::ArgumentPackage,
            in_case(named(
                "<argumentPackage xmi:id=\"p\">",
                "</argumentPackage>",
            )),
        ),
        (
            "an ArgumentPackage in argumentElement",
            ElementKind::ArgumentPackage,
            in_arguments(named(
                "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"p\">",
                "</argumentElement>",
            )),
        ),
        (
            "a Claim",
            ElementKind::Claim,
            in_arguments(named(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"p\">",
                "</argumentElement>",
            )),
        ),
    ]
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
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

/// Asserts that `template` is refused as `Unsupported`, with no code, at its
/// one mark, in both modes. Returns the reason.
fn assert_unsupported_at_mark(what: &str, template: &str) -> String {
    let (input, marks) = marked(template);
    assert_eq!(marks.len(), 1, "{ROW}: setup: {what} has not one mark");
    let error = refused(what, &input);
    let ReadError::Unsupported { reason, .. } = &error else {
        panic!("{ROW}: {what}: not Unsupported: {error:?} ({error})");
    };
    assert_eq!(
        (error.code(), error.location()),
        (None, Some(marks[0])),
        "{ROW}: {what}: not refused with no code at the mark: {error}"
    );
    reason.clone()
}

fn write_in(row: &str, what: &str, mode: Mode, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// Writes `doc` in `mode`, reads the output back, and asserts that it reads
/// equal and writes the same bytes again. Returns the output.
fn assert_round_trip(what: &str, doc: &Document, mode: Mode) -> String {
    let text = write_in(ROW, what, mode, doc);
    let back = match read(Mode::Tolerant, &text) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: {what}: the output does not read back: {error:?}\n{text}"),
    };
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{ROW}: {what}: the output reads back different\n{text}"
    );
    assert_eq!(
        back, *doc,
        "{ROW}: {what}: the output reads back !=\n{text}"
    );
    assert_eq!(
        write_in(ROW, what, mode, &back),
        text,
        "{ROW}: {what}: writing what was read back gives other bytes"
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

/// A document built by edits: an ArgumentPackage `arguments` holding a Claim
/// `c1`.
fn built() -> Document {
    let mut doc = Document::new();
    let operations: [Operation; 2] = [
        CreateArgumentPackage::root(LangString::new("Arguments").with_lang("en"))
            .with_id("arguments")
            .into(),
        CreateClaim::new("arguments", LangString::new("Claim").with_lang("en"))
            .with_id("c1")
            .into(),
    ];
    for operation in operations {
        if let Err(error) = edit::apply(&mut doc, operation) {
            panic!("{ROW}: setup: {error:?}");
        }
    }
    doc
}

// --- Item 1: read and write, content first ------------------------------------------

#[test]
fn sacm23_base_005_a_tagged_value_reads_on_each_readable_kind_and_is_written_content_first() {
    let body = format!("<taggedValue xmi:id=\"t\">{KEY}{CONTENT}</taggedValue>");
    let reversed = format!("<taggedValue xmi:id=\"t\">{CONTENT}{KEY}</taggedValue>");
    let reversed_contexts = contexts(&reversed);
    for (index, (context, parent_kind, input)) in contexts(&body).into_iter().enumerate() {
        let what = format!("a TaggedValue in {context}");
        let doc = read_ok(ROW, &what, &input);
        let t = element(ROW, &doc, "t");
        assert_eq!(
            (
                t.kind(),
                t.parent().map(|p| (p.id().as_str().to_owned(), p.kind())),
                t.key().cloned(),
                t.content().cloned(),
                t.name()
            ),
            (
                ElementKind::TaggedValue,
                Some(("p".to_owned(), parent_kind)),
                Some(mls(&[LangString::new("k")])),
                Some(mls(&[LangString::new("v").with_lang("en")])),
                None
            ),
            "{ROW}: {what}: the TaggedValue as read"
        );
        assert_eq!(
            ids(element(ROW, &doc, "p").tagged_values()),
            ["t"],
            "{ROW}: {what}: tagged_values() of the parent"
        );
        let text = assert_round_trip(&what, &doc, Mode::Strict);
        assert!(
            has_lines(
                &text,
                &[
                    "<taggedValue xmi:id=\"t\">",
                    "<content>",
                    "<value lang=\"en\" content=\"v\" />",
                    "</content>",
                    "<key>",
                    "<value content=\"k\" />",
                    "</key>",
                    "</taggedValue>"
                ]
            ),
            "{ROW}: {what}: not written content first, then key:\n{text}"
        );
        let other = read_ok(ROW, &what, &reversed_contexts[index].2);
        assert_eq!(
            semantic_compare(&other, &doc),
            [],
            "{ROW}: {what}: content before key does not read equal"
        );
    }
}

#[test]
fn sacm23_base_005_a_key_a_content_and_their_values_are_indented_one_level_below_their_owner() {
    let doc = read_ok(
        ROW,
        "a TaggedValue of a Claim",
        &in_ap(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"><name content=\"C\"/>\
             <taggedValue xmi:id=\"t\"><key><value lang=\"en\" content=\"k\"/>\
             <value lang=\"sv\" content=\"n\"/></key>\
             <content><value content=\"v\"/></content></taggedValue></argumentElement>",
        ),
    );
    let text = write_in(ROW, "a TaggedValue of a Claim", Mode::Strict, &doc);
    let expected = [
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\">",
        "    <name content=\"C\" />",
        "    <taggedValue xmi:id=\"t\">",
        "      <content>",
        "        <value content=\"v\" />",
        "      </content>",
        "      <key>",
        "        <value lang=\"en\" content=\"k\" />",
        "        <value lang=\"sv\" content=\"n\" />",
        "      </key>",
        "    </taggedValue>",
        "  </argumentElement>",
    ];
    assert!(
        text.lines()
            .collect::<Vec<_>>()
            .windows(expected.len())
            .any(|window| window == expected),
        "{ROW}: a key, a content and their values at depths 3 and 4 are not indented one level \
         below their owner:\n{text}"
    );
}

// --- Item 2: three key states ---------------------------------------------------------

#[test]
fn sacm23_base_005_absent_empty_and_default_keys_are_three_states_that_round_trip() {
    let states = [
        ("no key", String::new(), None, None),
        (
            "<key/>",
            "<key/>".to_owned(),
            Some(MultiLangString::default()),
            Some(vec!["<key />"]),
        ),
        (
            "<key><value/></key>",
            "<key><value/></key>".to_owned(),
            Some(mls(&[LangString::default()])),
            Some(vec!["<key>", "<value />", "</key>"]),
        ),
    ];
    let mut documents = Vec::new();
    for (what, key, expected, written) in states {
        let doc = read_ok(ROW, what, &tagged(&format!("{key}{CONTENT}")));
        let t = element(ROW, &doc, "t");
        assert_eq!(t.key(), expected.as_ref(), "{ROW}: {what}: key()");
        assert_eq!(
            (t.key_id(), t.key_value_ids().len()),
            (None, expected.as_ref().map_or(0, MultiLangString::len)),
            "{ROW}: {what}: the key's ids"
        );
        // Tolerant: a missing or empty key is a validation Error (SACM23-VAL-010).
        let text = assert_round_trip(what, &doc, Mode::Tolerant);
        match written {
            Some(written) => assert!(
                has_lines(&text, &written),
                "{ROW}: {what}: not written as {written:?}:\n{text}"
            ),
            None => assert!(
                !text.contains("<key"),
                "{ROW}: {what}: a key is written for an absent one:\n{text}"
            ),
        }
        documents.push(doc);
    }
    for (left, right) in [(0, 1), (0, 2), (1, 2)] {
        let differences = semantic_compare(&documents[left], &documents[right]);
        assert!(
            matches!(differences.as_slice(), [Difference::Key { id, .. }] if id.as_str() == "t"),
            "{ROW}: key states {left} and {right} are not one Key difference: {differences:?}"
        );
    }
}

// --- Item 3: equal keys ----------------------------------------------------------------

#[test]
fn sacm23_base_005_two_tagged_values_with_the_same_key_are_both_kept_in_order() {
    let doc = read_ok(
        ROW,
        "two TaggedValues with key k",
        &in_ap(&format!(
            "<taggedValue xmi:id=\"t2\">{KEY}<content><value content=\"2\"/></content></taggedValue>\
             <taggedValue xmi:id=\"t1\">{KEY}<content><value content=\"1\"/></content></taggedValue>"
        )),
    );
    let p = element(ROW, &doc, "p");
    assert_eq!(
        ids(p.tagged_values()),
        ["t2", "t1"],
        "{ROW}: both TaggedValues are not kept in document order"
    );
    let keys: Vec<_> = p.tagged_values().map(|t| t.key().cloned()).collect();
    assert_eq!(
        keys,
        vec![Some(mls(&[LangString::new("k")])); 2],
        "{ROW}: the equal keys are not both kept"
    );
    assert_round_trip("two TaggedValues with key k", &doc, Mode::Strict);
}

// --- Item 4: refusals ---------------------------------------------------------------

#[test]
fn sacm23_base_005_a_second_key_or_content_is_repeated_value_in_both_modes() {
    for (what, body, property) in [
        (
            "two keys",
            format!("{MARK}{KEY}{CONTENT}\n  {MARK}{KEY}"),
            "key",
        ),
        ("two empty keys", format!("{MARK}<key/>{MARK}<key/>"), "key"),
        (
            "two contents around a key",
            format!("{MARK}{CONTENT}{KEY}{MARK}{CONTENT}"),
            "content",
        ),
    ] {
        let (input, marks) = marked(&tagged(&body));
        let error = refused(what, &input);
        assert!(
            matches!(
                &error,
                ReadError::RepeatedValue { element, property: found, .. }
                    if element.as_str() == "t" && found == property
            ),
            "{ROW}: {what}: not RepeatedValue {{ t, {property:?} }}: {error:?}"
        );
        assert_eq!(
            (error.code(), error.locations()),
            (Some(Code::MULTIPLICITY_VIOLATION), marks.as_slice()),
            "{ROW}: {what}: not SACM-MULT-001 at both occurrences"
        );
    }
}

#[test]
fn sacm23_base_005_the_key_and_value_shorthands_are_unsupported_naming_lib_compat_008() {
    let cases = [
        (
            "key= on taggedValue",
            in_ap(&format!(
                "<taggedValue xmi:id=\"t\" {MARK}key=\"k\">{CONTENT}</taggedValue>"
            )),
        ),
        (
            "value= on taggedValue",
            in_ap(&format!(
                "<taggedValue xmi:id=\"t\" {MARK}value=\"v\">{KEY}</taggedValue>"
            )),
        ),
        (
            "<key lang content>",
            tagged(&format!("<key {MARK}lang=\"en\" content=\"k\"/>{CONTENT}")),
        ),
        (
            "<key>k</key>",
            tagged(&format!("<key>{MARK}k</key>{CONTENT}")),
        ),
    ];
    for (what, template) in &cases {
        let reason = assert_unsupported_at_mark(what, template);
        assert!(
            reason.contains("LIB-COMPAT-008"),
            "{ROW}: {what}: the reason {reason:?} does not name LIB-COMPAT-008"
        );
    }
}

#[test]
fn sacm23_base_005_key_or_value_attributes_off_a_tagged_value_and_another_on_one_are_only_unread() {
    let claim = |attribute: &str| {
        in_ap(&format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\" {MARK}{attribute}/>"
        ))
    };
    let description =
        |attribute: &str| in_ap(&format!("<description xmi:id=\"d\" {MARK}{attribute}/>"));
    let unread = |attribute: &str, element: &str| {
        format!(
            "the attribute \"{attribute}\" of the element \"{element}\" is not read in this release"
        )
    };
    let cases = [
        (
            "key= on a Description",
            description("key=\"k\""),
            unread("key", "description"),
        ),
        (
            "value= on a Description",
            description("value=\"v\""),
            unread("value", "description"),
        ),
        (
            "key= on a Claim",
            claim("key=\"k\""),
            unread("key", "argumentElement"),
        ),
        (
            "value= on a Claim",
            claim("value=\"v\""),
            unread("value", "argumentElement"),
        ),
        (
            "foo= on a TaggedValue",
            in_ap(&format!(
                "<taggedValue xmi:id=\"t\" {MARK}foo=\"1\">{KEY}{CONTENT}</taggedValue>"
            )),
            unread("foo", "taggedValue"),
        ),
    ];
    for (what, template, expected) in &cases {
        let reason = assert_unsupported_at_mark(what, template);
        assert_eq!(
            reason, *expected,
            "{ROW}: {what}: not an ordinary unread attribute"
        );
        assert!(
            !reason.contains("LIB-COMPAT-008"),
            "{ROW}: {what}: the reason {reason:?} names LIB-COMPAT-008"
        );
    }
}

#[test]
fn sacm23_base_005_content_a_tagged_value_cannot_hold_is_unsupported_where_it_starts() {
    let cases = [
        (
            "a taggedValue in a TaggedValue",
            tagged(&format!(
                "{KEY}{MARK}<taggedValue xmi:id=\"u\">{KEY}</taggedValue>"
            )),
        ),
        (
            "a note in a TaggedValue",
            tagged(&format!("{KEY}{MARK}<note xmi:id=\"u\"/>")),
        ),
        (
            "a name on a TaggedValue",
            tagged(&format!("{MARK}<name content=\"x\"/>{KEY}")),
        ),
        ("sacm:key", tagged(&format!("{MARK}<sacm:key/>"))),
        (
            "text after a key value",
            tagged(&format!("<key><value content=\"k\"/>{MARK}x</key>")),
        ),
        ("foo on key", tagged(&format!("<key {MARK}foo=\"1\"/>"))),
        (
            "xml:lang on a key value",
            tagged(&format!(
                "<key><value {MARK}xml:lang=\"en\" content=\"k\"/></key>"
            )),
        ),
        (
            "another child of key",
            tagged(&format!("<key>{MARK}<foo/></key>")),
        ),
        (
            "a type naming another class on key",
            tagged(&format!("{MARK}<key xmi:type=\"sacm:LangString\"/>")),
        ),
    ];
    for (what, template) in &cases {
        assert_unsupported_at_mark(what, template);
    }
    // The type attributes naming the classes are accepted.
    let typed = read_ok(
        ROW,
        "typed",
        &in_ap(
            "<taggedValue xmi:type=\"sacm:TaggedValue\" xmi:id=\"t\">\
             <key xmi:type=\"sacm:MultiLangString\"><value xmi:type=\"sacm:LangString\" content=\"k\"/></key>\
             </taggedValue>",
        ),
    );
    let plain = read_ok(ROW, "untyped", &tagged(KEY));
    assert_eq!(
        semantic_compare(&typed, &plain),
        [],
        "{ROW}: the typed TaggedValue does not read equal to the untyped one"
    );
}

// --- Item 5: key ids -------------------------------------------------------------------

#[test]
fn sacm23_base_005_key_ids_are_kept_by_role_and_position_and_written_first() {
    let doc = read_ok(
        ROW,
        "key and content ids",
        &tagged(
            "<key xmi:id=\"kw\"><value content=\"k\"/><value lang=\"sv\" content=\"n\" xmi:id=\"kv\"/></key>\
             <content xmi:id=\"cw\"><value xmi:id=\"cv\" content=\"v\"/></content>",
        ),
    );
    let t = element(ROW, &doc, "t");
    assert_eq!(
        (
            t.key_id().map(ElementId::as_str),
            t.key_value_ids().to_vec(),
            t.content_id().map(ElementId::as_str),
            t.content_value_ids().to_vec()
        ),
        (
            Some("kw"),
            some_ids(&[None, Some("kv")]),
            Some("cw"),
            some_ids(&[Some("cv")])
        ),
        "{ROW}: the key's and the content's ids are not each kept at their role and position"
    );
    for id in ["kw", "kv", "cw", "cv"] {
        assert!(doc.get(id).is_none(), "{ROW}: the id {id:?} is an element");
    }
    let text = assert_round_trip("key and content ids", &doc, Mode::Strict);
    assert!(
        has_lines(
            &text,
            &[
                "<taggedValue xmi:id=\"t\">",
                "<content xmi:id=\"cw\">",
                "<value xmi:id=\"cv\" content=\"v\" />",
                "</content>",
                "<key xmi:id=\"kw\">",
                "<value content=\"k\" />",
                "<value xmi:id=\"kv\" lang=\"sv\" content=\"n\" />",
                "</key>",
                "</taggedValue>"
            ]
        ),
        "{ROW}: the ids are not written first on their elements:\n{text}"
    );
    for (what, template, id) in [
        (
            "a key id repeating the content id",
            tagged(&format!(
                "{MARK}<content xmi:id=\"w\"/>{MARK}<key xmi:id=\"w\"/>"
            )),
            "w",
        ),
        (
            "a key value id repeating the owner's id",
            in_ap(&format!(
                "{MARK}<taggedValue xmi:id=\"t\"><key>{MARK}<value xmi:id=\"t\"/></key></taggedValue>"
            )),
            "t",
        ),
    ] {
        let (input, marks) = marked(&template);
        let error = refused(what, &input);
        assert!(
            matches!(&error, ReadError::DuplicateId { id: found, .. } if found.as_str() == id),
            "{ROW}: {what}: not DuplicateId for {id:?}: {error:?}"
        );
        assert_eq!(
            error.locations(),
            marks.as_slice(),
            "{ROW}: {what}: not both start tags"
        );
    }
    // The edit duplicate check sees a key id.
    let mut doc = read_ok(ROW, "a key id", &tagged("<key xmi:id=\"kw\"/>"));
    let answer = edit::apply(
        &mut doc,
        CreateTaggedValue::new("p", LangString::new("k"), LangString::new("v")).with_id("kw"),
    );
    assert!(
        matches!(&answer, Err(EditError::DuplicateId { id, .. }) if id.as_str() == "kw"),
        "{ROW}: a caller id equal to a key id is not DuplicateId: {answer:?}"
    );
}

// --- Item 6: comparison ----------------------------------------------------------------

#[test]
fn sacm23_base_005_semantic_compare_reports_key_key_id_and_key_value_ids_separately_from_content() {
    let doc = |what: &str, body: &str| read_ok(ROW, what, &tagged(body));
    let plain = doc("plain", &format!("{KEY}{CONTENT}"));
    let other_key = doc(
        "another key",
        &format!("<key><value content=\"K\"/></key>{CONTENT}"),
    );
    let key_id = doc(
        "a key id",
        &format!("<key xmi:id=\"kw\"><value content=\"k\"/></key>{CONTENT}"),
    );
    let key_value_id = doc(
        "a key value id",
        &format!("<key><value xmi:id=\"kv\" content=\"k\"/></key>{CONTENT}"),
    );
    let t = || ElementId::from("t");
    let cases = [
        (
            "another key",
            &plain,
            &other_key,
            Difference::Key {
                id: t(),
                left: Some(mls(&[LangString::new("k")])),
                right: Some(mls(&[LangString::new("K")])),
            },
        ),
        (
            "a key id",
            &key_id,
            &plain,
            Difference::KeyId {
                id: t(),
                left: Some(ElementId::from("kw")),
                right: None,
            },
        ),
        (
            "a key value id",
            &plain,
            &key_value_id,
            Difference::KeyValueIds {
                id: t(),
                left: vec![None],
                right: vec![Some(ElementId::from("kv"))],
            },
        ),
    ];
    for (what, left, right, expected) in cases {
        assert_eq!(
            semantic_compare(left, right),
            std::slice::from_ref(&expected),
            "{ROW}: {what}: not the one difference"
        );
        assert_eq!(
            expected.element_id(),
            Some(&t()),
            "{ROW}: {what}: element_id() is not the TaggedValue"
        );
        let shown = expected.to_string();
        assert!(
            !shown.contains('\n') && shown.contains("\"t\"") && shown.contains("key"),
            "{ROW}: {what}: Display {shown:?} is not one line naming \"t\" and the key"
        );
    }
    // Every MultiLangString feature of one TaggedValue, in the ADR 0025 order.
    let all = doc(
        "every id and value",
        "<key xmi:id=\"kw\"><value xmi:id=\"kv\" content=\"K\"/></key>\
         <content xmi:id=\"cw\"><value xmi:id=\"cv\" content=\"V\"/></content>",
    );
    let differences = semantic_compare(&plain, &all);
    assert!(
        matches!(
            differences.as_slice(),
            [
                Difference::Content { .. },
                Difference::ContentId { .. },
                Difference::ContentValueIds { .. },
                Difference::Key { .. },
                Difference::KeyId { .. },
                Difference::KeyValueIds { .. }
            ]
        ),
        "{ROW}: not Content, ContentId, ContentValueIds, Key, KeyId, KeyValueIds: {differences:?}"
    );
}

#[test]
fn sacm23_base_005_key_and_key_value_ids_differences_display_every_value_in_order() {
    let t = || ElementId::from("t");
    let one = mls(&[ls(Some("en"), Some("a"))]);
    let three = mls(&[
        ls(Some("en"), Some("a")),
        ls(None, Some("b")),
        ls(Some(""), None),
    ]);
    let cases = [
        (
            "no key and an empty one",
            Difference::Key {
                id: t(),
                left: None,
                right: Some(MultiLangString::default()),
            },
            "element \"t\" differs in key: no key on the left, [] on the right",
        ),
        (
            "one value and three",
            Difference::Key {
                id: t(),
                left: Some(one),
                right: Some(three.clone()),
            },
            "element \"t\" differs in key: [[lang \"en\", content \"a\"]] on the left, \
             [[lang \"en\", content \"a\"], [no lang, content \"b\"], [lang \"\", no content]] \
             on the right",
        ),
        (
            "three values and none",
            Difference::Key {
                id: t(),
                left: Some(three),
                right: None,
            },
            "element \"t\" differs in key: \
             [[lang \"en\", content \"a\"], [no lang, content \"b\"], [lang \"\", no content]] \
             on the left, no key on the right",
        ),
        (
            "three value ids and one",
            Difference::KeyValueIds {
                id: t(),
                left: some_ids(&[None, Some("v1"), Some("v2")]),
                right: some_ids(&[Some("v0")]),
            },
            "element \"t\" differs in the xmi:ids of its key's values: \
             [no id, \"v1\", \"v2\"] on the left, [\"v0\"] on the right",
        ),
        (
            "no value ids and two",
            Difference::KeyValueIds {
                id: t(),
                left: Vec::new(),
                right: some_ids(&[Some("v1"), None]),
            },
            "element \"t\" differs in the xmi:ids of its key's values: \
             [] on the left, [\"v1\", no id] on the right",
        ),
    ];
    for (what, difference, expected) in cases {
        assert_eq!(
            difference.to_string(),
            expected,
            "{ROW}: {what}: the Display of {difference:?}"
        );
    }
}

// --- Item 7: CreateTaggedValue -----------------------------------------------------------

#[test]
fn sacm23_base_005_create_tagged_value_generates_ids_reports_its_fields_and_round_trips() {
    let key = mls(&[
        LangString::new("ASIL"),
        LangString::new("Integrity").with_lang("en"),
    ]);
    let content = MultiLangString::from(LangString::new("D"));
    let operation = CreateTaggedValue::new("c1", key.clone(), content.clone());
    assert_eq!(
        (
            operation.parent(),
            operation.id(),
            operation.key(),
            operation.content()
        ),
        (&ElementId::from("c1"), None, &key, &content),
        "{ROW}: CreateTaggedValue's fields"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateTaggedValue",
        "{ROW}: operation_name()"
    );
    let mut doc = built();
    for (parent, expected) in [("c1", "tagged_value_1"), ("arguments", "tagged_value_2")] {
        let mutation = match edit::apply(
            &mut doc,
            CreateTaggedValue::new(parent, key.clone(), content.clone()),
        ) {
            Ok(mutation) => mutation,
            Err(error) => panic!("{ROW}: CreateTaggedValue under {parent}: {error:?}"),
        };
        assert_eq!(
            (
                mutation.created_ids(),
                mutation.changed_ids(),
                mutation.deleted_ids()
            ),
            (
                [ElementId::from(expected)].as_slice(),
                [].as_slice(),
                [].as_slice()
            ),
            "{ROW}: CreateTaggedValue under {parent}: the mutation's ids"
        );
        let t = element(ROW, &doc, expected);
        assert_eq!(
            (
                t.kind(),
                t.parent().map(|p| p.id().as_str().to_owned()),
                t.key().cloned(),
                t.content().cloned(),
                t.key_id(),
                t.key_value_ids().to_vec(),
                t.content_value_ids().to_vec()
            ),
            (
                ElementKind::TaggedValue,
                Some(parent.to_owned()),
                Some(key.clone()),
                Some(content.clone()),
                None,
                vec![None, None],
                vec![None]
            ),
            "{ROW}: the created TaggedValue under {parent}"
        );
    }
    let with_id = edit::apply(
        &mut doc,
        CreateTaggedValue::new("c1", key.clone(), content.clone()).with_id("mine"),
    );
    assert!(
        with_id.is_ok_and(|m| m.created_ids() == [ElementId::from("mine")]),
        "{ROW}: a caller id is not used"
    );
    let found = validation::validate(&doc);
    assert!(
        !diagnostic::has_errors(&found),
        "{ROW}: a built document has validation Errors: {found:?}"
    );
    assert_round_trip("built TaggedValues", &doc, Mode::Strict);
}

#[test]
fn sacm23_base_005_create_tagged_value_returns_the_caller_id_key_and_content_it_was_given() {
    let key = mls(&[ls(Some("en"), Some("ASIL")), ls(None, Some("Integrity"))]);
    let content = mls(&[ls(Some("en"), Some("D"))]);
    let c1 = ElementId::from("c1");
    let operation = CreateTaggedValue::new("c1", key.clone(), content.clone());
    assert_eq!(
        (
            operation.parent(),
            operation.id(),
            operation.key(),
            operation.content()
        ),
        (&c1, None, &key, &content),
        "{ROW}: CreateTaggedValue without with_id"
    );
    let operation = operation.with_id("x");
    assert_eq!(
        (
            operation.parent(),
            operation.id(),
            operation.key(),
            operation.content()
        ),
        (&c1, Some(&ElementId::from("x")), &key, &content),
        "{ROW}: CreateTaggedValue with with_id(\"x\")"
    );
}

#[test]
fn sacm23_base_005_create_tagged_value_refuses_like_every_create_operation() {
    let mut doc = built();
    if let Err(error) = edit::apply(
        &mut doc,
        CreateNote::new("c1", LangString::new("n")).with_id("n"),
    ) {
        panic!("{ROW}: setup: {error:?}");
    }
    if let Err(error) = edit::apply(
        &mut doc,
        CreateTaggedValue::new("c1", LangString::new("k"), LangString::new("v")).with_id("t"),
    ) {
        panic!("{ROW}: setup: {error:?}");
    }
    let before = doc.clone();
    let create = |parent: &str, id: &str| {
        CreateTaggedValue::new(parent, LangString::new("k"), LangString::new("v")).with_id(id)
    };
    let answers = [
        (
            "an unknown parent",
            edit::apply(&mut doc, create("nowhere", "1bad")),
        ),
        ("a Note parent", edit::apply(&mut doc, create("n", "1bad"))),
        (
            "a TaggedValue parent",
            edit::apply(&mut doc, create("t", "x")),
        ),
        ("a bad id", edit::apply(&mut doc, create("c1", "1bad"))),
        ("an id in use", edit::apply(&mut doc, create("c1", "n"))),
    ];
    let expected = [
        "ParentNotFound",
        "InvalidParent Note",
        "InvalidParent TaggedValue",
        "InvalidId",
        "DuplicateId",
    ];
    for ((what, answer), expected) in answers.iter().zip(expected) {
        let found = match answer {
            Err(EditError::ParentNotFound { parent, .. }) if parent.as_str() == "nowhere" => {
                "ParentNotFound".to_owned()
            }
            Err(EditError::InvalidParent {
                parent_kind,
                kind: ElementKind::TaggedValue,
                ..
            }) => format!("InvalidParent {parent_kind}"),
            Err(EditError::InvalidId { id, .. }) if id.as_str() == "1bad" => "InvalidId".to_owned(),
            Err(EditError::DuplicateId { id, .. }) if id.as_str() == "n" => {
                "DuplicateId".to_owned()
            }
            other => format!("{other:?}"),
        };
        assert_eq!(found, expected, "{ROW}: {what}: the refusal");
    }
    assert_eq!(doc, before, "{ROW}: a refused edit changed the document");
    // Every ModelElement kind is a parent, and no UtilityElement kind is. A
    // parent that is not a package is placed in an ArtifactPackage root,
    // which admits every ModelElement (ADR 0014).
    for &kind in ElementKind::ALL {
        let mut doc = Document::new();
        let p = ElementId::from("p");
        let setup = if kind.is_package() {
            doc.insert(None, kind, p, None)
        } else {
            let root = ElementId::from("root");
            doc.insert(None, ElementKind::ArtifactPackage, root.clone(), None)
                .and_then(|()| doc.insert(Some(&root), kind, p, None))
        };
        if let Err(error) = setup {
            panic!("{ROW}: setup could not insert a {kind}: {error}");
        }
        let answer = edit::apply(&mut doc, create("p", "u"));
        if kind.is_a(AbstractClass::ModelElement) {
            assert!(
                answer.is_ok() && ids(element(ROW, &doc, "p").tagged_values()) == ["u"],
                "{ROW}: a TaggedValue under a {kind} is not created in its role: {answer:?}"
            );
        } else {
            assert!(
                matches!(
                    &answer,
                    Err(EditError::InvalidParent { parent_kind, kind: ElementKind::TaggedValue, .. })
                        if *parent_kind == kind
                ),
                "{ROW}: a TaggedValue under a {kind} is not InvalidParent: {answer:?}"
            );
        }
    }
}

#[test]
fn sacm23_base_005_a_character_xml_cannot_represent_in_a_key_value_fails_the_write() {
    for (field, value, attribute) in [
        (
            "lang",
            ls(Some("e\u{1}n"), Some("k")),
            WriteAttribute::KeyValueLang,
        ),
        (
            "content",
            ls(None, Some("k\u{FFFF}")),
            WriteAttribute::KeyValueContent,
        ),
    ] {
        let mut doc = built();
        if let Err(error) = edit::apply(
            &mut doc,
            CreateTaggedValue::new("c1", value, LangString::new("v")).with_id("t"),
        ) {
            panic!("{ROW}: setup: {error:?}");
        }
        for mode in MODES {
            let error = match xmi::write(&doc, &WriteOptions::default().with_mode(mode)) {
                Ok(text) => panic!("{ROW}: a key {field} the XML cannot hold was written:\n{text}"),
                Err(error) => error,
            };
            assert!(
                matches!(
                    &error,
                    WriteError::UnrepresentableCharacter { element, attribute: found, .. }
                        if element.as_str() == "t" && *found == attribute
                ),
                "{ROW}: a key's {field} ({mode:?}): not UnrepresentableCharacter on t, \
                 {attribute:?}: {error:?}"
            );
        }
        let shown = attribute.to_string();
        assert!(
            shown.contains("key") && shown.contains(field),
            "{ROW}: WriteAttribute {attribute:?} displays as {shown:?}"
        );
    }
}

// --- SACM23-VAL-020: key ids that are not NCNames ---------------------------------------

#[test]
fn sacm23_val_020_a_key_or_key_value_id_that_is_not_an_ncname_is_one_id_002_on_the_tagged_value() {
    for (what, key, bad) in [
        (
            "a key id",
            "<key xmi:id=\".k\"><value content=\"k\"/></key>",
            ".k",
        ),
        (
            "a key value id",
            "<key><value xmi:id=\"\u{b7}v\" content=\"k\"/></key>",
            "\u{b7}v",
        ),
    ] {
        let doc = read_ok(VAL_020, what, &tagged(&format!("{key}{CONTENT}")));
        let found = validation::validate(&doc);
        let [finding] = found.as_slice() else {
            panic!("{VAL_020}: {what}: not one finding: {found:?}");
        };
        assert_eq!(
            (
                finding.code(),
                finding.severity(),
                finding.requirement_id(),
                finding.clause(),
                finding.affected(),
                finding.location()
            ),
            (
                Code::ID_INVALID,
                Severity::Error,
                VAL_020,
                Some("7.6.1"),
                [ElementId::from("t")].as_slice(),
                None
            ),
            "{VAL_020}: {what}: not one SACM-ID-002 Error on the TaggedValue"
        );
        let quoted = format!("{bad:?}");
        assert!(
            finding.message().contains(&quoted),
            "{VAL_020}: {what}: the message {:?} does not quote {quoted}",
            finding.message()
        );
        assert!(
            matches!(
                xmi::write(&doc, &WriteOptions::default()),
                Err(WriteError::Invalid { .. })
            ),
            "{VAL_020}: {what}: a strict write does not refuse it"
        );
        let text = write_in(VAL_020, what, Mode::Tolerant, &doc);
        assert_eq!(
            read_ok(VAL_020, what, &text),
            doc,
            "{VAL_020}: {what}: a tolerant write does not keep the id"
        );
    }
}

#[test]
fn sacm23_val_020_the_message_names_a_bad_key_value_id_by_its_1_based_position() {
    let tail = "which is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)";
    for (what, key, expected) in [
        (
            "the key's own id",
            "<key xmi:id=\".k\"><value content=\"k\"/></key>",
            format!("the key of TaggedValue \"t\" has the xmi:id \".k\", {tail}"),
        ),
        (
            "the first key value's id",
            "<key><value xmi:id=\"a b\" content=\"k\"/><value lang=\"sv\" content=\"n\"/></key>",
            format!("value 1 of the key of TaggedValue \"t\" has the xmi:id \"a b\", {tail}"),
        ),
        (
            "the second key value's id",
            "<key><value xmi:id=\"k1\" content=\"k\"/><value xmi:id=\"a:b\" lang=\"sv\" content=\"n\"/></key>",
            format!("value 2 of the key of TaggedValue \"t\" has the xmi:id \"a:b\", {tail}"),
        ),
    ] {
        let doc = read_ok(VAL_020, what, &tagged(&format!("{key}{CONTENT}")));
        let found = validation::validate(&doc);
        let [finding] = found.as_slice() else {
            panic!("{VAL_020}: {what}: not one finding: {found:?}");
        };
        assert_eq!(
            (finding.code(), finding.message()),
            (Code::ID_INVALID, expected.as_str()),
            "{VAL_020}: {what}: not the SACM-ID-002 message naming the value's position"
        );
    }
}

// --- SACM23-XMI-004: key ids are written escaped ----------------------------------------

#[test]
fn sacm23_xmi_004_a_key_or_key_value_id_is_written_escaped_and_reads_back_exactly() {
    let written = "k&amp;&lt;&gt;&quot;&#9;&#10;&#13;";
    let held = "k&<>\"\t\n\r";
    let doc = match read(
        Mode::Tolerant,
        &tagged(&format!(
            "<key xmi:id=\"{written}1\"><value xmi:id=\"{written}2\" content=\"k\"/></key>"
        )),
    ) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{XMI_004}: key ids: refused: {error:?}"),
    };
    let t = element(XMI_004, &doc, "t");
    assert_eq!(
        (
            t.key_id().map(|id| id.as_str().to_owned()),
            t.key_value_ids().to_vec()
        ),
        (
            Some(format!("{held}1")),
            vec![Some(ElementId::from(format!("{held}2")))]
        ),
        "{XMI_004}: setup: the key ids are not read unescaped"
    );
    let text = write_in(XMI_004, "key ids", Mode::Tolerant, &doc);
    let key_line = format!("<key xmi:id=\"{written}1\">");
    let value_line = format!("<value xmi:id=\"{written}2\" content=\"k\" />");
    assert!(
        has_lines(&text, &[key_line.as_str(), value_line.as_str(), "</key>"]),
        "{XMI_004}: the key ids are not written escaped:\n{text}"
    );
    let back = match read(Mode::Tolerant, &text) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{XMI_004}: the output does not read back: {error:?}"),
    };
    assert_eq!(back, doc, "{XMI_004}: the key ids do not read back exactly");
    // An edit-built key is written with no id.
    let mut built = built();
    if let Err(error) = edit::apply(
        &mut built,
        CreateTaggedValue::new("c1", LangString::new("k"), LangString::new("v")),
    ) {
        panic!("{XMI_004}: setup: {error:?}");
    }
    let text = write_in(XMI_004, "a built key", Mode::Strict, &built);
    assert!(
        has_lines(&text, &["<key>", "<value content=\"k\" />", "</key>"]),
        "{XMI_004}: an edit-built key is not written without ids:\n{text}"
    );
}
