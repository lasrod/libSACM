//! Tests for `sacm::xmi::write`, the XMI writer (SACM23-XMI-003, SACM23-XMI-004,
//! SACM23-XMI-006, SACM23-XMI-007, SACM23-XMI-013 and LIB-API-010; ADR 0017).
//!
//! No element of the output carries `xmi:version` (SACM23-XMI-024): every
//! test that writes through `write_checked` checks it, and the row's own
//! tests are in `xmi_version.rs`.
//!
//! Documents are built with the `test-support` feature's `Document::insert`
//! (the crate dev-depends on itself with that feature) or read with
//! `xmi::read`, and written in tolerant mode ([`options`]): most of
//! them are nameless, which a strict write refuses (LIB-COMPAT-007), and these
//! tests check the bytes of the output form, not the save mode. The modes have
//! their own tests in `load_and_save_modes.rs`.
//!
//! Written output is checked in three ways:
//! - byte for byte, against an expected string or a golden file under
//!   `tests/data/sacm23/xmi/golden/`, where the bytes are the requirement
//!   (SACM23-XMI-007);
//! - through `parse_output`, a strict reader of the pinned output form and of
//!   nothing else (ADR 0017): the declaration, LF lines, two spaces per depth,
//!   one tag per line, ` />` for an element with no children and only for
//!   one, double-quoted attributes, and values that hold no raw `<`, `>` or
//!   TAB and no reference but the seven pinned escapes. Every test that uses it
//!   checks the form as well;
//! - read back through `xmi::read`, which reads `AssuranceCasePackage`,
//!   `ArgumentPackage` and `Claim` content (SACM23-XMI-016); output with
//!   other content may be refused only as `Unsupported`, never as
//!   `Malformed`.
//!
//! The role and type expected for each child come from `expected_tag`, which is
//! written from the containment roles in `docs/sacm/metamodel-inventory.md`,
//! not taken from the crate. A name is not a child element of the model: it
//! is checked apart, as a `name` child written first with `lang` then
//! `content` (SACM23-XMI-012; ADR 0017). Its own tests are in
//! `xmi_names.rs`.
//!
//! "Document order" for the refusals is `Document::elements`' order: pre-order,
//! roots and siblings in the order they were added (ADR 0017 W4).
//!
//! Not checked here:
//! - SACM23-XMI-003's "the reader and the writer share one constant per
//!   namespace URI": the constants are crate-private (ADR 0017 W5), so this is
//!   a source review item.
//! - SACM23-XMI-004's "no `xmi:id` is written on a `LangString` or
//!   `MultiLangString`": for a name, `assert_matches_document` checks it on
//!   every written document, and `xmi_names.rs` (SACM23-XMI-012) states it.
//!   No `MultiLangString` can be written yet (SACM23-BASE-004).
//! - The `#[non_exhaustive]` attributes of `WriteError`, its variants and
//!   `WriteOptions`: a test cannot show that an exhaustive pattern fails to
//!   compile. The patterns here use `..`, as a user must.

use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Debug, Display};

use sacm::diagnostic::Code;
use sacm::edit::{self, CreateArgumentPackage, CreateClaim};
use sacm::model::{
    AbstractClass, AssertionDeclaration, Document, ElementId, ElementKind, InsertError, LangString,
    SacmElement,
};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteAttribute, WriteError, WriteOptions};

/// The pinned SACM 2.3 namespace (D-NS-PIN).
const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
/// The XMI 2.5.1 namespace.
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";
/// Line 1 of every output.
const DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";
/// The attributes the document element carries before any `xmi:id`, as written.
const DOCUMENT_ATTRIBUTES: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);
/// Every attribute the writer may write today, in the order it must write them:
/// `assertionDeclaration` directly after `xmi:id` (ADR 0017). There
/// is no `xmi:version` (SACM23-XMI-024).
const ATTRIBUTE_ORDER: [&str; 5] = [
    "xmlns:sacm",
    "xmlns:xmi",
    "xmi:type",
    "xmi:id",
    "assertionDeclaration",
];
/// The element a name is written as (SACM23-XMI-012; ADR 0017).
const NAME_ELEMENT: &str = "name";
/// The attributes a `name` child may carry, in the order it must carry them.
const NAME_ATTRIBUTE_ORDER: [&str; 2] = ["lang", "content"];
/// The golden files of SACM23-XMI-007.
const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23/xmi/golden/");
/// The reader's valid fixtures (SACM23-XMI-001).
const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23/xmi/");

/// A wrapper whose `AssuranceCasePackage` and `ArgumentPackage` roots have ids
/// holding markup characters, TAB, LF and CR, non-ASCII characters, and ids
/// that are not `NCName`s. The ids it reads as are [`MARKED_UP_IDS`].
const MARKED_UP_ROOTS: &str = concat!(
    "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n",
    "  <sacm:AssuranceCasePackage xmi:id=\"amp&amp;lt&lt;gt&gt;quot&quot;apos'\"/>\n",
    "  <sacm:ArgumentPackage xmi:id=\"tab&#9;lf&#10;cr&#13;end\"/>\n",
    "  <sacm:AssuranceCasePackage xmi:id=\"raw>gt\"/>\n",
    "  <sacm:ArgumentPackage xmi:id='single\"quoted'/>\n",
    "  <sacm:AssuranceCasePackage xmi:id=\"\u{e9}\u{4e2d}\u{1F600} \u{fc}\"/>\n",
    "  <sacm:ArgumentPackage xmi:id=\"\u{7f}\u{85}\u{2028}\u{fffd}\u{10ffff}\"/>\n",
    "  <sacm:AssuranceCasePackage xmi:id=\" 1lead\"/>\n",
    "  <sacm:ArgumentPackage xmi:id=\"trail: \"/>\n",
    "  <sacm:AssuranceCasePackage xmi:id=\"1digit\"/>\n",
    "  <sacm:ArgumentPackage xmi:id=\"a:b\"/>\n",
    "</xmi:XMI>\n",
);
/// The (id, kind) of each root of [`MARKED_UP_ROOTS`], in order.
const MARKED_UP_IDS: [(&str, ElementKind); 10] = [
    ("amp&lt<gt>quot\"apos'", ElementKind::AssuranceCasePackage),
    ("tab\tlf\ncr\rend", ElementKind::ArgumentPackage),
    ("raw>gt", ElementKind::AssuranceCasePackage),
    ("single\"quoted", ElementKind::ArgumentPackage),
    (
        "\u{e9}\u{4e2d}\u{1F600} \u{fc}",
        ElementKind::AssuranceCasePackage,
    ),
    (
        "\u{7f}\u{85}\u{2028}\u{fffd}\u{10ffff}",
        ElementKind::ArgumentPackage,
    ),
    (" 1lead", ElementKind::AssuranceCasePackage),
    ("trail: ", ElementKind::ArgumentPackage),
    ("1digit", ElementKind::AssuranceCasePackage),
    ("a:b", ElementKind::ArgumentPackage),
];

// ---------------------------------------------------------------------------
// Building documents
// ---------------------------------------------------------------------------

/// One `insert` call with no name: the parent's id (`None` for a root), the
/// kind and the id.
type Step<'a> = (Option<&'a str>, ElementKind, &'a str);

/// Inserts one nameless element, or fails the test as a setup error.
fn insert(row: &str, doc: &mut Document, (parent, kind, element): Step<'_>) {
    let parent_id = parent.map(ElementId::from);
    if let Err(error) = doc.insert(parent_id.as_ref(), kind, ElementId::from(element), None) {
        panic!("{row}: setup could not insert the {kind} {element:?} under {parent:?}: {error}");
    }
}

/// Inserts one element with a name, or fails the test as a setup error.
fn insert_named(
    row: &str,
    doc: &mut Document,
    (parent, kind, element): Step<'_>,
    name: LangString,
) {
    let parent_id = parent.map(ElementId::from);
    if let Err(error) = doc.insert(
        parent_id.as_ref(),
        kind,
        ElementId::from(element),
        Some(name),
    ) {
        panic!("{row}: setup could not insert the named {kind} {element:?}: {error}");
    }
}

/// Sets an Assertion's declaration, or fails the test as a setup error.
fn declare(row: &str, doc: &mut Document, element: &str, declaration: AssertionDeclaration) {
    if let Err(error) = doc.set_assertion_declaration(&ElementId::from(element), declaration) {
        panic!("{row}: setup could not declare {element:?} {declaration}: {error}");
    }
}

/// A document built by `steps`, in order.
fn build(row: &str, steps: &[Step<'_>]) -> Document {
    let mut doc = Document::new();
    for &step in steps {
        insert(row, &mut doc, step);
    }
    doc
}

/// The twelve package kinds, in `ElementKind::ALL` order.
fn package_kinds() -> impl Iterator<Item = ElementKind> {
    ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_package())
}

/// The kinds whose class specializes `class`, in `ElementKind::ALL` order.
fn kinds_of(class: AbstractClass) -> Vec<ElementKind> {
    ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_a(class))
        .collect()
}

/// A nameless document with default declarations that holds every
/// `ElementKind`. Each of the twelve package kinds is a root. Under each root
/// is one child of every kind its roles admit, in `ElementKind::ALL` order;
/// under each `ModelElement` child, a Description; and under each
/// `ArtifactAsset` child, a Property that holds a Note.
fn every_kind_document(row: &str) -> Document {
    every_kind_document_named(row, false)
}

/// [`every_kind_document`], with every `ModelElement` named when `named` is
/// set: `lang` `"en"` and `content` `"Name of <id>"`. A `UtilityElement` has
/// no name.
fn every_kind_document_named(row: &str, named: bool) -> Document {
    let name_of =
        |id: &str| named.then(|| LangString::new(format!("Name of {id}")).with_lang("en"));
    let mut doc = Document::new();
    for root in package_kinds() {
        let root_id = format!("{root}.root");
        if let Err(error) = doc.insert(
            None,
            root,
            ElementId::from(root_id.as_str()),
            name_of(&root_id),
        ) {
            panic!("{row}: setup could not insert the root {root_id:?}: {error}");
        }
        for &kind in ElementKind::ALL {
            let child = format!("{root_id}/{kind}");
            let parent = ElementId::from(root_id.as_str());
            let name = if kind.is_a(AbstractClass::ModelElement) {
                name_of(&child)
            } else {
                None
            };
            match doc.insert(Some(&parent), kind, ElementId::from(child.as_str()), name) {
                Ok(()) => {}
                Err(InsertError::ContainmentNotAllowed { .. }) => continue,
                Err(error) => panic!("{row}: setup could not insert {child:?}: {error}"),
            }
            if kind.is_a(AbstractClass::ModelElement) {
                let description = format!("{child}/Description");
                insert(
                    row,
                    &mut doc,
                    (Some(child.as_str()), ElementKind::Description, &description),
                );
            }
            if kind.is_a(AbstractClass::ArtifactAsset) {
                let property = format!("{child}/Property");
                let note = format!("{property}/Note");
                if let Err(error) = doc.insert(
                    Some(&ElementId::from(child.as_str())),
                    ElementKind::Property,
                    ElementId::from(property.as_str()),
                    name_of(&property),
                ) {
                    panic!("{row}: setup could not insert {property:?}: {error}");
                }
                insert(
                    row,
                    &mut doc,
                    (Some(property.as_str()), ElementKind::Note, &note),
                );
            }
        }
    }
    let kinds: HashSet<ElementKind> = doc.elements().map(SacmElement::kind).collect();
    for kind in ElementKind::ALL {
        assert!(
            kinds.contains(kind),
            "{row}: setup: the every-kind document has no {kind}"
        );
    }
    doc
}

/// The document of the nested golden file: an AssuranceCasePackage holding an
/// ArgumentPackage (a Note, then a Claim that holds a Description), an
/// `ArgumentPackageInterface`, and an ArtifactPackage holding an Artifact that
/// holds a Property.
fn nested_document(row: &str) -> Document {
    build(
        row,
        &[
            (None, ElementKind::AssuranceCasePackage, "case"),
            (Some("case"), ElementKind::ArgumentPackage, "arguments"),
            (Some("arguments"), ElementKind::Note, "note_1"),
            (Some("arguments"), ElementKind::Claim, "claim_1"),
            (Some("claim_1"), ElementKind::Description, "description_1"),
            (
                Some("case"),
                ElementKind::ArgumentPackageInterface,
                "interface_1",
            ),
            (Some("case"), ElementKind::ArtifactPackage, "artifacts"),
            (Some("artifacts"), ElementKind::Artifact, "artifact_1"),
            (Some("artifact_1"), ElementKind::Property, "property_1"),
        ],
    )
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
fn projection(doc: &Document) -> (Vec<String>, Vec<Shown>) {
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

/// The (id, kind) of every element, in document order.
fn id_kinds(doc: &Document) -> Vec<(String, ElementKind)> {
    doc.elements()
        .map(|element| (element.id().as_str().to_owned(), element.kind()))
        .collect()
}

/// [`id_kinds`] of an expected list.
fn owned(expected: &[(&str, ElementKind)]) -> Vec<(String, ElementKind)> {
    expected
        .iter()
        .map(|&(id, kind)| (id.to_owned(), kind))
        .collect()
}

// ---------------------------------------------------------------------------
// Writing and reading
// ---------------------------------------------------------------------------

/// The options of every write here: tolerant, because most documents here are
/// nameless and a strict write refuses a validation Error (LIB-COMPAT-007).
/// The output form is the same in both modes.
fn options() -> WriteOptions {
    WriteOptions::default().with_mode(Mode::Tolerant)
}

/// Writes `doc` with [`options`].
fn write(doc: &Document) -> Result<String, WriteError> {
    xmi::write(doc, &options())
}

/// Writes `doc`, which the test expects to be written.
fn write_ok(row: &str, doc: &Document) -> String {
    match write(doc) {
        Ok(text) => text,
        Err(error) => panic!("{row}: write refused {doc:?}: {error} ({error:?})"),
    }
}

/// Writes `doc`, which the test expects to be refused, and returns why.
fn write_err(row: &str, doc: &Document) -> WriteError {
    match write(doc) {
        Ok(text) => panic!("{row}: write accepted {doc:?} and returned:\n{text}"),
        Err(error) => error,
    }
}

/// Reads `input`, which the test expects to be read.
fn read_ok(row: &str, what: &str, input: &[u8]) -> Document {
    match xmi::read(input, &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!(
            "{row}: {what} does not read: {error}\n{}",
            String::from_utf8_lossy(input)
        ),
    }
}

/// Asserts that the reader parses `text` as well-formed XML: it reads it, or
/// refuses it only as content it cannot represent yet.
fn assert_well_formed(row: &str, text: &str) {
    match xmi::read(text.as_bytes(), &ReadOptions::default()) {
        Ok(_) | Err(ReadError::Unsupported { .. }) => {}
        Err(error) => panic!("{row}: the reader refuses the written output: {error}\n{text}"),
    }
}

/// Reads the golden file `name`.
fn golden(row: &str, name: &str) -> Vec<u8> {
    let path = format!("{GOLDEN_DIR}{name}");
    std::fs::read(&path).unwrap_or_else(|error| panic!("{row}: cannot read {path}: {error}"))
}

/// Asserts that `text` is `expected` byte for byte, showing the first line
/// that differs.
fn assert_bytes(row: &str, what: &str, text: &str, expected: &[u8]) {
    if text.as_bytes() == expected {
        return;
    }
    let expected = String::from_utf8_lossy(expected);
    let mismatch = text
        .split('\n')
        .zip(expected.split('\n'))
        .enumerate()
        .find(|(_, (got, want))| got != want);
    match mismatch {
        Some((index, (got, want))) => panic!(
            "{row}: {what}: line {} differs\n   got: {got:?}\nwanted: {want:?}\n--- got ---\n{text}",
            index + 1
        ),
        None => panic!(
            "{row}: {what}: the lines agree but the lengths differ ({} bytes, wanted {})\n{text:?}",
            text.len(),
            expected.len()
        ),
    }
}

// ---------------------------------------------------------------------------
// The strict reader of the pinned output form
// ---------------------------------------------------------------------------

/// One element of written output, as [`parse_output`] read it.
#[derive(Debug)]
struct Written {
    /// The line it starts on, from 1.
    line: usize,
    /// The element's name as written: `prefix:Local`, or `Local`.
    name: String,
    /// The attributes in written order, with the values unescaped.
    attributes: Vec<(String, String)>,
    /// The index of the containing element; `None` for the document element.
    parent: Option<usize>,
    /// The indices of the contained elements, in written order.
    children: Vec<usize>,
}

impl Written {
    /// The unescaped value of the attribute `name`, if it is written.
    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(attribute, _)| attribute == name)
            .map(|(_, value)| value.as_str())
    }

    /// The prefix of the element's name, if it has one.
    fn prefix(&self) -> Option<&str> {
        self.name.split_once(':').map(|(prefix, _)| prefix)
    }
}

/// Reads written output in the pinned form, and fails the test on anything
/// outside it. Returns the elements in pre-order; the document element is
/// first.
fn parse_output(row: &str, text: &str) -> Vec<Written> {
    assert!(
        !text.starts_with('\u{FEFF}'),
        "{row}: the output starts with a BOM"
    );
    assert!(
        !text.contains('\r'),
        "{row}: the output holds a CR\n{text:?}"
    );
    let Some(body) = text
        .strip_prefix(DECLARATION)
        .and_then(|rest| rest.strip_prefix('\n'))
    else {
        panic!("{row}: line 1 is not {DECLARATION:?}\n{text}");
    };
    let Some(body) = body.strip_suffix('\n') else {
        panic!("{row}: the output does not end in LF\n{text:?}");
    };
    let mut elements: Vec<Written> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    // Whether the previous line was a start tag that is not self-closed.
    let mut just_opened = false;
    let mut done = false;
    for (index, line) in body.split('\n').enumerate() {
        let number = index + 2;
        assert!(
            !done,
            "{row}: line {number} follows the document element: {line:?}"
        );
        let tag = line.trim_start_matches(' ');
        let indent = line.len() - tag.len();
        if let Some(rest) = tag.strip_prefix("</") {
            let Some(name) = rest.strip_suffix('>') else {
                panic!("{row}: line {number} is not an end tag: {line:?}");
            };
            let Some(top) = open.pop() else {
                panic!("{row}: line {number} closes nothing: {line:?}");
            };
            assert_eq!(
                indent,
                2 * open.len(),
                "{row}: line {number} is indented {indent} spaces at depth {}: {line:?}",
                open.len()
            );
            assert_eq!(
                elements[top].name, name,
                "{row}: line {number} closes <{}>: {line:?}",
                elements[top].name
            );
            assert!(
                !just_opened,
                "{row}: line {number}: <{name}> has no children but is not written as <x … />"
            );
            just_opened = false;
            done = open.is_empty();
            continue;
        }
        let (empty, name, attributes) = parse_start_tag(row, number, line, open.len());
        let index = elements.len();
        let parent = open.last().copied();
        if let Some(parent) = parent {
            elements[parent].children.push(index);
        }
        elements.push(Written {
            line: number,
            name,
            attributes,
            parent,
            children: Vec::new(),
        });
        just_opened = !empty;
        if empty {
            done = open.is_empty();
        } else {
            open.push(index);
        }
    }
    assert!(
        open.is_empty(),
        "{row}: the output ends with <{}> open",
        elements[open[0]].name
    );
    elements
}

/// Reads line `number`, a start tag at `depth` open elements: whether it is
/// self-closed, the element name, and the attributes.
fn parse_start_tag(
    row: &str,
    number: usize,
    line: &str,
    depth: usize,
) -> (bool, String, Vec<(String, String)>) {
    let tag = line.trim_start_matches(' ');
    let indent = line.len() - tag.len();
    let Some(rest) = tag.strip_prefix('<') else {
        panic!("{row}: line {number} is not one tag: {line:?}");
    };
    assert_eq!(
        indent,
        2 * depth,
        "{row}: line {number} is indented {indent} spaces at depth {depth}: {line:?}"
    );
    let (empty, rest) = if let Some(rest) = rest.strip_suffix(" />") {
        (true, rest)
    } else if let Some(rest) = rest.strip_suffix('>') {
        (false, rest)
    } else {
        panic!("{row}: line {number} does not end in `>` or ` />`: {line:?}");
    };
    let (name, attributes) = rest
        .split_once(' ')
        .map_or((rest, ""), |(name, rest)| (name, rest));
    assert!(
        !name.is_empty() && !name.contains(['"', '=', '/', '<', '>', '&', '\t']),
        "{row}: line {number} has no element name: {line:?}"
    );
    let attributes = parse_attributes(row, number, line, attributes);
    (empty, name.to_owned(), attributes)
}

/// Reads the attributes of a start tag: `name="value"`, separated by one
/// space. `text` starts after the space that follows the element name.
fn parse_attributes(row: &str, number: usize, line: &str, text: &str) -> Vec<(String, String)> {
    let mut attributes = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let Some((name, after)) = rest.split_once("=\"") else {
            panic!("{row}: line {number} has an attribute that is not name=\"value\": {line:?}");
        };
        let Some((value, after)) = after.split_once('"') else {
            panic!("{row}: line {number} has an unterminated attribute value: {line:?}");
        };
        assert!(
            !name.is_empty() && !name.contains([' ', '"', '<', '>', '&']),
            "{row}: line {number} has a bad attribute name {name:?}: {line:?}"
        );
        assert!(
            attributes
                .iter()
                .all(|(seen, _): &(String, String)| seen != name),
            "{row}: line {number} writes {name} twice: {line:?}"
        );
        attributes.push((name.to_owned(), unescape(row, number, value)));
        rest = match after.strip_prefix(' ') {
            Some(next) => {
                assert!(
                    !next.is_empty(),
                    "{row}: line {number} has a trailing space: {line:?}"
                );
                next
            }
            None if after.is_empty() => after,
            None => {
                panic!("{row}: line {number}: attributes are not separated by one space: {line:?}")
            }
        };
    }
    attributes
}

/// Unescapes an attribute value written with the pinned escapes only.
fn unescape(row: &str, number: usize, value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(c) = rest.chars().next() {
        rest = &rest[c.len_utf8()..];
        match c {
            '&' => {
                let Some((reference, after)) = rest.split_once(';') else {
                    panic!("{row}: line {number}: a raw `&` in the value {value:?}");
                };
                out.push(match reference {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "#9" => '\t',
                    "#10" => '\n',
                    "#13" => '\r',
                    other => panic!(
                        "{row}: line {number}: &{other}; is not a pinned escape, in {value:?}"
                    ),
                });
                rest = after;
            }
            '<' | '>' | '\t' => panic!("{row}: line {number}: a raw {c:?} in the value {value:?}"),
            _ => out.push(c),
        }
    }
    out
}

/// The element written with the `xmi:id` `id`.
fn by_id<'a>(row: &str, elements: &'a [Written], id: &str) -> &'a Written {
    let mut found = elements
        .iter()
        .filter(|element| element.attribute("xmi:id") == Some(id));
    let Some(element) = found.next() else {
        panic!("{row}: no element is written with xmi:id {id:?}");
    };
    assert!(
        found.next().is_none(),
        "{row}: more than one element is written with xmi:id {id:?}"
    );
    element
}

/// The element name and `xmi:type` value that `kind` is written with, as a
/// root (`parent` is `None`) or as a child of a `parent` element, from the
/// containment roles of `docs/sacm/metamodel-inventory.md`.
fn expected_tag(
    row: &str,
    parent: Option<ElementKind>,
    kind: ElementKind,
) -> (String, Option<String>) {
    let typed = |role: &str| (role.to_owned(), Some(format!("sacm:{kind}")));
    let untyped = |role: &str| (role.to_owned(), None);
    let Some(parent) = parent else {
        return (format!("sacm:{kind}"), None);
    };
    match kind {
        ElementKind::Description => return untyped("description"),
        ElementKind::ImplementationConstraint => return untyped("implementationConstraint"),
        ElementKind::Note => return untyped("note"),
        ElementKind::TaggedValue => return untyped("taggedValue"),
        _ => {}
    }
    if parent.is_kind_of(ElementKind::AssuranceCasePackage) {
        for (package, role) in [
            (ElementKind::AssuranceCasePackage, "assuranceCasePackage"),
            (ElementKind::ArgumentPackage, "argumentPackage"),
            (ElementKind::ArtifactPackage, "artifactPackage"),
            (ElementKind::TerminologyPackage, "terminologyPackage"),
        ] {
            if kind == package {
                return untyped(role);
            }
            if kind.is_kind_of(package) {
                return typed(role);
            }
        }
    } else if parent.is_kind_of(ElementKind::ArgumentPackage) {
        return typed("argumentElement");
    } else if parent.is_kind_of(ElementKind::ArtifactPackage) {
        return typed("artifactElement");
    } else if parent.is_kind_of(ElementKind::TerminologyPackage) {
        return typed("terminologyElement");
    } else if parent.is_a(AbstractClass::ArtifactAsset) && kind == ElementKind::Property {
        return untyped("property");
    }
    panic!("{row}: test oracle: no role of {parent} holds {kind}");
}

/// The attributes a `name` child is written with for `name`: `lang`, then
/// `content`, each only when present (ADR 0012, ADR 0017).
fn name_attributes(name: &LangString) -> Vec<(String, String)> {
    [("lang", name.lang()), ("content", name.content())]
        .into_iter()
        .filter_map(|(attribute, value)| {
            value.map(|value| (attribute.to_owned(), value.to_owned()))
        })
        .collect()
}

/// Asserts that `written`, the output of `element`, holds its name as ADR
/// 0017 says: one `name` child, written first, with `lang` then
/// `content` and nothing else; no `name` child for no name. Returns whether
/// it has one, and its other children in written order.
fn assert_written_name(
    row: &str,
    what: &str,
    element: SacmElement<'_>,
    written: &Written,
    elements: &[Written],
) -> (bool, Vec<usize>) {
    let (name_children, model_children): (Vec<usize>, Vec<usize>) = written
        .children
        .iter()
        .partition(|&&child| elements[child].name == NAME_ELEMENT);
    let Some(name) = element.name() else {
        assert!(
            name_children.is_empty(),
            "{row}: {what} has no name but is written with a `name` child"
        );
        return (false, model_children);
    };
    assert_eq!(
        name_children.len(),
        1,
        "{row}: {what} is named but has {} `name` children",
        name_children.len()
    );
    assert_eq!(
        written.children.first(),
        name_children.first(),
        "{row}: {what}: the `name` child is not the first child"
    );
    let name_element = &elements[name_children[0]];
    assert_eq!(
        name_element.attributes,
        name_attributes(name),
        "{row}: {what}: the `name` child (line {}) has the wrong attributes",
        name_element.line
    );
    assert!(
        name_element.children.is_empty(),
        "{row}: {what}: the `name` child (line {}) has children",
        name_element.line
    );
    (true, model_children)
}

/// The `assertionDeclaration` value `element` is written with: its literal
/// when it is an Assertion not declared `asserted`, and none otherwise (ADR
/// 0017; XMI 2.5.1 §9.4.1).
fn written_declaration(element: SacmElement<'_>) -> Option<&'static str> {
    element
        .assertion_declaration()
        .filter(|&declaration| declaration != AssertionDeclaration::Asserted)
        .map(AssertionDeclaration::as_str)
}

/// Asserts that `elements`, the parsed output of writing `doc`, is `doc`: one
/// root bare or several as the children of one `xmi:XMI`, every element under
/// its expected name, type and id, its name as its first child exactly when
/// it has one, its `assertionDeclaration` exactly when it is not the default,
/// its other children in document order, and no element that the document
/// does not have.
fn assert_matches_document(row: &str, doc: &Document, elements: &[Written]) {
    let roots: Vec<SacmElement<'_>> = doc.roots().collect();
    let written_roots: Vec<usize> = if roots.len() == 1 {
        vec![0]
    } else {
        assert_eq!(
            elements[0].name,
            "xmi:XMI",
            "{row}: {} roots are not written in xmi:XMI",
            roots.len()
        );
        elements[0].children.clone()
    };
    let written_ids: Vec<Option<&str>> = written_roots
        .iter()
        .map(|&root| elements[root].attribute("xmi:id"))
        .collect();
    let root_ids: Vec<Option<&str>> = roots.iter().map(|root| Some(root.id().as_str())).collect();
    assert_eq!(
        written_ids, root_ids,
        "{row}: the roots are not written in roots() order"
    );
    let mut pending: Vec<(usize, SacmElement<'_>)> = written_roots
        .into_iter()
        .zip(roots.iter().copied())
        .rev()
        .collect();
    let mut seen = 0;
    let mut names = 0;
    while let Some((index, element)) = pending.pop() {
        seen += 1;
        let written = &elements[index];
        let what = format!(
            "the {} {:?} (line {})",
            element.kind(),
            element.id().as_str(),
            written.line
        );
        let (name, type_value) =
            expected_tag(row, element.parent().map(SacmElement::kind), element.kind());
        assert_eq!(
            written.name, name,
            "{row}: {what} is written under the wrong name"
        );
        assert_eq!(
            written.attribute("xmi:type"),
            type_value.as_deref(),
            "{row}: {what} has the wrong xmi:type"
        );
        assert_eq!(
            written.attribute("xmi:id"),
            Some(element.id().as_str()),
            "{row}: {what} has the wrong xmi:id"
        );
        assert_eq!(
            written.attribute("assertionDeclaration"),
            written_declaration(element),
            "{row}: {what} has the wrong assertionDeclaration"
        );
        let (has_name, model_children) =
            assert_written_name(row, &what, element, written, elements);
        names += usize::from(has_name);
        let children: Vec<SacmElement<'_>> = element.children().collect();
        let written_children: Vec<Option<&str>> = model_children
            .iter()
            .map(|&child| elements[child].attribute("xmi:id"))
            .collect();
        let child_ids: Vec<Option<&str>> = children
            .iter()
            .map(|child| Some(child.id().as_str()))
            .collect();
        assert_eq!(
            written_children, child_ids,
            "{row}: {what} has the wrong children, or in the wrong order"
        );
        pending.extend(model_children.iter().copied().zip(children).rev());
    }
    let wrapper = usize::from(roots.len() > 1);
    assert_eq!(
        seen + names + wrapper,
        elements.len(),
        "{row}: the output has elements that the document does not have"
    );
}

/// Asserts that the document element alone declares the namespaces, with
/// exactly the attributes it should have: the two declarations, then, on a
/// bare root, its `xmi:id`. No element carries `xmi:version`
/// (SACM23-XMI-024).
fn assert_document_element_attributes(row: &str, doc: &Document, elements: &[Written]) {
    let pinned = [("xmlns:sacm", SACM_NS), ("xmlns:xmi", XMI_NS)];
    let mut expected: Vec<(String, String)> = pinned
        .iter()
        .map(|&(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
    let mut roots = doc.roots();
    if let (Some(root), None) = (roots.next(), roots.next()) {
        expected.push(("xmi:id".to_owned(), root.id().as_str().to_owned()));
    }
    assert_eq!(
        elements[0].attributes, expected,
        "{row}: the document element <{}> has the wrong attributes",
        elements[0].name
    );
    for element in &elements[1..] {
        for (name, _) in &element.attributes {
            assert!(
                !name.starts_with("xmlns"),
                "{row}: <{}> on line {} carries {name}, which only the document element may",
                element.name,
                element.line
            );
        }
    }
    for element in elements {
        assert!(
            element.attribute("xmi:version").is_none(),
            "{row}: <{}> on line {} carries xmi:version, which nothing may (SACM23-XMI-024)",
            element.name,
            element.line
        );
    }
}

/// Asserts that every element's attributes are a subsequence of
/// [`ATTRIBUTE_ORDER`], or of [`NAME_ATTRIBUTE_ORDER`] for a `name` child:
/// no other attribute, and none out of order.
fn assert_attribute_order(row: &str, elements: &[Written]) {
    for element in elements {
        let order: &[&str] = if element.name == NAME_ELEMENT {
            &NAME_ATTRIBUTE_ORDER
        } else {
            &ATTRIBUTE_ORDER
        };
        let positions: Vec<Option<usize>> = element
            .attributes
            .iter()
            .map(|(name, _)| order.iter().position(|pinned| pinned == name))
            .collect();
        assert!(
            positions.iter().all(Option::is_some)
                && positions.windows(2).all(|pair| pair[0] < pair[1]),
            "{row}: <{}> on line {} has attributes {:?}; the pinned order is {order:?}",
            element.name,
            element.line,
            element
                .attributes
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>()
        );
    }
}

/// Writes `doc` and checks everything about the output that holds for every
/// document: the pinned form, the document element's attributes, the
/// attribute order, the elements against the document, and well-formedness.
fn write_checked(row: &str, doc: &Document) -> (String, Vec<Written>) {
    let text = write_ok(row, doc);
    let elements = parse_output(row, &text);
    assert_document_element_attributes(row, doc, &elements);
    assert_attribute_order(row, &elements);
    assert_matches_document(row, doc, &elements);
    assert_well_formed(row, &text);
    (text, elements)
}

/// The output for one bare root `<sacm:{kind}>` with the escaped id `id`.
fn bare_root(kind: ElementKind, escaped_id: &str) -> String {
    format!("{DECLARATION}\n<sacm:{kind}{DOCUMENT_ATTRIBUTES} xmi:id=\"{escaped_id}\" />\n")
}

/// Asserts that `error` refuses the element `id` because its id holds
/// `character`, names `WriteAttribute::Id` as the attribute that holds it (ADR 0017),
/// and says so.
fn assert_unrepresentable(row: &str, error: &WriteError, id: &str, character: char) {
    match error {
        WriteError::UnrepresentableCharacter {
            element,
            character: found,
            attribute,
            ..
        } => {
            assert_eq!(
                element.as_str(),
                id,
                "{row}: the refusal names the wrong element"
            );
            assert_eq!(
                *attribute,
                WriteAttribute::Id,
                "{row}: the refusal names the wrong attribute"
            );
            assert_eq!(
                *found, character,
                "{row}: the refusal names the wrong character"
            );
        }
        other => {
            panic!("{row}: {id:?} is refused as {other:?}, not as an unrepresentable character")
        }
    }
    assert_eq!(
        error.element().map(ElementId::as_str),
        Some(id),
        "{row}: element() names the wrong element"
    );
    let code: Option<Code> = error.code();
    assert_eq!(
        code, None,
        "{row}: an unrepresentable character has no catalog code"
    );
    assert_eq!(
        error.to_string(),
        format!(
            "the element {id:?} cannot be written: its xmi:id holds U+{:04X}, which XML 1.0 cannot represent",
            u32::from(character)
        ),
        "{row}: the refusal's message"
    );
}

/// Compiles only for a type that follows ADR 0011's error convention.
fn assert_error_type<T: Error + Debug + Display + Send + Sync + 'static>() {}

// ---------------------------------------------------------------------------
// SACM23-XMI-003: pinned strict namespace
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_003_one_root_declares_the_pinned_namespaces_on_the_root_only() {
    let row = "SACM23-XMI-003";
    let doc = build(
        row,
        &[
            (None, ElementKind::AssuranceCasePackage, "case"),
            (Some("case"), ElementKind::ArgumentPackage, "arguments"),
            (Some("arguments"), ElementKind::Claim, "c1"),
        ],
    );
    let (_, elements) = write_checked(row, &doc);
    let root = &elements[0];
    assert_eq!(
        root.name, "sacm:AssuranceCasePackage",
        "{row}: the document element"
    );
    let attributes: Vec<(&str, &str)> = root
        .attributes
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    assert_eq!(
        attributes,
        [
            ("xmlns:sacm", SACM_NS),
            ("xmlns:xmi", XMI_NS),
            ("xmi:id", "case")
        ],
        "{row}: the bare root's attributes, with no xmi:version (SACM23-XMI-024)"
    );
}

#[test]
fn sacm23_xmi_003_several_roots_declare_the_namespaces_on_xmi_xmi_only() {
    let row = "SACM23-XMI-003";
    let doc = build(
        row,
        &[
            (None, ElementKind::AssuranceCasePackage, "case"),
            (Some("case"), ElementKind::ArgumentPackage, "nested"),
            (None, ElementKind::ArgumentPackage, "arguments"),
            (Some("arguments"), ElementKind::Claim, "c1"),
        ],
    );
    let (_, elements) = write_checked(row, &doc);
    let wrapper = &elements[0];
    assert_eq!(wrapper.name, "xmi:XMI", "{row}: the document element");
    let attributes: Vec<(&str, &str)> = wrapper
        .attributes
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    assert_eq!(
        attributes,
        [("xmlns:sacm", SACM_NS), ("xmlns:xmi", XMI_NS)],
        "{row}: the xmi:XMI element's attributes, with no xmi:version (SACM23-XMI-024)"
    );
    for &root in &wrapper.children {
        let names: Vec<&str> = elements[root]
            .attributes
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(
            names,
            ["xmi:id"],
            "{row}: a root inside xmi:XMI carries only its xmi:id"
        );
    }
}

#[test]
fn sacm23_xmi_003_no_element_but_the_document_element_declares_a_namespace() {
    let row = "SACM23-XMI-003";
    for doc in [every_kind_document(row), nested_document(row)] {
        let (text, _) = write_checked(row, &doc);
        assert_eq!(
            text.matches("xmlns:sacm=").count(),
            1,
            "{row}: xmlns:sacm is declared more than once"
        );
        assert_eq!(
            text.matches("xmlns:xmi=").count(),
            1,
            "{row}: xmlns:xmi is declared more than once"
        );
        assert!(
            !text.contains("xmi:version"),
            "{row}: xmi:version is written (SACM23-XMI-024)"
        );
    }
}

#[test]
fn sacm23_xmi_003_output_has_no_default_namespace_no_xsi_and_only_the_sacm_and_xmi_prefixes() {
    let row = "SACM23-XMI-003";
    let single = build(row, &[(None, ElementKind::ArgumentPackage, "arguments")]);
    for doc in [single, nested_document(row), every_kind_document(row)] {
        let (text, elements) = write_checked(row, &doc);
        assert!(
            !text.contains("xmlns="),
            "{row}: the output declares a default namespace"
        );
        assert!(
            !text.contains("xmlns:xsi"),
            "{row}: the output declares xsi"
        );
        assert_eq!(
            text.matches("xmlns:").count(),
            2,
            "{row}: the output declares a namespace beyond sacm and xmi"
        );
        for element in &elements {
            assert!(
                matches!(element.prefix(), None | Some("sacm" | "xmi")),
                "{row}: <{}> on line {} has a prefix other than sacm and xmi",
                element.name,
                element.line
            );
            for (name, value) in &element.attributes {
                let prefix = name.split_once(':').map(|(prefix, _)| prefix);
                assert!(
                    matches!(prefix, Some("xmlns" | "xmi")),
                    "{row}: the attribute {name} on line {} is not in the xmi namespace",
                    element.line
                );
                if name == "xmi:type" {
                    assert!(
                        value.starts_with("sacm:"),
                        "{row}: the type value {value:?} is not sacm-prefixed"
                    );
                }
            }
        }
    }
}

#[test]
fn sacm23_xmi_003_roots_are_sacm_prefixed_by_kind_and_children_are_unprefixed() {
    let row = "SACM23-XMI-003";
    let doc = every_kind_document(row);
    let (_, elements) = write_checked(row, &doc);
    let roots: Vec<String> = elements[0]
        .children
        .iter()
        .map(|&root| elements[root].name.clone())
        .collect();
    let expected: Vec<String> = package_kinds().map(|kind| format!("sacm:{kind}")).collect();
    assert_eq!(roots, expected, "{row}: the roots' element names");
    for element in elements
        .iter()
        .filter(|element| element.parent.is_some_and(|parent| parent != 0))
    {
        assert_eq!(
            element.prefix(),
            None,
            "{row}: the child <{}> on line {} is prefixed",
            element.name,
            element.line
        );
    }
}

#[test]
fn sacm23_xmi_003_one_and_two_roots_read_back_to_the_same_ids_kinds_and_order() {
    let row = "SACM23-XMI-003";
    let acp = (None, ElementKind::AssuranceCasePackage, "case");
    let ap = (None, ElementKind::ArgumentPackage, "arguments");
    for steps in [vec![acp], vec![ap], vec![acp, ap], vec![ap, acp]] {
        let doc = build(row, &steps);
        let (text, _) = write_checked(row, &doc);
        let back = read_ok(row, "the written output", text.as_bytes());
        assert_eq!(
            id_kinds(&back),
            id_kinds(&doc),
            "{row}: the output reads back differently\n{text}"
        );
    }
}

// ---------------------------------------------------------------------------
// SACM23-XMI-004: xmi:id identity
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_004_every_element_of_every_kind_has_exactly_one_xmi_id_equal_to_its_id() {
    let row = "SACM23-XMI-004";
    let doc = every_kind_document(row);
    let (text, elements) = write_checked(row, &doc);
    let start = usize::from(doc.roots().count() > 1);
    for element in &elements[start..] {
        let ids = element
            .attributes
            .iter()
            .filter(|(name, _)| name == "xmi:id")
            .count();
        assert_eq!(
            ids, 1,
            "{row}: <{}> on line {} has {ids} xmi:id attributes",
            element.name, element.line
        );
    }
    let written: HashSet<&str> = elements[start..]
        .iter()
        .filter_map(|element| element.attribute("xmi:id"))
        .collect();
    let expected: HashSet<&str> = doc
        .elements()
        .map(|element| element.id().as_str())
        .collect();
    assert_eq!(
        written, expected,
        "{row}: the written ids are not the document's ids"
    );
    assert_eq!(
        text.matches(" xmi:id=\"").count(),
        doc.element_count(),
        "{row}: the output has an xmi:id for an element the document does not have"
    );
}

#[test]
fn sacm23_xmi_004_read_write_read_keeps_root_ids_with_markup_line_breaks_non_ascii_and_non_ncnames()
{
    let row = "SACM23-XMI-004";
    let doc = read_ok(
        row,
        "setup: the marked-up roots",
        MARKED_UP_ROOTS.as_bytes(),
    );
    assert_eq!(
        id_kinds(&doc),
        owned(&MARKED_UP_IDS),
        "{row}: setup: the input reads as other ids"
    );
    let (text, _) = write_checked(row, &doc);
    let back = read_ok(row, "the written output", text.as_bytes());
    assert_eq!(
        id_kinds(&back),
        owned(&MARKED_UP_IDS),
        "{row}: the ids do not survive a write\n{text}"
    );
}

#[test]
fn sacm23_xmi_004_id_characters_are_escaped_with_the_pinned_references_and_nothing_else() {
    let row = "SACM23-XMI-004";
    let id = "a&b<c>d\"e'f\tg\nh\ri \u{e9}\u{1F600}";
    let escaped = "a&amp;b&lt;c&gt;d&quot;e'f&#9;g&#10;h&#13;i \u{e9}\u{1F600}";
    let root = build(row, &[(None, ElementKind::ArgumentPackage, id)]);
    assert_bytes(
        row,
        "a root id",
        &write_ok(row, &root),
        bare_root(ElementKind::ArgumentPackage, escaped).as_bytes(),
    );

    let nested = build(
        row,
        &[
            (None, ElementKind::ArgumentPackage, "arguments"),
            (Some("arguments"), ElementKind::Claim, id),
        ],
    );
    let expected = format!(
        "{DECLARATION}\n<sacm:ArgumentPackage{DOCUMENT_ATTRIBUTES} xmi:id=\"arguments\">\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{escaped}\" />\n</sacm:ArgumentPackage>\n"
    );
    assert_bytes(
        row,
        "a child id",
        &write_ok(row, &nested),
        expected.as_bytes(),
    );
}

#[test]
fn sacm23_xmi_004_an_id_holding_u0001_is_refused_naming_the_element_and_the_character() {
    let row = "SACM23-XMI-004";
    let doc = build(row, &[(None, ElementKind::ArgumentPackage, "a\u{1}")]);
    let error = write_err(row, &doc);
    assert_unrepresentable(row, &error, "a\u{1}", '\u{1}');
    assert_eq!(
        error.to_string(),
        "the element \"a\\u{1}\" cannot be written: its xmi:id holds U+0001, which XML 1.0 cannot represent",
        "{row}: the pinned message"
    );
}

#[test]
fn sacm23_xmi_004_an_id_holding_ufffe_is_refused_naming_the_element_and_the_character() {
    let row = "SACM23-XMI-004";
    let doc = build(
        row,
        &[
            (None, ElementKind::ArgumentPackage, "arguments"),
            (Some("arguments"), ElementKind::Claim, "c\u{FFFE}"),
        ],
    );
    let error = write_err(row, &doc);
    assert_unrepresentable(row, &error, "c\u{FFFE}", '\u{FFFE}');
    assert_eq!(
        error.to_string(),
        "the element \"c\\u{fffe}\" cannot be written: its xmi:id holds U+FFFE, which XML 1.0 cannot represent",
        "{row}: the message quotes the id as Debug does"
    );
}

#[test]
fn sacm23_xmi_004_every_character_outside_xml_char_in_an_id_is_refused() {
    let row = "SACM23-XMI-004";
    let controls = (0x00..=0x1F_u32).filter(|&c| !matches!(c, 0x09 | 0x0A | 0x0D));
    for character in controls.chain([0xFFFE, 0xFFFF]).filter_map(char::from_u32) {
        for id in [
            format!("{character}"),
            format!("x{character}y"),
            format!("xy{character}"),
        ] {
            let doc = build(row, &[(None, ElementKind::AssuranceCasePackage, &id)]);
            assert_unrepresentable(row, &write_err(row, &doc), &id, character);
        }
    }
}

#[test]
fn sacm23_xmi_004_an_id_with_two_unrepresentable_characters_names_the_first() {
    let row = "SACM23-XMI-004";
    for (id, first) in [
        ("a\u{1}b\u{FFFE}", '\u{1}'),
        ("a\u{FFFE}b\u{1}", '\u{FFFE}'),
    ] {
        let doc = build(row, &[(None, ElementKind::ArgumentPackage, id)]);
        assert_unrepresentable(row, &write_err(row, &doc), id, first);
    }
}

#[test]
fn sacm23_xmi_004_the_xml_char_boundary_characters_in_an_id_are_written_and_read_back() {
    let row = "SACM23-XMI-004";
    let allowed = [
        '\t',
        '\n',
        '\r',
        ' ',
        '\u{7F}',
        '\u{85}',
        '\u{D7FF}',
        '\u{E000}',
        '\u{FFFD}',
        '\u{10000}',
        '\u{10FFFF}',
    ];
    for character in allowed {
        let id = format!("x{character}y");
        let doc = build(row, &[(None, ElementKind::ArgumentPackage, &id)]);
        let (text, _) = write_checked(row, &doc);
        let back = read_ok(row, "the written output", text.as_bytes());
        assert_eq!(
            id_kinds(&back),
            id_kinds(&doc),
            "{row}: the id with U+{:04X} does not read back",
            u32::from(character)
        );
    }
}

#[test]
fn sacm23_xmi_004_writing_takes_a_shared_reference_and_leaves_the_document_unchanged() {
    let row = "SACM23-XMI-004";
    let signature: fn(&Document, &WriteOptions) -> Result<String, WriteError> = xmi::write;
    let mut named = nested_document(row);
    insert_named(
        row,
        &mut named,
        (Some("arguments"), ElementKind::Claim, "named"),
        LangString::new("Top").with_lang("en"),
    );
    let mut declared = nested_document(row);
    declare(row, &mut declared, "claim_1", AssertionDeclaration::Assumed);
    let bad_id = build(
        row,
        &[
            (None, ElementKind::ArgumentPackage, "arguments"),
            (Some("arguments"), ElementKind::Claim, "c\u{1}"),
        ],
    );
    for doc in [
        every_kind_document(row),
        nested_document(row),
        named,
        declared,
        bad_id,
    ] {
        let before = projection(&doc);
        let first = signature(&doc, &options());
        assert_eq!(
            projection(&doc),
            before,
            "{row}: writing changed the document"
        );
        let second = signature(&doc, &options());
        assert_eq!(
            projection(&doc),
            before,
            "{row}: writing again changed the document"
        );
        assert_eq!(
            format!("{first:?}"),
            format!("{second:?}"),
            "{row}: writing the same document twice gave different results"
        );
    }
}

// ---------------------------------------------------------------------------
// SACM23-XMI-006: type attribute where the class is not known from the role
// ---------------------------------------------------------------------------

/// Writes one `package` root holding one child of every kind of `class`, and
/// asserts that each is written as `<{role} xmi:type="sacm:<Kind>" …>`.
fn assert_every_kind_typed(row: &str, package: ElementKind, class: AbstractClass, role: &str) {
    let kinds = kinds_of(class);
    assert!(
        kinds.len() >= 4,
        "{row}: setup: {class:?} has only {} kinds",
        kinds.len()
    );
    let mut doc = Document::new();
    insert(row, &mut doc, (None, package, "package"));
    for kind in &kinds {
        insert(row, &mut doc, (Some("package"), *kind, kind.name()));
    }
    let (text, elements) = write_checked(row, &doc);
    let lines: Vec<&str> = text.lines().collect();
    for kind in kinds {
        let written = by_id(row, &elements, kind.name());
        assert_eq!(
            written.name, role,
            "{row}: the {kind} in a {package} is not written under {role}"
        );
        let expected = format!("sacm:{kind}");
        assert_eq!(
            written.attribute("xmi:type"),
            Some(expected.as_str()),
            "{row}: the {kind} in a {package} has the wrong type"
        );
        let line = format!("  <{role} xmi:type=\"sacm:{kind}\" xmi:id=\"{kind}\" />");
        assert!(
            lines.contains(&line.as_str()),
            "{row}: the output has no line {line:?}\n{text}"
        );
    }
}

#[test]
fn sacm23_xmi_006_every_argumentation_element_kind_under_an_argument_package_is_typed() {
    assert_every_kind_typed(
        "SACM23-XMI-006",
        ElementKind::ArgumentPackage,
        AbstractClass::ArgumentationElement,
        "argumentElement",
    );
}

#[test]
fn sacm23_xmi_006_every_artifact_element_kind_under_an_artifact_package_is_typed() {
    assert_every_kind_typed(
        "SACM23-XMI-006",
        ElementKind::ArtifactPackage,
        AbstractClass::ArtifactElement,
        "artifactElement",
    );
}

#[test]
fn sacm23_xmi_006_every_terminology_element_kind_under_a_terminology_package_is_typed() {
    assert_every_kind_typed(
        "SACM23-XMI-006",
        ElementKind::TerminologyPackage,
        AbstractClass::TerminologyElement,
        "terminologyElement",
    );
}

#[test]
fn sacm23_xmi_006_in_the_four_package_roles_only_interfaces_and_bindings_are_typed() {
    let row = "SACM23-XMI-006";
    let mut doc = Document::new();
    insert(
        row,
        &mut doc,
        (None, ElementKind::AssuranceCasePackage, "case"),
    );
    for kind in package_kinds() {
        insert(row, &mut doc, (Some("case"), kind, kind.name()));
    }
    let expected = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<sacm:AssuranceCasePackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
        " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"case\">\n",
        "  <assuranceCasePackage xmi:id=\"AssuranceCasePackage\" />\n",
        "  <assuranceCasePackage xmi:type=\"sacm:AssuranceCasePackageInterface\" xmi:id=\"AssuranceCasePackageInterface\" />\n",
        "  <assuranceCasePackage xmi:type=\"sacm:AssuranceCasePackageBinding\" xmi:id=\"AssuranceCasePackageBinding\" />\n",
        "  <terminologyPackage xmi:id=\"TerminologyPackage\" />\n",
        "  <terminologyPackage xmi:type=\"sacm:TerminologyPackageInterface\" xmi:id=\"TerminologyPackageInterface\" />\n",
        "  <terminologyPackage xmi:type=\"sacm:TerminologyPackageBinding\" xmi:id=\"TerminologyPackageBinding\" />\n",
        "  <argumentPackage xmi:id=\"ArgumentPackage\" />\n",
        "  <argumentPackage xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"ArgumentPackageInterface\" />\n",
        "  <argumentPackage xmi:type=\"sacm:ArgumentPackageBinding\" xmi:id=\"ArgumentPackageBinding\" />\n",
        "  <artifactPackage xmi:id=\"ArtifactPackage\" />\n",
        "  <artifactPackage xmi:type=\"sacm:ArtifactPackageInterface\" xmi:id=\"ArtifactPackageInterface\" />\n",
        "  <artifactPackage xmi:type=\"sacm:ArtifactPackageBinding\" xmi:id=\"ArtifactPackageBinding\" />\n",
        "</sacm:AssuranceCasePackage>\n",
    );
    let (text, _) = write_checked(row, &doc);
    assert_bytes(
        row,
        "the twelve packages in their roles",
        &text,
        expected.as_bytes(),
    );
}

#[test]
fn sacm23_xmi_006_utility_property_and_root_elements_have_no_type_attribute() {
    let row = "SACM23-XMI-006";
    let doc = every_kind_document(row);
    let (_, elements) = write_checked(row, &doc);
    let untyped_roles = [
        "description",
        "implementationConstraint",
        "note",
        "taggedValue",
        "property",
    ];
    for role in untyped_roles {
        let count = elements
            .iter()
            .filter(|element| element.name == role)
            .count();
        assert!(count > 0, "{row}: setup: nothing is written under {role}");
    }
    for element in &elements {
        let is_root = element.prefix() == Some("sacm");
        if is_root || untyped_roles.contains(&element.name.as_str()) {
            assert_eq!(
                element.attribute("xmi:type"),
                None,
                "{row}: <{}> on line {} has a type attribute",
                element.name,
                element.line
            );
        }
    }
    for kind in package_kinds() {
        let single = build(row, &[(None, kind, "root")]);
        let (_, elements) = write_checked(row, &single);
        assert_eq!(
            elements[0].attribute("xmi:type"),
            None,
            "{row}: the bare {kind} root has a type attribute"
        );
    }
}

#[test]
fn sacm23_xmi_006_output_never_contains_xsi() {
    let row = "SACM23-XMI-006";
    let mut docs = vec![every_kind_document(row), nested_document(row)];
    docs.extend(package_kinds().map(|kind| build(row, &[(None, kind, "root")])));
    for doc in docs {
        let text = write_ok(row, &doc);
        assert!(
            !text.contains("xsi:"),
            "{row}: the output has an xsi: name\n{text}"
        );
        assert!(
            !text.contains("xmlns:xsi"),
            "{row}: the output declares xsi\n{text}"
        );
        assert!(
            !text.contains("XMLSchema-instance"),
            "{row}: the output names the XSI namespace\n{text}"
        );
    }
}

#[test]
fn sacm23_xmi_006_every_child_has_the_role_and_type_its_parent_class_and_kind_give() {
    let row = "SACM23-XMI-006";
    // `write_checked` compares every element with `expected_tag`, under
    // interface and binding parents as well as plain packages.
    let doc = every_kind_document(row);
    let (_, elements) = write_checked(row, &doc);
    let typed = elements
        .iter()
        .filter(|element| element.attribute("xmi:type").is_some())
        .count();
    assert!(typed > 100, "{row}: setup: only {typed} typed children");
}

// ---------------------------------------------------------------------------
// SACM23-XMI-007: deterministic export
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_007_writing_a_document_twice_or_its_clone_gives_identical_bytes() {
    let row = "SACM23-XMI-007";
    for doc in [every_kind_document(row), nested_document(row)] {
        let first = write_ok(row, &doc);
        assert_bytes(
            row,
            "a second write",
            &write_ok(row, &doc),
            first.as_bytes(),
        );
        let clone = doc.clone();
        assert_bytes(
            row,
            "the clone's write",
            &write_ok(row, &clone),
            first.as_bytes(),
        );
    }
}

#[test]
fn sacm23_xmi_007_two_documents_built_by_the_same_inserts_write_identical_bytes() {
    let row = "SACM23-XMI-007";
    let first = every_kind_document(row);
    let second = every_kind_document(row);
    assert!(
        first.element_count() >= 50,
        "{row}: setup: only {} elements",
        first.element_count()
    );
    assert_bytes(
        row,
        "the second document",
        &write_ok(row, &second),
        write_ok(row, &first).as_bytes(),
    );
}

#[test]
fn sacm23_xmi_007_one_argument_package_matches_the_single_root_golden_file() {
    let row = "SACM23-XMI-007";
    let doc = build(row, &[(None, ElementKind::ArgumentPackage, "arguments")]);
    let (text, _) = write_checked(row, &doc);
    assert_bytes(
        row,
        "single-root.golden.sacm.xmi",
        &text,
        &golden(row, "single-root.golden.sacm.xmi"),
    );
}

#[test]
fn sacm23_xmi_007_three_roots_match_the_several_roots_golden_file() {
    let row = "SACM23-XMI-007";
    let doc = build(
        row,
        &[
            (None, ElementKind::ArgumentPackage, "arguments_1"),
            (None, ElementKind::AssuranceCasePackage, "case"),
            (None, ElementKind::ArgumentPackage, "arguments_2"),
        ],
    );
    let (text, _) = write_checked(row, &doc);
    assert_bytes(
        row,
        "several-roots.golden.sacm.xmi",
        &text,
        &golden(row, "several-roots.golden.sacm.xmi"),
    );
}

#[test]
fn sacm23_xmi_007_a_nested_assurance_case_package_matches_the_nested_golden_file() {
    let row = "SACM23-XMI-007";
    let (text, _) = write_checked(row, &nested_document(row));
    assert_bytes(
        row,
        "nested.golden.sacm.xmi",
        &text,
        &golden(row, "nested.golden.sacm.xmi"),
    );
}

#[test]
fn sacm23_xmi_007_output_is_lf_only_without_a_bom_starts_with_the_declaration_and_ends_in_one_lf() {
    let row = "SACM23-XMI-007";
    let single = build(row, &[(None, ElementKind::ArgumentPackage, "arguments")]);
    let several = build(
        row,
        &[
            (None, ElementKind::ArgumentPackage, "a"),
            (None, ElementKind::AssuranceCasePackage, "b"),
        ],
    );
    for doc in [
        single,
        several,
        nested_document(row),
        every_kind_document(row),
    ] {
        let text = write_ok(row, &doc);
        let bytes = text.as_bytes();
        assert!(
            !bytes.starts_with(&[0xEF, 0xBB, 0xBF]),
            "{row}: the output starts with a BOM"
        );
        assert!(!bytes.contains(&b'\r'), "{row}: the output holds a CR byte");
        assert_eq!(
            text.split('\n').next(),
            Some(DECLARATION),
            "{row}: line 1 is not the declaration"
        );
        assert!(
            text.ends_with('\n') && !text.ends_with("\n\n"),
            "{row}: the output does not end in exactly one LF"
        );
    }
}

#[test]
fn sacm23_xmi_007_lines_are_indented_two_spaces_per_depth_and_childless_elements_self_close() {
    let row = "SACM23-XMI-007";
    // A chain of Properties, eight deep, under an Artifact.
    let mut doc = build(
        row,
        &[
            (None, ElementKind::ArtifactPackage, "p"),
            (Some("p"), ElementKind::Artifact, "a"),
        ],
    );
    let mut parent = "a".to_owned();
    for depth in 1..=8 {
        let id = format!("p{depth}");
        insert(
            row,
            &mut doc,
            (Some(parent.as_str()), ElementKind::Property, &id),
        );
        parent = id;
    }
    let mut lines = vec![
        DECLARATION.to_owned(),
        format!("<sacm:ArtifactPackage{DOCUMENT_ATTRIBUTES} xmi:id=\"p\">"),
        "  <artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"a\">".to_owned(),
    ];
    for depth in 1..=8 {
        let indent = "  ".repeat(depth + 1);
        let end = if depth == 8 { " />" } else { ">" };
        lines.push(format!("{indent}<property xmi:id=\"p{depth}\"{end}"));
    }
    for depth in (1..8).rev() {
        lines.push(format!("{}</property>", "  ".repeat(depth + 1)));
    }
    lines.push("  </artifactElement>".to_owned());
    lines.push("</sacm:ArtifactPackage>".to_owned());
    let expected = lines.join("\n") + "\n";
    let (text, _) = write_checked(row, &doc);
    assert_bytes(row, "a deep chain", &text, expected.as_bytes());
    // `parse_output` enforces both rules on every line of the large document.
    write_checked(row, &every_kind_document(row));
}

#[test]
fn sacm23_xmi_007_attributes_are_written_in_the_pinned_order() {
    let row = "SACM23-XMI-007";
    for doc in [
        every_kind_document(row),
        nested_document(row),
        build(row, &[(None, ElementKind::TerminologyPackage, "t")]),
    ] {
        let text = write_ok(row, &doc);
        assert_attribute_order(row, &parse_output(row, &text));
    }
}

#[test]
fn sacm23_xmi_007_children_are_written_in_model_order_across_roles() {
    let row = "SACM23-XMI-007";
    let in_order = build(
        row,
        &[
            (None, ElementKind::ArgumentPackage, "arguments"),
            (Some("arguments"), ElementKind::Note, "n"),
            (Some("arguments"), ElementKind::Claim, "c"),
            (Some("arguments"), ElementKind::Description, "d"),
        ],
    );
    let expected = format!(
        "{DECLARATION}\n<sacm:ArgumentPackage{DOCUMENT_ATTRIBUTES} xmi:id=\"arguments\">\n\
         \x20 <note xmi:id=\"n\" />\n\
         \x20 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\" />\n\
         \x20 <description xmi:id=\"d\" />\n\
         </sacm:ArgumentPackage>\n"
    );
    assert_bytes(
        row,
        "Note, Claim, Description",
        &write_ok(row, &in_order),
        expected.as_bytes(),
    );

    let reversed = build(
        row,
        &[
            (None, ElementKind::ArgumentPackage, "arguments"),
            (Some("arguments"), ElementKind::Description, "d"),
            (Some("arguments"), ElementKind::Claim, "c"),
            (Some("arguments"), ElementKind::Note, "n"),
        ],
    );
    let expected = format!(
        "{DECLARATION}\n<sacm:ArgumentPackage{DOCUMENT_ATTRIBUTES} xmi:id=\"arguments\">\n\
         \x20 <description xmi:id=\"d\" />\n\
         \x20 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\" />\n\
         \x20 <note xmi:id=\"n\" />\n\
         </sacm:ArgumentPackage>\n"
    );
    assert_bytes(
        row,
        "Description, Claim, Note",
        &write_ok(row, &reversed),
        expected.as_bytes(),
    );
}

#[test]
fn sacm23_xmi_007_reading_a_golden_file_and_writing_it_gives_the_golden_bytes() {
    let row = "SACM23-XMI-007";
    for name in [
        "single-root.golden.sacm.xmi",
        "several-roots.golden.sacm.xmi",
    ] {
        let bytes = golden(row, name);
        let doc = read_ok(row, name, &bytes);
        assert_bytes(row, name, &write_ok(row, &doc), &bytes);
    }
}

// ---------------------------------------------------------------------------
// SACM23-XMI-013: export of one or more roots
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_013_one_root_is_the_document_element_with_no_xmi_element() {
    let row = "SACM23-XMI-013";
    for doc in [
        nested_document(row),
        build(row, &[(None, ElementKind::AssuranceCasePackage, "case")]),
    ] {
        let (text, elements) = write_checked(row, &doc);
        assert_eq!(
            elements[0].name, "sacm:AssuranceCasePackage",
            "{row}: the document element"
        );
        assert!(
            !text.contains("xmi:XMI"),
            "{row}: a single root is written in an xmi:XMI element\n{text}"
        );
    }
}

#[test]
fn sacm23_xmi_013_several_roots_are_the_children_of_one_xmi_element_in_document_order() {
    let row = "SACM23-XMI-013";
    let doc = build(
        row,
        &[
            (None, ElementKind::ArgumentPackage, "first"),
            (None, ElementKind::AssuranceCasePackage, "second"),
            (Some("first"), ElementKind::Claim, "claim"),
            (Some("second"), ElementKind::ArgumentPackage, "inner"),
            (None, ElementKind::ArgumentPackage, "third"),
        ],
    );
    let (text, elements) = write_checked(row, &doc);
    assert_eq!(
        text.matches("<xmi:XMI").count(),
        1,
        "{row}: not exactly one xmi:XMI start tag"
    );
    assert_eq!(
        text.matches("</xmi:XMI>").count(),
        1,
        "{row}: not exactly one xmi:XMI end tag"
    );
    let roots: Vec<(&str, Option<&str>)> = elements[0]
        .children
        .iter()
        .map(|&root| {
            (
                elements[root].name.as_str(),
                elements[root].attribute("xmi:id"),
            )
        })
        .collect();
    assert_eq!(
        roots,
        [
            ("sacm:ArgumentPackage", Some("first")),
            ("sacm:AssuranceCasePackage", Some("second")),
            ("sacm:ArgumentPackage", Some("third")),
        ],
        "{row}: the xmi:XMI element's children"
    );

    let two = build(
        row,
        &[
            (None, ElementKind::AssuranceCasePackage, "case"),
            (None, ElementKind::ArgumentPackage, "arguments"),
        ],
    );
    let expected = format!(
        "{DECLARATION}\n<xmi:XMI{DOCUMENT_ATTRIBUTES}>\n\
         \x20 <sacm:AssuranceCasePackage xmi:id=\"case\" />\n\
         \x20 <sacm:ArgumentPackage xmi:id=\"arguments\" />\n\
         </xmi:XMI>\n"
    );
    assert_bytes(row, "two roots", &write_ok(row, &two), expected.as_bytes());
}

#[test]
fn sacm23_xmi_013_each_package_kind_is_written_as_a_single_root_under_its_class_name() {
    let row = "SACM23-XMI-013";
    let mut count = 0;
    for kind in package_kinds() {
        let doc = build(row, &[(None, kind, "root")]);
        let (text, _) = write_checked(row, &doc);
        assert_bytes(
            row,
            &format!("a bare {kind}"),
            &text,
            bare_root(kind, "root").as_bytes(),
        );
        count += 1;
    }
    assert_eq!(
        count, 12,
        "{row}: setup: there are not twelve package kinds"
    );
}

#[test]
fn sacm23_xmi_013_an_empty_document_is_refused_with_no_root() {
    let row = "SACM23-XMI-013";
    let error = write_err(row, &Document::new());
    assert!(
        matches!(error, WriteError::NoRoot),
        "{row}: an empty document is refused as {error:?}"
    );
    assert_eq!(error.element(), None, "{row}: NoRoot names no element");
    let code: Option<Code> = error.code();
    assert_eq!(code, None, "{row}: NoRoot has no catalog code");
    assert_eq!(
        error.to_string(),
        "the document has no root to write",
        "{row}: the pinned message"
    );
}

#[test]
fn sacm23_xmi_013_one_and_three_roots_read_back_to_the_same_ids_kinds_and_order() {
    let row = "SACM23-XMI-013";
    let acp = |id| (None, ElementKind::AssuranceCasePackage, id);
    let ap = |id| (None, ElementKind::ArgumentPackage, id);
    for steps in [
        vec![acp("case")],
        vec![ap("arguments")],
        vec![ap("a1"), acp("c"), ap("a2")],
        vec![acp("c1"), acp("c2"), ap("a")],
    ] {
        let doc = build(row, &steps);
        let (text, _) = write_checked(row, &doc);
        let back = read_ok(row, "the written output", text.as_bytes());
        assert_eq!(
            id_kinds(&back),
            id_kinds(&doc),
            "{row}: the output reads back differently\n{text}"
        );
    }
}

/// The fixed inputs of the round-trip checks: every valid fixture, the two
/// golden files the reader can read, and inline inputs in other forms that it
/// reads (other prefixes, `xsi:type`, a default namespace, comments and
/// processing instructions, UTF-16, and the marked-up ids).
fn readable_inputs(row: &str) -> Vec<(String, Vec<u8>)> {
    let mut inputs = Vec::new();
    for name in [
        "argument-package-root-valid.sacm.xmi",
        "assurance-case-package-root-valid.sacm.xmi",
        "wrapped-roots-valid.sacm.xmi",
    ] {
        let path = format!("{FIXTURE_DIR}{name}");
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("{row}: cannot read {path}: {error}"));
        inputs.push((name.to_owned(), bytes));
    }
    for name in [
        "single-root.golden.sacm.xmi",
        "several-roots.golden.sacm.xmi",
    ] {
        inputs.push((name.to_owned(), golden(row, name)));
    }
    let inline: [(&str, &str); 5] = [
        (
            "other prefixes",
            concat!(
                "<s:AssuranceCasePackage xmlns:s=\"http://www.omg.org/spec/SACM/20220301\"",
                " xmlns:x=\"http://www.omg.org/spec/XMI/20131001\" x:id=\"p\" x:type=\"s:AssuranceCasePackage\"/>",
            ),
        ),
        (
            "xsi:type",
            concat!(
                "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
                " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
                " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\"/>",
            ),
        ),
        (
            "a default namespace",
            concat!(
                "<ArgumentPackage xmlns=\"http://www.omg.org/spec/SACM/20220301\"",
                " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"d\"/>",
            ),
        ),
        (
            "comments and processing instructions",
            concat!(
                "<?xml version=\"1.0\"?>\n<!-- c -->\n<?pi x?>\n",
                "<xmi:XMI xmi:version=\"2.0\" xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
                " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n",
                "  <!-- before -->\n  <sacm:ArgumentPackage xmi:id=\"w1\"> <!-- in --> </sacm:ArgumentPackage>\n",
                "  <?pi y?>\n  <sacm:AssuranceCasePackage xmi:id=\"w2\"/>\n</xmi:XMI>\n",
            ),
        ),
        ("the marked-up ids", MARKED_UP_ROOTS),
    ];
    for (name, text) in inline {
        inputs.push((name.to_owned(), text.as_bytes().to_vec()));
    }
    let utf16: Vec<u8> = format!("\u{FEFF}{MARKED_UP_ROOTS}")
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    inputs.push(("the marked-up ids in UTF-16LE".to_owned(), utf16));
    inputs
}

#[test]
fn sacm23_xmi_013_every_readable_input_writes_and_reads_back_to_the_same_ids_and_kinds() {
    let row = "SACM23-XMI-013";
    for (name, input) in readable_inputs(row) {
        let doc = read_ok(row, &format!("setup: {name}"), &input);
        let (text, _) = write_checked(row, &doc);
        let back = read_ok(row, &format!("the output for {name}"), text.as_bytes());
        assert_eq!(
            id_kinds(&back),
            id_kinds(&doc),
            "{row}: {name} reads back differently after a write\n{text}"
        );
    }
}

#[test]
fn sacm23_xmi_007_writing_what_was_read_from_written_output_gives_the_same_bytes() {
    let row = "SACM23-XMI-007";
    for (name, input) in readable_inputs(row) {
        let first = write_ok(row, &read_ok(row, &format!("setup: {name}"), &input));
        let second = write_ok(
            row,
            &read_ok(row, &format!("the output for {name}"), first.as_bytes()),
        );
        assert_bytes(
            row,
            &format!("the second write of {name}"),
            &second,
            first.as_bytes(),
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-API-010: the writer never drops content
// ---------------------------------------------------------------------------

#[test]
fn lib_api_010_a_named_element_is_written_not_refused() {
    let row = "LIB-API-010";
    let name = || LangString::new("Named").with_lang("en");
    let cases: [(&[Step<'_>], Step<'_>); 5] = [
        (&[], (None, ElementKind::AssuranceCasePackage, "case")),
        (
            &[(None, ElementKind::AssuranceCasePackage, "case")],
            (Some("case"), ElementKind::ArgumentPackage, "arguments"),
        ),
        (
            &[(None, ElementKind::ArgumentPackage, "arguments")],
            (Some("arguments"), ElementKind::Claim, "c1"),
        ),
        (
            &[(None, ElementKind::ArgumentPackage, "arguments")],
            (Some("arguments"), ElementKind::AssertedInference, "i1"),
        ),
        (
            &[
                (None, ElementKind::ArtifactPackage, "artifacts"),
                (Some("artifacts"), ElementKind::Artifact, "a1"),
            ],
            (Some("a1"), ElementKind::Property, "p1"),
        ),
    ];
    for (setup, named) in cases {
        let mut doc = build(row, setup);
        insert_named(row, &mut doc, named, name());
        // `write_checked` fails the test on a refusal, and checks the name.
        write_checked(row, &doc);
    }
}

/// The name a `ModelElement` of [`complete_document`] gets, in turn: absent
/// fields, an empty content, empty fields, only a `lang`, only a `content`,
/// non-ASCII fields, and plain fields.
fn name_in_turn(turn: usize) -> LangString {
    match turn % 7 {
        0 => LangString::default(),
        1 => LangString::new(""),
        2 => LangString::new("").with_lang(""),
        3 => LangString::default().with_lang("en"),
        4 => LangString::new("Only content"),
        5 => {
            LangString::new("S\u{e4}kerhet \u{2713} \u{65e5}\u{672c} \u{1F600}").with_lang("sv-SE")
        }
        _ => LangString::new("Plain").with_lang("en"),
    }
}

/// A document that holds every feature a `Document` can hold today:
/// - every `ElementKind` in an admitted position: each of the twelve package
///   kinds as a root, under each one child of every kind its roles admit,
///   under each `ModelElement` child a Description, and under each
///   `ArtifactAsset` child a Property that holds a Note;
/// - every `ModelElement` named, the names of [`name_in_turn`] taken in turn;
/// - every Assertion declared, the six literals taken in turn; and, in one
///   more ArgumentPackage root `declarations` created with `edit::apply`,
///   one element `<Kind>.<literal>` per Assertion kind and literal: a Claim
///   created with `edit::apply`, the other kinds inserted.
fn complete_document(row: &str) -> Document {
    let mut names = 0_usize;
    let mut next_name = || {
        names += 1;
        name_in_turn(names)
    };
    let mut declarations = AssertionDeclaration::ALL.iter().copied().cycle();
    let mut doc = Document::new();
    for root in package_kinds() {
        let root_id = format!("{root}.root");
        if let Err(error) = doc.insert(
            None,
            root,
            ElementId::from(root_id.as_str()),
            Some(next_name()),
        ) {
            panic!("{row}: setup could not insert the root {root_id:?}: {error}");
        }
        for &kind in ElementKind::ALL {
            let child = format!("{root_id}/{kind}");
            let parent = ElementId::from(root_id.as_str());
            let name = kind.is_a(AbstractClass::ModelElement).then(&mut next_name);
            match doc.insert(Some(&parent), kind, ElementId::from(child.as_str()), name) {
                Ok(()) => {}
                Err(InsertError::ContainmentNotAllowed { .. }) => continue,
                Err(error) => panic!("{row}: setup could not insert {child:?}: {error}"),
            }
            if kind.is_a(AbstractClass::Assertion) {
                let declaration = declarations.next().unwrap_or_default();
                declare(row, &mut doc, &child, declaration);
            }
            if kind.is_a(AbstractClass::ModelElement) {
                let description = format!("{child}/Description");
                insert(
                    row,
                    &mut doc,
                    (Some(child.as_str()), ElementKind::Description, &description),
                );
            }
            if kind.is_a(AbstractClass::ArtifactAsset) {
                let property = format!("{child}/Property");
                let note = format!("{property}/Note");
                insert_named(
                    row,
                    &mut doc,
                    (Some(child.as_str()), ElementKind::Property, &property),
                    next_name(),
                );
                insert(
                    row,
                    &mut doc,
                    (Some(property.as_str()), ElementKind::Note, &note),
                );
            }
        }
    }
    let package = CreateArgumentPackage::root(next_name()).with_id("declarations");
    if let Err(error) = edit::apply(&mut doc, package) {
        panic!("{row}: setup: edit::apply refused the declarations package: {error}");
    }
    for kind in kinds_of(AbstractClass::Assertion) {
        for &declaration in AssertionDeclaration::ALL {
            let id = format!("{kind}.{}", declaration.as_str());
            if kind == ElementKind::Claim {
                let claim = CreateClaim::new("declarations", next_name()).with_id(id.as_str());
                if let Err(error) = edit::apply(&mut doc, claim) {
                    panic!("{row}: setup: edit::apply refused the Claim {id:?}: {error}");
                }
            } else {
                insert_named(
                    row,
                    &mut doc,
                    (Some("declarations"), kind, &id),
                    next_name(),
                );
            }
            declare(row, &mut doc, &id, declaration);
        }
    }
    doc
}

#[test]
fn lib_api_010_every_feature_a_document_holds_is_written() {
    let row = "LIB-API-010";
    let doc = complete_document(row);

    // Setup: every kind, every name variant and every (Assertion kind,
    // literal) pair is in the document.
    let kinds: HashSet<ElementKind> = doc.elements().map(SacmElement::kind).collect();
    for kind in ElementKind::ALL {
        assert!(kinds.contains(kind), "{row}: setup: no {kind}");
    }
    let names: Vec<&LangString> = doc.elements().filter_map(SacmElement::name).collect();
    for turn in 0..7 {
        let variant = name_in_turn(turn);
        assert!(
            names.contains(&&variant),
            "{row}: setup: no element is named {variant:?}"
        );
    }
    assert!(
        doc.elements()
            .filter(|element| element.kind().is_a(AbstractClass::ModelElement))
            .all(|element| element.name().is_some()),
        "{row}: setup: a ModelElement has no name"
    );
    let pairs: HashSet<(ElementKind, AssertionDeclaration)> = doc
        .elements()
        .filter_map(|element| Some((element.kind(), element.assertion_declaration()?)))
        .collect();
    assert_eq!(
        pairs.len(),
        36,
        "{row}: setup: not every Assertion kind holds every literal"
    );

    // `write_checked` fails the test on a refusal, and checks the form, every
    // element's tag, type, id, name and declaration, and that nothing else is
    // written.
    let (text, elements) = write_checked(row, &doc);
    let mut last_line = 0;
    for element in doc.elements() {
        let id = element.id().as_str();
        let written = by_id(row, &elements, id);
        let what = format!("the {} {id:?} (line {})", element.kind(), written.line);
        assert!(
            written.line > last_line,
            "{row}: {what} is not written in document order"
        );
        last_line = written.line;
        let name_lines = written
            .children
            .iter()
            .filter(|&&child| elements[child].name == NAME_ELEMENT)
            .count();
        assert_eq!(
            name_lines,
            usize::from(element.name().is_some()),
            "{row}: {what} has {name_lines} name lines"
        );
        assert_eq!(
            written.attribute("assertionDeclaration"),
            written_declaration(element),
            "{row}: {what} is written with the wrong assertionDeclaration"
        );
    }
    let non_default = doc
        .elements()
        .filter(|&element| written_declaration(element).is_some())
        .count();
    assert_eq!(
        text.matches(" assertionDeclaration=\"").count(),
        non_default,
        "{row}: the output does not hold one assertionDeclaration per non-default Assertion"
    );

    // Declared `asserted` again, explicitly: still the default, so the same
    // bytes as a document that never set it.
    let plain = every_kind_document_named(row, true);
    let mut redeclared = every_kind_document_named(row, true);
    let assertions: Vec<String> = redeclared
        .elements()
        .filter(|element| element.kind().is_a(AbstractClass::Assertion))
        .map(|element| element.id().as_str().to_owned())
        .collect();
    assert!(!assertions.is_empty(), "{row}: setup: no Assertion");
    for id in &assertions {
        declare(row, &mut redeclared, id, AssertionDeclaration::Assumed);
        declare(row, &mut redeclared, id, AssertionDeclaration::Asserted);
    }
    assert_bytes(
        row,
        "the redeclared document",
        &write_ok(row, &redeclared),
        write_ok(row, &plain).as_bytes(),
    );
}

/// Writes `doc` in strict mode, which the test expects to refuse it.
fn strict_write_err(row: &str, doc: &Document) -> WriteError {
    match xmi::write(doc, &WriteOptions::default().with_mode(Mode::Strict)) {
        Ok(text) => panic!("{row}: a strict write accepted {doc:?} and returned:\n{text}"),
        Err(error) => error,
    }
}

/// The name of `error`'s variant, or `None` for any other: `WriteError` is
/// `#[non_exhaustive]`, so a match outside the crate needs a fallback arm.
fn write_error_variant(error: &WriteError) -> Option<&'static str> {
    match error {
        WriteError::NoRoot => Some("NoRoot"),
        WriteError::UnrepresentableCharacter { .. } => Some("UnrepresentableCharacter"),
        WriteError::Invalid { .. } => Some("Invalid"),
        _ => None,
    }
}

#[test]
fn lib_api_010_write_error_has_exactly_the_variants_no_root_unrepresentable_character_and_invalid()
{
    let row = "LIB-API-010";
    // Every refusal a document can cause in tolerant mode, which does not
    // validate: no root, and a character outside XML 1.0 `Char` in an id, a
    // name's `lang` and a name's `content`. Strict mode adds `Invalid`, for
    // a document `validate` reports an Error for; its rules are
    // LIB-COMPAT-007's, tested in `load_and_save_modes.rs`.
    let mut refused: Vec<(&str, Document, Option<&str>)> = vec![
        ("an empty document", Document::new(), None),
        (
            "an id holding U+0001",
            build(row, &[(None, ElementKind::ArgumentPackage, "a\u{1}")]),
            Some("a\u{1}"),
        ),
    ];
    for (what, name) in [
        (
            "a name's lang holding U+0002",
            LangString::new("n").with_lang("\u{2}"),
        ),
        (
            "a name's content holding U+FFFE",
            LangString::new("\u{FFFE}"),
        ),
    ] {
        let mut doc = build(row, &[(None, ElementKind::ArgumentPackage, "a")]);
        insert_named(row, &mut doc, (Some("a"), ElementKind::Claim, "c"), name);
        refused.push((what, doc, Some("c")));
    }
    for (what, doc, element) in &refused {
        let error = write_err(row, doc);
        let expected = if element.is_some() {
            "UnrepresentableCharacter"
        } else {
            "NoRoot"
        };
        assert_eq!(
            write_error_variant(&error),
            Some(expected),
            "{row}: {what} is refused as {error:?}"
        );
        assert_eq!(
            error.element().map(ElementId::as_str),
            *element,
            "{row}: {what}: element() names the wrong element"
        );
        let code: Option<Code> = error.code();
        assert_eq!(code, None, "{row}: {what}: {error:?} has a catalog code");
    }
    // In strict mode, a nameless root is a validation Error (SACM23-VAL-018).
    let invalid = strict_write_err(
        row,
        &build(row, &[(None, ElementKind::ArgumentPackage, "a")]),
    );
    assert_eq!(
        write_error_variant(&invalid),
        Some("Invalid"),
        "{row}: a nameless root written strictly is refused as {invalid:?}"
    );
    assert_eq!(
        invalid.element(),
        None,
        "{row}: Invalid names an element: {invalid:?}"
    );
    let code: Option<Code> = invalid.code();
    assert_eq!(code, None, "{row}: {invalid:?} has a catalog code");

    // Everything else a document can hold is written, so no other variant
    // can be returned: every Assertion kind with every literal, alone and in
    // the complete document.
    for kind in kinds_of(AbstractClass::Assertion) {
        for &declaration in AssertionDeclaration::ALL {
            let mut doc = build(
                row,
                &[
                    (None, ElementKind::ArgumentPackage, "arguments"),
                    (Some("arguments"), kind, "x"),
                ],
            );
            declare(row, &mut doc, "x", declaration);
            if let Err(error) = write(&doc) {
                panic!("{row}: a {kind} declared {declaration} is refused: {error:?} ({error})");
            }
        }
    }
    if let Err(error) = write(&complete_document(row)) {
        panic!("{row}: the complete document is refused: {error:?} ({error})");
    }
}

#[test]
fn lib_api_010_write_error_is_a_send_sync_error_whose_variants_have_no_catalog_code() {
    let row = "LIB-API-010";
    assert_error_type::<WriteError>();

    let no_root = write_err(row, &Document::new());
    let unrepresentable = write_err(
        row,
        &build(row, &[(None, ElementKind::ArgumentPackage, "\u{FFFF}")]),
    );
    // An unrepresentable character in a name (SACM23-XMI-011).
    let mut named = Document::new();
    insert_named(
        row,
        &mut named,
        (None, ElementKind::ArgumentPackage, "a"),
        LangString::new("n\u{1}"),
    );
    let unrepresentable_name = write_err(row, &named);
    // A nameless root, written strictly (LIB-COMPAT-007).
    let invalid = strict_write_err(
        row,
        &build(row, &[(None, ElementKind::ArgumentPackage, "a")]),
    );
    for error in [no_root, unrepresentable, unrepresentable_name, invalid] {
        let code: Option<Code> = error.code();
        assert_eq!(code, None, "{row}: {error:?} has a catalog code");
        let message = error.to_string();
        assert!(
            !message.contains('\n'),
            "{row}: the message is not one line: {message:?}"
        );
        assert!(
            message.starts_with(char::is_lowercase),
            "{row}: the message does not start in lower case: {message:?}"
        );
        assert!(
            !message.ends_with(['.', '!', ':']),
            "{row}: the message ends in punctuation: {message:?}"
        );
        assert!(error.source().is_none(), "{row}: {error:?} wraps a cause");
    }
}
