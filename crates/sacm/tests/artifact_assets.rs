//! Tests for reading, writing and editing the artifact assets slice 0052
//! reads: `Property` in the `property` role (SACM23-ART-003), `Technique`
//! (SACM23-ART-008) and `Participant` (SACM23-ART-009) (formal/23-05-08
//! §12.6 `property:Property[0..*]`, §12.8, §12.12, §12.13; ptc/22-03-13
//! `Artifact::ArtifactAsset.property` (composite), `Artifact::Property`,
//! `Artifact::Technique`, `Artifact::Participant`; ADR 0033, ADR 0034).
//!
//! A `property` child is read untyped or typed `sacm:Property` under every
//! `ArtifactAsset` this release reads (Property, Technique, Participant), at
//! any depth, and is written without a type. A `property` typed as another
//! class, or under an element that is no `ArtifactAsset`, is preserved content
//! of its parent (SACM-XMI-003). A Technique and a Participant are read typed
//! in `artifactElement` with every Base feature, write back to `==`, and take
//! the existing edits. Properties are content: deleting an asset holding one
//! is `NotEmpty` unless recursive.
//!
//! `SacmElement::properties()` and the creates are tested in
//! `artifact_edits.rs`, with the other edits of slice 0052.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::edit::{
    self, DeleteElement, EditError, Mutation, Operation, PackageDeletePolicy, ReorderElements,
    SetFlag, SetGid, SetName, SetReferences,
};
use sacm::model::{Document, ElementId, ElementKind, Flag, LangString, ReferenceEnd, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ART_003: &str = "SACM23-ART-003";
const ART_008: &str = "SACM23-ART-008";
const ART_009: &str = "SACM23-ART-009";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where an occurrence is expected. Removed before
/// reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

// --- Inputs ------------------------------------------------------------------------

/// An ArtifactPackage root `a` named "A" holding `body` on line 3.
fn in_a(body: &str) -> String {
    format!(
        "<sacm:ArtifactPackage{NS} xmi:id=\"a\">\n  <name content=\"A\"/>\n  {body}\n\
         </sacm:ArtifactPackage>"
    )
}

/// The element `h` of class `class`, typed in `artifactElement` of [`in_a`],
/// named "H", holding `content` after its name.
fn host(class: &str, content: &str) -> String {
    in_a(&format!(
        "<artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"h\"><name content=\"H\"/>{content}\
         </artifactElement>"
    ))
}

/// A `property` element `id` with `attributes`, named after its id, holding
/// `content`.
fn property(id: &str, attributes: &str, content: &str) -> String {
    format!("<property{attributes} xmi:id=\"{id}\"><name content=\"{id}\"/>{content}</property>")
}

/// The three `ArtifactAsset`s this release reads.
const ASSETS: [&str; 3] = ["Property", "Technique", "Participant"];

fn loc(row: &str, line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{row}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its one mark, and the mark's location: lines end at
/// LF, and columns count characters.
fn one_mark(row: &str, template: &str) -> (String, SourceLocation) {
    let Some(offset) = template.find(MARK) else {
        panic!("{row}: setup: {template:?} has no mark");
    };
    let text = template.replacen(MARK, "", 1);
    assert!(
        !text.contains(MARK),
        "{row}: setup: {template:?} has two marks"
    );
    let (mut line, mut column) = (1_u32, 1_u32);
    for c in template[..offset].chars() {
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (text, loc(row, line, column))
}

// --- Reading -----------------------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read to `==`
/// documents reporting only diagnostics with `allowed` (none when `None`),
/// and returns the strict one.
fn read_reporting(row: &str, what: &str, input: &str, allowed: Option<Code>) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => {
                assert!(
                    output
                        .diagnostics()
                        .iter()
                        .all(|found| Some(found.code()) == allowed),
                    "{row}: {what}: reading in {mode:?} mode reported {:?}",
                    output.diagnostics()
                );
                if allowed.is_some() {
                    assert!(
                        !output.diagnostics().is_empty(),
                        "{row}: {what}: reading in {mode:?} mode reported no {allowed:?}"
                    );
                }
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

/// Reads `input` in both modes with no diagnostic.
fn read_ok(row: &str, what: &str, input: &str) -> Document {
    read_reporting(row, what, input, None)
}

/// Asserts that `template`, without its one mark, reads in both modes and
/// keeps one construct as preserved content of `holder`: exactly one
/// SACM-XMI-003 at the mark, affecting `holder`, a Warning in tolerant mode
/// and an Error in strict mode, and the same document in both.
fn assert_preserved_at(row: &str, what: &str, template: &str, holder: &str) -> Document {
    let (input, at) = one_mark(row, template);
    let mut documents = Vec::new();
    for (mode, severity) in [
        (Mode::Strict, Severity::Error),
        (Mode::Tolerant, Severity::Warning),
    ] {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!(
                "{row}: {what}: a {mode:?} read refused it instead of preserving it: {error:?}"
            ),
        };
        let found: Vec<_> = output
            .diagnostics()
            .iter()
            .map(|found| {
                (
                    found.code(),
                    found.severity(),
                    found.location(),
                    found.affected().to_vec(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [(
                Code::XMI_UNKNOWN_CONTENT,
                severity,
                Some(at),
                vec![ElementId::from(holder)]
            )],
            "{row}: {what}: not one SACM-XMI-003 at the construct affecting {holder:?} in \
             {mode:?} mode"
        );
        let doc = output.into_document();
        assert!(
            doc.get(holder)
                .is_some_and(SacmElement::has_preserved_content),
            "{row}: {what}: {holder:?} holds no preserved content in {mode:?} mode"
        );
        documents.push(doc);
    }
    assert_eq!(
        documents[0], documents[1],
        "{row}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

fn element<'d>(row: &str, doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{row}: the document has no element {id:?}");
    };
    found
}

fn children(row: &str, doc: &Document, id: &str) -> Vec<String> {
    element(row, doc, id)
        .children()
        .map(|child| child.id().as_str().to_owned())
        .collect()
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|&value| value.to_owned()).collect()
}

fn list(values: &[&str]) -> Vec<ElementId> {
    values.iter().copied().map(ElementId::from).collect()
}

/// `(kind, parent id)` of the element `id`.
fn placed(row: &str, doc: &Document, id: &str) -> (ElementKind, Option<String>) {
    let found = element(row, doc, id);
    (
        found.kind(),
        found.parent().map(|parent| parent.id().as_str().to_owned()),
    )
}

// --- Writing -----------------------------------------------------------------------

fn write_in(row: &str, what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// Writes `doc` in `mode`, reads the output back and writes that again: the
/// read-back must be `==` and the second write must give the same bytes.
fn round_trip(row: &str, what: &str, doc: &Document, mode: Mode) -> String {
    let text = write_in(row, what, doc, mode);
    let back = read_ok(row, &format!("{what}, written in {mode:?} mode"), &text);
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{row}: {what}: the output does not read back equal\n{text}"
    );
    assert!(back == *doc, "{row}: {what}: the output reads back !=");
    assert_eq!(
        write_in(row, what, &back, mode),
        text,
        "{row}: {what}: writing what was read back gives other bytes"
    );
    text
}

fn line_with<'t>(row: &str, text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{row}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
}

// --- Editing -----------------------------------------------------------------------

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// Applies `operation`, which the test expects to succeed.
fn applied(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{row}: {operation:?} was refused: {error} ({error:?})"),
    }
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before, and returns why.
fn refused_edit(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let before = doc.clone();
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{row}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{row}: the refused {operation:?} changed the document"
    );
    error
}

// =====================================================================================
// SACM23-ART-003 item 1: reading `property`
// =====================================================================================

#[test]
fn sacm23_art_003_property_reads_untyped_or_typed_under_each_readable_asset_in_both_modes() {
    for class in ASSETS {
        for (form, attributes) in [
            ("untyped", ""),
            ("typed sacm:Property", " xmi:type=\"sacm:Property\""),
        ] {
            let what = format!("a property, {form}, under a {class}");
            let doc = read_ok(
                ART_003,
                &what,
                &host(class, &property("p1", attributes, "")),
            );
            assert_eq!(
                (
                    placed(ART_003, &doc, "p1"),
                    placed(ART_003, &doc, "h").0,
                    children(ART_003, &doc, "h"),
                    element(ART_003, &doc, "p1")
                        .name()
                        .and_then(LangString::content)
                ),
                (
                    (ElementKind::Property, Some("h".to_owned())),
                    ElementKind::ALL
                        .iter()
                        .copied()
                        .find(|kind| kind.name() == class)
                        .unwrap_or(ElementKind::Property),
                    ids(&["p1"]),
                    Some("p1")
                ),
                "{ART_003}: {what}: not a Property p1 in h"
            );
        }
        // By xsi:type: read, with SACM-XMI-010.
        let what = format!("an xsi-typed property under a {class}");
        let doc = read_reporting(
            ART_003,
            &what,
            &host(class, &property("p1", " xsi:type=\"sacm:Property\"", "")),
            Some(Code::XMI_XSI_TYPE),
        );
        assert_eq!(
            placed(ART_003, &doc, "p1"),
            (ElementKind::Property, Some("h".to_owned())),
            "{ART_003}: {what}: not a Property p1 in h"
        );
        // Several, in document order, among utility elements.
        let what = format!("two properties and a note under a {class}");
        let doc = read_ok(
            ART_003,
            &what,
            &host(
                class,
                &format!(
                    "{}<note xmi:id=\"n\"><content><value content=\"N\"/></content></note>{}",
                    property("p2", "", ""),
                    property("p1", " xmi:type=\"sacm:Property\"", "")
                ),
            ),
        );
        assert_eq!(
            children(ART_003, &doc, "h"),
            ids(&["p2", "n", "p1"]),
            "{ART_003}: {what}: not in document order"
        );
    }
}

#[test]
fn sacm23_art_003_property_nests_three_deep_and_reads_typed_in_artifact_element() {
    for class in ["Technique", "Participant", "Property"] {
        let what = format!("properties three deep under a {class}");
        let deep = property(
            "p1",
            "",
            &property("p2", " xmi:type=\"sacm:Property\"", &property("p3", "", "")),
        );
        let doc = read_ok(ART_003, &what, &host(class, &deep));
        assert_eq!(
            (
                placed(ART_003, &doc, "p1"),
                placed(ART_003, &doc, "p2"),
                placed(ART_003, &doc, "p3"),
                children(ART_003, &doc, "p3")
            ),
            (
                (ElementKind::Property, Some("h".to_owned())),
                (ElementKind::Property, Some("p1".to_owned())),
                (ElementKind::Property, Some("p2".to_owned())),
                Vec::new()
            ),
            "{ART_003}: {what}: not the chain h > p1 > p2 > p3"
        );
    }
    // A Property directly in an ArtifactPackage: a child of the package.
    let what = "a Property typed in artifactElement";
    let doc = read_ok(
        ART_003,
        what,
        &in_a(
            "<artifactElement xmi:type=\"sacm:Property\" xmi:id=\"p1\"><name content=\"P1\"/>\
             <property xmi:id=\"p2\"><name content=\"P2\"/></property></artifactElement>",
        ),
    );
    assert_eq!(
        (placed(ART_003, &doc, "p1"), placed(ART_003, &doc, "p2")),
        (
            (ElementKind::Property, Some("a".to_owned())),
            (ElementKind::Property, Some("p1".to_owned()))
        ),
        "{ART_003}: {what}: not a Property p1 in a holding the Property p2"
    );
}

#[test]
fn sacm23_art_003_a_property_typed_as_another_class_is_preserved_by_its_asset() {
    for class in ASSETS {
        for other in [
            "Technique",
            "Participant",
            "Claim",
            "ArtifactAsset",
            "Artifact",
        ] {
            let what = format!("a property typed sacm:{other} under a {class}");
            assert_preserved_at(
                ART_003,
                &what,
                &host(
                    class,
                    &format!("{MARK}<property xmi:type=\"sacm:{other}\" xmi:id=\"p1\"/>"),
                ),
                "h",
            );
        }
    }
}

#[test]
fn sacm23_art_003_a_property_under_an_element_that_is_no_artifact_asset_is_preserved_by_it() {
    let child = format!("{MARK}<property xmi:id=\"p9\"><name content=\"P9\"/></property>");
    let cases = [
        (
            "under an ArtifactPackage root",
            format!(
                "<sacm:ArtifactPackage{NS} xmi:id=\"a\">\n  <name content=\"A\"/>\n  {child}\n\
                 </sacm:ArtifactPackage>"
            ),
            "a",
        ),
        (
            "under an ArtifactPackage in artifactPackage",
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\n  \
                 <artifactPackage xmi:id=\"a\"><name content=\"A\"/>{child}</artifactPackage>\n\
                 </sacm:AssuranceCasePackage>"
            ),
            "a",
        ),
        (
            "under an ArtifactGroup",
            in_a(&format!(
                "<artifactElement xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"g\">\
                 <name content=\"G\"/>{child}</artifactElement>"
            )),
            "g",
        ),
        (
            "under a Claim in an ArtifactPackage",
            in_a(&format!(
                "<artifactElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"><name content=\"C\"/>\
                 {child}</artifactElement>"
            )),
            "c",
        ),
        (
            "under a Claim in an ArgumentPackage",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\n  \
                 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"><name content=\"C\"/>\
                 {child}</argumentElement>\n</sacm:ArgumentPackage>"
            ),
            "c",
        ),
        (
            "under an ArtifactReference",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\n  \
                 <argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"r\">\
                 <name content=\"R\"/>{child}</argumentElement>\n</sacm:ArgumentPackage>"
            ),
            "r",
        ),
        (
            "under an AssuranceCasePackage",
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\n  \
                 {child}\n</sacm:AssuranceCasePackage>"
            ),
            "case",
        ),
    ];
    for (what, template, holder) in &cases {
        let what = format!("a property {what}");
        let doc = assert_preserved_at(ART_003, &what, template, holder);
        assert!(
            doc.get("p9").is_none(),
            "{ART_003}: {what}: the preserved property was read as an element"
        );
    }
}

// =====================================================================================
// SACM23-ART-003 item 2: writing `property`
// =====================================================================================

#[test]
fn sacm23_art_003_property_children_are_written_without_a_type_and_round_trip() {
    for class in ASSETS {
        let what = format!("typed and untyped properties under a {class}");
        let doc = read_ok(
            ART_003,
            &what,
            &host(
                class,
                &format!(
                    "{}{}",
                    property("p1", " xmi:type=\"sacm:Property\"", &property("p2", "", "")),
                    property("p3", "", "")
                ),
            ),
        );
        for mode in MODES {
            let text = round_trip(ART_003, &what, &doc, mode);
            assert_eq!(
                (
                    line_with(ART_003, &text, "h"),
                    line_with(ART_003, &text, "p1"),
                    line_with(ART_003, &text, "p2"),
                    line_with(ART_003, &text, "p3")
                ),
                (
                    format!("<artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"h\">").as_str(),
                    "<property xmi:id=\"p1\">",
                    "<property xmi:id=\"p2\">",
                    "<property xmi:id=\"p3\">"
                ),
                "{ART_003}: {what}: the properties are not written untyped in {mode:?} mode\n{text}"
            );
        }
    }
    // A Property in artifactElement is written there, typed.
    let what = "a Property in artifactElement";
    let doc = read_ok(
        ART_003,
        what,
        &in_a(
            "<artifactElement xmi:type=\"sacm:Property\" xmi:id=\"p1\"><name content=\"P\"/>\
               </artifactElement>",
        ),
    );
    let text = round_trip(ART_003, what, &doc, Mode::Strict);
    assert_eq!(
        line_with(ART_003, &text, "p1"),
        "<artifactElement xmi:type=\"sacm:Property\" xmi:id=\"p1\">",
        "{ART_003}: {what}: not written typed in artifactElement\n{text}"
    );
}

// =====================================================================================
// SACM23-ART-003 item 4: properties are content
// =====================================================================================

#[test]
fn sacm23_art_003_deleting_an_asset_holding_properties_is_not_empty_unless_recursive() {
    let mut doc = read_ok(
        ART_003,
        "a Technique holding a Property, and a Property chain",
        &in_a(&format!(
            "<artifactElement xmi:type=\"sacm:Technique\" xmi:id=\"t\"><name content=\"T\"/>{}\
             </artifactElement><artifactElement xmi:type=\"sacm:Property\" xmi:id=\"q1\">\
             <name content=\"Q1\"/>{}</artifactElement>",
            property("tp", "", ""),
            property("q2", "", &property("q3", "", ""))
        )),
    );
    for (id, contained) in [("t", 1), ("q1", 2), ("q2", 1)] {
        let error = refused_edit(ART_003, &mut doc, DeleteElement::new(id));
        assert!(
            matches!(
                &error,
                EditError::NotEmpty { element, contained: count, .. }
                    if element.as_str() == id && *count == contained
            ),
            "{ART_003}: deleting {id}: not NotEmpty {{ {id}, contained: {contained} }}: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_PACKAGE_NOT_EMPTY,
            "{ART_003}: deleting {id}: not SACM-CMD-006"
        );
    }
    // The innermost Property holds nothing: deleted alone.
    let mutation = applied(ART_003, &mut doc, DeleteElement::new("q3"));
    assert_eq!(
        mutation.deleted_ids(),
        list(&["q3"]).as_slice(),
        "{ART_003}: deleting the empty q3"
    );
    let mutation = applied(
        ART_003,
        &mut doc,
        DeleteElement::new("t").with_package_policy(PackageDeletePolicy::DeleteRecursively),
    );
    assert_eq!(
        (mutation.deleted_ids(), children(ART_003, &doc, "a")),
        (list(&["t", "tp"]).as_slice(), ids(&["q1"])),
        "{ART_003}: a recursive delete of t"
    );
}

// =====================================================================================
// SACM23-ART-008 / SACM23-ART-009 item 1: every Base feature, read and written
// =====================================================================================

/// An ArtifactPackage `a` holding three elements of class `class`: `x1` with
/// a description, a note, a tagged value, a gid, `isCitation`, a
/// `citedElement` (`x3`), an `abstractForm` (`x2`) and a property; `x2`
/// abstract; and `x3` plain.
fn full(class: &str) -> String {
    in_a(&format!(
        "<artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"x1\" gid=\"G-1\" isCitation=\"true\" \
         citedElement=\"x3\" abstractForm=\"x2\">\n    <name lang=\"en\" content=\"X1\"/>\n    \
         <description xmi:id=\"x1_d\"><content><value lang=\"en\" content=\"Top-down\"/></content>\
         </description>\n    \
         <note xmi:id=\"x1_n\"><content><value content=\"Reviewed\"/></content></note>\n    \
         <taggedValue xmi:id=\"x1_t\"><key><value content=\"Tool\"/></key>\
         <content><value content=\"FTA+\"/></content></taggedValue>\n    \
         <property xmi:id=\"x1_p\"><name content=\"Qualified\"/></property>\n  \
         </artifactElement>\n  \
         <artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"x2\" isAbstract=\"true\">\
         <name content=\"X2\"/></artifactElement>\n  \
         <artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"x3\"><name content=\"X3\"/>\
         </artifactElement>"
    ))
}

fn assert_every_base_feature_round_trips(row: &str, class: &str, kind: ElementKind) {
    let what = format!("a {class} with every Base feature");
    let doc = read_ok(row, &what, &full(class));
    let x1 = element(row, &doc, "x1");
    assert_eq!(
        (
            x1.kind(),
            x1.gid(),
            x1.is_citation(),
            x1.cited_element().map(ElementId::as_str),
            x1.abstract_form().map(ElementId::as_str),
            x1.is_abstract(),
            element(row, &doc, "x2").is_abstract(),
            x1.descriptions().count(),
            x1.notes().count(),
            x1.tagged_values().count(),
            children(row, &doc, "x1"),
        ),
        (
            kind,
            Some("G-1"),
            true,
            Some("x3"),
            Some("x2"),
            false,
            true,
            1,
            1,
            1,
            ids(&["x1_d", "x1_n", "x1_t", "x1_p"]),
        ),
        "{row}: {what}: not read with every feature"
    );
    assert_eq!(
        placed(row, &doc, "x1_p"),
        (ElementKind::Property, Some("x1".to_owned())),
        "{row}: {what}: the property is not a Property in x1"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: {what}: setup: the document does not validate cleanly"
    );
    for mode in MODES {
        let text = round_trip(row, &what, &doc, mode);
        assert_eq!(
            line_with(row, &text, "x1"),
            format!(
                "<artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"x1\" gid=\"G-1\" \
                 isCitation=\"true\" citedElement=\"x3\" abstractForm=\"x2\">"
            ),
            "{row}: {what}: the start tag of x1 in {mode:?} mode\n{text}"
        );
    }
    // The same in an AssuranceCasePackage's artifactPackage.
    let nested = full(class)
        .replacen("<sacm:ArtifactPackage", "<sacm:AssuranceCasePackage", 1)
        .replacen(
            " xmi:id=\"a\">\n  <name content=\"A\"/>",
            " xmi:id=\"case\"><name content=\"Case\"/><artifactPackage xmi:id=\"a\">\
             <name content=\"A\"/>",
            1,
        )
        .replacen(
            "</sacm:ArtifactPackage>",
            "</artifactPackage></sacm:AssuranceCasePackage>",
            1,
        );
    let what = format!("{what}, in an AssuranceCasePackage");
    let doc = read_ok(row, &what, &nested);
    assert_eq!(
        placed(row, &doc, "x1"),
        (kind, Some("a".to_owned())),
        "{row}: {what}: x1 is not in a"
    );
    round_trip(row, &what, &doc, Mode::Strict);
}

#[test]
fn sacm23_art_008_a_technique_with_every_base_feature_reads_in_both_modes_and_writes_back() {
    assert_every_base_feature_round_trips(ART_008, "Technique", ElementKind::Technique);
}

#[test]
fn sacm23_art_009_a_participant_with_every_base_feature_reads_in_both_modes_and_writes_back() {
    assert_every_base_feature_round_trips(ART_009, "Participant", ElementKind::Participant);
}

// =====================================================================================
// SACM23-ART-008 / SACM23-ART-009 item 3: the existing edits apply
// =====================================================================================

#[expect(
    clippy::too_many_lines,
    reason = "one edit after the other on one document reads best as one function"
)]
fn assert_the_existing_edits_apply(row: &str, class: &str) {
    let mut doc = read_ok(row, &format!("three {class}s"), &full(class));
    // SetName.
    let mutation = applied(row, &mut doc, SetName::new("x3", named("Renamed")));
    assert_eq!(
        (mutation.changed_ids(), element(row, &doc, "x3").name()),
        (list(&["x3"]).as_slice(), Some(&named("Renamed"))),
        "{row}: SetName on a {class}"
    );
    // SetGid, then its removal.
    applied(row, &mut doc, SetGid::new("x3", "G-3"));
    assert_eq!(
        element(row, &doc, "x3").gid(),
        Some("G-3"),
        "{row}: SetGid on a {class}"
    );
    applied(row, &mut doc, SetGid::remove("x1"));
    assert_eq!(
        element(row, &doc, "x1").gid(),
        None,
        "{row}: SetGid::remove on a {class}"
    );
    // SetFlag: isCitation and isAbstract apply, isCounter does not.
    applied(row, &mut doc, SetFlag::new("x3", Flag::IsCitation, true));
    applied(row, &mut doc, SetFlag::new("x3", Flag::IsAbstract, true));
    assert_eq!(
        (
            element(row, &doc, "x3").is_citation(),
            element(row, &doc, "x3").is_abstract()
        ),
        (true, true),
        "{row}: SetFlag on a {class}"
    );
    let error = refused_edit(row, &mut doc, SetFlag::new("x3", Flag::IsCounter, true));
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable { element, feature, .. }
                if element.as_str() == "x3" && *feature == "isCounter"
        ),
        "{row}: isCounter on a {class} is not FeatureNotApplicable: {error:?}"
    );
    // SetReferences(CitedElement); metaClaim is no end of a {class}.
    applied(
        row,
        &mut doc,
        SetReferences::new("x3", ReferenceEnd::CitedElement, ["x2"]),
    );
    assert_eq!(
        element(row, &doc, "x3")
            .cited_element()
            .map(ElementId::as_str),
        Some("x2"),
        "{row}: SetReferences(CitedElement) on a {class}"
    );
    let error = refused_edit(
        row,
        &mut doc,
        SetReferences::new("x3", ReferenceEnd::MetaClaim, ["x2"]),
    );
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable { feature, .. } if *feature == "metaClaim"
        ),
        "{row}: metaClaim on a {class} is not FeatureNotApplicable: {error:?}"
    );
    let error = refused_edit(
        row,
        &mut doc,
        SetReferences::new("x3", ReferenceEnd::GroupMember, ["x2"]),
    );
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable { feature, .. } if *feature == "argumentElement"
        ),
        "{row}: argumentElement on a {class} is not FeatureNotApplicable: {error:?}"
    );
    // ReorderElements: of the package's children, and of the asset's own.
    applied(row, &mut doc, ReorderElements::new("a", ["x3", "x1", "x2"]));
    assert_eq!(
        children(row, &doc, "a"),
        ids(&["x3", "x1", "x2"]),
        "{row}: ReorderElements of the package holding {class}s"
    );
    applied(
        row,
        &mut doc,
        ReorderElements::new("x1", ["x1_p", "x1_t", "x1_n", "x1_d"]),
    );
    assert_eq!(
        children(row, &doc, "x1"),
        ids(&["x1_p", "x1_t", "x1_n", "x1_d"]),
        "{row}: ReorderElements of a {class}'s children"
    );
    // The edited document validates and writes back.
    let text = write_in(row, "the edited document", &doc, Mode::Strict);
    let back = read_ok(row, "the edited document, written", &text);
    assert!(
        back == doc,
        "{row}: the edited document reads back !=\n{text}"
    );
    // DeleteElement: x2 is the abstractForm of x1 and cited by x3.
    let error = refused_edit(row, &mut doc, DeleteElement::new("x2"));
    assert!(
        matches!(&error, EditError::Referenced { id, .. } if id.as_str() == "x2"),
        "{row}: deleting a referenced {class} is not Referenced: {error:?}"
    );
    // x1 holds a Property: NotEmpty; recursively, it goes with its children.
    let error = refused_edit(row, &mut doc, DeleteElement::new("x1"));
    assert!(
        matches!(&error, EditError::NotEmpty { contained: 1, .. }),
        "{row}: deleting a {class} holding a Property is not NotEmpty {{ contained: 1 }}: \
         {error:?}"
    );
    let mutation = applied(
        row,
        &mut doc,
        DeleteElement::new("x1").with_package_policy(PackageDeletePolicy::DeleteRecursively),
    );
    assert_eq!(
        mutation.deleted_ids().first().map(ElementId::as_str),
        Some("x1"),
        "{row}: a recursive delete of a {class}"
    );
    for gone in ["x1", "x1_d", "x1_n", "x1_t", "x1_p"] {
        assert!(
            !doc.contains(gone),
            "{row}: {gone} survives the delete of its {class}"
        );
    }
    let mutation = applied(row, &mut doc, DeleteElement::new("x3"));
    assert_eq!(
        (mutation.deleted_ids(), children(row, &doc, "a")),
        (list(&["x3"]).as_slice(), ids(&["x2"])),
        "{row}: deleting a {class} that nothing references"
    );
}

#[test]
fn sacm23_art_008_set_name_gid_flag_references_reorder_and_delete_apply_to_a_technique() {
    assert_the_existing_edits_apply(ART_008, "Technique");
}

#[test]
fn sacm23_art_009_set_name_gid_flag_references_reorder_and_delete_apply_to_a_participant() {
    assert_the_existing_edits_apply(ART_009, "Participant");
}
