//! Tests for the cross-document links the reader holds (SACM23-XMI-029; XMI
//! 2.5.1 formal/15-06-07 §7.6.2 "The link attributes act as a union of two
//! linking mechanisms, any one of which may be used at one time."; §7.10.1;
//! §7.10.2 "The URI reference must be of the form `URI#id_value`"; production
//! 2c, rules 2l, 2m and 2n "A URI Reference, optionally preceded by the type
//! of the object being referenced."; rule 4e; SACM 2.3 formal/23-05-08 §8.4;
//! ADR 0040 amendment 1).
//!
//! - An `href` whose fragment holds `(` or `)` is an `XPointer`, held as an
//!   `Href` on any reference end, an `expression` or a proxy; its document
//!   part may be empty.
//! - A type before the URI reference (rule 2n) is held as `sacm:<Class> <URI>`,
//!   and `Href::kind()` gives the class; with `#id` the value is the id.
//! - A role element of a composite in reference form whose `href` names an
//!   element of another document, or an `XPointer`, is a cross-file containment
//!   proxy: a `Proxy` of its container, written after the container's
//!   children, never resolved, counted by no rule.
//! - An `href` as an `ExpressionLangString`'s `expression` is held as
//!   `LangString::expression_href()`.
//! - Identity, type, `XLink` and any other attributes on a reference element,
//!   and `href` beside `xmi:idref` on a reference element or a proxy, keep
//!   the element whole with SACM-XMI-003 (`element-kept`), in both modes.
//! - A diagnostic about a same-file proxy that is kept affects its container,
//!   never the id the proxy names (ADR 0040 amendment 1, A9).
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{ArgumentValue, Code, Diagnostic, Severity};
use sacm::model::{Document, ReferenceEnd, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

use sacm::compare::Difference;
use sacm::edit::{
    self, CopyElements, DeleteElement, EditError, MoveElement, Mutation, Operation,
    PackageDeletePolicy,
};
use sacm::model::{ElementId, ElementKind, Href, Proxy};
use sacm::validation;

const ROW: &str = "SACM23-XMI-029";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a diagnostic is expected. Removed before
/// reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// Declares `xsi`.
const XSI: &str = " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";

/// Declares `xlink` (the namespace of the row's acceptance).
const XLINK: &str = " xmlns:xlink=\"http://www.w3.org/1999/xlink\"";

/// Golden L1 of the design brief: its input and its output in both modes.
const L1: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/cross-document-links-valid.sacm.xmi"
);
const L1_GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/cross-document-links.golden.sacm.xmi"
);

// --- Inputs ----------------------------------------------------------------------

fn file(path: &str) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{ROW}: setup: cannot read {path}: {error}"))
}

/// An element of `class` in the role `role`, `id`, with `attributes`, named
/// after its id, holding `body` after its name.
fn el(role: &str, class: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<{role} xmi:type=\"sacm:{class}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</{role}>"
    )
}

/// A root of `class`, `id`, declaring `sacm`, `xmi` and `declarations`, named
/// after its id, holding `body`.
fn root(class: &str, id: &str, declarations: &str, body: &str) -> String {
    format!(
        "<sacm:{class}{NS}{declarations} xmi:id=\"{id}\"><name content=\"{id}\"/>{body}</sacm:{class}>"
    )
}

/// A Claim `id` with `attributes`, holding `body`.
fn claim(id: &str, attributes: &str, body: &str) -> String {
    el("argumentElement", "Claim", id, attributes, body)
}

/// The ArgumentPackage root `p`, declaring `declarations`, holding `body`.
fn in_p(declarations: &str, body: &str) -> String {
    root("ArgumentPackage", "p", declarations, body)
}

/// The AssuranceCasePackage root `case`, declaring `declarations`, holding
/// `body`.
fn in_case(declarations: &str, body: &str) -> String {
    root("AssuranceCasePackage", "case", declarations, body)
}

/// The ArgumentPackage `p` holding the citing Claim `c1`, which holds `body`,
/// and the Claim `c2`.
fn citing(declarations: &str, body: &str) -> String {
    in_p(
        declarations,
        &format!(
            "{}{}",
            claim("c1", " isCitation=\"true\"", body),
            claim("c2", "", "")
        ),
    )
}

/// The TerminologyPackage `terms` holding the Term `t1`, whose name element
/// is `name`, and the Term `t2`.
fn naming(declarations: &str, name: &str) -> String {
    format!(
        "<sacm:TerminologyPackage{NS}{declarations} xmi:id=\"terms\"><name content=\"terms\"/>\
         <terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"t1\" value=\"blade\">{name}</terminologyElement>\
         <terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"t2\" value=\"hub\"><name content=\"t2\"/>\
         </terminologyElement></sacm:TerminologyPackage>"
    )
}

/// The ArgumentPackage `p` holding the Claim `c2` and the `ArgumentGroup` `g`,
/// which holds `body`.
fn grouping(declarations: &str, body: &str) -> String {
    in_p(
        declarations,
        &format!(
            "{}{}",
            claim("c2", "", ""),
            el("argumentElement", "ArgumentGroup", "g", "", body)
        ),
    )
}

/// `template` without its marks, and the line and column of each mark in
/// order: lines end at LF, and columns count characters.
fn marked(template: &str) -> (String, Vec<(u32, u32)>) {
    let (mut line, mut column) = (1_u32, 1_u32);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push((line, column));
            continue;
        }
        text.push(c);
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (text, marks)
}

// --- Reading and writing ---------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in `mode`, which must return a document.
fn read_ok(what: &str, mode: Mode, input: &str) -> ReadOutput {
    match read(mode, input) {
        Ok(output) => output,
        Err(error) => {
            panic!("{ROW}: {what}: a {mode:?} read refused it: {error} ({error:?})\n{input}")
        }
    }
}

/// Reads `input` in both modes, which must each return a document with no
/// diagnostic and no preserved content, the two `==`; returns the strict
/// one.
fn read_clean(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        let output = read_ok(what, mode, input);
        assert_eq!(
            output.diagnostics(),
            [],
            "{ROW}: {what}: a {mode:?} read gives diagnostics"
        );
        let doc = output.into_document();
        assert!(
            !doc.has_preserved_content() && !doc.elements().any(SacmElement::has_preserved_content),
            "{ROW}: {what}: a {mode:?} read keeps preserved content"
        );
        documents.push(doc);
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: {what}: the modes read different documents: {:?}",
        semantic_compare(&documents[0], &documents[1])
    );
    documents.swap_remove(0)
}

/// `doc` written in `mode`, which must succeed.
fn written(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused it: {error} ({error:?})"),
    }
}

/// Writes `doc` in `mode`, reads the output back in `mode` and writes that
/// again: the document read back must be `==`, and the second write must
/// give the same bytes. Returns the output.
fn round_trip(what: &str, doc: &Document, mode: Mode) -> String {
    let text = written(what, doc, mode);
    let back = read_ok(&format!("{what}: the {mode:?} output"), mode, &text).into_document();
    assert!(
        back == *doc,
        "{ROW}: {what}: the {mode:?} output read back is not ==: {:?}\n{text}",
        semantic_compare(doc, &back)
    );
    assert_eq!(
        written(what, &back, mode),
        text,
        "{ROW}: {what}: the second {mode:?} write is not byte-stable"
    );
    text
}

/// [`round_trip`] in both modes, which must give the same bytes.
fn round_trip_both(what: &str, doc: &Document) -> String {
    let strict = round_trip(what, doc, Mode::Strict);
    assert_eq!(
        round_trip(what, doc, Mode::Tolerant),
        strict,
        "{ROW}: {what}: the two write modes give different bytes"
    );
    strict
}

/// The trimmed lines of `text`.
fn lines(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).collect()
}

fn element<'d>(what: &str, doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: {what}: the document has no element {id:?}");
    };
    found
}

/// The ids of the roots of `doc`, in order.
fn root_ids(doc: &Document) -> Vec<String> {
    doc.roots()
        .map(|found| found.id().as_str().to_owned())
        .collect()
}

/// The severity of SACM-XMI-003 in `mode`.
fn kept_severity(mode: Mode) -> Severity {
    if mode == Mode::Strict {
        Severity::Error
    } else {
        Severity::Warning
    }
}

/// A diagnostic as its code, severity, template, location, affected ids and
/// arguments, each argument as its text or count.
type Found = (
    &'static str,
    Severity,
    &'static str,
    Option<(u32, u32)>,
    Vec<String>,
    Vec<(&'static str, String)>,
);

fn found(diagnostic: &Diagnostic) -> Found {
    (
        diagnostic.code().as_str(),
        diagnostic.severity(),
        diagnostic.template(),
        diagnostic.location().map(|at| (at.line(), at.column())),
        diagnostic
            .affected()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
        diagnostic
            .arguments()
            .iter()
            .map(|argument| {
                let value = match argument.value() {
                    ArgumentValue::Text(text) => text.clone(),
                    ArgumentValue::Count(count) => count.to_string(),
                    other => format!("{other:?}"),
                };
                (argument.name(), value)
            })
            .collect(),
    )
}

fn diagnostics(output: &ReadOutput) -> Vec<Found> {
    output.diagnostics().iter().map(found).collect()
}

/// SACM-XMI-003 `element-kept` at `at`: the element `kept_element`, kept by
/// the `holder_class` `holder`.
fn element_kept(
    mode: Mode,
    at: (u32, u32),
    kept_element: &str,
    holder_class: &str,
    holder: &str,
) -> Found {
    (
        "SACM-XMI-003",
        kept_severity(mode),
        "SACM-XMI-003/element-kept",
        Some(at),
        vec![holder.to_owned()],
        vec![
            ("element", kept_element.to_owned()),
            ("holder_class", holder_class.to_owned()),
            ("holder_id", holder.to_owned()),
        ],
    )
}

/// Reads `template`, which marks the `<` of one element, in both modes: each
/// gives exactly one SACM-XMI-003 `element-kept` there (an Error strict, a
/// Warning tolerant), naming `kept_element` and affecting `holder`, which
/// keeps it as preserved content; the two documents are `==`; a strict write
/// refuses the document (SACM-XMI-006), and a tolerant write round-trips it.
/// Returns the document.
fn assert_element_kept(
    what: &str,
    template: &str,
    kept_element: &str,
    holder_class: &str,
    holder: &str,
) -> Document {
    let (input, marks) = marked(template);
    assert_eq!(marks.len(), 1, "{ROW}: setup: {what}: not one mark");
    let mut documents = Vec::new();
    for mode in MODES {
        let output = read_ok(what, mode, &input);
        assert_eq!(
            diagnostics(&output),
            [element_kept(
                mode,
                marks[0],
                kept_element,
                holder_class,
                holder
            )],
            "{ROW}: {what}: a {mode:?} read does not give one element-kept at the element's `<`\n{input}"
        );
        assert!(
            output
                .diagnostics()
                .iter()
                .all(|one| one.code() != Code::XMI_EXTERNAL_HREF),
            "{ROW}: {what}: SACM-XMI-007 in {mode:?} mode"
        );
        documents.push(output.into_document());
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: {what}: the modes read different documents: {:?}",
        semantic_compare(&documents[0], &documents[1])
    );
    let doc = documents.swap_remove(0);
    assert!(
        element(what, &doc, holder).has_preserved_content(),
        "{ROW}: {what}: {holder} does not keep the element"
    );
    match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Strict)) {
        Ok(text) => panic!("{ROW}: {what}: a strict write accepted it:\n{text}"),
        Err(error) => assert!(
            matches!(error, WriteError::PreservedContent { .. })
                && error.code() == Code::XMI_STRICT_SAVE_REFUSED,
            "{ROW}: {what}: a strict write is not PreservedContent (SACM-XMI-006): {error:?}"
        ),
    }
    round_trip(what, &doc, Mode::Tolerant);
    doc
}

/// The `href`s of the end `end` of `id`, as strings.
fn hrefs(what: &str, doc: &Document, id: &str, end: ReferenceEnd) -> Vec<String> {
    element(what, doc, id)
        .hrefs(end)
        .iter()
        .map(|href| href.as_str().to_owned())
        .collect()
}

/// The ids of the end `end` of `id`.
fn ids(what: &str, doc: &Document, id: &str, end: ReferenceEnd) -> Vec<String> {
    element(what, doc, id)
        .references(end)
        .iter()
        .map(|one| one.as_str().to_owned())
        .collect()
}

/// The one `href` of the end `end` of `id`.
fn one_href<'d>(what: &str, doc: &'d Document, id: &str, end: ReferenceEnd) -> &'d Href {
    let found = element(what, doc, id).hrefs(end);
    let [href] = found else {
        panic!("{ROW}: {what}: the {end:?} of {id} holds not one href: {found:?}");
    };
    href
}

/// The proxies of `id`, each as its role and its href.
fn proxies(what: &str, doc: &Document, id: &str) -> Vec<(&'static str, String)> {
    element(what, doc, id)
        .proxies()
        .iter()
        .map(|proxy| (proxy.role(), proxy.href().as_str().to_owned()))
        .collect()
}

/// `(role, href)` pairs as [`proxies`] gives them.
fn pairs(expected: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    expected
        .iter()
        .map(|(role, href)| (*role, (*href).to_owned()))
        .collect()
}

/// The `kind()`, `uri()`, `document()`, `fragment()` and `is_xpointer()` of
/// `href`.
fn parts(href: &Href) -> (Option<ElementKind>, &str, &str, &str, bool) {
    (
        href.kind(),
        href.uri(),
        href.document(),
        href.fragment(),
        href.is_xpointer(),
    )
}

/// The (code, severity, template, row, affected) of each finding of
/// `validate`.
fn findings(doc: &Document) -> Vec<(String, Severity, String, String, Vec<String>)> {
    validation::validate(doc)
        .iter()
        .map(|one| {
            (
                one.code().as_str().to_owned(),
                one.severity(),
                one.template().to_owned(),
                one.requirement_id().to_owned(),
                one.affected()
                    .iter()
                    .map(|id| id.as_str().to_owned())
                    .collect(),
            )
        })
        .collect()
}

// --- Edits -----------------------------------------------------------------------

/// Applies `operation`, which the test expects to succeed, after checking
/// that `edit::check` accepts it.
fn applied(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    let checked = edit::check(doc, &operation);
    let mutation = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {operation:?} was refused: {error} ({error:?})"),
    };
    assert_eq!(
        checked,
        Ok(()),
        "{ROW}: check refuses {operation:?}, which apply accepts"
    );
    mutation
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before, with `check` refusing it the same, and returns why.
fn refuse(doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let before = doc.clone();
    let checked = edit::check(doc, &operation);
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{ROW}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{ROW}: the refused {operation:?} changed the document"
    );
    assert_eq!(
        checked,
        Err(error.clone()),
        "{ROW}: check answers {operation:?} otherwise than apply"
    );
    error
}

fn strs(ids: &[ElementId]) -> Vec<&str> {
    ids.iter().map(ElementId::as_str).collect()
}

// =================================================================================
// 1. XPointer
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_029_an_xpointer_href_is_held_as_an_href() {
    const XPOINTER: &str = "o.xmi#xpointer(id('c'))";
    const FRAGMENT: &str = "xpointer(id('c'))";
    // The binding `b` of the case: one XPointer and one plain href.
    let binding = in_case(
        "",
        &el(
            "argumentPackage",
            "ArgumentPackageBinding",
            "b",
            "",
            &format!(
                "<participantPackage href=\"{XPOINTER}\"/><participantPackage href=\"o.xmi#q\"/>"
            ),
        ),
    );
    for (what, input, holder, end, role, others) in [
        (
            "an XPointer on citedElement",
            citing("", &format!("<citedElement href=\"{XPOINTER}\"/>")),
            "c1",
            ReferenceEnd::CitedElement,
            "citedElement",
            vec![],
        ),
        (
            "an XPointer on metaClaim",
            in_p(
                "",
                &claim("c1", "", &format!("<metaClaim href=\"{XPOINTER}\"/>")),
            ),
            "c1",
            ReferenceEnd::MetaClaim,
            "metaClaim",
            vec![],
        ),
        (
            "an XPointer on participantPackage",
            binding,
            "b",
            ReferenceEnd::ParticipantPackage,
            "participantPackage",
            vec!["o.xmi#q"],
        ),
    ] {
        let doc = read_clean(what, &input);
        assert_eq!(
            ids(what, &doc, holder, end),
            Vec::<String>::new(),
            "{ROW}: {what}: the XPointer is held as an id"
        );
        let mut expected = vec![XPOINTER];
        expected.extend(others);
        assert_eq!(
            hrefs(what, &doc, holder, end),
            expected,
            "{ROW}: {what}: hrefs()"
        );
        let href = &element(what, &doc, holder).hrefs(end)[0];
        assert_eq!(
            (href.as_str(), parts(href)),
            (XPOINTER, (None, XPOINTER, "o.xmi", FRAGMENT, true)),
            "{ROW}: {what}: as_str, kind, uri, document, fragment and is_xpointer"
        );
        // Written back exactly, in element form, by both saves.
        let text = round_trip_both(what, &doc);
        let line = format!("<{role} href=\"{XPOINTER}\" />");
        assert_eq!(
            lines(&text)
                .iter()
                .filter(|found| **found == line.as_str())
                .count(),
            1,
            "{ROW}: {what}: not written back as one line {line}\n{text}"
        );
        assert!(
            !text.contains(&format!(" {role}=\"")),
            "{ROW}: {what}: the end is written as an attribute\n{text}"
        );
    }
    // The document part may be empty, and the value is held trimmed.
    for (what, value, held, document, fragment) in [
        (
            "a same-document XPointer",
            "#xpointer((//*[@xmi:uuid='u'])[1])",
            "#xpointer((//*[@xmi:uuid='u'])[1])",
            "",
            "xpointer((//*[@xmi:uuid='u'])[1])",
        ),
        (
            "a padded same-document XPointer",
            " &#9;#xpointer(id('c'))&#10; ",
            "#xpointer(id('c'))",
            "",
            "xpointer(id('c'))",
        ),
        // Either parenthesis alone makes an XPointer.
        (
            "a fragment with ( only",
            "o.xmi#a(b",
            "o.xmi#a(b",
            "o.xmi",
            "a(b",
        ),
        (
            "a fragment with ) only",
            "o.xmi#a)b",
            "o.xmi#a)b",
            "o.xmi",
            "a)b",
        ),
        ("a same-document fragment with ) only", "#)", "#)", "", ")"),
    ] {
        let doc = read_clean(
            what,
            &citing("", &format!("<citedElement href=\"{value}\"/>")),
        );
        assert_eq!(
            element(what, &doc, "c1").cited_element(),
            None,
            "{ROW}: {what}: held as an id"
        );
        let href = one_href(what, &doc, "c1", ReferenceEnd::CitedElement);
        assert_eq!(
            (href.as_str(), parts(href)),
            (held, (None, held, document, fragment, true)),
            "{ROW}: {what}: as_str, kind, uri, document, fragment and is_xpointer"
        );
        let text = round_trip_both(what, &doc);
        assert!(
            lines(&text).contains(&format!("<citedElement href=\"{held}\" />").as_str()),
            "{ROW}: {what}: not written back exactly, in element form\n{text}"
        );
    }
    // Parentheses in the document part make no XPointer: it is the fragment
    // that holds them.
    let what = "parentheses in the document part";
    let doc = read_clean(what, &citing("", "<citedElement href=\"a(1).xmi#c\"/>"));
    let href = one_href(what, &doc, "c1", ReferenceEnd::CitedElement);
    assert_eq!(
        parts(href),
        (None, "a(1).xmi#c", "a(1).xmi", "c", false),
        "{ROW}: {what}: kind, uri, document, fragment and is_xpointer"
    );
    // White space or a second `#` inside an XPointer is still a defect (ADR
    // 0040 D1, amendment 1 A1), as is a fragment that is empty.
    for value in [
        "o.xmi#xpointer(id('a b'))",
        "o.xmi#xpointer(id('a&#9;b'))",
        "o.xmi#xpointer(id('a#b'))",
        "#xpointer(id('a#b'))",
        "#",
    ] {
        let what = format!("href={value:?} on citedElement");
        let doc = assert_element_kept(
            &what,
            &citing("", &format!("{MARK}<citedElement href=\"{value}\"/>")),
            "citedElement",
            "Claim",
            "c1",
        );
        assert!(
            element(&what, &doc, "c1")
                .hrefs(ReferenceEnd::CitedElement)
                .is_empty()
                && element(&what, &doc, "c1").cited_element().is_none(),
            "{ROW}: {what}: the kept element is also read"
        );
    }
}

// =================================================================================
// 2. A type before the URI reference (rule 2n)
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_029_a_type_prefixed_href_is_held_with_its_kind() {
    const HELD: &str = "sacm:Claim o.xmi#c1";
    // `p` is bound to the pinned SACM namespace, `x` to a foreign one, and
    // `q` to none.
    let declarations =
        " xmlns:p=\"http://www.omg.org/spec/SACM/20220301\" xmlns:x=\"urn:example:x\"";
    for (what, value) in [
        ("sacm:Claim, as written", "sacm:Claim o.xmi#c1"),
        // A literal tab and line feed are two spaces after attribute-value
        // normalization (XML 1.0 §3.3.3); character references keep them.
        (
            "p:Claim, a literal tab and line feed",
            "p:Claim\t\no.xmi#c1",
        ),
        (
            "p:Claim, a tab and a line feed by reference",
            "p:Claim&#9;&#10;o.xmi#c1",
        ),
        ("p:Claim, one space", "p:Claim o.xmi#c1"),
        (
            "sacm:Claim, padded, a run of spaces",
            "  sacm:Claim &#13;  o.xmi#c1&#9;",
        ),
    ] {
        let doc = read_clean(
            what,
            &citing(declarations, &format!("<citedElement href=\"{value}\"/>")),
        );
        assert_eq!(
            element(what, &doc, "c1").cited_element(),
            None,
            "{ROW}: {what}: held as an id"
        );
        let href = one_href(what, &doc, "c1", ReferenceEnd::CitedElement);
        assert_eq!(
            (href.as_str(), parts(href)),
            (
                HELD,
                (Some(ElementKind::Claim), "o.xmi#c1", "o.xmi", "c1", false)
            ),
            "{ROW}: {what}: not held as {HELD:?} with its kind and uri"
        );
        let text = round_trip_both(what, &doc);
        assert!(
            lines(&text).contains(&format!("<citedElement href=\"{HELD}\" />").as_str()),
            "{ROW}: {what}: not written as {HELD:?}, in element form\n{text}"
        );
    }
    // A typed XPointer, with and without a document part.
    for (what, end_element, value, expected) in [
        (
            "a typed XPointer",
            "citedElement",
            "sacm:Claim o.xmi#xpointer(id('c'))",
            (
                Some(ElementKind::Claim),
                "o.xmi#xpointer(id('c'))",
                "o.xmi",
                "xpointer(id('c'))",
                true,
            ),
        ),
        (
            "a typed same-document XPointer",
            "metaClaim",
            "sacm:ArgumentPackageInterface #xpointer(x)",
            (
                Some(ElementKind::ArgumentPackageInterface),
                "#xpointer(x)",
                "",
                "xpointer(x)",
                true,
            ),
        ),
    ] {
        let end = if end_element == "citedElement" {
            ReferenceEnd::CitedElement
        } else {
            ReferenceEnd::MetaClaim
        };
        let doc = read_clean(
            what,
            &citing("", &format!("<{end_element} href=\"{value}\"/>")),
        );
        let href = one_href(what, &doc, "c1", end);
        assert_eq!(
            (href.as_str(), parts(href)),
            (value, expected),
            "{ROW}: {what}: as_str, kind, uri, document, fragment and is_xpointer"
        );
        round_trip_both(what, &doc);
    }
    // The type of an href is not checked against its end (amendment 1, A4).
    let what = "sacm:ArtifactPackage on the citedElement of a Claim";
    let doc = read_clean(
        what,
        &citing("", "<citedElement href=\"sacm:ArtifactPackage o.xmi#a\"/>"),
    );
    assert_eq!(
        one_href(what, &doc, "c1", ReferenceEnd::CitedElement).kind(),
        Some(ElementKind::ArtifactPackage),
        "{ROW}: {what}: kind()"
    );
    assert!(
        validation::validate(&doc)
            .iter()
            .all(|one| !one.code().as_str().starts_with("SACM-REF-")),
        "{ROW}: {what}: validate checks the type of an href: {:?}",
        findings(&doc)
    );
    // A type-prefixed `#id` is the id: the type is not held.
    let what = "sacm:Claim #c2";
    let doc = read_clean(what, &citing("", "<citedElement href=\"sacm:Claim #c2\"/>"));
    assert_eq!(
        (
            element(what, &doc, "c1")
                .cited_element()
                .map(ElementId::as_str),
            hrefs(what, &doc, "c1", ReferenceEnd::CitedElement)
        ),
        (Some("c2"), Vec::new()),
        "{ROW}: {what}: not read as the id c2"
    );
    assert!(
        doc == read_clean(
            "citedElement xmi:idref",
            &citing("", "<citedElement xmi:idref=\"c2\"/>")
        ),
        "{ROW}: {what}: not the document xmi:idref=\"c2\" gives"
    );
    round_trip_both(what, &doc);
    let what = "p:Claim, a run of white space, #c2";
    let doc = read_clean(
        what,
        &citing(declarations, "<citedElement href=\" p:Claim &#10; #c2 \"/>"),
    );
    assert_eq!(
        element(what, &doc, "c1")
            .cited_element()
            .map(ElementId::as_str),
        Some("c2"),
        "{ROW}: {what}: not read as the id c2"
    );
    // What is no type of the root's SACM family, or leaves no URI reference:
    // SACM-XMI-003, keeping the reference element.
    for (what, value) in [
        ("an abstract class", "sacm:Assertion o.xmi#c"),
        ("no class", "sacm:Nope o.xmi#c"),
        ("a class name in another case", "sacm:claim o.xmi#c"),
        ("a foreign prefix", "x:Claim o.xmi#c"),
        ("an unbound prefix", "q:Claim o.xmi#c"),
        // Unprefixed means the default namespace, and the reference element
        // stands in none.
        (
            "an unprefixed class with no default namespace",
            "Claim o.xmi#c",
        ),
        ("an empty prefix", ":Claim o.xmi#c"),
        ("an empty local name", "sacm: o.xmi#c"),
        ("white space in the URI part", "sacm:Claim o.xmi#a b"),
        ("two types", "sacm:Claim sacm:Claim o.xmi#c"),
        ("a typed URI with no fragment", "sacm:Claim o.xmi"),
        ("a typed URI with an empty fragment", "sacm:Claim o.xmi#"),
        ("a typed URI with two hashes", "sacm:Claim o.xmi#a#b"),
        ("an abstract class before #id", "sacm:Assertion #c2"),
        ("a foreign prefix before #id", "x:Claim #c2"),
        ("a type with no URI reference", "sacm:Claim"),
    ] {
        let what = format!("{what}: href={value:?}");
        let doc = assert_element_kept(
            &what,
            &citing(
                declarations,
                &format!("{MARK}<citedElement href=\"{value}\"/>"),
            ),
            "citedElement",
            "Claim",
            "c1",
        );
        let c1 = element(&what, &doc, "c1");
        assert!(
            c1.hrefs(ReferenceEnd::CitedElement).is_empty() && c1.cited_element().is_none(),
            "{ROW}: {what}: the kept element is also read"
        );
        let text = written(&what, &doc, Mode::Tolerant);
        assert!(
            text.contains(&format!("href=\"{value}\"")),
            "{ROW}: {what}: a tolerant write does not write the value back\n{text}"
        );
    }
}

// =================================================================================
// 3. Cross-file containment proxies
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_029_a_cross_file_containment_proxy_is_held_as_a_proxy() {
    // Golden L1.
    let input = file(L1);
    let golden = file(L1_GOLDEN);
    let what = "Golden L1";
    let doc = read_clean(what, &input);
    assert_eq!(
        proxies(what, &doc, "case"),
        pairs(&[
            ("argumentPackage", "arguments.sacm.xmi#args"),
            (
                "argumentPackage",
                "sacm:ArgumentPackageInterface braking.sacm.xmi#api"
            ),
        ]),
        "{ROW}: {what}: case.proxies()"
    );
    let held = element(what, &doc, "case").proxies();
    assert_eq!(
        (
            parts(held[0].href()),
            parts(held[1].href()),
            held[0] == held[0].clone(),
            held[0] == held[1]
        ),
        (
            (
                None,
                "arguments.sacm.xmi#args",
                "arguments.sacm.xmi",
                "args",
                false
            ),
            (
                Some(ElementKind::ArgumentPackageInterface),
                "braking.sacm.xmi#api",
                "braking.sacm.xmi",
                "api",
                false
            ),
            true,
            false
        ),
        "{ROW}: {what}: the hrefs of the proxies, and Proxy's =="
    );
    for other in ["b", "terms", "t1"] {
        assert!(
            element(what, &doc, other).proxies().is_empty(),
            "{ROW}: {what}: {other} holds a proxy"
        );
    }
    // A proxy is no child, and is never resolved.
    assert_eq!(
        (
            element(what, &doc, "case")
                .children()
                .map(|child| child.id().as_str().to_owned())
                .collect::<Vec<_>>(),
            root_ids(&doc),
            doc.element_count()
        ),
        (
            vec!["b".to_owned(), "terms".to_owned()],
            vec!["case".to_owned()],
            4
        ),
        "{ROW}: {what}: the children of case, the roots and the element count"
    );
    for mode in MODES {
        assert_eq!(
            written(what, &doc, mode),
            golden,
            "{ROW}: {what}: a {mode:?} write is not the L1 output"
        );
    }
    assert_eq!(
        round_trip_both(what, &doc),
        golden,
        "{ROW}: {what}: the round trip"
    );
    assert!(
        read_clean("the L1 output", &golden) == doc,
        "{ROW}: {what}: the output read back is not =="
    );

    // What keeps the role element whole instead: `element-kept`.
    for (what, proxy) in [
        (
            "a type its role does not admit",
            "<argumentPackage xmi:type=\"sacm:ArtifactPackage\" href=\"o.xmi#c\"/>",
        ),
        (
            "a type attribute and a prefix that disagree",
            "<argumentPackage xmi:type=\"sacm:ArgumentPackageInterface\" \
             href=\"sacm:ArgumentPackageBinding o.xmi#c\"/>",
        ),
        (
            "a type attribute and a prefix that disagree, the attribute last",
            "<argumentPackage href=\"sacm:ArgumentPackage o.xmi#c\" \
             xmi:type=\"sacm:ArgumentPackageInterface\"/>",
        ),
        (
            "href with a child",
            "<argumentPackage href=\"o.xmi#c\"><name content=\"x\"/></argumentPackage>",
        ),
        (
            "href with text",
            "<argumentPackage href=\"o.xmi#c\">x</argumentPackage>",
        ),
        (
            "href with xmi:uuid",
            "<argumentPackage href=\"o.xmi#c\" xmi:uuid=\"u\"/>",
        ),
        (
            "xmi:uuid with href",
            "<argumentPackage xmi:uuid=\"u\" href=\"o.xmi#c\"/>",
        ),
        (
            "href with xmi:label",
            "<argumentPackage href=\"o.xmi#c\" xmi:label=\"l\"/>",
        ),
        (
            "href with xmi:id",
            "<argumentPackage href=\"o.xmi#c\" xmi:id=\"x\"/>",
        ),
        (
            "href with another attribute",
            "<argumentPackage href=\"o.xmi#c\" foo=\"1\"/>",
        ),
        (
            "a typed proxy with xmi:uuid",
            "<argumentPackage xmi:type=\"sacm:ArgumentPackage\" href=\"o.xmi#c\" xmi:uuid=\"u\"/>",
        ),
        (
            "an href that names no element",
            "<argumentPackage href=\"o.xmi\"/>",
        ),
        (
            "a typed href whose type is no class",
            "<argumentPackage href=\"sacm:Nope o.xmi#c\"/>",
        ),
    ] {
        let doc = assert_element_kept(
            what,
            &in_case("", &format!("{MARK}{proxy}")),
            "argumentPackage",
            "AssuranceCasePackage",
            "case",
        );
        assert!(
            element(what, &doc, "case").proxies().is_empty(),
            "{ROW}: {what}: the kept element is also held as a proxy"
        );
    }

    // What is held: (what, the container's id, the input, the proxies).
    let artifact = root(
        "ArtifactPackage",
        "a",
        "",
        &el(
            "artifactElement",
            "Artifact",
            "art",
            "",
            "<property href=\"o.xmi#pr\"/>",
        ),
    );
    for (what, holder, input, expected) in [
        (
            "under the concrete role property of an Artifact",
            "art",
            artifact,
            vec![("property", "o.xmi#pr")],
        ),
        (
            "untyped, under the abstract role argumentElement",
            "p",
            in_p("", "<argumentElement href=\"o.xmi#c\"/>"),
            vec![("argumentElement", "o.xmi#c")],
        ),
        (
            "typed, under the abstract role argumentElement",
            "p",
            in_p(
                "",
                "<argumentElement xmi:type=\"sacm:Claim\" href=\"o.xmi#c\"/>",
            ),
            vec![("argumentElement", "sacm:Claim o.xmi#c")],
        ),
        (
            "typed by its href, under the abstract role argumentElement",
            "p",
            in_p("", "<argumentElement href=\"sacm:Claim o.xmi#c\"/>"),
            vec![("argumentElement", "sacm:Claim o.xmi#c")],
        ),
        (
            "under terminologyElement",
            "terms",
            root(
                "TerminologyPackage",
                "terms",
                "",
                "<terminologyElement href=\"o.xmi#t\"/>",
            ),
            vec![("terminologyElement", "o.xmi#t")],
        ),
        (
            "a type attribute and a prefix that agree",
            "case",
            in_case(
                "",
                "<argumentPackage xmi:type=\"sacm:ArgumentPackageInterface\" \
                 href=\"sacm:ArgumentPackageInterface o.xmi#api\"/>",
            ),
            vec![("argumentPackage", "sacm:ArgumentPackageInterface o.xmi#api")],
        ),
        (
            "the type attribute after the href",
            "case",
            in_case(
                "",
                "<argumentPackage href=\"o.xmi#api\" xmi:type=\"sacm:ArgumentPackageInterface\"/>",
            ),
            vec![("argumentPackage", "sacm:ArgumentPackageInterface o.xmi#api")],
        ),
        (
            "a same-document XPointer",
            "case",
            in_case("", "<argumentPackage href=\"#xpointer(id('x'))\"/>"),
            vec![("argumentPackage", "#xpointer(id('x'))")],
        ),
        (
            "a typed XPointer to another document",
            "case",
            in_case(
                "",
                "<artifactPackage xmi:type=\"sacm:ArtifactPackage\" href=\" o.xmi#xpointer(id('x')) \"/>",
            ),
            vec![(
                "artifactPackage",
                "sacm:ArtifactPackage o.xmi#xpointer(id('x'))",
            )],
        ),
        (
            "holding white space, a comment and a processing instruction",
            "case",
            in_case(
                "",
                "<argumentPackage href=\"o.xmi#x\"> <!-- c --> <?pi x?> </argumentPackage>",
            ),
            vec![("argumentPackage", "o.xmi#x")],
        ),
    ] {
        let doc = read_clean(what, &input);
        assert_eq!(
            proxies(what, &doc, holder),
            pairs(&expected),
            "{ROW}: {what}: proxies()"
        );
        assert_eq!(
            element(what, &doc, holder).children().count(),
            0,
            "{ROW}: {what}: the proxy is a child"
        );
        let text = round_trip_both(what, &doc);
        for (role, href) in expected {
            assert!(
                lines(&text).contains(&format!("<{role} href=\"{href}\" />").as_str()),
                "{ROW}: {what}: not written as <{role} href=\"{href}\" />\n{text}"
            );
        }
    }

    // In a group, `argumentElement` is a reference end, not a composite role:
    // the href is the end's, and no proxy.
    let what = "argumentElement href in an ArgumentGroup";
    let doc = read_clean(what, &grouping("", "<argumentElement href=\"o.xmi#c\"/>"));
    assert_eq!(
        (
            hrefs(what, &doc, "g", ReferenceEnd::GroupMember),
            element(what, &doc, "g").proxies().len()
        ),
        (vec!["o.xmi#c".to_owned()], 0),
        "{ROW}: {what}: not an href of the end and no proxy"
    );

    // Document order across roles and children, and written after the
    // children, in held order.
    let what = "proxies among children";
    let input = in_case(
        "",
        &format!(
            "<argumentPackage href=\"a.xmi#a\"/>{}<terminologyPackage href=\"t.xmi#t\"/>{}\
             <argumentPackage href=\"b.xmi#b\"/><argumentPackage href=\"a.xmi#a\"/>",
            el("artifactPackage", "ArtifactPackage", "arts", "", ""),
            el(
                "argumentPackage",
                "ArgumentPackage",
                "args",
                "",
                "<argumentElement href=\"c.xmi#c\"/>"
            ),
        ),
    );
    let doc = read_clean(what, &input);
    let expected = [
        ("argumentPackage", "a.xmi#a"),
        ("terminologyPackage", "t.xmi#t"),
        ("argumentPackage", "b.xmi#b"),
        ("argumentPackage", "a.xmi#a"),
    ];
    assert_eq!(
        (proxies(what, &doc, "case"), proxies(what, &doc, "args")),
        (pairs(&expected), pairs(&[("argumentElement", "c.xmi#c")])),
        "{ROW}: {what}: proxies(), in document order, repeats included"
    );
    let text = round_trip_both(what, &doc);
    let found = lines(&text);
    let position = |line: &str, from: usize| {
        found[from..]
            .iter()
            .position(|one| *one == line)
            .map_or_else(
                || panic!("{ROW}: {what}: no line {line} from line {from}:\n{text}"),
                |index| index + from,
            )
    };
    let nested = position("<argumentElement href=\"c.xmi#c\" />", 0);
    let args_end = position("</argumentPackage>", nested);
    assert_eq!(
        args_end,
        nested + 1,
        "{ROW}: {what}: the proxy of args is not the last line before its end tag\n{text}"
    );
    let first = position("<argumentPackage href=\"a.xmi#a\" />", args_end);
    assert_eq!(
        &found[first..],
        [
            "<argumentPackage href=\"a.xmi#a\" />",
            "<terminologyPackage href=\"t.xmi#t\" />",
            "<argumentPackage href=\"b.xmi#b\" />",
            "<argumentPackage href=\"a.xmi#a\" />",
            "</sacm:AssuranceCasePackage>",
        ],
        "{ROW}: {what}: the proxies of case are not written after its children, in held order\n{text}"
    );
    let arts = found
        .iter()
        .position(|one| one.contains("xmi:id=\"arts\""))
        .unwrap_or_else(|| panic!("{ROW}: {what}: arts is not written\n{text}"));
    assert!(
        arts < nested,
        "{ROW}: {what}: the children are not written in document order\n{text}"
    );

    // A type-prefixed `#id` is the id: a proxy of a root of the same
    // document, which moves into place (SACM23-XMI-030).
    let what = "a proxy with href=\"sacm:ArgumentPackage #args\"";
    let wrapped = format!(
        "<xmi:XMI{NS}><sacm:AssuranceCasePackage xmi:id=\"case\"><name content=\"case\"/>\
         <argumentPackage href=\"sacm:ArgumentPackage #args\"/></sacm:AssuranceCasePackage>\
         <sacm:ArgumentPackage xmi:id=\"args\"><name content=\"args\"/></sacm:ArgumentPackage></xmi:XMI>"
    );
    let doc = read_clean(what, &wrapped);
    assert_eq!(
        (
            root_ids(&doc),
            element(what, &doc, "args")
                .parent()
                .map(|parent| parent.id().as_str().to_owned()),
            element(what, &doc, "case").proxies().len()
        ),
        (vec!["case".to_owned()], Some("case".to_owned()), 0),
        "{ROW}: {what}: args is not moved into case"
    );
}

// =================================================================================
// 4. An href as an expression
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_029_an_expression_href_is_held_on_the_lang_string() {
    const EXPRESSION_NAME: &str = "<name xmi:type=\"sacm:ExpressionLangString\" lang=\"en\">\
                                   <expression href=\"glossary.sacm.xmi#blade\"/></name>";
    // Golden L1.
    let what = "Golden L1";
    let input = file(L1);
    assert!(
        input.contains(EXPRESSION_NAME),
        "{ROW}: setup: the L1 input does not hold the name of t1"
    );
    let doc = read_clean(what, &input);
    let Some(name) = element(what, &doc, "t1").name() else {
        panic!("{ROW}: {what}: t1 has no name");
    };
    assert_eq!(
        (
            name.expression_href().map(Href::as_str),
            name.expression(),
            name.is_expression_lang_string(),
            name.lang(),
            name.content()
        ),
        (
            Some("glossary.sacm.xmi#blade"),
            None,
            true,
            Some("en"),
            None
        ),
        "{ROW}: {what}: the name of t1"
    );
    assert_eq!(
        element(what, &doc, "t1").expressions(),
        [],
        "{ROW}: {what}: the href is among the expression ids of t1"
    );
    // The href counts as the expression being present: the one finding is
    // the Term's missing `value` (SACM23-TERM-003), which L1 does not give.
    let multiplicity = |doc: &Document| -> Vec<(String, String, Vec<String>)> {
        findings(doc)
            .into_iter()
            .filter(|(code, ..)| code == "SACM-MULT-001")
            .map(|(_, _, template, row, affected)| (template, row, affected))
            .collect()
    };
    assert_eq!(
        multiplicity(&doc),
        [(
            "SACM-MULT-001/missing".to_owned(),
            "SACM23-TERM-003".to_owned(),
            vec!["t1".to_owned()]
        )],
        "{ROW}: {what}: validate reports the expression of t1 as missing"
    );
    // With the Term's value, no SACM-MULT-001 at all; without the expression
    // element, the name's.
    let with_value = input.replacen("xmi:id=\"t1\">", "xmi:id=\"t1\" value=\"blade\">", 1);
    assert!(
        with_value != input,
        "{ROW}: setup: the L1 input has no start tag of t1 to give a value"
    );
    let what = "Golden L1 with the value of t1";
    let valued = read_clean(what, &with_value);
    assert_eq!(
        multiplicity(&valued),
        [],
        "{ROW}: {what}: validate gives SACM-MULT-001"
    );
    let what = "Golden L1 with the value of t1 and no expression";
    let without = read_clean(
        what,
        &with_value.replacen(
            EXPRESSION_NAME,
            "<name xmi:type=\"sacm:ExpressionLangString\" lang=\"en\"/>",
            1,
        ),
    );
    assert_eq!(
        multiplicity(&without),
        [(
            "SACM-MULT-001/name-without-expression".to_owned(),
            "SACM23-BASE-009".to_owned(),
            vec!["t1".to_owned()]
        )],
        "{ROW}: {what}: setup: the control gives no SACM-MULT-001 for the name"
    );
    // A content beside the href is SACM-EXPR-001, as beside an id.
    let what = "a content beside the expression href";
    let both = read_clean(
        what,
        &with_value.replacen(
            "<name xmi:type=\"sacm:ExpressionLangString\" lang=\"en\">",
            "<name xmi:type=\"sacm:ExpressionLangString\" lang=\"en\" content=\"x\">",
            1,
        ),
    );
    let name = element(what, &both, "t1").name();
    assert_eq!(
        (
            name.and_then(|name| name.expression_href())
                .map(Href::as_str),
            name.and_then(|name| name.content())
        ),
        (Some("glossary.sacm.xmi#blade"), Some("x")),
        "{ROW}: {what}: the name of t1"
    );
    let found = validation::validate(&both);
    assert_eq!(
        found
            .iter()
            .map(|one| (
                one.code().as_str(),
                one.severity(),
                one.requirement_id(),
                strs(one.affected())
            ))
            .collect::<Vec<_>>(),
        [(
            "SACM-EXPR-001",
            Severity::Warning,
            "SACM23-VAL-009",
            vec!["t1"]
        )],
        "{ROW}: {what}: validate does not give one SACM-EXPR-001"
    );
    assert!(
        found[0].template().starts_with("SACM-EXPR-001/"),
        "{ROW}: {what}: the template {}",
        found[0].template()
    );
    let text = round_trip_both(what, &both);
    let written_lines = lines(&text);
    let Some(at) = written_lines
        .iter()
        .position(|one| *one == "<expression href=\"glossary.sacm.xmi#blade\" />")
    else {
        panic!("{ROW}: {what}: the expression element is not written\n{text}");
    };
    assert!(
        written_lines[at - 1].starts_with("<name xmi:type=\"sacm:ExpressionLangString\" ")
            && written_lines[at - 1].contains(" content=\"x\"")
            && written_lines[at - 1].contains(" lang=\"en\"")
            && !written_lines[at - 1].ends_with("/>")
            && written_lines[at + 1] == "</name>",
        "{ROW}: {what}: the expression element is not the one child of the name\n{text}"
    );

    // A value of a Note's content is held the same way.
    let what = "an expression href in a value of a Note's content";
    let note = |value: &str| {
        in_p(
            "",
            &claim(
                "c1",
                "",
                &format!("<note xmi:id=\"n\"><content>{value}</content></note>"),
            ),
        )
    };
    let doc = read_clean(
        what,
        &note(
            "<value xmi:type=\"sacm:ExpressionLangString\" lang=\"en\">\
             <expression href=\"glossary.sacm.xmi#blade\"/></value><value lang=\"sv\" content=\"blad\"/>",
        ),
    );
    let Some(content) = element(what, &doc, "n").content() else {
        panic!("{ROW}: {what}: the Note has no content");
    };
    assert_eq!(
        content
            .values()
            .iter()
            .map(|value| (
                value.expression_href().map(Href::as_str),
                value.expression(),
                value.is_expression_lang_string(),
                value.lang()
            ))
            .collect::<Vec<_>>(),
        [
            (Some("glossary.sacm.xmi#blade"), None, true, Some("en")),
            (None, None, false, Some("sv"))
        ],
        "{ROW}: {what}: the values of the content"
    );
    assert_eq!(
        multiplicity(&doc),
        [],
        "{ROW}: {what}: validate gives SACM-MULT-001"
    );
    let text = round_trip_both(what, &doc);
    assert!(
        lines(&text).contains(&"<expression href=\"glossary.sacm.xmi#blade\" />"),
        "{ROW}: {what}: the expression element is not written\n{text}"
    );

    // An XPointer and a typed href are held too; `#id` stays the id.
    for (what, value, expected) in [
        (
            "a same-document XPointer as an expression",
            "#xpointer(id('t2'))",
            (None, "#xpointer(id('t2'))", "", "xpointer(id('t2'))", true),
        ),
        (
            "an XPointer to another document as an expression",
            " g.xmi#xpointer(id('t')) ",
            (
                None,
                "g.xmi#xpointer(id('t'))",
                "g.xmi",
                "xpointer(id('t'))",
                true,
            ),
        ),
        (
            "a typed href as an expression",
            "sacm:Term g.xmi#blade",
            (
                Some(ElementKind::Term),
                "g.xmi#blade",
                "g.xmi",
                "blade",
                false,
            ),
        ),
    ] {
        let doc = read_clean(
            what,
            &naming(
                "",
                &format!(
                    "<name xmi:type=\"sacm:ExpressionLangString\"><expression href=\"{value}\"/></name>"
                ),
            ),
        );
        let name = element(what, &doc, "t1").name();
        let Some(href) = name.and_then(|name| name.expression_href()) else {
            panic!("{ROW}: {what}: the name of t1 has no expression href: {name:?}");
        };
        assert_eq!(
            (parts(href), name.and_then(|name| name.expression())),
            (expected, None),
            "{ROW}: {what}: the expression href"
        );
        assert_eq!(
            multiplicity(&doc),
            [],
            "{ROW}: {what}: validate gives SACM-MULT-001"
        );
        round_trip_both(what, &doc);
    }
    for (what, value) in [
        ("href=\"#t2\" as an expression", "#t2"),
        ("href=\"sacm:Term #t2\" as an expression", "sacm:Term #t2"),
    ] {
        let doc = read_clean(
            what,
            &naming(
                "",
                &format!(
                    "<name xmi:type=\"sacm:ExpressionLangString\"><expression href=\"{value}\"/></name>"
                ),
            ),
        );
        let name = element(what, &doc, "t1").name();
        assert_eq!(
            (
                name.and_then(|name| name.expression())
                    .map(ElementId::as_str),
                name.and_then(|name| name.expression_href())
            ),
            (Some("t2"), None),
            "{ROW}: {what}: not read as the id t2"
        );
        assert_eq!(
            strs(element(what, &doc, "t1").expressions()),
            ["t2"],
            "{ROW}: {what}: expressions()"
        );
    }
}

// =================================================================================
// 5. Identity and XLink attributes on a reference element
// =================================================================================

#[test]
fn sacm23_xmi_029_identity_and_xlink_attributes_on_a_reference_element_are_sacm_xmi_003() {
    let declarations = format!("{XSI}{XLINK}");
    // The attributes; for an `expression`, the type names its target's class.
    let attributes = [
        "xmi:id=\"r\"",
        "xmi:type=\"sacm:Claim\"",
        "xsi:type=\"sacm:Claim\"",
        "xmi:uuid=\"u\"",
        "xmi:label=\"l\"",
        "xlink:href=\"o.xmi#c\" xlink:type=\"simple\"",
        "xlink:href=\"o.xmi#c\"",
        "xlink:type=\"simple\"",
        "foo=\"1\"",
    ];
    // (the link, or none: an element with XLink attributes only)
    let links = ["xmi:idref=\"c2\"", "href=\"o.xmi#c\"", "href=\"#c2\"", ""];
    for attribute in attributes {
        for link in links {
            let is_xlink = attribute.starts_with("xlink:");
            if link.is_empty() && !is_xlink {
                // A reference element with no link at all is another row's.
                continue;
            }
            for body in [format!("{link} {attribute}"), format!("{attribute} {link}")] {
                let body = body.trim();
                // citedElement of a Claim.
                let what = format!("<citedElement {body}/>");
                let doc = assert_element_kept(
                    &what,
                    &citing(&declarations, &format!("{MARK}<citedElement {body}/>")),
                    "citedElement",
                    "Claim",
                    "c1",
                );
                assert_eq!(
                    (
                        ids(&what, &doc, "c1", ReferenceEnd::CitedElement),
                        hrefs(&what, &doc, "c1", ReferenceEnd::CitedElement)
                    ),
                    (Vec::new(), Vec::new()),
                    "{ROW}: {what}: the kept element is also read"
                );
                // argumentElement of an ArgumentGroup.
                let what = format!("<argumentElement {body}/> in a group");
                let doc = assert_element_kept(
                    &what,
                    &grouping(&declarations, &format!("{MARK}<argumentElement {body}/>")),
                    "argumentElement",
                    "ArgumentGroup",
                    "g",
                );
                assert_eq!(
                    (
                        ids(&what, &doc, "g", ReferenceEnd::GroupMember),
                        hrefs(&what, &doc, "g", ReferenceEnd::GroupMember)
                    ),
                    (Vec::new(), Vec::new()),
                    "{ROW}: {what}: the kept element is also read"
                );
                // expression of an ExpressionLangString: the whole name is
                // kept.
                let body = body
                    .replace("sacm:Claim", "sacm:Term")
                    .replace("\"c2\"", "\"t2\"")
                    .replace("#c2", "#t2");
                let what = format!("<expression {body}/> in a name");
                let doc = assert_element_kept(
                    &what,
                    &naming(
                        &declarations,
                        &format!(
                            "{MARK}<name xmi:type=\"sacm:ExpressionLangString\" lang=\"en\">\
                             <expression {body}/></name>"
                        ),
                    ),
                    "name",
                    "Term",
                    "t1",
                );
                assert!(
                    element(&what, &doc, "t1").name().is_none()
                        && element(&what, &doc, "t1").expressions().is_empty(),
                    "{ROW}: {what}: the kept name is also read"
                );
            }
        }
    }
}

// =================================================================================
// 6. href beside xmi:idref
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_029_href_beside_xmi_idref_is_sacm_xmi_003() {
    let hrefs_tried = [
        "o.xmi#c",
        "#c2",
        "o.xmi#xpointer(id('c'))",
        "sacm:Claim o.xmi#c",
    ];
    for href in hrefs_tried {
        for links in [
            format!("xmi:idref=\"c2\" href=\"{href}\""),
            format!("href=\"{href}\" xmi:idref=\"c2\""),
        ] {
            // On a reference element: citedElement, and a group's
            // argumentElement.
            let what = format!("<citedElement {links}/>");
            let doc = assert_element_kept(
                &what,
                &citing("", &format!("{MARK}<citedElement {links}/>")),
                "citedElement",
                "Claim",
                "c1",
            );
            assert_eq!(
                (
                    ids(&what, &doc, "c1", ReferenceEnd::CitedElement),
                    hrefs(&what, &doc, "c1", ReferenceEnd::CitedElement)
                ),
                (Vec::new(), Vec::new()),
                "{ROW}: {what}: the kept element is also read"
            );
            let what = format!("<argumentElement {links}/> in a group");
            let doc = assert_element_kept(
                &what,
                &grouping("", &format!("{MARK}<argumentElement {links}/>")),
                "argumentElement",
                "ArgumentGroup",
                "g",
            );
            assert_eq!(
                (
                    ids(&what, &doc, "g", ReferenceEnd::GroupMember),
                    hrefs(&what, &doc, "g", ReferenceEnd::GroupMember)
                ),
                (Vec::new(), Vec::new()),
                "{ROW}: {what}: the kept element is also read"
            );
            // On an `expression`: the whole name is kept.
            let in_name = links.replace("\"c2\"", "\"t2\"").replace("#c2", "#t2");
            let what = format!("<expression {in_name}/> in a name");
            let doc = assert_element_kept(
                &what,
                &naming(
                    "",
                    &format!(
                        "{MARK}<name xmi:type=\"sacm:ExpressionLangString\"><expression {in_name}/></name>"
                    ),
                ),
                "name",
                "Term",
                "t1",
            );
            assert!(
                element(&what, &doc, "t1").name().is_none(),
                "{ROW}: {what}: the kept name is also read"
            );
            // On a proxy: under a concrete and an abstract role, untyped and
            // typed. `c2` is no root, and `args` is one: neither moves.
            for (role, class, holder_class, holder, type_attribute) in [
                (
                    "argumentPackage",
                    "AssuranceCasePackage",
                    "AssuranceCasePackage",
                    "case",
                    "",
                ),
                (
                    "argumentPackage",
                    "AssuranceCasePackage",
                    "AssuranceCasePackage",
                    "case",
                    "xmi:type=\"sacm:ArgumentPackage\" ",
                ),
                (
                    "argumentElement",
                    "ArgumentPackage",
                    "ArgumentPackage",
                    "case",
                    "",
                ),
                (
                    "argumentElement",
                    "ArgumentPackage",
                    "ArgumentPackage",
                    "case",
                    "xmi:type=\"sacm:ArgumentPackage\" ",
                ),
            ] {
                let on_proxy = links.replace("\"c2\"", "\"args\"").replace("#c2", "#args");
                let what = format!("the proxy <{role} {type_attribute}{on_proxy}/> in a {class}");
                let template = format!(
                    "<xmi:XMI{NS}><sacm:{class} xmi:id=\"{holder}\"><name content=\"{holder}\"/>\
                     {MARK}<{role} {type_attribute}{on_proxy}/></sacm:{class}>\
                     <sacm:ArgumentPackage xmi:id=\"args\"><name content=\"args\"/></sacm:ArgumentPackage>\
                     </xmi:XMI>"
                );
                let doc = assert_element_kept(&what, &template, role, holder_class, holder);
                assert_eq!(
                    (
                        root_ids(&doc),
                        element(&what, &doc, holder).children().count()
                    ),
                    (vec![holder.to_owned(), "args".to_owned()], 0),
                    "{ROW}: {what}: a root moved, or the kept proxy is also read"
                );
            }
        }
    }
}

// =================================================================================
// 8. Proxies in compare, delete, copy, move and validate
// =================================================================================

/// The differences of `left` and `right`, each as its variant's name; an
/// `Hrefs` with its end.
fn difference_names(left: &Document, right: &Document) -> Vec<String> {
    semantic_compare(left, right)
        .iter()
        .map(|difference| match difference {
            Difference::Hrefs { end, .. } => format!("Hrefs({})", end.name()),
            Difference::Proxies { .. } => "Proxies".to_owned(),
            Difference::Name { .. } => "Name".to_owned(),
            Difference::ChildOrder { .. } => "ChildOrder".to_owned(),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_029_proxies_are_compared_deleted_copied_and_moved() {
    // --- Compare -----------------------------------------------------------
    let with = |proxies: &str| in_case("", proxies);
    let left = read_clean("left", &with("<argumentPackage href=\"a.xmi#a\"/>"));
    let right = read_clean(
        "right",
        &with(
            "<argumentPackage href=\"sacm:ArgumentPackage a.xmi#a\"/><artifactPackage href=\"b.xmi#b\"/>",
        ),
    );
    let found = semantic_compare(&left, &right);
    let [
        Difference::Proxies {
            id,
            left: l,
            right: r,
        },
    ] = found.as_slice()
    else {
        panic!("{ROW}: not one Proxies difference: {found:?}");
    };
    let shown = |proxies: &[Proxy]| -> Vec<(&'static str, String)> {
        proxies
            .iter()
            .map(|proxy| (proxy.role(), proxy.href().as_str().to_owned()))
            .collect()
    };
    assert_eq!(
        (id.as_str(), shown(l), shown(r)),
        (
            "case",
            pairs(&[("argumentPackage", "a.xmi#a")]),
            pairs(&[
                ("argumentPackage", "sacm:ArgumentPackage a.xmi#a"),
                ("artifactPackage", "b.xmi#b")
            ])
        ),
        "{ROW}: the Proxies difference"
    );
    assert_eq!(
        (
            found[0].element_id().map(ElementId::as_str),
            l.as_slice() == element("left", &left, "case").proxies(),
            r.as_slice() == element("right", &right, "case").proxies()
        ),
        (Some("case"), true, true),
        "{ROW}: element_id(), and the sides are the documents' proxies"
    );
    assert!(
        left != right,
        "{ROW}: documents that differ in proxies are =="
    );
    let swapped = semantic_compare(&right, &left);
    assert!(
        matches!(
            swapped.as_slice(),
            [Difference::Proxies { left, right, .. }] if left.len() == 2 && right.len() == 1
        ),
        "{ROW}: the swapped comparison: {swapped:?}"
    );
    // One side with none; another role; another order: each is one Proxies
    // difference.
    let a = "<argumentPackage href=\"a.xmi#a\"/>";
    let b = "<argumentPackage href=\"b.xmi#b\"/>";
    for (what, one, other) in [
        ("a proxy against none", with(a), with("")),
        (
            "another role",
            with("<artifactPackage href=\"a.xmi#a\"/>"),
            with("<terminologyPackage href=\"a.xmi#a\"/>"),
        ),
        (
            "another order",
            with(&format!("{a}{b}")),
            with(&format!("{b}{a}")),
        ),
        ("a repeat", with(&format!("{a}{a}")), with(a)),
    ] {
        let one = read_clean(what, &one);
        let other = read_clean(what, &other);
        assert_eq!(
            (difference_names(&one, &other), one == other),
            (vec!["Proxies".to_owned()], false),
            "{ROW}: {what}: not one Proxies difference"
        );
    }
    let again = read_clean("the same proxies", &with(&format!("{a}{b}")));
    let same = read_clean("the same proxies", &with(&format!("{a}{b}")));
    assert!(
        semantic_compare(&again, &same).is_empty() && again == same,
        "{ROW}: equal proxies are not =="
    );
    // The order: right after the Hrefs of the element, before its Name.
    let ordered = |cited: &str, proxy: &str, name: &str| {
        format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\" isCitation=\"true\">\
             <name content=\"{name}\"/><citedElement href=\"{cited}\"/>\
             <argumentPackage href=\"{proxy}\"/></sacm:AssuranceCasePackage>"
        )
    };
    let left = read_clean("left of three", &ordered("o.xmi#c", "a.xmi#a", "case"));
    let right = read_clean("right of three", &ordered("o.xmi#d", "b.xmi#b", "Case"));
    assert_eq!(
        difference_names(&left, &right),
        ["Hrefs(citedElement)", "Proxies", "Name"],
        "{ROW}: the order of the differences"
    );

    // --- Delete ------------------------------------------------------------
    let nested = |inner: &str, extra: &str| {
        in_case(
            "",
            &format!(
                "{}{}{extra}",
                el(
                    "assuranceCasePackage",
                    "AssuranceCasePackage",
                    "inner",
                    "",
                    inner
                ),
                el(
                    "assuranceCasePackage",
                    "AssuranceCasePackage",
                    "other",
                    "",
                    ""
                ),
            ),
        )
    };
    let args = el("argumentPackage", "ArgumentPackage", "args", "", "");
    for (what, inner, contained) in [
        ("only a proxy", a.to_owned(), 1_usize),
        ("two proxies", format!("{a}{b}"), 2),
        ("a proxy and a child", format!("{a}{args}"), 2),
    ] {
        let mut doc = read_clean(what, &nested(&inner, ""));
        let error = refuse(
            &mut doc,
            DeleteElement::new("inner").with_package_policy(PackageDeletePolicy::RejectIfNonEmpty),
        );
        let EditError::NotEmpty {
            element: holder,
            contained: counted,
            ..
        } = &error
        else {
            panic!("{ROW}: {what}: RejectIfNonEmpty is not NotEmpty: {error:?}");
        };
        assert_eq!(
            (holder.as_str(), *counted),
            ("inner", contained),
            "{ROW}: {what}: NotEmpty"
        );
        let mutation = applied(
            &mut doc,
            DeleteElement::new("inner").with_package_policy(PackageDeletePolicy::DeleteRecursively),
        );
        assert!(
            mutation
                .deleted_ids()
                .iter()
                .any(|id| id.as_str() == "inner")
                && !doc.contains("inner")
                && element(what, &doc, "case").proxies().is_empty(),
            "{ROW}: {what}: DeleteRecursively does not delete inner and its proxies"
        );
        round_trip_both(what, &doc);
    }
    // The control: with no proxy, RejectIfNonEmpty deletes it.
    let mut doc = read_clean("no proxy", &nested("", ""));
    applied(
        &mut doc,
        DeleteElement::new("inner").with_package_policy(PackageDeletePolicy::RejectIfNonEmpty),
    );
    assert!(
        !doc.contains("inner"),
        "{ROW}: setup: an empty package is not deleted under RejectIfNonEmpty"
    );
    // A proxy of a package under the one to delete counts too.
    let what = "a proxy in a package of the one to delete";
    let mut doc = read_clean(
        what,
        &nested(
            &el(
                "argumentPackage",
                "ArgumentPackage",
                "args",
                "",
                "<argumentElement href=\"c.xmi#c\"/>",
            ),
            "",
        ),
    );
    let error = refuse(
        &mut doc,
        DeleteElement::new("inner").with_package_policy(PackageDeletePolicy::RejectIfNonEmpty),
    );
    assert!(
        matches!(error, EditError::NotEmpty { contained: 2, .. }),
        "{ROW}: {what}: not NotEmpty {{ contained: 2 }}: {error:?}"
    );

    // --- Copy and move -----------------------------------------------------
    let both = format!("{a}<artifactPackage href=\"sacm:ArtifactPackage b.xmi#b\"/>");
    let expected = pairs(&[
        ("argumentPackage", "a.xmi#a"),
        ("artifactPackage", "sacm:ArtifactPackage b.xmi#b"),
    ]);
    let what = "a copy";
    let mut doc = read_clean(what, &nested(&both, ""));
    let copy = CopyElements::new(&doc, ["inner"], "other")
        .unwrap_or_else(|error| panic!("{ROW}: setup: the copy of inner: {error}"));
    let mutation = applied(&mut doc, copy);
    let Some(copied) = mutation
        .created_ids()
        .first()
        .map(|id| id.as_str().to_owned())
    else {
        panic!("{ROW}: {what}: the copy created nothing");
    };
    assert_eq!(
        (
            proxies(what, &doc, &copied),
            proxies(what, &doc, "inner"),
            element(what, &doc, &copied)
                .parent()
                .map(|parent| parent.id().as_str().to_owned())
        ),
        (expected.clone(), expected.clone(), Some("other".to_owned())),
        "{ROW}: {what}: the copy, or its source, does not keep the proxies"
    );
    round_trip_both(what, &doc);
    let what = "a move";
    let mut doc = read_clean(what, &nested(&both, ""));
    applied(&mut doc, MoveElement::new("inner", "other"));
    assert_eq!(
        (
            proxies(what, &doc, "inner"),
            element(what, &doc, "inner")
                .parent()
                .map(|parent| parent.id().as_str().to_owned()),
            proxies(what, &doc, "other"),
            proxies(what, &doc, "case")
        ),
        (expected, Some("other".to_owned()), Vec::new(), Vec::new()),
        "{ROW}: {what}: the move does not keep the proxies with inner"
    );
    round_trip_both(what, &doc);

    // --- Validate ----------------------------------------------------------
    // The package `p` nests the ArgumentPackage `q` and holds the Claim `c1`
    // (SACM-PKG-001), and `c1` names nothing as its metaClaim (SACM-REF-001):
    // proxies of a Claim and of an ArgumentPackage in `p` and in `q` change
    // nothing.
    let finding = |proxies_of_p: &str, proxies_of_q: &str| {
        in_p(
            "",
            &format!(
                "{}{}{proxies_of_p}",
                claim("c1", " metaClaim=\"nope\"", ""),
                el("argumentElement", "ArgumentPackage", "q", "", proxies_of_q),
            ),
        )
    };
    let without = read_clean("no proxies", &finding("", ""));
    let with_proxies = read_clean(
        "proxies",
        &finding(
            "<argumentElement xmi:type=\"sacm:Claim\" href=\"o.xmi#c\"/>\
             <argumentElement xmi:type=\"sacm:ArgumentPackage\" href=\"o.xmi#p\"/>\
             <argumentElement href=\"o.xmi#x\"/>",
            "<argumentElement xmi:type=\"sacm:Claim\" href=\"o.xmi#c\"/>",
        ),
    );
    assert_eq!(
        (
            element("proxies", &with_proxies, "p").proxies().len(),
            element("proxies", &with_proxies, "q").proxies().len()
        ),
        (3, 1),
        "{ROW}: setup: the proxies of p and q"
    );
    let expected = validation::validate(&without);
    assert!(
        expected
            .iter()
            .any(|one| one.code().as_str() == "SACM-PKG-001")
            && expected
                .iter()
                .any(|one| one.code().as_str() == "SACM-REF-001"),
        "{ROW}: setup: the control gives no SACM-PKG-001 and SACM-REF-001: {:?}",
        findings(&without)
    );
    assert_eq!(
        validation::validate(&with_proxies),
        expected,
        "{ROW}: validate gives other diagnostics with the proxies"
    );
}

// =================================================================================
// 9. The accessors of an Href
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the forms of one acceptance item read best as one list"
)]
fn sacm23_xmi_029_href_accessors() {
    // (the value as read, as_str, kind, uri, document, fragment, is_xpointer)
    let forms = [
        ("plain", "o.xmi#c", None, "o.xmi#c", "o.xmi", "c", false),
        (
            "an XPointer",
            "o.xmi#xpointer(id('c'))",
            None,
            "o.xmi#xpointer(id('c'))",
            "o.xmi",
            "xpointer(id('c'))",
            true,
        ),
        (
            "a same-document XPointer",
            "#xpointer(id('c'))",
            None,
            "#xpointer(id('c'))",
            "",
            "xpointer(id('c'))",
            true,
        ),
        (
            "typed",
            "sacm:Claim o.xmi#c1",
            Some(ElementKind::Claim),
            "o.xmi#c1",
            "o.xmi",
            "c1",
            false,
        ),
        (
            "a typed same-document XPointer",
            "sacm:ArgumentPackageInterface #xpointer(x)",
            Some(ElementKind::ArgumentPackageInterface),
            "#xpointer(x)",
            "",
            "xpointer(x)",
            true,
        ),
        (
            "a URL with a path",
            "http://example.org/a/b.xmi#c_1",
            None,
            "http://example.org/a/b.xmi#c_1",
            "http://example.org/a/b.xmi",
            "c_1",
            false,
        ),
        (
            "typed, with a colon in the document part",
            "sacm:Term http://example.org/g.xmi#blade",
            Some(ElementKind::Term),
            "http://example.org/g.xmi#blade",
            "http://example.org/g.xmi",
            "blade",
            false,
        ),
    ];
    for (what, value, kind, uri, document, fragment, is_xpointer) in forms {
        // As the reader holds it, on the metaClaim of a Claim.
        let doc = read_clean(
            what,
            &in_p(
                "",
                &claim("c1", "", &format!("<metaClaim href=\"{value}\"/>")),
            ),
        );
        let read = one_href(what, &doc, "c1", ReferenceEnd::MetaClaim);
        assert_eq!(
            (read.as_str(), parts(read)),
            (value, (kind, uri, document, fragment, is_xpointer)),
            "{ROW}: {what}: the href as read"
        );
        assert_eq!(
            (read.to_string(), AsRef::<str>::as_ref(read)),
            (value.to_owned(), value),
            "{ROW}: {what}: Display and AsRef are as_str"
        );
        // As `Href::new` builds it: the same value.
        let built = match Href::new(value) {
            Ok(built) => built,
            Err(error) => panic!("{ROW}: {what}: Href::new({value:?}) refused it: {error}"),
        };
        assert_eq!(
            (&built == read, built.as_str(), parts(&built)),
            (true, value, (kind, uri, document, fragment, is_xpointer)),
            "{ROW}: {what}: the href as built"
        );
    }
    // Equality and ordering stay those of the whole string: a typed href is
    // not the untyped one.
    let doc = read_clean(
        "typed and untyped",
        &in_p(
            "",
            &claim(
                "c1",
                "",
                "<metaClaim href=\"sacm:Claim o.xmi#c\"/><metaClaim href=\"o.xmi#c\"/>",
            ),
        ),
    );
    let found = element("typed and untyped", &doc, "c1").hrefs(ReferenceEnd::MetaClaim);
    assert_eq!(found.len(), 2, "{ROW}: setup: the two hrefs");
    assert_eq!(
        (
            found[0] == found[1],
            found[0].cmp(&found[1]),
            found[0].uri() == found[1].uri()
        ),
        (false, "sacm:Claim o.xmi#c".cmp("o.xmi#c"), true),
        "{ROW}: Eq and Ord are not the whole string's"
    );
}

// =================================================================================
// 10. The diagnostics of a kept same-file proxy
// =================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_029_a_kept_proxy_s_diagnostics_affect_its_container() {
    /// A diagnostic as its code, severity, template, location and affected
    /// ids.
    type Summary = (
        &'static str,
        Severity,
        &'static str,
        Option<(u32, u32)>,
        Vec<String>,
    );
    /// `found` without its arguments.
    fn summary(found: &Found) -> Summary {
        (found.0, found.1, found.2, found.3, found.4.clone())
    }
    let no_root = |mode: Mode,
                   at: (u32, u32),
                   role: &str,
                   target: &str,
                   holder_class: &str,
                   holder: &str|
     -> Found {
        (
            "SACM-XMI-003",
            kept_severity(mode),
            "SACM-XMI-003/proxy-of-no-root",
            Some(at),
            vec![holder.to_owned()],
            vec![
                ("element", role.to_owned()),
                ("target", target.to_owned()),
                ("holder_class", holder_class.to_owned()),
                ("holder_id", holder.to_owned()),
            ],
        )
    };
    let assert_none_affects = |what: &str, output: &ReadOutput, id: &str| {
        for diagnostic in output.diagnostics() {
            assert!(
                diagnostic.affected().iter().all(|one| one.as_str() != id),
                "{ROW}: {what}: {} names {id:?} as affected: {:?}",
                diagnostic.template(),
                diagnostic.affected()
            );
        }
    };
    // SACM-XMI-010 on a kept proxy: the proxy's role, its container and the
    // class its xsi:type names; never the id the proxy names.
    let typed = |mode: Mode,
                 at: (u32, u32),
                 role: &str,
                 class: &str,
                 holder_class: &str,
                 holder: &str|
     -> Found {
        (
            "SACM-XMI-010",
            kept_severity(mode),
            "SACM-XMI-010/xsi-type-on-proxy",
            Some(at),
            vec![holder.to_owned()],
            vec![
                ("element", role.to_owned()),
                ("holder_class", holder_class.to_owned()),
                ("holder_id", holder.to_owned()),
                ("class", class.to_owned()),
            ],
        )
    };
    // Only the target of the SACM-XMI-003 that keeps the proxy names the id
    // the proxy names.
    let assert_no_argument_names = |what: &str, output: &ReadOutput, id: &str| {
        for diagnostic in output.diagnostics() {
            for argument in diagnostic.arguments() {
                let target_of_kept =
                    diagnostic.code().as_str() == "SACM-XMI-003" && argument.name() == "target";
                assert!(
                    target_of_kept || *argument.value() != ArgumentValue::Text(id.to_owned()),
                    "{ROW}: {what}: the argument {} of {} is {id:?}",
                    argument.name(),
                    diagnostic.template()
                );
            }
        }
    };

    // The model's spelling of ArtifactAssetRelationship as the proxy's type,
    // read tolerantly (SACM23-XMI-009): the proxy names no root and is kept.
    let what = "a proxy typed sacm:ArtifactAssertedRelationship, Tolerant";
    let (input, marks) = marked(&root(
        "ArtifactPackage",
        "holder",
        "",
        &format!(
            "{MARK}<artifactElement xmi:type=\"sacm:ArtifactAssertedRelationship\" xmi:idref=\"nope\"/>"
        ),
    ));
    let output = read_ok(what, Mode::Tolerant, &input);
    assert_eq!(
        diagnostics(&output),
        [
            no_root(
                Mode::Tolerant,
                marks[0],
                "artifactElement",
                "nope",
                "ArtifactPackage",
                "holder"
            ),
            (
                "SACM-XMI-013",
                Severity::Warning,
                "SACM-XMI-013/model-type-in-root-read",
                Some(marks[0]),
                vec!["holder".to_owned()],
                vec![
                    ("element", "artifactElement".to_owned()),
                    ("container", "sacm:ArtifactPackage".to_owned()),
                ],
            ),
        ],
        "{ROW}: {what}: not proxy-of-no-root and its SACM-XMI-013, each affecting the container"
    );
    assert_none_affects(what, &output, "nope");
    assert!(
        element(what, output.document(), "holder").has_preserved_content(),
        "{ROW}: {what}: the holder does not keep the proxy"
    );

    // A proxy typed with xsi:type that names no root: its SACM-XMI-010, at
    // the attribute, affects the container too, in both modes; under a
    // concrete and an abstract role.
    for (about, input, role, class, holder_class, holder) in [
        (
            "an xsi:type proxy of nope under argumentPackage",
            in_case(
                XSI,
                &format!(
                    "{MARK}<argumentPackage {MARK}xsi:type=\"sacm:ArgumentPackage\" xmi:idref=\"nope\"/>"
                ),
            ),
            "argumentPackage",
            "ArgumentPackage",
            "AssuranceCasePackage",
            "case",
        ),
        (
            "an xsi:type proxy of nope under argumentElement",
            in_p(
                XSI,
                &format!(
                    "{MARK}<argumentElement {MARK}xsi:type=\"sacm:Claim\" xmi:idref=\"nope\"/>"
                ),
            ),
            "argumentElement",
            "Claim",
            "ArgumentPackage",
            "p",
        ),
        (
            "an xsi:type proxy of nope by href",
            in_case(
                XSI,
                &format!(
                    "{MARK}<argumentPackage href=\"#nope\" {MARK}xsi:type=\"sacm:ArgumentPackage\"/>"
                ),
            ),
            "argumentPackage",
            "ArgumentPackage",
            "AssuranceCasePackage",
            "case",
        ),
    ] {
        let (input, marks) = marked(&input);
        for mode in MODES {
            let what = format!("{about}, {mode:?}");
            let output = read_ok(&what, mode, &input);
            let found = diagnostics(&output);
            assert_eq!(
                found.iter().map(summary).collect::<Vec<_>>(),
                [
                    summary(&no_root(mode, marks[0], role, "nope", holder_class, holder)),
                    (
                        "SACM-XMI-010",
                        kept_severity(mode),
                        "SACM-XMI-010/xsi-type-on-proxy",
                        Some(marks[1]),
                        vec![holder.to_owned()],
                    ),
                ],
                "{ROW}: {what}: not proxy-of-no-root and its SACM-XMI-010, each affecting the container"
            );
            assert_eq!(
                found[0],
                no_root(mode, marks[0], role, "nope", holder_class, holder),
                "{ROW}: {what}: the proxy-of-no-root"
            );
            assert_eq!(
                found[1],
                typed(mode, marks[1], role, class, holder_class, holder),
                "{ROW}: {what}: the SACM-XMI-010, its template and every argument"
            );
            assert_none_affects(&what, &output, "nope");
            assert_no_argument_names(&what, &output, "nope");
        }
    }

    // A spelled role and xsi:type together, read tolerantly: three
    // diagnostics, each affecting the container.
    let what = "an xsi:type proxy of nope in the role argumentationElement, Tolerant";
    let (input, marks) = marked(&in_p(
        XSI,
        &format!("{MARK}<argumentationElement {MARK}xsi:type=\"sacm:Claim\" xmi:idref=\"nope\"/>"),
    ));
    let output = read_ok(what, Mode::Tolerant, &input);
    assert_eq!(
        diagnostics(&output).iter().map(summary).collect::<Vec<_>>(),
        [
            (
                "SACM-XMI-003",
                Severity::Warning,
                "SACM-XMI-003/proxy-of-no-root",
                Some(marks[0]),
                vec!["p".to_owned()]
            ),
            (
                "SACM-XMI-013",
                Severity::Warning,
                "SACM-XMI-013/package-role-in-root-read",
                Some(marks[0]),
                vec!["p".to_owned()]
            ),
            (
                "SACM-XMI-010",
                Severity::Warning,
                "SACM-XMI-010/xsi-type-on-proxy",
                Some(marks[1]),
                vec!["p".to_owned()]
            ),
        ],
        "{ROW}: {what}: not three diagnostics, each affecting the container"
    );
    assert_eq!(
        diagnostics(&output).get(2),
        Some(&typed(
            Mode::Tolerant,
            marks[1],
            "argumentationElement",
            "Claim",
            "ArgumentPackage",
            "p"
        )),
        "{ROW}: {what}: the SACM-XMI-010, its template and every argument"
    );
    assert_none_affects(what, &output, "nope");
    assert_no_argument_names(what, &output, "nope");

    // A kept proxy that names a root of the wrong kind, or one another proxy
    // names too: its SACM-XMI-010 affects the container, not that root.
    for (about, proxies_of_case, template, holders, role, class) in [
        (
            "an xsi:type proxy of the ArtifactPackage arts under argumentPackage",
            format!(
                "{MARK}<argumentPackage {MARK}xsi:type=\"sacm:ArgumentPackage\" xmi:idref=\"arts\"/>"
            ),
            "SACM-XMI-003/proxy-wrong-kind",
            1,
            "argumentPackage",
            "ArgumentPackage",
        ),
        (
            "two xsi:type proxies of arts",
            format!(
                "{MARK}<artifactPackage {MARK}xsi:type=\"sacm:ArtifactPackage\" xmi:idref=\"arts\"/>\
                 {MARK}<artifactPackage {MARK}xsi:type=\"sacm:ArtifactPackage\" xmi:idref=\"arts\"/>"
            ),
            "SACM-XMI-003/proxy-shared",
            2,
            "artifactPackage",
            "ArtifactPackage",
        ),
    ] {
        let (input, marks) = marked(&format!(
            "<xmi:XMI{NS}{XSI}><sacm:AssuranceCasePackage xmi:id=\"case\"><name content=\"case\"/>\
             {proxies_of_case}</sacm:AssuranceCasePackage>\
             <sacm:ArtifactPackage xmi:id=\"arts\"><name content=\"arts\"/></sacm:ArtifactPackage></xmi:XMI>"
        ));
        for mode in MODES {
            let what = format!("{about}, {mode:?}");
            let output = read_ok(&what, mode, &input);
            let mut expected: Vec<String> = (0..holders)
                .map(|index| -> Summary {
                    (
                        "SACM-XMI-003",
                        kept_severity(mode),
                        template,
                        Some(marks[2 * index]),
                        vec!["case".to_owned()],
                    )
                })
                .chain((0..holders).map(|index| -> Summary {
                    (
                        "SACM-XMI-010",
                        kept_severity(mode),
                        "SACM-XMI-010/xsi-type-on-proxy",
                        Some(marks[2 * index + 1]),
                        vec!["case".to_owned()],
                    )
                }))
                .map(|one| format!("{one:?}"))
                .collect();
            let mut found: Vec<String> = diagnostics(&output)
                .iter()
                .map(|one| format!("{:?}", summary(one)))
                .collect();
            // The order of the diagnostics is SACM23-XMI-030's; here only
            // what each affects matters.
            expected.sort();
            found.sort();
            assert_eq!(
                found, expected,
                "{ROW}: {what}: a diagnostic of a kept proxy does not affect its container"
            );
            assert_none_affects(&what, &output, "arts");
            let typed_found: Vec<Found> = diagnostics(&output)
                .into_iter()
                .filter(|one| one.0 == "SACM-XMI-010")
                .collect();
            let typed_expected: Vec<Found> = (0..holders)
                .map(|index| {
                    typed(
                        mode,
                        marks[2 * index + 1],
                        role,
                        class,
                        "AssuranceCasePackage",
                        "case",
                    )
                })
                .collect();
            assert_eq!(
                typed_found, typed_expected,
                "{ROW}: {what}: each SACM-XMI-010, its template and every argument"
            );
            assert_no_argument_names(&what, &output, "arts");
            assert_eq!(
                root_ids(output.document()),
                ["case", "arts"],
                "{ROW}: {what}: a root moved"
            );
        }
    }
}
