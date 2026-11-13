//! Tests for SACM23-XMI-024: `xmi::write` writes no `xmi:version` attribute,
//! for one root or several, and the reader is unchanged (XMI 2.5.1 §7.5.3
//! "identification of the XMI version is indicated by the presence of the
//! XMI namespace declaration", and its `XMI` complexType declares no
//! attributes; production 1a; ADR 0017 amendment 3; D-XMI-VERSION).
//!
//! Written output is inspected through [`start_tags`], a scanner of the start
//! tags of output in the pinned form (double-quoted values, no markup in
//! them). "An attribute in the XMI namespace with local name `version`" is
//! checked by namespace, not by spelling: every prefix the output binds to
//! the XMI 2.5.1 namespace counts, and a plain text check for `xmi:version`
//! backs it up.
//!
//! "The same document" is `Document`'s `==`, the semantic comparison of
//! LIB-RT-001; a failure lists `semantic_compare`'s differences.
//!
//! Not checked here: SACM23-XMI-024 item 2's "the only difference from the
//! previous golden is that ` xmi:version="2.0"` is gone". That is a property
//! of the change to the golden files, checked by diff when they were changed;
//! the byte-for-byte golden comparisons are SACM23-XMI-007's, -011's and
//! -017's tests. [`sacm23_xmi_024_no_golden_file_carries_xmi_version`] keeps
//! the attribute out of them.

use sacm::compare::semantic_compare;
use sacm::diagnostic::SourceLocation;
use sacm::model::{
    AssertionDeclaration, Document, ElementId, ElementKind, InsertError, LangString,
};
use sacm::xmi::{self, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-XMI-024";

/// The pinned SACM 2.3 namespace (D-NS-PIN).
const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
/// The XMI 2.5.1 namespace.
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";
/// The namespace declarations of an input's document element.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);
/// The golden files of SACM23-XMI-007, -011 and -017.
const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23/xmi/golden/");
/// Every golden file.
const GOLDEN_FILES: [&str; 5] = [
    "single-root.golden.sacm.xmi",
    "several-roots.golden.sacm.xmi",
    "nested.golden.sacm.xmi",
    "named.golden.sacm.xmi",
    "needs-support-claim.golden.sacm.xmi",
];
/// The values of `xmi:version` the reader is given: the one assurance-forge
/// and EMF-based tools write, and the one of XMI 2.1 files.
const VERSIONS: [&str; 2] = ["2.0", "2.1"];

// ---------------------------------------------------------------------------
// Building documents
// ---------------------------------------------------------------------------

/// Inserts one element, or fails the test as a setup error.
fn insert(
    doc: &mut Document,
    parent: Option<&str>,
    kind: ElementKind,
    id: &str,
    name: Option<LangString>,
) {
    let parent_id = parent.map(ElementId::from);
    if let Err(error) = doc.insert(parent_id.as_ref(), kind, ElementId::from(id), name) {
        panic!("{ROW}: setup could not insert the {kind} {id:?} under {parent:?}: {error}");
    }
}

/// The twelve package kinds, in `ElementKind::ALL` order.
fn package_kinds() -> Vec<ElementKind> {
    let kinds: Vec<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_package())
        .collect();
    assert_eq!(
        kinds.len(),
        12,
        "{ROW}: setup: there are not twelve package kinds"
    );
    kinds
}

/// A document whose only element is one root of `kind`, with the id `root`.
fn single_root(kind: ElementKind) -> Document {
    let mut doc = Document::new();
    insert(&mut doc, None, kind, "root", None);
    doc
}

/// Three nameless roots in the order ArgumentPackage, AssuranceCasePackage,
/// ArgumentPackage.
fn three_roots() -> Document {
    let mut doc = Document::new();
    insert(
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "arguments_1",
        None,
    );
    insert(
        &mut doc,
        None,
        ElementKind::AssuranceCasePackage,
        "case",
        None,
    );
    insert(
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "arguments_2",
        None,
    );
    doc
}

/// A document the reader reads back in full: a named ArgumentPackage root
/// holding a named `needsSupport` Claim and an asserted one, then an
/// AssuranceCasePackage root holding a nested AssuranceCasePackage and an
/// ArgumentPackage that holds a Claim, then an ArgumentPackage root.
fn readable_document() -> Document {
    let mut doc = Document::new();
    let named = |content: &str| Some(LangString::new(content).with_lang("en"));
    insert(
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "arguments",
        named("Arguments"),
    );
    insert(
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "c1",
        named("The system is safe"),
    );
    insert(&mut doc, Some("arguments"), ElementKind::Claim, "c2", None);
    if let Err(error) =
        doc.set_assertion_declaration(&ElementId::from("c1"), AssertionDeclaration::NeedsSupport)
    {
        panic!("{ROW}: setup could not declare c1 needsSupport: {error}");
    }
    insert(
        &mut doc,
        None,
        ElementKind::AssuranceCasePackage,
        "case",
        None,
    );
    insert(
        &mut doc,
        Some("case"),
        ElementKind::AssuranceCasePackage,
        "sub",
        None,
    );
    insert(
        &mut doc,
        Some("case"),
        ElementKind::ArgumentPackage,
        "inner",
        None,
    );
    insert(&mut doc, Some("inner"), ElementKind::Claim, "c3", None);
    insert(&mut doc, None, ElementKind::ArgumentPackage, "last", None);
    doc
}

/// A document of twelve roots, one of each package kind, each holding one
/// child of every kind its roles admit. Every element is nameless.
fn every_kind_document() -> Document {
    let mut doc = Document::new();
    for root in package_kinds() {
        let root_id = format!("{root}.root");
        insert(&mut doc, None, root, &root_id, None);
        for &kind in ElementKind::ALL {
            let child = format!("{root_id}/{kind}");
            match doc.insert(
                Some(&ElementId::from(root_id.as_str())),
                kind,
                ElementId::from(child.as_str()),
                None,
            ) {
                Ok(()) | Err(InsertError::ContainmentNotAllowed { .. }) => {}
                Err(error) => panic!("{ROW}: setup could not insert {child:?}: {error}"),
            }
        }
    }
    doc
}

// ---------------------------------------------------------------------------
// Writing and reading
// ---------------------------------------------------------------------------

/// Writes `doc` with the default options, which the test expects to succeed.
fn write_ok(what: &str, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default()) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: write refused the document: {error} ({error:?})"),
    }
}

/// Reads `input` with the default options, which the test expects to succeed.
fn read_ok(what: &str, input: &str) -> ReadOutput {
    match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: read refused the input: {error}\n{input}"),
    }
}

/// Asserts that `left == right`, listing the differences when not.
fn assert_same_document(what: &str, left: &Document, right: &Document) {
    assert!(
        left == right,
        "{ROW}: {what}: the documents differ: {:?}",
        semantic_compare(left, right)
    );
}

// ---------------------------------------------------------------------------
// The start tags of written output
// ---------------------------------------------------------------------------

/// One start tag of written output.
#[derive(Debug)]
struct Tag {
    /// The line it starts on, from 1.
    line: usize,
    /// How many elements contain it: 0 for the document element.
    depth: usize,
    /// The element's name as written.
    name: String,
    /// The attributes in written order, with the values as written.
    attributes: Vec<(String, String)>,
}

/// The start tags of `text`, in document order. Fails the test on markup the
/// writer's pinned form does not have.
fn start_tags(what: &str, text: &str) -> Vec<Tag> {
    let mut tags = Vec::new();
    let mut depth = 0_usize;
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let line = text.len() - rest.len() + start;
        let line = text[..line].matches('\n').count() + 1;
        let after = &rest[start + 1..];
        let Some(end) = after.find('>') else {
            panic!("{ROW}: {what}: a tag on line {line} is not closed\n{text}");
        };
        let body = &after[..end];
        rest = &after[end + 1..];
        if body.starts_with('?') || body.starts_with('!') {
            continue;
        }
        if body.starts_with('/') {
            depth = depth.checked_sub(1).unwrap_or_else(|| {
                panic!("{ROW}: {what}: an end tag on line {line} closes nothing\n{text}")
            });
            continue;
        }
        let (body, empty) = match body.strip_suffix('/') {
            Some(body) => (body, true),
            None => (body, false),
        };
        let (name, attributes) = body.trim_end().split_once(' ').unwrap_or((body, ""));
        let name = name.to_owned();
        let attributes = parse_attributes(what, line, attributes);
        tags.push(Tag {
            line,
            depth,
            name,
            attributes,
        });
        if !empty {
            depth += 1;
        }
    }
    assert!(
        !tags.is_empty(),
        "{ROW}: {what}: the output has no element\n{text}"
    );
    tags
}

/// The `name="value"` pairs of a start tag after its name.
fn parse_attributes(what: &str, line: usize, mut text: &str) -> Vec<(String, String)> {
    let mut attributes = Vec::new();
    loop {
        text = text.trim_start();
        if text.is_empty() {
            return attributes;
        }
        let Some((name, after)) = text.split_once("=\"") else {
            panic!(
                "{ROW}: {what}: line {line} has an attribute not in the form name=\"value\": {text:?}"
            );
        };
        let Some((value, after)) = after.split_once('"') else {
            panic!("{ROW}: {what}: line {line} has an unterminated attribute value: {text:?}");
        };
        attributes.push((name.to_owned(), value.to_owned()));
        text = after;
    }
}

/// The prefixes that `tags` bind to the XMI 2.5.1 namespace.
fn xmi_prefixes(tags: &[Tag]) -> Vec<&str> {
    tags.iter()
        .flat_map(|tag| &tag.attributes)
        .filter(|(_, value)| value == XMI_NS)
        .filter_map(|(name, _)| name.strip_prefix("xmlns:"))
        .collect()
}

/// Asserts that no element of `text` carries an attribute in the XMI
/// namespace with local name `version`.
fn assert_no_xmi_version(what: &str, text: &str) {
    let tags = start_tags(what, text);
    let prefixes = xmi_prefixes(&tags);
    assert!(
        !prefixes.is_empty(),
        "{ROW}: {what}: setup: the output binds no prefix to the XMI namespace\n{text}"
    );
    for tag in &tags {
        for (name, value) in &tag.attributes {
            let in_xmi_version = name
                .split_once(':')
                .is_some_and(|(prefix, local)| local == "version" && prefixes.contains(&prefix));
            assert!(
                !in_xmi_version,
                "{ROW}: {what}: <{}> on line {} carries {name}=\"{value}\"\n{text}",
                tag.name, tag.line
            );
        }
    }
    assert!(
        !text.contains("xmi:version"),
        "{ROW}: {what}: the output contains \"xmi:version\"\n{text}"
    );
}

/// The attributes of `tag` as `(&str, &str)` pairs.
fn pairs(tag: &Tag) -> Vec<(&str, &str)> {
    tag.attributes
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect()
}

/// Asserts that the document element of `text` is `name` and carries exactly
/// the two pinned namespace declarations, then `xmi:id="root_id"` when it is
/// a bare root: nothing else, in the XMI namespace or out of it. No other
/// element declares a namespace.
fn assert_document_element(what: &str, text: &str, name: &str, root_id: Option<&str>) {
    let tags = start_tags(what, text);
    let document = &tags[0];
    assert_eq!(
        document.name, name,
        "{ROW}: {what}: the document element\n{text}"
    );
    let mut expected = vec![("xmlns:sacm", SACM_NS), ("xmlns:xmi", XMI_NS)];
    if let Some(id) = root_id {
        expected.push(("xmi:id", id));
    }
    assert_eq!(
        pairs(document),
        expected,
        "{ROW}: {what}: the attributes of the document element <{}>",
        document.name
    );
    for tag in &tags[1..] {
        for (name, _) in &tag.attributes {
            assert!(
                !name.starts_with("xmlns"),
                "{ROW}: {what}: <{}> on line {} declares {name}",
                tag.name,
                tag.line
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Inputs with and without xmi:version
// ---------------------------------------------------------------------------

/// Where an input template takes `xmi:version`: `{w}` on the wrapper, `{r}`
/// on each root.
#[derive(Clone, Copy, Debug)]
enum Placement {
    Wrapper,
    Roots,
    Both,
}

/// One input template, read with and without `xmi:version`.
struct Template {
    what: &'static str,
    text: String,
    placements: &'static [Placement],
}

/// The input templates. Each reads, without `xmi:version`, to a document
/// whose content the reader represents in full.
fn templates() -> Vec<Template> {
    let x = format!(" xmlns:s=\"{SACM_NS}\" xmlns:x=\"{XMI_NS}\"");
    vec![
        Template {
            what: "a bare ArgumentPackage root with names and a needsSupport Claim",
            text: format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <sacm:ArgumentPackage{NS}{{r}} xmi:id=\"arguments\">\n\
                 \x20 <name lang=\"en\" content=\"Arguments\"/>\n\
                 \x20 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" assertionDeclaration=\"needsSupport\">\n\
                 \x20   <name lang=\"en\" content=\"The system is safe\"/>\n\
                 \x20 </argumentElement>\n\
                 </sacm:ArgumentPackage>\n"
            ),
            placements: &[Placement::Roots],
        },
        Template {
            what: "a bare AssuranceCasePackage root with nested packages",
            text: format!(
                "<sacm:AssuranceCasePackage{{r}}{NS} xmi:id=\"case\">\
                 <assuranceCasePackage xmi:id=\"sub\"/>\
                 <argumentPackage xmi:id=\"inner\">\
                 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c3\"/>\
                 </argumentPackage></sacm:AssuranceCasePackage>"
            ),
            placements: &[Placement::Roots],
        },
        Template {
            what: "an xmi:XMI wrapper holding ArgumentPackage, AssuranceCasePackage, ArgumentPackage",
            text: format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <xmi:XMI{NS}{{w}}>\n\
                 \x20 <sacm:ArgumentPackage xmi:id=\"arguments_1\"{{r}}/>\n\
                 \x20 <sacm:AssuranceCasePackage{{r}} xmi:id=\"case\">\n\
                 \x20   <argumentPackage xmi:id=\"inner\"/>\n\
                 \x20 </sacm:AssuranceCasePackage>\n\
                 \x20 <sacm:ArgumentPackage xmi:id=\"arguments_2\"{{r}}/>\n\
                 </xmi:XMI>\n"
            ),
            placements: &[Placement::Wrapper, Placement::Roots, Placement::Both],
        },
        Template {
            what: "an x:XMI wrapper whose XMI prefix is x",
            text: format!(
                "<x:XMI{x}{{w}}><s:ArgumentPackage x:id=\"a1\"{{r}}/>\
                 <s:AssuranceCasePackage x:id=\"case\"{{r}}/></x:XMI>"
            ),
            placements: &[Placement::Wrapper, Placement::Roots, Placement::Both],
        },
    ]
}

/// `template` with `xmi:version="version"` where `placement` puts it, or
/// with no `xmi:version` when `version` is `None`. The prefix is the one the
/// template binds to the XMI namespace.
fn versioned(template: &Template, placement: Placement, version: Option<&str>) -> String {
    let prefix = if template.text.contains("xmlns:x=") {
        "x"
    } else {
        "xmi"
    };
    let attribute = version
        .map(|v| format!(" {prefix}:version=\"{v}\""))
        .unwrap_or_default();
    let (wrapper, roots) = match placement {
        Placement::Wrapper => (attribute.as_str(), ""),
        Placement::Roots => ("", attribute.as_str()),
        Placement::Both => (attribute.as_str(), attribute.as_str()),
    };
    template.text.replace("{w}", wrapper).replace("{r}", roots)
}

/// Every (description, input without `xmi:version`, input with it).
fn version_cases() -> Vec<(String, String, String)> {
    let mut cases = Vec::new();
    for template in templates() {
        for &placement in template.placements {
            for version in VERSIONS {
                cases.push((
                    format!(
                        "{}, xmi:version=\"{version}\" on {placement:?}",
                        template.what
                    ),
                    versioned(&template, placement, None),
                    versioned(&template, placement, Some(version)),
                ));
            }
        }
    }
    cases
}

// ---------------------------------------------------------------------------
// 1. No xmi:version on export; exactly the two declarations
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_024_each_package_kind_as_a_single_root_is_written_without_xmi_version() {
    for kind in package_kinds() {
        let what = format!("one {kind} root");
        let text = write_ok(&what, &single_root(kind));
        assert_no_xmi_version(&what, &text);
        assert_document_element(&what, &text, &format!("sacm:{kind}"), Some("root"));
    }
}

#[test]
fn sacm23_xmi_024_ap_acp_ap_roots_are_written_without_xmi_version_on_the_wrapper_or_a_root() {
    let what = "the roots ArgumentPackage, AssuranceCasePackage, ArgumentPackage";
    let text = write_ok(what, &three_roots());
    assert_no_xmi_version(what, &text);
    assert_document_element(what, &text, "xmi:XMI", None);
    let tags = start_tags(what, &text);
    let roots: Vec<(&str, Vec<(&str, &str)>)> = tags
        .iter()
        .filter(|tag| tag.depth == 1)
        .map(|tag| (tag.name.as_str(), pairs(tag)))
        .collect();
    assert_eq!(
        roots,
        [
            ("sacm:ArgumentPackage", vec![("xmi:id", "arguments_1")]),
            ("sacm:AssuranceCasePackage", vec![("xmi:id", "case")]),
            ("sacm:ArgumentPackage", vec![("xmi:id", "arguments_2")]),
        ],
        "{ROW}: {what}: each root inside xmi:XMI carries only its xmi:id\n{text}"
    );
}

#[test]
fn sacm23_xmi_024_no_element_of_a_document_holding_every_kind_carries_xmi_version() {
    let what = "twelve package roots, each holding every kind its roles admit";
    let text = write_ok(what, &every_kind_document());
    assert_no_xmi_version(what, &text);
    assert_document_element(what, &text, "xmi:XMI", None);
}

#[test]
fn sacm23_xmi_024_the_document_element_carries_nothing_in_the_xmi_namespace_but_an_id() {
    let cases = [
        (
            "one ArgumentPackage root",
            single_root(ElementKind::ArgumentPackage),
            true,
        ),
        ("the roots AP, ACP, AP", three_roots(), false),
        (
            "a readable document of three roots",
            readable_document(),
            false,
        ),
    ];
    for (what, doc, bare) in cases {
        let text = write_ok(what, &doc);
        let tags = start_tags(what, &text);
        let prefixes = xmi_prefixes(&tags);
        assert_eq!(
            prefixes,
            ["xmi"],
            "{ROW}: {what}: the prefixes bound to the XMI namespace"
        );
        let in_xmi: Vec<&str> = tags[0]
            .attributes
            .iter()
            .map(|(name, _)| name.as_str())
            .filter(|name| name.starts_with("xmi:"))
            .collect();
        let expected: &[&str] = if bare { &["xmi:id"] } else { &[] };
        assert_eq!(
            in_xmi, expected,
            "{ROW}: {what}: the document element's attributes in the XMI namespace\n{text}"
        );
        let declarations: Vec<(&str, &str)> = pairs(&tags[0])
            .into_iter()
            .filter(|(name, _)| name.starts_with("xmlns"))
            .collect();
        assert_eq!(
            declarations,
            [("xmlns:sacm", SACM_NS), ("xmlns:xmi", XMI_NS)],
            "{ROW}: {what}: the document element's namespace declarations"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. The golden files
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_024_no_golden_file_carries_xmi_version() {
    for name in GOLDEN_FILES {
        let path = format!("{GOLDEN_DIR}{name}");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{ROW}: cannot read {path}: {error}"));
        assert_no_xmi_version(name, &text);
        let tags = start_tags(name, &text);
        let root_id = (tags[0].name != "xmi:XMI").then(|| {
            tags[0]
                .attributes
                .iter()
                .find(|(attribute, _)| attribute == "xmi:id")
                .map_or("", |(_, value)| value.as_str())
                .to_owned()
        });
        let element = tags[0].name.clone();
        assert_document_element(name, &text, &element, root_id.as_deref());
    }
}

// ---------------------------------------------------------------------------
// 3. Reading xmi:version and writing it back
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_024_reading_xmi_version_on_the_wrapper_or_the_roots_gives_an_equal_document() {
    for (what, without, with) in version_cases() {
        let plain = read_ok(&format!("{what}: without the attribute"), &without);
        let versioned = read_ok(&what, &with);
        assert_same_document(&what, versioned.document(), plain.document());
        assert_eq!(
            versioned.diagnostics(),
            plain.diagnostics(),
            "{ROW}: {what}: the diagnostics differ from the read without xmi:version"
        );
    }
}

#[test]
fn sacm23_xmi_024_writing_a_document_read_with_xmi_version_gives_the_same_bytes_without_it() {
    for (what, without, with) in version_cases() {
        let plain = write_ok(
            &format!("{what}: without the attribute"),
            read_ok(&format!("{what}: without the attribute"), &without).document(),
        );
        let versioned = write_ok(&what, read_ok(&what, &with).document());
        assert_no_xmi_version(&what, &versioned);
        assert_eq!(
            versioned, plain,
            "{ROW}: {what}: the output differs from the one for the input without xmi:version"
        );
    }
}

#[test]
fn sacm23_xmi_024_a_golden_file_with_xmi_version_added_writes_back_as_the_golden_file() {
    // The golden files whose content the reader represents in full. The
    // nested one holds a Note, a Description and an Artifact.
    let readable = [
        "single-root.golden.sacm.xmi",
        "several-roots.golden.sacm.xmi",
        "named.golden.sacm.xmi",
        "needs-support-claim.golden.sacm.xmi",
    ];
    let declaration = format!(" xmlns:xmi=\"{XMI_NS}\"");
    for name in readable {
        let path = format!("{GOLDEN_DIR}{name}");
        let golden = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{ROW}: cannot read {path}: {error}"));
        assert_eq!(
            golden.matches(&declaration).count(),
            1,
            "{ROW}: {name}: setup: the golden file does not declare xmlns:xmi once"
        );
        for version in VERSIONS {
            let what = format!("{name} with xmi:version=\"{version}\" on its document element");
            let input = golden.replacen(
                &declaration,
                &format!("{declaration} xmi:version=\"{version}\""),
                1,
            );
            let text = write_ok(&what, read_ok(&what, &input).document());
            assert_eq!(
                text, golden,
                "{ROW}: {what}: the output is not the golden file"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Determinism
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_024_writing_twice_a_clone_and_a_read_back_give_identical_bytes_without_xmi_version() {
    let mut documents = vec![
        (
            "a readable document of three roots".to_owned(),
            readable_document(),
            true,
        ),
        ("the roots AP, ACP, AP".to_owned(), three_roots(), true),
        (
            "every kind under twelve roots".to_owned(),
            every_kind_document(),
            false,
        ),
    ];
    for kind in package_kinds() {
        documents.push((format!("one {kind} root"), single_root(kind), false));
    }
    for (what, doc, readable) in documents {
        let first = write_ok(&what, &doc);
        assert_no_xmi_version(&what, &first);
        assert_eq!(
            write_ok(&what, &doc),
            first,
            "{ROW}: {what}: a second write differs"
        );
        assert_eq!(
            write_ok(&what, &doc.clone()),
            first,
            "{ROW}: {what}: the clone writes differently"
        );
        if readable {
            let back = read_ok(&format!("{what}: the written output"), &first);
            assert_same_document(&what, back.document(), &doc);
            assert_eq!(
                write_ok(&what, back.document()),
                first,
                "{ROW}: {what}: write(read(write(d))) differs from write(d)"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 5. The reader is unchanged
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_024_the_reader_ignores_xmi_version_on_the_wrapper_and_on_the_roots() {
    let reference = read_ok(
        "the reference",
        &format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\"/>"),
    );
    let two = read_ok(
        "the two-root reference",
        &format!(
            "<xmi:XMI{NS}><sacm:ArgumentPackage xmi:id=\"a\"/>\
             <sacm:AssuranceCasePackage xmi:id=\"n\"/></xmi:XMI>"
        ),
    );
    for version in VERSIONS {
        let v = format!(" xmi:version=\"{version}\"");
        let bare = format!("<sacm:ArgumentPackage{NS}{v} xmi:id=\"a\"/>");
        let output = read_ok(&format!("{v} on a bare root"), &bare);
        assert_same_document(
            &format!("{v} on a bare root"),
            output.document(),
            reference.document(),
        );
        assert!(
            output.diagnostics().is_empty(),
            "{ROW}: {v} on a bare root: diagnostics {:?}",
            output.diagnostics()
        );
        let wrapped = [
            (
                "on the wrapper",
                format!(
                    "<xmi:XMI{NS}{v}><sacm:ArgumentPackage xmi:id=\"a\"/>\
                     <sacm:AssuranceCasePackage xmi:id=\"n\"/></xmi:XMI>"
                ),
            ),
            (
                "on the roots in a wrapper",
                format!(
                    "<xmi:XMI{NS}><sacm:ArgumentPackage xmi:id=\"a\"{v}/>\
                     <sacm:AssuranceCasePackage{v} xmi:id=\"n\"/></xmi:XMI>"
                ),
            ),
        ];
        for (place, input) in wrapped {
            let what = format!("{v} {place}");
            let output = read_ok(&what, &input);
            assert_same_document(&what, output.document(), two.document());
            assert!(
                output.diagnostics().is_empty(),
                "{ROW}: {what}: diagnostics {:?}",
                output.diagnostics()
            );
        }
    }
}

#[test]
fn sacm23_xmi_024_the_reader_refuses_xmi_version_on_a_child_at_the_attribute_name() {
    // Each child is on line 2 of its root; the attribute's name is located.
    let acp = |child: &str| {
        format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n{child}\n</sacm:AssuranceCasePackage>"
        )
    };
    let ap = |child: &str| {
        format!("<sacm:ArgumentPackage{NS} xmi:id=\"arguments\">\n{child}\n</sacm:ArgumentPackage>")
    };
    for version in VERSIONS {
        let v = format!("xmi:version=\"{version}\"");
        let children = [
            (
                "an AssuranceCasePackage in assuranceCasePackage",
                acp(&format!("  <assuranceCasePackage xmi:id=\"sub\" {v}/>")),
                format!("  <assuranceCasePackage xmi:id=\"sub\" {v}/>"),
            ),
            (
                "an ArgumentPackage in argumentPackage",
                acp(&format!("  <argumentPackage {v} xmi:id=\"inner\"/>")),
                format!("  <argumentPackage {v} xmi:id=\"inner\"/>"),
            ),
            (
                "an ArgumentPackage in argumentElement",
                ap(&format!(
                    "  <argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"ap\" {v}/>"
                )),
                format!("  <argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"ap\" {v}/>"),
            ),
            (
                "a Claim in argumentElement",
                ap(&format!(
                    "  <argumentElement {v} xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>"
                )),
                format!("  <argumentElement {v} xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>"),
            ),
        ];
        for (what, input, line_2) in children {
            let what = format!("{v} on {what}");
            let Some(offset) = line_2.find("xmi:version") else {
                panic!("{ROW}: {what}: setup: line 2 has no xmi:version");
            };
            let column = u32::try_from(line_2[..offset].chars().count() + 1)
                .unwrap_or_else(|_| panic!("{ROW}: {what}: setup: the column is too large"));
            let Some(expected) = SourceLocation::new(2, column) else {
                panic!("{ROW}: {what}: setup: SourceLocation::new(2, {column}) refused");
            };
            match xmi::read(input.as_bytes(), &ReadOptions::default()) {
                Ok(output) => panic!(
                    "{ROW}: {what}: a document of {} elements was returned for {input:?}",
                    output.document().element_count()
                ),
                Err(error @ ReadError::Unsupported { .. }) => {
                    assert_eq!(
                        error.location(),
                        Some(expected),
                        "{ROW}: {what}: the refusal's location"
                    );
                    assert_eq!(
                        error.code(),
                        None,
                        "{ROW}: {what}: Unsupported has no catalog code"
                    );
                }
                Err(error) => {
                    panic!("{ROW}: {what}: refused as {error:?}, not as Unsupported at {expected}")
                }
            }
        }
    }
}
